//! MultiVM-specific API implementations

use crate::{
    error::{RpcError, RpcResult},
    types::{
        CrossVmStats, JsonRpcRequest, JsonRpcResponse, MultiVmRequest, MultiVmResponse,
        NetworkStatsDetail, TransactionStatus, VmType,
    },
};
use multivm_account_mapping::{AccountAddress, AccountMappingLayer, MultivmAccountId};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, warn};

/// MultiVM API handler
pub struct MultiVmApi {
    /// Account mapping service
    account_mapping: Option<Arc<dyn AccountMappingLayer>>,
    /// Enable extended features
    extended_features: bool,
}

impl MultiVmApi {
    pub fn new() -> Self {
        Self {
            account_mapping: None,
            extended_features: true,
        }
    }

    pub fn with_account_mapping(account_mapping: Arc<dyn AccountMappingLayer>) -> Self {
        Self {
            account_mapping: Some(account_mapping),
            extended_features: true,
        }
    }

    /// Handle MultiVM-specific RPC request
    pub async fn handle_request(&self, request: JsonRpcRequest) -> RpcResult<JsonRpcResponse> {
        debug!("Processing MultiVM request: {}", request.method);

        let result = match request.method.as_str() {
            "multivm_getVersion" => self.get_version().await,
            "multivm_getNetworkStats" => self.get_network_stats(&request).await,
            "multivm_getCrossChainBalance" => self.get_cross_chain_balance(&request).await,
            "multivm_getAccountBinding" => self.get_account_binding(&request).await,
            "multivm_resolveMutliVmAccount" => self.resolve_multivm_account(&request).await,
            "multivm_submitCrossVmTransaction" => self.submit_cross_vm_transaction(&request).await,
            "multivm_getTransactionStatus" => self.get_transaction_status(&request).await,
            "multivm_getSupportedChains" => self.get_supported_chains().await,
            "multivm_estimateCrossVmFee" => self.estimate_cross_vm_fee(&request).await,
            _ => {
                return Err(RpcError::InvalidMethod {
                    method: request.method.clone(),
                });
            }
        };

        match result {
            Ok(value) => Ok(JsonRpcResponse::success(request.id, value)),
            Err(e) => Ok(JsonRpcResponse::error(
                request.id,
                e.to_jsonrpc_error(),
            )),
        }
    }

    /// Get MultiVM version and capabilities
    async fn get_version(&self) -> RpcResult<Value> {
        Ok(json!({
            "version": crate::VERSION,
            "name": crate::NAME,
            "supported_vms": ["ethereum", "solana"],
            "features": {
                "account_mapping": self.account_mapping.is_some(),
                "cross_vm_transactions": self.extended_features,
                "balance_aggregation": true,
                "network_stats": true
            },
            "api_version": "1.0.0"
        }))
    }

    /// Get network statistics across all VMs
    async fn get_network_stats(&self, request: &JsonRpcRequest) -> RpcResult<Value> {
        let include_details = request
            .params
            .as_ref()
            .and_then(|p| p.get("include_details"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Mock network stats - in production, this would aggregate real data
        let ethereum_stats = NetworkStatsDetail {
            height: 18_500_000,
            tps: 15.2,
            avg_block_time: 12.1,
            active_nodes: 8000,
            health_score: 95,
        };

        let solana_stats = NetworkStatsDetail {
            height: 250_000_000,
            tps: 2500.0,
            avg_block_time: 0.4,
            active_nodes: 1500,
            health_score: 98,
        };

        let cross_vm_stats = CrossVmStats {
            total_cross_vm_txs: 125_000,
            cross_vm_txs_24h: 1_234,
            bound_accounts: 45_600,
            success_rate: 99.2,
        };

        let response = MultiVmResponse::NetworkStats {
            ethereum: ethereum_stats,
            solana: solana_stats,
            cross_vm: cross_vm_stats,
        };

        if include_details {
            Ok(serde_json::to_value(response)?)
        } else {
            // Return simplified stats
            Ok(json!({
                "ethereum_height": 18_500_000,
                "solana_height": 250_000_000,
                "cross_vm_transactions_24h": 1_234,
                "bound_accounts": 45_600,
                "overall_health": 96
            }))
        }
    }

    /// Get cross-chain balance for an address
    async fn get_cross_chain_balance(&self, request: &JsonRpcRequest) -> RpcResult<Value> {
        let params = request.params.as_ref().ok_or_else(|| RpcError::InvalidParams {
            reason: "Missing parameters".to_string(),
        })?;

        let address = params
            .get("address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError::InvalidParams {
                reason: "Missing or invalid 'address' parameter".to_string(),
            })?;

        let vm_types = params
            .get("vm_types")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .filter_map(|s| match s {
                        "ethereum" => Some(VmType::Ethereum),
                        "solana" => Some(VmType::Solana),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec![VmType::Ethereum, VmType::Solana]);

        // In production, this would query actual balances from each VM
        let mut balances = HashMap::new();

        for vm_type in vm_types {
            let balance = match vm_type {
                VmType::Ethereum => {
                    // Mock Ethereum balance
                    "1.234567890123456789" // ETH balance
                }
                VmType::Solana => {
                    // Mock Solana balance
                    "5.678901234" // SOL balance
                }
                VmType::MultiVm => continue,
            };
            balances.insert(vm_type, balance.to_string());
        }

        let response = MultiVmResponse::CrossChainBalance {
            balances,
            total_usd: Some("2850.75".to_string()), // Mock USD value
        };

        Ok(serde_json::to_value(response)?)
    }

    /// Get account binding information
    async fn get_account_binding(&self, request: &JsonRpcRequest) -> RpcResult<Value> {
        let params = request.params.as_ref().ok_or_else(|| RpcError::InvalidParams {
            reason: "Missing parameters".to_string(),
        })?;

        let multivm_account = params
            .get("multivm_account")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError::InvalidParams {
                reason: "Missing or invalid 'multivm_account' parameter".to_string(),
            })?;

        if let Some(ref account_mapping) = self.account_mapping {
            // Parse MultiVM account ID
            let multivm_id = MultivmAccountId::from_seed(multivm_account.as_bytes());

            match account_mapping.get_binding(&multivm_id).await {
                Ok(Some(binding)) => {
                    let response = MultiVmResponse::AccountBinding {
                        ethereum_account: binding.evm_account.map(|addr| addr.to_string()),
                        solana_account: binding.svm_account.map(|addr| addr.to_string()),
                        binding_timestamp: binding.created_at,
                    };
                    Ok(serde_json::to_value(response)?)
                }
                Ok(None) => Err(RpcError::InvalidParams {
                    reason: "MultiVM account not found".to_string(),
                }),
                Err(e) => Err(RpcError::Internal {
                    message: format!("Failed to get account binding: {}", e),
                }),
            }
        } else {
            Err(RpcError::Internal {
                message: "Account mapping not available".to_string(),
            })
        }
    }

    /// Resolve MultiVM account from regular address
    async fn resolve_multivm_account(&self, request: &JsonRpcRequest) -> RpcResult<Value> {
        let params = request.params.as_ref().ok_or_else(|| RpcError::InvalidParams {
            reason: "Missing parameters".to_string(),
        })?;

        let address = params
            .get("address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError::InvalidParams {
                reason: "Missing or invalid 'address' parameter".to_string(),
            })?;

        if let Some(ref account_mapping) = self.account_mapping {
            // Parse address (try both Ethereum and Solana formats)
            let account_addr = if address.starts_with("0x") && address.len() == 42 {
                // Ethereum address - for now we'll create a mock address
                // In production, this would use proper address parsing
                AccountAddress::Ethereum(multivm_account_mapping::address::EthereumAddress(
                    [0u8; 20] // Mock address
                ))
            } else if address.len() >= 32 && address.len() <= 44 {
                // Solana address (base58) - mock for now
                AccountAddress::Solana(multivm_account_mapping::address::SolanaAddress(
                    [0u8; 32] // Mock address
                ))
            } else {
                return Err(RpcError::InvalidParams {
                    reason: "Invalid address format".to_string(),
                });
            };

            match account_mapping.resolve_multivm_account(&account_addr).await {
                Ok(Some(multivm_id)) => Ok(json!({
                    "multivm_account": multivm_id.to_string(),
                    "original_address": address,
                    "vm_type": if address.starts_with("0x") { "ethereum" } else { "solana" }
                })),
                Ok(None) => Err(RpcError::InvalidParams {
                    reason: "Address not bound to any MultiVM account".to_string(),
                }),
                Err(e) => Err(RpcError::Internal {
                    message: format!("Failed to resolve MultiVM account: {}", e),
                }),
            }
        } else {
            Err(RpcError::Internal {
                message: "Account mapping not available".to_string(),
            })
        }
    }

    /// Submit cross-VM transaction
    async fn submit_cross_vm_transaction(&self, request: &JsonRpcRequest) -> RpcResult<Value> {
        let params = request.params.as_ref().ok_or_else(|| RpcError::InvalidParams {
            reason: "Missing parameters".to_string(),
        })?;

        let from_vm = params
            .get("from_vm")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "ethereum" => Some(VmType::Ethereum),
                "solana" => Some(VmType::Solana),
                _ => None,
            })
            .ok_or_else(|| RpcError::InvalidParams {
                reason: "Missing or invalid 'from_vm' parameter".to_string(),
            })?;

        let to_vm = params
            .get("to_vm")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "ethereum" => Some(VmType::Ethereum),
                "solana" => Some(VmType::Solana),
                _ => None,
            })
            .ok_or_else(|| RpcError::InvalidParams {
                reason: "Missing or invalid 'to_vm' parameter".to_string(),
            })?;

        let _transaction_data = params
            .get("transaction_data")
            .ok_or_else(|| RpcError::InvalidParams {
                reason: "Missing 'transaction_data' parameter".to_string(),
            })?;

        // In production, this would actually process the cross-VM transaction
        let transaction_id = format!("{}_{}", 
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            rand::random::<u32>()
        );

        let mut confirmations = HashMap::new();
        confirmations.insert(from_vm, 1u64);
        confirmations.insert(to_vm, 0u64);

        let response = MultiVmResponse::CrossVmTransaction {
            transaction_id: transaction_id.clone(),
            status: TransactionStatus::Pending,
            confirmations,
        };

        debug!("Submitted cross-VM transaction: {}", transaction_id);
        Ok(serde_json::to_value(response)?)
    }

    /// Get cross-VM transaction status
    async fn get_transaction_status(&self, request: &JsonRpcRequest) -> RpcResult<Value> {
        let params = request.params.as_ref().ok_or_else(|| RpcError::InvalidParams {
            reason: "Missing parameters".to_string(),
        })?;

        let transaction_id = params
            .get("transaction_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError::InvalidParams {
                reason: "Missing or invalid 'transaction_id' parameter".to_string(),
            })?;

        // Mock transaction status lookup
        let mut confirmations = HashMap::new();
        confirmations.insert(VmType::Ethereum, 12u64);
        confirmations.insert(VmType::Solana, 32u64);

        let response = MultiVmResponse::CrossVmTransaction {
            transaction_id: transaction_id.to_string(),
            status: TransactionStatus::Confirmed,
            confirmations,
        };

        Ok(serde_json::to_value(response)?)
    }

    /// Get supported blockchain networks
    async fn get_supported_chains(&self) -> RpcResult<Value> {
        Ok(json!({
            "chains": [
                {
                    "name": "ethereum",
                    "display_name": "Ethereum",
                    "native_token": "ETH",
                    "decimals": 18,
                    "chain_id": 1,
                    "features": ["smart_contracts", "evm", "account_binding"]
                },
                {
                    "name": "solana",
                    "display_name": "Solana",
                    "native_token": "SOL",
                    "decimals": 9,
                    "features": ["programs", "svm", "account_binding", "high_throughput"]
                }
            ],
            "cross_vm_features": [
                "account_binding",
                "balance_aggregation",
                "cross_chain_transactions",
                "unified_identity"
            ]
        }))
    }

    /// Estimate fee for cross-VM operation
    async fn estimate_cross_vm_fee(&self, request: &JsonRpcRequest) -> RpcResult<Value> {
        let params = request.params.as_ref().ok_or_else(|| RpcError::InvalidParams {
            reason: "Missing parameters".to_string(),
        })?;

        let operation = params
            .get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("transfer");

        let from_vm = params
            .get("from_vm")
            .and_then(|v| v.as_str())
            .unwrap_or("ethereum");

        let to_vm = params
            .get("to_vm")
            .and_then(|v| v.as_str())
            .unwrap_or("solana");

        // Mock fee estimation
        let base_fee = match (from_vm, to_vm) {
            ("ethereum", "solana") => 0.001, // ETH
            ("solana", "ethereum") => 0.1,   // SOL
            _ => 0.0001,
        };

        let priority_fee = base_fee * 0.1;
        let total_fee = base_fee + priority_fee;

        Ok(json!({
            "operation": operation,
            "from_vm": from_vm,
            "to_vm": to_vm,
            "fees": {
                "base_fee": base_fee,
                "priority_fee": priority_fee,
                "total_fee": total_fee,
                "currency": if from_vm == "ethereum" { "ETH" } else { "SOL" }
            },
            "estimated_time": "2-5 minutes"
        }))
    }
}

/// Utility functions for MultiVM API
pub mod utils {
    use super::*;

    /// Validate MultiVM account format
    pub fn validate_multivm_account(account: &str) -> bool {
        // MultiVM accounts are typically hex-encoded or base58-encoded
        account.len() >= 32 && account.len() <= 64
    }

    /// Parse VM type from string
    pub fn parse_vm_type(vm_str: &str) -> Option<VmType> {
        match vm_str.to_lowercase().as_str() {
            "ethereum" | "eth" | "evm" => Some(VmType::Ethereum),
            "solana" | "sol" | "svm" => Some(VmType::Solana),
            "multivm" | "multi" => Some(VmType::MultiVm),
            _ => None,
        }
    }

    /// Format balance for display
    pub fn format_balance(balance: &str, decimals: u8) -> String {
        match balance.parse::<f64>() {
            Ok(value) => {
                if value == 0.0 {
                    "0".to_string()
                } else if value < 0.000001 {
                    format!("{:.precision$e}", value, precision = decimals.min(6) as usize)
                } else {
                    format!("{:.precision$}", value, precision = decimals.min(6) as usize)
                }
            }
            Err(_) => balance.to_string(),
        }
    }

    /// Generate transaction ID
    pub fn generate_transaction_id() -> String {
        use std::time::SystemTime;
        
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        format!("mvtx_{}_{:08x}", timestamp, rand::random::<u32>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_get_version() {
        let api = MultiVmApi::new();
        let result = api.get_version().await.unwrap();
        
        assert!(result.get("version").is_some());
        assert!(result.get("supported_vms").is_some());
        assert!(result.get("features").is_some());
    }

    #[tokio::test]
    async fn test_get_supported_chains() {
        let api = MultiVmApi::new();
        let result = api.get_supported_chains().await.unwrap();
        
        let chains = result.get("chains").unwrap().as_array().unwrap();
        assert_eq!(chains.len(), 2);
        
        let ethereum = &chains[0];
        assert_eq!(ethereum.get("name").unwrap().as_str().unwrap(), "ethereum");
        
        let solana = &chains[1];
        assert_eq!(solana.get("name").unwrap().as_str().unwrap(), "solana");
    }

    #[tokio::test]
    async fn test_estimate_cross_vm_fee() {
        let api = MultiVmApi::new();
        
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "multivm_estimateCrossVmFee".to_string(),
            params: Some(json!({
                "operation": "transfer",
                "from_vm": "ethereum",
                "to_vm": "solana"
            })),
            id: Some(json!(1)),
        };

        let result = api.estimate_cross_vm_fee(&request).await.unwrap();
        
        assert!(result.get("fees").is_some());
        assert!(result.get("estimated_time").is_some());
    }

    #[test]
    fn test_utility_functions() {
        assert!(utils::validate_multivm_account("1234567890abcdef1234567890abcdef12345678"));
        assert!(!utils::validate_multivm_account("short"));
        
        assert_eq!(utils::parse_vm_type("ethereum"), Some(VmType::Ethereum));
        assert_eq!(utils::parse_vm_type("solana"), Some(VmType::Solana));
        assert_eq!(utils::parse_vm_type("invalid"), None);
        
        let formatted = utils::format_balance("1.234567890123456789", 18);
        assert!(formatted.contains("1.234568")); // Rounded to 6 decimal places
        
        let tx_id = utils::generate_transaction_id();
        assert!(tx_id.starts_with("mvtx_"));
    }
}