//! Malachite Consensus Integration
//!
//! This module provides integration with the Malachite BFT consensus engine
//! from Informal Systems for the MultiVM architecture.

pub mod config;
pub mod engine;
pub mod types;
pub mod validator;

pub use config::{ConsensusParams, MalachiteConfig, NetworkConfig};
pub use engine::{EngineMetrics, MalachiteBlock, MalachiteEngine, MalachiteTransaction};
pub use types::{
    BlockHeight, BlockId, ConsensusCommit, ConsensusPhase, ConsensusProposal, Round,
    ValidatorAddress, ValidatorInfo, VoteType,
};
pub use validator::MalachiteValidator;

// Alias for backward compatibility
pub use engine::MalachiteEngine as MalachiteConsensus;
