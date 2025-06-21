//! Cross-VM state management for the MultiVM consensus layer

use crate::traits::{
    CrossVMState, CrossVMStateCoordinator, StateChange, StateCheckpoint, ValidationResult,
};
use crate::{ConsensusError, ConsensusResult};
use crate::messages::VmType;
use async_trait::async_trait;
use multivm_account_mapping::{AccountBinding, MultivmAccountId, SpecialTransaction};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

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
}

impl Default for StateManagerConfig {
    fn default() -> Self {
        Self {
            max_checkpoints: 100,
            checkpoint_interval: 10,
            enable_verification: true,
            max_pending_changes: 1000,
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
                state.svm_state_root = hex::encode(&new_root);
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
                state.evm_state_root = hex::encode(&new_root);
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
                state.svm_state_root = hex::encode(&new_root);
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
                state.evm_state_root = hex::encode(&new_root);
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
                state.evm_state_root = hex::encode(&new_root);
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
                state.svm_state_root = hex::encode(&new_root);
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
        hasher.update(&change.target.as_bytes());
        hasher.update(&serde_json::to_vec(&change.change_type).map_err(|e| {
            ConsensusError::StateError(format!("Failed to serialize change type: {}", e))
        })?);
        hasher.update(&serde_json::to_vec(&change.new_value).map_err(|e| {
            ConsensusError::StateError(format!("Failed to serialize new value: {}", e))
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
    pub async fn update_vm_state(&self, vm_type: VmType, height: u64, state_root: String) -> ConsensusResult<()> {
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

#[cfg(test)]
mod tests {
    use super::*;
    

    #[tokio::test]
    async fn test_state_manager_creation() {
        let config = StateManagerConfig::default();
        let manager = CrossVMStateManager::new(config);

        let stats = manager.get_state_statistics();
        assert_eq!(stats.current_height, 0);
        assert_eq!(stats.global_nonce, 0);
        assert_eq!(stats.account_bindings_count, 0);
    }

    #[tokio::test]
    async fn test_height_update() {
        let config = StateManagerConfig::default();
        let manager = CrossVMStateManager::new(config);

        manager.update_height(100).unwrap();

        let stats = manager.get_state_statistics();
        assert_eq!(stats.current_height, 100);
    }

    #[tokio::test]
    async fn test_state_root_updates() {
        let config = StateManagerConfig::default();
        let manager = CrossVMStateManager::new(config);

        manager
            .update_svm_state_root("svm_root_123".to_string())
            .unwrap();
        manager
            .update_evm_state_root("evm_root_456".to_string())
            .unwrap();

        let stats = manager.get_state_statistics();
        assert_eq!(stats.svm_state_root, "svm_root_123");
        assert_eq!(stats.evm_state_root, "evm_root_456");
    }

    #[tokio::test]
    async fn test_cross_vm_transaction_validation() {
        let config = StateManagerConfig::default();
        let manager = CrossVMStateManager::new(config);

        let tx = SpecialTransaction::CrossVmTransfer {
            from: multivm_account_mapping::MultivmAccountId::from_seed(b"test_from"),
            to: multivm_account_mapping::MultivmAccountId::from_seed(b"test_to"),
            amount: 100,
            asset_type: multivm_account_mapping::AssetType::Native,
            memo: Some("test transfer".to_string()),
        };

        let result = manager.validate_cross_vm_transaction(&tx).await.unwrap();
        assert!(result.is_valid());
    }

    #[tokio::test]
    async fn test_checkpoint_creation() {
        let config = StateManagerConfig::default();
        let manager = CrossVMStateManager::new(config);

        manager.update_height(50).unwrap();

        let checkpoint = manager.create_checkpoint().await.unwrap();
        assert_eq!(checkpoint.height, 50);
        assert!(!checkpoint.state_hash.is_empty());
    }
}
