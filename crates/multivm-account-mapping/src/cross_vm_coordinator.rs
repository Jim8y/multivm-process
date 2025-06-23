//! Enhanced Cross-VM Transaction Coordinator
//!
//! This module provides a high-level interface for executing cross-VM transactions
//! with automatic account mapping, asset handling, and atomic execution guarantees.
//!
//! ## Architecture
//!
//! MultiVM acts as a coordinator that orchestrates transactions across different
//! execution engines:
//! - **Reth Process**: Handles Ethereum/EVM transactions
//! - **Solana Process**: Handles Solana/SVM transactions  
//! - **MultiVM Coordinator**: Provides atomic cross-VM transaction guarantees
//!
//! The coordinator does NOT execute transactions itself, but rather:
//! 1. Coordinates atomic operations across external execution engines
//! 2. Manages account mappings between different VMs
//! 3. Ensures consistent state across all participating VMs
//! 4. Provides rollback capabilities if any step fails

use crate::{
    AccountAddress, AccountMappingLayer, AssetType, AtomicConstraints, AtomicCoordinatorConfig,
    AtomicTransactionCoordinator, CrossVmTransaction, CrossVmTxType, FeeConfiguration,
    MultivmAccountId, OperationResources, OperationType, TransactionId, TransactionMetadata,
    TransactionPriority, VmOperation, VmType,
};
use multivm_common::{MultivmError, MultivmResult};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{Mutex, RwLock};
use tracing::info;

/// Enhanced cross-VM transaction coordinator
pub struct CrossVmCoordinator {
    /// Atomic transaction coordinator
    atomic_coordinator: Arc<AtomicTransactionCoordinator>,
    /// Account mapping layer
    _account_mapping: Arc<dyn AccountMappingLayer>,
    /// Asset registry
    asset_registry: Arc<RwLock<AssetRegistry>>,
    /// Transaction history
    transaction_history: Arc<RwLock<HashMap<TransactionId, CrossVmTransactionRecord>>>,
    /// Configuration
    config: CrossVmCoordinatorConfig,
    /// Metrics
    metrics: Arc<Mutex<CrossVmCoordinatorMetrics>>,
}

/// Configuration for cross-VM coordinator
#[derive(Debug, Clone)]
pub struct CrossVmCoordinatorConfig {
    /// Maximum concurrent transactions
    pub max_concurrent_transactions: usize,
    /// Default transaction timeout
    pub default_timeout: Duration,
    /// Enable automatic retry
    pub enable_retry: bool,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Enable transaction history
    pub enable_history: bool,
    /// Asset validation settings
    pub asset_validation: AssetValidationConfig,
}

/// Asset validation configuration
#[derive(Debug, Clone)]
pub struct AssetValidationConfig {
    /// Minimum transfer amount
    pub min_transfer_amount: u64,
    /// Maximum transfer amount
    pub max_transfer_amount: u64,
    /// Required confirmations per asset type
    pub required_confirmations: HashMap<AssetType, u32>,
    /// Enable balance verification
    pub verify_balances: bool,
}

/// Asset registry for managing cross-VM assets
pub struct AssetRegistry {
    /// Registered assets
    assets: HashMap<String, RegisteredAsset>,
    /// Supported VM pairs for each asset
    vm_support: HashMap<String, Vec<VmPair>>,
}

/// Registered asset information
#[derive(Debug, Clone)]
pub struct RegisteredAsset {
    /// Asset identifier
    pub id: String,
    /// Asset name
    pub name: String,
    /// Asset type
    pub asset_type: AssetType,
    /// Decimals
    pub decimals: u8,
    /// Supported VMs
    pub supported_vms: Vec<VmType>,
    /// Contract addresses per VM
    pub contracts: HashMap<VmType, String>,
    /// Transfer limits
    pub limits: TransferLimits,
}

/// VM pair for cross-VM transfers
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VmPair {
    pub source: VmType,
    pub target: VmType,
}

/// Transfer limits for an asset
#[derive(Debug, Clone)]
pub struct TransferLimits {
    /// Minimum transfer amount
    pub min_amount: u64,
    /// Maximum transfer amount
    pub max_amount: u64,
    /// Daily transfer limit
    pub daily_limit: u64,
    /// Per-transaction fee
    pub transfer_fee: u64,
}

/// Cross-VM transaction record
#[derive(Debug, Clone)]
pub struct CrossVmTransactionRecord {
    /// Transaction ID
    pub id: TransactionId,
    /// Transaction type
    pub tx_type: CrossVmTxType,
    /// Source account
    pub source_account: MultivmAccountId,
    /// Target account
    pub target_account: MultivmAccountId,
    /// Asset being transferred
    pub asset: RegisteredAsset,
    /// Amount
    pub amount: u64,
    /// Status
    pub status: CrossVmTransactionStatus,
    /// Created timestamp
    pub created_at: SystemTime,
    /// Completed timestamp
    pub completed_at: Option<SystemTime>,
    /// Error message
    pub error: Option<String>,
    /// Transaction hashes per VM
    pub tx_hashes: HashMap<VmType, String>,
}

/// Cross-VM transaction status
#[derive(Debug, Clone)]
pub enum CrossVmTransactionStatus {
    Pending,
    Validating,
    Preparing,
    Prepared,
    Executing,
    Completed,
    Failed,
    Cancelled,
}

/// Cross-VM coordinator metrics
#[derive(Debug, Clone, Default)]
pub struct CrossVmCoordinatorMetrics {
    /// Total transactions processed
    pub total_transactions: u64,
    /// Successful transactions
    pub successful_transactions: u64,
    /// Failed transactions
    pub failed_transactions: u64,
    /// Average execution time
    pub avg_execution_time_ms: u64,
    /// Total volume transferred
    pub total_volume: u64,
    /// Active transactions
    pub active_transactions: u64,
}

/// Simple cross-VM transfer request
#[derive(Debug, Clone)]
pub struct SimpleCrossVmTransfer {
    /// Source account
    pub from: MultivmAccountId,
    /// Target account  
    pub to: MultivmAccountId,
    /// Amount to transfer
    pub amount: u64,
    /// Asset to transfer
    pub asset_id: String,
    /// Optional memo
    pub memo: Option<String>,
    /// Priority
    pub priority: TransactionPriority,
    /// Maximum acceptable fee
    pub max_fee: u64,
}

/// Cross-VM swap request
#[derive(Debug, Clone)]
pub struct CrossVmSwapRequest {
    /// Party A account
    pub party_a: MultivmAccountId,
    /// Party B account
    pub party_b: MultivmAccountId,
    /// Asset A
    pub asset_a: String,
    /// Amount A
    pub amount_a: u64,
    /// Asset B
    pub asset_b: String,
    /// Amount B
    pub amount_b: u64,
    /// Swap expiration
    pub expires_at: SystemTime,
    /// Maximum slippage tolerance
    pub max_slippage: f64,
}

impl CrossVmCoordinator {
    /// Create new cross-VM coordinator
    pub async fn new(
        config: CrossVmCoordinatorConfig,
        account_mapping: Arc<dyn AccountMappingLayer>,
    ) -> MultivmResult<Self> {
        // Create atomic coordinator
        let atomic_config = AtomicCoordinatorConfig {
            max_concurrent_transactions: config.max_concurrent_transactions,
            prepare_timeout: Duration::from_secs(30),
            commit_timeout: Duration::from_secs(60),
            enable_retry: config.enable_retry,
            max_retries: config.max_retries,
            retry_backoff: Duration::from_secs(5),
        };

        let atomic_coordinator = Arc::new(AtomicTransactionCoordinator::new(atomic_config));

        // Create asset registry
        let asset_registry = Arc::new(RwLock::new(AssetRegistry::new()));

        // Initialize default assets
        Self::initialize_default_assets(&asset_registry).await?;

        Ok(Self {
            atomic_coordinator,
            _account_mapping: account_mapping,
            asset_registry,
            transaction_history: Arc::new(RwLock::new(HashMap::new())),
            config,
            metrics: Arc::new(Mutex::new(CrossVmCoordinatorMetrics::default())),
        })
    }

    /// Execute a simple cross-VM transfer
    pub async fn execute_transfer(
        &self,
        transfer: SimpleCrossVmTransfer,
    ) -> MultivmResult<TransactionId> {
        info!(
            "Executing cross-VM transfer: {:?} -> {:?}, amount: {}",
            transfer.from, transfer.to, transfer.amount
        );

        // Validate transfer
        self.validate_transfer(&transfer).await?;

        // Get asset information
        let asset = self.get_asset(&transfer.asset_id).await?;

        // Determine source and target VMs
        let (source_vm, target_vm) = self.determine_vms(&transfer.from, &transfer.to).await?;

        // Build cross-VM transaction
        let cross_vm_tx = self
            .build_transfer_transaction(&transfer, &asset, source_vm, target_vm)
            .await?;

        // Execute atomic transaction
        let tx_id = self
            .atomic_coordinator
            .execute_atomic_transaction(cross_vm_tx)
            .await?;

        // Record transaction
        if self.config.enable_history {
            self.record_transaction(&tx_id, &transfer, &asset).await?;
        }

        // Update metrics
        self.update_metrics_for_transfer(&transfer).await;

        Ok(tx_id)
    }

    /// Execute a cross-VM atomic swap
    pub async fn execute_swap(&self, swap: CrossVmSwapRequest) -> MultivmResult<TransactionId> {
        info!(
            "Executing cross-VM swap: {} <-> {}",
            swap.asset_a, swap.asset_b
        );

        // Validate swap
        self.validate_swap(&swap).await?;

        // Get asset information
        let asset_a = self.get_asset(&swap.asset_a).await?;
        let asset_b = self.get_asset(&swap.asset_b).await?;

        // Build atomic swap transaction
        let cross_vm_tx = self
            .build_swap_transaction(&swap, &asset_a, &asset_b)
            .await?;

        // Execute atomic transaction
        let tx_id = self
            .atomic_coordinator
            .execute_atomic_transaction(cross_vm_tx)
            .await?;

        Ok(tx_id)
    }

    /// Get transaction status
    pub async fn get_transaction_status(
        &self,
        tx_id: &TransactionId,
    ) -> MultivmResult<CrossVmTransactionStatus> {
        // Check atomic coordinator status
        let atomic_status = self
            .atomic_coordinator
            .get_transaction_status(tx_id)
            .await?;

        // Map to cross-VM status
        let status = match atomic_status {
            crate::TransactionPhase::Validating => CrossVmTransactionStatus::Validating,
            crate::TransactionPhase::Preparing => CrossVmTransactionStatus::Preparing,
            crate::TransactionPhase::Prepared => CrossVmTransactionStatus::Prepared,
            crate::TransactionPhase::Committing => CrossVmTransactionStatus::Executing,
            crate::TransactionPhase::Committed => CrossVmTransactionStatus::Completed,
            crate::TransactionPhase::Aborted => CrossVmTransactionStatus::Failed,
            crate::TransactionPhase::Failed => CrossVmTransactionStatus::Failed,
            _ => CrossVmTransactionStatus::Pending,
        };

        Ok(status)
    }

    /// Validate transfer request
    async fn validate_transfer(&self, transfer: &SimpleCrossVmTransfer) -> MultivmResult<()> {
        // Check amount limits
        if transfer.amount < self.config.asset_validation.min_transfer_amount {
            return Err(MultivmError::Configuration(
                "Transfer amount below minimum".to_string(),
            ));
        }

        if transfer.amount > self.config.asset_validation.max_transfer_amount {
            return Err(MultivmError::Configuration(
                "Transfer amount above maximum".to_string(),
            ));
        }

        // Validate accounts exist - for now skip actual validation
        // TODO: Implement proper account binding validation with actual account addresses
        // let _source_binding = self.account_mapping.get_account_binding(&source_account).await?;
        // let _target_binding = self.account_mapping.get_account_binding(&target_account).await?;

        Ok(())
    }

    /// Validate swap request
    async fn validate_swap(&self, swap: &CrossVmSwapRequest) -> MultivmResult<()> {
        // Check expiration
        if SystemTime::now() > swap.expires_at {
            return Err(MultivmError::Configuration("Swap has expired".to_string()));
        }

        // Check slippage tolerance
        if swap.max_slippage < 0.0 || swap.max_slippage > 1.0 {
            return Err(MultivmError::Configuration(
                "Invalid slippage tolerance".to_string(),
            ));
        }

        Ok(())
    }

    /// Determine VMs for accounts
    async fn determine_vms(
        &self,
        _from: &MultivmAccountId,
        _to: &MultivmAccountId,
    ) -> MultivmResult<(VmType, VmType)> {
        // This is simplified - in practice, we'd look up the actual account bindings
        // to determine which VMs the accounts are bound to
        Ok((VmType::Evm, VmType::Svm))
    }

    /// Build transfer transaction
    async fn build_transfer_transaction(
        &self,
        transfer: &SimpleCrossVmTransfer,
        asset: &RegisteredAsset,
        source_vm: VmType,
        target_vm: VmType,
    ) -> MultivmResult<CrossVmTransaction> {
        // Create source operation (lock funds)
        let source_op = VmOperation {
            vm: source_vm,
            operation: OperationType::Lock {
                account: AccountAddress::Ethereum(crate::EthereumAddress([0u8; 20])), // Simplified
                amount: transfer.amount,
                asset: asset.asset_type.clone(),
            },
            resources: OperationResources {
                compute_units: 10000,
                fee_estimate: asset.limits.transfer_fee,
                required_balance: transfer.amount,
            },
            dependencies: vec![],
        };

        // Create target operation (mint/transfer)
        let target_op = VmOperation {
            vm: target_vm,
            operation: OperationType::Mint {
                to: AccountAddress::Solana(crate::SolanaAddress([0u8; 32])), // Simplified
                amount: transfer.amount,
                asset: asset.asset_type.clone(),
            },
            resources: OperationResources {
                compute_units: 5000,
                fee_estimate: asset.limits.transfer_fee / 2,
                required_balance: 0,
            },
            dependencies: vec![],
        };

        Ok(CrossVmTransaction {
            tx_type: CrossVmTxType::Transfer {
                from: transfer.from.clone(),
                to: transfer.to.clone(),
                amount: transfer.amount,
                asset: asset.asset_type.clone(),
            },
            source_ops: vec![source_op],
            target_ops: vec![target_op],
            constraints: AtomicConstraints {
                max_execution_time: self.config.default_timeout,
                confirmations: HashMap::new(),
                max_slippage: None,
                deadline: Some(SystemTime::now() + self.config.default_timeout),
            },
            metadata: TransactionMetadata {
                memo: transfer.memo.clone(),
                tags: vec!["cross_vm_transfer".to_string()],
                priority: transfer.priority.clone(),
                fee_config: FeeConfiguration {
                    max_total_fee: transfer.max_fee,
                    fee_distribution: HashMap::new(),
                    fee_asset: AssetType::Native,
                },
            },
        })
    }

    /// Build swap transaction
    async fn build_swap_transaction(
        &self,
        _swap: &CrossVmSwapRequest,
        _asset_a: &RegisteredAsset,
        _asset_b: &RegisteredAsset,
    ) -> MultivmResult<CrossVmTransaction> {
        // Simplified implementation
        // In practice, this would build complex atomic swap operations
        Err(MultivmError::UnsupportedOperation(
            "Atomic swaps not yet implemented".to_string(),
        ))
    }

    /// Get asset by ID
    async fn get_asset(&self, asset_id: &str) -> MultivmResult<RegisteredAsset> {
        let registry = self.asset_registry.read().await;
        registry
            .assets
            .get(asset_id)
            .cloned()
            .ok_or_else(|| MultivmError::Configuration(format!("Asset not found: {}", asset_id)))
    }

    /// Record transaction in history
    async fn record_transaction(
        &self,
        tx_id: &TransactionId,
        transfer: &SimpleCrossVmTransfer,
        asset: &RegisteredAsset,
    ) -> MultivmResult<()> {
        let record = CrossVmTransactionRecord {
            id: tx_id.clone(),
            tx_type: CrossVmTxType::Transfer {
                from: transfer.from.clone(),
                to: transfer.to.clone(),
                amount: transfer.amount,
                asset: asset.asset_type.clone(),
            },
            source_account: transfer.from.clone(),
            target_account: transfer.to.clone(),
            asset: asset.clone(),
            amount: transfer.amount,
            status: CrossVmTransactionStatus::Pending,
            created_at: SystemTime::now(),
            completed_at: None,
            error: None,
            tx_hashes: HashMap::new(),
        };

        self.transaction_history
            .write()
            .await
            .insert(tx_id.clone(), record);
        Ok(())
    }

    /// Update metrics for transfer
    async fn update_metrics_for_transfer(&self, transfer: &SimpleCrossVmTransfer) {
        let mut metrics = self.metrics.lock().await;
        metrics.total_transactions += 1;
        metrics.total_volume += transfer.amount;
        metrics.active_transactions += 1;
    }

    /// Initialize default assets
    async fn initialize_default_assets(registry: &Arc<RwLock<AssetRegistry>>) -> MultivmResult<()> {
        let mut reg = registry.write().await;

        // Add native ETH (coordinated via external Reth process)
        reg.register_asset(RegisteredAsset {
            id: "ETH".to_string(),
            name: "Ethereum".to_string(),
            asset_type: AssetType::Native,
            decimals: 18,
            supported_vms: vec![VmType::Evm], // Coordinated via external Reth process
            contracts: HashMap::new(),
            limits: TransferLimits {
                min_amount: 1000,                           // 0.000000000001 ETH
                max_amount: 1_000_000_000_000_000_000,      // 1 ETH
                daily_limit: 10_000_000_000_000_000_000u64, // 10 ETH
                transfer_fee: 100_000,                      // Base fee
            },
        })?;

        // Add native SOL (coordinated via external Solana process)
        reg.register_asset(RegisteredAsset {
            id: "SOL".to_string(),
            name: "Solana".to_string(),
            asset_type: AssetType::Native,
            decimals: 9,
            supported_vms: vec![VmType::Svm], // Coordinated via external Solana process
            contracts: HashMap::new(),
            limits: TransferLimits {
                min_amount: 1000,            // 0.000001 SOL
                max_amount: 1_000_000_000,   // 1 SOL
                daily_limit: 10_000_000_000, // 10 SOL
                transfer_fee: 5000,          // 0.000005 SOL
            },
        })?;

        Ok(())
    }

    /// Get coordinator metrics
    pub async fn get_metrics(&self) -> CrossVmCoordinatorMetrics {
        self.metrics.lock().await.clone()
    }
}

impl AssetRegistry {
    fn new() -> Self {
        Self {
            assets: HashMap::new(),
            vm_support: HashMap::new(),
        }
    }

    fn register_asset(&mut self, asset: RegisteredAsset) -> MultivmResult<()> {
        // Add VM support mappings
        for source_vm in &asset.supported_vms {
            for target_vm in &asset.supported_vms {
                if source_vm != target_vm {
                    let pair = VmPair {
                        source: *source_vm,
                        target: *target_vm,
                    };
                    self.vm_support
                        .entry(asset.id.clone())
                        .or_default()
                        .push(pair);
                }
            }
        }

        self.assets.insert(asset.id.clone(), asset);
        Ok(())
    }
}

impl Default for CrossVmCoordinatorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_transactions: 100,
            default_timeout: Duration::from_secs(300), // 5 minutes
            enable_retry: true,
            max_retries: 3,
            enable_history: true,
            asset_validation: AssetValidationConfig {
                min_transfer_amount: 1,
                max_transfer_amount: u64::MAX,
                required_confirmations: HashMap::new(),
                verify_balances: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemoryStorage;

    #[tokio::test]
    async fn test_cross_vm_coordinator_creation() {
        let storage = Arc::new(MemoryStorage::new());
        let account_mapping = storage.clone();
        let config = CrossVmCoordinatorConfig::default();

        let coordinator = CrossVmCoordinator::new(config, account_mapping).await;
        assert!(coordinator.is_ok());
    }

    #[tokio::test]
    async fn test_asset_registry() {
        let registry = Arc::new(RwLock::new(AssetRegistry::new()));
        assert!(CrossVmCoordinator::initialize_default_assets(&registry)
            .await
            .is_ok());

        let reg = registry.read().await;
        assert!(reg.assets.contains_key("ETH"));
        assert!(reg.assets.contains_key("SOL"));
    }
}
