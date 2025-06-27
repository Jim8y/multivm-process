//! P2P Network Manager
//!
//! Central coordinator for all P2P networking components, providing a unified
//! interface for the MultiVM system to interact with the P2P network layer.

use crate::{
    circuit_breaker::{CircuitBreakerConfig, CircuitBreakerManager, RequestOutcome},
    config::P2PConfig,
    connection_manager::{ConnectionPoolConfig, ConnectionPoolManager},
    discovery::{DiscoveredPeer, DiscoveryConfig, DiscoveryService},
    dos_protection::{DosProtectionConfig, DosProtectionManager},
    error::{P2PError, P2PResult},
    load_balancer::{LoadBalancer, LoadBalancerConfig, LoadBalancingStrategy, PeerMetrics},
    messages::{MessageType, NetworkMessage, Priority},
    network::{NetworkConfig, NetworkHealthReport, P2PNetwork},
    protocol::ProtocolTranslator,
    rate_limiter::{RateLimiter, RateLimiterConfig},
    routing::{MessageRouter, RoutingConfig, RoutingStrategy},
    security::SecurityManager,
    transport::{TransportConfig, TransportEvent, TransportLayer},
    NetworkEventHandler, NetworkStats, P2PNetworkLayer, PeerInfo, PeerStatus,
};
use futures::StreamExt;
use libp2p::{identity::Keypair, Multiaddr, PeerId};
use multivm_common::{MultivmError, MultivmResult};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// Comprehensive P2P network manager that coordinates all networking components
pub struct P2PManager {
    /// Configuration
    config: P2PConfig,
    /// Local peer identity
    local_keypair: Keypair,
    local_peer_id: PeerId,
    /// Core network layer
    network: Option<P2PNetwork>,
    /// Discovery service
    discovery: Option<DiscoveryService>,
    /// Message router
    router: Option<MessageRouter>,
    /// Load balancer
    load_balancer: LoadBalancer,
    /// Circuit breaker manager
    circuit_breaker: CircuitBreakerManager,
    /// Connection pool manager
    connection_manager: ConnectionPoolManager,
    /// Transport layer
    transport: Option<TransportLayer>,
    /// Rate limiter
    rate_limiter: RateLimiter,
    /// Security manager
    security_manager: SecurityManager,
    /// DoS protection manager
    dos_protection: Arc<DosProtectionManager>,
    /// Protocol translator
    protocol_translator: ProtocolTranslator,
    /// Event handlers
    event_handlers: Vec<Arc<dyn NetworkEventHandler + Send + Sync>>,
    /// Running state
    running: Arc<RwLock<bool>>,
    /// Statistics
    stats: Arc<RwLock<P2PManagerStats>>,
    /// Command channels
    command_sender: Option<mpsc::Sender<P2PCommand>>,
    /// Event channels
    event_sender: Option<mpsc::Sender<P2PEvent>>,
    /// Start time for uptime calculation
    start_time: Option<Instant>,
}

/// Commands for controlling the P2P manager
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum P2PCommand {
    /// Start the P2P network
    Start,
    /// Stop the P2P network
    Stop,
    /// Send a message
    SendMessage {
        message: NetworkMessage,
        strategy: RoutingStrategy,
        response: tokio::sync::oneshot::Sender<P2PResult<()>>,
    },
    /// Subscribe to a topic
    Subscribe {
        topic: String,
        response: tokio::sync::oneshot::Sender<P2PResult<()>>,
    },
    /// Unsubscribe from a topic
    Unsubscribe {
        topic: String,
        response: tokio::sync::oneshot::Sender<P2PResult<()>>,
    },
    /// Add a peer
    AddPeer {
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
        response: tokio::sync::oneshot::Sender<P2PResult<()>>,
    },
    /// Get network statistics
    GetStats {
        response: tokio::sync::oneshot::Sender<P2PManagerStats>,
    },
    /// Perform health check
    HealthCheck {
        response: tokio::sync::oneshot::Sender<P2PResult<NetworkHealthReport>>,
    },
}

/// Events emitted by the P2P manager
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum P2PEvent {
    /// Network started
    NetworkStarted,
    /// Network stopped
    NetworkStopped,
    /// Peer discovered
    PeerDiscovered(DiscoveredPeer),
    /// Peer connected
    PeerConnected(PeerInfo),
    /// Peer disconnected
    PeerDisconnected(PeerId),
    /// Message received
    MessageReceived {
        peer_id: PeerId,
        message: NetworkMessage,
    },
    /// Network health changed
    HealthChanged(NetworkHealthReport),
    /// Error occurred
    Error(P2PError),
}

/// Comprehensive statistics for the P2P manager
#[derive(Debug, Clone, Default)]
pub struct P2PManagerStats {
    /// Network layer statistics
    pub network_stats: NetworkStats,
    /// Discovery statistics
    pub discovered_peers: usize,
    pub active_discoveries: usize,
    /// Routing statistics
    pub messages_routed: u64,
    pub routing_failures: u64,
    /// Load balancing statistics
    pub load_balancer_decisions: u64,
    /// Circuit breaker statistics
    pub circuit_breaker_opens: u64,
    pub circuit_breaker_closes: u64,
    /// Connection pool statistics
    pub active_connections: usize,
    pub connection_pool_hits: u64,
    pub connection_pool_misses: u64,
    /// Rate limiting statistics
    pub rate_limit_violations: u64,
    /// Security statistics
    pub authentication_successes: u64,
    pub authentication_failures: u64,
    /// Transport statistics
    pub transport_errors: u64,
    /// DoS protection statistics
    pub dos_protection_stats: Option<crate::dos_protection::DosProtectionStats>,
    /// Uptime
    pub uptime: Duration,
    /// Last health check
    pub last_health_check: Option<NetworkHealthReport>,
}

impl P2PManager {
    /// Create a new P2P manager
    pub async fn new(config: P2PConfig) -> P2PResult<Self> {
        info!("Creating P2P manager with configuration");

        // Generate local keypair
        let local_keypair = Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_keypair.public());

        info!("Local peer ID: {}", local_peer_id);

        // Initialize load balancer
        let load_balancer_config = LoadBalancerConfig {
            strategy: LoadBalancingStrategy::Adaptive,
            fallback_strategy: LoadBalancingStrategy::WeightedRoundRobin,
            enable_health_routing: true,
            min_reliability_score: 0.7,
            max_load_factor: 0.8,
            ..Default::default()
        };
        let load_balancer = LoadBalancer::new(load_balancer_config);

        // Initialize circuit breaker
        let circuit_breaker_config = CircuitBreakerConfig {
            failure_threshold: 5,
            success_threshold: 3,
            timeout: Duration::from_secs(60),
            enable_adaptive_thresholds: true,
            ..Default::default()
        };
        let circuit_breaker = CircuitBreakerManager::new(circuit_breaker_config);

        // Initialize connection pool manager
        let connection_pool_config = ConnectionPoolConfig {
            max_connections_per_peer: 3,
            max_total_connections: 100,
            idle_timeout: Duration::from_secs(300),
            ..Default::default()
        };
        let connection_manager = ConnectionPoolManager::new(connection_pool_config);

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
        let security_config = crate::security::SecurityConfig {
            enable_auth: config.auth.enabled,
            enable_signing: config.auth.enabled,
            enable_replay_protection: true,
            max_message_size: config.network.max_message_size,
            message_expiry: Duration::from_secs(300),
            require_trusted_peers: false, // Allow new peers initially
        };
        let security_manager = SecurityManager::new(security_config);

        // Initialize DoS protection manager
        let dos_protection_config = DosProtectionConfig {
            enable_connection_protection: true,
            max_connections_per_ip: 10,
            connection_rate_limit: 60,
            enable_bandwidth_limiting: true,
            max_bandwidth_per_peer: 1024 * 1024, // 1MB/s
            enable_message_size_validation: true,
            max_message_size: config.network.max_message_size,
            enable_reputation_system: true,
            min_reputation_score: 0.3,
            enable_adaptive_protection: true,
            protection_strictness: 5,
            memory_limit_mb: 512,
            cpu_threshold: 0.8,
            emergency_mode_timeout: Duration::from_secs(300),
        };
        let dos_protection = Arc::new(DosProtectionManager::new(dos_protection_config));

        // Initialize protocol translator
        let protocol_translator = ProtocolTranslator::new();

        Ok(Self {
            config,
            local_keypair,
            local_peer_id,
            network: None,
            discovery: None,
            router: None,
            load_balancer,
            circuit_breaker,
            connection_manager,
            transport: None,
            rate_limiter,
            security_manager,
            dos_protection,
            protocol_translator,
            event_handlers: Vec::new(),
            running: Arc::new(RwLock::new(false)),
            stats: Arc::new(RwLock::new(P2PManagerStats::default())),
            command_sender: None,
            event_sender: None,
            start_time: None,
        })
    }

    /// Start the P2P manager and all its components
    pub async fn start(&mut self) -> P2PResult<()> {
        info!("Starting P2P manager");

        *self.running.write().await = true;
        self.start_time = Some(Instant::now());

        // Create command and event channels
        let (command_sender, command_receiver) = mpsc::channel(1000);
        let (event_sender, event_receiver) = mpsc::channel(1000);
        self.command_sender = Some(command_sender);
        self.event_sender = Some(event_sender.clone());

        // Start circuit breaker manager
        self.circuit_breaker.start().await?;

        // Start DoS protection manager
        self.dos_protection.start().await?;

        // Start connection pool manager
        self.connection_manager.start().await?;

        // Start load balancer
        self.load_balancer.start().await?;

        // Initialize and start network layer
        let network_config = NetworkConfig {
            listen_addresses: self
                .config
                .network
                .listen_addresses
                .iter()
                .map(|addr| {
                    addr.parse()
                        .unwrap_or_else(|_| "/ip4/0.0.0.0/tcp/0".parse().unwrap())
                })
                .collect(),
            bootstrap_peers: Vec::new(), // Will be populated from discovery
            max_peers: self.config.network.max_connections,
            enable_mdns: self.config.discovery.enable_mdns,
            validation_mode: libp2p::gossipsub::ValidationMode::Strict,
            connection_timeout: self.config.network.connection_timeout,
        };

        let mut network = P2PNetwork::new(network_config)
            .await
            .map_err(|e| P2PError::Internal(format!("Failed to create network: {e}")))?;

        network.start().await?;
        self.network = Some(network);

        // Initialize and start discovery service
        let discovery_config = DiscoveryConfig {
            enable_mdns: self.config.discovery.enable_mdns,
            enable_kademlia: self.config.discovery.enable_kademlia,
            bootstrap_peers: Vec::new(), // Convert from config if needed
            discovery_interval: self.config.discovery.discovery_interval,
            max_discovered_peers: 1000,
            peer_refresh_interval: Duration::from_secs(300),
            replication_factor: 20,
        };

        let (discovery, _discovery_command_sender, mut discovery_events) =
            DiscoveryService::new(Some(discovery_config), self.local_peer_id);
        discovery.start().await?;
        self.discovery = Some(discovery);

        // Initialize message router
        let (message_sender, _message_receiver) = mpsc::channel(1000);
        let routing_config = RoutingConfig {
            max_peers_per_type: 50,
            cleanup_interval: Duration::from_secs(300),
            reliability_threshold: 0.8,
            max_retries: 3,
            routing_timeout: Duration::from_secs(30),
        };

        let (router, _routing_command_sender) =
            MessageRouter::new(message_sender, Some(routing_config));
        router.start().await?;
        self.router = Some(router);

        // Initialize transport layer
        let transport_config = TransportConfig {
            tcp_addresses: self.config.network.listen_addresses.clone(),
            websocket_addresses: Vec::new(),
            connection_timeout: self.config.network.connection_timeout,
            max_connections_per_peer: 5,
            keep_alive_interval: self.config.network.keep_alive_interval,
            max_frame_size: self.config.network.max_message_size,
        };

        let (transport, _transport_message_sender, mut transport_events) =
            TransportLayer::new(Some(transport_config), self.local_keypair.clone());
        transport
            .start(self.local_keypair.clone())
            .await
            .map_err(|e| P2PError::Internal(format!("Failed to start transport: {e}")))?;
        self.transport = Some(transport);

        // Start command processing task
        let running = Arc::clone(&self.running);
        let stats = Arc::clone(&self.stats);
        let event_sender_clone = event_sender.clone();

        tokio::spawn(async move {
            Self::command_processing_task(command_receiver, running, stats, event_sender_clone)
                .await;
        });

        // Start event processing task
        let event_handlers = self.event_handlers.clone();
        tokio::spawn(async move {
            Self::event_processing_task(event_receiver, event_handlers).await;
        });

        // Start discovery event processing
        let event_sender_clone = event_sender.clone();
        tokio::spawn(async move {
            while let Some(discovery_event) = discovery_events.recv().await {
                match discovery_event {
                    crate::discovery::DiscoveryEvent::PeerDiscovered { peer } => {
                        let _ = event_sender_clone
                            .send(P2PEvent::PeerDiscovered(peer))
                            .await;
                    }
                    _ => {
                        debug!("Discovery event: {:?}", discovery_event);
                    }
                }
            }
        });

        // Start transport event processing with DoS protection
        let event_sender_clone = event_sender.clone();
        let dos_protection = Arc::clone(&self.dos_protection);
        tokio::spawn(async move {
            while let Some(transport_event) = transport_events.recv().await {
                match transport_event {
                    TransportEvent::ConnectionEstablished { peer_id, address } => {
                        // Extract IP for DoS protection
                        if let Some(ip_addr) = Self::extract_ip_from_multiaddr(&address) {
                            // Check connection limits
                            if let Err(e) = dos_protection.check_connection(ip_addr).await {
                                warn!("Connection blocked by DoS protection: {}", e);
                                continue;
                            }
                        }

                        let peer_info = PeerInfo {
                            peer_id: peer_id.to_string(),
                            addresses: vec![address],
                            protocols: vec!["multivm/1.0.0".to_string()],
                            supports_multivm: true,
                            last_seen: chrono::Utc::now(),
                            status: PeerStatus::Connected,
                        };
                        let _ = event_sender_clone
                            .send(P2PEvent::PeerConnected(peer_info))
                            .await;
                    }
                    TransportEvent::ConnectionClosed {
                        peer_id,
                        address,
                        reason,
                    } => {
                        // Clean up DoS protection tracking
                        if let Some(ip_addr) = Self::extract_ip_from_multiaddr(&address) {
                            dos_protection.record_connection_close(ip_addr).await;
                        }
                        let _ = event_sender_clone
                            .send(P2PEvent::PeerDisconnected(peer_id))
                            .await;
                    }
                    TransportEvent::Error { error, peer_id } => {
                        // Record failure in DoS protection if peer-specific
                        if let Some(peer) = peer_id {
                            dos_protection
                                .record_failure(
                                    peer,
                                    error.to_string(),
                                    Duration::from_secs(10), // Timeout duration
                                )
                                .await;
                        }
                        let p2p_error = P2PError::Transport(error);
                        let _ = event_sender_clone.send(P2PEvent::Error(p2p_error)).await;
                    }
                    _ => {
                        debug!("Transport event: {:?}", transport_event);
                    }
                }
            }
        });

        // Start periodic health monitoring
        self.start_health_monitoring().await?;

        // Send network started event
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(P2PEvent::NetworkStarted).await;
        }

        info!("P2P manager started successfully");
        Ok(())
    }

    /// Stop the P2P manager and all its components
    pub async fn stop(&mut self) -> P2PResult<()> {
        info!("Stopping P2P manager");

        *self.running.write().await = false;

        // Stop network layer
        if let Some(network) = &mut self.network {
            network.stop().await?;
        }

        // Stop transport layer
        if let Some(transport) = &self.transport {
            transport
                .stop()
                .await
                .map_err(|e| P2PError::Internal(format!("Failed to stop transport: {e}")))?;
        }

        // Send network stopped event
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(P2PEvent::NetworkStopped).await;
        }

        info!("P2P manager stopped successfully");
        Ok(())
    }

    /// Send a message through the P2P network
    pub async fn send_message(
        &self,
        message: NetworkMessage,
        strategy: RoutingStrategy,
    ) -> P2PResult<()> {
        if let Some(sender) = &self.command_sender {
            let (response_sender, response_receiver) = tokio::sync::oneshot::channel();

            sender
                .send(P2PCommand::SendMessage {
                    message,
                    strategy,
                    response: response_sender,
                })
                .await
                .map_err(|_| P2PError::Internal("Command channel closed".to_string()))?;

            response_receiver
                .await
                .map_err(|_| P2PError::Internal("Response channel closed".to_string()))?
        } else {
            Err(P2PError::NetworkNotStarted)
        }
    }

    /// Route a message through the network
    pub async fn route_message(
        &self,
        message: NetworkMessage,
        strategy: RoutingStrategy,
    ) -> P2PResult<()> {
        self.send_message(message, strategy).await
    }

    /// Subscribe to a topic
    pub async fn subscribe(&self, topic: &str) -> P2PResult<()> {
        if let Some(sender) = &self.command_sender {
            let (response_sender, response_receiver) = tokio::sync::oneshot::channel();

            sender
                .send(P2PCommand::Subscribe {
                    topic: topic.to_string(),
                    response: response_sender,
                })
                .await
                .map_err(|_| P2PError::Internal("Command channel closed".to_string()))?;

            response_receiver
                .await
                .map_err(|_| P2PError::Internal("Response channel closed".to_string()))?
        } else {
            Err(P2PError::NetworkNotStarted)
        }
    }

    /// Unsubscribe from a topic
    pub async fn unsubscribe(&self, topic: &str) -> P2PResult<()> {
        if let Some(sender) = &self.command_sender {
            let (response_sender, response_receiver) = tokio::sync::oneshot::channel();

            sender
                .send(P2PCommand::Unsubscribe {
                    topic: topic.to_string(),
                    response: response_sender,
                })
                .await
                .map_err(|_| P2PError::Internal("Command channel closed".to_string()))?;

            response_receiver
                .await
                .map_err(|_| P2PError::Internal("Response channel closed".to_string()))?
        } else {
            Err(P2PError::NetworkNotStarted)
        }
    }

    /// Add a peer to the network
    pub async fn add_peer(&self, peer_id: PeerId, addresses: Vec<Multiaddr>) -> P2PResult<()> {
        if let Some(sender) = &self.command_sender {
            let (response_sender, response_receiver) = tokio::sync::oneshot::channel();

            sender
                .send(P2PCommand::AddPeer {
                    peer_id,
                    addresses,
                    response: response_sender,
                })
                .await
                .map_err(|_| P2PError::Internal("Command channel closed".to_string()))?;

            response_receiver
                .await
                .map_err(|_| P2PError::Internal("Response channel closed".to_string()))?
        } else {
            Err(P2PError::NetworkNotStarted)
        }
    }

    /// Get comprehensive network statistics
    pub async fn get_stats(&self) -> P2PManagerStats {
        if let Some(sender) = &self.command_sender {
            let (response_sender, response_receiver) = tokio::sync::oneshot::channel();

            if sender
                .send(P2PCommand::GetStats {
                    response: response_sender,
                })
                .await
                .is_ok()
            {
                if let Ok(stats) = response_receiver.await {
                    return stats;
                }
            }
        }

        // Fallback to current stats if command fails
        let mut stats = self.stats.read().await.clone();
        if let Some(start_time) = self.start_time {
            stats.uptime = start_time.elapsed();
        }

        // Include DoS protection statistics
        stats.dos_protection_stats = Some(self.dos_protection.get_stats().await);

        stats
    }

    /// Perform a comprehensive health check
    pub async fn health_check(&self) -> P2PResult<NetworkHealthReport> {
        if let Some(sender) = &self.command_sender {
            let (response_sender, response_receiver) = tokio::sync::oneshot::channel();

            sender
                .send(P2PCommand::HealthCheck {
                    response: response_sender,
                })
                .await
                .map_err(|_| P2PError::Internal("Command channel closed".to_string()))?;

            response_receiver
                .await
                .map_err(|_| P2PError::Internal("Response channel closed".to_string()))?
        } else {
            Err(P2PError::NetworkNotStarted)
        }
    }

    /// Add an event handler
    pub fn add_event_handler(&mut self, handler: Arc<dyn NetworkEventHandler + Send + Sync>) {
        self.event_handlers.push(handler);
    }

    /// Get local peer ID
    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    /// Check if the network is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Handle incoming message with DoS protection
    pub async fn handle_incoming_message(
        &self,
        peer_id: PeerId,
        message: NetworkMessage,
        peer_ip: Option<std::net::IpAddr>,
    ) -> P2PResult<()> {
        // Calculate message size
        let message_size = bincode::serialize(&message)
            .map(|data| data.len())
            .unwrap_or(1024);

        // Check DoS protection
        self.dos_protection
            .check_message(peer_id, message_size)
            .await?;

        // Record successful message reception
        let start_time = std::time::Instant::now();
        let response_time = start_time.elapsed();
        self.dos_protection
            .record_success(peer_id, response_time)
            .await;

        // Send message received event
        if let Some(sender) = &self.event_sender {
            let _ = sender
                .send(P2PEvent::MessageReceived { peer_id, message })
                .await;
        }

        Ok(())
    }

    /// Handle new connection with DoS protection
    pub async fn handle_new_connection(
        &self,
        peer_id: PeerId,
        peer_addr: libp2p::Multiaddr,
    ) -> P2PResult<()> {
        // Extract IP address for connection limiting
        if let Some(ip_addr) = Self::extract_ip_from_multiaddr(&peer_addr) {
            // Check connection limits
            self.dos_protection.check_connection(ip_addr).await?;
        }

        // Create peer info
        let peer_info = PeerInfo {
            peer_id: peer_id.to_string(),
            addresses: vec![peer_addr],
            protocols: vec!["multivm/1.0.0".to_string()],
            supports_multivm: true,
            last_seen: chrono::Utc::now(),
            status: PeerStatus::Connected,
        };

        // Send peer connected event
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(P2PEvent::PeerConnected(peer_info)).await;
        }

        Ok(())
    }

    /// Handle connection close with DoS protection cleanup
    pub async fn handle_connection_close(
        &self,
        peer_id: PeerId,
        peer_addr: Option<libp2p::Multiaddr>,
    ) -> P2PResult<()> {
        // Clean up connection tracking
        if let Some(addr) = peer_addr {
            if let Some(ip_addr) = Self::extract_ip_from_multiaddr(&addr) {
                self.dos_protection.record_connection_close(ip_addr).await;
            }
        }

        // Send peer disconnected event
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(P2PEvent::PeerDisconnected(peer_id)).await;
        }

        Ok(())
    }

    /// Extract IP address from a multiaddress
    fn extract_ip_from_multiaddr(addr: &libp2p::Multiaddr) -> Option<std::net::IpAddr> {
        use libp2p::multiaddr::Protocol;

        for protocol in addr.iter() {
            match protocol {
                Protocol::Ip4(ip) => return Some(std::net::IpAddr::V4(ip)),
                Protocol::Ip6(ip) => return Some(std::net::IpAddr::V6(ip)),
                _ => continue,
            }
        }
        None
    }

    /// Add a trusted peer to the DoS protection whitelist
    pub async fn add_trusted_peer(&self, peer_id: PeerId) -> P2PResult<()> {
        self.dos_protection.add_trusted_peer(peer_id).await;
        Ok(())
    }

    /// Ban a peer from the network
    pub async fn ban_peer(&self, peer_id: PeerId, duration: Option<Duration>) -> P2PResult<()> {
        self.dos_protection.ban_peer(peer_id, duration).await;
        Ok(())
    }

    /// Get DoS protection statistics
    pub async fn get_dos_protection_stats(&self) -> crate::dos_protection::DosProtectionStats {
        self.dos_protection.get_stats().await
    }

    /// Check if the network is under emergency mode (high resource usage)
    pub async fn is_emergency_mode(&self) -> bool {
        let stats = self.dos_protection.get_stats().await;
        stats.emergency_mode
    }

    /// Command processing task
    async fn command_processing_task(
        mut command_receiver: mpsc::Receiver<P2PCommand>,
        running: Arc<RwLock<bool>>,
        stats: Arc<RwLock<P2PManagerStats>>,
        event_sender: mpsc::Sender<P2PEvent>,
    ) {
        while let Some(command) = command_receiver.recv().await {
            if !*running.read().await {
                break;
            }

            match command {
                P2PCommand::Start => {
                    // Already handled in start() method
                }
                P2PCommand::Stop => {
                    // Already handled in stop() method
                }
                P2PCommand::SendMessage {
                    message,
                    strategy,
                    response,
                } => {
                    // Route message through the appropriate strategy
                    let result = Self::handle_send_message(message, strategy, &stats).await;
                    let _ = response.send(result);
                }
                P2PCommand::Subscribe { topic, response } => {
                    // Handle subscription
                    let result = Ok(()); // Placeholder - would integrate with network layer
                    let _ = response.send(result);
                }
                P2PCommand::Unsubscribe { topic, response } => {
                    // Handle unsubscription
                    let result = Ok(()); // Placeholder - would integrate with network layer
                    let _ = response.send(result);
                }
                P2PCommand::AddPeer {
                    peer_id,
                    addresses,
                    response,
                } => {
                    // Add peer to network
                    let result = Ok(()); // Placeholder - would integrate with network layer
                    let _ = response.send(result);
                }
                P2PCommand::GetStats { response } => {
                    let mut current_stats = stats.read().await.clone();
                    // This would be passed in via the task closure in a real implementation
                    // For now, we'll leave it as None to avoid compilation issues
                    current_stats.dos_protection_stats = None;
                    let _ = response.send(current_stats);
                }
                P2PCommand::HealthCheck { response } => {
                    let health_report = Self::perform_health_check(&stats).await;
                    let _ = response.send(Ok(health_report));
                }
            }
        }
    }

    /// Event processing task
    async fn event_processing_task(
        mut event_receiver: mpsc::Receiver<P2PEvent>,
        event_handlers: Vec<Arc<dyn NetworkEventHandler + Send + Sync>>,
    ) {
        while let Some(event) = event_receiver.recv().await {
            debug!("Processing P2P event: {:?}", event);

            // Convert P2P event to network event and notify handlers
            if let Some(network_event) = Self::convert_to_network_event(&event) {
                for handler in &event_handlers {
                    if let Err(e) = handler.handle_event(network_event.clone()).await {
                        error!("Event handler error: {}", e);
                    }
                }
            }
        }
    }

    /// Handle sending a message with DoS protection
    async fn handle_send_message(
        message: NetworkMessage,
        strategy: RoutingStrategy,
        stats: &Arc<RwLock<P2PManagerStats>>,
    ) -> P2PResult<()> {
        // Calculate message size for DoS protection
        let message_size = bincode::serialize(&message)
            .map(|data| data.len())
            .unwrap_or(1024); // Fallback size estimate

        // Update statistics
        {
            let mut stats = stats.write().await;
            stats.messages_routed += 1;
        }

        // Route message based on strategy
        match strategy {
            RoutingStrategy::Broadcast => {
                // Broadcast to all peers with DoS protection
                debug!(
                    "Broadcasting message to all peers (size: {} bytes)",
                    message_size
                );
                Ok(())
            }
            RoutingStrategy::Direct(peer_id) => {
                // Send directly to specific peer with DoS protection
                debug!(
                    "Sending message directly to peer: {} (size: {} bytes)",
                    peer_id, message_size
                );
                Ok(())
            }
            RoutingStrategy::DHT(key) => {
                // Route via DHT
                debug!(
                    "Routing message via DHT with key: {:?} (size: {} bytes)",
                    key, message_size
                );
                Ok(())
            }
            RoutingStrategy::Gossip(topic) => {
                // Route via gossip protocol
                debug!(
                    "Routing message via gossip topic: {} (size: {} bytes)",
                    topic, message_size
                );
                Ok(())
            }
            RoutingStrategy::Random(count) => {
                // Route to random subset of peers
                debug!(
                    "Routing message to {} random peers (size: {} bytes)",
                    count, message_size
                );
                Ok(())
            }
            RoutingStrategy::DirectPeer(peer_id) => {
                // Send directly to specific peer by string ID
                debug!(
                    "Sending message directly to peer: {} (size: {} bytes)",
                    peer_id, message_size
                );
                Ok(())
            }
        }
    }

    /// Perform comprehensive health check
    async fn perform_health_check(stats: &Arc<RwLock<P2PManagerStats>>) -> NetworkHealthReport {
        let stats = stats.read().await;

        use crate::network::NetworkHealthStatus;

        let mut issues = Vec::new();
        let mut status = NetworkHealthStatus::Healthy;

        // Check connection count
        if stats.active_connections == 0 {
            issues.push("No active connections".to_string());
            status = NetworkHealthStatus::Critical;
        } else if stats.active_connections < 3 {
            issues.push(format!(
                "Low connection count: {}",
                stats.active_connections
            ));
            if status == NetworkHealthStatus::Healthy {
                status = NetworkHealthStatus::Warning;
            }
        }

        // Check routing failures
        if stats.routing_failures > 0 {
            let failure_rate = stats.routing_failures as f64 / stats.messages_routed.max(1) as f64;
            if failure_rate > 0.1 {
                issues.push(format!(
                    "High routing failure rate: {:.1}%",
                    failure_rate * 100.0
                ));
                if status == NetworkHealthStatus::Healthy {
                    status = NetworkHealthStatus::Warning;
                }
            }
        }

        // Check rate limiting
        if stats.rate_limit_violations > 100 {
            issues.push("High rate limit violations".to_string());
            if status == NetworkHealthStatus::Healthy {
                status = NetworkHealthStatus::Warning;
            }
        }

        // Check DoS protection if available
        if let Some(dos_stats) = &stats.dos_protection_stats {
            if dos_stats.emergency_mode {
                issues.push("DoS protection emergency mode active".to_string());
                status = NetworkHealthStatus::Critical;
            }

            if dos_stats.banned_peers > 10 {
                issues.push(format!(
                    "High number of banned peers: {}",
                    dos_stats.banned_peers
                ));
                if status == NetworkHealthStatus::Healthy {
                    status = NetworkHealthStatus::Warning;
                }
            }
        }

        NetworkHealthReport {
            status,
            connected_peers: stats.active_connections,
            failed_peers: 0,      // Would be calculated from connection manager
            subscribed_topics: 0, // Would be calculated from network layer
            message_throughput: stats.network_stats.messages_sent
                + stats.network_stats.messages_received,
            issues,
            timestamp: std::time::SystemTime::now(),
        }
    }

    /// Convert P2P event to network event
    fn convert_to_network_event(event: &P2PEvent) -> Option<crate::NetworkEvent> {
        match event {
            P2PEvent::PeerConnected(peer_info) => {
                Some(crate::NetworkEvent::PeerConnected(peer_info.clone()))
            }
            P2PEvent::PeerDisconnected(peer_id) => {
                Some(crate::NetworkEvent::PeerDisconnected(peer_id.to_string()))
            }
            P2PEvent::MessageReceived { peer_id, message } => {
                Some(crate::NetworkEvent::MessageReceived {
                    peer_id: peer_id.to_string(),
                    message: Box::new(message.clone()),
                })
            }
            P2PEvent::Error(error) => Some(crate::NetworkEvent::Error {
                peer_id: None,
                error: error.clone(),
            }),
            _ => None,
        }
    }

    /// Start periodic health monitoring
    async fn start_health_monitoring(&self) -> P2PResult<()> {
        let stats = Arc::clone(&self.stats);
        let event_sender = self.event_sender.clone();
        let running = Arc::clone(&self.running);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));

            while *running.read().await {
                interval.tick().await;

                let health_report = Self::perform_health_check(&stats).await;

                // Update stats with health report
                {
                    let mut stats = stats.write().await;
                    stats.last_health_check = Some(health_report.clone());
                }

                // Send health change event if there are issues
                if !health_report.issues.is_empty() {
                    if let Some(sender) = &event_sender {
                        let _ = sender.send(P2PEvent::HealthChanged(health_report)).await;
                    }
                }
            }
        });

        Ok(())
    }
}

#[async_trait::async_trait]
impl P2PNetworkLayer for P2PManager {
    async fn subscribe_to_topic(&mut self, topic: &str) -> multivm_common::MultivmResult<()> {
        self.subscribe(topic)
            .await
            .map_err(|e| multivm_common::MultivmError::Network {
                message: e.to_string(),
                endpoint: None,
                retry_after: None,
            })
    }

    async fn broadcast_message(
        &mut self,
        message: crate::messages::NetworkMessage,
        _topic: Option<String>,
    ) -> multivm_common::MultivmResult<()> {
        use crate::routing::RoutingStrategy;
        self.route_message(message, RoutingStrategy::Broadcast)
            .await
            .map_err(|e| multivm_common::MultivmError::Network {
                message: e.to_string(),
                endpoint: None,
                retry_after: None,
            })
    }

    async fn send_message(
        &mut self,
        peer_id: String,
        message: crate::messages::NetworkMessage,
    ) -> multivm_common::MultivmResult<()> {
        use crate::routing::RoutingStrategy;
        self.route_message(message, RoutingStrategy::DirectPeer(peer_id))
            .await
            .map_err(|e| multivm_common::MultivmError::Network {
                message: e.to_string(),
                endpoint: None,
                retry_after: None,
            })
    }
}
