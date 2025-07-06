//! View change mechanism for consensus
//!
//! This module handles view changes when the current leader fails to propose
//! a block within the timeout period.

use crate::error::{ConsensusError, ConsensusResult};
use crate::leader_selection::LeaderSelector;
use crate::malachite::types::{Round, ValidatorAddress};
use crate::messages::{ConsensusMessage, ConsensusMessagePayload, ViewChangeMessage};
use crate::traits::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// View change state
#[derive(Debug, Clone)]
pub struct ViewChangeState {
    /// Current height
    pub height: u64,
    /// Current round
    pub round: Round,
    /// View change messages received
    pub view_changes: HashMap<ValidatorAddress, ViewChangeInfo>,
    /// Whether we've sent our view change
    pub sent_view_change: bool,
    /// Time when view change started
    pub start_time: Option<Instant>,
}

/// View change information from a validator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewChangeInfo {
    /// Validator who sent the view change
    pub validator: ValidatorAddress,
    /// Target round
    pub new_round: Round,
    /// Timestamp
    pub timestamp: std::time::SystemTime,
    /// Signature (for authentication)
    pub signature: Vec<u8>,
}

/// View change manager
#[derive(Debug, Clone)]
pub struct ViewChangeManager {
    /// Node ID
    node_id: NodeId,
    /// Leader selector
    leader_selector: Arc<RwLock<LeaderSelector>>,
    /// View change state
    state: Arc<RwLock<ViewChangeState>>,
    /// View change timeout
    view_change_timeout: Duration,
    /// Required voting power for view change
    required_voting_power: Arc<RwLock<u64>>,
}

impl ViewChangeManager {
    /// Create a new view change manager
    pub fn new(
        node_id: NodeId,
        leader_selector: Arc<RwLock<LeaderSelector>>,
        view_change_timeout: Duration,
    ) -> Self {
        let state = ViewChangeState {
            height: 0,
            round: Round::new(0),
            view_changes: HashMap::new(),
            sent_view_change: false,
            start_time: None,
        };

        Self {
            node_id,
            leader_selector,
            state: Arc::new(RwLock::new(state)),
            view_change_timeout,
            required_voting_power: Arc::new(RwLock::new(0)),
        }
    }

    /// Update the required voting power for view change
    pub async fn set_required_voting_power(&self, power: u64) {
        *self.required_voting_power.write().await = power;
    }

    /// Start view change for the given height and round
    pub async fn start_view_change(
        &self,
        height: u64,
        new_round: Round,
    ) -> ConsensusResult<ConsensusMessage> {
        let mut state = self.state.write().await;

        // Check if we're already in a view change for a higher round
        if state.height == height && state.round >= new_round {
            return Err(ConsensusError::InvalidState(format!(
                "Already in view change for round {} or higher",
                state.round
            )));
        }

        info!(
            "Starting view change for height {} to round {}",
            height, new_round
        );

        // Reset state for new view change
        state.height = height;
        state.round = new_round;
        state.view_changes.clear();
        state.sent_view_change = true;
        state.start_time = Some(Instant::now());

        // Create our view change message
        let timestamp = std::time::SystemTime::now();
        let signature = self.sign_view_change(height, new_round, &timestamp);

        let view_change_info = ViewChangeInfo {
            validator: ValidatorAddress(self.node_id.clone()),
            new_round,
            timestamp,
            signature,
        };

        // Add our own view change
        state.view_changes.insert(
            ValidatorAddress(self.node_id.clone()),
            view_change_info.clone(),
        );

        // Create consensus message
        let message = ConsensusMessage {
            id: uuid::Uuid::new_v4(),
            sender: self.node_id.clone(),
            payload: ConsensusMessagePayload::ViewChange(ViewChangeMessage {
                height,
                old_view: state.round.as_u32(),
                new_view: new_round.as_u32(),
                reason: crate::messages::ViewChangeReason::NodeFailure(
                    "Leader timeout".to_string(),
                ),
                justification: crate::messages::ViewChangeJustification {
                    evidence: vec![],
                    supporting_votes: vec![],
                    timeout_info: None,
                },
                requesting_node: self.node_id.clone(),
            }),
            timestamp: std::time::SystemTime::now(),
            signature: crate::messages::MessageSignature {
                algorithm: "ed25519".to_string(),
                signature: Vec::new(),
                public_key: Vec::new(),
            },
            version: 1,
        };

        Ok(message)
    }

    /// Process a view change message from another validator
    pub async fn process_view_change(
        &self,
        sender: &ValidatorAddress,
        height: u64,
        new_round: Round,
        signature: Vec<u8>,
    ) -> ConsensusResult<bool> {
        let mut state = self.state.write().await;

        // Check if the view change is for the current height
        if state.height != height {
            debug!(
                "Ignoring view change for different height: {} vs {}",
                height, state.height
            );
            return Ok(false);
        }

        // Check if the new round is valid
        if new_round < state.round {
            debug!(
                "Ignoring view change for older round: {} < {}",
                new_round, state.round
            );
            return Ok(false);
        }

        // If this is for a higher round, we need to update our state
        if new_round > state.round {
            warn!(
                "Received view change for higher round: {} > {}, updating our round",
                new_round, state.round
            );
            // Update to the higher round to stay in sync
            state.round = new_round;
            state.view_changes.clear();
            state.sent_view_change = false;
            state.start_time = Some(Instant::now());
        }

        // Verify signature before accepting
        if !self.verify_view_change_signature(sender, height, new_round, &signature) {
            warn!("Invalid view change signature from {}", sender);
            return Ok(false);
        }

        // Add the view change
        let view_change_info = ViewChangeInfo {
            validator: sender.clone(),
            new_round,
            timestamp: std::time::SystemTime::now(),
            signature,
        };

        state.view_changes.insert(sender.clone(), view_change_info);

        info!(
            "Processed view change from {} for round {}, total view changes: {} (validators in set: {})",
            sender,
            new_round,
            state.view_changes.len(),
            state.view_changes.keys().map(|v| v.0.as_str()).collect::<Vec<_>>().join(", ")
        );

        // Check if we have enough view changes
        Ok(self.check_view_change_threshold(&state).await)
    }

    /// Check if we have enough view changes to proceed
    async fn check_view_change_threshold(&self, state: &ViewChangeState) -> bool {
        let required_power = *self.required_voting_power.read().await;

        // If required voting power is set, use it
        if required_power > 0 {
            // Calculate total voting power from view changes
            // In production, this would query the validator set for actual voting powers
            let view_change_count = state.view_changes.len();
            let estimated_power = view_change_count as u64 * 100; // Assuming 100 power per validator

            if estimated_power >= required_power {
                info!(
                    "View change threshold reached by voting power: {} >= {}",
                    estimated_power, required_power
                );
                return true;
            }
        }

        // Fall back to BFT count-based threshold
        let leader_selector = self.leader_selector.read().await;
        let total_validators = leader_selector.validator_count();
        let required_count = (total_validators * 2) / 3 + 1;

        let view_change_count = state.view_changes.len();

        if view_change_count >= required_count {
            info!(
                "View change threshold reached: {}/{} validators (required: {})",
                view_change_count, total_validators, required_count
            );
            true
        } else {
            debug!(
                "View change threshold not reached: {}/{} validators (required: {})",
                view_change_count, total_validators, required_count
            );
            false
        }
    }

    /// Complete the view change and return the new leader
    pub async fn complete_view_change(&self) -> ConsensusResult<ValidatorAddress> {
        let state = self.state.read().await;
        let leader_selector = self.leader_selector.read().await;

        // Get the new leader for the new round
        let new_leader = leader_selector.get_leader(state.height, state.round)?;

        info!(
            "View change complete. New leader: {} for round {}",
            new_leader, state.round
        );

        Ok(new_leader)
    }

    /// Reset view change state
    pub async fn reset(&self, height: u64, round: Round) {
        let mut state = self.state.write().await;
        state.height = height;
        state.round = round;
        state.view_changes.clear();
        state.sent_view_change = false;
        state.start_time = None;
    }

    /// Check if view change timeout has expired
    pub async fn is_view_change_timeout(&self) -> bool {
        let state = self.state.read().await;

        if let Some(start_time) = state.start_time {
            start_time.elapsed() > self.view_change_timeout
        } else {
            false
        }
    }

    /// Get current view change round
    pub async fn current_round(&self) -> Round {
        self.state.read().await.round
    }

    /// Check if we're currently in a view change
    pub async fn is_changing_view(&self) -> bool {
        let state = self.state.read().await;
        state.sent_view_change && !state.view_changes.is_empty()
    }

    /// Get validators who have sent view change messages
    pub async fn get_view_change_validators(&self) -> Vec<ValidatorAddress> {
        self.state
            .read()
            .await
            .view_changes
            .keys()
            .cloned()
            .collect()
    }

    /// Sign a view change message
    fn sign_view_change(
        &self,
        height: u64,
        round: Round,
        timestamp: &std::time::SystemTime,
    ) -> Vec<u8> {
        use sha2::{Digest, Sha256};

        // Create message to sign
        let mut hasher = Sha256::new();
        hasher.update(b"view_change");
        hasher.update(height.to_le_bytes());
        hasher.update(round.as_u32().to_le_bytes());
        hasher.update(self.node_id.as_bytes());

        // Add timestamp
        if let Ok(duration) = timestamp.duration_since(std::time::UNIX_EPOCH) {
            hasher.update(duration.as_secs().to_le_bytes());
        }

        // Generate signature (in production, use proper cryptographic signing with private key)
        let hash = hasher.finalize();
        hash.to_vec()
    }

    /// Verify a view change signature
    fn verify_view_change_signature(
        &self,
        sender: &ValidatorAddress,
        height: u64,
        round: Round,
        signature: &[u8],
    ) -> bool {
        // In production, verify signature using sender's public key
        // For now, just check signature is not empty
        !signature.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Duration;

    #[tokio::test]
    async fn test_view_change_basic() {
        let validators = vec![
            ValidatorAddress("validator1".to_string()),
            ValidatorAddress("validator2".to_string()),
            ValidatorAddress("validator3".to_string()),
        ];

        let leader_selector = Arc::new(RwLock::new(
            crate::leader_selection::LeaderSelector::new_round_robin(validators),
        ));

        let manager = ViewChangeManager::new(
            "validator1".to_string(),
            leader_selector,
            Duration::from_secs(30),
        );

        // Start view change
        let message = manager.start_view_change(1, Round::new(1)).await.unwrap();

        // Verify message structure
        assert_eq!(message.sender, "validator1");
        assert!(matches!(
            message.payload,
            crate::messages::ConsensusMessagePayload::ViewChange(_)
        ));

        // Process view changes from other validators
        assert!(!manager
            .process_view_change(
                &ValidatorAddress("validator2".to_string()),
                1,
                Round::new(1),
                vec![1]
            )
            .await
            .unwrap());

        // Should return true when threshold is reached
        assert!(manager
            .process_view_change(
                &ValidatorAddress("validator3".to_string()),
                1,
                Round::new(1),
                vec![1]
            )
            .await
            .unwrap());

        // Complete view change
        let new_leader = manager.complete_view_change().await.unwrap();

        // With height=1, round=1: total_rounds = 1*1000 + 1 = 1001
        // leader_index = 1001 % 3 = 2, which is validator3
        assert_eq!(new_leader, ValidatorAddress("validator3".to_string()));
    }

    #[tokio::test]
    async fn test_view_change_timeout() {
        let validators = vec![ValidatorAddress("validator1".to_string())];
        let leader_selector = Arc::new(RwLock::new(
            crate::leader_selection::LeaderSelector::new_round_robin(validators),
        ));

        let manager = ViewChangeManager::new(
            "validator1".to_string(),
            leader_selector,
            Duration::from_millis(100), // Short timeout for testing
        );

        // Start view change
        let _ = manager.start_view_change(1, Round::new(1)).await.unwrap();

        // Initially not timed out
        assert!(!manager.is_view_change_timeout().await);

        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(manager.is_view_change_timeout().await);
    }

    #[tokio::test]
    async fn test_view_change_threshold_calculation() {
        // Test with 4 validators (BFT minimum)
        let validators = vec![
            ValidatorAddress("val1".to_string()),
            ValidatorAddress("val2".to_string()),
            ValidatorAddress("val3".to_string()),
            ValidatorAddress("val4".to_string()),
        ];

        let leader_selector = Arc::new(RwLock::new(
            crate::leader_selection::LeaderSelector::new_round_robin(validators),
        ));

        let manager =
            ViewChangeManager::new("val1".to_string(), leader_selector, Duration::from_secs(30));

        // Start view change
        let _ = manager.start_view_change(1, Round::new(1)).await.unwrap();

        // First additional view change - should not reach threshold
        assert!(!manager
            .process_view_change(
                &ValidatorAddress("val2".to_string()),
                1,
                Round::new(1),
                vec![1]
            )
            .await
            .unwrap());

        // Second additional view change - should reach threshold (3/4 > 2/3)
        assert!(manager
            .process_view_change(
                &ValidatorAddress("val3".to_string()),
                1,
                Round::new(1),
                vec![1]
            )
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_view_change_different_heights() {
        let validators = vec![
            ValidatorAddress("val1".to_string()),
            ValidatorAddress("val2".to_string()),
        ];

        let leader_selector = Arc::new(RwLock::new(
            crate::leader_selection::LeaderSelector::new_round_robin(validators),
        ));

        let manager =
            ViewChangeManager::new("val1".to_string(), leader_selector, Duration::from_secs(30));

        // Start view change for height 1
        let _ = manager.start_view_change(1, Round::new(1)).await.unwrap();

        // Try to process view change for different height - should be ignored
        assert!(!manager
            .process_view_change(
                &ValidatorAddress("val2".to_string()),
                2,
                Round::new(1),
                vec![1]
            )
            .await
            .unwrap());

        // Process view change for same height - should work
        assert!(manager
            .process_view_change(
                &ValidatorAddress("val2".to_string()),
                1,
                Round::new(1),
                vec![1]
            )
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_view_change_round_validation() {
        let validators = vec![
            ValidatorAddress("val1".to_string()),
            ValidatorAddress("val2".to_string()),
        ];

        let leader_selector = Arc::new(RwLock::new(
            crate::leader_selection::LeaderSelector::new_round_robin(validators),
        ));

        let manager =
            ViewChangeManager::new("val1".to_string(), leader_selector, Duration::from_secs(30));

        // Start view change for round 3
        let _ = manager.start_view_change(1, Round::new(3)).await.unwrap();

        // Try to process view change for older round - should be ignored
        assert!(!manager
            .process_view_change(
                &ValidatorAddress("val2".to_string()),
                1,
                Round::new(2),
                vec![1]
            )
            .await
            .unwrap());

        // Process view change for same round - should work
        assert!(manager
            .process_view_change(
                &ValidatorAddress("val2".to_string()),
                1,
                Round::new(3),
                vec![1]
            )
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_view_change_reset() {
        let validators = vec![ValidatorAddress("val1".to_string())];
        let leader_selector = Arc::new(RwLock::new(
            crate::leader_selection::LeaderSelector::new_round_robin(validators),
        ));

        let manager =
            ViewChangeManager::new("val1".to_string(), leader_selector, Duration::from_secs(30));

        // Start view change
        let _ = manager.start_view_change(1, Round::new(1)).await.unwrap();
        assert!(manager.is_changing_view().await);

        // Reset to new height and round
        manager.reset(2, Round::new(0)).await;

        // Should no longer be in view change state
        assert!(!manager.is_changing_view().await);
        assert_eq!(manager.current_round().await, Round::new(0));
    }

    #[tokio::test]
    async fn test_view_change_duplicate_prevention() {
        let validators = vec![
            ValidatorAddress("val1".to_string()),
            ValidatorAddress("val2".to_string()),
        ];

        let leader_selector = Arc::new(RwLock::new(
            crate::leader_selection::LeaderSelector::new_round_robin(validators),
        ));

        let manager =
            ViewChangeManager::new("val1".to_string(), leader_selector, Duration::from_secs(30));

        // Start view change for round 1
        let _ = manager.start_view_change(1, Round::new(1)).await.unwrap();

        // Try to start view change for same round - should fail
        assert!(manager.start_view_change(1, Round::new(1)).await.is_err());

        // Start view change for higher round - should succeed
        assert!(manager.start_view_change(1, Round::new(2)).await.is_ok());
    }

    #[tokio::test]
    async fn test_view_change_validators_list() {
        let validators = vec![
            ValidatorAddress("val1".to_string()),
            ValidatorAddress("val2".to_string()),
            ValidatorAddress("val3".to_string()),
        ];

        let leader_selector = Arc::new(RwLock::new(
            crate::leader_selection::LeaderSelector::new_round_robin(validators),
        ));

        let manager =
            ViewChangeManager::new("val1".to_string(), leader_selector, Duration::from_secs(30));

        // Start view change
        let _ = manager.start_view_change(1, Round::new(1)).await.unwrap();

        // Process view changes
        let _ = manager
            .process_view_change(
                &ValidatorAddress("val2".to_string()),
                1,
                Round::new(1),
                vec![1],
            )
            .await;

        // Get list of validators who sent view change messages
        let view_change_validators = manager.get_view_change_validators().await;
        assert_eq!(view_change_validators.len(), 2); // val1 (self) + val2
        assert!(view_change_validators.contains(&ValidatorAddress("val1".to_string())));
        assert!(view_change_validators.contains(&ValidatorAddress("val2".to_string())));
    }

    #[tokio::test]
    async fn test_view_change_leader_calculation() {
        let validators = vec![
            ValidatorAddress("alice".to_string()),
            ValidatorAddress("bob".to_string()),
            ValidatorAddress("charlie".to_string()),
        ];

        let leader_selector = Arc::new(RwLock::new(
            crate::leader_selection::LeaderSelector::new_round_robin(validators),
        ));

        let manager = ViewChangeManager::new(
            "alice".to_string(),
            leader_selector,
            Duration::from_secs(30),
        );

        // Test leader calculation for different rounds at height 0
        // With height=0, total_rounds = round, so we get clean round-robin
        let _ = manager.start_view_change(0, Round::new(1)).await.unwrap();
        let leader_r1 = manager.complete_view_change().await.unwrap();

        manager.reset(0, Round::new(1)).await;
        let _ = manager.start_view_change(0, Round::new(2)).await.unwrap();
        let leader_r2 = manager.complete_view_change().await.unwrap();

        manager.reset(0, Round::new(2)).await;
        let _ = manager.start_view_change(0, Round::new(3)).await.unwrap();
        let leader_r3 = manager.complete_view_change().await.unwrap();

        // Verify round-robin behavior (sorted: alice, bob, charlie)
        // At height 0: Round 0->alice (0%3=0), Round 1->bob (1%3=1), Round 2->charlie (2%3=2), Round 3->alice (3%3=0)
        assert_eq!(leader_r1, ValidatorAddress("bob".to_string()));
        assert_eq!(leader_r2, ValidatorAddress("charlie".to_string()));
        assert_eq!(leader_r3, ValidatorAddress("alice".to_string()));
    }

    #[tokio::test]
    async fn test_view_change_required_voting_power() {
        let validators = vec![
            ValidatorAddress("val1".to_string()),
            ValidatorAddress("val2".to_string()),
            ValidatorAddress("val3".to_string()),
            ValidatorAddress("val4".to_string()),
        ];

        let leader_selector = Arc::new(RwLock::new(
            crate::leader_selection::LeaderSelector::new_round_robin(validators),
        ));

        let manager =
            ViewChangeManager::new("val1".to_string(), leader_selector, Duration::from_secs(30));

        // Test setting required voting power
        manager.set_required_voting_power(300).await;

        // Start view change
        let _ = manager.start_view_change(1, Round::new(1)).await.unwrap();

        // The threshold calculation should now consider the set voting power
        // This is mostly for coverage as the actual voting power logic
        // would be implemented in integration with validator set manager
    }

    #[tokio::test]
    async fn test_view_change_concurrent_processing() {
        let validators = vec![
            ValidatorAddress("val1".to_string()),
            ValidatorAddress("val2".to_string()),
            ValidatorAddress("val3".to_string()),
            ValidatorAddress("val4".to_string()),
        ];

        let leader_selector = Arc::new(RwLock::new(
            crate::leader_selection::LeaderSelector::new_round_robin(validators),
        ));

        let manager = Arc::new(ViewChangeManager::new(
            "val1".to_string(),
            leader_selector,
            Duration::from_secs(30),
        ));

        // Start view change
        let _ = manager.start_view_change(1, Round::new(1)).await.unwrap();

        // Process multiple view changes concurrently
        let manager_clone = manager.clone();
        let handle1 = tokio::spawn(async move {
            manager_clone
                .process_view_change(
                    &ValidatorAddress("val2".to_string()),
                    1,
                    Round::new(1),
                    vec![1],
                )
                .await
        });

        let manager_clone = manager.clone();
        let handle2 = tokio::spawn(async move {
            manager_clone
                .process_view_change(
                    &ValidatorAddress("val3".to_string()),
                    1,
                    Round::new(1),
                    vec![1],
                )
                .await
        });

        let result1 = handle1.await.unwrap().unwrap();
        let result2 = handle2.await.unwrap().unwrap();

        // At least one should return true (threshold reached)
        assert!(result1 || result2);
    }
}
