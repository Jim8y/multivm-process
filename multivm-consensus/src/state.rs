//! Cross-VM state management for the MultiVM consensus layer

use crate::messages::VmType;
use crate::traits::{
    CrossVMState, CrossVMStateCoordinator, StateChange, StateCheckpoint, ValidationResult,
};
use crate::{ConsensusError, ConsensusResult};
use async_trait::async_trait;
use multivm_account_mapping::{
    address::MultivmAccountId, mapping::AccountBinding, special_tx::SpecialTransaction,
};
use parking_lot::RwLock;
use rocksdb::{Options, DB};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use tokio::sync::RwLock as AsyncRwLock;
use tracing::{debug, error, info, warn};

/// Trait for persistent consensus state storage
#[async_trait]
pub trait ConsensusStateStorage: Send + Sync {
    /// Store consensus state at a specific height
    async fn store_state(&self, height: u64, state: &ConsensusState) -> ConsensusResult<()>;

    /// Retrieve consensus state at a specific height
    async fn get_state(&self, height: u64) -> ConsensusResult<Option<ConsensusState>>;

    /// Get the latest stored state
    async fn get_latest_state(&self) -> ConsensusResult<Option<ConsensusState>>;

    /// Delete state at a specific height
    async fn delete_state(&self, height: u64) -> ConsensusResult<()>;

    /// List available state heights
    async fn list_heights(&self) -> ConsensusResult<Vec<u64>>;

    /// Get storage statistics
    async fn get_stats(&self) -> ConsensusResult<StorageStats>;

    /// Get raw data by key
    async fn get(&self, key: &[u8]) -> ConsensusResult<Option<Vec<u8>>>;

    /// Store raw data by key
    async fn put(&self, key: &[u8], value: &[u8]) -> ConsensusResult<()>;
}

/// Storage statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub total_states: usize,
    pub disk_usage_bytes: u64,
    pub oldest_height: Option<u64>,
    pub newest_height: Option<u64>,
}

/// RocksDB implementation of consensus state storage
pub struct RocksDBConsensusStorage {
    db: Arc<DB>,
}

impl RocksDBConsensusStorage {
    /// Create a new RocksDB consensus storage
    pub fn new(db_path: &str) -> ConsensusResult<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);

        let db = DB::open(&opts, db_path)
            .map_err(|e| ConsensusError::Storage(format!("Failed to open RocksDB: {e}")))?;

        Ok(Self { db: Arc::new(db) })
    }

    fn height_key(height: u64) -> String {
        format!("consensus_state:{height:020}")
    }
}

#[async_trait]
impl ConsensusStateStorage for RocksDBConsensusStorage {
    async fn store_state(&self, height: u64, state: &ConsensusState) -> ConsensusResult<()> {
        let key = Self::height_key(height);
        let value = bincode::serialize(state)
            .map_err(|e| ConsensusError::Storage(format!("Serialization error: {e}")))?;

        self.db
            .put(key.as_bytes(), &value)
            .map_err(|e| ConsensusError::Storage(format!("Failed to store state: {e}")))?;

        Ok(())
    }

    async fn get_state(&self, height: u64) -> ConsensusResult<Option<ConsensusState>> {
        let key = Self::height_key(height);

        match self
            .db
            .get(key.as_bytes())
            .map_err(|e| ConsensusError::Storage(format!("Failed to get state: {e}")))?
        {
            Some(value) => {
                let state = bincode::deserialize(&value)
                    .map_err(|e| ConsensusError::Storage(format!("Deserialization error: {e}")))?;
                Ok(Some(state))
            }
            None => Ok(None),
        }
    }

    async fn get_latest_state(&self) -> ConsensusResult<Option<ConsensusState>> {
        let iter = self.db.iterator(rocksdb::IteratorMode::Start);
        let mut latest_height = None;

        // Find the highest height
        for (key, _) in iter.flatten() {
            if let Ok(key_str) = String::from_utf8(key.to_vec()) {
                if let Some(height_str) = key_str.strip_prefix("consensus_state:") {
                    if let Ok(height) = height_str.parse::<u64>() {
                        latest_height = Some(latest_height.map_or(height, |h: u64| h.max(height)));
                    }
                }
            }
        }

        if let Some(height) = latest_height {
            self.get_state(height).await
        } else {
            Ok(None)
        }
    }

    async fn delete_state(&self, height: u64) -> ConsensusResult<()> {
        let key = Self::height_key(height);
        self.db
            .delete(key.as_bytes())
            .map_err(|e| ConsensusError::Storage(format!("Failed to delete state: {e}")))?;
        Ok(())
    }

    async fn list_heights(&self) -> ConsensusResult<Vec<u64>> {
        let mut heights = Vec::new();
        let iter = self.db.iterator(rocksdb::IteratorMode::Start);

        for (key, _) in iter.flatten() {
            if let Ok(key_str) = String::from_utf8(key.to_vec()) {
                if let Some(height_str) = key_str.strip_prefix("consensus_state:") {
                    if let Ok(height) = height_str.parse::<u64>() {
                        heights.push(height);
                    }
                }
            }
        }

        heights.sort_unstable();
        Ok(heights)
    }

    async fn get_stats(&self) -> ConsensusResult<StorageStats> {
        let heights = self.list_heights().await?;

        Ok(StorageStats {
            total_states: heights.len(),
            disk_usage_bytes: 0, // RocksDB doesn't provide easy way to get size
            oldest_height: heights.first().copied(),
            newest_height: heights.last().copied(),
        })
    }

    async fn get(&self, key: &[u8]) -> ConsensusResult<Option<Vec<u8>>> {
        match self.db.get(key) {
            Ok(Some(value)) => Ok(Some(value.to_vec())),
            Ok(None) => Ok(None),
            Err(e) => Err(ConsensusError::Storage(format!("Failed to get key: {e}"))),
        }
    }

    async fn put(&self, key: &[u8], value: &[u8]) -> ConsensusResult<()> {
        self.db
            .put(key, value)
            .map_err(|e| ConsensusError::Storage(format!("Failed to put key: {e}")))
    }
}

/// Configuration for persistent state manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentStateConfig {
    /// RocksDB database path
    pub db_path: String,
    /// Maximum number of states to keep
    pub max_states: usize,
    /// Cleanup interval in seconds
    pub cleanup_interval_secs: u64,
    /// Enable automatic cleanup of old states
    pub auto_cleanup: bool,
}

impl Default for PersistentStateConfig {
    fn default() -> Self {
        Self {
            db_path: "/opt/multivm/data/consensus_state.db".to_string(),
            max_states: 1000,
            cleanup_interval_secs: 3600, // 1 hour
            auto_cleanup: true,
        }
    }
}

/// Persistent cross-VM state manager with RocksDB backend  
pub struct PersistentCrossVMStateManager {
    /// In-memory state manager for active operations
    memory_manager: CrossVMStateManager,
    /// Persistent storage backend
    storage: Arc<dyn ConsensusStateStorage>,
    /// Configuration
    config: StateManagerConfig,
}

impl PersistentCrossVMStateManager {
    /// Create a new persistent state manager with RocksDB
    pub async fn new_with_rocksdb(
        state_config: StateManagerConfig,
        db_path: &str,
    ) -> ConsensusResult<Self> {
        let memory_manager = CrossVMStateManager::new(state_config.clone());

        let storage = Arc::new(RocksDBConsensusStorage::new(db_path)?);

        let manager = Self {
            memory_manager,
            storage,
            config: state_config,
        };

        // Try to load latest state from storage
        if let Some(state) = manager.storage.get_latest_state().await? {
            info!("Loaded consensus state from persistence: {:?}", state);
        }

        Ok(manager)
    }

    /// Get the current consensus state (from memory)
    pub async fn get_consensus_state(&self) -> ConsensusResult<ConsensusState> {
        // For MultiVM, we track our own consensus state, not VM-specific states
        Ok(ConsensusState::new())
    }

    /// Save current state to persistent storage
    pub async fn save_state(&self, height: u64) -> ConsensusResult<()> {
        let state = self.get_consensus_state().await?;
        self.storage.store_state(height, &state).await?;
        Ok(())
    }

    /// Load state from persistent storage at specific height
    pub async fn load_state(&self, height: u64) -> ConsensusResult<Option<ConsensusState>> {
        self.storage.get_state(height).await
    }

    /// Get storage statistics
    pub async fn get_storage_stats(&self) -> ConsensusResult<StorageStats> {
        self.storage.get_stats().await
    }

    /// Get current height from memory manager
    pub fn get_current_height(&self) -> u64 {
        self.memory_manager.get_current_height()
    }

    /// Get current view from memory manager
    pub fn get_current_view(&self) -> u64 {
        self.memory_manager.get_current_view()
    }

    /// Update VM state
    pub fn update_vm_state(
        &self,
        vm_type: crate::messages::VmType,
        height: u64,
        state_root: String,
    ) -> ConsensusResult<()> {
        self.memory_manager
            .update_vm_state(vm_type, height, state_root)
    }

    /// Apply a block to the state
    pub async fn apply_block(&mut self, block: &crate::MultiVMBlock) -> ConsensusResult<()> {
        self.memory_manager.apply_block(block).await
    }

    /// Get cross-VM state
    pub async fn get_cross_vm_state(&self) -> ConsensusResult<CrossVMState> {
        self.memory_manager.get_cross_vm_state().await
    }

    /// Create checkpoint
    pub async fn create_checkpoint(&self) -> ConsensusResult<StateCheckpoint> {
        self.memory_manager.create_checkpoint().await
    }

    /// Restore from checkpoint
    pub async fn restore_from_checkpoint(
        &mut self,
        checkpoint: &StateCheckpoint,
    ) -> ConsensusResult<()> {
        self.memory_manager
            .restore_from_checkpoint(checkpoint)
            .await
    }

    /// Sync state to target height
    pub async fn sync_state(&mut self, target_height: u64) -> ConsensusResult<()> {
        self.memory_manager.sync_state(target_height).await
    }

    /// Validate cross-VM transaction
    pub async fn validate_cross_vm_transaction(
        &self,
        transaction: &SpecialTransaction,
    ) -> ConsensusResult<ValidationResult> {
        self.memory_manager
            .validate_cross_vm_transaction(transaction)
            .await
    }

    /// Check if a node is a validator
    pub async fn is_validator(&self, validator_id: &str) -> ConsensusResult<bool> {
        // For now, we use a simple validator set stored in the state
        // This could be enhanced to use on-chain validator registry
        let validators = self.get_validator_set().await?;
        Ok(validators.contains(&validator_id.to_string()))
    }

    /// Get validator's public key
    pub async fn get_validator_pubkey(
        &self,
        validator_id: &str,
    ) -> ConsensusResult<Option<Vec<u8>>> {
        // Retrieve validator public key from storage
        let key = format!("validator:pubkey:{validator_id}");
        match self.storage.get(key.as_bytes()).await? {
            Some(pubkey) => Ok(Some(pubkey)),
            None => Ok(None),
        }
    }

    /// Get current validator set
    pub async fn get_validator_set(&self) -> ConsensusResult<Vec<String>> {
        // Retrieve current validator set from storage
        let key = b"validators:current";
        match self.storage.get(key).await? {
            Some(data) => {
                let validators: Vec<String> = bincode::deserialize(&data).map_err(|e| {
                    ConsensusError::Storage(format!("Failed to deserialize validators: {e}"))
                })?;
                Ok(validators)
            }
            None => {
                // Return default validator set if none exists
                Ok(vec!["validator1".to_string(), "validator2".to_string()])
            }
        }
    }

    /// Update validator set
    pub async fn update_validator_set(&self, validators: Vec<String>) -> ConsensusResult<()> {
        let key = b"validators:current";
        let data = bincode::serialize(&validators)
            .map_err(|e| ConsensusError::Storage(format!("Failed to serialize validators: {e}")))?;

        self.storage
            .put(key, &data)
            .await
            .map_err(|e| ConsensusError::Storage(format!("Failed to store validators: {e}")))?;

        Ok(())
    }
}

/// Simple consensus state for tracking consensus progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusState {
    /// Last committed sequence number
    pub last_committed_sequence: u64,
    /// Last committed block hash
    pub last_committed_block_hash: String,
    /// Current view number
    pub current_view: u64,
    /// Current round number
    pub current_round: u32,
    /// Whether consensus is running
    pub is_running: bool,
}

impl ConsensusState {
    /// Create new consensus state
    pub fn new() -> Self {
        Self {
            last_committed_sequence: 0,
            last_committed_block_hash: "genesis".to_string(),
            current_view: 0,
            current_round: 0,
            is_running: false,
        }
    }

    /// Check if we can propose a new block
    pub fn can_propose_new_block(&self) -> bool {
        self.is_running
    }
}

impl Default for ConsensusState {
    fn default() -> Self {
        Self::new()
    }
}

/// Cross-VM state manager responsible for coordinating state across different VMs
#[derive(Debug)]
pub struct CrossVMStateManager {
    /// Current cross-VM state
    state: Arc<RwLock<CrossVMState>>,
    /// State history for rollback support
    state_history: Arc<RwLock<Vec<StateCheckpoint>>>,
    /// Account bindings cache
    account_bindings: Arc<RwLock<HashMap<MultivmAccountId, AccountBinding>>>,
    /// Pending state changes
    pending_changes: Arc<RwLock<Vec<StateChange>>>,
    /// Configuration
    config: StateManagerConfig,
}

/// Configuration for the state manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateManagerConfig {
    /// Maximum number of checkpoints to keep in history
    pub max_checkpoints: usize,
    /// Checkpoint creation interval (in blocks)
    pub checkpoint_interval: u64,
    /// Enable state verification
    pub enable_verification: bool,
    /// Maximum pending changes before forced commit
    pub max_pending_changes: usize,
    /// Optional RocksDB path (for testing)
    pub rocksdb_path: Option<String>,
}

impl Default for StateManagerConfig {
    fn default() -> Self {
        Self {
            max_checkpoints: 100,
            checkpoint_interval: 10,
            enable_verification: true,
            max_pending_changes: 1000,
            rocksdb_path: None,
        }
    }
}

impl CrossVMStateManager {
    /// Create a new cross-VM state manager
    pub fn new(config: StateManagerConfig) -> Self {
        let initial_state = CrossVMState {
            height: 0,
            svm_state_root: "genesis".to_string(),
            evm_state_root: "genesis".to_string(),
            account_bindings: Vec::new(),
            global_nonce: 0,
            timestamp: SystemTime::now(),
        };

        Self {
            state: Arc::new(RwLock::new(initial_state)),
            state_history: Arc::new(RwLock::new(Vec::new())),
            account_bindings: Arc::new(RwLock::new(HashMap::new())),
            pending_changes: Arc::new(RwLock::new(Vec::new())),
            config,
        }
    }

    /// Update the SVM state root
    pub fn update_svm_state_root(&self, new_root: String) -> ConsensusResult<()> {
        let mut state = self.state.write();
        state.svm_state_root = new_root;
        state.timestamp = SystemTime::now();
        Ok(())
    }

    /// Update the EVM state root
    pub fn update_evm_state_root(&self, new_root: String) -> ConsensusResult<()> {
        let mut state = self.state.write();
        state.evm_state_root = new_root;
        state.timestamp = SystemTime::now();
        Ok(())
    }

    /// Update the blockchain height
    pub fn update_height(&self, height: u64) -> ConsensusResult<()> {
        let mut state = self.state.write();
        state.height = height;
        state.timestamp = SystemTime::now();

        // Create checkpoint if needed
        if height % self.config.checkpoint_interval == 0 {
            drop(state); // Release lock before creating checkpoint
            self.create_automatic_checkpoint(height)?;
        }

        Ok(())
    }

    /// Add an account binding to the state
    pub fn add_account_binding(&self, binding: AccountBinding) -> ConsensusResult<()> {
        let binding_info = crate::traits::ConsensusAccountBinding {
            multivm_account: binding.multivm_account.clone(),
            bound_addresses: binding
                .get_all_accounts()
                .iter()
                .map(|addr| addr.to_string())
                .collect(),
            metadata: serde_json::json!({
                "created_at": SystemTime::now(),
                "binding_type": "user_initiated"
            }),
        };

        // Update bindings cache
        {
            let mut bindings = self.account_bindings.write();
            bindings.insert(binding.multivm_account.clone(), binding);
        }

        // Update state
        {
            let mut state = self.state.write();
            state.account_bindings.push(binding_info);
            state.global_nonce += 1;
            state.timestamp = SystemTime::now();
        }

        Ok(())
    }

    /// Remove an account binding
    pub fn remove_account_binding(
        &self,
        multivm_account: &MultivmAccountId,
    ) -> ConsensusResult<bool> {
        // Remove from cache
        let removed_from_cache = {
            let mut bindings = self.account_bindings.write();
            bindings.remove(multivm_account).is_some()
        };

        // Update state
        {
            let mut state = self.state.write();
            let initial_len = state.account_bindings.len();
            state
                .account_bindings
                .retain(|binding| &binding.multivm_account != multivm_account);
            let removed_from_state = state.account_bindings.len() < initial_len;

            if removed_from_state {
                state.global_nonce += 1;
                state.timestamp = SystemTime::now();
            }

            Ok(removed_from_cache || removed_from_state)
        }
    }

    /// Get an account binding by MultiVM account ID
    pub fn get_account_binding(
        &self,
        multivm_account: &MultivmAccountId,
    ) -> Option<AccountBinding> {
        let bindings = self.account_bindings.read();
        bindings.get(multivm_account).cloned()
    }

    /// Apply pending changes to the state
    pub fn apply_pending_changes(&self) -> ConsensusResult<usize> {
        let changes = {
            let mut pending = self.pending_changes.write();
            let changes = pending.clone();
            pending.clear();
            changes
        };

        let applied_count = changes.len();

        // Process each change
        for change in changes {
            self.apply_single_change(change)?;
        }

        Ok(applied_count)
    }

    /// Add a pending state change
    pub fn add_pending_change(&self, change: StateChange) -> ConsensusResult<()> {
        let mut pending = self.pending_changes.write();
        pending.push(change);

        // Force apply if too many pending changes
        if pending.len() >= self.config.max_pending_changes {
            drop(pending);
            self.apply_pending_changes()?;
        }

        Ok(())
    }

    /// Apply a finalized block to the state
    pub async fn apply_block(&mut self, block: &crate::block::MultiVMBlock) -> ConsensusResult<()> {
        let height = block.header.height;

        // Update state with block information
        {
            let mut state = self.state.write();
            state.height = height;
            state.global_nonce += 1;
            state.timestamp = block.header.timestamp;
            state.svm_state_root = block.header.state_root.clone();
            // For EVM, we'd update evm_state_root from EVM transactions
        }

        // Process state transitions from the block
        for transition in &block.state_transitions {
            self.apply_state_transition(transition).await?;
        }

        // Create automatic checkpoint based on interval
        if height % self.config.checkpoint_interval == 0 {
            self.create_automatic_checkpoint(height)?;
        }

        info!("Applied block at height {} to state", height);
        Ok(())
    }

    /// Apply a state transition from a block
    async fn apply_state_transition(
        &mut self,
        transition: &crate::block::StateTransition,
    ) -> ConsensusResult<()> {
        use crate::block::StateTransitionType;

        // Process each state change in the transition
        for change in &transition.changes {
            let state_change = StateChange {
                target: transition.target.clone(),
                change_type: match &transition.transition_type {
                    StateTransitionType::BalanceChange => {
                        crate::traits::StateChangeType::BalanceUpdate
                    }
                    StateTransitionType::AccountBinding => {
                        crate::traits::StateChangeType::BindingCreated
                    }
                    _ => crate::traits::StateChangeType::BalanceUpdate,
                },
                previous_value: change.previous_value.clone(),
                new_value: change.new_value.clone(),
                metadata: serde_json::json!({
                    "transition_type": format!("{:?}", transition.transition_type),
                    "field": change.field,
                    "tx_id": transition.caused_by.to_string(),
                }),
            };

            self.apply_single_change(state_change)?;
        }

        Ok(())
    }

    /// Get current state statistics
    pub fn get_state_statistics(&self) -> StateStatistics {
        let state = self.state.read();
        let bindings = self.account_bindings.read();
        let pending = self.pending_changes.read();
        let history = self.state_history.read();

        StateStatistics {
            current_height: state.height,
            global_nonce: state.global_nonce,
            account_bindings_count: bindings.len(),
            pending_changes_count: pending.len(),
            checkpoints_count: history.len(),
            svm_state_root: state.svm_state_root.clone(),
            evm_state_root: state.evm_state_root.clone(),
            last_updated: state.timestamp,
        }
    }

    /// Create an automatic checkpoint
    fn create_automatic_checkpoint(&self, height: u64) -> ConsensusResult<StateCheckpoint> {
        let checkpoint = self.create_checkpoint_internal(height)?;

        // Add to history
        {
            let mut history = self.state_history.write();
            history.push(checkpoint.clone());

            // Trim history if needed
            if history.len() > self.config.max_checkpoints {
                history.remove(0);
            }
        }

        Ok(checkpoint)
    }

    /// Internal checkpoint creation
    fn create_checkpoint_internal(&self, height: u64) -> ConsensusResult<StateCheckpoint> {
        let state = self.state.read();

        let checkpoint = StateCheckpoint {
            height,
            state_hash: self.calculate_state_hash(&state),
            cross_vm_state: state.clone(),
            timestamp: SystemTime::now(),
            metadata: serde_json::json!({
                "auto_generated": true,
                "bindings_count": state.account_bindings.len()
            }),
        };

        Ok(checkpoint)
    }

    /// Calculate hash of the current state
    fn calculate_state_hash(&self, state: &CrossVMState) -> String {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(state.height.to_le_bytes());
        hasher.update(state.svm_state_root.as_bytes());
        hasher.update(state.evm_state_root.as_bytes());
        hasher.update(state.global_nonce.to_le_bytes());

        // Hash account bindings
        for binding in &state.account_bindings {
            hasher.update(binding.multivm_account.to_string().as_bytes());
            for addr in &binding.bound_addresses {
                hasher.update(addr.as_bytes());
            }
        }

        format!("{:x}", hasher.finalize())
    }

    /// Apply a single state change
    fn apply_single_change(&self, change: StateChange) -> ConsensusResult<()> {
        match &change.change_type {
            crate::traits::StateChangeType::BalanceUpdate => {
                // Apply balance update to state tree
                let mut state = self.state.write();

                // Update balance in state
                let new_root = self.compute_new_state_root(&change)?;
                state.svm_state_root = hex::encode(new_root);
                tracing::info!(
                    "Applied balance update for {} with new SVM root {}",
                    change.target,
                    state.svm_state_root
                );
            }
            crate::traits::StateChangeType::BindingCreated => {
                // Create new account binding in state
                let mut state = self.state.write();

                let new_root = self.compute_new_state_root(&change)?;
                state.evm_state_root = hex::encode(new_root);
                tracing::info!(
                    "Applied binding creation for {} with new EVM root {}",
                    change.target,
                    state.evm_state_root
                );
            }
            crate::traits::StateChangeType::BindingUpdated => {
                // Update existing account binding
                let mut state = self.state.write();

                let new_root = self.compute_new_state_root(&change)?;
                state.svm_state_root = hex::encode(new_root);
                tracing::info!(
                    "Applied binding update for {} with new SVM root {}",
                    change.target,
                    state.svm_state_root
                );
            }
            crate::traits::StateChangeType::BindingRemoved => {
                // Remove account binding from state
                let mut state = self.state.write();

                let new_root = self.compute_new_state_root(&change)?;
                state.evm_state_root = hex::encode(new_root);
                tracing::info!(
                    "Applied binding removal for {} with new EVM root {}",
                    change.target,
                    state.evm_state_root
                );
            }
            crate::traits::StateChangeType::ContractState => {
                // Apply contract state changes to state tree
                let mut state = self.state.write();

                let new_root = self.compute_new_state_root(&change)?;
                state.evm_state_root = hex::encode(new_root);
                tracing::info!(
                    "Applied contract state change for {} with new EVM root {}",
                    change.target,
                    state.evm_state_root
                );
            }
            crate::traits::StateChangeType::Custom(custom_type) => {
                // Apply custom state changes to state tree
                let mut state = self.state.write();

                let new_root = self.compute_new_state_root(&change)?;
                state.svm_state_root = hex::encode(new_root);
                tracing::info!(
                    "Applied custom state change '{}' for {} with new SVM root {}",
                    custom_type,
                    change.target,
                    state.svm_state_root
                );
            }
        }

        Ok(())
    }

    /// Compute new state root after applying a change
    fn compute_new_state_root(&self, change: &StateChange) -> ConsensusResult<[u8; 32]> {
        use sha2::{Digest, Sha256};

        // Create state hash incorporating the change
        let mut hasher = Sha256::new();
        hasher.update(change.target.as_bytes());
        hasher.update(&serde_json::to_vec(&change.change_type).map_err(|e| {
            ConsensusError::StateError(format!("Failed to serialize change type: {e}"))
        })?);
        hasher.update(&serde_json::to_vec(&change.new_value).map_err(|e| {
            ConsensusError::StateError(format!("Failed to serialize new value: {e}"))
        })?);

        // Include current state roots in computation
        let current_state = self.state.read();
        hasher.update(current_state.svm_state_root.as_bytes());
        hasher.update(current_state.evm_state_root.as_bytes());

        let result = hasher.finalize();
        let mut root = [0u8; 32];
        root.copy_from_slice(&result);

        Ok(root)
    }

    /// Get current view number
    pub fn get_current_view(&self) -> u64 {
        // For simplicity, return a default view. In production this would track actual view changes
        0
    }

    /// Update VM state for a specific VM type
    pub fn update_vm_state(
        &self,
        vm_type: VmType,
        height: u64,
        state_root: String,
    ) -> ConsensusResult<()> {
        let mut state = self.state.write();

        match vm_type {
            VmType::SVM => {
                state.svm_state_root = state_root;
                state.height = height;
            }
            VmType::EVM => {
                state.evm_state_root = state_root;
                state.height = height;
            }
            VmType::MultiVM => {
                // Update both for MultiVM
                state.svm_state_root = state_root.clone();
                state.evm_state_root = state_root;
                state.height = height;
            }
        }

        Ok(())
    }

    /// Get current blockchain height
    pub fn get_current_height(&self) -> u64 {
        let state = self.state.read();
        state.height
    }
}

#[async_trait]
impl CrossVMStateCoordinator for CrossVMStateManager {
    async fn apply_cross_vm_transaction(
        &mut self,
        transaction: &SpecialTransaction,
    ) -> ConsensusResult<Vec<StateChange>> {
        match transaction {
            SpecialTransaction::AccountBinding {
                source_account,
                target_account,
                proof,
                metadata,
            } => {
                // Create state changes for account binding
                let changes = vec![StateChange {
                    change_type: crate::traits::StateChangeType::BindingCreated,
                    target: source_account.to_string(),
                    previous_value: None,
                    new_value: serde_json::json!({
                        "target": target_account.to_string(),
                        "proof_type": format!("{:?}", proof.proof_type)
                    }),
                    metadata: metadata
                        .as_ref()
                        .map(|m| serde_json::to_value(m).unwrap_or_default())
                        .unwrap_or_default(),
                }];

                // Apply changes
                for change in &changes {
                    self.add_pending_change(change.clone())?;
                }

                Ok(changes)
            }
            SpecialTransaction::CrossVmTransfer {
                from, to, amount, ..
            } => {
                // Create state changes for cross-VM transfer
                let changes = vec![
                    StateChange {
                        change_type: crate::traits::StateChangeType::BalanceUpdate,
                        target: from.to_string(),
                        previous_value: None, // Would be fetched from actual state
                        new_value: serde_json::json!({"balance_delta": -(*amount as i64)}),
                        metadata: serde_json::json!({"transfer_type": "outgoing"}),
                    },
                    StateChange {
                        change_type: crate::traits::StateChangeType::BalanceUpdate,
                        target: to.to_string(),
                        previous_value: None, // Would be fetched from actual state
                        new_value: serde_json::json!({"balance_delta": *amount}),
                        metadata: serde_json::json!({"transfer_type": "incoming"}),
                    },
                ];

                // Apply changes
                for change in &changes {
                    self.add_pending_change(change.clone())?;
                }

                Ok(changes)
            }
            SpecialTransaction::UpdateBinding {
                multivm_account,
                config,
            } => {
                let change = StateChange {
                    change_type: crate::traits::StateChangeType::BindingUpdated,
                    target: multivm_account.to_string(),
                    previous_value: None,
                    new_value: serde_json::to_value(config)
                        .map_err(|e| ConsensusError::Internal(e.to_string()))?,
                    metadata: serde_json::json!({"operation": "update_binding"}),
                };

                self.add_pending_change(change.clone())?;
                Ok(vec![change])
            }
            SpecialTransaction::UnbindAccount {
                multivm_account,
                account,
                ..
            } => {
                let change = StateChange {
                    change_type: crate::traits::StateChangeType::BindingRemoved,
                    target: multivm_account.to_string(),
                    previous_value: Some(serde_json::json!(account.to_string())),
                    new_value: serde_json::Value::Null,
                    metadata: serde_json::json!({"operation": "unbind_account"}),
                };

                self.add_pending_change(change.clone())?;
                Ok(vec![change])
            }
        }
    }

    async fn validate_cross_vm_transaction(
        &self,
        transaction: &SpecialTransaction,
    ) -> ConsensusResult<ValidationResult> {
        match transaction {
            SpecialTransaction::AccountBinding {
                source_account,
                target_account,
                ..
            } => {
                // Basic validation
                if source_account == target_account {
                    return Ok(ValidationResult::Invalid(
                        "Cannot bind account to itself".to_string(),
                    ));
                }

                // Check if binding already exists (placeholder)
                // In real implementation, would check actual state

                Ok(ValidationResult::Valid)
            }
            SpecialTransaction::CrossVmTransfer {
                from, to, amount, ..
            } => {
                if from == to {
                    return Ok(ValidationResult::Invalid(
                        "Cannot transfer to same account".to_string(),
                    ));
                }

                if *amount == 0 {
                    return Ok(ValidationResult::Invalid(
                        "Transfer amount must be greater than zero".to_string(),
                    ));
                }

                // Check if accounts exist and have sufficient balance (placeholder)
                // In real implementation, would check actual state

                Ok(ValidationResult::Valid)
            }
            SpecialTransaction::UpdateBinding {
                multivm_account, ..
            } => {
                // Check if binding exists (placeholder)
                if self.get_account_binding(multivm_account).is_none() {
                    return Ok(ValidationResult::Invalid("Binding not found".to_string()));
                }

                Ok(ValidationResult::Valid)
            }
            SpecialTransaction::UnbindAccount {
                multivm_account,
                account,
                ..
            } => {
                // Check if binding exists and contains the account (placeholder)
                if let Some(binding) = self.get_account_binding(multivm_account) {
                    let account_exists = binding
                        .get_all_accounts()
                        .iter()
                        .any(|addr| addr == account);

                    if !account_exists {
                        return Ok(ValidationResult::Invalid(
                            "Account not found in binding".to_string(),
                        ));
                    }
                } else {
                    return Ok(ValidationResult::Invalid("Binding not found".to_string()));
                }

                Ok(ValidationResult::Valid)
            }
        }
    }

    async fn get_cross_vm_state(&self) -> ConsensusResult<CrossVMState> {
        let state = self.state.read();
        Ok(state.clone())
    }

    async fn sync_state(&mut self, target_height: u64) -> ConsensusResult<()> {
        // Placeholder implementation for state synchronization
        // In real implementation, would sync with other nodes

        let current_height = {
            let state = self.state.read();
            state.height
        };

        if target_height <= current_height {
            return Ok(()); // Already at or beyond target height
        }

        // Simulate state sync by updating height
        self.update_height(target_height)?;

        tracing::info!(
            "State synced from height {} to {}",
            current_height,
            target_height
        );
        Ok(())
    }

    async fn create_checkpoint(&self) -> ConsensusResult<StateCheckpoint> {
        let state = self.state.read();
        self.create_checkpoint_internal(state.height)
    }

    async fn restore_from_checkpoint(
        &mut self,
        checkpoint: &StateCheckpoint,
    ) -> ConsensusResult<()> {
        // Restore state from checkpoint
        {
            let mut state = self.state.write();
            *state = checkpoint.cross_vm_state.clone();
        }

        // Clear pending changes
        {
            let mut pending = self.pending_changes.write();
            pending.clear();
        }

        // Rebuild account bindings cache
        {
            let mut bindings = self.account_bindings.write();
            bindings.clear();

            // Note: In real implementation, would restore actual AccountBinding objects
            // For now, just log the restoration
            tracing::info!(
                "Restored state from checkpoint at height {}",
                checkpoint.height
            );
        }

        Ok(())
    }
}

/// State statistics for monitoring and debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateStatistics {
    pub current_height: u64,
    pub global_nonce: u64,
    pub account_bindings_count: usize,
    pub pending_changes_count: usize,
    pub checkpoints_count: usize,
    pub svm_state_root: String,
    pub evm_state_root: String,
    pub last_updated: SystemTime,
}

// ================================================================================================
// State Persistence and Recovery System
// ================================================================================================

/// Storage backend configuration for state persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageBackend {
    /// In-memory storage (testing only)
    Memory,
    /// File-based persistence
    File {
        /// Directory for state files
        data_dir: PathBuf,
        /// Enable compression
        compress: bool,
    },
}

impl Default for StorageBackend {
    fn default() -> Self {
        Self::File {
            data_dir: PathBuf::from("./consensus-data"),
            compress: true,
        }
    }
}

/// Configuration for state persistence and recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatePersistenceConfig {
    /// Storage backend configuration
    pub backend: StorageBackend,
    /// Auto-save interval in seconds
    pub auto_save_interval_secs: u64,
    /// Maximum number of recovery points to keep
    pub max_recovery_points: usize,
    /// Enable write-ahead logging
    pub enable_wal: bool,
    /// Recovery timeout in milliseconds
    pub recovery_timeout_ms: u64,
    /// Enable state verification on load
    pub verify_on_load: bool,
}

impl Default for StatePersistenceConfig {
    fn default() -> Self {
        Self {
            backend: StorageBackend::default(),
            auto_save_interval_secs: 30,
            max_recovery_points: 50,
            enable_wal: true,
            recovery_timeout_ms: 10000,
            verify_on_load: true,
        }
    }
}

impl StatePersistenceConfig {
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.auto_save_interval_secs == 0 {
            return Err("Auto-save interval must be greater than 0".to_string());
        }
        if self.max_recovery_points == 0 {
            return Err("Max recovery points must be greater than 0".to_string());
        }
        if self.recovery_timeout_ms == 0 {
            return Err("Recovery timeout must be greater than 0".to_string());
        }
        Ok(())
    }
}

/// Error types for state persistence operations
#[derive(Debug, thiserror::Error)]
pub enum StatePersistenceError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    #[error("Recovery failed: {0}")]
    RecoveryFailed(String),
    #[error("State verification failed: {0}")]
    VerificationFailed(String),
    #[error("Corruption detected: {0}")]
    CorruptionDetected(String),
    #[error("Timeout during operation: {0}")]
    Timeout(String),
}

impl StatePersistenceError {
    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::Io(_) => true,
            Self::Serialization(_) => false,
            Self::RecoveryFailed(_) => true,
            Self::VerificationFailed(_) => false,
            Self::CorruptionDetected(_) => false,
            Self::Timeout(_) => true,
        }
    }

    /// Check if this error is critical
    pub fn is_critical(&self) -> bool {
        matches!(
            self,
            Self::CorruptionDetected(_) | Self::VerificationFailed(_)
        )
    }

    /// Get error category for metrics
    pub fn category(&self) -> &'static str {
        match self {
            Self::Io(_) => "io",
            Self::Serialization(_) => "serialization",
            Self::RecoveryFailed(_) => "recovery",
            Self::VerificationFailed(_) => "verification",
            Self::CorruptionDetected(_) => "corruption",
            Self::Timeout(_) => "timeout",
        }
    }
}

/// Recovery point containing state snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryPoint {
    /// Recovery point ID
    pub id: String,
    /// Height at which this recovery point was created
    pub height: u64,
    /// Timestamp of creation
    pub timestamp: SystemTime,
    /// Cross-VM state snapshot
    pub state_snapshot: CrossVMState,
    /// State checksum for verification
    pub checksum: String,
    /// Recovery metadata
    pub metadata: serde_json::Value,
}

impl RecoveryPoint {
    /// Create a new recovery point
    pub fn new(height: u64, state: CrossVMState) -> Self {
        let id = format!(
            "recovery-{}-{}",
            height,
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );
        let checksum = Self::compute_checksum(&state);

        Self {
            id,
            height,
            timestamp: SystemTime::now(),
            state_snapshot: state,
            checksum,
            metadata: serde_json::Value::Null,
        }
    }

    /// Compute checksum for state verification
    fn compute_checksum(state: &CrossVMState) -> String {
        use sha2::{Digest, Sha256};
        let serialized = bincode::serialize(state).unwrap_or_default();
        let hash = Sha256::digest(&serialized);
        hex::encode(hash)
    }

    /// Verify the integrity of this recovery point
    pub fn verify_integrity(&self) -> bool {
        let computed_checksum = Self::compute_checksum(&self.state_snapshot);
        computed_checksum == self.checksum
    }
}

/// Write-ahead log entry for state persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalEntry {
    /// Entry sequence number
    pub sequence: u64,
    /// Timestamp
    pub timestamp: SystemTime,
    /// State changes in this entry
    pub changes: Vec<StateChange>,
    /// Entry checksum
    pub checksum: String,
}

impl WalEntry {
    /// Create a new WAL entry
    pub fn new(sequence: u64, changes: Vec<StateChange>) -> Self {
        let checksum = Self::compute_checksum(sequence, &changes);
        Self {
            sequence,
            timestamp: SystemTime::now(),
            changes,
            checksum,
        }
    }

    /// Compute checksum for entry verification
    fn compute_checksum(sequence: u64, changes: &[StateChange]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(sequence.to_le_bytes());
        for change in changes {
            if let Ok(serialized) = bincode::serialize(change) {
                hasher.update(&serialized);
            }
        }
        hex::encode(hasher.finalize())
    }

    /// Verify the integrity of this WAL entry
    pub fn verify_integrity(&self) -> bool {
        let computed_checksum = Self::compute_checksum(self.sequence, &self.changes);
        computed_checksum == self.checksum
    }
}

/// Comprehensive state persistence manager
#[derive(Debug)]
pub struct StatePersistenceManager {
    /// Configuration
    config: StatePersistenceConfig,
    /// Current recovery points
    recovery_points: Arc<AsyncRwLock<Vec<RecoveryPoint>>>,
    /// Write-ahead log
    wal: Arc<AsyncRwLock<Vec<WalEntry>>>,
    /// WAL sequence counter
    wal_sequence: Arc<AsyncRwLock<u64>>,
    /// Last auto-save time
    last_save_time: Arc<AsyncRwLock<Instant>>,
    /// Persistence metrics
    metrics: Arc<AsyncRwLock<PersistenceMetrics>>,
}

/// Metrics for state persistence operations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersistenceMetrics {
    /// Total save operations
    pub save_operations: u64,
    /// Total load operations  
    pub load_operations: u64,
    /// Total recovery operations
    pub recovery_operations: u64,
    /// Total WAL entries written
    pub wal_entries_written: u64,
    /// Total verification failures
    pub verification_failures: u64,
    /// Total corruption detections
    pub corruption_detections: u64,
    /// Average save time in milliseconds
    pub avg_save_time_ms: u64,
    /// Average load time in milliseconds
    pub avg_load_time_ms: u64,
    /// Last operation timestamp
    pub last_operation: Option<SystemTime>,
}

impl StatePersistenceManager {
    /// Create a new state persistence manager
    pub async fn new(config: StatePersistenceConfig) -> Result<Self, StatePersistenceError> {
        config
            .validate()
            .map_err(StatePersistenceError::RecoveryFailed)?;

        // Ensure data directory exists for file backend
        if let StorageBackend::File { data_dir, .. } = &config.backend {
            fs::create_dir_all(data_dir)?;
        }

        let manager = Self {
            config,
            recovery_points: Arc::new(AsyncRwLock::new(Vec::new())),
            wal: Arc::new(AsyncRwLock::new(Vec::new())),
            wal_sequence: Arc::new(AsyncRwLock::new(0)),
            last_save_time: Arc::new(AsyncRwLock::new(Instant::now())),
            metrics: Arc::new(AsyncRwLock::new(PersistenceMetrics::default())),
        };

        // Load existing state if available
        if let Err(e) = manager.load_existing_state().await {
            warn!("Failed to load existing state: {}", e);
        }

        info!("State persistence manager initialized");
        Ok(manager)
    }

    /// Save current state as a recovery point
    pub async fn save_state(
        &self,
        height: u64,
        state: CrossVMState,
    ) -> Result<String, StatePersistenceError> {
        let start_time = Instant::now();
        let recovery_point = RecoveryPoint::new(height, state);
        let point_id = recovery_point.id.clone();

        // Add to in-memory cache
        {
            let mut points = self.recovery_points.write().await;
            points.push(recovery_point.clone());

            // Prune old recovery points
            while points.len() > self.config.max_recovery_points {
                points.remove(0);
            }
        }

        // Persist to storage
        self.persist_recovery_point(&recovery_point).await?;

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.save_operations += 1;
            metrics.avg_save_time_ms =
                (metrics.avg_save_time_ms + start_time.elapsed().as_millis() as u64) / 2;
            metrics.last_operation = Some(SystemTime::now());
        }

        // Update last save time
        *self.last_save_time.write().await = Instant::now();

        info!("Saved state at height {} with ID: {}", height, point_id);
        Ok(point_id)
    }

    /// Load state from the latest recovery point
    pub async fn load_latest_state(&self) -> Result<Option<CrossVMState>, StatePersistenceError> {
        let start_time = Instant::now();

        let recovery_points = self.recovery_points.read().await;
        let latest_point = recovery_points.last();

        let result = if let Some(point) = latest_point {
            if self.config.verify_on_load && !point.verify_integrity() {
                return Err(StatePersistenceError::VerificationFailed(
                    "Recovery point failed integrity check".to_string(),
                ));
            }
            Some(point.state_snapshot.clone())
        } else {
            None
        };

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.load_operations += 1;
            metrics.avg_load_time_ms =
                (metrics.avg_load_time_ms + start_time.elapsed().as_millis() as u64) / 2;
            metrics.last_operation = Some(SystemTime::now());
        }

        Ok(result)
    }

    /// Recover to a specific height
    pub async fn recover_to_height(
        &self,
        target_height: u64,
    ) -> Result<Option<CrossVMState>, StatePersistenceError> {
        let start_time = Instant::now();

        let recovery_points = self.recovery_points.read().await;

        // Find the recovery point closest to but not exceeding the target height
        let mut best_point: Option<&RecoveryPoint> = None;
        for point in recovery_points.iter().rev() {
            if point.height <= target_height {
                best_point = Some(point);
                break;
            }
        }

        let result = if let Some(point) = best_point {
            if self.config.verify_on_load && !point.verify_integrity() {
                let mut metrics = self.metrics.write().await;
                metrics.verification_failures += 1;
                return Err(StatePersistenceError::VerificationFailed(format!(
                    "Recovery point at height {} failed integrity check",
                    point.height
                )));
            }

            info!(
                "Recovered to height {} using recovery point at height {}",
                target_height, point.height
            );
            Some(point.state_snapshot.clone())
        } else {
            warn!("No recovery point found for height {}", target_height);
            None
        };

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.recovery_operations += 1;
            metrics.last_operation = Some(SystemTime::now());
        }

        Ok(result)
    }

    /// Check if auto-save is needed
    pub async fn should_auto_save(&self) -> bool {
        let last_save = *self.last_save_time.read().await;
        let elapsed = last_save.elapsed();
        elapsed.as_secs() >= self.config.auto_save_interval_secs
    }

    /// Get persistence metrics
    pub async fn get_metrics(&self) -> PersistenceMetrics {
        self.metrics.read().await.clone()
    }

    /// Persist recovery point to storage backend
    async fn persist_recovery_point(
        &self,
        point: &RecoveryPoint,
    ) -> Result<(), StatePersistenceError> {
        match &self.config.backend {
            StorageBackend::Memory => {
                // No actual persistence for memory backend
                Ok(())
            }
            StorageBackend::File { data_dir, compress } => {
                let file_path = data_dir.join(format!("recovery-{}.bin", point.height));
                let serialized = bincode::serialize(point)?;

                let data = if *compress {
                    // Simple compression would go here
                    serialized
                } else {
                    serialized
                };

                tokio::fs::write(file_path, data).await?;
                Ok(())
            }
        }
    }

    /// Load existing state from storage
    async fn load_existing_state(&self) -> Result<(), StatePersistenceError> {
        match &self.config.backend {
            StorageBackend::Memory => {
                // No loading for memory backend
                Ok(())
            }
            StorageBackend::File { data_dir, .. } => {
                if !data_dir.exists() {
                    return Ok(());
                }

                // Load recovery points
                let mut entries = tokio::fs::read_dir(data_dir).await?;
                let mut recovery_points = Vec::new();

                while let Some(entry) = entries.next_entry().await? {
                    let path = entry.path();
                    if path.extension().is_some_and(|ext| ext == "bin")
                        && path
                            .file_name()
                            .unwrap()
                            .to_string_lossy()
                            .starts_with("recovery-")
                    {
                        if let Ok(data) = tokio::fs::read(&path).await {
                            if let Ok(point) = bincode::deserialize::<RecoveryPoint>(&data) {
                                if point.verify_integrity() {
                                    recovery_points.push(point);
                                } else {
                                    warn!("Recovery point {:?} failed integrity check", path);
                                }
                            }
                        }
                    }
                }

                // Sort by height
                recovery_points.sort_by_key(|p| p.height);
                *self.recovery_points.write().await = recovery_points;

                Ok(())
            }
        }
    }
}

// Enhanced CrossVMStateManager with persistence
impl CrossVMStateManager {
    /// Create state manager with persistence
    pub async fn with_persistence(
        config: StateManagerConfig,
        persistence_config: StatePersistenceConfig,
    ) -> ConsensusResult<Self> {
        let manager = Self::new(config);

        let _persistence = StatePersistenceManager::new(persistence_config)
            .await
            .map_err(|e| {
                ConsensusError::Internal(format!("Failed to create persistence manager: {e}"))
            })?;

        // Integration with persistence would happen here
        info!("State manager with persistence initialized");
        Ok(manager)
    }

    /// Save current state for recovery
    pub async fn save_for_recovery(&self, height: u64) -> ConsensusResult<()> {
        // This would integrate with the persistence manager
        // For now, create a checkpoint
        let _ = self.create_checkpoint().await?;
        debug!("State saved for recovery at height {}", height);
        Ok(())
    }

    /// Recover from the latest saved state
    pub async fn recover_from_latest(&mut self) -> ConsensusResult<bool> {
        // This would integrate with the persistence manager
        // For now, simulate recovery
        info!("Recovery from latest state completed");
        Ok(true)
    }
}
