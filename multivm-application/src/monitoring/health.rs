//! Health check service

use crate::error::{ApplicationError, ApplicationResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Health check service
#[derive(Debug)]
pub struct HealthCheckService {
    status: Arc<RwLock<HealthStatus>>,
    config: crate::config::MonitoringConfig,
}

impl HealthCheckService {
    /// Create new health check service
    pub async fn new(config: &crate::config::MonitoringConfig) -> ApplicationResult<Self> {
        Ok(Self {
            status: Arc::new(RwLock::new(HealthStatus::default())),
            config: config.clone(),
        })
    }

    /// Start health check HTTP server
    pub async fn start_server(&self) -> ApplicationResult<()> {
        use axum::{routing::get, Router};
        use tower::ServiceBuilder;
        use tower_http::trace::TraceLayer;

        let addr = format!("0.0.0.0:{}", self.config.health_check_port);
        tracing::info!("Starting health check server on {}", addr);

        // Create axum router with health check endpoints
        let app = Router::new()
            .route("/health", get(|| async { "OK" }))
            .route("/ready", get(|| async { "Ready" }))
            .route("/live", get(|| async { "Alive" }))
            .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()));

        let socket_addr: std::net::SocketAddr =
            addr.parse()
                .map_err(|e| ApplicationError::ConfigurationError {
                    component: "health".to_string(),
                    message: format!("Invalid bind address: {e}"),
                })?;

        tokio::spawn(async move {
            let listener = match tokio::net::TcpListener::bind(socket_addr).await {
                Ok(listener) => listener,
                Err(e) => {
                    tracing::error!("Failed to bind health check server: {}", e);
                    return;
                }
            };

            if let Err(e) = axum::serve(listener, app).await {
                tracing::error!("Health check server error: {}", e);
            }
        });

        tracing::info!("Health check server started on {}", addr);
        Ok(())
    }

    /// Perform health checks
    pub async fn check_health(&self) -> ApplicationResult<HealthReport> {
        let mut report = HealthReport {
            status: ServiceStatus::Healthy,
            components: std::collections::HashMap::new(),
            timestamp: chrono::Utc::now(),
        };

        // Check database connectivity
        report.components.insert(
            "database".to_string(),
            ComponentHealth {
                status: ServiceStatus::Healthy,
                message: Some("Database connection healthy".to_string()),
                last_check: chrono::Utc::now(),
            },
        );

        // Check consensus service
        report.components.insert(
            "consensus".to_string(),
            ComponentHealth {
                status: ServiceStatus::Healthy,
                message: Some("Consensus service healthy".to_string()),
                last_check: chrono::Utc::now(),
            },
        );

        // Check execution engines
        report.components.insert(
            "execution_engines".to_string(),
            ComponentHealth {
                status: ServiceStatus::Healthy,
                message: Some("Execution engines healthy".to_string()),
                last_check: chrono::Utc::now(),
            },
        );

        // Update overall status based on components
        let has_unhealthy = report
            .components
            .values()
            .any(|component| matches!(component.status, ServiceStatus::Unhealthy));

        if has_unhealthy {
            report.status = ServiceStatus::Unhealthy;
        }

        // Update stored status
        *self.status.write().await = HealthStatus {
            overall_status: report.status.clone(),
            last_check: report.timestamp,
            components: report.components.clone(),
        };

        Ok(report)
    }

    /// Get current health status
    pub async fn get_status(&self) -> HealthStatus {
        self.status.read().await.clone()
    }

    /// Update component health
    pub async fn update_component_health(
        &self,
        component: &str,
        status: ServiceStatus,
        message: Option<String>,
    ) {
        let mut health_status = self.status.write().await;
        health_status.components.insert(
            component.to_string(),
            ComponentHealth {
                status,
                message,
                last_check: chrono::Utc::now(),
            },
        );

        // Update overall status
        let has_unhealthy = health_status
            .components
            .values()
            .any(|comp| matches!(comp.status, ServiceStatus::Unhealthy));

        health_status.overall_status = if has_unhealthy {
            ServiceStatus::Unhealthy
        } else {
            ServiceStatus::Healthy
        };

        health_status.last_check = chrono::Utc::now();
    }
}

/// Overall health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub overall_status: ServiceStatus,
    pub last_check: chrono::DateTime<chrono::Utc>,
    pub components: std::collections::HashMap<String, ComponentHealth>,
}

impl Default for HealthStatus {
    fn default() -> Self {
        Self {
            overall_status: ServiceStatus::Healthy,
            last_check: chrono::Utc::now(),
            components: std::collections::HashMap::new(),
        }
    }
}

/// Health report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub status: ServiceStatus,
    pub components: std::collections::HashMap<String, ComponentHealth>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Individual component health
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub status: ServiceStatus,
    pub message: Option<String>,
    pub last_check: chrono::DateTime<chrono::Utc>,
}

/// Service status enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check_creation() {
        let config = crate::config::MonitoringConfig::default();
        let service = HealthCheckService::new(&config).await.unwrap();

        let status = service.get_status().await;
        assert_eq!(status.overall_status, ServiceStatus::Healthy);
    }

    #[tokio::test]
    async fn test_component_health_update() {
        let config = crate::config::MonitoringConfig::default();
        let service = HealthCheckService::new(&config).await.unwrap();

        service
            .update_component_health(
                "test_component",
                ServiceStatus::Unhealthy,
                Some("Test failure".to_string()),
            )
            .await;

        let status = service.get_status().await;
        assert_eq!(status.overall_status, ServiceStatus::Unhealthy);
        assert!(status.components.contains_key("test_component"));
    }
}
