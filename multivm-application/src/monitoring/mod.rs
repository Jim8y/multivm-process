pub mod health;
pub mod metrics;
pub mod tracing;

use crate::error::ApplicationResult;
use std::sync::Arc;

/// Monitoring service that combines metrics, health checks, and tracing
#[derive(Debug)]
pub struct MonitoringService {
    pub metrics: Arc<metrics::MetricsService>,
    pub health: Arc<health::HealthCheckService>,
    pub tracing: Arc<tracing::TracingService>,
}

impl MonitoringService {
    /// Create new monitoring service
    pub async fn new(config: &crate::config::MonitoringConfig) -> ApplicationResult<Self> {
        let metrics = Arc::new(metrics::MetricsService::new(&config.metrics).await?);
        let health = Arc::new(health::HealthCheckService::new(&config.health_check).await?);
        let tracing = Arc::new(tracing::TracingService::new(&config.tracing).await?);

        // Initialize tracing
        tracing.initialize().await?;

        Ok(Self {
            metrics,
            health,
            tracing,
        })
    }

    /// Get system metrics
    pub async fn get_system_metrics(
        &self,
    ) -> ApplicationResult<crate::api::rest::handlers::system::SystemMetrics> {
        // Mock implementation - combine metrics from different sources
        Ok(crate::api::rest::handlers::system::SystemMetrics {
            memory_usage: crate::api::rest::handlers::system::MemoryMetrics {
                total_bytes: 8 * 1024 * 1024 * 1024,     // 8GB
                used_bytes: 2 * 1024 * 1024 * 1024,      // 2GB used
                available_bytes: 6 * 1024 * 1024 * 1024, // 6GB available
            },
            cpu_usage: crate::api::rest::handlers::system::CpuMetrics {
                usage_percentage: 25.0,
                load_average: vec![1.5, 1.2, 1.0],
            },
            network_metrics: crate::api::rest::handlers::system::NetworkMetrics {
                bytes_sent: 1024 * 1024 * 100,     // 100MB sent
                bytes_received: 1024 * 1024 * 200, // 200MB received
                connections_active: 50,
            },
            request_metrics: crate::api::rest::handlers::system::RequestMetrics {
                total_requests: 1000,
                requests_per_second: 10.0,
                average_response_time_ms: 150.0,
                error_rate: 0.02, // 2% error rate
            },
        })
    }

    /// Record request metrics
    pub async fn record_request_metrics(
        &self,
        method: &str,
        path: &str,
        status: u16,
        duration_ms: u64,
    ) -> ApplicationResult<()> {
        self.metrics
            .record_http_request(method, path, status, duration_ms as f64);
        Ok(())
    }

    /// Start metrics server
    pub async fn start_metrics_server(&self) -> ApplicationResult<()> {
        self.metrics.start_server().await
    }

    /// Start health check server
    pub async fn start_health_check_server(&self) -> ApplicationResult<()> {
        self.health.start_server().await
    }
}

// Re-export types
pub use health::{HealthCheckService, HealthReport, HealthStatus};
pub use metrics::MetricsService;
pub use tracing::{TraceSpan, TracingService};
