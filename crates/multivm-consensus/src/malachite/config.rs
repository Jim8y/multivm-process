//! Configuration types for Malachite consensus

use serde::{Deserialize, Serialize};
use super::types::ValidatorInfo;

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
}

impl Default for ConsensusParams {
    fn default() -> Self {
        Self {
            block_time_ms: 1000,
            max_block_size: 1024 * 1024,
            timeout_propose_ms: 3000,
            timeout_prevote_ms: 1000,
            timeout_precommit_ms: 1000,
        }
    }
}

impl From<MalachiteConfig> for ConsensusParams {
    fn from(config: MalachiteConfig) -> Self {
        config.consensus_params
    }
}