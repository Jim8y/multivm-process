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

pub mod config;
pub mod discovery;
pub mod error;
pub mod messages;
pub mod network;
pub mod protocol;
pub mod rate_limiter;
pub mod routing;
pub mod secure_network;
pub mod security;
pub mod transport;

#[cfg(feature = "metrics")]
pub mod metrics;

// Test modules
#[cfg(test)]
mod discovery_tests;
#[cfg(test)]
mod network_tests;
#[cfg(test)]
mod protocol_tests;
#[cfg(test)]
mod routing_tests;
#[cfg(test)]
mod transport_tests;

// Re-exports for public API
pub use config::P2PConfig as P2PNetworkConfig;
pub use discovery::*;
pub use error::*;
pub use messages::*;
pub use network::{NetworkManager, P2PNetwork};
pub use protocol::*;
pub use routing::*;
pub use transport::*;

// Re-export commonly used types
pub use messages::{ExecutionPriority, Priority};

use multivm_common::MultivmResult;

/// Main trait for the MultiVM P2P networking layer
#[async_trait::async_trait]
pub trait P2PNetworkLayer: Send + Sync {
    /// Start the P2P network layer
    async fn start(&mut self) -> MultivmResult<()>;

    /// Stop the P2P network layer
    async fn stop(&mut self) -> MultivmResult<()>;

    /// Send a message to a specific peer
    async fn send_to_peer(&mut self, peer_id: String, message: NetworkMessage)
        -> MultivmResult<()>;

    /// Send a message to a specific peer (consensus-compatible alias)
    async fn send_message(
        &mut self,
        peer_id: String,
        message: NetworkMessage,
    ) -> MultivmResult<()> {
        self.send_to_peer(peer_id, message).await
    }

    /// Broadcast a message to all connected peers
    async fn broadcast(&mut self, message: NetworkMessage) -> MultivmResult<()>;

    /// Broadcast a message to all connected peers with optional topic (consensus-compatible)
    async fn broadcast_message(
        &mut self,
        message: NetworkMessage,
        topic: Option<String>,
    ) -> MultivmResult<()> {
        // Add topic to message metadata if provided
        let mut msg = message;
        if let Some(topic) = topic {
            msg.metadata.insert("topic".to_string(), topic);
        }
        self.broadcast(msg).await
    }

    /// Subscribe to messages of a specific topic
    async fn subscribe(&mut self, topic: &str) -> MultivmResult<()>;

    /// Subscribe to messages of a specific topic (consensus-compatible alias)
    async fn subscribe_to_topic(&mut self, topic: &str) -> MultivmResult<()> {
        self.subscribe(topic).await
    }

    /// Unsubscribe from messages of a specific topic
    async fn unsubscribe(&mut self, topic: &str) -> MultivmResult<()>;

    /// Get list of connected peers
    async fn get_connected_peers(&self) -> MultivmResult<Vec<PeerInfo>>;

    /// Get network statistics
    async fn get_network_stats(&self) -> MultivmResult<NetworkStats>;

    /// Handle incoming message (called by the network layer)
    async fn handle_incoming_message(
        &mut self,
        message: NetworkMessage,
        peer_id: String,
    ) -> MultivmResult<()>;
}

/// Information about a connected peer
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PeerInfo {
    /// Peer ID as string
    pub peer_id: String,
    /// Multi-addresses the peer can be reached at
    pub addresses: Vec<multiaddr::Multiaddr>,
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
        message: Box<NetworkMessage>,
    },
    /// A message was sent successfully
    MessageSent { peer_id: String, message_id: String },
    /// An error occurred
    Error {
        peer_id: Option<String>,
        error: P2PError,
    },
}

/// Trait for handling network events
#[async_trait::async_trait]
pub trait NetworkEventHandler: Send + Sync {
    /// Handle a network event
    async fn handle_event(&mut self, event: NetworkEvent) -> MultivmResult<()>;

    /// Handle peer connection event
    async fn on_peer_connected(&self, peer_info: &PeerInfo) -> MultivmResult<()>;

    /// Handle peer disconnection event  
    async fn on_peer_disconnected(&self, peer_id: &str) -> MultivmResult<()>;
}
