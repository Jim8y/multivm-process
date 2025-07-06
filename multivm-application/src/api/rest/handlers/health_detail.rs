//! Detailed health check endpoint for comprehensive system status

use crate::ApplicationState;
use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;

/// Detailed health check request parameters
#[derive(Debug, Deserialize)]
pub struct DetailedHealthRequest {
    /// Include component details
    #[serde(default = "default_true")]
    pub include_components: bool,

    /// Include metrics
    #[serde(default = "default_true")]
    pub include_metrics: bool,

    /// Include recent errors
    #[serde(default)]
    pub include_errors: bool,

    /// Include dependencies
    #[serde(default = "default_true")]
    pub include_dependencies: bool,
}

fn default_true() -> bool {
    true
}

/// Detailed health response
#[derive(Debug, Serialize)]
pub struct DetailedHealthResponse {
    /// Overall status
    pub status: HealthStatus,

    /// Human-readable status message
    pub message: String,

    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// System uptime (seconds)
    pub uptime_seconds: f64,

    /// Component health details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<ComponentHealthDetails>,

    /// System metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<SystemMetrics>,

    /// Recent errors
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recent_errors: Option<Vec<ErrorInfo>>,

    /// Dependency status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<DependencyStatus>,

    /// Version information
    pub version: VersionInfo,
}

/// Overall health status
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Everything is working normally
    Healthy,
    /// Some components are degraded but system is operational
    Degraded,
    /// System is not operational
    Unhealthy,
}

/// Component health details
#[derive(Debug, Serialize)]
pub struct ComponentHealthDetails {
    /// Individual component statuses
    pub components: Vec<ComponentStatus>,

    /// Total healthy components
    pub healthy_count: usize,

    /// Total degraded components
    pub degraded_count: usize,

    /// Total unhealthy components
    pub unhealthy_count: usize,
}

/// Individual component status
#[derive(Debug, Serialize)]
pub struct ComponentStatus {
    /// Component name
    pub name: String,

    /// Component status
    pub status: HealthStatus,

    /// Status message
    pub message: String,

    /// Last check timestamp
    pub last_check: chrono::DateTime<chrono::Utc>,

    /// Response time (ms)
    pub response_time_ms: f64,
}

/// System metrics
#[derive(Debug, Serialize)]
pub struct SystemMetrics {
    /// CPU usage percentage
    pub cpu_usage_percent: f64,

    /// Memory usage
    pub memory: MemoryMetrics,

    /// Disk usage
    pub disk: DiskMetrics,

    /// Network metrics
    pub network: NetworkMetrics,

    /// Request metrics
    pub requests: RequestMetrics,
}

/// Memory metrics
#[derive(Debug, Serialize)]
pub struct MemoryMetrics {
    /// Used memory (MB)
    pub used_mb: u64,

    /// Total memory (MB)
    pub total_mb: u64,

    /// Usage percentage
    pub usage_percent: f64,
}

/// Disk metrics
#[derive(Debug, Serialize)]
pub struct DiskMetrics {
    /// Used disk space (GB)
    pub used_gb: f64,

    /// Total disk space (GB)
    pub total_gb: f64,

    /// Usage percentage
    pub usage_percent: f64,
}

/// Network metrics
#[derive(Debug, Serialize)]
pub struct NetworkMetrics {
    /// Active connections
    pub active_connections: u32,

    /// Bytes sent
    pub bytes_sent: u64,

    /// Bytes received
    pub bytes_received: u64,
}

/// Request metrics
#[derive(Debug, Serialize)]
pub struct RequestMetrics {
    /// Total requests
    pub total_requests: u64,

    /// Requests per second
    pub requests_per_second: f64,

    /// Average response time (ms)
    pub avg_response_time_ms: f64,

    /// Error rate
    pub error_rate: f64,
}

/// Error information
#[derive(Debug, Serialize)]
pub struct ErrorInfo {
    /// Error timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Error type
    pub error_type: String,

    /// Error message
    pub message: String,

    /// Component that generated the error
    pub component: String,
}

/// Dependency status
#[derive(Debug, Serialize)]
pub struct DependencyStatus {
    /// Database status
    pub database: ServiceStatus,

    /// Cache status
    pub cache: ServiceStatus,

    /// Consensus network status
    pub consensus_network: ServiceStatus,

    /// External VM processes
    pub vm_processes: VmProcessStatus,
}

/// Service status
#[derive(Debug, Serialize)]
pub struct ServiceStatus {
    /// Service name
    pub name: String,

    /// Is available
    pub available: bool,

    /// Response time (ms)
    pub response_time_ms: Option<f64>,

    /// Additional info
    pub info: Option<String>,
}

/// VM process status
#[derive(Debug, Serialize)]
pub struct VmProcessStatus {
    /// EVM status
    pub evm: ServiceStatus,

    /// SVM status
    pub svm: ServiceStatus,
}

/// Version information
#[derive(Debug, Serialize)]
pub struct VersionInfo {
    /// Application version
    pub version: String,

    /// Git commit hash
    pub commit: Option<String>,

    /// Build timestamp
    pub build_time: Option<String>,

    /// Rust version
    pub rust_version: String,
}

/// Get detailed health status
pub async fn get_detailed_health(
    State(state): State<Arc<ApplicationState>>,
    Json(params): Json<DetailedHealthRequest>,
) -> impl IntoResponse {
    let start = Instant::now();

    // Get overall health status
    let health_report =
        match crate::monitoring::health_checks::perform_full_health_check(&state).await {
            Ok(report) => report,
            Err(e) => {
                return Json(DetailedHealthResponse {
                    status: HealthStatus::Unhealthy,
                    message: format!("Health check failed: {}", e),
                    timestamp: chrono::Utc::now(),
                    uptime_seconds: 0.0,
                    components: None,
                    metrics: None,
                    recent_errors: None,
                    dependencies: None,
                    version: VersionInfo {
                        version: crate::VERSION.to_string(),
                        commit: None,
                        build_time: None,
                        rust_version: std::env!("CARGO_PKG_RUST_VERSION").to_string(),
                    },
                });
            }
        };

    let overall_status = if health_report.overall_health {
        HealthStatus::Healthy
    } else {
        // Check if any critical components are down
        let critical_down = health_report.checks.iter().any(|c| {
            !c.healthy
                && matches!(
                    c.name.as_str(),
                    "consensus" | "process_manager" | "database"
                )
        });

        if critical_down {
            HealthStatus::Unhealthy
        } else {
            HealthStatus::Degraded
        }
    };

    let message = match overall_status {
        HealthStatus::Healthy => "All systems operational".to_string(),
        HealthStatus::Degraded => "System degraded but operational".to_string(),
        HealthStatus::Unhealthy => "System is experiencing issues".to_string(),
    };

    let mut response = DetailedHealthResponse {
        status: overall_status,
        message,
        timestamp: chrono::Utc::now(),
        uptime_seconds: state.start_time.elapsed().as_secs_f64(),
        components: None,
        metrics: None,
        recent_errors: None,
        dependencies: None,
        version: VersionInfo {
            version: crate::VERSION.to_string(),
            commit: option_env!("GIT_COMMIT").map(|s| s.to_string()),
            build_time: option_env!("BUILD_TIME").map(|s| s.to_string()),
            rust_version: env!("CARGO_PKG_RUST_VERSION").to_string(),
        },
    };

    // Add component details if requested
    if params.include_components {
        let mut components = Vec::new();
        let mut healthy_count = 0;
        let mut degraded_count = 0;
        let mut unhealthy_count = 0;

        for check in &health_report.checks {
            let status = if check.healthy {
                healthy_count += 1;
                HealthStatus::Healthy
            } else if check.name == "cache" || check.name == "evm" || check.name == "svm" {
                degraded_count += 1;
                HealthStatus::Degraded
            } else {
                unhealthy_count += 1;
                HealthStatus::Unhealthy
            };

            components.push(ComponentStatus {
                name: check.name.clone(),
                status,
                message: check.message.clone(),
                last_check: health_report.timestamp,
                response_time_ms: 0.0, // Would be tracked in real implementation
            });
        }

        response.components = Some(ComponentHealthDetails {
            components,
            healthy_count,
            degraded_count,
            unhealthy_count,
        });
    }

    // Add metrics if requested
    if params.include_metrics {
        if let Ok(system_metrics) = state.monitoring.get_system_metrics().await {
            response.metrics = Some(SystemMetrics {
                cpu_usage_percent: system_metrics.cpu_usage.usage_percentage,
                memory: MemoryMetrics {
                    used_mb: system_metrics.memory_usage.used_bytes / (1024 * 1024),
                    total_mb: system_metrics.memory_usage.total_bytes / (1024 * 1024),
                    usage_percent: (system_metrics.memory_usage.used_bytes as f64
                        / system_metrics.memory_usage.total_bytes as f64)
                        * 100.0,
                },
                disk: DiskMetrics {
                    used_gb: 100.0, // Mock data
                    total_gb: 500.0,
                    usage_percent: 20.0,
                },
                network: NetworkMetrics {
                    active_connections: system_metrics.network_metrics.connections_active as u32,
                    bytes_sent: system_metrics.network_metrics.bytes_sent,
                    bytes_received: system_metrics.network_metrics.bytes_received,
                },
                requests: RequestMetrics {
                    total_requests: system_metrics.request_metrics.total_requests,
                    requests_per_second: system_metrics.request_metrics.requests_per_second,
                    avg_response_time_ms: system_metrics.request_metrics.average_response_time_ms,
                    error_rate: system_metrics.request_metrics.error_rate,
                },
            });
        }
    }

    // Add recent errors if requested
    if params.include_errors {
        // Would fetch from error tracking system
        response.recent_errors = Some(vec![]);
    }

    // Add dependency status if requested
    if params.include_dependencies {
        let db_start = Instant::now();
        let (db_healthy, db_msg) = crate::monitoring::health_checks::check_database_health(&state)
            .await
            .unwrap_or((false, "Database check failed".to_string()));
        let db_time = db_start.elapsed().as_millis() as f64;

        let cache_start = Instant::now();
        let (cache_healthy, cache_msg) =
            crate::monitoring::health_checks::check_cache_health(&state)
                .await
                .unwrap_or((false, "Cache check failed".to_string()));
        let cache_time = cache_start.elapsed().as_millis() as f64;

        let consensus_healthy = state.consensus_manager.read().await.is_some();

        let (evm_healthy, evm_msg) = crate::monitoring::health_checks::check_evm_health(&state)
            .await
            .unwrap_or((false, "EVM check failed".to_string()));
        let (svm_healthy, svm_msg) = crate::monitoring::health_checks::check_svm_health(&state)
            .await
            .unwrap_or((false, "SVM check failed".to_string()));

        response.dependencies = Some(DependencyStatus {
            database: ServiceStatus {
                name: "PostgreSQL".to_string(),
                available: db_healthy,
                response_time_ms: Some(db_time),
                info: Some(db_msg),
            },
            cache: ServiceStatus {
                name: "Redis".to_string(),
                available: cache_healthy,
                response_time_ms: Some(cache_time),
                info: Some(cache_msg),
            },
            consensus_network: ServiceStatus {
                name: "Malachite Consensus".to_string(),
                available: consensus_healthy,
                response_time_ms: None,
                info: if consensus_healthy {
                    Some("Consensus network connected".to_string())
                } else {
                    Some("Consensus not initialized".to_string())
                },
            },
            vm_processes: VmProcessStatus {
                evm: ServiceStatus {
                    name: "Reth (EVM)".to_string(),
                    available: evm_healthy,
                    response_time_ms: None,
                    info: Some(evm_msg),
                },
                svm: ServiceStatus {
                    name: "Solana (SVM)".to_string(),
                    available: svm_healthy,
                    response_time_ms: None,
                    info: Some(svm_msg),
                },
            },
        });
    }

    let _total_time = start.elapsed();

    Json(response)
}

/// Simple health check endpoint (Kubernetes compatible)
pub async fn simple_health_check(State(state): State<Arc<ApplicationState>>) -> impl IntoResponse {
    let health_report =
        match crate::monitoring::health_checks::perform_full_health_check(&state).await {
            Ok(report) => report,
            Err(_) => {
                return Json(SimpleHealthResponse {
                    status: "unhealthy".to_string(),
                    timestamp: chrono::Utc::now(),
                });
            }
        };

    if health_report.overall_health {
        Json(SimpleHealthResponse {
            status: "ok".to_string(),
            timestamp: chrono::Utc::now(),
        })
    } else {
        Json(SimpleHealthResponse {
            status: "unhealthy".to_string(),
            timestamp: chrono::Utc::now(),
        })
    }
}

/// Simple health response for Kubernetes
#[derive(Debug, Serialize)]
pub struct SimpleHealthResponse {
    pub status: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
