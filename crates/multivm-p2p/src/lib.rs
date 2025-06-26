//! # MultiVM P2P Networking Layer
//!
//! This module implements the P2P networking layer for the MultiVM architecture,
//! providing unified network interface and communication across different VMs.
//!
//! ## Core Features
//!
//! - **Unified Network Interface**: Single network layer for SVM, EVM, and MultiVM messages
//! - **Protocol Translation**: Automatic conversion between VM-specific protocols
//! - **Message Routing**: Intelligent routing and broadcasting of cross-VM messages
//! - **Node Discovery**: Automatic discovery and connection to other MultiVM nodes
//! - **Network Isolation**: Native P2P disabled for Solana and Reth nodes in MultiVM architecture
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                   MultiVM P2P Layer                         │
//! ├─────────────────┬─────────────────┬─────────────────────────┤
//! │   SVM Messages  │   EVM Messages  │   MultiVM Messages      │
//! │                 │                 │   (Account Binding,     │
//! │                 │                 │    Cross-VM Transfers)  │
//! └─────────────────┴─────────────────┴─────────────────────────┘
//! ```

#![allow(dead_code, unused_variables, unused_imports)]

pub mod circuit_breaker;
pub mod config;
pub mod connection_manager;
// pub mod discovery;  // Temporarily disabled - NetworkBehaviour issues
pub mod encryption;
pub mod error;
// pub mod gossip;  // Temporarily disabled - NetworkBehaviour issues
pub mod load_balancer;
pub mod messages;
// pub mod network;  // Temporarily disabled - NetworkBehaviour issues
pub mod protocol;
pub mod rate_limiter;
pub mod routing;
// pub mod secure_network;  // Temporarily disabled - NetworkBehaviour issues
pub mod security;
// pub mod transport;  // Temporarily disabled - NetworkBehaviour issues

#[cfg(feature = "metrics")]
pub mod metrics;

// Test modules

/// Information about a connected peer
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PeerInfo {
    /// Peer ID as string
    pub peer_id: String,
    /// Multi-addresses the peer can be reached at
    pub addresses: Vec<libp2p::Multiaddr>,
    /// Protocol versions supported by the peer
    pub protocols: Vec<String>,
    /// Whether this peer supports MultiVM protocol
    pub supports_multivm: bool,
    /// Last seen timestamp
    pub last_seen: chrono::DateTime<chrono::Utc>,
    /// Connection status
    pub status: PeerStatus,
}

/// Status of a peer connection
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PeerStatus {
    /// Currently connected and active
    Connected,
    /// Attempting to connect
    Connecting,
    /// Recently disconnected
    Disconnected,
    /// Connection failed
    Failed,
}

// Re-export NetworkStats from common module
pub use multivm_common::traits::monitoring::NetworkStats;

// Re-export message types for external use
pub use messages::{
    ControlMessage, DiscoveryMessage, MessagePayload, MessageSource, MessageTarget,
    MultiVmMessage, NetworkMessage, NodeStatus, VmType
};

// Re-export network types - temporarily disabled
// pub use network::{P2PNetwork, NetworkConfig, NetworkHealthReport, NetworkHealthStatus};

// Define the P2P network layer trait for consensus compatibility
#[async_trait::async_trait]
pub trait P2PNetworkLayer: Send + Sync {
    async fn subscribe_to_topic(&mut self, topic: &str) -> multivm_common::MultivmResult<()>;
    async fn broadcast_message(&mut self, message: messages::NetworkMessage, topic: Option<String>) -> multivm_common::MultivmResult<()>;
    async fn send_message(&mut self, peer_id: String, message: messages::NetworkMessage) -> multivm_common::MultivmResult<()>;
}

// Implement the trait for P2PNetwork - temporarily disabled
// #[async_trait::async_trait]
// impl P2PNetworkLayer for P2PNetwork {
//     async fn subscribe_to_topic(&mut self, topic: &str) -> multivm_common::MultivmResult<()> {
//         self.subscribe_topic(topic).await
//     }
//
//     async fn broadcast_message(&mut self, message: messages::NetworkMessage, topic: Option<String>) -> multivm_common::MultivmResult<()> {
//         if let Some(topic_name) = topic {
//             let data = bincode::serialize(&message).map_err(|e| {
//                 multivm_common::MultivmError::Network {
//                     message: format!("Serialization failed: {}", e),
//                     endpoint: None,
//                     retry_after: None,
//                 }
//             })?;
//             self.publish_message(&topic_name, data).await
//         } else {
//             self.broadcast(message).await
//         }
//     }
//
//     async fn send_message(&mut self, peer_id: String, message: messages::NetworkMessage) -> multivm_common::MultivmResult<()> {
//         self.send_to_peer(peer_id, message).await
//     }
// }

/// Event emitted by the P2P network layer
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// A new peer has connected
    PeerConnected(PeerInfo),
    /// A peer has disconnected
    PeerDisconnected(String),
    /// A message was received
    MessageReceived {
        peer_id: String,
        message: Box<crate::messages::NetworkMessage>,
    },
    /// A message was sent successfully
    MessageSent { peer_id: String, message_id: String },
    /// An error occurred
    Error {
        peer_id: Option<String>,
        error: crate::error::P2PError,
    },
}

/// Trait for handling network events
#[async_trait::async_trait]
pub trait NetworkEventHandler: Send + Sync {
    /// Handle a network event
    async fn handle_event(&mut self, event: NetworkEvent) -> multivm_common::MultivmResult<()>;

    /// Handle peer connection event
    async fn on_peer_connected(&self, peer_info: &PeerInfo) -> multivm_common::MultivmResult<()>;

    /// Handle peer disconnection event
    async fn on_peer_disconnected(&self, peer_id: &str) -> multivm_common::MultivmResult<()>;
}
