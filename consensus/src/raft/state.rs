//! Raft state management

use crate::{NodeId, NodeState, storage::LogEntry};
use std::collections::{HashMap, HashSet};
use tokio::sync::RwLock;

/// Volatile state on all servers
#[derive(Debug)]
pub struct ServerState {
    /// Latest term server has seen
    pub current_term: u64,
    
    /// Candidate that received vote in current term
    pub voted_for: Option<NodeId>,
    
    /// Current role
    pub state: NodeState,
    
    /// Current leader ID
    pub leader_id: Option<NodeId>,
    
    /// Index of highest log entry known to be committed
    pub commit_index: u64,
    
    /// Index of highest log entry applied to state machine
    pub last_applied: u64,
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            current_term: 0,
            voted_for: None,
            state: NodeState::Follower,
            leader_id: None,
            commit_index: 0,
            last_applied: 0,
        }
    }
}

/// Volatile state on leaders (reinitialized after election)
#[derive(Debug)]
pub struct LeaderState {
    /// For each server, index of the next log entry to send
    pub next_index: HashMap<NodeId, u64>,
    
    /// For each server, index of highest log entry known to be replicated
    pub match_index: HashMap<NodeId, u64>,
    
    /// Track which nodes have responded to heartbeats
    pub heartbeat_responses: HashSet<NodeId>,
    
    /// Pending client requests
    pub pending_requests: HashMap<u64, tokio::sync::oneshot::Sender<()>>,
}

impl LeaderState {
    pub fn new(peers: &[NodeId], last_log_index: u64) -> Self {
        let mut next_index = HashMap::new();
        let mut match_index = HashMap::new();
        
        for &peer in peers {
            next_index.insert(peer, last_log_index + 1);
            match_index.insert(peer, 0);
        }
        
        Self {
            next_index,
            match_index,
            heartbeat_responses: HashSet::new(),
            pending_requests: HashMap::new(),
        }
    }
    
    /// Update progress for a follower
    pub fn update_progress(&mut self, follower: NodeId, match_idx: u64) {
        if let Some(match_index) = self.match_index.get_mut(&follower) {
            *match_index = match_idx.max(*match_index);
        }
        if let Some(next_index) = self.next_index.get_mut(&follower) {
            *next_index = match_idx + 1;
        }
    }
    
    /// Decrement next_index for a follower (on conflict)
    pub fn decrement_next_index(&mut self, follower: NodeId) {
        if let Some(next_index) = self.next_index.get_mut(&follower) {
            *next_index = (*next_index).saturating_sub(1).max(1);
        }
    }
}

/// Candidate state during elections
#[derive(Debug)]
pub struct CandidateState {
    /// Votes received in current election
    pub votes_received: HashSet<NodeId>,
    
    /// Nodes that have responded (either granted or denied)
    pub responses_received: HashSet<NodeId>,
}

impl CandidateState {
    pub fn new(node_id: NodeId) -> Self {
        let mut votes_received = HashSet::new();
        votes_received.insert(node_id); // Vote for self
        
        Self {
            votes_received,
            responses_received: HashSet::new(),
        }
    }
    
    /// Check if we have majority
    pub fn has_majority(&self, total_nodes: usize) -> bool {
        self.votes_received.len() > total_nodes / 2
    }
}

/// Combined Raft state
#[derive(Debug)]
pub struct RaftState {
    /// Basic server state
    pub server: RwLock<ServerState>,
    
    /// Leader-specific state
    pub leader: RwLock<Option<LeaderState>>,
    
    /// Candidate-specific state
    pub candidate: RwLock<Option<CandidateState>>,
}

impl RaftState {
    pub fn new() -> Self {
        Self {
            server: RwLock::new(ServerState::new()),
            leader: RwLock::new(None),
            candidate: RwLock::new(None),
        }
    }
    
    /// Transition to follower state
    pub async fn become_follower(&self, term: u64, leader: Option<NodeId>) {
        {
            let mut server = self.server.write().await;
            server.state = NodeState::Follower;
            server.current_term = term;
            server.voted_for = None;
            server.leader_id = leader;
        }
        
        // Clear leader and candidate state
        *self.leader.write().await = None;
        *self.candidate.write().await = None;
    }
    
    /// Transition to candidate state
    pub async fn become_candidate(&self, node_id: NodeId) {
        {
            let mut server = self.server.write().await;
            server.state = NodeState::Candidate;
            server.current_term += 1;
            server.voted_for = Some(node_id);
            server.leader_id = None;
        }
        
        // Initialize candidate state
        *self.candidate.write().await = Some(CandidateState::new(node_id));
        *self.leader.write().await = None;
    }
    
    /// Transition to leader state
    pub async fn become_leader(&self, node_id: NodeId, peers: &[NodeId], last_log_index: u64) {
        {
            let mut server = self.server.write().await;
            server.state = NodeState::Leader;
            server.leader_id = Some(node_id);
        }
        
        // Initialize leader state
        *self.leader.write().await = Some(LeaderState::new(peers, last_log_index));
        *self.candidate.write().await = None;
    }
}