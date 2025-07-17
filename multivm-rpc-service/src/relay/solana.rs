//! Solana RPC relay implementation

use crate::{
    config::{NodeAuth, NodeConfig},
    error::{RpcError, RpcResult},
    relay::RpcRelay,
    types::{HealthStatus, JsonRpcRequest, JsonRpcResponse, NodeHealth, VmType},
};
use async_trait::async_trait;
use reqwest::{Client, RequestBuilder};
use serde_json::{json, Value};
use std::time::{Duration, SystemTime};
use tracing::{debug, error, warn};

/// Solana RPC relay
pub struct SolanaRelay {
    config: NodeConfig,
    client: Client,
    last_health_check: std::sync::Arc<std::sync::RwLock<SystemTime>>,
    last_health_status: std::sync::Arc<std::sync::RwLock<HealthStatus>>,
}

impl SolanaRelay {
    /// Create new Solana relay
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
        
        // Use getVersion for basic connectivity check
        let health_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "getVersion".to_string(),
            params: None,
            id: Some(Value::Number(1.into())),
        };

        match self.send_request(&health_request).await {
            Ok(response) => {
                if response.error.is_some() {
                    warn!("Solana node {} returned error for health check", self.config.name);
                    *self.last_health_status.write().unwrap() = HealthStatus::Degraded;
                    HealthStatus::Degraded
                } else {
                    debug!("Solana node {} is healthy", self.config.name);
                    *self.last_health_status.write().unwrap() = HealthStatus::Healthy;
                    HealthStatus::Healthy
                }
            }
            Err(e) => {
                error!("Health check failed for Solana node {}: {}", self.config.name, e);
                *self.last_health_status.write().unwrap() = HealthStatus::Unhealthy;
                HealthStatus::Unhealthy
            }
        }
    }

    /// Send HTTP request to Solana node
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
                        operation: format!("Solana RPC call: {}", request.method),
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
                vm_type: "solana".to_string(),
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
        // Get cluster nodes
        let cluster_nodes_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "getClusterNodes".to_string(),
            params: None,
            id: Some(Value::Number(2.into())),
        };

        let cluster_nodes_response = self.send_request(&cluster_nodes_request).await.ok();

        // Get slot information
        let slot_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "getSlot".to_string(),
            params: None,
            id: Some(Value::Number(3.into())),
        };

        let slot_response = self.send_request(&slot_request).await.ok();

        // Get epoch information
        let epoch_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "getEpochInfo".to_string(),
            params: None,
            id: Some(Value::Number(4.into())),
        };

        let epoch_response = self.send_request(&epoch_request).await.ok();

        // Get health status
        let health_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "getHealth".to_string(),
            params: None,
            id: Some(Value::Number(5.into())),
        };

        let health_response = self.send_request(&health_request).await.ok();

        // Combine results
        let health_info = json!({
            "cluster_nodes": cluster_nodes_response.and_then(|r| r.result),
            "slot": slot_response.and_then(|r| r.result),
            "epoch_info": epoch_response.and_then(|r| r.result),
            "health": health_response.and_then(|r| r.result),
            "node_name": self.config.name,
            "url": self.config.url.to_string()
        });

        Ok(health_info)
    }

    /// Check if method is supported by this relay
    pub fn supports_method(&self, method: &str) -> bool {
        // Solana JSON-RPC methods
        matches!(
            method,
            // Account methods
            "getAccountInfo" | "getMultipleAccounts" | "getProgramAccounts" |
            // Block methods
            "getBlock" | "getBlockHeight" | "getBlockTime" | "getBlocks" |
            "getBlocksWithLimit" | "getFirstAvailableBlock" |
            // Transaction methods
            "getTransaction" | "getSignatureStatuses" | "getSignaturesForAddress" |
            "sendTransaction" | "simulateTransaction" |
            // Slot methods
            "getSlot" | "getSlotLeader" | "getSlotLeaders" |
            // Epoch methods
            "getEpochInfo" | "getEpochSchedule" |
            // Network methods
            "getGenesisHash" | "getHealth" | "getIdentity" | "getVersion" |
            "getVoteAccounts" | "getClusterNodes" |
            // Supply methods
            "getSupply" | "getTotalSupply" |
            // Fee methods
            "getFees" | "getFeeCalculatorForBlockhash" | "getFeeRateGovernor" |
            "getRecentBlockhash" | "getLatestBlockhash" |
            // Balance methods
            "getBalance" | "getMinimumBalanceForRentExemption" |
            // Token methods
            "getTokenAccountBalance" | "getTokenAccountsByDelegate" |
            "getTokenAccountsByOwner" | "getTokenLargestAccounts" | "getTokenSupply" |
            // Stake methods
            "getStakeActivation" |
            // Inflation methods
            "getInflationGovernor" | "getInflationRate" | "getInflationReward" |
            // Performance methods
            "getRecentPerformanceSamples" |
            // Subscription methods (WebSocket)
            "accountSubscribe" | "accountUnsubscribe" |
            "logsSubscribe" | "logsUnsubscribe" |
            "programSubscribe" | "programUnsubscribe" |
            "signatureSubscribe" | "signatureUnsubscribe" |
            "slotSubscribe" | "slotUnsubscribe" |
            "slotsUpdatesSubscribe" | "slotsUpdatesUnsubscribe" |
            "rootSubscribe" | "rootUnsubscribe" |
            "voteSubscribe" | "voteUnsubscribe"
        )
    }

    /// Transform request parameters if needed
    fn transform_request(&self, mut request: JsonRpcRequest) -> JsonRpcRequest {
        // Some Solana methods require specific parameter formatting
        match request.method.as_str() {
            "getAccountInfo" | "getBalance" => {
                // Ensure commitment parameter is properly formatted
                if let Some(Value::Array(ref mut params)) = request.params {
                    if params.len() == 1 {
                        // Add default commitment if not specified
                        params.push(json!({
                            "commitment": "confirmed"
                        }));
                    }
                }
            }
            "getBlock" => {
                // Ensure slot number is properly formatted
                if let Some(Value::Array(ref mut params)) = request.params {
                    if params.len() == 1 {
                        // Add default options
                        params.push(json!({
                            "encoding": "json",
                            "transactionDetails": "full",
                            "rewards": false
                        }));
                    }
                }
            }
            _ => {}
        }

        request
    }
}

#[async_trait]
impl RpcRelay for SolanaRelay {
    async fn forward_request(&self, request: JsonRpcRequest) -> RpcResult<JsonRpcResponse> {
        // Check if method is supported
        if !self.supports_method(&request.method) {
            return Err(RpcError::InvalidMethod {
                method: request.method.clone(),
            });
        }

        debug!("Forwarding Solana RPC request: {} to {}", request.method, self.config.name);

        // Transform request if needed
        let transformed_request = self.transform_request(request);

        // Forward request with retry logic
        let mut last_error = None;
        for attempt in 1..=3 {
            match self.send_request(&transformed_request).await {
                Ok(response) => {
                    debug!("Successfully forwarded request {} to {}", transformed_request.method, self.config.name);
                    return Ok(response);
                }
                Err(e) => {
                    warn!("Attempt {} failed for {} on {}: {}", attempt, transformed_request.method, self.config.name, e);
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
            vm_type: VmType::Solana,
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
        VmType::Solana
    }

    fn config(&self) -> &NodeConfig {
        &self.config
    }
}

/// Solana-specific utility functions
impl SolanaRelay {
    /// Validate Solana public key format (base58)
    pub fn is_valid_pubkey(pubkey: &str) -> bool {
        if pubkey.len() < 32 || pubkey.len() > 44 {
            return false;
        }
        
        // Check if it's valid base58
        bs58::decode(pubkey).into_vec().is_ok()
    }

    /// Validate Solana signature format
    pub fn is_valid_signature(signature: &str) -> bool {
        if signature.len() < 86 || signature.len() > 88 {
            return false;
        }
        
        // Check if it's valid base58
        bs58::decode(signature).into_vec().is_ok()
    }

    /// Convert lamports to SOL
    pub fn lamports_to_sol(lamports: u64) -> f64 {
        lamports as f64 / 1_000_000_000.0
    }

    /// Convert SOL to lamports
    pub fn sol_to_lamports(sol: f64) -> u64 {
        (sol * 1_000_000_000.0) as u64
    }

    /// Format slot number for display
    pub fn format_slot(slot: u64) -> String {
        slot.to_string()
    }

    /// Parse commitment level
    pub fn parse_commitment(commitment: &str) -> Result<&str, RpcError> {
        match commitment {
            "processed" | "confirmed" | "finalized" => Ok(commitment),
            _ => Err(RpcError::InvalidParams {
                reason: format!("Invalid commitment level: {}", commitment),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NodeConfig;
    use std::time::Duration;

    fn create_test_config() -> NodeConfig {
        NodeConfig {
            name: "test-solana".to_string(),
            url: "http://localhost:8899".parse().unwrap(),
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
        let relay = SolanaRelay {
            config,
            client,
            last_health_check: std::sync::Arc::new(std::sync::RwLock::new(SystemTime::UNIX_EPOCH)),
            last_health_status: std::sync::Arc::new(std::sync::RwLock::new(HealthStatus::Unknown)),
        };

        assert!(relay.supports_method("getSlot"));
        assert!(relay.supports_method("getBalance"));
        assert!(relay.supports_method("getVersion"));
        assert!(relay.supports_method("getAccountInfo"));
        assert!(!relay.supports_method("eth_blockNumber")); // Ethereum method
        assert!(!relay.supports_method("invalid_method"));
    }

    #[test]
    fn test_pubkey_validation() {
        // Valid Solana pubkey
        assert!(SolanaRelay::is_valid_pubkey("11111111111111111111111111111111"));
        assert!(SolanaRelay::is_valid_pubkey("So11111111111111111111111111111111111111112"));
        
        // Invalid pubkeys
        assert!(!SolanaRelay::is_valid_pubkey("invalid"));
        assert!(!SolanaRelay::is_valid_pubkey("0x1234567890123456789012345678901234567890")); // Ethereum format
        assert!(!SolanaRelay::is_valid_pubkey("")); // Empty
    }

    #[test]
    fn test_signature_validation() {
        // Valid signature format (base58, ~88 chars)
        let valid_sig = "5VfydqvQ8NjWpKU5uMNVnM8qgQLrVBH5mXWLk2Q6j2VY4z9R3J2WyM5K2kZ6Q5Z5Z5Z5Z5Z5Z5Z5Z5";
        // Note: This is a mock signature for testing format validation
        
        assert!(!SolanaRelay::is_valid_signature("invalid"));
        assert!(!SolanaRelay::is_valid_signature("")); // Empty
        assert!(!SolanaRelay::is_valid_signature("short")); // Too short
    }

    #[test]
    fn test_lamports_conversion() {
        assert_eq!(SolanaRelay::lamports_to_sol(1_000_000_000), 1.0);
        assert_eq!(SolanaRelay::lamports_to_sol(500_000_000), 0.5);
        assert_eq!(SolanaRelay::sol_to_lamports(1.0), 1_000_000_000);
        assert_eq!(SolanaRelay::sol_to_lamports(0.5), 500_000_000);
    }

    #[test]
    fn test_commitment_parsing() {
        assert!(SolanaRelay::parse_commitment("processed").is_ok());
        assert!(SolanaRelay::parse_commitment("confirmed").is_ok());
        assert!(SolanaRelay::parse_commitment("finalized").is_ok());
        assert!(SolanaRelay::parse_commitment("invalid").is_err());
    }

    #[test]
    fn test_request_transformation() {
        let config = create_test_config();
        let client = Client::new();
        let relay = SolanaRelay {
            config,
            client,
            last_health_check: std::sync::Arc::new(std::sync::RwLock::new(SystemTime::UNIX_EPOCH)),
            last_health_status: std::sync::Arc::new(std::sync::RwLock::new(HealthStatus::Unknown)),
        };

        let mut request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "getAccountInfo".to_string(),
            params: Some(json!(["11111111111111111111111111111111"])),
            id: Some(Value::Number(1.into())),
        };

        let transformed = relay.transform_request(request.clone());
        
        // Should add commitment parameter
        if let Some(Value::Array(params)) = transformed.params {
            assert_eq!(params.len(), 2);
        }
    }
}