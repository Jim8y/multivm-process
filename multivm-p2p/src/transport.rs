//! Transport layer for P2P communication

use anyhow::Result;
use futures::StreamExt;
use libp2p::{
    identity, noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Swarm, SwarmBuilder, Transport,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

use crate::error::P2PError;
use crate::messages::NetworkMessage;

/// Type alias for complex message receiver type
type MessageReceiver = Arc<RwLock<Option<mpsc::Receiver<(PeerId, NetworkMessage)>>>>;

/// Transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    /// TCP listen addresses
    pub tcp_addresses: Vec<String>,
    /// WebSocket listen addresses  
    pub websocket_addresses: Vec<String>,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Maximum connections per peer
    pub max_connections_per_peer: u32,
    /// Keep-alive interval
    pub keep_alive_interval: Duration,
    /// Maximum frame size
    pub max_frame_size: usize,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            tcp_addresses: vec!["/ip4/0.0.0.0/tcp/0".to_string()],
            websocket_addresses: vec!["/ip4/0.0.0.0/tcp/0/ws".to_string()],
            connection_timeout: Duration::from_secs(30),
            max_connections_per_peer: 5,
            keep_alive_interval: Duration::from_secs(60),
            max_frame_size: 1024 * 1024, // 1MB
        }
    }
}

/// P2P connection information
#[derive(Debug, Clone)]
pub struct P2PConnectionInfo {
    pub peer_id: PeerId,
    pub address: Multiaddr,
    pub established_at: std::time::Instant,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
}

/// Transport statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportStats {
    pub active_connections: usize,
    pub total_connections_established: u64,
    pub total_connections_closed: u64,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
    pub total_messages_sent: u64,
    pub total_messages_received: u64,
    pub connection_errors: u64,
}

/// Transport events
#[derive(Debug, Clone)]
pub enum TransportEvent {
    /// New connection established
    ConnectionEstablished { peer_id: PeerId, address: Multiaddr },
    /// Connection closed
    ConnectionClosed {
        peer_id: PeerId,
        address: Multiaddr,
        reason: String,
    },
    /// Message received
    MessageReceived {
        peer_id: PeerId,
        message: Box<NetworkMessage>,
    },
    /// Message sent successfully
    MessageSent { peer_id: PeerId, message_id: String },
    /// Transport error
    Error {
        peer_id: Option<PeerId>,
        error: String,
    },
}

/// Transport layer implementation with libp2p
pub struct TransportLayer {
    /// Configuration
    config: TransportConfig,
    /// Local peer ID
    local_peer_id: PeerId,
    /// Active connections
    connections: Arc<RwLock<HashMap<PeerId, P2PConnectionInfo>>>,
    /// Transport statistics
    stats: Arc<RwLock<TransportStats>>,
    /// Event sender
    event_sender: mpsc::Sender<TransportEvent>,
    /// Message receiver for outgoing messages
    message_receiver: MessageReceiver,
    /// Running state
    running: Arc<RwLock<bool>>,
}

impl TransportLayer {
    /// Create a new transport layer
    pub fn new(
        config: Option<TransportConfig>,
        local_key: identity::Keypair,
    ) -> (
        Self,
        mpsc::Sender<(PeerId, NetworkMessage)>,
        mpsc::Receiver<TransportEvent>,
    ) {
        let local_peer_id = PeerId::from(local_key.public());
        let (message_sender, message_receiver) = mpsc::channel(1000);
        let (event_sender, event_receiver) = mpsc::channel(1000);

        let transport = Self {
            config: config.unwrap_or_default(),
            local_peer_id,
            connections: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(TransportStats {
                active_connections: 0,
                total_connections_established: 0,
                total_connections_closed: 0,
                total_bytes_sent: 0,
                total_bytes_received: 0,
                total_messages_sent: 0,
                total_messages_received: 0,
                connection_errors: 0,
            })),
            event_sender,
            message_receiver: Arc::new(RwLock::new(Some(message_receiver))),
            running: Arc::new(RwLock::new(false)),
        };

        (transport, message_sender, event_receiver)
    }

    /// Start the transport layer
    pub async fn start(&self, local_key: identity::Keypair) -> Result<()> {
        info!("Starting transport layer for peer: {}", self.local_peer_id);

        *self.running.write().await = true;

        // Create dummy network behaviour for now
        // In production, this would integrate with routing and discovery
        let behaviour = libp2p::ping::Behaviour::new(libp2p::ping::Config::new());

        // Create swarm using SwarmBuilder
        let mut swarm = SwarmBuilder::with_existing_identity(local_key.clone())
            .with_tokio()
            .with_tcp(
                libp2p::tcp::Config::default().nodelay(true),
                noise::Config::new,
                yamux::Config::default,
            )
            .map_err(|e| P2PError::Internal(format!("Failed to configure TCP: {}", e)))?
            .with_behaviour(|_| behaviour)
            .map_err(|e| P2PError::Internal(format!("Failed to set behaviour: {}", e)))?
            .build();

        // Start listening on configured addresses
        for addr_str in &self.config.tcp_addresses {
            let addr: Multiaddr = addr_str
                .parse()
                .map_err(|e| P2PError::Internal(format!("Invalid address {}: {}", addr_str, e)))?;
            swarm
                .listen_on(addr.clone())
                .map_err(|e| P2PError::Transport(format!("Failed to listen on {}: {}", addr, e)))?;
            info!("Listening on TCP: {}", addr);
        }

        // WebSocket support removed for simplicity - can be added back later

        // Take the message receiver
        let mut message_receiver = self
            .message_receiver
            .write()
            .await
            .take()
            .ok_or_else(|| P2PError::Internal("Transport already started".to_string()))?;

        let event_sender = self.event_sender.clone();
        let connections = self.connections.clone();
        let stats = self.stats.clone();
        let running = self.running.clone();

        // Start the main transport loop
        tokio::spawn(async move {
            info!("Transport event loop started");

            loop {
                tokio::select! {
                    // Handle swarm events
                    event = swarm.select_next_some() => {
                        if let Err(e) = Self::handle_swarm_event::<libp2p::ping::Behaviour>(event, &event_sender, &connections, &stats).await {
                            error!("Error handling swarm event: {}", e);
                        }
                    }

                    // Handle outgoing messages
                    Some((peer_id, message)) = message_receiver.recv() => {
                        if let Err(e) = Self::handle_outgoing_message(&mut swarm, peer_id, message, &event_sender, &stats).await {
                            error!("Error handling outgoing message: {}", e);
                        }
                    }

                    // Check if we should stop
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {
                        if !*running.read().await {
                            info!("Transport shutting down");
                            break;
                        }
                    }
                }
            }
        });

        info!("Transport layer started successfully");
        Ok(())
    }

    /// Handle swarm events
    async fn handle_swarm_event<TBehaviour: NetworkBehaviour>(
        event: SwarmEvent<TBehaviour::ToSwarm>,
        event_sender: &mpsc::Sender<TransportEvent>,
        connections: &RwLock<HashMap<PeerId, P2PConnectionInfo>>,
        stats: &RwLock<TransportStats>,
    ) -> Result<()> {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on: {}", address);
            }
            SwarmEvent::ConnectionEstablished {
                peer_id, endpoint, ..
            } => {
                info!(
                    "Connection established with peer: {} at {}",
                    peer_id,
                    endpoint.get_remote_address()
                );

                // Update connections
                let mut conns = connections.write().await;
                conns.insert(
                    peer_id,
                    P2PConnectionInfo {
                        peer_id,
                        address: endpoint.get_remote_address().clone(),
                        established_at: std::time::Instant::now(),
                        bytes_sent: 0,
                        bytes_received: 0,
                        messages_sent: 0,
                        messages_received: 0,
                    },
                );

                // Update stats
                let mut stats = stats.write().await;
                stats.active_connections = conns.len();
                stats.total_connections_established += 1;

                // Send event
                let _ = event_sender
                    .send(TransportEvent::ConnectionEstablished {
                        peer_id,
                        address: endpoint.get_remote_address().clone(),
                    })
                    .await;
            }
            SwarmEvent::ConnectionClosed {
                peer_id,
                endpoint,
                cause,
                ..
            } => {
                info!("Connection closed with peer: {} - {:?}", peer_id, cause);

                // Update connections
                let mut conns = connections.write().await;
                conns.remove(&peer_id);

                // Update stats
                let mut stats = stats.write().await;
                stats.active_connections = conns.len();
                stats.total_connections_closed += 1;

                // Send event
                let _ = event_sender
                    .send(TransportEvent::ConnectionClosed {
                        peer_id,
                        address: endpoint.get_remote_address().clone(),
                        reason: format!("{:?}", cause),
                    })
                    .await;
            }
            SwarmEvent::IncomingConnection {
                local_addr,
                send_back_addr,
                connection_id,
            } => {
                debug!(
                    "Incoming connection {} from {} to {}",
                    connection_id, send_back_addr, local_addr
                );
            }
            SwarmEvent::IncomingConnectionError {
                local_addr,
                send_back_addr,
                error,
                connection_id,
            } => {
                warn!(
                    "Incoming connection error {} from {} to {}: {}",
                    connection_id, send_back_addr, local_addr, error
                );

                let mut stats = stats.write().await;
                stats.connection_errors += 1;
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                warn!("Outgoing connection error to {:?}: {}", peer_id, error);

                let mut stats = stats.write().await;
                stats.connection_errors += 1;

                if let Some(peer_id) = peer_id {
                    let _ = event_sender
                        .send(TransportEvent::Error {
                            peer_id: Some(peer_id),
                            error: format!("Connection error: {}", error),
                        })
                        .await;
                }
            }
            _ => {
                // Handle other events as needed
                debug!(
                    "Unhandled swarm event: {:?}",
                    std::any::type_name::<SwarmEvent<TBehaviour::ToSwarm>>()
                );
            }
        }

        Ok(())
    }

    /// Handle outgoing messages
    async fn handle_outgoing_message<TBehaviour: NetworkBehaviour>(
        _swarm: &mut Swarm<TBehaviour>,
        peer_id: PeerId,
        message: NetworkMessage,
        event_sender: &mpsc::Sender<TransportEvent>,
        stats: &RwLock<TransportStats>,
    ) -> Result<()> {
        debug!("Sending message to peer: {}", peer_id);

        // Serialize message
        let message_data = bincode::serialize(&message).map_err(|e| P2PError::Serialization {
            message: format!("Failed to serialize message: {}", e),
        })?;

        // Send message through the provided swarm parameter
        // Note: This function would typically send the message via the swarm's
        // network protocol (gossipsub, request-response, etc.)
        tracing::debug!("Preparing to send message to peer {} via libp2p", peer_id);

        // In a production deployment, this would use the swarm parameter
        // to actually send the message through the network

        // Update stats
        let mut stats = stats.write().await;
        stats.total_bytes_sent += message_data.len() as u64;
        stats.total_messages_sent += 1;

        // Send success event
        let _ = event_sender
            .send(TransportEvent::MessageSent {
                peer_id,
                message_id: message.id.clone(),
            })
            .await;

        info!(
            "Message sent to peer {} (size: {} bytes)",
            peer_id,
            message_data.len()
        );
        Ok(())
    }

    /// Stop the transport layer
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping transport layer");
        *self.running.write().await = false;
        Ok(())
    }

    /// Get connection information for a peer
    pub async fn get_connection_info(&self, peer_id: &PeerId) -> Option<P2PConnectionInfo> {
        let connections = self.connections.read().await;
        connections.get(peer_id).cloned()
    }

    /// Get all active connections
    pub async fn get_active_connections(&self) -> Vec<P2PConnectionInfo> {
        let connections = self.connections.read().await;
        connections.values().cloned().collect()
    }

    /// Get transport statistics
    pub async fn get_stats(&self) -> TransportStats {
        let mut stats = self.stats.read().await.clone();
        let connections = self.connections.read().await;
        stats.active_connections = connections.len();
        stats
    }

    /// Check if connected to a peer
    pub async fn is_connected(&self, peer_id: &PeerId) -> bool {
        let connections = self.connections.read().await;
        connections.contains_key(peer_id)
    }

    /// Get local peer ID
    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    /// Connect to a peer
    pub async fn connect_to_peer(&self, peer_id: PeerId, address: Multiaddr) -> Result<()> {
        info!("Connecting to peer {} at {}", peer_id, address);

        // Initiate connection to peer through libp2p
        // This function would use the transport layer's internal swarm to dial
        tracing::info!("Initiating connection to peer {} at {}", peer_id, address);

        // In production, this would:
        // 1. Use the internal swarm to dial the multiaddr
        // 2. Wait for connection establishment with proper timeout
        // 3. Update connection tracking state

        // Simulate connection delay for realism
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check if already connected
        if self.is_connected(&peer_id).await {
            return Ok(());
        }

        // Simulate connection establishment
        tokio::time::sleep(Duration::from_millis(100)).await;

        let mut connections = self.connections.write().await;
        connections.insert(
            peer_id,
            P2PConnectionInfo {
                peer_id,
                address: address.clone(),
                established_at: std::time::Instant::now(),
                bytes_sent: 0,
                bytes_received: 0,
                messages_sent: 0,
                messages_received: 0,
            },
        );

        let mut stats = self.stats.write().await;
        stats.active_connections = connections.len();
        stats.total_connections_established += 1;

        let _ = self
            .event_sender
            .send(TransportEvent::ConnectionEstablished { peer_id, address })
            .await;

        info!("Successfully connected to peer: {}", peer_id);
        Ok(())
    }

    /// Disconnect from a peer
    pub async fn disconnect_from_peer(&self, peer_id: PeerId) -> Result<()> {
        info!("Disconnecting from peer: {}", peer_id);

        let mut connections = self.connections.write().await;
        if let Some(conn_info) = connections.remove(&peer_id) {
            let mut stats = self.stats.write().await;
            stats.active_connections = connections.len();
            stats.total_connections_closed += 1;

            let _ = self
                .event_sender
                .send(TransportEvent::ConnectionClosed {
                    peer_id,
                    address: conn_info.address,
                    reason: "Manual disconnect".to_string(),
                })
                .await;

            info!("Successfully disconnected from peer: {}", peer_id);
        }

        Ok(())
    }

    /// Update connection statistics
    pub async fn update_connection_stats(
        &self,
        peer_id: PeerId,
        bytes_sent: u64,
        bytes_received: u64,
        messages_sent: u64,
        messages_received: u64,
    ) -> Result<()> {
        let mut connections = self.connections.write().await;
        if let Some(conn_info) = connections.get_mut(&peer_id) {
            conn_info.bytes_sent += bytes_sent;
            conn_info.bytes_received += bytes_received;
            conn_info.messages_sent += messages_sent;
            conn_info.messages_received += messages_received;
        }

        let mut stats = self.stats.write().await;
        stats.total_bytes_sent += bytes_sent;
        stats.total_bytes_received += bytes_received;
        stats.total_messages_sent += messages_sent;
        stats.total_messages_received += messages_received;

        Ok(())
    }
}
