//! Peer reputation system for P2P network
//!
//! This module implements a comprehensive reputation scoring system that tracks
//! peer behavior and adjusts trust levels accordingly. It helps protect the network
//! from malicious actors and prioritizes connections to reliable peers.

#[cfg(feature = "persistence")]
use crate::error::P2PError;
use crate::error::P2PResult;
use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Reputation score bounds
const MIN_REPUTATION: f64 = 0.0;
const MAX_REPUTATION: f64 = 100.0;
const INITIAL_REPUTATION: f64 = 50.0;

/// Reputation thresholds
const TRUSTED_THRESHOLD: f64 = 70.0;
const SUSPICIOUS_THRESHOLD: f64 = 30.0;
const BANNED_THRESHOLD: f64 = 10.0;

/// Score adjustments for various events
const SUCCESSFUL_MESSAGE_SCORE: f64 = 0.1;
const FAILED_MESSAGE_SCORE: f64 = -0.5;
const INVALID_MESSAGE_SCORE: f64 = -2.0;
const SECURITY_VIOLATION_SCORE: f64 = -10.0;
const SUCCESSFUL_AUTH_SCORE: f64 = 1.0;
const FAILED_AUTH_SCORE: f64 = -5.0;
const GOOD_LATENCY_SCORE: f64 = 0.2;
const BAD_LATENCY_SCORE: f64 = -0.3;
const DISCONNECT_SCORE: f64 = -1.0;

/// Decay rate for reputation scores (per hour)
const REPUTATION_DECAY_RATE: f64 = 0.95;

/// Reputation system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationConfig {
    /// Enable reputation tracking
    pub enabled: bool,
    /// Initial reputation score for new peers
    pub initial_score: f64,
    /// Minimum reputation before auto-ban
    pub ban_threshold: f64,
    /// Maximum reputation score
    pub max_score: f64,
    /// Reputation decay interval
    pub decay_interval: Duration,
    /// Enable persistent storage
    pub persist_scores: bool,
    /// Score recovery rate for banned peers
    pub recovery_rate: f64,
}

impl Default for ReputationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            initial_score: INITIAL_REPUTATION,
            ban_threshold: BANNED_THRESHOLD,
            max_score: MAX_REPUTATION,
            decay_interval: Duration::from_secs(3600), // 1 hour
            persist_scores: true,
            recovery_rate: 0.1,
        }
    }
}

/// Peer reputation status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ReputationStatus {
    Trusted,
    Normal,
    Suspicious,
    Banned,
}

/// Reputation event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReputationEvent {
    MessageSent { success: bool },
    MessageReceived { valid: bool },
    AuthenticationAttempt { success: bool },
    SecurityViolation { severity: String },
    LatencyMeasurement { latency: Duration },
    ConnectionEvent { connected: bool },
    ManualAdjustment { delta: f64, reason: String },
}

/// Peer reputation data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerReputation {
    /// Peer ID
    pub peer_id: PeerId,
    /// Current reputation score
    pub score: f64,
    /// Reputation status
    pub status: ReputationStatus,
    /// First seen timestamp
    pub first_seen: SystemTime,
    /// Last interaction timestamp
    pub last_interaction: SystemTime,
    /// Total interactions
    pub total_interactions: u64,
    /// Successful interactions
    pub successful_interactions: u64,
    /// Failed interactions
    pub failed_interactions: u64,
    /// Security violations
    pub security_violations: u32,
    /// Average response latency
    pub avg_latency: Option<Duration>,
    /// Ban expiry time (if banned)
    pub ban_expiry: Option<SystemTime>,
    /// Reputation history
    pub history: Vec<ReputationHistoryEntry>,
}

/// Reputation history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationHistoryEntry {
    pub timestamp: SystemTime,
    pub event: ReputationEvent,
    pub score_before: f64,
    pub score_after: f64,
}

/// Reputation manager
pub struct ReputationManager {
    config: ReputationConfig,
    reputations: Arc<RwLock<HashMap<PeerId, PeerReputation>>>,
    #[cfg(feature = "persistence")]
    db: Option<Arc<rocksdb::DB>>,
    decay_task_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

impl ReputationManager {
    /// Create a new reputation manager synchronously (without persistence or background tasks)
    /// This is useful for testing and simple use cases
    pub fn new_sync(config: ReputationConfig) -> Self {
        let reputations = Arc::new(RwLock::new(HashMap::new()));

        Self {
            config,
            reputations,
            #[cfg(feature = "persistence")]
            db: None,
            decay_task_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a new reputation manager
    pub async fn new(config: ReputationConfig) -> P2PResult<Self> {
        let reputations = Arc::new(RwLock::new(HashMap::new()));

        #[cfg(feature = "persistence")]
        let db = if config.persist_scores {
            let mut opts = rocksdb::Options::default();
            opts.create_if_missing(true);

            let db = rocksdb::DB::open(&opts, "reputation_scores.db")
                .map_err(|e| P2PError::Internal(format!("Failed to open reputation DB: {}", e)))?;

            Some(Arc::new(db))
        } else {
            None
        };

        let manager = Self {
            config,
            reputations,
            #[cfg(feature = "persistence")]
            db,
            decay_task_handle: Arc::new(RwLock::new(None)),
        };

        // Load persisted scores
        #[cfg(feature = "persistence")]
        manager.load_persisted_scores().await?;

        // Start decay task
        manager.start_decay_task().await;

        Ok(manager)
    }

    /// Record a reputation event
    pub async fn record_event(&self, peer_id: PeerId, event: ReputationEvent) -> P2PResult<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let mut reputations = self.reputations.write().await;
        let reputation = reputations
            .entry(peer_id)
            .or_insert_with(|| PeerReputation {
                peer_id,
                score: self.config.initial_score,
                status: ReputationStatus::Normal,
                first_seen: SystemTime::now(),
                last_interaction: SystemTime::now(),
                total_interactions: 0,
                successful_interactions: 0,
                failed_interactions: 0,
                security_violations: 0,
                avg_latency: None,
                ban_expiry: None,
                history: Vec::new(),
            });

        let score_before = reputation.score;
        let score_delta = self.calculate_score_delta(&event, reputation);

        // Update score
        reputation.score = (reputation.score + score_delta)
            .max(MIN_REPUTATION)
            .min(self.config.max_score);

        // Update interaction counters
        reputation.total_interactions += 1;
        reputation.last_interaction = SystemTime::now();

        match &event {
            ReputationEvent::MessageSent { success }
            | ReputationEvent::MessageReceived { valid: success }
            | ReputationEvent::AuthenticationAttempt { success } => {
                if *success {
                    reputation.successful_interactions += 1;
                } else {
                    reputation.failed_interactions += 1;
                }
            }
            ReputationEvent::SecurityViolation { .. } => {
                reputation.security_violations += 1;
                reputation.failed_interactions += 1;
            }
            ReputationEvent::LatencyMeasurement { latency } => {
                // Update average latency
                if let Some(avg) = reputation.avg_latency {
                    reputation.avg_latency = Some(Duration::from_millis(
                        ((avg.as_millis() * 9 + latency.as_millis()) / 10) as u64,
                    ));
                } else {
                    reputation.avg_latency = Some(*latency);
                }
            }
            _ => {}
        }

        // Update status
        reputation.status = self.determine_status(reputation.score);

        // Add to history
        reputation.history.push(ReputationHistoryEntry {
            timestamp: SystemTime::now(),
            event: event.clone(),
            score_before,
            score_after: reputation.score,
        });

        // Keep history size reasonable
        if reputation.history.len() > 100 {
            reputation.history.remove(0);
        }

        // Log significant changes
        if (score_delta.abs() > 5.0) || reputation.status == ReputationStatus::Banned {
            warn!(
                "Significant reputation change for peer {}: {} -> {} ({})",
                peer_id,
                score_before,
                reputation.score,
                match event {
                    ReputationEvent::SecurityViolation { ref severity } =>
                        format!("Security violation: {}", severity),
                    _ => format!("{:?}", event),
                }
            );
        }

        // Handle ban status
        if reputation.status == ReputationStatus::Banned && reputation.ban_expiry.is_none() {
            reputation.ban_expiry = Some(SystemTime::now() + Duration::from_secs(3600));
            // 1 hour ban
        }

        // Persist if enabled
        #[cfg(feature = "persistence")]
        if self.config.persist_scores {
            self.persist_reputation(&peer_id, reputation).await?;
        }

        Ok(())
    }

    /// Get peer reputation
    pub async fn get_reputation(&self, peer_id: &PeerId) -> Option<PeerReputation> {
        let reputations = self.reputations.read().await;
        reputations.get(peer_id).cloned()
    }

    /// Get peer reputation score
    pub async fn get_score(&self, peer_id: &PeerId) -> f64 {
        let reputations = self.reputations.read().await;
        reputations
            .get(peer_id)
            .map(|r| r.score)
            .unwrap_or(self.config.initial_score)
    }

    /// Get peer reputation status
    pub async fn get_status(&self, peer_id: &PeerId) -> ReputationStatus {
        let reputations = self.reputations.read().await;
        reputations
            .get(peer_id)
            .map(|r| r.status.clone())
            .unwrap_or(ReputationStatus::Normal)
    }

    /// Check if peer is banned
    pub async fn is_banned(&self, peer_id: &PeerId) -> bool {
        let reputations = self.reputations.read().await;
        if let Some(reputation) = reputations.get(peer_id) {
            if reputation.status == ReputationStatus::Banned {
                // Check if ban has expired
                if let Some(expiry) = reputation.ban_expiry {
                    return SystemTime::now() < expiry;
                }
                return true;
            }
        }
        false
    }

    /// Get trusted peers
    pub async fn get_trusted_peers(&self) -> Vec<PeerId> {
        let reputations = self.reputations.read().await;
        reputations
            .iter()
            .filter(|(_, r)| r.status == ReputationStatus::Trusted)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get suspicious peers
    pub async fn get_suspicious_peers(&self) -> Vec<PeerId> {
        let reputations = self.reputations.read().await;
        reputations
            .iter()
            .filter(|(_, r)| r.status == ReputationStatus::Suspicious)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Reset peer reputation
    pub async fn reset_reputation(&self, peer_id: &PeerId) -> P2PResult<()> {
        let mut reputations = self.reputations.write().await;
        if let Some(reputation) = reputations.get_mut(peer_id) {
            reputation.score = self.config.initial_score;
            reputation.status = ReputationStatus::Normal;
            reputation.ban_expiry = None;
            reputation.history.clear();

            info!("Reset reputation for peer {}", peer_id);
        }
        Ok(())
    }

    /// Calculate score delta for an event
    fn calculate_score_delta(&self, event: &ReputationEvent, _reputation: &PeerReputation) -> f64 {
        match event {
            ReputationEvent::MessageSent { success } => {
                if *success {
                    SUCCESSFUL_MESSAGE_SCORE
                } else {
                    FAILED_MESSAGE_SCORE
                }
            }
            ReputationEvent::MessageReceived { valid } => {
                if *valid {
                    SUCCESSFUL_MESSAGE_SCORE
                } else {
                    INVALID_MESSAGE_SCORE
                }
            }
            ReputationEvent::AuthenticationAttempt { success } => {
                if *success {
                    SUCCESSFUL_AUTH_SCORE
                } else {
                    FAILED_AUTH_SCORE
                }
            }
            ReputationEvent::SecurityViolation { .. } => SECURITY_VIOLATION_SCORE,
            ReputationEvent::LatencyMeasurement { latency } => {
                if latency.as_millis() < 100 {
                    GOOD_LATENCY_SCORE
                } else if latency.as_millis() > 1000 {
                    BAD_LATENCY_SCORE
                } else {
                    0.0
                }
            }
            ReputationEvent::ConnectionEvent { connected } => {
                if *connected {
                    0.5
                } else {
                    DISCONNECT_SCORE
                }
            }
            ReputationEvent::ManualAdjustment { delta, .. } => *delta,
        }
    }

    /// Determine reputation status from score
    fn determine_status(&self, score: f64) -> ReputationStatus {
        if score >= TRUSTED_THRESHOLD {
            ReputationStatus::Trusted
        } else if score <= self.config.ban_threshold {
            ReputationStatus::Banned
        } else if score <= SUSPICIOUS_THRESHOLD {
            ReputationStatus::Suspicious
        } else {
            ReputationStatus::Normal
        }
    }

    /// Start reputation decay task
    async fn start_decay_task(&self) {
        let reputations = self.reputations.clone();
        let decay_interval = self.config.decay_interval;
        let initial_score = self.config.initial_score;

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(decay_interval);

            loop {
                interval.tick().await;

                let mut reps = reputations.write().await;
                for reputation in reps.values_mut() {
                    // Decay towards initial score
                    let diff = reputation.score - initial_score;
                    reputation.score = initial_score + (diff * REPUTATION_DECAY_RATE);

                    // Check for ban expiry
                    if let Some(expiry) = reputation.ban_expiry {
                        if SystemTime::now() > expiry {
                            reputation.ban_expiry = None;
                            reputation.score = reputation.score.max(BANNED_THRESHOLD + 1.0);
                        }
                    }
                }
            }
        });

        let mut handle_lock = self.decay_task_handle.write().await;
        *handle_lock = Some(handle);
    }

    /// Load persisted reputation scores
    #[cfg(feature = "persistence")]
    async fn load_persisted_scores(&self) -> P2PResult<()> {
        if let Some(db) = &self.db {
            let mut reputations = self.reputations.write().await;

            let iter = db.iterator(rocksdb::IteratorMode::Start);
            for item in iter {
                let (key, value) = item.map_err(|e| {
                    P2PError::Internal(format!("Failed to read reputation DB: {}", e))
                })?;

                let peer_id_str = String::from_utf8_lossy(&key);
                if let Ok(peer_id) = peer_id_str.parse::<PeerId>() {
                    if let Ok(reputation) = bincode::deserialize::<PeerReputation>(&value) {
                        reputations.insert(peer_id, reputation);
                    }
                }
            }

            info!("Loaded {} peer reputations from storage", reputations.len());
        }
        Ok(())
    }

    /// Persist a peer's reputation
    #[cfg(feature = "persistence")]
    async fn persist_reputation(
        &self,
        peer_id: &PeerId,
        reputation: &PeerReputation,
    ) -> P2PResult<()> {
        if let Some(db) = &self.db {
            let key = peer_id.to_string();
            let value = bincode::serialize(reputation).map_err(|e| {
                P2PError::Internal(format!("Failed to serialize reputation: {}", e))
            })?;

            db.put(key.as_bytes(), value)
                .map_err(|e| P2PError::Internal(format!("Failed to persist reputation: {}", e)))?;
        }
        Ok(())
    }

    /// Get reputation statistics
    pub async fn get_stats(&self) -> ReputationStats {
        let reputations = self.reputations.read().await;

        let total_peers = reputations.len();
        let trusted_peers = reputations
            .values()
            .filter(|r| r.status == ReputationStatus::Trusted)
            .count();
        let normal_peers = reputations
            .values()
            .filter(|r| r.status == ReputationStatus::Normal)
            .count();
        let suspicious_peers = reputations
            .values()
            .filter(|r| r.status == ReputationStatus::Suspicious)
            .count();
        let banned_peers = reputations
            .values()
            .filter(|r| r.status == ReputationStatus::Banned)
            .count();

        let avg_score = if total_peers > 0 {
            reputations.values().map(|r| r.score).sum::<f64>() / total_peers as f64
        } else {
            self.config.initial_score
        };

        ReputationStats {
            total_peers,
            trusted_peers,
            normal_peers,
            suspicious_peers,
            banned_peers,
            avg_score,
        }
    }

    /// Shutdown the reputation manager
    pub async fn shutdown(&self) -> P2PResult<()> {
        // Stop decay task
        let mut handle_lock = self.decay_task_handle.write().await;
        if let Some(handle) = handle_lock.take() {
            handle.abort();
        }

        // Final persist
        #[cfg(feature = "persistence")]
        if self.config.persist_scores {
            let reputations = self.reputations.read().await;
            for (peer_id, reputation) in reputations.iter() {
                self.persist_reputation(peer_id, reputation).await?;
            }
        }

        Ok(())
    }
}

/// Reputation statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationStats {
    pub total_peers: usize,
    pub trusted_peers: usize,
    pub normal_peers: usize,
    pub suspicious_peers: usize,
    pub banned_peers: usize,
    pub avg_score: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_reputation_manager() {
        let config = ReputationConfig {
            persist_scores: false,
            ..Default::default()
        };

        let manager = ReputationManager::new(config).await.unwrap();
        let peer_id = PeerId::random();

        // Initial score
        assert_eq!(manager.get_score(&peer_id).await, INITIAL_REPUTATION);
        assert_eq!(manager.get_status(&peer_id).await, ReputationStatus::Normal);

        // Successful interaction
        manager
            .record_event(peer_id, ReputationEvent::MessageSent { success: true })
            .await
            .unwrap();
        assert!(manager.get_score(&peer_id).await > INITIAL_REPUTATION);

        // Failed interaction
        manager
            .record_event(peer_id, ReputationEvent::MessageReceived { valid: false })
            .await
            .unwrap();
        let score = manager.get_score(&peer_id).await;
        assert!(score < INITIAL_REPUTATION);

        // Security violation
        manager
            .record_event(
                peer_id,
                ReputationEvent::SecurityViolation {
                    severity: "high".to_string(),
                },
            )
            .await
            .unwrap();
        assert!(manager.get_score(&peer_id).await < score);
    }

    #[tokio::test]
    async fn test_reputation_banning() {
        let config = ReputationConfig {
            persist_scores: false,
            ban_threshold: 20.0,
            ..Default::default()
        };

        let manager = ReputationManager::new(config).await.unwrap();
        let peer_id = PeerId::random();

        // Multiple security violations should lead to ban
        for _ in 0..5 {
            manager
                .record_event(
                    peer_id,
                    ReputationEvent::SecurityViolation {
                        severity: "critical".to_string(),
                    },
                )
                .await
                .unwrap();
        }

        assert_eq!(manager.get_status(&peer_id).await, ReputationStatus::Banned);
        assert!(manager.is_banned(&peer_id).await);
    }
}
