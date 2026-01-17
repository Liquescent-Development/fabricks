//! Types for metrics collection and auto-scaling.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Configuration for the metrics collector.
#[derive(Debug, Clone)]
pub struct MetricsCollectorConfig {
    /// How often to aggregate metrics (default: 10 seconds).
    pub aggregation_interval: Duration,

    /// Size of the rolling window for latency samples (default: 1000).
    pub latency_window_size: usize,

    /// Maximum age of metrics before they're considered stale (default: 5 minutes).
    pub max_metrics_age: Duration,

    /// Baseline latency in milliseconds for load calculation (default: 50ms).
    pub baseline_latency_ms: f64,
}

impl Default for MetricsCollectorConfig {
    fn default() -> Self {
        Self {
            aggregation_interval: Duration::from_secs(10),
            latency_window_size: 1000,
            max_metrics_age: Duration::from_secs(300),
            baseline_latency_ms: 50.0,
        }
    }
}

/// Configuration for the auto-scaler.
#[derive(Debug, Clone)]
pub struct AutoScalerConfig {
    /// How often to check for scaling decisions (default: 30 seconds).
    pub check_interval: Duration,

    /// Minimum time between scaling operations (default: 60 seconds).
    pub cooldown_period: Duration,

    /// CPU/load percentage threshold to trigger scale up (default: 80%).
    pub scale_up_threshold: f64,

    /// CPU/load percentage threshold to trigger scale down (default: 40%).
    pub scale_down_threshold: f64,

    /// Baseline latency in milliseconds for load calculation (default: 50ms).
    pub baseline_latency_ms: f64,
}

impl Default for AutoScalerConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(30),
            cooldown_period: Duration::from_secs(60),
            scale_up_threshold: 80.0,
            scale_down_threshold: 40.0,
            baseline_latency_ms: 50.0,
        }
    }
}

/// Aggregated metrics for a service.
///
/// These metrics are collected over time and used for auto-scaling decisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceMetrics {
    /// Service ID.
    pub service_id: String,

    /// Timestamp when these metrics were last updated.
    pub timestamp: DateTime<Utc>,

    /// Total number of requests processed.
    pub request_count: u64,

    /// Requests per second (averaged over the collection interval).
    pub request_rate: f64,

    /// Average latency in milliseconds.
    pub latency_avg_ms: f64,

    /// 50th percentile latency in milliseconds.
    pub latency_p50_ms: f64,

    /// 99th percentile latency in milliseconds.
    pub latency_p99_ms: f64,

    /// Number of active/running instances.
    pub active_instances: usize,

    /// Calculated load percentage (0-100) based on latency trends.
    pub load_percent: f64,
}

impl ServiceMetrics {
    /// Creates new metrics for a service.
    #[must_use]
    pub fn new(service_id: String) -> Self {
        Self {
            service_id,
            timestamp: Utc::now(),
            request_count: 0,
            request_rate: 0.0,
            latency_avg_ms: 0.0,
            latency_p50_ms: 0.0,
            latency_p99_ms: 0.0,
            active_instances: 0,
            load_percent: 0.0,
        }
    }

    /// Returns whether these metrics are stale.
    #[must_use]
    pub fn is_stale(&self, max_age: Duration) -> bool {
        let now = Utc::now();
        let age = now.signed_duration_since(self.timestamp);
        // Convert max_age to i64 safely - for any realistic max_age (< 292 years)
        // this will succeed. If it somehow exceeds i64::MAX, treat as not stale.
        let max_age_secs = i64::try_from(max_age.as_secs()).unwrap_or(i64::MAX);
        age.num_seconds() > max_age_secs
    }
}

/// A single latency sample for tracking.
#[derive(Debug, Clone, Copy)]
pub struct LatencySample {
    /// Latency duration.
    pub latency: Duration,

    /// When the sample was recorded.
    pub recorded_at: std::time::Instant,
}

impl LatencySample {
    /// Creates a new latency sample.
    #[must_use]
    pub fn new(latency: Duration) -> Self {
        Self {
            latency,
            recorded_at: std::time::Instant::now(),
        }
    }
}

/// Summary response for all metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSummary {
    /// All service metrics.
    pub services: Vec<ServiceMetrics>,

    /// Timestamp of the summary.
    pub timestamp: DateTime<Utc>,
}

impl MetricsSummary {
    /// Creates a new metrics summary.
    #[must_use]
    pub fn new(services: Vec<ServiceMetrics>) -> Self {
        Self {
            services,
            timestamp: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector_config_default() {
        let config = MetricsCollectorConfig::default();
        assert_eq!(config.aggregation_interval, Duration::from_secs(10));
        assert_eq!(config.latency_window_size, 1000);
        assert_eq!(config.max_metrics_age, Duration::from_secs(300));
    }

    #[test]
    fn test_auto_scaler_config_default() {
        let config = AutoScalerConfig::default();
        assert_eq!(config.check_interval, Duration::from_secs(30));
        assert_eq!(config.cooldown_period, Duration::from_secs(60));
        assert_eq!(config.scale_up_threshold, 80.0);
        assert_eq!(config.scale_down_threshold, 40.0);
    }

    #[test]
    fn test_service_metrics_new() {
        let metrics = ServiceMetrics::new("svc-123".to_string());
        assert_eq!(metrics.service_id, "svc-123");
        assert_eq!(metrics.request_count, 0);
        assert_eq!(metrics.request_rate, 0.0);
    }

    #[test]
    fn test_service_metrics_stale() {
        let mut metrics = ServiceMetrics::new("svc-123".to_string());

        // Fresh metrics should not be stale
        assert!(!metrics.is_stale(Duration::from_secs(60)));

        // Set timestamp to the past
        metrics.timestamp = Utc::now() - chrono::Duration::seconds(120);

        // Now it should be stale with a 60 second max age
        assert!(metrics.is_stale(Duration::from_secs(60)));

        // But not with a 180 second max age
        assert!(!metrics.is_stale(Duration::from_secs(180)));
    }

    #[test]
    fn test_latency_sample() {
        let sample = LatencySample::new(Duration::from_millis(50));
        assert_eq!(sample.latency, Duration::from_millis(50));
    }
}
