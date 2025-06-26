//! Malachite consensus validator implementation
//! 
//! This module provides the validator functionality for the Malachite BFT consensus protocol.

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::error::ConsensusError;
use super::config::ConsensusParams;

/// Malachite consensus validator
#[derive(Debug)]
pub struct MalachiteValidator {
    /// Validator configuration
    config: ConsensusParams,
    
    /// Validator state
    state: Arc<RwLock<ValidatorState>>,
}

/// Internal validator state
#[derive(Debug, Default)]
struct ValidatorState {
    /// Current height
    current_height: u64,
    
    /// Current round
    current_round: u32,
    
    /// Is validator active
    is_active: bool,
    
    /// Last block time
    last_block_time: Option<std::time::Instant>,
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
        state.current_round = 0;
        
        info!("Malachite validator initialized successfully");
        Ok(())
    }
    
    /// Start the validator
    pub async fn start(&mut self) -> Result<(), ConsensusError> {
        info!("Starting Malachite validator");
        
        let mut state = self.state.write().await;
        if !state.is_active {
            return Err(ConsensusError::Configuration("Validator not initialized".to_string()));
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
        self.state.read().await.current_round
    }
    
    /// Advance to next height
    pub async fn advance_height(&mut self) -> Result<(), ConsensusError> {
        let mut state = self.state.write().await;
        
        if !state.is_active {
            return Err(ConsensusError::Configuration("Validator not active".to_string()));
        }
        
        state.current_height += 1;
        state.current_round = 0;
        state.last_block_time = Some(std::time::Instant::now());
        
        debug!("Advanced to height {}", state.current_height);
        Ok(())
    }
    
    /// Advance to next round
    pub async fn advance_round(&mut self) -> Result<(), ConsensusError> {
        let mut state = self.state.write().await;
        
        if !state.is_active {
            return Err(ConsensusError::Configuration("Validator not active".to_string()));
        }
        
        state.current_round += 1;
        
        debug!("Advanced to round {} at height {}", state.current_round, state.current_height);
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