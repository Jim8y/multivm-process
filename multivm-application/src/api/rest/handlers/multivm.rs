//! # MultiVM Handlers
//!
//! Handlers for MultiVM-specific operations including cross-VM account binding and transactions.

use super::{calculate_response_time, start_request_timer, success_response};
use crate::ApplicationState;
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Bind accounts across VMs
pub async fn bind_accounts(
    State(_state): State<Arc<ApplicationState>>,
    headers: HeaderMap,
    Json(_request): Json<AccountBindingRequest>,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();
    let response_time = calculate_response_time(start_time);

    let response = AccountBindingResponse {
        multivm_account: "M123".to_string(),
        svm_account: Some("SVM123".to_string()),
        evm_account: Some("0x123".to_string()),
        binding_id: "binding_123".to_string(),
    };

    success_response(response, request_id, response_time).into_response()
}

/// Get account bindings
pub async fn get_account_bindings(
    State(_state): State<Arc<ApplicationState>>,
    Path(_address): Path<String>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();
    let response_time = calculate_response_time(start_time);

    let bindings = AccountBindings {
        multivm_account: "M123".to_string(),
        bindings: vec![],
    };

    success_response(bindings, request_id, response_time).into_response()
}

/// Unbind accounts
pub async fn unbind_accounts(
    State(_state): State<Arc<ApplicationState>>,
    headers: HeaderMap,
    Json(_request): Json<AccountUnbindingRequest>,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();
    let response_time = calculate_response_time(start_time);

    success_response(
        "Account unbound successfully".to_string(),
        request_id,
        response_time,
    )
    .into_response()
}

/// Send cross-VM transaction
pub async fn send_cross_vm_transaction(
    State(state): State<Arc<ApplicationState>>,
    headers: HeaderMap,
    Json(request): Json<CrossVmTransactionRequest>,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    // Convert request to JSON for processing
    let transaction_data = serde_json::to_value(&request).unwrap_or_default();

    // Process through execution engines
    match state
        .execution_engines
        .read()
        .await
        .process_cross_vm_transaction(transaction_data)
        .await
    {
        Ok(tx_id) => {
            tracing::info!("Cross-VM transaction processed successfully: {}", tx_id);
            let response_time = calculate_response_time(start_time);
            let response = CrossVmTransactionResponse {
                transaction_id: tx_id,
                status: "pending".to_string(),
            };
            success_response(response, request_id, response_time).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to process Cross-VM transaction: {}", e);
            let response_time = calculate_response_time(start_time);
            crate::api::rest::handlers::error_response(
                "MULTIVM_ERROR",
                &e.to_string(),
                request_id,
                response_time,
            )
            .into_response()
        }
    }
}

/// Get cross-VM transaction status
pub async fn get_cross_vm_transaction_status(
    State(_state): State<Arc<ApplicationState>>,
    Path(_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();
    let response_time = calculate_response_time(start_time);

    let status = CrossVmTransactionStatus {
        transaction_id: "crossvm_123".to_string(),
        status: "confirmed".to_string(),
        svm_tx_signature: Some("svm_sig_123".to_string()),
        evm_tx_hash: Some("0xevm_hash_123".to_string()),
    };

    success_response(status, request_id, response_time).into_response()
}

/// Get latest MultiVM block
pub async fn get_latest_multivm_block(
    State(_state): State<Arc<ApplicationState>>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();
    let response_time = calculate_response_time(start_time);

    let block = MultivmBlock {
        block_number: 123,
        block_hash: "multivm_block_123".to_string(),
        svm_components: vec![],
        evm_components: vec![],
        multivm_transactions: vec![],
    };

    success_response(block, request_id, response_time).into_response()
}

/// Get MultiVM block by ID
pub async fn get_multivm_block(
    State(_state): State<Arc<ApplicationState>>,
    Path(_block_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();
    let response_time = calculate_response_time(start_time);

    let block = MultivmBlock {
        block_number: 123,
        block_hash: "multivm_block_123".to_string(),
        svm_components: vec![],
        evm_components: vec![],
        multivm_transactions: vec![],
    };

    success_response(block, request_id, response_time).into_response()
}

/// Get system state summary
pub async fn get_system_state(
    State(_state): State<Arc<ApplicationState>>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();
    let response_time = calculate_response_time(start_time);

    let state = SystemState {
        latest_block: 123,
        total_accounts: 1000,
        total_transactions: 50000,
        vm_states: VmStates {
            solana_latest_slot: 12345,
            reth_latest_block: 6789,
        },
    };

    success_response(state, request_id, response_time).into_response()
}

// Request and response types

#[derive(Debug, Deserialize)]
pub struct AccountBindingRequest {
    pub svm_account: Option<String>,
    pub evm_account: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AccountBindingResponse {
    pub multivm_account: String,
    pub svm_account: Option<String>,
    pub evm_account: Option<String>,
    pub binding_id: String,
}

#[derive(Debug, Serialize)]
pub struct AccountBindings {
    pub multivm_account: String,
    pub bindings: Vec<AccountBinding>,
}

#[derive(Debug, Serialize)]
pub struct AccountBinding {
    pub vm_type: String,
    pub vm_account: String,
    pub binding_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct AccountUnbindingRequest {
    pub binding_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CrossVmTransactionRequest {
    pub from_vm: String,
    pub to_vm: String,
    pub transaction_data: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct CrossVmTransactionResponse {
    pub transaction_id: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct CrossVmTransactionStatus {
    pub transaction_id: String,
    pub status: String,
    pub svm_tx_signature: Option<String>,
    pub evm_tx_hash: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MultivmBlock {
    pub block_number: u64,
    pub block_hash: String,
    pub svm_components: Vec<serde_json::Value>,
    pub evm_components: Vec<serde_json::Value>,
    pub multivm_transactions: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct SystemState {
    pub latest_block: u64,
    pub total_accounts: u64,
    pub total_transactions: u64,
    pub vm_states: VmStates,
}

#[derive(Debug, Serialize)]
pub struct VmStates {
    pub solana_latest_slot: u64,
    pub reth_latest_block: u64,
}
