//! User Service
//!
//! User management with in-memory storage.
//!
//! Endpoints:
//! - GET /health - Health check
//! - GET / - List all users
//! - GET /{id} - Get user by ID
//! - POST / - Create new user

#[allow(warnings)]
mod bindings;

use std::cell::RefCell;
use std::collections::HashMap;

use bindings::exports::wasi::http::incoming_handler::Guest;
use bindings::wasi::http::types::{
    Fields, IncomingRequest, Method, OutgoingBody, OutgoingResponse, ResponseOutparam,
};

thread_local! {
    static USERS: RefCell<HashMap<u32, User>> = RefCell::new(create_initial_users());
    static NEXT_ID: RefCell<u32> = RefCell::new(4);
}

#[derive(Clone)]
struct User {
    id: u32,
    email: String,
    name: String,
    created_at: String,
}

fn create_initial_users() -> HashMap<u32, User> {
    let mut users = HashMap::new();
    users.insert(1, User {
        id: 1,
        email: "alice@example.com".to_string(),
        name: "Alice Johnson".to_string(),
        created_at: "2024-01-15T10:30:00Z".to_string(),
    });
    users.insert(2, User {
        id: 2,
        email: "bob@example.com".to_string(),
        name: "Bob Smith".to_string(),
        created_at: "2024-01-20T14:45:00Z".to_string(),
    });
    users.insert(3, User {
        id: 3,
        email: "carol@example.com".to_string(),
        name: "Carol Williams".to_string(),
        created_at: "2024-02-01T09:00:00Z".to_string(),
    });
    users
}

struct Component;

impl Guest for Component {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        let method = request.method();
        let path = request.path_with_query().unwrap_or_default();
        let path = path.trim_start_matches('/');

        let (status, body) = match (&method, path) {
            (Method::Get, "health") => {
                (200, r#"{"status":"ok","service":"user"}"#.to_string())
            }
            (Method::Get, "" | "/") => {
                let users_json = USERS.with(|users| {
                    let users = users.borrow();
                    let items: Vec<String> = users.values().map(|u| {
                        format!(
                            r#"{{"id":{},"email":"{}","name":"{}","created_at":"{}"}}"#,
                            u.id, u.email, u.name, u.created_at
                        )
                    }).collect();
                    format!("[{}]", items.join(","))
                });
                (200, users_json)
            }
            (Method::Get, id_str) => {
                if let Ok(id) = id_str.parse::<u32>() {
                    USERS.with(|users| {
                        let users = users.borrow();
                        if let Some(u) = users.get(&id) {
                            (200, format!(
                                r#"{{"id":{},"email":"{}","name":"{}","created_at":"{}"}}"#,
                                u.id, u.email, u.name, u.created_at
                            ))
                        } else {
                            (404, r#"{"error":"User not found"}"#.to_string())
                        }
                    })
                } else {
                    (400, r#"{"error":"Invalid user ID"}"#.to_string())
                }
            }
            (Method::Post, "" | "/") => {
                // Create a sample user
                let new_id = NEXT_ID.with(|id| {
                    let mut id = id.borrow_mut();
                    let new_id = *id;
                    *id += 1;
                    new_id
                });

                let user = User {
                    id: new_id,
                    email: format!("user{}@example.com", new_id),
                    name: format!("User {}", new_id),
                    created_at: "2024-03-01T12:00:00Z".to_string(),
                };

                USERS.with(|users| {
                    users.borrow_mut().insert(new_id, user.clone());
                });

                (201, format!(
                    r#"{{"id":{},"email":"{}","name":"{}","created_at":"{}"}}"#,
                    user.id, user.email, user.name, user.created_at
                ))
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
