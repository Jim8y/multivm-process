//! Performance optimization utilities

use crate::{
    error::{RpcError, RpcResult},
    types::{JsonRpcRequest, JsonRpcResponse},
};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

/// Custom serialization for Duration
pub mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_millis().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }
}
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, warn};

/// Performance configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PerformanceConfig {
    /// Enable request batching
    pub enable_batching: bool,
    /// Maximum batch size
    pub max_batch_size: usize,
    /// Batch timeout
    #[serde(with = "duration_serde")]
    pub batch_timeout: Duration,
    /// Enable connection pooling
    pub enable_connection_pooling: bool,
    /// Connection pool size per backend
    pub connection_pool_size: usize,
    /// Enable request compression
    pub enable_compression: bool,
    /// Enable response streaming
    pub enable_streaming: bool,
    /// Request timeout optimization
    pub adaptive_timeout: bool,
    /// Maximum concurrent requests per client
    pub max_concurrent_per_client: usize,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            enable_batching: true,
            max_batch_size: 100,
            batch_timeout: Duration::from_millis(10),
            enable_connection_pooling: true,
            connection_pool_size: 50,
            enable_compression: true,
            enable_streaming: true,
            adaptive_timeout: true,
            max_concurrent_per_client: 100,
        }
    }
}

impl PerformanceConfig {
    /// Production configuration with aggressive optimization
    pub fn production() -> Self {
        Self {
            enable_batching: true,
            max_batch_size: 200,
            batch_timeout: Duration::from_millis(5),
            enable_connection_pooling: true,
            connection_pool_size: 100,
            enable_compression: true,
            enable_streaming: true,
            adaptive_timeout: true,
            max_concurrent_per_client: 200,
        }
    }

    /// Development configuration with debugging-friendly settings
    pub fn development() -> Self {
        Self {
            enable_batching: false,
            max_batch_size: 10,
            batch_timeout: Duration::from_millis(100),
            enable_connection_pooling: true,
            connection_pool_size: 10,
            enable_compression: false,
            enable_streaming: false,
            adaptive_timeout: false,
            max_concurrent_per_client: 50,
        }
    }
}

/// Request batcher for optimizing multiple requests
pub struct RequestBatcher {
    config: PerformanceConfig,
    pending_requests: Arc<RwLock<Vec<PendingRequest>>>,
    batch_semaphore: Arc<Semaphore>,
}

struct PendingRequest {
    request: JsonRpcRequest,
    response_tx: tokio::sync::oneshot::Sender<RpcResult<JsonRpcResponse>>,
    timestamp: Instant,
}

impl RequestBatcher {
    pub fn new(config: PerformanceConfig) -> Self {
        Self {
            config,
            pending_requests: Arc::new(RwLock::new(Vec::new())),
            batch_semaphore: Arc::new(Semaphore::new(1)),
        }
    }

    /// Add request to batch
    pub async fn add_request(
        &self,
        request: JsonRpcRequest,
    ) -> tokio::sync::oneshot::Receiver<RpcResult<JsonRpcResponse>> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        if !self.config.enable_batching || !self.is_batchable(&request) {
            // Send immediately if batching is disabled or request is not batchable
            return rx;
        }

        let pending = PendingRequest {
            request,
            response_tx: tx,
            timestamp: Instant::now(),
        };

        let mut requests = self.pending_requests.write().await;
        requests.push(pending);

        // Check if we should flush the batch
        if requests.len() >= self.config.max_batch_size {
            drop(requests);
            self.flush_batch().await;
        } else if requests.len() == 1 {
            // Start batch timeout timer for first request
            let batcher = self.clone();
            tokio::spawn(async move {
                tokio::time::sleep(batcher.config.batch_timeout).await;
                batcher.flush_batch().await;
            });
        }

        rx
    }

    /// Check if request can be batched
    fn is_batchable(&self, request: &JsonRpcRequest) -> bool {
        // Only batch read-only requests
        request.is_read_only()
    }

    /// Flush pending batch
    async fn flush_batch(&self) {
        let _permit = match self.batch_semaphore.try_acquire() {
            Ok(permit) => permit,
            Err(_) => return, // Another flush is in progress
        };

        let mut requests = self.pending_requests.write().await;
        if requests.is_empty() {
            return;
        }

        let batch = std::mem::take(&mut *requests);
        drop(requests);

        debug!("Flushing batch of {} requests", batch.len());

        // Process batch (this would be implemented by the actual relay)
        for pending in batch {
            // For now, just return an error - this would be replaced with actual batch processing
            let _ = pending.response_tx.send(Err(RpcError::Internal {
                message: "Batch processing not implemented".to_string(),
            }));
        }
    }
}

impl Clone for RequestBatcher {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            pending_requests: Arc::clone(&self.pending_requests),
            batch_semaphore: Arc::clone(&self.batch_semaphore),
        }
    }
}

/// Connection pool manager
pub struct ConnectionPool {
    config: PerformanceConfig,
    pools: Arc<RwLock<HashMap<String, Vec<PooledConnection>>>>,
}

struct PooledConnection {
    id: String,
    created_at: Instant,
    last_used: Instant,
    in_use: bool,
}

impl ConnectionPool {
    pub fn new(config: PerformanceConfig) -> Self {
        Self {
            config,
            pools: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get connection from pool
    pub async fn get_connection(&self, backend: &str) -> Option<String> {
        if !self.config.enable_connection_pooling {
            return None;
        }

        let mut pools = self.pools.write().await;
        let pool = pools.entry(backend.to_string()).or_insert_with(Vec::new);

        // Find available connection
        for conn in pool.iter_mut() {
            if !conn.in_use {
                conn.in_use = true;
                conn.last_used = Instant::now();
                return Some(conn.id.clone());
            }
        }

        // Create new connection if pool not full
        if pool.len() < self.config.connection_pool_size {
            let id = format!("{}_{}", backend, uuid::Uuid::new_v4());
            pool.push(PooledConnection {
                id: id.clone(),
                created_at: Instant::now(),
                last_used: Instant::now(),
                in_use: true,
            });
            return Some(id);
        }

        None
    }

    /// Return connection to pool
    pub async fn return_connection(&self, backend: &str, connection_id: &str) {
        let mut pools = self.pools.write().await;
        if let Some(pool) = pools.get_mut(backend) {
            if let Some(conn) = pool.iter_mut().find(|c| c.id == connection_id) {
                conn.in_use = false;
            }
        }
    }

    /// Clean up idle connections
    pub async fn cleanup_idle_connections(&self, max_idle_time: Duration) {
        let mut pools = self.pools.write().await;
        let now = Instant::now();

        for pool in pools.values_mut() {
            pool.retain(|conn| {
                !conn.in_use && now.duration_since(conn.last_used) < max_idle_time
            });
        }
    }
}

/// Adaptive timeout calculator
pub struct AdaptiveTimeout {
    response_times: Arc<RwLock<HashMap<String, Vec<Duration>>>>,
    config: PerformanceConfig,
}

impl AdaptiveTimeout {
    pub fn new(config: PerformanceConfig) -> Self {
        Self {
            response_times: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Record response time
    pub async fn record_response_time(&self, method: &str, duration: Duration) {
        if !self.config.adaptive_timeout {
            return;
        }

        let mut times = self.response_times.write().await;
        let method_times = times.entry(method.to_string()).or_insert_with(Vec::new);
        
        // Keep last 100 samples
        if method_times.len() >= 100 {
            method_times.remove(0);
        }
        method_times.push(duration);
    }

    /// Calculate optimal timeout for method
    pub async fn calculate_timeout(&self, method: &str, default: Duration) -> Duration {
        if !self.config.adaptive_timeout {
            return default;
        }

        let times = self.response_times.read().await;
        if let Some(method_times) = times.get(method) {
            if method_times.is_empty() {
                return default;
            }

            // Calculate 99th percentile
            let mut sorted = method_times.clone();
            sorted.sort();
            let p99_index = (sorted.len() as f64 * 0.99) as usize;
            let p99 = sorted[p99_index.min(sorted.len() - 1)];

            // Add 20% buffer
            let timeout = p99.mul_f64(1.2);
            
            // Ensure timeout is between min and max bounds
            let min_timeout = Duration::from_secs(1);
            let max_timeout = Duration::from_secs(30);
            
            timeout.max(min_timeout).min(max_timeout)
        } else {
            default
        }
    }
}

/// Memory pool for reducing allocations
pub struct MemoryPool<T> {
    pool: Arc<RwLock<Vec<T>>>,
    factory: Arc<dyn Fn() -> T + Send + Sync>,
    max_size: usize,
}

impl<T: Send + 'static> MemoryPool<T> {
    pub fn new(max_size: usize, factory: impl Fn() -> T + Send + Sync + 'static) -> Self {
        Self {
            pool: Arc::new(RwLock::new(Vec::new())),
            factory: Arc::new(factory),
            max_size,
        }
    }

    /// Get object from pool
    pub async fn get(&self) -> T {
        let mut pool = self.pool.write().await;
        if let Some(obj) = pool.pop() {
            obj
        } else {
            (self.factory)()
        }
    }

    /// Return object to pool
    pub async fn put(&self, obj: T) {
        let mut pool = self.pool.write().await;
        if pool.len() < self.max_size {
            pool.push(obj);
        }
    }
}

/// Performance monitor
pub struct PerformanceMonitor {
    start_time: Instant,
    metrics: Arc<RwLock<PerformanceMetrics>>,
}

#[derive(Debug, Clone, Default)]
struct PerformanceMetrics {
    total_requests: u64,
    total_response_time: Duration,
    cache_hits: u64,
    cache_misses: u64,
    batch_count: u64,
    batch_size_total: u64,
    connection_reuse_count: u64,
    timeout_adjustments: u64,
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            metrics: Arc::new(RwLock::new(PerformanceMetrics::default())),
        }
    }

    /// Record request
    pub async fn record_request(&self, response_time: Duration) {
        let mut metrics = self.metrics.write().await;
        metrics.total_requests += 1;
        metrics.total_response_time += response_time;
    }

    /// Record cache hit
    pub async fn record_cache_hit(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.cache_hits += 1;
    }

    /// Record cache miss
    pub async fn record_cache_miss(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.cache_misses += 1;
    }

    /// Get performance summary
    pub async fn get_summary(&self) -> PerformanceSummary {
        let metrics = self.metrics.read().await;
        let uptime = self.start_time.elapsed();

        let avg_response_time = if metrics.total_requests > 0 {
            metrics.total_response_time / metrics.total_requests as u32
        } else {
            Duration::ZERO
        };

        let cache_hit_rate = if metrics.cache_hits + metrics.cache_misses > 0 {
            metrics.cache_hits as f64 / (metrics.cache_hits + metrics.cache_misses) as f64
        } else {
            0.0
        };

        let requests_per_second = if uptime.as_secs() > 0 {
            metrics.total_requests as f64 / uptime.as_secs_f64()
        } else {
            0.0
        };

        PerformanceSummary {
            uptime,
            total_requests: metrics.total_requests,
            avg_response_time,
            cache_hit_rate,
            requests_per_second,
            batch_efficiency: if metrics.batch_count > 0 {
                metrics.batch_size_total as f64 / metrics.batch_count as f64
            } else {
                0.0
            },
            connection_reuse_rate: if metrics.total_requests > 0 {
                metrics.connection_reuse_count as f64 / metrics.total_requests as f64
            } else {
                0.0
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct PerformanceSummary {
    pub uptime: Duration,
    pub total_requests: u64,
    pub avg_response_time: Duration,
    pub cache_hit_rate: f64,
    pub requests_per_second: f64,
    pub batch_efficiency: f64,
    pub connection_reuse_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_config() {
        let config = PerformanceConfig::default();
        assert!(config.enable_batching);
        assert_eq!(config.max_batch_size, 100);

        let prod = PerformanceConfig::production();
        assert_eq!(prod.max_batch_size, 200);
        assert_eq!(prod.connection_pool_size, 100);
    }

    #[tokio::test]
    async fn test_connection_pool() {
        let config = PerformanceConfig::default();
        let pool = ConnectionPool::new(config);

        // Get connection
        let conn1 = pool.get_connection("backend1").await;
        assert!(conn1.is_some());

        // Return connection
        pool.return_connection("backend1", &conn1.unwrap()).await;

        // Should reuse connection
        let conn2 = pool.get_connection("backend1").await;
        assert!(conn2.is_some());
    }

    #[tokio::test]
    async fn test_adaptive_timeout() {
        let config = PerformanceConfig::default();
        let timeout = AdaptiveTimeout::new(config);

        // Record some response times
        for _ in 0..10 {
            timeout.record_response_time("test_method", Duration::from_millis(100)).await;
        }

        // Calculate timeout should be minimum 1 second due to min_timeout constraint
        let calculated = timeout.calculate_timeout("test_method", Duration::from_secs(5)).await;
        assert_eq!(calculated, Duration::from_secs(1)); // Min timeout is 1 second
    }

    #[tokio::test]
    async fn test_memory_pool() {
        let pool = MemoryPool::new(10, || vec![0u8; 1024]);

        let obj1 = pool.get().await;
        assert_eq!(obj1.len(), 1024);

        pool.put(obj1).await;

        let obj2 = pool.get().await;
        assert_eq!(obj2.len(), 1024);
    }

    #[tokio::test]
    async fn test_performance_monitor() {
        let monitor = PerformanceMonitor::new();

        monitor.record_request(Duration::from_millis(100)).await;
        monitor.record_request(Duration::from_millis(200)).await;
        monitor.record_cache_hit().await;
        monitor.record_cache_miss().await;

        let summary = monitor.get_summary().await;
        assert_eq!(summary.total_requests, 2);
        assert_eq!(summary.avg_response_time, Duration::from_millis(150));
        assert_eq!(summary.cache_hit_rate, 0.5);
    }
}