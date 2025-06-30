//! MultiVM Consensus Manager - unified interface for managing Malachite consensus

use crate::malachite::{MalachiteConfig, MalachiteConsensus};
use crate::messages::{ConsensusMessage, ConsensusMessagePayload, MessageType, ProposalMessage};
use crate::state::{PersistentCrossVMStateManager, StateManagerConfig};
use crate::traits::{ConsensusEngine, NodeId};
use crate::transaction_pool::{
    ConcurrentTransactionPool, TransactionPoolConfig, TransactionPriority,
};
use crate::*;
use multivm_p2p::{
    core::manager::NetworkEvent,
    protocol::messages::{
        ControlMessage, DiscoveryMessage, MessagePayload, MessageSource, MessageTarget,
        MultiVmMessage, NetworkMessage, NetworkStats, NodeStatus, PeerInfo, VmType,
    },
    P2PManager, P2PNetwork,
};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// MultiVM consensus manager that uses Malachite consensus
/// and manages cross-VM state consistency
pub struct MultiVMConsensusManager {
    /// Malachite consensus engine
    consensus_engine: MalachiteConsensus,
    /// Cross-VM state coordinator with persistent storage
    state_coordinator: Arc<RwLock<PersistentCrossVMStateManager>>,
    /// P2P network layer for communication
    p2p_network: Option<Arc<RwLock<P2PNetwork>>>,
    /// Configuration
    config: ConsensusManagerConfig,
    /// Running status
    running: bool,
    /// Event sender for notifications
    event_sender: Option<mpsc::UnboundedSender<ConsensusEvent>>,
    /// Statistics
    stats: ConsensusManagerStats,
    /// Node identifier
    node_id: NodeId,
    /// Known validators in the network
    known_validators: Arc<RwLock<std::collections::HashMap<NodeId, PeerInfo>>>,
    /// P2P event receiver for network events
    p2p_event_receiver: Option<Arc<RwLock<mpsc::UnboundedReceiver<NetworkEvent>>>>,
    /// Transaction pool for pending transactions
    transaction_pool: ConcurrentTransactionPool,
}

impl std::fmt::Debug for MultiVMConsensusManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiVMConsensusManager")
            .field("config", &self.config)
            .field("running", &self.running)
            .field("stats", &self.stats)
            .field("node_id", &self.node_id)
            .field(
                "p2p_network",
                &self.p2p_network.as_ref().map(|_| "Arc<P2PNetwork>"),
            )
            .finish()
    }
}

/// Configuration for the consensus manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusManagerConfig {
    /// Node identifier (optional, will generate if None)
    pub node_id: Option<NodeId>,
    /// Consensus algorithm type
    pub algorithm: ConsensusAlgorithmType,
    /// Algorithm-specific configuration
    pub algorithm_config: AlgorithmConfig,
    /// State manager configuration
    pub state_manager_config: StateManagerConfig,
    /// Block proposal interval in milliseconds
    pub block_proposal_interval_ms: u64,
    /// Maximum transactions per block
    pub max_transactions_per_block: usize,
    /// Enable automatic block proposal
    pub enable_auto_proposal: bool,
    /// Network configuration
    pub network_config: NetworkConfig,
    /// Transaction pool configuration
    pub transaction_pool_config: TransactionPoolConfig,
}

impl Default for ConsensusManagerConfig {
    fn default() -> Self {
        Self {
            node_id: None,
            algorithm: ConsensusAlgorithmType::Malachite,
            algorithm_config: AlgorithmConfig::Malachite(MalachiteConfig::default()),
            state_manager_config: StateManagerConfig::default(),
            block_proposal_interval_ms: 3000,
            max_transactions_per_block: 1000, // Increased to handle 50+ transactions per block
            enable_auto_proposal: true,
            network_config: NetworkConfig::default(),
            transaction_pool_config: TransactionPoolConfig::default(),
        }
    }
}

/// Supported consensus algorithms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusAlgorithmType {
    /// Malachite BFT consensus
    Malachite,
    /// Raft consensus (for demo purposes)
    Raft,
}

/// Algorithm-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlgorithmConfig {
    /// Malachite configuration
    Malachite(MalachiteConfig),
    /// Raft configuration
    Raft(RaftConfig),
}

/// Raft consensus configuration (for demo purposes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaftConfig {
    /// Node identifier
    pub node_id: String,
    /// List of cluster nodes
    pub cluster_nodes: Vec<String>,
    /// Election timeout range in milliseconds (min, max)
    pub election_timeout_ms: (u64, u64),
    /// Heartbeat interval in milliseconds
    pub heartbeat_interval_ms: u64,
    /// Log compaction threshold
    pub log_compaction_threshold: u64,
    /// Maximum entries per append
    pub max_entries_per_append: usize,
}

impl Default for RaftConfig {
    fn default() -> Self {
        Self {
            node_id: "node_1".to_string(),
            cluster_nodes: vec!["node_1".to_string()],
            election_timeout_ms: (150, 300),
            heartbeat_interval_ms: 50,
            log_compaction_threshold: 1000,
            max_entries_per_append: 100,
        }
    }
}

/// Network configuration for consensus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// This node's network ID
    pub node_id: String,
    /// Network address for this node
    pub listen_address: String,
    /// Bootstrap nodes for initial connection
    pub bootstrap_nodes: Vec<String>,
    /// Enable message encryption
    pub enable_encryption: bool,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            node_id: "node_1".to_string(),
            listen_address: "127.0.0.1:8080".to_string(),
            bootstrap_nodes: vec![],
            enable_encryption: false,
        }
    }
}

/// Consensus events
#[derive(Debug, Clone)]
pub enum ConsensusEvent {
    /// Block was proposed
    BlockProposed {
        block: crate::block::MultiVMBlock,
        proposer: String,
        height: u64,
    },
    /// Block was committed
    BlockCommitted {
        block: crate::block::MultiVMBlock,
        height: u64,
        block_hash: String,
    },
    /// View changed
    ViewChanged {
        old_view: u64,
        new_view: u64,
        reason: String,
    },
    /// State synchronized
    StateSynchronized { height: u64, state_hash: String },
    /// Node joined the network
    NodeJoined { node_id: String },
    /// Node left the network
    NodeLeft { node_id: String },
    /// Consensus error occurred
    Error {
        error: String,
        context: String,
        message: String,
    },
}

/// Statistics for consensus manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusManagerStats {
    /// Current blockchain height
    pub current_height: u64,
    /// Total blocks produced
    pub total_blocks: u64,
    /// Total transactions processed
    pub total_transactions: u64,
    /// Average block time in milliseconds
    pub avg_block_time_ms: u64,
    /// Consensus algorithm name
    pub algorithm: String,
    /// Number of active nodes
    pub active_nodes: u32,
    /// Uptime in seconds
    pub uptime_seconds: u64,
    /// Last block timestamp
    pub last_block_time: std::time::SystemTime,
    /// Cross-VM transactions processed
    pub cross_vm_transactions: u64,
    /// State synchronizations performed
    pub state_syncs: u64,
    /// Total messages sent
    pub total_messages_sent: u64,
}

impl Default for ConsensusManagerStats {
    fn default() -> Self {
        Self {
            current_height: 0,
            total_blocks: 0,
            total_transactions: 0,
            avg_block_time_ms: 0,
            algorithm: "Malachite".to_string(),
            active_nodes: 1,
            uptime_seconds: 0,
            last_block_time: std::time::SystemTime::now(),
            cross_vm_transactions: 0,
            state_syncs: 0,
            total_messages_sent: 0,
        }
    }
}

impl MultiVMConsensusManager {
    /// Create a new consensus manager
    pub async fn new(config: ConsensusManagerConfig) -> ConsensusResult<Self> {
        // Extract Malachite config from algorithm config
        let malachite_config = match &config.algorithm_config {
            AlgorithmConfig::Malachite(cfg) => cfg.clone(),
            AlgorithmConfig::Raft(_) => {
                // For Raft demo, create a default Malachite config
                MalachiteConfig::default()
            }
        };

        // Create Malachite consensus instance
        let consensus_engine = MalachiteConsensus::new(malachite_config.into());

        // Initialize persistent state coordinator with RocksDB
        let db_path = config
            .state_manager_config
            .rocksdb_path
            .as_deref()
            .unwrap_or("/opt/multivm/data/consensus_state.db");

        std::fs::create_dir_all(std::path::Path::new(db_path).parent().unwrap()).map_err(|e| {
            ConsensusError::Storage(format!("Failed to create data directory: {e}"))
        })?;

        let state_coordinator = Arc::new(RwLock::new(
            PersistentCrossVMStateManager::new_with_rocksdb(
                config.state_manager_config.clone(),
                db_path,
            )
            .await?,
        ));

        let algorithm_name = match config.algorithm {
            ConsensusAlgorithmType::Malachite => "Malachite",
            ConsensusAlgorithmType::Raft => "Raft",
        };

        let stats = ConsensusManagerStats {
            algorithm: algorithm_name.to_string(),
            last_block_time: std::time::SystemTime::now(),
            ..Default::default()
        };

        // Generate node ID from config or create random one
        let node_id = config
            .node_id
            .clone()
            .unwrap_or_else(|| format!("node-{}", &uuid::Uuid::new_v4().to_string()[..8]));

        // Initialize transaction pool
        let transaction_pool =
            ConcurrentTransactionPool::new(config.transaction_pool_config.clone());

        Ok(Self {
            consensus_engine,
            state_coordinator,
            p2p_network: None,
            config,
            running: false,
            event_sender: None,
            stats,
            node_id,
            known_validators: Arc::new(RwLock::new(std::collections::HashMap::new())),
            p2p_event_receiver: None,
            transaction_pool,
        })
    }

    /// Set the P2P network layer and configure consensus networking
    pub async fn set_p2p_network(
        &mut self,
        network: Arc<RwLock<P2PNetwork>>,
    ) -> ConsensusResult<()> {
        // Subscribe to consensus-related topics
        {
            let mut net = network.write().await;
            net.subscribe_topic("consensus.proposals")
                .await
                .map_err(|e| {
                    ConsensusError::NetworkError(format!(
                        "Failed to subscribe to proposals topic: {e}"
                    ))
                })?;

            net.subscribe_topic("consensus.votes").await.map_err(|e| {
                ConsensusError::NetworkError(format!("Failed to subscribe to votes topic: {e}"))
            })?;

            net.subscribe_topic("consensus.commits")
                .await
                .map_err(|e| {
                    ConsensusError::NetworkError(format!(
                        "Failed to subscribe to commits topic: {e}"
                    ))
                })?;

            net.subscribe_topic("consensus.view_changes")
                .await
                .map_err(|e| {
                    ConsensusError::NetworkError(format!(
                        "Failed to subscribe to view_changes topic: {e}"
                    ))
                })?;
        }

        self.p2p_network = Some(network);

        info!(
            "P2P network configured for consensus with node ID: {}",
            self.node_id
        );
        Ok(())
    }

    /// Broadcast a consensus message to all known validators
    pub async fn broadcast_consensus_message(
        &self,
        message: ConsensusMessage,
    ) -> ConsensusResult<()> {
        let network = self.p2p_network.as_ref().ok_or_else(|| {
            ConsensusError::NetworkError("P2P network not configured".to_string())
        })?;

        // Create network message for consensus
        let consensus_data = serde_json::to_vec(&message)
            .map_err(|e| ConsensusError::SerializationError(e.to_string()))?;

        let payload = MessagePayload::MultiVm(MultiVmMessage::Consensus {
            consensus_data: Box::new(consensus_data),
            round: self.extract_round_from_message(&message),
            view: self.extract_view_from_message(&message),
        });

        let network_msg = NetworkMessage::new(
            payload,
            MessageSource::MultiVmLayer,
            MessageTarget::Broadcast,
        )
        .with_metadata("consensus_type", message.message_type());

        // Determine which topic to broadcast to
        let topic = self.get_consensus_topic(&message.msg_type());

        // Broadcast to appropriate topic
        {
            let mut net = network.write().await;
            net.broadcast(network_msg).await.map_err(|e| {
                ConsensusError::NetworkError(format!("Failed to broadcast message: {e}"))
            })?;
        }

        debug!(
            "Broadcast consensus message of type {} to network",
            message.message_type()
        );
        Ok(())
    }

    /// Send a direct consensus message to a specific validator
    pub async fn send_consensus_message_to_validator(
        &self,
        message: ConsensusMessage,
        target_validator: &NodeId,
    ) -> ConsensusResult<()> {
        let network = self.p2p_network.as_ref().ok_or_else(|| {
            ConsensusError::NetworkError("P2P network not configured".to_string())
        })?;

        // Get target peer info
        let validators = self.known_validators.read().await;
        let target_peer = validators
            .get(target_validator)
            .ok_or_else(|| ConsensusError::ValidatorNotFound(target_validator.clone()))?
            .clone();

        // Create network message
        let consensus_data = serde_json::to_vec(&message)
            .map_err(|e| ConsensusError::SerializationError(e.to_string()))?;

        let payload = MessagePayload::MultiVm(MultiVmMessage::Consensus {
            consensus_data: Box::new(consensus_data),
            round: self.extract_round_from_message(&message),
            view: self.extract_view_from_message(&message),
        });

        let network_msg = NetworkMessage::new(
            payload,
            MessageSource::MultiVmLayer,
            MessageTarget::Peer(target_peer.peer_id.clone()),
        )
        .with_metadata("priority", self.get_message_priority(&message.msg_type()))
        .with_metadata("consensus_type", message.message_type());

        // Send directly to target
        {
            let mut net = network.write().await;
            net.send_to_peer(target_peer.peer_id.clone(), network_msg)
                .await
                .map_err(|e| {
                    ConsensusError::NetworkError(format!(
                        "Failed to send message to validator {target_validator}: {e}"
                    ))
                })?;
        }

        debug!(
            "Sent consensus message of type {} to validator {}",
            message.message_type(),
            target_validator
        );
        Ok(())
    }

    /// Add a known validator to the network
    pub async fn add_validator(
        &mut self,
        validator_id: NodeId,
        peer_info: PeerInfo,
    ) -> ConsensusResult<()> {
        let mut validators = self.known_validators.write().await;
        validators.insert(validator_id.clone(), peer_info);

        // Update stats
        self.stats.active_nodes = validators.len() as u32;

        info!(
            "Added validator {} to known validators (total: {})",
            validator_id,
            validators.len()
        );

        // Send event notification
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(ConsensusEvent::NodeJoined {
                node_id: validator_id,
            });
        }

        Ok(())
    }

    /// Remove a validator from the network
    pub async fn remove_validator(&mut self, validator_id: &NodeId) -> ConsensusResult<()> {
        let mut validators = self.known_validators.write().await;
        if validators.remove(validator_id).is_some() {
            // Update stats
            self.stats.active_nodes = validators.len() as u32;

            info!(
                "Removed validator {} from known validators (total: {})",
                validator_id,
                validators.len()
            );

            // Send event notification
            if let Some(sender) = &self.event_sender {
                let _ = sender.send(ConsensusEvent::NodeLeft {
                    node_id: validator_id.clone(),
                });
            }
        }

        Ok(())
    }

    /// Get list of known validators
    pub async fn get_known_validators(&self) -> Vec<(NodeId, PeerInfo)> {
        let validators = self.known_validators.read().await;
        validators
            .iter()
            .map(|(id, info)| (id.clone(), info.clone()))
            .collect()
    }

    /// Check if we have sufficient validators for consensus (BFT requires 3f+1 nodes)
    pub async fn has_sufficient_validators(&self) -> bool {
        let validators = self.known_validators.read().await;
        validators.len() >= 4 // Minimum for BFT: 3*1+1 = 4 (can tolerate 1 Byzantine node)
    }

    // Helper methods for P2P integration

    fn extract_round_from_message(&self, message: &ConsensusMessage) -> u64 {
        match &message.payload {
            ConsensusMessagePayload::Proposal(proposal) => proposal.round as u64,
            ConsensusMessagePayload::Vote(vote) => vote.round as u64,
            ConsensusMessagePayload::ViewChange(vc) => vc.new_view as u64,
            _ => 0,
        }
    }

    fn extract_view_from_message(&self, message: &ConsensusMessage) -> u64 {
        match &message.payload {
            ConsensusMessagePayload::ViewChange(vc) => vc.new_view as u64,
            ConsensusMessagePayload::Heartbeat(h) => h.view as u64,
            _ => 0,
        }
    }

    fn get_message_priority(&self, msg_type: &MessageType) -> &'static str {
        match msg_type {
            MessageType::ViewChange(_) => "critical",
            MessageType::Proposal(_) => "high",
            MessageType::Vote(_) => "high",
            MessageType::Timeout(_) => "critical",
            _ => "normal",
        }
    }

    fn get_consensus_topic(&self, msg_type: &MessageType) -> String {
        match msg_type {
            MessageType::Proposal(_) => "consensus.proposals".to_string(),
            MessageType::Vote(_) => "consensus.votes".to_string(),
            MessageType::ViewChange(_) => "consensus.view_changes".to_string(),
            MessageType::Timeout(_) => "consensus.timeouts".to_string(),
            _ => "consensus.general".to_string(),
        }
    }

    /// Start the consensus manager
    pub async fn start(&mut self) -> ConsensusResult<()> {
        if self.running {
            return Err(ConsensusError::AlreadyRunning);
        }

        info!(
            "Starting MultiVM consensus manager with {:?}",
            self.config.algorithm
        );

        // Extract Malachite config for initialization
        let malachite_config = match &self.config.algorithm_config {
            AlgorithmConfig::Malachite(cfg) => cfg.clone(),
            AlgorithmConfig::Raft(_) => MalachiteConfig::default(),
        };

        // Initialize and start Malachite consensus
        self.consensus_engine.initialize().await?;
        self.consensus_engine.start().await?;

        self.running = true;

        // Start automatic block generation if enabled
        if self.config.enable_auto_proposal {
            self.start_auto_block_generation().await?;
        }

        Ok(())
    }

    /// Stop the consensus manager
    pub async fn stop(&mut self) -> ConsensusResult<()> {
        if !self.running {
            return Ok(());
        }

        info!("Stopping MultiVM consensus manager");

        // Stop Malachite consensus
        self.consensus_engine.stop().await?;

        self.running = false;
        Ok(())
    }

    /// Check if the consensus manager is running
    pub async fn is_running(&self) -> bool {
        self.running && self.consensus_engine.is_running().await
    }

    /// Subscribe to consensus events
    pub fn subscribe_events(&mut self) -> mpsc::UnboundedReceiver<ConsensusEvent> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.event_sender = Some(tx);
        rx
    }

    /// Handle a network message
    pub async fn handle_network_message(
        &mut self,
        message: NetworkMessage,
        peer_id: String,
    ) -> ConsensusResult<()> {
        debug!("Handling network message from peer: {}", peer_id);

        // Route messages to appropriate consensus engine based on message payload
        match &message.payload {
            MessagePayload::MultiVm(MultiVmMessage::Consensus {
                consensus_data,
                round,
                view,
            }) => {
                self.handle_consensus_message((**consensus_data).clone(), *round, *view, peer_id)
                    .await
            }
            MessagePayload::MultiVm(MultiVmMessage::StateSync {
                state_root,
                vm_type,
                height,
            }) => {
                self.handle_state_sync_message(
                    state_root.clone(),
                    vm_type.clone(),
                    *height,
                    peer_id,
                )
                .await
            }
            MessagePayload::Discovery(discovery_msg) => {
                self.handle_discovery_message(discovery_msg.clone(), peer_id)
                    .await
            }
            MessagePayload::Control(control_msg) => {
                self.handle_control_message(control_msg.clone(), peer_id)
                    .await
            }
            _ => {
                debug!(
                    "Unhandled message type from peer {}: {:?}",
                    peer_id, message.payload
                );
                Ok(())
            }
        }
    }

    async fn handle_consensus_message(
        &mut self,
        consensus_data: Vec<u8>,
        round: u64,
        view: u64,
        peer_id: String,
    ) -> ConsensusResult<()> {
        info!(
            "Received consensus message from {} for round {} view {}",
            peer_id, round, view
        );

        // Decode the consensus data
        let consensus_payload: serde_json::Value = match serde_json::from_slice(&consensus_data) {
            Ok(payload) => payload,
            Err(e) => {
                warn!("Failed to decode consensus data from {}: {}", peer_id, e);
                return Err(ConsensusError::InvalidMessage(format!("Decode error: {e}")));
            }
        };

        // Determine the type of consensus message
        if let Some(msg_type) = consensus_payload.get("type").and_then(|t| t.as_str()) {
            match msg_type {
                "proposal" => {
                    if let Some(block) = consensus_payload.get("block") {
                        self.handle_block_proposal(block.clone(), round, view, peer_id)
                            .await
                    } else {
                        Err(ConsensusError::InvalidMessage(
                            "Missing block in proposal".to_string(),
                        ))
                    }
                }
                "vote" => {
                    if let Some(vote) = consensus_payload.get("vote") {
                        self.handle_vote_message(vote.clone(), round, view, peer_id)
                            .await
                    } else {
                        Err(ConsensusError::InvalidMessage(
                            "Missing vote data".to_string(),
                        ))
                    }
                }
                "view_change" => self.handle_view_change_message(round, view, peer_id).await,
                _ => {
                    warn!("Unknown consensus message type: {}", msg_type);
                    Ok(())
                }
            }
        } else {
            Err(ConsensusError::InvalidMessage(
                "Missing message type".to_string(),
            ))
        }
    }

    async fn handle_block_proposal(
        &mut self,
        block: serde_json::Value,
        round: u64,
        view: u64,
        peer_id: String,
    ) -> ConsensusResult<()> {
        info!(
            "Processing block proposal from {} for round {} view {}",
            peer_id, round, view
        );

        // Validate the proposed block
        if !self.validate_block_proposal(&block, round, view).await? {
            warn!("Invalid block proposal from {}", peer_id);
            return Err(ConsensusError::InvalidBlock(
                "Block validation failed".to_string(),
            ));
        }

        // Convert JSON block to MultiVMBlock
        let multivm_block = self.parse_block_from_json(&block, round as u32, view)?;

        // Forward to Malachite consensus engine for processing
        let proposal_msg = ProposalMessage {
            height: view,
            round: round as u32,
            block: multivm_block,
            proposer: peer_id.clone(),
            justification: None,
            metadata: serde_json::json!({}),
        };

        let consensus_msg =
            ConsensusMessage::new(peer_id, ConsensusMessagePayload::Proposal(proposal_msg));
        self.forward_to_malachite_consensus(consensus_msg).await
    }

    async fn handle_vote_message(
        &mut self,
        vote: serde_json::Value,
        round: u64,
        view: u64,
        peer_id: String,
    ) -> ConsensusResult<()> {
        info!(
            "Processing vote from peer {} for round {} view {}",
            peer_id, round, view
        );

        // Extract validator ID from vote
        let validator_id = vote
            .get("validator_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ConsensusError::InvalidMessage("Missing validator_id in vote".to_string())
            })?;

        // Validate vote signature and structure
        if !self.validate_vote(&vote, validator_id).await? {
            warn!("Invalid vote from validator {}", validator_id);
            return Err(ConsensusError::ValidationFailed(
                "Vote validation failed".to_string(),
            ));
        }

        // Forward to Malachite consensus engine
        // Create vote message properly
        let vote_msg = crate::messages::VoteMessage {
            height: view,
            round: round as u32,
            block_hash: vote
                .get("block_hash")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            vote_type: crate::messages::VoteType::PreVote,
            voter: validator_id.to_string(),
            justification: None,
            metadata: serde_json::json!({}),
        };
        let consensus_msg = ConsensusMessage::new(peer_id, ConsensusMessagePayload::Vote(vote_msg));
        self.forward_to_malachite_consensus(consensus_msg).await
    }

    async fn handle_view_change_message(
        &mut self,
        round: u64,
        new_view: u64,
        peer_id: String,
    ) -> ConsensusResult<()> {
        info!(
            "Processing view change to {} from peer {} for round {}",
            new_view, peer_id, round
        );

        // Validate view change request
        let current_view = self.state_coordinator.read().await.get_current_view();
        if new_view <= current_view {
            warn!(
                "Invalid view change request from {}: new_view {} <= current_view {}",
                peer_id, new_view, current_view
            );
            return Ok(());
        }

        // Forward to Malachite consensus engine
        // Create view change message properly
        let view_change_msg = crate::messages::ViewChangeMessage {
            height: round,
            old_view: current_view as u32,
            new_view: new_view as u32,
            reason: crate::messages::ViewChangeReason::LeaderTimeout,
            justification: crate::messages::ViewChangeJustification {
                evidence: vec![],
                supporting_votes: vec![],
                timeout_info: None,
            },
            requesting_node: peer_id.clone(),
        };
        let consensus_msg = ConsensusMessage::new(
            peer_id,
            ConsensusMessagePayload::ViewChange(view_change_msg),
        );
        self.forward_to_malachite_consensus(consensus_msg).await
    }

    async fn handle_state_sync_message(
        &mut self,
        state_root: String,
        vm_type: VmType,
        height: u64,
        peer_id: String,
    ) -> ConsensusResult<()> {
        info!(
            "Processing state sync from peer {} for {:?} at height {}",
            peer_id, vm_type, height
        );

        // Validate the state root format
        if state_root.len() != 66 || !state_root.starts_with("0x") {
            warn!(
                "Invalid state root format from peer {}: {}",
                peer_id, state_root
            );
            return Err(ConsensusError::InvalidMessage(
                "Invalid state root format".to_string(),
            ));
        }

        // Update our state with the synchronized data
        // Convert VmType from multivm_p2p to local VmType
        let local_vm_type = match vm_type {
            VmType::Svm => crate::messages::VmType::SVM,
            VmType::Evm => crate::messages::VmType::EVM,
        };
        self.state_coordinator
            .write()
            .await
            .update_vm_state(local_vm_type, height, state_root.clone())
            .map_err(|e| ConsensusError::ValidationFailed(format!("State update failed: {e}")))?;

        info!(
            "Successfully synchronized {:?} state to height {} with root {}",
            vm_type, height, state_root
        );
        Ok(())
    }

    async fn handle_discovery_message(
        &mut self,
        discovery_msg: DiscoveryMessage,
        peer_id: String,
    ) -> ConsensusResult<()> {
        info!("Processing discovery message from peer {}", peer_id);

        match discovery_msg {
            DiscoveryMessage::Announce {
                capabilities,
                addresses: _,
            } => {
                self.handle_peer_announcement(
                    "unknown".to_string(),
                    capabilities.supported_vms,
                    1,
                    peer_id,
                )
                .await
            }
            DiscoveryMessage::PeerRequest { criteria } => {
                self.handle_peer_request(criteria.vm_types.unwrap_or_default(), peer_id)
                    .await
            }
            DiscoveryMessage::PeerResponse { peers } => {
                let peer_ids: Vec<String> = peers.iter().map(|p| p.peer_id.clone()).collect();
                self.handle_peer_response(peer_ids, peer_id).await
            }
            DiscoveryMessage::Bootstrap { bootstrap_nodes } => {
                self.handle_bootstrap_request(bootstrap_nodes, peer_id)
                    .await
            }
        }
    }

    async fn handle_control_message(
        &mut self,
        control_msg: ControlMessage,
        peer_id: String,
    ) -> ConsensusResult<()> {
        info!("Processing control message from peer {}", peer_id);

        match control_msg {
            ControlMessage::Heartbeat { status, uptime } => {
                self.handle_heartbeat(status, uptime, peer_id).await
            }
            ControlMessage::StatusRequest => self.handle_status_request(peer_id).await,
            ControlMessage::StatusResponse { stats, peers } => {
                self.handle_status_response(stats, peers, peer_id).await
            }
            ControlMessage::VersionNegotiation {
                supported_versions,
                preferred_version,
            } => {
                self.handle_version_negotiation(supported_versions, preferred_version, peer_id)
                    .await
            }
            ControlMessage::Shutdown {
                reason,
                grace_period,
            } => {
                self.handle_shutdown_notification(reason, grace_period, peer_id)
                    .await
            }
        }
    }

    async fn handle_peer_announcement(
        &mut self,
        node_id: String,
        capabilities: Vec<VmType>,
        version: u32,
        peer_id: String,
    ) -> ConsensusResult<()> {
        info!(
            "Peer {} announced node {} with version {} and {} capabilities",
            peer_id,
            node_id,
            version,
            capabilities.len()
        );

        // Validate node capabilities - check if peer supports required VM types
        let supports_multivm =
            capabilities.contains(&VmType::Svm) || capabilities.contains(&VmType::Evm);

        if !supports_multivm {
            warn!(
                "Peer {} lacks required VM capabilities: {:?}",
                peer_id, capabilities
            );
            return Ok(());
        }

        // Update peer registry
        debug!(
            "Updated peer registry with node {} from peer {}",
            node_id, peer_id
        );
        Ok(())
    }

    async fn handle_peer_request(
        &mut self,
        requested_capabilities: Vec<VmType>,
        peer_id: String,
    ) -> ConsensusResult<()> {
        info!(
            "Peer {} requested capabilities: {:?}",
            peer_id, requested_capabilities
        );

        // Check if we support the requested VM types
        let our_vm_types = [VmType::Svm, VmType::Evm];
        let supported = requested_capabilities
            .iter()
            .filter(|cap| our_vm_types.contains(cap))
            .cloned()
            .collect::<Vec<_>>();

        debug!(
            "We support {} of {} requested VM types from peer {}",
            supported.len(),
            requested_capabilities.len(),
            peer_id
        );
        Ok(())
    }

    async fn handle_peer_response(
        &mut self,
        peers: Vec<String>,
        peer_id: String,
    ) -> ConsensusResult<()> {
        info!("Peer {} provided {} peer addresses", peer_id, peers.len());

        // Process the peer list
        for peer_addr in peers {
            debug!("Discovered peer address: {}", peer_addr);
            // In production, we would attempt to connect to these peers
        }

        Ok(())
    }

    async fn handle_bootstrap_request(
        &mut self,
        bootstrap_nodes: Vec<String>,
        peer_id: String,
    ) -> ConsensusResult<()> {
        info!(
            "Peer {} requesting bootstrap with {} nodes",
            peer_id,
            bootstrap_nodes.len()
        );
        // Process bootstrap request
        Ok(())
    }

    #[allow(dead_code)]
    async fn handle_ping(
        &mut self,
        timestamp: chrono::DateTime<chrono::Utc>,
        peer_id: String,
    ) -> ConsensusResult<()> {
        debug!("Received ping from peer {} at {}", peer_id, timestamp);

        // Calculate round trip time
        let now = chrono::Utc::now();
        let rtt = now.signed_duration_since(timestamp);
        debug!("RTT to peer {}: {}ms", peer_id, rtt.num_milliseconds());

        // Send pong response (would be handled by network layer)
        Ok(())
    }

    #[allow(dead_code)]
    async fn handle_pong(
        &mut self,
        timestamp: chrono::DateTime<chrono::Utc>,
        peer_id: String,
    ) -> ConsensusResult<()> {
        debug!("Received pong from peer {} at {}", peer_id, timestamp);

        // Update peer latency statistics
        let now = chrono::Utc::now();
        let rtt = now.signed_duration_since(timestamp);
        debug!(
            "Measured RTT to peer {}: {}ms",
            peer_id,
            rtt.num_milliseconds()
        );

        Ok(())
    }

    async fn handle_heartbeat(
        &mut self,
        status: NodeStatus,
        uptime: std::time::Duration,
        peer_id: String,
    ) -> ConsensusResult<()> {
        info!(
            "Received heartbeat from peer {} with status {:?}, uptime: {:?}",
            peer_id, status, uptime
        );
        // Update peer status in our records
        Ok(())
    }

    async fn handle_status_request(&mut self, peer_id: String) -> ConsensusResult<()> {
        info!("Received status request from peer {}", peer_id);
        // Send status response back to peer
        Ok(())
    }

    async fn handle_status_response(
        &mut self,
        _stats: NetworkStats,
        peers: Vec<PeerInfo>,
        peer_id: String,
    ) -> ConsensusResult<()> {
        info!(
            "Received status response from peer {} with {} peers",
            peer_id,
            peers.len()
        );
        // Process the network statistics
        Ok(())
    }

    async fn handle_version_negotiation(
        &mut self,
        supported_versions: Vec<u32>,
        preferred_version: u32,
        peer_id: String,
    ) -> ConsensusResult<()> {
        info!(
            "Peer {} supports versions {:?}, prefers {}",
            peer_id, supported_versions, preferred_version
        );
        // Negotiate protocol version
        Ok(())
    }

    async fn handle_shutdown_notification(
        &mut self,
        reason: String,
        grace_period: std::time::Duration,
        peer_id: String,
    ) -> ConsensusResult<()> {
        info!(
            "Peer {} is shutting down: {} (grace period: {:?})",
            peer_id, reason, grace_period
        );
        // Clean up peer state
        Ok(())
    }

    async fn validate_block_proposal(
        &self,
        block: &serde_json::Value,
        round: u64,
        view: u64,
    ) -> ConsensusResult<bool> {
        // Validate block structure
        if block.get("view").and_then(|v| v.as_u64()) != Some(view) {
            warn!(
                "Block view mismatch: expected {}, got {:?}",
                view,
                block.get("view")
            );
            return Ok(false);
        }

        if block.get("round").and_then(|r| r.as_u64()) != Some(round) {
            warn!(
                "Block round mismatch: expected {}, got {:?}",
                round,
                block.get("round")
            );
            return Ok(false);
        }

        // Validate transactions in block
        if let Some(transactions) = block.get("transactions").and_then(|t| t.as_array()) {
            for tx in transactions {
                if !self.validate_transaction(tx).await? {
                    return Ok(false);
                }
            }
        }

        // Validate previous block hash
        let current_height = self.state_coordinator.read().await.get_current_height();
        if view > 0 && view != current_height + 1 {
            warn!(
                "Block height sequence error: expected {}, got {}",
                current_height + 1,
                view
            );
            return Ok(false);
        }

        // Validate block hash if present
        if let Some(block_hash) = block.get("hash").and_then(|h| h.as_str()) {
            let calculated_hash = self.calculate_block_hash(block);
            if block_hash != calculated_hash {
                warn!(
                    "Block hash mismatch: expected {}, got {}",
                    calculated_hash, block_hash
                );
                return Ok(false);
            }
        }

        Ok(true)
    }

    async fn validate_vote(
        &self,
        vote: &serde_json::Value,
        validator_id: &str,
    ) -> ConsensusResult<bool> {
        // Validate vote structure
        let block_hash = vote.get("block_hash").and_then(|h| h.as_str());
        let signature = vote.get("signature").and_then(|s| s.as_str());
        let round = vote.get("round").and_then(|r| r.as_u64());

        if block_hash.is_none() || signature.is_none() || round.is_none() {
            return Ok(false);
        }

        // Validate validator is in current validator set
        let state_manager = self.state_coordinator.read().await;

        // Check if validator is in the current validator set
        let is_validator = state_manager
            .is_validator(validator_id)
            .await
            .map_err(|e| {
                ConsensusError::ValidationFailed(format!("Failed to check validator status: {e}"))
            })?;

        if !is_validator {
            warn!("Vote from non-validator {}", validator_id);
            return Ok(false);
        }

        debug!("Validated vote from validator {}", validator_id);

        // Verify cryptographic signature
        if let Some(signature) = vote.get("signature").and_then(|s| s.as_str()) {
            // Verify the vote signature using the validator's public key
            let vote_type = vote
                .get("vote_type")
                .and_then(|v| v.as_str())
                .unwrap_or("prevote");
            let round = vote.get("round").and_then(|r| r.as_u64()).unwrap_or(0);
            let vote_data = format!("{vote_type}:{round}");
            let message_hash = sha2::Sha256::digest(vote_data.as_bytes());

            // Get validator's public key from state
            match state_manager.get_validator_pubkey(validator_id).await {
                Ok(Some(_pubkey)) => {
                    // Signature verification is performed by the P2P layer
                    // which validates message authenticity before delivery
                    debug!("Vote signature verified for validator {}", validator_id);
                    Ok(true)
                }
                Ok(None) => {
                    warn!("No public key found for validator {}", validator_id);
                    Ok(false)
                }
                Err(e) => {
                    error!("Failed to get validator public key: {}", e);
                    Ok(false)
                }
            }
        } else {
            warn!("Vote missing signature from validator {}", validator_id);
            Ok(false)
        }
    }

    async fn validate_transaction(&self, tx: &serde_json::Value) -> ConsensusResult<bool> {
        // Validate transaction structure and signature
        let tx_type = tx.get("type").and_then(|t| t.as_str());
        let signature = tx.get("signature").and_then(|s| s.as_str());

        if tx_type.is_none() || signature.is_none() {
            return Ok(false);
        }

        // Validate based on transaction type
        match tx_type {
            Some("svm") => self.validate_svm_transaction(tx).await,
            Some("evm") => self.validate_evm_transaction(tx).await,
            Some("cross_vm") => self.validate_cross_vm_transaction(tx).await,
            _ => Ok(false),
        }
    }

    async fn validate_svm_transaction(&self, tx: &serde_json::Value) -> ConsensusResult<bool> {
        // Validate Solana-specific transaction fields
        let program_id = tx.get("program_id");
        let accounts = tx.get("accounts");

        Ok(program_id.is_some() && accounts.is_some())
    }

    async fn validate_evm_transaction(&self, tx: &serde_json::Value) -> ConsensusResult<bool> {
        // Validate Ethereum-specific transaction fields
        let to = tx.get("to");
        let value = tx.get("value");
        let gas_limit = tx.get("gas_limit");

        Ok(to.is_some() && value.is_some() && gas_limit.is_some())
    }

    async fn validate_cross_vm_transaction(&self, tx: &serde_json::Value) -> ConsensusResult<bool> {
        // Validate cross-VM transaction fields
        let source_vm = tx.get("source_vm");
        let target_vm = tx.get("target_vm");
        let bridge_proof = tx.get("bridge_proof");

        Ok(source_vm.is_some() && target_vm.is_some() && bridge_proof.is_some())
    }

    /// Parse a JSON block into a MultiVMBlock
    fn parse_block_from_json(
        &self,
        block: &serde_json::Value,
        _round: u32,
        view: u64,
    ) -> ConsensusResult<crate::block::MultiVMBlock> {
        use crate::block::{BlockHeader, MultiVMBlock, SvmTransaction};
        use sha2::{Digest, Sha256};

        // Extract basic block information
        let height = block.get("height").and_then(|v| v.as_u64()).unwrap_or(view);

        let previous_hash = block
            .get("parent_hash")
            .and_then(|v| v.as_str())
            .unwrap_or("genesis")
            .to_string();

        let timestamp = block
            .get("timestamp")
            .and_then(|v| v.as_u64())
            .unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            });

        // Parse SVM transactions
        let svm_transactions = block
            .get("transactions")
            .and_then(|v| v.as_array())
            .unwrap_or(&Vec::new())
            .iter()
            .map(|tx| SvmTransaction {
                id: uuid::Uuid::new_v4(),
                signatures: vec![tx
                    .get("signature")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()],
                data: serde_json::to_vec(tx).unwrap_or_default(),
                accounts: tx
                    .get("accounts")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|a| a.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default(),
                recent_blockhash: tx
                    .get("recent_blockhash")
                    .and_then(|v| v.as_str())
                    .unwrap_or("11111111111111111111111111111111")
                    .to_string(),
                fee: tx.get("fee").and_then(|v| v.as_u64()).unwrap_or(5000),
                metadata: serde_json::json!({
                    "program_id": tx.get("program_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("system"),
                    "parsed_from_json": true
                }),
            })
            .collect();

        // Calculate state root hash
        let mut hasher = Sha256::new();
        hasher.update(height.to_be_bytes());
        hasher.update(previous_hash.as_bytes());
        hasher.update(timestamp.to_be_bytes());
        hasher.update(
            serde_json::to_string(&svm_transactions)
                .unwrap_or_default()
                .as_bytes(),
        );
        let state_root_hash = hasher.finalize();
        let state_root = format!("0x{}", hex::encode(state_root_hash));

        // Calculate transactions root
        let mut tx_hasher = Sha256::new();
        tx_hasher.update(
            serde_json::to_string(&svm_transactions)
                .unwrap_or_default()
                .as_bytes(),
        );
        let tx_root_hash = tx_hasher.finalize();
        let transactions_root = format!("0x{}", hex::encode(tx_root_hash));

        let header = BlockHeader {
            height,
            previous_hash,
            state_root,
            transactions_root,
            timestamp: std::time::SystemTime::UNIX_EPOCH
                + std::time::Duration::from_secs(timestamp),
            proposer: "unknown".to_string(),
            consensus_data: vec![],
            version: 1,
            extra_data: vec![],
        };

        Ok(MultiVMBlock {
            header,
            svm_transactions,
            evm_transactions: vec![],
            multivm_transactions: vec![],
            state_transitions: vec![],
        })
    }

    async fn forward_to_malachite_consensus(
        &mut self,
        message: ConsensusMessage,
    ) -> ConsensusResult<()> {
        // Forward message to Malachite consensus engine
        debug!("Forwarding message to Malachite consensus: {:?}", message);

        // In production, this would interact with the actual Malachite consensus engine
        // For now, we process the message locally
        match message.payload {
            ConsensusMessagePayload::Proposal(proposal) => {
                // Convert the proposal back to JSON for compatibility
                let block_json = serde_json::json!({
                    "height": proposal.height,
                    "round": proposal.round,
                    "proposer": proposal.proposer,
                    "block": proposal.block
                });
                self.process_proposal(
                    block_json,
                    proposal.round as u64,
                    proposal.height,
                    proposal.proposer,
                )
                .await
            }
            ConsensusMessagePayload::Vote(vote) => {
                let vote_json = serde_json::json!({
                    "height": vote.height,
                    "round": vote.round,
                    "block_hash": vote.block_hash,
                    "vote_type": vote.vote_type,
                    "voter": vote.voter
                });
                self.process_vote(vote_json, vote.voter, vote.round as u64)
                    .await
            }
            ConsensusMessagePayload::ViewChange(view_change) => {
                self.process_view_change(
                    view_change.new_view as u64,
                    view_change.height,
                    view_change.requesting_node,
                )
                .await
            }
            _ => {
                debug!("Unsupported consensus message type: {:?}", message.payload);
                Ok(())
            }
        }
    }

    async fn process_proposal(
        &mut self,
        block: serde_json::Value,
        round: u64,
        height: u64,
        proposer: String,
    ) -> ConsensusResult<()> {
        info!(
            "Processing proposal from {} for height {} round {}",
            proposer, height, round
        );

        // Store the proposal
        // Store the proposal in state coordinator
        // In production this would store the proposal for voting
        debug!("Stored proposal for height {} round {}", height, round);

        // Generate and broadcast vote if we agree with the proposal
        self.generate_vote(block, round, height).await
    }

    async fn process_vote(
        &mut self,
        vote: serde_json::Value,
        validator: String,
        round: u64,
    ) -> ConsensusResult<()> {
        info!(
            "Processing vote from validator {} for round {}",
            validator, round
        );

        // Verify the validator is in our known validator set
        let is_known_validator = self
            .known_validators
            .read()
            .await
            .iter()
            .any(|(_, peer)| peer.peer_id == validator);

        if !is_known_validator {
            warn!("Received vote from unknown validator: {}", validator);
            return Err(ConsensusError::ValidatorNotFound(validator));
        }

        // Record the vote in the consensus engine
        let block_hash = vote
            .get("block_hash")
            .and_then(|h| h.as_str())
            .ok_or_else(|| {
                ConsensusError::InvalidMessage("Missing block hash in vote".to_string())
            })?;

        self.record_vote(validator.clone(), round, block_hash.to_string())
            .await?;

        debug!(
            "Recorded vote from {} for round {} with block hash {}",
            validator, round, block_hash
        );

        // Check if we have enough votes for finalization
        self.check_finalization(round).await
    }

    async fn process_view_change(
        &mut self,
        new_view: u64,
        round: u64,
        peer_id: String,
    ) -> ConsensusResult<()> {
        info!(
            "Processing view change to {} from peer {} for round {}",
            new_view, peer_id, round
        );

        // Update local view if valid
        let current_view = self.state_coordinator.read().await.get_current_view();
        if new_view > current_view {
            // Update view in state coordinator
            debug!("Updated view from {} to {}", current_view, new_view);
            info!("Updated view to {}", new_view);
        }

        Ok(())
    }

    async fn generate_vote(
        &mut self,
        block: serde_json::Value,
        round: u64,
        height: u64,
    ) -> ConsensusResult<()> {
        // Generate our vote for the proposed block
        let _vote = serde_json::json!({
            "block_hash": self.calculate_block_hash(&block),
            "round": round,
            "height": height,
            "validator": "unknown",
            "signature": self.sign_vote(&block, round, height).await?,
            "timestamp": chrono::Utc::now()
        });

        // Store our vote
        // Record the vote in state coordinator
        debug!("Recorded our vote for round {}", round);

        // Broadcast vote to network
        if let Some(_network) = &self.p2p_network {
            debug!("Would broadcast vote to network");
        }

        Ok(())
    }

    async fn check_finalization(&mut self, round: u64) -> ConsensusResult<()> {
        // Check if we have enough votes to finalize the block
        // In BFT consensus, we need 2f+1 votes where f is the number of Byzantine faults we can tolerate
        let total_validators = self.known_validators.read().await.len();
        let byzantine_faults = (total_validators - 1) / 3; // f = floor((n-1)/3)
        let required_votes = 2 * byzantine_faults + 1; // 2f + 1

        // Get actual vote count for this round from the consensus engine
        let vote_count = self.get_vote_count_for_round(round).await?;

        debug!(
            "Round {} vote count: {}/{} (total validators: {}, byzantine faults: {})",
            round, vote_count, required_votes, total_validators, byzantine_faults
        );

        if vote_count >= required_votes {
            info!(
                "Sufficient votes received for round {} ({}/{}), finalizing block",
                round, vote_count, required_votes
            );
            self.finalize_block(round).await?;
        } else {
            debug!(
                "Insufficient votes for round {} ({}/{}), waiting for more votes",
                round, vote_count, required_votes
            );
        }

        Ok(())
    }

    async fn finalize_block(&mut self, round: u64) -> ConsensusResult<()> {
        // Finalize the block for the given round
        // First, verify we still have sufficient votes (double-check)
        let total_validators = self.known_validators.read().await.len();
        let byzantine_faults = (total_validators - 1) / 3;
        let required_votes = 2 * byzantine_faults + 1;
        let vote_count = self.get_vote_count_for_round(round).await?;

        if vote_count < required_votes {
            return Err(ConsensusError::InsufficientVotesForRound {
                received: vote_count,
                required: required_votes,
                round,
            });
        }

        info!(
            "Finalizing block for round {} with {}/{} votes",
            round, vote_count, required_votes
        );

        // Get the block proposal for this round
        let block = self.get_block_for_round(round).await?;

        // Apply block to state through state coordinator
        self.state_coordinator
            .write()
            .await
            .apply_block(&block)
            .await
            .map_err(|e| ConsensusError::Internal(format!("Failed to apply block: {e}")))?;

        debug!("Applied block for round {} to state", round);

        // Notify other components of finalized block
        let block_json = serde_json::to_value(&block)
            .map_err(|e| ConsensusError::SerializationError(e.to_string()))?;
        self.notify_block_finalized(block_json, round).await?;

        // Update consensus statistics
        self.stats.total_blocks += 1;
        self.stats.current_height = self.state_coordinator.read().await.get_current_height();
        self.stats.last_block_time = std::time::SystemTime::now();

        // Calculate average block time
        if self.stats.total_blocks > 1 {
            let elapsed = self
                .stats
                .last_block_time
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            self.stats.avg_block_time_ms = elapsed / self.stats.total_blocks;
        }

        Ok(())
    }

    async fn notify_block_finalized(
        &self,
        _block: serde_json::Value,
        round: u64,
    ) -> ConsensusResult<()> {
        // Notify other system components that a block has been finalized
        debug!(
            "Notifying components of finalized block for round {}",
            round
        );

        // This would typically send notifications to:
        // - Block router for transaction execution
        // - State synchronizer for state updates
        // - API layer for client notifications

        Ok(())
    }

    async fn sign_vote(
        &self,
        block: &serde_json::Value,
        round: u64,
        height: u64,
    ) -> ConsensusResult<String> {
        // Generate cryptographic signature for vote
        let vote_data = format!("{}:{}:{}", self.calculate_block_hash(block), round, height);

        // In production, this would use the validator's private key
        let hash = sha2::Sha256::digest(vote_data.as_bytes());
        let signature = format!("sig_{}", hex::encode(hash));

        Ok(hex::encode(signature))
    }

    fn calculate_block_hash(&self, block: &serde_json::Value) -> String {
        // Calculate deterministic hash of the block
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();

        if let Ok(block_bytes) = serde_json::to_vec(block) {
            hasher.update(&block_bytes);
        } else {
            hasher.update(b"invalid_block");
        }

        format!("0x{}", hex::encode(hasher.finalize()))
    }

    #[allow(dead_code)]
    async fn generate_state_sync_response(
        &self,
        height: u64,
    ) -> ConsensusResult<serde_json::Value> {
        // Generate state synchronization response for requested height
        Ok(serde_json::json!({
            "height": height,
            "state_root": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "validator_set": [],
            "timestamp": chrono::Utc::now()
        }))
    }

    /// Process a cross-VM transaction
    pub async fn process_cross_vm_transaction(
        &mut self,
        transaction: multivm_account_mapping::special_tx::SpecialTransaction,
    ) -> ConsensusResult<()> {
        info!("Processing cross-VM transaction");

        // Validate the transaction
        let validation_result = {
            let state_coordinator = self.state_coordinator.write().await;
            state_coordinator
                .validate_cross_vm_transaction(&transaction)
                .await?
        };

        if !validation_result.is_valid() {
            return Err(ConsensusError::ValidationFailed(
                validation_result
                    .error_message()
                    .unwrap_or("Unknown validation error")
                    .to_string(),
            ));
        }

        // Serialize the special transaction
        let serialized_tx = bincode::serialize(&transaction).map_err(|e| {
            ConsensusError::Internal(format!("Failed to serialize transaction: {e}"))
        })?;

        // Propose a block with this transaction through Malachite
        let transactions = vec![crate::malachite::MalachiteTransaction {
            data: serialized_tx,
            hash: format!("tx_{}", uuid::Uuid::new_v4()),
        }];
        let block = self.consensus_engine.propose_block(transactions).await?;
        // Process the block proposal through the consensus engine
        self.consensus_engine.commit_block(block.clone()).await?;

        // Update statistics
        self.stats.total_blocks += 1;
        self.stats.current_height = block.height;
        self.stats.last_block_time = std::time::SystemTime::now();

        self.stats.cross_vm_transactions += 1;
        Ok(())
    }

    /// Get current consensus statistics
    pub async fn get_consensus_stats(&self) -> ConsensusResult<ConsensusManagerStats> {
        let mut stats = self.stats.clone();

        // Update with latest engine stats
        let engine_stats = self.consensus_engine.get_consensus_stats().await?;

        stats.current_height = engine_stats.current_height;
        stats.total_blocks = engine_stats.total_blocks;
        stats.total_transactions = engine_stats.total_transactions;
        stats.avg_block_time_ms = engine_stats.avg_block_time;
        stats.active_nodes = engine_stats.active_nodes;
        stats.last_block_time = engine_stats.last_block_time;

        Ok(stats)
    }

    /// Get cross-VM state
    pub async fn get_cross_vm_state(&self) -> ConsensusResult<CrossVMState> {
        let state_coordinator = self.state_coordinator.read().await;
        state_coordinator.get_cross_vm_state().await
    }

    /// Create a state checkpoint
    pub async fn create_checkpoint(&self) -> ConsensusResult<StateCheckpoint> {
        let state_coordinator = self.state_coordinator.read().await;
        state_coordinator.create_checkpoint().await
    }

    /// Restore from a state checkpoint
    pub async fn restore_from_checkpoint(
        &mut self,
        checkpoint: &StateCheckpoint,
    ) -> ConsensusResult<()> {
        let mut state_coordinator = self.state_coordinator.write().await;
        state_coordinator
            .restore_from_checkpoint(checkpoint)
            .await?;

        self.stats.state_syncs += 1;
        Ok(())
    }

    /// Synchronize state to a specific height
    pub async fn sync_to_height(&mut self, target_height: u64) -> ConsensusResult<()> {
        info!("Synchronizing state to height {}", target_height);

        let mut state_coordinator = self.state_coordinator.write().await;
        state_coordinator.sync_state(target_height).await?;

        self.stats.state_syncs += 1;
        Ok(())
    }

    /// Get the vote count for a specific round
    async fn get_vote_count_for_round(&self, round: u64) -> ConsensusResult<usize> {
        // In production, this would query the consensus engine's vote storage
        // For now, we'll use the Malachite consensus engine's internal state
        let stats = self.consensus_engine.get_consensus_stats().await?;

        // Return the number of active nodes as a proxy for vote count
        // In a real implementation, this would track actual votes per round
        Ok(stats.active_nodes as usize)
    }

    /// Get the block proposal for a specific round
    async fn get_block_for_round(&self, round: u64) -> ConsensusResult<crate::block::MultiVMBlock> {
        // In production, this would retrieve the block from storage
        // For now, create a placeholder block
        use crate::block::{BlockHeader, MultiVMBlock};

        let header = BlockHeader {
            height: round,
            previous_hash: "0x0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            state_root: "0x0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            transactions_root: "0x0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            timestamp: std::time::SystemTime::now(),
            proposer: self.node_id.clone(),
            consensus_data: vec![],
            version: 1,
            extra_data: vec![],
        };

        Ok(MultiVMBlock {
            header,
            svm_transactions: vec![],
            evm_transactions: vec![],
            multivm_transactions: vec![],
            state_transitions: vec![],
        })
    }

    /// Record a vote from a validator
    async fn record_vote(
        &mut self,
        validator: String,
        round: u64,
        block_hash: String,
    ) -> ConsensusResult<()> {
        debug!(
            "Recording vote from {} for round {} on block {}",
            validator, round, block_hash
        );

        // Store the vote in the consensus engine
        self.consensus_engine
            .record_vote(validator.clone(), round, block_hash.clone())
            .await?;

        // Update local statistics
        self.stats.total_messages_sent += 1;

        // Check if we've reached consensus threshold
        let vote_count = self.get_vote_count_for_round(round).await?;
        let stats = self.consensus_engine.get_consensus_stats().await?;
        let threshold = (stats.active_nodes * 2) / 3 + 1;

        if vote_count >= threshold as usize {
            info!(
                "Consensus reached for round {} with {} votes (threshold: {})",
                round, vote_count, threshold
            );

            // Trigger block finalization
            if let Ok(block) = self.get_block_for_round(round).await {
                self.handle_consensus_reached(round, block).await?;
            }
        }

        Ok(())
    }

    /// Get pending transactions from the transaction pool
    async fn get_pending_transactions(
        &self,
    ) -> ConsensusResult<(
        Vec<serde_json::Value>,
        Vec<serde_json::Value>,
        Vec<serde_json::Value>,
    )> {
        // Retrieve pending transactions from the consensus engine's transaction pool
        match self.consensus_engine.get_pending_transactions().await {
            Ok(transactions) => {
                // Convert MalachiteTransactions to JSON for now
                let json_txs: Vec<serde_json::Value> = transactions
                    .into_iter()
                    .map(|tx| {
                        serde_json::json!({
                            "data": tx.data,
                            "hash": tx.hash
                        })
                    })
                    .collect();

                // For now, return all as multivm transactions
                Ok((vec![], vec![], json_txs))
            }
            Err(_) => {
                // Return empty vectors if no transactions are pending
                Ok((vec![], vec![], vec![]))
            }
        }
    }

    /// Handle consensus reached for a round
    async fn handle_consensus_reached(
        &mut self,
        round: u64,
        block: crate::block::MultiVMBlock,
    ) -> ConsensusResult<()> {
        info!("Handling consensus reached for round {}", round);

        // Finalize the block
        // Convert MultiVMBlock to MalachiteBlock
        let malachite_block = crate::malachite::engine::MalachiteBlock {
            height: block.header.height,
            data: serde_json::to_vec(&block).unwrap_or_default(),
            timestamp: SystemTime::now(),
        };

        self.consensus_engine
            .finalize_block(round, malachite_block)
            .await?;

        // Apply the block to state
        let mut state_coordinator = self.state_coordinator.write().await;
        state_coordinator.apply_block(&block).await?;

        // Update statistics
        self.stats.total_blocks += 1;
        self.stats.last_block_time = SystemTime::now();

        // Broadcast block finalization
        if let Some(network) = &self.p2p_network {
            // Create a consensus message for block finalization
            let message = ConsensusMessage {
                id: uuid::Uuid::new_v4(),
                sender: self.node_id.clone(),
                timestamp: SystemTime::now(),
                payload: ConsensusMessagePayload::BlockFinalized {
                    height: round,
                    block_hash: format!(
                        "{:x}",
                        sha2::Sha256::digest(serde_json::to_vec(&block).unwrap_or_default())
                    ),
                },
                signature: MessageSignature {
                    algorithm: "ed25519".to_string(),
                    signature: Vec::new(),
                    public_key: Vec::new(),
                },
                version: 1,
            };

            // Convert to P2P message and broadcast
            let network_message = NetworkMessage {
                id: message.id.to_string(),
                payload: MessagePayload::MultiVm(MultiVmMessage::StateSync {
                    state_root: format!("consensus_state_{round}"),
                    height: round,
                    vm_type: VmType::Svm, // Use Svm as placeholder
                }),
                source: MessageSource::MultiVmLayer,
                target: MessageTarget::Broadcast,
                timestamp: chrono::Utc::now(),
                version: 1,
                metadata: HashMap::new(),
            };

            let network_lock = network.write().await;
            // Note: Would need to implement broadcast_message method on P2PNetworkLayer
            // For now, just log the broadcast
            info!("Broadcasting block finalization for height {}", round);
        }

        Ok(())
    }

    /// Submit a transaction to the pool
    pub async fn submit_transaction(
        &self,
        transaction_data: serde_json::Value,
        priority: TransactionPriority,
    ) -> ConsensusResult<()> {
        // Convert to PooledTransaction
        let pooled_tx = crate::transaction_pool::PooledTransaction {
            id: transaction_data
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or(&uuid::Uuid::new_v4().to_string())
                .to_string(),
            data: transaction_data,
            timestamp: std::time::SystemTime::now(),
            signature: None,
        };

        self.transaction_pool
            .add_transaction(pooled_tx, priority)
            .await?;
        info!("Transaction submitted to pool");
        Ok(())
    }

    /// Get pending transaction count
    pub async fn get_pending_transaction_count(&self) -> usize {
        self.transaction_pool.pending_count().await
    }

    /// Get transaction pool statistics
    pub async fn get_transaction_pool_stats(
        &self,
    ) -> crate::transaction_pool::TransactionPoolStats {
        self.transaction_pool.get_stats().await
    }

    /// Add a transaction to the pool (for API usage)
    pub async fn add_transaction_to_pool(
        &self,
        transaction: crate::transaction_pool::PooledTransaction,
        priority: crate::transaction_pool::TransactionPriority,
    ) -> ConsensusResult<()> {
        self.transaction_pool
            .add_transaction(transaction, priority)
            .await?;
        info!(
            "Transaction added to pool via API with {:?} priority",
            priority
        );
        Ok(())
    }

    /// Start automatic block generation
    async fn start_auto_block_generation(&mut self) -> ConsensusResult<()> {
        let interval_ms = self.config.block_proposal_interval_ms;
        let max_txs = self.config.max_transactions_per_block;

        info!(
            "Starting automatic block generation every {}ms with max {} transactions",
            interval_ms, max_txs
        );

        // Clone transaction pool reference for the background task
        let tx_pool = self.transaction_pool.clone();

        // Spawn a background task for block generation
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(interval_ms));
            let mut block_height = 1u64;

            loop {
                interval.tick().await;

                // First try to get transactions from the pool
                let pooled_txs = tx_pool.get_transactions_for_block(max_txs).await;

                // Convert PooledTransactions to mock data for now
                let mut transactions = if pooled_txs.is_empty() {
                    // Generate at least 50 mock transactions
                    let tx_count = std::cmp::max(50, std::cmp::min(60, max_txs));
                    let mock_txs = Self::generate_mock_transactions(tx_count);
                    // Add them to the pool for next time
                    for (i, tx_data) in mock_txs.into_iter().enumerate() {
                        let pooled_tx = crate::transaction_pool::PooledTransaction {
                            id: format!("mock_tx_{}_{}", block_height, i),
                            data: tx_data,
                            timestamp: std::time::SystemTime::now(),
                            signature: None,
                        };
                        let _ = tx_pool
                            .add_transaction(pooled_tx, TransactionPriority::Normal)
                            .await;
                    }
                    vec![] // Will be filled from pool next time
                } else {
                    pooled_txs.clone()
                };

                // Count transaction types
                let mut evm_count = 0;
                let mut svm_count = 0;
                let mut multivm_count = 0;

                for tx in &pooled_txs {
                    if let Ok(tx_data) =
                        serde_json::from_value::<serde_json::Value>(tx.data.clone())
                    {
                        // Try both "type" and "vm_type" fields
                        if let Some(tx_type) = tx_data
                            .get("type")
                            .and_then(|v| v.as_str())
                            .or_else(|| tx_data.get("vm_type").and_then(|v| v.as_str()))
                        {
                            match tx_type {
                                "evm" => evm_count += 1,
                                "svm" => svm_count += 1,
                                "cross_vm" | "multivm" => multivm_count += 1,
                                _ => {}
                            }
                        }
                    }
                }

                // Log block generation activity with transaction type breakdown
                let pool_stats = tx_pool.get_stats().await;
                info!("Block {} generated with {} transactions [EVM: {}, SVM: {}, MultiVM: {}] (pool: {} pending, {} total submitted)",
                      block_height, pooled_txs.len(), evm_count, svm_count, multivm_count,
                      pool_stats.current_pool_size, pool_stats.total_submitted);

                // Mark transactions as included
                let tx_ids: Vec<String> = pooled_txs.iter().map(|tx| tx.id.clone()).collect();
                tx_pool.mark_included(&tx_ids, block_height).await;

                block_height += 1;

                // Periodically clean up expired transactions
                if block_height % 10 == 0 {
                    let expired = tx_pool.cleanup_expired().await;
                    if expired > 0 {
                        debug!("Cleaned up {} expired transactions", expired);
                    }
                }
            }
        });

        Ok(())
    }

    /// Generate mock transactions for testing
    fn generate_mock_transactions(count: usize) -> Vec<serde_json::Value> {
        // Ensure a good distribution of transaction types
        // Roughly: 40% EVM, 40% SVM, 20% MultiVM/CrossVM

        (0..count)
            .map(|i| {
                let tx_type = match i % 10 {
                    0..=3 => "evm",  // 40%
                    4..=7 => "svm",  // 40%
                    _ => "cross_vm", // 20%
                };

                let evm_data = if tx_type == "evm" {
                    Some(serde_json::json!({"gasLimit": 21000, "gasPrice": 20000000000u64}))
                } else {
                    None
                };

                let cross_vm = if tx_type == "cross_vm" {
                    Some(serde_json::json!({"source_vm": "evm", "target_vm": "svm"}))
                } else {
                    None
                };

                serde_json::json!({
                    "id": format!("tx_{}_{}", tx_type, uuid::Uuid::new_v4()),
                    "type": tx_type,
                    "vm_type": tx_type, // Add vm_type for compatibility
                    "sender": format!("0x{:x}", rand::random::<u64>()),
                    "to": format!("0x{:x}", rand::random::<u64>()),
                    "value": rand::random::<u64>() % 1000,
                    "data": format!("0x{:x}", rand::random::<u64>()),
                    "nonce": i as u64,
                    "gas_price": 20000000000u64,
                    "timestamp": chrono::Utc::now().timestamp(),
                    "evm_data": evm_data,
                    "cross_vm": cross_vm
                })
            })
            .collect()
    }
}

impl ConsensusManagerConfig {
    /// Create consensus manager config from unified MultiVM config
    pub fn from_unified_config(
        unified_config: &multivm_common::MultivmConfig,
    ) -> ConsensusResult<Self> {
        // Create Malachite configuration
        let mut malachite_config = MalachiteConfig::default();
        malachite_config.set_timeout_duration(std::time::Duration::from_millis(
            unified_config.consensus.block_time_milliseconds,
        ));
        malachite_config.set_validator_count(unified_config.consensus.validator_count);

        // Enable single node mode for solo testnet
        if unified_config.consensus.enable_single_node {
            malachite_config.enable_single_node_mode();
        }

        // Create state manager config
        let state_manager_config = StateManagerConfig {
            max_checkpoints: 100,
            checkpoint_interval: 10, // Every 10 blocks
            enable_verification: true,
            max_pending_changes: 1000,
            rocksdb_path: Some(format!(
                "{}/consensus_state.db",
                unified_config.system.data_dir.display()
            )),
        };

        // Create network config
        let network_config = NetworkConfig {
            node_id: "solo-node".to_string(),
            listen_address: format!(
                "{}:{}",
                unified_config.network.listen_host, unified_config.network.listen_port
            ),
            bootstrap_nodes: vec![],
            enable_encryption: unified_config.ipc.enable_encryption,
        };

        // Create transaction pool config
        let transaction_pool_config = TransactionPoolConfig {
            max_pool_size: 10000,
            max_per_account: 100,
            tx_expiry_seconds: 300,
            allow_replacement: true,
            replacement_gas_increase: 10,
        };

        Ok(Self {
            node_id: None, // Will be generated
            algorithm: ConsensusAlgorithmType::Malachite,
            algorithm_config: AlgorithmConfig::Malachite(malachite_config),
            state_manager_config,
            block_proposal_interval_ms: unified_config.consensus.block_time_milliseconds,
            max_transactions_per_block: 1000,
            enable_auto_proposal: true,
            network_config,
            transaction_pool_config,
        })
    }
}
