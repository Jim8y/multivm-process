//! Leader selection mechanism for consensus
//!
//! This module implements round-robin leader selection for the MultiVM consensus.

use crate::error::{ConsensusError, ConsensusResult};
use crate::malachite::types::{Round, ValidatorAddress};
use crate::traits::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tracing::{debug, info};

/// Leader selection strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LeaderSelectionStrategy {
    /// Round-robin selection based on validator order
    RoundRobin,
    /// Weighted selection based on stake (future enhancement)
    WeightedRoundRobin,
}

/// Leader selector implementation
#[derive(Debug, Clone)]
pub struct LeaderSelector {
    /// Selection strategy
    strategy: LeaderSelectionStrategy,
    /// Ordered list of validators
    validators: Vec<ValidatorAddress>,
    /// Validator weights (for weighted strategies)
    weights: BTreeMap<ValidatorAddress, u64>,
}

impl LeaderSelector {
    /// Create a new leader selector with round-robin strategy
    pub fn new_round_robin(mut validators: Vec<ValidatorAddress>) -> Self {
        // Sort validators for deterministic ordering
        validators.sort();

        Self {
            strategy: LeaderSelectionStrategy::RoundRobin,
            validators,
            weights: BTreeMap::new(),
        }
    }

    /// Create a new leader selector with weighted round-robin
    pub fn new_weighted(validators: Vec<(ValidatorAddress, u64)>) -> Self {
        let mut weights = BTreeMap::new();
        let mut validator_list = Vec::new();

        for (validator, weight) in validators {
            weights.insert(validator.clone(), weight);
            validator_list.push(validator);
        }

        // Sort validators by address for deterministic ordering
        validator_list.sort();

        Self {
            strategy: LeaderSelectionStrategy::WeightedRoundRobin,
            validators: validator_list,
            weights,
        }
    }

    /// Get the leader for a specific height and round
    pub fn get_leader(&self, height: u64, round: Round) -> ConsensusResult<ValidatorAddress> {
        if self.validators.is_empty() {
            return Err(ConsensusError::Configuration(
                "No validators available for leader selection".to_string(),
            ));
        }

        match self.strategy {
            LeaderSelectionStrategy::RoundRobin => self.round_robin_selection(height, round),
            LeaderSelectionStrategy::WeightedRoundRobin => {
                self.weighted_round_robin_selection(height, round)
            }
        }
    }

    /// Simple round-robin leader selection
    fn round_robin_selection(
        &self,
        height: u64,
        round: Round,
    ) -> ConsensusResult<ValidatorAddress> {
        // Use both height and round to determine the leader
        // This ensures leader rotation even within the same height
        let total_rounds = height
            .saturating_mul(1000)
            .saturating_add(round.as_u32() as u64);
        let leader_index = (total_rounds as usize) % self.validators.len();

        let leader = self.validators[leader_index].clone();
        debug!(
            "Selected leader {} for height {} round {} (index: {})",
            leader, height, round, leader_index
        );

        Ok(leader)
    }

    /// Weighted round-robin leader selection based on voting power
    fn weighted_round_robin_selection(
        &self,
        height: u64,
        round: Round,
    ) -> ConsensusResult<ValidatorAddress> {
        use sha2::{Digest, Sha256};

        // Create deterministic seed from height and round
        let mut hasher = Sha256::new();
        hasher.update(b"weighted_leader_selection");
        hasher.update(height.to_le_bytes());
        hasher.update(round.as_u32().to_le_bytes());
        let hash = hasher.finalize();

        // Convert hash to a deterministic number
        let seed = u64::from_le_bytes(hash[0..8].try_into().unwrap());

        // For production implementation with weights:
        // 1. Get validator weights from validator set
        // 2. Calculate cumulative weights
        // 3. Use seed to select based on weight distribution

        // For now, use simple round-robin with deterministic offset
        let offset = (seed % self.validators.len() as u64) as usize;
        let total_rounds = height
            .saturating_mul(1000)
            .saturating_add(round.as_u32() as u64);
        let leader_index = ((total_rounds as usize) + offset) % self.validators.len();
        let leader = self.validators[leader_index].clone();

        debug!(
            "Selected weighted leader {} for height {} round {} (seed: {}, offset: {})",
            leader, height, round, seed, offset
        );

        Ok(leader)
    }

    /// Check if a given validator is the leader for the specified height and round
    pub fn is_leader(
        &self,
        validator: &ValidatorAddress,
        height: u64,
        round: Round,
    ) -> ConsensusResult<bool> {
        let leader = self.get_leader(height, round)?;
        Ok(leader == *validator)
    }

    /// Update the validator set
    pub fn update_validators(&mut self, validators: Vec<ValidatorAddress>) {
        info!(
            "Updating validator set with {} validators",
            validators.len()
        );
        self.validators = validators;
        self.validators.sort(); // Ensure deterministic ordering
    }

    /// Update validator weights (for weighted strategies)
    pub fn update_weights(&mut self, weights: BTreeMap<ValidatorAddress, u64>) {
        self.weights = weights;
    }

    /// Get the current validator set
    pub fn validators(&self) -> &[ValidatorAddress] {
        &self.validators
    }

    /// Get the number of validators
    pub fn validator_count(&self) -> usize {
        self.validators.len()
    }

    /// Get the next leader after the current one
    pub fn get_next_leader(
        &self,
        current_height: u64,
        current_round: Round,
    ) -> ConsensusResult<ValidatorAddress> {
        // Simply get the leader for the next round
        self.get_leader(current_height, current_round.increment())
    }
}

/// Convert NodeId to ValidatorAddress
impl From<NodeId> for ValidatorAddress {
    fn from(node_id: NodeId) -> Self {
        ValidatorAddress(node_id)
    }
}

/// Convert ValidatorAddress to NodeId
impl From<ValidatorAddress> for NodeId {
    fn from(addr: ValidatorAddress) -> Self {
        addr.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_robin_selection() {
        let validators = vec![
            ValidatorAddress("validator1".to_string()),
            ValidatorAddress("validator2".to_string()),
            ValidatorAddress("validator3".to_string()),
        ];

        let selector = LeaderSelector::new_round_robin(validators.clone());

        // Test leader selection for different heights and rounds
        assert_eq!(
            selector.get_leader(0, Round::new(0)).unwrap(),
            ValidatorAddress("validator1".to_string())
        );
        assert_eq!(
            selector.get_leader(0, Round::new(1)).unwrap(),
            ValidatorAddress("validator2".to_string())
        );
        assert_eq!(
            selector.get_leader(0, Round::new(2)).unwrap(),
            ValidatorAddress("validator3".to_string())
        );
        assert_eq!(
            selector.get_leader(0, Round::new(3)).unwrap(),
            ValidatorAddress("validator1".to_string())
        );

        // Test with different height
        assert_eq!(
            selector.get_leader(1, Round::new(0)).unwrap(),
            ValidatorAddress("validator2".to_string())
        );
    }

    #[test]
    fn test_is_leader() {
        let validators = vec![
            ValidatorAddress("validator1".to_string()),
            ValidatorAddress("validator2".to_string()),
        ];

        let selector = LeaderSelector::new_round_robin(validators);

        assert!(selector
            .is_leader(
                &ValidatorAddress("validator1".to_string()),
                0,
                Round::new(0)
            )
            .unwrap());
        assert!(!selector
            .is_leader(
                &ValidatorAddress("validator2".to_string()),
                0,
                Round::new(0)
            )
            .unwrap());
        assert!(selector
            .is_leader(
                &ValidatorAddress("validator2".to_string()),
                0,
                Round::new(1)
            )
            .unwrap());
    }

    #[test]
    fn test_empty_validators() {
        let selector = LeaderSelector::new_round_robin(vec![]);
        assert!(selector.get_leader(0, Round::new(0)).is_err());
    }

    #[test]
    fn test_deterministic_selection() {
        let validators = vec![
            ValidatorAddress("validator1".to_string()),
            ValidatorAddress("validator2".to_string()),
            ValidatorAddress("validator3".to_string()),
            ValidatorAddress("validator4".to_string()),
        ];

        let selector = LeaderSelector::new_round_robin(validators);

        // Test that the same height+round always produces the same leader
        for height in 0..10 {
            for round in 0..20 {
                let leader1 = selector.get_leader(height, Round::new(round)).unwrap();
                let leader2 = selector.get_leader(height, Round::new(round)).unwrap();
                assert_eq!(leader1, leader2, "Leader selection must be deterministic");
            }
        }
    }

    #[test]
    fn test_fair_rotation() {
        let validators = vec![
            ValidatorAddress("validator1".to_string()),
            ValidatorAddress("validator2".to_string()),
            ValidatorAddress("validator3".to_string()),
        ];

        let selector = LeaderSelector::new_round_robin(validators.clone());
        let mut leader_counts = std::collections::HashMap::new();

        // Test fair rotation over many rounds
        for round in 0..30 {
            let leader = selector.get_leader(0, Round::new(round)).unwrap();
            *leader_counts.entry(leader).or_insert(0) += 1;
        }

        // Each validator should be selected exactly 10 times (30 rounds / 3 validators)
        for validator in &validators {
            assert_eq!(
                leader_counts[validator], 10,
                "Leader selection should be fair"
            );
        }
    }

    #[test]
    fn test_validator_update() {
        let initial_validators = vec![
            ValidatorAddress("validator1".to_string()),
            ValidatorAddress("validator2".to_string()),
        ];

        let mut selector = LeaderSelector::new_round_robin(initial_validators);

        // Test initial state
        assert_eq!(selector.validator_count(), 2);
        assert_eq!(
            selector.get_leader(0, Round::new(0)).unwrap(),
            ValidatorAddress("validator1".to_string())
        );

        // Update validators
        let new_validators = vec![
            ValidatorAddress("validator1".to_string()),
            ValidatorAddress("validator2".to_string()),
            ValidatorAddress("validator3".to_string()),
        ];
        selector.update_validators(new_validators);

        // Test updated state
        assert_eq!(selector.validator_count(), 3);
        assert_eq!(
            selector.get_leader(0, Round::new(2)).unwrap(),
            ValidatorAddress("validator3".to_string())
        );
    }

    #[test]
    fn test_get_next_leader() {
        let validators = vec![
            ValidatorAddress("validator1".to_string()),
            ValidatorAddress("validator2".to_string()),
            ValidatorAddress("validator3".to_string()),
        ];

        let selector = LeaderSelector::new_round_robin(validators);

        // Test next leader calculation
        assert_eq!(
            selector.get_next_leader(0, Round::new(0)).unwrap(),
            ValidatorAddress("validator2".to_string())
        );
        assert_eq!(
            selector.get_next_leader(0, Round::new(2)).unwrap(),
            ValidatorAddress("validator1".to_string())
        );
    }

    #[test]
    fn test_validator_sorting() {
        // Test that validators are sorted for deterministic behavior
        let unsorted_validators = vec![
            ValidatorAddress("validator3".to_string()),
            ValidatorAddress("validator1".to_string()),
            ValidatorAddress("validator2".to_string()),
        ];

        let selector = LeaderSelector::new_round_robin(unsorted_validators);

        // Should be sorted: validator1, validator2, validator3
        assert_eq!(
            selector.get_leader(0, Round::new(0)).unwrap(),
            ValidatorAddress("validator1".to_string())
        );
        assert_eq!(
            selector.get_leader(0, Round::new(1)).unwrap(),
            ValidatorAddress("validator2".to_string())
        );
        assert_eq!(
            selector.get_leader(0, Round::new(2)).unwrap(),
            ValidatorAddress("validator3".to_string())
        );
    }

    #[test]
    fn test_large_heights_and_rounds() {
        let validators = vec![
            ValidatorAddress("validator1".to_string()),
            ValidatorAddress("validator2".to_string()),
        ];

        let selector = LeaderSelector::new_round_robin(validators);

        // Test with large values to ensure no overflow
        let large_height = u64::MAX / 2;
        let large_round = u32::MAX / 2;

        let leader = selector.get_leader(large_height, Round::new(large_round));
        assert!(
            leader.is_ok(),
            "Should handle large height and round values"
        );
    }

    #[test]
    fn test_single_validator() {
        let validators = vec![ValidatorAddress("solo_validator".to_string())];
        let selector = LeaderSelector::new_round_robin(validators);

        // Single validator should always be the leader
        for round in 0..10 {
            assert_eq!(
                selector.get_leader(0, Round::new(round)).unwrap(),
                ValidatorAddress("solo_validator".to_string())
            );
            assert!(selector
                .is_leader(
                    &ValidatorAddress("solo_validator".to_string()),
                    0,
                    Round::new(round)
                )
                .unwrap());
        }
    }

    #[test]
    fn test_node_id_conversions() {
        let node_id = "test_node_123".to_string();
        let validator_addr = ValidatorAddress::from(node_id.clone());
        let converted_back: NodeId = validator_addr.into();

        assert_eq!(node_id, converted_back);
    }
}
