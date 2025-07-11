//! Connection management and pooling

use crate::{NetworkError, Result};
use consensus::{Message, NodeId};
use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use multivm_core::utils::RetryConfig;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tracing::{error, info, warn};

/// Maximum message size (10MB)
const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024;

/// Connection to a remote node
#[derive(Debug)]
pub struct Connection {
    node_id: NodeId,
    sink: mpsc::Sender<Vec<u8>>,
    _handle: tokio::task::JoinHandle<()>,
}

impl Connection {
    /// Create a new connection
    pub async fn new<S>(node_id: NodeId, stream: S) -> Result<Self>
    where
        S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    {
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(100);
        
        // Use length-delimited framing
        let framed = Framed::new(
            stream,
            LengthDelimitedCodec::builder()
                .max_frame_length(MAX_MESSAGE_SIZE)
                .new_codec(),
        );
        
        let (mut sink, _stream) = framed.split();
        
        // Spawn write task
        let write_handle = tokio::spawn(async move {
            while let Some(data) = rx.recv().await {
                if let Err(e) = sink.send(data.into()).await {
                    error!(node_id = %node_id, error = %e, "Failed to send message");
                    break;
                }
            }
        });
        
        Ok(Self {
            node_id,
            sink: tx,
            _handle: write_handle,
        })
    }
    
    /// Send a message
    pub async fn send(&self, msg: &Message) -> Result<()> {
        let data = bincode::serialize(msg)?;
        
        if data.len() > MAX_MESSAGE_SIZE {
            return Err(NetworkError::MessageTooLarge {
                size: data.len(),
                max: MAX_MESSAGE_SIZE,
            });
        }
        
        self.sink
            .send(data)
            .await
            .map_err(|_| NetworkError::ConnectionClosed)?;
            
        Ok(())
    }
    
    /// Check if connection is alive
    pub fn is_alive(&self) -> bool {
        !self.sink.is_closed()
    }
}

/// Connection pool for managing connections to multiple nodes
#[derive(Debug)]
pub struct ConnectionPool {
    connections: Arc<DashMap<NodeId, Arc<Connection>>>,
    config: ConnectionConfig,
}

#[derive(Clone, Debug)]
pub struct ConnectionConfig {
    pub max_connections_per_node: usize,
    pub connection_timeout: std::time::Duration,
    pub retry_config: RetryConfig,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            max_connections_per_node: 5,
            connection_timeout: std::time::Duration::from_secs(10),
            retry_config: RetryConfig::default(),
        }
    }
}

impl ConnectionPool {
    /// Create a new connection pool
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            connections: Arc::new(DashMap::new()),
            config,
        }
    }
    
    /// Add a connection to the pool
    pub async fn add_connection<S>(&self, node_id: NodeId, stream: S) -> Result<()>
    where
        S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    {
        let conn = Connection::new(node_id, stream).await?;
        self.connections.insert(node_id, Arc::new(conn));
        info!(node_id = %node_id, "Added connection to pool");
        Ok(())
    }
    
    /// Get a connection from the pool
    pub fn get(&self, node_id: &NodeId) -> Option<Arc<Connection>> {
        self.connections.get(node_id).map(|c| c.clone())
    }
    
    /// Remove a connection from the pool
    pub fn remove(&self, node_id: &NodeId) {
        if let Some((_, _conn)) = self.connections.remove(node_id) {
            info!(node_id = %node_id, "Removed connection from pool");
        }
    }
    
    /// Remove dead connections
    pub fn cleanup(&self) {
        self.connections.retain(|node_id, conn| {
            if conn.is_alive() {
                true
            } else {
                warn!(node_id = %node_id, "Removing dead connection");
                false
            }
        });
    }
    
    /// Get all active node IDs
    pub fn active_nodes(&self) -> Vec<NodeId> {
        self.connections
            .iter()
            .filter(|entry| entry.value().is_alive())
            .map(|entry| *entry.key())
            .collect()
    }
}

/// Message router for handling incoming messages
#[derive(Debug)]
pub struct MessageRouter {
    handlers: Arc<RwLock<Vec<mpsc::Sender<(NodeId, Message)>>>>,
}

impl MessageRouter {
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Subscribe to incoming messages
    pub fn subscribe(&self) -> mpsc::Receiver<(NodeId, Message)> {
        let (tx, rx) = mpsc::channel(1000);
        self.handlers.write().push(tx);
        rx
    }
    
    /// Route a message to all subscribers
    pub async fn route(&self, from: NodeId, msg: Message) {
        let handlers = self.handlers.read().clone();
        for handler in handlers {
            if handler.send((from, msg.clone())).await.is_err() {
                // Handler disconnected, will be cleaned up later
            }
        }
        
        // Clean up disconnected handlers
        self.handlers.write().retain(|h| !h.is_closed());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_connection_pool() {
        let pool = ConnectionPool::new(ConnectionConfig::default());
        assert_eq!(pool.active_nodes().len(), 0);
        
        // Add a mock connection would require more setup
        // This is a basic structure test
    }
}