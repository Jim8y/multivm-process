//! Configuration types for Malachite consensus

use super::types::ValidatorInfo;
use serde::{Deserialize, Serialize};

/// Configuration for Malachite consensus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MalachiteConfig {
    /// Node identifier
    pub node_id: String,
    /// Network configuration
    pub network_config: NetworkConfig,
    /// Consensus parameters
    pub consensus_params: ConsensusParams,
    /// Initial validator set
    pub validators: Vec<ValidatorInfo>,
}

impl Default for MalachiteConfig {
    fn default() -> Self {
        Self {
            node_id: "default-node".to_string(),
            network_config: NetworkConfig::default(),
            consensus_params: ConsensusParams::default(),
            validators: vec![],
        }
    }
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Listen address
    pub listen_addr: String,
    /// Peer addresses
    pub peers: Vec<String>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1:26656".to_string(),
            peers: vec![],
        }
    }
}

/// Consensus parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusParams {
    /// Block time in milliseconds
    pub block_time_ms: u64,
    /// Maximum block size in bytes
    pub max_block_size: usize,
    /// Timeout for propose step
    pub timeout_propose_ms: u64,
    /// Timeout for prevote step
    pub timeout_prevote_ms: u64,
    /// Timeout for precommit step
    pub timeout_precommit_ms: u64,
    /// Timeout for commit step (Malachite BFT)
    pub timeout_commit_ms: u64,
    /// Maximum number of transactions per block (Malachite BFT)
    pub max_transactions_per_block: usize,
    /// Validator set size (Malachite BFT)
    pub validator_set_size: usize,
}

impl Default for ConsensusParams {
    fn default() -> Self {
        Self {
            block_time_ms: 5000,
            max_block_size: 1024 * 1024,
            timeout_propose_ms: 3000,
            timeout_prevote_ms: 1000,
            timeout_precommit_ms: 1000,
            timeout_commit_ms: 5000,
            max_transactions_per_block: 1000,
            validator_set_size: 3,
        }
    }
}

impl From<MalachiteConfig> for ConsensusParams {
    fn from(config: MalachiteConfig) -> Self {
        config.consensus_params
    }
}

impl MalachiteConfig {
    /// Set timeout duration for consensus steps
    pub fn set_timeout_duration(&mut self, duration: std::time::Duration) {
        let duration_ms = duration.as_millis() as u64;
        self.consensus_params.timeout_propose_ms = duration_ms;
        self.consensus_params.timeout_prevote_ms = duration_ms / 3;
        self.consensus_params.timeout_precommit_ms = duration_ms / 3;
        self.consensus_params.timeout_commit_ms = duration_ms;
        self.consensus_params.block_time_ms = duration_ms;
    }

    /// Set validator count
    pub fn set_validator_count(&mut self, count: u32) {
        // For solo testnet, we create a single validator
        if count == 1 {
            self.validators = vec![ValidatorInfo {
                public_key: "solo-validator-key".to_string(),
                voting_power: 100,
            }];
        }
    }

    /// Enable single node mode for solo testnet
    pub fn enable_single_node_mode(&mut self) {
        self.network_config.peers = vec![];
        self.set_validator_count(1);
    }
}
