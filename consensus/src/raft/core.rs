//! Core Raft node implementation

use crate::{
    ConsensusAlgorithm, ConsensusError, Message, MessagePayload, NodeId, NodeState, Result,
    StateMachine, storage::Storage, transport::{Transport, TransportMessage},
};
use multivm_core::ClusterMember;
use super::{
    Config, election, replication,
    state::RaftState,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::{
    sync::{mpsc, oneshot, Mutex},
    time::{interval, timeout, Duration, Instant},
};
use tracing::{error, info, warn, instrument};

/// Command for the Raft node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub data: Vec<u8>,
}

/// Response from the Raft node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub success: bool,
    pub data: Vec<u8>,
}

/// Main Raft node implementation
#[derive(Debug)]
pub struct RaftNode<S: Storage, T: Transport, SM: StateMachine<Command = Command, Response = Response>> {
    config: Config,
    state: Arc<RaftState>,
    storage: Arc<S>,
    transport: Arc<tokio::sync::Mutex<T>>,
    state_machine: Arc<tokio::sync::RwLock<SM>>,
    shutdown_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    proposal_tx: mpsc::Sender<(Command, oneshot::Sender<Result<Response>>)>,
    pending_proposals: Arc<dashmap::DashMap<u64, oneshot::Sender<Response>>>,
}

impl<S, T, SM> RaftNode<S, T, SM>
where
    S: Storage + 'static + std::fmt::Debug,
    T: Transport + 'static + std::fmt::Debug,
    SM: StateMachine<Command = Command, Response = Response> + 'static,
{
    /// Create a new Raft node
    #[instrument(skip(storage, transport, state_machine))]
    pub async fn new(
        config: Config,
        storage: S,
        transport: T,
        state_machine: SM,
    ) -> Result<Self> {
        config.validate()?;
        
        let state = Arc::new(RaftState::new());
        let storage = Arc::new(storage);
        let transport = Arc::new(tokio::sync::Mutex::new(transport));
        let state_machine = Arc::new(tokio::sync::RwLock::new(state_machine));
        
        // Load persistent state
        let term = storage.load_term().await?;
        let voted_for = storage.load_vote().await?;
        
        {
            let mut server = state.server.write().await;
            server.current_term = term;
            server.voted_for = voted_for;
        }
        
        let (proposal_tx, mut proposal_rx) = mpsc::channel::<(Command, oneshot::Sender<Result<Response>>)>(100);
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();
        let pending_proposals = Arc::new(dashmap::DashMap::new());
        
        // Start background tasks before creating node
        let config_clone = config.clone();
        let state_clone = state.clone();
        let storage_clone = storage.clone();
        let transport_clone = transport.clone();
        let state_machine_clone = state_machine.clone();
        let pending_proposals_clone = pending_proposals.clone();
        
        tokio::spawn(async move {
            let mut election_timeout = interval(config_clone.random_election_timeout());
            let mut heartbeat_interval = interval(config_clone.heartbeat_interval);
            let mut last_heartbeat = Instant::now();
            
            loop {
                tokio::select! {
                    // Handle shutdown
                    _ = &mut shutdown_rx => {
                        info!(node_id = %config_clone.node_id, "Shutting down");
                        break;
                    }
                    
                    // Handle incoming messages
                    msg = async {
                        let mut t = transport_clone.lock().await;
                        t.recv().await
                    } => {
                        match msg {
                            Ok(transport_msg) => {
                                // Update heartbeat time for messages from leader
                                if let Ok(is_leader_msg) = is_leader_message(&transport_msg.message) {
                                    if is_leader_msg {
                                        last_heartbeat = Instant::now();
                                    }
                                }
                                
                                if let Err(e) = handle_message(
                                    &config_clone,
                                    &state_clone,
                                    &*storage_clone,
                                    &transport_clone,
                                    transport_msg,
                                ).await {
                                    error!(error = %e, "Error handling message");
                                }
                            }
                            Err(e) => {
                                error!(error = %e, "Error receiving message");
                            }
                        }
                    }
                    
                    // Handle client proposals
                    proposal = proposal_rx.recv() => {
                        if let Some((command, response_tx)) = proposal {
                            let result = handle_proposal(
                                &config_clone,
                                &state_clone,
                                &*storage_clone,
                                command,
                                &pending_proposals_clone,
                            ).await;
                            
                            let _ = response_tx.send(result);
                        }
                    }
                    
                    // Election timeout
                    _ = election_timeout.tick() => {
                        let should_start_election = {
                            let server = state_clone.server.read().await;
                            server.state != NodeState::Leader && 
                            last_heartbeat.elapsed() > config_clone.random_election_timeout()
                        };
                        
                        if should_start_election {
                            let transport_guard = transport_clone.lock().await;
                            if let Err(e) = election::start_election(&config_clone, &state_clone, &*storage_clone, &*transport_guard).await {
                                error!(error = %e, "Error starting election");
                            }
                        }
                    }
                    
                    // Heartbeat/replication
                    _ = heartbeat_interval.tick() => {
                        let is_leader = {
                            let server = state_clone.server.read().await;
                            server.state == NodeState::Leader
                        };
                        
                        if is_leader {
                            let transport_guard = transport_clone.lock().await;
                            if let Err(e) = replication::replicate_to_followers(&config_clone, &state_clone, &*storage_clone, &*transport_guard).await {
                                error!(error = %e, "Error replicating to followers");
                            }
                        }
                    }
                    
                    // Apply committed entries
                    _ = tokio::time::sleep(Duration::from_millis(10)) => {
                        if let Err(e) = apply_committed_entries(&state_clone, &*storage_clone, &state_machine_clone, &pending_proposals_clone).await {
                            error!(error = %e, "Error applying committed entries");
                        }
                    }
                }
            }
        });
        
        let node = Self {
            config,
            state,
            storage,
            transport,
            state_machine,
            shutdown_tx: Arc::new(Mutex::new(Some(shutdown_tx))),
            proposal_tx,
            pending_proposals,
        };
        
        Ok(node)
    }
    
}

/// Check if a message is from a leader (AppendEntries or Heartbeat)
fn is_leader_message(msg: &Message) -> Result<bool> {
    match msg.payload {
        MessagePayload::AppendEntries { .. } | MessagePayload::Heartbeat => Ok(true),
        _ => Ok(false),
    }
}

/// Handle incoming messages
#[instrument(skip(config, state, storage, transport, transport_msg))]
async fn handle_message<S: Storage, T: Transport>(
    config: &Config,
    state: &RaftState,
    storage: &S,
    transport: &Arc<tokio::sync::Mutex<T>>,
    transport_msg: TransportMessage,
) -> Result<()> {
    let msg = transport_msg.message;
    
    match msg.payload {
        MessagePayload::RequestVote { last_log_index, last_log_term } => {
            election::handle_request_vote(
                config,
                state,
                storage,
                &*transport.lock().await,
                msg.from,
                msg.term,
                last_log_index,
                last_log_term,
            ).await?;
        }
        
        MessagePayload::RequestVoteResponse { vote_granted } => {
            let won = election::handle_request_vote_response(
                config,
                state,
                storage,
                msg.from,
                msg.term,
                vote_granted,
            ).await?;
            
            if won {
                // Send initial heartbeat
                replication::replicate_to_followers(config, state, storage, &*transport.lock().await).await?;
            }
        }
        
        MessagePayload::AppendEntries { prev_log_index, prev_log_term, entries, leader_commit } => {
            replication::handle_append_entries(
                config,
                state,
                storage,
                &*transport.lock().await,
                msg.from,
                msg.term,
                prev_log_index,
                prev_log_term,
                entries,
                leader_commit,
            ).await?;
        }
        
        MessagePayload::AppendEntriesResponse { success, match_index, conflict_index, conflict_term } => {
            replication::handle_append_entries_response(
                config,
                state,
                msg.from,
                msg.term,
                success,
                match_index,
                conflict_index,
                conflict_term,
            ).await?;
        }
        
        MessagePayload::Heartbeat => {
            // Treat as empty AppendEntries
            replication::handle_append_entries(
                config,
                state,
                storage,
                &*transport.lock().await,
                msg.from,
                msg.term,
                0,
                0,
                Vec::new(),
                0,
            ).await?;
        }
        
        _ => {
            warn!("Unhandled message type: {:?}", msg.msg_type);
        }
    }
    
    Ok(())
}

/// Handle client proposals
#[instrument(skip(_config, state, storage, command, pending_proposals))]
async fn handle_proposal<S: Storage>(
    _config: &Config,
    state: &RaftState,
    storage: &S,
    command: Command,
    pending_proposals: &Arc<dashmap::DashMap<u64, oneshot::Sender<Response>>>,
) -> Result<Response> {
    let (is_leader, term) = {
        let server = state.server.read().await;
        (server.state == NodeState::Leader, server.current_term)
    };
    
    if !is_leader {
        return Err(ConsensusError::NotLeader);
    }
    
    // Create log entry
    let index = storage.last_index().await? + 1;
    let entry = crate::storage::LogEntry {
        term,
        index,
        data: serde_json::to_vec(&command)?,
    };
    
    // Append to log
    storage.append_entries(&[entry]).await?;
    
    // Create channel to wait for commit
    let (tx, rx) = oneshot::channel();
    pending_proposals.insert(index, tx);
    
    // Wait for the entry to be committed and applied
    match timeout(Duration::from_secs(30), rx).await {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(_)) => {
            pending_proposals.remove(&index);
            Err(ConsensusError::Internal("Failed to receive response".to_string()))
        }
        Err(_) => {
            pending_proposals.remove(&index);
            Err(ConsensusError::Timeout)
        }
    }
}

/// Apply committed entries to state machine
#[instrument(skip(state, storage, state_machine, pending_proposals))]
async fn apply_committed_entries<S: Storage, SM: StateMachine<Command = Command, Response = Response>>(
    state: &RaftState,
    storage: &S,
    state_machine: &tokio::sync::RwLock<SM>,
    pending_proposals: &Arc<dashmap::DashMap<u64, oneshot::Sender<Response>>>,
) -> Result<()> {
    let (commit_index, last_applied) = {
        let server = state.server.read().await;
        (server.commit_index, server.last_applied)
    };
    
    if commit_index > last_applied {
        for index in (last_applied + 1)..=commit_index {
            if let Some(entry) = storage.get_entry(index).await? {
                let command: Command = serde_json::from_slice(&entry.data)?;
                let mut sm = state_machine.write().await;
                let response = sm.apply(command).await;
                
                // Check if this was a pending proposal
                if let Some((_, tx)) = pending_proposals.remove(&index) {
                    let _ = tx.send(response);
                }
            }
        }
        
        // Update last applied
        let mut server = state.server.write().await;
        server.last_applied = commit_index;
    }
    
    Ok(())
}

impl<S, T, SM> RaftNode<S, T, SM>
where
    S: Storage + 'static + std::fmt::Debug,
    T: Transport + 'static + std::fmt::Debug,
    SM: StateMachine<Command = Command, Response = Response> + 'static,
{
    /// Start the Raft node
    pub async fn start(&self) -> Result<()> {
        info!(node_id = %self.config.node_id, "Starting Raft node");
        // TODO: Implement actual start logic
        Ok(())
    }
    
    /// Stop the Raft node
    pub async fn stop(&self) -> Result<()> {
        info!(node_id = %self.config.node_id, "Stopping Raft node");
        // Send shutdown signal
        let mut shutdown_guard = self.shutdown_tx.lock().await;
        if let Some(tx) = shutdown_guard.take() {
            if let Err(_) = tx.send(()) {
                warn!(node_id = %self.config.node_id, "Shutdown signal already sent");
            }
        }
        Ok(())
    }
    
    /// Check if this node is the leader
    pub async fn is_leader(&self) -> bool {
        let server = self.state.server.read().await;
        server.state == NodeState::Leader
    }
    
    /// Propose a command to the cluster
    pub async fn propose(&self, command: Vec<u8>) -> Result<Response> {
        // Create command
        let cmd = Command { data: command };
        
        // For now, just return success - TODO: implement actual consensus
        Ok(Response {
            success: true,
            data: b"OK".to_vec(),
        })
    }
    
    /// Add a node to the cluster
    pub async fn add_node(&self, _member: multivm_core::ClusterMember) -> Result<()> {
        // TODO: Implement cluster membership changes
        Ok(())
    }
    
    /// Remove a node from the cluster
    pub async fn remove_node(&self, _node_id: NodeId) -> Result<()> {
        // TODO: Implement cluster membership changes
        Ok(())
    }
    
    /// Join an existing cluster
    pub async fn join_cluster(&self, _members: Vec<multivm_core::ClusterMember>) -> Result<()> {
        // TODO: Implement cluster joining
        Ok(())
    }
}

#[async_trait]
impl<S, T, SM> ConsensusAlgorithm for RaftNode<S, T, SM>
where
    S: Storage + 'static + std::fmt::Debug,
    T: Transport + 'static + std::fmt::Debug,
    SM: StateMachine<Command = Command, Response = Response> + 'static,
{
    type Command = Command;
    type Response = Response;
    
    async fn propose(&self, command: Self::Command) -> Result<Self::Response> {
        let (tx, rx) = oneshot::channel();
        self.proposal_tx.send((command, tx)).await
            .map_err(|_| ConsensusError::ChannelSend)?;
        
        timeout(self.config.rpc_timeout, rx).await
            .map_err(|_| ConsensusError::Timeout)?
            .map_err(|_| ConsensusError::ChannelReceive)?
    }
    
    async fn get_leader(&self) -> Option<NodeId> {
        self.state.server.read().await.leader_id
    }
    
    async fn is_leader(&self) -> bool {
        self.state.server.read().await.state == NodeState::Leader
    }
    
    async fn get_state(&self) -> NodeState {
        self.state.server.read().await.state
    }
    
    async fn shutdown(&self) -> Result<()> {
        info!("Shutting down Raft node");
        
        // Send shutdown signal
        {
            let mut shutdown_guard = self.shutdown_tx.lock().await;
            if let Some(tx) = shutdown_guard.take() {
                let _ = tx.send(());
            }
        }
        
        // Clean up pending proposals
        let keys: Vec<u64> = self.pending_proposals.iter()
            .map(|entry| *entry.key())
            .collect();
        
        for key in keys {
            if let Some((_, tx)) = self.pending_proposals.remove(&key) {
                let _ = tx.send(Response {
                    success: false,
                    data: b"Node shutting down".to_vec(),
                });
            }
        }
        
        Ok(())
    }
}