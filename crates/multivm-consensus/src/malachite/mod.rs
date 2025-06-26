//! Malachite Consensus Integration
//!
//! This module provides integration with the Malachite BFT consensus engine
//! from Informal Systems for the MultiVM architecture.

pub mod config;
pub mod types;
pub mod validator;
pub mod engine;

pub use config::{MalachiteConfig, NetworkConfig, ConsensusParams};
pub use types::{
    ConsensusPhase, VoteType, Round, BlockId, BlockHeight, ValidatorAddress,
    ValidatorInfo, ConsensusProposal, ConsensusCommit,
};
pub use validator::MalachiteValidator;
pub use engine::{MalachiteEngine, EngineMetrics, MalachiteBlock, MalachiteTransaction};

// Alias for backward compatibility
pub use engine::MalachiteEngine as MalachiteConsensus;