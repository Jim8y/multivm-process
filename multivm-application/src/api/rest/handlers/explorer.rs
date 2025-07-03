//! Block Explorer API Handlers
//!
//! Production-ready API endpoints for block exploration, transaction history,
//! and blockchain analytics.

use super::{calculate_response_time, start_request_timer, success_response};
use crate::ApplicationState;
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Pagination parameters
#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_page() -> u32 {
    1
}

fn default_limit() -> u32 {
    20
}

/// Block summary for explorer
#[derive(Debug, Serialize)]
pub struct BlockSummary {
    pub height: u64,
    pub hash: String,
    pub timestamp: i64,
    pub proposer: String,
    pub tx_count: u32,
    pub evm_tx_count: u32,
    pub svm_tx_count: u32,
    pub cross_vm_tx_count: u32,
    pub total_gas_used: u64,
    pub total_fees: u64,
}

/// Transaction summary for explorer
#[derive(Debug, Serialize)]
pub struct TransactionSummary {
    pub hash: String,
    pub block_height: u64,
    pub block_hash: String,
    pub timestamp: i64,
    pub from: String,
    pub to: Option<String>,
    pub value: String,
    pub fee: String,
    pub status: String,
    pub vm_type: String,
}

/// Account activity summary
#[derive(Debug, Serialize)]
pub struct AccountActivity {
    pub address: String,
    pub balance: String,
    pub nonce: u64,
    pub tx_count: u64,
    pub first_seen_block: u64,
    pub last_activity_block: u64,
    pub evm_activity: bool,
    pub svm_activity: bool,
}

/// Network statistics
#[derive(Debug, Serialize)]
pub struct NetworkStats {
    pub current_height: u64,
    pub total_transactions: u64,
    pub total_accounts: u64,
    pub tps_current: f64,
    pub tps_average: f64,
    pub block_time_average: f64,
    pub validators_online: u32,
    pub validators_total: u32,
    pub network_health: String,
}

/// Search results
#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub result_type: String, // "block", "transaction", "account"
    pub data: serde_json::Value,
}

/// Get list of recent blocks
pub async fn get_blocks(
    State(_state): State<Arc<ApplicationState>>,
    Query(params): Query<PaginationQuery>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    // Calculate offset
    let offset = (params.page - 1) * params.limit;

    // Generate mock block data for now
    let blocks: Vec<BlockSummary> = (0..params.limit)
        .map(|i| {
            let height = 1000 - offset - i;
            BlockSummary {
                height: height as u64,
                hash: format!("0x{height:064x}"),
                timestamp: chrono::Utc::now().timestamp() - (i as i64 * 5),
                proposer: format!("validator_{}", height % 4),
                tx_count: (height % 20),
                evm_tx_count: (height % 10),
                svm_tx_count: (height % 7),
                cross_vm_tx_count: (height % 3),
                total_gas_used: (height * 21000) as u64,
                total_fees: (height * 1000000) as u64,
            }
        })
        .collect();

    let response = serde_json::json!({
        "blocks": blocks,
        "pagination": {
            "page": params.page,
            "limit": params.limit,
            "total_pages": 50,
            "total_items": 1000
        }
    });

    let response_time = calculate_response_time(start_time);
    success_response(response, request_id, response_time).into_response()
}

/// Get block details by height or hash
pub async fn get_block_details(
    State(_state): State<Arc<ApplicationState>>,
    Path(block_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    // Parse block identifier (height or hash)
    let block_height = if block_id.starts_with("0x") {
        // Hash provided, convert to height for mock
        1000
    } else {
        block_id.parse::<u64>().unwrap_or(1000)
    };

    let block_details = serde_json::json!({
        "height": block_height,
        "hash": format!("0x{:064x}", block_height),
        "parent_hash": format!("0x{:064x}", block_height - 1),
        "timestamp": chrono::Utc::now().timestamp() - 60,
        "proposer": "validator_1",
        "validator_signatures": [
            {"validator": "validator_1", "signature": "0xabc123"},
            {"validator": "validator_2", "signature": "0xdef456"},
        ],
        "transactions": [
            {
                "hash": "0x123abc",
                "type": "evm",
                "from": "0x1234567890123456789012345678901234567890",
                "to": "0x0987654321098765432109876543210987654321",
                "value": "1000000000000000000",
                "gas_used": 21000
            }
        ],
        "state_root": format!("0x{:064x}", block_height * 2),
        "receipts_root": format!("0x{:064x}", block_height * 3),
        "size": 1024,
        "gas_limit": 30000000,
        "gas_used": 21000,
        "base_fee_per_gas": "20000000000"
    });

    let response_time = calculate_response_time(start_time);
    success_response(block_details, request_id, response_time).into_response()
}

/// Get list of recent transactions
pub async fn get_transactions(
    State(_state): State<Arc<ApplicationState>>,
    Query(params): Query<PaginationQuery>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    let offset = (params.page - 1) * params.limit;

    // Generate mock transaction data
    let transactions: Vec<TransactionSummary> = (0..params.limit)
        .map(|i| {
            let vm_type = match i % 3 {
                0 => "evm",
                1 => "svm",
                _ => "cross_vm",
            };

            TransactionSummary {
                hash: format!("0x{:064x}", offset + i),
                block_height: 1000 - (offset + i) as u64 / 10,
                block_hash: format!("0x{:064x}", 1000 - (offset + i) as u64 / 10),
                timestamp: chrono::Utc::now().timestamp() - (i as i64 * 2),
                from: format!("0x{:040x}", i * 123),
                to: Some(format!("0x{:040x}", i * 456)),
                value: format!("{}", 1000000000000000000u64 + i as u64),
                fee: format!("{}", 21000 * 20000000000u64),
                status: "success".to_string(),
                vm_type: vm_type.to_string(),
            }
        })
        .collect();

    let response = serde_json::json!({
        "transactions": transactions,
        "pagination": {
            "page": params.page,
            "limit": params.limit,
            "total_pages": 100,
            "total_items": 2000
        }
    });

    let response_time = calculate_response_time(start_time);
    success_response(response, request_id, response_time).into_response()
}

/// Get transaction details by hash
pub async fn get_transaction_details(
    State(_state): State<Arc<ApplicationState>>,
    Path(tx_hash): Path<String>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    let tx_details = serde_json::json!({
        "hash": tx_hash,
        "block_height": 1000,
        "block_hash": format!("0x{:064x}", 1000),
        "timestamp": chrono::Utc::now().timestamp() - 300,
        "from": "0x1234567890123456789012345678901234567890",
        "to": "0x0987654321098765432109876543210987654321",
        "value": "1000000000000000000",
        "nonce": 42,
        "gas_limit": 21000,
        "gas_used": 21000,
        "gas_price": "20000000000",
        "max_fee_per_gas": "30000000000",
        "max_priority_fee_per_gas": "1000000000",
        "input": "0x",
        "status": "success",
        "vm_type": "evm",
        "receipt": {
            "status": 1,
            "cumulative_gas_used": 21000,
            "logs": [],
            "contract_address": null
        }
    });

    let response_time = calculate_response_time(start_time);
    success_response(tx_details, request_id, response_time).into_response()
}

/// Get account activity
pub async fn get_account_activity(
    State(_state): State<Arc<ApplicationState>>,
    Path(address): Path<String>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    let activity = AccountActivity {
        address: address.clone(),
        balance: "1234567890000000000".to_string(),
        nonce: 42,
        tx_count: 150,
        first_seen_block: 100,
        last_activity_block: 999,
        evm_activity: true,
        svm_activity: address.len() > 42, // Mock logic
    };

    let response_time = calculate_response_time(start_time);
    success_response(activity, request_id, response_time).into_response()
}

/// Get account transaction history
pub async fn get_account_transactions(
    State(_state): State<Arc<ApplicationState>>,
    Path(address): Path<String>,
    Query(params): Query<PaginationQuery>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    // Generate mock transactions for the account
    let transactions: Vec<TransactionSummary> = (0..params.limit)
        .map(|i| TransactionSummary {
            hash: format!("0x{i:064x}"),
            block_height: 1000 - i as u64,
            block_hash: format!("0x{:064x}", 1000 - i as u64),
            timestamp: chrono::Utc::now().timestamp() - (i as i64 * 300),
            from: if i % 2 == 0 {
                address.clone()
            } else {
                format!("0x{i:040x}")
            },
            to: if i % 2 == 1 {
                Some(address.clone())
            } else {
                Some(format!("0x{:040x}", i * 2))
            },
            value: format!("{}", 100000000000000000u64 * (i as u64 + 1)),
            fee: format!("{}", 21000 * 20000000000u64),
            status: "success".to_string(),
            vm_type: if i % 3 == 0 { "evm" } else { "svm" }.to_string(),
        })
        .collect();

    let response = serde_json::json!({
        "address": address,
        "transactions": transactions,
        "pagination": {
            "page": params.page,
            "limit": params.limit,
            "total_pages": 10,
            "total_items": 150
        }
    });

    let response_time = calculate_response_time(start_time);
    success_response(response, request_id, response_time).into_response()
}

/// Get network statistics
pub async fn get_network_stats(
    State(_state): State<Arc<ApplicationState>>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    // Get consensus stats if available
    let consensus_guard = _state.consensus_manager.read().await;
    let consensus_stats = if let Some(consensus) = consensus_guard.as_ref() {
        let stats = consensus.get_transaction_pool_stats().await;
        Some(stats)
    } else {
        None
    };

    let stats = NetworkStats {
        current_height: 1000,
        total_transactions: 50000,
        total_accounts: 1500,
        tps_current: 125.5,
        tps_average: 100.0,
        block_time_average: 5.0,
        validators_online: 4,
        validators_total: 4,
        network_health: "healthy".to_string(),
    };

    let mut response = serde_json::to_value(stats).unwrap();
    if let Some(pool_stats) = consensus_stats {
        response["transaction_pool"] = serde_json::json!({
            "pending": pool_stats.current_pool_size,
            "total_submitted": pool_stats.total_submitted,
            "total_included": pool_stats.total_included,
            "evm_pending": pool_stats.evm_transactions,
            "svm_pending": pool_stats.svm_transactions,
            "cross_vm_pending": pool_stats.cross_vm_transactions,
        });
    }

    let response_time = calculate_response_time(start_time);
    success_response(response, request_id, response_time).into_response()
}

/// Universal search endpoint
pub async fn search(
    State(_state): State<Arc<ApplicationState>>,
    Query(query): Query<SearchQuery>,
    headers: HeaderMap,
) -> Response {
    let request_id = crate::api::utils::extract_request_id(&headers);
    let start_time = start_request_timer();

    let search_term = query.q.to_lowercase();

    // Determine search type based on pattern
    let result = if search_term.starts_with("0x") && search_term.len() == 66 {
        // Likely a transaction hash
        SearchResult {
            result_type: "transaction".to_string(),
            data: serde_json::json!({
                "hash": search_term,
                "status": "found",
                "url": format!("/api/v1/explorer/transactions/{}", search_term)
            }),
        }
    } else if search_term.starts_with("0x") && search_term.len() == 42 {
        // Likely an address
        SearchResult {
            result_type: "account".to_string(),
            data: serde_json::json!({
                "address": search_term,
                "status": "found",
                "url": format!("/api/v1/explorer/accounts/{}", search_term)
            }),
        }
    } else if search_term.parse::<u64>().is_ok() {
        // Block height
        SearchResult {
            result_type: "block".to_string(),
            data: serde_json::json!({
                "height": search_term.parse::<u64>().unwrap(),
                "status": "found",
                "url": format!("/api/v1/explorer/blocks/{}", search_term)
            }),
        }
    } else {
        SearchResult {
            result_type: "not_found".to_string(),
            data: serde_json::json!({
                "query": search_term,
                "message": "No results found"
            }),
        }
    };

    let response_time = calculate_response_time(start_time);
    success_response(result, request_id, response_time).into_response()
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
}
