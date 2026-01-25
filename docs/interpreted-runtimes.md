# Interpreted Language Runtimes

Documentation for using interpreted languages (JavaScript, Python) with Fabricks.

---

## Table of Contents

- [Overview](#overview)
- [How It Works](#how-it-works)
- [JavaScript/Node.js Runtime](#javascriptnodejs-runtime)
- [Python Runtime](#python-runtime)
- [Multi-Layer OCI Architecture](#multi-layer-oci-architecture)
- [Handler Interface](#handler-interface)
- [Building and Running](#building-and-running)
- [Examples](#examples)
- [Creating Custom Runtimes](#creating-custom-runtimes)
- [Limitations](#limitations)

---

## Overview

Fabricks supports interpreted languages through **pre-built WASM runtimes** that load and execute your source code at runtime. This means you can write JavaScript or Python without needing to understand WebAssembly or compilation toolchains.

**Supported Runtimes:**

- **JavaScript/Node.js** - Uses SpiderMonkey engine via `jco componentize`
- **Python** - Uses CPython interpreter via `componentize-py`

**Key Benefits:**

- No compilation step for your source code
- Familiar development experience
- Fast iteration (no rebuild needed)
- Standard language features and syntax

---

## How It Works

Fabricks uses a **multi-layer OCI image approach** for interpreted runtimes:

```
┌─────────────────────────────────────────────────────────────┐
│                    OCI Image (2+ layers)                     │
├─────────────────────────────────────────────────────────────┤
│  Layer 0: Runtime WASM Module                                │
│  - Pre-compiled language runtime (SpiderMonkey/CPython)      │
│  - WASI HTTP handler framework                               │
│  - ~8-15MB (depending on runtime)                            │
├─────────────────────────────────────────────────────────────┤
│  Layer 1: User Source Files (tar.gz)                         │
│  - Your .js or .py files                                     │
│  - .fabricks.toml (entrypoint configuration)                 │
│  - Any other application files                               │
│  - Typically < 1MB                                           │
└─────────────────────────────────────────────────────────────┘
```

**At Runtime:**

1. **Daemon loads module:**
   - Daemon reads OCI manifest to identify runtime layer (Layer 0)
   - Loads runtime WASM module into wasmtime

2. **Source mounting:**
   - Daemon extracts source files from Layer 1+ to temporary directory
   - Mounts source directory at `/app` via WASI filesystem preopens

3. **Execution:**
   - Runtime WASM reads `/app/.fabricks.toml` to find entrypoint
   - Runtime loads and executes your source code
   - Daemon proxies HTTP requests to the runtime's WASI HTTP handler

---

## JavaScript/Node.js Runtime

### Overview

The JavaScript runtime bundles the **SpiderMonkey** JavaScript engine (used by Firefox) into a WASM component that implements the `wasi:http/incoming-handler` interface.

**Technology Stack:**
- **Engine:** SpiderMonkey (via StarlingMonkey)
- **Componentization:** `jco componentize` from Bytecode Alliance
- **Size:** ~13MB runtime WASM

### Fabrickfile Configuration

```toml
fabrick_version = "1.0"

[info]
name = "my-js-app"
version = "1.0.0"
type = "http"

[runtime]
image = "nodejs-runtime:1.0.0"      # Runtime OCI image
handler = "app.js:handler"          # Entrypoint: file:function

[source]
path = "."                          # Directory containing your .js files

[capabilities.network]
listen = [8080]
```

### Handler Interface

Your JavaScript handler receives a request object and returns a response object:

```javascript
// app.js
function handler(request) {
    // Request format:
    // {
    //     method: "GET",
    //     path: "/hello",
    //     query: { name: "world" },
    //     headers: { "content-type": "application/json" },
    //     url: "http://localhost:8080/hello?name=world"
    // }

    // Response format:
    return {
        status: 200,
        headers: { "content-type": "text/plain" },
        body: "Hello from JavaScript!"
    };
}
```

**Request Object Fields:**
- `method` (string) - HTTP method (GET, POST, etc.)
- `path` (string) - URL path
- `query` (object) - Query parameters as key-value pairs
- `headers` (object) - Request headers (lowercase keys)
- `url` (string) - Full request URL

**Response Object Fields:**
- `status` (number) - HTTP status code
- `headers` (object) - Response headers
- `body` (string|object) - Response body (objects are JSON-stringified)

### Complete Example

```javascript
// app.js
function handler(request) {
    const { method, path, query, headers } = request;

    // Route requests
    if (path === '/') {
        return {
            status: 200,
            headers: { 'content-type': 'text/plain' },
            body: 'Hello from JavaScript on Fabricks!'
        };
    }

    if (path === '/greet') {
        const name = query.name || 'World';
        return {
            status: 200,
            headers: { 'content-type': 'text/plain' },
            body: `Hello, ${name}!`
        };
    }

    if (path === '/json') {
        // Objects are automatically JSON-stringified
        return {
            status: 200,
            headers: { 'content-type': 'application/json' },
            body: {
                message: 'Hello from JavaScript!',
                timestamp: Date.now()
            }
        };
    }

    // 404 for unknown paths
    return {
        status: 404,
        headers: { 'content-type': 'text/plain' },
        body: `Not Found: ${method} ${path}`
    };
}
```

### Limitations

- **No Node.js built-ins:** This is WASI, not Node.js. No `fs`, `http`, `crypto`, etc.
- **No ES modules:** Use function declarations, not `export`
- **No async/await:** Handlers must be synchronous (for now)
- **No dynamic imports:** All code must be in loaded files
- **No npm packages:** Pure JavaScript only (unless bundled)

---

## Python Runtime

### Overview

The Python runtime bundles **CPython** (the standard Python interpreter) into a WASM component that implements `wasi:http/incoming-handler`.

**Technology Stack:**
- **Interpreter:** CPython 3.12
- **Componentization:** `componentize-py` from Bytecode Alliance
- **Size:** ~15MB runtime WASM

### Fabrickfile Configuration

```toml
fabrick_version = "1.0"

[info]
name = "my-python-app"
version = "1.0.0"
type = "http"

[runtime]
image = "python-runtime:3.12"       # Runtime OCI image
handler = "app:handler"             # Entrypoint: module:function

[source]
path = "."                          # Directory containing your .py files

[capabilities.network]
listen = [8080]
```

### Handler Interface

Your Python handler receives a dict and returns a dict:

```python
# app.py
def handler(request):
    # Request format:
    # {
    #     "method": "GET",
    #     "path": "/hello",
    #     "query": {"name": "world"},
    #     "headers": {"content-type": "application/json"}
    # }

    # Response format:
    return {
        "status": 200,
        "headers": {"content-type": "text/plain"},
        "body": "Hello from Python!"
    }
```

**Request Dict Fields:**
- `method` (str) - HTTP method
- `path` (str) - URL path
- `query` (dict) - Query parameters
- `headers` (dict) - Request headers (lowercase keys)

**Response Dict Fields:**
- `status` (int) - HTTP status code
- `headers` (dict) - Response headers
- `body` (str) - Response body

### Complete Example

```python
# app.py
import json

def handler(request):
    method = request.get("method", "GET")
    path = request.get("path", "/")
    query = request.get("query", {})
    headers = request.get("headers", {})

    # Route requests
    if path == "/" or path == "":
        return {
            "status": 200,
            "headers": {"content-type": "text/plain"},
            "body": "Hello from Python on Fabricks!"
        }

    if path == "/greet":
        name = query.get("name", "World")
        return {
            "status": 200,
            "headers": {"content-type": "text/plain"},
            "body": f"Hello, {name}!"
        }

    if path == "/json":
        data = {
            "message": "Hello from Python!",
            "service": "python-hello",
            "version": "1.0.0"
        }
        return {
            "status": 200,
            "headers": {"content-type": "application/json"},
            "body": json.dumps(data)
        }

    # 404 for unknown paths
    return {
        "status": 404,
        "headers": {"content-type": "text/plain"},
        "body": f"Not Found: {method} {path}"
    }
```

### Limitations

- **Pure Python only:** No C extensions or binary modules
- **Limited stdlib:** Only WASI-compatible standard library modules
- **No async:** Handlers must be synchronous
- **No pip packages:** Pure Python packages only (unless bundled)

---

## Multi-Layer OCI Architecture

### Why Multi-Layer?

Traditional WASM builds compile everything into a single `.wasm` file. For interpreted languages, this would mean:
- Recompiling the entire runtime + your code every time you change anything
- Large builds (runtime is 8-15MB)
- Slow iteration

The multi-layer approach separates:
1. **Runtime layer** (Layer 0) - Built once, reused
2. **Source layer** (Layer 1+) - Rebuilt on every change, tiny size

### Layer Structure

```
OCI Manifest:
{
  "layers": [
    {
      "mediaType": "application/vnd.oci.image.layer.v1.tar+gzip",
      "digest": "sha256:abc123...",
      "size": 13420532,
      "annotations": {
        "org.opencontainers.image.title": "nodejs-runtime.wasm",
        "dev.fabricks.layer.type": "runtime"
      }
    },
    {
      "mediaType": "application/vnd.oci.image.layer.v1.tar+gzip",
      "digest": "sha256:def456...",
      "size": 2048,
      "annotations": {
        "org.opencontainers.image.title": "source.tar.gz",
        "dev.fabricks.layer.type": "source"
      }
    }
  ]
}
```

### Building Multi-Layer Images

When you run `fabricks build` on a project with a `[runtime]` section:

1. **Runtime Resolution:**
   - Checks if `runtime.image` exists in local OCI storage
   - If not found, pulls from registry
   - Validates runtime implements required interfaces

2. **Source Packaging:**
   - Packages all files from `[source].path` into tar.gz
   - Creates `.fabricks.toml` with entrypoint configuration
   - Compresses source layer

3. **Manifest Creation:**
   - Creates OCI manifest with 2+ layers
   - Layer 0: Runtime WASM (from runtime.image)
   - Layer 1: Source files (newly created)
   - Stores in local OCI storage with tag

### Running Multi-Layer Images

When you run `fabricks service run` or `fabricks mortar up`:

1. **Daemon loads manifest:**
   - Reads OCI manifest from storage
   - Identifies runtime layer (Layer 0)

2. **Runtime extraction:**
   - Loads runtime WASM into memory
   - Prepares wasmtime instance

3. **Source mounting:**
   - Extracts source layer(s) to temporary directory
   - Sets up WASI preopens: `/app -> /tmp/fabricks/srv_xyz/app`
   - Runtime can now read files from `/app`

4. **Execution:**
   - Runtime reads `/app/.fabricks.toml`
   - Loads handler from specified entrypoint
   - Starts HTTP handler loop

---

## Handler Interface

### Common Patterns

#### Routing

```javascript
// JavaScript
function handler(request) {
    const routes = {
        '/': homeHandler,
        '/users': usersHandler,
        '/api/status': statusHandler
    };

    const handler = routes[request.path] || notFoundHandler;
    return handler(request);
}
```

```python
# Python
def handler(request):
    path = request.get("path", "/")

    routes = {
        "/": home_handler,
        "/users": users_handler,
        "/api/status": status_handler
    }

    route_handler = routes.get(path, not_found_handler)
    return route_handler(request)
```

#### JSON Responses

```javascript
// JavaScript - objects are auto-stringified
function handler(request) {
    return {
        status: 200,
        headers: { 'content-type': 'application/json' },
        body: { message: 'Success', data: [...] }
    };
}
```

```python
# Python - must stringify manually
import json

def handler(request):
    return {
        "status": 200,
        "headers": {"content-type": "application/json"},
        "body": json.dumps({"message": "Success", "data": [...]})
    }
```

#### Error Handling

```javascript
// JavaScript
function handler(request) {
    try {
        // ... your logic
        return { status: 200, body: 'OK' };
    } catch (error) {
        return {
            status: 500,
            headers: { 'content-type': 'text/plain' },
            body: `Error: ${error.message}`
        };
    }
}
```

```python
# Python
def handler(request):
    try:
        # ... your logic
        return {"status": 200, "body": "OK"}
    except Exception as e:
        return {
            "status": 500,
            "headers": {"content-type": "text/plain"},
            "body": f"Error: {str(e)}"
        }
```

---

## Building and Running

### Quick Start

1. **Create your handler:**

```javascript
// app.js
function handler(request) {
    return {
        status: 200,
        headers: { 'content-type': 'text/plain' },
        body: 'Hello from JavaScript!'
    };
}
```

2. **Create Fabrickfile:**

```toml
fabrick_version = "1.0"

[info]
name = "my-app"
version = "1.0.0"
type = "http"

[runtime]
image = "nodejs-runtime:1.0.0"
handler = "app.js:handler"

[source]
path = "."

[capabilities.network]
listen = [8080]
```

3. **Build and run:**

```bash
# Build (packages source, stores in OCI)
fabricks build

# Run via daemon
fabricks service run my-app:1.0.0

# Or run directly (if daemon not needed)
fabricks run .
```

### Development Workflow

```bash
# Start daemon in background
fabricksd &

# Initial build
fabricks build

# Run service
fabricks service run my-app:1.0.0 -p 8080:8080

# Make changes to app.js or app.py...

# Rebuild (only source layer changes, fast!)
fabricks build

# Restart service (daemon reloads source automatically)
fabricks service restart my-app

# Or rebuild and restart in one command
fabricks service run my-app:1.0.0 --force-recreate
```

### Using fabricks-mortar.toml

```toml
mortar_version = "1.0"

[project]
name = "my-app"

[service.api]
build = "./api"           # Directory with Fabrickfile
networks = ["public"]

environment = {
    LOG_LEVEL = "debug"
}

[service.api.replicas]
min = 2
max = 10

[network.public]
ingress = "0.0.0.0/0"
```

```bash
# Build all services
fabricks mortar build

# Start all services
fabricks mortar up

# Scale up
fabricks mortar scale api=5
```

---

## Examples

### Example Projects

The Fabricks repository includes example projects:

#### Node.js Examples

- **`examples/runtimes/nodejs/`** - The Node.js runtime itself (for maintainers)
- **`examples/nodejs-hello/`** - Simple Hello World HTTP service
  - Demonstrates basic request handling
  - Shows routing and query parameter handling
  - JSON response examples

#### Python Examples

- **`examples/runtimes/python/`** - The Python runtime itself (for maintainers)
- **`examples/python-hello/`** - Simple Hello World HTTP service
  - Demonstrates basic request handling
  - Shows routing and query parameter handling
  - JSON response examples

### Running Examples

```bash
# Node.js example
cd examples/nodejs-hello
fabricks build
fabricks service run nodejs-hello:1.0.0 -p 8089:8089

# Test it
curl http://localhost:8089/
curl http://localhost:8089/greet?name=Fabricks
curl http://localhost:8089/json

# Python example
cd examples/python-hello
fabricks build
fabricks service run python-hello:1.0.0 -p 8088:8088

# Test it
curl http://localhost:8088/
curl http://localhost:8088/greet?name=Fabricks
curl http://localhost:8088/json
```

---

## Creating Custom Runtimes

### For Advanced Users and Maintainers

You can create custom runtimes with additional features or different languages.

#### JavaScript Runtime (using jco)

```bash
# 1. Install jco
npm install -g @bytecodealliance/jco

# 2. Create handler.js that implements wasi:http/incoming-handler
# See examples/runtimes/nodejs/src/handler.js

# 3. Create WIT definition
# See examples/runtimes/nodejs/wit/world.wit

# 4. Componentize
jco componentize src/handler.js \
    --wit wit/world.wit \
    --world-name http-handler \
    --out runtime.wasm

# 5. Tag and publish
fabricks tag runtime.wasm mycompany/my-js-runtime:1.0.0
fabricks push mycompany/my-js-runtime:1.0.0
```

#### Python Runtime (using componentize-py)

```bash
# 1. Install componentize-py
pip install componentize-py

# 2. Fetch WIT dependencies
wkg wit fetch

# 3. Create handler.py that implements wasi:http/incoming-handler
# See examples/runtimes/python/src/handler.py

# 4. Componentize
componentize-py \
    -d wit \
    -w wasi:http/proxy@0.2.0 \
    componentize src/handler \
    -o runtime.wasm

# 5. Tag and publish
fabricks tag runtime.wasm mycompany/my-python-runtime:3.12
fabricks push mycompany/my-python-runtime:3.12
```

### Custom Runtime Requirements

Your runtime must:

1. **Implement WASI HTTP interfaces:**
   - `wasi:http/incoming-handler` for HTTP services
   - OR `wasi:cli/run` for command-line tools

2. **Read entrypoint from `/app/.fabricks.toml`:**
   ```toml
   entrypoint = "app:handler"  # module:function format
   ```

3. **Load and execute user code from `/app`:**
   - Add `/app` to module search path
   - Import the specified module
   - Call the specified function for each request

4. **Bridge request/response:**
   - Convert WASI HTTP request to simple dict/object
   - Convert dict/object response to WASI HTTP response

---

## Limitations

### Current Limitations

#### JavaScript Runtime

- **No Node.js APIs:** No `fs`, `http`, `crypto`, `process`, etc.
- **No npm packages:** Cannot import third-party packages (unless bundled)
- **No ES modules:** Must use function declarations
- **Synchronous only:** No `async`/`await` support yet
- **No WebAssembly import:** Cannot import other WASM modules

#### Python Runtime

- **No C extensions:** Pure Python only
- **Limited stdlib:** WASI-compatible modules only
- **No pip packages:** Cannot install from PyPI (unless pure Python and bundled)
- **Synchronous only:** No `asyncio` support yet

#### General Limitations

- **HTTP only:** Currently only supports HTTP services (type = "http")
- **Single handler:** One entrypoint function per service
- **No state persistence:** No built-in database or persistence (use external services)
- **Limited debugging:** No debugger support in WASI environment

### Workarounds

#### Using External Dependencies

Bundle dependencies into your source code:

```bash
# For JavaScript - use a bundler
npm install
npm run bundle  # outputs single file with dependencies

# For Python - vendor dependencies
pip install -t ./vendor requests
# Then import from vendor directory
```

#### Database Access

Use network capabilities to connect to external databases:

```toml
[capabilities.network]
listen = [8080]
connect = ["postgres:5432", "redis:6379"]
```

```javascript
// Use HTTP to access databases via network
// Or use WASI sockets when available
```

---

## Summary

Interpreted runtimes in Fabricks provide:

- **Easy onboarding:** Write code in familiar languages
- **Fast iteration:** No compilation step for source changes
- **Multi-layer efficiency:** Runtime layer cached, only source layer rebuilds
- **Standard interfaces:** Same handler pattern across languages
- **Production-ready:** Same security, networking, and orchestration as compiled WASM

Start building with interpreted runtimes:

```bash
# Check out the examples
cd examples/nodejs-hello
fabricks build && fabricks service run nodejs-hello:1.0.0 -p 8089:8089

cd examples/python-hello
fabricks build && fabricks service run python-hello:1.0.0 -p 8088:8088
```

For more information:
- [Fabrickfile Reference](fabrickfile-mortar-reference.md)
- [CLI Reference](cli-reference.md)
- [Examples](../examples/)
