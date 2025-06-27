//! Malachite consensus validator implementation
//!
//! This module provides the validator functionality for the Malachite BFT consensus protocol.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::config::ConsensusParams;
use super::types::{ConsensusPhase, Round, ValidatorAddress};
use crate::error::ConsensusError;

/// Malachite consensus validator
#[derive(Debug)]
pub struct MalachiteValidator {
    /// Validator configuration
    config: ConsensusParams,

    /// Validator state
    state: Arc<RwLock<ValidatorState>>,
}

/// Internal validator state
#[derive(Debug)]
struct ValidatorState {
    /// Current height
    current_height: u64,

    /// Current round
    current_round: Round,

    /// Current consensus phase
    current_phase: ConsensusPhase,

    /// Is validator active
    is_active: bool,

    /// Last block time
    last_block_time: Option<Instant>,

    /// Known validators and their voting power
    validators: HashMap<ValidatorAddress, u64>,

    /// Votes collected for current round
    prevotes: HashMap<ValidatorAddress, String>, // validator -> block_hash

    /// Precommits collected for current round
    precommits: HashMap<ValidatorAddress, String>, // validator -> block_hash

    /// Current proposed block hash
    proposed_block: Option<String>,

    /// Round timeouts
    round_start_time: Option<Instant>,

    /// Locked value (for BFT safety)
    locked_value: Option<String>,

    /// Locked round
    locked_round: Option<Round>,

    /// Valid value (for BFT liveness)
    valid_value: Option<String>,

    /// Valid round
    valid_round: Option<Round>,
}

impl Default for ValidatorState {
    fn default() -> Self {
        Self {
            current_height: 0,
            current_round: Round::new(0),
            current_phase: ConsensusPhase::NewHeight,
            is_active: false,
            last_block_time: None,
            validators: HashMap::new(),
            prevotes: HashMap::new(),
            precommits: HashMap::new(),
            proposed_block: None,
            round_start_time: None,
            locked_value: None,
            locked_round: None,
            valid_value: None,
            valid_round: None,
        }
    }
}

impl MalachiteValidator {
    /// Create a new validator instance
    pub fn new(config: ConsensusParams) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(ValidatorState::default())),
        }
    }

    /// Initialize the validator
    pub async fn initialize(&mut self) -> Result<(), ConsensusError> {
        info!("Initializing Malachite validator");

        let mut state = self.state.write().await;
        state.is_active = true;
        state.current_height = 0;
        state.current_round = Round::new(0);

        info!("Malachite validator initialized successfully");
        Ok(())
    }

    /// Start the validator
    pub async fn start(&mut self) -> Result<(), ConsensusError> {
        info!("Starting Malachite validator");

        let mut state = self.state.write().await;
        if !state.is_active {
            return Err(ConsensusError::Configuration(
                "Validator not initialized".to_string(),
            ));
        }

        state.last_block_time = Some(std::time::Instant::now());

        info!("Malachite validator started successfully");
        Ok(())
    }

    /// Stop the validator
    pub async fn stop(&mut self) -> Result<(), ConsensusError> {
        info!("Stopping Malachite validator");

        let mut state = self.state.write().await;
        state.is_active = false;
        state.last_block_time = None;

        info!("Malachite validator stopped successfully");
        Ok(())
    }

    /// Get validator status
    pub async fn is_active(&self) -> bool {
        self.state.read().await.is_active
    }

    /// Get current height
    pub async fn current_height(&self) -> u64 {
        self.state.read().await.current_height
    }

    /// Get current round
    pub async fn current_round(&self) -> u32 {
        self.state.read().await.current_round.as_u32()
    }

    /// Advance to next height
    pub async fn advance_height(&mut self) -> Result<(), ConsensusError> {
        let mut state = self.state.write().await;

        if !state.is_active {
            return Err(ConsensusError::Configuration(
                "Validator not active".to_string(),
            ));
        }

        state.current_height += 1;
        state.current_round = Round::new(0);
        state.current_phase = ConsensusPhase::NewHeight;
        state.last_block_time = Some(Instant::now());

        // Reset round-specific state
        state.prevotes.clear();
        state.precommits.clear();
        state.proposed_block = None;
        state.round_start_time = Some(Instant::now());

        debug!("Advanced to height {}", state.current_height);
        Ok(())
    }

    /// Advance to next round
    pub async fn advance_round(&mut self) -> Result<(), ConsensusError> {
        let mut state = self.state.write().await;

        if !state.is_active {
            return Err(ConsensusError::Configuration(
                "Validator not active".to_string(),
            ));
        }

        state.current_round = state.current_round.increment();
        state.current_phase = ConsensusPhase::Propose;

        // Reset round-specific state
        state.prevotes.clear();
        state.precommits.clear();
        state.proposed_block = None;
        state.round_start_time = Some(Instant::now());

        debug!(
            "Advanced to round {} at height {}",
            state.current_round, state.current_height
        );
        Ok(())
    }

    /// Get validator configuration
    pub fn config(&self) -> &ConsensusParams {
        &self.config
    }

    /// Update validator configuration
    pub fn update_config(&mut self, config: ConsensusParams) {
        info!("Updating validator configuration");
        self.config = config;
    }

    /// Add a validator to the known validator set
    pub async fn add_validator(&self, address: ValidatorAddress, voting_power: u64) {
        let mut state = self.state.write().await;
        state.validators.insert(address.clone(), voting_power);
        debug!(
            "Added validator {} with voting power {}",
            address, voting_power
        );
    }

    /// Propose a block for the current round
    pub async fn propose_block(&self, block_hash: String) -> Result<(), ConsensusError> {
        let mut state = self.state.write().await;

        if state.current_phase != ConsensusPhase::Propose {
            return Err(ConsensusError::Configuration(format!(
                "Cannot propose in phase {:?}",
                state.current_phase
            )));
        }

        state.proposed_block = Some(block_hash.clone());
        state.current_phase = ConsensusPhase::Prevote;

        debug!(
            "Proposed block {} for round {}",
            block_hash, state.current_round
        );
        Ok(())
    }

    /// Cast a prevote for the given block hash
    pub async fn prevote(
        &self,
        validator: ValidatorAddress,
        block_hash: String,
    ) -> Result<(), ConsensusError> {
        let mut state = self.state.write().await;

        if !state.validators.contains_key(&validator) {
            return Err(ConsensusError::Configuration(format!(
                "Unknown validator: {validator}"
            )));
        }

        state.prevotes.insert(validator.clone(), block_hash.clone());
        debug!(
            "Received prevote from {} for block {}",
            validator, block_hash
        );

        // Check if we have +2/3 prevotes for any value
        if self.has_two_thirds_majority(&state.prevotes, &state.validators) {
            state.current_phase = ConsensusPhase::Precommit;
            debug!("Achieved +2/3 prevotes, moving to precommit phase");
        }

        Ok(())
    }

    /// Cast a precommit for the given block hash
    pub async fn precommit(
        &self,
        validator: ValidatorAddress,
        block_hash: String,
    ) -> Result<(), ConsensusError> {
        let mut state = self.state.write().await;

        if !state.validators.contains_key(&validator) {
            return Err(ConsensusError::Configuration(format!(
                "Unknown validator: {validator}"
            )));
        }

        state
            .precommits
            .insert(validator.clone(), block_hash.clone());
        debug!(
            "Received precommit from {} for block {}",
            validator, block_hash
        );

        // Check if we have +2/3 precommits for any value
        if self.has_two_thirds_majority(&state.precommits, &state.validators) {
            state.current_phase = ConsensusPhase::Commit;
            debug!("Achieved +2/3 precommits, moving to commit phase");
        }

        Ok(())
    }

    /// Check if ready to commit a block
    pub async fn can_commit(&self) -> Option<String> {
        let state = self.state.read().await;

        if state.current_phase == ConsensusPhase::Commit {
            // Find the block hash with +2/3 precommits
            self.get_majority_value(&state.precommits, &state.validators)
        } else {
            None
        }
    }

    /// Get the current consensus phase
    pub async fn current_phase(&self) -> ConsensusPhase {
        self.state.read().await.current_phase.clone()
    }

    /// Check if round timeout has expired
    pub async fn is_round_timeout(&self) -> bool {
        let state = self.state.read().await;

        if let Some(start_time) = state.round_start_time {
            let timeout = match state.current_phase {
                ConsensusPhase::Propose => Duration::from_millis(self.config.timeout_propose_ms),
                ConsensusPhase::Prevote => Duration::from_millis(self.config.timeout_prevote_ms),
                ConsensusPhase::Precommit => {
                    Duration::from_millis(self.config.timeout_precommit_ms)
                }
                _ => Duration::from_secs(10), // Default timeout
            };

            start_time.elapsed() > timeout
        } else {
            false
        }
    }

    /// Check if we have a 2/3 majority for any value
    fn has_two_thirds_majority(
        &self,
        votes: &HashMap<ValidatorAddress, String>,
        validators: &HashMap<ValidatorAddress, u64>,
    ) -> bool {
        let total_power: u64 = validators.values().sum();
        let required_power = (total_power * 2) / 3 + 1;

        // Count votes by value
        let mut value_power: HashMap<String, u64> = HashMap::new();
        for (validator, value) in votes {
            if let Some(power) = validators.get(validator) {
                *value_power.entry(value.clone()).or_insert(0) += power;
            }
        }

        // Check if any value has +2/3 majority
        value_power.values().any(|&power| power >= required_power)
    }

    /// Get the value with 2/3+ majority, if any
    fn get_majority_value(
        &self,
        votes: &HashMap<ValidatorAddress, String>,
        validators: &HashMap<ValidatorAddress, u64>,
    ) -> Option<String> {
        let total_power: u64 = validators.values().sum();
        let required_power = (total_power * 2) / 3 + 1;

        // Count votes by value
        let mut value_power: HashMap<String, u64> = HashMap::new();
        for (validator, value) in votes {
            if let Some(power) = validators.get(validator) {
                *value_power.entry(value.clone()).or_insert(0) += power;
            }
        }

        // Return the value with +2/3 majority
        for (value, power) in value_power {
            if power >= required_power {
                return Some(value);
            }
        }

        None
    }

    /// Handle round timeout - advance to next round
    pub async fn handle_timeout(&mut self) -> Result<(), ConsensusError> {
        warn!("Round timeout detected, advancing to next round");
        self.advance_round().await
    }

    /// Reset consensus state for new height
    pub async fn reset_for_new_height(&mut self, height: u64) -> Result<(), ConsensusError> {
        let mut state = self.state.write().await;

        state.current_height = height;
        state.current_round = Round::new(0);
        state.current_phase = ConsensusPhase::NewHeight;
        state.prevotes.clear();
        state.precommits.clear();
        state.proposed_block = None;
        state.round_start_time = Some(Instant::now());
        state.locked_value = None;
        state.locked_round = None;
        state.valid_value = None;
        state.valid_round = None;

        debug!("Reset consensus state for height {}", height);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_validator_lifecycle() {
        let config = ConsensusParams::default();
        let mut validator = MalachiteValidator::new(config);

        // Test initialization
        assert!(validator.initialize().await.is_ok());
        assert!(validator.is_active().await);
        assert_eq!(validator.current_height().await, 0);
        assert_eq!(validator.current_round().await, 0);

        // Test starting
        assert!(validator.start().await.is_ok());

        // Test height advancement
        assert!(validator.advance_height().await.is_ok());
        assert_eq!(validator.current_height().await, 1);
        assert_eq!(validator.current_round().await, 0);

        // Test round advancement
        assert!(validator.advance_round().await.is_ok());
        assert_eq!(validator.current_round().await, 1);

        // Test stopping
        assert!(validator.stop().await.is_ok());
        assert!(!validator.is_active().await);
    }
}
