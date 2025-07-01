//! Gateway Module - Unified VM Interface
//!
//! This module provides a clean, unified interface for interacting with different
//! virtual machines (EVM, SVM) through a single consistent API.

pub mod unified;

// Re-export unified types as the primary interface
pub use unified::{
    GasInfo, GatewayResponse, GatewayStats, ResponseMetadata, UnifiedAccount, UnifiedBlock,
    UnifiedGateway, UnifiedGatewayConfig, UnifiedTransaction,
};

use crate::cache::CacheLayer;
use crate::error::ApplicationResult;
use multivm_common::VmType;
use std::sync::Arc;

/// Gateway factory for creating VM-specific gateways
pub struct GatewayFactory;

impl GatewayFactory {
    /// Create a new EVM gateway
    pub async fn create_evm_gateway(
        rpc_url: String,
        cache: Arc<CacheLayer>,
    ) -> ApplicationResult<UnifiedGateway> {
        let gateway_config = UnifiedGatewayConfig {
            vm_type: VmType::Evm,
            rpc_url,
            backup_urls: vec![],
            timeout: std::time::Duration::from_secs(30),
            max_retries: 3,
            chain_id: 1, // Default EVM chain ID
            production_mode: true,
        };

        UnifiedGateway::new(gateway_config, cache).await
    }

    /// Create a new SVM gateway
    pub async fn create_svm_gateway(
        rpc_url: String,
        cache: Arc<CacheLayer>,
    ) -> ApplicationResult<UnifiedGateway> {
        let gateway_config = UnifiedGatewayConfig {
            vm_type: VmType::Svm,
            rpc_url,
            backup_urls: vec![],
            timeout: std::time::Duration::from_secs(30),
            max_retries: 3,
            chain_id: 0, // Solana doesn't use chain_id
            production_mode: true,
        };

        UnifiedGateway::new(gateway_config, cache).await
    }

    /// Create a unified gateway that can handle multiple VM types
    pub async fn create_unified_gateway(
        cache: Arc<CacheLayer>,
    ) -> ApplicationResult<UnifiedGateway> {
        let gateway_config = UnifiedGatewayConfig {
            vm_type: VmType::Evm, // Default to EVM
            rpc_url: "http://localhost:8545".to_string(),
            backup_urls: vec![],
            timeout: std::time::Duration::from_secs(30),
            max_retries: 3,
            chain_id: 31337, // Local development chain
            production_mode: false,
        };

        UnifiedGateway::new(gateway_config, cache).await
    }
}

/// Common gateway traits for extensibility
#[async_trait::async_trait]
pub trait VmGateway: Send + Sync {
    /// Get the VM type this gateway handles
    fn vm_type(&self) -> VmType;

    /// Check if the backend is healthy
    async fn health_check(&self) -> ApplicationResult<bool>;

    /// Get gateway statistics
    async fn get_stats(&self) -> ApplicationResult<GatewayStats>;

    /// Get latest block
    async fn get_latest_block(&self) -> ApplicationResult<GatewayResponse<UnifiedBlock>>;

    /// Send raw transaction
    async fn send_raw_transaction(
        &self,
        raw_tx: &str,
    ) -> ApplicationResult<GatewayResponse<String>>;
}

#[async_trait::async_trait]
impl VmGateway for UnifiedGateway {
    fn vm_type(&self) -> VmType {
        self.config.vm_type
    }

    async fn health_check(&self) -> ApplicationResult<bool> {
        self.health_check().await
    }

    async fn get_stats(&self) -> ApplicationResult<GatewayStats> {
        self.get_stats().await
    }

    async fn get_latest_block(&self) -> ApplicationResult<GatewayResponse<UnifiedBlock>> {
        self.get_latest_block().await
    }

    async fn send_raw_transaction(
        &self,
        raw_tx: &str,
    ) -> ApplicationResult<GatewayResponse<String>> {
        self.send_raw_transaction(raw_tx).await
    }
}

/// Utility functions for gateway operations
pub mod utils {
    use super::*;

    /// Create a development gateway with mock endpoints
    pub async fn create_dev_gateway(
        vm_type: VmType,
        cache: Arc<CacheLayer>,
    ) -> ApplicationResult<UnifiedGateway> {
        let (rpc_url, chain_id) = match vm_type {
            VmType::Evm => ("http://localhost:8545".to_string(), 31337),
            VmType::Svm => ("http://localhost:8899".to_string(), 0),
        };

        let gateway_config = UnifiedGatewayConfig {
            vm_type,
            rpc_url,
            backup_urls: vec![],
            timeout: std::time::Duration::from_secs(10), // Shorter timeout for dev
            max_retries: 1,
            chain_id,
            production_mode: false,
        };

        UnifiedGateway::new(gateway_config, cache).await
    }

    /// Create a production gateway with proper configuration
    pub async fn create_prod_gateway(
        vm_type: VmType,
        rpc_url: String,
        backup_urls: Vec<String>,
        cache: Arc<CacheLayer>,
    ) -> ApplicationResult<UnifiedGateway> {
        let chain_id = match vm_type {
            VmType::Evm => 1, // Ethereum mainnet
            VmType::Svm => 0, // Solana doesn't use chain_id
        };

        let gateway_config = UnifiedGatewayConfig {
            vm_type,
            rpc_url,
            backup_urls,
            timeout: std::time::Duration::from_secs(30),
            max_retries: 3,
            chain_id,
            production_mode: true,
        };

        UnifiedGateway::new(gateway_config, cache).await
    }

    /// Validate an address for a specific VM type
    pub fn validate_address(vm_type: VmType, address: &str) -> bool {
        match vm_type {
            VmType::Evm => address.starts_with("0x") && address.len() == 42,
            VmType::Svm => address.len() >= 32 && address.len() <= 44,
        }
    }

    /// Validate a transaction hash for a specific VM type
    pub fn validate_tx_hash(vm_type: VmType, tx_hash: &str) -> bool {
        match vm_type {
            VmType::Evm => tx_hash.starts_with("0x") && tx_hash.len() == 66,
            VmType::Svm => tx_hash.len() >= 32 && tx_hash.len() <= 88,
        }
    }

    /// Get the default RPC port for a VM type
    pub fn get_default_rpc_port(vm_type: VmType) -> u16 {
        match vm_type {
            VmType::Evm => 8545,
            VmType::Svm => 8899,
        }
    }

    /// Format a balance for display based on VM type
    pub fn format_balance(vm_type: VmType, balance: &str) -> String {
        match vm_type {
            VmType::Evm => {
                // Convert wei to ETH
                if let Ok(wei) = balance.parse::<u128>() {
                    let eth = wei as f64 / 1e18;
                    format!("{eth:.6} ETH")
                } else {
                    balance.to_string()
                }
            }
            VmType::Svm => {
                // Convert lamports to SOL
                if let Ok(lamports) = balance.parse::<u64>() {
                    let sol = lamports as f64 / 1e9;
                    format!("{sol:.6} SOL")
                } else {
                    balance.to_string()
                }
            }
        }
    }
}

// Type aliases for backward compatibility and convenience
pub type EvmGateway = UnifiedGateway;
pub type SvmGateway = UnifiedGateway;
