//! Ethereum (Reth) RPC relay implementation

use crate::{
    config::{NodeAuth, NodeConfig},
    error::{RpcError, RpcResult},
    relay::RpcRelay,
    types::{HealthStatus, JsonRpcRequest, JsonRpcResponse, NodeHealth, VmType},
};
use async_trait::async_trait;
use reqwest::{Client, RequestBuilder};
use serde_json::Value;
use std::time::{Duration, SystemTime};
use tracing::{debug, error, warn};

/// Ethereum RPC relay for Reth nodes
pub struct EthereumRelay {
    config: NodeConfig,
    client: Client,
    last_health_check: std::sync::Arc<std::sync::RwLock<SystemTime>>,
    last_health_status: std::sync::Arc<std::sync::RwLock<HealthStatus>>,
}

impl EthereumRelay {
    /// Create new Ethereum relay
    pub async fn new(config: NodeConfig) -> RpcResult<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .pool_max_idle_per_host(config.max_concurrent_requests as usize)
            .build()
            .map_err(|e| RpcError::ProxyError {
                message: format!("Failed to create HTTP client: {}", e),
            })?;

        let relay = Self {
            config,
            client,
            last_health_check: std::sync::Arc::new(std::sync::RwLock::new(SystemTime::UNIX_EPOCH)),
            last_health_status: std::sync::Arc::new(std::sync::RwLock::new(HealthStatus::Unknown)),
        };

        // Perform initial health check
        let _ = relay.check_health().await;

        Ok(relay)
    }

    /// Check node health
    async fn check_health(&self) -> HealthStatus {
        let _start_time = SystemTime::now();
        
        // Use web3_clientVersion for basic connectivity check
        let health_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "web3_clientVersion".to_string(),
            params: None,
            id: Some(Value::Number(1.into())),
        };

        match self.send_request(&health_request).await {
            Ok(response) => {
                if response.error.is_some() {
                    warn!("Ethereum node {} returned error for health check", self.config.name);
                    *self.last_health_status.write().unwrap() = HealthStatus::Degraded;
                    HealthStatus::Degraded
                } else {
                    debug!("Ethereum node {} is healthy", self.config.name);
                    *self.last_health_status.write().unwrap() = HealthStatus::Healthy;
                    HealthStatus::Healthy
                }
            }
            Err(e) => {
                error!("Health check failed for Ethereum node {}: {}", self.config.name, e);
                *self.last_health_status.write().unwrap() = HealthStatus::Unhealthy;
                HealthStatus::Unhealthy
            }
        }
    }

    /// Send HTTP request to Reth node
    async fn send_request(&self, request: &JsonRpcRequest) -> RpcResult<JsonRpcResponse> {
        let mut req_builder = self.client.post(self.config.url.as_str());

        // Add authentication if configured
        if let Some(ref auth) = self.config.auth {
            req_builder = self.add_authentication(req_builder, auth);
        }

        // Send request
        let response = req_builder
            .json(request)
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    RpcError::TimeoutError {
                        operation: format!("Ethereum RPC call: {}", request.method),
                    }
                } else {
                    RpcError::NetworkError {
                        message: format!("Failed to send request to {}: {}", self.config.name, e),
                    }
                }
            })?;

        // Check HTTP status
        if !response.status().is_success() {
            return Err(RpcError::RelayError {
                vm_type: "ethereum".to_string(),
                message: format!("HTTP {} from {}", response.status(), self.config.name),
            });
        }

        // Parse JSON response
        let rpc_response: JsonRpcResponse = response
            .json()
            .await
            .map_err(|e| RpcError::SerializationError {
                message: format!("Failed to parse response from {}: {}", self.config.name, e),
            })?;

        Ok(rpc_response)
    }

    /// Add authentication to request
    fn add_authentication(&self, mut req_builder: RequestBuilder, auth: &NodeAuth) -> RequestBuilder {
        if let Some(ref token) = auth.bearer_token {
            req_builder = req_builder.bearer_auth(token);
        } else if let (Some(ref username), Some(ref password)) = (&auth.username, &auth.password) {
            req_builder = req_builder.basic_auth(username, Some(password));
        }
        req_builder
    }

    /// Get extended health information
    async fn get_extended_health(&self) -> RpcResult<Value> {
        // Get chain ID
        let chain_id_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "eth_chainId".to_string(),
            params: None,
            id: Some(Value::Number(2.into())),
        };

        let chain_id_response = self.send_request(&chain_id_request).await?;

        // Get latest block number
        let block_number_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "eth_blockNumber".to_string(),
            params: None,
            id: Some(Value::Number(3.into())),
        };

        let block_number_response = self.send_request(&block_number_request).await?;

        // Get syncing status
        let syncing_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "eth_syncing".to_string(),
            params: None,
            id: Some(Value::Number(4.into())),
        };

        let syncing_response = self.send_request(&syncing_request).await?;

        // Combine results
        let health_info = serde_json::json!({
            "chain_id": chain_id_response.result,
            "block_number": block_number_response.result,
            "syncing": syncing_response.result,
            "node_name": self.config.name,
            "url": self.config.url.to_string()
        });

        Ok(health_info)
    }

    /// Check if method is supported by this relay
    pub fn supports_method(&self, method: &str) -> bool {
        // Ethereum JSON-RPC methods
        matches!(
            method,
            // Standard methods
            "web3_clientVersion" | "web3_sha3" |
            "net_version" | "net_listening" | "net_peerCount" |
            "eth_protocolVersion" | "eth_syncing" | "eth_coinbase" |
            "eth_mining" | "eth_hashrate" | "eth_gasPrice" |
            "eth_accounts" | "eth_blockNumber" | "eth_getBalance" |
            "eth_getStorageAt" | "eth_getTransactionCount" |
            "eth_getBlockTransactionCountByHash" | "eth_getBlockTransactionCountByNumber" |
            "eth_getUncleCountByBlockHash" | "eth_getUncleCountByBlockNumber" |
            "eth_getCode" | "eth_sign" | "eth_signTransaction" |
            "eth_sendTransaction" | "eth_sendRawTransaction" |
            "eth_call" | "eth_estimateGas" | "eth_getBlockByHash" |
            "eth_getBlockByNumber" | "eth_getTransactionByHash" |
            "eth_getTransactionByBlockHashAndIndex" | "eth_getTransactionByBlockNumberAndIndex" |
            "eth_getTransactionReceipt" | "eth_getUncleByBlockHashAndIndex" |
            "eth_getUncleByBlockNumberAndIndex" | "eth_newFilter" |
            "eth_newBlockFilter" | "eth_newPendingTransactionFilter" |
            "eth_uninstallFilter" | "eth_getFilterChanges" |
            "eth_getFilterLogs" | "eth_getLogs" | "eth_getWork" |
            "eth_submitWork" | "eth_submitHashrate" | "eth_chainId" |
            // EIP methods
            "eth_getProof" | "eth_feeHistory" | "eth_maxPriorityFeePerGas" |
            // Debug methods (if enabled)
            "debug_traceTransaction" | "debug_traceBlockByNumber" | "debug_traceBlockByHash" |
            // Trace methods (if enabled)
            "trace_transaction" | "trace_block" | "trace_filter" |
            // Admin methods (if enabled)
            "admin_nodeInfo" | "admin_peers"
        )
    }
}

#[async_trait]
impl RpcRelay for EthereumRelay {
    async fn forward_request(&self, request: JsonRpcRequest) -> RpcResult<JsonRpcResponse> {
        // Check if method is supported
        if !self.supports_method(&request.method) {
            return Err(RpcError::InvalidMethod {
                method: request.method.clone(),
            });
        }

        debug!("Forwarding Ethereum RPC request: {} to {}", request.method, self.config.name);

        // Forward request with retry logic
        let mut last_error = None;
        for attempt in 1..=3 {
            match self.send_request(&request).await {
                Ok(response) => {
                    debug!("Successfully forwarded request {} to {}", request.method, self.config.name);
                    return Ok(response);
                }
                Err(e) => {
                    warn!("Attempt {} failed for {} on {}: {}", attempt, request.method, self.config.name, e);
                    last_error = Some(e);
                    
                    // Don't retry on client errors
                    if matches!(last_error, Some(RpcError::InvalidParams { .. })) {
                        break;
                    }
                    
                    // Wait before retry
                    if attempt < 3 {
                        tokio::time::sleep(Duration::from_millis(100 * attempt as u64)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| RpcError::Internal {
            message: "All retry attempts failed".to_string(),
        }))
    }

    async fn get_health(&self) -> NodeHealth {
        let start_time = SystemTime::now();
        let status = self.check_health().await;
        let response_time = start_time.elapsed().unwrap_or_default();

        // Update last health check time
        *self.last_health_check.write().unwrap() = SystemTime::now();

        let version_info = if status == HealthStatus::Healthy {
            // Try to get version info
            match self.get_extended_health().await {
                Ok(info) => Some(info.to_string()),
                Err(_) => None,
            }
        } else {
            None
        };

        NodeHealth {
            name: self.config.name.clone(),
            vm_type: VmType::Ethereum,
            status,
            response_time_ms: response_time.as_millis() as u64,
            last_success: if status == HealthStatus::Healthy {
                SystemTime::now()
            } else {
                SystemTime::UNIX_EPOCH
            },
            error_message: if status != HealthStatus::Healthy {
                Some(format!("Node {} is not responding correctly", self.config.name))
            } else {
                None
            },
            version_info,
        }
    }

    fn vm_type(&self) -> VmType {
        VmType::Ethereum
    }

    fn config(&self) -> &NodeConfig {
        &self.config
    }
}

/// Ethereum-specific utility functions
impl EthereumRelay {
    /// Convert hex string to u64 (for block numbers, etc.)
    pub fn hex_to_u64(hex_str: &str) -> RpcResult<u64> {
        let cleaned = hex_str.strip_prefix("0x").unwrap_or(hex_str);
        u64::from_str_radix(cleaned, 16).map_err(|e| RpcError::SerializationError {
            message: format!("Invalid hex number: {}", e),
        })
    }

    /// Convert u64 to hex string
    pub fn u64_to_hex(value: u64) -> String {
        format!("0x{:x}", value)
    }

    /// Validate Ethereum address format
    pub fn is_valid_address(address: &str) -> bool {
        if !address.starts_with("0x") {
            return false;
        }
        if address.len() != 42 {
            return false;
        }
        address[2..].chars().all(|c| c.is_ascii_hexdigit())
    }

    /// Validate transaction hash format
    pub fn is_valid_tx_hash(hash: &str) -> bool {
        if !hash.starts_with("0x") {
            return false;
        }
        if hash.len() != 66 {
            return false;
        }
        hash[2..].chars().all(|c| c.is_ascii_hexdigit())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NodeConfig;
    use std::time::Duration;

    fn create_test_config() -> NodeConfig {
        NodeConfig {
            name: "test-reth".to_string(),
            url: "http://localhost:8545".parse().unwrap(),
            timeout: Duration::from_secs(5),
            max_concurrent_requests: 10,
            health_check_interval: Duration::from_secs(30),
            priority: 100,
            auth: None,
        }
    }

    #[test]
    fn test_method_support() {
        let config = create_test_config();
        let client = Client::new();
        let relay = EthereumRelay {
            config,
            client,
            last_health_check: std::sync::Arc::new(std::sync::RwLock::new(SystemTime::UNIX_EPOCH)),
            last_health_status: std::sync::Arc::new(std::sync::RwLock::new(HealthStatus::Unknown)),
        };

        assert!(relay.supports_method("eth_blockNumber"));
        assert!(relay.supports_method("eth_getBalance"));
        assert!(relay.supports_method("web3_clientVersion"));
        assert!(!relay.supports_method("getSlot")); // Solana method
        assert!(!relay.supports_method("invalid_method"));
    }

    #[test]
    fn test_hex_conversion() {
        assert_eq!(EthereumRelay::hex_to_u64("0x1a").unwrap(), 26);
        assert_eq!(EthereumRelay::hex_to_u64("1a").unwrap(), 26);
        assert_eq!(EthereumRelay::u64_to_hex(26), "0x1a");
    }

    #[test]
    fn test_address_validation() {
        assert!(EthereumRelay::is_valid_address("0x1234567890123456789012345678901234567890"));
        assert!(!EthereumRelay::is_valid_address("1234567890123456789012345678901234567890"));
        assert!(!EthereumRelay::is_valid_address("0x123")); // Too short
        assert!(!EthereumRelay::is_valid_address("0x123456789012345678901234567890123456789g")); // Invalid char
    }

    #[test]
    fn test_tx_hash_validation() {
        assert!(EthereumRelay::is_valid_tx_hash("0x1234567890123456789012345678901234567890123456789012345678901234"));
        assert!(!EthereumRelay::is_valid_tx_hash("1234567890123456789012345678901234567890123456789012345678901234"));
        assert!(!EthereumRelay::is_valid_tx_hash("0x123")); // Too short
    }
}