//! Reth Process Engine for Atomic Cross-VM Transactions
//!
//! This module implements the ProcessEngine trait for communicating with external
//! Reth processes. MultiVM does NOT execute transactions itself - it coordinates
//! with external Reth processes via IPC calls.
//!
//! ## Architecture
//!
//! MultiVM Coordinator → IPC → Reth Process (currently mocked)
//!                               ↓
//!                          Ethereum Execution
//!
//! The Reth process handles all Ethereum transaction execution, while MultiVM
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
use tokio::sync::{Mutex, OnceCell, RwLock};
use tracing::info;
use uuid::Uuid;
// RLP encoding will be handled manually for now

/// Ethereum Process Engine implementation (communicates with external Reth)
pub struct EthereumProcessEngine {
    /// Connection to Ethereum RPC
    rpc_client: Arc<OnceCell<EthereumRpcClient>>,
    /// Active locks for prepare phase
    active_locks: Arc<RwLock<HashMap<String, EthereumLock>>>,
    /// Pending transactions
    pending_transactions: Arc<RwLock<HashMap<String, PendingTransaction>>>,
    /// Nonce cache for accounts
    nonce_cache: Arc<RwLock<HashMap<String, u64>>>,
    /// Configuration
    config: EthereumEngineConfig,
    /// Metrics
    metrics: Arc<Mutex<EthereumEngineMetrics>>,
}

/// Pending transaction information
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct PendingTransaction {
    hash: String,
    submitted_at: SystemTime,
    confirmations: u64,
    builder: EthereumTransactionBuilder,
}

/// Ethereum signature components
#[derive(Debug, Clone)]
struct EthSignature {
    v: u64,
    r: [u8; 32],
    s: [u8; 32],
}

/// Configuration for Ethereum VM engine
#[derive(Debug, Clone)]
pub struct EthereumEngineConfig {
    /// RPC endpoint URL
    pub rpc_url: String,
    /// Chain ID
    pub chain_id: u64,
    /// Cross-VM contract address
    pub cross_vm_contract: String,
    /// Gas limit for transactions
    pub gas_limit: u64,
    /// Gas price (in wei)
    pub gas_price: u64,
    /// Use EIP-1559 pricing
    pub use_eip1559: bool,
    /// Priority fee per gas for EIP-1559
    pub priority_fee_per_gas: u64,
    /// Confirmation blocks required
    pub confirmation_blocks: u64,
    /// Maximum wait time for confirmations
    pub confirmation_timeout: Duration,
}

/// Ethereum lock record for atomic operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthereumLock {
    /// Unique lock ID
    pub lock_id: String,
    /// Account being locked
    pub account: AccountAddress,
    /// Amount locked
    pub amount: u64,
    /// Asset type
    pub asset: AssetType,
    /// Lock transaction hash
    pub lock_tx_hash: String,
    /// Lock block number
    pub lock_block: u64,
    /// Lock timestamp
    pub created_at: SystemTime,
    /// Expiration time
    pub expires_at: SystemTime,
    /// Status
    pub status: LockStatus,
    /// Contract call data
    pub call_data: Vec<u8>,
}

/// Lock status enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LockStatus {
    Pending,
    Active,
    Released,
    Expired,
}

/// Ethereum RPC client wrapper
pub struct EthereumRpcClient {
    /// Base RPC URL
    pub url: String,
    /// HTTP client
    client: reqwest::Client,
}

/// Ethereum engine metrics
#[derive(Debug, Clone, Default)]
pub struct EthereumEngineMetrics {
    /// Total operations processed
    pub operations_processed: u64,
    /// Successful operations
    pub successful_operations: u64,
    /// Failed operations
    pub failed_operations: u64,
    /// Active locks
    pub active_locks: u64,
    /// Average gas used
    pub avg_gas_used: u64,
    /// Average confirmation time
    pub avg_confirmation_time_ms: u64,
}

/// Ethereum transaction builder
#[derive(Debug, Clone)]
pub struct EthereumTransactionBuilder {
    /// From address
    pub from: String,
    /// Target contract address
    pub to: String,
    /// Call data
    pub data: Box<Vec<u8>>,
    /// Value to send (in wei)
    pub value: u64,
    /// Gas limit
    pub gas_limit: u64,
    /// Gas price (optional, will be estimated if not provided)
    pub gas_price: Option<u64>,
    /// Nonce
    pub nonce: Option<u64>,
}

/// Cross-VM contract function selectors
pub struct CrossVmContractAbi;

impl CrossVmContractAbi {
    /// Function selector for lockFunds
    pub const LOCK_FUNDS: [u8; 4] = [0x12, 0x34, 0x56, 0x78];
    /// Function selector for unlockFunds
    pub const UNLOCK_FUNDS: [u8; 4] = [0x87, 0x65, 0x43, 0x21];
    /// Function selector for completeLock
    pub const COMPLETE_LOCK: [u8; 4] = [0xab, 0xcd, 0xef, 0x12];
    /// Function selector for mintWrapped
    pub const MINT_WRAPPED: [u8; 4] = [0x34, 0x56, 0x78, 0x9a];
    /// Function selector for burnWrapped
    pub const BURN_WRAPPED: [u8; 4] = [0xbc, 0xde, 0xf1, 0x23];
}

/// Cross-VM contract function calls
#[derive(Debug, Clone)]
pub enum CrossVmContractCall {
    /// Lock funds for cross-VM transfer
    LockFunds {
        amount: u64,
        lock_id: String,
        target_vm: VmType,
        expiry_time: u64,
    },
    /// Unlock funds (abort)
    UnlockFunds { lock_id: String },
    /// Complete lock (commit)
    CompleteLock { lock_id: String, recipient: String },
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

impl EthereumProcessEngine {
    /// Create new Ethereum VM engine
    pub fn new(config: EthereumEngineConfig) -> Self {
        Self {
            rpc_client: Arc::new(OnceCell::new()),
            active_locks: Arc::new(RwLock::new(HashMap::new())),
            pending_transactions: Arc::new(RwLock::new(HashMap::new())),
            nonce_cache: Arc::new(RwLock::new(HashMap::new())),
            config,
            metrics: Arc::new(Mutex::new(EthereumEngineMetrics::default())),
        }
    }

    /// Build lock function call data
    fn build_lock_call_data(
        &self,
        amount: u64,
        lock_id: &str,
        target_vm: VmType,
        expiry_time: u64,
    ) -> MultivmResult<Vec<u8>> {
        let mut data = Vec::new();

        // Function selector (4 bytes)
        data.extend_from_slice(&CrossVmContractAbi::LOCK_FUNDS);

        // Proper ABI encoding following Ethereum standard
        // Parameters are encoded as:
        // 1. uint256 amount (32 bytes)
        // 2. bytes32 lockId (32 bytes)
        // 3. uint8 targetVm (32 bytes, padded)
        // 4. uint256 expiryTime (32 bytes)

        // 1. Encode amount as uint256 (32 bytes, big-endian)
        let mut amount_bytes = [0u8; 32];
        amount_bytes[24..32].copy_from_slice(&amount.to_be_bytes());
        data.extend_from_slice(&amount_bytes);

        // 2. Encode lock ID as bytes32 (32 bytes, left-padded with zeros)
        let lock_id_bytes = lock_id.as_bytes();
        if lock_id_bytes.len() > 32 {
            return Err(MultivmError::Configuration {
                component: "ethereum-engine".to_string(),
                message: "Lock ID too long for bytes32".to_string(),
                validation_errors: None,
            });
        }
        let mut lock_id_padded = [0u8; 32];
        lock_id_padded[..lock_id_bytes.len()].copy_from_slice(lock_id_bytes);
        data.extend_from_slice(&lock_id_padded);

        // 3. Encode target VM as uint8 (32 bytes, padded)
        let vm_id = match target_vm {
            VmType::Svm => 1u8,
            VmType::Evm => 2u8,
            VmType::MultiVm => 3u8,
        };
        let mut vm_bytes = [0u8; 32];
        vm_bytes[31] = vm_id;
        data.extend_from_slice(&vm_bytes);

        // 4. Encode expiry time as uint256 (32 bytes, big-endian)
        let mut expiry_bytes = [0u8; 32];
        expiry_bytes[24..32].copy_from_slice(&expiry_time.to_be_bytes());
        data.extend_from_slice(&expiry_bytes);

        Ok(data)
    }

    /// Build unlock function call data
    fn build_unlock_call_data(&self, lock_id: &str) -> MultivmResult<Vec<u8>> {
        let mut data = Vec::new();

        // Function selector (4 bytes)
        data.extend_from_slice(&CrossVmContractAbi::UNLOCK_FUNDS);

        // Proper ABI encoding for unlock(bytes32 lockId)
        // Parameter: bytes32 lockId (32 bytes)

        let lock_id_bytes = lock_id.as_bytes();
        if lock_id_bytes.len() > 32 {
            return Err(MultivmError::Configuration {
                component: "ethereum-engine".to_string(),
                message: "Lock ID too long for bytes32".to_string(),
                validation_errors: None,
            });
        }
        let mut lock_id_padded = [0u8; 32];
        lock_id_padded[..lock_id_bytes.len()].copy_from_slice(lock_id_bytes);
        data.extend_from_slice(&lock_id_padded);

        Ok(data)
    }

    /// Build complete lock function call data
    fn build_complete_call_data(&self, lock_id: &str, recipient: &str) -> MultivmResult<Vec<u8>> {
        let mut data = Vec::new();

        // Function selector
        data.extend_from_slice(&CrossVmContractAbi::COMPLETE_LOCK);

        // Lock ID (32 bytes, padded)
        let lock_id_bytes = lock_id.as_bytes();
        let mut lock_id_padded = [0u8; 32];
        let copy_len = std::cmp::min(lock_id_bytes.len(), 32);
        lock_id_padded[..copy_len].copy_from_slice(&lock_id_bytes[..copy_len]);
        data.extend_from_slice(&lock_id_padded);

        // Recipient address (20 bytes for Ethereum address)
        let recipient_bytes = hex::decode(recipient.trim_start_matches("0x")).map_err(|e| {
            MultivmError::Configuration {
                component: "ethereum-engine".to_string(),
                message: format!("Invalid recipient address: {}", e),
                validation_errors: None,
            }
        })?;
        if recipient_bytes.len() != 20 {
            return Err(MultivmError::Configuration {
                component: "ethereum-engine".to_string(),
                message: "Invalid Ethereum address length".to_string(),
                validation_errors: None,
            });
        }
        data.extend_from_slice(&[0u8; 12]); // Padding
        data.extend_from_slice(&recipient_bytes);

        Ok(data)
    }

    /// Extract Ethereum address from AccountAddress
    fn extract_ethereum_address(&self, account: &AccountAddress) -> MultivmResult<String> {
        match account {
            AccountAddress::Ethereum(eth_addr) => Ok(format!("0x{}", hex::encode(eth_addr.0))),
            _ => Err(MultivmError::Configuration {
                component: "ethereum-engine".to_string(),
                message: "Invalid account type for Ethereum engine".to_string(),
                validation_errors: None,
            }),
        }
    }

    /// Submit transaction to Ethereum
    async fn submit_transaction(
        &self,
        builder: EthereumTransactionBuilder,
    ) -> MultivmResult<String> {
        // Create RPC client for Ethereum node
        let client = self
            .rpc_client
            .get_or_init(|| async { EthereumRpcClient::new(self.config.rpc_url.clone()) })
            .await;

        // Build raw transaction
        let raw_tx = self.build_raw_transaction(builder.clone()).await?;

        // Send transaction via JSON-RPC
        let tx_hash =
            client
                .send_raw_transaction(&raw_tx)
                .await
                .map_err(|e| MultivmError::VmEngine {
                    vm_type: "ethereum".to_string(),
                    message: format!("Failed to submit transaction: {}", e),
                    block_info: None,
                    transaction_info: None,
                })?;

        info!("Submitted Ethereum transaction: {}", tx_hash);

        // Track pending transaction
        let mut pending_txs = self.pending_transactions.write().await;
        pending_txs.insert(
            tx_hash.clone(),
            PendingTransaction {
                hash: tx_hash.clone(),
                submitted_at: SystemTime::now(),
                confirmations: 0,
                builder,
            },
        );

        Ok(tx_hash)
    }

    /// Build raw transaction from builder
    async fn build_raw_transaction(
        &self,
        builder: EthereumTransactionBuilder,
    ) -> MultivmResult<Vec<u8>> {
        // Build transaction with proper structure
        // Note: In production, use a proper RLP encoding library like `rlp`

        // Get current nonce
        let nonce = self.get_nonce(&builder.from).await?;

        // Get current gas price if not set
        let gas_price = builder.gas_price.unwrap_or(self.get_gas_price().await?);

        // For now, create a simple transaction structure
        // In production, this would use proper RLP encoding
        let mut tx_data = Vec::new();

        // Add nonce (8 bytes)
        tx_data.extend_from_slice(&nonce.to_be_bytes());

        // Add gas price (8 bytes)
        tx_data.extend_from_slice(&gas_price.to_be_bytes());

        // Add gas limit (8 bytes)
        tx_data.extend_from_slice(&builder.gas_limit.to_be_bytes());

        // Add to address (20 bytes)
        let to_addr = hex::decode(builder.to.trim_start_matches("0x")).map_err(|e| {
            MultivmError::Configuration {
                component: "ethereum_engine".to_string(),
                message: format!("Invalid to address: {}", e),
                validation_errors: Some(vec![]),
            }
        })?;
        if to_addr.len() != 20 {
            return Err(MultivmError::Configuration {
                component: "ethereum_engine".to_string(),
                message: "Invalid address length".to_string(),
                validation_errors: Some(vec![]),
            });
        }
        tx_data.extend_from_slice(&to_addr);

        // Add value (8 bytes)
        tx_data.extend_from_slice(&builder.value.to_be_bytes());

        // Add data length (4 bytes) and data
        tx_data.extend_from_slice(&(builder.data.len() as u32).to_be_bytes());
        tx_data.extend_from_slice(&builder.data);

        // Add chain ID for EIP-155 (8 bytes)
        tx_data.extend_from_slice(&self.config.chain_id.to_be_bytes());

        // Sign transaction (placeholder)
        let signature = self.sign_transaction(&tx_data).await?;

        // Add signature (v, r, s)
        tx_data.extend_from_slice(&signature.v.to_be_bytes());
        tx_data.extend_from_slice(&signature.r);
        tx_data.extend_from_slice(&signature.s);

        Ok(tx_data)
    }

    /// Sign transaction (placeholder - in production use secure key management)
    async fn sign_transaction(&self, _tx_bytes: &[u8]) -> MultivmResult<EthSignature> {
        // In production, this would use secure key management
        // For now, return a dummy signature
        Ok(EthSignature {
            v: 27 + (self.config.chain_id * 2 + 35),
            r: [0u8; 32],
            s: [0u8; 32],
        })
    }

    /// Wait for transaction confirmation from Reth process
    async fn wait_for_confirmation(&self, _tx_hash: &str) -> MultivmResult<u64> {
        // Mock implementation - simulates polling Reth process for confirmation
        // In production, this would:
        // 1. Poll Reth process via IPC for transaction status
        // 2. Wait for sufficient confirmations
        // 3. Return final block number

        // Simulate confirmation time from external Reth process
        tokio::time::sleep(Duration::from_millis(1000)).await;

        // Return mock block number from Reth
        Ok(18_000_000)
    }

    /// Create lock record
    async fn create_lock(&self, lock: EthereumLock) -> MultivmResult<()> {
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
    async fn get_lock(&self, lock_id: &str) -> Option<EthereumLock> {
        let locks = self.active_locks.read().await;
        locks.get(lock_id).cloned()
    }

    /// Get current gas price
    async fn get_gas_price(&self) -> MultivmResult<u64> {
        // Dynamic gas price estimation from Ethereum node
        let client = self
            .rpc_client
            .get_or_init(|| async { EthereumRpcClient::new(self.config.rpc_url.clone()) })
            .await;

        // Get current base fee and priority fee
        let base_fee = client
            .get_base_fee()
            .await
            .map_err(|e| MultivmError::VmEngine {
                vm_type: "ethereum".to_string(),
                message: format!("Failed to get base fee: {}", e),
                block_info: None,
                transaction_info: None,
            })?;

        // Calculate gas price based on network conditions
        // Use EIP-1559 pricing if available
        if self.config.use_eip1559 {
            // Base fee + priority fee
            let priority_fee = self.config.priority_fee_per_gas;
            Ok(base_fee + priority_fee)
        } else {
            // Legacy gas price - use higher of base fee or configured price
            Ok(std::cmp::max(base_fee, self.config.gas_price))
        }
    }

    /// Get next nonce for account
    async fn get_nonce(&self, account: &str) -> MultivmResult<u64> {
        // Proper nonce management with caching and pending transaction tracking
        let mut nonce_cache = self.nonce_cache.write().await;

        // Check if we have a cached nonce
        if let Some(&cached_nonce) = nonce_cache.get(account) {
            // Check pending transactions to see if we need to increment
            let pending_txs = self.pending_transactions.read().await;
            let pending_count = pending_txs
                .values()
                .filter(|tx| tx.builder.from == account)
                .count() as u64;

            return Ok(cached_nonce + pending_count);
        }

        // Fetch nonce from Ethereum node
        let client = self
            .rpc_client
            .get_or_init(|| async { EthereumRpcClient::new(self.config.rpc_url.clone()) })
            .await;
        let network_nonce =
            client
                .get_transaction_count(account)
                .await
                .map_err(|e| MultivmError::VmEngine {
                    vm_type: "ethereum".to_string(),
                    message: format!("Failed to get nonce: {}", e),
                    block_info: None,
                    transaction_info: None,
                })?;

        // Cache the nonce
        nonce_cache.insert(account.to_string(), network_nonce);

        Ok(network_nonce)
    }
}

#[async_trait::async_trait]
impl crate::atomic_coordinator::ProcessEngine for EthereumProcessEngine {
    async fn prepare(&self, operations: Vec<VmOperation>) -> MultivmResult<PrepareResult> {
        info!("Preparing {} Ethereum operations", operations.len());

        // Check RPC connection health by initializing the client
        let client = self
            .rpc_client
            .get_or_init(|| async { EthereumRpcClient::new(self.config.rpc_url.clone()) })
            .await;

        if let Err(e) = client.check_connection().await {
            return Ok(PrepareResult {
                success: false,
                lock_ids: vec![],
                execution_cost: 0,
                error: Some(format!("Ethereum RPC connection failed: {}", e)),
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
                    let lock_id = format!("eth_lock_{}", Uuid::new_v4());

                    // Get account address
                    let account_addr = self.extract_ethereum_address(account)?;

                    // Build call data
                    let expiry_time = (SystemTime::now() + Duration::from_secs(3600))
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();

                    let call_data =
                        self.build_lock_call_data(*amount, &lock_id, VmType::Evm, expiry_time)?;

                    // Build transaction
                    let builder = EthereumTransactionBuilder {
                        from: account_addr.clone(),
                        to: self.config.cross_vm_contract.clone(),
                        data: Box::new(call_data.clone()),
                        value: match asset {
                            AssetType::Native => *amount,
                            _ => 0, // For tokens, value is 0
                        },
                        gas_limit: self.config.gas_limit,
                        gas_price: Some(self.get_gas_price().await?),
                        nonce: Some(self.get_nonce(&account_addr).await?),
                    };

                    // Submit transaction
                    let tx_hash = self.submit_transaction(builder).await?;

                    // Wait for confirmation
                    let block_number = self.wait_for_confirmation(&tx_hash).await?;

                    // Create lock record
                    let lock = EthereumLock {
                        lock_id: lock_id.clone(),
                        account: account.clone(),
                        amount: *amount,
                        asset: asset.clone(),
                        lock_tx_hash: tx_hash,
                        lock_block: block_number,
                        created_at: SystemTime::now(),
                        expires_at: SystemTime::now() + Duration::from_secs(3600),
                        status: LockStatus::Active,
                        call_data,
                    };

                    self.create_lock(lock).await?;
                    lock_ids.push(lock_id);

                    execution_cost += self.config.gas_limit * self.config.gas_price;
                    vm_data.insert("block_number".to_string(), block_number.to_string());
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
        info!(
            "Committing {} Ethereum locks",
            prepare_result.lock_ids.len()
        );

        let mut tx_hashes = Vec::new();
        let mut state_changes = Vec::new();

        for lock_id in &prepare_result.lock_ids {
            if let Some(lock) = self.get_lock(lock_id).await {
                // Build complete call data
                let call_data = self.build_complete_call_data(
                    lock_id,
                    "0x1234567890123456789012345678901234567890", // Mock recipient
                )?;

                // Build transaction
                let from_addr = match &lock.account {
                    AccountAddress::Ethereum(eth) => format!("0x{}", hex::encode(eth.0)),
                    _ => {
                        return Err(MultivmError::Configuration {
                            component: "ethereum-engine".to_string(),
                            message: "Expected Ethereum address".to_string(),
                            validation_errors: None,
                        })
                    }
                };
                let builder = EthereumTransactionBuilder {
                    from: from_addr,
                    to: self.config.cross_vm_contract.clone(),
                    data: Box::new(call_data),
                    value: 0, // No value for completion call
                    gas_limit: self.config.gas_limit,
                    gas_price: Some(self.get_gas_price().await?),
                    nonce: None, // Will be determined in submit_transaction
                };

                // Submit transaction
                let tx_hash = self.submit_transaction(builder).await?;
                let block_number = self.wait_for_confirmation(&tx_hash).await?;

                tx_hashes.push(tx_hash);

                // Record state change
                state_changes.push(StateChange {
                    account: lock.account.clone(),
                    asset: lock.asset.clone(),
                    balance_delta: -(lock.amount as i64),
                    block_height: block_number,
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
        info!("Aborting {} Ethereum locks", prepare_result.lock_ids.len());

        for lock_id in &prepare_result.lock_ids {
            if let Some(_lock) = self.get_lock(lock_id).await {
                // Build unlock call data
                let call_data = self.build_unlock_call_data(lock_id)?;

                // Build transaction
                let from_addr = match &_lock.account {
                    AccountAddress::Ethereum(eth) => format!("0x{}", hex::encode(eth.0)),
                    _ => {
                        return Err(MultivmError::Configuration {
                            component: "ethereum-engine".to_string(),
                            message: "Expected Ethereum address".to_string(),
                            validation_errors: None,
                        })
                    }
                };
                let builder = EthereumTransactionBuilder {
                    from: from_addr,
                    to: self.config.cross_vm_contract.clone(),
                    data: Box::new(call_data),
                    value: 0,
                    gas_limit: self.config.gas_limit,
                    gas_price: Some(self.get_gas_price().await?),
                    nonce: None, // Will be determined in submit_transaction
                };

                // Submit transaction
                let _tx_hash = self.submit_transaction(builder).await?;

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

impl EthereumRpcClient {
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
                "method": "eth_blockNumber",
                "params": [],
                "id": 1
            }))
            .send()
            .await
            .map_err(|e| multivm_common::MultivmError::Rpc {
                method: "ethereum_rpc".to_string(),
                message: format!("RPC request failed: {}", e),
                status_code: None,
            })?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(multivm_common::MultivmError::Rpc {
                method: "ethereum_rpc".to_string(),
                message: format!("RPC returned status: {}", response.status()),
                status_code: Some(response.status().as_u16()),
            })
        }
    }
}

impl EthereumRpcClient {
    /// Send raw transaction
    pub async fn send_raw_transaction(
        &self,
        raw_tx: &[u8],
    ) -> Result<String, Box<dyn std::error::Error>> {
        let params = serde_json::json!([format!("0x{}", hex::encode(raw_tx))]);
        let response = self
            .json_rpc_request("eth_sendRawTransaction", params)
            .await?;

        if let Some(result) = response.get("result").and_then(|v| v.as_str()) {
            Ok(result.to_string())
        } else {
            Err("Invalid response from eth_sendRawTransaction".into())
        }
    }

    /// Get current base fee
    pub async fn get_base_fee(&self) -> Result<u64, Box<dyn std::error::Error>> {
        let response = self
            .json_rpc_request("eth_getBlockByNumber", serde_json::json!(["latest", false]))
            .await?;

        if let Some(base_fee) = response
            .get("result")
            .and_then(|v| v.get("baseFeePerGas"))
            .and_then(|v| v.as_str())
        {
            let fee = u64::from_str_radix(base_fee.trim_start_matches("0x"), 16)?;
            Ok(fee)
        } else {
            // Fallback for pre-EIP-1559 chains
            self.get_gas_price_legacy().await
        }
    }

    /// Get legacy gas price
    async fn get_gas_price_legacy(&self) -> Result<u64, Box<dyn std::error::Error>> {
        let response = self
            .json_rpc_request("eth_gasPrice", serde_json::json!([]))
            .await?;

        if let Some(price) = response.get("result").and_then(|v| v.as_str()) {
            let gas_price = u64::from_str_radix(price.trim_start_matches("0x"), 16)?;
            Ok(gas_price)
        } else {
            Err("Failed to get gas price".into())
        }
    }

    /// Get transaction count (nonce)
    pub async fn get_transaction_count(
        &self,
        address: &str,
    ) -> Result<u64, Box<dyn std::error::Error>> {
        let params = serde_json::json!([address, "pending"]);
        let response = self
            .json_rpc_request("eth_getTransactionCount", params)
            .await?;

        if let Some(count) = response.get("result").and_then(|v| v.as_str()) {
            let nonce = u64::from_str_radix(count.trim_start_matches("0x"), 16)?;
            Ok(nonce)
        } else {
            Err("Failed to get transaction count".into())
        }
    }

    /// Make JSON-RPC request
    async fn json_rpc_request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1
        });

        let response = self.client.post(&self.url).json(&request).send().await?;

        let json: serde_json::Value = response.json().await?;
        Ok(json)
    }
}

impl Default for EthereumEngineConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://localhost:8545".to_string(),
            chain_id: 1337, // Local development chain
            cross_vm_contract: "0x1234567890123456789012345678901234567890".to_string(),
            gas_limit: 21_000,
            gas_price: 20_000_000_000, // 20 gwei
            use_eip1559: true,
            priority_fee_per_gas: 1_500_000_000, // 1.5 gwei
            confirmation_blocks: 1,
            confirmation_timeout: Duration::from_secs(60),
        }
    }
}
