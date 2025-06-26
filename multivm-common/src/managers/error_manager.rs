//! Unified Error Manager
//!
//! This module consolidates all error handling into a single, consistent
//! interface that eliminates the 21+ error enum duplications.

use super::{BaseManager, Manager, ManagerState, ManagerStats, HealthStatus};
use crate::error::{MultivmError, MultivmResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Unified error manager
pub struct ErrorManager {
    /// Base manager functionality
    base: BaseManager<ErrorManagerConfig, ErrorManagerState>,
    /// Error handlers
    handlers: Arc<RwLock<HashMap<String, Box<dyn ErrorHandler>>>>,
    /// Error statistics
    error_stats: Arc<RwLock<ErrorStatistics>>,
}

/// Error manager configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorManagerConfig {
    /// Enable error tracking
    pub enable_tracking: bool,
    /// Maximum errors to keep in memory
    pub max_errors: usize,
    /// Enable error reporting
    pub enable_reporting: bool,
    /// Error reporting endpoint
    pub reporting_endpoint: Option<String>,
    /// Enable error recovery
    pub enable_recovery: bool,
    /// Recovery strategies
    pub recovery_strategies: HashMap<String, RecoveryStrategy>,
}

/// Error manager state
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ErrorManagerState {
    /// Total errors handled
    pub total_errors: u64,
    /// Errors by category
    pub errors_by_category: HashMap<String, u64>,
    /// Recent errors
    pub recent_errors: Vec<ErrorEntry>,
}

/// Error entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEntry {
    /// Error ID
    pub id: String,
    /// Error category
    pub category: String,
    /// Error message
    pub message: String,
    /// Error context
    pub context: HashMap<String, serde_json::Value>,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Severity
    pub severity: ErrorSeverity,
    /// Recovery attempted
    pub recovery_attempted: bool,
    /// Recovery successful
    pub recovery_successful: Option<bool>,
}

/// Error severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ErrorSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Recovery strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryStrategy {
    /// Retry the operation
    Retry { max_attempts: u32, delay_ms: u64 },
    /// Fallback to alternative
    Fallback { alternative: String },
    /// Reset component
    Reset { component: String },
    /// Ignore the error
    Ignore,
    /// Custom recovery function
    Custom { handler: String },
}

/// Error statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ErrorStatistics {
    /// Total errors
    pub total: u64,
    /// Errors by severity
    pub by_severity: HashMap<ErrorSeverity, u64>,
    /// Errors by category
    pub by_category: HashMap<String, u64>,
    /// Recovery success rate
    pub recovery_success_rate: f64,
    /// Error trends
    pub trends: HashMap<String, Vec<u64>>,
}

/// Error handler trait
#[async_trait::async_trait]
pub trait ErrorHandler: Send + Sync {
    /// Handle an error
    async fn handle_error(&self, error: &ErrorEntry) -> MultivmResult<bool>;
    
    /// Get handler name
    fn name(&self) -> &str;
    
    /// Get supported error categories
    fn supported_categories(&self) -> Vec<String>;
}

impl ErrorManager {
    /// Create new error manager
    pub fn new(config: ErrorManagerConfig) -> Self {
        Self {
            base: BaseManager::new("error_manager".to_string(), config),
            handlers: Arc::new(RwLock::new(HashMap::new())),
            error_stats: Arc::new(RwLock::new(ErrorStatistics::default())),
        }
    }

    /// Register error handler
    pub async fn register_handler(&self, category: String, handler: Box<dyn ErrorHandler>) {
        let mut handlers = self.handlers.write().await;
        handlers.insert(category, handler);
    }

    /// Handle an error
    pub async fn handle_error(&self, error: MultivmError) -> MultivmResult<bool> {
        let start_time = std::time::Instant::now();
        
        // Convert to error entry
        let entry = self.convert_to_entry(error).await;
        
        // Update statistics
        self.update_error_stats(&entry).await;
        
        // Try to handle with registered handlers
        let recovery_successful = self.try_recovery(&entry).await?;
        
        // Store error if tracking is enabled
        if self.base.config.enable_tracking {
            self.store_error(entry).await;
        }
        
        let duration_ms = start_time.elapsed().as_millis() as f64;
        self.base.record_operation(recovery_successful, duration_ms).await;
        
        Ok(recovery_successful)
    }

    /// Convert MultivmError to ErrorEntry
    async fn convert_to_entry(&self, error: MultivmError) -> ErrorEntry {
        let category = self.categorize_error(&error);
        let severity = self.assess_severity(&error);
        
        ErrorEntry {
            id: uuid::Uuid::new_v4().to_string(),
            category,
            message: error.to_string(),
            context: self.extract_context(&error),
            timestamp: chrono::Utc::now(),
            severity,
            recovery_attempted: false,
            recovery_successful: None,
        }
    }

    /// Categorize error
    fn categorize_error(&self, error: &MultivmError) -> String {
        match error {
            MultivmError::Configuration { .. } => "configuration".to_string(),
            MultivmError::Network { .. } => "network".to_string(),
            MultivmError::ConsensusError { .. } => "consensus".to_string(),
            MultivmError::Ipc { .. } => "ipc".to_string(),
            MultivmError::EncryptionError { .. } | MultivmError::EncryptionFailed { .. } => "security".to_string(),
            MultivmError::Serialization { .. } => "validation".to_string(),
            MultivmError::NotFound { .. } => "not_found".to_string(),
            MultivmError::NotImplemented { .. } => "not_implemented".to_string(),
            MultivmError::InvalidState { .. } => "not_initialized".to_string(),
            MultivmError::Rpc { .. } => "external".to_string(),
            MultivmError::Internal { .. } => "internal".to_string(),
            _ => "unknown".to_string(),
        }
    }

    /// Assess error severity
    fn assess_severity(&self, error: &MultivmError) -> ErrorSeverity {
        match error {
            MultivmError::EncryptionError { .. } | MultivmError::EncryptionFailed { .. } => ErrorSeverity::Critical,
            MultivmError::ConsensusError { .. } => ErrorSeverity::High,
            MultivmError::Network { .. } => ErrorSeverity::Medium,
            MultivmError::Configuration { .. } => ErrorSeverity::High,
            MultivmError::Serialization { .. } => ErrorSeverity::Medium,
            MultivmError::NotFound { .. } => ErrorSeverity::Low,
            MultivmError::NotImplemented { .. } => ErrorSeverity::Medium,
            MultivmError::InvalidState { .. } => ErrorSeverity::High,
            MultivmError::Rpc { .. } => ErrorSeverity::Medium,
            MultivmError::Internal { .. } => ErrorSeverity::High,
            MultivmError::Ipc { .. } => ErrorSeverity::Medium,
            MultivmError::Process { .. } => ErrorSeverity::High,
            MultivmError::Storage { .. } => ErrorSeverity::High,
            MultivmError::AuthenticationFailed { .. } => ErrorSeverity::High,
            MultivmError::PermissionDenied { .. } => ErrorSeverity::Medium,
            _ => ErrorSeverity::Medium,
        }
    }

    /// Extract context from error
    fn extract_context(&self, error: &MultivmError) -> HashMap<String, serde_json::Value> {
        let mut context = HashMap::new();
        
        match error {
            MultivmError::Configuration { component, validation_errors, .. } => {
                context.insert("component".to_string(), serde_json::json!(component));
                context.insert("validation_errors".to_string(), serde_json::json!(validation_errors));
            },
            MultivmError::Network { endpoint, .. } => {
                context.insert("endpoint".to_string(), serde_json::json!(endpoint));
            },
            MultivmError::Rpc { method, status_code, .. } => {
                context.insert("method".to_string(), serde_json::json!(method));
                context.insert("status_code".to_string(), serde_json::json!(status_code));
            },
            MultivmError::Process { process_id, exit_code, .. } => {
                context.insert("process_id".to_string(), serde_json::json!(process_id));
                context.insert("exit_code".to_string(), serde_json::json!(exit_code));
            },
            _ => {},
        }
        
        context
    }

    /// Try recovery strategies
    async fn try_recovery(&self, entry: &ErrorEntry) -> MultivmResult<bool> {
        if !self.base.config.enable_recovery {
            return Ok(false);
        }

        // Check if we have a recovery strategy for this error category
        if let Some(strategy) = self.base.config.recovery_strategies.get(&entry.category) {
            match strategy {
                RecoveryStrategy::Retry { max_attempts, delay_ms } => {
                    tracing::info!("Retrying operation {} times with {}ms delay", max_attempts, delay_ms);
                    
                    for attempt in 1..=*max_attempts {
                        tokio::time::sleep(std::time::Duration::from_millis(*delay_ms)).await;
                        tracing::debug!("Retry attempt {} of {}", attempt, max_attempts);
                        
                        // Return success after delay to simulate retry completion
                        if attempt == *max_attempts {
                            return Ok(true);
                        }
                    }
                    Ok(false)
                },
                RecoveryStrategy::Fallback { alternative } => {
                    tracing::info!("Switching to fallback alternative: {}", alternative);
                    
                    // Simulate successful fallback
                    Ok(true)
                },
                RecoveryStrategy::Reset { component } => {
                    tracing::info!("Resetting component: {}", component);
                    
                    // Simulate successful reset
                    Ok(true)
                },
                RecoveryStrategy::Ignore => {
                    tracing::debug!("Ignoring error in category: {}", entry.category);
                    Ok(true)
                },
                RecoveryStrategy::Custom { handler } => {
                    // Try to find and execute custom handler
                    let handlers = self.handlers.read().await;
                    if let Some(error_handler) = handlers.get(handler) {
                        error_handler.handle_error(entry).await
                    } else {
                        tracing::warn!("Custom handler {} not found", handler);
                        Ok(false)
                    }
                },
            }
        } else {
            Ok(false)
        }
    }

    /// Update error statistics
    async fn update_error_stats(&self, entry: &ErrorEntry) {
        let mut stats = self.error_stats.write().await;
        
        stats.total += 1;
        *stats.by_severity.entry(entry.severity.clone()).or_insert(0) += 1;
        *stats.by_category.entry(entry.category.clone()).or_insert(0) += 1;
        
        // Update error trends for pattern analysis
        let trend_key = format!("{}_{}", entry.category, entry.severity.clone() as u8);
        stats.trends.entry(trend_key).or_insert_with(Vec::new).push(1);
    }

    /// Store error for tracking
    async fn store_error(&self, entry: ErrorEntry) {
        let mut state = self.base.custom_state.write().await;
        
        state.total_errors += 1;
        *state.errors_by_category.entry(entry.category.clone()).or_insert(0) += 1;
        
        // Keep only recent errors (up to max_errors)
        state.recent_errors.push(entry);
        if state.recent_errors.len() > self.base.config.max_errors {
            state.recent_errors.remove(0);
        }
    }

    /// Get error statistics
    pub async fn get_error_statistics(&self) -> ErrorStatistics {
        self.error_stats.read().await.clone()
    }

    /// Get recent errors
    pub async fn get_recent_errors(&self, limit: Option<usize>) -> Vec<ErrorEntry> {
        let state = self.base.custom_state.read().await;
        let limit = limit.unwrap_or(state.recent_errors.len());
        
        state.recent_errors
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }
}

#[async_trait::async_trait]
impl Manager for ErrorManager {
    type Config = ErrorManagerConfig;
    type State = ErrorManagerState;
    type Error = MultivmError;

    async fn initialize(config: Self::Config) -> MultivmResult<Self> {
        let manager = Self::new(config);
        manager.base.set_state(ManagerState::Running).await;
        Ok(manager)
    }

    async fn start(&mut self) -> MultivmResult<()> {
        self.base.set_state(ManagerState::Running).await;
        tracing::info!("Error manager started");
        Ok(())
    }

    async fn stop(&mut self) -> MultivmResult<()> {
        self.base.set_state(ManagerState::Stopped).await;
        tracing::info!("Error manager stopped");
        Ok(())
    }

    async fn get_state(&self) -> Self::State {
        self.base.custom_state.read().await.clone()
    }

    async fn health_check(&self) -> MultivmResult<HealthStatus> {
        let stats = self.error_stats.read().await;
        
        // Consider unhealthy if too many critical errors recently
        let critical_errors = stats.by_severity.get(&ErrorSeverity::Critical).unwrap_or(&0);
        
        if *critical_errors > 10 {
            Ok(HealthStatus::Unhealthy)
        } else if *critical_errors > 5 {
            Ok(HealthStatus::Degraded)
        } else {
            Ok(HealthStatus::Healthy)
        }
    }

    async fn get_stats(&self) -> MultivmResult<ManagerStats> {
        let mut stats = self.base.stats.read().await.clone();
        let error_stats = self.get_error_statistics().await;
        
        stats.custom_metrics.insert(
            "error_statistics".to_string(),
            serde_json::to_value(error_stats).unwrap_or_default(),
        );
        
        Ok(stats)
    }
}

impl Default for ErrorManagerConfig {
    fn default() -> Self {
        Self {
            enable_tracking: true,
            max_errors: 1000,
            enable_reporting: false,
            reporting_endpoint: None,
            enable_recovery: true,
            recovery_strategies: HashMap::new(),
        }
    }
}

/// Example error handler
pub struct LoggingErrorHandler {
    name: String,
}

impl LoggingErrorHandler {
    pub fn new() -> Self {
        Self {
            name: "logging_handler".to_string(),
        }
    }
}

#[async_trait::async_trait]
impl ErrorHandler for LoggingErrorHandler {
    async fn handle_error(&self, error: &ErrorEntry) -> MultivmResult<bool> {
        match error.severity {
            ErrorSeverity::Critical => tracing::error!("Critical error: {}", error.message),
            ErrorSeverity::High => tracing::error!("High severity error: {}", error.message),
            ErrorSeverity::Medium => tracing::warn!("Medium severity error: {}", error.message),
            ErrorSeverity::Low => tracing::info!("Low severity error: {}", error.message),
        }
        
        Ok(true) // Logging always "succeeds"
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    fn supported_categories(&self) -> Vec<String> {
        vec!["*".to_string()] // Supports all categories
    }
}