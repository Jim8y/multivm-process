//! TCP transport implementation for consensus

use crate::{
    connection::{Connection, ConnectionConfig, ConnectionPool, MessageRouter},
    tls::TlsConfig,
    NetworkError, Result,
};
use async_trait::async_trait;
use consensus::{Message, NodeId, TransportMessage};
use futures::StreamExt;
use multivm_core::utils::retry_with_backoff;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::time::{timeout, Duration};
use tokio_rustls::{TlsAcceptor, TlsConnector};
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tracing::{debug, error, info, warn};

/// TCP transport configuration
#[derive(Clone, Debug)]
pub struct TcpTransportConfig {
    pub listen_addr: SocketAddr,
    pub tls: Option<TlsConfig>,
    pub connection: ConnectionConfig,
    pub handshake_timeout: std::time::Duration,
    pub max_connections: usize,
    pub connection_rate_limit: u64, // connections per second
}

impl Default for TcpTransportConfig {
    fn default() -> Self {
        Self {
            listen_addr: std::net::SocketAddr::from(([0, 0, 0, 0], 7000)),
            tls: None,
            connection: ConnectionConfig::default(),
            handshake_timeout: std::time::Duration::from_secs(10),
            max_connections: 1000,
            connection_rate_limit: 100,
        }
    }
}

/// TCP-based transport for consensus
#[derive(Debug)]
pub struct TcpTransport {
    node_id: NodeId,
    config: TcpTransportConfig,
    pool: Arc<ConnectionPool>,
    router: Arc<MessageRouter>,
    peers: Arc<RwLock<HashMap<NodeId, SocketAddr>>>,
    inbox: mpsc::Receiver<(NodeId, Message)>,
    rpc_requests: Arc<dashmap::DashMap<u64, oneshot::Sender<Message>>>,
    rpc_counter: Arc<std::sync::atomic::AtomicU64>,
    _listener_handle: Option<tokio::task::JoinHandle<()>>,
}

impl TcpTransport {
    /// Create a new TCP transport
    pub async fn new(
        node_id: NodeId,
        config: TcpTransportConfig,
        peers: HashMap<NodeId, SocketAddr>,
    ) -> Result<Self> {
        let pool = Arc::new(ConnectionPool::new(config.connection.clone()));
        let router = Arc::new(MessageRouter::new());
        let peers = Arc::new(RwLock::new(peers));
        
        // Create inbox from router
        let inbox = router.subscribe();
        let rpc_requests = Arc::new(dashmap::DashMap::new());
        let rpc_counter = Arc::new(std::sync::atomic::AtomicU64::new(0));
        
        // Start listener
        let listener_handle = if config.listen_addr.port() > 0 {
            Some(Self::start_listener(
                config.clone(),
                pool.clone(),
                router.clone(),
            ).await?)
        } else {
            None
        };
        
        Ok(Self {
            node_id,
            config,
            pool,
            router,
            peers,
            inbox,
            rpc_requests,
            rpc_counter,
            _listener_handle: listener_handle,
        })
    }
    
    /// Start TCP listener
    async fn start_listener(
        config: TcpTransportConfig,
        pool: Arc<ConnectionPool>,
        router: Arc<MessageRouter>,
    ) -> Result<tokio::task::JoinHandle<()>> {
        let listener = TcpListener::bind(config.listen_addr).await?;
        info!(addr = %config.listen_addr, "TCP transport listening");
        
        let tls_acceptor = if let Some(tls_config) = &config.tls {
            Some(TlsAcceptor::from(tls_config.load_server_config().await?))
        } else {
            None
        };
        
        let rate_limiter = Arc::new(multivm_core::utils::RateLimiter::new(
            config.connection_rate_limit,
            config.connection_rate_limit * 2, // burst
        ));
        let active_connections = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        
        let handle = tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, peer_addr)) => {
                        // Check connection limit
                        let current = active_connections.load(std::sync::atomic::Ordering::SeqCst);
                        if current >= config.max_connections {
                            warn!(peer_addr = %peer_addr, "Connection limit reached");
                            continue;
                        }
                        
                        // Rate limit connections
                        if !rate_limiter.try_acquire(1) {
                            warn!(peer_addr = %peer_addr, "Connection rate limit exceeded");
                            continue;
                        }
                        
                        debug!(peer_addr = %peer_addr, "Accepted connection");
                        active_connections.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        
                        let pool = pool.clone();
                        let router = router.clone();
                        let tls_acceptor = tls_acceptor.clone();
                        let config = config.clone();
                        let active_connections = active_connections.clone();
                        
                        tokio::spawn(async move {
                            if let Err(e) = Self::handle_connection(
                                stream,
                                peer_addr,
                                tls_acceptor,
                                config,
                                pool,
                                router,
                            ).await {
                                error!(peer_addr = %peer_addr, error = %e, "Connection error");
                            }
                            active_connections.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                        });
                    }
                    Err(e) => {
                        error!(error = %e, "Accept error");
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    }
                }
            }
        });
        
        Ok(handle)
    }
    
    /// Handle incoming connection
    async fn handle_connection(
        stream: TcpStream,
        _peer_addr: SocketAddr,
        tls_acceptor: Option<TlsAcceptor>,
        config: TcpTransportConfig,
        pool: Arc<ConnectionPool>,
        router: Arc<MessageRouter>,
    ) -> Result<()> {
        // Apply TLS if configured
        if let Some(acceptor) = tls_acceptor {
            let tls_stream = acceptor.accept(stream).await.map_err(|e| {
                NetworkError::Tls(format!("TLS accept failed: {}", e))
            })?;
            
            // Frame the TLS connection
            let framed = Framed::new(
                tls_stream,
                LengthDelimitedCodec::builder()
                    .max_frame_length(10 * 1024 * 1024)
                    .new_codec(),
            );
            
            // Handle the TLS connection
            Self::handle_framed_connection(framed, config, pool, router).await
        } else {
            // Frame the plain TCP connection
            let framed = Framed::new(
                stream,
                LengthDelimitedCodec::builder()
                    .max_frame_length(10 * 1024 * 1024)
                    .new_codec(),
            );
            
            // Handle the plain connection
            Self::handle_framed_connection(framed, config, pool, router).await
        }
    }
    
    /// Handle a framed connection (works with both TLS and plain TCP)
    async fn handle_framed_connection<S>(
        mut framed: Framed<S, LengthDelimitedCodec>,
        config: TcpTransportConfig,
        pool: Arc<ConnectionPool>,
        router: Arc<MessageRouter>,
    ) -> Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin,
    {
        // Read handshake message
        let handshake_result = tokio::time::timeout(
            config.handshake_timeout,
            async {
                if let Some(Ok(data)) = framed.next().await {
                    let msg: Message = bincode::deserialize(&data[..])?;
                    Ok(msg)
                } else {
                    Err(NetworkError::InvalidMessage)
                }
            }
        ).await;
        
        let msg = match handshake_result {
            Ok(Ok(msg)) => msg,
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(NetworkError::Timeout),
        };
        
        // Extract node ID from handshake
        let peer_node_id = msg.from;
        info!(node_id = %peer_node_id, "Peer connected");
        
        // Note: We can't add to the pool here because we're already using the stream
        // In production, you'd need a bidirectional connection handler
        
        // Route incoming messages
        while let Some(result) = framed.next().await {
            match result {
                Ok(data) => {
                    match bincode::deserialize::<Message>(&data[..]) {
                        Ok(msg) => {
                            router.route(peer_node_id, msg).await;
                        }
                        Err(e) => {
                            warn!(error = %e, "Failed to deserialize message");
                        }
                    }
                }
                Err(e) => {
                    error!(error = %e, "Connection error");
                    break;
                }
            }
        }
        
        pool.remove(&peer_node_id);
        info!(node_id = %peer_node_id, "Peer disconnected");
        
        Ok(())
    }
    
    /// Connect to a peer
    async fn connect_to_peer(&self, node_id: NodeId) -> Result<()> {
        let addr = self.peers.read().await
            .get(&node_id)
            .ok_or_else(|| NetworkError::NodeNotFound(node_id.to_string()))?
            .clone();
            
        info!(node_id = %node_id, addr = %addr, "Connecting to peer");
        
        let stream = TcpStream::connect(addr).await?;
        
        // Apply TLS if configured and add to pool
        if let Some(tls_config) = &self.config.tls {
            let connector = TlsConnector::from(tls_config.load_client_config().await?);
            // Use the address as a string for server name validation
            // In production, this should be a proper domain name
            let domain = match addr.ip() {
                std::net::IpAddr::V4(ip) => {
                    rustls::pki_types::ServerName::IpAddress(
                        rustls::pki_types::IpAddr::V4(ip.into())
                    )
                }
                std::net::IpAddr::V6(ip) => {
                    rustls::pki_types::ServerName::IpAddress(
                        rustls::pki_types::IpAddr::V6(ip.into())
                    )
                }
            };
            let tls_stream = connector.connect(domain, stream).await
                .map_err(|e| NetworkError::Tls(format!("TLS connect failed: {}", e)))?;
            self.pool.add_connection(node_id, tls_stream).await?;
        } else {
            self.pool.add_connection(node_id, stream).await?;
        }
        
        // Send handshake
        let handshake = Message {
            term: 0,
            from: self.node_id,
            to: node_id,
            msg_type: consensus::MessageType::Handshake,
            payload: consensus::MessagePayload::Handshake,
        };
        
        // Send handshake
        if let Some(conn) = self.pool.get(&node_id) {
            conn.send(&handshake).await?;
        }
        
        Ok(())
    }
    
    /// Ensure connection to peer
    async fn ensure_connection(&self, node_id: NodeId) -> Result<Arc<Connection>> {
        // Check if we have an active connection
        if let Some(conn) = self.pool.get(&node_id) {
            if conn.is_alive() {
                return Ok(conn);
            }
        }
        
        // Try to connect
        let conn = retry_with_backoff(&self.config.connection.retry_config, || async {
            self.connect_to_peer(node_id).await
                .map_err(|e| multivm_core::Error::Network(e.to_string()))?;
            self.pool.get(&node_id)
                .ok_or_else(|| multivm_core::Error::Network("Failed to establish connection".to_string()))
        }).await
        .map_err(|e| NetworkError::ConnectionFailed(e.to_string()))?;
        
        Ok(conn)
    }
}

#[async_trait]
impl consensus::transport::Transport for TcpTransport {
    async fn send(&self, to: NodeId, message: Message) -> consensus::Result<()> {
        let conn = self.ensure_connection(to).await
            .map_err(|e| consensus::ConsensusError::Network(e.to_string()))?;
            
        conn.send(&message).await
            .map_err(|e| consensus::ConsensusError::Network(e.to_string()))?;
            
        Ok(())
    }
    
    async fn send_rpc(&self, to: NodeId, message: Message) -> consensus::Result<Message> {
        // Generate unique RPC ID
        let rpc_id = self.rpc_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        
        // Add RPC ID to message metadata
        // Note: This assumes Message has a way to carry RPC metadata
        // In production, you'd modify the Message type to include an rpc_id field
        
        // Create response channel
        let (tx, rx) = oneshot::channel();
        self.rpc_requests.insert(rpc_id, tx);
        
        // Send the message
        if let Err(e) = self.send(to, message).await {
            self.rpc_requests.remove(&rpc_id);
            return Err(e);
        }
        
        // Wait for response with timeout
        match timeout(Duration::from_secs(30), rx).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => {
                self.rpc_requests.remove(&rpc_id);
                Err(consensus::ConsensusError::Other("RPC response channel closed".to_string()))
            }
            Err(_) => {
                self.rpc_requests.remove(&rpc_id);
                Err(consensus::ConsensusError::Timeout)
            }
        }
    }
    
    async fn recv(&mut self) -> consensus::Result<TransportMessage> {
        match self.inbox.recv().await {
            Some((_, msg)) => Ok(TransportMessage {
                message: msg,
                response_tx: None,
            }),
            None => Err(consensus::ConsensusError::ChannelReceive),
        }
    }
    
    fn local_id(&self) -> NodeId {
        self.node_id
    }
    
    async fn is_reachable(&self, node: NodeId) -> bool {
        if let Some(conn) = self.pool.get(&node) {
            conn.is_alive()
        } else {
            // Try to connect
            self.ensure_connection(node).await.is_ok()
        }
    }
}

impl Drop for TcpTransport {
    fn drop(&mut self) {
        // Cleanup will happen when handle is dropped
    }
}