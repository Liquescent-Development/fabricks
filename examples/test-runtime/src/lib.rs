//! Test Runtime
//!
//! A minimal WASM component that serves as a base image for testing
//! multi-layer OCI composition. It exports a simple utility interface
//! and handles HTTP requests.

#[allow(warnings)]
mod bindings;

use bindings::exports::fabricks::test_runtime::utils::Guest as UtilsGuest;
use bindings::exports::wasi::http::incoming_handler::Guest as HttpGuest;
use bindings::wasi::http::types::{
    Fields, IncomingRequest, OutgoingBody, OutgoingResponse, ResponseOutparam,
};

/// The runtime component implementation.
struct Component;

/// Implementation of the utils interface exported by the runtime.
impl UtilsGuest for Component {
    fn greet(name: String) -> String {
        format!("Hello, {name}! Greetings from test-runtime.")
    }

    fn version() -> String {
        "1.0.0".to_string()
    }
}

/// Implementation of the HTTP handler.
impl HttpGuest for Component {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        let path = request.path_with_query().unwrap_or_default();

        let (status, body_content) = match path.as_str() {
            "/" | "" => (200, "Test Runtime v1.0.0 - Ready"),
            "/health" => (200, "OK"),
            "/version" => (200, "1.0.0"),
            _ => (404, "Not Found"),
        };

        let headers = Fields::new();
        let response = OutgoingResponse::new(headers);
        response
            .set_status_code(status)
            .expect("failed to set status code");

        let body = response.body().expect("failed to get body");
        {
            let stream = body.write().expect("failed to get write stream");
            stream
                .blocking_write_and_flush(body_content.as_bytes())
                .expect("failed to write body");
        }

        OutgoingBody::finish(body, None).expect("failed to finish body");
        ResponseOutparam::set(response_out, Ok(response));
    }
}

bindings::export!(Component with_types_in bindings);
