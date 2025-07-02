//! Production-grade audit logging system for P2P network security
//!
//! This module provides comprehensive audit logging with persistent storage,
//! enabling security analysis, compliance reporting, and forensic investigation
//! of network activities.

use crate::error::{P2PResult, P2PError};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, RwLock};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// Maximum in-memory audit entries before forced flush
const MAX_MEMORY_ENTRIES: usize = 10_000;

/// Default flush interval for audit logs
const DEFAULT_FLUSH_INTERVAL: Duration = Duration::from_secs(30);

/// Audit event types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AuditEventType {
    Authentication,
    Authorization,
    NetworkConnection,
    MessageTransfer,
    SecurityViolation,
    SystemEvent,
    AdminAction,
}

/// Audit event severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AuditSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Comprehensive audit event structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Unique event ID
    pub id: String,
    /// Event timestamp
    pub timestamp: SystemTime,
    /// Event type category
    pub event_type: AuditEventType,
    /// Severity level
    pub severity: AuditSeverity,
    /// Source IP address (if applicable)
    pub source_ip: Option<IpAddr>,
    /// Peer ID (if applicable)
    pub peer_id: Option<String>,
    /// User/session identifier (if applicable)
    pub user_id: Option<String>,
    /// Action or operation performed
    pub action: String,
    /// Resource accessed or affected
    pub resource: Option<String>,
    /// Result of the operation (success/failure)
    pub result: AuditResult,
    /// Additional context data
    pub details: serde_json::Value,
    /// Session ID for correlation
    pub session_id: Option<String>,
}

/// Audit operation result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AuditResult {
    Success,
    Failure(String),
    Blocked(String),
    Warning(String),
}

/// Audit logger configuration
#[derive(Debug, Clone)]
pub struct AuditConfig {
    /// Enable persistent storage
    pub enable_persistence: bool,
    /// Database path for persistent storage
    pub db_path: Option<String>,
    /// Maximum in-memory entries
    pub max_memory_entries: usize,
    /// Flush interval for batched writes
    pub flush_interval: Duration,
    /// Retention period for audit logs
    pub retention_period: Duration,
    /// Enable real-time alerts for critical events
    pub enable_alerts: bool,
    /// Minimum severity level to log
    pub min_severity: AuditSeverity,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enable_persistence: true,
            db_path: Some("audit_logs.db".to_string()),
            max_memory_entries: MAX_MEMORY_ENTRIES,
            flush_interval: DEFAULT_FLUSH_INTERVAL,
            retention_period: Duration::from_secs(30 * 24 * 60 * 60), // 30 days
            enable_alerts: true,
            min_severity: AuditSeverity::Low,
        }
    }
}

/// Production-grade audit logger with persistent storage
pub struct AuditLogger {
    config: AuditConfig,
    #[cfg(feature = "persistence")]
    db: Option<Arc<rocksdb::DB>>,
    memory_buffer: Arc<Mutex<VecDeque<AuditEvent>>>,
    stats: Arc<RwLock<AuditStats>>,
}

/// Audit logging statistics
#[derive(Debug, Default, Clone)]
pub struct AuditStats {
    pub total_events: u64,
    pub events_by_type: std::collections::HashMap<AuditEventType, u64>,
    pub events_by_severity: std::collections::HashMap<AuditSeverity, u64>,
    pub last_flush: Option<SystemTime>,
    pub flush_count: u64,
    pub errors: u64,
}

impl AuditLogger {
    /// Create a new audit logger
    pub async fn new(config: AuditConfig) -> P2PResult<Self> {
        let memory_buffer = Arc::new(Mutex::new(VecDeque::new()));
        let stats = Arc::new(RwLock::new(AuditStats::default()));

        #[cfg(feature = "persistence")]
        let db = if config.enable_persistence {
            if let Some(db_path) = &config.db_path {
                let mut opts = rocksdb::Options::default();
                opts.create_if_missing(true);
                opts.set_max_open_files(1000);
                opts.set_use_fsync(false);
                opts.set_bytes_per_sync(1024 * 1024);

                let db = rocksdb::DB::open(&opts, db_path).map_err(|e| {
                    P2PError::Internal(format!("Failed to open audit database: {}", e))
                })?;

                info!("Audit database opened at: {}", db_path);
                Some(Arc::new(db))
            } else {
                None
            }
        } else {
            None
        };

        let logger = Self {
            config,
            #[cfg(feature = "persistence")]
            db,
            memory_buffer,
            stats,
        };

        // Start background flush task
        logger.start_flush_task().await;

        Ok(logger)
    }

    /// Log an audit event
    pub async fn log_event(&self, event: AuditEvent) -> P2PResult<()> {
        // Check severity filter
        if !self.should_log_severity(&event.severity) {
            return Ok(());
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_events += 1;
            *stats
                .events_by_type
                .entry(event.event_type.clone())
                .or_insert(0) += 1;
            *stats
                .events_by_severity
                .entry(event.severity.clone())
                .or_insert(0) += 1;
        }

        // Add to memory buffer
        {
            let mut buffer = self.memory_buffer.lock().await;
            buffer.push_back(event.clone());

            // Force flush if buffer is full
            if buffer.len() >= self.config.max_memory_entries {
                drop(buffer);
                self.flush_to_storage().await?;
            }
        }

        // Handle critical events immediately
        if event.severity == AuditSeverity::Critical && self.config.enable_alerts {
            self.handle_critical_event(&event).await;
        }

        Ok(())
    }

    /// Log authentication events
    pub async fn log_auth_event(
        &self,
        source_ip: Option<IpAddr>,
        user_id: Option<String>,
        action: &str,
        result: AuditResult,
        details: serde_json::Value,
    ) -> P2PResult<()> {
        let event = AuditEvent {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: SystemTime::now(),
            event_type: AuditEventType::Authentication,
            severity: match &result {
                AuditResult::Success => AuditSeverity::Low,
                AuditResult::Failure(_) => AuditSeverity::Medium,
                AuditResult::Blocked(_) => AuditSeverity::High,
                AuditResult::Warning(_) => AuditSeverity::Low,
            },
            source_ip,
            peer_id: None,
            user_id,
            action: action.to_string(),
            resource: Some("auth".to_string()),
            result,
            details,
            session_id: None,
        };

        self.log_event(event).await
    }

    /// Log network connection events
    pub async fn log_connection_event(
        &self,
        source_ip: Option<IpAddr>,
        peer_id: Option<String>,
        action: &str,
        result: AuditResult,
    ) -> P2PResult<()> {
        let event = AuditEvent {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: SystemTime::now(),
            event_type: AuditEventType::NetworkConnection,
            severity: AuditSeverity::Low,
            source_ip,
            peer_id,
            user_id: None,
            action: action.to_string(),
            resource: Some("network".to_string()),
            result,
            details: serde_json::Value::Null,
            session_id: None,
        };

        self.log_event(event).await
    }

    /// Log security violation events
    pub async fn log_security_violation(
        &self,
        source_ip: Option<IpAddr>,
        peer_id: Option<String>,
        violation_type: &str,
        details: serde_json::Value,
    ) -> P2PResult<()> {
        let event = AuditEvent {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: SystemTime::now(),
            event_type: AuditEventType::SecurityViolation,
            severity: AuditSeverity::High,
            source_ip,
            peer_id,
            user_id: None,
            action: format!("security_violation: {violation_type}"),
            resource: Some("security".to_string()),
            result: AuditResult::Blocked("Security policy violation".to_string()),
            details,
            session_id: None,
        };

        self.log_event(event).await
    }

    /// Get audit statistics
    pub async fn get_stats(&self) -> AuditStats {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// Flush in-memory events to persistent storage
    pub async fn flush_to_storage(&self) -> P2PResult<()> {
        let events = {
            let mut buffer = self.memory_buffer.lock().await;
            let events: Vec<AuditEvent> = buffer.drain(..).collect();
            events
        };

        if events.is_empty() {
            return Ok(());
        }

        debug!("Flushing {} audit events to storage", events.len());

        #[cfg(feature = "persistence")]
        if let Some(db) = &self.db {
            for event in &events {
                let key = format!(
                    "{}:{}",
                    event
                        .timestamp
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    event.id
                );
                let value = serde_json::to_vec(&event).map_err(|e| {
                    P2PError::Internal(format!("Failed to serialize audit event: {}", e))
                })?;

                db.put(key.as_bytes(), value).map_err(|e| {
                    P2PError::Internal(format!("Failed to store audit event: {}", e))
                })?;
            }
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.last_flush = Some(SystemTime::now());
            stats.flush_count += 1;
        }

        info!("Successfully flushed {} audit events", events.len());
        Ok(())
    }

    /// Query audit events by criteria
    #[cfg(feature = "persistence")]
    pub async fn query_events(
        &self,
        start_time: Option<SystemTime>,
        end_time: Option<SystemTime>,
        event_type: Option<AuditEventType>,
        severity: Option<AuditSeverity>,
        limit: Option<usize>,
    ) -> P2PResult<Vec<AuditEvent>> {
        let db = self
            .db
            .as_ref()
            .ok_or_else(|| P2PError::Internal("Database not available".to_string()))?;

        let mut events = Vec::new();
        let iter = db.iterator(rocksdb::IteratorMode::Start);

        for item in iter {
            let (key, value) =
                item.map_err(|e| P2PError::Internal(format!("Database iteration error: {}", e)))?;

            let event: AuditEvent = serde_json::from_slice(&value).map_err(|e| {
                P2PError::Internal(format!("Failed to deserialize audit event: {}", e))
            })?;

            // Apply filters
            if let Some(start) = start_time {
                if event.timestamp < start {
                    continue;
                }
            }

            if let Some(end) = end_time {
                if event.timestamp > end {
                    continue;
                }
            }

            if let Some(ref filter_type) = event_type {
                if &event.event_type != filter_type {
                    continue;
                }
            }

            if let Some(ref filter_severity) = severity {
                if &event.severity != filter_severity {
                    continue;
                }
            }

            events.push(event);

            if let Some(max) = limit {
                if events.len() >= max {
                    break;
                }
            }
        }

        Ok(events)
    }

    /// Clean up old audit logs based on retention policy
    pub async fn cleanup_old_logs(&self) -> P2PResult<()> {
        #[cfg(feature = "persistence")]
        if let Some(db) = &self.db {
            let retention_cutoff = SystemTime::now() - self.config.retention_period;
            let cutoff_timestamp = retention_cutoff
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let mut to_delete = Vec::new();
            let iter = db.iterator(rocksdb::IteratorMode::Start);

            for item in iter {
                let (key, _value) = item
                    .map_err(|e| P2PError::Internal(format!("Database iteration error: {}", e)))?;

                let key_str = String::from_utf8_lossy(&key);
                if let Some(timestamp_str) = key_str.split(':').next() {
                    if let Ok(timestamp) = timestamp_str.parse::<u64>() {
                        if timestamp < cutoff_timestamp {
                            to_delete.push(key.to_vec());
                        }
                    }
                }
            }

            // Delete old entries
            for key in to_delete {
                db.delete(&key).map_err(|e| {
                    P2PError::Internal(format!("Failed to delete old audit log: {}", e))
                })?;
            }

            info!("Cleaned up old audit logs");
        }

        Ok(())
    }

    /// Start background flush task
    async fn start_flush_task(&self) {
        let memory_buffer = self.memory_buffer.clone();
        let config = self.config.clone();
        #[cfg(feature = "persistence")]
        let db = self.db.clone();
        let stats = self.stats.clone();

        tokio::spawn(async move {
            let mut interval = interval(config.flush_interval);

            loop {
                interval.tick().await;

                let events = {
                    let mut buffer = memory_buffer.lock().await;
                    let events: Vec<AuditEvent> = buffer.drain(..).collect();
                    events
                };

                if !events.is_empty() {
                    #[cfg(feature = "persistence")]
                    if let Some(db) = &db {
                        for event in &events {
                            let key = format!(
                                "{}:{}",
                                event
                                    .timestamp
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs(),
                                event.id
                            );
                            if let Ok(value) = serde_json::to_vec(&event) {
                                if let Err(e) = db.put(key.as_bytes(), value) {
                                    error!("Failed to store audit event: {}", e);
                                    if let Ok(mut stats) = stats.try_write() {
                                        stats.errors += 1;
                                    }
                                }
                            }
                        }

                        if let Ok(mut stats) = stats.try_write() {
                            stats.last_flush = Some(SystemTime::now());
                            stats.flush_count += 1;
                        }
                    }
                }
            }
        });
    }

    /// Handle critical events with immediate alerts
    async fn handle_critical_event(&self, event: &AuditEvent) {
        warn!(
            "CRITICAL AUDIT EVENT: {} - {} from {:?}",
            event.action,
            match &event.result {
                AuditResult::Success => "SUCCESS",
                AuditResult::Failure(msg) => msg,
                AuditResult::Blocked(msg) => msg,
                AuditResult::Warning(msg) => msg,
            },
            event.source_ip
        );

        // In production, this could send alerts via email, Slack, etc.
        // For now, we just log at the warning level
    }

    /// Check if severity level should be logged
    fn should_log_severity(&self, severity: &AuditSeverity) -> bool {
        match (&self.config.min_severity, severity) {
            (AuditSeverity::Low, _) => true,
            (AuditSeverity::Medium, AuditSeverity::Low) => false,
            (AuditSeverity::Medium, _) => true,
            (AuditSeverity::High, AuditSeverity::Low | AuditSeverity::Medium) => false,
            (AuditSeverity::High, _) => true,
            (AuditSeverity::Critical, AuditSeverity::Critical) => true,
            (AuditSeverity::Critical, _) => false,
        }
    }
}

// Stub implementation for when persistence feature is disabled
#[cfg(not(feature = "persistence"))]
impl AuditLogger {
    pub async fn query_events(
        &self,
        _start_time: Option<SystemTime>,
        _end_time: Option<SystemTime>,
        _event_type: Option<AuditEventType>,
        _severity: Option<AuditSeverity>,
        _limit: Option<usize>,
    ) -> P2PResult<Vec<AuditEvent>> {
        // Return events from memory buffer only
        let buffer = self.memory_buffer.lock().await;
        Ok(buffer.iter().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[tokio::test]
    async fn test_audit_logger_creation() {
        let config = AuditConfig {
            enable_persistence: false,
            ..Default::default()
        };

        let logger = AuditLogger::new(config).await.unwrap();
        let stats = logger.get_stats().await;
        assert_eq!(stats.total_events, 0);
    }

    #[tokio::test]
    async fn test_auth_event_logging() {
        let config = AuditConfig {
            enable_persistence: false,
            ..Default::default()
        };

        let logger = AuditLogger::new(config).await.unwrap();

        logger
            .log_auth_event(
                Some(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
                Some("test_user".to_string()),
                "login",
                AuditResult::Success,
                serde_json::json!({"method": "jwt"}),
            )
            .await
            .unwrap();

        let stats = logger.get_stats().await;
        assert_eq!(stats.total_events, 1);
        assert_eq!(
            stats.events_by_type.get(&AuditEventType::Authentication),
            Some(&1)
        );
    }

    #[tokio::test]
    async fn test_severity_filtering() {
        let config = AuditConfig {
            enable_persistence: false,
            min_severity: AuditSeverity::High,
            ..Default::default()
        };

        let logger = AuditLogger::new(config).await.unwrap();

        // This should be filtered out (Low severity)
        logger
            .log_auth_event(
                None,
                None,
                "low_severity_action",
                AuditResult::Success,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        // This should be logged (High severity)
        logger
            .log_security_violation(
                Some(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))),
                None,
                "rate_limit_exceeded",
                serde_json::json!({"attempts": 100}),
            )
            .await
            .unwrap();

        let stats = logger.get_stats().await;
        assert_eq!(stats.total_events, 1); // Only the high severity event
    }
}
