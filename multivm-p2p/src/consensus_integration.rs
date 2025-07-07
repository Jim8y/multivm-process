//! Consensus integration for P2P network layer
//!
//! This module provides the bridge between the P2P networking layer and the
//! MultiVM consensus system, enabling transaction broadcasting, block propagation,
//! and state synchronization across the network.

use crate::{
    core::network::P2PNetwork,
    error::{P2PError, P2PResult},
    protocol::messages::{MessagePayload, MessageSource, MessageTarget, NetworkMessage, VmType},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info};

/// Consensus block data for P2P propagation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusBlock {
    /// Block height
    pub height: u64,
    /// Block hash
    pub hash: String,
    /// Previous block hash
    pub previous_hash: String,
    /// Timestamp when block was created
    pub timestamp: u64,
    /// Transactions in this block
    pub transactions: Vec<ConsensusTransaction>,
    /// Block proposer
    pub proposer: String,
    /// VM type this block belongs to
    pub vm_type: VmType,
    /// Cross-VM operations in this block
    pub cross_vm_operations: Vec<CrossVmOperation>,
}

/// Transaction data for consensus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusTransaction {
    /// Transaction hash
    pub hash: String,
    /// Transaction data
    pub data: Vec<u8>,
    /// VM type for this transaction
    pub vm_type: VmType,
    /// Gas/compute limit
    pub gas_limit: u64,
    /// Transaction fee
    pub fee: u64,
    /// Nonce for ordering
    pub nonce: u64,
}

/// Cross-VM operation for state synchronization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossVmOperation {
    /// Operation ID
    pub id: String,
    /// Source VM
    pub source_vm: VmType,
    /// Target VM
    pub target_vm: VmType,
    /// Operation type
    pub operation_type: String,
    /// Operation data
    pub data: Vec<u8>,
    /// Status of the operation
    pub status: CrossVmOperationStatus,
}

/// Status of cross-VM operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrossVmOperationStatus {
    Pending,
    InProgress,
    Completed,
    Failed(String),
    Rolled(String),
}

/// Consensus event from the consensus layer
#[derive(Debug, Clone)]
pub enum ConsensusEvent {
    /// New block created and ready for propagation
    BlockCreated(ConsensusBlock),
    /// Transaction received from P2P network
    TransactionReceived(ConsensusTransaction),
    /// State synchronization request
    StateSyncRequest {
        vm_type: VmType,
        from_height: u64,
        to_height: u64,
    },
    /// Cross-VM operation completed
    CrossVmOperationCompleted(CrossVmOperation),
}

/// P2P event for consensus layer
#[derive(Debug, Clone)]
pub enum P2PConsensusEvent {
    /// Peer connected with consensus capabilities
    ConsensusCapablePeerConnected {
        peer_id: String,
        capabilities: PeerConsensusCapabilities,
    },
    /// Block received from network
    BlockReceived(ConsensusBlock),
    /// Transaction received from network
    TransactionReceived(ConsensusTransaction),
    /// State sync response received
    StateSyncResponse {
        vm_type: VmType,
        blocks: Vec<ConsensusBlock>,
        from_height: u64,
        to_height: u64,
    },
    /// Cross-VM operation completed
    CrossVmOperationCompleted(CrossVmOperation),
}

/// Consensus capabilities of a peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerConsensusCapabilities {
    /// Supported VM types
    pub supported_vms: Vec<VmType>,
    /// Maximum block size this peer can handle
    pub max_block_size: u64,
    /// Current block height known by this peer
    pub current_height: HashMap<VmType, u64>,
    /// Whether peer supports cross-VM operations
    pub supports_cross_vm: bool,
}

/// Consensus integration manager
pub struct ConsensusIntegration {
    /// P2P network layer
    network: Arc<RwLock<P2PNetwork>>,
    /// Event sender to consensus layer
    consensus_tx: mpsc::UnboundedSender<P2PConsensusEvent>,
    /// Event receiver from consensus layer
    consensus_rx: Arc<RwLock<Option<mpsc::UnboundedReceiver<ConsensusEvent>>>>,
    /// Known peer capabilities
    peer_capabilities: Arc<RwLock<HashMap<String, PeerConsensusCapabilities>>>,
    /// Block cache for recent blocks
    block_cache: Arc<RwLock<HashMap<String, ConsensusBlock>>>,
    /// Transaction pool for pending transactions
    pending_transactions: Arc<RwLock<HashMap<String, ConsensusTransaction>>>,
}

impl ConsensusIntegration {
    /// Create new consensus integration
    pub fn new(
        network: Arc<RwLock<P2PNetwork>>,
    ) -> (
        Self,
        mpsc::UnboundedReceiver<P2PConsensusEvent>,
        mpsc::UnboundedSender<ConsensusEvent>,
    ) {
        let (consensus_tx, consensus_rx) = mpsc::unbounded_channel();
        let (p2p_tx, p2p_rx) = mpsc::unbounded_channel();

        let integration = Self {
            network,
            consensus_tx,
            consensus_rx: Arc::new(RwLock::new(Some(p2p_rx))),
            peer_capabilities: Arc::new(RwLock::new(HashMap::new())),
            block_cache: Arc::new(RwLock::new(HashMap::new())),
            pending_transactions: Arc::new(RwLock::new(HashMap::new())),
        };

        (integration, consensus_rx, p2p_tx)
    }

    /// Start the consensus integration task
    pub async fn start(&self) -> P2PResult<()> {
        let consensus_rx = {
            let mut rx_lock = self.consensus_rx.write().await;
            rx_lock
                .take()
                .ok_or_else(|| P2PError::Internal("Consensus receiver already taken".to_string()))?
        };

        let network = self.network.clone();
        let consensus_tx = self.consensus_tx.clone();
        let peer_capabilities = self.peer_capabilities.clone();
        let block_cache = self.block_cache.clone();
        let pending_transactions = self.pending_transactions.clone();

        tokio::spawn(async move {
            Self::consensus_event_loop(
                consensus_rx,
                network,
                consensus_tx,
                peer_capabilities,
                block_cache,
                pending_transactions,
            )
            .await;
        });

        info!("Consensus integration started");
        Ok(())
    }

    /// Main event loop for consensus integration
    async fn consensus_event_loop(
        mut consensus_rx: mpsc::UnboundedReceiver<ConsensusEvent>,
        network: Arc<RwLock<P2PNetwork>>,
        _consensus_tx: mpsc::UnboundedSender<P2PConsensusEvent>,
        _peer_capabilities: Arc<RwLock<HashMap<String, PeerConsensusCapabilities>>>,
        block_cache: Arc<RwLock<HashMap<String, ConsensusBlock>>>,
        pending_transactions: Arc<RwLock<HashMap<String, ConsensusTransaction>>>,
    ) {
        while let Some(event) = consensus_rx.recv().await {
            match event {
                ConsensusEvent::BlockCreated(block) => {
                    if let Err(e) = Self::broadcast_block(&network, &block).await {
                        error!("Failed to broadcast block: {}", e);
                    } else {
                        // Cache the block
                        block_cache.write().await.insert(block.hash.clone(), block);
                    }
                }
                ConsensusEvent::TransactionReceived(tx) => {
                    // Add to pending transactions
                    pending_transactions
                        .write()
                        .await
                        .insert(tx.hash.clone(), tx.clone());

                    // Broadcast transaction to network
                    if let Err(e) = Self::broadcast_transaction(&network, &tx).await {
                        error!("Failed to broadcast transaction: {}", e);
                    }
                }
                ConsensusEvent::StateSyncRequest {
                    vm_type,
                    from_height,
                    to_height,
                } => {
                    if let Err(e) =
                        Self::request_state_sync(&network, vm_type, from_height, to_height).await
                    {
                        error!("Failed to request state sync: {}", e);
                    }
                }
                ConsensusEvent::CrossVmOperationCompleted(operation) => {
                    if let Err(e) = Self::broadcast_cross_vm_operation(&network, &operation).await {
                        error!("Failed to broadcast cross-VM operation: {}", e);
                    }
                }
            }
        }
    }

    /// Broadcast block to the network
    async fn broadcast_block(
        network: &Arc<RwLock<P2PNetwork>>,
        block: &ConsensusBlock,
    ) -> P2PResult<()> {
        let message = NetworkMessage {
            id: format!("block_{}", block.hash),
            source: MessageSource::MultiVmLayer,
            target: MessageTarget::Broadcast,
            payload: MessagePayload::consensus_block(block.clone()),
            timestamp: chrono::Utc::now(),
            version: 1,
            metadata: std::collections::HashMap::new(),
        };

        let mut network_guard = network.write().await;
        network_guard
            .broadcast(message)
            .await
            .map_err(|e| P2PError::Internal(e.to_string()))
    }

    /// Broadcast transaction to the network
    async fn broadcast_transaction(
        network: &Arc<RwLock<P2PNetwork>>,
        tx: &ConsensusTransaction,
    ) -> P2PResult<()> {
        let message = NetworkMessage {
            id: format!("tx_{}", tx.hash),
            source: MessageSource::MultiVmLayer,
            target: MessageTarget::Broadcast,
            payload: MessagePayload::transaction(tx.clone()),
            timestamp: chrono::Utc::now(),
            version: 1,
            metadata: std::collections::HashMap::new(),
        };

        let mut network_guard = network.write().await;
        network_guard
            .broadcast(message)
            .await
            .map_err(|e| P2PError::Internal(e.to_string()))
    }

    /// Request state synchronization from peers
    async fn request_state_sync(
        network: &Arc<RwLock<P2PNetwork>>,
        vm_type: VmType,
        from_height: u64,
        to_height: u64,
    ) -> P2PResult<()> {
        let request = StateSyncRequest {
            vm_type,
            from_height,
            to_height,
            requester_id: "local_node".to_string(), // Production: Use actual local peer ID
        };

        let message = NetworkMessage {
            id: format!(
                "state_sync_{}_{}_{}_{}",
                vm_type,
                from_height,
                to_height,
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            ),
            source: MessageSource::MultiVmLayer,
            target: MessageTarget::Broadcast,
            payload: MessagePayload::state_sync_request(request),
            timestamp: chrono::Utc::now(),
            version: 1,
            metadata: std::collections::HashMap::new(),
        };

        let mut network_guard = network.write().await;
        network_guard
            .broadcast(message)
            .await
            .map_err(|e| P2PError::Internal(e.to_string()))
    }

    /// Broadcast cross-VM operation completion
    async fn broadcast_cross_vm_operation(
        network: &Arc<RwLock<P2PNetwork>>,
        operation: &CrossVmOperation,
    ) -> P2PResult<()> {
        let message = NetworkMessage {
            id: format!("cross_vm_{}", operation.id),
            source: MessageSource::MultiVmLayer,
            target: MessageTarget::Broadcast,
            payload: MessagePayload::cross_vm_operation(operation.clone()),
            timestamp: chrono::Utc::now(),
            version: 1,
            metadata: std::collections::HashMap::new(),
        };

        let mut network_guard = network.write().await;
        network_guard
            .broadcast(message)
            .await
            .map_err(|e| P2PError::Internal(e.to_string()))
    }

    /// Handle incoming P2P message for consensus
    pub async fn handle_network_message(&self, message: &NetworkMessage) -> P2PResult<()> {
        match &message.payload {
            MessagePayload::Custom(json_value) => {
                // Try to deserialize as different consensus types
                if let Ok(block) = serde_json::from_value::<ConsensusBlock>(json_value.clone()) {
                    // Cache the block
                    self.block_cache
                        .write()
                        .await
                        .insert(block.hash.clone(), block.clone());

                    // Forward to consensus layer
                    let event = P2PConsensusEvent::BlockReceived(block);
                    if let Err(e) = self.consensus_tx.send(event) {
                        error!("Failed to send block to consensus layer: {}", e);
                    }
                } else if let Ok(tx) =
                    serde_json::from_value::<ConsensusTransaction>(json_value.clone())
                {
                    // Add to pending transactions
                    self.pending_transactions
                        .write()
                        .await
                        .insert(tx.hash.clone(), tx.clone());

                    // Forward to consensus layer
                    let event = P2PConsensusEvent::TransactionReceived(tx);
                    if let Err(e) = self.consensus_tx.send(event) {
                        error!("Failed to send transaction to consensus layer: {}", e);
                    }
                } else if let Ok(request) =
                    serde_json::from_value::<StateSyncRequest>(json_value.clone())
                {
                    // Handle state sync request
                    if let Err(e) = self.handle_state_sync_request(&request).await {
                        error!("Failed to handle state sync request: {}", e);
                    }
                } else if let Ok(response) =
                    serde_json::from_value::<StateSyncResponse>(json_value.clone())
                {
                    // Handle state sync response
                    let event = P2PConsensusEvent::StateSyncResponse {
                        vm_type: response.vm_type,
                        blocks: response.blocks.clone(),
                        from_height: response.from_height,
                        to_height: response.to_height,
                    };
                    if let Err(e) = self.consensus_tx.send(event) {
                        error!(
                            "Failed to send state sync response to consensus layer: {}",
                            e
                        );
                    }
                } else if let Ok(operation) =
                    serde_json::from_value::<CrossVmOperation>(json_value.clone())
                {
                    // Forward cross-VM operation to consensus layer
                    let event = P2PConsensusEvent::CrossVmOperationCompleted(operation);
                    if let Err(e) = self.consensus_tx.send(event) {
                        error!(
                            "Failed to send cross-VM operation to consensus layer: {}",
                            e
                        );
                    }
                } else {
                    debug!("Ignoring unknown custom message type");
                }
            }
            _ => {
                // Ignore other message types
                debug!("Ignoring non-consensus message: {:?}", message.payload);
            }
        }

        Ok(())
    }

    /// Handle state synchronization request
    async fn handle_state_sync_request(&self, request: &StateSyncRequest) -> P2PResult<()> {
        // Get blocks from cache or request from consensus layer
        let blocks = self
            .get_blocks_for_sync(request.vm_type, request.from_height, request.to_height)
            .await?;

        let response = StateSyncResponse {
            vm_type: request.vm_type,
            blocks,
            from_height: request.from_height,
            to_height: request.to_height,
            responder_id: "local_node".to_string(), // Production: Use actual local peer ID
        };

        let message = NetworkMessage {
            id: format!(
                "state_sync_response_{}_{}_{}_{}",
                request.vm_type,
                request.from_height,
                request.to_height,
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            ),
            source: MessageSource::MultiVmLayer,
            target: MessageTarget::Peer(request.requester_id.clone()),
            payload: MessagePayload::state_sync_response(response),
            timestamp: chrono::Utc::now(),
            version: 1,
            metadata: std::collections::HashMap::new(),
        };

        let mut network_guard = self.network.write().await;
        network_guard
            .send_to_peer(request.requester_id.clone(), message)
            .await
            .map_err(|e| P2PError::Internal(e.to_string()))
    }

    /// Get blocks for state synchronization
    async fn get_blocks_for_sync(
        &self,
        vm_type: VmType,
        from_height: u64,
        to_height: u64,
    ) -> P2PResult<Vec<ConsensusBlock>> {
        // First check cache
        let cache = self.block_cache.read().await;
        let mut blocks = Vec::new();

        for _height in from_height..=to_height {
            // In a real implementation, we would have a height-indexed cache or query the consensus layer
            // For now, return cached blocks that match the VM type
            for block in cache.values() {
                if block.vm_type == vm_type
                    && block.height >= from_height
                    && block.height <= to_height
                {
                    blocks.push(block.clone());
                }
            }
        }

        // Sort by height
        blocks.sort_by_key(|b| b.height);

        // Production implementation would:
        // 1. Maintain a height-indexed cache for efficient lookups
        // 2. Query the consensus layer for missing blocks
        // 3. Validate block continuity and integrity
        // For now, return available cached blocks
        Ok(blocks)
    }
}

/// State synchronization request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSyncRequest {
    pub vm_type: VmType,
    pub from_height: u64,
    pub to_height: u64,
    pub requester_id: String,
}

/// State synchronization response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSyncResponse {
    pub vm_type: VmType,
    pub blocks: Vec<ConsensusBlock>,
    pub from_height: u64,
    pub to_height: u64,
    pub responder_id: String,
}

/// Extend MessagePayload to include consensus messages
impl MessagePayload {
    pub fn consensus_block(block: ConsensusBlock) -> Self {
        MessagePayload::Custom(serde_json::to_value(block).unwrap_or_default())
    }

    pub fn transaction(tx: ConsensusTransaction) -> Self {
        MessagePayload::Custom(serde_json::to_value(tx).unwrap_or_default())
    }

    pub fn state_sync_request(request: StateSyncRequest) -> Self {
        MessagePayload::Custom(serde_json::to_value(request).unwrap_or_default())
    }

    pub fn state_sync_response(response: StateSyncResponse) -> Self {
        MessagePayload::Custom(serde_json::to_value(response).unwrap_or_default())
    }

    pub fn cross_vm_operation(operation: CrossVmOperation) -> Self {
        MessagePayload::Custom(serde_json::to_value(operation).unwrap_or_default())
    }
}
