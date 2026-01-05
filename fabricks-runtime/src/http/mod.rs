//! HTTP runtime support for WASM services.
//!
//! This module provides runtime support for WASM components that implement
//! the `wasi:http/incoming-handler` interface, allowing them to handle
//! incoming HTTP requests.
//!
//! The daemon binds ports and routes requests to WASM handlers - the WASM
//! modules never directly bind sockets.

mod runtime;
mod state;
pub mod types;

pub use runtime::{HttpRuntime, HttpRuntimeConfig};
pub use state::{OutboundHandler, WasiHttpState};
pub use types::{HttpRequest, HttpResponse, Scheme};
