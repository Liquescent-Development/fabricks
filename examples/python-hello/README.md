# Python Hello World

A simple Python HTTP service demonstrating the Fabricks Python runtime.

**No WASM toolchain required!** Just write Python and run through the Fabricks daemon.

## Quick Start

```bash
# 1. Start the daemon (if not already running)
fabricksd &

# 2. Build your Python service
fabricks build examples/python-hello

# 3. Run it through the daemon
fabricks run python-hello:1.0.0
# Output: Service 'python-hello' started with ID: svc-xxxxxxxx
```

By default, services are **internal only** for security. To allow external HTTP access:

```bash
# 4. Create a network with external access
fabricks network create public
# Output: Created network 'public' (net-xxxxxxxx)

# 5. Add your service to the network
fabricks network join public python-hello

# 6. Test it!
curl http://localhost:8088/
# Output: Hello from Python on Fabricks!
```

## Architecture

All execution goes through the Fabricks daemon, which provides:

- **Capability-based security** - Services can only access explicitly granted resources
- **Network isolation** - Services are internal by default, must be explicitly exposed
- **HTTP proxying** - The daemon routes HTTP requests to your WASM service

```
┌──────────────┐    ┌─────────────────────────────────────────────┐
│   curl       │───▶│  fabricksd (daemon)                         │
│   browser    │    │  ├─ Network security (public/internal)      │
│   etc.       │    │  ├─ HTTP proxy                              │
└──────────────┘    │  └─ WASM runtime                            │
                    │       └─ Python runtime + your code         │
                    └─────────────────────────────────────────────┘
```

## How It Works

1. **Write Python** - Create a handler function in `app.py`
2. **Configure** - Point to it in your `Fabrickfile`
3. **Build** - Fabricks packages your code with the Python runtime
4. **Run** - The daemon loads and executes your service

## Files

- `app.py` - Your Python HTTP handler
- `Fabrickfile` - Simple configuration

## Handler Interface

Your handler receives a request dict and returns a response dict:

```python
def handler(request):
    # Request contains:
    # - method: "GET", "POST", etc.
    # - path: "/hello"
    # - query: {"name": "World"}
    # - headers: {"content-type": "..."}

    return {
        "status": 200,
        "headers": {"content-type": "text/plain"},
        "body": "Hello, World!"
    }
```

## Endpoints

This example provides:

- `GET /` - Returns "Hello from Python on Fabricks!"
- `GET /health` - Health check endpoint
- `GET /greet?name=Alice` - Personalized greeting
- `GET /json` - JSON response example

```bash
# Test all endpoints (after adding to public network)
curl http://localhost:8088/
curl http://localhost:8088/health
curl "http://localhost:8088/greet?name=Alice"
curl http://localhost:8088/json
```

## Configuration

The `Fabrickfile` is minimal:

```toml
fabrick_version = "1.0"

[info]
name = "python-hello"
version = "1.0.0"
type = "http"

[from]
source = "python"
version = "3.12"

[source]
path = "."
entrypoint = "app:handler"

[capabilities.network]
listen = [8088]
```

That's it! No build commands, no WASM toolchain, no complexity.

## Network Security

Services in Fabricks are **internal by default**. This means:

- Other Fabricks services can call them
- External HTTP requests are blocked

To expose a service externally:

1. Create a network with external access: `fabricks network create public`
2. Add your service to that network: `fabricks network join public python-hello`

This follows the principle of least privilege - services must explicitly opt-in to external exposure.

## Service Management

```bash
# List all services
fabricks service ls

# Get service details
fabricks service inspect python-hello

# Stop service
fabricks service stop python-hello

# Remove service
fabricks service rm python-hello

# List networks
fabricks network ls

# Remove service from network
fabricks network leave public python-hello
```

## Multi-Layer OCI Architecture

This example uses Fabricks' **multi-layer OCI approach** for interpreted runtimes:

- **Layer 0:** Pre-built Python runtime WASM (~15MB) - CPython 3.12 + WASI HTTP framework
- **Layer 1:** Your source code (app.py) - packaged as tar.gz, mounted at `/app` at runtime

Benefits:
- Runtime built once, reused across all Python projects
- Only source layer rebuilds when you change code (fast iteration)
- No compilation step for your Python code!

## Learn More

- **[Interpreted Runtimes Documentation](../../docs/interpreted-runtimes.md)** - Complete guide to JS/Python runtimes
- **[Fabrickfile Reference](../../docs/fabrickfile-mortar-reference.md#runtime-optional)** - Runtime configuration options
- **[CLI Reference](../../docs/cli-reference.md#fabricks-service-run)** - Running services via daemon
- **[Python Runtime Details](../runtimes/python/)** - Runtime implementation (for maintainers)