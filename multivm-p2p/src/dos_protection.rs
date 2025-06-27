//! Enhanced DoS/DDoS Protection for P2P Networking
//!
//! Provides comprehensive protection against various types of DoS attacks including:
//! - Connection flooding
//! - Message flooding
//! - Bandwidth exhaustion
//! - Resource exhaustion
//! - Peer reputation management

use crate::circuit_breaker::{CircuitBreakerConfig, CircuitBreakerManager, RequestOutcome};
use crate::error::{P2PError, P2PResult};
use crate::rate_limiter::{RateLimiter, RateLimiterConfig};
use libp2p::PeerId;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Maximum connections per IP address
const MAX_CONNECTIONS_PER_IP: usize = 10;

/// Maximum bandwidth per peer (bytes per second)
const MAX_BANDWIDTH_PER_PEER: u64 = 1024 * 1024; // 1MB/s

/// Connection rate limit per IP (connections per minute)
const CONNECTION_RATE_LIMIT: u32 = 60;

/// DoS protection configuration
#[derive(Debug, Clone)]
pub struct DosProtectionConfig {
    /// Enable connection flooding protection
    pub enable_connection_protection: bool,
    /// Maximum connections per IP
    pub max_connections_per_ip: usize,
    /// Connection rate limit per IP (per minute)
    pub connection_rate_limit: u32,
    /// Enable bandwidth limiting
    pub enable_bandwidth_limiting: bool,
    /// Maximum bandwidth per peer (bytes/sec)
    pub max_bandwidth_per_peer: u64,
    /// Enable message size validation
    pub enable_message_size_validation: bool,
    /// Maximum message size
    pub max_message_size: usize,
    /// Enable peer reputation system
    pub enable_reputation_system: bool,
    /// Minimum reputation score (0.0 to 1.0)
    pub min_reputation_score: f64,
    /// Enable adaptive protection
    pub enable_adaptive_protection: bool,
    /// Protection strictness level (1-10)
    pub protection_strictness: u8,
    /// Memory usage limit (MB)
    pub memory_limit_mb: usize,
    /// CPU usage threshold (0.0 to 1.0)
    pub cpu_threshold: f64,
    /// Emergency mode timeout
    pub emergency_mode_timeout: Duration,
}

impl Default for DosProtectionConfig {
    fn default() -> Self {
        Self {
            enable_connection_protection: true,
            max_connections_per_ip: MAX_CONNECTIONS_PER_IP,
            connection_rate_limit: CONNECTION_RATE_LIMIT,
            enable_bandwidth_limiting: true,
            max_bandwidth_per_peer: MAX_BANDWIDTH_PER_PEER,
            enable_message_size_validation: true,
            max_message_size: 1024 * 1024, // 1MB
            enable_reputation_system: true,
            min_reputation_score: 0.3,
            enable_adaptive_protection: true,
            protection_strictness: 5,
            memory_limit_mb: 512,
            cpu_threshold: 0.8,
            emergency_mode_timeout: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Peer reputation score
#[derive(Debug, Clone)]
pub struct PeerReputation {
    /// Reputation score (0.0 to 1.0)
    pub score: f64,
    /// Last update time
    pub last_updated: Instant,
    /// Number of successful interactions
    pub successful_interactions: u64,
    /// Number of failed interactions
    pub failed_interactions: u64,
    /// Number of timeouts
    pub timeouts: u64,
    /// Bandwidth usage history
    pub bandwidth_history: Vec<(Instant, u64)>,
    /// Connection count
    pub connection_count: u32,
    /// Is peer currently banned
    pub is_banned: bool,
    /// Ban expiry time
    pub ban_expiry: Option<Instant>,
    /// Trust level
    pub trust_level: TrustLevel,
}

/// Trust level for peers
#[derive(Debug, Clone, PartialEq)]
pub enum TrustLevel {
    /// Unknown peer
    Unknown,
    /// Trusted peer (whitelist)
    Trusted,
    /// Suspicious peer
    Suspicious,
    /// Malicious peer (blacklist)
    Malicious,
}

impl Default for PeerReputation {
    fn default() -> Self {
        Self {
            score: 0.5, // Neutral starting score
            last_updated: Instant::now(),
            successful_interactions: 0,
            failed_interactions: 0,
            timeouts: 0,
            bandwidth_history: Vec::new(),
            connection_count: 0,
            is_banned: false,
            ban_expiry: None,
            trust_level: TrustLevel::Unknown,
        }
    }
}

/// Connection tracking for IPs
#[derive(Debug, Clone)]
struct IpConnectionTracker {
    /// Current connection count
    connections: u32,
    /// Connection timestamps for rate limiting
    connection_times: Vec<Instant>,
    /// Total data transferred
    total_bytes: u64,
    /// First seen time
    first_seen: Instant,
    /// Last activity
    last_activity: Instant,
    /// Is IP banned
    is_banned: bool,
    /// Ban expiry
    ban_expiry: Option<Instant>,
}

impl Default for IpConnectionTracker {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            connections: 0,
            connection_times: Vec::new(),
            total_bytes: 0,
            first_seen: now,
            last_activity: now,
            is_banned: false,
            ban_expiry: None,
        }
    }
}

/// Bandwidth tracker for peers
#[derive(Debug, Clone)]
struct BandwidthTracker {
    /// Bytes transferred in current window
    current_window_bytes: u64,
    /// Window start time
    window_start: Instant,
    /// Window duration
    window_duration: Duration,
    /// Total bytes transferred
    total_bytes: u64,
    /// Peak bandwidth usage
    peak_bandwidth: u64,
}

impl BandwidthTracker {
    fn new(window_duration: Duration) -> Self {
        Self {
            current_window_bytes: 0,
            window_start: Instant::now(),
            window_duration,
            total_bytes: 0,
            peak_bandwidth: 0,
        }
    }

    fn record_bytes(&mut self, bytes: u64) -> bool {
        let now = Instant::now();

        // Reset window if expired
        if now.duration_since(self.window_start) >= self.window_duration {
            self.current_window_bytes = 0;
            self.window_start = now;
        }

        self.current_window_bytes += bytes;
        self.total_bytes += bytes;

        // Check if bandwidth limit exceeded
        let current_bandwidth = self.current_window_bytes;
        if current_bandwidth > self.peak_bandwidth {
            self.peak_bandwidth = current_bandwidth;
        }

        true
    }

    fn get_current_bandwidth(&self) -> u64 {
        let elapsed = self.window_start.elapsed();
        if elapsed.as_secs() > 0 {
            self.current_window_bytes / elapsed.as_secs()
        } else {
            self.current_window_bytes
        }
    }
}

/// System resource monitor
#[derive(Debug, Clone)]
struct ResourceMonitor {
    /// Memory usage (MB)
    memory_usage_mb: usize,
    /// CPU usage (0.0 to 1.0)
    cpu_usage: f64,
    /// Network I/O rate (bytes/sec)
    network_io_rate: u64,
    /// Active connections count
    active_connections: usize,
    /// Last update time
    last_updated: Instant,
    /// Emergency mode active
    emergency_mode: bool,
    /// Emergency mode start time
    emergency_start: Option<Instant>,
}

impl Default for ResourceMonitor {
    fn default() -> Self {
        Self {
            memory_usage_mb: 0,
            cpu_usage: 0.0,
            network_io_rate: 0,
            active_connections: 0,
            last_updated: Instant::now(),
            emergency_mode: false,
            emergency_start: None,
        }
    }
}

/// DoS protection manager
pub struct DosProtectionManager {
    config: DosProtectionConfig,
    rate_limiter: RateLimiter,
    circuit_breaker: CircuitBreakerManager,
    peer_reputations: Arc<RwLock<HashMap<PeerId, PeerReputation>>>,
    ip_trackers: Arc<RwLock<HashMap<IpAddr, IpConnectionTracker>>>,
    bandwidth_trackers: Arc<RwLock<HashMap<PeerId, BandwidthTracker>>>,
    resource_monitor: Arc<RwLock<ResourceMonitor>>,
    start_time: Instant,
}

impl DosProtectionManager {
    /// Create a new DoS protection manager
    pub fn new(config: DosProtectionConfig) -> Self {
        let rate_limiter_config = RateLimiterConfig {
            per_peer_rate: 100,
            per_peer_burst: 10,
            global_rate: 1000,
            global_burst: 100,
            enabled: true,
        };

        let circuit_breaker_config = CircuitBreakerConfig::default();

        Self {
            rate_limiter: RateLimiter::from_config(rate_limiter_config),
            circuit_breaker: CircuitBreakerManager::new(circuit_breaker_config),
            peer_reputations: Arc::new(RwLock::new(HashMap::new())),
            ip_trackers: Arc::new(RwLock::new(HashMap::new())),
            bandwidth_trackers: Arc::new(RwLock::new(HashMap::new())),
            resource_monitor: Arc::new(RwLock::new(ResourceMonitor::default())),
            config,
            start_time: Instant::now(),
        }
    }

    /// Start the DoS protection manager
    pub async fn start(&self) -> P2PResult<()> {
        info!("Starting DoS protection manager");

        // Start circuit breaker
        self.circuit_breaker.start().await?;

        // Start monitoring tasks
        self.start_monitoring_tasks().await;

        Ok(())
    }

    /// Check if a connection from an IP should be allowed
    pub async fn check_connection(&self, ip: IpAddr) -> P2PResult<()> {
        if !self.config.enable_connection_protection {
            return Ok(());
        }

        let mut ip_trackers = self.ip_trackers.write().await;
        let tracker = ip_trackers
            .entry(ip)
            .or_insert_with(IpConnectionTracker::default);

        // Check if IP is banned
        if tracker.is_banned {
            if let Some(ban_expiry) = tracker.ban_expiry {
                if Instant::now() < ban_expiry {
                    return Err(P2PError::ConnectionBlocked(format!("IP {ip} is banned")));
                } else {
                    // Ban expired, reset
                    tracker.is_banned = false;
                    tracker.ban_expiry = None;
                }
            } else {
                return Err(P2PError::ConnectionBlocked(format!(
                    "IP {ip} is permanently banned"
                )));
            }
        }

        // Check connection limit
        if tracker.connections >= self.config.max_connections_per_ip as u32 {
            warn!("Connection limit exceeded for IP: {}", ip);
            return Err(P2PError::ConnectionBlocked(format!(
                "Too many connections from IP {ip}"
            )));
        }

        // Check connection rate limit
        let now = Instant::now();
        tracker
            .connection_times
            .retain(|&time| now.duration_since(time) < Duration::from_secs(60));

        if tracker.connection_times.len() >= self.config.connection_rate_limit as usize {
            warn!("Connection rate limit exceeded for IP: {}", ip);
            // Temporarily ban this IP
            tracker.is_banned = true;
            tracker.ban_expiry = Some(now + Duration::from_secs(300)); // 5 minute ban
            return Err(P2PError::ConnectionBlocked(format!(
                "Connection rate limit exceeded for IP {ip}"
            )));
        }

        // Record connection
        tracker.connections += 1;
        tracker.connection_times.push(now);
        tracker.last_activity = now;

        Ok(())
    }

    /// Record connection close for an IP
    pub async fn record_connection_close(&self, ip: IpAddr) {
        let mut ip_trackers = self.ip_trackers.write().await;
        if let Some(tracker) = ip_trackers.get_mut(&ip) {
            tracker.connections = tracker.connections.saturating_sub(1);
        }
    }

    /// Check if a message from a peer should be allowed
    pub async fn check_message(&self, peer_id: PeerId, message_size: usize) -> P2PResult<()> {
        // Check circuit breaker
        if !self.circuit_breaker.can_execute(peer_id).await {
            return Err(P2PError::ConnectionBlocked(format!(
                "Circuit breaker open for peer {peer_id}"
            )));
        }

        // Check message size
        if self.config.enable_message_size_validation && message_size > self.config.max_message_size
        {
            warn!(
                "Oversized message from peer {}: {} bytes",
                peer_id, message_size
            );
            self.record_violation(peer_id, "oversized_message").await;
            return Err(P2PError::MessageTooLarge(message_size));
        }

        // Check peer reputation
        if self.config.enable_reputation_system {
            let reputation = self.get_peer_reputation(peer_id).await;
            if reputation.is_banned {
                return Err(P2PError::ConnectionBlocked(format!(
                    "Peer {peer_id} is banned"
                )));
            }

            if reputation.score < self.config.min_reputation_score {
                warn!(
                    "Low reputation peer {} (score: {:.2})",
                    peer_id, reputation.score
                );
                return Err(P2PError::ConnectionBlocked(format!(
                    "Peer {peer_id} reputation too low"
                )));
            }
        }

        // Check bandwidth limit
        if self.config.enable_bandwidth_limiting {
            let mut bandwidth_trackers = self.bandwidth_trackers.write().await;
            let tracker = bandwidth_trackers
                .entry(peer_id)
                .or_insert_with(|| BandwidthTracker::new(Duration::from_secs(1)));

            tracker.record_bytes(message_size as u64);

            if tracker.get_current_bandwidth() > self.config.max_bandwidth_per_peer {
                warn!("Bandwidth limit exceeded for peer {}", peer_id);
                self.record_violation(peer_id, "bandwidth_exceeded").await;
                return Err(P2PError::RateLimitExceeded(format!(
                    "Bandwidth limit exceeded for peer {peer_id}"
                )));
            }
        }

        // Check emergency mode
        {
            let resource_monitor = self.resource_monitor.read().await;
            if resource_monitor.emergency_mode {
                // In emergency mode, only allow high-reputation peers
                let reputation = self.get_peer_reputation(peer_id).await;
                if reputation.trust_level != TrustLevel::Trusted && reputation.score < 0.8 {
                    return Err(P2PError::ConnectionBlocked(
                        "Emergency mode: only trusted peers allowed".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Record a successful interaction with a peer
    pub async fn record_success(&self, peer_id: PeerId, response_time: Duration) {
        // Update circuit breaker
        self.circuit_breaker
            .record_success(peer_id, response_time)
            .await;

        // Update reputation
        self.update_reputation(peer_id, true, response_time).await;
    }

    /// Record a failed interaction with a peer
    pub async fn record_failure(&self, peer_id: PeerId, error: String, response_time: Duration) {
        // Update circuit breaker
        self.circuit_breaker
            .record_failure(peer_id, error, response_time)
            .await;

        // Update reputation
        self.update_reputation(peer_id, false, response_time).await;
    }

    /// Record a violation by a peer
    pub async fn record_violation(&self, peer_id: PeerId, violation_type: &str) {
        warn!(
            "Recording violation for peer {}: {}",
            peer_id, violation_type
        );

        let mut reputations = self.peer_reputations.write().await;
        let reputation = reputations
            .entry(peer_id)
            .or_insert_with(PeerReputation::default);

        // Severe penalty for violations
        reputation.score = (reputation.score - 0.2).max(0.0);
        reputation.failed_interactions += 1;
        reputation.last_updated = Instant::now();

        // Ban peer if reputation too low
        if reputation.score < 0.1 {
            reputation.is_banned = true;
            reputation.ban_expiry = Some(Instant::now() + Duration::from_secs(3600)); // 1 hour ban
            reputation.trust_level = TrustLevel::Malicious;
            warn!("Banned peer {} due to low reputation", peer_id);
        } else if reputation.score < 0.3 {
            reputation.trust_level = TrustLevel::Suspicious;
        }
    }

    /// Add a trusted peer (whitelist)
    pub async fn add_trusted_peer(&self, peer_id: PeerId) {
        let mut reputations = self.peer_reputations.write().await;
        let reputation = reputations
            .entry(peer_id)
            .or_insert_with(PeerReputation::default);

        reputation.trust_level = TrustLevel::Trusted;
        reputation.score = 1.0;
        reputation.is_banned = false;
        reputation.ban_expiry = None;

        info!("Added trusted peer: {}", peer_id);
    }

    /// Ban a peer (blacklist)
    pub async fn ban_peer(&self, peer_id: PeerId, duration: Option<Duration>) {
        let mut reputations = self.peer_reputations.write().await;
        let reputation = reputations
            .entry(peer_id)
            .or_insert_with(PeerReputation::default);

        reputation.is_banned = true;
        reputation.ban_expiry = duration.map(|d| Instant::now() + d);
        reputation.trust_level = TrustLevel::Malicious;
        reputation.score = 0.0;

        info!("Banned peer: {} (duration: {:?})", peer_id, duration);
    }

    /// Get peer reputation
    async fn get_peer_reputation(&self, peer_id: PeerId) -> PeerReputation {
        let reputations = self.peer_reputations.read().await;
        reputations.get(&peer_id).cloned().unwrap_or_default()
    }

    /// Update peer reputation based on interaction outcome
    async fn update_reputation(&self, peer_id: PeerId, success: bool, response_time: Duration) {
        let mut reputations = self.peer_reputations.write().await;
        let reputation = reputations
            .entry(peer_id)
            .or_insert_with(PeerReputation::default);

        if success {
            reputation.successful_interactions += 1;
            // Increase reputation for successful interactions
            reputation.score = (reputation.score + 0.01).min(1.0);

            // Fast response times get bonus
            if response_time < Duration::from_millis(100) {
                reputation.score = (reputation.score + 0.005).min(1.0);
            }
        } else {
            reputation.failed_interactions += 1;
            // Decrease reputation for failures
            reputation.score = (reputation.score - 0.05).max(0.0);
        }

        // Handle timeouts
        if response_time > Duration::from_secs(10) {
            reputation.timeouts += 1;
            reputation.score = (reputation.score - 0.02).max(0.0);
        }

        // Update trust level based on score
        if reputation.score > 0.8 && reputation.trust_level == TrustLevel::Unknown {
            reputation.trust_level = TrustLevel::Trusted;
        } else if reputation.score < 0.3 {
            reputation.trust_level = TrustLevel::Suspicious;
        }

        reputation.last_updated = Instant::now();
    }

    /// Start monitoring tasks for resource usage and adaptive protection
    async fn start_monitoring_tasks(&self) {
        // Resource monitoring task
        let resource_monitor = Arc::clone(&self.resource_monitor);
        let config = self.config.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop {
                interval.tick().await;
                Self::update_resource_monitor(&resource_monitor, &config).await;
            }
        });

        // Cleanup task
        let peer_reputations = Arc::clone(&self.peer_reputations);
        let ip_trackers = Arc::clone(&self.ip_trackers);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
            loop {
                interval.tick().await;
                Self::cleanup_old_data(&peer_reputations, &ip_trackers).await;
            }
        });
    }

    /// Update resource monitor and check for emergency conditions
    async fn update_resource_monitor(
        resource_monitor: &Arc<RwLock<ResourceMonitor>>,
        config: &DosProtectionConfig,
    ) {
        let mut monitor = resource_monitor.write().await;

        // In a real implementation, this would check actual system resources
        // For now, we'll simulate some basic monitoring
        monitor.memory_usage_mb = 128; // Placeholder
        monitor.cpu_usage = 0.2; // Placeholder
        monitor.network_io_rate = 1024 * 100; // Placeholder
        monitor.last_updated = Instant::now();

        // Check for emergency conditions
        let should_activate_emergency = monitor.memory_usage_mb > config.memory_limit_mb
            || monitor.cpu_usage > config.cpu_threshold;

        if should_activate_emergency && !monitor.emergency_mode {
            monitor.emergency_mode = true;
            monitor.emergency_start = Some(Instant::now());
            warn!("Activated emergency mode due to resource pressure");
        } else if monitor.emergency_mode {
            if let Some(start_time) = monitor.emergency_start {
                if start_time.elapsed() > config.emergency_mode_timeout
                    && !should_activate_emergency
                {
                    monitor.emergency_mode = false;
                    monitor.emergency_start = None;
                    info!("Deactivated emergency mode");
                }
            }
        }
    }

    /// Cleanup old data to prevent memory leaks
    async fn cleanup_old_data(
        peer_reputations: &Arc<RwLock<HashMap<PeerId, PeerReputation>>>,
        ip_trackers: &Arc<RwLock<HashMap<IpAddr, IpConnectionTracker>>>,
    ) {
        let cleanup_threshold = Duration::from_secs(3600); // 1 hour
        let now = Instant::now();

        // Cleanup old peer reputations
        {
            let mut reputations = peer_reputations.write().await;
            reputations.retain(|_, reputation| {
                // Keep if recently updated, trusted, or banned
                now.duration_since(reputation.last_updated) < cleanup_threshold
                    || reputation.trust_level == TrustLevel::Trusted
                    || reputation.is_banned
            });
        }

        // Cleanup old IP trackers
        {
            let mut trackers = ip_trackers.write().await;
            trackers.retain(|_, tracker| {
                // Keep if recently active or banned
                now.duration_since(tracker.last_activity) < cleanup_threshold || tracker.is_banned
            });
        }

        debug!("Completed DoS protection data cleanup");
    }

    /// Get comprehensive statistics
    pub async fn get_stats(&self) -> DosProtectionStats {
        let reputations = self.peer_reputations.read().await;
        let ip_trackers = self.ip_trackers.read().await;
        let resource_monitor = self.resource_monitor.read().await;

        let banned_peers = reputations.values().filter(|r| r.is_banned).count();
        let trusted_peers = reputations
            .values()
            .filter(|r| r.trust_level == TrustLevel::Trusted)
            .count();
        let suspicious_peers = reputations
            .values()
            .filter(|r| r.trust_level == TrustLevel::Suspicious)
            .count();

        let banned_ips = ip_trackers.values().filter(|t| t.is_banned).count();
        let active_connections: u32 = ip_trackers.values().map(|t| t.connections).sum();

        DosProtectionStats {
            total_peers: reputations.len(),
            banned_peers,
            trusted_peers,
            suspicious_peers,
            total_ips: ip_trackers.len(),
            banned_ips,
            active_connections,
            emergency_mode: resource_monitor.emergency_mode,
            memory_usage_mb: resource_monitor.memory_usage_mb,
            cpu_usage: resource_monitor.cpu_usage,
            uptime: self.start_time.elapsed(),
        }
    }
}

/// DoS protection statistics
#[derive(Debug, Clone)]
pub struct DosProtectionStats {
    pub total_peers: usize,
    pub banned_peers: usize,
    pub trusted_peers: usize,
    pub suspicious_peers: usize,
    pub total_ips: usize,
    pub banned_ips: usize,
    pub active_connections: u32,
    pub emergency_mode: bool,
    pub memory_usage_mb: usize,
    pub cpu_usage: f64,
    pub uptime: Duration,
}
