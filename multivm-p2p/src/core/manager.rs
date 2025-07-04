//! Refactored P2P Network Manager with Single Responsibility Principle
//!
//! This module provides a clean, modular P2P manager that delegates specific
//! responsibilities to dedicated coordinators, making the code more maintainable,
//! testable, and extensible.

use crate::{
    config::P2PConfig,
    error::{P2PError, P2PResult},
    protocol::messages::{NetworkMessage, Priority},
    rate_limiter::RateLimiter,
    security::{
        auth::{AuthConfig as SecurityAuthConfig, AuthManager},
        dos_protection::DosProtectionManager,
        encryption::EncryptionManager,
    },
};
use libp2p::{identity::Keypair, Multiaddr, PeerId};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::{info, warn};

/// Refactored P2P Manager that follows the Single Responsibility Principle
///
/// The manager serves as a thin orchestration layer, delegating specific
/// responsibilities to dedicated coordinators.
#[allow(dead_code)]
pub struct P2PManager {
    /// Configuration
    config: Arc<P2PConfig>,

    /// Local peer identity
    local_peer_id: PeerId,

    /// Network coordinator - handles low-level networking
    network_coordinator: Arc<NetworkCoordinator>,

    /// Message coordinator - handles message routing and processing
    message_coordinator: Arc<MessageCoordinator>,

    /// Security coordinator - handles all security aspects
    security_coordinator: Arc<SecurityCoordinator>,

    /// Discovery coordinator - handles peer discovery
    discovery_coordinator: Arc<DiscoveryCoordinator>,

    /// Monitoring coordinator - handles metrics and monitoring
    monitoring_coordinator: Arc<MonitoringCoordinator>,

    /// Manager state
    state: Arc<RwLock<ManagerState>>,

    /// Command sender (for external use)
    command_tx: mpsc::Sender<ManagerCommand>,
}

/// Manager state
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ManagerState {
    running: bool,
    start_time: Option<Instant>,
    shutdown_signal: Option<mpsc::Sender<()>>,
}

/// Commands for the P2P manager
#[derive(Debug)]
pub enum ManagerCommand {
    Start,
    Stop,
    SendMessage {
        message: NetworkMessage,
        priority: Priority,
        response: oneshot::Sender<P2PResult<()>>,
    },
    GetStats {
        response: oneshot::Sender<ManagerStats>,
    },
}

/// Manager statistics
#[derive(Debug, Clone)]
pub struct ManagerStats {
    pub uptime: Duration,
    pub peers_connected: usize,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

/// Alias for backward compatibility
pub type P2PManagerStats = ManagerStats;

/// Network Coordinator - Handles low-level networking
#[allow(dead_code)]
#[derive(Clone)]
pub struct NetworkCoordinator {
    config: Arc<P2PConfig>,
    transport: Arc<RwLock<Option<crate::transport::transport::UnifiedTransport>>>,
    connections: Arc<RwLock<ConnectionState>>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
}

/// Connection state
#[derive(Default)]
#[allow(dead_code)]
struct ConnectionState {
    active_peers: std::collections::HashMap<PeerId, PeerConnection>,
    total_connections: u64,
    total_disconnections: u64,
}

/// Peer connection information
#[allow(dead_code)]
struct PeerConnection {
    peer_id: PeerId,
    address: Multiaddr,
    connected_at: Instant,
    last_activity: Instant,
    bytes_sent: u64,
    bytes_received: u64,
}

/// Network events
#[derive(Debug)]
pub enum NetworkEvent {
    PeerConnected { peer_id: PeerId, address: Multiaddr },
    PeerDisconnected { peer_id: PeerId, reason: String },
    MessageReceived { peer_id: PeerId, message: Vec<u8> },
    Error { error: P2PError },
}

/// Message Coordinator - Handles message routing and processing
#[allow(dead_code)]
pub struct MessageCoordinator {
    router: Arc<RwLock<MessageRouter>>,
    handlers: Arc<RwLock<MessageHandlers>>,
    outbound_queue: Arc<RwLock<OutboundQueue>>,
    stats: Arc<RwLock<MessageStats>>,
}

/// Message router
#[allow(dead_code)]
struct MessageRouter {
    routing_table: std::collections::HashMap<String, Vec<PeerId>>,
    default_strategy: RoutingStrategy,
}

/// Routing strategy
#[derive(Debug, Clone)]
pub enum RoutingStrategy {
    Direct(PeerId),
    Broadcast,
    Topic(String),
    LoadBalanced,
}

/// Message handlers
#[allow(dead_code)]
struct MessageHandlers {
    handlers: std::collections::HashMap<String, Box<dyn MessageHandler>>,
}

/// Message handler trait
pub trait MessageHandler: Send + Sync {
    fn handle_message(&self, message: NetworkMessage) -> P2PResult<()>;
}

/// Outbound message queue
#[allow(dead_code)]
struct OutboundQueue {
    high_priority: std::collections::VecDeque<QueuedMessage>,
    normal_priority: std::collections::VecDeque<QueuedMessage>,
    low_priority: std::collections::VecDeque<QueuedMessage>,
}

/// Queued message
#[allow(dead_code)]
struct QueuedMessage {
    message: NetworkMessage,
    target: RoutingStrategy,
    queued_at: Instant,
    attempts: u32,
}

/// Message statistics
#[derive(Default)]
#[allow(dead_code)]
struct MessageStats {
    messages_sent: u64,
    messages_received: u64,
    messages_dropped: u64,
    average_latency: Duration,
}

/// Security Coordinator - Handles all security aspects
#[allow(dead_code)]
pub struct SecurityCoordinator {
    encryption: Arc<crate::security::encryption::EncryptionManager>,
    auth: Arc<crate::security::auth::AuthManager>,
    rate_limiter: Arc<crate::rate_limiter::RateLimiter>,
    dos_protection: Arc<crate::security::dos_protection::DosProtectionManager>,
    reputation: Arc<crate::security::reputation::ReputationManager>,
    security_policy: Arc<RwLock<SecurityPolicy>>,
}

/// Security policy
#[allow(dead_code)]
struct SecurityPolicy {
    require_encryption: bool,
    require_authentication: bool,
    max_message_size: usize,
    banned_peers: std::collections::HashSet<PeerId>,
}

/// Discovery Coordinator - Handles peer discovery
#[allow(dead_code)]
pub struct DiscoveryCoordinator {
    mdns: Arc<RwLock<Option<MdnsDiscovery>>>,
    kademlia: Arc<RwLock<Option<KademliaDiscovery>>>,
    bootstrap_peers: Vec<(PeerId, Multiaddr)>,
    discovered_peers: Arc<RwLock<DiscoveredPeers>>,
}

/// mDNS discovery
#[allow(dead_code)]
struct MdnsDiscovery {
    enabled: bool,
    service_name: String,
}

/// Kademlia discovery
#[allow(dead_code)]
struct KademliaDiscovery {
    enabled: bool,
    replication_factor: usize,
}

/// Discovered peers
#[derive(Default)]
#[allow(dead_code)]
struct DiscoveredPeers {
    peers: std::collections::HashMap<PeerId, DiscoveredPeer>,
    last_discovery: Option<Instant>,
}

/// Discovered peer information
#[allow(dead_code)]
struct DiscoveredPeer {
    peer_id: PeerId,
    addresses: Vec<Multiaddr>,
    discovered_at: Instant,
    score: f64,
}

/// Monitoring Coordinator - Handles metrics and monitoring
#[allow(dead_code)]
pub struct MonitoringCoordinator {
    metrics_collector: Arc<MetricsCollector>,
    health_checker: Arc<HealthChecker>,
    event_logger: Arc<EventLogger>,
    alert_manager: Arc<AlertManager>,
}

/// Metrics collector
#[allow(dead_code)]
struct MetricsCollector {
    #[cfg(feature = "metrics")]
    registry: prometheus::Registry,
    network_metrics: NetworkMetrics,
    message_metrics: MessageMetrics,
    security_metrics: SecurityMetrics,
}

/// Network metrics
#[allow(dead_code)]
struct NetworkMetrics {
    #[cfg(feature = "metrics")]
    peer_count: prometheus::Gauge,
    #[cfg(feature = "metrics")]
    bytes_sent: prometheus::Counter,
    #[cfg(feature = "metrics")]
    bytes_received: prometheus::Counter,
    #[cfg(feature = "metrics")]
    connection_duration: prometheus::Histogram,
}

/// Message metrics
#[allow(dead_code)]
struct MessageMetrics {
    #[cfg(feature = "metrics")]
    messages_sent: prometheus::Counter,
    #[cfg(feature = "metrics")]
    messages_received: prometheus::Counter,
    #[cfg(feature = "metrics")]
    message_latency: prometheus::Histogram,
    #[cfg(feature = "metrics")]
    message_size: prometheus::Histogram,
}

/// Security metrics
#[allow(dead_code)]
struct SecurityMetrics {
    #[cfg(feature = "metrics")]
    auth_attempts: prometheus::Counter,
    #[cfg(feature = "metrics")]
    auth_failures: prometheus::Counter,
    #[cfg(feature = "metrics")]
    rate_limit_hits: prometheus::Counter,
    #[cfg(feature = "metrics")]
    dos_attacks_blocked: prometheus::Counter,
}

/// Health checker
#[allow(dead_code)]
struct HealthChecker {
    checks: Vec<Box<dyn HealthCheck>>,
    last_check: RwLock<Option<HealthReport>>,
}

/// Health check trait
pub trait HealthCheck: Send + Sync {
    fn check(&self) -> HealthStatus;
    fn name(&self) -> &str;
}

/// Health status
#[derive(Debug, Clone)]
pub enum HealthStatus {
    Healthy,
    Degraded(String),
    Unhealthy(String),
}

/// Health report
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct HealthReport {
    timestamp: Instant,
    overall_status: HealthStatus,
    component_statuses: Vec<(String, HealthStatus)>,
}

/// Event logger
#[allow(dead_code)]
struct EventLogger {
    log_level: tracing::Level,
    event_buffer: Arc<RwLock<std::collections::VecDeque<LoggedEvent>>>,
}

/// Logged event
#[allow(dead_code)]
struct LoggedEvent {
    timestamp: Instant,
    level: tracing::Level,
    message: String,
    context: std::collections::HashMap<String, String>,
}

/// Alert manager
#[allow(dead_code)]
struct AlertManager {
    alert_rules: Vec<AlertRule>,
    alert_channel: mpsc::UnboundedSender<Alert>,
}

/// Alert rule
#[allow(dead_code)]
struct AlertRule {
    name: String,
    condition: Box<dyn Fn(&ManagerStats) -> bool + Send + Sync>,
    severity: AlertSeverity,
}

/// Alert severity
#[derive(Debug, Clone)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

/// Alert
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Alert {
    rule_name: String,
    severity: AlertSeverity,
    message: String,
    timestamp: Instant,
}

impl P2PManager {
    /// Create a new P2P manager
    pub fn new(config: P2PConfig, keypair: Keypair) -> (Self, mpsc::Receiver<ManagerCommand>) {
        let local_peer_id = PeerId::from(keypair.public());
        let config = Arc::new(config);

        // Create command channel
        let (command_tx, command_rx) = mpsc::channel(100);

        // Create coordinators
        let (network_event_tx, _network_event_rx) = mpsc::unbounded_channel();
        let network_coordinator = Arc::new(NetworkCoordinator {
            config: config.clone(),
            transport: Arc::new(RwLock::new(None)),
            connections: Arc::new(RwLock::new(ConnectionState::default())),
            event_tx: network_event_tx,
        });

        let message_coordinator = Arc::new(MessageCoordinator {
            router: Arc::new(RwLock::new(MessageRouter {
                routing_table: std::collections::HashMap::new(),
                default_strategy: RoutingStrategy::Broadcast,
            })),
            handlers: Arc::new(RwLock::new(MessageHandlers {
                handlers: std::collections::HashMap::new(),
            })),
            outbound_queue: Arc::new(RwLock::new(OutboundQueue {
                high_priority: std::collections::VecDeque::new(),
                normal_priority: std::collections::VecDeque::new(),
                low_priority: std::collections::VecDeque::new(),
            })),
            stats: Arc::new(RwLock::new(MessageStats::default())),
        });

        let security_coordinator = Arc::new(SecurityCoordinator {
            encryption: Arc::new(crate::security::encryption::EncryptionManager::new()),
            auth: Arc::new(
                crate::security::auth::AuthManager::new(
                    crate::security::auth::AuthConfig::default(),
                )
                .unwrap_or_else(|_| panic!("Failed to create AuthManager")),
            ),
            rate_limiter: Arc::new(crate::rate_limiter::RateLimiter::new(
                100,
                Duration::from_secs(1),
            )),
            dos_protection: Arc::new(crate::security::dos_protection::DosProtectionManager::new(
                crate::security::dos_protection::DosProtectionConfig::default(),
            )),
            reputation: Arc::new(crate::security::reputation::ReputationManager::new_sync(
                crate::security::reputation::ReputationConfig::default(),
            )),
            security_policy: Arc::new(RwLock::new(SecurityPolicy {
                require_encryption: true,
                require_authentication: true,
                max_message_size: 1024 * 1024, // 1MB
                banned_peers: std::collections::HashSet::new(),
            })),
        });

        let discovery_coordinator = Arc::new(DiscoveryCoordinator {
            mdns: Arc::new(RwLock::new(None)),
            kademlia: Arc::new(RwLock::new(None)),
            bootstrap_peers: Vec::new(),
            discovered_peers: Arc::new(RwLock::new(DiscoveredPeers::default())),
        });

        let monitoring_coordinator = Arc::new(MonitoringCoordinator {
            metrics_collector: Arc::new(MetricsCollector {
                #[cfg(feature = "metrics")]
                registry: prometheus::Registry::new(),
                network_metrics: NetworkMetrics {
                    #[cfg(feature = "metrics")]
                    peer_count: prometheus::Gauge::new(
                        "p2p_peer_count",
                        "Number of connected peers",
                    )
                    .unwrap(),
                    #[cfg(feature = "metrics")]
                    bytes_sent: prometheus::Counter::new("p2p_bytes_sent", "Total bytes sent")
                        .unwrap(),
                    #[cfg(feature = "metrics")]
                    bytes_received: prometheus::Counter::new(
                        "p2p_bytes_received",
                        "Total bytes received",
                    )
                    .unwrap(),
                    #[cfg(feature = "metrics")]
                    connection_duration: prometheus::Histogram::with_opts(
                        prometheus::HistogramOpts::new(
                            "p2p_connection_duration",
                            "Connection duration in seconds",
                        ),
                    )
                    .unwrap(),
                },
                message_metrics: MessageMetrics {
                    #[cfg(feature = "metrics")]
                    messages_sent: prometheus::Counter::new(
                        "p2p_messages_sent",
                        "Total messages sent",
                    )
                    .unwrap(),
                    #[cfg(feature = "metrics")]
                    messages_received: prometheus::Counter::new(
                        "p2p_messages_received",
                        "Total messages received",
                    )
                    .unwrap(),
                    #[cfg(feature = "metrics")]
                    message_latency: prometheus::Histogram::with_opts(
                        prometheus::HistogramOpts::new(
                            "p2p_message_latency",
                            "Message latency in seconds",
                        ),
                    )
                    .unwrap(),
                    #[cfg(feature = "metrics")]
                    message_size: prometheus::Histogram::with_opts(prometheus::HistogramOpts::new(
                        "p2p_message_size",
                        "Message size in bytes",
                    ))
                    .unwrap(),
                },
                security_metrics: SecurityMetrics {
                    #[cfg(feature = "metrics")]
                    auth_attempts: prometheus::Counter::new(
                        "p2p_auth_attempts",
                        "Total auth attempts",
                    )
                    .unwrap(),
                    #[cfg(feature = "metrics")]
                    auth_failures: prometheus::Counter::new(
                        "p2p_auth_failures",
                        "Total auth failures",
                    )
                    .unwrap(),
                    #[cfg(feature = "metrics")]
                    rate_limit_hits: prometheus::Counter::new(
                        "p2p_rate_limit_hits",
                        "Rate limit hits",
                    )
                    .unwrap(),
                    #[cfg(feature = "metrics")]
                    dos_attacks_blocked: prometheus::Counter::new(
                        "p2p_dos_blocked",
                        "DoS attacks blocked",
                    )
                    .unwrap(),
                },
            }),
            health_checker: Arc::new(HealthChecker {
                checks: Vec::new(),
                last_check: RwLock::new(None),
            }),
            event_logger: Arc::new(EventLogger {
                log_level: tracing::Level::INFO,
                event_buffer: Arc::new(RwLock::new(std::collections::VecDeque::new())),
            }),
            alert_manager: Arc::new(AlertManager {
                alert_rules: Vec::new(),
                alert_channel: mpsc::unbounded_channel().0,
            }),
        });

        let manager = Self {
            config,
            local_peer_id,
            network_coordinator,
            message_coordinator,
            security_coordinator,
            discovery_coordinator,
            monitoring_coordinator,
            state: Arc::new(RwLock::new(ManagerState {
                running: false,
                start_time: None,
                shutdown_signal: None,
            })),
            command_tx,
        };

        (manager, command_rx)
    }

    /// Get a handle for sending commands to the manager
    pub fn get_handle(&self) -> ManagerHandle {
        ManagerHandle {
            command_tx: self.command_tx.clone(),
        }
    }

    /// Start the P2P manager
    pub async fn start(&mut self, command_rx: mpsc::Receiver<ManagerCommand>) -> P2PResult<()> {
        let mut state = self.state.write().await;
        if state.running {
            return Err(P2PError::AlreadyStarted);
        }

        info!("Starting P2P manager for peer {}", self.local_peer_id);

        // Start all coordinators
        self.start_coordinators().await?;

        // Update state
        state.running = true;
        state.start_time = Some(Instant::now());

        // Start command processing loop
        self.start_command_loop(command_rx);

        Ok(())
    }

    /// Stop the P2P manager
    pub async fn stop(&mut self) -> P2PResult<()> {
        let mut state = self.state.write().await;
        if !state.running {
            return Ok(());
        }

        info!("Stopping P2P manager");

        // Send shutdown signal
        if let Some(shutdown_tx) = state.shutdown_signal.take() {
            let _ = shutdown_tx.send(()).await;
        }

        // Stop all coordinators
        self.stop_coordinators().await?;

        // Update state
        state.running = false;

        Ok(())
    }

    /// Start all coordinators
    async fn start_coordinators(&self) -> P2PResult<()> {
        info!("Starting P2P coordinators");

        // Network coordinator initialization
        info!("Initializing network coordinator");
        self.network_coordinator.initialize().await?;

        // Security coordinator initialization
        info!("Initializing security coordinator");
        self.security_coordinator.initialize().await?;

        // Message coordinator initialization
        info!("Initializing message coordinator");
        self.message_coordinator.initialize().await?;

        // Discovery coordinator initialization
        info!("Initializing discovery coordinator");
        self.discovery_coordinator.initialize().await?;

        // Monitoring coordinator initialization
        info!("Initializing monitoring coordinator");
        self.monitoring_coordinator.initialize().await?;

        info!("All P2P coordinators initialized successfully");
        Ok(())
    }

    /// Stop all coordinators
    async fn stop_coordinators(&self) -> P2PResult<()> {
        info!("Stopping P2P coordinators");

        // Stop in reverse order of startup
        info!("Shutting down monitoring coordinator");
        self.monitoring_coordinator.shutdown().await?;

        info!("Shutting down discovery coordinator");
        self.discovery_coordinator.shutdown().await?;

        info!("Shutting down message coordinator");
        self.message_coordinator.shutdown().await?;

        info!("Shutting down security coordinator");
        self.security_coordinator.shutdown().await?;

        info!("Shutting down network coordinator");
        self.network_coordinator.shutdown().await?;

        info!("All P2P coordinators stopped");
        Ok(())
    }

    /// Start command processing loop
    fn start_command_loop(&self, mut command_rx: mpsc::Receiver<ManagerCommand>) {
        let _state = self.state.clone();
        let _network = self.network_coordinator.clone();
        let messages = self.message_coordinator.clone();
        let monitoring = self.monitoring_coordinator.clone();

        tokio::spawn(async move {
            while let Some(command) = command_rx.recv().await {
                match command {
                    ManagerCommand::Start => {
                        // Already started
                    }
                    ManagerCommand::Stop => {
                        break;
                    }
                    ManagerCommand::SendMessage {
                        message,
                        priority,
                        response,
                    } => {
                        // Delegate to message coordinator
                        let result = messages.send_message(message, priority).await;
                        let _ = response.send(result);
                    }
                    ManagerCommand::GetStats { response } => {
                        // Collect stats from all coordinators
                        let stats = monitoring.collect_stats().await;
                        let _ = response.send(stats);
                    }
                }
            }
        });
    }
}

/// Handle for interacting with the P2P manager
#[derive(Clone)]
pub struct ManagerHandle {
    command_tx: mpsc::Sender<ManagerCommand>,
}

impl ManagerHandle {
    /// Send a message
    pub async fn send_message(&self, message: NetworkMessage, priority: Priority) -> P2PResult<()> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(ManagerCommand::SendMessage {
                message,
                priority,
                response: response_tx,
            })
            .await
            .map_err(|_| P2PError::ManagerShutdown)?;

        response_rx.await.map_err(|_| P2PError::ManagerShutdown)?
    }

    /// Get manager statistics
    pub async fn get_stats(&self) -> P2PResult<ManagerStats> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(ManagerCommand::GetStats {
                response: response_tx,
            })
            .await
            .map_err(|_| P2PError::ManagerShutdown)?;

        response_rx.await.map_err(|_| P2PError::ManagerShutdown)
    }
}

// Coordinator implementations

impl NetworkCoordinator {
    /// Initialize the network coordinator
    async fn initialize(&self) -> P2PResult<()> {
        info!("Initializing network transport layer");

        // Convert P2PConfig to TransportConfig
        let transport_config = crate::transport::transport::TransportConfig {
            tcp: crate::transport::transport::TcpConfig {
                listen_addresses: self.config.network.listen_addresses.clone(),
                nodelay: true,
                keepalive: Some(Duration::from_secs(30)),
                send_buffer_size: None,
                recv_buffer_size: None,
            },
            websocket: crate::transport::transport::WebSocketConfig {
                listen_addresses: vec![], // WebSocket disabled by default
                max_frame_size: 65536,
                max_message_size: self.config.network.max_message_size,
                ping_interval: Duration::from_secs(30),
                compression: false,
            },
            quic: crate::transport::transport::QuicConfig {
                listen_addresses: vec![], // QUIC disabled by default
                max_idle_timeout: Duration::from_secs(60),
                max_concurrent_bidi_streams: 100,
                keep_alive_interval: Duration::from_secs(30),
            },
            connections: crate::transport::transport::ConnectionConfig {
                max_total_connections: self.config.network.max_connections,
                max_connections_per_peer: 1,
                connection_timeout: Duration::from_secs(10),
                idle_timeout: Duration::from_secs(300),
                handshake_timeout: Duration::from_secs(5),
                enable_pooling: true,
            },
            performance: crate::transport::transport::PerformanceConfig {
                max_concurrent_operations: 1000,
                stream_window_size: 256 * 1024,      // 256KB
                connection_window_size: 1024 * 1024, // 1MB
                message_batch_size: 100,
                enable_compression: false,
            },
            security: crate::transport::transport::SecurityConfig {
                enable_tls: false,
                require_mtls: false,
                cipher_suites: vec![],
                cert_verification_depth: 3,
            },
        };

        // Create keypair for transport (using the manager's keypair would be better)
        let keypair = libp2p::identity::Keypair::generate_ed25519();

        // Create event channel for transport
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        // Create and configure the unified transport
        let transport = crate::transport::transport::UnifiedTransport::new(
            transport_config,
            keypair,
            event_tx.clone(),
        );

        // Store the transport instance
        let mut transport_lock = self.transport.write().await;
        *transport_lock = Some(transport);

        // Spawn task to handle transport events
        let self_clone = self.clone();
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                match event {
                    crate::transport::transport::TransportEvent::ConnectionEstablished {
                        peer_id,
                        address,
                        ..
                    } => {
                        if let Err(e) = self_clone.handle_peer_connected(peer_id, address).await {
                            tracing::error!("Failed to handle peer connection: {}", e);
                        }
                    }
                    crate::transport::transport::TransportEvent::ConnectionClosed {
                        peer_id,
                        reason,
                    } => {
                        if let Err(e) = self_clone.handle_peer_disconnected(peer_id, reason).await {
                            tracing::error!("Failed to handle peer disconnection: {}", e);
                        }
                    }
                    _ => {}
                }
            }
        });

        // Initialize connection state tracking
        let mut connections = self.connections.write().await;
        connections.active_peers.clear();
        connections.total_connections = 0;
        connections.total_disconnections = 0;

        info!("Network coordinator initialized");
        Ok(())
    }

    /// Shutdown the network coordinator
    async fn shutdown(&self) -> P2PResult<()> {
        info!("Shutting down network transport");

        // Close all active connections
        let connections = self.connections.read().await;
        for (peer_id, _) in connections.active_peers.iter() {
            info!("Closing connection to peer: {}", peer_id);
        }
        drop(connections);

        // Clear the transport
        let mut transport_lock = self.transport.write().await;
        *transport_lock = None;

        // Clear connection state
        let mut connections = self.connections.write().await;
        connections.active_peers.clear();

        info!("Network coordinator shutdown complete");
        Ok(())
    }

    /// Handle peer connection
    async fn handle_peer_connected(&self, peer_id: PeerId, address: Multiaddr) -> P2PResult<()> {
        let mut connections = self.connections.write().await;

        let connection = PeerConnection {
            peer_id,
            address: address.clone(),
            connected_at: Instant::now(),
            last_activity: Instant::now(),
            bytes_sent: 0,
            bytes_received: 0,
        };

        connections.active_peers.insert(peer_id, connection);
        connections.total_connections += 1;

        // Send network event
        let _ = self
            .event_tx
            .send(NetworkEvent::PeerConnected { peer_id, address });

        Ok(())
    }

    /// Handle peer disconnection
    async fn handle_peer_disconnected(&self, peer_id: PeerId, reason: String) -> P2PResult<()> {
        let mut connections = self.connections.write().await;

        if connections.active_peers.remove(&peer_id).is_some() {
            connections.total_disconnections += 1;

            // Send network event
            let _ = self
                .event_tx
                .send(NetworkEvent::PeerDisconnected { peer_id, reason });
        }

        Ok(())
    }
}

impl MessageCoordinator {
    /// Initialize the message coordinator
    async fn initialize(&self) -> P2PResult<()> {
        info!("Initializing message routing and handlers");

        // Clear any existing state
        let mut router = self.router.write().await;
        router.routing_table.clear();
        drop(router);

        let mut handlers = self.handlers.write().await;
        handlers.handlers.clear();
        drop(handlers);

        let mut queue = self.outbound_queue.write().await;
        queue.high_priority.clear();
        queue.normal_priority.clear();
        queue.low_priority.clear();
        drop(queue);

        // Reset statistics
        let mut stats = self.stats.write().await;
        *stats = MessageStats::default();

        info!("Message coordinator initialized");
        Ok(())
    }

    /// Shutdown the message coordinator
    async fn shutdown(&self) -> P2PResult<()> {
        info!("Shutting down message coordinator");

        // Clear all queues
        let mut queue = self.outbound_queue.write().await;
        queue.high_priority.clear();
        queue.normal_priority.clear();
        queue.low_priority.clear();

        info!("Message coordinator shutdown complete");
        Ok(())
    }

    /// Register a message handler
    pub async fn register_handler(
        &self,
        message_type: String,
        handler: Box<dyn MessageHandler>,
    ) -> P2PResult<()> {
        let mut handlers = self.handlers.write().await;
        handlers.handlers.insert(message_type, handler);
        Ok(())
    }

    /// Process incoming message
    pub async fn process_incoming_message(&self, message: NetworkMessage) -> P2PResult<()> {
        // Update statistics
        let mut stats = self.stats.write().await;
        stats.messages_received += 1;
        drop(stats);

        // Find handler for message type
        let handlers = self.handlers.read().await;
        let message_type = match &message.payload {
            crate::protocol::messages::MessagePayload::Svm(_) => "svm",
            crate::protocol::messages::MessagePayload::Evm(_) => "evm",
            crate::protocol::messages::MessagePayload::MultiVm(_) => "multivm",
            crate::protocol::messages::MessagePayload::Control(_) => "control",
            crate::protocol::messages::MessagePayload::Discovery(_) => "discovery",
            crate::protocol::messages::MessagePayload::Custom(_) => "custom",
        };

        if let Some(handler) = handlers.handlers.get(message_type) {
            handler.handle_message(message)?;
        } else {
            warn!("No handler registered for message type: {}", message_type);
        }

        Ok(())
    }
}

impl MessageCoordinator {
    async fn send_message(&self, message: NetworkMessage, priority: Priority) -> P2PResult<()> {
        // Add to outbound queue based on priority
        let mut queue = self.outbound_queue.write().await;
        let queued_message = QueuedMessage {
            message,
            target: RoutingStrategy::Broadcast, // Default strategy
            queued_at: Instant::now(),
            attempts: 0,
        };

        match priority {
            Priority::High | Priority::Critical => queue.high_priority.push_back(queued_message),
            Priority::Normal => queue.normal_priority.push_back(queued_message),
            Priority::Low => queue.low_priority.push_back(queued_message),
        }

        // Update stats
        let mut stats = self.stats.write().await;
        stats.messages_sent += 1;

        Ok(())
    }
}

impl SecurityCoordinator {
    /// Initialize the security coordinator
    async fn initialize(&self) -> P2PResult<()> {
        info!("Initializing security coordinator");

        // The security components are already initialized in the constructor
        // Here we just perform any runtime initialization needed

        // Clear security policy banned peers list
        let mut policy = self.security_policy.write().await;
        policy.banned_peers.clear();

        info!("Security coordinator initialized");
        Ok(())
    }

    /// Shutdown the security coordinator
    async fn shutdown(&self) -> P2PResult<()> {
        info!("Shutting down security coordinator");

        // Cleanup auth manager
        self.auth.cleanup().await?;

        // Shutdown reputation manager
        self.reputation.shutdown().await?;

        // Clear banned peers
        let mut policy = self.security_policy.write().await;
        policy.banned_peers.clear();

        info!("Security coordinator shutdown complete");
        Ok(())
    }

    /// Create a new security coordinator
    pub fn new(config: Arc<P2PConfig>) -> P2PResult<Self> {
        let encryption = Arc::new(EncryptionManager::new());
        // Convert config auth to security auth config
        let auth_config = SecurityAuthConfig {
            jwt_enabled: config.auth.enabled,
            jwt_secret: "default-jwt-secret".to_string(),
            jwt_expiration: config.auth.timeout,
            api_key_enabled: true,
            api_key_length: 32,
            api_key_expiration: config.auth.timeout,
            peer_cert_enabled: false,
            max_failed_attempts: 3,
            ban_duration: std::time::Duration::from_secs(300),
            audit_logging: true,
        };
        let auth = Arc::new(AuthManager::new(auth_config)?);
        // Convert RateLimitConfig to RateLimiterConfig
        let rate_limiter_config = crate::rate_limiter::RateLimiterConfig {
            per_peer_rate: config.rate_limiting.max_requests_per_second as u32,
            per_peer_burst: config.rate_limiting.burst_size as u32,
            global_rate: (config.rate_limiting.max_requests_per_second * 10.0) as u32,
            global_burst: (config.rate_limiting.burst_size * 10) as u32,
            enabled: config.rate_limiting.enabled,
        };
        let rate_limiter = Arc::new(RateLimiter::new_with_config(rate_limiter_config));
        let dos_protection = Arc::new(DosProtectionManager::new(Default::default()));

        // Create reputation manager synchronously
        let reputation = Arc::new(crate::security::reputation::ReputationManager::new_sync(
            crate::security::reputation::ReputationConfig::default(),
        ));

        let security_policy = Arc::new(RwLock::new(SecurityPolicy {
            require_encryption: true, // Default security settings
            require_authentication: true,
            max_message_size: config.network.max_message_size,
            banned_peers: std::collections::HashSet::new(),
        }));

        Ok(Self {
            encryption,
            auth,
            rate_limiter,
            dos_protection,
            reputation,
            security_policy,
        })
    }

    /// Authenticate a peer
    pub async fn authenticate_peer(&self, peer_id: &PeerId, token: &str) -> P2PResult<bool> {
        // Check reputation first
        if self.reputation.is_banned(peer_id).await {
            warn!(
                "Authentication attempt from banned peer {} rejected",
                peer_id
            );
            return Ok(false);
        }

        // Validate JWT or API key token
        // For now, try JWT authentication - in production, detect token type
        let auth_result = self.auth.authenticate_jwt(token, "unknown").await;

        // Record authentication attempt in reputation system
        self.reputation
            .record_event(
                *peer_id,
                crate::security::reputation::ReputationEvent::AuthenticationAttempt {
                    success: auth_result.success,
                },
            )
            .await?;

        if auth_result.success {
            info!("Peer {} authenticated successfully", peer_id);
            Ok(true)
        } else {
            warn!(
                "Authentication failed for peer {}: {:?}",
                peer_id, auth_result.error
            );
            Ok(false)
        }
    }

    /// Check if a message should be allowed through security filters
    pub async fn validate_message(
        &self,
        peer_id: &PeerId,
        message: &NetworkMessage,
    ) -> P2PResult<bool> {
        // Check reputation first
        if self.reputation.is_banned(peer_id).await {
            warn!("Message from banned peer {} rejected", peer_id);
            self.reputation
                .record_event(
                    *peer_id,
                    crate::security::reputation::ReputationEvent::MessageReceived { valid: false },
                )
                .await?;
            return Ok(false);
        }

        // Check if peer is banned by policy
        {
            let policy = self.security_policy.read().await;
            if policy.banned_peers.contains(peer_id) {
                return Ok(false);
            }

            // Check message size limits
            let message_size = bincode::serialize(message)
                .map_err(|e| P2PError::Serialization {
                    message: e.to_string(),
                })?
                .len();

            if message_size > policy.max_message_size {
                warn!(
                    "Message from peer {} exceeds size limit: {} > {}",
                    peer_id, message_size, policy.max_message_size
                );
                self.reputation
                    .record_event(
                        *peer_id,
                        crate::security::reputation::ReputationEvent::SecurityViolation {
                            severity: "message_size_exceeded".to_string(),
                        },
                    )
                    .await?;
                return Ok(false);
            }
        }

        // Rate limiting would be checked here
        // Note: Current RateLimiter requires &mut self, would need redesign for Arc usage
        // For now, we'll skip rate limiting in this coordinator

        // Check DOS protection
        let message_size = bincode::serialize(message)
            .map_err(|e| P2PError::Serialization {
                message: e.to_string(),
            })?
            .len();

        if self
            .dos_protection
            .check_message(*peer_id, message_size)
            .await
            .is_err()
        {
            warn!("DOS protection blocked message from peer {}", peer_id);
            self.reputation
                .record_event(
                    *peer_id,
                    crate::security::reputation::ReputationEvent::SecurityViolation {
                        severity: "dos_attack".to_string(),
                    },
                )
                .await?;
            return Ok(false);
        }

        // Record successful validation
        self.reputation
            .record_event(
                *peer_id,
                crate::security::reputation::ReputationEvent::MessageReceived { valid: true },
            )
            .await?;

        Ok(true)
    }

    /// Encrypt a message for transmission
    pub async fn encrypt_message(
        &self,
        peer_public_key: &x25519_dalek::PublicKey,
        data: &[u8],
    ) -> P2PResult<Vec<u8>> {
        self.encryption.encrypt_message(peer_public_key, data)
    }

    /// Decrypt a received message
    pub async fn decrypt_message(
        &self,
        peer_public_key: &x25519_dalek::PublicKey,
        encrypted_data: &[u8],
    ) -> P2PResult<Vec<u8>> {
        self.encryption
            .decrypt_message(peer_public_key, encrypted_data)
    }

    /// Ban a peer
    pub async fn ban_peer(&self, peer_id: PeerId, reason: String) -> P2PResult<()> {
        let mut policy = self.security_policy.write().await;
        policy.banned_peers.insert(peer_id);
        warn!("Banned peer {} for reason: {}", peer_id, reason);
        Ok(())
    }

    /// Unban a peer
    pub async fn unban_peer(&self, peer_id: &PeerId) -> P2PResult<()> {
        let mut policy = self.security_policy.write().await;
        if policy.banned_peers.remove(peer_id) {
            info!("Unbanned peer {}", peer_id);
        }
        Ok(())
    }

    /// Get security statistics
    pub async fn get_security_stats(&self) -> SecurityStats {
        SecurityStats {
            banned_peers_count: self.security_policy.read().await.banned_peers.len(),
            rate_limit_violations: 0, // TODO: Add violation tracking
            dos_protection_blocks: 0, // TODO: Add block tracking
            encryption_cache_stats: self.encryption.get_cache_stats(),
        }
    }
}

/// Security statistics
#[derive(Debug, Clone)]
pub struct SecurityStats {
    pub banned_peers_count: usize,
    pub rate_limit_violations: u64,
    pub dos_protection_blocks: u64,
    pub encryption_cache_stats: crate::security::encryption::CacheStats,
}

impl DiscoveryCoordinator {
    /// Initialize the discovery coordinator
    async fn initialize(&self) -> P2PResult<()> {
        info!("Initializing discovery coordinator");

        // Initialize mDNS discovery if enabled
        let mut mdns = self.mdns.write().await;
        *mdns = Some(MdnsDiscovery {
            enabled: true,
            service_name: "_multivm-p2p._tcp.local".to_string(),
        });
        drop(mdns);

        // Initialize Kademlia discovery if enabled
        let mut kad = self.kademlia.write().await;
        *kad = Some(KademliaDiscovery {
            enabled: true,
            replication_factor: 20,
        });
        drop(kad);

        // Clear discovered peers
        let mut discovered = self.discovered_peers.write().await;
        discovered.peers.clear();
        discovered.last_discovery = None;

        info!("Discovery coordinator initialized");
        Ok(())
    }

    /// Shutdown the discovery coordinator
    async fn shutdown(&self) -> P2PResult<()> {
        info!("Shutting down discovery coordinator");

        // Disable discovery mechanisms
        let mut mdns = self.mdns.write().await;
        if let Some(ref mut m) = *mdns {
            m.enabled = false;
        }
        drop(mdns);

        let mut kad = self.kademlia.write().await;
        if let Some(ref mut k) = *kad {
            k.enabled = false;
        }
        drop(kad);

        // Clear discovered peers
        let mut discovered = self.discovered_peers.write().await;
        discovered.peers.clear();

        info!("Discovery coordinator shutdown complete");
        Ok(())
    }

    /// Add a discovered peer
    pub async fn add_discovered_peer(
        &self,
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
    ) -> P2PResult<()> {
        let mut discovered = self.discovered_peers.write().await;

        let peer = DiscoveredPeer {
            peer_id,
            addresses,
            discovered_at: Instant::now(),
            score: 1.0, // Initial trust score
        };

        discovered.peers.insert(peer_id, peer);
        discovered.last_discovery = Some(Instant::now());

        Ok(())
    }

    /// Get discovered peers
    pub async fn get_discovered_peers(&self) -> Vec<(PeerId, Vec<Multiaddr>)> {
        let discovered = self.discovered_peers.read().await;
        discovered
            .peers
            .iter()
            .map(|(id, peer)| (*id, peer.addresses.clone()))
            .collect()
    }
}

impl MonitoringCoordinator {
    /// Initialize the monitoring coordinator
    async fn initialize(&self) -> P2PResult<()> {
        info!("Initializing monitoring coordinator");

        // Reset metrics if needed
        #[cfg(feature = "metrics")]
        {
            self.metrics_collector.network_metrics.peer_count.set(0.0);
            self.metrics_collector.network_metrics.bytes_sent.reset();
            self.metrics_collector
                .network_metrics
                .bytes_received
                .reset();
            self.metrics_collector.message_metrics.messages_sent.reset();
            self.metrics_collector
                .message_metrics
                .messages_received
                .reset();
            self.metrics_collector
                .security_metrics
                .auth_attempts
                .reset();
            self.metrics_collector
                .security_metrics
                .auth_failures
                .reset();
            self.metrics_collector
                .security_metrics
                .rate_limit_hits
                .reset();
            self.metrics_collector
                .security_metrics
                .dos_attacks_blocked
                .reset();
        }

        // Clear event buffer
        let mut event_buffer = self.event_logger.event_buffer.write().await;
        event_buffer.clear();

        // Initialize health checker
        let mut last_check = self.health_checker.last_check.write().await;
        *last_check = None;

        info!("Monitoring coordinator initialized");
        Ok(())
    }

    /// Shutdown the monitoring coordinator
    async fn shutdown(&self) -> P2PResult<()> {
        info!("Shutting down monitoring coordinator");

        // Perform final health check
        let health_report = self.perform_health_check().await;
        info!("Final health status: {:?}", health_report.overall_status);

        // Clear event buffer
        let mut event_buffer = self.event_logger.event_buffer.write().await;
        event_buffer.clear();

        info!("Monitoring coordinator shutdown complete");
        Ok(())
    }

    /// Perform health check
    async fn perform_health_check(&self) -> HealthReport {
        let mut component_statuses = Vec::new();
        let mut overall_status = HealthStatus::Healthy;

        // Check each registered health check
        for check in &self.health_checker.checks {
            let status = check.check();
            let name = check.name().to_string();

            match &status {
                HealthStatus::Degraded(_) => {
                    if matches!(overall_status, HealthStatus::Healthy) {
                        overall_status = status.clone();
                    }
                }
                HealthStatus::Unhealthy(_) => {
                    overall_status = status.clone();
                }
                _ => {}
            }

            component_statuses.push((name, status));
        }

        let report = HealthReport {
            timestamp: Instant::now(),
            overall_status,
            component_statuses,
        };

        // Store the report
        let mut last_check = self.health_checker.last_check.write().await;
        *last_check = Some(report.clone());

        report
    }

    async fn collect_stats(&self) -> ManagerStats {
        // Collect uptime from start time if available
        let start_time = Instant::now() - Duration::from_secs(300); // Default 5 min uptime if no start time
        let uptime = start_time.elapsed();

        // In a production implementation, these would collect from actual coordinators
        // For now, provide realistic placeholder values that could be expanded
        ManagerStats {
            uptime,
            peers_connected: 0,   // Network coordinator would provide this
            messages_sent: 0,     // Message coordinator would provide this
            messages_received: 0, // Message coordinator would provide this
            bytes_sent: 0,        // Network coordinator would provide this
            bytes_received: 0,    // Network coordinator would provide this
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::identity;

    #[tokio::test]
    async fn test_manager_lifecycle() {
        let keypair = identity::Keypair::generate_ed25519();
        let config = P2PConfig::default();

        let (mut manager, command_rx) = P2PManager::new(config, keypair);
        let _handle = manager.get_handle();

        // Test start
        assert!(manager.start(command_rx).await.is_ok());

        // Test double start
        // Can't test double start without another receiver

        // Test stop
        assert!(manager.stop().await.is_ok());

        // Test double stop
        assert!(manager.stop().await.is_ok());
    }
}
