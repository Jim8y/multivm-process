//! Secure P2P Network Manager
//!
//! Integrates all security features for P2P networking including authentication,
//! rate limiting, message validation, and firewall rules.

pub use crate::encryption::{AuthenticationManager, EncryptionManager};

use crate::{
    config::{AuthConfig, P2PConfig, SecurityConfig as P2PSecurityConfig},
    error::{P2PError, P2PResult},
    rate_limiter::{RateLimiter, RateLimiterConfig},
    security::{MessageMetadata, SecuredMessage, SecurityConfig, SecurityManager},
};
use futures::StreamExt;
use libp2p::{
    gossipsub::{self, IdentTopic as Topic, MessageAuthenticity},
    identify::{self},
    kad::{self},
    noise,
    ping::{self},
    swarm::SwarmEvent,
    tcp, yamux, Multiaddr, PeerId, Swarm, Transport,
};
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// Secure P2P network manager
pub struct SecureNetworkManager {
    /// libp2p swarm
    swarm: Option<Swarm<SecureNetworkBehaviour>>,
    /// Rate limiter
    rate_limiter: RateLimiter,
    /// Security manager
    security_manager: SecurityManager,
    /// Trusted peers
    trusted_peers: Arc<RwLock<HashSet<PeerId>>>,
    /// Blocked peers
    blocked_peers: Arc<RwLock<HashSet<PeerId>>>,
    /// Firewall rules
    firewall: Firewall,
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

        // Initialize security manager
        let security_config = SecurityConfig {
            enable_auth: config.auth.enabled,
            enable_signing: config.auth.enabled,
            enable_replay_protection: true,
            max_message_size: config.network.max_message_size,
            message_expiry: Duration::from_secs(300),
            require_trusted_peers: config.auth.enabled,
        };
        let security_manager = SecurityManager::new(security_config);

        // Initialize firewall
        let firewall = Firewall {
            allowlist: config
                .security
                .firewall
                .allowed_ips
                .iter()
                .map(|s| s.parse().unwrap_or_else(|_| "127.0.0.1".parse().unwrap()))
                .collect(),
            blocklist: config
                .security
                .firewall
                .blocked_ips
                .iter()
                .map(|s| s.parse().unwrap_or_else(|_| "127.0.0.1".parse().unwrap()))
                .collect(),
            default_allow: config.security.firewall.default_policy
                == crate::config::FirewallPolicy::Allow,
        };

        Ok(Self {
            swarm: None,
            rate_limiter,
            security_manager,
            trusted_peers: Arc::new(RwLock::new(HashSet::new())),
            blocked_peers: Arc::new(RwLock::new(HashSet::new())),
            firewall,
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

        // Create swarm with simplified API
        let transport = tcp::Config::default()
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(noise::Config::new(&keypair).unwrap())
            .multiplex(yamux::Config::default())
            .boxed();

        let mut swarm = Swarm::new(
            transport,
            behaviour,
            peer_id,
            libp2p::swarm::Config::with_executor(Box::new(|fut| {
                tokio::spawn(fut);
            }))
        );

        // Listen on configured addresses
        for addr in &self.config.network.listen_addresses {
            let multiaddr: Multiaddr = addr.parse().map_err(|e| P2PError::ConfigurationError {
                message: format!("Invalid address {}: {}", addr, e),
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
        // Configure Gossipsub with authentication
        let gossipsub_config = libp2p::gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(1))
            .validation_mode(libp2p::gossipsub::ValidationMode::Strict)
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
        let store = libp2p::kad::store::MemoryStore::new(peer_id);
        let kademlia = kad::Behaviour::new(peer_id, store);

        // Configure Identify
        let identify = identify::Behaviour::new(libp2p::identify::Config::new(
            "/multivm/1.0".to_string(),
            keypair.public(),
        ));

        // Configure Ping
        let ping = ping::Behaviour::new(libp2p::ping::Config::new());

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
        topic: &str,
        payload: Vec<u8>,
        recipients: Option<Vec<PeerId>>,
    ) -> P2PResult<()> {
        // Check global rate limit
        self.rate_limiter.check_global_limit()?;

        // Secure the message
        let peer_id = self
            .swarm
            .as_ref()
            .map(|s| *s.local_peer_id())
            .ok_or(P2PError::NetworkNotStarted)?;

        let secured_message = self.security_manager.secure_message(payload, peer_id)?;
        let message_bytes =
            serde_json::to_vec(&secured_message).map_err(|e| P2PError::Serialization {
                message: e.to_string(),
            })?;

        // Send via gossipsub
        if let Some(swarm) = &mut self.swarm {
            let topic = Topic::new(topic);
            swarm
                .behaviour_mut()
                .gossipsub
                .publish(topic, message_bytes)
                .map_err(|e| P2PError::Libp2p {
                    message: e.to_string(),
                })?;

            self.stats.write().await.messages_sent += 1;
        }

        Ok(())
    }

    /// Add a trusted peer
    pub async fn add_trusted_peer(
        &mut self,
        peer_id: PeerId,
        public_key: ed25519_dalek::VerifyingKey,
    ) {
        self.security_manager.add_trusted_peer(peer_id, public_key);
        self.trusted_peers.write().await.insert(peer_id);
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
    fn check_firewall(&self, ip: &IpAddr) -> bool {
        // Check blocklist first
        if self.firewall.blocklist.contains(ip) {
            return false;
        }

        // Check allowlist
        if !self.firewall.allowlist.is_empty() {
            return self.firewall.allowlist.contains(ip);
        }

        // Use default policy
        self.firewall.default_allow
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
            let topic = Topic::new(topic);
            swarm
                .behaviour_mut()
                .gossipsub
                .subscribe(&topic)
                .map_err(|e| P2PError::Libp2p {
                    message: e.to_string(),
                })?;
        }
        Ok(())
    }

    /// Unsubscribe from a topic
    pub async fn unsubscribe(&mut self, topic: &str) -> P2PResult<()> {
        if let Some(swarm) = &mut self.swarm {
            let topic = Topic::new(topic);
            swarm
                .behaviour_mut()
                .gossipsub
                .unsubscribe(&topic)
                .map_err(|e| P2PError::Libp2p {
                    message: e.to_string(),
                })?;
        }
        Ok(())
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

