//! Payment Processor Service
//!
//! PCI-DSS compliant payment processing in an isolated zone.
//! Mocks Stripe API calls for demonstration purposes.
//!
//! This service demonstrates:
//! - Isolated security zone (no direct internet access in real deployment)
//! - Mock external API integration (Stripe)
//! - Audit-friendly logging patterns
//!
//! Endpoints:
//! - GET /health - Health check
//! - POST /charge - Process a charge (mock Stripe API)
//! - POST /refund - Process a refund (mock Stripe API)
//! - POST /webhook - Handle Stripe webhooks (mock)

#[allow(warnings)]
mod bindings;

use std::cell::RefCell;
use std::collections::HashMap;

use bindings::exports::wasi::http::incoming_handler::Guest;
use bindings::wasi::http::types::{
    Fields, IncomingRequest, Method, OutgoingBody, OutgoingResponse, ResponseOutparam,
};

thread_local! {
    static CHARGES: RefCell<HashMap<String, Charge>> = RefCell::new(HashMap::new());
    static NEXT_ID: RefCell<u32> = RefCell::new(1);
}

#[derive(Clone)]
struct Charge {
    id: String,
    amount: u64,
    currency: String,
    status: ChargeStatus,
    created: u64,
    refunded: bool,
    refund_id: Option<String>,
}

#[derive(Clone, PartialEq)]
enum ChargeStatus {
    Succeeded,
    Pending,
    Failed,
}

impl ChargeStatus {
    fn as_str(&self) -> &'static str {
        match self {
            ChargeStatus::Succeeded => "succeeded",
            ChargeStatus::Pending => "pending",
            ChargeStatus::Failed => "failed",
        }
    }
}

struct Component;

impl Guest for Component {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        let method = request.method();
        let path = request.path_with_query().unwrap_or_default();
        let path = path.trim_start_matches('/');

        let (status, body) = route_request(&method, path);
        send_response(response_out, status, &body);
    }
}

fn route_request(method: &Method, path: &str) -> (u16, String) {
    match (method, path) {
        (Method::Get, "health") => {
            (200, r#"{"status":"ok","service":"payment-processor","pci_zone":"isolated"}"#.to_string())
        }

        // POST /charge - Create a charge (mock Stripe charges/create)
        (Method::Post, "charge") => {
            create_charge()
        }

        // GET /charge/{id} - Get charge details
        (Method::Get, path) if path.starts_with("charge/") => {
            let id = path.trim_start_matches("charge/");
            get_charge(id)
        }

        // POST /refund - Create a refund (mock Stripe refunds/create)
        (Method::Post, "refund") => {
            // In real implementation, would read charge_id from body
            create_refund()
        }

        // POST /webhook - Handle Stripe webhook events
        (Method::Post, "webhook") => {
            handle_webhook()
        }

        _ => {
            (404, r#"{"error":"Not found"}"#.to_string())
        }
    }
}

fn create_charge() -> (u16, String) {
    let charge_id = NEXT_ID.with(|id| {
        let mut id = id.borrow_mut();
        let charge_id = format!("ch_{:024x}", *id);
        *id += 1;
        charge_id
    });

    // Mock Stripe charge response
    // In a real implementation, this would call api.stripe.com
    let charge = Charge {
        id: charge_id.clone(),
        amount: 9999, // Amount in cents
        currency: "usd".to_string(),
        status: ChargeStatus::Succeeded, // Mock successful charge
        created: 1709294400, // Unix timestamp
        refunded: false,
        refund_id: None,
    };

    CHARGES.with(|charges| {
        charges.borrow_mut().insert(charge_id.clone(), charge.clone());
    });

    // Return Stripe-like response
    (200, format!(
        r#"{{"id":"{}","object":"charge","amount":{},"currency":"{}","status":"{}","created":{},"refunded":{},"livemode":false}}"#,
        charge.id, charge.amount, charge.currency, charge.status.as_str(),
        charge.created, charge.refunded
    ))
}

fn get_charge(id: &str) -> (u16, String) {
    CHARGES.with(|charges| {
        let charges = charges.borrow();
        if let Some(c) = charges.get(id) {
            (200, format!(
                r#"{{"id":"{}","object":"charge","amount":{},"currency":"{}","status":"{}","created":{},"refunded":{}}}"#,
                c.id, c.amount, c.currency, c.status.as_str(), c.created, c.refunded
            ))
        } else {
            (404, r#"{"error":{"type":"invalid_request_error","message":"No such charge"}}"#.to_string())
        }
    })
}

fn create_refund() -> (u16, String) {
    // Find the most recent charge to refund (simplified for demo)
    let result = CHARGES.with(|charges| {
        let mut charges = charges.borrow_mut();

        // Find first non-refunded charge
        for charge in charges.values_mut() {
            if !charge.refunded && charge.status == ChargeStatus::Succeeded {
                charge.refunded = true;
                let refund_id = format!("re_{:024x}", charge.created);
                charge.refund_id = Some(refund_id.clone());

                return Some((charge.id.clone(), charge.amount, refund_id));
            }
        }
        None
    });

    match result {
        Some((charge_id, amount, refund_id)) => {
            // Return Stripe-like refund response
            (200, format!(
                r#"{{"id":"{}","object":"refund","amount":{},"charge":"{}","currency":"usd","status":"succeeded"}}"#,
                refund_id, amount, charge_id
            ))
        }
        None => {
            (400, r#"{"error":{"type":"invalid_request_error","message":"No eligible charge found for refund"}}"#.to_string())
        }
    }
}

fn handle_webhook() -> (u16, String) {
    // Mock webhook handling
    // In real implementation, would verify webhook signature using STRIPE_WEBHOOK_SECRET

    // Acknowledge receipt of webhook
    (200, r#"{"received":true}"#.to_string())
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
