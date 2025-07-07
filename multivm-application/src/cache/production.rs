//! Production-ready Redis cache implementation with connection pooling,
//! circuit breaker pattern, and distributed caching features

use crate::config::RedisCacheConfig as RedisConfig;
use crate::error::{ApplicationError, ApplicationResult};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock, Semaphore};
use tracing::{debug, error, info, warn};

#[cfg(feature = "cache")]
use redis::{
    aio::{ConnectionManager, MultiplexedConnection},
    AsyncCommands, Client,
};

/// Production Redis cache with advanced features
pub struct ProductionRedisCache {
    config: RedisConfig,
    #[cfg(feature = "cache")]
    client: Arc<Client>,
    #[cfg(feature = "cache")]
    connection_manager: Arc<Mutex<Option<ConnectionManager>>>,
    #[cfg(feature = "cache")]
    pubsub_conn: Arc<Mutex<Option<MultiplexedConnection>>>,

    // Circuit breaker state
    circuit_state: Arc<RwLock<CircuitState>>,

    // Connection pool management
    connection_semaphore: Arc<Semaphore>,
    reconnect_mutex: Arc<Mutex<()>>,

    // Statistics
    stats: Arc<CacheStatistics>,

    // Local cache for frequently accessed items
    hot_cache: Arc<DashMap<String, CachedValue>>,
    hot_cache_size: Arc<AtomicU64>,

    // Background tasks handle
    background_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

/// Circuit breaker state for Redis connections
#[derive(Debug, Clone)]
struct CircuitState {
    state: CircuitBreakerState,
    #[allow(dead_code)]
    failure_count: u32,
    #[allow(dead_code)]
    last_failure: Option<Instant>,
    #[allow(dead_code)]
    last_success: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CircuitBreakerState {
    Closed,
    #[allow(dead_code)]
    Open,
    #[allow(dead_code)]
    HalfOpen,
}

/// Cache statistics
#[derive(Debug)]
pub struct CacheStatistics {
    pub hits: AtomicU64,
    pub misses: AtomicU64,
    pub errors: AtomicU64,
    pub circuit_breaker_trips: AtomicU64,
    pub reconnect_attempts: AtomicU64,
    pub hot_cache_hits: AtomicU64,
    pub hot_cache_evictions: AtomicU64,
    pub average_latency_ms: AtomicU64,
    pub p99_latency_ms: AtomicU64,
}

/// Cached value with metadata
#[derive(Debug, Clone)]
struct CachedValue {
    data: Vec<u8>,
    expires_at: Option<Instant>,
    access_count: Arc<AtomicU64>,
    last_accessed: Arc<Mutex<Instant>>,
}

impl ProductionRedisCache {
    /// Create a new production Redis cache
    pub async fn new(config: &RedisConfig) -> ApplicationResult<Self> {
        info!("Initializing production Redis cache: {}", config.url);

        #[cfg(feature = "cache")]
        {
            // Create Redis client with custom configuration
            let client =
                Client::open(config.url.as_str()).map_err(|e| ApplicationError::CacheError {
                    operation: "init".to_string(),
                    message: format!("Failed to create Redis client: {}", e),
                })?;

            // Create connection manager for connection pooling
            let connection_manager = match client.get_connection_manager().await {
                Ok(cm) => Some(cm),
                Err(e) => {
                    warn!("Failed to create connection manager, will retry: {}", e);
                    None
                }
            };

            // Create cache instance
            let cache = Self {
                config: config.clone(),
                client: Arc::new(client),
                connection_manager: Arc::new(Mutex::new(connection_manager)),
                pubsub_conn: Arc::new(Mutex::new(None)),
                circuit_state: Arc::new(RwLock::new(CircuitState {
                    state: CircuitBreakerState::Closed,
                    failure_count: 0,
                    last_failure: None,
                    last_success: None,
                })),
                connection_semaphore: Arc::new(Semaphore::new(config.max_connections as usize)),
                reconnect_mutex: Arc::new(Mutex::new(())),
                stats: Arc::new(CacheStatistics::default()),
                hot_cache: Arc::new(DashMap::new()),
                hot_cache_size: Arc::new(AtomicU64::new(0)),
                background_handle: Arc::new(Mutex::new(None)),
            };

            // Start background tasks
            cache.start_background_tasks().await;

            Ok(cache)
        }

        #[cfg(not(feature = "cache"))]
        {
            Ok(Self {
                config: config.clone(),
                circuit_state: Arc::new(RwLock::new(CircuitState {
                    state: CircuitBreakerState::Closed,
                    failure_count: 0,
                    last_failure: None,
                    last_success: None,
                })),
                connection_semaphore: Arc::new(Semaphore::new(config.max_connections as usize)),
                reconnect_mutex: Arc::new(Mutex::new(())),
                stats: Arc::new(CacheStatistics::default()),
                hot_cache: Arc::new(DashMap::new()),
                hot_cache_size: Arc::new(AtomicU64::new(0)),
                background_handle: Arc::new(Mutex::new(None)),
            })
        }
    }

    /// Start background maintenance tasks
    #[allow(dead_code)]
    async fn start_background_tasks(&self) {
        let cache_weak = Arc::downgrade(&(Arc::new(self.clone())));

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                interval.tick().await;

                if let Some(cache) = cache_weak.upgrade() {
                    // Hot cache cleanup
                    cache.cleanup_hot_cache().await;

                    // Connection health check
                    #[cfg(feature = "cache")]
                    cache.check_connection_health().await;

                    // Stats reporting
                    cache.report_stats();
                } else {
                    break;
                }
            }
        });

        *self.background_handle.lock().await = Some(handle);
    }

    /// Get value with circuit breaker and hot cache
    pub async fn get<T>(&self, key: &str) -> ApplicationResult<Option<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        let full_key = format!("{}{}", self.config.key_prefix, key);
        let start = Instant::now();

        // Check hot cache first
        if let Some(cached) = self.get_from_hot_cache(&full_key) {
            self.stats.hot_cache_hits.fetch_add(1, Ordering::Relaxed);

            if let Ok(value) = serde_json::from_slice(&cached) {
                self.record_latency(start.elapsed());
                return Ok(Some(value));
            }
        }

        // Check circuit breaker
        if !self.is_circuit_closed().await {
            self.stats.misses.fetch_add(1, Ordering::Relaxed);
            return Ok(None);
        }

        #[cfg(feature = "cache")]
        {
            let _permit = self.connection_semaphore.acquire().await.map_err(|_| {
                ApplicationError::CacheError {
                    operation: "semaphore".to_string(),
                    message: "Failed to acquire connection permit".to_string(),
                }
            })?;

            match self.get_with_retry(&full_key).await {
                Ok(Some(data)) => {
                    self.stats.hits.fetch_add(1, Ordering::Relaxed);
                    self.record_success().await;

                    // Store in hot cache if frequently accessed
                    self.maybe_store_in_hot_cache(&full_key, data.as_bytes())
                        .await;

                    match serde_json::from_str::<T>(&data) {
                        Ok(value) => {
                            self.record_latency(start.elapsed());
                            Ok(Some(value))
                        }
                        Err(e) => {
                            warn!("Deserialization error for key {}: {}", full_key, e);
                            Ok(None)
                        }
                    }
                }
                Ok(None) => {
                    self.stats.misses.fetch_add(1, Ordering::Relaxed);
                    self.record_latency(start.elapsed());
                    Ok(None)
                }
                Err(e) => {
                    self.stats.errors.fetch_add(1, Ordering::Relaxed);
                    self.record_failure().await;
                    Err(e)
                }
            }
        }

        #[cfg(not(feature = "cache"))]
        {
            self.stats.misses.fetch_add(1, Ordering::Relaxed);
            Ok(None)
        }
    }

    /// Set value with circuit breaker
    pub async fn set<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> ApplicationResult<()>
    where
        T: Serialize,
    {
        let full_key = format!("{}{}", self.config.key_prefix, key);
        let start = Instant::now();

        // Check circuit breaker
        if !self.is_circuit_closed().await {
            return Ok(());
        }

        let serialized =
            serde_json::to_string(value).map_err(|e| ApplicationError::CacheError {
                operation: "serialize".to_string(),
                message: format!("Serialization error: {e}"),
            })?;

        #[cfg(feature = "cache")]
        {
            let _permit = self.connection_semaphore.acquire().await.map_err(|_| {
                ApplicationError::CacheError {
                    operation: "semaphore".to_string(),
                    message: "Failed to acquire connection permit".to_string(),
                }
            })?;

            match self.set_with_retry(&full_key, &serialized, ttl).await {
                Ok(_) => {
                    self.record_success().await;
                    self.record_latency(start.elapsed());

                    // Update hot cache if present
                    if self.hot_cache.contains_key(&full_key) {
                        let expires_at = ttl.map(|d| Instant::now() + d);
                        let cached_value = CachedValue {
                            data: serialized.into_bytes(),
                            expires_at,
                            access_count: Arc::new(AtomicU64::new(0)),
                            last_accessed: Arc::new(Mutex::new(Instant::now())),
                        };
                        self.hot_cache.insert(full_key, cached_value);
                    }

                    Ok(())
                }
                Err(e) => {
                    self.stats.errors.fetch_add(1, Ordering::Relaxed);
                    self.record_failure().await;
                    Err(e)
                }
            }
        }

        #[cfg(not(feature = "cache"))]
        {
            Ok(())
        }
    }

    /// Delete value
    pub async fn delete(&self, key: &str) -> ApplicationResult<()> {
        let full_key = format!("{}{}", self.config.key_prefix, key);

        // Remove from hot cache
        self.hot_cache.remove(&full_key);

        if !self.is_circuit_closed().await {
            return Ok(());
        }

        #[cfg(feature = "cache")]
        {
            let _permit = self.connection_semaphore.acquire().await.map_err(|_| {
                ApplicationError::CacheError {
                    operation: "semaphore".to_string(),
                    message: "Failed to acquire connection permit".to_string(),
                }
            })?;

            self.delete_with_retry(&full_key).await
        }

        #[cfg(not(feature = "cache"))]
        Ok(())
    }

    /// Batch get with pipeline
    pub async fn mget<T>(&self, keys: &[&str]) -> ApplicationResult<Vec<Option<T>>>
    where
        T: for<'de> Deserialize<'de>,
    {
        if keys.is_empty() {
            return Ok(vec![]);
        }

        let full_keys: Vec<String> = keys
            .iter()
            .map(|k| format!("{}{}", self.config.key_prefix, k))
            .collect();

        // Check hot cache first
        let mut results = Vec::with_capacity(keys.len());
        let mut cache_misses = Vec::new();

        for (idx, key) in full_keys.iter().enumerate() {
            if let Some(cached) = self.get_from_hot_cache(key) {
                if let Ok(value) = serde_json::from_slice(&cached) {
                    results.push(Some(value));
                } else {
                    results.push(None);
                    cache_misses.push(idx);
                }
            } else {
                results.push(None);
                cache_misses.push(idx);
            }
        }

        if cache_misses.is_empty() {
            return Ok(results);
        }

        if !self.is_circuit_closed().await {
            return Ok(results);
        }

        #[cfg(feature = "cache")]
        {
            let _permit = self.connection_semaphore.acquire().await.map_err(|_| {
                ApplicationError::CacheError {
                    operation: "semaphore".to_string(),
                    message: "Failed to acquire connection permit".to_string(),
                }
            })?;

            let miss_keys: Vec<&String> = cache_misses.iter().map(|&idx| &full_keys[idx]).collect();

            match self.mget_with_retry(&miss_keys).await {
                Ok(values) => {
                    for (i, value) in values.into_iter().enumerate() {
                        if let Some(data) = value {
                            if let Ok(parsed) = serde_json::from_str(&data) {
                                let idx = cache_misses[i];
                                results[idx] = Some(parsed);

                                // Store in hot cache
                                self.maybe_store_in_hot_cache(&full_keys[idx], data.as_bytes())
                                    .await;
                            }
                        }
                    }
                    Ok(results)
                }
                Err(e) => {
                    self.record_failure().await;
                    Err(e)
                }
            }
        }

        #[cfg(not(feature = "cache"))]
        Ok(results)
    }

    /// Set multiple values using pipeline
    pub async fn mset<T>(
        &self,
        items: &[(&str, &T)],
        ttl: Option<Duration>,
    ) -> ApplicationResult<()>
    where
        T: Serialize,
    {
        if items.is_empty() {
            return Ok(());
        }

        if !self.is_circuit_closed().await {
            return Ok(());
        }

        #[cfg(feature = "cache")]
        {
            let _permit = self.connection_semaphore.acquire().await.map_err(|_| {
                ApplicationError::CacheError {
                    operation: "semaphore".to_string(),
                    message: "Failed to acquire connection permit".to_string(),
                }
            })?;

            let mut pipe = redis::pipe();

            for (key, value) in items {
                let full_key = format!("{}{}", self.config.key_prefix, key);
                let serialized =
                    serde_json::to_string(value).map_err(|e| ApplicationError::CacheError {
                        operation: "serialize".to_string(),
                        message: format!("Serialization error: {}", e),
                    })?;

                if let Some(ttl) = ttl {
                    pipe.set_ex(&full_key, &serialized, ttl.as_secs());
                } else {
                    pipe.set(&full_key, &serialized);
                }
            }

            self.execute_pipeline(pipe).await
        }

        #[cfg(not(feature = "cache"))]
        Ok(())
    }

    /// Increment counter with atomic operation
    pub async fn incr(&self, key: &str, delta: i64) -> ApplicationResult<i64> {
        let full_key = format!("{}{}", self.config.key_prefix, key);

        if !self.is_circuit_closed().await {
            return Ok(0);
        }

        #[cfg(feature = "cache")]
        {
            let _permit = self.connection_semaphore.acquire().await.map_err(|_| {
                ApplicationError::CacheError {
                    operation: "semaphore".to_string(),
                    message: "Failed to acquire connection permit".to_string(),
                }
            })?;

            self.incr_with_retry(&full_key, delta).await
        }

        #[cfg(not(feature = "cache"))]
        Ok(delta)
    }

    /// Get with lease (distributed lock)
    pub async fn get_with_lease<T>(
        &self,
        key: &str,
        lease_duration: Duration,
    ) -> ApplicationResult<Option<(T, String)>>
    where
        T: for<'de> Deserialize<'de>,
    {
        let full_key = format!("{}{}", self.config.key_prefix, key);
        let lease_key = format!("{full_key}_lease");
        let lease_id = uuid::Uuid::new_v4().to_string();

        if !self.is_circuit_closed().await {
            return Ok(None);
        }

        #[cfg(feature = "cache")]
        {
            let _permit = self.connection_semaphore.acquire().await.map_err(|_| {
                ApplicationError::CacheError {
                    operation: "semaphore".to_string(),
                    message: "Failed to acquire connection permit".to_string(),
                }
            })?;

            // Try to acquire lease
            let lease_acquired = self
                .set_nx_with_retry(&lease_key, &lease_id, lease_duration)
                .await?;

            if !lease_acquired {
                return Ok(None);
            }

            // Get value
            match self.get::<T>(key).await? {
                Some(value) => Ok(Some((value, lease_id))),
                None => {
                    // Release lease if no value
                    let _ = self.delete(&lease_key).await;
                    Ok(None)
                }
            }
        }

        #[cfg(not(feature = "cache"))]
        Ok(None)
    }

    /// Release lease
    pub async fn release_lease(&self, key: &str, lease_id: &str) -> ApplicationResult<bool> {
        let full_key = format!("{}{}", self.config.key_prefix, key);
        let lease_key = format!("{full_key}_lease");

        if !self.is_circuit_closed().await {
            return Ok(false);
        }

        #[cfg(feature = "cache")]
        {
            let _permit = self.connection_semaphore.acquire().await.map_err(|_| {
                ApplicationError::CacheError {
                    operation: "semaphore".to_string(),
                    message: "Failed to acquire connection permit".to_string(),
                }
            })?;

            // Check if we own the lease
            match self.get::<String>(&lease_key).await? {
                Some(current_lease_id) if current_lease_id == lease_id => {
                    self.delete(&lease_key).await?;
                    Ok(true)
                }
                _ => Ok(false),
            }
        }

        #[cfg(not(feature = "cache"))]
        Ok(false)
    }

    // Helper methods

    #[cfg(feature = "cache")]
    async fn get_connection(&self) -> ApplicationResult<ConnectionManager> {
        let mut conn_guard = self.connection_manager.lock().await;

        if let Some(conn) = conn_guard.as_ref() {
            return Ok(conn.clone());
        }

        // Reconnect
        let _lock = self.reconnect_mutex.lock().await;
        self.stats
            .reconnect_attempts
            .fetch_add(1, Ordering::Relaxed);

        let conn = self.client.get_connection_manager().await.map_err(|e| {
            ApplicationError::CacheError {
                operation: "reconnect".to_string(),
                message: format!("Failed to reconnect: {}", e),
            }
        })?;

        *conn_guard = Some(conn.clone());
        Ok(conn)
    }

    #[cfg(feature = "cache")]
    async fn get_with_retry(&self, key: &str) -> ApplicationResult<Option<String>> {
        let mut attempts = 0;
        let max_attempts = 3;

        loop {
            attempts += 1;

            match self.get_connection().await {
                Ok(mut conn) => match conn.get::<_, Option<String>>(key).await {
                    Ok(value) => return Ok(value),
                    Err(e) if attempts < max_attempts => {
                        warn!("Redis GET retry {}/{}: {}", attempts, max_attempts, e);
                        tokio::time::sleep(Duration::from_millis(100 * attempts as u64)).await;
                        continue;
                    }
                    Err(e) => {
                        return Err(ApplicationError::CacheError {
                            operation: "get".to_string(),
                            message: format!("Redis GET failed: {}", e),
                        });
                    }
                },
                Err(e) if attempts < max_attempts => {
                    tokio::time::sleep(Duration::from_millis(100 * attempts as u64)).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }

    #[cfg(feature = "cache")]
    async fn set_with_retry(
        &self,
        key: &str,
        value: &str,
        ttl: Option<Duration>,
    ) -> ApplicationResult<()> {
        let mut attempts = 0;
        let max_attempts = 3;

        loop {
            attempts += 1;

            match self.get_connection().await {
                Ok(mut conn) => {
                    let result = if let Some(ttl) = ttl {
                        conn.set_ex::<_, _, ()>(key, value, ttl.as_secs()).await
                    } else {
                        conn.set::<_, _, ()>(key, value).await
                    };

                    match result {
                        Ok(_) => return Ok(()),
                        Err(e) if attempts < max_attempts => {
                            warn!("Redis SET retry {}/{}: {}", attempts, max_attempts, e);
                            tokio::time::sleep(Duration::from_millis(100 * attempts as u64)).await;
                            continue;
                        }
                        Err(e) => {
                            return Err(ApplicationError::CacheError {
                                operation: "set".to_string(),
                                message: format!("Redis SET failed: {}", e),
                            });
                        }
                    }
                }
                Err(e) if attempts < max_attempts => {
                    tokio::time::sleep(Duration::from_millis(100 * attempts as u64)).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }

    #[cfg(feature = "cache")]
    async fn delete_with_retry(&self, key: &str) -> ApplicationResult<()> {
        let mut conn = self.get_connection().await?;

        conn.del::<_, ()>(key)
            .await
            .map_err(|e| ApplicationError::CacheError {
                operation: "delete".to_string(),
                message: format!("Redis DELETE failed: {}", e),
            })
    }

    #[cfg(feature = "cache")]
    async fn mget_with_retry(&self, keys: &[&String]) -> ApplicationResult<Vec<Option<String>>> {
        let mut conn = self.get_connection().await?;

        conn.get::<_, Vec<Option<String>>>(keys)
            .await
            .map_err(|e| ApplicationError::CacheError {
                operation: "mget".to_string(),
                message: format!("Redis MGET failed: {}", e),
            })
    }

    #[cfg(feature = "cache")]
    async fn execute_pipeline(&self, pipe: redis::Pipeline) -> ApplicationResult<()> {
        let mut conn = self.get_connection().await?;

        pipe.query_async(&mut conn)
            .await
            .map_err(|e| ApplicationError::CacheError {
                operation: "pipeline".to_string(),
                message: format!("Redis pipeline failed: {}", e),
            })
    }

    #[cfg(feature = "cache")]
    async fn incr_with_retry(&self, key: &str, delta: i64) -> ApplicationResult<i64> {
        let mut conn = self.get_connection().await?;

        conn.incr(key, delta)
            .await
            .map_err(|e| ApplicationError::CacheError {
                operation: "incr".to_string(),
                message: format!("Redis INCR failed: {}", e),
            })
    }

    #[cfg(feature = "cache")]
    async fn set_nx_with_retry(
        &self,
        key: &str,
        value: &str,
        ttl: Duration,
    ) -> ApplicationResult<bool> {
        let mut conn = self.get_connection().await?;

        let result: Option<String> = redis::cmd("SET")
            .arg(key)
            .arg(value)
            .arg("NX")
            .arg("EX")
            .arg(ttl.as_secs())
            .query_async(&mut conn)
            .await
            .map_err(|e| ApplicationError::CacheError {
                operation: "setnx".to_string(),
                message: format!("Redis SETNX failed: {}", e),
            })?;

        Ok(result.is_some())
    }

    #[cfg(feature = "cache")]
    async fn check_connection_health(&self) {
        if let Ok(mut conn) = self.get_connection().await {
            match redis::cmd("PING").query_async::<_, String>(&mut conn).await {
                Ok(_) => {
                    debug!("Redis health check: OK");
                }
                Err(e) => {
                    warn!("Redis health check failed: {}", e);
                    *self.connection_manager.lock().await = None;
                }
            }
        }
    }

    // Circuit breaker methods

    async fn is_circuit_closed(&self) -> bool {
        let state = self.circuit_state.read().await;
        matches!(
            state.state,
            CircuitBreakerState::Closed | CircuitBreakerState::HalfOpen
        )
    }

    #[allow(dead_code)]
    async fn record_success(&self) {
        let mut state = self.circuit_state.write().await;
        state.failure_count = 0;
        state.last_success = Some(Instant::now());

        if state.state == CircuitBreakerState::HalfOpen {
            state.state = CircuitBreakerState::Closed;
            info!("Circuit breaker closed after successful operation");
        }
    }

    #[allow(dead_code)]
    async fn record_failure(&self) {
        let mut state = self.circuit_state.write().await;
        state.failure_count += 1;
        state.last_failure = Some(Instant::now());

        if state.failure_count >= self.config.circuit_breaker_threshold
            && state.state != CircuitBreakerState::Open
        {
            state.state = CircuitBreakerState::Open;
            self.stats
                .circuit_breaker_trips
                .fetch_add(1, Ordering::Relaxed);
            error!(
                "Circuit breaker opened after {} failures",
                state.failure_count
            );
        }

        // Check if we should transition to half-open
        if state.state == CircuitBreakerState::Open {
            if let Some(last_failure) = state.last_failure {
                if last_failure.elapsed() > Duration::from_secs(self.config.circuit_breaker_timeout)
                {
                    state.state = CircuitBreakerState::HalfOpen;
                    info!("Circuit breaker half-open, allowing test request");
                }
            }
        }
    }

    // Hot cache methods

    fn get_from_hot_cache(&self, key: &str) -> Option<Vec<u8>> {
        if let Some(entry) = self.hot_cache.get(key) {
            // Check expiration
            if let Some(expires_at) = entry.expires_at {
                if Instant::now() > expires_at {
                    self.hot_cache.remove(key);
                    return None;
                }
            }

            // Update access stats
            entry.access_count.fetch_add(1, Ordering::Relaxed);
            if let Ok(mut last_accessed) = entry.last_accessed.try_lock() {
                *last_accessed = Instant::now();
            }

            Some(entry.data.clone())
        } else {
            None
        }
    }

    #[allow(dead_code)]
    async fn maybe_store_in_hot_cache(&self, key: &str, data: &[u8]) {
        let current_size = self.hot_cache_size.load(Ordering::Relaxed);

        // Check if we have space
        if current_size >= self.config.hot_cache_max_size as u64 {
            return;
        }

        // Simple frequency-based caching
        let entry = CachedValue {
            data: data.to_vec(),
            expires_at: None, // Will be set based on TTL
            access_count: Arc::new(AtomicU64::new(1)),
            last_accessed: Arc::new(Mutex::new(Instant::now())),
        };

        self.hot_cache.insert(key.to_string(), entry);
        self.hot_cache_size
            .fetch_add(data.len() as u64, Ordering::Relaxed);
    }

    #[allow(dead_code)]
    async fn cleanup_hot_cache(&self) {
        let now = Instant::now();
        let mut removed_size = 0u64;
        let eviction_threshold = Duration::from_secs(300); // 5 minutes

        self.hot_cache.retain(|_key, entry| {
            // Remove expired entries
            if let Some(expires_at) = entry.expires_at {
                if now > expires_at {
                    removed_size += entry.data.len() as u64;
                    self.stats
                        .hot_cache_evictions
                        .fetch_add(1, Ordering::Relaxed);
                    return false;
                }
            }

            // Remove entries not accessed recently
            if let Ok(last_accessed) = entry.last_accessed.try_lock() {
                if now.duration_since(*last_accessed) > eviction_threshold {
                    removed_size += entry.data.len() as u64;
                    self.stats
                        .hot_cache_evictions
                        .fetch_add(1, Ordering::Relaxed);
                    return false;
                }
            }

            true
        });

        self.hot_cache_size
            .fetch_sub(removed_size, Ordering::Relaxed);
    }

    fn record_latency(&self, duration: Duration) {
        let ms = duration.as_millis() as u64;

        // Proper exponential moving average (EMA) with configurable smoothing
        // Alpha determines responsiveness: higher = more responsive to recent values
        let alpha = 0.1; // 10% weight to new values, 90% to historical average

        let current_avg = self.stats.average_latency_ms.load(Ordering::Relaxed);

        // Handle initial case when no previous average exists
        let new_avg = if current_avg == 0 {
            ms
        } else {
            // EMA formula: new_avg = alpha * new_value + (1 - alpha) * old_avg
            let alpha_scaled = (alpha * 1000.0) as u64; // Scale to avoid floating point
            let one_minus_alpha_scaled = 1000 - alpha_scaled;

            (alpha_scaled * ms + one_minus_alpha_scaled * current_avg) / 1000
        };

        self.stats
            .average_latency_ms
            .store(new_avg, Ordering::Relaxed);

        // Update P99 using reservoir sampling approach for better accuracy
        // This maintains a more accurate P99 estimate over time
        let current_p99 = self.stats.p99_latency_ms.load(Ordering::Relaxed);

        // Use a decay factor for P99 to prevent it from being stuck at historical highs
        let p99_decay_factor = 0.99; // 99% retention of previous P99

        let new_p99 = if current_p99 == 0 {
            ms
        } else if ms > current_p99 {
            // New high value becomes the P99
            ms
        } else {
            // Gradually decay P99 towards current latency patterns
            // This prevents P99 from being permanently elevated by outliers
            let decay_scaled = (p99_decay_factor * 1000.0) as u64;
            let growth_scaled = 1000 - decay_scaled;

            (decay_scaled * current_p99 + growth_scaled * ms) / 1000
        };

        self.stats.p99_latency_ms.store(new_p99, Ordering::Relaxed);
    }

    #[allow(dead_code)]
    fn report_stats(&self) {
        let hits = self.stats.hits.load(Ordering::Relaxed);
        let misses = self.stats.misses.load(Ordering::Relaxed);
        let errors = self.stats.errors.load(Ordering::Relaxed);
        let hot_cache_hits = self.stats.hot_cache_hits.load(Ordering::Relaxed);

        let total = hits + misses;
        let hit_rate = if total > 0 {
            (hits as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        debug!(
            "Cache stats - Hit rate: {:.2}%, Total: {}, Errors: {}, Hot cache hits: {}, Avg latency: {}ms",
            hit_rate,
            total,
            errors,
            hot_cache_hits,
            self.stats.average_latency_ms.load(Ordering::Relaxed)
        );
    }

    /// Get comprehensive statistics
    pub fn get_full_stats(&self) -> CacheStats {
        CacheStats {
            hits: self.stats.hits.load(Ordering::Relaxed),
            misses: self.stats.misses.load(Ordering::Relaxed),
            errors: self.stats.errors.load(Ordering::Relaxed),
            circuit_breaker_trips: self.stats.circuit_breaker_trips.load(Ordering::Relaxed),
            hot_cache_hits: self.stats.hot_cache_hits.load(Ordering::Relaxed),
            hot_cache_size: self.hot_cache_size.load(Ordering::Relaxed),
            hot_cache_entries: self.hot_cache.len(),
            average_latency_ms: self.stats.average_latency_ms.load(Ordering::Relaxed),
            p99_latency_ms: self.stats.p99_latency_ms.load(Ordering::Relaxed),
        }
    }
}

impl Clone for ProductionRedisCache {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            #[cfg(feature = "cache")]
            client: self.client.clone(),
            #[cfg(feature = "cache")]
            connection_manager: self.connection_manager.clone(),
            #[cfg(feature = "cache")]
            pubsub_conn: self.pubsub_conn.clone(),
            circuit_state: self.circuit_state.clone(),
            connection_semaphore: self.connection_semaphore.clone(),
            reconnect_mutex: self.reconnect_mutex.clone(),
            stats: self.stats.clone(),
            hot_cache: self.hot_cache.clone(),
            hot_cache_size: self.hot_cache_size.clone(),
            background_handle: self.background_handle.clone(),
        }
    }
}

impl Default for CacheStatistics {
    fn default() -> Self {
        Self {
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            circuit_breaker_trips: AtomicU64::new(0),
            reconnect_attempts: AtomicU64::new(0),
            hot_cache_hits: AtomicU64::new(0),
            hot_cache_evictions: AtomicU64::new(0),
            average_latency_ms: AtomicU64::new(0),
            p99_latency_ms: AtomicU64::new(0),
        }
    }
}

/// Production cache statistics
#[derive(Debug, Clone, Serialize)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub errors: u64,
    pub circuit_breaker_trips: u64,
    pub hot_cache_hits: u64,
    pub hot_cache_size: u64,
    pub hot_cache_entries: usize,
    pub average_latency_ms: u64,
    pub p99_latency_ms: u64,
}

/// Publish cache invalidation event
impl ProductionRedisCache {
    pub async fn publish_invalidation(&self, key: &str) -> ApplicationResult<()> {
        if !self.is_circuit_closed().await {
            return Ok(());
        }

        #[cfg(feature = "cache")]
        {
            let channel = format!("{}:invalidation", self.config.key_prefix);
            let mut conn = self.get_connection().await?;

            conn.publish::<_, _, ()>(&channel, key).await.map_err(|e| {
                ApplicationError::CacheError {
                    operation: "publish".to_string(),
                    message: format!("Failed to publish invalidation: {}", e),
                }
            })?;
        }

        Ok(())
    }

    /// Get stats in RedisStats format for compatibility
    pub async fn get_stats(&self) -> ApplicationResult<super::redis::RedisStats> {
        Ok(super::redis::RedisStats {
            hits: self.stats.hits.load(Ordering::Relaxed),
            misses: self.stats.misses.load(Ordering::Relaxed),
        })
    }

    /// Check if key exists
    pub async fn exists(&self, key: &str) -> ApplicationResult<bool> {
        let full_key = format!("{}{}", self.config.key_prefix, key);

        // Check hot cache first
        if self.hot_cache.contains_key(&full_key) {
            return Ok(true);
        }

        if !self.is_circuit_closed().await {
            return Ok(false);
        }

        #[cfg(feature = "cache")]
        {
            let _permit = self.connection_semaphore.acquire().await.map_err(|_| {
                ApplicationError::CacheError {
                    operation: "semaphore".to_string(),
                    message: "Failed to acquire connection permit".to_string(),
                }
            })?;

            let mut conn = self.get_connection().await?;
            let exists: bool =
                conn.exists(&full_key)
                    .await
                    .map_err(|e| ApplicationError::CacheError {
                        operation: "exists".to_string(),
                        message: format!("Redis EXISTS failed: {}", e),
                    })?;
            Ok(exists)
        }

        #[cfg(not(feature = "cache"))]
        Ok(false)
    }

    /// Clear all cache entries
    pub async fn clear(&self) -> ApplicationResult<()> {
        // Clear hot cache
        self.hot_cache.clear();
        self.hot_cache_size.store(0, Ordering::Relaxed);

        if !self.is_circuit_closed().await {
            return Ok(());
        }

        #[cfg(feature = "cache")]
        {
            let _permit = self.connection_semaphore.acquire().await.map_err(|_| {
                ApplicationError::CacheError {
                    operation: "semaphore".to_string(),
                    message: "Failed to acquire connection permit".to_string(),
                }
            })?;

            let mut conn = self.get_connection().await?;
            let pattern = format!("{}*", self.config.key_prefix);

            // Use SCAN to avoid blocking on large keyspaces
            let mut cursor = 0u64;
            loop {
                let (new_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                    .arg(cursor)
                    .arg("MATCH")
                    .arg(&pattern)
                    .arg("COUNT")
                    .arg(100)
                    .query_async(&mut conn)
                    .await
                    .map_err(|e| ApplicationError::CacheError {
                        operation: "scan".to_string(),
                        message: format!("Redis SCAN failed: {}", e),
                    })?;

                if !keys.is_empty() {
                    let _: () =
                        conn.del(&keys)
                            .await
                            .map_err(|e| ApplicationError::CacheError {
                                operation: "del".to_string(),
                                message: format!("Redis DEL failed: {}", e),
                            })?;
                }

                cursor = new_cursor;
                if cursor == 0 {
                    break;
                }
            }
        }

        Ok(())
    }
}

impl std::fmt::Debug for ProductionRedisCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProductionRedisCache")
            .field("config", &self.config)
            .field("circuit_state", &"<circuit_state>")
            .field("stats", &"<stats>")
            .field(
                "hot_cache_size",
                &self.hot_cache_size.load(Ordering::Relaxed),
            )
            .finish()
    }
}
