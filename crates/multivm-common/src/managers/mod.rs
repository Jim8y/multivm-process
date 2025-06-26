//! Comprehensive Manager System
//!
//! This module provides a unified manager pattern that eliminates duplication
//! and provides consistent interfaces across all components.

use crate::{HealthStatus, MultivmResult, ProcessingMetrics};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Common manager trait that all managers should implement
#[async_trait::async_trait]
pub trait Manager: Send + Sync {
    type Config: Clone + Send + Sync;
    type State: Clone + Send + Sync;

    /// Initialize the manager with configuration
    async fn initialize(config: Self::Config) -> MultivmResult<Self>
    where
        Self: Sized;

    /// Start the manager
    async fn start(&mut self) -> MultivmResult<()>;

    /// Stop the manager gracefully
    async fn stop(&mut self) -> MultivmResult<()>;

    /// Get current state
    async fn get_state(&self) -> Self::State;

    /// Health check
    async fn health_check(&self) -> MultivmResult<HealthStatus>;

    /// Get manager statistics
    async fn get_stats(&self) -> MultivmResult<ProcessingMetrics>;
}

/// Manager lifecycle state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ManagerState {
    Uninitialized,
    Initializing,
    Running,
    Stopping,
    Stopped,
    Error(String),
}

/// Manager statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ManagerStats {
    /// Manager name
    pub name: String,
    /// Uptime in seconds
    pub uptime_seconds: u64,
    /// Total operations performed
    pub total_operations: u64,
    /// Successful operations
    pub successful_operations: u64,
    /// Failed operations
    pub failed_operations: u64,
    /// Average operation time in milliseconds
    pub avg_operation_time_ms: f64,
    /// Current active connections/sessions
    pub active_connections: u64,
    /// Memory usage in bytes
    pub memory_usage_bytes: u64,
    /// Custom metrics
    pub custom_metrics: std::collections::HashMap<String, serde_json::Value>,
}

/// Base manager implementation that provides common functionality
#[derive(Debug)]
pub struct BaseManager<C, S> {
    /// Manager name
    pub name: String,
    /// Configuration
    pub config: C,
    /// Current state
    pub state: Arc<RwLock<ManagerState>>,
    /// Statistics
    pub stats: Arc<RwLock<ManagerStats>>,
    /// Start time
    pub start_time: Instant,
    /// Custom state
    pub custom_state: Arc<RwLock<S>>,
}

impl<C, S> BaseManager<C, S>
where
    C: Clone + Send + Sync,
    S: Clone + Send + Sync + Default,
{
    /// Create new base manager
    pub fn new(name: String, config: C) -> Self {
        Self {
            name: name.clone(),
            config,
            state: Arc::new(RwLock::new(ManagerState::Uninitialized)),
            stats: Arc::new(RwLock::new(ManagerStats {
                name,
                ..Default::default()
            })),
            start_time: Instant::now(),
            custom_state: Arc::new(RwLock::new(S::default())),
        }
    }

    /// Update manager state
    pub async fn set_state(&self, new_state: ManagerState) {
        let mut state = self.state.write().await;
        *state = new_state;
    }

    /// Get current state
    pub async fn get_current_state(&self) -> ManagerState {
        self.state.read().await.clone()
    }

    /// Update statistics
    pub async fn update_stats<F>(&self, updater: F)
    where
        F: FnOnce(&mut ManagerStats),
    {
        let mut stats = self.stats.write().await;
        stats.uptime_seconds = self.start_time.elapsed().as_secs();
        updater(&mut stats);
    }

    /// Record operation
    pub async fn record_operation(&self, success: bool, duration_ms: f64) {
        self.update_stats(|stats| {
            stats.total_operations += 1;
            if success {
                stats.successful_operations += 1;
            } else {
                stats.failed_operations += 1;
            }

            // Update average operation time
            let total_ops = stats.total_operations as f64;
            stats.avg_operation_time_ms =
                (stats.avg_operation_time_ms * (total_ops - 1.0) + duration_ms) / total_ops;
        })
        .await;
    }

    /// Get health status based on error rate
    pub async fn calculate_health(&self) -> HealthStatus {
        let stats = self.stats.read().await;

        if stats.total_operations == 0 {
            return HealthStatus::Healthy;
        }

        let error_rate = stats.failed_operations as f64 / stats.total_operations as f64;

        match error_rate {
            rate if rate < 0.01 => HealthStatus::Healthy, // < 1% error rate
            rate if rate < 0.05 => HealthStatus::Degraded, // < 5% error rate
            _ => HealthStatus::Unhealthy,                 // >= 5% error rate
        }
    }

    /// Convert to ProcessingMetrics
    pub async fn to_processing_metrics(&self) -> ProcessingMetrics {
        let stats = self.stats.read().await;
        ProcessingMetrics {
            cpu_time: std::time::Duration::ZERO,
            memory_usage_bytes: stats.memory_usage_bytes,
            disk_reads: 0,
            disk_writes: 0,
            network_bytes: 0,
            compute_units_used: 0,
            transaction_count: 0,
            account_updates: 0,
            total_requests: stats.total_operations,
            successful_requests: stats.successful_operations,
            failed_requests: stats.failed_operations,
            average_response_time_ms: stats.avg_operation_time_ms,
            peak_memory_usage_mb: stats.memory_usage_bytes / 1024 / 1024,
            cpu_usage_percent: 0.0, // Would need system monitoring to get this
        }
    }
}

/// Manager registry for centralized management
#[derive(Debug)]
pub struct ManagerRegistry {
    managers: Arc<RwLock<std::collections::HashMap<String, Arc<dyn std::any::Any + Send + Sync>>>>,
}

impl ManagerRegistry {
    /// Create new manager registry
    pub fn new() -> Self {
        Self {
            managers: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Register a manager
    pub async fn register<M>(&self, name: String, manager: Arc<M>)
    where
        M: 'static + Send + Sync,
    {
        let mut managers = self.managers.write().await;
        managers.insert(name, manager);
    }

    /// Get a manager by name
    pub async fn get<M>(&self, name: &str) -> Option<Arc<M>>
    where
        M: 'static + Send + Sync,
    {
        let managers = self.managers.read().await;
        managers
            .get(name)
            .and_then(|m| m.downcast_ref::<Arc<M>>())
            .cloned()
    }

    /// List all registered managers
    pub async fn list_managers(&self) -> Vec<String> {
        let managers = self.managers.read().await;
        managers.keys().cloned().collect()
    }

    /// Remove a manager
    pub async fn unregister(&self, name: &str) -> bool {
        let mut managers = self.managers.write().await;
        managers.remove(name).is_some()
    }

    /// Get manager count
    pub async fn count(&self) -> usize {
        let managers = self.managers.read().await;
        managers.len()
    }
}

impl Default for ManagerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Global manager registry instance
static MANAGER_REGISTRY: once_cell::sync::Lazy<ManagerRegistry> =
    once_cell::sync::Lazy::new(ManagerRegistry::new);

/// Get the global manager registry
pub fn get_manager_registry() -> &'static ManagerRegistry {
    &MANAGER_REGISTRY
}

/// Simple manager implementation for basic use cases
#[derive(Debug)]
pub struct SimpleManager<C, S> {
    base: BaseManager<C, S>,
    is_running: Arc<RwLock<bool>>,
}

impl<C, S> SimpleManager<C, S>
where
    C: Clone + Send + Sync,
    S: Clone + Send + Sync + Default,
{
    /// Create a new simple manager
    pub fn new(name: String, config: C) -> Self {
        Self {
            base: BaseManager::new(name, config),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// Check if the manager is running
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }

    /// Get the base manager
    pub fn base(&self) -> &BaseManager<C, S> {
        &self.base
    }
}

#[async_trait::async_trait]
impl<C, S> Manager for SimpleManager<C, S>
where
    C: Clone + Send + Sync,
    S: Clone + Send + Sync + Default,
{
    type Config = C;
    type State = S;

    async fn initialize(config: Self::Config) -> MultivmResult<Self> {
        Ok(Self::new("simple_manager".to_string(), config))
    }

    async fn start(&mut self) -> MultivmResult<()> {
        self.base.set_state(ManagerState::Running).await;
        let mut running = self.is_running.write().await;
        *running = true;
        Ok(())
    }

    async fn stop(&mut self) -> MultivmResult<()> {
        self.base.set_state(ManagerState::Stopped).await;
        let mut running = self.is_running.write().await;
        *running = false;
        Ok(())
    }

    async fn get_state(&self) -> Self::State {
        self.base.custom_state.read().await.clone()
    }

    async fn health_check(&self) -> MultivmResult<HealthStatus> {
        if self.is_running().await {
            Ok(self.base.calculate_health().await)
        } else {
            Ok(HealthStatus::Unhealthy)
        }
    }

    async fn get_stats(&self) -> MultivmResult<ProcessingMetrics> {
        Ok(self.base.to_processing_metrics().await)
    }
}

/// Utility functions for manager operations
pub mod utils {
    use super::*;

    /// Create a health check for multiple managers
    pub async fn check_managers_health(
        managers: &[&dyn Manager<Config = (), State = ()>],
    ) -> HealthStatus {
        let mut healthy_count = 0;
        let mut total_count = 0;

        for manager in managers {
            total_count += 1;
            if let Ok(health) = manager.health_check().await {
                if health.is_operational() {
                    healthy_count += 1;
                }
            }
        }

        if total_count == 0 {
            return HealthStatus::Healthy;
        }

        let health_ratio = healthy_count as f64 / total_count as f64;
        match health_ratio {
            ratio if ratio >= 0.9 => HealthStatus::Healthy,
            ratio if ratio >= 0.5 => HealthStatus::Degraded,
            _ => HealthStatus::Unhealthy,
        }
    }

    /// Aggregate statistics from multiple managers
    pub async fn aggregate_stats(
        managers: &[&dyn Manager<Config = (), State = ()>],
    ) -> ProcessingMetrics {
        let mut total_metrics = ProcessingMetrics::default();

        for manager in managers {
            if let Ok(metrics) = manager.get_stats().await {
                total_metrics.total_requests += metrics.total_requests;
                total_metrics.successful_requests += metrics.successful_requests;
                total_metrics.failed_requests += metrics.failed_requests;
                total_metrics.peak_memory_usage_mb = total_metrics
                    .peak_memory_usage_mb
                    .max(metrics.peak_memory_usage_mb);
                total_metrics.cpu_usage_percent = total_metrics
                    .cpu_usage_percent
                    .max(metrics.cpu_usage_percent);
            }
        }

        // Recalculate average response time
        if total_metrics.total_requests > 0 {
            // Calculate average response time across all managers
            total_metrics.average_response_time_ms /= managers.len() as f64;
        }

        total_metrics
    }
}

/// Convenience macro for implementing the Manager trait
#[macro_export]
macro_rules! impl_simple_manager {
    ($manager_type:ty, $config_type:ty, $state_type:ty) => {
        #[async_trait::async_trait]
        impl Manager for $manager_type {
            type Config = $config_type;
            type State = $state_type;

            async fn initialize(config: Self::Config) -> MultivmResult<Self> {
                // Default implementation - override as needed
                Ok(Self::new(config))
            }

            async fn start(&mut self) -> MultivmResult<()> {
                // Default implementation - override as needed
                Ok(())
            }

            async fn stop(&mut self) -> MultivmResult<()> {
                // Default implementation - override as needed
                Ok(())
            }

            async fn get_state(&self) -> Self::State {
                // Default implementation - override as needed
                Default::default()
            }

            async fn health_check(&self) -> MultivmResult<HealthStatus> {
                // Default implementation
                Ok(HealthStatus::Healthy)
            }

            async fn get_stats(&self) -> MultivmResult<ProcessingMetrics> {
                // Default implementation
                Ok(ProcessingMetrics::default())
            }
        }
    };
}
