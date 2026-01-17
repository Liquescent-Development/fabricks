//! Auto-scaling and metrics collection.
//!
//! This module provides:
//!
//! - [`MetricsCollector`] - Collects request counts and latencies per service
//! - [`AutoScaler`] - Automatically scales services based on load
//! - [`ServiceMetrics`] - Aggregated metrics for a service
//!
//! # Metrics Collection
//!
//! The metrics collector runs as a background task, aggregating request
//! counts and latencies into [`ServiceMetrics`] at regular intervals.
//!
//! ```ignore
//! // Record a request
//! metrics_collector.record_request("svc-123", Duration::from_millis(50)).await;
//!
//! // Get current metrics
//! let metrics = metrics_collector.get_metrics("svc-123").await;
//! ```
//!
//! # Auto-Scaling
//!
//! The auto-scaler monitors metrics and scales services based on thresholds:
//!
//! - Scale up when load exceeds `scale_up_threshold` (default 80%)
//! - Scale down when load drops below `scale_down_threshold` (default 40%)
//! - Cooldown period prevents rapid scaling (default 60 seconds)
//!
//! Load is calculated from latency relative to a baseline. When latency
//! increases, load increases, triggering scale-up.

mod autoscaler;
mod metrics;
mod types;

pub use autoscaler::{AutoScaler, SharedAutoScaler};
pub use metrics::{MetricsCollector, SharedMetricsCollector};
pub use types::{
    AutoScalerConfig, LatencySample, MetricsCollectorConfig, MetricsSummary, ServiceMetrics,
};
