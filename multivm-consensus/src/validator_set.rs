//! Validator set management for consensus
//!
//! This module manages the validator set, including adding/removing validators,
//! tracking voting power, and handling validator set transitions.

use crate::error::{ConsensusError, ConsensusResult};
use crate::malachite::types::ValidatorAddress;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Validator set change type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidatorSetChange {
    /// Add a new validator
    Add {
        address: ValidatorAddress,
        public_key: String,
        voting_power: u64,
    },
    /// Remove a validator
    Remove { address: ValidatorAddress },
    /// Update validator voting power
    UpdatePower {
        address: ValidatorAddress,
        new_power: u64,
    },
}

/// Validator information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Validator {
    /// Validator address (node ID)
    pub address: ValidatorAddress,
    /// Public key for signature verification
    pub public_key: String,
    /// Voting power (stake-based)
    pub voting_power: u64,
    /// Whether the validator is active
    pub is_active: bool,
    /// Last seen timestamp
    pub last_seen: Option<std::time::SystemTime>,
}

/// Validator set manager
#[derive(Debug, Clone)]
pub struct ValidatorSetManager {
    /// Current validator set
    validators: Arc<RwLock<BTreeMap<ValidatorAddress, Validator>>>,
    /// Minimum number of validators required
    min_validators: usize,
    /// Maximum number of validators allowed
    max_validators: usize,
    /// Total voting power
    total_voting_power: Arc<RwLock<u64>>,
    /// Pending validator set changes
    pending_changes: Arc<RwLock<Vec<ValidatorSetChange>>>,
}

impl ValidatorSetManager {
    /// Create a new validator set manager
    pub fn new(min_validators: usize, max_validators: usize) -> Self {
        Self {
            validators: Arc::new(RwLock::new(BTreeMap::new())),
            min_validators,
            max_validators,
            total_voting_power: Arc::new(RwLock::new(0)),
            pending_changes: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Initialize with a set of validators
    pub async fn initialize(&self, validators: Vec<Validator>) -> ConsensusResult<()> {
        if validators.len() < self.min_validators {
            return Err(ConsensusError::Configuration(format!(
                "Insufficient validators: {} < minimum {}",
                validators.len(),
                self.min_validators
            )));
        }

        let mut validator_map = self.validators.write().await;
        let mut total_power = 0u64;

        validator_map.clear();
        for validator in validators {
            total_power = total_power.saturating_add(validator.voting_power);
            validator_map.insert(validator.address.clone(), validator);
        }

        *self.total_voting_power.write().await = total_power;

        info!(
            "Initialized validator set with {} validators, total voting power: {}",
            validator_map.len(),
            total_power
        );

        Ok(())
    }

    /// Add a new validator
    pub async fn add_validator(&self, validator: Validator) -> ConsensusResult<()> {
        let mut validators = self.validators.write().await;

        if validators.len() >= self.max_validators {
            return Err(ConsensusError::Configuration(format!(
                "Maximum validators reached: {}",
                self.max_validators
            )));
        }

        if validators.contains_key(&validator.address) {
            return Err(ConsensusError::Configuration(format!(
                "Validator {} already exists",
                validator.address
            )));
        }

        let mut total_power = self.total_voting_power.write().await;
        *total_power = total_power.saturating_add(validator.voting_power);

        info!(
            "Adding validator {} with voting power {}",
            validator.address, validator.voting_power
        );

        validators.insert(validator.address.clone(), validator);
        Ok(())
    }

    /// Remove a validator
    pub async fn remove_validator(&self, address: &ValidatorAddress) -> ConsensusResult<()> {
        let mut validators = self.validators.write().await;

        if validators.len() <= self.min_validators {
            return Err(ConsensusError::Configuration(format!(
                "Cannot remove validator: minimum {} required",
                self.min_validators
            )));
        }

        if let Some(validator) = validators.remove(address) {
            let mut total_power = self.total_voting_power.write().await;
            *total_power = total_power.saturating_sub(validator.voting_power);

            info!(
                "Removed validator {} with voting power {}",
                address, validator.voting_power
            );
            Ok(())
        } else {
            Err(ConsensusError::Configuration(format!(
                "Validator {} not found",
                address
            )))
        }
    }

    /// Update validator voting power
    pub async fn update_voting_power(
        &self,
        address: &ValidatorAddress,
        new_power: u64,
    ) -> ConsensusResult<()> {
        let mut validators = self.validators.write().await;

        if let Some(validator) = validators.get_mut(address) {
            let mut total_power = self.total_voting_power.write().await;
            *total_power = total_power.saturating_sub(validator.voting_power);
            *total_power = total_power.saturating_add(new_power);

            info!(
                "Updating validator {} voting power: {} -> {}",
                address, validator.voting_power, new_power
            );

            validator.voting_power = new_power;
            Ok(())
        } else {
            Err(ConsensusError::Configuration(format!(
                "Validator {} not found",
                address
            )))
        }
    }

    /// Get all active validators
    pub async fn get_active_validators(&self) -> Vec<Validator> {
        self.validators
            .read()
            .await
            .values()
            .filter(|v| v.is_active)
            .cloned()
            .collect()
    }

    /// Get validator by address
    pub async fn get_validator(&self, address: &ValidatorAddress) -> Option<Validator> {
        self.validators.read().await.get(address).cloned()
    }

    /// Get total voting power
    pub async fn get_total_voting_power(&self) -> u64 {
        *self.total_voting_power.read().await
    }

    /// Calculate the required voting power for consensus (2/3 + 1)
    pub async fn get_required_voting_power(&self) -> u64 {
        let total = self.get_total_voting_power().await;
        (total * 2) / 3 + 1
    }

    /// Check if a set of validators has sufficient voting power
    pub async fn has_sufficient_power(&self, validators: &HashSet<ValidatorAddress>) -> bool {
        let mut power = 0u64;
        let validator_map = self.validators.read().await;

        for address in validators {
            if let Some(validator) = validator_map.get(address) {
                power = power.saturating_add(validator.voting_power);
            }
        }

        power >= self.get_required_voting_power().await
    }

    /// Get ordered list of validator addresses for leader selection
    pub async fn get_ordered_validators(&self) -> Vec<ValidatorAddress> {
        let validators = self.validators.read().await;
        let mut addresses: Vec<_> = validators
            .values()
            .filter(|v| v.is_active)
            .map(|v| v.address.clone())
            .collect();
        addresses.sort(); // Ensure deterministic ordering
        addresses
    }

    /// Mark validator as inactive (e.g., due to timeout or misbehavior)
    pub async fn mark_inactive(&self, address: &ValidatorAddress) -> ConsensusResult<()> {
        let mut validators = self.validators.write().await;

        if let Some(validator) = validators.get_mut(address) {
            if validator.is_active {
                validator.is_active = false;
                warn!("Marked validator {} as inactive", address);
            }
            Ok(())
        } else {
            Err(ConsensusError::Configuration(format!(
                "Validator {} not found",
                address
            )))
        }
    }

    /// Mark validator as active
    pub async fn mark_active(&self, address: &ValidatorAddress) -> ConsensusResult<()> {
        let mut validators = self.validators.write().await;

        if let Some(validator) = validators.get_mut(address) {
            if !validator.is_active {
                validator.is_active = true;
                validator.last_seen = Some(std::time::SystemTime::now());
                info!("Marked validator {} as active", address);
            }
            Ok(())
        } else {
            Err(ConsensusError::Configuration(format!(
                "Validator {} not found",
                address
            )))
        }
    }

    /// Update last seen timestamp for a validator
    pub async fn update_last_seen(&self, address: &ValidatorAddress) {
        if let Some(validator) = self.validators.write().await.get_mut(address) {
            validator.last_seen = Some(std::time::SystemTime::now());
        }
    }

    /// Get the number of active validators
    pub async fn active_validator_count(&self) -> usize {
        self.validators
            .read()
            .await
            .values()
            .filter(|v| v.is_active)
            .count()
    }

    /// Apply pending validator set changes
    pub async fn apply_pending_changes(&self) -> ConsensusResult<()> {
        let changes = {
            let mut pending = self.pending_changes.write().await;
            std::mem::take(&mut *pending)
        };

        for change in changes {
            match change {
                ValidatorSetChange::Add {
                    address,
                    public_key,
                    voting_power,
                } => {
                    let validator = Validator {
                        address,
                        public_key,
                        voting_power,
                        is_active: true,
                        last_seen: Some(std::time::SystemTime::now()),
                    };
                    self.add_validator(validator).await?;
                }
                ValidatorSetChange::Remove { address } => {
                    self.remove_validator(&address).await?;
                }
                ValidatorSetChange::UpdatePower { address, new_power } => {
                    self.update_voting_power(&address, new_power).await?;
                }
            }
        }

        Ok(())
    }

    /// Add a pending validator set change
    pub async fn add_pending_change(&self, change: ValidatorSetChange) {
        self.pending_changes.write().await.push(change);
    }

    /// Check if an address is a validator
    pub async fn is_validator(&self, address: &ValidatorAddress) -> bool {
        self.validators.read().await.contains_key(address)
    }

    /// Check if an address is an active validator
    pub async fn is_active_validator(&self, address: &ValidatorAddress) -> bool {
        self.validators
            .read()
            .await
            .get(address)
            .map(|v| v.is_active)
            .unwrap_or(false)
    }

    /// Get all validators (for testing)
    pub async fn get_all_validators(&self) -> Vec<Validator> {
        self.validators.read().await.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_validator_set_management() {
        let manager = ValidatorSetManager::new(1, 10);

        // Create validators
        let validator1 = Validator {
            address: ValidatorAddress("validator1".to_string()),
            public_key: "pubkey1".to_string(),
            voting_power: 100,
            is_active: true,
            last_seen: None,
        };

        let validator2 = Validator {
            address: ValidatorAddress("validator2".to_string()),
            public_key: "pubkey2".to_string(),
            voting_power: 200,
            is_active: true,
            last_seen: None,
        };

        // Initialize validator set
        assert!(manager
            .initialize(vec![validator1.clone(), validator2.clone()])
            .await
            .is_ok());

        // Check total voting power
        assert_eq!(manager.get_total_voting_power().await, 300);

        // Check required voting power (2/3 + 1)
        assert_eq!(manager.get_required_voting_power().await, 201);

        // Test sufficient power
        let mut validator_set = HashSet::new();
        validator_set.insert(validator2.address.clone());
        assert!(!manager.has_sufficient_power(&validator_set).await); // 200 < 201

        validator_set.insert(validator1.address.clone());
        assert!(manager.has_sufficient_power(&validator_set).await); // 300 >= 201

        // Test validator removal
        assert!(manager.remove_validator(&validator1.address).await.is_ok());
        assert_eq!(manager.get_total_voting_power().await, 200);

        // Test adding validator
        assert!(manager.add_validator(validator1).await.is_ok());
        assert_eq!(manager.get_total_voting_power().await, 300);
    }

    #[tokio::test]
    async fn test_bft_voting_thresholds() {
        let manager = ValidatorSetManager::new(1, 10);

        // Test with 4 validators (minimum for BFT)
        let validators = vec![
            Validator {
                address: ValidatorAddress("val1".to_string()),
                public_key: "key1".to_string(),
                voting_power: 100,
                is_active: true,
                last_seen: None,
            },
            Validator {
                address: ValidatorAddress("val2".to_string()),
                public_key: "key2".to_string(),
                voting_power: 100,
                is_active: true,
                last_seen: None,
            },
            Validator {
                address: ValidatorAddress("val3".to_string()),
                public_key: "key3".to_string(),
                voting_power: 100,
                is_active: true,
                last_seen: None,
            },
            Validator {
                address: ValidatorAddress("val4".to_string()),
                public_key: "key4".to_string(),
                voting_power: 100,
                is_active: true,
                last_seen: None,
            },
        ];

        assert!(manager.initialize(validators.clone()).await.is_ok());
        assert_eq!(manager.get_total_voting_power().await, 400);
        assert_eq!(manager.get_required_voting_power().await, 267); // (400 * 2) / 3 + 1

        // Test various combinations
        let mut validator_set = HashSet::new();

        // 1 validator: insufficient
        validator_set.insert(ValidatorAddress("val1".to_string()));
        assert!(!manager.has_sufficient_power(&validator_set).await);

        // 2 validators: insufficient (200 < 267)
        validator_set.insert(ValidatorAddress("val2".to_string()));
        assert!(!manager.has_sufficient_power(&validator_set).await);

        // 3 validators: sufficient (300 >= 267)
        validator_set.insert(ValidatorAddress("val3".to_string()));
        assert!(manager.has_sufficient_power(&validator_set).await);

        // 4 validators: definitely sufficient
        validator_set.insert(ValidatorAddress("val4".to_string()));
        assert!(manager.has_sufficient_power(&validator_set).await);
    }

    #[tokio::test]
    async fn test_validator_limits() {
        let manager = ValidatorSetManager::new(2, 3); // Min 2, Max 3

        let validator1 = Validator {
            address: ValidatorAddress("val1".to_string()),
            public_key: "key1".to_string(),
            voting_power: 100,
            is_active: true,
            last_seen: None,
        };

        // Test minimum validators
        assert!(manager.initialize(vec![validator1.clone()]).await.is_err()); // Below minimum

        let validator2 = Validator {
            address: ValidatorAddress("val2".to_string()),
            public_key: "key2".to_string(),
            voting_power: 100,
            is_active: true,
            last_seen: None,
        };

        assert!(manager
            .initialize(vec![validator1.clone(), validator2.clone()])
            .await
            .is_ok()); // At minimum

        // Test maximum validators
        let validator3 = Validator {
            address: ValidatorAddress("val3".to_string()),
            public_key: "key3".to_string(),
            voting_power: 100,
            is_active: true,
            last_seen: None,
        };

        assert!(manager.add_validator(validator3.clone()).await.is_ok()); // At maximum

        let validator4 = Validator {
            address: ValidatorAddress("val4".to_string()),
            public_key: "key4".to_string(),
            voting_power: 100,
            is_active: true,
            last_seen: None,
        };

        assert!(manager.add_validator(validator4).await.is_err()); // Above maximum
    }

    #[tokio::test]
    async fn test_validator_activity_tracking() {
        let manager = ValidatorSetManager::new(1, 10);

        let validator = Validator {
            address: ValidatorAddress("val1".to_string()),
            public_key: "key1".to_string(),
            voting_power: 100,
            is_active: true,
            last_seen: None,
        };

        assert!(manager.initialize(vec![validator.clone()]).await.is_ok());

        // Test marking as inactive
        assert!(manager.mark_inactive(&validator.address).await.is_ok());
        assert!(!manager.is_active_validator(&validator.address).await);
        assert_eq!(manager.active_validator_count().await, 0);

        // Test marking as active
        assert!(manager.mark_active(&validator.address).await.is_ok());
        assert!(manager.is_active_validator(&validator.address).await);
        assert_eq!(manager.active_validator_count().await, 1);

        // Test updating last seen
        manager.update_last_seen(&validator.address).await;
        let validators = manager.get_all_validators().await;
        assert!(validators[0].last_seen.is_some());
    }

    #[tokio::test]
    async fn test_ordered_validators() {
        let manager = ValidatorSetManager::new(1, 10);

        let validators = vec![
            Validator {
                address: ValidatorAddress("zebra".to_string()),
                public_key: "key1".to_string(),
                voting_power: 100,
                is_active: true,
                last_seen: None,
            },
            Validator {
                address: ValidatorAddress("alpha".to_string()),
                public_key: "key2".to_string(),
                voting_power: 100,
                is_active: true,
                last_seen: None,
            },
            Validator {
                address: ValidatorAddress("beta".to_string()),
                public_key: "key3".to_string(),
                voting_power: 100,
                is_active: false, // Inactive
                last_seen: None,
            },
        ];

        assert!(manager.initialize(validators).await.is_ok());

        let ordered = manager.get_ordered_validators().await;
        // Should be sorted alphabetically and only include active validators
        assert_eq!(ordered.len(), 2);
        assert_eq!(ordered[0], ValidatorAddress("alpha".to_string()));
        assert_eq!(ordered[1], ValidatorAddress("zebra".to_string()));
    }

    #[tokio::test]
    async fn test_edge_case_voting_powers() {
        let manager = ValidatorSetManager::new(1, 10);

        // Test with unequal voting powers
        let validators = vec![
            Validator {
                address: ValidatorAddress("large".to_string()),
                public_key: "key1".to_string(),
                voting_power: 700,
                is_active: true,
                last_seen: None,
            },
            Validator {
                address: ValidatorAddress("medium".to_string()),
                public_key: "key2".to_string(),
                voting_power: 200,
                is_active: true,
                last_seen: None,
            },
            Validator {
                address: ValidatorAddress("small".to_string()),
                public_key: "key3".to_string(),
                voting_power: 100,
                is_active: true,
                last_seen: None,
            },
        ];

        assert!(manager.initialize(validators).await.is_ok());
        assert_eq!(manager.get_total_voting_power().await, 1000);
        assert_eq!(manager.get_required_voting_power().await, 667); // (1000 * 2) / 3 + 1

        // Large validator alone should be sufficient (700 >= 667)
        let mut validator_set = HashSet::new();
        validator_set.insert(ValidatorAddress("large".to_string()));
        assert!(manager.has_sufficient_power(&validator_set).await);

        // Medium + small should not be sufficient
        validator_set.clear();
        validator_set.insert(ValidatorAddress("medium".to_string()));
        validator_set.insert(ValidatorAddress("small".to_string()));
        assert!(!manager.has_sufficient_power(&validator_set).await); // 300 < 667
    }

    #[tokio::test]
    async fn test_validator_exists_checks() {
        let manager = ValidatorSetManager::new(1, 10);

        let validator = Validator {
            address: ValidatorAddress("val1".to_string()),
            public_key: "key1".to_string(),
            voting_power: 100,
            is_active: true,
            last_seen: None,
        };

        // Before initialization
        assert!(!manager.is_validator(&validator.address).await);
        assert!(!manager.is_active_validator(&validator.address).await);

        // After initialization
        assert!(manager.initialize(vec![validator.clone()]).await.is_ok());
        assert!(manager.is_validator(&validator.address).await);
        assert!(manager.is_active_validator(&validator.address).await);

        // After removal - should fail because it's the only validator
        assert!(manager.remove_validator(&validator.address).await.is_err());
        assert!(manager.is_validator(&validator.address).await);
        assert!(manager.is_active_validator(&validator.address).await);
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        let manager = ValidatorSetManager::new(1, 100);

        let validator = Validator {
            address: ValidatorAddress("val1".to_string()),
            public_key: "key1".to_string(),
            voting_power: 100,
            is_active: true,
            last_seen: None,
        };

        assert!(manager.initialize(vec![validator.clone()]).await.is_ok());

        // Test concurrent updates
        let manager = Arc::new(manager);
        let addr = validator.address.clone();

        let mut handles = vec![];
        for _ in 0..10 {
            let manager_clone = manager.clone();
            let addr_clone = addr.clone();
            handles.push(tokio::spawn(async move {
                manager_clone.update_last_seen(&addr_clone).await;
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        // Should still be consistent
        assert!(manager.is_validator(&validator.address).await);
        assert_eq!(manager.get_total_voting_power().await, 100);
    }

    #[tokio::test]
    async fn test_zero_voting_power() {
        let manager = ValidatorSetManager::new(1, 10);

        let validator = Validator {
            address: ValidatorAddress("val1".to_string()),
            public_key: "key1".to_string(),
            voting_power: 0, // Zero voting power
            is_active: true,
            last_seen: None,
        };

        assert!(manager.initialize(vec![validator]).await.is_ok());
        assert_eq!(manager.get_total_voting_power().await, 0);
        assert_eq!(manager.get_required_voting_power().await, 1); // 0 * 2 / 3 + 1 = 1

        let mut validator_set = HashSet::new();
        validator_set.insert(ValidatorAddress("val1".to_string()));
        assert!(!manager.has_sufficient_power(&validator_set).await); // 0 < 1
    }
}
