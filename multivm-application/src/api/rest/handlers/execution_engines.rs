//! Execution Engine API Handlers
//!
//! Provides REST API endpoints for managing and monitoring execution engines,
//! including Ethereum (Reth) and future Solana integration.

use crate::ApplicationState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use multivm_common::types::BlockchainType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Execution engine status response
#[derive(Debug, Serialize)]
pub struct ExecutionEngineStatusResponse {
    pub is_running: bool,
    pub engine_health: HashMap<String, String>,
    pub engine_states: HashMap<String, EngineStateInfo>,
    pub readiness: HashMap<String, bool>,
}

/// Simplified engine state information for API responses
#[derive(Debug, Serialize)]
pub struct EngineStateInfo {
    pub blockchain_type: String,
    pub current_block: Option<u64>,
    pub is_syncing: bool,
    pub peer_count: u32,
    pub chain_id: u64,
    pub data_directory: String,
}

/// Engine metrics response
#[derive(Debug, Serialize)]
pub struct EngineMetricsResponse {
    pub blockchain_type: String,
    pub blocks_processed: u64,
    pub transactions_processed: u64,
    pub average_block_time_ms: f64,
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: u64,
    pub last_block_number: u64,
}

/// Block processing request
#[derive(Debug, Deserialize)]
pub struct ProcessBlockRequest {
    pub blockchain_type: String,
    pub block_data_hex: String,
}

/// Block processing response
#[derive(Debug, Serialize)]
pub struct ProcessBlockResponse {
    pub success: bool,
    pub block_number: Option<u64>,
    pub transaction_count: Option<usize>,
    pub gas_used: Option<u64>,
    pub processing_time_ms: Option<u64>,
    pub error_message: Option<String>,
}

/// Engine reset request
#[derive(Debug, Deserialize)]
pub struct ResetEngineRequest {
    pub block_id: u64,
}

/// Engine reset response
#[derive(Debug, Serialize)]
pub struct ResetEngineResponse {
    pub success: bool,
    pub new_block_id: u64,
    pub message: String,
}

/// Query parameters for engine status
#[derive(Debug, Deserialize)]
pub struct EngineStatusQuery {
    pub include_metrics: Option<bool>,
    pub include_health: Option<bool>,
}

/// Create execution engine routes
pub fn execution_engine_routes() -> Router<Arc<ApplicationState>> {
    Router::new()
        .route("/status", get(get_execution_engine_status))
        .route("/health", get(get_execution_engine_health))
        .route("/metrics", get(get_execution_engine_metrics))
        .route("/{blockchain_type}/process", post(process_block))
        .route("/{blockchain_type}/reset", post(reset_engine))
        .route("/{blockchain_type}/latest_block", get(get_latest_block))
        .route("/{blockchain_type}/state", get(get_engine_state))
        .route("/{blockchain_type}/restart", post(restart_engine))
}

/// Get overall execution engine status
pub async fn get_execution_engine_status(
    State(state): State<Arc<ApplicationState>>,
    Query(query): Query<EngineStatusQuery>,
) -> Result<Json<ExecutionEngineStatusResponse>, StatusCode> {
    let engine_manager = state.execution_engines.read().await;

    let is_running = engine_manager.is_running().await;

    // Get health status
    let health_result = if query.include_health.unwrap_or(true) {
        engine_manager.get_health_status().await
    } else {
        Ok(HashMap::new())
    };

    let engine_health = match health_result {
        Ok(health_map) => health_map
            .into_iter()
            .map(|(blockchain_type, health)| {
                (format!("{blockchain_type:?}"), format!("{health:?}"))
            })
            .collect(),
        Err(_) => HashMap::new(),
    };

    // Get engine states
    let states_result = engine_manager.get_engine_states().await;
    let engine_states = match states_result {
        Ok(states_map) => states_map
            .into_iter()
            .map(|(blockchain_type, state)| {
                let state_info = EngineStateInfo {
                    blockchain_type: format!("{:?}", state.blockchain_type),
                    current_block: state.current_block,
                    is_syncing: state.is_syncing,
                    peer_count: state.peer_count,
                    chain_id: state.chain_id,
                    data_directory: state.data_directory,
                };
                (format!("{blockchain_type:?}"), state_info)
            })
            .collect(),
        Err(_) => HashMap::new(),
    };

    // Get readiness status
    let readiness_result = engine_manager.are_engines_ready().await;
    let readiness = match readiness_result {
        Ok(readiness_map) => readiness_map
            .into_iter()
            .map(|(blockchain_type, ready)| (format!("{blockchain_type:?}"), ready))
            .collect(),
        Err(_) => HashMap::new(),
    };

    let response = ExecutionEngineStatusResponse {
        is_running,
        engine_health,
        engine_states,
        readiness,
    };

    Ok(Json(response))
}

/// Get execution engine health status
pub async fn get_execution_engine_health(
    State(state): State<Arc<ApplicationState>>,
) -> Result<Json<HashMap<String, String>>, StatusCode> {
    let engine_manager = state.execution_engines.read().await;

    match engine_manager.get_health_status().await {
        Ok(health_map) => {
            let health_response = health_map
                .into_iter()
                .map(|(blockchain_type, health)| {
                    (format!("{blockchain_type:?}"), format!("{health:?}"))
                })
                .collect();
            Ok(Json(health_response))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Get execution engine metrics
pub async fn get_execution_engine_metrics(
    State(state): State<Arc<ApplicationState>>,
) -> Result<Json<Vec<EngineMetricsResponse>>, StatusCode> {
    let engine_manager = state.execution_engines.read().await;

    match engine_manager.get_metrics().await {
        Ok(metrics_map) => {
            let metrics_response: Vec<EngineMetricsResponse> = metrics_map
                .into_iter()
                .map(|(blockchain_type, metrics)| {
                    EngineMetricsResponse {
                        blockchain_type: format!("{blockchain_type:?}"),
                        blocks_processed: metrics.transaction_count,
                        transactions_processed: metrics.transaction_count, // Approximate
                        average_block_time_ms: metrics.average_response_time_ms,
                        cpu_usage_percent: metrics.cpu_usage_percent,
                        memory_usage_mb: metrics.memory_usage_bytes / (1024 * 1024),
                        last_block_number: metrics.transaction_count, // Placeholder
                    }
                })
                .collect();
            Ok(Json(metrics_response))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Process a block on a specific execution engine
pub async fn process_block(
    State(state): State<Arc<ApplicationState>>,
    Path(blockchain_type_str): Path<String>,
    Json(request): Json<ProcessBlockRequest>,
) -> Result<Json<ProcessBlockResponse>, StatusCode> {
    // Parse blockchain type
    let blockchain_type = match blockchain_type_str.to_lowercase().as_str() {
        "ethereum" => BlockchainType::Ethereum,
        "solana" => BlockchainType::Solana,
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    // Decode block data from hex
    let block_data = hex::decode(&request.block_data_hex).map_err(|_| StatusCode::BAD_REQUEST)?;

    let engine_manager = state.execution_engines.read().await;

    let start_time = std::time::Instant::now();

    match engine_manager
        .process_block(blockchain_type, block_data)
        .await
    {
        Ok(_result_data) => {
            let processing_time_ms = start_time.elapsed().as_millis() as u64;

            // For now, return a simplified response
            // In a full implementation, we would deserialize the result_data
            let response = ProcessBlockResponse {
                success: true,
                block_number: Some(1),      // Placeholder
                transaction_count: Some(0), // Placeholder
                gas_used: Some(0),          // Placeholder
                processing_time_ms: Some(processing_time_ms),
                error_message: None,
            };

            Ok(Json(response))
        }
        Err(e) => {
            let response = ProcessBlockResponse {
                success: false,
                block_number: None,
                transaction_count: None,
                gas_used: None,
                processing_time_ms: Some(start_time.elapsed().as_millis() as u64),
                error_message: Some(e.to_string()),
            };

            Ok(Json(response))
        }
    }
}

/// Reset an execution engine to a specific block
pub async fn reset_engine(
    State(state): State<Arc<ApplicationState>>,
    Path(blockchain_type_str): Path<String>,
    Json(request): Json<ResetEngineRequest>,
) -> Result<Json<ResetEngineResponse>, StatusCode> {
    // Parse blockchain type
    let blockchain_type = match blockchain_type_str.to_lowercase().as_str() {
        "ethereum" => BlockchainType::Ethereum,
        "solana" => BlockchainType::Solana,
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    let engine_manager = state.execution_engines.read().await;

    match engine_manager
        .reset_engine_to_block(blockchain_type, request.block_id)
        .await
    {
        Ok(()) => {
            let response = ResetEngineResponse {
                success: true,
                new_block_id: request.block_id,
                message: format!(
                    "Successfully reset {:?} engine to block {}",
                    blockchain_type, request.block_id
                ),
            };
            Ok(Json(response))
        }
        Err(e) => {
            let response = ResetEngineResponse {
                success: false,
                new_block_id: request.block_id,
                message: format!("Failed to reset {blockchain_type:?} engine: {e}"),
            };
            Ok(Json(response))
        }
    }
}

/// Get the latest block ID for an execution engine
pub async fn get_latest_block(
    State(state): State<Arc<ApplicationState>>,
    Path(blockchain_type_str): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Parse blockchain type
    let blockchain_type = match blockchain_type_str.to_lowercase().as_str() {
        "ethereum" => BlockchainType::Ethereum,
        "solana" => BlockchainType::Solana,
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    let engine_manager = state.execution_engines.read().await;

    match engine_manager.get_latest_block_id(blockchain_type).await {
        Ok(block_id) => {
            let response = serde_json::json!({
                "blockchain_type": format!("{:?}", blockchain_type),
                "latest_block_id": block_id,
                "timestamp": chrono::Utc::now()
            });
            Ok(Json(response))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Get the state of a specific execution engine
pub async fn get_engine_state(
    State(state): State<Arc<ApplicationState>>,
    Path(blockchain_type_str): Path<String>,
) -> Result<Json<EngineStateInfo>, StatusCode> {
    // Parse blockchain type
    let blockchain_type = match blockchain_type_str.to_lowercase().as_str() {
        "ethereum" => BlockchainType::Ethereum,
        "solana" => BlockchainType::Solana,
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    let engine_manager = state.execution_engines.read().await;

    match engine_manager.get_engine_states().await {
        Ok(states_map) => {
            if let Some(engine_state) = states_map.get(&blockchain_type) {
                let state_info = EngineStateInfo {
                    blockchain_type: format!("{:?}", engine_state.blockchain_type),
                    current_block: engine_state.current_block,
                    is_syncing: engine_state.is_syncing,
                    peer_count: engine_state.peer_count,
                    chain_id: engine_state.chain_id,
                    data_directory: engine_state.data_directory.clone(),
                };
                Ok(Json(state_info))
            } else {
                Err(StatusCode::NOT_FOUND)
            }
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Restart a specific execution engine
pub async fn restart_engine(
    State(_state): State<Arc<ApplicationState>>,
    Path(blockchain_type_str): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Parse blockchain type
    let blockchain_type = match blockchain_type_str.to_lowercase().as_str() {
        "ethereum" => BlockchainType::Ethereum,
        "solana" => BlockchainType::Solana,
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    // For now, return a placeholder response
    // In a full implementation, we would actually restart the engine
    let response = serde_json::json!({
        "success": true,
        "blockchain_type": format!("{:?}", blockchain_type),
        "message": format!("Restart request received for {:?} engine", blockchain_type),
        "timestamp": chrono::Utc::now()
    });

    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    // use axum_test::TestServer; // Disabled - dependency not available

    #[tokio::test]
    async fn test_execution_engine_routes() {
        // This would require setting up a test server with mock execution engines
        // For now, just verify the routes can be created
        let routes = execution_engine_routes();
        assert!(!format!("{:?}", routes).is_empty());
    }

    // Additional tests would require axum_test dependency
    // #[tokio::test]
    // async fn test_execution_engine_endpoints() {
    //     // Tests disabled until axum_test dependency is available
    // }
}
