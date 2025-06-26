//! Core types for Malachite consensus integration

use serde::{Deserialize, Serialize};
use std::fmt::Display;
// use informalsystems_malachitebft_core_types::{Address, Height, Value};

/// Consensus phases for state machine
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsensusPhase {
    NewHeight,
    Propose,
    Prevote,
    Precommit,
    Commit,
}

/// Vote types for consensus
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoteType {
    Prevote,
    Precommit,
}

/// Round type for consensus
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Round(u32);

impl Round {
    pub fn new(round: u32) -> Self {
        Self(round)
    }

    pub fn as_u32(&self) -> u32 {
        self.0
    }

    pub fn increment(&self) -> Self {
        Self(self.0 + 1)
    }
}

impl Display for Round {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// MultiVM Block ID type for Malachite Value trait
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockId(String);

impl Display for BlockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl BlockId {
    pub fn from_hash(hash: String) -> Self {
        Self(hash)
    }
}

/// MultiVM Height implementation
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockHeight(u64);

impl Display for BlockHeight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// impl Height for BlockHeight {
//     const ZERO: Self = BlockHeight(0);
//     const INITIAL: Self = BlockHeight(1);
//
//     fn increment_by(&self, n: u64) -> Self {
//         BlockHeight(self.0 + n)
//     }
//
//     fn decrement_by(&self, n: u64) -> Option<Self> {
//         if self.0 >= n {
//             Some(BlockHeight(self.0 - n))
//         } else {
//             None
//         }
//     }
//
//     fn as_u64(&self) -> u64 {
//         self.0
//     }
// }

/// MultiVM Address implementation
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValidatorAddress(pub String);

// impl Address for ValidatorAddress {}

impl Display for ValidatorAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
/// Implement Value trait for MultiVMBlock
// impl Value for MultiVMBlock {
//     type Id = BlockId;
//
//     fn id(&self) -> Self::Id {
//         BlockId::from_hash(self.calculate_hash())
//     }
// }

/// Validator information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorInfo {
    pub public_key: String,
    pub voting_power: u64,
}

/// Consensus message types
#[derive(Debug, Clone)]
pub struct ConsensusProposal {
    pub block_id: BlockId,
    pub height: u64,
    pub round: u32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct ConsensusCommit {
    pub block_id: BlockId,
    pub height: u64,
    pub signatures: Vec<Vec<u8>>,
}
