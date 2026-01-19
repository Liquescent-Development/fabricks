//! REST API Service
//!
//! A simple REST API demonstrating HTTP service patterns in Fabricks.
//! Uses in-memory storage for items.
//!
//! Endpoints:
//! - GET  /health       - Health check
//! - GET  /items        - List all items
//! - GET  /items/{id}   - Get item by ID
//! - POST /items        - Create new item
//! - DELETE /items/{id} - Delete item

#[allow(warnings)]
mod bindings;

use std::cell::RefCell;
use std::collections::HashMap;

use bindings::exports::wasi::http::incoming_handler::Guest;
use bindings::wasi::http::types::{
    Fields, IncomingRequest, Method, OutgoingBody, OutgoingResponse, ResponseOutparam,
};

// Thread-local storage for items (persists across requests within same instance)
thread_local! {
    static ITEMS: RefCell<HashMap<u32, Item>> = RefCell::new(create_initial_items());
    static NEXT_ID: RefCell<u32> = RefCell::new(4);
}

#[derive(Clone)]
struct Item {
    id: u32,
    name: String,
    description: String,
    price: f64,
}

fn create_initial_items() -> HashMap<u32, Item> {
    let mut items = HashMap::new();
    items.insert(1, Item {
        id: 1,
        name: "Widget".to_string(),
        description: "A useful widget".to_string(),
        price: 19.99,
    });
    items.insert(2, Item {
        id: 2,
        name: "Gadget".to_string(),
        description: "An amazing gadget".to_string(),
        price: 29.99,
    });
    items.insert(3, Item {
        id: 3,
        name: "Gizmo".to_string(),
        description: "A fantastic gizmo".to_string(),
        price: 39.99,
    });
    items
}

struct Component;

impl Guest for Component {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        let method = request.method();
        let path = request.path_with_query().unwrap_or_default();

        // Parse the path
        let path_parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();

        let (status, content_type, body) = match (&method, path_parts.as_slice()) {
            // Health check
            (Method::Get, ["health"]) => {
                (200, "application/json", r#"{"status":"ok"}"#.to_string())
            }

            // List all items
            (Method::Get, ["items"]) => {
                let items_json = ITEMS.with(|items| {
                    let items = items.borrow();
                    let items_vec: Vec<String> = items.values().map(|item| {
                        format!(
                            r#"{{"id":{},"name":"{}","description":"{}","price":{}}}"#,
                            item.id, item.name, item.description, item.price
                        )
                    }).collect();
                    format!("[{}]", items_vec.join(","))
                });
                (200, "application/json", items_json)
            }

            // Get single item
            (Method::Get, ["items", id_str]) => {
                if let Ok(id) = id_str.parse::<u32>() {
                    ITEMS.with(|items| {
                        let items = items.borrow();
                        if let Some(item) = items.get(&id) {
                            (200, "application/json", format!(
                                r#"{{"id":{},"name":"{}","description":"{}","price":{}}}"#,
                                item.id, item.name, item.description, item.price
                            ))
                        } else {
                            (404, "application/json", r#"{"error":"Item not found"}"#.to_string())
                        }
                    })
                } else {
                    (400, "application/json", r#"{"error":"Invalid item ID"}"#.to_string())
                }
            }

            // Create new item
            (Method::Post, ["items"]) => {
                // Read request body
                let body_content = read_request_body(&request);

                // Simple JSON parsing (minimal implementation)
                if let Some((name, description, price)) = parse_item_json(&body_content) {
                    let new_id = NEXT_ID.with(|id| {
                        let mut id = id.borrow_mut();
                        let new_id = *id;
                        *id += 1;
                        new_id
                    });

                    let item = Item {
                        id: new_id,
                        name,
                        description,
                        price,
                    };

                    ITEMS.with(|items| {
                        items.borrow_mut().insert(new_id, item.clone());
                    });

                    (201, "application/json", format!(
                        r#"{{"id":{},"name":"{}","description":"{}","price":{}}}"#,
                        item.id, item.name, item.description, item.price
                    ))
                } else {
                    (400, "application/json", r#"{"error":"Invalid JSON body"}"#.to_string())
                }
            }

            // Delete item
            (Method::Delete, ["items", id_str]) => {
                if let Ok(id) = id_str.parse::<u32>() {
                    ITEMS.with(|items| {
                        let mut items = items.borrow_mut();
                        if items.remove(&id).is_some() {
                            (200, "application/json", r#"{"deleted":true}"#.to_string())
                        } else {
                            (404, "application/json", r#"{"error":"Item not found"}"#.to_string())
                        }
                    })
                } else {
                    (400, "application/json", r#"{"error":"Invalid item ID"}"#.to_string())
                }
            }

            // Root path
            (Method::Get, [""]) | (Method::Get, []) => {
                (200, "application/json", r#"{"service":"multi-service-api","version":"1.0.0","endpoints":["/health","/items","/items/{id}"]}"#.to_string())
            }

            // Not found
            _ => {
                (404, "application/json", r#"{"error":"Not found"}"#.to_string())
            }
        };

        send_response(response_out, status, content_type, &body);
    }
}

fn read_request_body(request: &IncomingRequest) -> String {
    let body = match request.consume() {
        Ok(body) => body,
        Err(_) => return String::new(),
    };

    let stream = match body.stream() {
        Ok(stream) => stream,
        Err(_) => return String::new(),
    };

    let mut data = Vec::new();
    loop {
        match stream.blocking_read(4096) {
            Ok(chunk) => {
                if chunk.is_empty() {
                    break;
                }
                data.extend_from_slice(&chunk);
            }
            Err(_) => break,
        }
    }

    String::from_utf8(data).unwrap_or_default()
}

// Simple JSON parsing for item creation
// Expects: {"name": "...", "description": "...", "price": ...}
fn parse_item_json(json: &str) -> Option<(String, String, f64)> {
    // Very basic JSON parsing - in production you'd use serde_json
    let json = json.trim();

    let name = extract_string_field(json, "name")?;
    let description = extract_string_field(json, "description")?;
    let price = extract_number_field(json, "price")?;

    Some((name, description, price))
}

fn extract_string_field(json: &str, field: &str) -> Option<String> {
    let pattern = format!(r#""{}":"#, field);
    let start = json.find(&pattern)? + pattern.len();
    let rest = &json[start..];

    // Find the opening quote
    let quote_start = rest.find('"')?;
    let rest = &rest[quote_start + 1..];

    // Find the closing quote (handling escaped quotes would be needed for production)
    let quote_end = rest.find('"')?;

    Some(rest[..quote_end].to_string())
}

fn extract_number_field(json: &str, field: &str) -> Option<f64> {
    let pattern = format!(r#""{}":"#, field);
    let start = json.find(&pattern)? + pattern.len();
    let rest = &json[start..].trim_start();

    // Find the end of the number (comma, }, or end of string)
    let end = rest.find(|c: char| c == ',' || c == '}').unwrap_or(rest.len());
    let num_str = rest[..end].trim();

    num_str.parse().ok()
}

fn send_response(response_out: ResponseOutparam, status: u16, content_type: &str, body: &str) {
    // Create headers with content-type
    let headers = Fields::from_list(&[
        ("content-type".to_string(), content_type.as_bytes().to_vec()),
    ]).expect("failed to create headers");

    let response = OutgoingResponse::new(headers);
    response.set_status_code(status).expect("failed to set status");

    let outgoing_body = response.body().expect("failed to get body");
    {
        let stream = outgoing_body.write().expect("failed to get write stream");
        stream
            .blocking_write_and_flush(body.as_bytes())
            .expect("failed to write body");
    }

    OutgoingBody::finish(outgoing_body, None).expect("failed to finish body");
    ResponseOutparam::set(response_out, Ok(response));
}

bindings::export!(Component with_types_in bindings);
