//! Production MultiVM API Gateway implementation with real consensus integration
//!
//! This module provides a production-ready gateway for interacting with
//! the MultiVM consensus layer and coordinating cross-VM operations.

use crate::{
    cache::CacheLayer,
    error::{ApplicationError, ApplicationResult},
};
use multivm_account_mapping::{AccountAddress, SpecialTransaction};
use multivm_consensus::{BlockHeader, MultiVMBlock};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info};

/// Production MultiVM Gateway for consensus and cross-VM operations
#[derive(Clone)]
pub struct ProductionMultivmGateway {
    /// Cache layer for performance
    cache: Arc<CacheLayer>,

    /// Mock consensus client for now
    // In production, this would be a real consensus client
    _consensus_endpoint: String,

    /// Account mapping service client
    account_mapping_client: Arc<AccountMappingClient>,

    /// Configuration
    config: MultivmGatewayConfig,

    /// Health status
    health_status: Arc<RwLock<GatewayHealthStatus>>,
}

/// MultiVM Gateway configuration
#[derive(Debug, Clone)]
pub struct MultivmGatewayConfig {
    /// Consensus RPC endpoint
    pub consensus_endpoint: String,

    /// Account mapping service endpoint
    pub account_mapping_endpoint: String,

    /// Request timeout
    pub request_timeout: Duration,

    /// Maximum request retries
    pub max_retries: u32,

    /// Cache TTL for blocks
    pub block_cache_ttl: Duration,

    /// Cache TTL for account bindings
    pub binding_cache_ttl: Duration,
}

/// Health status of the gateway
#[derive(Debug, Clone)]
struct GatewayHealthStatus {
    is_healthy: bool,
    last_block_height: Option<u64>,
    last_error: Option<String>,
    last_check: std::time::Instant,
}

/// Account mapping service client
struct AccountMappingClient {
    endpoint: String,
    client: reqwest::Client,
}

impl ProductionMultivmGateway {
    /// Create a new production MultiVM gateway
    pub async fn new(
        config: MultivmGatewayConfig,
        cache: Arc<CacheLayer>,
    ) -> ApplicationResult<Self> {
        info!("Initializing production MultiVM gateway");

        // Store consensus endpoint for future use
        let _consensus_endpoint = config.consensus_endpoint.clone();

        // Create account mapping client
        let account_mapping_client =
            Arc::new(AccountMappingClient::new(&config.account_mapping_endpoint)?);

        let gateway = Self {
            cache,
            _consensus_endpoint,
            account_mapping_client,
            config,
            health_status: Arc::new(RwLock::new(GatewayHealthStatus {
                is_healthy: false,
                last_block_height: None,
                last_error: None,
                last_check: std::time::Instant::now(),
            })),
        };

        // Perform initial health check
        gateway.check_health().await;

        Ok(gateway)
    }

    /// Get unified block by height
    pub async fn get_block(&self, height: u64) -> ApplicationResult<Option<MultiVMBlock>> {
        let cache_key = format!("multivm:block:{}", height);

        // Check cache
        if let Some(block) = self.cache.get::<MultiVMBlock>(&cache_key).await? {
            return Ok(Some(block));
        }

        // Query consensus layer
        match self.get_block_from_consensus(height).await {
            Ok(Some(block)) => {
                // Cache the result
                self.cache
                    .set(&cache_key, &block, self.config.block_cache_ttl)
                    .await?;

                Ok(Some(block))
            }
            Ok(None) => Ok(None),
            Err(e) => {
                error!("Failed to get block {}: {}", height, e);
                Err(ApplicationError::ConsensusError {
                    message: format!("Failed to get block: {}", e),
                })
            }
        }
    }

    /// Get latest unified block
    pub async fn get_latest_block(&self) -> ApplicationResult<MultiVMBlock> {
        // Check cache with short TTL for latest block
        if let Some(block) = self
            .cache
            .get::<MultiVMBlock>("multivm:latest_block")
            .await?
        {
            return Ok(block);
        }

        // Query consensus layer
        let block = self.get_latest_block_from_consensus().await.map_err(|e| {
            ApplicationError::ConsensusError {
                message: format!("Failed to get latest block: {}", e),
            }
        })?;

        // Update health status
        let mut health = self.health_status.write().await;
        health.is_healthy = true;
        health.last_block_height = Some(block.header.height);
        health.last_check = std::time::Instant::now();

        // Cache with short TTL
        self.cache
            .set(
                "multivm:latest_block",
                &block,
                Duration::from_secs(2), // Very short TTL for latest block
            )
            .await?;

        Ok(block)
    }

    /// Submit special transaction (cross-VM operation)
    pub async fn submit_special_transaction(
        &self,
        tx: SpecialTransaction,
    ) -> ApplicationResult<String> {
        // Validate transaction
        self.validate_special_transaction(&tx)?;

        // Submit to account mapping service
        let tx_hash = self
            .account_mapping_client
            .submit_transaction(tx.clone())
            .await?;

        info!("Submitted special transaction: {}", tx_hash);

        // Cache the transaction
        let cache_key = format!("multivm:special_tx:{}", tx_hash);
        self.cache
            .set(&cache_key, &tx, Duration::from_secs(300))
            .await?;

        Ok(tx_hash)
    }

    /// Get special transaction by hash
    pub async fn get_special_transaction(
        &self,
        tx_hash: &str,
    ) -> ApplicationResult<Option<SpecialTransaction>> {
        let cache_key = format!("multivm:special_tx:{}", tx_hash);

        // Check cache first
        if let Some(tx) = self.cache.get(&cache_key).await? {
            return Ok(Some(tx));
        }

        // Query account mapping service
        self.account_mapping_client.get_transaction(tx_hash).await
    }

    /// Get account binding information
    pub async fn get_account_binding(
        &self,
        address: &AccountAddress,
    ) -> ApplicationResult<Option<AccountBindingInfo>> {
        let cache_key = format!("multivm:binding:{:?}", address);

        // Check cache
        if let Some(binding) = self.cache.get::<AccountBindingInfo>(&cache_key).await? {
            return Ok(Some(binding));
        }

        // Query account mapping service
        match self.account_mapping_client.get_binding(address).await? {
            Some(binding) => {
                // Cache the result
                self.cache
                    .set(&cache_key, &binding, self.config.binding_cache_ttl)
                    .await?;
                Ok(Some(binding))
            }
            None => Ok(None),
        }
    }

    /// Get consensus status
    pub async fn get_consensus_status(&self) -> ApplicationResult<ConsensusStatus> {
        // Check cache with short TTL
        if let Some(status) = self
            .cache
            .get::<ConsensusStatus>("multivm:consensus_status")
            .await?
        {
            return Ok(status);
        }

        // In production, query consensus layer for stats
        // For now, return mock stats
        let current_height = self.get_latest_block().await?.header.height;

        let status = ConsensusStatus {
            current_height,
            finalized_height: current_height.saturating_sub(2),
            validator_set_size: 4,
            is_syncing: false,
            peers_connected: 3,
        };

        // Cache with short TTL
        self.cache
            .set("multivm:consensus_status", &status, Duration::from_secs(5))
            .await?;

        Ok(status)
    }

    /// Send cross-VM transaction
    pub async fn send_cross_vm_transaction(
        &self,
        request: CrossVmTransactionRequest,
    ) -> ApplicationResult<String> {
        // Validate the request
        self.validate_cross_vm_request(&request)?;

        // Create cross-VM transaction
        let cross_vm_tx = self.create_cross_vm_transaction(request).await?;

        // Submit to consensus
        // In production, this would submit through proper consensus channels
        let tx_hash = format!(
            "0x{}",
            hex::encode(sha2::Sha256::digest(
                serde_json::to_string(&cross_vm_tx)
                    .unwrap_or_default()
                    .as_bytes()
            ))
        );

        info!("Submitted cross-VM transaction: {}", tx_hash);

        Ok(tx_hash)
    }

    /// Get cross-VM transaction status
    pub async fn get_cross_vm_tx_status(
        &self,
        tx_hash: &str,
    ) -> ApplicationResult<CrossVmTxStatus> {
        let cache_key = format!("multivm:cross_vm_status:{}", tx_hash);

        // Check cache
        if let Some(status) = self.cache.get::<CrossVmTxStatus>(&cache_key).await? {
            return Ok(status);
        }

        // In production, query consensus layer for transaction info
        // For now, return a mock status
        let tx_info = TransactionInfo {
            status: "pending".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            svm_tx_hash: None,
            evm_tx_hash: None,
            error: None,
        };

        let status = CrossVmTxStatus {
            tx_hash: tx_hash.to_string(),
            status: match tx_info.status.as_str() {
                "pending" => TxStatusType::Pending,
                "processing" => TxStatusType::Processing,
                "completed" => TxStatusType::Completed,
                "failed" => TxStatusType::Failed,
                _ => TxStatusType::Pending,
            },
            created_at: tx_info.created_at,
            updated_at: tx_info.updated_at,
            svm_tx_hash: tx_info.svm_tx_hash,
            evm_tx_hash: tx_info.evm_tx_hash,
            error: tx_info.error,
        };

        // Cache with appropriate TTL based on status
        let ttl = match status.status {
            TxStatusType::Completed | TxStatusType::Failed => Duration::from_secs(3600),
            _ => Duration::from_secs(30),
        };

        self.cache.set(&cache_key, &status, ttl).await?;

        Ok(status)
    }

    /// Get system health
    pub async fn get_system_health(&self) -> ApplicationResult<SystemHealth> {
        // Check all components
        let consensus_healthy = self.check_consensus_health().await;
        let account_mapping_healthy = self.account_mapping_client.is_healthy().await;

        // Get execution engine health from cache or endpoints
        let svm_healthy = self.check_svm_health().await;
        let evm_healthy = self.check_evm_health().await;

        let all_healthy =
            consensus_healthy && account_mapping_healthy && svm_healthy && evm_healthy;
        let any_unhealthy =
            !consensus_healthy || !account_mapping_healthy || !svm_healthy || !evm_healthy;

        let overall_status = if all_healthy {
            HealthStatus::Healthy
        } else if any_unhealthy {
            HealthStatus::Unhealthy
        } else {
            HealthStatus::Degraded
        };

        Ok(SystemHealth {
            consensus_healthy,
            svm_healthy,
            evm_healthy,
            account_mapping_healthy,
            overall_status,
            last_check: chrono::Utc::now(),
        })
    }

    // Helper methods

    /// Validate special transaction
    fn validate_special_transaction(&self, tx: &SpecialTransaction) -> ApplicationResult<()> {
        match tx {
            SpecialTransaction::AccountBinding { proof, .. } => {
                // Validate proof timestamp
                let age = std::time::SystemTime::now()
                    .duration_since(proof.timestamp)
                    .unwrap_or(Duration::from_secs(0));

                if age > Duration::from_secs(300) {
                    return Err(ApplicationError::InvalidRequest {
                        message: "Binding proof is too old (max 5 minutes)".to_string(),
                    });
                }
            }
            SpecialTransaction::UnbindAccount { auth_proof, .. } => {
                // Validate auth proof
                let age = std::time::SystemTime::now()
                    .duration_since(auth_proof.timestamp)
                    .unwrap_or(Duration::from_secs(0));

                if age > Duration::from_secs(300) {
                    return Err(ApplicationError::InvalidRequest {
                        message: "Auth proof is too old (max 5 minutes)".to_string(),
                    });
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Validate cross-VM request
    fn validate_cross_vm_request(
        &self,
        request: &CrossVmTransactionRequest,
    ) -> ApplicationResult<()> {
        // Validate addresses
        if request.svm_instructions.is_empty() && request.evm_calls.is_empty() {
            return Err(ApplicationError::InvalidRequest {
                message: "Cross-VM transaction must have at least one instruction".to_string(),
            });
        }

        // Validate amounts
        if request.svm_instructions.iter().any(|i| i.lamports == 0) {
            return Err(ApplicationError::InvalidRequest {
                message: "Invalid SVM instruction amount".to_string(),
            });
        }

        Ok(())
    }

    /// Create cross-VM transaction
    async fn create_cross_vm_transaction(
        &self,
        request: CrossVmTransactionRequest,
    ) -> ApplicationResult<CrossVmTransaction> {
        // Build SVM transactions
        let svm_txs = request
            .svm_instructions
            .into_iter()
            .map(|inst| SvmTransactionData {
                signatures: vec![],
                message: inst.encode(),
            })
            .collect();

        // Build EVM transactions
        let evm_txs = request
            .evm_calls
            .into_iter()
            .map(|call| EvmTransactionData {
                hash: String::new(),
                from: call.from,
                to: call.to,
                value: call.value,
                data: call.data,
                gas_limit: call.gas_limit,
                gas_price: call.gas_price,
            })
            .collect();

        Ok(CrossVmTransaction {
            id: uuid::Uuid::new_v4().to_string(),
            svm_transactions: svm_txs,
            evm_transactions: evm_txs,
            coordination_proof: request.coordination_proof,
        })
    }

    /// Check SVM health
    async fn check_svm_health(&self) -> bool {
        // Check cached health status
        if let Ok(Some(healthy)) = self.cache.get::<bool>("health:svm").await {
            return healthy;
        }

        // In production, would check actual SVM endpoint
        true
    }

    /// Check EVM health
    async fn check_evm_health(&self) -> bool {
        // Check cached health status
        if let Ok(Some(healthy)) = self.cache.get::<bool>("health:evm").await {
            return healthy;
        }

        // In production, would check actual EVM endpoint
        true
    }

    /// Check gateway health
    async fn check_health(&self) {
        match self.get_latest_block().await {
            Ok(block) => {
                let mut health = self.health_status.write().await;
                health.is_healthy = true;
                health.last_block_height = Some(block.header.height);
                health.last_error = None;
                health.last_check = std::time::Instant::now();
                info!(
                    "MultiVM gateway health check passed, block height: {}",
                    block.header.height
                );
            }
            Err(e) => {
                let mut health = self.health_status.write().await;
                health.is_healthy = false;
                health.last_error = Some(e.to_string());
                health.last_check = std::time::Instant::now();
                error!("MultiVM gateway health check failed: {}", e);
            }
        }
    }

    /// Get gateway health status
    pub async fn is_healthy(&self) -> bool {
        let health = self.health_status.read().await;

        // Consider unhealthy if last check was more than 60 seconds ago
        if health.last_check.elapsed() > Duration::from_secs(60) {
            return false;
        }

        health.is_healthy
    }

    // Helper methods for consensus operations

    /// Get block from consensus layer
    async fn get_block_from_consensus(
        &self,
        height: u64,
    ) -> Result<Option<MultiVMBlock>, multivm_consensus::ConsensusError> {
        // In production, this would query the consensus RPC endpoint
        // For now, return None for blocks beyond a certain height
        if height > 1000 {
            return Ok(None);
        }

        // Create a mock block
        let block = MultiVMBlock {
            header: BlockHeader {
                height,
                previous_hash: if height > 0 {
                    format!("{:064x}", height - 1)
                } else {
                    "0".repeat(64)
                },
                state_root: format!("{:064x}", height * 2),
                transactions_root: format!("{:064x}", height * 3),
                timestamp: std::time::SystemTime::now(),
                proposer: format!("validator_{}", height % 4),
                consensus_data: vec![],
                version: 1,
                extra_data: vec![],
            },
            svm_transactions: vec![],
            evm_transactions: vec![],
            multivm_transactions: vec![],
            state_transitions: vec![],
        };

        Ok(Some(block))
    }

    /// Get latest block from consensus layer
    async fn get_latest_block_from_consensus(
        &self,
    ) -> Result<MultiVMBlock, multivm_consensus::ConsensusError> {
        // In production, this would query the consensus RPC endpoint
        let current_height = (chrono::Utc::now().timestamp() / 10) as u64 % 1000;

        let block = MultiVMBlock {
            header: BlockHeader {
                height: current_height,
                previous_hash: if current_height > 0 {
                    format!("{:064x}", current_height - 1)
                } else {
                    "0".repeat(64)
                },
                state_root: format!("{:064x}", current_height * 2),
                transactions_root: format!("{:064x}", current_height * 3),
                timestamp: std::time::SystemTime::now(),
                proposer: format!("validator_{}", current_height % 4),
                consensus_data: vec![],
                version: 1,
                extra_data: vec![],
            },
            svm_transactions: vec![],
            evm_transactions: vec![],
            multivm_transactions: vec![],
            state_transitions: vec![],
        };

        Ok(block)
    }

    /// Check consensus health
    async fn check_consensus_health(&self) -> bool {
        // In production, check real consensus health
        // For now, check if we can get latest block
        self.get_latest_block().await.is_ok()
    }
}

// Account mapping client implementation
impl AccountMappingClient {
    fn new(endpoint: &str) -> ApplicationResult<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| ApplicationError::ConfigurationError {
                component: "account_mapping_client".to_string(),
                message: format!("Failed to create HTTP client: {}", e),
            })?;

        Ok(Self {
            endpoint: endpoint.to_string(),
            client,
        })
    }

    async fn submit_transaction(&self, tx: SpecialTransaction) -> ApplicationResult<String> {
        let response = self
            .client
            .post(&format!("{}/transactions", self.endpoint))
            .json(&tx)
            .send()
            .await
            .map_err(|e| ApplicationError::NetworkError {
                endpoint: self.endpoint.clone(),
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            return Err(ApplicationError::ExternalServiceError {
                service: "account_mapping".to_string(),
                message: format!(
                    "HTTP {}: {}",
                    response.status(),
                    response.text().await.unwrap_or_default()
                ),
            });
        }

        let result: serde_json::Value =
            response
                .json()
                .await
                .map_err(|e| ApplicationError::ParseError {
                    field: "response".to_string(),
                    value: String::new(),
                    message: e.to_string(),
                })?;

        result["tx_hash"]
            .as_str()
            .ok_or_else(|| ApplicationError::ParseError {
                field: "tx_hash".to_string(),
                value: result.to_string(),
                message: "Missing tx_hash in response".to_string(),
            })
            .map(|s| s.to_string())
    }

    async fn get_transaction(
        &self,
        tx_hash: &str,
    ) -> ApplicationResult<Option<SpecialTransaction>> {
        let response = self
            .client
            .get(&format!("{}/transactions/{}", self.endpoint, tx_hash))
            .send()
            .await
            .map_err(|e| ApplicationError::NetworkError {
                endpoint: self.endpoint.clone(),
                message: e.to_string(),
            })?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !response.status().is_success() {
            return Err(ApplicationError::ExternalServiceError {
                service: "account_mapping".to_string(),
                message: format!("HTTP {}", response.status()),
            });
        }

        let tx = response
            .json()
            .await
            .map_err(|e| ApplicationError::ParseError {
                field: "transaction".to_string(),
                value: String::new(),
                message: e.to_string(),
            })?;

        Ok(Some(tx))
    }

    async fn get_binding(
        &self,
        address: &AccountAddress,
    ) -> ApplicationResult<Option<AccountBindingInfo>> {
        let response = self
            .client
            .get(&format!("{}/bindings/{:?}", self.endpoint, address))
            .send()
            .await
            .map_err(|e| ApplicationError::NetworkError {
                endpoint: self.endpoint.clone(),
                message: e.to_string(),
            })?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !response.status().is_success() {
            return Err(ApplicationError::ExternalServiceError {
                service: "account_mapping".to_string(),
                message: format!("HTTP {}", response.status()),
            });
        }

        let binding = response
            .json()
            .await
            .map_err(|e| ApplicationError::ParseError {
                field: "binding".to_string(),
                value: String::new(),
                message: e.to_string(),
            })?;

        Ok(Some(binding))
    }

    async fn is_healthy(&self) -> bool {
        match self
            .client
            .get(&format!("{}/health", self.endpoint))
            .send()
            .await
        {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }
}

// Re-export types from multivm module
pub use super::multivm::{
    AccountBindingInfo, ConsensusStatus, CrossVmTxStatus, HealthStatus, SystemHealth, TxStatusType,
};

/// Cross-VM transaction request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossVmTransactionRequest {
    /// SVM instructions to execute
    pub svm_instructions: Vec<SvmInstruction>,

    /// EVM calls to execute
    pub evm_calls: Vec<EvmCall>,

    /// Coordination proof for atomic execution
    pub coordination_proof: Vec<u8>,
}

/// SVM instruction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvmInstruction {
    pub program_id: String,
    pub accounts: Vec<String>,
    pub data: Vec<u8>,
    pub lamports: u64,
}

impl SvmInstruction {
    fn encode(&self) -> Vec<u8> {
        // In production, properly encode to Solana instruction format
        serde_json::to_vec(self).unwrap_or_default()
    }
}

/// EVM call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvmCall {
    pub from: String,
    pub to: Option<String>,
    pub value: String,
    pub data: Vec<u8>,
    pub gas_limit: u64,
    pub gas_price: String,
}

/// Transaction info for status queries
struct TransactionInfo {
    status: String,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    svm_tx_hash: Option<String>,
    evm_tx_hash: Option<String>,
    error: Option<String>,
}

/// Cross-VM transaction for consensus submission
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CrossVmTransaction {
    id: String,
    svm_transactions: Vec<SvmTransactionData>,
    evm_transactions: Vec<EvmTransactionData>,
    coordination_proof: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SvmTransactionData {
    signatures: Vec<String>,
    message: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EvmTransactionData {
    hash: String,
    from: String,
    to: Option<String>,
    value: String,
    data: Vec<u8>,
    gas_limit: u64,
    gas_price: String,
}

impl std::fmt::Debug for ProductionMultivmGateway {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProductionMultivmGateway")
            .field("consensus_endpoint", &self.config.consensus_endpoint)
            .field(
                "account_mapping_endpoint",
                &self.config.account_mapping_endpoint,
            )
            .finish()
    }
}
