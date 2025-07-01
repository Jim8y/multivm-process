//! Message types for P2P networking

use multivm_account_mapping::{address::AccountAddress, special_tx::SpecialTransaction};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Network message envelope that wraps all P2P communications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMessage {
    /// Unique message identifier
    pub id: String,
    /// Message type and payload
    pub payload: MessagePayload,
    /// Source of the message
    pub source: MessageSource,
    /// Target(s) for the message
    pub target: MessageTarget,
    /// Timestamp when message was created
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Protocol version
    pub version: u32,
    /// Optional metadata
    pub metadata: HashMap<String, String>,
}

/// Different types of message payloads
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagePayload {
    /// Solana VM specific messages
    Svm(SvmMessage),
    /// Ethereum VM specific messages
    Evm(EvmMessage),
    /// MultiVM layer specific messages
    MultiVm(MultiVmMessage),
    /// Network control messages
    Control(ControlMessage),
    /// Discovery and peer management
    Discovery(DiscoveryMessage),
    /// Custom/extensible message payload for consensus and other specialized messages
    Custom(serde_json::Value),
}

/// Source information for a message
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageSource {
    /// Message from SVM execution layer
    SvmExecution,
    /// Message from EVM execution layer
    EvmExecution,
    /// Message from MultiVM coordination layer
    MultiVmLayer,
    /// Message from P2P network layer
    NetworkLayer,
    /// Message from external peer
    Peer(String),
}

/// Target specification for message routing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageTarget {
    /// Broadcast to all connected peers
    Broadcast,
    /// Send to specific peer
    Peer(String),
    /// Send to peers supporting specific protocol
    Protocol(String),
    /// Send to local execution layer
    Local(VmType),
    /// Send to MultiVM coordination layer
    MultiVmLayer,
}

/// Virtual Machine types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum VmType {
    /// Solana Virtual Machine
    Svm,
    /// Ethereum Virtual Machine
    Evm,
}

impl std::fmt::Display for VmType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VmType::Svm => write!(f, "SVM"),
            VmType::Evm => write!(f, "EVM"),
        }
    }
}

/// Message type enumeration for routing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MessageType {
    /// SVM messages
    Svm,
    /// EVM messages
    Evm,
    /// MultiVM messages
    MultiVm,
    /// Control messages
    Control,
    /// Discovery messages
    Discovery,
}

/// Solana VM specific messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SvmMessage {
    /// Transaction to be processed
    Transaction {
        /// Serialized Solana transaction
        transaction_data: Box<Vec<u8>>,
        /// Transaction signature
        signature: String,
    },
    /// Block data
    Block {
        /// Serialized block data
        block_data: Box<Vec<u8>>,
        /// Block hash
        block_hash: String,
        /// Block height
        height: u64,
    },
    /// Gossip message
    Gossip {
        /// Gossip data
        data: Box<Vec<u8>>,
        /// Gossip type
        gossip_type: String,
    },
}

/// Ethereum VM specific messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvmMessage {
    /// Transaction to be processed
    Transaction {
        /// RLP-encoded transaction
        transaction_data: Box<Vec<u8>>,
        /// Transaction hash
        tx_hash: String,
    },
    /// Block data
    Block {
        /// RLP-encoded block
        block_data: Box<Vec<u8>>,
        /// Block hash
        block_hash: String,
        /// Block number
        block_number: u64,
    },
    /// Engine API message
    Engine {
        /// Engine API method
        method: String,
        /// Parameters
        params: Box<serde_json::Value>,
    },
}

/// MultiVM layer specific messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MultiVmMessage {
    /// Special transaction for cross-VM operations
    SpecialTransaction {
        /// The special transaction
        transaction: SpecialTransaction,
        /// Execution context
        context: ExecutionContext,
    },
    /// Account binding notification
    AccountBinding {
        /// Source account
        source: AccountAddress,
        /// Target account
        target: AccountAddress,
        /// Binding proof
        proof_hash: String,
    },
    /// Cross-VM state synchronization
    StateSync {
        /// State root hash
        state_root: String,
        /// VM type
        vm_type: VmType,
        /// Block height/number
        height: u64,
    },
    /// Consensus message
    Consensus {
        /// Consensus data
        consensus_data: Box<Vec<u8>>,
        /// Round number
        round: u64,
        /// View number
        view: u64,
    },
}

/// Execution context for MultiVM messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// Block height when operation should be executed
    pub target_height: u64,
    /// Required confirmations
    pub confirmations_required: u32,
    /// Timeout for operation
    pub timeout: std::time::Duration,
    /// Priority level
    pub priority: ExecutionPriority,
}

/// Priority levels for execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExecutionPriority {
    /// Low priority, best effort
    Low,
    /// Normal priority
    Normal,
    /// High priority, expedited processing
    High,
    /// Critical, immediate processing required
    Critical,
}

/// Message priority levels (alias for ExecutionPriority for consensus compatibility)
pub type Priority = ExecutionPriority;

/// Network control messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlMessage {
    /// Heartbeat/keepalive
    Heartbeat {
        /// Node status
        status: NodeStatus,
        /// Uptime
        uptime: std::time::Duration,
    },
    /// Request for network status
    StatusRequest,
    /// Network status response
    StatusResponse {
        /// Network statistics
        stats: NetworkStats,
        /// Connected peers
        peers: Vec<PeerInfo>,
    },
    /// Protocol version negotiation
    VersionNegotiation {
        /// Supported protocol versions
        supported_versions: Vec<u32>,
        /// Preferred version
        preferred_version: u32,
    },
    /// Shutdown notification
    Shutdown {
        /// Reason for shutdown
        reason: String,
        /// Grace period before disconnection
        grace_period: std::time::Duration,
    },
}

/// Node operational status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeStatus {
    /// Node is starting up
    Starting,
    /// Node is fully operational
    Active,
    /// Node is synchronizing with network
    Syncing,
    /// Node is shutting down
    Shutting,
    /// Node encountered an error
    Error(String),
}

/// Discovery and peer management messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiscoveryMessage {
    /// Announce presence to network
    Announce {
        /// Node capabilities
        capabilities: NodeCapabilities,
        /// Listen addresses
        addresses: Vec<String>,
    },
    /// Request for peer information
    PeerRequest {
        /// Requested peer attributes
        criteria: PeerCriteria,
    },
    /// Response with peer information
    PeerResponse {
        /// List of matching peers
        peers: Vec<PeerAdvertisement>,
    },
    /// Bootstrap request
    Bootstrap {
        /// Bootstrap nodes
        bootstrap_nodes: Vec<String>,
    },
}

/// Node capabilities and features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCapabilities {
    /// Supported VM types
    pub supported_vms: Vec<VmType>,
    /// Protocol versions
    pub protocol_versions: Vec<u32>,
    /// Feature flags
    pub features: Vec<String>,
    /// Resource limits
    pub limits: ResourceLimits,
}

/// Resource limits for the node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum concurrent connections
    pub max_connections: usize,
    /// Maximum message size (bytes)
    pub max_message_size: usize,
    /// Rate limit (messages per second)
    pub rate_limit: f64,
}

/// Criteria for peer selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerCriteria {
    /// Required VM support
    pub vm_types: Option<Vec<VmType>>,
    /// Minimum protocol version
    pub min_protocol_version: Option<u32>,
    /// Required features
    pub required_features: Vec<String>,
    /// Geographic constraints
    pub region: Option<String>,
}

/// Peer advertisement information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerAdvertisement {
    /// Peer identifier
    pub peer_id: String,
    /// Multiaddresses
    pub addresses: Vec<String>,
    /// Capabilities
    pub capabilities: NodeCapabilities,
    /// Last seen timestamp
    pub last_seen: chrono::DateTime<chrono::Utc>,
    /// Reputation score
    pub reputation: f64,
}

/// Network statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkStats {
    pub connected_peers: usize,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub messages_dropped: u64,
    pub rate_limit_violations: u64,
    pub auth_failures: u64,
    pub firewall_blocks: u64,
}

/// Peer status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PeerStatus {
    /// Peer is connected and active
    Connected,
    /// Peer is disconnected
    Disconnected,
    /// Peer is connecting
    Connecting,
    /// Peer is banned or blocked
    Banned,
    /// Peer status is unknown
    Unknown,
}

/// Peer information for network operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Peer ID as string
    pub peer_id: String,
    /// Multi-addresses the peer can be reached at
    pub addresses: Vec<libp2p::Multiaddr>,
    /// Supported protocols
    pub protocols: Vec<String>,
    /// Whether peer supports MultiVM operations
    pub supports_multivm: bool,
    /// Last seen timestamp
    pub last_seen: chrono::DateTime<chrono::Utc>,
    /// Current peer status
    pub status: PeerStatus,
}

impl NetworkMessage {
    /// Create a new network message
    pub fn new(payload: MessagePayload, source: MessageSource, target: MessageTarget) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            payload,
            source,
            target,
            timestamp: chrono::Utc::now(),
            version: 1, // Current protocol version
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the message
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Get message size estimate in bytes
    pub fn estimated_size(&self) -> usize {
        // Rough estimate based on serialized size
        bincode::serialized_size(self).unwrap_or(0) as usize
    }

    /// Check if message is for broadcast
    pub fn is_broadcast(&self) -> bool {
        matches!(self.target, MessageTarget::Broadcast)
    }

    /// Check if message is for specific peer
    pub fn is_peer_message(&self) -> bool {
        matches!(self.target, MessageTarget::Peer(_))
    }

    /// Get the target peer ID if applicable
    pub fn target_peer(&self) -> Option<&str> {
        match &self.target {
            MessageTarget::Peer(peer_id) => Some(peer_id),
            _ => None,
        }
    }

    /// Check if message should be processed locally
    pub fn is_local(&self) -> bool {
        matches!(
            self.target,
            MessageTarget::Local(_) | MessageTarget::MultiVmLayer
        )
    }

    /// Infer the message type from the payload
    pub fn infer_type(&self) -> MessageType {
        match &self.payload {
            MessagePayload::Svm(_) => MessageType::Svm,
            MessagePayload::Evm(_) => MessageType::Evm,
            MessagePayload::MultiVm(_) => MessageType::MultiVm,
            MessagePayload::Control(_) => MessageType::Control,
            MessagePayload::Discovery(_) => MessageType::Discovery,
            MessagePayload::Custom(_) => MessageType::Control, // Treat custom messages as control type
        }
    }
}

impl Default for ExecutionPriority {
    fn default() -> Self {
        Self::Normal
    }
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_connections: 100,
            max_message_size: 1024 * 1024, // 1MB
            rate_limit: 100.0,             // 100 messages per second
        }
    }
}

impl Default for NodeCapabilities {
    fn default() -> Self {
        Self {
            supported_vms: vec![VmType::Svm, VmType::Evm],
            protocol_versions: vec![1],
            features: vec![],
            limits: ResourceLimits::default(),
        }
    }
}

impl NetworkMessage {
    /// Check if message is for VM-specific execution
    pub fn is_vm_specific(&self) -> bool {
        matches!(self.target, MessageTarget::Local(_))
    }

    /// Get the target VM if applicable
    pub fn target_vm(&self) -> Option<VmType> {
        match &self.target {
            MessageTarget::Local(vm_type) => Some(*vm_type),
            _ => None,
        }
    }
}
