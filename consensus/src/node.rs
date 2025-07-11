//! Node-related types and utilities

use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};
use uuid::Uuid;

/// Unique identifier for a node in the consensus cluster
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(Uuid);

impl NodeId {
    /// Create a new random NodeId
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
    
    /// Create a NodeId from a UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
    
    /// Get the underlying UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
    
    /// Create from bytes
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }
    
    /// Get as bytes
    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// State of a node in the consensus algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeState {
    /// Node is a follower
    Follower,
    /// Node is a candidate for leadership
    Candidate,
    /// Node is the current leader
    Leader,
    /// Node is shutting down
    Shutdown,
}

impl Display for NodeState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeState::Follower => write!(f, "Follower"),
            NodeState::Candidate => write!(f, "Candidate"),
            NodeState::Leader => write!(f, "Leader"),
            NodeState::Shutdown => write!(f, "Shutdown"),
        }
    }
}

/// Information about a node in the cluster
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    /// Unique identifier for the node
    pub id: NodeId,
    /// Network address of the node
    pub address: String,
    /// Current state of the node
    pub state: NodeState,
    /// Whether this node can vote in elections
    pub voting: bool,
}

impl Node {
    /// Create a new node
    pub fn new(id: NodeId, address: impl Into<String>, voting: bool) -> Self {
        Self {
            id,
            address: address.into(),
            state: NodeState::Follower,
            voting,
        }
    }
    
    /// Check if this node can participate in voting
    pub fn can_vote(&self) -> bool {
        self.voting && self.state != NodeState::Shutdown
    }
}