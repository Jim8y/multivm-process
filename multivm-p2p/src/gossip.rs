//! Gossip Protocol Implementation for P2P Message Propagation
//!
//! Provides efficient message broadcasting and propagation across the P2P network
//! using epidemic-style gossip protocols with optimizations for MultiVM operations.

use crate::{
    error::{P2PError, P2PResult},
    messages::{NetworkMessage, MessageType, Priority},
    rate_limiter::RateLimiter,
};
use libp2p::{gossipsub, PeerId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

/// Gossip protocol configuration
#[derive(Debug, Clone)]
pub struct GossipConfig {
    /// Maximum number of peers to gossip to per round
    pub fanout: usize,
    /// Gossip interval
    pub gossip_interval: Duration,
    /// Message TTL (time to live)
    pub message_ttl: Duration,
    /// Maximum message cache size
    pub max_cache_size: usize,
    /// Duplicate detection window
    pub duplicate_window: Duration,
    /// Enable message compression
    pub enable_compression: bool,
    /// Priority-based propagation
    pub enable_priority_propagation: bool,
    /// Heartbeat interval for peer health
    pub heartbeat_interval: Duration,
    /// Maximum retransmission attempts
    pub max_retransmissions: u32,
}

impl Default for GossipConfig {
    fn default() -> Self {
        Self {
            fanout: 6,
            gossip_interval: Duration::from_millis(100),
            message_ttl: Duration::from_secs(300), // 5 minutes
            max_cache_size: 10000,
            duplicate_window: Duration::from_secs(60),
            enable_compression: true,
            enable_priority_propagation: true,
            heartbeat_interval: Duration::from_secs(1),
            max_retransmissions: 3,
        }
    }
}

/// Gossip message wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GossipMessage {
    /// Unique message ID
    pub id: String,
    /// Original message
    pub payload: NetworkMessage,
    /// Message priority
    pub priority: Priority,
    /// Time to live (hops remaining)
    pub ttl: u32,
    /// Propagation path (for loop detection)
    pub path: Vec<String>,
    /// Timestamp when message was created
    pub timestamp: SystemTime,
    /// Compression flag
    pub compressed: bool,
    /// Retransmission count
    pub retransmissions: u32,
}

/// Gossip protocol statistics
#[derive(Debug, Clone, Default)]
pub struct GossipStats {
    /// Total messages sent
    pub messages_sent: u64,
    /// Total messages received
    pub messages_received: u64,
    /// Duplicate messages filtered
    pub duplicates_filtered: u64,
    /// Messages expired due to TTL
    pub messages_expired: u64,
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Average propagation delay
    pub avg_propagation_delay_ms: f64,
    /// Active gossip peers
    pub active_peers: usize,
    /// Cache hit rate
    pub cache_hit_rate: f64,
}

/// Peer gossip state
#[derive(Debug, Clone)]
struct PeerGossipState {
    peer_id: PeerId,
    last_heartbeat: Instant,
    messages_sent: u64,
    messages_received: u64,
    reliability_score: f64,
    is_active: bool,
}

/// Message cache entry
#[derive(Debug, Clone)]
struct CacheEntry {
    message: GossipMessage,
    first_seen: Instant,
    propagated_to: HashSet<PeerId>,
    propagation_count: u32,
}

/// Command processing task context
struct CommandProcessingContext {
    running: Arc<RwLock<bool>>,
    message_cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    peer_states: Arc<RwLock<HashMap<PeerId, PeerGossipState>>>,
    pending_messages: Arc<RwLock<VecDeque<GossipMessage>>>,
    stats: Arc<RwLock<GossipStats>>,
    event_sender: Option<mpsc::Sender<GossipEvent>>,
    config: GossipConfig,
    local_peer_id: PeerId,
}

/// Message processing context
struct MessageProcessingContext<'a> {
    message_cache: &'a Arc<RwLock<HashMap<String, CacheEntry>>>,
    peer_states: &'a Arc<RwLock<HashMap<PeerId, PeerGossipState>>>,
    pending_messages: &'a Arc<RwLock<VecDeque<GossipMessage>>>,
    stats: &'a Arc<RwLock<GossipStats>>,
    event_sender: &'a Option<mpsc::Sender<GossipEvent>>,
    config: &'a GossipConfig,
    local_peer_id: PeerId,
}

/// Gossip protocol manager
pub struct GossipProtocol {
    /// Configuration
    config: GossipConfig,
    /// Local peer ID
    local_peer_id: PeerId,
    /// Message cache for duplicate detection and retransmission
    message_cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    /// Peer states
    peer_states: Arc<RwLock<HashMap<PeerId, PeerGossipState>>>,
    /// Pending messages queue
    pending_messages: Arc<RwLock<VecDeque<GossipMessage>>>,
    /// Rate limiter
    rate_limiter: RateLimiter,
    /// Statistics
    stats: Arc<RwLock<GossipStats>>,
    /// Command channel
    command_sender: Option<mpsc::Sender<GossipCommand>>,
    /// Event channel
    event_sender: Option<mpsc::Sender<GossipEvent>>,
    /// Running state
    running: Arc<RwLock<bool>>,
}

/// Commands for controlling gossip protocol
#[derive(Debug)]
pub enum GossipCommand {
    /// Broadcast a message
    Broadcast {
        message: NetworkMessage,
        priority: Priority,
        response: tokio::sync::oneshot::Sender<P2PResult<()>>,
    },
    /// Add a peer
    AddPeer {
        peer_id: PeerId,
        response: tokio::sync::oneshot::Sender<P2PResult<()>>,
    },
    /// Remove a peer
    RemovePeer {
        peer_id: PeerId,
        response: tokio::sync::oneshot::Sender<P2PResult<()>>,
    },
    /// Process incoming gossip message
    ProcessMessage {
        message: GossipMessage,
        from_peer: PeerId,
        response: tokio::sync::oneshot::Sender<P2PResult<()>>,
    },
    /// Get statistics
    GetStats {
        response: tokio::sync::oneshot::Sender<GossipStats>,
    },
}

/// Events emitted by gossip protocol
#[derive(Debug, Clone)]
pub enum GossipEvent {
    /// Message received and validated
    MessageReceived {
        message: NetworkMessage,
        from_peer: PeerId,
    },
    /// Message propagated to peers
    MessagePropagated {
        message_id: String,
        peer_count: usize,
    },
    /// Duplicate message detected
    DuplicateDetected {
        message_id: String,
        from_peer: PeerId,
    },
    /// Peer became active/inactive
    PeerStatusChanged {
        peer_id: PeerId,
        is_active: bool,
    },
}

impl GossipProtocol {
    /// Create a new gossip protocol instance
    pub fn new(config: GossipConfig, local_peer_id: PeerId) -> (Self, mpsc::Receiver<GossipEvent>) {
        let (event_sender, event_receiver) = mpsc::channel(1000);
        
        let rate_limiter = RateLimiter::new(100, Duration::from_secs(1)); // 100 messages per second

        let protocol = Self {
            config,
            local_peer_id,
            message_cache: Arc::new(RwLock::new(HashMap::new())),
            peer_states: Arc::new(RwLock::new(HashMap::new())),
            pending_messages: Arc::new(RwLock::new(VecDeque::new())),
            rate_limiter,
            stats: Arc::new(RwLock::new(GossipStats::default())),
            command_sender: None,
            event_sender: Some(event_sender),
            running: Arc::new(RwLock::new(false)),
        };

        (protocol, event_receiver)
    }

    /// Start the gossip protocol
    pub async fn start(&mut self) -> P2PResult<()> {
        info!("Starting gossip protocol");

        *self.running.write().await = true;

        let (command_sender, command_receiver) = mpsc::channel(1000);
        self.command_sender = Some(command_sender);

        // Start command processing task
        let running = Arc::clone(&self.running);
        let message_cache = Arc::clone(&self.message_cache);
        let peer_states = Arc::clone(&self.peer_states);
        let pending_messages = Arc::clone(&self.pending_messages);
        let stats = Arc::clone(&self.stats);
        let event_sender = self.event_sender.clone();
        let config = self.config.clone();
        let local_peer_id = self.local_peer_id;

        tokio::spawn(async move {
            let ctx = CommandProcessingContext {
                running: running.clone(),
                message_cache: message_cache.clone(),
                peer_states: peer_states.clone(),
                pending_messages: pending_messages.clone(),
                stats: stats.clone(),
                event_sender: event_sender.clone(),
                config: config.clone(),
                local_peer_id,
            };
            Self::command_processing_task(command_receiver, ctx).await;
        });

        // Start gossip propagation task
        let running = Arc::clone(&self.running);
        let message_cache = Arc::clone(&self.message_cache);
        let peer_states = Arc::clone(&self.peer_states);
        let pending_messages = Arc::clone(&self.pending_messages);
        let stats = Arc::clone(&self.stats);
        let config = self.config.clone();

        tokio::spawn(async move {
            Self::gossip_propagation_task(
                running,
                message_cache,
                peer_states,
                pending_messages,
                stats,
                config,
            ).await;
        });

        // Start maintenance task
        self.start_maintenance_task().await;

        info!("Gossip protocol started successfully");
        Ok(())
    }

    /// Stop the gossip protocol
    pub async fn stop(&mut self) -> P2PResult<()> {
        info!("Stopping gossip protocol");
        *self.running.write().await = false;
        Ok(())
    }

    /// Broadcast a message through gossip
    pub async fn broadcast(&self, message: NetworkMessage, priority: Priority) -> P2PResult<()> {
        if let Some(sender) = &self.command_sender {
            let (response_sender, response_receiver) = tokio::sync::oneshot::channel();
            
            sender.send(GossipCommand::Broadcast {
                message,
                priority,
                response: response_sender,
            }).await.map_err(|_| P2PError::Internal("Command channel closed".to_string()))?;

            response_receiver.await
                .map_err(|_| P2PError::Internal("Response channel closed".to_string()))?
        } else {
            Err(P2PError::Internal("Gossip protocol not started".to_string()))
        }
    }

    /// Add a peer to gossip network
    pub async fn add_peer(&self, peer_id: PeerId) -> P2PResult<()> {
        if let Some(sender) = &self.command_sender {
            let (response_sender, response_receiver) = tokio::sync::oneshot::channel();
            
            sender.send(GossipCommand::AddPeer {
                peer_id,
                response: response_sender,
            }).await.map_err(|_| P2PError::Internal("Command channel closed".to_string()))?;

            response_receiver.await
                .map_err(|_| P2PError::Internal("Response channel closed".to_string()))?
        } else {
            Err(P2PError::Internal("Gossip protocol not started".to_string()))
        }
    }

    /// Remove a peer from gossip network
    pub async fn remove_peer(&self, peer_id: PeerId) -> P2PResult<()> {
        if let Some(sender) = &self.command_sender {
            let (response_sender, response_receiver) = tokio::sync::oneshot::channel();
            
            sender.send(GossipCommand::RemovePeer {
                peer_id,
                response: response_sender,
            }).await.map_err(|_| P2PError::Internal("Command channel closed".to_string()))?;

            response_receiver.await
                .map_err(|_| P2PError::Internal("Response channel closed".to_string()))?
        } else {
            Err(P2PError::Internal("Gossip protocol not started".to_string()))
        }
    }

    /// Process incoming gossip message
    pub async fn process_message(&self, message: GossipMessage, from_peer: PeerId) -> P2PResult<()> {
        if let Some(sender) = &self.command_sender {
            let (response_sender, response_receiver) = tokio::sync::oneshot::channel();
            
            sender.send(GossipCommand::ProcessMessage {
                message,
                from_peer,
                response: response_sender,
            }).await.map_err(|_| P2PError::Internal("Command channel closed".to_string()))?;

            response_receiver.await
                .map_err(|_| P2PError::Internal("Response channel closed".to_string()))?
        } else {
            Err(P2PError::Internal("Gossip protocol not started".to_string()))
        }
    }

    /// Get gossip statistics
    pub async fn get_stats(&self) -> GossipStats {
        if let Some(sender) = &self.command_sender {
            let (response_sender, response_receiver) = tokio::sync::oneshot::channel();
            
            if sender.send(GossipCommand::GetStats {
                response: response_sender,
            }).await.is_ok() {
                if let Ok(stats) = response_receiver.await {
                    return stats;
                }
            }
        }

        // Fallback to current stats
        self.stats.read().await.clone()
    }

    /// Command processing task
    async fn command_processing_task(
        mut command_receiver: mpsc::Receiver<GossipCommand>,
        ctx: CommandProcessingContext,
    ) {
        while let Some(command) = command_receiver.recv().await {
            if !*ctx.running.read().await {
                break;
            }

            match command {
                GossipCommand::Broadcast { message, priority, response } => {
                    let result = Self::handle_broadcast(
                        message,
                        priority,
                        &ctx.message_cache,
                        &ctx.pending_messages,
                        &ctx.stats,
                        &ctx.config,
                        ctx.local_peer_id,
                    ).await;
                    let _ = response.send(result);
                }
                GossipCommand::AddPeer { peer_id, response } => {
                    let result = Self::handle_add_peer(peer_id, &ctx.peer_states).await;
                    let _ = response.send(result);
                }
                GossipCommand::RemovePeer { peer_id, response } => {
                    let result = Self::handle_remove_peer(peer_id, &ctx.peer_states).await;
                    let _ = response.send(result);
                }
                GossipCommand::ProcessMessage { message, from_peer, response } => {
                    let msg_ctx = MessageProcessingContext {
                        message_cache: &ctx.message_cache,
                        peer_states: &ctx.peer_states,
                        pending_messages: &ctx.pending_messages,
                        stats: &ctx.stats,
                        event_sender: &ctx.event_sender,
                        config: &ctx.config,
                        local_peer_id: ctx.local_peer_id,
                    };
                    let result = Self::handle_process_message(message, from_peer, msg_ctx).await;
                    let _ = response.send(result);
                }
                GossipCommand::GetStats { response } => {
                    let current_stats = ctx.stats.read().await.clone();
                    let _ = response.send(current_stats);
                }
            }
        }
    }

    /// Handle broadcast command
    async fn handle_broadcast(
        message: NetworkMessage,
        priority: Priority,
        message_cache: &Arc<RwLock<HashMap<String, CacheEntry>>>,
        pending_messages: &Arc<RwLock<VecDeque<GossipMessage>>>,
        stats: &Arc<RwLock<GossipStats>>,
        config: &GossipConfig,
        local_peer_id: PeerId,
    ) -> P2PResult<()> {
        let gossip_message = GossipMessage {
            id: message.id.clone(),
            payload: message,
            priority,
            ttl: 10, // Default TTL
            path: vec![local_peer_id.to_string()],
            timestamp: SystemTime::now(),
            compressed: config.enable_compression,
            retransmissions: 0,
        };

        // Add to cache
        {
            let mut cache = message_cache.write().await;
            cache.insert(gossip_message.id.clone(), CacheEntry {
                message: gossip_message.clone(),
                first_seen: Instant::now(),
                propagated_to: HashSet::new(),
                propagation_count: 0,
            });

            // Cleanup old entries if cache is full
            if cache.len() > config.max_cache_size {
                let oldest_key = cache.iter()
                    .min_by_key(|(_, entry)| entry.first_seen)
                    .map(|(key, _)| key.clone());
                if let Some(key) = oldest_key {
                    cache.remove(&key);
                }
            }
        }

        // Add to pending messages for propagation
        {
            let mut pending = pending_messages.write().await;
            pending.push_back(gossip_message);
        }

        // Update statistics
        {
            let mut stats = stats.write().await;
            stats.messages_sent += 1;
        }

        debug!("Message queued for gossip broadcast");
        Ok(())
    }

    /// Handle add peer command
    async fn handle_add_peer(
        peer_id: PeerId,
        peer_states: &Arc<RwLock<HashMap<PeerId, PeerGossipState>>>,
    ) -> P2PResult<()> {
        let mut states = peer_states.write().await;
        states.insert(peer_id, PeerGossipState {
            peer_id,
            last_heartbeat: Instant::now(),
            messages_sent: 0,
            messages_received: 0,
            reliability_score: 1.0,
            is_active: true,
        });

        debug!("Added peer to gossip network: {}", peer_id);
        Ok(())
    }

    /// Handle remove peer command
    async fn handle_remove_peer(
        peer_id: PeerId,
        peer_states: &Arc<RwLock<HashMap<PeerId, PeerGossipState>>>,
    ) -> P2PResult<()> {
        let mut states = peer_states.write().await;
        states.remove(&peer_id);

        debug!("Removed peer from gossip network: {}", peer_id);
        Ok(())
    }

    /// Handle process message command
    async fn handle_process_message(
        message: GossipMessage,
        from_peer: PeerId,
        ctx: MessageProcessingContext<'_>,
    ) -> P2PResult<()> {
        // Check if message is expired
        if let Ok(age) = message.timestamp.elapsed() {
            if age > ctx.config.message_ttl {
                ctx.stats.write().await.messages_expired += 1;
                return Ok(());
            }
        }

        // Check for duplicates
        {
            let cache = ctx.message_cache.read().await;
            if cache.contains_key(&message.id) {
                ctx.stats.write().await.duplicates_filtered += 1;
                if let Some(sender) = ctx.event_sender {
                    let _ = sender.send(GossipEvent::DuplicateDetected {
                        message_id: message.id,
                        from_peer,
                    }).await;
                }
                return Ok(());
            }
        }

        // Check if we're in the propagation path (loop detection)
        if message.path.contains(&ctx.local_peer_id.to_string()) {
            debug!("Loop detected in gossip message, dropping");
            return Ok(());
        }

        // Update peer state
        {
            let mut states = ctx.peer_states.write().await;
            if let Some(state) = states.get_mut(&from_peer) {
                state.messages_received += 1;
                state.last_heartbeat = Instant::now();
            }
        }

        // Add to cache
        {
            let mut cache = ctx.message_cache.write().await;
            cache.insert(message.id.clone(), CacheEntry {
                message: message.clone(),
                first_seen: Instant::now(),
                propagated_to: HashSet::new(),
                propagation_count: 0,
            });
        }

        // Forward message if TTL allows
        if message.ttl > 0 {
            let mut forwarded_message = message.clone();
            forwarded_message.ttl -= 1;
            forwarded_message.path.push(ctx.local_peer_id.to_string());

            let mut pending = ctx.pending_messages.write().await;
            pending.push_back(forwarded_message);
        }

        // Update statistics
        {
            let mut stats = ctx.stats.write().await;
            stats.messages_received += 1;
        }

        // Send event
        if let Some(sender) = ctx.event_sender {
            let _ = sender.send(GossipEvent::MessageReceived {
                message: message.payload,
                from_peer,
            }).await;
        }

        Ok(())
    }

    /// Gossip propagation task
    async fn gossip_propagation_task(
        running: Arc<RwLock<bool>>,
        message_cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
        peer_states: Arc<RwLock<HashMap<PeerId, PeerGossipState>>>,
        pending_messages: Arc<RwLock<VecDeque<GossipMessage>>>,
        stats: Arc<RwLock<GossipStats>>,
        config: GossipConfig,
    ) {
        let mut interval = tokio::time::interval(config.gossip_interval);

        while *running.read().await {
            interval.tick().await;

            // Process pending messages
            let message_to_propagate = {
                let mut pending = pending_messages.write().await;
                pending.pop_front()
            };

            if let Some(message) = message_to_propagate {
                Self::propagate_message(
                    message,
                    &message_cache,
                    &peer_states,
                    &stats,
                    &config,
                ).await;
            }
        }
    }

    /// Propagate a message to selected peers
    async fn propagate_message(
        message: GossipMessage,
        message_cache: &Arc<RwLock<HashMap<String, CacheEntry>>>,
        peer_states: &Arc<RwLock<HashMap<PeerId, PeerGossipState>>>,
        stats: &Arc<RwLock<GossipStats>>,
        config: &GossipConfig,
    ) {
        // Select peers for propagation
        let selected_peers = {
            let states = peer_states.read().await;
            let active_peers: Vec<PeerId> = states.values()
                .filter(|state| state.is_active)
                .map(|state| state.peer_id)
                .collect();

            // Select fanout number of peers
            let mut selected = Vec::new();
            let fanout = config.fanout.min(active_peers.len());
            
            if config.enable_priority_propagation {
                // Priority-based selection (select most reliable peers for high priority)
                let mut sorted_peers: Vec<_> = states.values().collect();
                sorted_peers.sort_by(|a, b| b.reliability_score.partial_cmp(&a.reliability_score).unwrap());
                
                for peer in sorted_peers.iter().take(fanout) {
                    selected.push(peer.peer_id);
                }
            } else {
                // Random selection
                use rand::seq::SliceRandom;
                let mut rng = rand::thread_rng();
                let mut peers = active_peers;
                peers.shuffle(&mut rng);
                selected.extend(peers.into_iter().take(fanout));
            }

            selected
        };

        // Update cache with propagation info
        {
            let mut cache = message_cache.write().await;
            if let Some(entry) = cache.get_mut(&message.id) {
                for peer_id in &selected_peers {
                    entry.propagated_to.insert(*peer_id);
                }
                entry.propagation_count += selected_peers.len() as u32;
            }
        }

        // Update statistics
        {
            let mut stats = stats.write().await;
            stats.messages_sent += selected_peers.len() as u64;
        }

        debug!("Propagated message {} to {} peers", message.id, selected_peers.len());
    }

    /// Start maintenance task
    async fn start_maintenance_task(&self) {
        let running = Arc::clone(&self.running);
        let message_cache = Arc::clone(&self.message_cache);
        let peer_states = Arc::clone(&self.peer_states);
        let stats = Arc::clone(&self.stats);
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            while *running.read().await {
                interval.tick().await;

                // Clean expired messages from cache
                {
                    let mut cache = message_cache.write().await;
                    let now = Instant::now();
                    cache.retain(|_, entry| {
                        now.duration_since(entry.first_seen) < config.duplicate_window
                    });
                }

                // Update peer activity status
                {
                    let mut states = peer_states.write().await;
                    let now = Instant::now();
                    for state in states.values_mut() {
                        let inactive_duration = now.duration_since(state.last_heartbeat);
                        state.is_active = inactive_duration < config.heartbeat_interval * 3;
                    }
                }

                // Update statistics
                {
                    let mut stats = stats.write().await;
                    let states = peer_states.read().await;
                    stats.active_peers = states.values().filter(|s| s.is_active).count();
                }

                debug!("Gossip maintenance completed");
            }
        });
    }
}

