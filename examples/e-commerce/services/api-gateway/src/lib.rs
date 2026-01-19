//! API Gateway Service
//!
//! Entry point for the e-commerce platform.
//! Routes requests to appropriate backend services.
//!
//! Endpoints:
//! - GET /health - Health check
//! - GET /api/products - Product catalog info
//! - GET /api/cart - Cart service info
//! - GET /api/orders - Order service info
//! - GET /api/users - User service info

#[allow(warnings)]
mod bindings;

use bindings::exports::wasi::http::incoming_handler::Guest;
use bindings::wasi::http::types::{
    Fields, IncomingRequest, Method, OutgoingBody, OutgoingResponse, ResponseOutparam,
};

struct Component;

impl Guest for Component {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        let method = request.method();
        let path = request.path_with_query().unwrap_or_default();

        let (status, body) = match (&method, path.as_str()) {
            (Method::Get, "/health") => {
                (200, r#"{"status":"ok","service":"api-gateway"}"#)
            }
            (Method::Get, "/" | "") => {
                (200, r#"{"service":"e-commerce-api-gateway","version":"1.0.0","endpoints":["/api/products","/api/cart","/api/orders","/api/users"]}"#)
            }
            (Method::Get, "/api/products") => {
                (200, r#"{"service":"product","description":"Product catalog - would route to product:8081","endpoints":["GET /","GET /{id}"]}"#)
            }
            (Method::Get, "/api/cart") => {
                (200, r#"{"service":"cart","description":"Shopping cart - would route to cart:8082","endpoints":["GET /{user_id}","POST /{user_id}/items","DELETE /{user_id}/items/{product_id}"]}"#)
            }
            (Method::Get, "/api/orders") => {
                (200, r#"{"service":"order","description":"Order management - would route to order:8083","endpoints":["GET /","GET /{id}","POST /"]}"#)
            }
            (Method::Get, "/api/users") => {
                (200, r#"{"service":"user","description":"User management - would route to user:8084","endpoints":["GET /{id}","POST /"]}"#)
            }
            _ => {
                (404, r#"{"error":"Not found"}"#)
            }
        };

        send_response(response_out, status, body);
    }
}

fn send_response(response_out: ResponseOutparam, status: u16, body: &str) {
    let headers = Fields::from_list(&[
        ("content-type".to_string(), b"application/json".to_vec()),
    ]).expect("failed to create headers");

    let response = OutgoingResponse::new(headers);
    response.set_status_code(status).expect("failed to set status");

    let outgoing_body = response.body().expect("failed to get body");
    {
        let stream = outgoing_body.write().expect("failed to get write stream");
        stream.blocking_write_and_flush(body.as_bytes()).expect("failed to write body");
    }

    OutgoingBody::finish(outgoing_body, None).expect("failed to finish body");
    ResponseOutparam::set(response_out, Ok(response));
}

bindings::export!(Component with_types_in bindings);
