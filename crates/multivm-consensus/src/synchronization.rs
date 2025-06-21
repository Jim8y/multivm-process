//! Block synchronization for MultiVM consensus
//!
//! This module provides comprehensive block synchronization capabilities
//! to ensure all validators maintain consistent blockchain state.

use crate::block::MultiVMBlock;
use crate::ConsensusError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::{mpsc, RwLock as AsyncRwLock};
use tracing::{debug, error, info};

// ================================================================================================
// Block Synchronization Types and Configuration
// ================================================================================================

/// Configuration for block synchronization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockSyncConfig {
    /// Enable block synchronization
    pub enabled: bool,
    /// Maximum number of blocks to request in a single sync operation
    pub max_blocks_per_request: usize,
    /// Maximum number of concurrent sync operations
    pub max_concurrent_syncs: usize,
    /// Timeout for sync requests in milliseconds
    pub sync_request_timeout_ms: u64,
    /// Interval between sync attempts in seconds
    pub sync_attempt_interval_secs: u64,
    /// Maximum number of retry attempts for failed sync requests
    pub max_retry_attempts: u32,
    /// Enable fast sync for initial synchronization
    pub enable_fast_sync: bool,
    /// Height difference threshold to trigger sync
    pub sync_threshold_height: u64,
    /// Maximum number of blocks to keep in sync cache
    pub max_cached_blocks: usize,
    /// Enable block verification during sync
    pub verify_blocks_during_sync: bool,
}

impl Default for BlockSyncConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_blocks_per_request: 100,
            max_concurrent_syncs: 5,
            sync_request_timeout_ms: 30000, // 30 seconds
            sync_attempt_interval_secs: 10,
            max_retry_attempts: 3,
            enable_fast_sync: true,
            sync_threshold_height: 10,
            max_cached_blocks: 1000,
            verify_blocks_during_sync: true,
        }
    }
}

impl BlockSyncConfig {
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.max_blocks_per_request == 0 {
            return Err("Max blocks per request must be greater than 0".to_string());
        }
        if self.max_concurrent_syncs == 0 {
            return Err("Max concurrent syncs must be greater than 0".to_string());
        }
        if self.sync_request_timeout_ms == 0 {
            return Err("Sync request timeout must be greater than 0".to_string());
        }
        if self.sync_attempt_interval_secs == 0 {
            return Err("Sync attempt interval must be greater than 0".to_string());
        }
        if self.max_cached_blocks == 0 {
            return Err("Max cached blocks must be greater than 0".to_string());
        }
        Ok(())
    }
}

/// Error types for block synchronization operations
#[derive(Debug, thiserror::Error)]
pub enum BlockSyncError {
    #[error("Sync request failed: {0}")]
    SyncRequestFailed(String),
    #[error("Sync timeout: {0}")]
    SyncTimeout(String),
    #[error("Invalid block data: {0}")]
    InvalidBlockData(String),
    #[error("Sync verification failed: {0}")]
    VerificationFailed(String),
    #[error("Peer unavailable: {0}")]
    PeerUnavailable(String),
    #[error("Consensus error: {0}")]
    ConsensusError(#[from] ConsensusError),
    #[error("Network error: {0}")]
    NetworkError(String),
}

impl BlockSyncError {
    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::SyncRequestFailed(_) => true,
            Self::SyncTimeout(_) => true,
            Self::InvalidBlockData(_) => false,
            Self::VerificationFailed(_) => false,
            Self::PeerUnavailable(_) => true,
            Self::ConsensusError(_) => true,
            Self::NetworkError(_) => true,
        }
    }

    /// Check if this error is critical
    pub fn is_critical(&self) -> bool {
        matches!(self, Self::InvalidBlockData(_) | Self::VerificationFailed(_))
    }

    /// Get error category for metrics
    pub fn category(&self) -> &'static str {
        match self {
            Self::SyncRequestFailed(_) => "sync_request_failed",
            Self::SyncTimeout(_) => "sync_timeout",
            Self::InvalidBlockData(_) => "invalid_block_data",
            Self::VerificationFailed(_) => "verification_failed",
            Self::PeerUnavailable(_) => "peer_unavailable",
            Self::ConsensusError(_) => "consensus_error",
            Self::NetworkError(_) => "network_error",
        }
    }
}

/// Request for block synchronization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRequest {
    /// Request ID for tracking
    pub id: String,
    /// Starting height for sync
    pub start_height: u64,
    /// Ending height for sync (optional)
    pub end_height: Option<u64>,
    /// Maximum number of blocks to return
    pub max_blocks: usize,
    /// Whether to include block data or just headers
    pub include_block_data: bool,
    /// Timestamp when request was created
    pub created_at: SystemTime,
}

impl SyncRequest {
    /// Create a new sync request
    pub fn new(start_height: u64, end_height: Option<u64>, max_blocks: usize) -> Self {
        let id = format!(
            "sync-{}-{}",
            start_height,
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );

        Self {
            id,
            start_height,
            end_height,
            max_blocks,
            include_block_data: true,
            created_at: SystemTime::now(),
        }
    }

    /// Check if request has expired
    pub fn is_expired(&self, timeout_ms: u64) -> bool {
        if let Ok(elapsed) = self.created_at.elapsed() {
            elapsed.as_millis() as u64 > timeout_ms
        } else {
            true
        }
    }
}

/// Response to block synchronization request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResponse {
    /// Response ID matching the request
    pub request_id: String,
    /// Blocks returned in response
    pub blocks: Vec<MultiVMBlock>,
    /// Whether this is the complete response or partial
    pub is_complete: bool,
    /// Current height of responding peer
    pub peer_height: u64,
    /// Timestamp when response was created
    pub created_at: SystemTime,
}

impl SyncResponse {
    /// Create a new sync response
    pub fn new(request_id: String, blocks: Vec<MultiVMBlock>, peer_height: u64) -> Self {
        Self {
            request_id,
            is_complete: true,
            blocks,
            peer_height,
            created_at: SystemTime::now(),
        }
    }

    /// Create a partial sync response
    pub fn partial(request_id: String, blocks: Vec<MultiVMBlock>, peer_height: u64) -> Self {
        Self {
            request_id,
            is_complete: false,
            blocks,
            peer_height,
            created_at: SystemTime::now(),
        }
    }
}

/// Status of block synchronization operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BlockSyncStatus {
    /// Synchronization is idle
    Idle,
    /// Synchronization is in progress
    Syncing,
    /// Synchronization completed successfully
    Completed,
    /// Synchronization failed
    Failed,
    /// Fast sync in progress
    FastSyncing,
}

impl Default for BlockSyncStatus {
    fn default() -> Self {
        Self::Idle
    }
}

/// Information about a sync operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncOperation {
    /// Operation ID
    pub id: String,
    /// Current status
    pub status: BlockSyncStatus,
    /// Target peer for sync
    pub target_peer: String,
    /// Starting height
    pub start_height: u64,
    /// Target height
    pub target_height: u64,
    /// Current height being synced
    pub current_height: u64,
    /// Number of blocks synced
    pub blocks_synced: u64,
    /// When operation started
    pub started_at: SystemTime,
    /// When operation completed (if applicable)
    pub completed_at: Option<SystemTime>,
    /// Errors encountered during sync
    pub errors: Vec<String>,
}

impl SyncOperation {
    /// Create a new sync operation
    pub fn new(target_peer: String, start_height: u64, target_height: u64) -> Self {
        let id = format!(
            "syncop-{}-{}",
            start_height,
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );

        Self {
            id,
            status: BlockSyncStatus::Syncing,
            target_peer,
            start_height,
            target_height,
            current_height: start_height,
            blocks_synced: 0,
            started_at: SystemTime::now(),
            completed_at: None,
            errors: Vec::new(),
        }
    }

    /// Calculate sync progress percentage
    pub fn progress_percentage(&self) -> u8 {
        if self.target_height <= self.start_height {
            return 100;
        }

        let total_blocks = self.target_height - self.start_height;
        let synced_blocks = self.current_height.saturating_sub(self.start_height);

        ((synced_blocks * 100) / total_blocks).min(100) as u8
    }
}

/// Metrics for block synchronization operations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BlockSyncMetrics {
    /// Total number of sync operations started
    pub sync_operations_started: u64,
    /// Total number of successful sync operations
    pub sync_operations_completed: u64,
    /// Total number of failed sync operations
    pub sync_operations_failed: u64,
    /// Total number of blocks synced
    pub total_blocks_synced: u64,
    /// Average sync speed in blocks per second
    pub avg_sync_speed_bps: f64,
    /// Current sync status
    pub current_status: BlockSyncStatus,
    /// Number of active sync operations
    pub active_sync_operations: usize,
    /// Number of cached blocks
    pub cached_blocks: usize,
    /// Network errors encountered
    pub network_errors: u64,
    /// Verification failures
    pub verification_failures: u64,
    /// Average request response time in milliseconds
    pub avg_response_time_ms: u64,
}

// ================================================================================================
// Block Synchronization Manager
// ================================================================================================

/// Comprehensive block synchronization manager
#[derive(Debug)]
pub struct BlockSyncManager {
    /// Configuration
    config: BlockSyncConfig,
    /// Current synchronization status
    sync_status: Arc<AsyncRwLock<BlockSyncStatus>>,
    /// Active sync operations
    active_operations: Arc<AsyncRwLock<HashMap<String, SyncOperation>>>,
    /// Block cache for synchronization
    block_cache: Arc<AsyncRwLock<HashMap<u64, MultiVMBlock>>>,
    /// Known peers and their heights
    peer_heights: Arc<AsyncRwLock<HashMap<String, u64>>>,
    /// Sync request queue
    request_queue: Arc<AsyncRwLock<VecDeque<SyncRequest>>>,
    /// Sync metrics
    metrics: Arc<AsyncRwLock<BlockSyncMetrics>>,
    /// Request channel for sending sync requests
    request_sender: Option<mpsc::UnboundedSender<SyncRequest>>,
    /// Response channel for receiving sync responses
    response_receiver: Option<Arc<AsyncRwLock<mpsc::UnboundedReceiver<SyncResponse>>>>,
}

impl BlockSyncManager {
    /// Create a new block synchronization manager
    pub async fn new(config: BlockSyncConfig) -> Result<Self, BlockSyncError> {
        config
            .validate()
            .map_err(BlockSyncError::InvalidBlockData)?;

        let (request_sender, _request_receiver) = mpsc::unbounded_channel();
        let (_response_sender, response_receiver) = mpsc::unbounded_channel();

        let manager = Self {
            config,
            sync_status: Arc::new(AsyncRwLock::new(BlockSyncStatus::Idle)),
            active_operations: Arc::new(AsyncRwLock::new(HashMap::new())),
            block_cache: Arc::new(AsyncRwLock::new(HashMap::new())),
            peer_heights: Arc::new(AsyncRwLock::new(HashMap::new())),
            request_queue: Arc::new(AsyncRwLock::new(VecDeque::new())),
            metrics: Arc::new(AsyncRwLock::new(BlockSyncMetrics::default())),
            request_sender: Some(request_sender),
            response_receiver: Some(Arc::new(AsyncRwLock::new(response_receiver))),
        };

        info!("Block synchronization manager initialized");
        Ok(manager)
    }

    /// Update peer height information
    pub async fn update_peer_height(&self, peer_id: String, height: u64) {
        let mut peers = self.peer_heights.write().await;
        peers.insert(peer_id.clone(), height);
        debug!("Updated peer {} height to {}", peer_id, height);
    }

    /// Check if synchronization is needed
    pub async fn needs_sync(&self, current_height: u64) -> bool {
        if !self.config.enabled {
            return false;
        }

        let peers = self.peer_heights.read().await;
        let max_peer_height = peers.values().max().copied().unwrap_or(0);

        max_peer_height > current_height + self.config.sync_threshold_height
    }

    /// Start synchronization process
    pub async fn start_sync(&self, current_height: u64) -> Result<String, BlockSyncError> {
        // Check if already syncing
        {
            let status = self.sync_status.read().await;
            if matches!(
                *status,
                BlockSyncStatus::Syncing | BlockSyncStatus::FastSyncing
            ) {
                return Err(BlockSyncError::SyncRequestFailed(
                    "Sync already in progress".to_string(),
                ));
            }
        }

        // Find best peer for sync
        let (target_peer, target_height) = self.find_best_sync_peer().await.ok_or_else(|| {
            BlockSyncError::PeerUnavailable("No suitable peers available".to_string())
        })?;

        if target_height <= current_height {
            return Err(BlockSyncError::SyncRequestFailed(
                "No sync needed".to_string(),
            ));
        }

        // Create sync operation
        let sync_op = SyncOperation::new(target_peer.clone(), current_height + 1, target_height);
        let operation_id = sync_op.id.clone();

        // Update status
        {
            let mut status = self.sync_status.write().await;
            *status = if target_height - current_height > 1000 {
                BlockSyncStatus::FastSyncing
            } else {
                BlockSyncStatus::Syncing
            };
        }

        // Store operation
        {
            let mut operations = self.active_operations.write().await;
            operations.insert(operation_id.clone(), sync_op);
        }

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.sync_operations_started += 1;
            metrics.active_sync_operations += 1;
            metrics.current_status = self.sync_status.read().await.clone();
        }

        info!(
            "Started sync operation {} with peer {} (height {} -> {})",
            operation_id, target_peer, current_height, target_height
        );

        // Start sync process
        self.execute_sync_operation(&operation_id).await?;

        Ok(operation_id)
    }

    /// Find the best peer for synchronization
    async fn find_best_sync_peer(&self) -> Option<(String, u64)> {
        let peers = self.peer_heights.read().await;

        // Find peer with highest height
        peers
            .iter()
            .max_by_key(|(_, height)| *height)
            .map(|(peer, height)| (peer.clone(), *height))
    }

    /// Execute synchronization operation
    async fn execute_sync_operation(&self, operation_id: &str) -> Result<(), BlockSyncError> {
        let start_time = Instant::now();

        loop {
            // Get current operation state
            let (target_peer, start_height, target_height, current_height) = {
                let operations = self.active_operations.read().await;
                let op = operations.get(operation_id).ok_or_else(|| {
                    BlockSyncError::SyncRequestFailed("Operation not found".to_string())
                })?;

                if op.current_height >= op.target_height {
                    break; // Sync completed
                }

                (
                    op.target_peer.clone(),
                    op.start_height,
                    op.target_height,
                    op.current_height,
                )
            };

            // Calculate blocks to request
            let remaining_blocks = target_height - current_height;
            let blocks_to_request =
                remaining_blocks.min(self.config.max_blocks_per_request as u64) as usize;

            // Create sync request
            let sync_request = SyncRequest::new(
                current_height,
                Some(current_height + blocks_to_request as u64 - 1),
                blocks_to_request,
            );

            // Send sync request
            match self.send_sync_request(&target_peer, sync_request).await {
                Ok(response) => {
                    // Process response
                    self.process_sync_response(operation_id, response).await?;
                }
                Err(e) => {
                    // Handle sync error
                    self.handle_sync_error(operation_id, e).await?;
                    break;
                }
            }

            // Add delay between requests
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Finalize operation
        self.finalize_sync_operation(operation_id, start_time.elapsed())
            .await?;

        Ok(())
    }

    /// Send synchronization request to peer
    async fn send_sync_request(
        &self,
        peer: &str,
        request: SyncRequest,
    ) -> Result<SyncResponse, BlockSyncError> {
        debug!(
            "Sending sync request to peer {}: {} blocks from height {}",
            peer, request.max_blocks, request.start_height
        );

        // Check if request has expired
        if request.is_expired(self.config.sync_request_timeout_ms) {
            return Err(BlockSyncError::SyncTimeout(
                "Request expired before sending".to_string(),
            ));
        }

        // Send request through request channel
        if let Some(sender) = &self.request_sender {
            if let Err(e) = sender.send(request.clone()) {
                return Err(BlockSyncError::NetworkError(format!(
                    "Failed to send request: {}",
                    e
                )));
            }
        } else {
            return Err(BlockSyncError::NetworkError(
                "Request sender not available".to_string(),
            ));
        }

        // Wait for response with timeout
        let timeout_duration = Duration::from_millis(self.config.sync_request_timeout_ms);
        let response_result =
            tokio::time::timeout(timeout_duration, self.wait_for_response(&request.id)).await;

        match response_result {
            Ok(Ok(response)) => {
                // Validate response
                if response.request_id != request.id {
                    return Err(BlockSyncError::InvalidBlockData(
                        "Response ID mismatch".to_string(),
                    ));
                }

                // Validate blocks are in expected range
                for block in &response.blocks {
                    if block.header.height < request.start_height {
                        return Err(BlockSyncError::InvalidBlockData(format!(
                            "Block height {} below requested start {}",
                            block.header.height, request.start_height
                        )));
                    }
                    if let Some(end_height) = request.end_height {
                        if block.header.height > end_height {
                            return Err(BlockSyncError::InvalidBlockData(format!(
                                "Block height {} above requested end {}",
                                block.header.height, end_height
                            )));
                        }
                    }
                }

                Ok(response)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(BlockSyncError::SyncTimeout(format!(
                "Timeout waiting for sync response from peer {}",
                peer
            ))),
        }
    }

    /// Wait for response to a specific request
    async fn wait_for_response(&self, request_id: &str) -> Result<SyncResponse, BlockSyncError> {
        // This would typically be implemented with a response handler that matches responses to requests
        // For production, integrate with P2P network layer to receive actual responses

        // Placeholder: In production, this would receive from response_receiver channel
        Err(BlockSyncError::NetworkError(
            "Response handler not implemented".to_string(),
        ))
    }

    /// Process synchronization response
    async fn process_sync_response(
        &self,
        operation_id: &str,
        response: SyncResponse,
    ) -> Result<(), BlockSyncError> {
        // Verify blocks if enabled
        if self.config.verify_blocks_during_sync {
            for block in &response.blocks {
                if let Err(e) = self.verify_sync_block(block).await {
                    return Err(BlockSyncError::VerificationFailed(format!(
                        "Block verification failed: {}",
                        e
                    )));
                }
            }
        }

        // Cache blocks
        {
            let mut cache = self.block_cache.write().await;
            for block in &response.blocks {
                cache.insert(block.header.height, block.clone());

                // Prune cache if too large
                while cache.len() > self.config.max_cached_blocks {
                    if let Some(min_height) = cache.keys().min().copied() {
                        cache.remove(&min_height);
                    }
                }
            }
        }

        // Update operation progress
        {
            let mut operations = self.active_operations.write().await;
            if let Some(op) = operations.get_mut(operation_id) {
                if let Some(last_block) = response.blocks.last() {
                    op.current_height = last_block.header.height + 1;
                    op.blocks_synced += response.blocks.len() as u64;
                }
            }
        }

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_blocks_synced += response.blocks.len() as u64;
            metrics.cached_blocks = self.block_cache.read().await.len();
        }

        debug!("Processed sync response: {} blocks", response.blocks.len());
        Ok(())
    }

    /// Verify a block during synchronization
    async fn verify_sync_block(&self, block: &MultiVMBlock) -> Result<(), BlockSyncError> {
        // Basic block validation
        if block.header.height == 0 {
            return Err(BlockSyncError::InvalidBlockData(
                "Invalid block height".to_string(),
            ));
        }

        if block.header.proposer.is_empty() {
            return Err(BlockSyncError::InvalidBlockData(
                "Invalid proposer".to_string(),
            ));
        }

        // Additional validation would go here
        Ok(())
    }

    /// Handle synchronization error
    async fn handle_sync_error(
        &self,
        operation_id: &str,
        error: BlockSyncError,
    ) -> Result<(), BlockSyncError> {
        error!("Sync error in operation {}: {}", operation_id, error);

        // Update operation with error
        {
            let mut operations = self.active_operations.write().await;
            if let Some(op) = operations.get_mut(operation_id) {
                op.errors.push(error.to_string());
                op.status = BlockSyncStatus::Failed;
                op.completed_at = Some(SystemTime::now());
            }
        }

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            match error.category() {
                "network_error" => metrics.network_errors += 1,
                "verification_failed" => metrics.verification_failures += 1,
                _ => {}
            }
        }

        Err(error)
    }

    /// Finalize synchronization operation
    async fn finalize_sync_operation(
        &self,
        operation_id: &str,
        duration: Duration,
    ) -> Result<(), BlockSyncError> {
        let blocks_synced = {
            let mut operations = self.active_operations.write().await;
            if let Some(op) = operations.get_mut(operation_id) {
                op.status = BlockSyncStatus::Completed;
                op.completed_at = Some(SystemTime::now());
                op.blocks_synced
            } else {
                return Err(BlockSyncError::SyncRequestFailed(
                    "Operation not found".to_string(),
                ));
            }
        };

        // Update status
        {
            let mut status = self.sync_status.write().await;
            *status = BlockSyncStatus::Completed;
        }

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.sync_operations_completed += 1;
            metrics.active_sync_operations = metrics.active_sync_operations.saturating_sub(1);
            metrics.current_status = BlockSyncStatus::Completed;

            // Calculate sync speed
            if duration.as_secs() > 0 {
                let speed = blocks_synced as f64 / duration.as_secs_f64();
                metrics.avg_sync_speed_bps = (metrics.avg_sync_speed_bps + speed) / 2.0;
            }
        }

        info!(
            "Sync operation {} completed: {} blocks in {:?}",
            operation_id, blocks_synced, duration
        );
        Ok(())
    }

    /// Get blocks from cache
    pub async fn get_cached_blocks(&self, start_height: u64, count: usize) -> Vec<MultiVMBlock> {
        let cache = self.block_cache.read().await;
        let mut blocks = Vec::new();

        for height in start_height..start_height + count as u64 {
            if let Some(block) = cache.get(&height) {
                blocks.push(block.clone());
            } else {
                break; // Missing block, return what we have
            }
        }

        blocks
    }

    /// Get current synchronization status
    pub async fn get_sync_status(&self) -> BlockSyncStatus {
        self.sync_status.read().await.clone()
    }

    /// Get active sync operations
    pub async fn get_active_operations(&self) -> Vec<SyncOperation> {
        self.active_operations
            .read()
            .await
            .values()
            .cloned()
            .collect()
    }

    /// Get synchronization metrics
    pub async fn get_metrics(&self) -> BlockSyncMetrics {
        let mut metrics = self.metrics.read().await.clone();
        metrics.cached_blocks = self.block_cache.read().await.len();
        metrics.active_sync_operations = self.active_operations.read().await.len();
        metrics
    }

    /// Clean up completed operations and old cache entries
    pub async fn cleanup(&self) {
        // Remove completed operations older than 1 hour
        {
            let mut operations = self.active_operations.write().await;
            let cutoff_time = SystemTime::now() - Duration::from_secs(3600);

            operations.retain(|_, op| {
                if let Some(completed_at) = op.completed_at {
                    completed_at > cutoff_time
                } else {
                    true // Keep active operations
                }
            });
        }

        // Prune old cache entries
        {
            let mut cache = self.block_cache.write().await;
            while cache.len() > self.config.max_cached_blocks {
                if let Some(min_height) = cache.keys().min().copied() {
                    cache.remove(&min_height);
                }
            }
        }

        debug!("Sync manager cleanup completed");
    }
}

/// Trait for block synchronization integration
#[async_trait]
pub trait BlockSynchronizer: Send + Sync {
    /// Check if synchronization is needed
    async fn needs_sync(&self, current_height: u64) -> bool;

    /// Start block synchronization
    async fn start_sync(&self, current_height: u64) -> Result<String, BlockSyncError>;

    /// Get cached blocks
    async fn get_cached_blocks(&self, start_height: u64, count: usize) -> Vec<MultiVMBlock>;

    /// Get sync status
    async fn get_sync_status(&self) -> BlockSyncStatus;

    /// Get sync metrics
    async fn get_metrics(&self) -> BlockSyncMetrics;
}

#[async_trait]
impl BlockSynchronizer for BlockSyncManager {
    async fn needs_sync(&self, current_height: u64) -> bool {
        self.needs_sync(current_height).await
    }

    async fn start_sync(&self, current_height: u64) -> Result<String, BlockSyncError> {
        self.start_sync(current_height).await
    }

    async fn get_cached_blocks(&self, start_height: u64, count: usize) -> Vec<MultiVMBlock> {
        self.get_cached_blocks(start_height, count).await
    }

    async fn get_sync_status(&self) -> BlockSyncStatus {
        self.get_sync_status().await
    }

    async fn get_metrics(&self) -> BlockSyncMetrics {
        self.get_metrics().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_block_sync_manager_creation() {
        let config = BlockSyncConfig::default();
        let manager = BlockSyncManager::new(config).await.unwrap();

        let status = manager.get_sync_status().await;
        assert_eq!(status, BlockSyncStatus::Idle);
    }

    #[tokio::test]
    async fn test_sync_request_creation() {
        let request = SyncRequest::new(100, Some(200), 50);
        assert_eq!(request.start_height, 100);
        assert_eq!(request.end_height, Some(200));
        assert_eq!(request.max_blocks, 50);
        assert!(request.id.starts_with("sync-100-"));
    }

    #[tokio::test]
    async fn test_sync_operation_progress() {
        let mut op = SyncOperation::new("peer1".to_string(), 100, 200);
        assert_eq!(op.progress_percentage(), 0);

        op.current_height = 150;
        assert_eq!(op.progress_percentage(), 50);

        op.current_height = 200;
        assert_eq!(op.progress_percentage(), 100);
    }

    #[tokio::test]
    async fn test_needs_sync_detection() {
        let config = BlockSyncConfig::default();
        let manager = BlockSyncManager::new(config).await.unwrap();

        // No peers - no sync needed
        assert!(!manager.needs_sync(100).await);

        // Add peer with higher height
        manager.update_peer_height("peer1".to_string(), 150).await;
        assert!(manager.needs_sync(100).await);

        // Peer height not significantly higher
        assert!(!manager.needs_sync(145).await);
    }

    #[tokio::test]
    async fn test_config_validation() {
        let mut config = BlockSyncConfig::default();
        assert!(config.validate().is_ok());

        config.max_blocks_per_request = 0;
        assert!(config.validate().is_err());
    }
}
