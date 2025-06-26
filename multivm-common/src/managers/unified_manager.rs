//! Unified Manager Implementation
//!
//! This module provides concrete implementations of the unified manager pattern
//! that can replace the many duplicated manager implementations across the project.

use super::{BaseManager, Manager, ManagerState, ManagerStats, HealthStatus};
use crate::error::{MultivmError, MultivmResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

/// Unified configuration that can handle any manager type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedManagerConfig {
    /// Manager type identifier
    pub manager_type: String,
    /// Manager-specific configuration as JSON
    pub config: serde_json::Value,
    /// Common settings
    pub common: CommonManagerConfig,
}

/// Common configuration for all managers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommonManagerConfig {
    /// Enable health checks
    pub enable_health_checks: bool,
    /// Health check interval in seconds
    pub health_check_interval: u64,
    /// Enable metrics collection
    pub enable_metrics: bool,
    /// Metrics collection interval in seconds
    pub metrics_interval: u64,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Retry delay in milliseconds
    pub retry_delay_ms: u64,
    /// Enable graceful shutdown
    pub enable_graceful_shutdown: bool,
    /// Shutdown timeout in seconds
    pub shutdown_timeout: u64,
}

/// Unified manager state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedManagerState {
    /// Lifecycle state
    pub lifecycle: ManagerState,
    /// Manager-specific state
    pub custom_state: serde_json::Value,
    /// Last health check result
    pub last_health_check: Option<HealthStatus>,
    /// Last error
    pub last_error: Option<String>,
}

/// Unified manager that can handle different types of operations
pub struct UnifiedManager {
    /// Base manager functionality
    base: BaseManager<UnifiedManagerConfig, UnifiedManagerState>,
    /// Operation handlers
    handlers: Arc<RwLock<HashMap<String, Box<dyn OperationHandler>>>>,
    /// Background tasks
    tasks: Arc<RwLock<Vec<tokio::task::JoinHandle<()>>>>,
}

/// Operation handler trait for different manager types
#[async_trait::async_trait]
pub trait OperationHandler: Send + Sync {
    /// Handle an operation
    async fn handle(&self, operation: &str, params: serde_json::Value) -> MultivmResult<serde_json::Value>;
    
    /// Get handler-specific health status
    async fn health_check(&self) -> MultivmResult<HealthStatus>;
    
    /// Initialize the handler
    async fn initialize(&mut self, config: serde_json::Value) -> MultivmResult<()>;
    
    /// Cleanup the handler
    async fn cleanup(&mut self) -> MultivmResult<()>;
}

impl UnifiedManager {
    /// Create new unified manager
    pub fn new(name: String, config: UnifiedManagerConfig) -> Self {
        Self {
            base: BaseManager::new(name, config),
            handlers: Arc::new(RwLock::new(HashMap::new())),
            tasks: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Register an operation handler
    pub async fn register_handler(&self, operation_type: String, handler: Box<dyn OperationHandler>) {
        let mut handlers = self.handlers.write().await;
        handlers.insert(operation_type, handler);
    }

    /// Execute an operation
    pub async fn execute_operation(&self, operation: &str, params: serde_json::Value) -> MultivmResult<serde_json::Value> {
        let start_time = std::time::Instant::now();
        
        let result = {
            let handlers = self.handlers.read().await;
            if let Some(handler) = handlers.get(operation) {
                handler.handle(operation, params).await
            } else {
                Err(MultivmError::NotImplemented {
                    feature: format!("Operation: {}", operation),
                    alternatives: None,
                })
            }
        };

        let duration_ms = start_time.elapsed().as_millis() as f64;
        self.base.record_operation(result.is_ok(), duration_ms).await;

        result
    }

    /// Start background tasks
    async fn start_background_tasks(&self) -> MultivmResult<()> {
        let mut tasks = self.tasks.write().await;
        
        // Health check task
        if self.base.config.common.enable_health_checks {
            let base = self.base.clone();
            let handlers = Arc::clone(&self.handlers);
            let interval = self.base.config.common.health_check_interval;
            
            let task = tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval));
                loop {
                    interval.tick().await;
                    if let Err(e) = Self::perform_health_check(&base, &handlers).await {
                        tracing::error!("Health check failed: {}", e);
                    }
                }
            });
            tasks.push(task);
        }

        // Metrics collection task
        if self.base.config.common.enable_metrics {
            let base = self.base.clone();
            let interval = self.base.config.common.metrics_interval;
            
            let task = tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval));
                loop {
                    interval.tick().await;
                    if let Err(e) = Self::collect_metrics(&base).await {
                        tracing::error!("Metrics collection failed: {}", e);
                    }
                }
            });
            tasks.push(task);
        }

        Ok(())
    }

    /// Perform health check on all handlers
    async fn perform_health_check(
        base: &BaseManager<UnifiedManagerConfig, UnifiedManagerState>,
        handlers: &Arc<RwLock<HashMap<String, Box<dyn OperationHandler>>>>,
    ) -> MultivmResult<()> {
        let handlers_guard = handlers.read().await;
        let mut overall_health = HealthStatus::Healthy;

        for (name, handler) in handlers_guard.iter() {
            match handler.health_check().await {
                Ok(HealthStatus::Healthy) => {},
                Ok(HealthStatus::Degraded) => {
                    if overall_health == HealthStatus::Healthy {
                        overall_health = HealthStatus::Degraded;
                    }
                },
                Ok(HealthStatus::Unhealthy) | Err(_) => {
                    overall_health = HealthStatus::Unhealthy;
                    tracing::warn!("Handler {} is unhealthy", name);
                },
            }
        }

        // Update state with health check result
        base.update_stats(|stats| {
            stats.custom_metrics.insert(
                "last_health_check".to_string(),
                serde_json::json!(overall_health),
            );
        }).await;

        Ok(())
    }

    /// Collect metrics from all components
    async fn collect_metrics(
        base: &BaseManager<UnifiedManagerConfig, UnifiedManagerState>,
    ) -> MultivmResult<()> {
        base.update_stats(|stats| {
            // Update memory usage
            #[cfg(target_os = "linux")]
            {
                if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
                    for line in status.lines() {
                        if line.starts_with("VmRSS:") {
                            if let Some(kb_str) = line.split_whitespace().nth(1) {
                                if let Ok(kb) = kb_str.parse::<u64>() {
                                    stats.memory_usage_bytes = kb * 1024;
                                }
                            }
                            break;
                        }
                    }
                }
            }

            // Add timestamp
            stats.custom_metrics.insert(
                "last_metrics_collection".to_string(),
                serde_json::json!(chrono::Utc::now().timestamp()),
            );
        }).await;

        Ok(())
    }

    /// Stop all background tasks
    async fn stop_background_tasks(&self) -> MultivmResult<()> {
        let mut tasks = self.tasks.write().await;
        
        for task in tasks.drain(..) {
            task.abort();
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl Manager for UnifiedManager {
    type Config = UnifiedManagerConfig;
    type State = UnifiedManagerState;
    type Error = MultivmError;

    async fn initialize(config: Self::Config) -> MultivmResult<Self> {
        let manager = Self::new("unified_manager".to_string(), config);
        
        // Initialize all handlers
        {
            let mut handlers = manager.handlers.write().await;
            for (name, handler) in handlers.iter_mut() {
                if let Err(e) = handler.initialize(manager.base.config.config.clone()).await {
                    tracing::error!("Failed to initialize handler {}: {}", name, e);
                    return Err(e);
                }
            }
        }

        manager.base.set_state(ManagerState::Running).await;
        Ok(manager)
    }

    async fn start(&mut self) -> MultivmResult<()> {
        self.base.set_state(ManagerState::Initializing).await;
        
        // Start background tasks
        self.start_background_tasks().await?;
        
        self.base.set_state(ManagerState::Running).await;
        tracing::info!("Unified manager started successfully");
        
        Ok(())
    }

    async fn stop(&mut self) -> MultivmResult<()> {
        self.base.set_state(ManagerState::Stopping).await;
        
        // Stop background tasks
        self.stop_background_tasks().await?;
        
        // Cleanup all handlers
        let mut handlers = self.handlers.write().await;
        for (name, handler) in handlers.iter_mut() {
            if let Err(e) = handler.cleanup().await {
                tracing::error!("Failed to cleanup handler {}: {}", name, e);
            }
        }
        
        self.base.set_state(ManagerState::Stopped).await;
        tracing::info!("Unified manager stopped successfully");
        
        Ok(())
    }

    async fn get_state(&self) -> Self::State {
        let lifecycle = self.base.get_current_state().await;
        let custom_state = self.base.custom_state.read().await;
        
        UnifiedManagerState {
            lifecycle,
            custom_state: serde_json::json!(custom_state.clone()),
            last_health_check: None, // Would be populated from actual health checks
            last_error: None,
        }
    }

    async fn health_check(&self) -> MultivmResult<HealthStatus> {
        Ok(self.base.calculate_health().await)
    }

    async fn get_stats(&self) -> MultivmResult<ManagerStats> {
        let stats = self.base.stats.read().await;
        Ok(stats.clone())
    }
}

impl Default for CommonManagerConfig {
    fn default() -> Self {
        Self {
            enable_health_checks: true,
            health_check_interval: 30,
            enable_metrics: true,
            metrics_interval: 60,
            max_retries: 3,
            retry_delay_ms: 1000,
            enable_graceful_shutdown: true,
            shutdown_timeout: 30,
        }
    }
}

impl Default for UnifiedManagerState {
    fn default() -> Self {
        Self {
            lifecycle: ManagerState::Uninitialized,
            custom_state: serde_json::Value::Null,
            last_health_check: None,
            last_error: None,
        }
    }
}

/// Convenience function to create a unified manager with default config
pub fn create_unified_manager(manager_type: &str, config: serde_json::Value) -> UnifiedManager {
    let unified_config = UnifiedManagerConfig {
        manager_type: manager_type.to_string(),
        config,
        common: CommonManagerConfig::default(),
    };
    
    UnifiedManager::new(manager_type.to_string(), unified_config)
}

/// Example operation handler for demonstration
pub struct ExampleOperationHandler {
    initialized: bool,
}

impl ExampleOperationHandler {
    pub fn new() -> Self {
        Self { initialized: false }
    }
}

#[async_trait::async_trait]
impl OperationHandler for ExampleOperationHandler {
    async fn handle(&self, operation: &str, params: serde_json::Value) -> MultivmResult<serde_json::Value> {
        if !self.initialized {
            return Err(MultivmError::InvalidState {
                message: "ExampleOperationHandler not initialized".to_string(),
                current_state: Some("uninitialized".to_string()),
                expected_state: Some("initialized".to_string()),
            });
        }

        match operation {
            "ping" => Ok(serde_json::json!({"response": "pong"})),
            "echo" => Ok(params),
            _ => Err(MultivmError::NotImplemented {
                feature: format!("Operation: {}", operation),
                alternatives: None,
            }),
        }
    }

    async fn health_check(&self) -> MultivmResult<HealthStatus> {
        if self.initialized {
            Ok(HealthStatus::Healthy)
        } else {
            Ok(HealthStatus::Unhealthy)
        }
    }

    async fn initialize(&mut self, _config: serde_json::Value) -> MultivmResult<()> {
        self.initialized = true;
        Ok(())
    }

    async fn cleanup(&mut self) -> MultivmResult<()> {
        self.initialized = false;
        Ok(())
    }
}