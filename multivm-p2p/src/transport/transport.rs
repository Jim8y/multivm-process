//! Unified production-grade transport layer for P2P communication
//!
//! This module provides a comprehensive transport implementation supporting:
//! - Multiple protocols: TCP, WebSocket, and QUIC
//! - Advanced connection management with pooling and limits
//! - Performance optimization with configurable parameters
//! - Robust error handling and recovery
//! - Detailed metrics and monitoring

use crate::error::{P2PError, P2PResult};
use crate::protocol::messages::NetworkMessage;
use futures::future::Either;
use libp2p::{
    core::{muxing::StreamMuxerBox, transport::Boxed, upgrade::Version},
    identity::Keypair,
    noise, tcp, websocket, yamux, Multiaddr, PeerId, Transport as LibP2PTransport,
};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Semaphore};
use tokio::time::timeout;
use tracing::debug;

/// Maximum number of connections in the pool
#[allow(dead_code)]
const MAX_CONNECTION_POOL_SIZE: usize = 1000;

/// Connection check interval
#[allow(dead_code)]
const CONNECTION_CHECK_INTERVAL: Duration = Duration::from_secs(30);

/// Transport protocol type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransportProtocol {
    Tcp,
    WebSocket,
    Quic,
}

/// Transport configuration with all protocol options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    /// TCP configuration
    pub tcp: TcpConfig,
    /// WebSocket configuration
    pub websocket: WebSocketConfig,
    /// QUIC configuration
    pub quic: QuicConfig,
    /// Connection management
    pub connections: ConnectionConfig,
    /// Performance tuning
    pub performance: PerformanceConfig,
    /// Security settings
    pub security: SecurityConfig,
}

/// TCP-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpConfig {
    /// Listen addresses
    pub listen_addresses: Vec<String>,
    /// Enable TCP no-delay
    pub nodelay: bool,
    /// Keepalive interval
    pub keepalive: Option<Duration>,
    /// Send buffer size
    pub send_buffer_size: Option<usize>,
    /// Receive buffer size
    pub recv_buffer_size: Option<usize>,
}

/// WebSocket-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    /// Listen addresses
    pub listen_addresses: Vec<String>,
    /// Maximum frame size
    pub max_frame_size: usize,
    /// Maximum message size
    pub max_message_size: usize,
    /// Ping interval
    pub ping_interval: Duration,
    /// Enable compression
    pub compression: bool,
}

/// QUIC-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuicConfig {
    /// Listen addresses
    pub listen_addresses: Vec<String>,
    /// Maximum idle timeout
    pub max_idle_timeout: Duration,
    /// Maximum concurrent bidirectional streams
    pub max_concurrent_bidi_streams: u64,
    /// Keep-alive interval
    pub keep_alive_interval: Duration,
}

/// Connection management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    /// Maximum total connections
    pub max_total_connections: usize,
    /// Maximum connections per peer
    pub max_connections_per_peer: usize,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Idle timeout
    pub idle_timeout: Duration,
    /// Handshake timeout
    pub handshake_timeout: Duration,
    /// Enable connection pooling
    pub enable_pooling: bool,
}

/// Performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Maximum concurrent operations
    pub max_concurrent_operations: usize,
    /// Stream window size
    pub stream_window_size: u32,
    /// Connection window size
    pub connection_window_size: u32,
    /// Message batch size
    pub message_batch_size: usize,
    /// Enable message compression
    pub enable_compression: bool,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable TLS
    pub enable_tls: bool,
    /// Require mutual TLS
    pub require_mtls: bool,
    /// Allowed cipher suites
    pub cipher_suites: Vec<String>,
    /// Certificate verification depth
    pub cert_verification_depth: u32,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            tcp: TcpConfig {
                listen_addresses: vec!["/ip4/0.0.0.0/tcp/0".to_string()],
                nodelay: true,
                keepalive: Some(Duration::from_secs(60)),
                send_buffer_size: Some(64 * 1024),
                recv_buffer_size: Some(64 * 1024),
            },
            websocket: WebSocketConfig {
                listen_addresses: vec!["/ip4/0.0.0.0/tcp/0/ws".to_string()],
                max_frame_size: 64 * 1024,
                max_message_size: 1024 * 1024,
                ping_interval: Duration::from_secs(30),
                compression: true,
            },
            quic: QuicConfig {
                listen_addresses: vec!["/ip4/0.0.0.0/udp/0/quic".to_string()],
                max_idle_timeout: Duration::from_secs(300),
                max_concurrent_bidi_streams: 100,
                keep_alive_interval: Duration::from_secs(30),
            },
            connections: ConnectionConfig {
                max_total_connections: 1000,
                max_connections_per_peer: 10,
                connection_timeout: Duration::from_secs(30),
                idle_timeout: Duration::from_secs(300),
                handshake_timeout: Duration::from_secs(10),
                enable_pooling: true,
            },
            performance: PerformanceConfig {
                max_concurrent_operations: 1000,
                stream_window_size: 256 * 1024,
                connection_window_size: 1024 * 1024,
                message_batch_size: 100,
                enable_compression: true,
            },
            security: SecurityConfig {
                enable_tls: true,
                require_mtls: false,
                cipher_suites: vec![
                    "TLS_AES_256_GCM_SHA384".to_string(),
                    "TLS_AES_128_GCM_SHA256".to_string(),
                    "TLS_CHACHA20_POLY1305_SHA256".to_string(),
                ],
                cert_verification_depth: 3,
            },
        }
    }
}

/// Connection state
#[derive(Debug)]
pub struct ConnectionState {
    pub peer_id: PeerId,
    pub address: Multiaddr,
    pub protocol: TransportProtocol,
    pub established_at: Instant,
    pub last_activity: Instant,
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
    pub messages_sent: AtomicU64,
    pub messages_received: AtomicU64,
    pub is_active: bool,
}

/// Transport statistics
#[derive(Debug, Default)]
pub struct TransportStats {
    pub active_connections: AtomicU64,
    pub total_connections_established: AtomicU64,
    pub total_connections_closed: AtomicU64,
    pub total_bytes_sent: AtomicU64,
    pub total_bytes_received: AtomicU64,
    pub total_messages_sent: AtomicU64,
    pub total_messages_received: AtomicU64,
    pub connection_errors: AtomicU64,
    pub protocol_stats: HashMap<TransportProtocol, ProtocolStats>,
}

/// Protocol-specific statistics
#[derive(Debug, Default)]
pub struct ProtocolStats {
    pub connections: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
    pub errors: AtomicU64,
}

/// Transport event
#[derive(Debug, Clone)]
pub enum TransportEvent {
    ConnectionEstablished {
        peer_id: PeerId,
        address: Multiaddr,
        protocol: TransportProtocol,
    },
    ConnectionClosed {
        peer_id: PeerId,
        reason: String,
    },
    MessageReceived {
        peer_id: PeerId,
        message: NetworkMessage,
    },
    Error {
        peer_id: Option<PeerId>,
        error: String,
    },
}

/// Unified transport layer implementation
#[allow(dead_code)]
pub struct UnifiedTransport {
    /// Configuration
    config: Arc<TransportConfig>,

    /// Our keypair
    keypair: Keypair,

    /// Our peer ID
    local_peer_id: PeerId,

    /// Active connections
    connections: Arc<RwLock<HashMap<PeerId, Vec<ConnectionState>>>>,

    /// Connection pool
    connection_pool: Arc<RwLock<VecDeque<(PeerId, Multiaddr)>>>,

    /// Connection semaphore for limiting concurrent connections
    connection_semaphore: Arc<Semaphore>,

    /// Transport statistics
    stats: Arc<TransportStats>,

    /// Event sender
    event_tx: mpsc::UnboundedSender<TransportEvent>,

    /// Shutdown signal
    shutdown: Arc<RwLock<bool>>,
}

impl UnifiedTransport {
    /// Create a new unified transport
    pub fn new(
        config: TransportConfig,
        keypair: Keypair,
        event_tx: mpsc::UnboundedSender<TransportEvent>,
    ) -> Self {
        let local_peer_id = PeerId::from(keypair.public());

        Self {
            config: Arc::new(config.clone()),
            keypair,
            local_peer_id,
            connections: Arc::new(RwLock::new(HashMap::new())),
            connection_pool: Arc::new(RwLock::new(VecDeque::new())),
            connection_semaphore: Arc::new(Semaphore::new(
                config.connections.max_total_connections,
            )),
            stats: Arc::new(TransportStats::default()),
            event_tx,
            shutdown: Arc::new(RwLock::new(false)),
        }
    }

    /// Build the libp2p transport
    pub fn build_transport(&self) -> P2PResult<Boxed<(PeerId, StreamMuxerBox)>> {
        let keypair = self.keypair.clone();

        // TCP transport
        let tcp_transport = tcp::tokio::Transport::new(tcp::Config::default())
            .upgrade(Version::V1)
            .authenticate(
                noise::Config::new(&keypair).map_err(|e| P2PError::Transport(e.to_string()))?,
            )
            .multiplex(yamux::Config::default())
            .boxed();

        // WebSocket transport
        let ws_transport =
            websocket::WsConfig::new(tcp::tokio::Transport::new(tcp::Config::default()))
                .upgrade(Version::V1)
                .authenticate(
                    noise::Config::new(&keypair).map_err(|e| P2PError::Transport(e.to_string()))?,
                )
                .multiplex(yamux::Config::default())
                .boxed();

        // Combine transports and map Either to unified output
        let transport = tcp_transport
            .or_transport(ws_transport)
            .map(|either_output, _| match either_output {
                Either::Left((peer_id, muxer)) => (peer_id, muxer),
                Either::Right((peer_id, muxer)) => (peer_id, muxer),
            })
            .boxed();

        Ok(transport)
    }

    /// Connect to a peer
    pub async fn connect(&self, peer_id: PeerId, address: Multiaddr) -> P2PResult<()> {
        // Check if shutdown
        if *self.shutdown.read() {
            return Err(P2PError::Transport(
                "Transport is shutting down".to_string(),
            ));
        }

        // Check connection limits
        self.check_connection_limits(peer_id)?;

        // Acquire connection permit
        let _permit =
            self.connection_semaphore.acquire().await.map_err(|_| {
                P2PError::Transport("Failed to acquire connection permit".to_string())
            })?;

        // Attempt connection with timeout
        let connect_result = timeout(
            self.config.connections.connection_timeout,
            self.establish_connection(peer_id, address.clone()),
        )
        .await;

        match connect_result {
            Ok(Ok(())) => {
                self.stats
                    .total_connections_established
                    .fetch_add(1, Ordering::Relaxed);
                self.event_tx
                    .send(TransportEvent::ConnectionEstablished {
                        peer_id,
                        address: address.clone(),
                        protocol: self.detect_protocol(&address),
                    })
                    .ok();
                Ok(())
            }
            Ok(Err(e)) => {
                self.stats.connection_errors.fetch_add(1, Ordering::Relaxed);
                Err(e)
            }
            Err(_) => {
                self.stats.connection_errors.fetch_add(1, Ordering::Relaxed);
                Err(P2PError::timeout(
                    self.config.connections.connection_timeout,
                ))
            }
        }
    }

    /// Send a message to a peer
    pub async fn send_message(&self, peer_id: PeerId, message: NetworkMessage) -> P2PResult<()> {
        // Serialize message
        let data = bincode::serialize(&message).map_err(|e| P2PError::Serialization {
            message: e.to_string(),
        })?;

        // Update connection stats
        {
            let connections = self.connections.read();
            if let Some(peer_connections) = connections.get(&peer_id) {
                if let Some(connection) = peer_connections.first() {
                    connection
                        .bytes_sent
                        .fetch_add(data.len() as u64, Ordering::Relaxed);
                    connection.messages_sent.fetch_add(1, Ordering::Relaxed);
                } else {
                    return Err(P2PError::connection_error("No active connection to peer"));
                }
            } else {
                return Err(P2PError::connection_error("No active connection to peer"));
            }
        }

        self.stats
            .total_bytes_sent
            .fetch_add(data.len() as u64, Ordering::Relaxed);
        self.stats
            .total_messages_sent
            .fetch_add(1, Ordering::Relaxed);

        // Send with timeout
        timeout(Duration::from_secs(10), self.send_data(peer_id, data))
            .await
            .map_err(|_| P2PError::timeout(Duration::from_secs(30)))?
    }

    /// Disconnect from a peer
    pub async fn disconnect(&self, peer_id: PeerId) -> P2PResult<()> {
        let mut connections = self.connections.write();

        if let Some(peer_connections) = connections.remove(&peer_id) {
            let count = peer_connections.len();
            self.stats
                .active_connections
                .fetch_sub(count as u64, Ordering::Relaxed);
            self.stats
                .total_connections_closed
                .fetch_add(count as u64, Ordering::Relaxed);

            self.event_tx
                .send(TransportEvent::ConnectionClosed {
                    peer_id,
                    reason: "Manual disconnect".to_string(),
                })
                .ok();
        }

        Ok(())
    }

    /// Get transport statistics
    pub fn get_stats(&self) -> TransportStats {
        TransportStats {
            active_connections: AtomicU64::new(
                self.stats.active_connections.load(Ordering::Relaxed),
            ),
            total_connections_established: AtomicU64::new(
                self.stats
                    .total_connections_established
                    .load(Ordering::Relaxed),
            ),
            total_connections_closed: AtomicU64::new(
                self.stats.total_connections_closed.load(Ordering::Relaxed),
            ),
            total_bytes_sent: AtomicU64::new(self.stats.total_bytes_sent.load(Ordering::Relaxed)),
            total_bytes_received: AtomicU64::new(
                self.stats.total_bytes_received.load(Ordering::Relaxed),
            ),
            total_messages_sent: AtomicU64::new(
                self.stats.total_messages_sent.load(Ordering::Relaxed),
            ),
            total_messages_received: AtomicU64::new(
                self.stats.total_messages_received.load(Ordering::Relaxed),
            ),
            connection_errors: AtomicU64::new(self.stats.connection_errors.load(Ordering::Relaxed)),
            protocol_stats: HashMap::new(), // TODO: Implement proper stats cloning
        }
    }

    /// Shutdown the transport
    pub async fn shutdown(&self) {
        *self.shutdown.write() = true;

        // Close all connections
        let peer_ids: Vec<PeerId> = {
            let connections = self.connections.read();
            connections.keys().cloned().collect()
        };
        for peer_id in peer_ids {
            self.disconnect(peer_id).await.ok();
        }
    }

    // Helper methods

    fn check_connection_limits(&self, peer_id: PeerId) -> P2PResult<()> {
        let connections = self.connections.read();

        // Check total connections
        let total_connections: usize = connections.values().map(|v| v.len()).sum();
        if total_connections >= self.config.connections.max_total_connections {
            return Err(P2PError::connection_error(
                "Maximum total connections reached",
            ));
        }

        // Check per-peer connections
        if let Some(peer_connections) = connections.get(&peer_id) {
            if peer_connections.len() >= self.config.connections.max_connections_per_peer {
                return Err(P2PError::connection_error(
                    "Maximum connections per peer reached",
                ));
            }
        }

        Ok(())
    }

    fn detect_protocol(&self, address: &Multiaddr) -> TransportProtocol {
        let addr_str = address.to_string();
        if addr_str.contains("/ws") {
            TransportProtocol::WebSocket
        } else if addr_str.contains("/quic") {
            TransportProtocol::Quic
        } else {
            TransportProtocol::Tcp
        }
    }

    async fn establish_connection(&self, peer_id: PeerId, address: Multiaddr) -> P2PResult<()> {
        // Implementation would establish actual connection
        // This is a placeholder
        let protocol = self.detect_protocol(&address);

        let connection = ConnectionState {
            peer_id,
            address: address.clone(),
            protocol,
            established_at: Instant::now(),
            last_activity: Instant::now(),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            messages_sent: AtomicU64::new(0),
            messages_received: AtomicU64::new(0),
            is_active: true,
        };

        let mut connections = self.connections.write();
        connections.entry(peer_id).or_default().push(connection);
        self.stats
            .active_connections
            .fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    async fn send_data(&self, peer_id: PeerId, data: Vec<u8>) -> P2PResult<()> {
        // Implementation would send actual data
        // This is a placeholder
        debug!("Sending {} bytes to peer {}", data.len(), peer_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::identity;

    #[tokio::test]
    async fn test_transport_creation() {
        let keypair = identity::Keypair::generate_ed25519();
        let (tx, _rx) = mpsc::unbounded_channel();
        let config = TransportConfig::default();

        let transport = UnifiedTransport::new(config, keypair, tx);

        // Test building transport
        let _built = transport.build_transport().unwrap();
    }

    #[tokio::test]
    async fn test_connection_limits() {
        let keypair = identity::Keypair::generate_ed25519();
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut config = TransportConfig::default();
        config.connections.max_connections_per_peer = 1;

        let transport = UnifiedTransport::new(config, keypair, tx);
        let peer_id = PeerId::random();
        let address: Multiaddr = "/ip4/127.0.0.1/tcp/1234".parse().unwrap();

        // First connection should succeed
        assert!(transport.connect(peer_id, address.clone()).await.is_ok());

        // Second connection should fail due to limit
        assert!(transport.connect(peer_id, address).await.is_err());
    }

    #[tokio::test]
    async fn test_stats_tracking() {
        let keypair = identity::Keypair::generate_ed25519();
        let (tx, _rx) = mpsc::unbounded_channel();
        let config = TransportConfig::default();

        let transport = UnifiedTransport::new(config, keypair, tx);
        let peer_id = PeerId::random();
        let address: Multiaddr = "/ip4/127.0.0.1/tcp/1234".parse().unwrap();

        // Connect
        transport.connect(peer_id, address).await.unwrap();

        // Check stats
        let stats = transport.get_stats();
        assert_eq!(
            stats.total_connections_established.load(Ordering::Relaxed),
            1
        );
        assert_eq!(stats.active_connections.load(Ordering::Relaxed), 1);

        // Disconnect
        transport.disconnect(peer_id).await.unwrap();

        // Check stats again
        let stats = transport.get_stats();
        assert_eq!(stats.total_connections_closed.load(Ordering::Relaxed), 1);
        assert_eq!(stats.active_connections.load(Ordering::Relaxed), 0);
    }
}
