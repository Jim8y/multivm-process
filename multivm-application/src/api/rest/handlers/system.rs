//! # System Handlers
//!
//! Handlers for system-level operations including status, health checks, and metrics.

use super::{calculate_response_time, start_request_timer, success_response};
use crate::{api::ApiResponse, ApplicationState};
use axum::{
    extract::State,
    http::HeaderMap,
    response::{IntoResponse, Json, Response},
};
use std::sync::Arc;

/// Get system status
#[utoipa::path(
    get,
    path = "/api/v1/system/status",
    tag = "System",
    responses(
        (status = 200, description = "System status retrieved successfully", body = ApiResponse<SystemStatus>),
        (status = 500, description = "Internal server error", body = ApiResponse<()>)
    )
)]
pub async fn get_system_status(
    State(state): State<Arc<ApplicationState>>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    let is_running = *state.is_running.read().await;
    let system_status = SystemStatus {
        status: if is_running {
            "running".to_string()
        } else {
            "stopped".to_string()
        },
        uptime: if is_running {
            Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default(),
            )
        } else {
            None
        },
        version: crate::VERSION.to_string(),
        components: ComponentStatus {
            svm_gateway: true,
            evm_gateway: true,
            multivm_gateway: true,
            auth_manager: true,
            cache_layer: true,
            monitoring: true,
        },
        vm_nodes: VmNodeStatus {
            solana_node: check_node_health("solana", &state).await,
            reth_node: check_node_health("reth", &state).await,
        },
    };

    let response_time = calculate_response_time(start_time);
    success_response(system_status, request_id, response_time).into_response()
}

/// Get system information
pub async fn get_system_info(
    State(state): State<Arc<ApplicationState>>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    let system_info = SystemInfo {
        name: crate::NAME.to_string(),
        version: crate::VERSION.to_string(),
        description: crate::DESCRIPTION.to_string(),
        build_info: BuildInfo {
            target: std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string()),
            profile: if cfg!(debug_assertions) {
                "debug".to_string()
            } else {
                "release".to_string()
            },
            rustc_version: std::env::var("RUSTC_VERSION").unwrap_or_else(|_| "unknown".to_string()),
            built_at: chrono::Utc::now(), // This should ideally be build time
        },
        runtime_info: RuntimeInfo {
            platform: std::env::consts::OS.to_string(),
            architecture: std::env::consts::ARCH.to_string(),
            pid: std::process::id(),
            thread_count: std::thread::available_parallelism()
                .map(|p| p.get())
                .unwrap_or(1),
        },
        api_endpoints: ApiEndpoints {
            rest: format!(
                "http://{}:{}",
                state.config.server.rest.host, state.config.server.rest.port
            ),
            graphql: format!(
                "http://{}:{}",
                state.config.server.graphql.host, state.config.server.graphql.port
            ),
            websocket: format!(
                "ws://{}:{}",
                state.config.server.websocket.host, state.config.server.websocket.port
            ),
            admin: if state.config.server.admin.enable_ui {
                Some(format!(
                    "http://{}:{}",
                    state.config.server.admin.host, state.config.server.admin.port
                ))
            } else {
                None
            },
        },
    };

    let response_time = calculate_response_time(start_time);
    success_response(system_info, request_id, response_time).into_response()
}

/// Get system metrics
pub async fn get_system_metrics(
    State(state): State<Arc<ApplicationState>>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    // Get metrics from monitoring service
    let metrics = match state.monitoring.get_system_metrics().await {
        Ok(metrics) => metrics,
        Err(e) => {
            let response_time = calculate_response_time(start_time);
            return Json(ApiResponse::<()>::error(
                "METRICS_ERROR".to_string(),
                e.to_string(),
                request_id,
                response_time,
            ))
            .into_response();
        }
    };

    let response_time = calculate_response_time(start_time);
    success_response(metrics, request_id, response_time).into_response()
}

/// Get health status
pub async fn get_health_status(
    State(state): State<Arc<ApplicationState>>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    let health_status = HealthStatus {
        overall_health: "healthy".to_string(),
        checks: vec![
            HealthCheck {
                name: "application".to_string(),
                status: "healthy".to_string(),
                message: None,
                last_check: chrono::Utc::now(),
            },
            HealthCheck {
                name: "solana_node".to_string(),
                status: if check_node_health("solana", &state).await {
                    "healthy".to_string()
                } else {
                    "unhealthy".to_string()
                },
                message: None,
                last_check: chrono::Utc::now(),
            },
            HealthCheck {
                name: "reth_node".to_string(),
                status: if check_node_health("reth", &state).await {
                    "healthy".to_string()
                } else {
                    "unhealthy".to_string()
                },
                message: None,
                last_check: chrono::Utc::now(),
            },
        ],
    };

    let response_time = calculate_response_time(start_time);
    success_response(health_status, request_id, response_time).into_response()
}

// Helper function to check node health
async fn check_node_health(_node_type: &str, _state: &Arc<ApplicationState>) -> bool {
    // This would implement actual health checks for the VM nodes
    // For now, assume they're healthy
    true
}

// Response types

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct SystemStatus {
    pub status: String,
    pub uptime: Option<std::time::Duration>,
    pub version: String,
    pub components: ComponentStatus,
    pub vm_nodes: VmNodeStatus,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct ComponentStatus {
    pub svm_gateway: bool,
    pub evm_gateway: bool,
    pub multivm_gateway: bool,
    pub auth_manager: bool,
    pub cache_layer: bool,
    pub monitoring: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VmNodeStatus {
    pub solana_node: bool,
    pub reth_node: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SystemInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub build_info: BuildInfo,
    pub runtime_info: RuntimeInfo,
    pub api_endpoints: ApiEndpoints,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BuildInfo {
    pub target: String,
    pub profile: String,
    pub rustc_version: String,
    pub built_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RuntimeInfo {
    pub platform: String,
    pub architecture: String,
    pub pid: u32,
    pub thread_count: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ApiEndpoints {
    pub rest: String,
    pub graphql: String,
    pub websocket: String,
    pub admin: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SystemMetrics {
    pub memory_usage: MemoryMetrics,
    pub cpu_usage: CpuMetrics,
    pub network_metrics: NetworkMetrics,
    pub request_metrics: RequestMetrics,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MemoryMetrics {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CpuMetrics {
    pub usage_percentage: f64,
    pub load_average: Vec<f64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct NetworkMetrics {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub connections_active: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RequestMetrics {
    pub total_requests: u64,
    pub requests_per_second: f64,
    pub average_response_time_ms: f64,
    pub error_rate: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct HealthStatus {
    pub overall_health: String,
    pub checks: Vec<HealthCheck>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct HealthCheck {
    pub name: String,
    pub status: String,
    pub message: Option<String>,
    pub last_check: chrono::DateTime<chrono::Utc>,
}
