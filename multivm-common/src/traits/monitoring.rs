use crate::{HealthStatus, MultivmError, ResourceLimits};
use async_trait::async_trait;
use std::time::Duration;

/// Trait for metrics collection and reporting
#[async_trait]
pub trait MetricsCollector: Send + Sync {
    /// Record a counter metric
    async fn increment_counter(
        &self,
        name: &str,
        value: u64,
        labels: Option<&[(&str, &str)]>,
    ) -> Result<(), MultivmError>;

    /// Record a gauge metric
    async fn set_gauge(
        &self,
        name: &str,
        value: f64,
        labels: Option<&[(&str, &str)]>,
    ) -> Result<(), MultivmError>;

    /// Record a histogram metric
    async fn record_histogram(
        &self,
        name: &str,
        value: f64,
        labels: Option<&[(&str, &str)]>,
    ) -> Result<(), MultivmError>;

    /// Record processing time
    async fn record_duration(
        &self,
        name: &str,
        duration: Duration,
        labels: Option<&[(&str, &str)]>,
    ) -> Result<(), MultivmError>;

    /// Get current metrics snapshot
    async fn get_metrics_snapshot(&self) -> Result<serde_json::Value, MultivmError>;
}

/// Trait for resource monitoring
#[async_trait]
pub trait ResourceMonitor: Send + Sync {
    /// Get current CPU usage percentage
    async fn get_cpu_usage(&self) -> Result<f64, MultivmError>;

    /// Get current memory usage in bytes
    async fn get_memory_usage(&self) -> Result<u64, MultivmError>;

    /// Get current disk usage in bytes
    async fn get_disk_usage(&self) -> Result<u64, MultivmError>;

    /// Get network I/O statistics
    async fn get_network_stats(&self) -> Result<NetworkStats, MultivmError>;

    /// Check if resource limits are exceeded
    async fn check_resource_limits(
        &self,
        limits: &ResourceLimits,
    ) -> Result<Vec<String>, MultivmError>;
}

/// Trait for health checking
#[async_trait]
pub trait HealthChecker: Send + Sync {
    /// Perform a health check
    async fn check_health(&self) -> Result<HealthStatus, MultivmError>;

    /// Get the health check interval
    fn health_check_interval(&self) -> Duration;

    /// Check if the component is healthy
    async fn is_healthy(&self) -> bool {
        self.check_health()
            .await
            .map(|s| s.is_operational())
            .unwrap_or(false)
    }
}

/// Comprehensive network statistics
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NetworkStats {
    // Connection metrics
    pub connected_peers: usize,

    // Message metrics
    pub messages_sent: u64,
    pub messages_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,

    // Protocol-specific metrics
    pub sent_by_protocol: std::collections::HashMap<String, u64>,
    pub received_by_protocol: std::collections::HashMap<String, u64>,

    // Bandwidth metrics
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub upload_rate: f64,   // bytes/second
    pub download_rate: f64, // bytes/second

    // Uptime
    pub uptime: std::time::Duration,
}
