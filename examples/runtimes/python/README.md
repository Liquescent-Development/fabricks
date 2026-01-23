# Fabricks Python Runtime

This is the official Python HTTP runtime for Fabricks. It bundles the CPython interpreter
with WASI HTTP support so users can write Python applications without any WASM toolchain.

## For End Users

**You don't need to build this yourself.** Just use it in your Fabrickfile:

```toml
[from]
source = "python"
version = "3.12"
```

Then write your Python code with a simple handler:

```python
# app.py
def handler(request):
    return {
        "status": 200,
        "headers": {"content-type": "text/plain"},
        "body": "Hello from Python!"
    }
```

## For Runtime Developers

If you want to build this runtime yourself or create a custom Python runtime:

### Prerequisites

- Python 3.10 or later
- componentize-py: `pip install componentize-py`

### Building

```bash
# Build the runtime
fabricks build examples/runtimes/python --tag fabricks.dev/runtimes/python:3.12

# Or build directly with componentize-py
cd examples/runtimes/python
componentize-py --wit-path wit --world http-handler componentize src -o python_runtime.wasm
```

### How It Works

1. **WIT Interface** (`wit/world.wit`): Defines the HTTP handler interface using WASI HTTP
2. **Python Framework** (`src/handler.py`): Bridges WASI HTTP to simple Python handlers
3. **componentize-py**: Bundles CPython + stdlib + our code into a WASM component

### Customizing

To create a custom Python runtime:

1. Fork this directory
2. Modify `src/handler.py` to add your framework code
3. Add any additional Python packages
4. Build with componentize-py

## Handler Interface

User handlers receive a simple dict and return a simple dict:

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

## Resources

- [componentize-py](https://github.com/bytecodealliance/componentize-py)
- [WASI HTTP](https://github.com/WebAssembly/wasi-http)
- [Fabricks Documentation](https://fabricks.dev/docs)
