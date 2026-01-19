//! Cart Service
//!
//! Shopping cart management with in-memory storage.
//!
//! Endpoints:
//! - GET /health - Health check
//! - GET /{user_id} - Get cart for user
//! - POST /{user_id}/items - Add item to cart
//! - DELETE /{user_id}/items/{product_id} - Remove item from cart

#[allow(warnings)]
mod bindings;

use std::cell::RefCell;
use std::collections::HashMap;

use bindings::exports::wasi::http::incoming_handler::Guest;
use bindings::wasi::http::types::{
    Fields, IncomingRequest, Method, OutgoingBody, OutgoingResponse, ResponseOutparam,
};

thread_local! {
    static CARTS: RefCell<HashMap<u32, Vec<CartItem>>> = RefCell::new(create_initial_carts());
}

#[derive(Clone)]
struct CartItem {
    product_id: u32,
    product_name: String,
    quantity: u32,
    price: f64,
}

fn create_initial_carts() -> HashMap<u32, Vec<CartItem>> {
    let mut carts = HashMap::new();
    // User 1 has some items in cart
    carts.insert(1, vec![
        CartItem { product_id: 1, product_name: "Laptop".to_string(), quantity: 1, price: 999.99 },
        CartItem { product_id: 2, product_name: "Headphones".to_string(), quantity: 2, price: 299.99 },
    ]);
    // User 2 has an empty cart
    carts.insert(2, vec![]);
    carts
}

struct Component;

impl Guest for Component {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        let method = request.method();
        let path = request.path_with_query().unwrap_or_default();
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();

        let (status, body) = match (&method, parts.as_slice()) {
            (Method::Get, ["health"]) => {
                (200, r#"{"status":"ok","service":"cart"}"#.to_string())
            }
            (Method::Get, [user_id]) => {
                if let Ok(user_id) = user_id.parse::<u32>() {
                    CARTS.with(|carts| {
                        let carts = carts.borrow();
                        if let Some(items) = carts.get(&user_id) {
                            let total: f64 = items.iter().map(|i| i.price * i.quantity as f64).sum();
                            let items_json: Vec<String> = items.iter().map(|i| {
                                format!(
                                    r#"{{"product_id":{},"product_name":"{}","quantity":{},"price":{},"subtotal":{}}}"#,
                                    i.product_id, i.product_name, i.quantity, i.price, i.price * i.quantity as f64
                                )
                            }).collect();
                            (200, format!(
                                r#"{{"user_id":{},"items":[{}],"total":{}}}"#,
                                user_id, items_json.join(","), total
                            ))
                        } else {
                            (200, format!(r#"{{"user_id":{},"items":[],"total":0}}"#, user_id))
                        }
                    })
                } else {
                    (400, r#"{"error":"Invalid user ID"}"#.to_string())
                }
            }
            (Method::Post, [user_id, "items"]) => {
                if let Ok(user_id) = user_id.parse::<u32>() {
                    // In a real app, we'd parse the request body
                    // For demo, just add a sample item
                    CARTS.with(|carts| {
                        let mut carts = carts.borrow_mut();
                        let cart = carts.entry(user_id).or_insert_with(Vec::new);
                        cart.push(CartItem {
                            product_id: 3,
                            product_name: "Coffee Maker".to_string(),
                            quantity: 1,
                            price: 79.99,
                        });
                        (201, r#"{"added":true,"product_id":3,"product_name":"Coffee Maker"}"#.to_string())
                    })
                } else {
                    (400, r#"{"error":"Invalid user ID"}"#.to_string())
                }
            }
            (Method::Delete, [user_id, "items", product_id]) => {
                if let (Ok(user_id), Ok(product_id)) = (user_id.parse::<u32>(), product_id.parse::<u32>()) {
                    CARTS.with(|carts| {
                        let mut carts = carts.borrow_mut();
                        if let Some(cart) = carts.get_mut(&user_id) {
                            let len_before = cart.len();
                            cart.retain(|item| item.product_id != product_id);
                            if cart.len() < len_before {
                                (200, r#"{"removed":true}"#.to_string())
                            } else {
                                (404, r#"{"error":"Item not found in cart"}"#.to_string())
                            }
                        } else {
                            (404, r#"{"error":"Cart not found"}"#.to_string())
                        }
                    })
                } else {
                    (400, r#"{"error":"Invalid ID"}"#.to_string())
                }
            }
            _ => {
                (404, r#"{"error":"Not found"}"#.to_string())
            }
        };

        send_response(response_out, status, &body);
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
