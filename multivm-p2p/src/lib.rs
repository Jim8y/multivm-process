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
//!
//! ## Examples
//!
//! ### Basic P2P Manager Usage
//!
//! ```rust
//! use multivm_p2p::core::manager::P2PManager;
//! use multivm_p2p::config::P2PConfig;
//!
//! let config = P2PConfig::default();
//! let manager = P2PManager::new(config);
//! 
//! // Manager is created but not yet started
//! assert!(!manager.is_running());
//! ```
//!
//! ### Network Configuration
//!
//! ```rust
//! use multivm_p2p::config::{P2PConfig, NetworkMode};
//!
//! let mut config = P2PConfig::default();
//! config.network_mode = NetworkMode::FullNode;
//! config.max_peers = 50;
//! 
//! assert_eq!(config.max_peers, 50);
//! ```

// Core modules
pub mod config;
pub mod error;

// Core networking
pub mod core {
    pub mod manager;
    pub mod network;
}

// Transport layer
pub mod transport {
    pub mod connection_manager;
    pub mod transport;
}

// Security modules
pub mod security {
    pub mod audit;
    pub mod auth;
    pub mod dos_protection;
    pub mod encryption;
    pub mod reputation;
}

// Protocol handling
pub mod protocol {
    pub mod messages;
    pub mod protocol;
    pub mod routing;
}

// Discovery modules
pub mod discovery {
    pub mod discovery;
    pub mod gossip;
}

// Monitoring
pub mod monitoring {
    pub mod metrics;
    pub mod monitoring;
}

// Additional modules
pub mod admin;
pub mod circuit_breaker;
pub mod consensus_integration;
pub mod load_balancer;
pub mod rate_limiter;
pub mod secure_network;

// Test modules are organized in the tests/ directory

#[cfg(feature = "metrics")]
pub use monitoring::metrics;

#[cfg(not(feature = "metrics"))]
pub mod metrics {
    //! Stub metrics module when metrics feature is disabled

    /// Stub for init_metrics when metrics are disabled
    pub fn init_metrics() {}

    /// Stub for record_message_sent when metrics are disabled
    pub fn record_message_sent(_message_type: &str, _target_type: &str) {}

    /// Stub for record_message_received when metrics are disabled
    pub fn record_message_received(_message_type: &str, _source_peer: &str) {}

    /// Stub for record_processing_duration when metrics are disabled
    pub fn record_processing_duration(_message_type: &str, _duration: f64) {}

    /// Stub for update_active_peers when metrics are disabled
    pub fn update_active_peers(_peer_type: &str, _connection_status: &str, _count: f64) {}

    /// Stub for update_bandwidth when metrics are disabled
    pub fn update_bandwidth(_direction: &str, _protocol: &str, _bytes_per_sec: f64) {}

    /// Stub for record_protocol_translation when metrics are disabled
    pub fn record_protocol_translation(_source_vm: &str, _target_vm: &str, _status: &str) {}

    /// Stub for record_routing_decision when metrics are disabled
    pub fn record_routing_decision(_strategy: &str, _message_type: &str, _result: &str) {}

    /// Stub for record_peer_discovery when metrics are disabled
    pub fn record_peer_discovery(_method: &str, _result: &str) {}

    /// Stub for update_network_health when metrics are disabled
    pub fn update_network_health(_component: &str, _status: f64) {}

    /// Stub for update_connection_pool when metrics are disabled
    pub fn update_connection_pool(_pool_type: &str, _status: &str, _count: f64) {}

    /// Stub for update_queue_depth when metrics are disabled
    pub fn update_queue_depth(_queue_type: &str, _priority: &str, _depth: f64) {}

    /// Stub for record_network_error when metrics are disabled
    pub fn record_network_error(_error_type: &str, _severity: &str) {}

    /// Stub for health_status_to_metric when metrics are disabled
    pub fn health_status_to_metric(_status: &crate::core::network::NetworkHealthStatus) -> f64 {
        0.0
    }
}

// Test modules
#[cfg(test)]
pub mod tests {
    pub mod integration_tests;
    pub mod message_tests;
    pub mod network_tests;
    pub mod security_tests;
}

// Re-exports for convenience
pub use config::P2PConfig;
pub use core::manager::{P2PManager, P2PManagerStats};
pub use core::network::P2PNetwork;
pub use error::{P2PError, P2PResult};
pub use protocol::messages::{MessagePayload, NetworkMessage, Priority};
pub use transport::transport::UnifiedTransport;

// Type aliases
pub type NodeId = libp2p::PeerId;
pub type MessageId = uuid::Uuid;

/// Version information for the P2P protocol
pub const P2P_PROTOCOL_VERSION: &str = "1.0.0";

/// Default P2P port
pub const DEFAULT_P2P_PORT: u16 = 26656;

/// Create a default P2P configuration
pub fn default_config() -> P2PConfig {
    P2PConfig::default()
}

/// Initialize the P2P module with tracing
#[cfg(feature = "tracing-subscriber")]
pub fn init_with_tracing() {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "multivm_p2p=debug,libp2p=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

/// Initialize the P2P module (no-op when tracing-subscriber is not available)
#[cfg(not(feature = "tracing-subscriber"))]
pub fn init_with_tracing() {
    // No-op when tracing-subscriber is not available
}

#[cfg(test)]
mod lib_tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = default_config();
        assert!(!config.network.listen_addresses.is_empty());
    }

    #[test]
    fn test_protocol_version() {
        assert_eq!(P2P_PROTOCOL_VERSION, "1.0.0");
    }
}
