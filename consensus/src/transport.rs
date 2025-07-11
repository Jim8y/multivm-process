//! Network transport layer for consensus messages

use crate::{Message, NodeId, Result, ConsensusError};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Message with transport metadata
#[derive(Debug, Clone)]
pub struct TransportMessage {
    /// The actual consensus message
    pub message: Message,
    /// Optional response channel for RPC-style communication
    pub response_tx: Option<mpsc::Sender<Message>>,
}

/// Transport trait for sending and receiving consensus messages
#[async_trait]
pub trait Transport: Send + Sync {
    /// Send a message to a specific node
    async fn send(&self, to: NodeId, message: Message) -> Result<()>;
    
    /// Send a message and wait for response (RPC-style)
    async fn send_rpc(&self, to: NodeId, message: Message) -> Result<Message>;
    
    /// Receive incoming messages
    async fn recv(&mut self) -> Result<TransportMessage>;
    
    /// Get the local node ID
    fn local_id(&self) -> NodeId;
    
    /// Check if a node is reachable
    async fn is_reachable(&self, node: NodeId) -> bool;
}

/// In-memory transport for testing
#[derive(Debug)]
pub struct MemoryTransport {
    local_id: NodeId,
    inbox: mpsc::Receiver<TransportMessage>,
    outboxes: Arc<RwLock<HashMap<NodeId, mpsc::Sender<TransportMessage>>>>,
}

impl MemoryTransport {
    /// Create a new memory transport
    pub fn new(local_id: NodeId, inbox: mpsc::Receiver<TransportMessage>) -> Self {
        Self {
            local_id,
            inbox,
            outboxes: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Register a peer's inbox
    pub async fn register_peer(&self, peer_id: NodeId, sender: mpsc::Sender<TransportMessage>) {
        self.outboxes.write().await.insert(peer_id, sender);
    }
}

#[async_trait]
impl Transport for MemoryTransport {
    async fn send(&self, to: NodeId, message: Message) -> Result<()> {
        let outboxes = self.outboxes.read().await;
        if let Some(sender) = outboxes.get(&to) {
            sender
                .send(TransportMessage {
                    message,
                    response_tx: None,
                })
                .await
                .map_err(|_| ConsensusError::ChannelSend)?;
            Ok(())
        } else {
            Err(ConsensusError::Network(format!("Node {} not found", to)))
        }
    }
    
    async fn send_rpc(&self, to: NodeId, message: Message) -> Result<Message> {
        let (response_tx, mut response_rx) = mpsc::channel(1);
        let outboxes = self.outboxes.read().await;
        
        if let Some(sender) = outboxes.get(&to) {
            sender
                .send(TransportMessage {
                    message,
                    response_tx: Some(response_tx),
                })
                .await
                .map_err(|_| ConsensusError::ChannelSend)?;
                
            response_rx
                .recv()
                .await
                .ok_or(ConsensusError::ChannelReceive)
        } else {
            Err(ConsensusError::Network(format!("Node {} not found", to)))
        }
    }
    
    async fn recv(&mut self) -> Result<TransportMessage> {
        self.inbox
            .recv()
            .await
            .ok_or(ConsensusError::ChannelReceive)
    }
    
    fn local_id(&self) -> NodeId {
        self.local_id
    }
    
    async fn is_reachable(&self, node: NodeId) -> bool {
        self.outboxes.read().await.contains_key(&node)
    }
}