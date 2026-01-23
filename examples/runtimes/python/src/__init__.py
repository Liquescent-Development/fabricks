"""
Fabricks Python HTTP Runtime

This module provides the HTTP handler framework for Python applications on Fabricks.
It implements the WASI HTTP incoming-handler interface.

When users write Python apps for Fabricks, they implement a simple handler function:

    def handler(request):
        return {
            "status": 200,
            "headers": {"content-type": "text/plain"},
            "body": "Hello, World!"
        }

The runtime handles all the WASI HTTP complexity.
"""

from .handler import IncomingHandler

__all__ = ["IncomingHandler"]
