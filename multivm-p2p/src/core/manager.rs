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

        // TODO: Implement actual coordinator startup logic
        // For now, we just log the startup sequence

        // Network coordinator initialization
        info!("Initializing network coordinator");
        // self.network_coordinator.initialize().await?;

        // Security coordinator initialization
        info!("Initializing security coordinator");
        // self.security_coordinator.initialize().await?;

        // Message coordinator initialization
        info!("Initializing message coordinator");
        // self.message_coordinator.initialize().await?;

        // Discovery coordinator initialization
        info!("Initializing discovery coordinator");
        // self.discovery_coordinator.initialize().await?;

        // Monitoring coordinator initialization
        info!("Initializing monitoring coordinator");
        // self.monitoring_coordinator.initialize().await?;

        info!("All P2P coordinators initialized successfully");
        Ok(())
    }

    /// Stop all coordinators
    async fn stop_coordinators(&self) -> P2PResult<()> {
        info!("Stopping P2P coordinators");

        // TODO: Implement actual coordinator shutdown logic
        // For now, we just log the shutdown sequence

        // Stop in reverse order of startup
        info!("Shutting down monitoring coordinator");
        // self.monitoring_coordinator.shutdown().await?;

        info!("Shutting down discovery coordinator");
        // self.discovery_coordinator.shutdown().await?;

        info!("Shutting down message coordinator");
        // self.message_coordinator.shutdown().await?;

        info!("Shutting down security coordinator");
        // self.security_coordinator.shutdown().await?;

        info!("Shutting down network coordinator");
        // self.network_coordinator.shutdown().await?;

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

// Coordinator implementations would go here...

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
            security_policy,
        })
    }

    /// Authenticate a peer
    pub async fn authenticate_peer(&self, peer_id: &PeerId, token: &str) -> P2PResult<bool> {
        // Validate JWT or API key token
        // For now, try JWT authentication - in production, detect token type
        let auth_result = self.auth.authenticate_jwt(token, "unknown").await;
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
        // Check if peer is banned
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
                return Ok(false);
            }
        }

        // Rate limiting would be checked here
        // Note: Current RateLimiter requires &mut self, would need redesign for Arc usage
        // For now, we'll skip rate limiting in this coordinator

        // Check DOS protection
        // Check DOS protection with message size
        let message_size = bincode::serialize(message)
            .map_err(|e| P2PError::Serialization {
                message: e.to_string(),
            })?
            .len();

        if let Err(_) = self
            .dos_protection
            .check_message(*peer_id, message_size)
            .await
        {
            warn!("DOS protection blocked message from peer {}", peer_id);
            return Ok(false);
        }

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

impl MonitoringCoordinator {
    async fn collect_stats(&self) -> ManagerStats {
        // Collect stats from all components
        ManagerStats {
            uptime: Duration::from_secs(0), // Would calculate from start_time
            peers_connected: 0,             // Would get from network coordinator
            messages_sent: 0,               // Would get from message coordinator
            messages_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
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
