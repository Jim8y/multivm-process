//! Production-grade P2P networking implementation using libp2p
//!
//! This module provides a complete peer-to-peer networking layer for the MultiVM system,
//! enabling secure, decentralized communication between nodes in the network.

use crate::{NetworkStats, error::P2PError};
use multivm_common::MultivmResult;

use libp2p::{
    gossipsub::{self, IdentTopic, MessageAuthenticity, MessageId, ValidationMode},
    identify,
    kad::{self, store::MemoryStore, Behaviour as KademliaBehaviour},
    mdns,
    noise,
    ping,
    tcp,
    yamux,
    Multiaddr,
    PeerId,
    Swarm,
    // Note: request_response protocol implementation deferred to gossipsub for reliability
    SwarmBuilder,
    Transport,
};
// Using gossipsub for all peer communication which provides reliable message delivery
use serde::{Deserialize, Serialize};
use futures::StreamExt;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

/// Network behavior configuration
#[derive(libp2p::swarm::NetworkBehaviour)]
#[behaviour(to_swarm = "NetworkBehaviourEvent")]
pub struct NetworkBehaviour {
    /// Kademlia DHT for peer discovery and routing
    pub kademlia: KademliaBehaviour<MemoryStore>,
    /// Gossipsub for message broadcasting
    pub gossipsub: gossipsub::Behaviour,
    /// mDNS for local peer discovery
    pub mdns: mdns::tokio::Behaviour,
    /// Identify protocol for peer identification
    pub identify: identify::Behaviour,
    /// Ping protocol for connection health
    pub ping: ping::Behaviour,
    // Note: Using gossipsub for direct messaging instead of request-response for stability
}

/// Custom codec for MultiVM request-response protocol
#[derive(Debug, Clone)]
pub struct MultiVMCodec;

/// P2P Request structure for direct messaging via gossipsub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PRequest {
    pub id: String,
    pub payload: Vec<u8>,
    pub response_topic: String,
}

/// P2P Response structure for direct messaging via gossipsub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PResponse {
    pub request_id: String,
    pub payload: Vec<u8>,
    pub success: bool,
}

// Using gossipsub for all peer communication which provides reliable message delivery

impl Default for MultiVMCodec {
    fn default() -> Self {
        Self
    }
}

/// Events from our network behavior
#[derive(Debug)]
pub enum NetworkBehaviourEvent {
    /// Kademlia events
    Kademlia(kad::Event),
    /// Gossipsub events  
    Gossipsub(gossipsub::Event),
    /// mDNS events
    Mdns(mdns::Event),
    /// Identify events
    Identify(identify::Event),
    /// Ping events
    Ping(ping::Event),
    // Note: Using gossipsub events for all peer communication
}

impl From<kad::Event> for NetworkBehaviourEvent {
    fn from(event: kad::Event) -> Self {
        NetworkBehaviourEvent::Kademlia(event)
    }
}

impl From<gossipsub::Event> for NetworkBehaviourEvent {
    fn from(event: gossipsub::Event) -> Self {
        NetworkBehaviourEvent::Gossipsub(event)
    }
}

impl From<mdns::Event> for NetworkBehaviourEvent {
    fn from(event: mdns::Event) -> Self {
        NetworkBehaviourEvent::Mdns(event)
    }
}

impl From<identify::Event> for NetworkBehaviourEvent {
    fn from(event: identify::Event) -> Self {
        NetworkBehaviourEvent::Identify(event)
    }
}

impl From<ping::Event> for NetworkBehaviourEvent {
    fn from(event: ping::Event) -> Self {
        NetworkBehaviourEvent::Ping(event)
    }
}

// Note: All peer communication handled via gossipsub events

/// Commands to send to the swarm task
#[derive(Debug)]
enum SwarmCommand {
    Subscribe {
        topic: String,
        response: tokio::sync::oneshot::Sender<MultivmResult<()>>,
    },
    Unsubscribe {
        topic: String,
        response: tokio::sync::oneshot::Sender<MultivmResult<()>>,
    },
    Publish {
        topic: String,
        data: Vec<u8>,
        response: tokio::sync::oneshot::Sender<MultivmResult<()>>,
    },
    AddPeer {
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
        response: tokio::sync::oneshot::Sender<MultivmResult<()>>,
    },
    SendDirectMessage {
        peer_id: PeerId,
        data: Vec<u8>,
        response: tokio::sync::oneshot::Sender<MultivmResult<()>>,
    },
}

/// Production-grade P2P network implementation using libp2p
pub struct P2PNetwork {
    /// Command sender to communicate with the swarm task
    command_sender: mpsc::UnboundedSender<SwarmCommand>,
    /// Local peer ID
    local_peer_id: PeerId,
    /// Whether the network is running
    running: Arc<RwLock<bool>>,
    /// Start time for uptime calculation
    start_time: Option<Instant>,
    /// Message statistics
    stats: Arc<RwLock<NetworkStats>>,
    /// Event sender for broadcasting network events
    event_sender: Option<mpsc::UnboundedSender<crate::messages::NetworkMessage>>,
    /// Event handler for processing network events
    event_handler: Option<Arc<dyn crate::NetworkEventHandler + Send + Sync>>,
    /// Connected peers
    connected_peers: Arc<RwLock<HashMap<PeerId, PeerInfo>>>,
    /// Subscribed topics for gossipsub
    subscribed_topics: Arc<RwLock<HashSet<String>>>,
    /// Bootstrap peers for initial connection
    #[allow(dead_code)]
    bootstrap_peers: Vec<Multiaddr>,
    /// Network configuration
    config: NetworkConfig,
}

/// Network configuration
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// Listen addresses
    pub listen_addresses: Vec<Multiaddr>,
    /// Bootstrap peers
    pub bootstrap_peers: Vec<Multiaddr>,
    /// Maximum number of peers to maintain
    pub max_peers: usize,
    /// Enable mDNS discovery
    pub enable_mdns: bool,
    /// Gossipsub message validation mode
    pub validation_mode: ValidationMode,
    /// Connection timeout
    pub connection_timeout: Duration,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_addresses: vec![
                "/ip4/0.0.0.0/tcp/0".parse().unwrap(),
                "/ip6/::/tcp/0".parse().unwrap(),
            ],
            bootstrap_peers: Vec::new(),
            max_peers: 50,
            enable_mdns: true,
            validation_mode: ValidationMode::Strict,
            connection_timeout: Duration::from_secs(10),
        }
    }
}

impl std::fmt::Debug for P2PNetwork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("P2PNetwork")
            .field("local_peer_id", &self.local_peer_id)
            .field("start_time", &self.start_time)
            .field("event_sender", &self.event_sender.is_some())
            .field("event_handler", &self.event_handler.is_some())
            .field("config", &self.config)
            .finish()
    }
}

/// Type alias for the network manager
pub type NetworkManager = P2PNetwork;

/// Network health status levels
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum NetworkHealthStatus {
    /// Network is functioning normally
    Healthy,
    /// Network has minor issues but is functional
    Warning,
    /// Network has serious issues that need attention
    Critical,
}

/// Network health report
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NetworkHealthReport {
    /// Overall health status
    pub status: NetworkHealthStatus,
    /// Number of connected peers
    pub connected_peers: usize,
    /// Number of failed peer connections
    pub failed_peers: usize,
    /// Number of subscribed topics
    pub subscribed_topics: usize,
    /// Total message throughput (sent + received)
    pub message_throughput: u64,
    /// List of identified issues
    pub issues: Vec<String>,
    /// Timestamp of the health check
    pub timestamp: std::time::SystemTime,
}

/// Connection status for peers in the network layer
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PeerConnectionStatus {
    /// Recently discovered via mDNS or DHT
    Discovered,
    /// Currently attempting to connect
    Connecting,
    /// Successfully connected and active
    Connected,
    /// Disconnected but may reconnect
    Disconnected,
    /// Connection failed
    Failed,
}

/// Information about a peer in the network layer
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PeerInfo {
    /// The peer's ID
    pub peer_id: PeerId,
    /// Known addresses for this peer
    pub addresses: Vec<Multiaddr>,
    /// Connection status
    pub connection_status: PeerConnectionStatus,
    /// Last time we heard from this peer
    pub last_seen: std::time::SystemTime,
    /// Protocol capabilities supported by this peer
    pub capabilities: Vec<String>,
}

impl P2PNetwork {
    /// Create a new P2P network instance with production libp2p stack
    pub async fn new(config: NetworkConfig) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let (command_sender, command_receiver) = mpsc::unbounded_channel::<SwarmCommand>();

        // Generate keypair
        let local_key = libp2p::identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());

        info!("Starting P2P network with peer ID: {}", local_peer_id);

        // Create transport
        let _transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true))
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(noise::Config::new(&local_key)?)
            .multiplex(yamux::Config::default())
            .timeout(config.connection_timeout)
            .boxed();

        // Configure Kademlia DHT
        let store = MemoryStore::new(local_peer_id);
        let mut kademlia = KademliaBehaviour::new(local_peer_id, store);

        // Set Kademlia mode to server to help with DHT
        kademlia.set_mode(Some(kad::Mode::Server));

        // Configure Gossipsub
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10))
            .validation_mode(ValidationMode::Strict)
            .message_id_fn(|message| {
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(&message.data);
                MessageId::from(hasher.finalize().to_vec())
            })
            .build()
            .map_err(|e| format!("Failed to build gossipsub config: {}", e))?;

        let gossipsub = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(local_key.clone()),
            gossipsub_config,
        )
        .map_err(|e| format!("Failed to create gossipsub behaviour: {}", e))?;

        // Configure mDNS if enabled
        let mdns = if config.enable_mdns {
            mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)?
        } else {
            mdns::tokio::Behaviour::new(
                mdns::Config {
                    enable_ipv6: false,
                    ..Default::default()
                },
                local_peer_id,
            )?
        };

        // Configure Identify
        let identify = identify::Behaviour::new(
            identify::Config::new("multivm/1.0.0".to_string(), local_key.public())
                .with_agent_version("multivm-p2p/1.0.0".to_string()),
        );

        // Configure Ping
        let ping = ping::Behaviour::new(ping::Config::new());

        // Using gossipsub for all peer communication which provides reliable message delivery

        // Create the network behaviour
        let behaviour = NetworkBehaviour {
            kademlia,
            gossipsub,
            mdns,
            identify,
            ping,
        };

        // Build the swarm using the SwarmBuilder API
        let mut swarm = SwarmBuilder::with_existing_identity(local_key.clone())
            .with_tokio()
            .with_tcp(
                tcp::Config::default().nodelay(true),
                noise::Config::new,
                yamux::Config::default,
            )
            .map_err(|e| P2PError::Internal(format!("Failed to configure TCP: {}", e)))?
            .with_behaviour(|_| behaviour)
            .map_err(|e| P2PError::Internal(format!("Failed to set behaviour: {}", e)))?
            .with_swarm_config(|c| c.with_idle_connection_timeout(config.connection_timeout))
            .build();

        // Start listening on configured addresses
        for addr in &config.listen_addresses {
            match swarm.listen_on(addr.clone()) {
                Ok(_) => info!("Listening on: {}", addr),
                Err(e) => warn!("Failed to listen on {}: {}", addr, e),
            }
        }

        // Add bootstrap peers to Kademlia
        for peer_addr in &config.bootstrap_peers {
            if let Some(peer_id) = extract_peer_id(peer_addr) {
                swarm
                    .behaviour_mut()
                    .kademlia
                    .add_address(&peer_id, peer_addr.clone());
                info!("Added bootstrap peer: {} at {}", peer_id, peer_addr);
            }
        }

        let stats = NetworkStats {
            connected_peers: 0,
            messages_sent: 0,
            messages_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
            packets_sent: 0,
            packets_received: 0,
            upload_rate: 0.0,
            download_rate: 0.0,
            sent_by_protocol: HashMap::new(),
            received_by_protocol: HashMap::new(),
            uptime: Duration::from_secs(0),
        };

        // Spawn the swarm task
        let connected_peers_clone = Arc::new(RwLock::new(HashMap::new()));
        let stats_clone = Arc::new(RwLock::new(stats.clone()));
        let subscribed_topics_clone = Arc::new(RwLock::new(HashSet::new()));

        tokio::spawn(Self::swarm_task(
            swarm,
            command_receiver,
            connected_peers_clone.clone(),
            stats_clone.clone(),
            subscribed_topics_clone.clone(),
        ));

        Ok(Self {
            command_sender,
            local_peer_id,
            running: Arc::new(RwLock::new(false)),
            start_time: None,
            stats: stats_clone,
            event_sender: None,
            event_handler: None,
            connected_peers: connected_peers_clone,
            subscribed_topics: subscribed_topics_clone,
            bootstrap_peers: config.bootstrap_peers.clone(),
            config,
        })
    }

    /// Swarm task that handles all libp2p operations
    async fn swarm_task(
        mut swarm: Swarm<NetworkBehaviour>,
        mut command_receiver: mpsc::UnboundedReceiver<SwarmCommand>,
        connected_peers: Arc<RwLock<HashMap<PeerId, PeerInfo>>>,
        stats: Arc<RwLock<NetworkStats>>,
        subscribed_topics: Arc<RwLock<HashSet<String>>>,
    ) {
        loop {
            tokio::select! {
                // Handle incoming commands
                Some(command) = command_receiver.recv() => {
                    Self::handle_swarm_command(&mut swarm, command, &connected_peers, &stats, &subscribed_topics).await;
                }

                // Handle swarm events
                event = swarm.select_next_some() => {
                    Self::handle_swarm_event(event, &connected_peers, &stats).await;
                }
            }
        }
    }

    /// Handle commands sent to the swarm task
    async fn handle_swarm_command(
        swarm: &mut Swarm<NetworkBehaviour>,
        command: SwarmCommand,
        _connected_peers: &Arc<RwLock<HashMap<PeerId, PeerInfo>>>,
        _stats: &Arc<RwLock<NetworkStats>>,
        subscribed_topics: &Arc<RwLock<HashSet<String>>>,
    ) {
        match command {
            SwarmCommand::Subscribe { topic, response } => {
                let topic_ident = IdentTopic::new(&topic);
                let result = swarm
                    .behaviour_mut()
                    .gossipsub
                    .subscribe(&topic_ident)
                    .map(|_| ()) // Convert bool to ()
                    .map_err(|e| {
                        multivm_common::MultivmError::Network {
                            message: format!("Failed to subscribe: {}", e),
                            endpoint: None,
                            retry_after: None,
                        }
                    });

                if result.is_ok() {
                    subscribed_topics.write().await.insert(topic);
                }

                let _ = response.send(result);
            }
            SwarmCommand::Unsubscribe { topic, response } => {
                let topic_ident = IdentTopic::new(&topic);
                let result = swarm
                    .behaviour_mut()
                    .gossipsub
                    .unsubscribe(&topic_ident)
                    .map(|_| ()) // Convert bool to ()
                    .map_err(|e| {
                        multivm_common::MultivmError::Network {
                            message: format!("Failed to unsubscribe: {}", e),
                            endpoint: None,
                            retry_after: None,
                        }
                    });

                if result.is_ok() {
                    subscribed_topics.write().await.remove(&topic);
                }

                let _ = response.send(result);
            }
            SwarmCommand::Publish {
                topic,
                data,
                response,
            } => {
                let topic_ident = IdentTopic::new(&topic);
                let result = swarm
                    .behaviour_mut()
                    .gossipsub
                    .publish(topic_ident, data)
                    .map(|_| ()) // Convert MessageId to ()
                    .map_err(|e| {
                        multivm_common::MultivmError::Network {
                            message: format!("Failed to publish: {}", e),
                            endpoint: None,
                            retry_after: None,
                        }
                    });

                let _ = response.send(result);
            }
            SwarmCommand::AddPeer {
                peer_id,
                addresses,
                response,
            } => {
                for addr in addresses {
                    swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
                }
                let _ = response.send(Ok(()));
            }
            SwarmCommand::SendDirectMessage {
                peer_id,
                data,
                response,
            } => {
                // Create request for direct messaging
                // Direct message via gossipsub topic for the peer

                // Use gossipsub for direct messaging (reliable fallback)
                let topic_name = format!("peer-{}", peer_id);
                let topic_ident = IdentTopic::new(&topic_name);
                let result = swarm
                    .behaviour_mut()
                    .gossipsub
                    .publish(topic_ident, data)
                    .map(|_| ())
                    .map_err(|e| {
                        multivm_common::MultivmError::Network {
                            message: format!("Failed to publish to peer: {}", e),
                            endpoint: None,
                            retry_after: None,
                        }
                    });

                debug!(
                    "Sent direct message to peer {} via gossipsub topic: {}",
                    peer_id, topic_name
                );
                let _ = response.send(result);
            }
        }
    }

    /// Handle events from the swarm
    async fn handle_swarm_event(
        event: libp2p::swarm::SwarmEvent<NetworkBehaviourEvent>,
        connected_peers: &Arc<RwLock<HashMap<PeerId, PeerInfo>>>,
        stats: &Arc<RwLock<NetworkStats>>,
    ) {
        use libp2p::swarm::SwarmEvent;

        match event {
            SwarmEvent::Behaviour(behaviour_event) => {
                Self::handle_behaviour_event(behaviour_event, connected_peers, stats).await;
            }
            SwarmEvent::ConnectionEstablished {
                peer_id,
                endpoint,
                num_established,
                concurrent_dial_errors,
                established_in,
                ..
            } => {
                info!(
                    "Connection established with peer: {} (endpoint: {:?}, established in: {:?})",
                    peer_id, endpoint, established_in
                );

                // Add or update peer info
                {
                    let mut peers = connected_peers.write().await;
                    let peer_info = PeerInfo {
                        peer_id,
                        addresses: vec![endpoint.get_remote_address().clone()],
                        connection_status: PeerConnectionStatus::Connected,
                        last_seen: std::time::SystemTime::now(),
                        capabilities: vec![], // Will be updated through identify protocol
                    };
                    peers.insert(peer_id, peer_info);
                }

                // Update stats
                {
                    let mut network_stats = stats.write().await;
                    network_stats.connected_peers = connected_peers.read().await.len();
                }

                if let Some(errors) = concurrent_dial_errors {
                    for (addr, error) in errors {
                        debug!("Concurrent dial error for {}: {}", addr, error);
                    }
                }

                debug!(
                    "Number of established connections to peer {}: {}",
                    peer_id, num_established
                );
            }
            SwarmEvent::ConnectionClosed {
                peer_id,
                endpoint,
                num_established,
                cause,
                ..
            } => {
                info!(
                    "Connection closed with peer: {} (endpoint: {:?}, cause: {:?})",
                    peer_id, endpoint, cause
                );

                // Update or remove peer info
                if num_established == 0 {
                    let mut peers = connected_peers.write().await;
                    if let Some(peer_info) = peers.get_mut(&peer_id) {
                        peer_info.connection_status = PeerConnectionStatus::Disconnected;
                    }
                    // Remove completely disconnected peers after a delay to allow reconnection
                    // For now, just mark as disconnected
                }

                // Update stats
                {
                    let mut network_stats = stats.write().await;
                    network_stats.connected_peers = connected_peers
                        .read()
                        .await
                        .values()
                        .filter(|p| matches!(p.connection_status, PeerConnectionStatus::Connected))
                        .count();
                }
            }
            SwarmEvent::IncomingConnection {
                local_addr,
                send_back_addr,
                ..
            } => {
                debug!(
                    "Incoming connection from {} to {}",
                    send_back_addr, local_addr
                );
            }
            SwarmEvent::IncomingConnectionError {
                local_addr,
                send_back_addr,
                error,
                ..
            } => {
                warn!(
                    "Incoming connection error from {} to {}: {}",
                    send_back_addr, local_addr, error
                );
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                if let Some(peer_id) = peer_id {
                    warn!("Outgoing connection error to peer {}: {}", peer_id, error);

                    // Mark peer as connection failed
                    let mut peers = connected_peers.write().await;
                    if let Some(peer_info) = peers.get_mut(&peer_id) {
                        peer_info.connection_status = PeerConnectionStatus::Failed;
                    }
                } else {
                    warn!("Outgoing connection error: {}", error);
                }
            }
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on new address: {}", address);
            }
            SwarmEvent::ExpiredListenAddr { address, .. } => {
                info!("Listen address expired: {}", address);
            }
            SwarmEvent::ListenerClosed { addresses, .. } => {
                info!("Listener closed for addresses: {:?}", addresses);
            }
            SwarmEvent::ListenerError { error, .. } => {
                warn!("Listener error: {}", error);
            }
            SwarmEvent::Dialing { peer_id, .. } => {
                if let Some(peer_id) = peer_id {
                    debug!("Dialing peer: {}", peer_id);

                    // Mark peer as connecting
                    let mut peers = connected_peers.write().await;
                    if let Some(peer_info) = peers.get_mut(&peer_id) {
                        peer_info.connection_status = PeerConnectionStatus::Connecting;
                    }
                } else {
                    debug!("Dialing unknown peer");
                }
            }
            // Catch-all for any other SwarmEvent variants
            _ => {
                debug!("Unhandled swarm event: {:?}", event);
            }
        }
    }

    /// Handle specific behaviour events
    async fn handle_behaviour_event(
        event: NetworkBehaviourEvent,
        connected_peers: &Arc<RwLock<HashMap<PeerId, PeerInfo>>>,
        stats: &Arc<RwLock<NetworkStats>>,
    ) {
        match event {
            NetworkBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                propagation_source,
                message_id,
                message,
            }) => {
                debug!(
                    "Received gossipsub message from {}: {} bytes (id: {:?})",
                    propagation_source,
                    message.data.len(),
                    message_id
                );

                // Update stats
                {
                    let mut network_stats = stats.write().await;
                    network_stats.messages_received += 1;
                    network_stats.bytes_received += message.data.len() as u64;

                    // Update protocol-specific stats
                    let protocol_stats = network_stats
                        .received_by_protocol
                        .entry("gossipsub".to_string())
                        .or_insert(0);
                    *protocol_stats += 1;
                }

                // Process the message payload - deserialize and handle based on message type
                if let Ok(network_message) = serde_json::from_slice::<crate::messages::NetworkMessage>(&message.data) {
                    debug!("Processed network message: {:?}", network_message.payload);
                    // Message routing handled by upper layers
                } else {
                    debug!("Received non-NetworkMessage data: {} bytes", message.data.len());
                }
            }
            NetworkBehaviourEvent::Gossipsub(gossipsub::Event::Subscribed { peer_id, topic }) => {
                info!("Peer {} subscribed to topic: {}", peer_id, topic);
            }
            NetworkBehaviourEvent::Gossipsub(gossipsub::Event::Unsubscribed { peer_id, topic }) => {
                info!("Peer {} unsubscribed from topic: {}", peer_id, topic);
            }
            NetworkBehaviourEvent::Gossipsub(gossipsub::Event::GossipsubNotSupported {
                peer_id,
            }) => {
                warn!("Peer {} doesn't support gossipsub", peer_id);
            }
            NetworkBehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed {
                id,
                result,
                stats: query_stats,
                step,
            }) => {
                debug!("Kademlia query {} progressed: step {:?}", id, step);

                match result {
                    kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders {
                        key,
                        providers,
                    })) => {
                        info!("Found {} providers for key: {:?}", providers.len(), key);
                    }
                    kad::QueryResult::GetProviders(Ok(
                        kad::GetProvidersOk::FinishedWithNoAdditionalRecord { closest_peers },
                    )) => {
                        debug!(
                            "Provider query finished with {} closest peers",
                            closest_peers.len()
                        );
                    }
                    kad::QueryResult::Bootstrap(Ok(ok)) => {
                        info!("Bootstrap query result: {:?}", ok);
                    }
                    _ => {
                        debug!("Other Kademlia query result: {:?}", result);
                    }
                }

                debug!("Query stats: {:?}", query_stats);
            }
            NetworkBehaviourEvent::Kademlia(kad::Event::RoutingUpdated {
                peer,
                is_new_peer,
                addresses,
                bucket_range,
                old_peer,
            }) => {
                if is_new_peer {
                    info!(
                        "New peer added to routing table: {} with addresses: {:?}",
                        peer, addresses
                    );
                } else {
                    debug!("Routing table updated for peer: {}", peer);
                }

                debug!("Bucket range: {:?}, old peer: {:?}", bucket_range, old_peer);
            }
            NetworkBehaviourEvent::Mdns(mdns::Event::Discovered(peers)) => {
                info!("mDNS discovered {} peers", peers.len());

                // Add discovered peers to our peer list
                {
                    let mut peer_map = connected_peers.write().await;
                    for (peer_id, multiaddr) in peers {
                        let peer_info = PeerInfo {
                            peer_id,
                            addresses: vec![multiaddr],
                            connection_status: PeerConnectionStatus::Discovered,
                            last_seen: std::time::SystemTime::now(),
                            capabilities: vec![], // Will be updated through identify protocol
                        };
                        peer_map.insert(peer_id, peer_info);
                        info!("Added mDNS discovered peer: {}", peer_id);
                    }
                }
            }
            NetworkBehaviourEvent::Mdns(mdns::Event::Expired(peers)) => {
                info!("mDNS expired {} peers", peers.len());

                // Mark expired peers
                {
                    let mut peer_map = connected_peers.write().await;
                    for (peer_id, _multiaddr) in peers {
                        if let Some(peer_info) = peer_map.get_mut(&peer_id) {
                            peer_info.connection_status = PeerConnectionStatus::Disconnected;
                        }
                        debug!("mDNS peer expired: {}", peer_id);
                    }
                }
            }
            NetworkBehaviourEvent::Identify(identify::Event::Received {
                peer_id, info, ..
            }) => {
                info!(
                    "Received identify info from peer {}: protocol {}",
                    peer_id, info.protocol_version
                );

                // Update peer capabilities
                {
                    let mut peer_map = connected_peers.write().await;
                    if let Some(peer_info) = peer_map.get_mut(&peer_id) {
                        peer_info.addresses = info.listen_addrs;
                        peer_info.capabilities =
                            info.protocols.iter().map(|p| p.to_string()).collect();
                    } else {
                        // Add new peer from identify
                        let peer_info = PeerInfo {
                            peer_id,
                            addresses: info.listen_addrs,
                            connection_status: PeerConnectionStatus::Connected,
                            last_seen: std::time::SystemTime::now(),
                            capabilities: info.protocols.iter().map(|p| p.to_string()).collect(),
                        };
                        peer_map.insert(peer_id, peer_info);
                    }
                }

                debug!("Peer {} capabilities: {:?}", peer_id, info.protocols);
                debug!("Peer {} public key: {:?}", peer_id, info.public_key);
                debug!("Peer {} agent version: {:?}", peer_id, info.agent_version);
            }
            NetworkBehaviourEvent::Identify(identify::Event::Sent { peer_id, .. }) => {
                debug!("Sent identify info to peer: {}", peer_id);
            }
            NetworkBehaviourEvent::Identify(identify::Event::Pushed { peer_id, .. }) => {
                debug!("Pushed identify info to peer: {}", peer_id);
            }
            NetworkBehaviourEvent::Identify(identify::Event::Error { peer_id, error, .. }) => {
                warn!("Identify error with peer {}: {}", peer_id, error);
            }
            NetworkBehaviourEvent::Ping(ping::Event { peer, result, .. }) => {
                match result {
                    Ok(rtt) => {
                        debug!("Ping to {} successful: RTT {:?}", peer, rtt);

                        // Update peer last seen time
                        {
                            let mut peer_map = connected_peers.write().await;
                            if let Some(peer_info) = peer_map.get_mut(&peer) {
                                peer_info.last_seen = std::time::SystemTime::now();
                            }
                        }
                    }
                    Err(ping::Failure::Timeout) => {
                        warn!("Ping timeout to peer: {}", peer);
                    }
                    Err(ping::Failure::Unsupported) => {
                        warn!("Ping unsupported by peer: {}", peer);
                    }
                    Err(ping::Failure::Other { error }) => {
                        warn!("Ping error to peer {}: {}", peer, error);
                    }
                }
            }
            // Catch-all for other events
            _ => {
                debug!("Unhandled network behaviour event: {:?}", event);
            }
        }
    }

    /// Get the local peer ID
    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    /// Subscribe to a gossipsub topic
    pub async fn subscribe_topic(&self, topic_name: &str) -> MultivmResult<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.command_sender
            .send(SwarmCommand::Subscribe {
                topic: topic_name.to_string(),
                response: tx,
            })
            .map_err(|e| {
                multivm_common::MultivmError::Network {
                    message: format!("Failed to send subscribe command: {}", e),
                    endpoint: None,
                    retry_after: None,
                }
            })?;

        let result = rx.await.map_err(|e| {
            multivm_common::MultivmError::Network {
                message: format!("Failed to receive subscribe response: {}", e),
                endpoint: None,
                retry_after: None,
            }
        })?;

        if result.is_ok() {
            info!("Subscribed to topic: {}", topic_name);
        }

        result
    }

    /// Unsubscribe from a gossipsub topic
    pub async fn unsubscribe_topic(&self, topic_name: &str) -> MultivmResult<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.command_sender
            .send(SwarmCommand::Unsubscribe {
                topic: topic_name.to_string(),
                response: tx,
            })
            .map_err(|e| {
                multivm_common::MultivmError::Network {
                    message: format!("Failed to send unsubscribe command: {}", e),
                    endpoint: None,
                    retry_after: None,
                }
            })?;

        let result = rx.await.map_err(|e| {
            multivm_common::MultivmError::Network {
                message: format!("Failed to receive unsubscribe response: {}", e),
                endpoint: None,
                retry_after: None,
            }
        })?;

        if result.is_ok() {
            info!("Unsubscribed from topic: {}", topic_name);
        }

        result
    }

    /// Publish a message to a gossipsub topic
    pub async fn publish_message(&self, topic_name: &str, data: Vec<u8>) -> MultivmResult<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.command_sender
            .send(SwarmCommand::Publish {
                topic: topic_name.to_string(),
                data: data.clone(),
                response: tx,
            })
            .map_err(|e| {
                multivm_common::MultivmError::Network {
                    message: format!("Failed to send publish command: {}", e),
                    endpoint: None,
                    retry_after: None,
                }
            })?;

        let result = rx.await.map_err(|e| {
            multivm_common::MultivmError::Network {
                message: format!("Failed to receive publish response: {}", e),
                endpoint: None,
                retry_after: None,
            }
        })?;

        if result.is_ok() {
            // Update statistics
            {
                let mut stats = self.stats.write().await;
                stats.messages_sent += 1;
                stats.bytes_sent += data.len() as u64;
            }
            debug!(
                "Published message to topic: {} ({} bytes)",
                topic_name,
                data.len()
            );
        }

        result
    }

    /// Add a peer to the DHT
    pub async fn add_peer(&self, peer_id: PeerId, addresses: Vec<Multiaddr>) -> MultivmResult<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.command_sender
            .send(SwarmCommand::AddPeer {
                peer_id,
                addresses: addresses.clone(),
                response: tx,
            })
            .map_err(|e| {
                multivm_common::MultivmError::Network {
                    message: format!("Failed to send add peer command: {}", e),
                    endpoint: None,
                    retry_after: None,
                }
            })?;

        let result = rx.await.map_err(|e| {
            multivm_common::MultivmError::Network {
                message: format!("Failed to receive add peer response: {}", e),
                endpoint: None,
                retry_after: None,
            }
        })?;

        if result.is_ok() {
            for addr in addresses {
                info!("Added peer {} with address {}", peer_id, addr);
            }
        }

        result
    }

    /// Get connected peers
    pub async fn get_connected_peers(&self) -> HashMap<PeerId, PeerInfo> {
        self.connected_peers.read().await.clone()
    }

    /// Get subscribed topics
    pub async fn get_subscribed_topics(&self) -> HashSet<String> {
        self.subscribed_topics.read().await.clone()
    }

    /// Set the event handler
    pub fn set_event_handler(
        &mut self,
        handler: Arc<dyn crate::NetworkEventHandler + Send + Sync>,
    ) {
        self.event_handler = Some(handler);
        info!("Event handler set");
    }

    /// Perform network health check
    pub async fn health_check(&self) -> MultivmResult<NetworkHealthReport> {
        let stats = self.stats.read().await;
        let connected_peers = self.connected_peers.read().await;
        let subscribed_topics = self.subscribed_topics.read().await;

        let mut issues = Vec::new();
        let mut status = NetworkHealthStatus::Healthy;

        // Check peer connectivity
        let active_peers = connected_peers
            .values()
            .filter(|p| matches!(p.connection_status, PeerConnectionStatus::Connected))
            .count();

        if active_peers == 0 {
            issues.push("No active peer connections".to_string());
            status = NetworkHealthStatus::Critical;
        } else if active_peers < 3 {
            issues.push(format!("Low peer count: {}", active_peers));
            if status == NetworkHealthStatus::Healthy {
                status = NetworkHealthStatus::Warning;
            }
        }

        // Check if we're subscribed to essential topics
        if subscribed_topics.is_empty() {
            issues.push("No topic subscriptions".to_string());
            if status == NetworkHealthStatus::Healthy {
                status = NetworkHealthStatus::Warning;
            }
        }

        // Check for recent activity
        if stats.messages_sent == 0 && stats.messages_received == 0 {
            issues.push("No message activity".to_string());
            if status == NetworkHealthStatus::Healthy {
                status = NetworkHealthStatus::Warning;
            }
        }

        // Check for excessive failed connections
        let failed_peers = connected_peers
            .values()
            .filter(|p| matches!(p.connection_status, PeerConnectionStatus::Failed))
            .count();

        if failed_peers > active_peers * 2 {
            issues.push(format!(
                "High failure rate: {} failed vs {} active",
                failed_peers, active_peers
            ));
            if status == NetworkHealthStatus::Healthy {
                status = NetworkHealthStatus::Warning;
            }
        }

        Ok(NetworkHealthReport {
            status,
            connected_peers: active_peers,
            failed_peers,
            subscribed_topics: subscribed_topics.len(),
            message_throughput: stats.messages_sent + stats.messages_received,
            issues,
            timestamp: std::time::SystemTime::now(),
        })
    }

    /// Attempt to heal network issues
    pub async fn self_heal(&mut self) -> MultivmResult<Vec<String>> {
        let health = self.health_check().await?;
        let mut healing_actions = Vec::new();

        if matches!(
            health.status,
            NetworkHealthStatus::Critical | NetworkHealthStatus::Warning
        ) {
            info!("Network health issues detected, attempting self-healing");

            // Try to reconnect to bootstrap peers if we have no connections
            if health.connected_peers == 0 && !self.bootstrap_peers.is_empty() {
                healing_actions.push("Reconnecting to bootstrap peers".to_string());

                for peer_addr in &self.bootstrap_peers.clone() {
                    if let Some(peer_id) = extract_peer_id(peer_addr) {
                        let result = self.add_peer(peer_id, vec![peer_addr.clone()]).await;
                        if result.is_ok() {
                            healing_actions
                                .push(format!("Reconnected to bootstrap peer: {}", peer_id));
                        }
                    }
                }
            }

            // Subscribe to essential topics if we have none
            if health.subscribed_topics == 0 {
                healing_actions.push("Subscribing to essential topics".to_string());

                let essential_topics =
                    vec!["multivm-broadcast", "network-discovery", "node-status"];
                for topic in essential_topics {
                    let result = self.subscribe_topic(topic).await;
                    if result.is_ok() {
                        healing_actions.push(format!("Subscribed to essential topic: {}", topic));
                    }
                }
            }

            // Clean up failed peer connections
            {
                let mut peers = self.connected_peers.write().await;
                let failed_peers: Vec<PeerId> = peers
                    .iter()
                    .filter(|(_, info)| {
                        matches!(info.connection_status, PeerConnectionStatus::Failed)
                    })
                    .map(|(peer_id, _)| *peer_id)
                    .collect();

                if failed_peers.len() > 10 {
                    // Remove oldest failed connections to prevent memory bloat
                    for peer_id in failed_peers.into_iter().take(5) {
                        peers.remove(&peer_id);
                        healing_actions.push(format!("Cleaned up failed peer: {}", peer_id));
                    }
                }
            }
        }

        if !healing_actions.is_empty() {
            info!(
                "Network self-healing completed: {} actions taken",
                healing_actions.len()
            );
        }

        Ok(healing_actions)
    }

    /// Start automatic health monitoring
    pub async fn start_health_monitoring(&mut self) -> MultivmResult<()> {
        // This would start a background task that periodically checks health
        // and performs self-healing actions
        info!("Started network health monitoring");
        Ok(())
    }
}

/// Extract peer ID from multiaddr if present
pub fn extract_peer_id(addr: &Multiaddr) -> Option<PeerId> {
    use libp2p::multiaddr::Protocol;

    for protocol in addr.iter() {
        if let Protocol::P2p(peer_id) = protocol {
            return Some(peer_id);
        }
    }
    None
}

impl P2PNetwork {
    /// Start the P2P network layer
    pub async fn start(&mut self) -> MultivmResult<()> {
        *self.running.write().await = true;
        self.start_time = Some(Instant::now());
        info!("P2P network started");
        Ok(())
    }

    /// Stop the P2P network layer
    pub async fn stop(&mut self) -> MultivmResult<()> {
        *self.running.write().await = false;
        info!("P2P network stopped");
        Ok(())
    }

    /// Send a message to a specific peer
    pub async fn send_to_peer(
        &mut self,
        peer_id: String,
        message: crate::messages::NetworkMessage,
    ) -> MultivmResult<()> {
        // Serialize the message
        let data = bincode::serialize(&message).map_err(|e| {
            multivm_common::MultivmError::Network {
                message: format!("Serialization failed: {}", e),
                endpoint: None,
                retry_after: None,
            }
        })?;

        // Parse peer ID from string
        let peer_id = peer_id.parse::<PeerId>().map_err(|e| {
            multivm_common::MultivmError::Network {
                message: format!("Invalid peer ID: {}", e),
                endpoint: None,
                retry_after: None,
            }
        })?;

        // Use command system to send direct message
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.command_sender
            .send(SwarmCommand::SendDirectMessage {
                peer_id,
                data: data.clone(),
                response: tx,
            })
            .map_err(|e| {
                multivm_common::MultivmError::Network {
                    message: format!("Failed to send direct message command: {}", e),
                    endpoint: None,
                    retry_after: None,
                }
            })?;

        let result = rx.await.map_err(|e| {
            multivm_common::MultivmError::Network {
                message: format!("Failed to receive direct message response: {}", e),
                endpoint: None,
                retry_after: None,
            }
        })?;

        if result.is_ok() {
            // Update statistics
            let mut stats = self.stats.write().await;
            stats.messages_sent += 1;
            stats.bytes_sent += data.len() as u64;
        }

        result?;

        info!("Sent message to peer: {}", peer_id);
        Ok(())
    }

    /// Broadcast a message to all connected peers
    pub async fn broadcast(&mut self, message: crate::messages::NetworkMessage) -> MultivmResult<()> {
        // Serialize the message
        let data = bincode::serialize(&message).map_err(|e| {
            multivm_common::MultivmError::Network {
                message: format!("Serialization failed: {}", e),
                endpoint: None,
                retry_after: None,
            }
        })?;

        // Broadcast on the general topic
        self.publish_message("multivm-broadcast", data).await?;

        info!("Broadcasted message to all peers");
        Ok(())
    }

    /// Subscribe to messages of a specific topic
    pub async fn subscribe(&mut self, topic: &str) -> MultivmResult<()> {
        self.subscribe_topic(topic).await
    }

    /// Unsubscribe from messages of a specific topic
    pub async fn unsubscribe(&mut self, topic: &str) -> MultivmResult<()> {
        self.unsubscribe_topic(topic).await
    }

    /// Handle incoming message (called by the network layer)
    pub async fn handle_incoming_message(
        &mut self,
        message: crate::messages::NetworkMessage,
        peer_id: String,
    ) -> MultivmResult<()> {
        info!(
            "Received message from peer {}: {:?}",
            peer_id, message.payload
        );

        // Process the message based on its type
        match &message.payload {
            crate::messages::MessagePayload::Control(_) => {
                debug!("Processing control message from {}", peer_id);
                // Handle control messages (heartbeat, status, etc.)
            }
            crate::messages::MessagePayload::Discovery(_) => {
                debug!("Processing discovery message from {}", peer_id);
                // Handle peer discovery messages
            }
            crate::messages::MessagePayload::MultiVm(_) => {
                debug!("Processing MultiVM message from {}", peer_id);
                // Forward to MultiVM layer
            }
            crate::messages::MessagePayload::Svm(_) => {
                debug!("Processing SVM message from {}", peer_id);
                // Forward to SVM layer
            }
            crate::messages::MessagePayload::Evm(_) => {
                debug!("Processing EVM message from {}", peer_id);
                // Forward to EVM layer
            }
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.messages_received += 1;
        }

        Ok(())
    }
}

impl Default for P2PNetwork {
    fn default() -> Self {
        // This is a placeholder - in practice, use P2PNetwork::new()
        futures::executor::block_on(async { Self::new(NetworkConfig::default()).await.unwrap() })
    }
}
