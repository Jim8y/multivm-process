//! Transaction submission endpoints
//!
//! Provides API endpoints for submitting transactions to the MultiVM system.

use crate::{
    api::{utils::generate_request_id, ApiResponse},
    ApplicationError, ApplicationResult, ApplicationState,
};
use axum::{
    extract::{Json, Path, State},
    response::Response as AxumResponse,
};
use multivm_common::VmType;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::sync::Arc;
use tracing::{error, info};

/// Transaction status for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiTransactionStatus {
    Pending,
    Included,
    Failed,
    Dropped,
}

/// Transaction submission request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSubmitRequest {
    /// VM type for the transaction
    pub vm_type: VmType,
    /// Raw transaction data (hex encoded)
    pub data: String,
    /// Optional sender address
    pub from: Option<String>,
    /// Optional recipient address
    pub to: Option<String>,
    /// Optional value/amount
    pub value: Option<String>,
    /// Optional gas price (EVM)
    pub gas_price: Option<String>,
    /// Optional gas limit (EVM)
    pub gas_limit: Option<u64>,
    /// Optional nonce
    pub nonce: Option<u64>,
    /// Priority level (high, normal, low)
    pub priority: Option<String>,
}

/// Transaction submission response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSubmitResponse {
    /// Transaction hash/ID
    pub tx_hash: String,
    /// VM type
    pub vm_type: VmType,
    /// Submission status
    pub status: String,
    /// Estimated inclusion time (seconds)
    pub estimated_inclusion_time: u64,
    /// Current pool position
    pub pool_position: Option<usize>,
}

/// Submit a new transaction
pub async fn submit_transaction(
    State(state): State<Arc<ApplicationState>>,
    Json(request): Json<TransactionSubmitRequest>,
) -> Result<Json<ApiResponse<TransactionSubmitResponse>>, AxumResponse> {
    let request_id = generate_request_id();
    let start_time = std::time::Instant::now();

    info!(
        "Transaction submission request: vm_type={:?}, priority={:?}",
        request.vm_type,
        request.priority.as_deref().unwrap_or("normal")
    );

    // Validate transaction data
    if request.data.is_empty() {
        return Ok(Json(ApiResponse::error(
            "INVALID_TRANSACTION".to_string(),
            "Transaction data cannot be empty".to_string(),
            request_id,
            start_time.elapsed().as_millis() as u64,
        )));
    }

    // Decode hex data
    let tx_data = match hex::decode(&request.data) {
        Ok(data) => data,
        Err(e) => {
            return Ok(Json(ApiResponse::error(
                "INVALID_HEX".to_string(),
                format!("Invalid hex data: {}", e),
                request_id,
                start_time.elapsed().as_millis() as u64,
            )));
        }
    };

    // Generate transaction hash
    let tx_hash = format!("{:x}", sha2::Sha256::digest(&tx_data));

    // Submit to consensus transaction pool
    let consensus_guard = state.consensus_manager.read().await;
    if let Some(consensus) = consensus_guard.as_ref() {
        // Determine priority
        let priority = match request.priority.as_deref() {
            Some("high") => multivm_consensus::transaction_pool::TransactionPriority::High,
            Some("low") => multivm_consensus::transaction_pool::TransactionPriority::Low,
            _ => multivm_consensus::transaction_pool::TransactionPriority::Normal,
        };

        // Create pooled transaction with JSON data
        let tx_json = serde_json::json!({
            "vm_type": request.vm_type,
            "data": hex::encode(&tx_data),
            "from": request.from.clone().unwrap_or_default(),
            "to": request.to.clone(),
            "value": request.value.clone(),
            "gas_price": request.gas_price.clone(),
            "gas_limit": request.gas_limit,
            "nonce": request.nonce
        });

        let pooled_tx = multivm_consensus::transaction_pool::PooledTransaction {
            id: tx_hash.clone(),
            data: tx_json,
            timestamp: std::time::SystemTime::now(),
            signature: Some(request.from.clone().unwrap_or_default()),
        };

        // Add to pool
        match consensus.add_transaction_to_pool(pooled_tx, priority).await {
            Ok(_) => {
                info!("Transaction {} added to pool", tx_hash);
            }
            Err(e) => {
                error!("Failed to add transaction to pool: {}", e);
                return Ok(Json(ApiResponse::error(
                    "POOL_ERROR".to_string(),
                    format!("Failed to add transaction to pool: {}", e),
                    request_id,
                    start_time.elapsed().as_millis() as u64,
                )));
            }
        }

        // Get pool stats for response
        let pool_stats = consensus.get_transaction_pool_stats().await;

        let response = TransactionSubmitResponse {
            tx_hash: tx_hash.clone(),
            vm_type: request.vm_type,
            status: "pending".to_string(),
            estimated_inclusion_time: 5, // Estimate based on block time
            pool_position: Some(pool_stats.current_pool_size),
        };

        Ok(Json(ApiResponse::success(
            response,
            request_id,
            start_time.elapsed().as_millis() as u64,
        )))
    } else {
        Ok(Json(ApiResponse::error(
            "CONSENSUS_UNAVAILABLE".to_string(),
            "Consensus manager not available".to_string(),
            request_id,
            start_time.elapsed().as_millis() as u64,
        )))
    }
}

/// Get transaction status
pub async fn get_transaction_status(
    State(state): State<Arc<ApplicationState>>,
    Path(tx_hash): Path<String>,
) -> Result<Json<ApiResponse<TransactionStatusResponse>>, AxumResponse> {
    let request_id = generate_request_id();
    let start_time = std::time::Instant::now();

    // Check if transaction is in pool
    let consensus_guard = state.consensus_manager.read().await;
    if let Some(consensus) = consensus_guard.as_ref() {
        let pool_stats = consensus.get_transaction_pool_stats().await;

        // In a real implementation, we would check if the specific transaction is in the pool
        // For now, return a mock response
        let response = TransactionStatusResponse {
            tx_hash: tx_hash.clone(),
            status: ApiTransactionStatus::Pending,
            pool_position: Some(pool_stats.current_pool_size / 2), // Mock position
            confirmations: 0,
            block_hash: None,
            block_number: None,
            timestamp: None,
        };

        Ok(Json(ApiResponse::success(
            response,
            request_id,
            start_time.elapsed().as_millis() as u64,
        )))
    } else {
        Ok(Json(ApiResponse::error(
            "CONSENSUS_UNAVAILABLE".to_string(),
            "Consensus manager not available".to_string(),
            request_id,
            start_time.elapsed().as_millis() as u64,
        )))
    }
}

/// Transaction status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionStatusResponse {
    /// Transaction hash
    pub tx_hash: String,
    /// Current status
    pub status: ApiTransactionStatus,
    /// Position in pool (if pending)
    pub pool_position: Option<usize>,
    /// Number of confirmations (if confirmed)
    pub confirmations: u64,
    /// Block hash (if confirmed)
    pub block_hash: Option<String>,
    /// Block number (if confirmed)
    pub block_number: Option<u64>,
    /// Timestamp (if confirmed)
    pub timestamp: Option<i64>,
}

/// Batch transaction submission request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchTransactionSubmitRequest {
    /// List of transactions to submit
    pub transactions: Vec<TransactionSubmitRequest>,
}

/// Batch transaction submission response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchTransactionSubmitResponse {
    /// Results for each transaction
    pub results: Vec<BatchSubmitResult>,
    /// Total submitted successfully
    pub submitted_count: usize,
    /// Total failed
    pub failed_count: usize,
}

/// Individual result in batch submission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchSubmitResult {
    /// Transaction index in request
    pub index: usize,
    /// Success status
    pub success: bool,
    /// Transaction hash (if successful)
    pub tx_hash: Option<String>,
    /// Error message (if failed)
    pub error: Option<String>,
}

/// Submit multiple transactions in batch
pub async fn submit_batch_transactions(
    State(state): State<Arc<ApplicationState>>,
    Json(request): Json<BatchTransactionSubmitRequest>,
) -> Result<Json<ApiResponse<BatchTransactionSubmitResponse>>, AxumResponse> {
    let request_id = generate_request_id();
    let start_time = std::time::Instant::now();

    if request.transactions.is_empty() {
        return Ok(Json(ApiResponse::error(
            "EMPTY_BATCH".to_string(),
            "Batch cannot be empty".to_string(),
            request_id,
            start_time.elapsed().as_millis() as u64,
        )));
    }

    if request.transactions.len() > 100 {
        return Ok(Json(ApiResponse::error(
            "BATCH_TOO_LARGE".to_string(),
            "Batch size cannot exceed 100 transactions".to_string(),
            request_id,
            start_time.elapsed().as_millis() as u64,
        )));
    }

    let mut results = Vec::new();
    let mut submitted_count = 0;
    let mut failed_count = 0;

    // Process each transaction
    for (index, tx_request) in request.transactions.into_iter().enumerate() {
        // Submit transaction (reuse single submission logic)
        match submit_single_transaction(&state, tx_request).await {
            Ok(tx_hash) => {
                results.push(BatchSubmitResult {
                    index,
                    success: true,
                    tx_hash: Some(tx_hash),
                    error: None,
                });
                submitted_count += 1;
            }
            Err(e) => {
                results.push(BatchSubmitResult {
                    index,
                    success: false,
                    tx_hash: None,
                    error: Some(e.to_string()),
                });
                failed_count += 1;
            }
        }
    }

    let response = BatchTransactionSubmitResponse {
        results,
        submitted_count,
        failed_count,
    };

    Ok(Json(ApiResponse::success(
        response,
        request_id,
        start_time.elapsed().as_millis() as u64,
    )))
}

/// Helper function to submit a single transaction
async fn submit_single_transaction(
    state: &Arc<ApplicationState>,
    request: TransactionSubmitRequest,
) -> ApplicationResult<String> {
    // Validate and decode transaction data
    if request.data.is_empty() {
        return Err(ApplicationError::ValidationError {
            field: "data".to_string(),
            message: "Transaction data cannot be empty".to_string(),
        });
    }

    let tx_data = hex::decode(&request.data).map_err(|e| ApplicationError::ValidationError {
        field: "data".to_string(),
        message: format!("Invalid hex data: {}", e),
    })?;

    // Generate transaction hash
    let tx_hash = format!("{:x}", sha2::Sha256::digest(&tx_data));

    // Submit to consensus transaction pool
    let consensus_guard = state.consensus_manager.read().await;
    if let Some(consensus) = consensus_guard.as_ref() {
        // Determine priority
        let priority = match request.priority.as_deref() {
            Some("high") => multivm_consensus::transaction_pool::TransactionPriority::High,
            Some("low") => multivm_consensus::transaction_pool::TransactionPriority::Low,
            _ => multivm_consensus::transaction_pool::TransactionPriority::Normal,
        };

        // Create pooled transaction with JSON data
        let tx_json = serde_json::json!({
            "vm_type": request.vm_type,
            "data": hex::encode(&tx_data),
            "from": request.from.clone().unwrap_or_default(),
            "to": request.to.clone(),
            "value": request.value.clone(),
            "gas_price": request.gas_price.clone(),
            "gas_limit": request.gas_limit,
            "nonce": request.nonce
        });

        let pooled_tx = multivm_consensus::transaction_pool::PooledTransaction {
            id: tx_hash.clone(),
            data: tx_json,
            timestamp: std::time::SystemTime::now(),
            signature: Some(request.from.unwrap_or_default()),
        };

        // Add to pool
        consensus
            .add_transaction_to_pool(pooled_tx, priority)
            .await
            .map_err(|e| ApplicationError::InternalError {
                component: "transaction_pool".to_string(),
                message: e.to_string(),
            })?;

        Ok(tx_hash)
    } else {
        Err(ApplicationError::ServiceUnavailable {
            service: "consensus".to_string(),
            reason: "Consensus manager not available".to_string(),
        })
    }
}

/// Cancel a pending transaction
pub async fn cancel_transaction(
    State(state): State<Arc<ApplicationState>>,
    Path(tx_hash): Path<String>,
) -> Result<Json<ApiResponse<CancelTransactionResponse>>, AxumResponse> {
    let request_id = generate_request_id();
    let start_time = std::time::Instant::now();

    let consensus_guard = state.consensus_manager.read().await;
    if let Some(consensus) = consensus_guard.as_ref() {
        // In a real implementation, we would remove the transaction from the pool
        // For now, return a mock response
        let response = CancelTransactionResponse {
            tx_hash: tx_hash.clone(),
            cancelled: true,
            message: "Transaction cancelled successfully".to_string(),
        };

        Ok(Json(ApiResponse::success(
            response,
            request_id,
            start_time.elapsed().as_millis() as u64,
        )))
    } else {
        Ok(Json(ApiResponse::error(
            "CONSENSUS_UNAVAILABLE".to_string(),
            "Consensus manager not available".to_string(),
            request_id,
            start_time.elapsed().as_millis() as u64,
        )))
    }
}

/// Cancel transaction response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelTransactionResponse {
    /// Transaction hash
    pub tx_hash: String,
    /// Whether cancellation was successful
    pub cancelled: bool,
    /// Status message
    pub message: String,
}
