//! Fork detection and resolution for MultiVM consensus
//!
//! This module provides comprehensive fork detection capabilities for the MultiVM
//! consensus system, following the established design patterns of the codebase.

use crate::block::MultiVMBlock;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock as AsyncRwLock;
use tracing::{debug, error, info, warn};

// ================================================================================================
// Fork Detection Types and Configuration
// ================================================================================================

/// Configuration for fork detection system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkDetectionConfig {
    /// Enable fork detection
    pub enabled: bool,
    /// Maximum number of concurrent forks to track
    pub max_tracked_forks: usize,
    /// Height difference threshold for fork detection
    pub fork_height_threshold: u64,
    /// Time window for fork detection in seconds
    pub detection_window_secs: u64,
    /// Number of confirmations required to resolve a fork
    pub resolution_confirmations: u32,
    /// Maximum time to wait for fork resolution in seconds
    pub resolution_timeout_secs: u64,
    /// Enable automatic fork resolution
    pub auto_resolution: bool,
    /// Minimum validator agreement percentage for resolution (0-100)
    pub min_validator_agreement: u8,
}

impl Default for ForkDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_tracked_forks: 10,
            fork_height_threshold: 1,
            detection_window_secs: 300, // 5 minutes
            resolution_confirmations: 3,
            resolution_timeout_secs: 600, // 10 minutes
            auto_resolution: true,
            min_validator_agreement: 67, // 2/3 majority
        }
    }
}

impl ForkDetectionConfig {
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.max_tracked_forks == 0 {
            return Err("Max tracked forks must be greater than 0".to_string());
        }
        if self.fork_height_threshold == 0 {
            return Err("Fork height threshold must be greater than 0".to_string());
        }
        if self.detection_window_secs == 0 {
            return Err("Detection window must be greater than 0".to_string());
        }
        if self.resolution_confirmations == 0 {
            return Err("Resolution confirmations must be greater than 0".to_string());
        }
        if self.min_validator_agreement > 100 {
            return Err("Validator agreement percentage must be <= 100".to_string());
        }
        Ok(())
    }
}

/// Error types for fork detection operations
#[derive(Debug, thiserror::Error)]
pub enum ForkDetectionError {
    #[error("Fork detected at height {height}: {description}")]
    ForkDetected { height: u64, description: String },
    #[error("Fork resolution failed: {0}")]
    ResolutionFailed(String),
    #[error("Fork resolution timeout: {0}")]
    ResolutionTimeout(String),
    #[error("Invalid fork data: {0}")]
    InvalidForkData(String),
    #[error("Consensus disagreement: {0}")]
    ConsensusDisagreement(String),
    #[error("Network partition detected: {0}")]
    NetworkPartition(String),
}

impl ForkDetectionError {
    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::ForkDetected { .. } => true,
            Self::ResolutionFailed(_) => true,
            Self::ResolutionTimeout(_) => true,
            Self::InvalidForkData(_) => false,
            Self::ConsensusDisagreement(_) => true,
            Self::NetworkPartition(_) => true,
        }
    }

    /// Check if this error is critical
    pub fn is_critical(&self) -> bool {
        matches!(
            self,
            Self::ForkDetected { .. } | Self::ResolutionFailed(_) | Self::NetworkPartition(_)
        )
    }

    /// Get error category for metrics
    pub fn category(&self) -> &'static str {
        match self {
            Self::ForkDetected { .. } => "fork_detected",
            Self::ResolutionFailed(_) => "resolution_failed",
            Self::ResolutionTimeout(_) => "resolution_timeout",
            Self::InvalidForkData(_) => "invalid_data",
            Self::ConsensusDisagreement(_) => "consensus_disagreement",
            Self::NetworkPartition(_) => "network_partition",
        }
    }
}

/// Information about a detected fork
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkInfo {
    /// Unique fork identifier
    pub id: String,
    /// Height at which the fork occurred
    pub fork_height: u64,
    /// Timestamp when fork was detected
    pub detected_at: SystemTime,
    /// Alternative blocks at the fork height
    pub alternative_blocks: Vec<MultiVMBlock>,
    /// Validators supporting each alternative
    pub validator_support: HashMap<String, HashSet<String>>,
    /// Current resolution status
    pub status: ForkStatus,
    /// Reason for the fork
    pub reason: ForkReason,
    /// Resolution metadata
    pub resolution_metadata: serde_json::Value,
}

impl ForkInfo {
    /// Create a new fork info
    pub fn new(fork_height: u64, reason: ForkReason) -> Self {
        let id = format!(
            "fork-{}-{}",
            fork_height,
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );

        Self {
            id,
            fork_height,
            detected_at: SystemTime::now(),
            alternative_blocks: Vec::new(),
            validator_support: HashMap::new(),
            status: ForkStatus::Detected,
            reason,
            resolution_metadata: serde_json::Value::Null,
        }
    }

    /// Add an alternative block to this fork
    pub fn add_alternative_block(
        &mut self,
        block: MultiVMBlock,
        supporting_validators: HashSet<String>,
    ) {
        let block_hash = block.calculate_hash();
        self.alternative_blocks.push(block);
        self.validator_support
            .insert(block_hash, supporting_validators);
    }

    /// Get the most supported block
    pub fn get_preferred_block(&self) -> Option<&MultiVMBlock> {
        let mut max_support = 0;
        let mut preferred_block = None;

        for (i, block) in self.alternative_blocks.iter().enumerate() {
            let block_hash = block.calculate_hash();
            if let Some(supporters) = self.validator_support.get(&block_hash) {
                if supporters.len() > max_support {
                    max_support = supporters.len();
                    preferred_block = Some((i, block));
                }
            }
        }

        preferred_block.map(|(_, block)| block)
    }

    /// Calculate validator agreement percentage for the preferred block
    pub fn get_validator_agreement_percentage(&self, total_validators: usize) -> u8 {
        if total_validators == 0 {
            return 0;
        }

        let max_support = self
            .validator_support
            .values()
            .map(|supporters| supporters.len())
            .max()
            .unwrap_or(0);

        ((max_support * 100) / total_validators) as u8
    }
}

/// Status of fork resolution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ForkStatus {
    /// Fork has been detected
    Detected,
    /// Fork resolution is in progress
    Resolving,
    /// Fork has been resolved
    Resolved,
    /// Fork resolution failed
    Failed,
    /// Fork resolution timed out
    TimedOut,
}

/// Reason for fork occurrence
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ForkReason {
    /// Network partition caused the fork
    NetworkPartition,
    /// Validator disagreement
    ValidatorDisagreement,
    /// Concurrent block proposals
    ConcurrentProposals,
    /// Byzantine behavior detected
    ByzantineBehavior { validator: String },
    /// Clock synchronization issues
    ClockSkew { time_diff_ms: i64 },
    /// Unknown reason
    Unknown,
}

/// Fork resolution strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ForkResolutionStrategy {
    /// Use the block with most validator support
    MajorityVote,
    /// Use the block with the lowest hash (deterministic)
    LowestHash,
    /// Use the block from the validator with highest stake
    HighestStake,
    /// Manual resolution required
    Manual,
}

/// Metrics for fork detection operations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ForkDetectionMetrics {
    /// Total number of forks detected
    pub forks_detected: u64,
    /// Total number of forks resolved
    pub forks_resolved: u64,
    /// Total number of fork resolution failures
    pub resolution_failures: u64,
    /// Total number of fork resolution timeouts
    pub resolution_timeouts: u64,
    /// Average fork resolution time in milliseconds
    pub avg_resolution_time_ms: u64,
    /// Maximum concurrent forks tracked
    pub max_concurrent_forks: usize,
    /// Current number of active forks
    pub active_forks: usize,
    /// Network partition detections
    pub network_partitions: u64,
    /// Byzantine behavior detections
    pub byzantine_detections: u64,
    /// Last detection timestamp
    pub last_detection: Option<SystemTime>,
}

// ================================================================================================
// Fork Detection and Resolution System
// ================================================================================================

/// Comprehensive fork detection and resolution manager
#[derive(Debug)]
pub struct ForkDetectionManager {
    /// Configuration
    config: ForkDetectionConfig,
    /// Currently tracked forks
    active_forks: Arc<AsyncRwLock<HashMap<String, ForkInfo>>>,
    /// Fork history for analysis
    fork_history: Arc<AsyncRwLock<VecDeque<ForkInfo>>>,
    /// Block cache for comparison
    block_cache: Arc<AsyncRwLock<HashMap<u64, Vec<MultiVMBlock>>>>,
    /// Validator set information
    validators: Arc<AsyncRwLock<HashSet<String>>>,
    /// Fork detection metrics
    metrics: Arc<AsyncRwLock<ForkDetectionMetrics>>,
    /// Resolution strategies by fork type
    resolution_strategies: Arc<AsyncRwLock<HashMap<ForkReason, ForkResolutionStrategy>>>,
}

impl ForkDetectionManager {
    /// Create a new fork detection manager
    pub async fn new(config: ForkDetectionConfig) -> Result<Self, ForkDetectionError> {
        config
            .validate()
            .map_err(ForkDetectionError::InvalidForkData)?;

        let mut strategies = HashMap::new();
        strategies.insert(
            ForkReason::NetworkPartition,
            ForkResolutionStrategy::MajorityVote,
        );
        strategies.insert(
            ForkReason::ValidatorDisagreement,
            ForkResolutionStrategy::MajorityVote,
        );
        strategies.insert(
            ForkReason::ConcurrentProposals,
            ForkResolutionStrategy::LowestHash,
        );
        strategies.insert(
            ForkReason::ByzantineBehavior {
                validator: String::new(),
            },
            ForkResolutionStrategy::MajorityVote,
        );
        strategies.insert(
            ForkReason::ClockSkew { time_diff_ms: 0 },
            ForkResolutionStrategy::LowestHash,
        );
        strategies.insert(ForkReason::Unknown, ForkResolutionStrategy::MajorityVote);

        let manager = Self {
            config,
            active_forks: Arc::new(AsyncRwLock::new(HashMap::new())),
            fork_history: Arc::new(AsyncRwLock::new(VecDeque::new())),
            block_cache: Arc::new(AsyncRwLock::new(HashMap::new())),
            validators: Arc::new(AsyncRwLock::new(HashSet::new())),
            metrics: Arc::new(AsyncRwLock::new(ForkDetectionMetrics::default())),
            resolution_strategies: Arc::new(AsyncRwLock::new(strategies)),
        };

        info!("Fork detection manager initialized");
        Ok(manager)
    }

    /// Update the validator set
    pub async fn update_validators(&self, validators: HashSet<String>) {
        *self.validators.write().await = validators;
        debug!(
            "Updated validator set with {} validators",
            self.validators.read().await.len()
        );
    }

    /// Process a new block and check for forks
    pub async fn process_block(
        &self,
        block: MultiVMBlock,
        validator: String,
    ) -> Result<Option<ForkInfo>, ForkDetectionError> {
        if !self.config.enabled {
            return Ok(None);
        }

        let height = block.header.height;

        // Add block to cache
        {
            let mut cache = self.block_cache.write().await;
            cache
                .entry(height)
                .or_insert_with(Vec::new)
                .push(block.clone());

            // Prune old blocks (keep only recent heights)
            let cutoff_height = height.saturating_sub(100);
            cache.retain(|&h, _| h >= cutoff_height);
        }

        // Check for fork at this height
        let fork_detected = self.detect_fork_at_height(height).await?;

        if let Some(mut fork_info) = fork_detected {
            // Add this block as an alternative
            let supporting_validators = HashSet::from([validator]);
            fork_info.add_alternative_block(block, supporting_validators);

            // Store the fork
            {
                let mut forks = self.active_forks.write().await;
                let fork_id = fork_info.id.clone();
                forks.insert(fork_id.clone(), fork_info.clone());

                // Update metrics
                let mut metrics = self.metrics.write().await;
                metrics.forks_detected += 1;
                metrics.active_forks = forks.len();
                metrics.last_detection = Some(SystemTime::now());

                if forks.len() > metrics.max_concurrent_forks {
                    metrics.max_concurrent_forks = forks.len();
                }
            }

            warn!(
                "Fork detected at height {} with ID: {}",
                height, fork_info.id
            );

            // Attempt automatic resolution if enabled
            if self.config.auto_resolution {
                if let Err(e) = self.attempt_resolution(&fork_info.id).await {
                    error!("Automatic fork resolution failed: {}", e);
                }
            }

            Ok(Some(fork_info))
        } else {
            Ok(None)
        }
    }

    /// Detect if there's a fork at the specified height
    async fn detect_fork_at_height(
        &self,
        height: u64,
    ) -> Result<Option<ForkInfo>, ForkDetectionError> {
        let cache = self.block_cache.read().await;

        if let Some(blocks) = cache.get(&height) {
            if blocks.len() > 1 {
                // Multiple blocks at same height - potential fork
                let reason = self.analyze_fork_reason(blocks).await;
                let fork_info = ForkInfo::new(height, reason);

                // Check if this is a genuine fork (not just network delay)
                if self.is_genuine_fork(blocks).await {
                    return Ok(Some(fork_info));
                }
            }
        }

        Ok(None)
    }

    /// Analyze the reason for a fork
    async fn analyze_fork_reason(&self, blocks: &[MultiVMBlock]) -> ForkReason {
        if blocks.len() < 2 {
            return ForkReason::Unknown;
        }

        // Check for clock skew
        let timestamps: Vec<_> = blocks.iter().map(|b| b.header.timestamp).collect();
        if let (Some(min_time), Some(max_time)) = (timestamps.iter().min(), timestamps.iter().max())
        {
            if let (Ok(min_duration), Ok(max_duration)) = (
                min_time.duration_since(SystemTime::UNIX_EPOCH),
                max_time.duration_since(SystemTime::UNIX_EPOCH),
            ) {
                let time_diff = max_duration.as_millis() as i64 - min_duration.as_millis() as i64;
                if time_diff.abs() > 10000 {
                    // 10 second threshold
                    return ForkReason::ClockSkew {
                        time_diff_ms: time_diff,
                    };
                }
            }
        }

        // Check for concurrent proposals (similar timestamps)
        let proposers: HashSet<_> = blocks.iter().map(|b| &b.header.proposer).collect();
        if proposers.len() == blocks.len() {
            return ForkReason::ConcurrentProposals;
        }

        // Default to validator disagreement
        ForkReason::ValidatorDisagreement
    }

    /// Determine if this is a genuine fork or just network delay
    async fn is_genuine_fork(&self, blocks: &[MultiVMBlock]) -> bool {
        // Check if blocks have different parent hashes (indicating actual fork)
        let parent_hashes: HashSet<_> = blocks.iter().map(|b| &b.header.previous_hash).collect();
        if parent_hashes.len() > 1 {
            return true;
        }

        // Check for significant differences in block content
        for i in 0..blocks.len() {
            for j in (i + 1)..blocks.len() {
                if self
                    .blocks_significantly_different(&blocks[i], &blocks[j])
                    .await
                {
                    return true;
                }
            }
        }

        false
    }

    /// Check if two blocks are significantly different
    async fn blocks_significantly_different(
        &self,
        block1: &MultiVMBlock,
        block2: &MultiVMBlock,
    ) -> bool {
        // Different proposers
        if block1.header.proposer != block2.header.proposer {
            return true;
        }

        // Different transaction counts
        let tx_count1 = block1.svm_transactions.len()
            + block1.evm_transactions.len()
            + block1.multivm_transactions.len();
        let tx_count2 = block2.svm_transactions.len()
            + block2.evm_transactions.len()
            + block2.multivm_transactions.len();

        if tx_count1 != tx_count2 {
            return true;
        }

        // Different state transitions
        if block1.state_transitions != block2.state_transitions {
            return true;
        }

        false
    }

    /// Attempt to resolve a fork
    pub async fn attempt_resolution(&self, fork_id: &str) -> Result<(), ForkDetectionError> {
        let mut fork_info = {
            let forks = self.active_forks.read().await;
            forks
                .get(fork_id)
                .ok_or_else(|| {
                    ForkDetectionError::InvalidForkData(format!("Fork {fork_id} not found"))
                })?
                .clone()
        };

        if fork_info.status != ForkStatus::Detected {
            return Ok(()); // Already being resolved or resolved
        }

        // Update status to resolving
        fork_info.status = ForkStatus::Resolving;
        {
            let mut forks = self.active_forks.write().await;
            forks.insert(fork_id.to_string(), fork_info.clone());
        }

        let start_time = Instant::now();

        // Get resolution strategy
        let strategy = {
            let strategies = self.resolution_strategies.read().await;
            strategies
                .get(&fork_info.reason)
                .cloned()
                .unwrap_or(ForkResolutionStrategy::MajorityVote)
        };

        // Attempt resolution based on strategy
        let resolution_result = match strategy {
            ForkResolutionStrategy::MajorityVote => self.resolve_by_majority_vote(&fork_info).await,
            ForkResolutionStrategy::LowestHash => self.resolve_by_lowest_hash(&fork_info).await,
            ForkResolutionStrategy::HighestStake => self.resolve_by_highest_stake(&fork_info).await,
            ForkResolutionStrategy::Manual => Err(ForkDetectionError::ResolutionFailed(
                "Manual resolution required".to_string(),
            )),
        };

        let resolution_time = start_time.elapsed();

        // Update fork status and metrics
        match resolution_result {
            Ok(_) => {
                fork_info.status = ForkStatus::Resolved;

                // Move to history
                {
                    let mut forks = self.active_forks.write().await;
                    forks.remove(fork_id);

                    let mut history = self.fork_history.write().await;
                    history.push_back(fork_info);

                    // Prune old history
                    while history.len() > 100 {
                        history.pop_front();
                    }
                }

                // Update metrics
                {
                    let mut metrics = self.metrics.write().await;
                    metrics.forks_resolved += 1;
                    metrics.avg_resolution_time_ms =
                        (metrics.avg_resolution_time_ms + resolution_time.as_millis() as u64) / 2;
                    metrics.active_forks = self.active_forks.read().await.len();
                }

                info!(
                    "Fork {} resolved successfully in {:?}",
                    fork_id, resolution_time
                );
                Ok(())
            }
            Err(e) => {
                fork_info.status = ForkStatus::Failed;

                // Update fork status
                {
                    let mut forks = self.active_forks.write().await;
                    forks.insert(fork_id.to_string(), fork_info);
                }

                // Update metrics
                {
                    let mut metrics = self.metrics.write().await;
                    metrics.resolution_failures += 1;
                }

                error!("Fork {} resolution failed: {}", fork_id, e);
                Err(e)
            }
        }
    }

    /// Resolve fork by majority vote
    async fn resolve_by_majority_vote(
        &self,
        fork_info: &ForkInfo,
    ) -> Result<MultiVMBlock, ForkDetectionError> {
        let total_validators = self.validators.read().await.len();
        if total_validators == 0 {
            return Err(ForkDetectionError::ResolutionFailed(
                "No validators available".to_string(),
            ));
        }

        let agreement_percentage = fork_info.get_validator_agreement_percentage(total_validators);
        if agreement_percentage < self.config.min_validator_agreement {
            return Err(ForkDetectionError::ConsensusDisagreement(format!(
                "Insufficient validator agreement: {}% < {}%",
                agreement_percentage, self.config.min_validator_agreement
            )));
        }

        if let Some(preferred_block) = fork_info.get_preferred_block() {
            Ok(preferred_block.clone())
        } else {
            Err(ForkDetectionError::ResolutionFailed(
                "No preferred block found".to_string(),
            ))
        }
    }

    /// Resolve fork by lowest hash (deterministic)
    async fn resolve_by_lowest_hash(
        &self,
        fork_info: &ForkInfo,
    ) -> Result<MultiVMBlock, ForkDetectionError> {
        if fork_info.alternative_blocks.is_empty() {
            return Err(ForkDetectionError::ResolutionFailed(
                "No alternative blocks available".to_string(),
            ));
        }

        let mut lowest_hash = String::new();
        let mut chosen_block = None;

        for block in &fork_info.alternative_blocks {
            let hash = block.calculate_hash();
            if lowest_hash.is_empty() || hash < lowest_hash {
                lowest_hash = hash;
                chosen_block = Some(block);
            }
        }

        if let Some(block) = chosen_block {
            Ok(block.clone())
        } else {
            Err(ForkDetectionError::ResolutionFailed(
                "Failed to select block by hash".to_string(),
            ))
        }
    }

    /// Resolve fork by highest stake (placeholder implementation)
    async fn resolve_by_highest_stake(
        &self,
        fork_info: &ForkInfo,
    ) -> Result<MultiVMBlock, ForkDetectionError> {
        // This would integrate with validator stake information
        // For now, fallback to majority vote
        self.resolve_by_majority_vote(fork_info).await
    }

    /// Get current fork detection metrics
    pub async fn get_metrics(&self) -> ForkDetectionMetrics {
        let mut metrics = self.metrics.read().await.clone();
        metrics.active_forks = self.active_forks.read().await.len();
        metrics
    }

    /// Get information about active forks
    pub async fn get_active_forks(&self) -> Vec<ForkInfo> {
        self.active_forks.read().await.values().cloned().collect()
    }

    /// Get fork history
    pub async fn get_fork_history(&self) -> Vec<ForkInfo> {
        self.fork_history.read().await.iter().cloned().collect()
    }

    /// Check for network partitions
    pub async fn detect_network_partition(&self) -> Result<bool, ForkDetectionError> {
        let active_forks = self.active_forks.read().await;

        // Simple heuristic: if we have multiple active forks, it might indicate network partition
        if active_forks.len() > self.config.max_tracked_forks / 2 {
            let mut metrics = self.metrics.write().await;
            metrics.network_partitions += 1;

            warn!(
                "Potential network partition detected: {} active forks",
                active_forks.len()
            );
            return Ok(true);
        }

        Ok(false)
    }

    /// Clean up old forks and resolved forks
    pub async fn cleanup(&self) {
        let cutoff_time =
            SystemTime::now() - Duration::from_secs(self.config.detection_window_secs);

        // Remove old resolved forks
        {
            let mut forks = self.active_forks.write().await;
            forks.retain(|_, fork| {
                fork.status == ForkStatus::Detected
                    || fork.status == ForkStatus::Resolving
                    || fork.detected_at > cutoff_time
            });
        }

        // Prune fork history
        {
            let mut history = self.fork_history.write().await;
            while history.len() > 100 {
                history.pop_front();
            }
        }

        debug!("Fork detection cleanup completed");
    }
}

/// Trait for fork detection integration with consensus
#[async_trait]
pub trait ForkDetector: Send + Sync {
    /// Process a new block for fork detection
    async fn process_block(
        &self,
        block: MultiVMBlock,
        validator: String,
    ) -> Result<Option<ForkInfo>, ForkDetectionError>;

    /// Check if a fork exists at the given height
    async fn has_fork_at_height(&self, height: u64) -> bool;

    /// Get the preferred block for a given height (post-resolution)
    async fn get_preferred_block(&self, height: u64) -> Option<MultiVMBlock>;

    /// Get fork detection metrics
    async fn get_metrics(&self) -> ForkDetectionMetrics;
}

#[async_trait]
impl ForkDetector for ForkDetectionManager {
    async fn process_block(
        &self,
        block: MultiVMBlock,
        validator: String,
    ) -> Result<Option<ForkInfo>, ForkDetectionError> {
        self.process_block(block, validator).await
    }

    async fn has_fork_at_height(&self, height: u64) -> bool {
        let forks = self.active_forks.read().await;
        forks.values().any(|fork| fork.fork_height == height)
    }

    async fn get_preferred_block(&self, height: u64) -> Option<MultiVMBlock> {
        let forks = self.active_forks.read().await;
        for fork in forks.values() {
            if fork.fork_height == height && fork.status == ForkStatus::Resolved {
                return fork.get_preferred_block().cloned();
            }
        }
        None
    }

    async fn get_metrics(&self) -> ForkDetectionMetrics {
        self.get_metrics().await
    }
}
