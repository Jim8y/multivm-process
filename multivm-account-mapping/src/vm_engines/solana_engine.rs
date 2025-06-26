//! Solana Process Engine for Atomic Cross-VM Transactions
//!
//! This module implements the ProcessEngine trait for communicating with external
//! Solana processes. MultiVM does NOT execute transactions itself - it coordinates
//! with external Solana processes via IPC calls.
//!
//! ## Architecture
//!
//! MultiVM Coordinator → IPC → Solana Process (currently mocked)
//!                               ↓
//!                          Solana Execution
//!
//! The Solana process handles all Solana transaction execution, while MultiVM
//! coordinates the atomic operations across multiple processes.

use crate::{
    address::AccountAddress,
    atomic_coordinator::{
        CommitResult, OperationStatus, OperationType, PrepareResult, StateChange, VmOperation,
        VmType,
    },
    special_tx::AssetType,
};
use multivm_common::{MultivmError, MultivmResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{Mutex, RwLock};
use tracing::info;
use uuid::Uuid;

/// Solana Process Engine implementation (communicates with external Solana)
pub struct SolanaProcessEngine {
    /// Connection to Solana RPC
    rpc_client: Arc<SolanaRpcClient>,
    /// Active locks for prepare phase
    active_locks: Arc<RwLock<HashMap<String, SolanaLock>>>,
    /// Configuration
    config: SolanaEngineConfig,
    /// Metrics
    metrics: Arc<Mutex<SolanaEngineMetrics>>,
}

/// Configuration for Solana VM engine
#[derive(Debug, Clone)]
pub struct SolanaEngineConfig {
    /// RPC endpoint URL
    pub rpc_url: String,
    /// Commitment level for confirmations
    pub commitment: SolanaCommitment,
    /// Maximum wait time for confirmations
    pub confirmation_timeout: Duration,
    /// Program ID for cross-VM operations
    pub cross_vm_program_id: String,
    /// Fee payer account
    pub fee_payer: Option<String>,
    /// Maximum transaction size
    pub max_transaction_size: usize,
}

/// Solana commitment levels
#[derive(Debug, Clone)]
pub enum SolanaCommitment {
    Processed,
    Confirmed,
    Finalized,
}

/// Solana lock record for atomic operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaLock {
    /// Unique lock ID
    pub lock_id: String,
    /// Account being locked
    pub account: AccountAddress,
    /// Amount locked
    pub amount: u64,
    /// Asset type
    pub asset: AssetType,
    /// Lock transaction signature
    pub lock_tx_signature: String,
    /// Lock timestamp
    pub created_at: SystemTime,
    /// Expiration time
    pub expires_at: SystemTime,
    /// Status
    pub status: LockStatus,
}

/// Lock status enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LockStatus {
    Pending,
    Active,
    Released,
    Expired,
}

/// Solana RPC client wrapper
pub struct SolanaRpcClient {
    /// Base RPC URL
    pub url: String,
    /// HTTP client
    client: reqwest::Client,
}

/// Solana engine metrics
#[derive(Debug, Clone, Default)]
pub struct SolanaEngineMetrics {
    /// Total operations processed
    pub operations_processed: u64,
    /// Successful operations
    pub successful_operations: u64,
    /// Failed operations
    pub failed_operations: u64,
    /// Active locks
    pub active_locks: u64,
    /// Average confirmation time
    pub avg_confirmation_time_ms: u64,
}

/// Solana transaction builder
pub struct SolanaTransactionBuilder {
    /// Instructions to include
    instructions: Vec<SolanaInstruction>,
    /// Fee payer
    fee_payer: Option<String>,
    /// Recent blockhash
    recent_blockhash: Option<String>,
}

/// Solana instruction representation
#[derive(Debug, Clone)]
pub struct SolanaInstruction {
    /// Program ID
    pub program_id: String,
    /// Accounts involved
    pub accounts: Vec<SolanaAccountMeta>,
    /// Instruction data
    pub data: Vec<u8>,
}

/// Solana account metadata
#[derive(Debug, Clone)]
pub struct SolanaAccountMeta {
    /// Account public key
    pub pubkey: String,
    /// Is signer
    pub is_signer: bool,
    /// Is writable
    pub is_writable: bool,
}

/// Cross-VM program instruction types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrossVmInstruction {
    /// Lock funds for cross-VM transfer
    LockFunds {
        amount: u64,
        lock_id: String,
        target_vm: VmType,
        expiry_time: u64,
    },
    /// Unlock funds (abort)
    UnlockFunds { lock_id: String },
    /// Complete transfer (commit)
    CompleteTransfer { lock_id: String, recipient: String },
    /// Mint wrapped tokens
    MintWrapped {
        amount: u64,
        recipient: String,
        origin_vm: VmType,
        origin_tx: String,
    },
    /// Burn wrapped tokens
    BurnWrapped { amount: u64, origin_vm: VmType },
}

impl SolanaProcessEngine {
    /// Create new Solana VM engine
    pub fn new(config: SolanaEngineConfig) -> Self {
        let rpc_client = Arc::new(SolanaRpcClient::new(config.rpc_url.clone()));

        Self {
            rpc_client,
            active_locks: Arc::new(RwLock::new(HashMap::new())),
            config,
            metrics: Arc::new(Mutex::new(SolanaEngineMetrics::default())),
        }
    }

    /// Build lock instruction for Solana
    async fn build_lock_instruction(
        &self,
        account: &AccountAddress,
        amount: u64,
        _asset: &AssetType,
        lock_id: &str,
        target_vm: VmType,
    ) -> MultivmResult<SolanaInstruction> {
        let instruction_data = CrossVmInstruction::LockFunds {
            amount,
            lock_id: lock_id.to_string(),
            target_vm,
            expiry_time: (SystemTime::now() + Duration::from_secs(3600))
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        let data =
            bincode::serialize(&instruction_data).map_err(|e| MultivmError::Serialization {
                message: e.to_string(),
                data_type: Some("solana_transaction".to_string()),
            })?;

        let accounts = vec![
            SolanaAccountMeta {
                pubkey: self.extract_solana_address(account)?,
                is_signer: true,
                is_writable: true,
            },
            // Add other required accounts (token program, etc.)
        ];

        Ok(SolanaInstruction {
            program_id: self.config.cross_vm_program_id.clone(),
            accounts,
            data,
        })
    }

    /// Extract Solana address from AccountAddress
    fn extract_solana_address(&self, account: &AccountAddress) -> MultivmResult<String> {
        match account {
            AccountAddress::Solana(sol_addr) => Ok(bs58::encode(sol_addr.0).into_string()),
            _ => Err(MultivmError::Configuration {
                component: "solana-engine".to_string(),
                message: "Invalid account type for Solana engine".to_string(),
                validation_errors: None,
            }),
        }
    }

    /// Submit transaction to external Solana process
    async fn submit_transaction(
        &self,
        _builder: SolanaTransactionBuilder,
    ) -> MultivmResult<String> {
        // Mock implementation - simulates IPC call to external Solana process
        // In production, this would:
        // 1. Serialize transaction data
        // 2. Send IPC/RPC request to Solana process
        // 3. Wait for Solana to execute transaction
        // 4. Return transaction signature from Solana

        let tx_signature = format!("solana_tx_{}", Uuid::new_v4());

        // Simulate IPC call latency to external Solana process
        tokio::time::sleep(Duration::from_millis(100)).await;

        info!(
            "[MOCK] Submitted transaction to Solana process: {}",
            tx_signature
        );
        Ok(tx_signature)
    }

    /// Wait for transaction confirmation from Solana process
    async fn wait_for_confirmation(&self, _signature: &str) -> MultivmResult<u64> {
        // Mock implementation - simulates polling Solana process for confirmation
        // In production, this would:
        // 1. Poll Solana process via IPC for transaction status
        // 2. Wait for sufficient confirmations
        // 3. Return final slot number

        // Simulate confirmation time from external Solana process
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Return mock slot number from Solana
        Ok(12345)
    }

    /// Create lock record
    async fn create_lock(&self, lock: SolanaLock) -> MultivmResult<()> {
        let mut locks = self.active_locks.write().await;
        locks.insert(lock.lock_id.clone(), lock);

        let mut metrics = self.metrics.lock().await;
        metrics.active_locks += 1;

        Ok(())
    }

    /// Release lock
    async fn release_lock(&self, lock_id: &str) -> MultivmResult<()> {
        let mut locks = self.active_locks.write().await;
        if let Some(mut lock) = locks.remove(lock_id) {
            lock.status = LockStatus::Released;

            let mut metrics = self.metrics.lock().await;
            metrics.active_locks = metrics.active_locks.saturating_sub(1);
        }

        Ok(())
    }

    /// Get active lock
    async fn get_lock(&self, lock_id: &str) -> Option<SolanaLock> {
        let locks = self.active_locks.read().await;
        locks.get(lock_id).cloned()
    }
}

#[async_trait::async_trait]
impl crate::atomic_coordinator::ProcessEngine for SolanaProcessEngine {
    async fn prepare(&self, operations: Vec<VmOperation>) -> MultivmResult<PrepareResult> {
        info!("Preparing {} Solana operations", operations.len());

        // Check RPC connection health
        if let Err(e) = self.rpc_client.check_connection().await {
            return Ok(PrepareResult {
                success: false,
                lock_ids: vec![],
                execution_cost: 0,
                error: Some(format!("Solana RPC connection failed: {}", e)),
                vm_data: HashMap::new(),
            });
        }

        let mut lock_ids = Vec::new();
        let mut execution_cost = 0u64;
        let mut vm_data = HashMap::new();

        let operation_count = operations.len();
        for operation in &operations {
            match &operation.operation {
                OperationType::Lock {
                    account,
                    amount,
                    asset,
                } => {
                    let lock_id = format!("solana_lock_{}", Uuid::new_v4());

                    // Build lock instruction
                    let instruction = self
                        .build_lock_instruction(account, *amount, asset, &lock_id, VmType::Svm)
                        .await?;

                    // Build transaction
                    let mut builder = SolanaTransactionBuilder::new();
                    builder.add_instruction(instruction);

                    // Submit transaction
                    let tx_signature = self.submit_transaction(builder).await?;

                    // Wait for confirmation
                    let block_height = self.wait_for_confirmation(&tx_signature).await?;

                    // Create lock record
                    let lock = SolanaLock {
                        lock_id: lock_id.clone(),
                        account: account.clone(),
                        amount: *amount,
                        asset: asset.clone(),
                        lock_tx_signature: tx_signature,
                        created_at: SystemTime::now(),
                        expires_at: SystemTime::now() + Duration::from_secs(3600),
                        status: LockStatus::Active,
                    };

                    self.create_lock(lock).await?;
                    lock_ids.push(lock_id);
                    execution_cost += 5000; // Mock cost in lamports

                    vm_data.insert("block_height".to_string(), block_height.to_string());
                }
                _ => {
                    return Err(MultivmError::UnsupportedOperation {
                        operation: "prepare_phase_operation".to_string(),
                        alternatives: Some(vec!["Use commit phase for this operation".to_string()]),
                    });
                }
            }
        }

        let mut metrics = self.metrics.lock().await;
        metrics.operations_processed += operation_count as u64;

        Ok(PrepareResult {
            success: true,
            lock_ids,
            execution_cost,
            error: None,
            vm_data,
        })
    }

    async fn commit(&self, prepare_result: PrepareResult) -> MultivmResult<CommitResult> {
        info!("Committing {} Solana locks", prepare_result.lock_ids.len());

        let mut tx_hashes = Vec::new();
        let mut state_changes = Vec::new();

        for lock_id in &prepare_result.lock_ids {
            if let Some(lock) = self.get_lock(lock_id).await {
                // Build commit instruction
                let instruction_data = CrossVmInstruction::CompleteTransfer {
                    lock_id: lock_id.clone(),
                    recipient: "target_account".to_string(), // This would be actual recipient
                };

                let data = bincode::serialize(&instruction_data).map_err(|e| {
                    MultivmError::Serialization {
                        message: e.to_string(),
                        data_type: Some("solana_transaction".to_string()),
                    }
                })?;

                let instruction = SolanaInstruction {
                    program_id: self.config.cross_vm_program_id.clone(),
                    accounts: vec![SolanaAccountMeta {
                        pubkey: self.extract_solana_address(&lock.account)?,
                        is_signer: true,
                        is_writable: true,
                    }],
                    data,
                };

                // Build and submit transaction
                let mut builder = SolanaTransactionBuilder::new();
                builder.add_instruction(instruction);

                let tx_signature = self.submit_transaction(builder).await?;
                let block_height = self.wait_for_confirmation(&tx_signature).await?;

                tx_hashes.push(tx_signature);

                // Record state change
                state_changes.push(StateChange {
                    account: lock.account.clone(),
                    asset: lock.asset.clone(),
                    balance_delta: -(lock.amount as i64),
                    block_height,
                });

                // Release lock
                self.release_lock(lock_id).await?;
            }
        }

        let mut metrics = self.metrics.lock().await;
        metrics.successful_operations += prepare_result.lock_ids.len() as u64;

        Ok(CommitResult {
            success: true,
            tx_hashes,
            state_changes,
            error: None,
        })
    }

    async fn abort(&self, prepare_result: PrepareResult) -> MultivmResult<()> {
        info!("Aborting {} Solana locks", prepare_result.lock_ids.len());

        for lock_id in &prepare_result.lock_ids {
            if let Some(_lock) = self.get_lock(lock_id).await {
                // Build unlock instruction
                let instruction_data = CrossVmInstruction::UnlockFunds {
                    lock_id: lock_id.clone(),
                };

                let data = bincode::serialize(&instruction_data).map_err(|e| {
                    MultivmError::Serialization {
                        message: e.to_string(),
                        data_type: Some("solana_transaction".to_string()),
                    }
                })?;

                let instruction = SolanaInstruction {
                    program_id: self.config.cross_vm_program_id.clone(),
                    accounts: vec![], // Add required accounts
                    data,
                };

                // Build and submit transaction
                let mut builder = SolanaTransactionBuilder::new();
                builder.add_instruction(instruction);

                let _tx_signature = self.submit_transaction(builder).await?;

                // Release lock
                self.release_lock(lock_id).await?;
            }
        }

        Ok(())
    }

    async fn status(&self, operation_id: &str) -> MultivmResult<OperationStatus> {
        if let Some(lock) = self.get_lock(operation_id).await {
            match lock.status {
                LockStatus::Active => Ok(OperationStatus::Prepared),
                LockStatus::Released => Ok(OperationStatus::Committed),
                LockStatus::Expired => Ok(OperationStatus::Aborted),
                LockStatus::Pending => Ok(OperationStatus::Pending),
            }
        } else {
            Ok(OperationStatus::Failed)
        }
    }
}

impl SolanaRpcClient {
    fn new(url: String) -> Self {
        Self {
            url,
            client: reqwest::Client::new(),
        }
    }

    /// Check if RPC connection is healthy
    pub async fn check_connection(&self) -> MultivmResult<()> {
        let response = self
            .client
            .post(&self.url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "method": "getHealth",
                "params": [],
                "id": 1
            }))
            .send()
            .await
            .map_err(|e| multivm_common::MultivmError::Rpc {
                method: "solana_rpc".to_string(),
                message: format!("RPC request failed: {}", e),
                status_code: None,
            })?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(multivm_common::MultivmError::Rpc {
                method: "solana_rpc".to_string(),
                message: format!("RPC returned status: {}", response.status()),
                status_code: Some(response.status().as_u16()),
            })
        }
    }
}

impl SolanaTransactionBuilder {
    fn new() -> Self {
        Self {
            instructions: Vec::new(),
            fee_payer: None,
            recent_blockhash: None,
        }
    }

    /// Set the fee payer for this transaction
    pub fn set_fee_payer(&mut self, fee_payer: String) {
        self.fee_payer = Some(fee_payer);
    }

    /// Set the recent blockhash for this transaction
    pub fn set_recent_blockhash(&mut self, blockhash: String) {
        self.recent_blockhash = Some(blockhash);
    }

    /// Get the fee payer
    pub fn fee_payer(&self) -> Option<&str> {
        self.fee_payer.as_deref()
    }

    /// Get the recent blockhash
    pub fn recent_blockhash(&self) -> Option<&str> {
        self.recent_blockhash.as_deref()
    }

    fn add_instruction(&mut self, instruction: SolanaInstruction) {
        self.instructions.push(instruction);
    }
}

impl Default for SolanaEngineConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://localhost:8899".to_string(),
            commitment: SolanaCommitment::Confirmed,
            confirmation_timeout: Duration::from_secs(30),
            cross_vm_program_id: "CrossVM11111111111111111111111111111111".to_string(),
            fee_payer: None,
            max_transaction_size: 1232, // Solana transaction size limit
        }
    }
}
