# Fabricks Python Runtime

This is the official Python HTTP runtime for Fabricks. It provides a pre-built Python interpreter (CPython) as a WASM component that loads and executes user Python code at runtime.

## For End Users

**You don't need to build this yourself.** The runtime is pre-built and available from the Fabricks registry.

Just write your Python code:

```python
# app.py
def handler(request):
    return {
        "status": 200,
        "headers": {"content-type": "text/plain"},
        "body": "Hello from Python!"
    }
```

And create a simple Fabrickfile:

```toml
fabrick_version = "1.0"

[info]
name = "my-python-app"
version = "1.0.0"
type = "http"

[from]
source = "python"
version = "3.12"

[source]
path = "."
entrypoint = "app:handler"

[capabilities.network]
listen = [8080]
```

Then build and run:

```bash
# Start daemon
fabricksd &

# Build and run
fabricks build
fabricks run my-python-app:1.0.0

# Expose externally (services are internal by default)
fabricks network create public
fabricks network join public my-python-app

# Test
curl http://localhost:8080/
```

**No Python toolchain, no WASM knowledge required!**

## How It Works

```
┌─────────────────────────────────────────────┐
│  Your Python Code (app.py)                  │
│  - Just regular Python                      │
│  - No WASM compilation needed               │
└─────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────┐
│  fabricks build                             │
│  1. Pulls pre-built Python runtime          │
│  2. Packages your .py files as a layer      │
│  3. Creates multi-layer OCI image           │
└─────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────┐
│  OCI Image (stored locally)                 │
│  Layer 0: python_runtime.wasm               │
│  Layer 1: your source files (tar+gzip)      │
└─────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────┐
│  fabricks run (via daemon)                  │
│  1. Daemon loads module from storage        │
│  2. Extracts source layer to temp dir       │
│  3. Mounts your code at /app                │
│  4. Runs Python runtime WASM                │
│  5. Runtime imports your handler            │
│  6. Daemon proxies HTTP requests            │
└─────────────────────────────────────────────┘
```

## Handler Interface

Your handler receives a simple dict and returns a simple dict:

```python
def handler(request):
    # Request format:
    # {
    #     "method": "GET",
    #     "path": "/hello",
    #     "query": {"name": "World"},
    #     "headers": {"content-type": "application/json"}
    # }

    # Response format:
    return {
        "status": 200,
        "headers": {"content-type": "text/plain"},
        "body": "Hello!"
    }
```

## Network Security

All services run through the Fabricks daemon, which enforces:

- **Internal by default** - Services can only be called by other Fabricks services
- **Explicit external access** - Must join a network with external access to receive external HTTP

```bash
# Create service (internal only)
fabricks run my-app:1.0.0

# Create public network and expose service
fabricks network create public
fabricks network join public my-app

# Now external curl works
curl http://localhost:8080/
```

## Building on Top of Images

You can create intermediate images and build on top of them, just like Docker:

```
fabricks.dev/runtimes/python:3.12       (base runtime)
        ↓
mycompany/flask-base:1.0                (adds Flask + framework)
        ↓
mycompany/my-app:1.0                    (adds your app code)
```

Each layer stacks, with later layers able to override files from earlier ones.

---

## For Runtime Developers

If you want to build this runtime yourself or create a custom Python runtime:

### Prerequisites

- Python 3.10 or later
- componentize-py: `pip install componentize-py`
- wkg (for WIT dependencies): `cargo install wkg`

### Building

```bash
# Fetch WIT dependencies
cd examples/runtimes/python
wkg wit fetch

# Build the runtime
componentize-py -d wit -w wasi:http/proxy@0.2.0 componentize src/handler -o python_runtime.wasm

# Tag and store in local registry
fabricks build examples/runtimes/python --tag fabricks.dev/runtimes/python:3.12
```

### How the Runtime Works

1. **WIT Interface** (`wit/`): Defines the HTTP handler interface using WASI HTTP
2. **Python Handler** (`src/handler.py`):
   - Implements `wasi:http/incoming-handler`
   - Adds `/app` to `sys.path`
   - Dynamically imports user code from `/app`
   - Bridges WASI HTTP to simple Python dicts
3. **componentize-py**: Bundles CPython + stdlib + handler into a WASM component

### Customizing

To create a custom Python runtime:

1. Fork this directory
2. Modify `src/handler.py` to add your framework code
3. Add any additional Python packages to be bundled
4. Build with componentize-py
5. Publish to your registry

## File Structure

```
examples/runtimes/python/
├── Fabrickfile           # Build configuration (for runtime developers)
├── README.md             # This file
├── wit/                  # WIT interface definitions
│   ├── world.wit         # HTTP handler world
│   └── deps/             # WASI dependencies (fetched by wkg)
└── src/
    ├── __init__.py
    └── handler.py        # Runtime implementation
```

## Resources

- [componentize-py](https://github.com/bytecodealliance/componentize-py)
- [WASI HTTP](https://github.com/WebAssembly/wasi-http)
- [Fabricks Documentation](https://fabricks.dev/docs)
