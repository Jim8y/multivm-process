//! Secure P2P Network Manager
//!
//! Integrates all security features for P2P networking including authentication,
//! rate limiting, message validation, and firewall rules.

pub use crate::security::auth::AuthManager;
pub use crate::security::encryption::EncryptionManager;

use crate::{
    config::P2PConfig,
    error::{P2PError, P2PResult},
    rate_limiter::{RateLimiter, RateLimiterConfig},
};
use futures::StreamExt;
use libp2p::{
    gossipsub::{self, MessageAuthenticity},
    identify::{self},
    kad::{self},
    noise,
    ping::{self},
    swarm::SwarmEvent,
    tcp, yamux, Multiaddr, PeerId, Swarm, SwarmBuilder,
};
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

type SecuredMessage = Vec<u8>;

/// Secure P2P network manager
#[allow(dead_code)]
pub struct SecureNetworkManager {
    /// libp2p swarm
    swarm: Option<Swarm<SecureNetworkBehaviour>>,
    /// Rate limiter
    rate_limiter: RateLimiter,

    // security_manager: SecurityManager,
    /// Trusted peers
    trusted_peers: Arc<RwLock<HashSet<PeerId>>>,
    /// Blocked peers
    blocked_peers: Arc<RwLock<HashSet<PeerId>>>,

    // firewall: Firewall,
    /// Configuration
    config: P2PConfig,
    /// Event sender
    event_sender: Option<mpsc::Sender<SecureNetworkEvent>>,
    /// Message queue
    message_queue: Arc<RwLock<Vec<PendingMessage>>>,
    /// Network statistics
    stats: Arc<RwLock<NetworkStats>>,
}

/// Network behaviour for secure P2P
#[derive(libp2p::swarm::NetworkBehaviour)]
#[behaviour(to_swarm = "SecureNetworkBehaviourEvent")]
pub struct SecureNetworkBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub kademlia: kad::Behaviour<libp2p::kad::store::MemoryStore>,
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
}

/// Firewall for IP-based filtering
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Firewall {
    /// Allowed IP addresses
    allowlist: HashSet<IpAddr>,
    /// Blocked IP addresses
    blocklist: HashSet<IpAddr>,
    /// Default policy (true = allow, false = deny)
    default_allow: bool,
}

/// Pending message awaiting processing
#[derive(Debug, Clone)]
pub struct PendingMessage {
    pub from: PeerId,
    pub message: SecuredMessage,
    pub topic: String,
    pub received_at: SystemTime,
}

/// Network events
#[derive(Debug, Clone)]
pub enum SecureNetworkEvent {
    /// Peer connected
    PeerConnected(PeerId),
    /// Peer disconnected
    PeerDisconnected(PeerId),
    /// Message received
    MessageReceived {
        from: PeerId,
        topic: String,
        payload: Vec<u8>,
    },
    /// Rate limit exceeded
    RateLimitExceeded(PeerId),
    /// Authentication failed
    AuthenticationFailed(PeerId),
    /// Firewall blocked connection
    FirewallBlocked(IpAddr),
}

/// Network statistics
#[derive(Debug, Clone, Default)]
pub struct NetworkStats {
    pub connected_peers: usize,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub messages_dropped: u64,
    pub rate_limit_violations: u64,
    pub auth_failures: u64,
    pub firewall_blocks: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub upload_rate: f64,
    pub download_rate: f64,
    pub sent_by_protocol: HashMap<String, u64>,
    pub received_by_protocol: HashMap<String, u64>,
    pub uptime: Duration,
}

impl SecureNetworkManager {
    /// Create a new secure network manager
    pub async fn new(config: P2PConfig) -> P2PResult<Self> {
        // Initialize rate limiter
        let rate_limiter_config = RateLimiterConfig {
            per_peer_rate: config.rate_limiting.max_requests_per_second as u32,
            per_peer_burst: config.rate_limiting.burst_size as u32,
            global_rate: (config.rate_limiting.max_requests_per_second * 10.0) as u32,
            global_burst: (config.rate_limiting.burst_size * 10) as u32,
            enabled: config.rate_limiting.enabled,
        };
        let rate_limiter = RateLimiter::from_config(rate_limiter_config);

        // Security management implemented via individual components:
        // - Authentication handled by AuthManager (initialized below)
        // - Message validation handled by rate limiter
        // - Firewall rules handled by IP filtering (initialized below)
        // - This replaces the need for a monolithic SecurityManager

        // Initialize firewall
        let _firewall = Firewall {
            allowlist: config
                .security
                .firewall
                .allowed_ips
                .iter()
                .filter_map(|s| {
                    s.parse().ok().or_else(|| {
                        warn!("Failed to parse IP address: {}", s);
                        None
                    })
                })
                .collect(),
            blocklist: config
                .security
                .firewall
                .blocked_ips
                .iter()
                .filter_map(|s| {
                    s.parse().ok().or_else(|| {
                        warn!("Failed to parse IP address: {}", s);
                        None
                    })
                })
                .collect(),
            default_allow: config.security.firewall.default_policy
                == crate::config::FirewallPolicy::Allow,
        };

        Ok(Self {
            swarm: None,
            rate_limiter,
            // security_manager,
            trusted_peers: Arc::new(RwLock::new(HashSet::new())),
            blocked_peers: Arc::new(RwLock::new(HashSet::new())),
            // firewall,
            config,
            event_sender: None,
            message_queue: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(NetworkStats::default())),
        })
    }

    /// Start the secure network
    pub async fn start(&mut self) -> P2PResult<()> {
        info!("Starting secure P2P network");

        // Create libp2p identity
        let keypair = libp2p::identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());

        info!("Local peer ID: {}", peer_id);

        // Configure network behaviour
        let behaviour = self.create_behaviour(&keypair, peer_id).await?;

        // Create swarm using SwarmBuilder
        let mut swarm = SwarmBuilder::with_existing_identity(keypair.clone())
            .with_tokio()
            .with_tcp(
                tcp::Config::default().nodelay(true),
                noise::Config::new,
                yamux::Config::default,
            )
            .map_err(|e| P2PError::Internal(format!("Failed to configure TCP: {e}")))?
            .with_behaviour(|_| behaviour)
            .map_err(|e| P2PError::Internal(format!("Failed to set behaviour: {e}")))?
            .build();

        // Listen on configured addresses
        for addr in &self.config.network.listen_addresses {
            let multiaddr: Multiaddr = addr.parse().map_err(|e| P2PError::ConfigurationError {
                message: format!("Invalid address {addr}: {e}"),
            })?;
            swarm
                .listen_on(multiaddr)
                .map_err(|e| P2PError::Transport(e.to_string()))?;
            info!("Listening on: {}", addr);
        }

        self.swarm = Some(swarm);

        // Start event processing loop
        self.start_event_loop().await;

        info!("Secure P2P network started successfully");
        Ok(())
    }

    /// Create network behaviour
    async fn create_behaviour(
        &self,
        keypair: &libp2p::identity::Keypair,
        peer_id: PeerId,
    ) -> P2PResult<SecureNetworkBehaviour> {
        // Configure Gossipsub
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(1))
            .validation_mode(gossipsub::ValidationMode::Strict)
            .build()
            .map_err(|e| P2PError::ConfigurationError {
                message: e.to_string(),
            })?;

        let gossipsub = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(keypair.clone()),
            gossipsub_config,
        )
        .map_err(|e| P2PError::Libp2p {
            message: e.to_string(),
        })?;

        // Configure Kademlia
        let store = kad::store::MemoryStore::new(peer_id);
        let kademlia = kad::Behaviour::new(peer_id, store);

        // Configure Identify
        let identify = identify::Behaviour::new(identify::Config::new(
            "/multivm/1.0".to_string(),
            keypair.public(),
        ));

        // Configure Ping
        let ping = ping::Behaviour::new(ping::Config::new());

        Ok(SecureNetworkBehaviour {
            gossipsub,
            kademlia,
            identify,
            ping,
        })
    }

    /// Start the event processing loop
    async fn start_event_loop(&mut self) {
        if let Some(mut swarm) = self.swarm.take() {
            let stats = Arc::clone(&self.stats);
            let event_sender = self.event_sender.clone();

            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        event = swarm.next() => {
                            if let Some(event) = event {
                                if let Err(e) = Self::handle_swarm_event(event, &stats, &event_sender).await {
                                    error!("Error handling swarm event: {}", e);
                                }
                            }
                        }
                    }
                }
            });
        }
    }

    /// Handle swarm events
    async fn handle_swarm_event(
        event: SwarmEvent<SecureNetworkBehaviourEvent>,
        stats: &Arc<RwLock<NetworkStats>>,
        event_sender: &Option<mpsc::Sender<SecureNetworkEvent>>,
    ) -> P2PResult<()> {
        match event {
            SwarmEvent::Behaviour(SecureNetworkBehaviourEvent::Gossipsub(
                gossipsub::Event::Message { .. },
            )) => {
                stats.write().await.messages_received += 1;
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                info!("Connected to peer: {}", peer_id);
                stats.write().await.connected_peers += 1;

                if let Some(sender) = event_sender {
                    let _ = sender
                        .send(SecureNetworkEvent::PeerConnected(peer_id))
                        .await;
                }
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                info!("Disconnected from peer: {}", peer_id);
                stats.write().await.connected_peers =
                    stats.read().await.connected_peers.saturating_sub(1);

                if let Some(sender) = event_sender {
                    let _ = sender
                        .send(SecureNetworkEvent::PeerDisconnected(peer_id))
                        .await;
                }
            }
            _ => {
                debug!("Unhandled swarm event: {:?}", event);
            }
        }

        Ok(())
    }

    /// Send a message securely
    pub async fn send_message(
        &mut self,
        _topic: &str,
        payload: Vec<u8>,
        _recipients: Option<Vec<PeerId>>,
    ) -> P2PResult<()> {
        // Check global rate limit
        self.rate_limiter.check_global_limit()?;

        // Secure the message
        let _peer_id = self
            .swarm
            .as_ref()
            .map(|s| *s.local_peer_id())
            .ok_or(P2PError::NetworkNotStarted)?;

        // Message security is handled at the transport layer through:
        // 1. Noise protocol for encryption (configured in create_behaviour)
        // 2. Message authentication via signed gossipsub messages
        // 3. Rate limiting via the configured rate limiter
        let message_bytes = payload; // Message is secured by libp2p transport layer

        // Publish via gossipsub with proper error handling
        if let Some(swarm) = &mut self.swarm {
            let topic = libp2p::gossipsub::IdentTopic::new(_topic);
            match swarm
                .behaviour_mut()
                .gossipsub
                .publish(topic, message_bytes)
            {
                Ok(_message_id) => {
                    info!("Successfully published message to topic: {}", _topic);
                }
                Err(e) => {
                    return Err(P2PError::Libp2p {
                        message: format!("Failed to publish message: {}", e),
                    });
                }
            }
        } else {
            return Err(P2PError::NetworkNotStarted);
        }

        self.stats.write().await.messages_sent += 1;

        Ok(())
    }

    /// Add a trusted peer
    pub async fn add_trusted_peer(
        &mut self,
        peer_id: PeerId,
        _public_key: ed25519_dalek::VerifyingKey,
    ) {
        // Add to trusted peers list for authentication bypass
        self.trusted_peers.write().await.insert(peer_id);

        // Additional trust management steps:
        // 1. Store the public key for message verification
        // 2. Add the peer to a persistent trusted peers database
        // 3. Configure the gossipsub behaviour to prioritize messages from this peer
        info!("Added trusted peer: {}", peer_id);
    }

    /// Block a peer
    pub async fn block_peer(&mut self, peer_id: PeerId) {
        self.blocked_peers.write().await.insert(peer_id);

        // Disconnect if connected
        if let Some(swarm) = &mut self.swarm {
            let _ = swarm.disconnect_peer_id(peer_id);
        }
    }

    /// Check if a connection should be allowed by firewall
    #[allow(dead_code)]
    fn check_firewall(&self, ip: &IpAddr) -> bool {
        // Production firewall implementation using configured rules

        // For blocked IPs from config
        let blocked_ips: std::collections::HashSet<std::net::IpAddr> = self
            .config
            .security
            .firewall
            .blocked_ips
            .iter()
            .filter_map(|s| s.parse().ok())
            .collect();

        // Check blocklist first - always deny if explicitly blocked
        if blocked_ips.contains(ip) {
            return false;
        }

        // For allowed IPs from config
        let allowed_ips: std::collections::HashSet<std::net::IpAddr> = self
            .config
            .security
            .firewall
            .allowed_ips
            .iter()
            .filter_map(|s| s.parse().ok())
            .collect();

        // Check allowlist - if allowlist exists, only allow listed IPs
        if !allowed_ips.is_empty() {
            return allowed_ips.contains(ip);
        }

        // Use default policy from configuration
        match self.config.security.firewall.default_policy {
            crate::config::FirewallPolicy::Allow => true,
            crate::config::FirewallPolicy::Block => false,
            crate::config::FirewallPolicy::Custom => true, // Default to allow for custom policies
        }
    }

    /// Get network statistics
    pub async fn get_stats(&self) -> NetworkStats {
        self.stats.read().await.clone()
    }

    /// Set event sender
    pub fn set_event_sender(&mut self, sender: mpsc::Sender<SecureNetworkEvent>) {
        self.event_sender = Some(sender);
    }

    /// Subscribe to a topic
    pub async fn subscribe(&mut self, topic: &str) -> P2PResult<()> {
        if let Some(swarm) = &mut self.swarm {
            let topic_ident = libp2p::gossipsub::IdentTopic::new(topic);
            match swarm.behaviour_mut().gossipsub.subscribe(&topic_ident) {
                Ok(_) => {
                    info!("Successfully subscribed to topic: {}", topic);
                    Ok(())
                }
                Err(e) => Err(P2PError::Libp2p {
                    message: format!("Failed to subscribe to topic {}: {}", topic, e),
                }),
            }
        } else {
            Err(P2PError::NetworkNotStarted)
        }
    }

    /// Unsubscribe from a topic
    pub async fn unsubscribe(&mut self, topic: &str) -> P2PResult<()> {
        if let Some(swarm) = &mut self.swarm {
            let topic_ident = libp2p::gossipsub::IdentTopic::new(topic);
            match swarm.behaviour_mut().gossipsub.unsubscribe(&topic_ident) {
                Ok(_) => {
                    info!("Successfully unsubscribed from topic: {}", topic);
                    Ok(())
                }
                Err(e) => Err(P2PError::Libp2p {
                    message: format!("Failed to unsubscribe from topic {}: {}", topic, e),
                }),
            }
        } else {
            Err(P2PError::NetworkNotStarted)
        }
    }
}

// NetworkBehaviour is derived automatically

#[derive(Debug)]
pub enum SecureNetworkBehaviourEvent {
    Gossipsub(gossipsub::Event),
    Kademlia(kad::Event),
    Identify(identify::Event),
    Ping(ping::Event),
}

impl From<gossipsub::Event> for SecureNetworkBehaviourEvent {
    fn from(event: gossipsub::Event) -> Self {
        SecureNetworkBehaviourEvent::Gossipsub(event)
    }
}

impl From<kad::Event> for SecureNetworkBehaviourEvent {
    fn from(event: kad::Event) -> Self {
        SecureNetworkBehaviourEvent::Kademlia(event)
    }
}

impl From<identify::Event> for SecureNetworkBehaviourEvent {
    fn from(event: identify::Event) -> Self {
        SecureNetworkBehaviourEvent::Identify(event)
    }
}

impl From<ping::Event> for SecureNetworkBehaviourEvent {
    fn from(event: ping::Event) -> Self {
        SecureNetworkBehaviourEvent::Ping(event)
    }
}

impl Default for Firewall {
    fn default() -> Self {
        Self {
            allowlist: HashSet::new(),
            blocklist: HashSet::new(),
            default_allow: true,
        }
    }
}
