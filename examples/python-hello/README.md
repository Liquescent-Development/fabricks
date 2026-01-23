# Python Hello World

A simple Python HTTP service demonstrating the Fabricks Python runtime.

**No WASM toolchain required!** Just write Python and run.

## Quick Start

```bash
# Build your Python service
fabricks build examples/python-hello

# Run it
fabricks run python-hello:latest
```

## How It Works

1. **Write Python** - Create a handler function in `app.py`
2. **Configure** - Point to it in your `Fabrickfile`
3. **Build & Run** - Fabricks handles the rest

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

## Configuration

The `Fabrickfile` is minimal:

```toml
[from]
source = "python"
version = "3.12"

[source]
path = "."
entrypoint = "app:handler"
```

That's it! No build commands, no WASM toolchain, no complexity.
