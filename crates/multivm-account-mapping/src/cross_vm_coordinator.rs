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

        // Get bound addresses for source and target accounts
        let source_addresses = self
            ._account_mapping
            .get_bound_addresses(&transfer.from)
            .await
            .map_err(|_| {
                MultivmError::Configuration(format!("Source account {} not found", transfer.from))
            })?;

        let target_addresses = self
            ._account_mapping
            .get_bound_addresses(&transfer.to)
            .await
            .map_err(|_| {
                MultivmError::Configuration(format!("Target account {} not found", transfer.to))
            })?;

        // Verify accounts have bindings
        if source_addresses.is_empty() {
            return Err(MultivmError::Configuration(format!(
                "Source account {} has no bound addresses",
                transfer.from
            )));
        }

        if target_addresses.is_empty() {
            return Err(MultivmError::Configuration(format!(
                "Target account {} has no bound addresses",
                transfer.to
            )));
        }

        // Additional validation: check if accounts have necessary VM support
        let asset = self.get_asset(&transfer.asset_id).await?;

        // Check if source has an address on a VM that supports the asset
        let has_source_vm_support = source_addresses.iter().any(|addr| {
            let vm = match addr {
                AccountAddress::Ethereum(_) => VmType::Evm,
                AccountAddress::Solana(_) => VmType::Svm,
            };
            asset.supported_vms.contains(&vm)
        });

        // Check if target has an address on a VM that supports the asset
        let has_target_vm_support = target_addresses.iter().any(|addr| {
            let vm = match addr {
                AccountAddress::Ethereum(_) => VmType::Evm,
                AccountAddress::Solana(_) => VmType::Svm,
            };
            asset.supported_vms.contains(&vm)
        });

        if !has_source_vm_support {
            return Err(MultivmError::Configuration(format!(
                "Source account {} does not support asset {} on any bound VM",
                transfer.from, transfer.asset_id
            )));
        }

        if !has_target_vm_support {
            return Err(MultivmError::Configuration(format!(
                "Target account {} does not support asset {} on any bound VM",
                transfer.to, transfer.asset_id
            )));
        }

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
        from: &MultivmAccountId,
        to: &MultivmAccountId,
    ) -> MultivmResult<(VmType, VmType)> {
        // Get bound addresses to determine which VMs are involved
        let from_addresses = self
            ._account_mapping
            .get_bound_addresses(from)
            .await
            .map_err(|_| {
                MultivmError::Configuration(format!("Failed to get addresses for account {}", from))
            })?;

        let to_addresses = self
            ._account_mapping
            .get_bound_addresses(to)
            .await
            .map_err(|_| {
                MultivmError::Configuration(format!("Failed to get addresses for account {}", to))
            })?;

        // Find the first supported VM for each account
        let source_vm = from_addresses
            .iter()
            .find_map(|addr| match addr {
                AccountAddress::Ethereum(_) => Some(VmType::Evm),
                AccountAddress::Solana(_) => Some(VmType::Svm),
            })
            .ok_or_else(|| {
                MultivmError::Configuration("Source account has no valid VM bindings".to_string())
            })?;

        let target_vm = to_addresses
            .iter()
            .find_map(|addr| match addr {
                AccountAddress::Ethereum(_) => Some(VmType::Evm),
                AccountAddress::Solana(_) => Some(VmType::Svm),
            })
            .ok_or_else(|| {
                MultivmError::Configuration("Target account has no valid VM bindings".to_string())
            })?;

        Ok((source_vm, target_vm))
    }

    /// Build transfer transaction
    async fn build_transfer_transaction(
        &self,
        transfer: &SimpleCrossVmTransfer,
        asset: &RegisteredAsset,
        source_vm: VmType,
        target_vm: VmType,
    ) -> MultivmResult<CrossVmTransaction> {
        // Get actual addresses for the accounts
        let source_addresses = self
            ._account_mapping
            .get_bound_addresses(&transfer.from)
            .await?;

        let target_addresses = self
            ._account_mapping
            .get_bound_addresses(&transfer.to)
            .await?;

        // Find the appropriate address for source VM
        let source_address = source_addresses
            .iter()
            .find(|addr| match (addr, source_vm) {
                (AccountAddress::Ethereum(_), VmType::Evm) => true,
                (AccountAddress::Solana(_), VmType::Svm) => true,
                _ => false,
            })
            .ok_or_else(|| {
                MultivmError::Configuration(format!(
                    "No {:?} address found for source account",
                    source_vm
                ))
            })?
            .clone();

        // Find the appropriate address for target VM
        let target_address = target_addresses
            .iter()
            .find(|addr| match (addr, target_vm) {
                (AccountAddress::Ethereum(_), VmType::Evm) => true,
                (AccountAddress::Solana(_), VmType::Svm) => true,
                _ => false,
            })
            .ok_or_else(|| {
                MultivmError::Configuration(format!(
                    "No {:?} address found for target account",
                    target_vm
                ))
            })?
            .clone();

        // Create source operation (lock funds)
        let source_op = VmOperation {
            vm: source_vm,
            operation: OperationType::Lock {
                account: source_address,
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
                to: target_address,
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
        swap: &CrossVmSwapRequest,
        asset_a: &RegisteredAsset,
        asset_b: &RegisteredAsset,
    ) -> MultivmResult<CrossVmTransaction> {
        // Get addresses for both parties
        let party_a_addresses = self
            ._account_mapping
            .get_bound_addresses(&swap.party_a)
            .await?;
        let party_b_addresses = self
            ._account_mapping
            .get_bound_addresses(&swap.party_b)
            .await?;

        // Determine VMs for each asset
        let (asset_a_vm, asset_b_vm) = (
            asset_a.supported_vms.first().ok_or_else(|| {
                MultivmError::Configuration("Asset A has no supported VMs".to_string())
            })?,
            asset_b.supported_vms.first().ok_or_else(|| {
                MultivmError::Configuration("Asset B has no supported VMs".to_string())
            })?,
        );

        // Find appropriate addresses
        let party_a_addr_for_asset_a = party_a_addresses
            .iter()
            .find(|addr| match (addr, asset_a_vm) {
                (AccountAddress::Ethereum(_), VmType::Evm) => true,
                (AccountAddress::Solana(_), VmType::Svm) => true,
                _ => false,
            })
            .ok_or_else(|| {
                MultivmError::Configuration(format!(
                    "Party A has no {:?} address for asset A",
                    asset_a_vm
                ))
            })?
            .clone();

        let party_b_addr_for_asset_a = party_b_addresses
            .iter()
            .find(|addr| match (addr, asset_a_vm) {
                (AccountAddress::Ethereum(_), VmType::Evm) => true,
                (AccountAddress::Solana(_), VmType::Svm) => true,
                _ => false,
            })
            .ok_or_else(|| {
                MultivmError::Configuration(format!(
                    "Party B has no {:?} address for asset A",
                    asset_a_vm
                ))
            })?
            .clone();

        let party_a_addr_for_asset_b = party_a_addresses
            .iter()
            .find(|addr| match (addr, asset_b_vm) {
                (AccountAddress::Ethereum(_), VmType::Evm) => true,
                (AccountAddress::Solana(_), VmType::Svm) => true,
                _ => false,
            })
            .ok_or_else(|| {
                MultivmError::Configuration(format!(
                    "Party A has no {:?} address for asset B",
                    asset_b_vm
                ))
            })?
            .clone();

        let party_b_addr_for_asset_b = party_b_addresses
            .iter()
            .find(|addr| match (addr, asset_b_vm) {
                (AccountAddress::Ethereum(_), VmType::Evm) => true,
                (AccountAddress::Solana(_), VmType::Svm) => true,
                _ => false,
            })
            .ok_or_else(|| {
                MultivmError::Configuration(format!(
                    "Party B has no {:?} address for asset B",
                    asset_b_vm
                ))
            })?
            .clone();

        // Create atomic swap operations
        let operations = vec![
            // Lock A's asset A
            VmOperation {
                vm: *asset_a_vm,
                operation: OperationType::Lock {
                    account: party_a_addr_for_asset_a.clone(),
                    amount: swap.amount_a,
                    asset: asset_a.asset_type.clone(),
                },
                resources: OperationResources {
                    compute_units: 15000,
                    fee_estimate: asset_a.limits.transfer_fee,
                    required_balance: swap.amount_a,
                },
                dependencies: vec![],
            },
            // Lock B's asset B
            VmOperation {
                vm: *asset_b_vm,
                operation: OperationType::Lock {
                    account: party_b_addr_for_asset_b.clone(),
                    amount: swap.amount_b,
                    asset: asset_b.asset_type.clone(),
                },
                resources: OperationResources {
                    compute_units: 15000,
                    fee_estimate: asset_b.limits.transfer_fee,
                    required_balance: swap.amount_b,
                },
                dependencies: vec![],
            },
            // Transfer asset A from A to B
            VmOperation {
                vm: *asset_a_vm,
                operation: OperationType::Transfer {
                    from: party_a_addr_for_asset_a,
                    to: party_b_addr_for_asset_a,
                    amount: swap.amount_a,
                    asset: asset_a.asset_type.clone(),
                },
                resources: OperationResources {
                    compute_units: 10000,
                    fee_estimate: asset_a.limits.transfer_fee,
                    required_balance: 0,
                },
                dependencies: vec![],
            },
            // Transfer asset B from B to A
            VmOperation {
                vm: *asset_b_vm,
                operation: OperationType::Transfer {
                    from: party_b_addr_for_asset_b,
                    to: party_a_addr_for_asset_b,
                    amount: swap.amount_b,
                    asset: asset_b.asset_type.clone(),
                },
                resources: OperationResources {
                    compute_units: 10000,
                    fee_estimate: asset_b.limits.transfer_fee,
                    required_balance: 0,
                },
                dependencies: vec![],
            },
        ];

        Ok(CrossVmTransaction {
            tx_type: CrossVmTxType::AtomicSwap {
                party_a: swap.party_a.clone(),
                party_b: swap.party_b.clone(),
                asset_a: asset_a.asset_type.clone(),
                amount_a: swap.amount_a,
                asset_b: asset_b.asset_type.clone(),
                amount_b: swap.amount_b,
            },
            source_ops: operations
                .iter()
                .filter(|op| matches!(op.operation, OperationType::Lock { .. }))
                .cloned()
                .collect(),
            target_ops: operations
                .iter()
                .filter(|op| matches!(op.operation, OperationType::Transfer { .. }))
                .cloned()
                .collect(),
            constraints: AtomicConstraints {
                max_execution_time: swap
                    .expires_at
                    .duration_since(SystemTime::now())
                    .unwrap_or(self.config.default_timeout),
                confirmations: {
                    let mut confirmations = HashMap::new();
                    confirmations.insert(*asset_a_vm, asset_a.limits.min_amount as u32);
                    confirmations.insert(*asset_b_vm, asset_b.limits.min_amount as u32);
                    confirmations
                },
                max_slippage: Some(swap.max_slippage),
                deadline: Some(swap.expires_at),
            },
            metadata: TransactionMetadata {
                memo: Some(format!(
                    "Atomic swap: {} {} for {} {}",
                    swap.amount_a, asset_a.name, swap.amount_b, asset_b.name
                )),
                tags: vec!["atomic_swap".to_string()],
                priority: TransactionPriority::High,
                fee_config: FeeConfiguration {
                    max_total_fee: asset_a.limits.transfer_fee * 2
                        + asset_b.limits.transfer_fee * 2,
                    fee_distribution: {
                        let mut distribution = HashMap::new();
                        distribution.insert(*asset_a_vm, asset_a.limits.transfer_fee);
                        distribution.insert(*asset_b_vm, asset_b.limits.transfer_fee);
                        distribution
                    },
                    fee_asset: AssetType::Native,
                },
            },
        })
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
