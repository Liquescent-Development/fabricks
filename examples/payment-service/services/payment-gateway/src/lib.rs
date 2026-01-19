//! Payment Gateway Service
//!
//! Validates and routes payment requests to the isolated payment processor.
//! Demonstrates PCI-like security patterns with bridge architecture.
//!
//! Endpoints:
//! - GET /health - Health check
//! - POST /payments - Process a new payment
//! - GET /payments/{id} - Get payment status
//! - POST /payments/{id}/refund - Refund a payment

#[allow(warnings)]
mod bindings;

use std::cell::RefCell;
use std::collections::HashMap;

use bindings::exports::wasi::http::incoming_handler::Guest;
use bindings::wasi::http::types::{
    Fields, IncomingRequest, Method, OutgoingBody, OutgoingResponse, ResponseOutparam,
};

thread_local! {
    static PAYMENTS: RefCell<HashMap<String, Payment>> = RefCell::new(HashMap::new());
    static NEXT_ID: RefCell<u32> = RefCell::new(1);
}

#[derive(Clone)]
struct Payment {
    id: String,
    amount: u64,
    currency: String,
    status: PaymentStatus,
    created_at: String,
    customer_id: String,
}

#[derive(Clone, PartialEq)]
enum PaymentStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Refunded,
}

impl PaymentStatus {
    fn as_str(&self) -> &'static str {
        match self {
            PaymentStatus::Pending => "pending",
            PaymentStatus::Processing => "processing",
            PaymentStatus::Completed => "completed",
            PaymentStatus::Failed => "failed",
            PaymentStatus::Refunded => "refunded",
        }
    }
}

struct Component;

impl Guest for Component {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        let method = request.method();
        let path = request.path_with_query().unwrap_or_default();
        let path = path.trim_start_matches('/');

        let (status, body) = route_request(&method, path, &request);
        send_response(response_out, status, &body);
    }
}

fn route_request(method: &Method, path: &str, _request: &IncomingRequest) -> (u16, String) {
    match (method, path) {
        (Method::Get, "health") => {
            (200, r#"{"status":"ok","service":"payment-gateway"}"#.to_string())
        }

        // POST /payments - Create a new payment
        (Method::Post, "payments") => {
            create_payment()
        }

        // GET /payments - List all payments
        (Method::Get, "payments") => {
            list_payments()
        }

        // GET /payments/{id} - Get payment status
        (Method::Get, path) if path.starts_with("payments/") && !path.contains("/refund") => {
            let id = path.trim_start_matches("payments/");
            get_payment(id)
        }

        // POST /payments/{id}/refund - Refund a payment
        (Method::Post, path) if path.starts_with("payments/") && path.ends_with("/refund") => {
            let id = path.trim_start_matches("payments/").trim_end_matches("/refund");
            refund_payment(id)
        }

        _ => {
            (404, r#"{"error":"Not found"}"#.to_string())
        }
    }
}

fn create_payment() -> (u16, String) {
    let payment_id = NEXT_ID.with(|id| {
        let mut id = id.borrow_mut();
        let payment_id = format!("pay_{:08x}", *id);
        *id += 1;
        payment_id
    });

    // Simulate calling the payment processor
    // In a real implementation, this would make an HTTP call to payment-processor:9000
    let payment = Payment {
        id: payment_id.clone(),
        amount: 9999, // $99.99 in cents
        currency: "usd".to_string(),
        status: PaymentStatus::Completed, // Simulated successful processing
        created_at: "2024-03-01T12:00:00Z".to_string(),
        customer_id: "cus_demo123".to_string(),
    };

    PAYMENTS.with(|payments| {
        payments.borrow_mut().insert(payment_id.clone(), payment.clone());
    });

    (201, format!(
        r#"{{"id":"{}","amount":{},"currency":"{}","status":"{}","created_at":"{}","customer_id":"{}"}}"#,
        payment.id, payment.amount, payment.currency, payment.status.as_str(),
        payment.created_at, payment.customer_id
    ))
}

fn list_payments() -> (u16, String) {
    let payments_json = PAYMENTS.with(|payments| {
        let payments = payments.borrow();
        let items: Vec<String> = payments.values().map(|p| {
            format!(
                r#"{{"id":"{}","amount":{},"currency":"{}","status":"{}","created_at":"{}","customer_id":"{}"}}"#,
                p.id, p.amount, p.currency, p.status.as_str(), p.created_at, p.customer_id
            )
        }).collect();
        format!("[{}]", items.join(","))
    });
    (200, payments_json)
}

fn get_payment(id: &str) -> (u16, String) {
    PAYMENTS.with(|payments| {
        let payments = payments.borrow();
        if let Some(p) = payments.get(id) {
            (200, format!(
                r#"{{"id":"{}","amount":{},"currency":"{}","status":"{}","created_at":"{}","customer_id":"{}"}}"#,
                p.id, p.amount, p.currency, p.status.as_str(), p.created_at, p.customer_id
            ))
        } else {
            (404, r#"{"error":"Payment not found"}"#.to_string())
        }
    })
}

fn refund_payment(id: &str) -> (u16, String) {
    PAYMENTS.with(|payments| {
        let mut payments = payments.borrow_mut();
        if let Some(p) = payments.get_mut(id) {
            if p.status == PaymentStatus::Completed {
                // Simulate calling the payment processor for refund
                p.status = PaymentStatus::Refunded;
                (200, format!(
                    r#"{{"id":"{}","amount":{},"currency":"{}","status":"{}","refunded":true}}"#,
                    p.id, p.amount, p.currency, p.status.as_str()
                ))
            } else {
                (400, format!(
                    r#"{{"error":"Cannot refund payment with status: {}"}}"#,
                    p.status.as_str()
                ))
            }
        } else {
            (404, r#"{"error":"Payment not found"}"#.to_string())
        }
    })
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
