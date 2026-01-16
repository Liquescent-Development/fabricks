//! Health monitor for tracking service health.
//!
//! The health monitor performs periodic health checks against registered
//! services and tracks their health state for routing decisions.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::Client;
use tokio::sync::{RwLock, broadcast};
use tokio::time::interval;
use tracing::{debug, info, warn};

use fabricks_common::models::common::HttpMethod;
use fabricks_common::models::health_check::HttpHealthCheck;

use super::types::{HealthCheckResult, HealthMonitorConfig, HealthStatus, ServiceHealth};

/// Registration for a service's health checks.
#[derive(Debug, Clone)]
pub struct HealthCheckRegistration {
    /// Service ID.
    pub service_id: String,
    /// HTTP health check configuration.
    pub http_check: HttpHealthCheck,
    /// Port the service is listening on.
    pub port: u16,
    /// When the next check should run.
    next_check: Instant,
}

impl HealthCheckRegistration {
    /// Creates a new health check registration.
    #[must_use]
    pub fn new(service_id: String, http_check: HttpHealthCheck, port: u16) -> Self {
        Self {
            service_id,
            http_check,
            port,
            next_check: Instant::now(),
        }
    }

    /// Returns the interval for this check.
    fn interval(&self) -> Duration {
        self.http_check.interval.as_ref().map_or(
            Duration::from_secs(30),
            fabricks_common::models::Duration::as_std,
        )
    }

    /// Returns the timeout for this check.
    fn timeout(&self) -> Duration {
        self.http_check.timeout.as_ref().map_or(
            Duration::from_secs(5),
            fabricks_common::models::Duration::as_std,
        )
    }

    /// Schedules the next check.
    fn schedule_next(&mut self) {
        self.next_check = Instant::now() + self.interval();
    }

    /// Returns true if the check is due.
    fn is_due(&self) -> bool {
        Instant::now() >= self.next_check
    }
}

/// Health monitor for tracking service health.
///
/// Runs periodic health checks against registered services and
/// tracks their health state.
pub struct HealthMonitor {
    /// Configuration for the monitor.
    config: HealthMonitorConfig,
    /// HTTP client for health checks.
    client: Client,
    /// Registered health checks by service ID.
    registrations: RwLock<HashMap<String, HealthCheckRegistration>>,
    /// Health state by service ID.
    health_states: RwLock<HashMap<String, ServiceHealth>>,
}

impl HealthMonitor {
    /// Creates a new health monitor.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created.
    pub fn new(config: HealthMonitorConfig) -> Result<Self, std::io::Error> {
        let client = Client::builder()
            .timeout(config.default_timeout)
            .build()
            .map_err(|e| std::io::Error::other(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self {
            config,
            client,
            registrations: RwLock::new(HashMap::new()),
            health_states: RwLock::new(HashMap::new()),
        })
    }

    /// Registers a service for health monitoring.
    pub async fn register(&self, service_id: String, http_check: HttpHealthCheck, port: u16) {
        let registration = HealthCheckRegistration::new(service_id.clone(), http_check, port);

        let mut registrations = self.registrations.write().await;
        registrations.insert(service_id.clone(), registration);

        let mut health_states = self.health_states.write().await;
        health_states.insert(service_id.clone(), ServiceHealth::new(service_id.clone()));

        info!(service_id = %service_id, "Registered service for health monitoring");
    }

    /// Unregisters a service from health monitoring.
    pub async fn unregister(&self, service_id: &str) {
        let mut registrations = self.registrations.write().await;
        registrations.remove(service_id);

        let mut health_states = self.health_states.write().await;
        health_states.remove(service_id);

        info!(service_id = %service_id, "Unregistered service from health monitoring");
    }

    /// Returns the current health state for a service.
    pub async fn get_health(&self, service_id: &str) -> Option<ServiceHealth> {
        let health_states = self.health_states.read().await;
        health_states.get(service_id).cloned()
    }

    /// Returns the current health status for a service.
    pub async fn get_status(&self, service_id: &str) -> HealthStatus {
        let health_states = self.health_states.read().await;
        health_states
            .get(service_id)
            .map_or(HealthStatus::Unknown, |h| h.status)
    }

    /// Returns all health states.
    pub async fn get_all_health(&self) -> HashMap<String, ServiceHealth> {
        let health_states = self.health_states.read().await;
        health_states.clone()
    }

    /// Returns true if a service is healthy.
    pub async fn is_healthy(&self, service_id: &str) -> bool {
        self.get_status(service_id).await == HealthStatus::Healthy
    }

    /// Runs the health monitor loop.
    ///
    /// This method runs until a shutdown signal is received, performing
    /// health checks on registered services at their configured intervals.
    pub async fn run(&self, mut shutdown: broadcast::Receiver<()>) {
        info!("Health monitor started");

        // Use a short tick interval to check for due health checks
        let tick_duration = Duration::from_millis(500);
        let mut tick = interval(tick_duration);

        loop {
            tokio::select! {
                _ = tick.tick() => {
                    self.run_due_checks().await;
                }
                _ = shutdown.recv() => {
                    info!("Health monitor shutting down");
                    break;
                }
            }
        }
    }

    /// Runs all health checks that are due.
    async fn run_due_checks(&self) {
        // Collect due checks
        let due_checks: Vec<HealthCheckRegistration> = {
            let registrations = self.registrations.read().await;
            registrations
                .values()
                .filter(|r| r.is_due())
                .cloned()
                .collect()
        };

        // Run checks concurrently
        for registration in due_checks {
            let service_id = registration.service_id.clone();
            self.run_check(registration).await;

            // Schedule next check
            let mut registrations = self.registrations.write().await;
            if let Some(reg) = registrations.get_mut(&service_id) {
                reg.schedule_next();
            }
        }
    }

    /// Runs a single health check.
    async fn run_check(&self, registration: HealthCheckRegistration) {
        let service_id = &registration.service_id;
        let path = &registration.http_check.path;
        let method = registration
            .http_check
            .method
            .as_ref()
            .copied()
            .unwrap_or_default();
        let expected_status = registration.http_check.expected_status.unwrap_or(200);
        let timeout = registration.timeout();

        let url = format!("http://127.0.0.1:{}{}", registration.port, path);

        debug!(
            service_id = %service_id,
            url = %url,
            method = ?method,
            "Running health check"
        );

        let start = Instant::now();

        let result = self
            .execute_http_check(&url, method, timeout, expected_status)
            .await;
        let elapsed = start.elapsed();

        // Record the result
        let mut health_states = self.health_states.write().await;
        if let Some(health) = health_states.get_mut(service_id) {
            match &result {
                Ok(status_code) => {
                    debug!(
                        service_id = %service_id,
                        status = %status_code,
                        elapsed_ms = %elapsed.as_millis(),
                        "Health check passed"
                    );
                    health.record(
                        HealthCheckResult::healthy_http(elapsed, *status_code),
                        self.config.max_history,
                    );
                }
                Err(error) => {
                    warn!(
                        service_id = %service_id,
                        error = %error,
                        elapsed_ms = %elapsed.as_millis(),
                        "Health check failed"
                    );
                    health.record(HealthCheckResult::unhealthy(error), self.config.max_history);
                }
            }
        }
    }

    /// Executes an HTTP health check.
    async fn execute_http_check(
        &self,
        url: &str,
        method: HttpMethod,
        timeout: Duration,
        expected_status: u16,
    ) -> Result<u16, String> {
        let req_method = match method {
            HttpMethod::Get => reqwest::Method::GET,
            HttpMethod::Head => reqwest::Method::HEAD,
            HttpMethod::Post => reqwest::Method::POST,
        };

        let client = self.client.clone();
        let request = client
            .request(req_method, url)
            .timeout(timeout)
            .build()
            .map_err(|e| format!("Failed to build request: {e}"))?;

        let response = client
            .execute(request)
            .await
            .map_err(|e| format!("Request failed: {e}"))?;

        let status = response.status().as_u16();

        if status == expected_status {
            Ok(status)
        } else {
            Err(format!(
                "Unexpected status: {status}, expected: {expected_status}"
            ))
        }
    }

    /// Performs an immediate health check for a service.
    ///
    /// This bypasses the regular scheduling and runs a check immediately.
    pub async fn check_now(&self, service_id: &str) -> Option<HealthCheckResult> {
        let registration = {
            let registrations = self.registrations.read().await;
            registrations.get(service_id).cloned()
        };

        let Some(registration) = registration else {
            warn!(service_id = %service_id, "No health check registered for service");
            return None;
        };

        let path = &registration.http_check.path;
        let method = registration
            .http_check
            .method
            .as_ref()
            .copied()
            .unwrap_or_default();
        let expected_status = registration.http_check.expected_status.unwrap_or(200);
        let timeout = registration.timeout();

        let url = format!("http://127.0.0.1:{}{}", registration.port, path);

        let start = Instant::now();
        let result = self
            .execute_http_check(&url, method, timeout, expected_status)
            .await;
        let elapsed = start.elapsed();

        let check_result = match result {
            Ok(status_code) => HealthCheckResult::healthy_http(elapsed, status_code),
            Err(error) => HealthCheckResult::unhealthy(error),
        };

        // Record the result
        let mut health_states = self.health_states.write().await;
        if let Some(health) = health_states.get_mut(service_id) {
            health.record(check_result.clone(), self.config.max_history);
        }

        Some(check_result)
    }

    /// Marks a service as starting (health checks not yet applicable).
    pub async fn mark_starting(&self, service_id: &str) {
        let mut health_states = self.health_states.write().await;
        if let Some(health) = health_states.get_mut(service_id) {
            health.status = HealthStatus::Starting;
        }
    }
}

impl std::fmt::Debug for HealthMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HealthMonitor")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// Shared reference to a health monitor.
pub type SharedHealthMonitor = Arc<HealthMonitor>;

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> HealthMonitorConfig {
        HealthMonitorConfig {
            default_interval: Duration::from_secs(1),
            default_timeout: Duration::from_secs(1),
            max_history: 10,
            failure_threshold: 3,
        }
    }

    fn test_http_check() -> HttpHealthCheck {
        use fabricks_common::models::common::Duration as CommonDuration;

        HttpHealthCheck {
            path: "/health".to_string(),
            port: Some(8080),
            interval: Some(CommonDuration::from_secs(1)),
            timeout: Some(CommonDuration::from_secs(1)),
            retries: Some(3),
            method: Some(HttpMethod::Get),
            expected_status: Some(200),
        }
    }

    #[tokio::test]
    async fn test_register_and_unregister() {
        let monitor = HealthMonitor::new(test_config()).expect("should create monitor");

        monitor
            .register("svc-1".to_string(), test_http_check(), 8080)
            .await;

        assert!(monitor.get_health("svc-1").await.is_some());
        assert_eq!(monitor.get_status("svc-1").await, HealthStatus::Unknown);

        monitor.unregister("svc-1").await;

        assert!(monitor.get_health("svc-1").await.is_none());
    }

    #[tokio::test]
    async fn test_get_all_health() {
        let monitor = HealthMonitor::new(test_config()).expect("should create monitor");

        monitor
            .register("svc-1".to_string(), test_http_check(), 8080)
            .await;
        monitor
            .register("svc-2".to_string(), test_http_check(), 8081)
            .await;

        let all = monitor.get_all_health().await;
        assert_eq!(all.len(), 2);
        assert!(all.contains_key("svc-1"));
        assert!(all.contains_key("svc-2"));
    }

    #[tokio::test]
    async fn test_is_healthy() {
        let monitor = HealthMonitor::new(test_config()).expect("should create monitor");

        monitor
            .register("svc-1".to_string(), test_http_check(), 8080)
            .await;

        // Initially not healthy (unknown)
        assert!(!monitor.is_healthy("svc-1").await);

        // Manually set to healthy for testing
        {
            let mut health_states = monitor.health_states.write().await;
            if let Some(health) = health_states.get_mut("svc-1") {
                health.status = HealthStatus::Healthy;
            }
        }

        assert!(monitor.is_healthy("svc-1").await);
    }

    #[tokio::test]
    async fn test_mark_starting() {
        let monitor = HealthMonitor::new(test_config()).expect("should create monitor");

        monitor
            .register("svc-1".to_string(), test_http_check(), 8080)
            .await;

        monitor.mark_starting("svc-1").await;

        assert_eq!(monitor.get_status("svc-1").await, HealthStatus::Starting);
    }

    #[tokio::test]
    async fn test_check_now_unregistered() {
        let monitor = HealthMonitor::new(test_config()).expect("should create monitor");

        let result = monitor.check_now("unknown-service").await;
        assert!(result.is_none());
    }

    #[test]
    fn test_health_check_registration() {
        let mut reg = HealthCheckRegistration::new("svc-1".to_string(), test_http_check(), 8080);

        // Should be due immediately after creation
        assert!(reg.is_due());

        // After scheduling next, should not be due
        reg.schedule_next();
        assert!(!reg.is_due());

        // Interval should be 1 second from the config
        assert_eq!(reg.interval(), Duration::from_secs(1));
        assert_eq!(reg.timeout(), Duration::from_secs(1));
    }
}
