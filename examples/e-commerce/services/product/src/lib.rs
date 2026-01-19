//! Product Service
//!
//! Product catalog management with in-memory storage.
//!
//! Endpoints:
//! - GET /health - Health check
//! - GET / - List all products
//! - GET /{id} - Get product by ID

#[allow(warnings)]
mod bindings;

use std::cell::RefCell;
use std::collections::HashMap;

use bindings::exports::wasi::http::incoming_handler::Guest;
use bindings::wasi::http::types::{
    Fields, IncomingRequest, Method, OutgoingBody, OutgoingResponse, ResponseOutparam,
};

thread_local! {
    static PRODUCTS: RefCell<HashMap<u32, Product>> = RefCell::new(create_initial_products());
}

#[derive(Clone)]
struct Product {
    id: u32,
    name: String,
    description: String,
    price: f64,
    category: String,
    in_stock: bool,
}

fn create_initial_products() -> HashMap<u32, Product> {
    let mut products = HashMap::new();
    products.insert(1, Product {
        id: 1,
        name: "Laptop".to_string(),
        description: "High-performance laptop".to_string(),
        price: 999.99,
        category: "Electronics".to_string(),
        in_stock: true,
    });
    products.insert(2, Product {
        id: 2,
        name: "Headphones".to_string(),
        description: "Wireless noise-canceling headphones".to_string(),
        price: 299.99,
        category: "Electronics".to_string(),
        in_stock: true,
    });
    products.insert(3, Product {
        id: 3,
        name: "Coffee Maker".to_string(),
        description: "Automatic drip coffee maker".to_string(),
        price: 79.99,
        category: "Kitchen".to_string(),
        in_stock: true,
    });
    products.insert(4, Product {
        id: 4,
        name: "Running Shoes".to_string(),
        description: "Lightweight running shoes".to_string(),
        price: 129.99,
        category: "Sports".to_string(),
        in_stock: false,
    });
    products
}

struct Component;

impl Guest for Component {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        let method = request.method();
        let path = request.path_with_query().unwrap_or_default();
        let path = path.trim_start_matches('/');

        let (status, body) = match (&method, path) {
            (Method::Get, "health") => {
                (200, r#"{"status":"ok","service":"product"}"#.to_string())
            }
            (Method::Get, "" | "/") => {
                let products_json = PRODUCTS.with(|products| {
                    let products = products.borrow();
                    let items: Vec<String> = products.values().map(|p| {
                        format!(
                            r#"{{"id":{},"name":"{}","description":"{}","price":{},"category":"{}","in_stock":{}}}"#,
                            p.id, p.name, p.description, p.price, p.category, p.in_stock
                        )
                    }).collect();
                    format!("[{}]", items.join(","))
                });
                (200, products_json)
            }
            (Method::Get, id_str) => {
                if let Ok(id) = id_str.parse::<u32>() {
                    PRODUCTS.with(|products| {
                        let products = products.borrow();
                        if let Some(p) = products.get(&id) {
                            (200, format!(
                                r#"{{"id":{},"name":"{}","description":"{}","price":{},"category":"{}","in_stock":{}}}"#,
                                p.id, p.name, p.description, p.price, p.category, p.in_stock
                            ))
                        } else {
                            (404, r#"{"error":"Product not found"}"#.to_string())
                        }
                    })
                } else {
                    (400, r#"{"error":"Invalid product ID"}"#.to_string())
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
