//! RPC client for communicating with external VM processes
//!
//! This module provides the actual RPC client implementation that communicates
//! with Reth (EVM) and Solana (SVM) processes via their JSON-RPC endpoints.

use crate::error::{ApplicationError, ApplicationResult};
use multivm_common::VmType;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tracing::{debug, error};

/// RPC client for external VM communication
#[derive(Clone, Debug)]
pub struct VmRpcClient {
    /// HTTP client
    client: reqwest::Client,
    /// VM type
    vm_type: VmType,
    /// RPC endpoint URL
    rpc_url: String,
    /// Request timeout
    #[allow(dead_code)]
    timeout: Duration,
}

/// JSON-RPC request
#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    method: String,
    params: Vec<Value>,
    id: u64,
}

/// JSON-RPC response
#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    result: Option<Value>,
    error: Option<JsonRpcError>,
    #[allow(dead_code)]
    id: u64,
}

/// JSON-RPC error
#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[allow(dead_code)]
    data: Option<Value>,
}

impl VmRpcClient {
    /// Create new RPC client
    pub fn new(vm_type: VmType, rpc_url: String, timeout: Duration) -> Self {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();

        Self {
            client,
            vm_type,
            rpc_url,
            timeout,
        }
    }

    /// Make JSON-RPC call to external VM
    pub async fn call_method(&self, method: &str, params: Vec<Value>) -> ApplicationResult<Value> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            method: method.to_string(),
            params,
            id: rand::random(),
        };

        debug!("Making RPC call to {:?}: {}", self.vm_type, method);

        let response = self
            .client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await
            .map_err(|e| ApplicationError::GatewayError {
                vm_type: format!("{:?}", self.vm_type),
                message: format!("RPC request failed: {}", e),
            })?;

        if !response.status().is_success() {
            return Err(ApplicationError::GatewayError {
                vm_type: format!("{:?}", self.vm_type),
                message: format!("RPC returned status: {}", response.status()),
            });
        }

        let rpc_response: JsonRpcResponse =
            response
                .json()
                .await
                .map_err(|e| ApplicationError::GatewayError {
                    vm_type: format!("{:?}", self.vm_type),
                    message: format!("Failed to parse RPC response: {}", e),
                })?;

        if let Some(error) = rpc_response.error {
            return Err(ApplicationError::GatewayError {
                vm_type: format!("{:?}", self.vm_type),
                message: format!("RPC error {}: {}", error.code, error.message),
            });
        }

        rpc_response
            .result
            .ok_or_else(|| ApplicationError::GatewayError {
                vm_type: format!("{:?}", self.vm_type),
                message: "RPC response missing result".to_string(),
            })
    }

    /// Get latest block number
    pub async fn get_latest_block_number(&self) -> ApplicationResult<u64> {
        let method = match self.vm_type {
            VmType::Evm => "eth_blockNumber",
            VmType::Svm => "getSlot",
        };

        let result = self.call_method(method, vec![]).await?;

        // Parse result based on VM type
        match self.vm_type {
            VmType::Evm => {
                let hex_str = result
                    .as_str()
                    .ok_or_else(|| ApplicationError::GatewayError {
                        vm_type: format!("{:?}", self.vm_type),
                        message: "Invalid block number format".to_string(),
                    })?;

                u64::from_str_radix(hex_str.trim_start_matches("0x"), 16).map_err(|e| {
                    ApplicationError::GatewayError {
                        vm_type: format!("{:?}", self.vm_type),
                        message: format!("Failed to parse block number: {}", e),
                    }
                })
            }
            VmType::Svm => result
                .as_u64()
                .ok_or_else(|| ApplicationError::GatewayError {
                    vm_type: format!("{:?}", self.vm_type),
                    message: "Invalid slot number format".to_string(),
                }),
        }
    }

    /// Get block by number
    pub async fn get_block_by_number(
        &self,
        block_number: u64,
        full_txs: bool,
    ) -> ApplicationResult<Value> {
        match self.vm_type {
            VmType::Evm => {
                let block_param = format!("0x{:x}", block_number);
                self.call_method(
                    "eth_getBlockByNumber",
                    vec![Value::String(block_param), Value::Bool(full_txs)],
                )
                .await
            }
            VmType::Svm => {
                self.call_method(
                    "getBlock",
                    vec![
                        Value::Number(block_number.into()),
                        Value::Object(
                            serde_json::json!({
                                "encoding": "json",
                                "transactionDetails": if full_txs { "full" } else { "signatures" },
                                "rewards": false
                            })
                            .as_object()
                            .unwrap()
                            .clone(),
                        ),
                    ],
                )
                .await
            }
        }
    }

    /// Get transaction by hash
    pub async fn get_transaction(&self, tx_hash: &str) -> ApplicationResult<Value> {
        match self.vm_type {
            VmType::Evm => {
                self.call_method(
                    "eth_getTransactionByHash",
                    vec![Value::String(tx_hash.to_string())],
                )
                .await
            }
            VmType::Svm => {
                self.call_method(
                    "getTransaction",
                    vec![
                        Value::String(tx_hash.to_string()),
                        Value::String("json".to_string()),
                    ],
                )
                .await
            }
        }
    }

    /// Get account balance
    pub async fn get_balance(&self, address: &str) -> ApplicationResult<String> {
        let result = match self.vm_type {
            VmType::Evm => {
                let balance = self
                    .call_method(
                        "eth_getBalance",
                        vec![
                            Value::String(address.to_string()),
                            Value::String("latest".to_string()),
                        ],
                    )
                    .await?;

                balance
                    .as_str()
                    .ok_or_else(|| ApplicationError::GatewayError {
                        vm_type: format!("{:?}", self.vm_type),
                        message: "Invalid balance format".to_string(),
                    })?
                    .to_string()
            }
            VmType::Svm => {
                let balance = self
                    .call_method("getBalance", vec![Value::String(address.to_string())])
                    .await?;

                balance["value"]
                    .as_u64()
                    .ok_or_else(|| ApplicationError::GatewayError {
                        vm_type: format!("{:?}", self.vm_type),
                        message: "Invalid balance format".to_string(),
                    })?
                    .to_string()
            }
        };

        Ok(result)
    }

    /// Send raw transaction
    pub async fn send_raw_transaction(&self, raw_tx: &str) -> ApplicationResult<String> {
        let method = match self.vm_type {
            VmType::Evm => "eth_sendRawTransaction",
            VmType::Svm => "sendTransaction",
        };

        let result = self
            .call_method(method, vec![Value::String(raw_tx.to_string())])
            .await?;

        result
            .as_str()
            .ok_or_else(|| ApplicationError::GatewayError {
                vm_type: format!("{:?}", self.vm_type),
                message: "Invalid transaction hash format".to_string(),
            })
            .map(|s| s.to_string())
    }

    /// Check if VM is healthy
    pub async fn health_check(&self) -> ApplicationResult<bool> {
        // Try to get latest block as health check
        match self.get_latest_block_number().await {
            Ok(block_num) => {
                debug!(
                    "{:?} health check passed, latest block: {}",
                    self.vm_type, block_num
                );
                Ok(true)
            }
            Err(e) => {
                error!("{:?} health check failed: {}", self.vm_type, e);
                Ok(false)
            }
        }
    }

    /// Get VM-specific information
    pub async fn get_vm_info(&self) -> ApplicationResult<Value> {
        match self.vm_type {
            VmType::Evm => {
                // Get network ID and client version
                let network_id = self.call_method("net_version", vec![]).await?;
                let client_version = self.call_method("web3_clientVersion", vec![]).await?;

                Ok(serde_json::json!({
                    "vm_type": "EVM",
                    "network_id": network_id,
                    "client_version": client_version,
                    "rpc_endpoint": self.rpc_url,
                }))
            }
            VmType::Svm => {
                // Get cluster info
                let version = self.call_method("getVersion", vec![]).await?;
                let genesis_hash = self.call_method("getGenesisHash", vec![]).await?;

                Ok(serde_json::json!({
                    "vm_type": "SVM",
                    "version": version,
                    "genesis_hash": genesis_hash,
                    "rpc_endpoint": self.rpc_url,
                }))
            }
        }
    }
}

/// Create RPC clients for both VMs
pub fn create_vm_rpc_clients(
    evm_url: String,
    svm_url: String,
    timeout: Duration,
) -> (VmRpcClient, VmRpcClient) {
    let evm_client = VmRpcClient::new(VmType::Evm, evm_url, timeout);
    let svm_client = VmRpcClient::new(VmType::Svm, svm_url, timeout);

    (evm_client, svm_client)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_client_creation() {
        let client = VmRpcClient::new(
            VmType::Evm,
            "http://localhost:8545".to_string(),
            Duration::from_secs(30),
        );

        assert_eq!(client.vm_type, VmType::Evm);
        assert_eq!(client.rpc_url, "http://localhost:8545");
    }
}
