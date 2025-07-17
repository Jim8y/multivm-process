//! Health checking service for RPC relays

use crate::{
    relay::RpcRelay,
    types::{HealthStatus, NodeHealth},
};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tokio::{sync::RwLock, time::interval};
use tracing::{debug, error, info, warn};

/// Health checker for monitoring relay node status
pub struct HealthChecker {
    /// Health check results
    health_results: Arc<RwLock<HashMap<String, NodeHealth>>>,
    /// Health check interval
    check_interval: Duration,
    /// Health check timeout
    check_timeout: Duration,
    /// Threshold for marking node as unhealthy
    failure_threshold: u32,
    /// Failure counts for each node
    failure_counts: Arc<RwLock<HashMap<String, u32>>>,
}

impl HealthChecker {
    pub fn new() -> Self {
        Self {
            health_results: Arc::new(RwLock::new(HashMap::new())),
            check_interval: Duration::from_secs(30),
            check_timeout: Duration::from_secs(5),
            failure_threshold: 3,
            failure_counts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn with_config(
        check_interval: Duration,
        check_timeout: Duration,
        failure_threshold: u32,
    ) -> Self {
        Self {
            health_results: Arc::new(RwLock::new(HashMap::new())),
            check_interval,
            check_timeout,
            failure_threshold,
            failure_counts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start monitoring relays in background
    pub async fn start_monitoring(&self, relays: Vec<Arc<dyn RpcRelay>>) {
        info!("Starting health monitoring for {} relays", relays.len());

        let health_results = self.health_results.clone();
        let failure_counts = self.failure_counts.clone();
        let check_interval = self.check_interval;
        let check_timeout = self.check_timeout;
        let failure_threshold = self.failure_threshold;

        tokio::spawn(async move {
            let mut interval_timer = interval(check_interval);

            loop {
                interval_timer.tick().await;
                
                debug!("Performing health checks for {} relays", relays.len());
                
                // Check all relays concurrently
                let mut check_tasks = Vec::new();
                
                for relay in &relays {
                    let relay_clone = relay.clone();
                    let health_results_clone = health_results.clone();
                    let failure_counts_clone = failure_counts.clone();
                    
                    let task = tokio::spawn(async move {
                        Self::check_single_relay(
                            relay_clone,
                            health_results_clone,
                            failure_counts_clone,
                            check_timeout,
                            failure_threshold,
                        )
                        .await;
                    });
                    
                    check_tasks.push(task);
                }

                // Wait for all health checks to complete
                for task in check_tasks {
                    if let Err(e) = task.await {
                        error!("Health check task failed: {}", e);
                    }
                }

                debug!("Health check round completed");
            }
        });
    }

    /// Check single relay health
    async fn check_single_relay(
        relay: Arc<dyn RpcRelay>,
        health_results: Arc<RwLock<HashMap<String, NodeHealth>>>,
        failure_counts: Arc<RwLock<HashMap<String, u32>>>,
        timeout: Duration,
        failure_threshold: u32,
    ) {
        let node_name = relay.config().name.clone();
        
        debug!("Checking health of relay: {}", node_name);

        // Perform health check with timeout
        let health_result = tokio::time::timeout(timeout, relay.get_health()).await;

        let health = match health_result {
            Ok(health) => health,
            Err(_) => {
                warn!("Health check timeout for relay: {}", node_name);
                NodeHealth {
                    name: node_name.clone(),
                    vm_type: relay.vm_type(),
                    status: HealthStatus::Unhealthy,
                    response_time_ms: timeout.as_millis() as u64,
                    last_success: SystemTime::UNIX_EPOCH,
                    error_message: Some("Health check timeout".to_string()),
                    version_info: None,
                }
            }
        };

        // Update failure count
        let mut failure_counts_lock = failure_counts.write().await;
        let current_failures = failure_counts_lock.get(&node_name).copied().unwrap_or(0);

        let new_failures = match health.status {
            HealthStatus::Healthy => {
                if current_failures > 0 {
                    info!("Relay {} recovered (was {} failures)", node_name, current_failures);
                }
                0
            }
            HealthStatus::Degraded => {
                warn!("Relay {} is degraded", node_name);
                current_failures + 1
            }
            HealthStatus::Unhealthy => {
                error!("Relay {} is unhealthy", node_name);
                current_failures + 1
            }
            HealthStatus::Unknown => current_failures,
        };

        failure_counts_lock.insert(node_name.clone(), new_failures);
        drop(failure_counts_lock);

        // Determine final status based on failure count
        let final_status = if new_failures >= failure_threshold {
            warn!("Relay {} exceeded failure threshold ({}/{})", 
                  node_name, new_failures, failure_threshold);
            HealthStatus::Unhealthy
        } else {
            health.status
        };

        let final_health = NodeHealth {
            status: final_status,
            ..health
        };

        // Store health result
        health_results.write().await.insert(node_name, final_health);
    }

    /// Get health status for specific relay
    pub async fn get_relay_health(&self, relay_name: &str) -> Option<NodeHealth> {
        self.health_results.read().await.get(relay_name).cloned()
    }

    /// Get health status for all relays
    pub async fn get_all_health(&self) -> HashMap<String, NodeHealth> {
        self.health_results.read().await.clone()
    }

    /// Get healthy relays
    pub async fn get_healthy_relays(&self) -> Vec<String> {
        self.health_results
            .read()
            .await
            .iter()
            .filter_map(|(name, health)| {
                if matches!(health.status, HealthStatus::Healthy) {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get unhealthy relays
    pub async fn get_unhealthy_relays(&self) -> Vec<String> {
        self.health_results
            .read()
            .await
            .iter()
            .filter_map(|(name, health)| {
                if matches!(health.status, HealthStatus::Unhealthy) {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Force health check for specific relay
    pub async fn force_check(&self, relay: Arc<dyn RpcRelay>) {
        let node_name = relay.config().name.clone();
        info!("Forcing health check for relay: {}", node_name);

        Self::check_single_relay(
            relay,
            self.health_results.clone(),
            self.failure_counts.clone(),
            self.check_timeout,
            self.failure_threshold,
        )
        .await;
    }

    /// Reset failure count for a relay
    pub async fn reset_failures(&self, relay_name: &str) {
        info!("Resetting failure count for relay: {}", relay_name);
        self.failure_counts.write().await.insert(relay_name.to_string(), 0);
    }

    /// Get health check statistics
    pub async fn get_stats(&self) -> HealthCheckStats {
        let health_results = self.health_results.read().await;
        let failure_counts = self.failure_counts.read().await;

        let total_relays = health_results.len();
        let mut healthy_count = 0;
        let mut degraded_count = 0;
        let mut unhealthy_count = 0;
        let mut unknown_count = 0;
        let mut total_response_time = 0u64;
        let mut max_response_time = 0u64;
        let mut min_response_time = u64::MAX;

        for health in health_results.values() {
            match health.status {
                HealthStatus::Healthy => healthy_count += 1,
                HealthStatus::Degraded => degraded_count += 1,
                HealthStatus::Unhealthy => unhealthy_count += 1,
                HealthStatus::Unknown => unknown_count += 1,
            }

            total_response_time += health.response_time_ms;
            max_response_time = max_response_time.max(health.response_time_ms);
            if health.response_time_ms > 0 {
                min_response_time = min_response_time.min(health.response_time_ms);
            }
        }

        if min_response_time == u64::MAX {
            min_response_time = 0;
        }

        let avg_response_time = if total_relays > 0 {
            total_response_time / total_relays as u64
        } else {
            0
        };

        let total_failures: u32 = failure_counts.values().sum();

        HealthCheckStats {
            total_relays,
            healthy_count,
            degraded_count,
            unhealthy_count,
            unknown_count,
            avg_response_time,
            min_response_time,
            max_response_time,
            total_failures,
            check_interval: self.check_interval,
            failure_threshold: self.failure_threshold,
        }
    }

    /// Update configuration
    pub fn update_config(
        &mut self,
        check_interval: Option<Duration>,
        check_timeout: Option<Duration>,
        failure_threshold: Option<u32>,
    ) {
        if let Some(interval) = check_interval {
            self.check_interval = interval;
            info!("Updated health check interval to {:?}", interval);
        }

        if let Some(timeout) = check_timeout {
            self.check_timeout = timeout;
            info!("Updated health check timeout to {:?}", timeout);
        }

        if let Some(threshold) = failure_threshold {
            self.failure_threshold = threshold;
            info!("Updated failure threshold to {}", threshold);
        }
    }
}

/// Health check statistics
#[derive(Debug, Clone)]
pub struct HealthCheckStats {
    pub total_relays: usize,
    pub healthy_count: usize,
    pub degraded_count: usize,
    pub unhealthy_count: usize,
    pub unknown_count: usize,
    pub avg_response_time: u64,
    pub min_response_time: u64,
    pub max_response_time: u64,
    pub total_failures: u32,
    pub check_interval: Duration,
    pub failure_threshold: u32,
}

impl HealthCheckStats {
    /// Calculate health percentage
    pub fn health_percentage(&self) -> f64 {
        if self.total_relays == 0 {
            return 100.0;
        }
        
        (self.healthy_count as f64 / self.total_relays as f64) * 100.0
    }

    /// Check if cluster is healthy
    pub fn is_cluster_healthy(&self) -> bool {
        self.healthy_count > 0 && (self.healthy_count + self.degraded_count) > self.unhealthy_count
    }

    /// Get summary string
    pub fn summary(&self) -> String {
        format!(
            "Health: {:.1}% ({}/{} healthy, avg response: {}ms)",
            self.health_percentage(),
            self.healthy_count,
            self.total_relays,
            self.avg_response_time
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::NodeConfig,
        types::{HealthStatus, NodeHealth, VmType},
        relay::RpcRelay,
    };
    use async_trait::async_trait;
    use std::time::SystemTime;

    struct MockRelay {
        name: String,
        health_status: HealthStatus,
        response_time: u64,
        config: NodeConfig,
    }

    impl MockRelay {
        fn new(name: String, health_status: HealthStatus, response_time: u64) -> Self {
            let config = NodeConfig {
                name: name.clone(),
                url: "http://localhost:8545".parse().unwrap(),
                timeout: Duration::from_secs(5),
                max_concurrent_requests: 10,
                health_check_interval: Duration::from_secs(30),
                priority: 100,
                auth: None,
            };

            Self {
                name,
                health_status,
                response_time,
                config,
            }
        }
    }

    #[async_trait]
    impl RpcRelay for MockRelay {
        async fn forward_request(&self, _request: crate::types::JsonRpcRequest) -> crate::error::RpcResult<crate::types::JsonRpcResponse> {
            unimplemented!()
        }

        async fn get_health(&self) -> NodeHealth {
            // Simulate some delay
            tokio::time::sleep(Duration::from_millis(self.response_time)).await;
            
            NodeHealth {
                name: self.name.clone(),
                vm_type: VmType::Ethereum,
                status: self.health_status,
                response_time_ms: self.response_time,
                last_success: if self.health_status == HealthStatus::Healthy {
                    SystemTime::now()
                } else {
                    SystemTime::UNIX_EPOCH
                },
                error_message: if self.health_status != HealthStatus::Healthy {
                    Some("Mock error".to_string())
                } else {
                    None
                },
                version_info: Some("mock-version".to_string()),
            }
        }

        fn vm_type(&self) -> VmType {
            VmType::Ethereum
        }

        fn config(&self) -> &NodeConfig {
            &self.config
        }
    }

    #[tokio::test]
    async fn test_health_checker_basic() {
        let checker = HealthChecker::with_config(
            Duration::from_millis(100),  // Fast check interval for testing
            Duration::from_secs(1),
            2, // Low failure threshold for testing
        );

        let relays: Vec<Arc<dyn RpcRelay>> = vec![
            Arc::new(MockRelay::new("healthy".to_string(), HealthStatus::Healthy, 10)),
            Arc::new(MockRelay::new("unhealthy".to_string(), HealthStatus::Unhealthy, 50)),
        ];

        checker.start_monitoring(relays).await;

        // Wait for a few checks
        tokio::time::sleep(Duration::from_millis(250)).await;

        let stats = checker.get_stats().await;
        assert!(stats.total_relays > 0);
        
        let healthy_relays = checker.get_healthy_relays().await;
        assert!(healthy_relays.contains(&"healthy".to_string()));
        
        let unhealthy_relays = checker.get_unhealthy_relays().await;
        assert!(unhealthy_relays.contains(&"unhealthy".to_string()));
    }

    #[tokio::test]
    async fn test_failure_threshold() {
        let checker = HealthChecker::with_config(
            Duration::from_millis(50),
            Duration::from_secs(1),
            2, // 2 failures before marking unhealthy
        );

        // This relay will be unhealthy and should trigger failure count
        let relays: Vec<Arc<dyn RpcRelay>> = vec![
            Arc::new(MockRelay::new("failing".to_string(), HealthStatus::Unhealthy, 10)),
        ];

        checker.start_monitoring(relays).await;

        // Wait for multiple check cycles
        tokio::time::sleep(Duration::from_millis(200)).await;

        let stats = checker.get_stats().await;
        assert!(stats.total_failures >= 2);
    }

    #[tokio::test]
    async fn test_force_check() {
        let checker = HealthChecker::new();
        
        let relay = Arc::new(MockRelay::new(
            "test".to_string(),
            HealthStatus::Healthy,
            25,
        ));

        // Force a health check
        checker.force_check(relay).await;

        let health = checker.get_relay_health("test").await;
        assert!(health.is_some());
        assert_eq!(health.unwrap().status, HealthStatus::Healthy);
    }

    #[test]
    fn test_health_stats() {
        let stats = HealthCheckStats {
            total_relays: 4,
            healthy_count: 3,
            degraded_count: 1,
            unhealthy_count: 0,
            unknown_count: 0,
            avg_response_time: 25,
            min_response_time: 10,
            max_response_time: 50,
            total_failures: 2,
            check_interval: Duration::from_secs(30),
            failure_threshold: 3,
        };

        assert_eq!(stats.health_percentage(), 75.0);
        assert!(stats.is_cluster_healthy());
        assert!(stats.summary().contains("75.0%"));
    }
}