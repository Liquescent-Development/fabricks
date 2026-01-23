# Python Hello World

A simple Python HTTP service running on Fabricks.

## Overview

This example demonstrates how easy it is to run Python on Fabricks:

1. Write regular Python code
2. Specify `source = "python"` in your Fabrickfile
3. Run `fabricks build`

That's it! Fabricks handles all the WASM complexity automatically.

## Files

- `Fabrickfile` - Service configuration
- `app.py` - Your Python application

## Building

```bash
fabricks build examples/python-hello
```

## Running

```bash
fabricks run examples/python-hello
```

## Endpoints

- `GET /` - Hello message
- `GET /health` - Health check
- `GET /greet?name=Alice` - Personalized greeting
- `GET /json` - JSON response

## How It Works

When you specify `source = "python"`, Fabricks automatically:

1. Uses [componentize-py](https://github.com/bytecodealliance/componentize-py) to bundle your Python code with the CPython interpreter
2. Creates a WASM component that implements the HTTP handler interface
3. Stores the result as an OCI artifact

You never need to learn about WASM toolchains - just write Python!
