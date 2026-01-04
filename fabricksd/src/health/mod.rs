//! Health monitoring for services.
//!
//! This module provides health checking capabilities for WASM services.
//! The health monitor performs periodic checks and tracks service health
//! status for routing decisions.
//!
//! # Health Check Types
//!
//! - **HTTP**: Makes HTTP requests to a health endpoint
//! - **TCP**: Attempts to establish a TCP connection (future)
//!
//! # Health States
//!
//! - `Unknown`: Initial state, no checks performed yet
//! - `Healthy`: Service is responding correctly
//! - `Unhealthy`: Service failed health checks
//! - `Degraded`: Service is partially healthy (future)

mod monitor;
mod types;

pub use monitor::{HealthCheckRegistration, HealthMonitor, SharedHealthMonitor};
pub use types::{HealthCheckResult, HealthMonitorConfig, HealthStatus, ServiceHealth};
