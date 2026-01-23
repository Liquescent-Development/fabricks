"""
Python Hello World HTTP Handler

This is a simple HTTP handler that demonstrates Python on Fabricks.
Just write regular Python - no WASM knowledge required!
"""


def handler(request):
    """
    Handle incoming HTTP requests.

    Args:
        request: HTTP request object with method, path, headers, body

    Returns:
        HTTP response dict with status, headers, body
    """
    path = request.get("path", "/")
    method = request.get("method", "GET")

    # Route requests
    if path == "/" or path == "":
        return {
            "status": 200,
            "headers": {"content-type": "text/plain"},
            "body": "Hello from Python on Fabricks!",
        }

    elif path == "/health":
        return {
            "status": 200,
            "headers": {"content-type": "text/plain"},
            "body": "OK",
        }

    elif path == "/greet":
        # Get name from query string
        query = request.get("query", {})
        name = query.get("name", "World")
        return {
            "status": 200,
            "headers": {"content-type": "text/plain"},
            "body": f"Hello, {name}!",
        }

    elif path == "/json":
        import json

        data = {
            "message": "Hello from Python!",
            "service": "python-hello",
            "version": "1.0.0",
        }
        return {
            "status": 200,
            "headers": {"content-type": "application/json"},
            "body": json.dumps(data),
        }

    else:
        return {
            "status": 404,
            "headers": {"content-type": "text/plain"},
            "body": f"Not Found: {method} {path}",
        }
