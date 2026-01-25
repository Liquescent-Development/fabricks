# Fabricks Node.js Runtime

This is the official Fabricks runtime for JavaScript applications. It bundles the SpiderMonkey JavaScript engine (via StarlingMonkey) with a WASI HTTP handler framework.

## Overview

The Node.js runtime allows users to write simple JavaScript HTTP handlers without needing to understand WebAssembly or the Component Model. The runtime:

1. Implements `wasi:http/incoming-handler` for HTTP request handling
2. Loads user JavaScript code from `/app` at runtime via WASI filesystem preopens
3. Bridges HTTP requests to a simple function-based interface

## User Experience

End users don't build this runtime directly. Instead, they use it via their Fabrickfile:

```toml
[from]
source = "javascript"
version = "20"

[source]
path = "."
entrypoint = "app:handler"
```

Then write a simple handler function:

```javascript
// app.js
function handler(request) {
    return {
        status: 200,
        headers: { "content-type": "text/plain" },
        body: "Hello from JavaScript!"
    };
}
```

## Handler Interface

### Request Object

The handler receives a request object with:

```javascript
{
    method: "GET",           // HTTP method
    path: "/hello",          // URL path
    query: { name: "world" }, // Query parameters as object
    headers: { ... },        // Request headers (lowercase keys)
    url: "http://..."        // Full URL
}
```

### Response Object

The handler returns a response object:

```javascript
{
    status: 200,                              // HTTP status code
    headers: { "content-type": "text/plain" }, // Response headers
    body: "Hello!"                            // Response body (string or object)
}
```

If body is an object, it will be JSON-stringified automatically.

## Building the Runtime

This is for Fabricks maintainers only. End users don't need to do this.

### Requirements

- Node.js 18+
- jco: `npm install -g @bytecodealliance/jco`

### Build Command

```bash
# From the repository root
fabricks build examples/runtimes/nodejs --tag fabricks.dev/runtimes/javascript:20

# Or manually with jco
cd examples/runtimes/nodejs
npx jco componentize src/handler.js \
    --wit wit/world.wit \
    --world-name http-handler \
    --out nodejs-runtime.wasm
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                  OCI Image (2 layers)                       │
├─────────────────────────────────────────────────────────────┤
│  Layer 0: nodejs-runtime.wasm                               │
│  - SpiderMonkey JS engine                                   │
│  - WASI HTTP handler framework                              │
│  - ~8MB (SpiderMonkey embedding)                            │
├─────────────────────────────────────────────────────────────┤
│  Layer 1: User source files (tar.gz)                        │
│  - app.js (user's handler)                                  │
│  - .fabricks.toml (entrypoint config)                       │
│  - Any other JS files                                       │
└─────────────────────────────────────────────────────────────┘

At runtime:
1. wasmtime loads nodejs-runtime.wasm
2. User files are mounted at /app via WASI preopens
3. Runtime reads /app/.fabricks.toml for entrypoint
4. Runtime evaluates user's JavaScript code
5. HTTP requests are routed to user's handler function
```

## Files

- `src/handler.js` - WASI HTTP handler that loads user code from /app
- `wit/world.wit` - WIT interface definition (wasi:http/proxy)
- `Fabrickfile` - Build configuration for the runtime

## Limitations

- User code uses `function` declarations (not ES module exports)
- Dynamic imports are not supported
- Node.js built-in modules are not available (this is WASI, not Node.js)
- Async handlers are not yet supported

## Related

- [Python Runtime](../python/) - Similar runtime for Python
- [nodejs-hello Example](../../nodejs-hello/) - User-facing example
