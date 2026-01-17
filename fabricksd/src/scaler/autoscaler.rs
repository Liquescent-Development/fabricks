//! Auto-scaler for dynamic service scaling.
//!
//! Monitors service metrics and automatically scales services up or down
//! based on load thresholds.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{RwLock, broadcast};
use tokio::time::interval;
use tracing::{debug, info, warn};

use crate::events::{Event, EventBus};
use crate::service::{ServiceInfo, ServiceManager};

use super::metrics::MetricsCollector;
use super::types::AutoScalerConfig;

/// Auto-scaler for dynamic service scaling.
///
/// Monitors metrics from the `MetricsCollector` and scales services
/// up or down based on configured thresholds.
pub struct AutoScaler {
    /// Configuration.
    config: AutoScalerConfig,

    /// Service manager for scaling operations.
    service_manager: Arc<RwLock<ServiceManager>>,

    /// Metrics collector for load data.
    metrics_collector: Arc<MetricsCollector>,

    /// Event bus for publishing scaling events.
    event_bus: Arc<EventBus>,

    /// Cooldown tracking per service.
    cooldowns: RwLock<HashMap<String, Instant>>,
}

impl AutoScaler {
    /// Creates a new auto-scaler.
    #[must_use]
    pub fn new(
        config: AutoScalerConfig,
        service_manager: Arc<RwLock<ServiceManager>>,
        metrics_collector: Arc<MetricsCollector>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        Self {
            config,
            service_manager,
            metrics_collector,
            event_bus,
            cooldowns: RwLock::new(HashMap::new()),
        }
    }

    /// Runs the auto-scaler loop.
    ///
    /// This should be spawned as a background task.
    pub async fn run(&self, mut shutdown: broadcast::Receiver<()>) {
        info!("Auto-scaler started");

        let mut tick = interval(self.config.check_interval);

        loop {
            tokio::select! {
                _ = tick.tick() => {
                    self.check_and_scale().await;
                }
                _ = shutdown.recv() => {
                    info!("Auto-scaler shutting down");
                    break;
                }
            }
        }
    }

    /// Checks all services and scales as needed.
    async fn check_and_scale(&self) {
        let services = self.get_scalable_services().await;

        for service in services {
            if self.in_cooldown(&service.id).await {
                debug!(
                    service_id = %service.id,
                    "Service in cooldown, skipping scaling check"
                );
                continue;
            }

            let Some(metrics) = self.metrics_collector.get_metrics(&service.id).await else {
                debug!(
                    service_id = %service.id,
                    "No metrics available for service"
                );
                continue;
            };

            // Get the service's replica configuration for thresholds
            let (min_replicas, max_replicas, cpu_threshold) = {
                let manager = self.service_manager.read().await;
                match manager.get_service(&service.id).await {
                    Ok(detail) => {
                        let replicas = &detail.config.replicas;
                        (
                            replicas.min as usize,
                            replicas.max.map(|m| m as usize),
                            replicas.cpu_threshold.map(f64::from),
                        )
                    }
                    Err(_) => continue,
                }
            };

            let current = service.replicas.running;
            let scale_up_threshold = cpu_threshold.unwrap_or(self.config.scale_up_threshold);

            // Check if we need to scale up
            if metrics.load_percent > scale_up_threshold {
                self.scale_up(&service.id, current, max_replicas).await;
            }
            // Check if we need to scale down
            else if metrics.load_percent < self.config.scale_down_threshold {
                self.scale_down(&service.id, current, min_replicas).await;
            }
        }
    }

    /// Gets all services that can be auto-scaled.
    async fn get_scalable_services(&self) -> Vec<ServiceInfo> {
        let manager = self.service_manager.read().await;
        let services = manager.list_services().await;

        // Filter to only running services with auto-scaling enabled
        services
            .into_iter()
            .filter(|s| s.state == crate::service::State::Running)
            .collect()
    }

    /// Checks if a service is in cooldown.
    async fn in_cooldown(&self, service_id: &str) -> bool {
        let cooldowns = self.cooldowns.read().await;
        if let Some(last_scale) = cooldowns.get(service_id) {
            last_scale.elapsed() < self.config.cooldown_period
        } else {
            false
        }
    }

    /// Sets cooldown for a service.
    async fn set_cooldown(&self, service_id: &str) {
        let mut cooldowns = self.cooldowns.write().await;
        cooldowns.insert(service_id.to_string(), Instant::now());
    }

    /// Scales up a service by one instance.
    async fn scale_up(&self, service_id: &str, current: usize, max_replicas: Option<usize>) {
        let max = max_replicas.unwrap_or(usize::MAX);

        if current >= max {
            debug!(
                service_id = %service_id,
                current = current,
                max = max,
                "Cannot scale up, already at maximum"
            );
            return;
        }

        let target = current + 1;
        info!(
            service_id = %service_id,
            from = current,
            to = target,
            "Scaling up service"
        );

        let manager = self.service_manager.write().await;
        if let Err(e) = manager.scale_service(service_id, target).await {
            warn!(
                service_id = %service_id,
                error = %e,
                "Failed to scale up service"
            );
            return;
        }
        drop(manager);

        self.set_cooldown(service_id).await;

        // Publish event
        let () = self
            .event_bus
            .publish(Event::auto_scaled(service_id, current, target, "load_exceeded"))
            .await;
    }

    /// Scales down a service by one instance.
    async fn scale_down(&self, service_id: &str, current: usize, min_replicas: usize) {
        if current <= min_replicas {
            debug!(
                service_id = %service_id,
                current = current,
                min = min_replicas,
                "Cannot scale down, already at minimum"
            );
            return;
        }

        let target = current - 1;
        info!(
            service_id = %service_id,
            from = current,
            to = target,
            "Scaling down service"
        );

        let manager = self.service_manager.write().await;
        if let Err(e) = manager.scale_service(service_id, target).await {
            warn!(
                service_id = %service_id,
                error = %e,
                "Failed to scale down service"
            );
            return;
        }
        drop(manager);

        self.set_cooldown(service_id).await;

        // Publish event
        let () = self
            .event_bus
            .publish(Event::auto_scaled(service_id, current, target, "load_low"))
            .await;
    }

    /// Forces a scaling check immediately (for testing).
    pub async fn check_now(&self) {
        self.check_and_scale().await;
    }

    /// Clears the cooldown for a service (for testing).
    pub async fn clear_cooldown(&self, service_id: &str) {
        let mut cooldowns = self.cooldowns.write().await;
        cooldowns.remove(service_id);
    }
}

impl std::fmt::Debug for AutoScaler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AutoScaler")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// Shared reference to an auto-scaler.
pub type SharedAutoScaler = Arc<AutoScaler>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cooldown() {
        // Test the cooldown mechanism in isolation via the data structure
        // (We can't easily test the full AutoScaler without a real ServiceManager)
        let cooldowns: RwLock<HashMap<String, Instant>> = RwLock::new(HashMap::new());

        // Initially no cooldown
        {
            let cds = cooldowns.read().await;
            assert!(cds.get("svc-1").is_none());
        }

        // Set cooldown
        {
            let mut cds = cooldowns.write().await;
            cds.insert("svc-1".to_string(), Instant::now());
        }

        // Should be in cooldown
        {
            let cds = cooldowns.read().await;
            let in_cd = cds
                .get("svc-1")
                .map(|t| t.elapsed() < std::time::Duration::from_secs(60))
                .unwrap_or(false);
            assert!(in_cd);
        }
    }
}
