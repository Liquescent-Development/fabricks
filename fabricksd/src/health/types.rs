//! Health check types and status tracking.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Health status of a service.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Service is healthy and responding correctly.
    Healthy,

    /// Service is unhealthy or not responding.
    Unhealthy,

    /// Health status is unknown (checks haven't run yet).
    #[default]
    Unknown,

    /// Service is starting up, health checks not yet applicable.
    Starting,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(f, "healthy"),
            Self::Unhealthy => write!(f, "unhealthy"),
            Self::Unknown => write!(f, "unknown"),
            Self::Starting => write!(f, "starting"),
        }
    }
}

/// Result of a single health check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    /// When the check was performed.
    pub timestamp: DateTime<Utc>,

    /// The health status determined by the check.
    pub status: HealthStatus,

    /// Response time if the check completed successfully.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "optional_duration_millis"
    )]
    pub response_time: Option<Duration>,

    /// Error message if the check failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// HTTP status code (for HTTP checks).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
}

impl HealthCheckResult {
    /// Creates a healthy result.
    #[must_use]
    pub fn healthy(response_time: Duration) -> Self {
        Self {
            timestamp: Utc::now(),
            status: HealthStatus::Healthy,
            response_time: Some(response_time),
            error: None,
            http_status: None,
        }
    }

    /// Creates a healthy HTTP result with status code.
    #[must_use]
    pub fn healthy_http(response_time: Duration, status_code: u16) -> Self {
        Self {
            timestamp: Utc::now(),
            status: HealthStatus::Healthy,
            response_time: Some(response_time),
            error: None,
            http_status: Some(status_code),
        }
    }

    /// Creates an unhealthy result with an error message.
    #[must_use]
    pub fn unhealthy(error: impl Into<String>) -> Self {
        Self {
            timestamp: Utc::now(),
            status: HealthStatus::Unhealthy,
            response_time: None,
            error: Some(error.into()),
            http_status: None,
        }
    }

    /// Creates an unhealthy HTTP result with status code.
    #[must_use]
    pub fn unhealthy_http(status_code: u16, response_time: Duration) -> Self {
        Self {
            timestamp: Utc::now(),
            status: HealthStatus::Unhealthy,
            response_time: Some(response_time),
            error: Some(format!("Unexpected HTTP status: {status_code}")),
            http_status: Some(status_code),
        }
    }
}

/// Health state for a service, tracking check history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceHealth {
    /// Service ID.
    pub service_id: String,

    /// Current health status.
    pub status: HealthStatus,

    /// Consecutive failures count.
    pub consecutive_failures: u32,

    /// Last successful check time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_healthy: Option<DateTime<Utc>>,

    /// Recent check results (limited history).
    pub history: Vec<HealthCheckResult>,
}

impl ServiceHealth {
    /// Creates a new service health tracker.
    #[must_use]
    pub fn new(service_id: String) -> Self {
        Self {
            service_id,
            status: HealthStatus::Unknown,
            consecutive_failures: 0,
            last_healthy: None,
            history: Vec::new(),
        }
    }

    /// Records a health check result.
    pub fn record(&mut self, result: HealthCheckResult, max_history: usize) {
        match result.status {
            HealthStatus::Healthy => {
                self.status = HealthStatus::Healthy;
                self.consecutive_failures = 0;
                self.last_healthy = Some(result.timestamp);
            }
            HealthStatus::Unhealthy => {
                self.consecutive_failures += 1;
                self.status = HealthStatus::Unhealthy;
            }
            _ => {}
        }

        self.history.push(result);

        // Trim history to max size
        if self.history.len() > max_history {
            self.history.remove(0);
        }
    }

    /// Returns the last check result.
    #[must_use]
    pub fn last_result(&self) -> Option<&HealthCheckResult> {
        self.history.last()
    }

    /// Returns true if the service should be restarted based on consecutive failures.
    #[must_use]
    pub fn should_restart(&self, failure_threshold: u32) -> bool {
        self.consecutive_failures >= failure_threshold
    }
}

/// Configuration for the health monitor.
#[derive(Debug, Clone)]
pub struct HealthMonitorConfig {
    /// Default interval between health checks.
    pub default_interval: Duration,

    /// Default timeout for health checks.
    pub default_timeout: Duration,

    /// Maximum health history entries per service.
    pub max_history: usize,

    /// Number of consecutive failures before marking unhealthy.
    pub failure_threshold: u32,
}

impl Default for HealthMonitorConfig {
    fn default() -> Self {
        Self {
            default_interval: Duration::from_secs(30),
            default_timeout: Duration::from_secs(5),
            max_history: 100,
            failure_threshold: 3,
        }
    }
}

/// Serialization helper for optional Duration as milliseconds.
mod optional_duration_millis {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serializer};

    // Serde requires this specific signature for custom serializers.
    #[allow(clippy::ref_option)]
    pub fn serialize<S: Serializer>(
        duration: &Option<Duration>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match duration {
            Some(d) => {
                // Saturate at u64::MAX (584 million years in milliseconds).
                let millis = u64::try_from(d.as_millis()).unwrap_or(u64::MAX);
                serializer.serialize_u64(millis)
            }
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Duration>, D::Error> {
        let millis: Option<u64> = Option::deserialize(deserializer)?;
        Ok(millis.map(Duration::from_millis))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_display() {
        assert_eq!(HealthStatus::Healthy.to_string(), "healthy");
        assert_eq!(HealthStatus::Unhealthy.to_string(), "unhealthy");
        assert_eq!(HealthStatus::Unknown.to_string(), "unknown");
        assert_eq!(HealthStatus::Starting.to_string(), "starting");
    }

    #[test]
    fn test_health_check_result_healthy() {
        let result = HealthCheckResult::healthy(Duration::from_millis(50));
        assert_eq!(result.status, HealthStatus::Healthy);
        assert_eq!(result.response_time, Some(Duration::from_millis(50)));
        assert!(result.error.is_none());
    }

    #[test]
    fn test_health_check_result_unhealthy() {
        let result = HealthCheckResult::unhealthy("Connection refused");
        assert_eq!(result.status, HealthStatus::Unhealthy);
        assert!(result.response_time.is_none());
        assert_eq!(result.error, Some("Connection refused".to_string()));
    }

    #[test]
    fn test_service_health_tracking() {
        let mut health = ServiceHealth::new("svc-123".to_string());
        assert_eq!(health.status, HealthStatus::Unknown);
        assert_eq!(health.consecutive_failures, 0);

        // Record healthy
        health.record(HealthCheckResult::healthy(Duration::from_millis(10)), 10);
        assert_eq!(health.status, HealthStatus::Healthy);
        assert_eq!(health.consecutive_failures, 0);
        assert!(health.last_healthy.is_some());

        // Record unhealthy
        health.record(HealthCheckResult::unhealthy("timeout"), 10);
        assert_eq!(health.status, HealthStatus::Unhealthy);
        assert_eq!(health.consecutive_failures, 1);

        // Record another unhealthy
        health.record(HealthCheckResult::unhealthy("timeout"), 10);
        assert_eq!(health.consecutive_failures, 2);

        // Record healthy resets counter
        health.record(HealthCheckResult::healthy(Duration::from_millis(10)), 10);
        assert_eq!(health.status, HealthStatus::Healthy);
        assert_eq!(health.consecutive_failures, 0);
    }

    #[test]
    fn test_service_health_history_limit() {
        let mut health = ServiceHealth::new("svc-123".to_string());

        for _ in 0..15 {
            health.record(HealthCheckResult::healthy(Duration::from_millis(10)), 10);
        }

        assert_eq!(health.history.len(), 10);
    }

    #[test]
    fn test_should_restart() {
        let mut health = ServiceHealth::new("svc-123".to_string());

        health.record(HealthCheckResult::unhealthy("error"), 10);
        assert!(!health.should_restart(3));

        health.record(HealthCheckResult::unhealthy("error"), 10);
        assert!(!health.should_restart(3));

        health.record(HealthCheckResult::unhealthy("error"), 10);
        assert!(health.should_restart(3));
    }
}
