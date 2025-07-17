//! RPC relay implementations for different blockchain nodes

pub mod ethereum;
pub mod solana;
pub mod load_balancer;
pub mod health_checker;

use crate::{
    config::NodeConfig,
    error::{RpcError, RpcResult},
    types::{JsonRpcRequest, JsonRpcResponse, NodeHealth, VmType},
};
use async_trait::async_trait;
use std::sync::Arc;

/// Generic RPC relay trait
#[async_trait]
pub trait RpcRelay: Send + Sync {
    /// Forward RPC request to backend node
    async fn forward_request(&self, request: JsonRpcRequest) -> RpcResult<JsonRpcResponse>;
    
    /// Get node health status
    async fn get_health(&self) -> NodeHealth;
    
    /// Get VM type this relay handles
    fn vm_type(&self) -> VmType;
    
    /// Get relay configuration
    fn config(&self) -> &NodeConfig;
}

/// RPC relay manager that handles multiple nodes per VM type
pub struct RelayManager {
    ethereum_relays: Vec<Arc<dyn RpcRelay>>,
    solana_relays: Vec<Arc<dyn RpcRelay>>,
    load_balancer: Arc<load_balancer::LoadBalancer>,
    health_checker: Arc<health_checker::HealthChecker>,
}

impl RelayManager {
    pub fn new() -> Self {
        Self {
            ethereum_relays: Vec::new(),
            solana_relays: Vec::new(),
            load_balancer: Arc::new(load_balancer::LoadBalancer::new()),
            health_checker: Arc::new(health_checker::HealthChecker::new()),
        }
    }

    /// Add Ethereum relay
    pub fn add_ethereum_relay(&mut self, relay: Arc<dyn RpcRelay>) {
        self.ethereum_relays.push(relay);
    }

    /// Add Solana relay
    pub fn add_solana_relay(&mut self, relay: Arc<dyn RpcRelay>) {
        self.solana_relays.push(relay);
    }

    /// Forward request to appropriate relay
    pub async fn forward_request(
        &self, 
        request: JsonRpcRequest, 
        vm_type: VmType
    ) -> RpcResult<JsonRpcResponse> {
        let relays = match vm_type {
            VmType::Ethereum => &self.ethereum_relays,
            VmType::Solana => &self.solana_relays,
            VmType::MultiVm => {
                return Err(RpcError::InvalidMethod {
                    method: request.method.clone(),
                });
            }
        };

        if relays.is_empty() {
            return Err(RpcError::BackendUnavailable {
                backend: vm_type.to_string(),
            });
        }

        // Select relay using load balancer
        let relay = self.load_balancer.select_relay(relays, &request).await?;
        relay.forward_request(request).await
    }

    /// Get health status for all relays
    pub async fn get_all_health(&self) -> Vec<NodeHealth> {
        let mut health_statuses = Vec::new();

        // Check Ethereum relays
        for relay in &self.ethereum_relays {
            health_statuses.push(relay.get_health().await);
        }

        // Check Solana relays
        for relay in &self.solana_relays {
            health_statuses.push(relay.get_health().await);
        }

        health_statuses
    }

    /// Start health checking background task
    pub async fn start_health_checking(&self) {
        let all_relays: Vec<Arc<dyn RpcRelay>> = self.ethereum_relays
            .iter()
            .chain(self.solana_relays.iter())
            .cloned()
            .collect();

        self.health_checker.start_monitoring(all_relays).await;
    }

    /// Get healthy relays by VM type
    pub async fn get_healthy_relays(&self, vm_type: VmType) -> Vec<Arc<dyn RpcRelay>> {
        let all_relays = match vm_type {
            VmType::Ethereum => &self.ethereum_relays,
            VmType::Solana => &self.solana_relays,
            VmType::MultiVm => return Vec::new(),
        };

        let mut healthy_relays = Vec::new();
        for relay in all_relays {
            let health = relay.get_health().await;
            if matches!(health.status, crate::types::HealthStatus::Healthy) {
                healthy_relays.push(relay.clone());
            }
        }

        healthy_relays
    }
}

/// Utility functions for relay management
pub mod utils {
    use super::*;
    use crate::config::BackendConfig;

    /// Create relay manager from configuration
    pub async fn create_relay_manager(config: &BackendConfig) -> RpcResult<RelayManager> {
        let mut manager = RelayManager::new();

        // Create Ethereum relays
        for node_config in &config.ethereum {
            let relay = ethereum::EthereumRelay::new(node_config.clone()).await?;
            manager.add_ethereum_relay(Arc::new(relay));
        }

        // Create Solana relays
        for node_config in &config.solana {
            let relay = solana::SolanaRelay::new(node_config.clone()).await?;
            manager.add_solana_relay(Arc::new(relay));
        }

        // Start health checking
        manager.start_health_checking().await;

        Ok(manager)
    }

    /// Test connection to a node
    pub async fn test_node_connection(config: &NodeConfig) -> RpcResult<bool> {
        let test_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: match config.url.path() {
                path if path.contains("eth") || config.url.port() == Some(8545) => "web3_clientVersion".to_string(),
                _ => "getVersion".to_string(),
            },
            params: None,
            id: Some(serde_json::Value::Number(1.into())),
        };

        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| RpcError::NetworkError {
                message: e.to_string(),
            })?;

        let response = client
            .post(config.url.clone())
            .json(&test_request)
            .send()
            .await
            .map_err(|e| RpcError::NetworkError {
                message: e.to_string(),
            })?;

        Ok(response.status().is_success())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NodeConfig;
    use std::time::Duration;
    use url::Url;

    fn create_test_node_config() -> NodeConfig {
        NodeConfig {
            name: "test-node".to_string(),
            url: "http://localhost:8545".parse().unwrap(),
            timeout: Duration::from_secs(5),
            max_concurrent_requests: 10,
            health_check_interval: Duration::from_secs(30),
            priority: 100,
            auth: None,
        }
    }

    #[tokio::test]
    async fn test_relay_manager_creation() {
        let manager = RelayManager::new();
        assert!(manager.ethereum_relays.is_empty());
        assert!(manager.solana_relays.is_empty());
    }

    #[tokio::test]
    async fn test_empty_relay_handling() {
        let manager = RelayManager::new();
        
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "eth_blockNumber".to_string(),
            params: None,
            id: Some(serde_json::Value::Number(1.into())),
        };

        let result = manager.forward_request(request, VmType::Ethereum).await;
        assert!(result.is_err());
        
        match result.unwrap_err() {
            RpcError::BackendUnavailable { backend } => {
                assert_eq!(backend, "ethereum");
            }
            _ => panic!("Expected BackendUnavailable error"),
        }
    }
}