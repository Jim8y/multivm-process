//! # SVM (Solana) Handlers
//!
//! Handlers for Solana Virtual Machine operations.

use super::{calculate_response_time, start_request_timer, success_response};
use crate::{
    gateway::svm::{SvmAccountInfo, SvmTransactionInfo as GatewaySvmTransaction},
    ApplicationState,
};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Create an error response as a generic Response
fn svm_error_response(
    code: &str,
    message: &str,
    request_id: String,
    response_time: u64,
) -> Response {
    let status_code = match code {
        "NOT_FOUND" => StatusCode::NOT_FOUND,
        "SVM_ERROR" => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };

    let api_response = crate::api::ApiResponse::<()> {
        data: None,
        error: Some(crate::api::ApiError {
            code: code.to_string(),
            message: message.to_string(),
            details: None,
        }),
        metadata: crate::api::ApiMetadata {
            request_id,
            timestamp: chrono::Utc::now(),
            response_time_ms: response_time,
            api_version: "v1".to_string(),
        },
    };

    (status_code, Json(api_response)).into_response()
}

/// Get account information
#[axum::debug_handler]
pub async fn get_account(
    State(state): State<Arc<ApplicationState>>,
    Path(address): Path<String>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    // Call SVM gateway
    let gateway_response = match state.svm_gateway.get_account_info(&address).await {
        Ok(resp) => resp,
        Err(e) => {
            let response_time = calculate_response_time(start_time);
            return svm_error_response("SVM_ERROR", &e.to_string(), request_id, response_time)
                .into_response();
        }
    };

    let account_info = match gateway_response.data {
        Some(info) => info,
        None => {
            let response_time = calculate_response_time(start_time);
            return svm_error_response("NOT_FOUND", "Account not found", request_id, response_time);
        }
    };

    let response_time = calculate_response_time(start_time);
    success_response(account_info, request_id, response_time).into_response()
}

/// Get account balance
pub async fn get_balance(
    State(state): State<Arc<ApplicationState>>,
    Path(address): Path<String>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    let gateway_response = match state.svm_gateway.get_balance(&address).await {
        Ok(resp) => resp,
        Err(e) => {
            let response_time = calculate_response_time(start_time);
            return svm_error_response("SVM_ERROR", &e.to_string(), request_id, response_time)
                .into_response();
        }
    };

    let balance = SvmBalance {
        lamports: gateway_response.data,
    };

    let response_time = calculate_response_time(start_time);
    success_response(balance, request_id, response_time).into_response()
}

/// Get account transaction history
pub async fn get_account_transactions(
    State(state): State<Arc<ApplicationState>>,
    Path(address): Path<String>,
    Query(params): Query<PaginationQuery>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    let gateway_response = match state.svm_gateway.get_account_transactions(&address).await {
        Ok(resp) => resp,
        Err(e) => {
            let response_time = calculate_response_time(start_time);
            return svm_error_response("SVM_ERROR", &e.to_string(), request_id, response_time)
                .into_response();
        }
    };

    let transactions = gateway_response.data;
    let transaction_list = SvmTransactionList {
        transactions: transactions.clone(),
        total: transactions.len(),
        limit: params.limit,
        offset: params.offset,
    };

    let response_time = calculate_response_time(start_time);
    success_response(transaction_list, request_id, response_time).into_response()
}

/// Send transaction
pub async fn send_transaction(
    State(state): State<Arc<ApplicationState>>,
    headers: HeaderMap,
    Json(request): Json<SendTransactionRequest>,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    let result = match state
        .svm_gateway
        .send_transaction(&request.transaction_data)
        .await
    {
        Ok(signature) => signature,
        Err(e) => {
            let response_time = calculate_response_time(start_time);
            return svm_error_response("SVM_ERROR", &e.to_string(), request_id, response_time)
                .into_response();
        }
    };

    let transaction_response = TransactionResponse {
        signature: result.data,
    };

    let response_time = calculate_response_time(start_time);
    success_response(transaction_response, request_id, response_time).into_response()
}

/// Get transaction by signature
pub async fn get_transaction(
    State(state): State<Arc<ApplicationState>>,
    Path(signature): Path<String>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    let transaction = match state.svm_gateway.get_transaction(&signature).await {
        Ok(tx) => tx,
        Err(e) => {
            let response_time = calculate_response_time(start_time);
            return svm_error_response("SVM_ERROR", &e.to_string(), request_id, response_time)
                .into_response();
        }
    };

    let response_time = calculate_response_time(start_time);
    let tx_data = transaction.data.unwrap_or_else(|| GatewaySvmTransaction {
        signature: "not_found".to_string(),
        slot: 0,
        block_time: None,
        confirmations: None,
        err: Some(serde_json::json!({"error": "Transaction not found"})),
    });
    success_response(tx_data, request_id, response_time).into_response()
}

/// Simulate transaction
pub async fn simulate_transaction(
    State(state): State<Arc<ApplicationState>>,
    headers: HeaderMap,
    Json(request): Json<SimulateTransactionRequest>,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    let result = match state
        .svm_gateway
        .simulate_transaction(&request.transaction_data)
        .await
    {
        Ok(result) => result,
        Err(e) => {
            let response_time = calculate_response_time(start_time);
            return svm_error_response("SVM_ERROR", &e.to_string(), request_id, response_time)
                .into_response();
        }
    };

    let simulation_result = SimulationResult {
        accounts: vec![],
        logs: result
            .data
            .get("logs")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        return_data: result
            .data
            .get("return_data")
            .and_then(|v| v.as_str())
            .map(String::from),
        units_consumed: result
            .data
            .get("units_consumed")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        err: result
            .data
            .get("err")
            .and_then(|v| v.as_str())
            .map(String::from),
    };

    let response_time = calculate_response_time(start_time);
    success_response(simulation_result, request_id, response_time).into_response()
}

/// Get latest block
pub async fn get_latest_block(
    State(state): State<Arc<ApplicationState>>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    let block = match state.svm_gateway.get_latest_block().await {
        Ok(block) => block,
        Err(e) => {
            let response_time = calculate_response_time(start_time);
            return svm_error_response("SVM_ERROR", &e.to_string(), request_id, response_time)
                .into_response();
        }
    };

    let response_time = calculate_response_time(start_time);
    success_response(block.data, request_id, response_time).into_response()
}

/// Get block by slot
pub async fn get_block(
    State(state): State<Arc<ApplicationState>>,
    Path(slot): Path<u64>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    let block = match state.svm_gateway.get_block(slot).await {
        Ok(block) => block,
        Err(e) => {
            let response_time = calculate_response_time(start_time);
            return svm_error_response("SVM_ERROR", &e.to_string(), request_id, response_time)
                .into_response();
        }
    };

    let block_data = match block.data {
        Some(b) => b,
        None => {
            let response_time = calculate_response_time(start_time);
            return svm_error_response("NOT_FOUND", "Block not found", request_id, response_time);
        }
    };

    let response_time = calculate_response_time(start_time);
    success_response(block_data, request_id, response_time).into_response()
}

/// Get block transactions
pub async fn get_block_transactions(
    State(state): State<Arc<ApplicationState>>,
    Path(slot): Path<u64>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    let transactions = match state.svm_gateway.get_block_transactions(slot).await {
        Ok(txs) => txs,
        Err(e) => {
            let response_time = calculate_response_time(start_time);
            return svm_error_response("SVM_ERROR", &e.to_string(), request_id, response_time)
                .into_response();
        }
    };

    let tx_data = transactions.data;
    let transaction_list = SvmTransactionList {
        transactions: tx_data.clone(),
        total: tx_data.len(),
        limit: 50, // Default limit
        offset: 0, // Default offset
    };

    let response_time = calculate_response_time(start_time);
    success_response(transaction_list, request_id, response_time).into_response()
}

/// Get program accounts
pub async fn get_program_accounts(
    State(state): State<Arc<ApplicationState>>,
    Path(program_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    let accounts = match state.svm_gateway.get_program_accounts(&program_id).await {
        Ok(accounts) => accounts,
        Err(e) => {
            let response_time = calculate_response_time(start_time);
            return svm_error_response("SVM_ERROR", &e.to_string(), request_id, response_time)
                .into_response();
        }
    };

    let response_time = calculate_response_time(start_time);
    success_response(accounts.data, request_id, response_time).into_response()
}

/// Get token accounts
pub async fn get_token_accounts(
    State(state): State<Arc<ApplicationState>>,
    Path(mint): Path<String>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    let accounts = match state.svm_gateway.get_token_accounts(&mint).await {
        Ok(accounts) => accounts,
        Err(e) => {
            let response_time = calculate_response_time(start_time);
            return svm_error_response("SVM_ERROR", &e.to_string(), request_id, response_time)
                .into_response();
        }
    };

    // Convert JSON values to SvmTokenAccount (mock implementation)
    let token_accounts: Vec<SvmTokenAccount> = accounts
        .data
        .into_iter()
        .map(|_| SvmTokenAccount {
            pubkey: "mock_pubkey".to_string(),
            account: SvmAccountInfo {
                lamports: 0,
                owner: "mock_owner".to_string(),
                executable: false,
                rent_epoch: 0,
                data: None,
            },
            token_amount: TokenAmount {
                amount: "0".to_string(),
                decimals: 9,
                ui_amount: Some(0.0),
                ui_amount_string: "0".to_string(),
            },
        })
        .collect();

    let response_time = calculate_response_time(start_time);
    success_response(token_accounts, request_id, response_time).into_response()
}

/// Get token supply
pub async fn get_token_supply(
    State(state): State<Arc<ApplicationState>>,
    Path(mint): Path<String>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    let supply = match state.svm_gateway.get_token_supply(&mint).await {
        Ok(supply) => supply,
        Err(e) => {
            let response_time = calculate_response_time(start_time);
            return svm_error_response("SVM_ERROR", &e.to_string(), request_id, response_time)
                .into_response();
        }
    };

    // Convert JSON to TokenSupply
    let token_supply = TokenSupply {
        amount: supply
            .data
            .get("total_supply")
            .and_then(|v| v.as_str())
            .unwrap_or("0")
            .to_string(),
        decimals: supply
            .data
            .get("decimals")
            .and_then(|v| v.as_u64())
            .unwrap_or(9) as u8,
        ui_amount: supply.data.get("ui_amount").and_then(|v| v.as_f64()),
        ui_amount_string: supply
            .data
            .get("ui_amount_string")
            .and_then(|v| v.as_str())
            .unwrap_or("0")
            .to_string(),
    };

    let response_time = calculate_response_time(start_time);
    success_response(token_supply, request_id, response_time).into_response()
}

// Request and response types

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize {
    50
}

#[derive(Debug, Deserialize)]
pub struct SendTransactionRequest {
    pub transaction_data: String,
}

#[derive(Debug, Deserialize)]
pub struct SimulateTransactionRequest {
    pub transaction_data: String,
}

#[derive(Debug, Serialize)]
pub struct SvmBalance {
    pub lamports: u64,
}

#[derive(Debug, Serialize)]
pub struct SvmTransaction {
    pub signature: String,
    pub slot: u64,
    pub block_time: Option<i64>,
    pub meta: TransactionMeta,
    pub transaction: TransactionData,
}

#[derive(Debug, Serialize)]
pub struct TransactionMeta {
    pub fee: u64,
    pub pre_balances: Vec<u64>,
    pub post_balances: Vec<u64>,
    pub status: TransactionStatus,
}

#[derive(Debug, Serialize)]
pub struct TransactionStatus {
    pub success: bool,
    pub err: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TransactionData {
    pub message: MessageData,
    pub signatures: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct MessageData {
    pub account_keys: Vec<String>,
    pub instructions: Vec<InstructionData>,
    pub recent_blockhash: String,
}

#[derive(Debug, Serialize)]
pub struct InstructionData {
    pub program_id_index: u8,
    pub accounts: Vec<u8>,
    pub data: String,
}

#[derive(Debug, Serialize)]
pub struct SvmTransactionList {
    pub transactions: Vec<GatewaySvmTransaction>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Serialize)]
pub struct TransactionResponse {
    pub signature: String,
}

#[derive(Debug, Serialize)]
pub struct SimulationResult {
    pub accounts: Vec<SvmAccountInfo>,
    pub logs: Vec<String>,
    pub return_data: Option<String>,
    pub units_consumed: u64,
    pub err: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SvmBlock {
    pub slot: u64,
    pub parent_slot: u64,
    pub blockhash: String,
    pub previous_blockhash: String,
    pub block_time: Option<i64>,
    pub transactions: Vec<GatewaySvmTransaction>,
    pub rewards: Vec<Reward>,
}

#[derive(Debug, Serialize)]
pub struct Reward {
    pub pubkey: String,
    pub lamports: i64,
    pub post_balance: u64,
    pub reward_type: Option<String>,
    pub commission: Option<u8>,
}

#[derive(Debug, Serialize)]
pub struct SvmTokenAccount {
    pub pubkey: String,
    pub account: SvmAccountInfo,
    pub token_amount: TokenAmount,
}

#[derive(Debug, Serialize)]
pub struct TokenAmount {
    pub amount: String,
    pub decimals: u8,
    pub ui_amount: Option<f64>,
    pub ui_amount_string: String,
}

#[derive(Debug, Serialize)]
pub struct TokenSupply {
    pub amount: String,
    pub decimals: u8,
    pub ui_amount: Option<f64>,
    pub ui_amount_string: String,
}
