# Node.js Hello World

A simple JavaScript HTTP service running on Fabricks. No WebAssembly knowledge required!

## Quick Start

```bash
# Build the service
fabricks build

# Run it
fabricks run nodejs-hello:1.0.0

# Test it
curl http://localhost:8088/
curl http://localhost:8088/health
curl http://localhost:8088/greet?name=Developer
curl http://localhost:8088/json
```

## How It Works

1. Write your JavaScript handler in `app.js`
2. Configure your service in `Fabrickfile`
3. Run `fabricks build` - Fabricks packages your code with the JavaScript runtime
4. Run `fabricks run` - Your service is now running!

## Handler Interface

Your handler receives a request object and returns a response object:

```javascript
function handler(request) {
    // request contains:
    // - method: "GET", "POST", etc.
    // - path: "/hello"
    // - query: { name: "world" }
    // - headers: { "content-type": "..." }

    return {
        status: 200,
        headers: { "content-type": "text/plain" },
        body: "Hello!"
    };
}
```

## Endpoints

This example provides:

| Endpoint | Description |
|----------|-------------|
| `GET /` | Returns "Hello from JavaScript on Fabricks!" |
| `GET /health` | Health check endpoint |
| `GET /greet?name=X` | Returns personalized greeting |
| `GET /json` | Returns JSON response |

## Files

- `app.js` - Your JavaScript handler code
- `Fabrickfile` - Service configuration

## Requirements

No local toolchain required! Fabricks handles everything.

The JavaScript runtime uses SpiderMonkey (Firefox's JS engine) compiled to WebAssembly.

## Multi-Layer OCI Architecture

This example uses Fabricks' **multi-layer OCI approach** for interpreted runtimes:

- **Layer 0:** Pre-built Node.js runtime WASM (~13MB) - SpiderMonkey engine + WASI HTTP framework
- **Layer 1:** Your source code (app.js) - packaged as tar.gz, mounted at `/app` at runtime

Benefits:
- Runtime built once, reused across all JS projects
- Only source layer rebuilds when you change code (fast iteration)
- No compilation step for your JavaScript!

## Learn More

- **[Interpreted Runtimes Documentation](../../docs/interpreted-runtimes.md)** - Complete guide to JS/Python runtimes
- **[Fabrickfile Reference](../../docs/fabrickfile-mortar-reference.md#runtime-optional)** - Runtime configuration options
- **[CLI Reference](../../docs/cli-reference.md#fabricks-service-run)** - Running services via daemon
- **[JavaScript Runtime Details](../runtimes/nodejs/)** - Runtime implementation (for maintainers)
