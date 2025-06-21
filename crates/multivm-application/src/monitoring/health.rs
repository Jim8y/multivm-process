//! Health check service

use crate::error::{ApplicationError, ApplicationResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Health check service
#[derive(Debug)]
pub struct HealthCheckService {
    status: Arc<RwLock<HealthStatus>>,
    config: crate::config::HealthCheckConfig,
}

impl HealthCheckService {
    /// Create new health check service
    pub async fn new(config: &crate::config::HealthCheckConfig) -> ApplicationResult<Self> {
        Ok(Self {
            status: Arc::new(RwLock::new(HealthStatus::default())),
            config: config.clone(),
        })
    }

    /// Start health check HTTP server
    pub async fn start_server(&self) -> ApplicationResult<()> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        tracing::info!("Starting health check server on {}", addr);

        // Start HTTP server with health check endpoints
        use warp::Filter;

        let health_route = warp::path("health")
            .and(warp::get())
            .map(|| warp::reply::with_status("OK", warp::http::StatusCode::OK));

        let ready_route = warp::path("ready")
            .and(warp::get())
            .map(|| warp::reply::with_status("Ready", warp::http::StatusCode::OK));

        let live_route = warp::path("live")
            .and(warp::get())
            .map(|| warp::reply::with_status("Alive", warp::http::StatusCode::OK));

        let routes = health_route.or(ready_route).or(live_route);

        let socket_addr: std::net::SocketAddr =
            addr.parse()
                .map_err(|e| ApplicationError::ConfigurationError {
                    component: "health".to_string(),
                    message: format!("Invalid bind address: {}", e),
                })?;

        tokio::spawn(async move {
            warp::serve(routes).run(socket_addr).await;
        });

        tracing::info!("Health check server started on {}", addr);
        Ok(())
    }

    /// Perform health checks
    pub async fn check_health(&self) -> ApplicationResult<HealthReport> {
        let mut report = HealthReport {
            status: ServiceStatus::Healthy,
            timestamp: chrono::Utc::now(),
            services: vec![],
            checks: vec![],
        };

        // Check REST API
        let rest_check = self.check_service("rest_api").await;
        report.checks.push(rest_check.clone());
        if !rest_check.healthy {
            report.status = ServiceStatus::Degraded;
        }

        // Check GraphQL
        let graphql_check = self.check_service("graphql").await;
        report.checks.push(graphql_check.clone());
        if !graphql_check.healthy {
            report.status = ServiceStatus::Degraded;
        }

        // Check WebSocket
        let ws_check = self.check_service("websocket").await;
        report.checks.push(ws_check.clone());
        if !ws_check.healthy {
            report.status = ServiceStatus::Degraded;
        }

        // Check Cache
        let cache_check = self.check_service("cache").await;
        report.checks.push(cache_check.clone());
        if !cache_check.healthy && cache_check.severity == CheckSeverity::Critical {
            report.status = ServiceStatus::Unhealthy;
        }

        // Check VM connections
        let svm_check = self.check_service("svm_connection").await;
        report.checks.push(svm_check.clone());
        if !svm_check.healthy && svm_check.severity == CheckSeverity::Critical {
            report.status = ServiceStatus::Unhealthy;
        }

        let evm_check = self.check_service("evm_connection").await;
        report.checks.push(evm_check.clone());
        if !evm_check.healthy && evm_check.severity == CheckSeverity::Critical {
            report.status = ServiceStatus::Unhealthy;
        }

        // Update internal status
        let mut status = self.status.write().await;
        status.last_check = chrono::Utc::now();
        status.overall_status = report.status.clone();
        status.failing_checks = report
            .checks
            .iter()
            .filter(|c| !c.healthy)
            .map(|c| c.name.clone())
            .collect();

        Ok(report)
    }

    /// Check individual service health
    async fn check_service(&self, service: &str) -> HealthCheck {
        // Mock implementation - would perform actual health checks
        match service {
            "rest_api" | "graphql" | "websocket" => HealthCheck {
                name: service.to_string(),
                healthy: true,
                severity: CheckSeverity::Critical,
                message: "Service is running".to_string(),
                duration_ms: 5,
            },
            "cache" => HealthCheck {
                name: service.to_string(),
                healthy: true,
                severity: CheckSeverity::Warning,
                message: "Cache is operational".to_string(),
                duration_ms: 10,
            },
            "svm_connection" | "evm_connection" => HealthCheck {
                name: service.to_string(),
                healthy: true,
                severity: CheckSeverity::Critical,
                message: "Connection established".to_string(),
                duration_ms: 20,
            },
            _ => HealthCheck {
                name: service.to_string(),
                healthy: false,
                severity: CheckSeverity::Warning,
                message: "Unknown service".to_string(),
                duration_ms: 0,
            },
        }
    }

    /// Get current health status
    pub async fn get_status(&self) -> HealthStatus {
        self.status.read().await.clone()
    }

    /// Set service as healthy
    pub async fn set_healthy(&self, service: &str) {
        let mut status = self.status.write().await;
        status.failing_checks.retain(|s| s != service);
        if status.failing_checks.is_empty() {
            status.overall_status = ServiceStatus::Healthy;
        }
    }

    /// Set service as unhealthy
    pub async fn set_unhealthy(&self, service: &str, severity: CheckSeverity) {
        let mut status = self.status.write().await;
        if !status.failing_checks.contains(&service.to_string()) {
            status.failing_checks.push(service.to_string());
        }

        match severity {
            CheckSeverity::Critical => status.overall_status = ServiceStatus::Unhealthy,
            CheckSeverity::Warning => {
                if status.overall_status == ServiceStatus::Healthy {
                    status.overall_status = ServiceStatus::Degraded;
                }
            }
            CheckSeverity::Info => {}
        }
    }
}

/// Health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub overall_status: ServiceStatus,
    pub last_check: chrono::DateTime<chrono::Utc>,
    pub failing_checks: Vec<String>,
}

impl Default for HealthStatus {
    fn default() -> Self {
        Self {
            overall_status: ServiceStatus::Healthy,
            last_check: chrono::Utc::now(),
            failing_checks: vec![],
        }
    }
}

/// Service status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ServiceStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Health report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub status: ServiceStatus,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub services: Vec<ServiceHealth>,
    pub checks: Vec<HealthCheck>,
}

/// Individual service health
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceHealth {
    pub name: String,
    pub status: ServiceStatus,
    pub uptime: std::time::Duration,
    pub last_error: Option<String>,
}

/// Health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub name: String,
    pub healthy: bool,
    pub severity: CheckSeverity,
    pub message: String,
    pub duration_ms: u64,
}

/// Check severity
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CheckSeverity {
    Info,
    Warning,
    Critical,
}
