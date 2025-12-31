//! Health check configuration types.

use serde::{Deserialize, Serialize};

use super::common::{Duration, HttpMethod};

/// Health check configuration.
///
/// Only one type of health check (http, tcp, or exec) should be specified.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HealthCheck {
    /// HTTP-based health check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<HttpHealthCheck>,

    /// TCP-based health check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tcp: Option<TcpHealthCheck>,

    /// Exec-based health check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exec: Option<ExecHealthCheck>,
}

/// HTTP-based health check configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HttpHealthCheck {
    /// Path to the health endpoint.
    pub path: String,

    /// Port to check (defaults to first listen port).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,

    /// Interval between checks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<Duration>,

    /// Timeout for each check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Duration>,

    /// Number of consecutive failures before unhealthy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retries: Option<u32>,

    /// HTTP method to use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<HttpMethod>,

    /// Expected HTTP status code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_status: Option<u16>,
}

/// TCP-based health check configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TcpHealthCheck {
    /// Port to check.
    pub port: u16,

    /// Interval between checks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<Duration>,

    /// Timeout for each check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Duration>,
}

/// Exec-based health check configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecHealthCheck {
    /// Command to execute.
    pub command: Vec<String>,

    /// Interval between checks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<Duration>,

    /// Timeout for each check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Duration>,
}

impl HealthCheck {
    /// Returns true if any health check is configured.
    #[must_use]
    pub fn is_configured(&self) -> bool {
        self.http.is_some() || self.tcp.is_some() || self.exec.is_some()
    }

    /// Returns the number of configured health checks (should be 0 or 1).
    #[must_use]
    pub fn configured_count(&self) -> usize {
        let mut count = 0;
        if self.http.is_some() {
            count += 1;
        }
        if self.tcp.is_some() {
            count += 1;
        }
        if self.exec.is_some() {
            count += 1;
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_check_configured() {
        let empty = HealthCheck::default();
        assert!(!empty.is_configured());
        assert_eq!(empty.configured_count(), 0);

        let http = HealthCheck {
            http: Some(HttpHealthCheck {
                path: "/health".to_string(),
                port: None,
                interval: None,
                timeout: None,
                retries: None,
                method: None,
                expected_status: None,
            }),
            ..Default::default()
        };
        assert!(http.is_configured());
        assert_eq!(http.configured_count(), 1);
    }
}
