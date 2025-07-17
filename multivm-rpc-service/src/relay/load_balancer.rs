//! Load balancer for RPC relay selection

use crate::{
    error::{RpcError, RpcResult},
    relay::RpcRelay,
    types::{HealthStatus, JsonRpcRequest, JsonRpcResponse, LoadBalanceStrategy},
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tracing::{debug, warn};

/// Load balancer for selecting appropriate relay nodes
pub struct LoadBalancer {
    round_robin_counter: AtomicUsize,
    strategy: LoadBalanceStrategy,
}

impl LoadBalancer {
    pub fn new() -> Self {
        Self {
            round_robin_counter: AtomicUsize::new(0),
            strategy: LoadBalanceStrategy::Priority,
        }
    }

    pub fn with_strategy(strategy: LoadBalanceStrategy) -> Self {
        Self {
            round_robin_counter: AtomicUsize::new(0),
            strategy,
        }
    }

    /// Select best relay from available nodes
    pub async fn select_relay(
        &self,
        relays: &[Arc<dyn RpcRelay>],
        request: &JsonRpcRequest,
    ) -> RpcResult<Arc<dyn RpcRelay>> {
        if relays.is_empty() {
            return Err(RpcError::BackendUnavailable {
                backend: "all".to_string(),
            });
        }

        // Filter healthy relays
        let healthy_relays = self.filter_healthy_relays(relays).await;
        
        if healthy_relays.is_empty() {
            warn!("No healthy relays available, falling back to all relays");
            // Fallback to all relays if no healthy ones
            return self.select_from_relays(relays, request).await;
        }

        self.select_from_relays(&healthy_relays, request).await
    }

    /// Filter relays by health status
    async fn filter_healthy_relays(
        &self,
        relays: &[Arc<dyn RpcRelay>],
    ) -> Vec<Arc<dyn RpcRelay>> {
        let mut healthy_relays = Vec::new();

        for relay in relays {
            let health = relay.get_health().await;
            if matches!(health.status, HealthStatus::Healthy | HealthStatus::Degraded) {
                healthy_relays.push(relay.clone());
            }
        }

        healthy_relays
    }

    /// Select relay from filtered list using configured strategy
    async fn select_from_relays(
        &self,
        relays: &[Arc<dyn RpcRelay>],
        request: &JsonRpcRequest,
    ) -> RpcResult<Arc<dyn RpcRelay>> {
        match self.strategy {
            LoadBalanceStrategy::RoundRobin => self.round_robin_selection(relays),
            LoadBalanceStrategy::Priority => self.priority_selection(relays).await,
            LoadBalanceStrategy::LeastConnections => self.least_connections_selection(relays).await,
            LoadBalanceStrategy::ResponseTime => self.response_time_selection(relays).await,
        }
    }

    /// Round-robin selection
    fn round_robin_selection(&self, relays: &[Arc<dyn RpcRelay>]) -> RpcResult<Arc<dyn RpcRelay>> {
        let index = self.round_robin_counter.fetch_add(1, Ordering::Relaxed) % relays.len();
        debug!("Selected relay {} using round-robin", relays[index].config().name);
        Ok(relays[index].clone())
    }

    /// Priority-based selection (highest priority first)
    async fn priority_selection(&self, relays: &[Arc<dyn RpcRelay>]) -> RpcResult<Arc<dyn RpcRelay>> {
        let mut relay_priorities: Vec<(Arc<dyn RpcRelay>, u8)> = relays
            .iter()
            .map(|relay| (relay.clone(), relay.config().priority))
            .collect();

        // Sort by priority (descending) and health status
        relay_priorities.sort_by(|a, b| {
            b.1.cmp(&a.1) // Higher priority first
        });

        // Select the highest priority healthy relay
        for (relay, priority) in relay_priorities {
            let health = relay.get_health().await;
            if matches!(health.status, HealthStatus::Healthy) {
                debug!("Selected relay {} with priority {} using priority strategy", 
                       relay.config().name, priority);
                return Ok(relay);
            }
        }

        // Fallback to first relay if none are healthy
        debug!("No healthy high-priority relays, selecting first available");
        Ok(relays[0].clone())
    }

    /// Least connections selection (simulated)
    async fn least_connections_selection(&self, relays: &[Arc<dyn RpcRelay>]) -> RpcResult<Arc<dyn RpcRelay>> {
        // Since we don't track actual connections, we'll use response time as a proxy
        // In a real implementation, you'd track active connections per relay
        
        let mut best_relay = None;
        let mut best_response_time = u64::MAX;

        for relay in relays {
            let health = relay.get_health().await;
            if health.response_time_ms < best_response_time && 
               matches!(health.status, HealthStatus::Healthy) {
                best_response_time = health.response_time_ms;
                best_relay = Some(relay.clone());
            }
        }

        match best_relay {
            Some(relay) => {
                debug!("Selected relay {} with {}ms response time using least connections strategy", 
                       relay.config().name, best_response_time);
                Ok(relay)
            }
            None => {
                debug!("No healthy relays found, selecting first available");
                Ok(relays[0].clone())
            }
        }
    }

    /// Response time-based selection (fastest first)
    async fn response_time_selection(&self, relays: &[Arc<dyn RpcRelay>]) -> RpcResult<Arc<dyn RpcRelay>> {
        let mut relay_times: Vec<(Arc<dyn RpcRelay>, u64)> = Vec::new();

        // Collect response times
        for relay in relays {
            let health = relay.get_health().await;
            if matches!(health.status, HealthStatus::Healthy | HealthStatus::Degraded) {
                relay_times.push((relay.clone(), health.response_time_ms));
            }
        }

        if relay_times.is_empty() {
            return Ok(relays[0].clone());
        }

        // Sort by response time (ascending)
        relay_times.sort_by_key(|(_, time)| *time);

        let (relay, response_time) = &relay_times[0];
        debug!("Selected relay {} with {}ms response time using response time strategy", 
               relay.config().name, response_time);
        
        Ok(relay.clone())
    }

    /// Check if relay supports specific method
    fn relay_supports_method(&self, relay: &Arc<dyn RpcRelay>, method: &str) -> bool {
        // This would depend on the specific relay implementation
        // For now, we assume all relays of the correct VM type support all methods
        true
    }

    /// Get load balancing statistics
    pub async fn get_stats(&self, relays: &[Arc<dyn RpcRelay>]) -> LoadBalancerStats {
        let mut total_relays = relays.len();
        let mut healthy_relays = 0;
        let mut degraded_relays = 0;
        let mut unhealthy_relays = 0;
        let mut total_response_time = 0u64;

        for relay in relays {
            let health = relay.get_health().await;
            total_response_time += health.response_time_ms;
            
            match health.status {
                HealthStatus::Healthy => healthy_relays += 1,
                HealthStatus::Degraded => degraded_relays += 1,
                HealthStatus::Unhealthy => unhealthy_relays += 1,
                HealthStatus::Unknown => {}
            }
        }

        LoadBalancerStats {
            total_relays,
            healthy_relays,
            degraded_relays,
            unhealthy_relays,
            average_response_time: if total_relays > 0 {
                total_response_time / total_relays as u64
            } else {
                0
            },
            current_strategy: self.strategy,
            round_robin_position: self.round_robin_counter.load(Ordering::Relaxed),
        }
    }

    /// Update load balancing strategy
    pub fn set_strategy(&mut self, strategy: LoadBalanceStrategy) {
        self.strategy = strategy;
        debug!("Load balancing strategy changed to {:?}", strategy);
    }
}

/// Load balancer statistics
#[derive(Debug, Clone)]
pub struct LoadBalancerStats {
    pub total_relays: usize,
    pub healthy_relays: usize,
    pub degraded_relays: usize,
    pub unhealthy_relays: usize,
    pub average_response_time: u64,
    pub current_strategy: LoadBalanceStrategy,
    pub round_robin_position: usize,
}

/// Weighted relay selection for advanced load balancing
pub struct WeightedRelay {
    pub relay: Arc<dyn RpcRelay>,
    pub weight: u32,
    pub current_load: AtomicUsize,
}

impl WeightedRelay {
    pub fn new(relay: Arc<dyn RpcRelay>, weight: u32) -> Self {
        Self {
            relay,
            weight,
            current_load: AtomicUsize::new(0),
        }
    }

    pub fn add_load(&self) {
        self.current_load.fetch_add(1, Ordering::Relaxed);
    }

    pub fn remove_load(&self) {
        self.current_load.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn get_load(&self) -> usize {
        self.current_load.load(Ordering::Relaxed)
    }

    pub fn get_weighted_load(&self) -> f64 {
        self.get_load() as f64 / self.weight as f64
    }
}

/// Advanced load balancer with weighted selection
pub struct WeightedLoadBalancer {
    relays: Vec<WeightedRelay>,
}

impl WeightedLoadBalancer {
    pub fn new() -> Self {
        Self {
            relays: Vec::new(),
        }
    }

    pub fn add_relay(&mut self, relay: Arc<dyn RpcRelay>, weight: u32) {
        self.relays.push(WeightedRelay::new(relay, weight));
    }

    /// Select relay based on weighted load
    pub async fn select_relay(&self) -> RpcResult<&WeightedRelay> {
        if self.relays.is_empty() {
            return Err(RpcError::BackendUnavailable {
                backend: "all".to_string(),
            });
        }

        // Find relay with lowest weighted load
        let mut best_relay = &self.relays[0];
        let mut best_weighted_load = f64::MAX;

        for relay in &self.relays {
            let health = relay.relay.get_health().await;
            if matches!(health.status, HealthStatus::Healthy) {
                let weighted_load = relay.get_weighted_load();
                if weighted_load < best_weighted_load {
                    best_weighted_load = weighted_load;
                    best_relay = relay;
                }
            }
        }

        Ok(best_relay)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::NodeConfig,
        types::{HealthStatus, NodeHealth, VmType},
    };
    use async_trait::async_trait;
    use std::time::{Duration, SystemTime};

    struct MockRelay {
        name: String,
        priority: u8,
        health_status: HealthStatus,
        response_time: u64,
        config: NodeConfig,
    }

    impl MockRelay {
        fn new(name: String, priority: u8, health_status: HealthStatus, response_time: u64) -> Self {
            let config = NodeConfig {
                name: name.clone(),
                url: "http://localhost:8545".parse().unwrap(),
                timeout: Duration::from_secs(5),
                max_concurrent_requests: 10,
                health_check_interval: Duration::from_secs(30),
                priority,
                auth: None,
            };

            Self {
                name,
                priority,
                health_status,
                response_time,
                config,
            }
        }
    }

    #[async_trait]
    impl RpcRelay for MockRelay {
        async fn forward_request(&self, _request: JsonRpcRequest) -> RpcResult<JsonRpcResponse> {
            unimplemented!()
        }

        async fn get_health(&self) -> NodeHealth {
            NodeHealth {
                name: self.name.clone(),
                vm_type: VmType::Ethereum,
                status: self.health_status,
                response_time_ms: self.response_time,
                last_success: SystemTime::now(),
                error_message: None,
                version_info: None,
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
    async fn test_round_robin_selection() {
        let lb = LoadBalancer::with_strategy(LoadBalanceStrategy::RoundRobin);
        
        let relays: Vec<Arc<dyn RpcRelay>> = vec![
            Arc::new(MockRelay::new("node1".to_string(), 100, HealthStatus::Healthy, 10)),
            Arc::new(MockRelay::new("node2".to_string(), 90, HealthStatus::Healthy, 20)),
            Arc::new(MockRelay::new("node3".to_string(), 80, HealthStatus::Healthy, 30)),
        ];

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "eth_blockNumber".to_string(),
            params: None,
            id: Some(serde_json::Value::Number(1.into())),
        };

        // Test round-robin cycling
        let first = lb.select_relay(&relays, &request).await.unwrap();
        let second = lb.select_relay(&relays, &request).await.unwrap();
        let third = lb.select_relay(&relays, &request).await.unwrap();
        let fourth = lb.select_relay(&relays, &request).await.unwrap();

        assert_eq!(first.config().name, "node1");
        assert_eq!(second.config().name, "node2");
        assert_eq!(third.config().name, "node3");
        assert_eq!(fourth.config().name, "node1"); // Wraps around
    }

    #[tokio::test]
    async fn test_priority_selection() {
        let lb = LoadBalancer::with_strategy(LoadBalanceStrategy::Priority);
        
        let relays: Vec<Arc<dyn RpcRelay>> = vec![
            Arc::new(MockRelay::new("low-priority".to_string(), 50, HealthStatus::Healthy, 10)),
            Arc::new(MockRelay::new("high-priority".to_string(), 100, HealthStatus::Healthy, 20)),
            Arc::new(MockRelay::new("medium-priority".to_string(), 75, HealthStatus::Healthy, 15)),
        ];

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "eth_blockNumber".to_string(),
            params: None,
            id: Some(serde_json::Value::Number(1.into())),
        };

        let selected = lb.select_relay(&relays, &request).await.unwrap();
        assert_eq!(selected.config().name, "high-priority");
    }

    #[tokio::test]
    async fn test_response_time_selection() {
        let lb = LoadBalancer::with_strategy(LoadBalanceStrategy::ResponseTime);
        
        let relays: Vec<Arc<dyn RpcRelay>> = vec![
            Arc::new(MockRelay::new("slow".to_string(), 100, HealthStatus::Healthy, 100)),
            Arc::new(MockRelay::new("fast".to_string(), 50, HealthStatus::Healthy, 10)),
            Arc::new(MockRelay::new("medium".to_string(), 75, HealthStatus::Healthy, 50)),
        ];

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "eth_blockNumber".to_string(),
            params: None,
            id: Some(serde_json::Value::Number(1.into())),
        };

        let selected = lb.select_relay(&relays, &request).await.unwrap();
        assert_eq!(selected.config().name, "fast");
    }

    #[tokio::test]
    async fn test_unhealthy_relay_filtering() {
        let lb = LoadBalancer::with_strategy(LoadBalanceStrategy::Priority);
        
        let relays: Vec<Arc<dyn RpcRelay>> = vec![
            Arc::new(MockRelay::new("unhealthy-high".to_string(), 100, HealthStatus::Unhealthy, 10)),
            Arc::new(MockRelay::new("healthy-low".to_string(), 50, HealthStatus::Healthy, 20)),
        ];

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "eth_blockNumber".to_string(),
            params: None,
            id: Some(serde_json::Value::Number(1.into())),
        };

        let selected = lb.select_relay(&relays, &request).await.unwrap();
        assert_eq!(selected.config().name, "healthy-low");
    }
}