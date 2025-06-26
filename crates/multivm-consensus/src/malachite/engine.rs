//! Malachite consensus engine implementation
//! 
//! This module provides the core consensus engine functionality for the Malachite BFT protocol.

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};
use async_trait::async_trait;

use crate::error::ConsensusError;
use crate::traits::{ConsensusEngine, ConsensusStats, ValidationResult};
use crate::ConsensusResult;
use super::config::ConsensusParams;
use super::validator::MalachiteValidator;

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
    pub fn new(config: ConsensusParams) -> Self {
        Self {
            config: config.clone(),
            validator: Some(MalachiteValidator::new(config)),
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
            return Err(ConsensusError::Configuration("Engine not running".to_string()));
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
        
        debug!("Block processed successfully at height {}", self.current_height().await);
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
        self.state.read().await.start_time.map(|start| start.elapsed())
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
        self.initialize().await.map_err(|e| e.into())
    }

    async fn start(&mut self) -> ConsensusResult<()> {
        self.start().await.map_err(|e| e.into())
    }

    async fn stop(&mut self) -> ConsensusResult<()> {
        self.stop().await.map_err(|e| e.into())
    }

    fn is_running(&self) -> bool {
        // Convert async method to sync by using a blocking approach
        // In a real implementation, this should be a sync field
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.is_running().await
            })
        })
    }

    async fn propose_block(&self, transactions: Vec<Self::Transaction>) -> ConsensusResult<Self::Block> {
        let height = self.current_height().await + 1;
        let mut data = Vec::new();
        
        // Serialize transactions into block data
        for tx in transactions {
            data.extend_from_slice(&tx.data);
        }
        
        Ok(MalachiteBlock {
            height,
            data,
            timestamp: std::time::SystemTime::now(),
        })
    }

    async fn validate_block(&self, _block: &Self::Block) -> ConsensusResult<bool> {
        // Simple validation - in a real implementation this would be more complex
        Ok(true)
    }

    async fn commit_block(&mut self, block: Self::Block) -> ConsensusResult<()> {
        self.process_block(block.data).await.map_err(|e| e.into())
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
            avg_block_time: metrics.avg_block_time.map(|d| d.as_millis() as u64).unwrap_or(0),
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
        let mut engine = MalachiteEngine::new(config);
        
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
        let mut engine = MalachiteEngine::new(config);
        
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
        let mut engine = MalachiteEngine::new(config.clone());
        
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