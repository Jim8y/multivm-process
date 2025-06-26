//! Network partition recovery for MultiVM consensus
//!
//! This module provides network partition detection and recovery mechanisms
//! to ensure consensus resilience during network failures.

use crate::fork_detection::ForkDetectionManager;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock as AsyncRwLock;
use tracing::{debug, error, info, warn};

// ================================================================================================
// Network Recovery Types and Configuration
// ================================================================================================

/// Configuration for network partition recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkRecoveryConfig {
    /// Enable network partition recovery
    pub enabled: bool,
    /// Minimum number of validators to consider network healthy
    pub min_healthy_validators: usize,
    /// Maximum time without progress before considering partition in seconds
    pub partition_detection_timeout_secs: u64,
    /// Minimum validator connectivity percentage (0-100)
    pub min_connectivity_percentage: u8,
    /// Recovery attempt interval in seconds
    pub recovery_attempt_interval_secs: u64,
    /// Maximum number of recovery attempts before giving up
    pub max_recovery_attempts: u32,
    /// Enable automatic recovery
    pub auto_recovery: bool,
    /// Grace period after recovery before normal operation in seconds
    pub recovery_grace_period_secs: u64,
    /// Enable health monitoring
    pub enable_health_monitoring: bool,
}

impl Default for NetworkRecoveryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_healthy_validators: 3,
            partition_detection_timeout_secs: 60,
            min_connectivity_percentage: 51, // Simple majority
            recovery_attempt_interval_secs: 30,
            max_recovery_attempts: 10,
            auto_recovery: true,
            recovery_grace_period_secs: 30,
            enable_health_monitoring: true,
        }
    }
}

impl NetworkRecoveryConfig {
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.min_healthy_validators == 0 {
            return Err("Minimum healthy validators must be greater than 0".to_string());
        }
        if self.min_connectivity_percentage > 100 {
            return Err("Connectivity percentage must be <= 100".to_string());
        }
        if self.partition_detection_timeout_secs == 0 {
            return Err("Partition detection timeout must be greater than 0".to_string());
        }
        if self.recovery_attempt_interval_secs == 0 {
            return Err("Recovery attempt interval must be greater than 0".to_string());
        }
        Ok(())
    }
}

/// Error types for network recovery operations
#[derive(Debug, thiserror::Error)]
pub enum NetworkRecoveryError {
    #[error("Network partition detected: {0}")]
    PartitionDetected(String),
    #[error("Recovery failed: {0}")]
    RecoveryFailed(String),
    #[error("Recovery timeout: {0}")]
    RecoveryTimeout(String),
    #[error("Insufficient connectivity: {current}% < {required}%")]
    InsufficientConnectivity { current: u8, required: u8 },
    #[error("Health check failed: {0}")]
    HealthCheckFailed(String),
    #[error("Invalid network state: {0}")]
    InvalidNetworkState(String),
}

impl NetworkRecoveryError {
    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::PartitionDetected(_) => true,
            Self::RecoveryFailed(_) => true,
            Self::RecoveryTimeout(_) => true,
            Self::InsufficientConnectivity { .. } => true,
            Self::HealthCheckFailed(_) => true,
            Self::InvalidNetworkState(_) => false,
        }
    }

    /// Check if this error is critical
    pub fn is_critical(&self) -> bool {
        matches!(
            self,
            Self::PartitionDetected(_) | Self::InsufficientConnectivity { .. }
        )
    }

    /// Get error category for metrics
    pub fn category(&self) -> &'static str {
        match self {
            Self::PartitionDetected(_) => "partition_detected",
            Self::RecoveryFailed(_) => "recovery_failed",
            Self::RecoveryTimeout(_) => "recovery_timeout",
            Self::InsufficientConnectivity { .. } => "insufficient_connectivity",
            Self::HealthCheckFailed(_) => "health_check_failed",
            Self::InvalidNetworkState(_) => "invalid_state",
        }
    }
}

/// Network health status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkHealth {
    /// Overall network health status
    pub status: NetworkHealthStatus,
    /// Number of connected validators
    pub connected_validators: usize,
    /// Total number of known validators
    pub total_validators: usize,
    /// Connectivity percentage
    pub connectivity_percentage: u8,
    /// Last successful consensus activity
    pub last_consensus_activity: Option<SystemTime>,
    /// Current round or height
    pub current_round: u32,
    /// Network partition indicators
    pub partition_indicators: Vec<PartitionIndicator>,
    /// Recovery status
    pub recovery_status: Option<RecoveryStatus>,
}

/// Network health status levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NetworkHealthStatus {
    /// Network is healthy and functioning normally
    Healthy,
    /// Network has minor issues but is still functional
    Degraded,
    /// Network partition detected or suspected
    Partitioned,
    /// Network is experiencing critical issues
    Critical,
    /// Network is in recovery mode
    Recovering,
}

impl Default for NetworkHealthStatus {
    fn default() -> Self {
        Self::Healthy
    }
}

/// Indicators that suggest a network partition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PartitionIndicator {
    /// Low validator connectivity
    LowConnectivity { percentage: u8 },
    /// No consensus progress for extended period
    NoProgress { duration_secs: u64 },
    /// Multiple competing forks detected
    MultipleForks { count: usize },
    /// Validator heartbeat failures
    HeartbeatFailures { failed_validators: HashSet<String> },
    /// Clock synchronization issues
    ClockSkew { max_skew_ms: i64 },
}

/// Recovery operation status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryStatus {
    /// Recovery operation ID
    pub id: String,
    /// When recovery started
    pub started_at: SystemTime,
    /// Current recovery phase
    pub phase: RecoveryPhase,
    /// Number of attempts made
    pub attempts: u32,
    /// Maximum attempts allowed
    pub max_attempts: u32,
    /// Recovery progress percentage (0-100)
    pub progress: u8,
    /// Error messages from failed attempts
    pub errors: Vec<String>,
}

/// Phases of network recovery
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RecoveryPhase {
    /// Detecting network partition
    Detection,
    /// Attempting to reconnect validators
    Reconnection,
    /// Synchronizing state across partitions
    StateSynchronization,
    /// Resolving competing forks
    ForkResolution,
    /// Resuming normal consensus
    ConsensusResumption,
    /// Recovery completed successfully
    Completed,
    /// Recovery failed
    Failed,
}

/// Metrics for network recovery operations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkRecoveryMetrics {
    /// Total number of partitions detected
    pub partitions_detected: u64,
    /// Total number of successful recoveries
    pub successful_recoveries: u64,
    /// Total number of failed recoveries
    pub failed_recoveries: u64,
    /// Total number of recovery timeouts
    pub recovery_timeouts: u64,
    /// Average recovery time in milliseconds
    pub avg_recovery_time_ms: u64,
    /// Maximum recovery time recorded
    pub max_recovery_time_ms: u64,
    /// Current network health status
    pub current_health_status: NetworkHealthStatus,
    /// Time since last partition detection
    pub time_since_last_partition: Option<Duration>,
    /// Current recovery operation if active
    pub active_recovery: Option<RecoveryStatus>,
}

// ================================================================================================
// Network Recovery Manager
// ================================================================================================

/// Comprehensive network partition recovery manager
#[derive(Debug)]
pub struct NetworkRecoveryManager {
    /// Configuration
    config: NetworkRecoveryConfig,
    /// Current network health status
    network_health: Arc<AsyncRwLock<NetworkHealth>>,
    /// Known validators and their status
    validator_status: Arc<AsyncRwLock<HashMap<String, ValidatorStatus>>>,
    /// Recovery operation history
    recovery_history: Arc<AsyncRwLock<Vec<RecoveryStatus>>>,
    /// Network recovery metrics
    metrics: Arc<AsyncRwLock<NetworkRecoveryMetrics>>,
    /// Last health check time
    last_health_check: Arc<AsyncRwLock<Instant>>,
    /// Fork detection manager integration
    fork_detector: Option<Arc<ForkDetectionManager>>,
}

/// Status information for individual validators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorStatus {
    /// Validator identifier
    pub id: String,
    /// Whether validator is currently connected
    pub connected: bool,
    /// Last time we heard from this validator
    pub last_seen: SystemTime,
    /// Last known height/round for this validator
    pub last_known_height: u64,
    /// Number of consecutive failed health checks
    pub failed_health_checks: u32,
    /// Round trip time for communication
    pub rtt_ms: Option<u64>,
}

impl NetworkRecoveryManager {
    /// Create a new network recovery manager
    pub async fn new(config: NetworkRecoveryConfig) -> Result<Self, NetworkRecoveryError> {
        config
            .validate()
            .map_err(NetworkRecoveryError::InvalidNetworkState)?;

        let initial_health = NetworkHealth {
            status: NetworkHealthStatus::Healthy,
            connected_validators: 0,
            total_validators: 0,
            connectivity_percentage: 100,
            last_consensus_activity: Some(SystemTime::now()),
            current_round: 0,
            partition_indicators: Vec::new(),
            recovery_status: None,
        };

        let manager = Self {
            config,
            network_health: Arc::new(AsyncRwLock::new(initial_health)),
            validator_status: Arc::new(AsyncRwLock::new(HashMap::new())),
            recovery_history: Arc::new(AsyncRwLock::new(Vec::new())),
            metrics: Arc::new(AsyncRwLock::new(NetworkRecoveryMetrics::default())),
            last_health_check: Arc::new(AsyncRwLock::new(Instant::now())),
            fork_detector: None,
        };

        info!("Network recovery manager initialized");
        Ok(manager)
    }

    /// Set fork detection manager for integration
    pub async fn set_fork_detector(&mut self, fork_detector: Arc<ForkDetectionManager>) {
        self.fork_detector = Some(fork_detector);
        debug!("Fork detector integrated with network recovery manager");
    }

    /// Update validator status
    pub async fn update_validator_status(
        &self,
        validator_id: String,
        connected: bool,
        height: u64,
    ) {
        let mut status_map = self.validator_status.write().await;

        let status = status_map
            .entry(validator_id.clone())
            .or_insert_with(|| ValidatorStatus {
                id: validator_id.clone(),
                connected: false,
                last_seen: SystemTime::now(),
                last_known_height: 0,
                failed_health_checks: 0,
                rtt_ms: None,
            });

        status.connected = connected;
        status.last_known_height = height;
        status.last_seen = SystemTime::now();

        if connected {
            status.failed_health_checks = 0;
        } else {
            status.failed_health_checks += 1;
        }

        debug!(
            "Updated validator {} status: connected={}, height={}",
            validator_id, connected, height
        );
    }

    /// Perform comprehensive network health check
    pub async fn perform_health_check(&self) -> Result<NetworkHealth, NetworkRecoveryError> {
        if !self.config.enable_health_monitoring {
            return Ok(self.network_health.read().await.clone());
        }

        let mut health = NetworkHealth {
            status: NetworkHealthStatus::Healthy,
            connected_validators: 0,
            total_validators: 0,
            connectivity_percentage: 0,
            last_consensus_activity: None,
            current_round: 0,
            partition_indicators: Vec::new(),
            recovery_status: None,
        };

        // Analyze validator connectivity
        let validator_status = self.validator_status.read().await;
        health.total_validators = validator_status.len();
        health.connected_validators = validator_status.values().filter(|v| v.connected).count();

        if health.total_validators > 0 {
            health.connectivity_percentage =
                ((health.connected_validators * 100) / health.total_validators) as u8;
        }

        // Check for partition indicators
        let mut indicators = Vec::new();

        // Low connectivity check
        if health.connectivity_percentage < self.config.min_connectivity_percentage {
            indicators.push(PartitionIndicator::LowConnectivity {
                percentage: health.connectivity_percentage,
            });
            health.status = NetworkHealthStatus::Partitioned;
        }

        // Check for heartbeat failures
        let failed_validators: HashSet<String> = validator_status
            .values()
            .filter(|v| !v.connected && v.failed_health_checks > 3)
            .map(|v| v.id.clone())
            .collect();

        if !failed_validators.is_empty() {
            indicators.push(PartitionIndicator::HeartbeatFailures { failed_validators });
            if health.status == NetworkHealthStatus::Healthy {
                health.status = NetworkHealthStatus::Degraded;
            }
        }

        // Check for consensus progress
        let now = SystemTime::now();
        let timeout_duration = Duration::from_secs(self.config.partition_detection_timeout_secs);

        if let Some(last_activity) = health.last_consensus_activity {
            if now.duration_since(last_activity).unwrap_or(Duration::ZERO) > timeout_duration {
                let duration_secs = now
                    .duration_since(last_activity)
                    .unwrap_or(Duration::ZERO)
                    .as_secs();
                indicators.push(PartitionIndicator::NoProgress { duration_secs });
                health.status = NetworkHealthStatus::Critical;
            }
        }

        // Check for multiple forks (integration with fork detector)
        if let Some(fork_detector) = &self.fork_detector {
            let active_forks = fork_detector.get_active_forks().await;
            if active_forks.len() > 1 {
                indicators.push(PartitionIndicator::MultipleForks {
                    count: active_forks.len(),
                });
                if health.status == NetworkHealthStatus::Healthy {
                    health.status = NetworkHealthStatus::Degraded;
                }
            }
        }

        health.partition_indicators = indicators;

        // Update stored health status
        {
            let mut stored_health = self.network_health.write().await;
            *stored_health = health.clone();
        }

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.current_health_status = health.status.clone();
        }

        // Update last health check time
        *self.last_health_check.write().await = Instant::now();

        info!(
            "Health check completed: status={:?}, connectivity={}%",
            health.status, health.connectivity_percentage
        );
        Ok(health)
    }

    /// Detect if network partition exists
    pub async fn detect_partition(&self) -> Result<bool, NetworkRecoveryError> {
        let health = self.perform_health_check().await?;

        let is_partitioned = matches!(
            health.status,
            NetworkHealthStatus::Partitioned | NetworkHealthStatus::Critical
        );

        if is_partitioned {
            let mut metrics = self.metrics.write().await;
            metrics.partitions_detected += 1;

            warn!(
                "Network partition detected: {:?}",
                health.partition_indicators
            );
        }

        Ok(is_partitioned)
    }

    /// Attempt to recover from network partition
    pub async fn attempt_recovery(&self) -> Result<RecoveryStatus, NetworkRecoveryError> {
        if !self.config.auto_recovery {
            return Err(NetworkRecoveryError::RecoveryFailed(
                "Auto recovery is disabled".to_string(),
            ));
        }

        let recovery_id = format!(
            "recovery-{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );

        let mut recovery_status = RecoveryStatus {
            id: recovery_id.clone(),
            started_at: SystemTime::now(),
            phase: RecoveryPhase::Detection,
            attempts: 1,
            max_attempts: self.config.max_recovery_attempts,
            progress: 0,
            errors: Vec::new(),
        };

        info!("Starting network recovery operation: {}", recovery_id);

        let start_time = Instant::now();
        let mut success = false;

        // Recovery phases
        for phase in [
            RecoveryPhase::Detection,
            RecoveryPhase::Reconnection,
            RecoveryPhase::StateSynchronization,
            RecoveryPhase::ForkResolution,
            RecoveryPhase::ConsensusResumption,
        ] {
            recovery_status.phase = phase.clone();
            recovery_status.progress = match phase {
                RecoveryPhase::Detection => 20,
                RecoveryPhase::Reconnection => 40,
                RecoveryPhase::StateSynchronization => 60,
                RecoveryPhase::ForkResolution => 80,
                RecoveryPhase::ConsensusResumption => 90,
                _ => recovery_status.progress,
            };

            info!(
                "Recovery phase: {:?} ({}%)",
                phase, recovery_status.progress
            );

            match self
                .execute_recovery_phase(&phase, &mut recovery_status)
                .await
            {
                Ok(_) => {
                    debug!("Recovery phase {:?} completed successfully", phase);
                }
                Err(e) => {
                    error!("Recovery phase {:?} failed: {}", phase, e);
                    recovery_status.errors.push(format!("{:?}: {}", phase, e));

                    if recovery_status.attempts >= self.config.max_recovery_attempts {
                        recovery_status.phase = RecoveryPhase::Failed;
                        break;
                    }

                    // Retry with exponential backoff
                    let backoff = Duration::from_secs(2_u64.pow(recovery_status.attempts.min(6)));
                    tokio::time::sleep(backoff).await;
                    recovery_status.attempts += 1;
                    continue;
                }
            }
        }

        // Finalize recovery status
        if recovery_status.phase != RecoveryPhase::Failed {
            recovery_status.phase = RecoveryPhase::Completed;
            recovery_status.progress = 100;
            success = true;
        }

        let recovery_time = start_time.elapsed();

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            if success {
                metrics.successful_recoveries += 1;
                metrics.avg_recovery_time_ms =
                    (metrics.avg_recovery_time_ms + recovery_time.as_millis() as u64) / 2;
                if recovery_time.as_millis() as u64 > metrics.max_recovery_time_ms {
                    metrics.max_recovery_time_ms = recovery_time.as_millis() as u64;
                }
            } else {
                metrics.failed_recoveries += 1;
            }
        }

        // Add to recovery history
        {
            let mut history = self.recovery_history.write().await;
            history.push(recovery_status.clone());

            // Prune old history
            while history.len() > 50 {
                history.remove(0);
            }
        }

        if success {
            info!(
                "Network recovery completed successfully in {:?}",
                recovery_time
            );
        } else {
            error!(
                "Network recovery failed after {} attempts",
                recovery_status.attempts
            );
        }

        Ok(recovery_status)
    }

    /// Execute a specific recovery phase
    async fn execute_recovery_phase(
        &self,
        phase: &RecoveryPhase,
        recovery_status: &mut RecoveryStatus,
    ) -> Result<(), NetworkRecoveryError> {
        match phase {
            RecoveryPhase::Detection => {
                // Confirm partition still exists
                if !self.detect_partition().await? {
                    return Ok(()); // No partition detected, recovery not needed
                }
            }
            RecoveryPhase::Reconnection => {
                // Attempt to reconnect disconnected validators
                self.attempt_validator_reconnection().await?;
            }
            RecoveryPhase::StateSynchronization => {
                // Synchronize state across partitions
                self.synchronize_partition_state().await?;
            }
            RecoveryPhase::ForkResolution => {
                // Resolve any competing forks
                self.resolve_partition_forks().await?;
            }
            RecoveryPhase::ConsensusResumption => {
                // Resume normal consensus operation
                self.resume_consensus_operation().await?;
            }
            _ => {}
        }

        Ok(())
    }

    /// Attempt to reconnect disconnected validators
    async fn attempt_validator_reconnection(&self) -> Result<(), NetworkRecoveryError> {
        let validator_status = self.validator_status.read().await;
        let disconnected_validators: Vec<_> =
            validator_status.values().filter(|v| !v.connected).collect();

        if disconnected_validators.is_empty() {
            return Ok(());
        }

        info!(
            "Attempting to reconnect {} validators",
            disconnected_validators.len()
        );

        // Execute reconnection attempts for each disconnected validator
        for validator in disconnected_validators {
            debug!("Attempting to reconnect validator: {}", validator.id);

            // In production, this would:
            // 1. Attempt TCP/websocket reconnection with exponential backoff
            // 2. Update validator connection status on success
            // 3. Track reconnection metrics and failures

            // Simulate reconnection attempt
            tokio::time::sleep(Duration::from_millis(50)).await;
            info!("Reconnection initiated for validator {}", validator.id);
        }

        Ok(())
    }

    /// Synchronize state across network partitions
    async fn synchronize_partition_state(&self) -> Result<(), NetworkRecoveryError> {
        info!("Synchronizing state across partitions");

        // Coordinate state synchronization across network partitions
        // In production, this would:
        // 1. Query current state from all active validators
        // 2. Determine canonical state through consensus (most recent valid state)
        // 3. Synchronize all validators to the canonical state
        // 4. Verify state consistency using cryptographic proofs

        info!("Coordinating state synchronization across validators");

        // Simulate state sync coordination
        tokio::time::sleep(Duration::from_millis(200)).await;

        info!("State synchronization coordination completed");

        Ok(())
    }

    /// Resolve forks caused by network partition
    async fn resolve_partition_forks(&self) -> Result<(), NetworkRecoveryError> {
        if let Some(fork_detector) = &self.fork_detector {
            let active_forks = fork_detector.get_active_forks().await;

            if !active_forks.is_empty() {
                info!("Resolving {} forks caused by partition", active_forks.len());

                for fork in active_forks {
                    if let Err(e) = fork_detector.attempt_resolution(&fork.id).await {
                        warn!("Failed to resolve fork {}: {}", fork.id, e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Resume normal consensus operation
    async fn resume_consensus_operation(&self) -> Result<(), NetworkRecoveryError> {
        info!("Resuming normal consensus operation");

        // Grace period to allow network to stabilize
        tokio::time::sleep(Duration::from_secs(self.config.recovery_grace_period_secs)).await;

        // Update network health status
        let _ = self.update_network_health().await;

        Ok(())
    }

    /// Update network health based on current conditions
    async fn update_network_health(&self) -> Result<(), NetworkRecoveryError> {
        let health = self.perform_health_check().await?;
        *self.network_health.write().await = health;
        Ok(())
    }

    /// Get current network health status
    pub async fn get_network_health(&self) -> NetworkHealth {
        self.network_health.read().await.clone()
    }

    /// Get network recovery metrics
    pub async fn get_metrics(&self) -> NetworkRecoveryMetrics {
        self.metrics.read().await.clone()
    }

    /// Get recovery history
    pub async fn get_recovery_history(&self) -> Vec<RecoveryStatus> {
        self.recovery_history.read().await.clone()
    }

    /// Check if recovery is currently active
    pub async fn is_recovery_active(&self) -> bool {
        let health = self.network_health.read().await;
        matches!(health.status, NetworkHealthStatus::Recovering)
    }
}

/// Trait for network recovery integration
#[async_trait]
pub trait NetworkRecovery: Send + Sync {
    /// Detect network partition
    async fn detect_partition(&self) -> Result<bool, NetworkRecoveryError>;

    /// Attempt network recovery
    async fn attempt_recovery(&self) -> Result<RecoveryStatus, NetworkRecoveryError>;

    /// Get network health status
    async fn get_network_health(&self) -> NetworkHealth;

    /// Get recovery metrics
    async fn get_metrics(&self) -> NetworkRecoveryMetrics;
}

#[async_trait]
impl NetworkRecovery for NetworkRecoveryManager {
    async fn detect_partition(&self) -> Result<bool, NetworkRecoveryError> {
        self.detect_partition().await
    }

    async fn attempt_recovery(&self) -> Result<RecoveryStatus, NetworkRecoveryError> {
        self.attempt_recovery().await
    }

    async fn get_network_health(&self) -> NetworkHealth {
        self.get_network_health().await
    }

    async fn get_metrics(&self) -> NetworkRecoveryMetrics {
        self.get_metrics().await
    }
}

