//! TCP runtime for WASM services.
//!
//! Provides the `TcpRuntime` struct for executing WASM components that handle
//! raw TCP connections using the "inetd model" - stdin/stdout connected to the socket.

pub mod runtime;
pub mod types;

pub use runtime::{TcpRuntime, TcpRuntimeConfig};
pub use types::TcpConnection;
