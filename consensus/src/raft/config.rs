//! Configuration for the Raft consensus algorithm

use crate::NodeId;
use std::time::Duration;

/// Configuration parameters for Raft
#[derive(Debug, Clone)]
pub struct Config {
    /// Unique identifier for this node
    pub node_id: NodeId,
    
    /// List of all nodes in the cluster (including this one)
    pub peers: Vec<NodeId>,
    
    /// Election timeout range (min, max) in milliseconds
    pub election_timeout_range: (u64, u64),
    
    /// Heartbeat interval in milliseconds
    pub heartbeat_interval: Duration,
    
    /// Maximum number of entries to send in a single AppendEntries RPC
    pub max_append_entries: usize,
    
    /// Maximum size of a single log entry in bytes
    pub max_entry_size: usize,
    
    /// How often to check if a snapshot is needed
    pub snapshot_interval: Duration,
    
    /// Number of log entries to keep after snapshot
    pub snapshot_threshold: u64,
    
    /// Maximum number of concurrent RPCs
    pub max_concurrent_rpcs: usize,
    
    /// RPC timeout
    pub rpc_timeout: Duration,
}

impl Config {
    /// Create a new configuration with sensible defaults
    pub fn new(node_id: NodeId, peers: Vec<NodeId>) -> Self {
        Self {
            node_id,
            peers,
            election_timeout_range: (150, 300),
            heartbeat_interval: Duration::from_millis(50),
            max_append_entries: 100,
            max_entry_size: 1024 * 1024, // 1MB
            snapshot_interval: Duration::from_secs(300), // 5 minutes
            snapshot_threshold: 1000,
            max_concurrent_rpcs: 10,
            rpc_timeout: Duration::from_millis(100),
        }
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> crate::Result<()> {
        use crate::ConsensusError;
        
        if self.peers.is_empty() {
            return Err(ConsensusError::Configuration(
                "Peers list cannot be empty".to_string()
            ));
        }
        
        if !self.peers.contains(&self.node_id) {
            return Err(ConsensusError::Configuration(
                "Node ID must be in peers list".to_string()
            ));
        }
        
        if self.election_timeout_range.0 >= self.election_timeout_range.1 {
            return Err(ConsensusError::Configuration(
                "Invalid election timeout range".to_string()
            ));
        }
        
        if self.heartbeat_interval.as_millis() >= self.election_timeout_range.0 as u128 {
            return Err(ConsensusError::Configuration(
                "Heartbeat interval must be less than minimum election timeout".to_string()
            ));
        }
        
        Ok(())
    }
    
    /// Get a random election timeout within the configured range
    pub fn random_election_timeout(&self) -> Duration {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let timeout_ms = rng.gen_range(self.election_timeout_range.0..=self.election_timeout_range.1);
        Duration::from_millis(timeout_ms)
    }
    
    /// Check if this node is a voting member
    pub fn is_voting_member(&self) -> bool {
        self.peers.contains(&self.node_id)
    }
    
    /// Get the quorum size (majority)
    pub fn quorum_size(&self) -> usize {
        (self.peers.len() / 2) + 1
    }
}