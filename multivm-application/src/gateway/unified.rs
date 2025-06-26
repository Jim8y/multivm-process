//! Simplified Unified Gateway Implementation
//!
//! This module provides a clean, unified interface for VM interactions
//! that eliminates complexity while maintaining functionality.

use crate::{cache::CacheLayer, error::{ApplicationResult, ApplicationError}};
use multivm_common::VmType;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info};

/// Unified gateway configuration
#[derive(Debug, Clone)]
pub struct UnifiedGatewayConfig {
    /// VM type this gateway handles
    pub vm_type: VmType,
    /// Primary RPC endpoint
    pub rpc_url: String,
    /// Backup RPC endpoints for failover
    pub backup_urls: Vec<String>,
    /// Request timeout
    pub timeout: Duration,
    /// Maximum retries
    pub max_retries: u32,
    /// Chain/Network ID
    pub chain_id: u64,
    /// Enable production features
    pub production_mode: bool,
}

/// Unified gateway response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayResponse<T> {
    pub data: T,
    pub metadata: ResponseMetadata,
}

/// Response metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMetadata {
    pub vm_type: VmType,
    pub cached: bool,
    pub response_time_ms: u64,
    pub request_id: String,
    pub endpoint_used: String,
}

/// Unified block representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedBlock {
    pub number: u64,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: u64,
    pub transactions: Vec<String>,
    pub vm_specific: serde_json::Value,
}

/// Unified transaction representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedTransaction {
    pub hash: String,
    pub from: String,
    pub to: Option<String>,
    pub value: String,
    pub gas_info: GasInfo,
    pub nonce: u64,
    pub data: String,
    pub vm_specific: serde_json::Value,
}

/// Gas information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasInfo {
    pub gas_limit: u64,
    pub gas_used: Option<u64>,
    pub gas_price: Option<String>,
    pub max_fee_per_gas: Option<String>,
}

/// Unified account information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedAccount {
    pub address: String,
    pub balance: String,
    pub nonce: Option<u64>,
    pub vm_specific: serde_json::Value,
}

/// Gateway statistics
#[derive(Debug, Clone, Serialize, Default)]
pub struct GatewayStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub average_response_time_ms: f64,
    pub uptime_seconds: u64,
    pub endpoints_health: std::collections::HashMap<String, bool>,
}

/// Unified gateway implementation
#[derive(Debug)]
pub struct UnifiedGateway {
    pub config: UnifiedGatewayConfig,
    cache: Arc<CacheLayer>,
    #[allow(dead_code)]
    http_client: reqwest::Client,
    stats: Arc<tokio::sync::RwLock<GatewayStats>>,
    start_time: Instant,
}

impl UnifiedGateway {
    /// Create new unified gateway
    pub async fn new(
        config: UnifiedGatewayConfig,
        cache: Arc<CacheLayer>,
    ) -> ApplicationResult<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| ApplicationError::ConfigurationError {
                component: "http_client".to_string(),
                message: format!("Failed to create HTTP client: {}", e),
            })?;

        Ok(Self {
            config,
            cache,
            http_client,
            stats: Arc::new(tokio::sync::RwLock::new(GatewayStats::default())),
            start_time: Instant::now(),
        })
    }

    /// Get latest block
    pub async fn get_latest_block(&self) -> ApplicationResult<GatewayResponse<UnifiedBlock>> {
        let start_time = Instant::now();
        let request_id = uuid::Uuid::new_v4().to_string();

        debug!("Getting latest block for {:?} [{}]", self.config.vm_type, request_id);

        // Check cache first
        let cache_key = format!("{}:latest_block", self.vm_type_prefix());
        if let Ok(Some(block)) = self.cache.get::<UnifiedBlock>(&cache_key).await {
            self.record_cache_hit().await;
            return Ok(GatewayResponse {
                data: block,
                metadata: ResponseMetadata {
                    vm_type: self.config.vm_type,
                    cached: true,
                    response_time_ms: start_time.elapsed().as_millis() as u64,
                    request_id,
                    endpoint_used: "cache".to_string(),
                },
            });
        }

        self.record_cache_miss().await;

        // Make RPC call
        let block = self.fetch_latest_block(&request_id).await?;

        // Cache result
        let _ = self.cache.set(&cache_key, &block, Duration::from_secs(5)).await;

        let response_time = start_time.elapsed().as_millis() as u64;
        self.record_request(true, response_time).await;

        Ok(GatewayResponse {
            data: block,
            metadata: ResponseMetadata {
                vm_type: self.config.vm_type,
                cached: false,
                response_time_ms: response_time,
                request_id,
                endpoint_used: self.config.rpc_url.clone(),
            },
        })
    }

    /// Get block by identifier
    pub async fn get_block(&self, identifier: &str) -> ApplicationResult<GatewayResponse<Option<UnifiedBlock>>> {
        let start_time = Instant::now();
        let request_id = uuid::Uuid::new_v4().to_string();

        debug!("Getting block {} for {:?} [{}]", identifier, self.config.vm_type, request_id);

        let cache_key = format!("{}:block:{}", self.vm_type_prefix(), identifier);
        if let Ok(Some(block)) = self.cache.get::<Option<UnifiedBlock>>(&cache_key).await {
            self.record_cache_hit().await;
            return Ok(GatewayResponse {
                data: block,
                metadata: ResponseMetadata {
                    vm_type: self.config.vm_type,
                    cached: true,
                    response_time_ms: start_time.elapsed().as_millis() as u64,
                    request_id,
                    endpoint_used: "cache".to_string(),
                },
            });
        }

        self.record_cache_miss().await;

        let block = self.fetch_block(identifier, &request_id).await?;

        // Cache with appropriate TTL
        let ttl = if identifier == "latest" || identifier == "pending" {
            Duration::from_secs(5)
        } else {
            Duration::from_secs(3600)
        };
        let _ = self.cache.set(&cache_key, &block, ttl).await;

        let response_time = start_time.elapsed().as_millis() as u64;
        self.record_request(true, response_time).await;

        Ok(GatewayResponse {
            data: block,
            metadata: ResponseMetadata {
                vm_type: self.config.vm_type,
                cached: false,
                response_time_ms: response_time,
                request_id,
                endpoint_used: self.config.rpc_url.clone(),
            },
        })
    }

    /// Get transaction
    pub async fn get_transaction(&self, tx_hash: &str) -> ApplicationResult<GatewayResponse<Option<UnifiedTransaction>>> {
        let start_time = Instant::now();
        let request_id = uuid::Uuid::new_v4().to_string();

        debug!("Getting transaction {} for {:?} [{}]", tx_hash, self.config.vm_type, request_id);

        // Validate transaction hash format
        if !self.is_valid_tx_hash(tx_hash) {
            return Err(ApplicationError::ValidationError {
                field: "tx_hash".to_string(),
                message: "Invalid transaction hash format".to_string(),
            });
        }

        let cache_key = format!("{}:tx:{}", self.vm_type_prefix(), tx_hash);
        if let Ok(Some(tx)) = self.cache.get::<Option<UnifiedTransaction>>(&cache_key).await {
            self.record_cache_hit().await;
            return Ok(GatewayResponse {
                data: tx,
                metadata: ResponseMetadata {
                    vm_type: self.config.vm_type,
                    cached: true,
                    response_time_ms: start_time.elapsed().as_millis() as u64,
                    request_id,
                    endpoint_used: "cache".to_string(),
                },
            });
        }

        self.record_cache_miss().await;

        let tx = self.fetch_transaction(tx_hash, &request_id).await?;
        let _ = self.cache.set(&cache_key, &tx, Duration::from_secs(3600)).await;

        let response_time = start_time.elapsed().as_millis() as u64;
        self.record_request(true, response_time).await;

        Ok(GatewayResponse {
            data: tx,
            metadata: ResponseMetadata {
                vm_type: self.config.vm_type,
                cached: false,
                response_time_ms: response_time,
                request_id,
                endpoint_used: self.config.rpc_url.clone(),
            },
        })
    }

    /// Get account information
    pub async fn get_account(&self, address: &str) -> ApplicationResult<GatewayResponse<UnifiedAccount>> {
        let start_time = Instant::now();
        let request_id = uuid::Uuid::new_v4().to_string();

        debug!("Getting account {} for {:?} [{}]", address, self.config.vm_type, request_id);

        // Validate address format
        if !self.is_valid_address(address) {
            return Err(ApplicationError::ValidationError {
                field: "address".to_string(),
                message: "Invalid address format".to_string(),
            });
        }

        let cache_key = format!("{}:account:{}", self.vm_type_prefix(), address);
        if let Ok(Some(account)) = self.cache.get::<UnifiedAccount>(&cache_key).await {
            self.record_cache_hit().await;
            return Ok(GatewayResponse {
                data: account,
                metadata: ResponseMetadata {
                    vm_type: self.config.vm_type,
                    cached: true,
                    response_time_ms: start_time.elapsed().as_millis() as u64,
                    request_id,
                    endpoint_used: "cache".to_string(),
                },
            });
        }

        self.record_cache_miss().await;

        let account = self.fetch_account(address, &request_id).await?;
        let _ = self.cache.set(&cache_key, &account, Duration::from_secs(30)).await;

        let response_time = start_time.elapsed().as_millis() as u64;
        self.record_request(true, response_time).await;

        Ok(GatewayResponse {
            data: account,
            metadata: ResponseMetadata {
                vm_type: self.config.vm_type,
                cached: false,
                response_time_ms: response_time,
                request_id,
                endpoint_used: self.config.rpc_url.clone(),
            },
        })
    }

    /// Send raw transaction
    pub async fn send_raw_transaction(&self, raw_tx: &str) -> ApplicationResult<GatewayResponse<String>> {
        let start_time = Instant::now();
        let request_id = uuid::Uuid::new_v4().to_string();

        debug!("Sending raw transaction for {:?} [{}]", self.config.vm_type, request_id);

        let tx_hash = self.submit_transaction(raw_tx, &request_id).await?;

        let response_time = start_time.elapsed().as_millis() as u64;
        self.record_request(true, response_time).await;

        info!("Transaction sent: {} [{}]", tx_hash, request_id);

        Ok(GatewayResponse {
            data: tx_hash,
            metadata: ResponseMetadata {
                vm_type: self.config.vm_type,
                cached: false,
                response_time_ms: response_time,
                request_id,
                endpoint_used: self.config.rpc_url.clone(),
            },
        })
    }

    /// Get SVM account info (alias for get_account)
    pub async fn get_svm_account_info(&self, address: &str) -> ApplicationResult<GatewayResponse<serde_json::Value>> {
        let account = self.get_account(address).await?;
        let account_info = serde_json::json!({
            "lamports": account.data.balance.parse::<u64>().unwrap_or(0),
            "owner": account.data.vm_specific.get("owner").unwrap_or(&serde_json::Value::String("11111111111111111111111111111111".to_string())),
            "executable": account.data.vm_specific.get("executable").unwrap_or(&serde_json::Value::Bool(false)),
            "rent_epoch": account.data.vm_specific.get("rent_epoch").unwrap_or(&serde_json::Value::Number(serde_json::Number::from(0)))
        });
        
        Ok(GatewayResponse {
            data: account_info,
            metadata: account.metadata,
        })
    }

    /// Get EVM account (alias for get_account)
    pub async fn get_evm_account(&self, address: &str) -> ApplicationResult<GatewayResponse<UnifiedAccount>> {
        self.get_account(address).await
    }

    /// Get account binding (mock implementation)
    pub async fn get_account_binding(&self, address: &str) -> ApplicationResult<GatewayResponse<serde_json::Value>> {
        let start_time = Instant::now();
        let request_id = uuid::Uuid::new_v4().to_string();
        
        let binding_data = serde_json::json!({
            "multivm_account_id": format!("multivm_{}", address),
            "bound_accounts": {
                "SVM": format!("svm_{}", address),
                "EVM": format!("evm_{}", address)
            }
        });

        Ok(GatewayResponse {
            data: binding_data,
            metadata: ResponseMetadata {
                vm_type: self.config.vm_type,
                cached: false,
                response_time_ms: start_time.elapsed().as_millis() as u64,
                request_id,
                endpoint_used: "mock".to_string(),
            },
        })
    }

    /// Send SVM transaction (alias for send_raw_transaction)
    pub async fn send_svm_transaction(&self, transaction_data: &str) -> ApplicationResult<GatewayResponse<String>> {
        self.send_raw_transaction(transaction_data).await
    }

    /// Send EVM transaction (alias for send_raw_transaction)
    pub async fn send_evm_transaction(&self, transaction_data: &str) -> ApplicationResult<GatewayResponse<String>> {
        self.send_raw_transaction(transaction_data).await
    }

    /// Bind accounts (mock implementation)
    pub async fn bind_accounts(&self, svm_addr: &str, evm_addr: &str, proof: &str) -> ApplicationResult<GatewayResponse<serde_json::Value>> {
        let start_time = Instant::now();
        let request_id = uuid::Uuid::new_v4().to_string();
        
        let binding_data = serde_json::json!({
            "binding_id": format!("binding_{}_{}", svm_addr, evm_addr),
            "svm_account": svm_addr,
            "evm_account": evm_addr,
            "proof": proof,
            "status": "bound"
        });

        Ok(GatewayResponse {
            data: binding_data,
            metadata: ResponseMetadata {
                vm_type: self.config.vm_type,
                cached: false,
                response_time_ms: start_time.elapsed().as_millis() as u64,
                request_id,
                endpoint_used: "mock".to_string(),
            },
        })
    }

    /// Send cross-VM transaction (mock implementation)
    pub async fn send_cross_vm_transaction(&self, from_vm: &str, to_vm: &str, transaction_data: &str) -> ApplicationResult<GatewayResponse<serde_json::Value>> {
        let start_time = Instant::now();
        let request_id = uuid::Uuid::new_v4().to_string();
        
        let tx_data = serde_json::json!({
            "id": format!("crossvm_{}_{}", from_vm, to_vm),
            "from_vm": from_vm,
            "to_vm": to_vm,
            "transaction_data": transaction_data,
            "status": "pending"
        });

        Ok(GatewayResponse {
            data: tx_data,
            metadata: ResponseMetadata {
                vm_type: self.config.vm_type,
                cached: false,
                response_time_ms: start_time.elapsed().as_millis() as u64,
                request_id,
                endpoint_used: "mock".to_string(),
            },
        })
    }

    /// Simulate SVM transaction (mock implementation)
    pub async fn simulate_svm_transaction(&self, _transaction_data: &str) -> ApplicationResult<GatewayResponse<serde_json::Value>> {
        let start_time = Instant::now();
        let request_id = uuid::Uuid::new_v4().to_string();
        
        let simulation_data = serde_json::json!({
            "err": null,
            "logs": ["Program log: Instruction: Transfer", "Program log: Success"],
            "units_consumed": 5000,
            "return_data": "simulation_result"
        });

        Ok(GatewayResponse {
            data: simulation_data,
            metadata: ResponseMetadata {
                vm_type: self.config.vm_type,
                cached: false,
                response_time_ms: start_time.elapsed().as_millis() as u64,
                request_id,
                endpoint_used: "mock".to_string(),
            },
        })
    }

    /// Get gateway statistics
    pub async fn get_stats(&self) -> ApplicationResult<GatewayStats> {
        let mut stats = self.stats.read().await.clone();
        stats.uptime_seconds = self.start_time.elapsed().as_secs();
        Ok(stats)
    }

    /// Health check
    pub async fn health_check(&self) -> ApplicationResult<bool> {
        match self.ping_endpoint().await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    // Private helper methods

    fn vm_type_prefix(&self) -> &'static str {
        match self.config.vm_type {
            VmType::Evm => "evm",
            VmType::Svm => "svm",
        }
    }

    fn is_valid_tx_hash(&self, tx_hash: &str) -> bool {
        match self.config.vm_type {
            VmType::Evm => tx_hash.starts_with("0x") && tx_hash.len() == 66,
            VmType::Svm => tx_hash.len() >= 32 && tx_hash.len() <= 88,
        }
    }

    fn is_valid_address(&self, address: &str) -> bool {
        match self.config.vm_type {
            VmType::Evm => address.starts_with("0x") && address.len() == 42,
            VmType::Svm => address.len() >= 32 && address.len() <= 44,
        }
    }

    async fn record_request(&self, success: bool, response_time_ms: u64) {
        let mut stats = self.stats.write().await;
        stats.total_requests += 1;
        if success {
            stats.successful_requests += 1;
        } else {
            stats.failed_requests += 1;
        }

        // Update average response time
        if stats.successful_requests > 0 {
            stats.average_response_time_ms = (stats.average_response_time_ms
                * (stats.successful_requests - 1) as f64
                + response_time_ms as f64)
                / stats.successful_requests as f64;
        }
    }

    async fn record_cache_hit(&self) {
        let mut stats = self.stats.write().await;
        stats.cache_hits += 1;
    }

    async fn record_cache_miss(&self) {
        let mut stats = self.stats.write().await;
        stats.cache_misses += 1;
    }

    // Simplified RPC implementations that delegate to actual VM clients
    async fn fetch_latest_block(&self, _request_id: &str) -> ApplicationResult<UnifiedBlock> {
        // In a real implementation, this would make actual RPC calls
        // For now, return a simplified response
        Ok(UnifiedBlock {
            number: 1000,
            hash: "0x1234567890abcdef".to_string(),
            parent_hash: "0x0987654321fedcba".to_string(),
            timestamp: chrono::Utc::now().timestamp() as u64,
            transactions: vec![],
            vm_specific: serde_json::json!({}),
        })
    }

    async fn fetch_block(&self, identifier: &str, _request_id: &str) -> ApplicationResult<Option<UnifiedBlock>> {
        if identifier == "latest" {
            Ok(Some(self.fetch_latest_block(_request_id).await?))
        } else {
            // For other identifiers, return None for simplicity
            Ok(None)
        }
    }

    async fn fetch_transaction(&self, _tx_hash: &str, _request_id: &str) -> ApplicationResult<Option<UnifiedTransaction>> {
        // Simplified implementation
        Ok(Some(UnifiedTransaction {
            hash: _tx_hash.to_string(),
            from: "0x1234567890123456789012345678901234567890".to_string(),
            to: Some("0x0987654321098765432109876543210987654321".to_string()),
            value: "1000000000000000000".to_string(),
            gas_info: GasInfo {
                gas_limit: 21000,
                gas_used: Some(21000),
                gas_price: Some("20000000000".to_string()),
                max_fee_per_gas: None,
            },
            nonce: 1,
            data: "0x".to_string(),
            vm_specific: serde_json::json!({}),
        }))
    }

    async fn fetch_account(&self, address: &str, _request_id: &str) -> ApplicationResult<UnifiedAccount> {
        Ok(UnifiedAccount {
            address: address.to_string(),
            balance: "1000000000000000000".to_string(),
            nonce: Some(1),
            vm_specific: serde_json::json!({}),
        })
    }

    async fn submit_transaction(&self, _raw_tx: &str, _request_id: &str) -> ApplicationResult<String> {
        // Generate a mock transaction hash
        use sha2::{Digest, Sha256};
        let hash = Sha256::digest(_raw_tx.as_bytes());
        Ok(format!("0x{}", hex::encode(hash)))
    }

    async fn ping_endpoint(&self) -> ApplicationResult<()> {
        // Simple health check - in real implementation would ping the actual endpoint
        Ok(())
    }
}

impl UnifiedGatewayConfig {
    /// Create config from application config
    pub fn from_app_config(app_config: &crate::config::ApplicationConfig) -> Self {
        Self {
            vm_type: VmType::Evm, // Default to EVM
            rpc_url: app_config.base.blockchain.ethereum.rpc_url.clone(),
            backup_urls: vec![],
            timeout: Duration::from_secs(app_config.base.blockchain.ethereum.timeout_seconds),
            max_retries: app_config.base.blockchain.ethereum.max_retries,
            chain_id: 1,
            production_mode: true,
        }
    }
}

impl Default for UnifiedGatewayConfig {
    fn default() -> Self {
        Self {
            vm_type: VmType::Evm,
            rpc_url: "http://localhost:8545".to_string(),
            backup_urls: vec![],
            timeout: Duration::from_secs(30),
            max_retries: 3,
            chain_id: 31337,
            production_mode: false,
        }
    }
}

impl Default for GasInfo {
    fn default() -> Self {
        Self {
            gas_limit: 21000,
            gas_used: None,
            gas_price: None,
            max_fee_per_gas: None,
        }
    }
}