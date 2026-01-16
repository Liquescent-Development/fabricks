//! WASI HTTP state management.
//!
//! Provides the combined state for WASI + HTTP capabilities, implementing
//! both `WasiView` and `WasiHttpView` traits.

use std::sync::Arc;

use wasmtime::component::ResourceTable;
use wasmtime_wasi::{WasiCtx, WasiView};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};

use crate::error::Result;

/// Handler for outbound HTTP requests from WASM modules.
///
/// This trait allows the daemon to intercept and validate outbound requests
/// before they are executed, enforcing capability and network restrictions.
pub trait OutboundHandler: Send + Sync {
    /// Checks if an outbound request to the given host:port is allowed.
    ///
    /// Returns `Ok(true)` if allowed, `Ok(false)` if denied, or an error.
    ///
    /// # Errors
    ///
    /// May return an error if the validation check itself fails
    /// (e.g., network manager unavailable).
    fn is_allowed(&self, host: &str, port: u16) -> Result<bool>;
}

/// Default outbound handler that denies all outbound requests.
///
/// This is used when no custom handler is provided.
pub struct DenyAllOutbound;

impl OutboundHandler for DenyAllOutbound {
    fn is_allowed(&self, _host: &str, _port: u16) -> Result<bool> {
        Ok(false)
    }
}

/// Combined state for WASI and WASI-HTTP capabilities.
///
/// This struct holds the context needed for both standard WASI operations
/// (filesystem, environment, etc.) and HTTP operations (incoming/outgoing
/// requests).
pub struct WasiHttpState {
    /// Standard WASI context with configured capabilities.
    wasi_ctx: WasiCtx,

    /// HTTP-specific context for WASI HTTP operations.
    http_ctx: WasiHttpCtx,

    /// Resource table for both WASI and HTTP resources.
    table: ResourceTable,

    /// Handler for validating outbound HTTP requests.
    #[allow(dead_code)]
    outbound_handler: Arc<dyn OutboundHandler>,
}

impl WasiHttpState {
    /// Creates a new WASI HTTP state.
    ///
    /// # Arguments
    ///
    /// * `wasi_ctx` - The standard WASI context
    /// * `outbound_handler` - Handler for validating outbound requests
    #[must_use]
    pub fn new(wasi_ctx: WasiCtx, outbound_handler: Arc<dyn OutboundHandler>) -> Self {
        Self {
            wasi_ctx,
            http_ctx: WasiHttpCtx::new(),
            table: ResourceTable::new(),
            outbound_handler,
        }
    }

    /// Creates a new WASI HTTP state with deny-all outbound policy.
    #[must_use]
    pub fn new_deny_outbound(wasi_ctx: WasiCtx) -> Self {
        Self::new(wasi_ctx, Arc::new(DenyAllOutbound))
    }
}

impl WasiView for WasiHttpState {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.wasi_ctx
    }

    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

impl WasiHttpView for WasiHttpState {
    fn ctx(&mut self) -> &mut WasiHttpCtx {
        &mut self.http_ctx
    }

    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }

    // Note: The outbound request interception happens at a higher level
    // in the daemon's egress proxy. The WasiHttpView's send_request is
    // not easily overridable in wasmtime-wasi-http, so we validate
    // connections at the daemon level before they reach here.
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasmtime_wasi::WasiCtxBuilder;

    #[test]
    fn test_deny_all_outbound() {
        let handler = DenyAllOutbound;
        assert!(
            !handler
                .is_allowed("example.com", 443)
                .expect("should not error")
        );
        assert!(
            !handler
                .is_allowed("localhost", 8080)
                .expect("should not error")
        );
    }

    #[test]
    fn test_wasi_http_state_creation() {
        let wasi_ctx = WasiCtxBuilder::new().build();
        let state = WasiHttpState::new_deny_outbound(wasi_ctx);

        // State should be created successfully
        assert!(
            !state
                .outbound_handler
                .is_allowed("test.com", 80)
                .expect("should work")
        );
    }
}
