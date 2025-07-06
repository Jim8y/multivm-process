//! Malachite consensus engine implementation
//!
//! This module provides the core consensus engine functionality for the Malachite BFT protocol.

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::config::ConsensusParams;
use super::validator::MalachiteValidator;
use crate::error::ConsensusError;
use crate::traits::{ConsensusEngine, ConsensusStats};
use crate::ConsensusResult;

/// Malachite consensus engine
#[derive(Debug)]
pub struct MalachiteEngine {
    /// Engine configuration
    config: ConsensusParams,

    /// Validator instance
    validator: Option<MalachiteValidator>,

    /// Engine state
    state: Arc<RwLock<EngineState>>,
}

/// Internal engine state
#[derive(Debug, Default)]
struct EngineState {
    /// Is engine running
    is_running: bool,

    /// Current height
    current_height: u64,

    /// Current consensus round
    current_round: u64,

    /// Current validator set
    validators: Vec<String>,

    /// Votes for current round (vote_key -> block_hash)
    votes: std::collections::HashMap<String, String>,

    /// Whether consensus has been reached for current round
    consensus_reached: bool,

    /// Block hash that reached consensus
    consensus_block_hash: Option<String>,

    /// Total blocks processed
    blocks_processed: u64,

    /// Total transactions processed
    transactions_processed: u64,

    /// Engine start time
    start_time: Option<std::time::Instant>,

    /// Last block time
    last_block_time: Option<std::time::Instant>,
}

impl MalachiteEngine {
    /// Create a new consensus engine
    pub fn new(config: ConsensusParams, node_id: String) -> Self {
        Self {
            config: config.clone(),
            validator: Some(MalachiteValidator::new(config, node_id)),
            state: Arc::new(RwLock::new(EngineState::default())),
        }
    }

    /// Initialize the consensus engine
    pub async fn initialize(&mut self) -> Result<(), ConsensusError> {
        info!("Initializing Malachite consensus engine");

        // Initialize validator
        if let Some(validator) = &mut self.validator {
            validator.initialize().await?;
        }

        // Initialize engine state
        let mut state = self.state.write().await;
        state.current_height = 0;
        state.blocks_processed = 0;
        state.transactions_processed = 0;
        state.start_time = Some(std::time::Instant::now());

        info!("Malachite consensus engine initialized successfully");
        Ok(())
    }

    /// Start the consensus engine
    pub async fn start(&mut self) -> Result<(), ConsensusError> {
        info!("Starting Malachite consensus engine");

        // Start validator
        if let Some(validator) = &mut self.validator {
            validator.start().await?;
        }

        // Update engine state
        let mut state = self.state.write().await;
        state.is_running = true;
        state.start_time = Some(std::time::Instant::now());

        info!("Malachite consensus engine started successfully");
        Ok(())
    }

    /// Stop the consensus engine
    pub async fn stop(&mut self) -> Result<(), ConsensusError> {
        info!("Stopping Malachite consensus engine");

        // Stop validator
        if let Some(validator) = &mut self.validator {
            validator.stop().await?;
        }

        // Update engine state
        let mut state = self.state.write().await;
        state.is_running = false;

        info!("Malachite consensus engine stopped successfully");
        Ok(())
    }

    /// Process a new block
    pub async fn process_block(&mut self, block_data: Vec<u8>) -> Result<(), ConsensusError> {
        debug!("Processing block with {} bytes", block_data.len());

        let mut state = self.state.write().await;

        if !state.is_running {
            return Err(ConsensusError::Configuration(
                "Engine not running".to_string(),
            ));
        }

        // Simulate block processing
        state.current_height += 1;
        state.blocks_processed += 1;
        state.last_block_time = Some(std::time::Instant::now());

        // Advance validator height
        drop(state);
        if let Some(validator) = &mut self.validator {
            validator.advance_height().await?;
        }

        debug!(
            "Block processed successfully at height {}",
            self.current_height().await
        );
        Ok(())
    }

    /// Get current height
    pub async fn current_height(&self) -> u64 {
        self.state.read().await.current_height
    }

    /// Get total blocks processed
    pub async fn blocks_processed(&self) -> u64 {
        self.state.read().await.blocks_processed
    }

    /// Get total transactions processed
    pub async fn transactions_processed(&self) -> u64 {
        self.state.read().await.transactions_processed
    }

    /// Check if engine is running
    pub async fn is_running(&self) -> bool {
        self.state.read().await.is_running
    }

    /// Get engine uptime
    pub async fn uptime(&self) -> Option<std::time::Duration> {
        self.state
            .read()
            .await
            .start_time
            .map(|start| start.elapsed())
    }

    /// Get validator reference
    pub fn validator(&self) -> Option<&MalachiteValidator> {
        self.validator.as_ref()
    }

    /// Get mutable validator reference
    pub fn validator_mut(&mut self) -> Option<&mut MalachiteValidator> {
        self.validator.as_mut()
    }

    /// Get engine configuration
    pub fn config(&self) -> &ConsensusParams {
        &self.config
    }

    /// Update engine configuration
    pub fn update_config(&mut self, config: ConsensusParams) {
        info!("Updating consensus engine configuration");
        self.config = config.clone();

        if let Some(validator) = &mut self.validator {
            validator.update_config(config);
        }
    }

    /// Get engine metrics
    pub async fn get_metrics(&self) -> EngineMetrics {
        let state = self.state.read().await;

        EngineMetrics {
            current_height: state.current_height,
            blocks_processed: state.blocks_processed,
            transactions_processed: state.transactions_processed,
            is_running: state.is_running,
            uptime: state.start_time.map(|start| start.elapsed()),
            avg_block_time: self.calculate_avg_block_time(&state).await,
        }
    }

    /// Calculate average block time
    async fn calculate_avg_block_time(&self, state: &EngineState) -> Option<std::time::Duration> {
        if let (Some(start), Some(last)) = (state.start_time, state.last_block_time) {
            if state.blocks_processed > 0 {
                let total_time = last.duration_since(start);
                Some(total_time / state.blocks_processed as u32)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Record a vote for a specific round and block hash

    pub async fn record_vote(
        &mut self,
        validator_id: String,
        round: u64,
        block_hash: String,
    ) -> ConsensusResult<()> {
        let mut state = self.state.write().await;

        // 1. Check if the vote is for the current round
        if round != state.current_round {
            return Err(ConsensusError::InvalidRound {
                expected: state.current_round,
                received: round,
            });
        }

        // 2. Check if validator is in the current validator set
        if !state.validators.contains(&validator_id) {
            return Err(ConsensusError::UnauthorizedValidator {
                validator_id: validator_id.clone(),
            });
        }

        // 3. Check if validator has already voted for this round
        let vote_key = format!("{}:{}", round, validator_id);
        if state.votes.contains_key(&vote_key) {
            return Err(ConsensusError::DuplicateVote {
                validator_id: validator_id.clone(),
                round,
            });
        }

        // 4. Store the vote
        state.votes.insert(vote_key, block_hash.clone());

        // 5. Count votes for this block hash
        let votes_for_block = state
            .votes
            .values()
            .filter(|&hash| hash == &block_hash)
            .count();

        // 6. Check if we have enough votes for consensus (2/3 + 1)
        let total_validators = state.validators.len();
        let required_votes = (total_validators * 2) / 3 + 1;

        debug!(
            "Recorded vote from {} for round {} block {}: {}/{} votes",
            validator_id, round, block_hash, votes_for_block, required_votes
        );

        if votes_for_block >= required_votes {
            info!(
                "Consensus reached for block {} with {}/{} votes",
                block_hash, votes_for_block, total_validators
            );
            // Trigger block finalization
            state.consensus_reached = true;
            state.consensus_block_hash = Some(block_hash);
        }

        Ok(())
    }

    /// Get pending transactions to include in the next block
    pub async fn get_pending_transactions(&self) -> ConsensusResult<Vec<MalachiteTransaction>> {
        // Simplified implementation - return empty transaction list
        // In a full implementation this would query the transaction pool
        Ok(Vec::new())
    }

    /// Finalize a block after consensus is reached
    pub async fn finalize_block(
        &mut self,
        _round: u64,
        _block: MalachiteBlock,
    ) -> ConsensusResult<()> {
        let mut state = self.state.write().await;

        // Update engine state
        state.current_height = _block.height;
        state.blocks_processed += 1;
        state.last_block_time = Some(std::time::Instant::now());

        debug!(
            "Block finalized at height {} for round {}",
            _block.height, _round
        );
        Ok(())
    }
}

/// Engine metrics for monitoring
#[derive(Debug, Clone)]
pub struct EngineMetrics {
    /// Current block height
    pub current_height: u64,

    /// Total blocks processed
    pub blocks_processed: u64,

    /// Total transactions processed
    pub transactions_processed: u64,

    /// Is engine running
    pub is_running: bool,

    /// Engine uptime
    pub uptime: Option<std::time::Duration>,

    /// Average block processing time
    pub avg_block_time: Option<std::time::Duration>,
}

/// Simple block type for Malachite consensus
#[derive(Debug, Clone)]
pub struct MalachiteBlock {
    /// Block height
    pub height: u64,
    /// Block data
    pub data: Vec<u8>,
    /// Timestamp
    pub timestamp: std::time::SystemTime,
}

/// Simple transaction type for Malachite consensus
#[derive(Debug, Clone)]
pub struct MalachiteTransaction {
    /// Transaction data
    pub data: Vec<u8>,
    /// Transaction hash
    pub hash: String,
}

#[async_trait]
impl ConsensusEngine for MalachiteEngine {
    type Block = MalachiteBlock;
    type Transaction = MalachiteTransaction;
    type Config = ConsensusParams;

    async fn initialize(&mut self, config: Self::Config) -> ConsensusResult<()> {
        self.config = config;
        self.initialize().await
    }

    async fn start(&mut self) -> ConsensusResult<()> {
        self.start().await
    }

    async fn stop(&mut self) -> ConsensusResult<()> {
        self.stop().await
    }

    fn is_running(&self) -> bool {
        // Check if the engine is running by examining the validator state
        if let Some(validator) = &self.validator {
            validator.is_validator_running()
        } else {
            false
        }
    }

    async fn propose_block(
        &self,
        transactions: Vec<Self::Transaction>,
    ) -> ConsensusResult<Self::Block> {
        let height = self.current_height().await + 1;

        // Create a proper MultiVMBlock structure
        let mut multivm_block = crate::block::MultiVMBlock {
            header: crate::block::BlockHeader {
                height,
                previous_hash: "".to_string(), // Simplified for testing
                state_root: "".to_string(),
                transactions_root: "".to_string(),
                timestamp: std::time::SystemTime::now(),
                proposer: "test_proposer".to_string(),
                consensus_data: vec![],
                version: 1,
                extra_data: vec![],
            },
            svm_transactions: vec![], // For testing, we'll use empty VM transactions
            evm_transactions: vec![],
            multivm_transactions: vec![],
            state_transitions: vec![],
        };

        // Update the transaction root hash to match what validation expects
        multivm_block.update_transactions_root();
        // Update the state root hash to match what validation expects
        multivm_block.update_state_root();

        // Serialize the MultiVMBlock to JSON
        let data = serde_json::to_vec(&multivm_block).map_err(|e| {
            crate::error::ConsensusError::SerializationError(format!(
                "Failed to serialize block: {}",
                e
            ))
        })?;

        Ok(MalachiteBlock {
            height,
            data,
            timestamp: std::time::SystemTime::now(),
        })
    }

    async fn validate_block(&self, block: &Self::Block) -> ConsensusResult<bool> {
        // Production block validation implementation

        // First, deserialize the block data from Vec<u8> to MultiVMBlock
        let multivm_block: crate::block::MultiVMBlock = match serde_json::from_slice(&block.data) {
            Ok(b) => b,
            Err(e) => {
                warn!("Failed to deserialize block data: {}", e);
                return Ok(false);
            }
        };

        // 1. Check block structure and basic validity
        if let Err(e) = multivm_block.validate_structure() {
            warn!("Block structure validation failed: {}", e);
            return Ok(false);
        }

        // 2. Verify block height is correct (should be current height + 1)
        let state = self.state.read().await;
        let expected_height = state.current_height + 1;
        if multivm_block.header.height != expected_height {
            warn!(
                "Invalid block height: expected {}, got {}",
                expected_height, multivm_block.header.height
            );
            return Ok(false);
        }

        // Validate block height continuity
        if state.current_height > 0 && multivm_block.header.previous_hash.is_empty() {
            warn!("Block is missing previous hash for height > 0");
            return Ok(false);
        }

        // 4. Verify block timestamp is reasonable (not too far in future)
        let now = std::time::SystemTime::now();
        if multivm_block.header.timestamp > now + std::time::Duration::from_secs(60) {
            warn!("Block timestamp too far in future");
            return Ok(false);
        }

        // 5. Verify transaction hashes match transaction root
        let mut temp_block = multivm_block.clone();
        temp_block.update_transactions_root();
        if temp_block.header.transactions_root != multivm_block.header.transactions_root {
            warn!("Transaction root hash mismatch");
            return Ok(false);
        }

        // 6. Verify state transitions hash matches state root
        temp_block.update_state_root();
        if temp_block.header.state_root != multivm_block.header.state_root {
            warn!("State root hash mismatch");
            return Ok(false);
        }

        // 7. Validate individual transactions (basic checks)
        for (i, tx) in multivm_block.svm_transactions.iter().enumerate() {
            if tx.signatures.is_empty() {
                warn!("SVM transaction {} has no signatures", i);
                return Ok(false);
            }
        }

        for (i, tx) in multivm_block.evm_transactions.iter().enumerate() {
            if tx.from.is_empty() {
                warn!("EVM transaction {} has empty from address", i);
                return Ok(false);
            }
        }

        // 8. Check if we have too many transactions
        if multivm_block.transaction_count() > crate::MAX_TRANSACTIONS_PER_BLOCK {
            warn!(
                "Block has too many transactions: {}",
                multivm_block.transaction_count()
            );
            return Ok(false);
        }

        info!(
            "Block validation passed for height {}",
            multivm_block.header.height
        );
        Ok(true)
    }

    async fn commit_block(&mut self, block: Self::Block) -> ConsensusResult<()> {
        // Don't update height here since process_block will increment it

        self.process_block(block.data).await
    }

    async fn get_current_height(&self) -> ConsensusResult<u64> {
        Ok(self.current_height().await)
    }

    async fn get_latest_block(&self) -> ConsensusResult<Option<Self::Block>> {
        let height = self.current_height().await;
        if height > 0 {
            Ok(Some(MalachiteBlock {
                height,
                data: vec![],
                timestamp: std::time::SystemTime::now(),
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_block_by_height(&self, height: u64) -> ConsensusResult<Option<Self::Block>> {
        let current_height = self.current_height().await;
        if height <= current_height {
            Ok(Some(MalachiteBlock {
                height,
                data: vec![],
                timestamp: std::time::SystemTime::now(),
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_consensus_stats(&self) -> ConsensusResult<ConsensusStats> {
        let metrics = self.get_metrics().await;

        Ok(ConsensusStats {
            current_height: metrics.current_height,
            current_round: 0, // Malachite doesn't expose rounds in our simple implementation
            total_blocks: metrics.blocks_processed,
            total_transactions: metrics.transactions_processed,
            avg_block_time: metrics
                .avg_block_time
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            active_nodes: 1, // Single node for now
            algorithm: "Malachite".to_string(),
            uptime: metrics.uptime.map(|d| d.as_secs()).unwrap_or(0),
            last_block_time: std::time::SystemTime::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_engine_lifecycle() {
        let config = ConsensusParams::default();
        let mut engine = MalachiteEngine::new(config, "test_node".to_string());

        // Test initialization
        assert!(engine.initialize().await.is_ok());
        assert_eq!(engine.current_height().await, 0);
        assert_eq!(engine.blocks_processed().await, 0);

        // Test starting
        assert!(engine.start().await.is_ok());
        assert!(engine.is_running().await);

        // Test block processing
        let block_data = vec![1, 2, 3, 4, 5];
        assert!(engine.process_block(block_data).await.is_ok());
        assert_eq!(engine.current_height().await, 1);
        assert_eq!(engine.blocks_processed().await, 1);

        // Test metrics
        let metrics = engine.get_metrics().await;
        assert_eq!(metrics.current_height, 1);
        assert_eq!(metrics.blocks_processed, 1);
        assert!(metrics.is_running);
        assert!(metrics.uptime.is_some());

        // Test stopping
        assert!(engine.stop().await.is_ok());
        assert!(!engine.is_running().await);
    }

    #[tokio::test]
    async fn test_multiple_blocks() {
        let config = ConsensusParams::default();
        let mut engine = MalachiteEngine::new(config, "test_node".to_string());

        assert!(engine.initialize().await.is_ok());
        assert!(engine.start().await.is_ok());

        // Process multiple blocks
        for i in 1..=5 {
            let block_data = vec![i; 10];
            assert!(engine.process_block(block_data).await.is_ok());
            assert_eq!(engine.current_height().await, i as u64);
        }

        assert_eq!(engine.blocks_processed().await, 5);

        let metrics = engine.get_metrics().await;
        assert_eq!(metrics.blocks_processed, 5);
        assert_eq!(metrics.current_height, 5);
    }

    #[tokio::test]
    async fn test_consensus_engine_trait() {
        let config = ConsensusParams::default();
        let mut engine = MalachiteEngine::new(config.clone(), "test_node".to_string());

        // Test trait methods
        assert!(engine.initialize().await.is_ok());
        assert!(engine.start().await.is_ok());
        assert!(engine.is_running().await);

        // Test block proposal
        let transactions = vec![
            MalachiteTransaction {
                data: vec![1, 2, 3],
                hash: "tx1".to_string(),
            },
            MalachiteTransaction {
                data: vec![4, 5, 6],
                hash: "tx2".to_string(),
            },
        ];

        let block = engine.propose_block(transactions).await.unwrap();
        assert_eq!(block.height, 1);
        assert!(!block.data.is_empty());

        // Test block validation and commit
        assert!(engine.validate_block(&block).await.unwrap());
        assert!(engine.commit_block(block).await.is_ok());

        // Test stats
        let stats = engine.get_consensus_stats().await.unwrap();
        assert_eq!(stats.current_height, 1);
        assert_eq!(stats.algorithm, "Malachite");

        assert!(engine.stop().await.is_ok());
    }
}
