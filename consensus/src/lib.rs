#![warn(missing_docs)]
#![forbid(unsafe_code)]

//! Production-ready distributed consensus library implementing the Raft algorithm.
//! 
//! This crate provides a robust, performant implementation of the Raft consensus algorithm
//! suitable for building distributed systems that require strong consistency guarantees.

pub mod error;
pub mod message;
pub mod node;
pub mod raft;
pub mod storage;
pub mod transport;

pub use error::{ConsensusError, Result};
pub use message::{Message, MessagePayload, MessageType};
pub use node::{Node, NodeId, NodeState};
pub use raft::{Config, RaftNode};
pub use storage::{LogEntry, Storage};
pub use transport::{Transport, TransportMessage};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// Core consensus algorithm trait that all implementations must satisfy
#[async_trait]
pub trait ConsensusAlgorithm: Send + Sync + Debug {
    /// The type of commands this consensus algorithm processes
    type Command: Send + Sync + Debug + Serialize + for<'de> Deserialize<'de>;
    
    /// The type of responses returned after command execution
    type Response: Send + Sync + Debug + Serialize + for<'de> Deserialize<'de>;
    
    /// Propose a new command to the consensus algorithm
    async fn propose(&self, command: Self::Command) -> Result<Self::Response>;
    
    /// Get the current leader of the consensus group
    async fn get_leader(&self) -> Option<NodeId>;
    
    /// Check if this node is the current leader
    async fn is_leader(&self) -> bool;
    
    /// Get the current state of the consensus algorithm
    async fn get_state(&self) -> NodeState;
    
    /// Shutdown the consensus algorithm gracefully
    async fn shutdown(&self) -> Result<()>;
}

/// Trait for state machines that process commands from the consensus log
#[async_trait]
pub trait StateMachine: Send + Sync + Debug {
    /// The type of commands this state machine processes
    type Command: Send + Sync + Debug + Serialize + for<'de> Deserialize<'de>;
    
    /// The type of responses returned after command execution
    type Response: Send + Sync + Debug + Serialize + for<'de> Deserialize<'de>;
    
    /// Apply a command to the state machine
    async fn apply(&mut self, command: Self::Command) -> Self::Response;
    
    /// Take a snapshot of the current state
    async fn snapshot(&self) -> Result<Vec<u8>>;
    
    /// Restore state from a snapshot
    async fn restore(&mut self, snapshot: &[u8]) -> Result<()>;
}