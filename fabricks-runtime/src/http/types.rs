//! HTTP request and response types.
//!
//! Provides wrapper types and conversion utilities between hyper and
//! wasmtime-wasi-http types.

use std::collections::HashMap;

use bytes::Bytes;

/// HTTP request wrapper for passing to WASM handlers.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// HTTP method (GET, POST, etc.).
    pub method: String,

    /// Request URI/path.
    pub uri: String,

    /// HTTP headers.
    pub headers: HashMap<String, String>,

    /// Request body.
    pub body: Bytes,

    /// The scheme (http or https).
    pub scheme: Scheme,

    /// The authority (host:port).
    pub authority: Option<String>,
}

/// HTTP scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scheme {
    /// HTTP (unencrypted).
    #[default]
    Http,
    /// HTTPS (TLS encrypted).
    Https,
}

impl HttpRequest {
    /// Creates a new HTTP request.
    #[must_use]
    pub fn new(method: impl Into<String>, uri: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            uri: uri.into(),
            headers: HashMap::new(),
            body: Bytes::new(),
            scheme: Scheme::Http,
            authority: None,
        }
    }

    /// Sets a header on the request.
    #[must_use]
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(name.into(), value.into());
        self
    }

    /// Sets the request body.
    #[must_use]
    pub fn with_body(mut self, body: impl Into<Bytes>) -> Self {
        self.body = body.into();
        self
    }

    /// Sets the scheme.
    #[must_use]
    pub const fn with_scheme(mut self, scheme: Scheme) -> Self {
        self.scheme = scheme;
        self
    }

    /// Sets the authority (host:port).
    #[must_use]
    pub fn with_authority(mut self, authority: impl Into<String>) -> Self {
        self.authority = Some(authority.into());
        self
    }
}

/// HTTP response wrapper returned from WASM handlers.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// HTTP status code.
    pub status: u16,

    /// Response headers.
    pub headers: HashMap<String, String>,

    /// Response body.
    pub body: Bytes,
}

impl HttpResponse {
    /// Creates a new HTTP response with the given status code.
    #[must_use]
    pub fn new(status: u16) -> Self {
        Self {
            status,
            headers: HashMap::new(),
            body: Bytes::new(),
        }
    }

    /// Creates an OK (200) response.
    #[must_use]
    pub fn ok() -> Self {
        Self::new(200)
    }

    /// Creates a Not Found (404) response.
    #[must_use]
    pub fn not_found() -> Self {
        Self::new(404)
    }

    /// Creates an Internal Server Error (500) response.
    #[must_use]
    pub fn internal_error() -> Self {
        Self::new(500)
    }

    /// Sets a header on the response.
    #[must_use]
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(name.into(), value.into());
        self
    }

    /// Sets the response body.
    #[must_use]
    pub fn with_body(mut self, body: impl Into<Bytes>) -> Self {
        self.body = body.into();
        self
    }

    /// Returns true if the response indicates success (2xx status).
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }
}

impl Default for HttpResponse {
    fn default() -> Self {
        Self::ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_request_builder() {
        let req = HttpRequest::new("GET", "/api/users")
            .with_header("Content-Type", "application/json")
            .with_authority("localhost:8080")
            .with_scheme(Scheme::Http);

        assert_eq!(req.method, "GET");
        assert_eq!(req.uri, "/api/users");
        assert_eq!(
            req.headers.get("Content-Type"),
            Some(&"application/json".to_string())
        );
        assert_eq!(req.authority, Some("localhost:8080".to_string()));
        assert_eq!(req.scheme, Scheme::Http);
    }

    #[test]
    fn test_http_response_builder() {
        let resp = HttpResponse::ok()
            .with_header("Content-Type", "text/plain")
            .with_body("Hello, World!");

        assert_eq!(resp.status, 200);
        assert!(resp.is_success());
        assert_eq!(
            resp.headers.get("Content-Type"),
            Some(&"text/plain".to_string())
        );
        assert_eq!(resp.body, Bytes::from("Hello, World!"));
    }

    #[test]
    fn test_response_status_helpers() {
        assert!(HttpResponse::ok().is_success());
        assert!(!HttpResponse::not_found().is_success());
        assert!(!HttpResponse::internal_error().is_success());
    }
}
