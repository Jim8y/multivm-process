//! Advanced Load Balancing for P2P Message Routing
//!
//! Provides sophisticated load balancing algorithms to optimize message distribution
//! across peers based on various metrics and strategies.

use crate::error::{P2PError, P2PResult};
use crate::messages::{MessagePayload, NetworkMessage, Priority};
use libp2p::PeerId;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Load balancing strategy
#[derive(Debug, Clone, PartialEq)]
pub enum LoadBalancingStrategy {
    /// Round-robin distribution
    RoundRobin,
    /// Weighted round-robin based on peer capacity
    WeightedRoundRobin,
    /// Least connections - route to peer with fewest active connections
    LeastConnections,
    /// Least response time - route to fastest responding peer
    LeastResponseTime,
    /// Consistent hashing for sticky routing
    ConsistentHash,
    /// Adaptive routing based on real-time metrics
    Adaptive,
    /// Random selection with weighted probability
    WeightedRandom,
    /// Geographic proximity routing
    Geographic,
}

/// Peer performance metrics
#[derive(Debug, Clone)]
pub struct PeerMetrics {
    /// Peer ID
    pub peer_id: PeerId,
    /// Number of active connections
    pub active_connections: u32,
    /// Average response time in milliseconds
    pub avg_response_time_ms: f64,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
    /// Current load factor (0.0 to 1.0, higher means more loaded)
    pub load_factor: f64,
    /// Bandwidth capacity in bytes per second
    pub bandwidth_capacity: u64,
    /// Current bandwidth utilization
    pub bandwidth_utilization: f64,
    /// Geographic region identifier
    pub region: Option<String>,
    /// Peer reliability score
    pub reliability_score: f64,
    /// Last update timestamp
    pub last_updated: Instant,
    /// Message queue depth
    pub queue_depth: u32,
    /// CPU utilization percentage
    pub cpu_utilization: f64,
    /// Memory utilization percentage
    pub memory_utilization: f64,
}

impl PeerMetrics {
    fn new(peer_id: PeerId) -> Self {
        Self {
            peer_id,
            active_connections: 0,
            avg_response_time_ms: 100.0,
            success_rate: 1.0,
            load_factor: 0.0,
            bandwidth_capacity: 1_000_000, // 1 Mbps default
            bandwidth_utilization: 0.0,
            region: None,
            reliability_score: 1.0,
            last_updated: Instant::now(),
            queue_depth: 0,
            cpu_utilization: 0.0,
            memory_utilization: 0.0,
        }
    }

    /// Calculate overall peer score for load balancing decisions
    fn calculate_score(&self, strategy: &LoadBalancingStrategy) -> f64 {
        match strategy {
            LoadBalancingStrategy::LeastConnections => 1.0 / (1.0 + self.active_connections as f64),
            LoadBalancingStrategy::LeastResponseTime => {
                1.0 / (1.0 + self.avg_response_time_ms / 100.0)
            }
            LoadBalancingStrategy::WeightedRoundRobin => {
                self.reliability_score * (1.0 - self.load_factor)
            }
            LoadBalancingStrategy::Adaptive => {
                let response_factor = 1.0 / (1.0 + self.avg_response_time_ms / 100.0);
                let load_factor = 1.0 - self.load_factor;
                let reliability_factor = self.reliability_score;
                let bandwidth_factor = 1.0 - self.bandwidth_utilization;
                let queue_factor = 1.0 / (1.0 + self.queue_depth as f64 / 10.0);

                (response_factor
                    + load_factor
                    + reliability_factor
                    + bandwidth_factor
                    + queue_factor)
                    / 5.0
            }
            LoadBalancingStrategy::WeightedRandom => self.success_rate * (1.0 - self.load_factor),
            _ => 1.0, // Default score for other strategies
        }
    }

    /// Check if peer metrics are stale
    fn is_stale(&self, max_age: Duration) -> bool {
        self.last_updated.elapsed() > max_age
    }
}

/// Load balancer configuration
#[derive(Debug, Clone)]
pub struct LoadBalancerConfig {
    /// Primary load balancing strategy
    pub strategy: LoadBalancingStrategy,
    /// Fallback strategy if primary fails
    pub fallback_strategy: LoadBalancingStrategy,
    /// Maximum age of peer metrics before considered stale
    pub metrics_max_age: Duration,
    /// Metrics update interval
    pub metrics_update_interval: Duration,
    /// Enable health-based routing
    pub enable_health_routing: bool,
    /// Minimum peer reliability score to consider for routing
    pub min_reliability_score: f64,
    /// Maximum load factor before considering peer overloaded
    pub max_load_factor: f64,
    /// Enable sticky sessions for consistent hashing
    pub enable_sticky_sessions: bool,
    /// Geographic preference (route to peers in same region first)
    pub prefer_local_region: bool,
    /// Local region identifier
    pub local_region: Option<String>,
}

impl Default for LoadBalancerConfig {
    fn default() -> Self {
        Self {
            strategy: LoadBalancingStrategy::Adaptive,
            fallback_strategy: LoadBalancingStrategy::WeightedRoundRobin,
            metrics_max_age: Duration::from_secs(300), // 5 minutes
            metrics_update_interval: Duration::from_secs(30),
            enable_health_routing: true,
            min_reliability_score: 0.5,
            max_load_factor: 0.8,
            enable_sticky_sessions: false,
            prefer_local_region: true,
            local_region: None,
        }
    }
}

/// Round-robin state for peers
#[derive(Debug)]
struct RoundRobinState {
    current_index: usize,
    peer_order: Vec<PeerId>,
}

impl RoundRobinState {
    fn new() -> Self {
        Self {
            current_index: 0,
            peer_order: Vec::new(),
        }
    }

    fn update_peers(&mut self, peers: Vec<PeerId>) {
        self.peer_order = peers;
        if self.current_index >= self.peer_order.len() {
            self.current_index = 0;
        }
    }

    fn next_peer(&mut self) -> Option<PeerId> {
        if self.peer_order.is_empty() {
            return None;
        }

        let peer = self.peer_order[self.current_index];
        self.current_index = (self.current_index + 1) % self.peer_order.len();
        Some(peer)
    }
}

/// Advanced load balancer for P2P routing
pub struct LoadBalancer {
    /// Configuration
    config: LoadBalancerConfig,
    /// Peer metrics
    peer_metrics: Arc<RwLock<HashMap<PeerId, PeerMetrics>>>,
    /// Round-robin state
    round_robin_state: Arc<RwLock<RoundRobinState>>,
    /// Consistent hashing ring (for sticky sessions)
    hash_ring: Arc<RwLock<Vec<(u64, PeerId)>>>,
    /// Message routing history for adaptive learning
    routing_history: Arc<RwLock<VecDeque<RoutingDecision>>>,
}

/// Record of a routing decision for learning
#[derive(Debug, Clone)]
struct RoutingDecision {
    peer_id: PeerId,
    message_priority: Priority,
    timestamp: Instant,
    response_time: Option<Duration>,
    success: bool,
}

impl LoadBalancer {
    /// Create a new load balancer
    pub fn new(config: LoadBalancerConfig) -> Self {
        Self {
            config,
            peer_metrics: Arc::new(RwLock::new(HashMap::new())),
            round_robin_state: Arc::new(RwLock::new(RoundRobinState::new())),
            hash_ring: Arc::new(RwLock::new(Vec::new())),
            routing_history: Arc::new(RwLock::new(VecDeque::new())),
        }
    }

    /// Start the load balancer (begins metric collection)
    pub async fn start(&self) -> P2PResult<()> {
        debug!(
            "Starting load balancer with strategy: {:?}",
            self.config.strategy
        );

        // Start metrics cleanup task
        let peer_metrics = Arc::clone(&self.peer_metrics);
        let max_age = self.config.metrics_max_age;
        let update_interval = self.config.metrics_update_interval;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(update_interval);
            loop {
                interval.tick().await;
                Self::cleanup_stale_metrics(&peer_metrics, max_age).await;
            }
        });

        Ok(())
    }

    /// Select the best peer for routing a message
    pub async fn select_peer(
        &self,
        message: &NetworkMessage,
        available_peers: &[PeerId],
    ) -> P2PResult<Option<PeerId>> {
        if available_peers.is_empty() {
            return Ok(None);
        }

        // Filter peers based on health and reliability
        let healthy_peers = self.filter_healthy_peers(available_peers).await;
        if healthy_peers.is_empty() {
            warn!("No healthy peers available for routing");
            return Ok(None);
        }

        // Apply geographic filtering if enabled
        let region_filtered_peers = if self.config.prefer_local_region {
            self.filter_by_region(&healthy_peers).await
        } else {
            healthy_peers.clone()
        };

        let final_peers = if region_filtered_peers.is_empty() {
            healthy_peers // Fall back to all healthy peers
        } else {
            region_filtered_peers
        };

        // Select peer based on strategy
        let selected_peer = match self.config.strategy {
            LoadBalancingStrategy::RoundRobin => self.select_round_robin(&final_peers).await,
            LoadBalancingStrategy::WeightedRoundRobin => {
                self.select_weighted_round_robin(&final_peers).await
            }
            LoadBalancingStrategy::LeastConnections => {
                self.select_least_connections(&final_peers).await
            }
            LoadBalancingStrategy::LeastResponseTime => {
                self.select_least_response_time(&final_peers).await
            }
            LoadBalancingStrategy::ConsistentHash => {
                self.select_consistent_hash(message, &final_peers).await
            }
            LoadBalancingStrategy::Adaptive => self.select_adaptive(&final_peers, message).await,
            LoadBalancingStrategy::WeightedRandom => {
                self.select_weighted_random(&final_peers).await
            }
            LoadBalancingStrategy::Geographic => self.select_geographic(&final_peers).await,
        };

        // Record routing decision
        if let Some(peer_id) = selected_peer {
            // Use default priority for routing decision tracking
            self.record_routing_decision(peer_id, Priority::Normal)
                .await;
        }

        Ok(selected_peer)
    }

    /// Update peer metrics
    pub async fn update_peer_metrics(&self, peer_id: PeerId, metrics: PeerMetrics) {
        let mut peer_metrics = self.peer_metrics.write().await;
        peer_metrics.insert(peer_id, metrics);
    }

    /// Record message response for adaptive learning
    pub async fn record_response(&self, peer_id: PeerId, response_time: Duration, success: bool) {
        // Update peer metrics
        {
            let mut peer_metrics = self.peer_metrics.write().await;
            if let Some(metrics) = peer_metrics.get_mut(&peer_id) {
                // Update average response time using exponential moving average
                let alpha = 0.1; // Smoothing factor
                metrics.avg_response_time_ms = alpha * response_time.as_millis() as f64
                    + (1.0 - alpha) * metrics.avg_response_time_ms;

                // Update success rate
                metrics.success_rate = alpha * (if success { 1.0 } else { 0.0 })
                    + (1.0 - alpha) * metrics.success_rate;

                metrics.last_updated = Instant::now();
            }
        }

        // Update routing history
        {
            let mut history = self.routing_history.write().await;
            history.push_back(RoutingDecision {
                peer_id,
                message_priority: Priority::Normal, // Default, could be passed as parameter
                timestamp: Instant::now(),
                response_time: Some(response_time),
                success,
            });

            // Keep history bounded
            if history.len() > 1000 {
                history.pop_front();
            }
        }
    }

    /// Get current load balancing statistics
    pub async fn get_stats(&self) -> LoadBalancingStats {
        let peer_metrics = self.peer_metrics.read().await;
        let history = self.routing_history.read().await;

        let total_peers = peer_metrics.len();
        let healthy_peers = peer_metrics
            .values()
            .filter(|m| {
                !m.is_stale(self.config.metrics_max_age)
                    && m.reliability_score >= self.config.min_reliability_score
            })
            .count();

        let total_requests = history.len();
        let successful_requests = history.iter().filter(|r| r.success).count();

        let avg_response_time = if !history.is_empty() {
            history
                .iter()
                .filter_map(|r| r.response_time)
                .map(|rt| rt.as_millis() as f64)
                .sum::<f64>()
                / history.len() as f64
        } else {
            0.0
        };

        LoadBalancingStats {
            strategy: self.config.strategy.clone(),
            total_peers,
            healthy_peers,
            total_requests,
            successful_requests,
            success_rate: if total_requests > 0 {
                successful_requests as f64 / total_requests as f64
            } else {
                1.0
            },
            avg_response_time_ms: avg_response_time,
        }
    }

    // Private helper methods for different selection strategies

    async fn filter_healthy_peers(&self, peers: &[PeerId]) -> Vec<PeerId> {
        if !self.config.enable_health_routing {
            return peers.to_vec();
        }

        let peer_metrics = self.peer_metrics.read().await;
        peers
            .iter()
            .filter(|&peer_id| {
                if let Some(metrics) = peer_metrics.get(peer_id) {
                    !metrics.is_stale(self.config.metrics_max_age)
                        && metrics.reliability_score >= self.config.min_reliability_score
                        && metrics.load_factor <= self.config.max_load_factor
                } else {
                    true // Include peers without metrics (new peers)
                }
            })
            .cloned()
            .collect()
    }

    async fn filter_by_region(&self, peers: &[PeerId]) -> Vec<PeerId> {
        let Some(local_region) = &self.config.local_region else {
            return peers.to_vec();
        };

        let peer_metrics = self.peer_metrics.read().await;
        let local_peers: Vec<PeerId> = peers
            .iter()
            .filter(|&peer_id| {
                peer_metrics.get(peer_id).and_then(|m| m.region.as_ref()) == Some(local_region)
            })
            .cloned()
            .collect();

        if local_peers.is_empty() {
            peers.to_vec() // Fall back to all peers
        } else {
            local_peers
        }
    }

    async fn select_round_robin(&self, peers: &[PeerId]) -> Option<PeerId> {
        let mut state = self.round_robin_state.write().await;
        state.update_peers(peers.to_vec());
        state.next_peer()
    }

    async fn select_weighted_round_robin(&self, peers: &[PeerId]) -> Option<PeerId> {
        let peer_metrics = self.peer_metrics.read().await;
        let mut weighted_peers = Vec::new();

        for &peer_id in peers {
            let weight = if let Some(metrics) = peer_metrics.get(&peer_id) {
                (metrics.calculate_score(&LoadBalancingStrategy::WeightedRoundRobin) * 10.0)
                    as usize
            } else {
                5 // Default weight for peers without metrics
            };

            for _ in 0..weight.max(1) {
                weighted_peers.push(peer_id);
            }
        }

        if weighted_peers.is_empty() {
            return None;
        }

        let mut state = self.round_robin_state.write().await;
        state.update_peers(weighted_peers);
        state.next_peer()
    }

    async fn select_least_connections(&self, peers: &[PeerId]) -> Option<PeerId> {
        let peer_metrics = self.peer_metrics.read().await;
        peers
            .iter()
            .min_by_key(|&peer_id| {
                peer_metrics
                    .get(peer_id)
                    .map(|m| m.active_connections)
                    .unwrap_or(0)
            })
            .cloned()
    }

    async fn select_least_response_time(&self, peers: &[PeerId]) -> Option<PeerId> {
        let peer_metrics = self.peer_metrics.read().await;
        peers
            .iter()
            .min_by(|&a, &b| {
                let a_time = peer_metrics
                    .get(a)
                    .map(|m| m.avg_response_time_ms)
                    .unwrap_or(1000.0);
                let b_time = peer_metrics
                    .get(b)
                    .map(|m| m.avg_response_time_ms)
                    .unwrap_or(1000.0);
                a_time
                    .partial_cmp(&b_time)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
    }

    async fn select_consistent_hash(
        &self,
        message: &NetworkMessage,
        peers: &[PeerId],
    ) -> Option<PeerId> {
        if !self.config.enable_sticky_sessions {
            return self.select_round_robin(peers).await;
        }

        // Use message ID for consistent hashing
        let hash = self.hash_string(&message.id);
        let hash_ring = self.hash_ring.read().await;

        if hash_ring.is_empty() {
            return self.select_round_robin(peers).await;
        }

        // Find the first peer whose hash is greater than or equal to the message hash
        for (peer_hash, peer_id) in hash_ring.iter() {
            if *peer_hash >= hash && peers.contains(peer_id) {
                return Some(*peer_id);
            }
        }

        // Wrap around to the beginning
        hash_ring
            .iter()
            .find(|(_, peer_id)| peers.contains(peer_id))
            .map(|(_, peer_id)| *peer_id)
    }

    async fn select_adaptive(&self, peers: &[PeerId], message: &NetworkMessage) -> Option<PeerId> {
        let peer_metrics = self.peer_metrics.read().await;

        // Adaptive selection based on message type and peer performance
        // Extract priority from metadata or use default based on message type
        let priority_weight = if let Some(priority_str) = message.metadata.get("priority") {
            match priority_str.as_str() {
                "critical" => 2.0,
                "high" => 1.5,
                "normal" => 1.0,
                "low" => 0.8,
                _ => 1.0,
            }
        } else {
            // Default priority based on message type
            match &message.payload {
                MessagePayload::Control(_) => 1.5, // Control messages are high priority
                MessagePayload::Discovery(_) => 1.0, // Normal priority
                _ => 1.0,                          // Normal priority for VM messages
            }
        };

        peers
            .iter()
            .max_by(|&a, &b| {
                let a_score = peer_metrics
                    .get(a)
                    .map(|m| m.calculate_score(&LoadBalancingStrategy::Adaptive) * priority_weight)
                    .unwrap_or(0.5);
                let b_score = peer_metrics
                    .get(b)
                    .map(|m| m.calculate_score(&LoadBalancingStrategy::Adaptive) * priority_weight)
                    .unwrap_or(0.5);
                a_score
                    .partial_cmp(&b_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
    }

    async fn select_weighted_random(&self, peers: &[PeerId]) -> Option<PeerId> {
        let peer_metrics = self.peer_metrics.read().await;
        let mut weights = Vec::new();
        let mut total_weight = 0.0;

        for &peer_id in peers {
            let weight = peer_metrics
                .get(&peer_id)
                .map(|m| m.calculate_score(&LoadBalancingStrategy::WeightedRandom))
                .unwrap_or(0.5);
            weights.push(weight);
            total_weight += weight;
        }

        if total_weight <= 0.0 {
            return peers.first().cloned();
        }

        let random_value = rand::random::<f64>() * total_weight;
        let mut cumulative_weight = 0.0;

        for (i, weight) in weights.iter().enumerate() {
            cumulative_weight += weight;
            if random_value <= cumulative_weight {
                return Some(peers[i]);
            }
        }

        peers.last().cloned()
    }

    async fn select_geographic(&self, peers: &[PeerId]) -> Option<PeerId> {
        // For geographic selection, prefer peers in the same region first
        let local_peers = self.filter_by_region(peers).await;
        if !local_peers.is_empty() {
            self.select_least_response_time(&local_peers).await
        } else {
            self.select_least_response_time(peers).await
        }
    }

    async fn record_routing_decision(&self, peer_id: PeerId, priority: Priority) {
        let mut history = self.routing_history.write().await;
        history.push_back(RoutingDecision {
            peer_id,
            message_priority: priority,
            timestamp: Instant::now(),
            response_time: None,
            success: true, // Will be updated when response is received
        });

        // Keep history bounded
        if history.len() > 1000 {
            history.pop_front();
        }
    }

    async fn cleanup_stale_metrics(
        peer_metrics: &Arc<RwLock<HashMap<PeerId, PeerMetrics>>>,
        max_age: Duration,
    ) {
        let mut metrics = peer_metrics.write().await;
        metrics.retain(|_, metric| !metric.is_stale(max_age));
    }

    fn hash_string(&self, s: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    }
}

/// Load balancing statistics
#[derive(Debug, Clone)]
pub struct LoadBalancingStats {
    pub strategy: LoadBalancingStrategy,
    pub total_peers: usize,
    pub healthy_peers: usize,
    pub total_requests: usize,
    pub successful_requests: usize,
    pub success_rate: f64,
    pub avg_response_time_ms: f64,
}

