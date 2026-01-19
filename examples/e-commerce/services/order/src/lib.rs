//! Order Service
//!
//! Order processing and management with in-memory storage.
//!
//! Endpoints:
//! - GET /health - Health check
//! - GET / - List all orders
//! - GET /{id} - Get order by ID
//! - POST / - Create new order

#[allow(warnings)]
mod bindings;

use std::cell::RefCell;
use std::collections::HashMap;

use bindings::exports::wasi::http::incoming_handler::Guest;
use bindings::wasi::http::types::{
    Fields, IncomingRequest, Method, OutgoingBody, OutgoingResponse, ResponseOutparam,
};

thread_local! {
    static ORDERS: RefCell<HashMap<u32, Order>> = RefCell::new(create_initial_orders());
    static NEXT_ID: RefCell<u32> = RefCell::new(3);
}

#[derive(Clone)]
struct Order {
    id: u32,
    user_id: u32,
    items: Vec<OrderItem>,
    total: f64,
    status: String,
}

#[derive(Clone)]
struct OrderItem {
    product_id: u32,
    product_name: String,
    quantity: u32,
    price: f64,
}

fn create_initial_orders() -> HashMap<u32, Order> {
    let mut orders = HashMap::new();
    orders.insert(1, Order {
        id: 1,
        user_id: 1,
        items: vec![
            OrderItem { product_id: 1, product_name: "Laptop".to_string(), quantity: 1, price: 999.99 },
        ],
        total: 999.99,
        status: "delivered".to_string(),
    });
    orders.insert(2, Order {
        id: 2,
        user_id: 1,
        items: vec![
            OrderItem { product_id: 2, product_name: "Headphones".to_string(), quantity: 1, price: 299.99 },
            OrderItem { product_id: 3, product_name: "Coffee Maker".to_string(), quantity: 1, price: 79.99 },
        ],
        total: 379.98,
        status: "processing".to_string(),
    });
    orders
}

struct Component;

impl Guest for Component {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        let method = request.method();
        let path = request.path_with_query().unwrap_or_default();
        let path = path.trim_start_matches('/');

        let (status, body) = match (&method, path) {
            (Method::Get, "health") => {
                (200, r#"{"status":"ok","service":"order"}"#.to_string())
            }
            (Method::Get, "" | "/") => {
                let orders_json = ORDERS.with(|orders| {
                    let orders = orders.borrow();
                    let items: Vec<String> = orders.values().map(|o| order_to_json(o)).collect();
                    format!("[{}]", items.join(","))
                });
                (200, orders_json)
            }
            (Method::Get, id_str) => {
                if let Ok(id) = id_str.parse::<u32>() {
                    ORDERS.with(|orders| {
                        let orders = orders.borrow();
                        if let Some(o) = orders.get(&id) {
                            (200, order_to_json(o))
                        } else {
                            (404, r#"{"error":"Order not found"}"#.to_string())
                        }
                    })
                } else {
                    (400, r#"{"error":"Invalid order ID"}"#.to_string())
                }
            }
            (Method::Post, "" | "/") => {
                // Create a sample order
                let new_id = NEXT_ID.with(|id| {
                    let mut id = id.borrow_mut();
                    let new_id = *id;
                    *id += 1;
                    new_id
                });

                let order = Order {
                    id: new_id,
                    user_id: 1,
                    items: vec![
                        OrderItem { product_id: 4, product_name: "Running Shoes".to_string(), quantity: 1, price: 129.99 },
                    ],
                    total: 129.99,
                    status: "pending".to_string(),
                };

                ORDERS.with(|orders| {
                    orders.borrow_mut().insert(new_id, order.clone());
                });

                (201, order_to_json(&order))
            }
            _ => {
                (404, r#"{"error":"Not found"}"#.to_string())
            }
        };

        send_response(response_out, status, &body);
    }
}

fn order_to_json(order: &Order) -> String {
    let items_json: Vec<String> = order.items.iter().map(|i| {
        format!(
            r#"{{"product_id":{},"product_name":"{}","quantity":{},"price":{}}}"#,
            i.product_id, i.product_name, i.quantity, i.price
        )
    }).collect();
    format!(
        r#"{{"id":{},"user_id":{},"items":[{}],"total":{},"status":"{}"}}"#,
        order.id, order.user_id, items_json.join(","), order.total, order.status
    )
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
