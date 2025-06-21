//! MultiVM API Gateway implementation

use crate::{cache::CacheLayer, error::ApplicationResult};
use multivm_account_mapping::{AccountAddress, SpecialTransaction};
use multivm_consensus::MultiVMBlock;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::sync::Arc;

/// MultiVM API Gateway for cross-VM operations
#[derive(Clone, Debug)]
pub struct MultivmApiGateway {
    cache: Arc<CacheLayer>,
    consensus_endpoint: String,
    account_mapping_endpoint: String,
}

impl MultivmApiGateway {
    /// Create new MultiVM API Gateway
    pub async fn new(
        config: &crate::config::MultivmClientConfig,
        cache: Arc<CacheLayer>,
    ) -> ApplicationResult<Self> {
        Ok(Self {
            cache,
            consensus_endpoint: config.consensus_endpoint.clone(),
            account_mapping_endpoint: config.account_mapping_endpoint.clone(),
        })
    }

    /// Get unified block by height
    pub async fn get_block(&self, height: u64) -> ApplicationResult<Option<MultiVMBlock>> {
        let cache_key = format!("multivm:block:{}", height);

        // Check cache
        if let Some(block) = self.cache.get::<MultiVMBlock>(&cache_key).await? {
            return Ok(Some(block));
        }

        // Mock implementation - would query consensus layer
        if height <= 100 {
            let block = MultiVMBlock {
                header: multivm_consensus::BlockHeader {
                    height,
                    previous_hash: if height > 0 {
                        format!("{:064x}", height - 1)
                    } else {
                        "0".repeat(64)
                    },
                    state_root: format!("{:064x}", height * 2),
                    transactions_root: format!("{:064x}", height * 3),
                    timestamp: std::time::SystemTime::now(),
                    proposer: format!("node_{}", height % 4),
                    consensus_data: vec![],
                    version: 1,
                    extra_data: vec![],
                },
                svm_transactions: vec![],
                evm_transactions: vec![],
                multivm_transactions: vec![],
                state_transitions: vec![],
            };

            self.cache
                .set(&cache_key, &block, std::time::Duration::from_secs(3600))
                .await?;
            Ok(Some(block))
        } else {
            Ok(None)
        }
    }

    /// Get latest unified block
    pub async fn get_latest_block(&self) -> ApplicationResult<MultiVMBlock> {
        // Check cache
        if let Some(block) = self
            .cache
            .get::<MultiVMBlock>("multivm:latest_block")
            .await?
        {
            return Ok(block);
        }

        // Enhanced mock implementation - simulate growing block height
        let current_height = (chrono::Utc::now().timestamp() / 10) % 100000; // New block every 10 seconds
        let block = MultiVMBlock {
            header: multivm_consensus::BlockHeader {
                height: current_height as u64,
                previous_hash: format!("{:064x}", current_height - 1),
                state_root: format!("{:064x}", current_height * 2),
                transactions_root: format!("{:064x}", current_height * 3),
                timestamp: std::time::SystemTime::now(),
                proposer: format!("node_{}", current_height % 4),
                consensus_data: vec![],
                version: 1,
                extra_data: vec![],
            },
            svm_transactions: vec![],
            evm_transactions: vec![],
            multivm_transactions: vec![],
            state_transitions: vec![],
        };

        self.cache
            .set(
                "multivm:latest_block",
                &block,
                std::time::Duration::from_secs(5),
            )
            .await?;
        Ok(block)
    }

    /// Submit special transaction (cross-VM operation)
    pub async fn submit_special_transaction(
        &self,
        tx: SpecialTransaction,
    ) -> ApplicationResult<String> {
        // Mock implementation - would submit to account mapping layer
        let tx_hash = format!(
            "0x{}",
            hex::encode(sha2::Sha256::digest(serde_json::to_string(&tx)?.as_bytes()))
        );

        // Cache the transaction
        let cache_key = format!("multivm:special_tx:{}", tx_hash);
        self.cache
            .set(&cache_key, &tx, std::time::Duration::from_secs(300))
            .await?;

        Ok(tx_hash)
    }

    /// Get special transaction by hash
    pub async fn get_special_transaction(
        &self,
        tx_hash: &str,
    ) -> ApplicationResult<Option<SpecialTransaction>> {
        let cache_key = format!("multivm:special_tx:{}", tx_hash);
        self.cache.get(&cache_key).await
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

        // Mock implementation - would query account mapping layer
        Ok(None)
    }

    /// Bind SVM and EVM accounts together
    pub async fn bind_accounts(
        &self,
        svm_account: String,
        evm_account: String,
    ) -> ApplicationResult<String> {
        // Mock implementation - would call account mapping layer
        let binding_id = format!("binding_{}_{}", &svm_account[..8], &evm_account[..8]);

        // Create mock binding info
        let mut binding_info = AccountBindingInfo::new(binding_id.clone());
        binding_info.svm_address = Some(svm_account.clone());
        binding_info.evm_address = Some(evm_account.clone());

        // Cache the binding
        let cache_key = format!("multivm:binding:{}", binding_id);
        self.cache
            .set(
                &cache_key,
                &binding_info,
                std::time::Duration::from_secs(3600),
            )
            .await?;

        Ok(binding_id)
    }

    /// Unbind accounts
    pub async fn unbind_accounts(&self, binding_id: &str) -> ApplicationResult<bool> {
        // Mock implementation - would call account mapping layer
        let cache_key = format!("multivm:binding:{}", binding_id);

        // Check if binding exists and remove it
        match self.cache.get::<AccountBindingInfo>(&cache_key).await? {
            Some(binding_info) => {
                // Store values before moving binding_info
                let svm_addr = binding_info.svm_address.clone().unwrap_or_default();
                let evm_addr = binding_info.evm_address.clone().unwrap_or_default();
                let multivm_id = binding_info.multivm_id.clone();

                // Remove from cache
                self.cache.delete(&cache_key).await?;

                // Store the unbinding transaction in the blockchain
                let unbind_tx = format!("unbind_{}_{}", svm_addr, evm_addr);

                // Create a mock account address for the unbinding operation
                let mock_account = multivm_account_mapping::AccountAddress::Solana(
                    multivm_account_mapping::SolanaAddress([0u8; 32]),
                );

                // Create special transaction for unbinding
                let special_tx = multivm_account_mapping::SpecialTransaction::UnbindAccount {
                    multivm_account: multivm_account_mapping::MultivmAccountId::from_seed(
                        multivm_id.as_bytes(),
                    ),
                    account: mock_account.clone(),
                    auth_proof: multivm_account_mapping::BindingProof {
                        account: mock_account,
                        proof_type: multivm_account_mapping::ProofType::Signature {
                            message: b"unbind_request".to_vec(),
                            signature: self.generate_unbind_signature(&multivm_id),
                        },
                        proof_data: self.generate_auth_proof_data(&binding_info),
                        timestamp: std::time::SystemTime::now(),
                    },
                };

                // Submit to account mapping layer for permanent storage
                let _tx_hash = self.submit_special_transaction(special_tx).await?;

                tracing::info!(
                    "Successfully unbound accounts for binding ID: {}",
                    binding_id
                );
                Ok(true)
            }
            None => {
                tracing::warn!("Binding not found for ID: {}", binding_id);
                Ok(false)
            }
        }
    }

    /// Send cross-VM transaction
    pub async fn send_cross_vm_transaction(
        &self,
        svm_tx: String,
        evm_tx: String,
    ) -> ApplicationResult<String> {
        // Mock implementation - would coordinate cross-VM transaction
        let cross_vm_tx_id = format!(
            "cross_vm_{}_{}",
            &svm_tx[..std::cmp::min(8, svm_tx.len())],
            &evm_tx[..std::cmp::min(8, evm_tx.len())]
        );

        // Cache the cross-VM transaction
        let cache_key = format!("multivm:cross_vm_tx:{}", cross_vm_tx_id);
        let tx_info = serde_json::json!({
            "id": cross_vm_tx_id,
            "svm_transaction": svm_tx,
            "evm_transaction": evm_tx,
            "status": "pending",
            "created_at": chrono::Utc::now(),
        });

        self.cache
            .set(&cache_key, &tx_info, std::time::Duration::from_secs(3600))
            .await?;

        Ok(cross_vm_tx_id)
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

        // Mock implementation
        let status = CrossVmTxStatus {
            tx_hash: tx_hash.to_string(),
            status: TxStatusType::Pending,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            svm_tx_hash: None,
            evm_tx_hash: None,
            error: None,
        };

        self.cache
            .set(&cache_key, &status, std::time::Duration::from_secs(60))
            .await?;
        Ok(status)
    }

    /// Generate signature for unbinding operation
    fn generate_unbind_signature(&self, binding_id: &str) -> Vec<u8> {
        // In production, this would:
        // 1. Create a proper message to sign
        // 2. Use the user's private key to sign it
        // 3. Return the actual signature

        // For now, create a deterministic "signature" based on binding ID
        let mut hasher = sha2::Sha256::new();
        hasher.update(b"unbind_signature:");
        hasher.update(binding_id.as_bytes());
        hasher.update(b":multivm_gateway");
        hasher.finalize().to_vec()[..64].to_vec()
    }

    /// Generate authentication proof data
    fn generate_auth_proof_data(&self, binding_info: &AccountBindingInfo) -> Vec<u8> {
        // In production, this would include:
        // 1. Merkle proofs of account ownership
        // 2. Cryptographic attestations
        // 3. Timestamp validation data

        // For now, create a simple proof structure
        let proof_data = serde_json::json!({
            "binding_id": binding_info.multivm_id,
            "svm_address": binding_info.svm_address,
            "evm_address": binding_info.evm_address,
            "created_at": binding_info.created_at,
            "proof_type": "unbind_authorization",
            "version": "1.0"
        });

        proof_data.to_string().into_bytes()
    }

    /// Get consensus status
    pub async fn get_consensus_status(&self) -> ApplicationResult<ConsensusStatus> {
        // Check cache
        if let Some(status) = self
            .cache
            .get::<ConsensusStatus>("multivm:consensus_status")
            .await?
        {
            return Ok(status);
        }

        // Mock implementation
        let status = ConsensusStatus {
            current_height: 100,
            finalized_height: 98,
            validator_set_size: 4,
            is_syncing: false,
            peers_connected: 3,
        };

        self.cache
            .set(
                "multivm:consensus_status",
                &status,
                std::time::Duration::from_secs(10),
            )
            .await?;
        Ok(status)
    }

    /// Get system health
    pub async fn get_system_health(&self) -> ApplicationResult<SystemHealth> {
        // Mock implementation
        Ok(SystemHealth {
            consensus_healthy: true,
            svm_healthy: true,
            evm_healthy: true,
            account_mapping_healthy: true,
            overall_status: HealthStatus::Healthy,
            last_check: chrono::Utc::now(),
        })
    }
}

// Re-export AccountBindingInfo from common
pub use multivm_common::types::AccountBindingInfo;

/// Cross-VM transaction status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossVmTxStatus {
    pub tx_hash: String,
    pub status: TxStatusType,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub svm_tx_hash: Option<String>,
    pub evm_tx_hash: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TxStatusType {
    Pending,
    Processing,
    Completed,
    Failed,
}

/// Consensus status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusStatus {
    pub current_height: u64,
    pub finalized_height: u64,
    pub validator_set_size: usize,
    pub is_syncing: bool,
    pub peers_connected: usize,
}

/// System health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub consensus_healthy: bool,
    pub svm_healthy: bool,
    pub evm_healthy: bool,
    pub account_mapping_healthy: bool,
    pub overall_status: HealthStatus,
    pub last_check: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}
