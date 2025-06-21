//! Message routing for P2P network

use anyhow::Result;
use libp2p::{Multiaddr, PeerId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

use crate::error::P2PError;
use crate::messages::{MessageType, NetworkMessage};

/// Message routing strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoutingStrategy {
    /// Broadcast to all connected peers
    Broadcast,
    /// Route to specific peer
    Direct(PeerId),
    /// Route using Kademlia DHT
    DHT(Vec<u8>), // Key for DHT lookup
    /// Route via GossipSub topic
    Gossip(String), // Topic name
    /// Route to random subset of peers
    Random(usize), // Number of peers
}

/// Route entry for message routing table
#[derive(Debug, Clone)]
struct RouteEntry {
    peer_id: PeerId,
    address: Multiaddr,
    last_seen: Instant,
    reliability_score: f64,
    message_count: u64,
}

/// Statistics for routing performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingStats {
    pub total_messages_routed: u64,
    pub successful_routes: u64,
    pub failed_routes: u64,
    pub active_routes: usize,
    pub avg_routing_time_ms: f64,
    pub peer_count: usize,
}

/// Message router with libp2p integration
pub struct MessageRouter {
    /// Routing table mapping message types to peers
    routing_table: Arc<RwLock<HashMap<MessageType, Vec<RouteEntry>>>>,
    /// GossipSub topics we're subscribed to
    subscribed_topics: Arc<RwLock<HashSet<String>>>,
    /// DHT for peer discovery and content routing
    dht_entries: Arc<RwLock<HashMap<Vec<u8>, PeerId>>>,
    /// Channel for sending routed messages
    message_sender: mpsc::Sender<(PeerId, NetworkMessage)>,
    /// Channel for receiving routing commands
    command_receiver: Arc<RwLock<Option<mpsc::Receiver<RoutingCommand>>>>,
    /// Routing statistics
    stats: Arc<RwLock<RoutingStats>>,
    /// Configuration
    config: RoutingConfig,
}

/// Routing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    /// Maximum number of peers per message type
    pub max_peers_per_type: usize,
    /// Route cleanup interval
    pub cleanup_interval: Duration,
    /// Peer reliability threshold
    pub reliability_threshold: f64,
    /// Maximum routing retries
    pub max_retries: u32,
    /// Routing timeout
    pub routing_timeout: Duration,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            max_peers_per_type: 50,
            cleanup_interval: Duration::from_secs(300), // 5 minutes
            reliability_threshold: 0.8,
            max_retries: 3,
            routing_timeout: Duration::from_secs(30),
        }
    }
}

/// Commands for controlling routing behavior
#[derive(Debug)]
pub enum RoutingCommand {
    /// Add a new route
    AddRoute {
        message_type: MessageType,
        peer_id: PeerId,
        address: Multiaddr,
    },
    /// Remove a route
    RemoveRoute {
        message_type: MessageType,
        peer_id: PeerId,
    },
    /// Subscribe to GossipSub topic
    Subscribe(String),
    /// Unsubscribe from GossipSub topic
    Unsubscribe(String),
    /// Update peer reliability
    UpdateReliability { peer_id: PeerId, success: bool },
    /// Get routing statistics
    GetStats(tokio::sync::oneshot::Sender<RoutingStats>),
}

impl MessageRouter {
    /// Create a new message router
    pub fn new(
        message_sender: mpsc::Sender<(PeerId, NetworkMessage)>,
        config: Option<RoutingConfig>,
    ) -> (Self, mpsc::Sender<RoutingCommand>) {
        let (command_sender, command_receiver) = mpsc::channel(100);

        let router = Self {
            routing_table: Arc::new(RwLock::new(HashMap::new())),
            subscribed_topics: Arc::new(RwLock::new(HashSet::new())),
            dht_entries: Arc::new(RwLock::new(HashMap::new())),
            message_sender,
            command_receiver: Arc::new(RwLock::new(Some(command_receiver))),
            stats: Arc::new(RwLock::new(RoutingStats {
                total_messages_routed: 0,
                successful_routes: 0,
                failed_routes: 0,
                active_routes: 0,
                avg_routing_time_ms: 0.0,
                peer_count: 0,
            })),
            config: config.unwrap_or_default(),
        };

        (router, command_sender)
    }

    /// Start the routing service
    pub async fn start(&self) -> Result<()> {
        info!("Starting message router");

        // Take the command receiver
        let mut command_receiver = self
            .command_receiver
            .write()
            .await
            .take()
            .ok_or_else(|| P2PError::Internal("Router already started".to_string()))?;

        // Start cleanup task
        let _cleanup_handle = self.start_cleanup_task().await;

        // Clone necessary fields for the task
        let routing_table = self.routing_table.clone();
        let subscribed_topics = self.subscribed_topics.clone();
        let dht_entries = self.dht_entries.clone();
        let message_sender = self.message_sender.clone();
        let stats = self.stats.clone();
        let config = self.config.clone();

        // Process routing commands
        tokio::spawn(async move {
            while let Some(command) = command_receiver.recv().await {
                if let Err(e) = Self::handle_command_static(
                    command,
                    &routing_table,
                    &subscribed_topics,
                    &dht_entries,
                    &message_sender,
                    &stats,
                    &config,
                )
                .await
                {
                    warn!("Error handling routing command: {}", e);
                }
            }
        });

        info!("Message router started");
        Ok(())
    }

    /// Start the cleanup task for stale routes
    async fn start_cleanup_task(&self) -> tokio::task::JoinHandle<()> {
        let cleanup_interval = self.config.cleanup_interval;
        let routing_table = self.routing_table.clone();
        let reliability_threshold = self.config.reliability_threshold;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);

            loop {
                interval.tick().await;

                // Clean up stale and unreliable routes
                let mut table = routing_table.write().await;
                for (msg_type, routes) in table.iter_mut() {
                    routes.retain(|route| {
                        let age = route.last_seen.elapsed();
                        let is_recent = age < Duration::from_secs(3600); // 1 hour
                        let is_reliable = route.reliability_score >= reliability_threshold;

                        if !is_recent || !is_reliable {
                            debug!(
                                "Removing stale route for {:?} to peer {}",
                                msg_type, route.peer_id
                            );
                        }

                        is_recent && is_reliable
                    });
                }

                // Remove empty entries
                table.retain(|_, routes| !routes.is_empty());
            }
        })
    }

    /// Handle routing commands
    async fn handle_command(&self, command: RoutingCommand) -> Result<()> {
        match command {
            RoutingCommand::AddRoute {
                message_type,
                peer_id,
                address,
            } => {
                self.add_route(message_type, peer_id, address).await?;
            }
            RoutingCommand::RemoveRoute {
                message_type,
                peer_id,
            } => {
                self.remove_route(message_type, peer_id).await?;
            }
            RoutingCommand::Subscribe(topic) => {
                self.subscribe_topic(topic).await?;
            }
            RoutingCommand::Unsubscribe(topic) => {
                self.unsubscribe_topic(topic).await?;
            }
            RoutingCommand::UpdateReliability { peer_id, success } => {
                self.update_peer_reliability(peer_id, success).await?;
            }
            RoutingCommand::GetStats(sender) => {
                let stats = self.get_routing_stats().await;
                let _ = sender.send(stats);
            }
        }
        Ok(())
    }

    /// Static version of handle_command for spawned tasks
    async fn handle_command_static(
        command: RoutingCommand,
        routing_table: &RwLock<HashMap<MessageType, Vec<RouteEntry>>>,
        subscribed_topics: &RwLock<HashSet<String>>,
        dht_entries: &RwLock<HashMap<Vec<u8>, PeerId>>,
        message_sender: &mpsc::Sender<(PeerId, NetworkMessage)>,
        stats: &RwLock<RoutingStats>,
        config: &RoutingConfig,
    ) -> Result<()> {
        match command {
            RoutingCommand::AddRoute {
                message_type,
                peer_id,
                address,
            } => {
                Self::add_route_static(message_type, peer_id, address, routing_table, stats)
                    .await?;
            }
            RoutingCommand::RemoveRoute {
                message_type,
                peer_id,
            } => {
                Self::remove_route_static(message_type, peer_id, routing_table, stats).await?;
            }
            RoutingCommand::Subscribe(topic) => {
                Self::subscribe_topic_static(topic, subscribed_topics).await?;
            }
            RoutingCommand::Unsubscribe(topic) => {
                Self::unsubscribe_topic_static(topic, subscribed_topics).await?;
            }
            RoutingCommand::UpdateReliability { peer_id, success } => {
                Self::update_peer_reliability_static(peer_id, success, routing_table, config)
                    .await?;
            }
            RoutingCommand::GetStats(sender) => {
                let stats =
                    Self::get_routing_stats_static(routing_table, subscribed_topics, stats).await;
                let _ = sender.send(stats);
            }
        }
        Ok(())
    }

    /// Route a message using the specified strategy
    pub async fn route_message(
        &self,
        message: NetworkMessage,
        strategy: RoutingStrategy,
    ) -> Result<()> {
        let start_time = Instant::now();

        let result = match strategy {
            RoutingStrategy::Broadcast => self.broadcast_message(message).await,
            RoutingStrategy::Direct(peer_id) => self.direct_message(message, peer_id).await,
            RoutingStrategy::DHT(key) => self.dht_route_message(message, key).await,
            RoutingStrategy::Gossip(topic) => self.gossip_message(message, topic).await,
            RoutingStrategy::Random(count) => self.random_route_message(message, count).await,
        };

        // Update statistics
        let mut stats = self.stats.write().await;
        stats.total_messages_routed += 1;

        match result {
            Ok(_) => {
                stats.successful_routes += 1;
            }
            Err(_) => {
                stats.failed_routes += 1;
            }
        }

        let routing_time = start_time.elapsed().as_millis() as f64;
        stats.avg_routing_time_ms =
            (stats.avg_routing_time_ms * (stats.total_messages_routed - 1) as f64 + routing_time)
                / stats.total_messages_routed as f64;

        result
    }

    /// Broadcast message to all known peers
    async fn broadcast_message(&self, message: NetworkMessage) -> Result<()> {
        let table = self.routing_table.read().await;
        let mut sent_count = 0;

        for routes in table.values() {
            for route in routes {
                if let Err(e) = self
                    .message_sender
                    .send((route.peer_id, message.clone()))
                    .await
                {
                    warn!("Failed to send message to peer {}: {}", route.peer_id, e);
                } else {
                    sent_count += 1;
                }
            }
        }

        debug!("Broadcast message to {} peers", sent_count);
        Ok(())
    }

    /// Send message directly to specific peer
    async fn direct_message(&self, message: NetworkMessage, peer_id: PeerId) -> Result<()> {
        self.message_sender
            .send((peer_id, message))
            .await
            .map_err(|e| P2PError::Internal(format!("Failed to send direct message: {}", e)).into())
    }

    /// Route message using DHT
    async fn dht_route_message(&self, message: NetworkMessage, key: Vec<u8>) -> Result<()> {
        let dht_entries = self.dht_entries.read().await;

        if let Some(peer_id) = dht_entries.get(&key) {
            self.direct_message(message, *peer_id).await
        } else {
            // If not in local DHT, broadcast to all DHT nodes
            self.broadcast_message(message).await
        }
    }

    /// Route message via GossipSub
    async fn gossip_message(&self, message: NetworkMessage, topic: String) -> Result<()> {
        // For GossipSub, we delegate to the network layer
        // This is a placeholder - actual implementation would use GossipSub directly
        debug!("Routing message via GossipSub topic: {}", topic);
        self.broadcast_message(message).await
    }

    /// Route message to random subset of peers
    async fn random_route_message(&self, message: NetworkMessage, count: usize) -> Result<()> {
        let table = self.routing_table.read().await;
        let mut all_peers = Vec::new();

        for routes in table.values() {
            for route in routes {
                all_peers.push(route.peer_id);
            }
        }

        if all_peers.is_empty() {
            return Err(P2PError::Internal("No peers available for routing".to_string()).into());
        }

        // Randomly select peers
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        all_peers.shuffle(&mut rng);

        let selected_count = count.min(all_peers.len());
        let mut sent_count = 0;

        for &peer_id in all_peers.iter().take(selected_count) {
            if let Err(e) = self.message_sender.send((peer_id, message.clone())).await {
                warn!("Failed to send message to random peer {}: {}", peer_id, e);
            } else {
                sent_count += 1;
            }
        }

        debug!("Sent message to {} random peers", sent_count);
        Ok(())
    }

    /// Add a new route to the routing table
    async fn add_route(
        &self,
        message_type: MessageType,
        peer_id: PeerId,
        address: Multiaddr,
    ) -> Result<()> {
        let mut table = self.routing_table.write().await;
        let routes = table.entry(message_type.clone()).or_insert_with(Vec::new);

        // Check if route already exists
        if let Some(existing) = routes.iter_mut().find(|r| r.peer_id == peer_id) {
            existing.address = address;
            existing.last_seen = Instant::now();
        } else {
            // Add new route if under limit
            if routes.len() < self.config.max_peers_per_type {
                routes.push(RouteEntry {
                    peer_id,
                    address,
                    last_seen: Instant::now(),
                    reliability_score: 1.0, // Start with perfect score
                    message_count: 0,
                });
                debug!("Added route for {:?} to peer {}", message_type, peer_id);
            } else {
                warn!("Route table full for message type {:?}", message_type);
            }
        }

        Ok(())
    }

    /// Remove a route from the routing table
    async fn remove_route(&self, message_type: MessageType, peer_id: PeerId) -> Result<()> {
        let mut table = self.routing_table.write().await;

        if let Some(routes) = table.get_mut(&message_type) {
            routes.retain(|route| route.peer_id != peer_id);
            debug!("Removed route for {:?} to peer {}", message_type, peer_id);

            // Remove empty entries
            if routes.is_empty() {
                table.remove(&message_type);
            }
        }

        Ok(())
    }

    /// Subscribe to a GossipSub topic
    async fn subscribe_topic(&self, topic: String) -> Result<()> {
        let mut topics = self.subscribed_topics.write().await;
        topics.insert(topic.clone());
        debug!("Subscribed to topic: {}", topic);
        Ok(())
    }

    /// Unsubscribe from a GossipSub topic
    async fn unsubscribe_topic(&self, topic: String) -> Result<()> {
        let mut topics = self.subscribed_topics.write().await;
        topics.remove(&topic);
        debug!("Unsubscribed from topic: {}", topic);
        Ok(())
    }

    /// Update peer reliability score
    async fn update_peer_reliability(&self, peer_id: PeerId, success: bool) -> Result<()> {
        let mut table = self.routing_table.write().await;

        for routes in table.values_mut() {
            if let Some(route) = routes.iter_mut().find(|r| r.peer_id == peer_id) {
                route.message_count += 1;

                // Update reliability using exponential moving average
                let alpha = 0.1; // Learning rate
                let new_score = if success { 1.0 } else { 0.0 };
                route.reliability_score =
                    alpha * new_score + (1.0 - alpha) * route.reliability_score;

                debug!(
                    "Updated reliability for peer {} to {:.3}",
                    peer_id, route.reliability_score
                );
                break;
            }
        }

        Ok(())
    }

    /// Get routing statistics
    pub async fn get_routing_stats(&self) -> RoutingStats {
        let mut stats = self.stats.read().await.clone();
        let table = self.routing_table.read().await;

        stats.active_routes = table.values().map(|routes| routes.len()).sum();
        stats.peer_count = table
            .values()
            .flat_map(|routes| routes.iter().map(|r| r.peer_id))
            .collect::<HashSet<_>>()
            .len();

        stats
    }

    /// Get best routes for a message type
    pub async fn get_best_routes(&self, message_type: &MessageType, count: usize) -> Vec<PeerId> {
        let table = self.routing_table.read().await;

        if let Some(routes) = table.get(message_type) {
            let mut sorted_routes = routes.clone();
            sorted_routes.sort_by(|a, b| {
                b.reliability_score
                    .partial_cmp(&a.reliability_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            sorted_routes
                .into_iter()
                .take(count)
                .map(|route| route.peer_id)
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Check if we have routes for a message type
    pub async fn has_routes(&self, message_type: &MessageType) -> bool {
        let table = self.routing_table.read().await;
        table
            .get(message_type)
            .map_or(false, |routes| !routes.is_empty())
    }

    // Static versions of methods for use in spawned tasks

    /// Static version of add_route
    async fn add_route_static(
        message_type: MessageType,
        peer_id: PeerId,
        address: Multiaddr,
        routing_table: &RwLock<HashMap<MessageType, Vec<RouteEntry>>>,
        stats: &RwLock<RoutingStats>,
    ) -> Result<()> {
        let mut table = routing_table.write().await;
        let routes = table.entry(message_type.clone()).or_insert_with(Vec::new);

        // Check if route already exists
        if let Some(existing) = routes.iter_mut().find(|r| r.peer_id == peer_id) {
            existing.address = address;
            existing.last_seen = Instant::now();
        } else {
            // Add new route if under limit (default max 50)
            if routes.len() < 50 {
                routes.push(RouteEntry {
                    peer_id,
                    address,
                    last_seen: Instant::now(),
                    reliability_score: 1.0,
                    message_count: 0,
                });
                debug!("Added route for {:?} to peer {}", message_type, peer_id);
            } else {
                warn!("Route table full for message type {:?}", message_type);
            }
        }

        Ok(())
    }

    /// Static version of remove_route
    async fn remove_route_static(
        message_type: MessageType,
        peer_id: PeerId,
        routing_table: &RwLock<HashMap<MessageType, Vec<RouteEntry>>>,
        stats: &RwLock<RoutingStats>,
    ) -> Result<()> {
        let mut table = routing_table.write().await;

        if let Some(routes) = table.get_mut(&message_type) {
            routes.retain(|route| route.peer_id != peer_id);
            debug!("Removed route for {:?} to peer {}", message_type, peer_id);

            // Remove empty entries
            if routes.is_empty() {
                table.remove(&message_type);
            }
        }

        Ok(())
    }

    /// Static version of subscribe_topic
    async fn subscribe_topic_static(
        topic: String,
        subscribed_topics: &RwLock<HashSet<String>>,
    ) -> Result<()> {
        let mut topics = subscribed_topics.write().await;
        topics.insert(topic.clone());
        debug!("Subscribed to topic: {}", topic);
        Ok(())
    }

    /// Static version of unsubscribe_topic
    async fn unsubscribe_topic_static(
        topic: String,
        subscribed_topics: &RwLock<HashSet<String>>,
    ) -> Result<()> {
        let mut topics = subscribed_topics.write().await;
        topics.remove(&topic);
        debug!("Unsubscribed from topic: {}", topic);
        Ok(())
    }

    /// Static version of update_peer_reliability
    async fn update_peer_reliability_static(
        peer_id: PeerId,
        success: bool,
        routing_table: &RwLock<HashMap<MessageType, Vec<RouteEntry>>>,
        config: &RoutingConfig,
    ) -> Result<()> {
        let mut table = routing_table.write().await;

        for routes in table.values_mut() {
            if let Some(route) = routes.iter_mut().find(|r| r.peer_id == peer_id) {
                route.message_count += 1;

                // Update reliability using exponential moving average
                let alpha = 0.1; // Learning rate
                let new_score = if success { 1.0 } else { 0.0 };
                route.reliability_score =
                    alpha * new_score + (1.0 - alpha) * route.reliability_score;

                debug!(
                    "Updated reliability for peer {} to {:.3}",
                    peer_id, route.reliability_score
                );
                break;
            }
        }

        Ok(())
    }

    /// Static version of get_routing_stats
    async fn get_routing_stats_static(
        routing_table: &RwLock<HashMap<MessageType, Vec<RouteEntry>>>,
        subscribed_topics: &RwLock<HashSet<String>>,
        stats: &RwLock<RoutingStats>,
    ) -> RoutingStats {
        let mut current_stats = stats.read().await.clone();
        let table = routing_table.read().await;

        current_stats.active_routes = table.values().map(|routes| routes.len()).sum();
        current_stats.peer_count = table
            .values()
            .flat_map(|routes| routes.iter().map(|r| r.peer_id))
            .collect::<HashSet<_>>()
            .len();

        current_stats
    }
}
