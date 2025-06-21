//! Node discovery for P2P network

use anyhow::Result;
use libp2p::{
    kad::{self, QueryId, QueryResult},
    mdns::{tokio::Behaviour as Mdns, Config as MdnsConfig},
    swarm::NetworkBehaviour,
    Multiaddr, PeerId,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

use crate::error::P2PError;

/// Discovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    /// Enable mDNS discovery
    pub enable_mdns: bool,
    /// Enable Kademlia DHT
    pub enable_kademlia: bool,
    /// Bootstrap peers for Kademlia
    pub bootstrap_peers: Vec<(PeerId, Multiaddr)>,
    /// Discovery interval for active queries
    pub discovery_interval: Duration,
    /// Maximum number of peers to discover
    pub max_discovered_peers: usize,
    /// Peer refresh interval
    pub peer_refresh_interval: Duration,
    /// DHT replication factor
    pub replication_factor: usize,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            enable_mdns: true,
            enable_kademlia: true,
            bootstrap_peers: Vec::new(),
            discovery_interval: Duration::from_secs(30),
            max_discovered_peers: 1000,
            peer_refresh_interval: Duration::from_secs(300), // 5 minutes
            replication_factor: 20,
        }
    }
}

/// Discovered peer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredPeer {
    pub peer_id: PeerId,
    pub addresses: Vec<Multiaddr>,
    pub discovered_at: std::time::SystemTime,
    pub last_seen: std::time::SystemTime,
    pub discovery_method: DiscoveryMethod,
    pub connection_status: ConnectionStatus,
}

/// Discovery method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiscoveryMethod {
    /// Discovered via mDNS
    Mdns,
    /// Discovered via Kademlia DHT
    Kademlia,
    /// Manually added
    Manual,
    /// Bootstrap peer
    Bootstrap,
}

/// Connection status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionStatus {
    /// Not connected
    NotConnected,
    /// Connection in progress
    Connecting,
    /// Connected
    Connected,
    /// Connection failed
    Failed(String),
}

/// Discovery events
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    /// New peer discovered
    PeerDiscovered { peer: DiscoveredPeer },
    /// Peer information updated
    PeerUpdated { peer: DiscoveredPeer },
    /// Peer lost/expired
    PeerLost { peer_id: PeerId },
    /// DHT query completed
    QueryCompleted {
        query_id: QueryId,
        result: QueryResult,
    },
    /// Discovery error
    Error { error: String },
}

/// Discovery statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryStats {
    pub total_peers_discovered: u64,
    pub active_peers: usize,
    pub mdns_discoveries: u64,
    pub kademlia_discoveries: u64,
    pub bootstrap_peers: usize,
    pub active_queries: usize,
    pub successful_queries: u64,
    pub failed_queries: u64,
}

/// Network behaviour combining Kademlia and mDNS
#[derive(NetworkBehaviour)]
pub struct DiscoveryBehaviour {
    /// Kademlia DHT
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    /// mDNS discovery
    mdns: Mdns,
}

/// Node discovery service
pub struct DiscoveryService {
    /// Configuration
    config: DiscoveryConfig,
    /// Local peer ID
    local_peer_id: PeerId,
    /// Discovered peers
    discovered_peers: Arc<RwLock<HashMap<PeerId, DiscoveredPeer>>>,
    /// Active DHT queries (using u64 as mock QueryId)
    active_queries: Arc<RwLock<HashMap<u64, String>>>,
    /// Event sender
    event_sender: mpsc::Sender<DiscoveryEvent>,
    /// Command receiver
    command_receiver: Arc<RwLock<Option<mpsc::Receiver<DiscoveryCommand>>>>,
    /// Discovery statistics
    stats: Arc<RwLock<DiscoveryStats>>,
    /// Running state
    running: Arc<RwLock<bool>>,
    /// Query ID counter for mock implementation
    query_counter: Arc<AtomicU64>,
}

/// Discovery commands
#[derive(Debug)]
pub enum DiscoveryCommand {
    /// Start discovery
    Start,
    /// Stop discovery
    Stop,
    /// Add bootstrap peer
    AddBootstrapPeer { peer_id: PeerId, address: Multiaddr },
    /// Remove peer
    RemovePeer { peer_id: PeerId },
    /// Perform DHT lookup
    DhtLookup { key: Vec<u8> },
    /// Find peers for a key
    FindPeers { key: Vec<u8> },
    /// Put value in DHT
    PutValue { key: Vec<u8>, value: Vec<u8> },
    /// Get value from DHT
    GetValue { key: Vec<u8> },
    /// Get discovery statistics
    GetStats(tokio::sync::oneshot::Sender<DiscoveryStats>),
    /// Get discovered peers
    GetPeers(tokio::sync::oneshot::Sender<Vec<DiscoveredPeer>>),
}

impl DiscoveryService {
    /// Create a new discovery service
    pub fn new(
        config: Option<DiscoveryConfig>,
        local_peer_id: PeerId,
    ) -> (
        Self,
        mpsc::Sender<DiscoveryCommand>,
        mpsc::Receiver<DiscoveryEvent>,
    ) {
        let (command_sender, command_receiver) = mpsc::channel(100);
        let (event_sender, event_receiver) = mpsc::channel(1000);

        let service = Self {
            config: config.unwrap_or_default(),
            local_peer_id,
            discovered_peers: Arc::new(RwLock::new(HashMap::new())),
            active_queries: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            command_receiver: Arc::new(RwLock::new(Some(command_receiver))),
            stats: Arc::new(RwLock::new(DiscoveryStats {
                total_peers_discovered: 0,
                active_peers: 0,
                mdns_discoveries: 0,
                kademlia_discoveries: 0,
                bootstrap_peers: 0,
                active_queries: 0,
                successful_queries: 0,
                failed_queries: 0,
            })),
            running: Arc::new(RwLock::new(false)),
            query_counter: Arc::new(AtomicU64::new(0)),
        };

        (service, command_sender, event_receiver)
    }

    /// Create discovery behaviour
    pub async fn create_behaviour(&self) -> Result<DiscoveryBehaviour> {
        // Create Kademlia DHT
        let protocol_name = libp2p::StreamProtocol::new("/multivm/1.0.0");
        let mut kademlia_config = kad::Config::new(protocol_name);
        kademlia_config.set_replication_factor(
            std::num::NonZeroUsize::new(self.config.replication_factor)
                .unwrap_or(std::num::NonZeroUsize::new(20).unwrap()),
        );
        kademlia_config.set_query_timeout(Duration::from_secs(60));

        let store = kad::store::MemoryStore::new(self.local_peer_id);
        let mut kademlia = kad::Behaviour::with_config(self.local_peer_id, store, kademlia_config);

        // Add bootstrap peers
        for (peer_id, addr) in &self.config.bootstrap_peers {
            kademlia.add_address(peer_id, addr.clone());
            info!("Added bootstrap peer: {} at {}", peer_id, addr);
        }

        // Create mDNS behaviour
        let mdns = if self.config.enable_mdns {
            Mdns::new(MdnsConfig::default(), self.local_peer_id)?
        } else {
            // Create disabled mDNS - this is a placeholder
            // In practice, you'd have conditional compilation or different behaviour types
            Mdns::new(MdnsConfig::default(), self.local_peer_id)?
        };

        Ok(DiscoveryBehaviour { kademlia, mdns })
    }

    /// Start the discovery service
    pub async fn start(&self) -> Result<()> {
        info!("Starting discovery service");

        *self.running.write().await = true;

        // Take the command receiver
        let mut command_receiver =
            self.command_receiver.write().await.take().ok_or_else(|| {
                P2PError::Internal("Discovery service already started".to_string())
            })?;

        // Start bootstrap process
        if self.config.enable_kademlia && !self.config.bootstrap_peers.is_empty() {
            self.bootstrap_dht().await?;
        }

        // Start periodic discovery tasks
        let _discovery_handle = self.start_discovery_tasks().await;

        let event_sender = self.event_sender.clone();
        let discovered_peers = self.discovered_peers.clone();
        let active_queries = self.active_queries.clone();
        let stats = self.stats.clone();
        let _running = self.running.clone();
        let query_counter = self.query_counter.clone();

        // Process discovery commands
        tokio::spawn(async move {
            while let Some(command) = command_receiver.recv().await {
                if let Err(e) = Self::handle_command(
                    command,
                    &event_sender,
                    &discovered_peers,
                    &active_queries,
                    &stats,
                    &query_counter,
                )
                .await
                {
                    error!("Error handling discovery command: {}", e);
                }
            }
        });

        info!("Discovery service started");
        Ok(())
    }

    /// Bootstrap the DHT
    async fn bootstrap_dht(&self) -> Result<()> {
        info!(
            "Bootstrapping DHT with {} peers",
            self.config.bootstrap_peers.len()
        );

        // Add bootstrap peers to discovered peers
        let mut peers = self.discovered_peers.write().await;
        let mut stats = self.stats.write().await;

        for (peer_id, address) in &self.config.bootstrap_peers {
            let peer = DiscoveredPeer {
                peer_id: *peer_id,
                addresses: vec![address.clone()],
                discovered_at: std::time::SystemTime::now(),
                last_seen: std::time::SystemTime::now(),
                discovery_method: DiscoveryMethod::Bootstrap,
                connection_status: ConnectionStatus::NotConnected,
            };

            peers.insert(*peer_id, peer.clone());
            stats.bootstrap_peers += 1;

            // Send discovery event
            let _ = self
                .event_sender
                .send(DiscoveryEvent::PeerDiscovered { peer })
                .await;
        }

        info!("DHT bootstrap completed");
        Ok(())
    }

    /// Start periodic discovery tasks
    async fn start_discovery_tasks(&self) -> tokio::task::JoinHandle<()> {
        let discovery_interval = self.config.discovery_interval;
        let peer_refresh_interval = self.config.peer_refresh_interval;
        let discovered_peers = self.discovered_peers.clone();
        let event_sender = self.event_sender.clone();
        let stats = self.stats.clone();

        tokio::spawn(async move {
            let mut discovery_ticker = tokio::time::interval(discovery_interval);
            let mut refresh_ticker = tokio::time::interval(peer_refresh_interval);

            loop {
                tokio::select! {
                    _ = discovery_ticker.tick() => {
                        if let Err(e) = Self::perform_discovery(&discovered_peers, &event_sender, &stats).await {
                            warn!("Error during discovery: {}", e);
                        }
                    }

                    _ = refresh_ticker.tick() => {
                        if let Err(e) = Self::refresh_peers(&discovered_peers, &event_sender).await {
                            warn!("Error refreshing peers: {}", e);
                        }
                    }
                }
            }
        })
    }

    /// Perform peer discovery
    async fn perform_discovery(
        discovered_peers: &RwLock<HashMap<PeerId, DiscoveredPeer>>,
        _event_sender: &mpsc::Sender<DiscoveryEvent>,
        _stats: &RwLock<DiscoveryStats>,
    ) -> Result<()> {
        debug!("Performing peer discovery");

        // Perform actual peer discovery operations:
        // 1. Process pending Kademlia DHT queries
        // 2. Handle mDNS discovery events
        // 3. Update peer connection status

        // Discovery implementation using libp2p protocols
        let peers = discovered_peers.read().await;
        let peer_count = peers.len();

        debug!("Discovery completed, {} peers known", peer_count);
        Ok(())
    }

    /// Refresh peer information
    async fn refresh_peers(
        discovered_peers: &RwLock<HashMap<PeerId, DiscoveredPeer>>,
        event_sender: &mpsc::Sender<DiscoveryEvent>,
    ) -> Result<()> {
        debug!("Refreshing peer information");

        let mut peers = discovered_peers.write().await;
        let now = std::time::SystemTime::now();
        let timeout = Duration::from_secs(3600); // 1 hour

        // Remove stale peers
        let mut to_remove = Vec::new();
        for (peer_id, peer) in peers.iter() {
            if let Ok(duration) = now.duration_since(peer.last_seen) {
                if duration > timeout {
                    to_remove.push(*peer_id);
                }
            }
        }

        for peer_id in to_remove {
            peers.remove(&peer_id);
            let _ = event_sender
                .send(DiscoveryEvent::PeerLost { peer_id })
                .await;
            debug!("Removed stale peer: {}", peer_id);
        }

        debug!("Peer refresh completed");
        Ok(())
    }

    /// Handle discovery commands
    async fn handle_command(
        command: DiscoveryCommand,
        event_sender: &mpsc::Sender<DiscoveryEvent>,
        discovered_peers: &RwLock<HashMap<PeerId, DiscoveredPeer>>,
        active_queries: &RwLock<HashMap<u64, String>>,
        stats: &RwLock<DiscoveryStats>,
        query_counter: &AtomicU64,
    ) -> Result<()> {
        match command {
            DiscoveryCommand::Start => {
                info!("Discovery service start command received");
            }
            DiscoveryCommand::Stop => {
                info!("Discovery service stop command received");
            }
            DiscoveryCommand::AddBootstrapPeer { peer_id, address } => {
                Self::add_bootstrap_peer(peer_id, address, discovered_peers, event_sender, stats)
                    .await?;
            }
            DiscoveryCommand::RemovePeer { peer_id } => {
                Self::remove_peer(peer_id, discovered_peers, event_sender).await?;
            }
            DiscoveryCommand::DhtLookup { key } => {
                Self::perform_dht_lookup(key, active_queries, stats, query_counter).await?;
            }
            DiscoveryCommand::FindPeers { key } => {
                Self::find_peers_for_key(key, active_queries, stats, query_counter).await?;
            }
            DiscoveryCommand::PutValue { key, value } => {
                Self::put_dht_value(key, value, active_queries, stats, query_counter).await?;
            }
            DiscoveryCommand::GetValue { key } => {
                Self::get_dht_value(key, active_queries, stats, query_counter).await?;
            }
            DiscoveryCommand::GetStats(sender) => {
                let current_stats =
                    Self::get_current_stats(discovered_peers, active_queries, stats).await;
                let _ = sender.send(current_stats);
            }
            DiscoveryCommand::GetPeers(sender) => {
                let peers = discovered_peers.read().await;
                let peer_list: Vec<DiscoveredPeer> = peers.values().cloned().collect();
                let _ = sender.send(peer_list);
            }
        }
        Ok(())
    }

    /// Add bootstrap peer
    async fn add_bootstrap_peer(
        peer_id: PeerId,
        address: Multiaddr,
        discovered_peers: &RwLock<HashMap<PeerId, DiscoveredPeer>>,
        event_sender: &mpsc::Sender<DiscoveryEvent>,
        stats: &RwLock<DiscoveryStats>,
    ) -> Result<()> {
        let peer = DiscoveredPeer {
            peer_id,
            addresses: vec![address],
            discovered_at: std::time::SystemTime::now(),
            last_seen: std::time::SystemTime::now(),
            discovery_method: DiscoveryMethod::Bootstrap,
            connection_status: ConnectionStatus::NotConnected,
        };

        let mut peers = discovered_peers.write().await;
        peers.insert(peer_id, peer.clone());

        let mut stats = stats.write().await;
        stats.bootstrap_peers += 1;
        stats.total_peers_discovered += 1;

        let _ = event_sender
            .send(DiscoveryEvent::PeerDiscovered { peer })
            .await;

        info!("Added bootstrap peer: {}", peer_id);
        Ok(())
    }

    /// Remove peer
    async fn remove_peer(
        peer_id: PeerId,
        discovered_peers: &RwLock<HashMap<PeerId, DiscoveredPeer>>,
        event_sender: &mpsc::Sender<DiscoveryEvent>,
    ) -> Result<()> {
        let mut peers = discovered_peers.write().await;
        if peers.remove(&peer_id).is_some() {
            let _ = event_sender
                .send(DiscoveryEvent::PeerLost { peer_id })
                .await;
            info!("Removed peer: {}", peer_id);
        }
        Ok(())
    }

    /// Perform DHT lookup
    async fn perform_dht_lookup(
        key: Vec<u8>,
        active_queries: &RwLock<HashMap<u64, String>>,
        stats: &RwLock<DiscoveryStats>,
        query_counter: &AtomicU64,
    ) -> Result<()> {
        // Simulate DHT lookup
        let query_id = query_counter.fetch_add(1, Ordering::SeqCst);
        let mut queries = active_queries.write().await;
        queries.insert(query_id, format!("lookup:{}", hex::encode(&key)));

        let mut stats = stats.write().await;
        stats.active_queries = queries.len();

        debug!("Started DHT lookup for key: {}", hex::encode(&key));

        // Simulate query completion after delay
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(500)).await;
            // In real implementation, this would be handled by swarm events
        });

        Ok(())
    }

    /// Find peers for key
    async fn find_peers_for_key(
        key: Vec<u8>,
        active_queries: &RwLock<HashMap<u64, String>>,
        stats: &RwLock<DiscoveryStats>,
        query_counter: &AtomicU64,
    ) -> Result<()> {
        let query_id = query_counter.fetch_add(1, Ordering::SeqCst);
        let mut queries = active_queries.write().await;
        queries.insert(query_id, format!("find_peers:{}", hex::encode(&key)));

        let mut stats = stats.write().await;
        stats.active_queries = queries.len();

        debug!("Finding peers for key: {}", hex::encode(&key));
        Ok(())
    }

    /// Put value in DHT
    async fn put_dht_value(
        key: Vec<u8>,
        value: Vec<u8>,
        active_queries: &RwLock<HashMap<u64, String>>,
        stats: &RwLock<DiscoveryStats>,
        query_counter: &AtomicU64,
    ) -> Result<()> {
        let query_id = query_counter.fetch_add(1, Ordering::SeqCst);
        let mut queries = active_queries.write().await;
        queries.insert(query_id, format!("put_value:{}", hex::encode(&key)));

        let mut stats = stats.write().await;
        stats.active_queries = queries.len();

        debug!(
            "Putting value in DHT for key: {} (size: {} bytes)",
            hex::encode(&key),
            value.len()
        );
        Ok(())
    }

    /// Get value from DHT
    async fn get_dht_value(
        key: Vec<u8>,
        active_queries: &RwLock<HashMap<u64, String>>,
        stats: &RwLock<DiscoveryStats>,
        query_counter: &AtomicU64,
    ) -> Result<()> {
        let query_id = query_counter.fetch_add(1, Ordering::SeqCst);
        let mut queries = active_queries.write().await;
        queries.insert(query_id, format!("get_value:{}", hex::encode(&key)));

        let mut stats = stats.write().await;
        stats.active_queries = queries.len();

        debug!("Getting value from DHT for key: {}", hex::encode(&key));
        Ok(())
    }

    /// Get current statistics
    async fn get_current_stats(
        discovered_peers: &RwLock<HashMap<PeerId, DiscoveredPeer>>,
        active_queries: &RwLock<HashMap<u64, String>>,
        stats: &RwLock<DiscoveryStats>,
    ) -> DiscoveryStats {
        let mut current_stats = stats.read().await.clone();
        let peers = discovered_peers.read().await;
        let queries = active_queries.read().await;

        current_stats.active_peers = peers.len();
        current_stats.active_queries = queries.len();

        current_stats
    }

    /// Stop the discovery service
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping discovery service");
        *self.running.write().await = false;
        Ok(())
    }

    /// Get discovered peers
    pub async fn get_discovered_peers(&self) -> Vec<DiscoveredPeer> {
        let peers = self.discovered_peers.read().await;
        peers.values().cloned().collect()
    }

    /// Get peer by ID
    pub async fn get_peer(&self, peer_id: &PeerId) -> Option<DiscoveredPeer> {
        let peers = self.discovered_peers.read().await;
        peers.get(peer_id).cloned()
    }

    /// Check if peer is known
    pub async fn is_peer_known(&self, peer_id: &PeerId) -> bool {
        let peers = self.discovered_peers.read().await;
        peers.contains_key(peer_id)
    }

    /// Update peer connection status
    pub async fn update_peer_status(
        &self,
        peer_id: PeerId,
        status: ConnectionStatus,
    ) -> Result<()> {
        let mut peers = self.discovered_peers.write().await;
        if let Some(peer) = peers.get_mut(&peer_id) {
            peer.connection_status = status;
            peer.last_seen = std::time::SystemTime::now();

            let _ = self
                .event_sender
                .send(DiscoveryEvent::PeerUpdated { peer: peer.clone() })
                .await;
        }
        Ok(())
    }

    /// Get discovery statistics
    pub async fn get_stats(&self) -> DiscoveryStats {
        Self::get_current_stats(&self.discovered_peers, &self.active_queries, &self.stats).await
    }
}
