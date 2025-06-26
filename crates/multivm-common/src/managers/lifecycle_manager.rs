//! Lifecycle Manager
//!
//! This module provides unified lifecycle management for all components,
//! eliminating duplicated startup/shutdown logic across the project.

use super::{BaseManager, Manager, ManagerState, ManagerStats, HealthStatus};
use crate::error::{MultivmError, MultivmResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Lifecycle manager that coordinates startup and shutdown of all components
pub struct LifecycleManager {
    /// Base manager functionality
    base: BaseManager<LifecycleManagerConfig, LifecycleManagerState>,
    /// Registered components
    components: Arc<RwLock<HashMap<String, Box<dyn LifecycleComponent>>>>,
    /// Dependency graph
    dependencies: Arc<RwLock<HashMap<String, Vec<String>>>>,
    /// Shutdown hooks
    shutdown_hooks: Arc<RwLock<Vec<Box<dyn ShutdownHook>>>>,
}

/// Lifecycle manager configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleManagerConfig {
    /// Startup timeout in seconds
    pub startup_timeout: u64,
    /// Shutdown timeout in seconds
    pub shutdown_timeout: u64,
    /// Enable graceful shutdown
    pub enable_graceful_shutdown: bool,
    /// Enable dependency resolution
    pub enable_dependency_resolution: bool,
    /// Maximum startup retries
    pub max_startup_retries: u32,
    /// Startup retry delay in milliseconds
    pub startup_retry_delay_ms: u64,
}

/// Lifecycle manager state
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LifecycleManagerState {
    /// Current phase
    pub current_phase: LifecyclePhase,
    /// Component states
    pub component_states: HashMap<String, ComponentState>,
    /// Startup order
    pub startup_order: Vec<String>,
    /// Shutdown order
    pub shutdown_order: Vec<String>,
    /// Last operation
    pub last_operation: Option<String>,
}

/// Lifecycle phases
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LifecyclePhase {
    Uninitialized,
    Starting,
    Running,
    Stopping,
    Stopped,
    Error,
}

/// Component state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentState {
    /// Component name
    pub name: String,
    /// Current state
    pub state: LifecyclePhase,
    /// Start time
    pub start_time: Option<chrono::DateTime<chrono::Utc>>,
    /// Stop time
    pub stop_time: Option<chrono::DateTime<chrono::Utc>>,
    /// Last error
    pub last_error: Option<String>,
    /// Restart count
    pub restart_count: u32,
}

/// Lifecycle component trait
#[async_trait::async_trait]
pub trait LifecycleComponent: Send + Sync {
    /// Component name
    fn name(&self) -> &str;
    
    /// Component dependencies
    fn dependencies(&self) -> Vec<String> {
        vec![]
    }
    
    /// Initialize the component
    async fn initialize(&mut self) -> MultivmResult<()>;
    
    /// Start the component
    async fn start(&mut self) -> MultivmResult<()>;
    
    /// Stop the component
    async fn stop(&mut self) -> MultivmResult<()>;
    
    /// Health check
    async fn health_check(&self) -> MultivmResult<HealthStatus>;
    
    /// Get component state
    async fn get_state(&self) -> MultivmResult<serde_json::Value>;
}

/// Shutdown hook trait
#[async_trait::async_trait]
pub trait ShutdownHook: Send + Sync {
    /// Execute shutdown hook
    async fn execute(&self) -> MultivmResult<()>;
    
    /// Hook priority (lower numbers execute first)
    fn priority(&self) -> u32 {
        100
    }
    
    /// Hook name
    fn name(&self) -> &str;
}

impl LifecycleManager {
    /// Create new lifecycle manager
    pub fn new(config: LifecycleManagerConfig) -> Self {
        Self {
            base: BaseManager::new("lifecycle_manager".to_string(), config),
            components: Arc::new(RwLock::new(HashMap::new())),
            dependencies: Arc::new(RwLock::new(HashMap::new())),
            shutdown_hooks: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Register a component
    pub async fn register_component(&self, component: Box<dyn LifecycleComponent>) -> MultivmResult<()> {
        let name = component.name().to_string();
        let dependencies = component.dependencies();
        
        // Store component
        let mut components = self.components.write().await;
        components.insert(name.clone(), component);
        
        // Store dependencies
        let mut deps = self.dependencies.write().await;
        deps.insert(name.clone(), dependencies);
        
        // Update state
        let mut state = self.base.custom_state.write().await;
        state.component_states.insert(name.clone(), ComponentState {
            name,
            state: LifecyclePhase::Uninitialized,
            start_time: None,
            stop_time: None,
            last_error: None,
            restart_count: 0,
        });
        
        Ok(())
    }

    /// Register shutdown hook
    pub async fn register_shutdown_hook(&self, hook: Box<dyn ShutdownHook>) {
        let mut hooks = self.shutdown_hooks.write().await;
        hooks.push(hook);
        
        // Sort by priority
        hooks.sort_by_key(|h| h.priority());
    }

    /// Start all components
    pub async fn start_all(&self) -> MultivmResult<()> {
        let start_time = std::time::Instant::now();
        
        // Update phase
        let mut state = self.base.custom_state.write().await;
        state.current_phase = LifecyclePhase::Starting;
        state.last_operation = Some("start_all".to_string());
        drop(state);

        // Resolve startup order
        let startup_order = if self.base.config.enable_dependency_resolution {
            self.resolve_startup_order().await?
        } else {
            self.components.read().await.keys().cloned().collect()
        };

        // Start components in order
        for component_name in &startup_order {
            if let Err(e) = self.start_component(component_name).await {
                tracing::error!("Failed to start component {}: {}", component_name, e);
                
                // Update state
                let mut state = self.base.custom_state.write().await;
                state.current_phase = LifecyclePhase::Error;
                
                return Err(e);
            }
        }

        // Update state
        let mut state = self.base.custom_state.write().await;
        state.current_phase = LifecyclePhase::Running;
        state.startup_order = startup_order;

        let duration_ms = start_time.elapsed().as_millis() as f64;
        self.base.record_operation(true, duration_ms).await;

        tracing::info!("All components started successfully");
        Ok(())
    }

    /// Stop all components
    pub async fn stop_all(&self) -> MultivmResult<()> {
        let start_time = std::time::Instant::now();
        
        // Update phase
        let mut state = self.base.custom_state.write().await;
        state.current_phase = LifecyclePhase::Stopping;
        state.last_operation = Some("stop_all".to_string());
        
        // Get shutdown order (reverse of startup order)
        let shutdown_order: Vec<String> = state.startup_order.iter().rev().cloned().collect();
        state.shutdown_order = shutdown_order.clone();
        drop(state);

        // Execute shutdown hooks first
        self.execute_shutdown_hooks().await?;

        // Stop components in reverse order
        for component_name in &shutdown_order {
            if let Err(e) = self.stop_component(component_name).await {
                tracing::error!("Failed to stop component {}: {}", component_name, e);
                // Continue stopping other components even if one fails
            }
        }

        // Update state
        let mut state = self.base.custom_state.write().await;
        state.current_phase = LifecyclePhase::Stopped;

        let duration_ms = start_time.elapsed().as_millis() as f64;
        self.base.record_operation(true, duration_ms).await;

        tracing::info!("All components stopped");
        Ok(())
    }

    /// Start a specific component
    async fn start_component(&self, name: &str) -> MultivmResult<()> {
        let mut components = self.components.write().await;
        
        if let Some(component) = components.get_mut(name) {
            // Update component state
            self.update_component_state(name, LifecyclePhase::Starting, None).await;
            
            // Initialize and start
            match component.initialize().await {
                Ok(()) => {
                    match component.start().await {
                        Ok(()) => {
                            self.update_component_state(name, LifecyclePhase::Running, None).await;
                            tracing::info!("Component {} started successfully", name);
                            Ok(())
                        },
                        Err(e) => {
                            let error_msg = format!("Failed to start component {}: {}", name, e);
                            self.update_component_state(name, LifecyclePhase::Error, Some(error_msg.clone())).await;
                            Err(MultivmError::Internal {
                                message: error_msg,
                                component: name.to_string(),
                                error_code: None,
                            })
                        }
                    }
                },
                Err(e) => {
                    let error_msg = format!("Failed to initialize component {}: {}", name, e);
                    self.update_component_state(name, LifecyclePhase::Error, Some(error_msg.clone())).await;
                    Err(MultivmError::Internal {
                        message: error_msg,
                        component: name.to_string(),
                        error_code: None,
                    })
                }
            }
        } else {
            Err(MultivmError::NotFound {
                resource: "component".to_string(),
                resource_id: Some(name.to_string()),
            })
        }
    }

    /// Stop a specific component
    async fn stop_component(&self, name: &str) -> MultivmResult<()> {
        let mut components = self.components.write().await;
        
        if let Some(component) = components.get_mut(name) {
            self.update_component_state(name, LifecyclePhase::Stopping, None).await;
            
            match component.stop().await {
                Ok(()) => {
                    self.update_component_state(name, LifecyclePhase::Stopped, None).await;
                    tracing::info!("Component {} stopped successfully", name);
                    Ok(())
                },
                Err(e) => {
                    let error_msg = format!("Failed to stop component {}: {}", name, e);
                    self.update_component_state(name, LifecyclePhase::Error, Some(error_msg.clone())).await;
                    Err(MultivmError::Internal {
                        message: error_msg,
                        component: name.to_string(),
                        error_code: None,
                    })
                }
            }
        } else {
            Err(MultivmError::NotFound {
                resource: "component".to_string(),
                resource_id: Some(name.to_string()),
            })
        }
    }

    /// Update component state
    async fn update_component_state(&self, name: &str, state: LifecyclePhase, error: Option<String>) {
        let mut manager_state = self.base.custom_state.write().await;
        
        if let Some(component_state) = manager_state.component_states.get_mut(name) {
            component_state.state = state.clone();
            component_state.last_error = error;
            
            match state {
                LifecyclePhase::Running => {
                    component_state.start_time = Some(chrono::Utc::now());
                },
                LifecyclePhase::Stopped => {
                    component_state.stop_time = Some(chrono::Utc::now());
                },
                _ => {},
            }
        }
    }

    /// Resolve startup order based on dependencies
    async fn resolve_startup_order(&self) -> MultivmResult<Vec<String>> {
        let dependencies = self.dependencies.read().await;
        let mut order = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut visiting = std::collections::HashSet::new();

        // Topological sort
        for component in dependencies.keys() {
            if !visited.contains(component) {
                self.visit_component(component, &dependencies, &mut order, &mut visited, &mut visiting)?;
            }
        }

        Ok(order)
    }

    /// Visit component for topological sort
    fn visit_component(
        &self,
        component: &str,
        dependencies: &HashMap<String, Vec<String>>,
        order: &mut Vec<String>,
        visited: &mut std::collections::HashSet<String>,
        visiting: &mut std::collections::HashSet<String>,
    ) -> MultivmResult<()> {
        if visiting.contains(component) {
            return Err(MultivmError::Configuration {
                component: "lifecycle_manager".to_string(),
                message: format!("Circular dependency detected involving component: {}", component),
                validation_errors: Some(vec![]),
            });
        }

        if visited.contains(component) {
            return Ok(());
        }

        visiting.insert(component.to_string());

        if let Some(deps) = dependencies.get(component) {
            for dep in deps {
                self.visit_component(dep, dependencies, order, visited, visiting)?;
            }
        }

        visiting.remove(component);
        visited.insert(component.to_string());
        order.push(component.to_string());

        Ok(())
    }

    /// Execute shutdown hooks
    async fn execute_shutdown_hooks(&self) -> MultivmResult<()> {
        let hooks = self.shutdown_hooks.read().await;
        
        for hook in hooks.iter() {
            if let Err(e) = hook.execute().await {
                tracing::error!("Shutdown hook {} failed: {}", hook.name(), e);
                // Continue with other hooks even if one fails
            } else {
                tracing::debug!("Shutdown hook {} executed successfully", hook.name());
            }
        }
        
        Ok(())
    }

    /// Get component health status
    pub async fn get_component_health(&self, name: &str) -> MultivmResult<HealthStatus> {
        let components = self.components.read().await;
        
        if let Some(component) = components.get(name) {
            component.health_check().await
        } else {
            Err(MultivmError::NotFound {
                resource: "component".to_string(),
                resource_id: Some(name.to_string()),
            })
        }
    }

    /// Get overall system health
    pub async fn get_system_health(&self) -> HealthStatus {
        let components = self.components.read().await;
        let mut healthy_count = 0;
        let mut total_count = 0;

        for component in components.values() {
            total_count += 1;
            if let Ok(HealthStatus::Healthy) = component.health_check().await {
                healthy_count += 1;
            }
        }

        if total_count == 0 {
            HealthStatus::Unhealthy
        } else if healthy_count == total_count {
            HealthStatus::Healthy
        } else if healthy_count > total_count / 2 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Unhealthy
        }
    }
}

#[async_trait::async_trait]
impl Manager for LifecycleManager {
    type Config = LifecycleManagerConfig;
    type State = LifecycleManagerState;
    type Error = MultivmError;

    async fn initialize(config: Self::Config) -> MultivmResult<Self> {
        let manager = Self::new(config);
        manager.base.set_state(ManagerState::Running).await;
        Ok(manager)
    }

    async fn start(&mut self) -> MultivmResult<()> {
        self.start_all().await
    }

    async fn stop(&mut self) -> MultivmResult<()> {
        self.stop_all().await
    }

    async fn get_state(&self) -> Self::State {
        self.base.custom_state.read().await.clone()
    }

    async fn health_check(&self) -> MultivmResult<HealthStatus> {
        Ok(self.get_system_health().await)
    }

    async fn get_stats(&self) -> MultivmResult<ManagerStats> {
        let mut stats = self.base.stats.read().await.clone();
        let state = self.get_state().await;
        
        stats.custom_metrics.insert(
            "lifecycle_state".to_string(),
            serde_json::to_value(state).unwrap_or_default(),
        );
        
        Ok(stats)
    }
}

impl Default for LifecycleManagerConfig {
    fn default() -> Self {
        Self {
            startup_timeout: 300,
            shutdown_timeout: 60,
            enable_graceful_shutdown: true,
            enable_dependency_resolution: true,
            max_startup_retries: 3,
            startup_retry_delay_ms: 1000,
        }
    }
}

impl Default for LifecyclePhase {
    fn default() -> Self {
        LifecyclePhase::Uninitialized
    }
}