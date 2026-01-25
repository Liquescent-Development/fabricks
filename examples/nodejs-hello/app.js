/**
 * Node.js Hello World HTTP Handler
 *
 * This is a simple HTTP handler that demonstrates JavaScript on Fabricks.
 * Just write regular JavaScript - no WASM knowledge required!
 */

function handler(request) {
    const path = request.path || '/';
    const method = request.method || 'GET';

    // Route requests
    if (path === '/' || path === '') {
        return {
            status: 200,
            headers: { 'content-type': 'text/plain' },
            body: 'Hello from JavaScript on Fabricks!',
        };
    }

    if (path === '/health') {
        return {
            status: 200,
            headers: { 'content-type': 'text/plain' },
            body: 'OK',
        };
    }

    if (path === '/greet') {
        // Get name from query string
        const query = request.query || {};
        const name = query.name || 'World';
        return {
            status: 200,
            headers: { 'content-type': 'text/plain' },
            body: `Hello, ${name}!`,
        };
    }

    if (path === '/json') {
        const data = {
            message: 'Hello from JavaScript!',
            service: 'nodejs-hello',
            version: '1.0.0',
        };
        return {
            status: 200,
            headers: { 'content-type': 'application/json' },
            body: JSON.stringify(data),
        };
    }

    // 404 for unknown paths
    return {
        status: 404,
        headers: { 'content-type': 'text/plain' },
        body: `Not Found: ${method} ${path}`,
    };
}
