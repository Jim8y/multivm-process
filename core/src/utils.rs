//! Common utilities for the MultiVM system

use crate::Result;
use std::time::Duration;
use tokio::time::timeout;

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of attempts
    pub max_attempts: u32,
    /// Initial backoff delay
    pub initial_delay: Duration,
    /// Maximum backoff delay
    pub max_delay: Duration,
    /// Backoff multiplier
    pub multiplier: f64,
    /// Request timeout
    pub timeout: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            multiplier: 2.0,
            timeout: Duration::from_secs(30),
        }
    }
}

/// Retry an async operation with exponential backoff
pub async fn retry_with_backoff<F, Fut, T>(
    config: &RetryConfig,
    mut operation: F,
) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut delay = config.initial_delay;
    let mut last_error = None;
    
    for attempt in 0..config.max_attempts {
        if attempt > 0 {
            tokio::time::sleep(delay).await;
            delay = std::cmp::min(
                config.max_delay,
                Duration::from_secs_f64(delay.as_secs_f64() * config.multiplier),
            );
        }
        
        match timeout(config.timeout, operation()).await {
            Ok(Ok(result)) => return Ok(result),
            Ok(Err(e)) => {
                tracing::warn!(
                    attempt = attempt + 1,
                    max_attempts = config.max_attempts,
                    error = %e,
                    "Operation failed, will retry"
                );
                last_error = Some(e);
            }
            Err(_) => {
                tracing::warn!(
                    attempt = attempt + 1,
                    max_attempts = config.max_attempts,
                    "Operation timed out, will retry"
                );
                last_error = Some(crate::Error::Timeout);
            }
        }
    }
    
    Err(last_error.unwrap_or_else(|| {
        crate::Error::Other("Retry failed with no error".to_string())
    }))
}

/// Rate limiter using token bucket algorithm
#[derive(Debug)]
pub struct RateLimiter {
    capacity: u64,
    rate: f64,
    state: parking_lot::Mutex<RateLimiterState>,
}

#[derive(Debug)]
struct RateLimiterState {
    tokens: f64,
    last_update: std::time::Instant,
}

impl RateLimiter {
    /// Create a new rate limiter
    /// 
    /// # Arguments
    /// * `rate_per_second` - Number of tokens generated per second
    /// * `burst_capacity` - Maximum number of tokens that can be stored
    /// 
    /// # Panics
    /// Panics if rate_per_second or burst_capacity is 0
    pub fn new(rate_per_second: u64, burst_capacity: u64) -> Self {
        assert!(rate_per_second > 0, "rate_per_second must be > 0");
        assert!(burst_capacity > 0, "burst_capacity must be > 0");
        
        Self {
            capacity: burst_capacity,
            rate: rate_per_second as f64,
            state: parking_lot::Mutex::new(RateLimiterState {
                tokens: burst_capacity as f64,
                last_update: std::time::Instant::now(),
            }),
        }
    }
    
    /// Try to acquire tokens
    pub fn try_acquire(&self, tokens: u64) -> bool {
        if tokens == 0 {
            return true;
        }
        
        let mut state = self.state.lock();
        
        // Replenish tokens based on time elapsed
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(state.last_update).as_secs_f64();
        state.tokens = (state.tokens + elapsed * self.rate).min(self.capacity as f64);
        state.last_update = now;
        
        // Try to acquire
        if state.tokens >= tokens as f64 {
            state.tokens -= tokens as f64;
            true
        } else {
            false
        }
    }
    
    /// Wait until tokens are available
    /// 
    /// # Arguments
    /// * `tokens` - Number of tokens to acquire
    /// 
    /// # Returns
    /// Returns when the tokens have been acquired
    pub async fn acquire(&self, tokens: u64) {
        if tokens == 0 {
            return;
        }
        
        // Calculate wait time more intelligently
        loop {
            // Try to acquire first
            if self.try_acquire(tokens) {
                return;
            }
            
            // Calculate wait time
            let wait_time = {
                let state = self.state.lock();
                // Calculate how long we need to wait for enough tokens
                let tokens_needed = tokens as f64 - state.tokens;
                let wait_seconds = tokens_needed / self.rate;
                Duration::from_secs_f64(wait_seconds.max(0.001))
            };
            
            // Wait for calculated time or a small amount
            tokio::time::sleep(wait_time.min(Duration::from_millis(100))).await;
        }
    }
    
    /// Get current number of available tokens
    pub fn available_tokens(&self) -> f64 {
        let mut state = self.state.lock();
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(state.last_update).as_secs_f64();
        state.tokens = (state.tokens + elapsed * self.rate).min(self.capacity as f64);
        state.last_update = now;
        state.tokens
    }
}

/// Shutdown signal handler
pub struct ShutdownSignal {
    tx: Option<tokio::sync::oneshot::Sender<()>>,
    rx: tokio::sync::oneshot::Receiver<()>,
}

impl ShutdownSignal {
    /// Create a new shutdown signal
    pub fn new() -> Self {
        let (tx, rx) = tokio::sync::oneshot::channel();
        Self {
            tx: Some(tx),
            rx,
        }
    }
    
    /// Trigger shutdown
    pub fn shutdown(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(());
        }
    }
    
    /// Wait for shutdown signal
    pub async fn wait(self) {
        let _ = self.rx.await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_retry_with_backoff() {
        let attempts = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let attempts_clone = attempts.clone();
        
        let config = RetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_millis(10),
            ..Default::default()
        };
        
        let result = retry_with_backoff(&config, || {
            let attempts = attempts_clone.clone();
            async move {
                let current = attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if current < 2 {
                    Err(crate::Error::Other("test error".to_string()))
                } else {
                    Ok(42)
                }
            }
        }).await;
        
        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 3);
    }
    
    #[tokio::test]
    async fn test_retry_timeout() {
        let config = RetryConfig {
            max_attempts: 2,
            timeout: Duration::from_millis(50),
            initial_delay: Duration::from_millis(10),
            ..Default::default()
        };
        
        let result = retry_with_backoff(&config, || async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            Ok::<(), crate::Error>(())
        }).await;
        
        assert!(matches!(result, Err(crate::Error::Timeout)));
    }
    
    #[tokio::test]
    async fn test_rate_limiter() {
        let limiter = RateLimiter::new(10, 5);
        
        // Should allow burst
        assert!(limiter.try_acquire(5));
        
        // Should not allow more than burst
        assert!(!limiter.try_acquire(1));
        
        // Wait for tokens to replenish
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(limiter.try_acquire(1));
    }
    
    #[tokio::test]
    async fn test_rate_limiter_async_acquire() {
        let limiter = RateLimiter::new(100, 10);
        
        // Use up all tokens
        assert!(limiter.try_acquire(10));
        
        // Check that tokens are nearly depleted (may have accumulated a tiny bit)
        let available = limiter.available_tokens();
        assert!(available < 1.0, "Available tokens: {}", available);
        
        // This should wait for tokens
        let start = std::time::Instant::now();
        limiter.acquire(5).await;
        let elapsed = start.elapsed();
        
        // Should have waited approximately 50ms for 5 tokens at 100/sec
        assert!(elapsed >= Duration::from_millis(40), "Elapsed: {:?}", elapsed);
        assert!(elapsed < Duration::from_millis(100), "Elapsed: {:?}", elapsed);
    }
    
    #[test]
    #[should_panic(expected = "rate_per_second must be > 0")]
    fn test_rate_limiter_zero_rate() {
        RateLimiter::new(0, 10);
    }
    
    #[test]
    #[should_panic(expected = "burst_capacity must be > 0")]
    fn test_rate_limiter_zero_capacity() {
        RateLimiter::new(10, 0);
    }
}