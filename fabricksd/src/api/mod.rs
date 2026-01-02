//! HTTP API server and handlers.
//!
//! This module provides the REST API for the daemon, including:
//! - Router configuration
//! - Response types
//! - Request handlers

mod response;
mod router;

pub mod handlers;

pub use response::{ApiError, ApiResponse};
pub use router::build_router;
