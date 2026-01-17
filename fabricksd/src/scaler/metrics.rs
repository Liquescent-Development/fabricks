//! Metrics collection for services.
//!
//! Collects request counts, latencies, and derives load metrics for
//! auto-scaling decisions.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use tokio::sync::{RwLock, broadcast};
use tokio::time::interval;
use tracing::{debug, info};

use super::types::{LatencySample, MetricsCollectorConfig, ServiceMetrics};

/// Per-service request tracker for collecting raw metrics.
struct RequestTracker {
    /// Total request count (monotonically increasing).
    total_count: AtomicU64,

    /// Request count at last aggregation (for calculating rate).
    last_count: AtomicU64,

    /// Last aggregation time.
    last_aggregation: RwLock<Instant>,

    /// Rolling window of latency samples.
    latencies: RwLock<Vec<LatencySample>>,

    /// Current number of active instances.
    active_instances: AtomicUsize,
}

impl RequestTracker {
    /// Creates a new request tracker.
    fn new() -> Self {
        Self {
            total_count: AtomicU64::new(0),
            last_count: AtomicU64::new(0),
            last_aggregation: RwLock::new(Instant::now()),
            latencies: RwLock::new(Vec::new()),
            active_instances: AtomicUsize::new(0),
        }
    }

    /// Records a request with its latency.
    async fn record_request(&self, latency: Duration, window_size: usize) {
        // Increment count
        self.total_count.fetch_add(1, Ordering::Relaxed);

        // Add latency sample
        let sample = LatencySample::new(latency);
        let mut latencies = self.latencies.write().await;
        latencies.push(sample);

        // Trim to window size
        if latencies.len() > window_size {
            latencies.remove(0);
        }
    }

    /// Sets the number of active instances.
    fn set_active_instances(&self, count: usize) {
        self.active_instances.store(count, Ordering::Relaxed);
    }

    /// Aggregates metrics from the tracker.
    async fn aggregate(&self, service_id: &str, baseline_latency_ms: f64) -> ServiceMetrics {
        let now = Instant::now();
        let current_count = self.total_count.load(Ordering::Relaxed);
        let previous_count = self.last_count.swap(current_count, Ordering::Relaxed);

        // Calculate time since last aggregation
        let mut last_agg = self.last_aggregation.write().await;
        let elapsed = now.duration_since(*last_agg);
        *last_agg = now;
        drop(last_agg);

        // Calculate request rate
        // Note: Using u32 for the conversion as request counts per interval
        // are realistically bounded well below u32::MAX. For higher counts,
        // the rate saturates which is acceptable for metrics purposes.
        let requests_since = current_count.saturating_sub(previous_count);
        let elapsed_secs = elapsed.as_secs_f64();
        let rate = if elapsed_secs > 0.0 {
            let bounded_requests = u32::try_from(requests_since).unwrap_or(u32::MAX);
            f64::from(bounded_requests) / elapsed_secs
        } else {
            0.0
        };

        // Calculate latency percentiles
        let latencies = self.latencies.read().await;
        let (avg_ms, p50_ms, p99_ms) = Self::calculate_percentiles(&latencies);
        drop(latencies);

        // Calculate load based on latency
        let load = Self::calculate_load(avg_ms, baseline_latency_ms);

        ServiceMetrics {
            service_id: service_id.to_string(),
            timestamp: Utc::now(),
            request_count: current_count,
            request_rate: rate,
            latency_avg_ms: avg_ms,
            latency_p50_ms: p50_ms,
            latency_p99_ms: p99_ms,
            active_instances: self.active_instances.load(Ordering::Relaxed),
            load_percent: load,
        }
    }

    /// Calculates latency percentiles from samples.
    fn calculate_percentiles(samples: &[LatencySample]) -> (f64, f64, f64) {
        if samples.is_empty() {
            return (0.0, 0.0, 0.0);
        }

        // Convert to milliseconds and sort
        let mut latencies_ms: Vec<f64> = samples
            .iter()
            .map(|s| s.latency.as_secs_f64() * 1000.0)
            .collect();
        latencies_ms.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let n = latencies_ms.len();

        // Average - sum first, then divide by count
        let sum = latencies_ms.iter().sum::<f64>();
        // Bound count to u32 for safe conversion (sample window is typically <10000)
        let count_bounded = u32::try_from(n).unwrap_or(u32::MAX);
        let avg = sum / f64::from(count_bounded);

        // P50 (median)
        let p50 = latencies_ms[n / 2];

        // P99 - use integer math: (n * 99) / 100
        let p99_idx = n.saturating_mul(99) / 100;
        let p99_idx = p99_idx.min(n.saturating_sub(1));
        let p99 = latencies_ms[p99_idx];

        (avg, p50, p99)
    }

    /// Calculates load percentage based on latency vs baseline.
    ///
    /// Uses a simple heuristic: if latency is at baseline, load is 50%.
    /// As latency increases, load increases proportionally.
    fn calculate_load(avg_latency_ms: f64, baseline_ms: f64) -> f64 {
        if baseline_ms <= 0.0 || avg_latency_ms <= 0.0 {
            return 0.0;
        }

        // Load factor: 1.0 at baseline, increases as latency increases
        let load_factor = avg_latency_ms / baseline_ms;

        // Convert to percentage: baseline = 50%, 2x baseline = 100%
        (load_factor * 50.0).min(100.0)
    }
}

/// Collector for service metrics.
///
/// Tracks request counts and latencies per service, aggregating them
/// periodically for use in auto-scaling decisions.
pub struct MetricsCollector {
    /// Configuration.
    config: MetricsCollectorConfig,

    /// Per-service request trackers.
    trackers: RwLock<HashMap<String, Arc<RequestTracker>>>,

    /// Aggregated metrics per service.
    metrics: RwLock<HashMap<String, ServiceMetrics>>,
}

impl MetricsCollector {
    /// Creates a new metrics collector.
    #[must_use]
    pub fn new(config: MetricsCollectorConfig) -> Self {
        Self {
            config,
            trackers: RwLock::new(HashMap::new()),
            metrics: RwLock::new(HashMap::new()),
        }
    }

    /// Creates a new metrics collector with default configuration.
    #[must_use]
    pub fn default_config() -> Self {
        Self::new(MetricsCollectorConfig::default())
    }

    /// Registers a service for metrics collection.
    pub async fn register_service(&self, service_id: &str) {
        let mut trackers = self.trackers.write().await;
        trackers
            .entry(service_id.to_string())
            .or_insert_with(|| Arc::new(RequestTracker::new()));

        let mut metrics = self.metrics.write().await;
        metrics
            .entry(service_id.to_string())
            .or_insert_with(|| ServiceMetrics::new(service_id.to_string()));

        debug!(service_id = %service_id, "Registered service for metrics collection");
    }

    /// Unregisters a service from metrics collection.
    pub async fn unregister_service(&self, service_id: &str) {
        let mut trackers = self.trackers.write().await;
        trackers.remove(service_id);

        let mut metrics = self.metrics.write().await;
        metrics.remove(service_id);

        debug!(service_id = %service_id, "Unregistered service from metrics collection");
    }

    /// Records a request for a service.
    ///
    /// Call this after each request completes with the total latency.
    pub async fn record_request(&self, service_id: &str, latency: Duration) {
        let tracker = {
            let trackers = self.trackers.read().await;
            trackers.get(service_id).cloned()
        };

        if let Some(tracker) = tracker {
            tracker
                .record_request(latency, self.config.latency_window_size)
                .await;
        }
    }

    /// Updates the active instance count for a service.
    pub async fn update_instance_count(&self, service_id: &str, count: usize) {
        let tracker = {
            let trackers = self.trackers.read().await;
            trackers.get(service_id).cloned()
        };

        if let Some(tracker) = tracker {
            tracker.set_active_instances(count);
        }
    }

    /// Gets metrics for a specific service.
    pub async fn get_metrics(&self, service_id: &str) -> Option<ServiceMetrics> {
        let metrics = self.metrics.read().await;
        metrics.get(service_id).cloned()
    }

    /// Gets all metrics.
    pub async fn get_all_metrics(&self) -> Vec<ServiceMetrics> {
        let metrics = self.metrics.read().await;
        metrics.values().cloned().collect()
    }

    /// Runs the metrics aggregation loop.
    ///
    /// This should be spawned as a background task.
    pub async fn run(&self, mut shutdown: broadcast::Receiver<()>) {
        info!("Metrics collector started");

        let mut tick = interval(self.config.aggregation_interval);

        loop {
            tokio::select! {
                _ = tick.tick() => {
                    self.aggregate_all().await;
                }
                _ = shutdown.recv() => {
                    info!("Metrics collector shutting down");
                    break;
                }
            }
        }
    }

    /// Aggregates metrics for all tracked services.
    async fn aggregate_all(&self) {
        let service_ids: Vec<String> = {
            let trackers = self.trackers.read().await;
            trackers.keys().cloned().collect()
        };

        for service_id in service_ids {
            let tracker = {
                let trackers = self.trackers.read().await;
                trackers.get(&service_id).cloned()
            };

            if let Some(tracker) = tracker {
                let aggregated = tracker
                    .aggregate(&service_id, self.config.baseline_latency_ms)
                    .await;

                let mut metrics = self.metrics.write().await;
                metrics.insert(service_id.clone(), aggregated);

                debug!(service_id = %service_id, "Aggregated metrics");
            }
        }
    }

    /// Forces an immediate aggregation for all services.
    ///
    /// Useful for testing or when metrics are needed immediately.
    pub async fn aggregate_now(&self) {
        self.aggregate_all().await;
    }
}

impl std::fmt::Debug for MetricsCollector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetricsCollector")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// Shared reference to a metrics collector.
pub type SharedMetricsCollector = Arc<MetricsCollector>;

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> MetricsCollectorConfig {
        MetricsCollectorConfig {
            aggregation_interval: Duration::from_millis(100),
            latency_window_size: 100,
            max_metrics_age: Duration::from_secs(60),
            baseline_latency_ms: 50.0,
        }
    }

    #[tokio::test]
    async fn test_register_and_unregister() {
        let collector = MetricsCollector::new(test_config());

        collector.register_service("svc-1").await;
        assert!(collector.get_metrics("svc-1").await.is_some());

        collector.unregister_service("svc-1").await;
        assert!(collector.get_metrics("svc-1").await.is_none());
    }

    #[tokio::test]
    async fn test_record_request() {
        let collector = MetricsCollector::new(test_config());
        collector.register_service("svc-1").await;

        // Record some requests
        for _ in 0..10 {
            collector
                .record_request("svc-1", Duration::from_millis(50))
                .await;
        }

        // Force aggregation
        collector.aggregate_now().await;

        let metrics = collector
            .get_metrics("svc-1")
            .await
            .expect("should have metrics");
        assert_eq!(metrics.request_count, 10);
    }

    #[tokio::test]
    async fn test_update_instance_count() {
        let collector = MetricsCollector::new(test_config());
        collector.register_service("svc-1").await;

        collector.update_instance_count("svc-1", 3).await;
        collector.aggregate_now().await;

        let metrics = collector
            .get_metrics("svc-1")
            .await
            .expect("should have metrics");
        assert_eq!(metrics.active_instances, 3);
    }

    #[tokio::test]
    async fn test_get_all_metrics() {
        let collector = MetricsCollector::new(test_config());

        collector.register_service("svc-1").await;
        collector.register_service("svc-2").await;

        let all = collector.get_all_metrics().await;
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_calculate_percentiles() {
        // Test with known values
        let samples: Vec<LatencySample> = vec![
            Duration::from_millis(10),
            Duration::from_millis(20),
            Duration::from_millis(30),
            Duration::from_millis(40),
            Duration::from_millis(50),
        ]
        .into_iter()
        .map(LatencySample::new)
        .collect();

        let (avg, p50, p99) = RequestTracker::calculate_percentiles(&samples);

        // Average should be 30
        assert!((avg - 30.0).abs() < 0.1);

        // P50 should be 30 (middle value)
        assert!((p50 - 30.0).abs() < 0.1);

        // P99 should be 50 (last value for small sample)
        assert!((p99 - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_calculate_load() {
        // At baseline, load should be 50%
        let load = RequestTracker::calculate_load(50.0, 50.0);
        assert!((load - 50.0).abs() < 0.1);

        // At 2x baseline, load should be 100%
        let load = RequestTracker::calculate_load(100.0, 50.0);
        assert!((load - 100.0).abs() < 0.1);

        // At 0.5x baseline, load should be 25%
        let load = RequestTracker::calculate_load(25.0, 50.0);
        assert!((load - 25.0).abs() < 0.1);

        // Load should not exceed 100%
        let load = RequestTracker::calculate_load(200.0, 50.0);
        assert!((load - 100.0).abs() < 0.1);
    }
}
