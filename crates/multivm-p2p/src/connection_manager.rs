//! Connection Pool Manager for P2P networking
//!
//! Provides explicit connection lifecycle management, pooling, and health monitoring
//! for P2P connections to improve performance and resource utilization.

use crate::error::{P2PError, P2PResult};
use libp2p::{Multiaddr, PeerId};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Configuration for connection pool management
#[derive(Debug, Clone)]
pub struct ConnectionPoolConfig {
    /// Maximum number of connections per peer
    pub max_connections_per_peer: usize,
    /// Minimum number of connections to maintain globally
    pub min_total_connections: usize,
    /// Maximum number of connections globally
    pub max_total_connections: usize,
    /// Connection idle timeout
    pub idle_timeout: Duration,
    /// Connection establishment timeout
    pub connection_timeout: Duration,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Maximum failed connection attempts before backoff
    pub max_failed_attempts: u32,
    /// Backoff multiplier for failed connections
    pub backoff_multiplier: f64,
    /// Maximum backoff duration
    pub max_backoff: Duration,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            max_connections_per_peer: 3,
            min_total_connections: 5,
            max_total_connections: 100,
            idle_timeout: Duration::from_secs(300), // 5 minutes
            connection_timeout: Duration::from_secs(10),
            health_check_interval: Duration::from_secs(30),
            max_failed_attempts: 3,
            backoff_multiplier: 2.0,
            max_backoff: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Connection state in the pool
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// Connection is being established
    Connecting,
    /// Connection is active and healthy
    Active,
    /// Connection is idle but available
    Idle,
    /// Connection is being used for data transfer
    InUse,
    /// Connection has failed and is being retried
    Failed,
    /// Connection is being gracefully closed
    Closing,
    /// Connection is closed
    Closed,
}

/// Information about a connection in the pool
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// Peer ID
    pub peer_id: PeerId,
    /// Connection address
    pub address: Multiaddr,
    /// Current state
    pub state: ConnectionState,
    /// When the connection was established
    pub established_at: Instant,
    /// Last activity timestamp
    pub last_activity: Instant,
    /// Number of failed connection attempts
    pub failed_attempts: u32,
    /// Next retry time (if failed)
    pub next_retry: Option<Instant>,
    /// Bytes sent over this connection
    pub bytes_sent: u64,
    /// Bytes received over this connection
    pub bytes_received: u64,
    /// Connection priority score
    pub priority_score: f64,
}

impl ConnectionInfo {
    fn new(peer_id: PeerId, address: Multiaddr) -> Self {
        let now = Instant::now();
        Self {
            peer_id,
            address,
            state: ConnectionState::Connecting,
            established_at: now,
            last_activity: now,
            failed_attempts: 0,
            next_retry: None,
            bytes_sent: 0,
            bytes_received: 0,
            priority_score: 1.0,
        }
    }

    /// Check if connection is healthy
    pub fn is_healthy(&self, idle_timeout: Duration) -> bool {
        match self.state {
            ConnectionState::Active | ConnectionState::Idle | ConnectionState::InUse => {
                self.last_activity.elapsed() < idle_timeout
            }
            _ => false,
        }
    }

    /// Check if connection should be retried
    pub fn should_retry(&self, max_attempts: u32) -> bool {
        self.state == ConnectionState::Failed
            && self.failed_attempts < max_attempts
            && self
                .next_retry
                .is_none_or(|retry_time| Instant::now() >= retry_time)
    }

    /// Calculate priority score based on performance metrics
    pub fn calculate_priority(&mut self) {
        let age_factor = 1.0 / (1.0 + self.established_at.elapsed().as_secs() as f64 / 3600.0);
        let activity_factor = 1.0 / (1.0 + self.last_activity.elapsed().as_secs() as f64 / 60.0);
        let reliability_factor = 1.0 / (1.0 + self.failed_attempts as f64);
        let throughput_factor = (self.bytes_sent + self.bytes_received) as f64 / 1024.0 / 1024.0; // MB

        self.priority_score =
            (age_factor + activity_factor + reliability_factor + throughput_factor.ln().max(0.0))
                / 4.0;
    }
}

/// Connection pool statistics
#[derive(Debug, Clone, Default)]
pub struct ConnectionPoolStats {
    /// Total number of connections
    pub total_connections: usize,
    /// Active connections
    pub active_connections: usize,
    /// Idle connections
    pub idle_connections: usize,
    /// Failed connections
    pub failed_connections: usize,
    /// Total bytes sent
    pub total_bytes_sent: u64,
    /// Total bytes received
    pub total_bytes_received: u64,
    /// Average connection age
    pub avg_connection_age_secs: f64,
    /// Connection success rate
    pub connection_success_rate: f64,
}

/// Connection Pool Manager
pub struct ConnectionPoolManager {
    /// Configuration
    config: ConnectionPoolConfig,
    /// Active connections by peer ID
    connections: Arc<RwLock<HashMap<PeerId, Vec<ConnectionInfo>>>>,
    /// Connection statistics
    stats: Arc<RwLock<ConnectionPoolStats>>,
    /// Total connection attempts
    total_attempts: Arc<RwLock<u64>>,
    /// Successful connections
    successful_connections: Arc<RwLock<u64>>,
}

impl ConnectionPoolManager {
    /// Create a new connection pool manager
    pub fn new(config: ConnectionPoolConfig) -> Self {
        Self {
            config,
            connections: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(ConnectionPoolStats::default())),
            total_attempts: Arc::new(RwLock::new(0)),
            successful_connections: Arc::new(RwLock::new(0)),
        }
    }

    /// Start the connection pool manager
    pub async fn start(&self) -> P2PResult<()> {
        info!("Starting connection pool manager");

        // Start health check task
        let connections = Arc::clone(&self.connections);
        let stats = Arc::clone(&self.stats);
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.health_check_interval);
            loop {
                interval.tick().await;
                if let Err(e) = Self::health_check_task(&connections, &stats, &config).await {
                    error!("Health check task error: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Request a connection to a peer
    pub async fn get_connection(&self, peer_id: PeerId) -> P2PResult<Option<ConnectionInfo>> {
        let mut connections = self.connections.write().await;

        if let Some(peer_connections) = connections.get_mut(&peer_id) {
            // Find the best available connection
            let mut best_connection = None;
            let mut best_score = 0.0;

            for (index, conn) in peer_connections.iter_mut().enumerate() {
                if conn.is_healthy(self.config.idle_timeout)
                    && matches!(conn.state, ConnectionState::Active | ConnectionState::Idle)
                {
                    conn.calculate_priority();
                    if conn.priority_score > best_score {
                        best_score = conn.priority_score;
                        best_connection = Some(index);
                    }
                }
            }

            if let Some(index) = best_connection {
                let conn = &mut peer_connections[index];
                conn.state = ConnectionState::InUse;
                conn.last_activity = Instant::now();
                return Ok(Some(conn.clone()));
            }
        }

        // No suitable connection found
        Ok(None)
    }

    /// Release a connection back to the pool
    pub async fn release_connection(&self, peer_id: PeerId, address: &Multiaddr) -> P2PResult<()> {
        let mut connections = self.connections.write().await;

        if let Some(peer_connections) = connections.get_mut(&peer_id) {
            for conn in peer_connections.iter_mut() {
                if conn.address == *address && conn.state == ConnectionState::InUse {
                    conn.state = ConnectionState::Idle;
                    conn.last_activity = Instant::now();
                    debug!("Released connection to peer {} at {}", peer_id, address);
                    return Ok(());
                }
            }
        }

        Err(P2PError::connection_error(format!(
            "Connection not found for peer {} at {}",
            peer_id, address
        )))
    }

    /// Add a new connection to the pool
    pub async fn add_connection(&self, peer_id: PeerId, address: Multiaddr) -> P2PResult<()> {
        let mut connections = self.connections.write().await;

        // Check global connection limit first
        let total_connections: usize = connections.values().map(|v| v.len()).sum();
        if total_connections >= self.config.max_total_connections {
            warn!("Global connection limit reached, rejecting new connection");
            return Err(P2PError::connection_error(
                "Global connection limit reached",
            ));
        }

        let peer_connections = connections.entry(peer_id).or_insert_with(Vec::new);

        // Check if we already have this connection
        if peer_connections.iter().any(|conn| conn.address == address) {
            return Ok(()); // Connection already exists
        }

        // Check peer connection limit
        if peer_connections.len() >= self.config.max_connections_per_peer {
            // Remove the lowest priority connection
            if let Some(lowest_index) = self.find_lowest_priority_connection(peer_connections) {
                let removed = peer_connections.remove(lowest_index);
                debug!(
                    "Removed low priority connection to make space: {:?}",
                    removed.address
                );
            }
        }

        // Add new connection
        let mut conn_info = ConnectionInfo::new(peer_id, address.clone());
        conn_info.state = ConnectionState::Active;
        peer_connections.push(conn_info);

        // Update statistics
        {
            let mut total_attempts = self.total_attempts.write().await;
            let mut successful = self.successful_connections.write().await;
            *total_attempts += 1;
            *successful += 1;
        }

        info!("Added new connection to peer {} at {}", peer_id, address);
        Ok(())
    }

    /// Remove a connection from the pool
    pub async fn remove_connection(&self, peer_id: PeerId, address: &Multiaddr) -> P2PResult<()> {
        let mut connections = self.connections.write().await;

        if let Some(peer_connections) = connections.get_mut(&peer_id) {
            peer_connections.retain(|conn| conn.address != *address);
            if peer_connections.is_empty() {
                connections.remove(&peer_id);
            }
            info!("Removed connection to peer {} at {}", peer_id, address);
        }

        Ok(())
    }

    /// Mark a connection as failed
    pub async fn mark_connection_failed(
        &self,
        peer_id: PeerId,
        address: &Multiaddr,
    ) -> P2PResult<()> {
        let mut connections = self.connections.write().await;

        if let Some(peer_connections) = connections.get_mut(&peer_id) {
            for conn in peer_connections.iter_mut() {
                if conn.address == *address {
                    conn.state = ConnectionState::Failed;
                    conn.failed_attempts += 1;

                    // Calculate backoff time
                    let backoff_duration = Duration::from_secs_f64(
                        (self
                            .config
                            .backoff_multiplier
                            .powi(conn.failed_attempts as i32)
                            * 1.0)
                            .min(self.config.max_backoff.as_secs_f64()),
                    );
                    conn.next_retry = Some(Instant::now() + backoff_duration);

                    warn!(
                        "Connection to peer {} at {} failed (attempt {}), next retry in {:?}",
                        peer_id, address, conn.failed_attempts, backoff_duration
                    );
                    return Ok(());
                }
            }
        }

        Err(P2PError::connection_error(format!(
            "Connection not found for peer {} at {}",
            peer_id, address
        )))
    }

    /// Update connection statistics
    pub async fn update_connection_stats(
        &self,
        peer_id: PeerId,
        address: &Multiaddr,
        bytes_sent: u64,
        bytes_received: u64,
    ) -> P2PResult<()> {
        let mut connections = self.connections.write().await;

        if let Some(peer_connections) = connections.get_mut(&peer_id) {
            for conn in peer_connections.iter_mut() {
                if conn.address == *address {
                    conn.bytes_sent += bytes_sent;
                    conn.bytes_received += bytes_received;
                    conn.last_activity = Instant::now();
                    return Ok(());
                }
            }
        }

        Ok(()) // Connection might have been removed, don't error
    }

    /// Get current pool statistics
    pub async fn get_stats(&self) -> ConnectionPoolStats {
        self.update_stats().await;
        self.stats.read().await.clone()
    }

    /// Get all connections for a peer
    pub async fn get_peer_connections(&self, peer_id: PeerId) -> Vec<ConnectionInfo> {
        let connections = self.connections.read().await;
        connections.get(&peer_id).cloned().unwrap_or_default()
    }

    /// Get all connected peers
    pub async fn get_connected_peers(&self) -> Vec<PeerId> {
        let connections = self.connections.read().await;
        connections.keys().cloned().collect()
    }

    /// Health check task
    async fn health_check_task(
        connections: &Arc<RwLock<HashMap<PeerId, Vec<ConnectionInfo>>>>,
        stats: &Arc<RwLock<ConnectionPoolStats>>,
        config: &ConnectionPoolConfig,
    ) -> P2PResult<()> {
        let mut connections = connections.write().await;
        let now = Instant::now();

        for (peer_id, peer_connections) in connections.iter_mut() {
            peer_connections.retain_mut(|conn| {
                // Remove unhealthy connections
                if !conn.is_healthy(config.idle_timeout) {
                    debug!(
                        "Removing unhealthy connection to peer {} at {}",
                        peer_id, conn.address
                    );
                    false
                } else {
                    // Update connection state based on activity
                    if conn.state == ConnectionState::InUse
                        && conn.last_activity.elapsed() > Duration::from_secs(60)
                    {
                        conn.state = ConnectionState::Idle;
                    }
                    true
                }
            });
        }

        // Remove peers with no connections
        connections.retain(|_, peer_connections| !peer_connections.is_empty());

        debug!(
            "Health check completed, {} peers remaining",
            connections.len()
        );
        Ok(())
    }

    /// Update internal statistics
    async fn update_stats(&self) {
        let connections = self.connections.read().await;
        let mut stats = self.stats.write().await;

        let mut total_connections = 0;
        let mut active_connections = 0;
        let mut idle_connections = 0;
        let mut failed_connections = 0;
        let mut total_bytes_sent = 0;
        let mut total_bytes_received = 0;
        let mut total_age_secs = 0.0;

        for peer_connections in connections.values() {
            for conn in peer_connections {
                total_connections += 1;
                total_bytes_sent += conn.bytes_sent;
                total_bytes_received += conn.bytes_received;
                total_age_secs += conn.established_at.elapsed().as_secs_f64();

                match conn.state {
                    ConnectionState::Active | ConnectionState::InUse => active_connections += 1,
                    ConnectionState::Idle => idle_connections += 1,
                    ConnectionState::Failed => failed_connections += 1,
                    _ => {}
                }
            }
        }

        let total_attempts = *self.total_attempts.read().await;
        let successful = *self.successful_connections.read().await;

        stats.total_connections = total_connections;
        stats.active_connections = active_connections;
        stats.idle_connections = idle_connections;
        stats.failed_connections = failed_connections;
        stats.total_bytes_sent = total_bytes_sent;
        stats.total_bytes_received = total_bytes_received;
        stats.avg_connection_age_secs = if total_connections > 0 {
            total_age_secs / total_connections as f64
        } else {
            0.0
        };
        stats.connection_success_rate = if total_attempts > 0 {
            successful as f64 / total_attempts as f64
        } else {
            1.0
        };
    }

    /// Find the lowest priority connection for removal
    fn find_lowest_priority_connection(&self, connections: &mut [ConnectionInfo]) -> Option<usize> {
        let mut lowest_index = None;
        let mut lowest_score = f64::INFINITY;

        for (index, conn) in connections.iter_mut().enumerate() {
            conn.calculate_priority();
            if conn.priority_score < lowest_score && conn.state != ConnectionState::InUse {
                lowest_score = conn.priority_score;
                lowest_index = Some(index);
            }
        }

        lowest_index
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::identity::Keypair;

    #[tokio::test]
    async fn test_connection_pool_basic() {
        let config = ConnectionPoolConfig::default();
        let pool = ConnectionPoolManager::new(config);

        let keypair = Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());
        let address: Multiaddr = "/ip4/127.0.0.1/tcp/8000".parse().unwrap();

        // Add connection
        assert!(pool.add_connection(peer_id, address.clone()).await.is_ok());

        // Get connection
        let conn = pool.get_connection(peer_id).await.unwrap();
        assert!(conn.is_some());

        // Release connection
        assert!(pool.release_connection(peer_id, &address).await.is_ok());

        // Remove connection
        assert!(pool.remove_connection(peer_id, &address).await.is_ok());
    }

    #[tokio::test]
    async fn test_connection_pool_limits() {
        let config = ConnectionPoolConfig {
            max_connections_per_peer: 2,
            ..Default::default()
        };
        let pool = ConnectionPoolManager::new(config);

        let keypair = Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());

        // Add maximum connections
        for i in 0..3 {
            let addr: Multiaddr = format!("/ip4/127.0.0.1/tcp/{}", 8000 + i).parse().unwrap();
            let result = pool.add_connection(peer_id, addr).await;
            if i < 2 {
                assert!(result.is_ok());
            }
        }

        let connections = pool.get_peer_connections(peer_id).await;
        assert_eq!(connections.len(), 2); // Should be limited to max_connections_per_peer
    }

    #[tokio::test]
    async fn test_connection_failure_handling() {
        let config = ConnectionPoolConfig::default();
        let pool = ConnectionPoolManager::new(config);

        let keypair = Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());
        let address: Multiaddr = "/ip4/127.0.0.1/tcp/8000".parse().unwrap();

        // Add and fail connection
        assert!(pool.add_connection(peer_id, address.clone()).await.is_ok());
        assert!(pool.mark_connection_failed(peer_id, &address).await.is_ok());

        let connections = pool.get_peer_connections(peer_id).await;
        assert_eq!(connections[0].state, ConnectionState::Failed);
        assert_eq!(connections[0].failed_attempts, 1);
    }

    #[tokio::test]
    async fn test_connection_stats() {
        let config = ConnectionPoolConfig::default();
        let pool = ConnectionPoolManager::new(config);

        let keypair = Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());
        let address: Multiaddr = "/ip4/127.0.0.1/tcp/8000".parse().unwrap();

        // Add connection and update stats
        assert!(pool.add_connection(peer_id, address.clone()).await.is_ok());
        assert!(pool
            .update_connection_stats(peer_id, &address, 1024, 2048)
            .await
            .is_ok());

        let stats = pool.get_stats().await;
        assert_eq!(stats.total_connections, 1);
        assert_eq!(stats.total_bytes_sent, 1024);
        assert_eq!(stats.total_bytes_received, 2048);
    }
}
