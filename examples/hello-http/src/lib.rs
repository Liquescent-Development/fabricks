#[allow(warnings)]
mod bindings;

use bindings::exports::wasi::http::incoming_handler::Guest;
use bindings::wasi::http::types::{
    Fields, IncomingRequest, OutgoingBody, OutgoingResponse, ResponseOutparam,
};

struct Component;

impl Guest for Component {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        // Get the request path for routing
        let path = request.path_with_query().unwrap_or_default();

        // Build response based on path
        let (status, body_content) = match path.as_str() {
            "/" | "" => (200, "Hello from Fabricks!"),
            "/health" => (200, "OK"),
            _ => (404, "Not Found"),
        };

        // Create response headers
        let headers = Fields::new();

        // Create the outgoing response
        let response = OutgoingResponse::new(headers);
        response.set_status_code(status).expect("failed to set status code");

        // Get the body and write to it
        let body = response.body().expect("failed to get body");
        {
            let stream = body.write().expect("failed to get write stream");
            stream
                .blocking_write_and_flush(body_content.as_bytes())
                .expect("failed to write body");
            // stream is dropped here
        }

        // Finish the body
        OutgoingBody::finish(body, None).expect("failed to finish body");

        // Send the response
        ResponseOutparam::set(response_out, Ok(response));
    }
}

bindings::export!(Component with_types_in bindings);
