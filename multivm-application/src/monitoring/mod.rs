pub mod dashboard;
pub mod health;
pub mod health_alerts;
pub mod health_checks;
pub mod metrics;
pub mod production_metrics;
pub mod tracing;

use crate::error::ApplicationResult;
use multivm_common::{Manager, ManagerState, MultivmResult};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Monitoring service that combines metrics, health checks, and tracing
#[derive(Debug)]
pub struct MonitoringService {
    pub metrics: Arc<metrics::MetricsService>,
    pub production_metrics: Arc<production_metrics::ProductionMetrics>,
    pub health: Arc<health::HealthCheckService>,
    pub tracing: Arc<tracing::TracingService>,
    pub dashboard: Arc<dashboard::DashboardService>,
    state: Arc<RwLock<ManagerState>>,
}

impl MonitoringService {
    /// Create new monitoring service
    pub async fn new(config: &crate::config::MonitoringConfig) -> ApplicationResult<Self> {
        let metrics = Arc::new(metrics::MetricsService::new(config).await?);
        let production_metrics = Arc::new(production_metrics::ProductionMetrics::new()?);
        let health = Arc::new(health::HealthCheckService::new(config).await?);
        let tracing = Arc::new(tracing::TracingService::new(config).await?);

        // Initialize tracing
        tracing.initialize().await?;

        // Dashboard service will be initialized later with ApplicationState
        let dashboard = Arc::new(dashboard::DashboardService::new());

        Ok(Self {
            metrics,
            production_metrics,
            health,
            tracing,
            dashboard,
            state: Arc::new(RwLock::new(ManagerState::Stopped)),
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

#[async_trait::async_trait]
impl Manager for MonitoringService {
    type Config = crate::config::MonitoringConfig;
    type State = ManagerState;

    async fn initialize(config: Self::Config) -> MultivmResult<Self> {
        Self::new(&config)
            .await
            .map_err(|e| multivm_common::MultivmError::Internal {
                component: "monitoring_service".to_string(),
                message: e.to_string(),
                error_code: None,
            })
    }

    async fn start(&mut self) -> MultivmResult<()> {
        let mut state = self.state.write().await;
        if *state == ManagerState::Running {
            return Ok(());
        }

        *state = ManagerState::Initializing;

        // Start metrics server
        if let Err(e) = self.start_metrics_server().await {
            *state = ManagerState::Error("Failed to start metrics server".to_string());
            return Err(multivm_common::MultivmError::Internal {
                component: "metrics_server".to_string(),
                message: e.to_string(),
                error_code: None,
            });
        }

        // Start health check server
        if let Err(e) = self.start_health_check_server().await {
            *state = ManagerState::Error("Failed to start health check server".to_string());
            return Err(multivm_common::MultivmError::Internal {
                component: "health_check_server".to_string(),
                message: e.to_string(),
                error_code: None,
            });
        }

        *state = ManagerState::Running;
        Ok(())
    }

    async fn stop(&mut self) -> MultivmResult<()> {
        let mut state = self.state.write().await;
        *state = ManagerState::Stopped;
        Ok(())
    }

    async fn get_state(&self) -> ManagerState {
        self.state.read().await.clone()
    }

    async fn get_stats(&self) -> MultivmResult<multivm_common::ProcessingMetrics> {
        Ok(multivm_common::ProcessingMetrics {
            cpu_time: std::time::Duration::ZERO,
            memory_usage_bytes: 0,
            disk_reads: 0,
            disk_writes: 0,
            network_bytes: 0,
            compute_units_used: 0,
            transaction_count: 0,
            account_updates: 0,
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            average_response_time_ms: 0.0,
            peak_memory_usage_mb: 0,
            cpu_usage_percent: 0.0,
        })
    }

    async fn health_check(&self) -> MultivmResult<multivm_common::HealthStatus> {
        let state = self.get_state().await;
        Ok(match state {
            ManagerState::Running => multivm_common::HealthStatus::Healthy,
            ManagerState::Initializing | ManagerState::Stopping => {
                multivm_common::HealthStatus::Degraded
            }
            ManagerState::Stopped | ManagerState::Error(_) => {
                multivm_common::HealthStatus::Unhealthy
            }
            ManagerState::Uninitialized => multivm_common::HealthStatus::Unhealthy,
        })
    }
}

// Re-export types
pub use health::{HealthCheckService, HealthReport, HealthStatus};
pub use metrics::MetricsService;
pub use production_metrics::ProductionMetrics;
pub use tracing::{TraceSpan, TracingService};
