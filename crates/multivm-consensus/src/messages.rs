//! Consensus message types for the MultiVM consensus layer

use crate::block::{BlockHash, MultiVMBlock};
use crate::traits::NodeId;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

/// Top-level consensus message envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusMessage {
    /// Unique message identifier
    pub id: MessageId,
    /// Message sender
    pub sender: NodeId,
    /// Message timestamp
    pub timestamp: SystemTime,
    /// Message type and payload
    pub payload: ConsensusMessagePayload,
    /// Message signature for verification
    pub signature: MessageSignature,
    /// Protocol version
    pub version: u32,
}

/// Message type enumeration for routing and priority determination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    /// Block proposal message
    Proposal(ProposalMessage),
    /// Vote message
    Vote(VoteMessage),
    /// View change message  
    ViewChange(ViewChangeMessage),
    /// Heartbeat message
    Heartbeat(HeartbeatMessage),
    /// State sync message
    StateSync(StateSyncMessage),
    /// Timeout message
    Timeout(TimeoutMessage),
    /// Query message
    Query(QueryMessage),
    /// Response message
    Response(ResponseMessage),
}

/// Different types of consensus messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusMessagePayload {
    /// Block proposal message
    Proposal(ProposalMessage),
    /// Vote message for a proposal
    Vote(VoteMessage),
    /// View change message
    ViewChange(ViewChangeMessage),
    /// Heartbeat message
    Heartbeat(HeartbeatMessage),
    /// State synchronization message
    StateSync(StateSyncMessage),
    /// Timeout message
    Timeout(TimeoutMessage),
    /// Query message
    Query(QueryMessage),
    /// Response to a query
    Response(ResponseMessage),
}

/// Block proposal message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalMessage {
    /// Block height
    pub height: u64,
    /// Consensus round
    pub round: u32,
    /// Proposed block
    pub block: MultiVMBlock,
    /// Proposer identifier
    pub proposer: NodeId,
    /// Justification for this proposal
    pub justification: Option<ProposalJustification>,
    /// Additional proposal metadata
    pub metadata: serde_json::Value,
}

/// Vote message for a proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteMessage {
    /// Block height being voted on
    pub height: u64,
    /// Consensus round
    pub round: u32,
    /// Hash of the block being voted on
    pub block_hash: BlockHash,
    /// Type of vote
    pub vote_type: VoteType,
    /// Voting node
    pub voter: NodeId,
    /// Vote justification
    pub justification: Option<VoteJustification>,
    /// Additional vote metadata
    pub metadata: serde_json::Value,
}

/// Types of votes in the consensus process
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum VoteType {
    /// Pre-vote (first phase of voting)
    PreVote,
    /// Pre-commit (second phase of voting)
    PreCommit,
    /// Final commit vote
    Commit,
    /// Vote to reject/abort
    Reject,
}

/// View change message for leader rotation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewChangeMessage {
    /// Current block height
    pub height: u64,
    /// Old view number
    pub old_view: u32,
    /// New view number
    pub new_view: u32,
    /// Reason for view change
    pub reason: ViewChangeReason,
    /// Evidence supporting the view change
    pub justification: ViewChangeJustification,
    /// Node requesting the view change
    pub requesting_node: NodeId,
}

/// Reasons for initiating a view change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ViewChangeReason {
    /// Leader timeout
    LeaderTimeout,
    /// Invalid proposal from leader
    InvalidProposal,
    /// Network partition detected
    NetworkPartition,
    /// Explicit leader rotation
    ScheduledRotation,
    /// Failure detection
    NodeFailure(NodeId),
}

/// Heartbeat message for liveness
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatMessage {
    /// Current block height
    pub height: u64,
    /// Current view/round
    pub view: u32,
    /// Node status information
    pub status: NodeStatus,
    /// Timestamp of the heartbeat
    pub timestamp: SystemTime,
    /// Additional node metrics
    pub metrics: NodeMetrics,
}

/// State synchronization messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateSyncMessage {
    /// Request for state at a specific height
    StateRequest {
        height: u64,
        chunk_index: u32,
        vm_type: Option<VmType>,
    },
    /// Response with state data
    StateResponse {
        height: u64,
        chunk_index: u32,
        chunk_data: Vec<u8>,
        vm_type: VmType,
        proof: StateProof,
    },
    /// Notification that sync is complete
    SyncComplete { height: u64, state_root: String },
    /// Request for available state snapshots
    SnapshotRequest,
    /// Response with available snapshots
    SnapshotResponse { snapshots: Vec<SnapshotInfo> },
}

/// Timeout message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutMessage {
    /// Block height where timeout occurred
    pub height: u64,
    /// Round where timeout occurred
    pub round: u32,
    /// Type of timeout
    pub timeout_type: TimeoutType,
    /// Node that detected the timeout
    pub detector: NodeId,
    /// Duration of the timeout
    pub duration_ms: u64,
}

/// Types of timeouts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimeoutType {
    /// Proposal timeout
    Proposal,
    /// Vote timeout
    Vote,
    /// Commit timeout
    Commit,
    /// View change timeout
    ViewChange,
}

/// Query message for requesting information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryMessage {
    /// Type of query
    pub query_type: QueryType,
    /// Query parameters
    pub parameters: serde_json::Value,
    /// Request ID for correlation
    pub request_id: Uuid,
}

/// Types of queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryType {
    /// Get current blockchain height
    GetHeight,
    /// Get block by height
    GetBlock(u64),
    /// Get consensus statistics
    GetStats,
    /// Get node status
    GetNodeStatus,
    /// Get peer information
    GetPeers,
    /// Custom query
    Custom(String),
}

/// Response message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMessage {
    /// Original request ID
    pub request_id: Uuid,
    /// Response payload
    pub payload: ResponsePayload,
    /// Response status
    pub status: ResponseStatus,
    /// Optional error message
    pub error: Option<String>,
}

/// Response payload types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponsePayload {
    /// Height response
    Height(u64),
    /// Block response
    Block(Option<MultiVMBlock>),
    /// Statistics response
    Stats(ConsensusStatistics),
    /// Node status response
    NodeStatus(NodeStatus),
    /// Peers list response
    Peers(Vec<PeerInfo>),
    /// Generic JSON response
    Json(serde_json::Value),
}

/// Response status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseStatus {
    Success,
    Error,
    NotFound,
    Timeout,
}

/// Supporting data structures
pub type MessageId = Uuid;

/// Type alias for Vote to export VoteMessage as Vote for convenience
pub type Vote = VoteMessage;

/// Message signature for verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSignature {
    /// Signature algorithm
    pub algorithm: String,
    /// Signature data
    pub signature: Vec<u8>,
    /// Public key for verification
    pub public_key: Vec<u8>,
}

/// Proposal justification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalJustification {
    /// Previous blocks this proposal builds on
    pub parent_hashes: Vec<BlockHash>,
    /// Quorum certificates or evidence
    pub certificates: Vec<QuorumCertificate>,
    /// Additional justification data
    pub extra_data: Vec<u8>,
}

/// Vote justification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteJustification {
    /// Reason for the vote
    pub reason: String,
    /// Supporting evidence
    pub evidence: Vec<u8>,
    /// Previous votes this builds on
    pub previous_votes: Vec<VoteSummary>,
}

/// View change justification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewChangeJustification {
    /// Evidence of leader failure or timeout
    pub evidence: Vec<u8>,
    /// Supporting votes from other nodes
    pub supporting_votes: Vec<ViewChangeVote>,
    /// Timeout information
    pub timeout_info: Option<TimeoutInfo>,
}

/// VM type enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VmType {
    SVM,
    EVM,
    MultiVM,
}

/// State proof for verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateProof {
    /// Merkle proof data
    pub proof_data: Vec<u8>,
    /// Root hash
    pub root_hash: String,
    /// Proof type
    pub proof_type: String,
}

/// Node status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStatus {
    /// Node identifier
    pub node_id: NodeId,
    /// Current blockchain height
    pub height: u64,
    /// Current view/round
    pub view: u32,
    /// Role in consensus
    pub role: NodeRole,
    /// Health status
    pub health: HealthStatus,
    /// Uptime in seconds
    pub uptime: u64,
}

/// Node role in consensus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeRole {
    Leader,
    Follower,
    Candidate,
    Observer,
}

/// Health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

/// Node metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetrics {
    /// CPU usage percentage
    pub cpu_usage: f64,
    /// Memory usage in bytes
    pub memory_usage: u64,
    /// Disk usage percentage
    pub disk_usage: f64,
    /// Network bytes sent
    pub network_sent: u64,
    /// Network bytes received
    pub network_received: u64,
    /// Transaction pool size
    pub tx_pool_size: u32,
    /// Peer connection count
    pub peer_count: u32,
}

/// Consensus statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusStatistics {
    /// Current height
    pub height: u64,
    /// Total blocks processed
    pub total_blocks: u64,
    /// Average block time
    pub avg_block_time_ms: u64,
    /// Total transactions processed
    pub total_transactions: u64,
    /// Current TPS
    pub current_tps: f64,
    /// Consensus algorithm
    pub algorithm: String,
    /// Active validators
    pub active_validators: u32,
}

/// Quorum certificate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuorumCertificate {
    /// Block hash this certificate is for
    pub block_hash: BlockHash,
    /// Height and round
    pub height: u64,
    pub round: u32,
    /// Aggregated signatures
    pub signatures: Vec<MessageSignature>,
    /// Voting power threshold met
    pub voting_power: u64,
}

/// Vote summary for justification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteSummary {
    pub height: u64,
    pub round: u32,
    pub vote_type: VoteType,
    pub voter: NodeId,
    pub block_hash: BlockHash,
}

/// View change vote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewChangeVote {
    pub voter: NodeId,
    pub old_view: u32,
    pub new_view: u32,
    pub signature: MessageSignature,
}

/// Timeout information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutInfo {
    pub timeout_type: TimeoutType,
    pub duration_ms: u64,
    pub detected_at: SystemTime,
}

/// Snapshot information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotInfo {
    pub height: u64,
    pub state_root: String,
    pub size_bytes: u64,
    pub created_at: SystemTime,
    pub vm_types: Vec<VmType>,
}

/// Peer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub node_id: NodeId,
    pub address: String,
    pub status: NodeStatus,
    pub last_seen: SystemTime,
    pub protocol_version: u32,
}

impl ConsensusMessage {
    /// Create a new consensus message
    pub fn new(sender: NodeId, payload: ConsensusMessagePayload) -> Self {
        Self {
            id: Uuid::new_v4(),
            sender,
            timestamp: SystemTime::now(),
            payload,
            signature: MessageSignature::default(),
            version: 1,
        }
    }

    /// Get the message type as a string
    pub fn message_type(&self) -> &'static str {
        match &self.payload {
            ConsensusMessagePayload::Proposal(_) => "proposal",
            ConsensusMessagePayload::Vote(_) => "vote",
            ConsensusMessagePayload::ViewChange(_) => "view_change",
            ConsensusMessagePayload::Heartbeat(_) => "heartbeat",
            ConsensusMessagePayload::StateSync(_) => "state_sync",
            ConsensusMessagePayload::Timeout(_) => "timeout",
            ConsensusMessagePayload::Query(_) => "query",
            ConsensusMessagePayload::Response(_) => "response",
        }
    }

    /// Get the message type as MessageType enum
    pub fn msg_type(&self) -> MessageType {
        match &self.payload {
            ConsensusMessagePayload::Proposal(msg) => MessageType::Proposal(msg.clone()),
            ConsensusMessagePayload::Vote(msg) => MessageType::Vote(msg.clone()),
            ConsensusMessagePayload::ViewChange(msg) => MessageType::ViewChange(msg.clone()),
            ConsensusMessagePayload::Heartbeat(msg) => MessageType::Heartbeat(msg.clone()),
            ConsensusMessagePayload::StateSync(msg) => MessageType::StateSync(msg.clone()),
            ConsensusMessagePayload::Timeout(msg) => MessageType::Timeout(msg.clone()),
            ConsensusMessagePayload::Query(msg) => MessageType::Query(msg.clone()),
            ConsensusMessagePayload::Response(msg) => MessageType::Response(msg.clone()),
        }
    }

    /// Check if this message requires immediate processing
    pub fn is_urgent(&self) -> bool {
        matches!(
            &self.payload,
            ConsensusMessagePayload::ViewChange(_) | ConsensusMessagePayload::Timeout(_)
        )
    }
}

impl Default for MessageSignature {
    fn default() -> Self {
        Self {
            algorithm: "ed25519".to_string(),
            signature: Vec::new(),
            public_key: Vec::new(),
        }
    }
}

impl Default for NodeMetrics {
    fn default() -> Self {
        Self {
            cpu_usage: 0.0,
            memory_usage: 0,
            disk_usage: 0.0,
            network_sent: 0,
            network_received: 0,
            tx_pool_size: 0,
            peer_count: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consensus_message_creation() {
        let payload = ConsensusMessagePayload::Heartbeat(HeartbeatMessage {
            height: 100,
            view: 1,
            status: NodeStatus {
                node_id: "node1".to_string(),
                height: 100,
                view: 1,
                role: NodeRole::Leader,
                health: HealthStatus::Healthy,
                uptime: 3600,
            },
            timestamp: SystemTime::now(),
            metrics: NodeMetrics::default(),
        });

        let message = ConsensusMessage::new("node1".to_string(), payload);

        assert_eq!(message.sender, "node1");
        assert_eq!(message.message_type(), "heartbeat");
        assert!(!message.is_urgent());
        assert_eq!(message.version, 1);
    }

    #[test]
    fn test_vote_message() {
        let vote = VoteMessage {
            height: 100,
            round: 1,
            block_hash: "block_hash".to_string(),
            vote_type: VoteType::PreVote,
            voter: "voter1".to_string(),
            justification: None,
            metadata: serde_json::Value::Null,
        };

        assert_eq!(vote.vote_type, VoteType::PreVote);
        assert_eq!(vote.height, 100);
    }

    #[test]
    fn test_view_change_message() {
        let view_change = ViewChangeMessage {
            height: 100,
            old_view: 1,
            new_view: 2,
            reason: ViewChangeReason::LeaderTimeout,
            justification: ViewChangeJustification {
                evidence: vec![1, 2, 3],
                supporting_votes: vec![],
                timeout_info: Some(TimeoutInfo {
                    timeout_type: TimeoutType::Proposal,
                    duration_ms: 5000,
                    detected_at: SystemTime::now(),
                }),
            },
            requesting_node: "node1".to_string(),
        };

        assert_eq!(view_change.old_view, 1);
        assert_eq!(view_change.new_view, 2);
        assert!(matches!(
            view_change.reason,
            ViewChangeReason::LeaderTimeout
        ));
    }

    #[test]
    fn test_message_urgency() {
        let heartbeat = ConsensusMessage::new(
            "node1".to_string(),
            ConsensusMessagePayload::Heartbeat(HeartbeatMessage {
                height: 100,
                view: 1,
                status: NodeStatus {
                    node_id: "node1".to_string(),
                    height: 100,
                    view: 1,
                    role: NodeRole::Leader,
                    health: HealthStatus::Healthy,
                    uptime: 3600,
                },
                timestamp: SystemTime::now(),
                metrics: NodeMetrics::default(),
            }),
        );

        let timeout = ConsensusMessage::new(
            "node1".to_string(),
            ConsensusMessagePayload::Timeout(TimeoutMessage {
                height: 100,
                round: 1,
                timeout_type: TimeoutType::Proposal,
                detector: "node1".to_string(),
                duration_ms: 5000,
            }),
        );

        assert!(!heartbeat.is_urgent());
        assert!(timeout.is_urgent());
    }

    #[test]
    fn test_query_response_flow() {
        let query = QueryMessage {
            query_type: QueryType::GetHeight,
            parameters: serde_json::Value::Null,
            request_id: Uuid::new_v4(),
        };

        let response = ResponseMessage {
            request_id: query.request_id,
            payload: ResponsePayload::Height(100),
            status: ResponseStatus::Success,
            error: None,
        };

        assert_eq!(query.request_id, response.request_id);
        assert!(matches!(response.status, ResponseStatus::Success));
    }

    #[test]
    fn test_state_sync_messages() {
        let request = StateSyncMessage::StateRequest {
            height: 100,
            chunk_index: 0,
            vm_type: Some(VmType::SVM),
        };

        let response = StateSyncMessage::StateResponse {
            height: 100,
            chunk_index: 0,
            chunk_data: vec![1, 2, 3, 4],
            vm_type: VmType::SVM,
            proof: StateProof {
                proof_data: vec![],
                root_hash: "root".to_string(),
                proof_type: "merkle".to_string(),
            },
        };

        assert!(matches!(request, StateSyncMessage::StateRequest { .. }));
        assert!(matches!(response, StateSyncMessage::StateResponse { .. }));
    }
}
