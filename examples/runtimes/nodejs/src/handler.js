/**
 * Fabricks Node.js Runtime - WASI HTTP Handler
 *
 * This runtime implements the wasi:http/incoming-handler interface and loads
 * user JavaScript code from /app at runtime via WASI filesystem preopens.
 */

// Import WASI HTTP types
import {
    Fields,
    OutgoingResponse,
    OutgoingBody,
    ResponseOutparam,
} from 'wasi:http/types@0.2.0';

// Import WASI filesystem types for loading user code
import { getDirectories } from 'wasi:filesystem/preopens@0.2.0';
import { Descriptor } from 'wasi:filesystem/types@0.2.0';

// Configuration
const APP_DIR = '/app';
const DEFAULT_MODULE = 'app';
const DEFAULT_HANDLER = 'handler';

// Debug info collected during handler loading
let debugInfo = [];

// User handler - loaded lazily on first request
let userHandler = null;
let handlerLoaded = false;

/**
 * Find the /app directory descriptor from preopens.
 */
function findAppDirectory() {
    try {
        const directories = getDirectories();
        debugInfo.push(`Found ${directories.length} preopened directories`);
        for (const [descriptor, path] of directories) {
            debugInfo.push(`  Preopen: "${path}"`);
            if (path === APP_DIR || path === APP_DIR + '/') {
                debugInfo.push(`  -> Matched /app!`);
                return descriptor;
            }
        }
        debugInfo.push(`No /app directory found in preopens`);
    } catch (e) {
        debugInfo.push(`Error getting directories: ${e}`);
        debugInfo.push(`Error stack: ${e.stack || 'N/A'}`);
    }
    return null;
}

/**
 * Read a file from a descriptor.
 */
function readFile(baseDescriptor, filePath) {
    debugInfo.push(`readFile: trying to open "${filePath}"`);

    // Open the file for reading
    let result;
    try {
        result = baseDescriptor.openAt(
            { symlinkFollow: true },  // path-flags
            filePath,                  // relative path
            {},                        // open-flags (none needed for read)
            { read: true }             // descriptor-flags
        );
    } catch (e) {
        debugInfo.push(`readFile: openAt threw: ${e}`);
        return null;
    }

    debugInfo.push(`readFile: openAt returned tag=${result?.tag}, val type=${typeof result?.val}`);

    // Check for error result
    if (result === null || result === undefined) {
        debugInfo.push(`readFile: openAt returned null/undefined`);
        return null;
    }

    // Handle result type (could be {tag:'ok', val:...} or {tag:'err', val:...})
    let fileDescriptor;
    if (result.tag === 'err') {
        debugInfo.push(`readFile: openAt error: ${result.val}`);
        return null;
    } else if (result.tag === 'ok') {
        fileDescriptor = result.val;
    } else {
        // Maybe it returns the descriptor directly?
        fileDescriptor = result;
    }

    if (!fileDescriptor) {
        debugInfo.push(`readFile: no fileDescriptor after openAt`);
        return null;
    }

    debugInfo.push(`readFile: got fileDescriptor, calling stat...`);

    // Get file size via stat
    let statResult;
    try {
        statResult = fileDescriptor.stat();
    } catch (e) {
        debugInfo.push(`readFile: stat threw: ${e}`);
        return null;
    }

    debugInfo.push(`readFile: stat returned tag=${statResult?.tag}`);

    let fileSize;
    if (statResult.tag === 'err') {
        debugInfo.push(`readFile: stat error`);
        return null;
    } else if (statResult.tag === 'ok') {
        fileSize = statResult.val.size;
    } else {
        fileSize = statResult.size;
    }

    debugInfo.push(`readFile: fileSize = ${fileSize}`);

    // Read entire file
    let readResult;
    try {
        readResult = fileDescriptor.read(fileSize, BigInt(0));
    } catch (e) {
        debugInfo.push(`readFile: read threw: ${e}`);
        return null;
    }

    debugInfo.push(`readFile: read returned`);

    let bytes;
    if (readResult.tag === 'err') {
        debugInfo.push(`readFile: read error`);
        return null;
    } else if (readResult.tag === 'ok') {
        [bytes] = readResult.val;
    } else if (Array.isArray(readResult)) {
        [bytes] = readResult;
    } else {
        bytes = readResult;
    }

    const text = new TextDecoder().decode(new Uint8Array(bytes));
    debugInfo.push(`readFile: successfully read ${text.length} chars`);
    return text;
}

/**
 * Check if a file exists.
 */
function fileExists(baseDescriptor, filePath) {
    debugInfo.push(`fileExists: checking "${filePath}"`);
    try {
        // In jco, WASI functions return the result directly when successful
        // and throw exceptions on error
        baseDescriptor.statAt(
            { symlinkFollow: true },
            filePath
        );
        debugInfo.push(`fileExists: statAt succeeded`);
        return true;
    } catch (e) {
        debugInfo.push(`fileExists: statAt threw: ${e.message || e}`);
        return false;
    }
}

/**
 * Load entrypoint configuration from /app/.fabricks.toml.
 */
function loadEntrypoint(appDir) {
    let moduleName = DEFAULT_MODULE;
    let handlerName = DEFAULT_HANDLER;

    const configContent = readFile(appDir, '.fabricks.toml');
    if (configContent) {
        // Simple TOML parsing for entrypoint
        for (const line of configContent.split('\n')) {
            const trimmed = line.trim();
            if (trimmed.startsWith('entrypoint')) {
                const match = trimmed.match(/entrypoint\s*=\s*["']([^"']+)["']/);
                if (match) {
                    const value = match[1];
                    if (value.includes(':')) {
                        [moduleName, handlerName] = value.split(':', 2);
                    } else {
                        moduleName = value;
                    }
                }
            }
        }
    }

    return { moduleName, handlerName };
}

/**
 * Dynamically load the user's handler function.
 */
function loadUserHandler() {
    const appDir = findAppDirectory();
    if (!appDir) {
        console.log('No /app directory found in preopens');
        return null;
    }

    const { moduleName, handlerName } = loadEntrypoint(appDir);

    // Try to find the module file
    let modulePath = `${moduleName}.js`;
    if (!fileExists(appDir, modulePath)) {
        // Try package style
        modulePath = `${moduleName}/index.js`;
        if (!fileExists(appDir, modulePath)) {
            console.log(`User module not found: ${moduleName}.js`);
            return null;
        }
    }

    // Read the module code
    const code = readFile(appDir, modulePath);
    if (!code) {
        console.log(`Failed to read module: ${modulePath}`);
        return null;
    }

    // Execute the code and extract the handler
    // We use Function constructor to create a module-like scope
    try {
        // Create a namespace object for the module
        const moduleExports = {};
        const moduleScope = {
            exports: moduleExports,
            module: { exports: moduleExports },
            console: console,
            JSON: JSON,
            parseInt: parseInt,
            parseFloat: parseFloat,
            encodeURIComponent: encodeURIComponent,
            decodeURIComponent: decodeURIComponent,
        };

        // Wrap code to capture function declarations
        const wrappedCode = `
            ${code}
            if (typeof ${handlerName} === 'function') {
                return ${handlerName};
            }
            return null;
        `;

        // eslint-disable-next-line no-new-func
        const factory = new Function('exports', 'module', 'console', 'JSON', 'parseInt', 'parseFloat', 'encodeURIComponent', 'decodeURIComponent', wrappedCode);
        const handler = factory(
            moduleScope.exports,
            moduleScope.module,
            moduleScope.console,
            moduleScope.JSON,
            moduleScope.parseInt,
            moduleScope.parseFloat,
            moduleScope.encodeURIComponent,
            moduleScope.decodeURIComponent
        );

        if (handler && typeof handler === 'function') {
            console.log(`Loaded handler '${handlerName}' from ${modulePath}`);
            return handler;
        }

        console.log(`Handler '${handlerName}' not found in module`);
        return null;
    } catch (e) {
        console.error(`Error loading user handler: ${e}`);
        return null;
    }
}

/**
 * Ensure the user handler is loaded.
 */
function ensureHandlerLoaded() {
    if (!handlerLoaded) {
        userHandler = loadUserHandler();
        handlerLoaded = true;
    }
}

/**
 * Default handler when no user handler is found.
 */
function defaultHandler(request) {
    const path = request.path || '/';

    if (path === '/' || path === '') {
        return {
            status: 200,
            headers: { 'content-type': 'text/plain' },
            body: `Fabricks Node.js Runtime v20

No user handler found.

Debug info:
${debugInfo.join('\n')}

Create /app/app.js with a handler function:

    function handler(request) {
        return {
            status: 200,
            body: "Hello!"
        };
    }
`,
        };
    }

    if (path === '/health') {
        return {
            status: 200,
            headers: { 'content-type': 'text/plain' },
            body: 'OK',
        };
    }

    return {
        status: 404,
        headers: { 'content-type': 'text/plain' },
        body: `Not Found: ${path}`,
    };
}

/**
 * Convert WASI HTTP method variant to string.
 */
function methodToString(methodVariant) {
    // In jco, WIT variants are represented as { tag: 'variant_name', val?: value }
    if (typeof methodVariant === 'string') {
        return methodVariant.toUpperCase();
    }
    if (methodVariant && typeof methodVariant === 'object') {
        const tag = methodVariant.tag;
        if (tag === 'other') {
            return String(methodVariant.val || 'OTHER').toUpperCase();
        }
        return String(tag || 'UNKNOWN').toUpperCase();
    }
    return 'UNKNOWN';
}

/**
 * Convert WASI HTTP IncomingRequest to simple object format.
 */
function convertRequest(request) {
    const pathWithQuery = request.pathWithQuery() || '/';

    let path = pathWithQuery;
    const query = {};

    const queryIndex = pathWithQuery.indexOf('?');
    if (queryIndex !== -1) {
        path = pathWithQuery.substring(0, queryIndex);
        const queryString = pathWithQuery.substring(queryIndex + 1);
        for (const pair of queryString.split('&')) {
            const eqIndex = pair.indexOf('=');
            if (eqIndex !== -1) {
                const key = pair.substring(0, eqIndex);
                const value = decodeURIComponent(pair.substring(eqIndex + 1).replace(/\+/g, ' '));
                query[key] = value;
            }
        }
    }

    const method = methodToString(request.method());

    return {
        method,
        path,
        query,
        headers: {},
    };
}

/**
 * Send response using WASI HTTP types.
 */
function sendResponse(response, responseOutparam) {
    const status = response.status || 200;
    const headersDict = response.headers || {};
    let body = response.body || '';

    if (typeof body === 'object') {
        body = JSON.stringify(body);
    }

    const bodyBytes = new Uint8Array(new TextEncoder().encode(body));

    // Build headers list
    const headersList = [];
    for (const [name, value] of Object.entries(headersDict)) {
        const valueBytes = new Uint8Array(new TextEncoder().encode(String(value)));
        headersList.push([name, valueBytes]);
    }

    // Create response with headers
    const fields = Fields.fromList(headersList);
    const outgoingResponse = new OutgoingResponse(fields);
    outgoingResponse.setStatusCode(status);

    // Write body
    const outgoingBody = outgoingResponse.body();
    const outputStream = outgoingBody.write();
    outputStream.blockingWriteAndFlush(bodyBytes);
    outputStream[Symbol.dispose]();

    // Finish
    OutgoingBody.finish(outgoingBody, undefined);
    ResponseOutparam.set(responseOutparam, { tag: 'ok', val: outgoingResponse });
}

/**
 * WASI HTTP incoming-handler implementation.
 */
export const incomingHandler = {
    handle(request, responseOutparam) {
        // First, try to load the user handler and capture any errors
        let loadError = null;
        try {
            ensureHandlerLoaded();
        } catch (e) {
            loadError = e;
        }

        const simpleRequest = convertRequest(request);

        let response;
        try {
            // If there was a load error, report it
            if (loadError) {
                response = {
                    status: 500,
                    headers: { 'content-type': 'text/plain' },
                    body: `User handler load error: ${loadError.message || loadError}\n\nStack: ${loadError.stack || 'N/A'}`,
                };
            } else if (userHandler) {
                response = userHandler(simpleRequest);
            } else {
                response = defaultHandler(simpleRequest);
            }
        } catch (e) {
            response = {
                status: 500,
                headers: { 'content-type': 'text/plain' },
                body: `Handler error: ${e.message || e}\n\nStack: ${e.stack || 'N/A'}`,
            };
        }

        sendResponse(response, responseOutparam);
    }
};
