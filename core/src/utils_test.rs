//! Comprehensive tests for utilities including stress tests

#[cfg(test)]
mod tests {
    use super::super::utils::*;
    use crate::Error;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{Duration, Instant};
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_retry_with_exponential_backoff() {
        let config = RetryConfig {
            max_attempts: 4,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(100),
            multiplier: 2.0,
            timeout: Duration::from_secs(1),
        };

        // Test measuring actual backoff delays
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();
        let mut delays = Vec::new();
        let start = Instant::now();

        let result = retry_with_backoff(&config, || {
            let attempts = attempts_clone.clone();
            let current = attempts.fetch_add(1, Ordering::SeqCst);
            let elapsed = start.elapsed();
            delays.push(elapsed);
            
            async move {
                if current < 2 {
                    Err(Error::Other("retry me".to_string()))
                } else {
                    Ok("success")
                }
            }
        }).await;

        assert_eq!(result.unwrap(), "success");
        assert_eq!(attempts.load(Ordering::SeqCst), 3);

        // Verify exponential backoff
        assert!(delays.len() >= 3);
        if delays.len() > 1 {
            let first_delay = delays[1].saturating_sub(delays[0]);
            assert!(first_delay >= Duration::from_millis(10));
            assert!(first_delay < Duration::from_millis(20));
        }
    }

    #[tokio::test]
    async fn test_retry_timeout_behavior() {
        let config = RetryConfig {
            max_attempts: 5,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(1),
            multiplier: 2.0,
            timeout: Duration::from_millis(50), // Short timeout
        };

        let start = Instant::now();
        let result = retry_with_backoff(&config, || async {
            sleep(Duration::from_millis(100)).await; // Longer than timeout
            Ok::<(), Error>(())
        }).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::Timeout));
        
        // Should have attempted multiple times within reasonable time
        let elapsed = start.elapsed();
        assert!(elapsed < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_retry_immediate_success() {
        let config = RetryConfig::default();
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result = retry_with_backoff(&config, || {
            let attempts = attempts_clone.clone();
            attempts.fetch_add(1, Ordering::SeqCst);
            async { Ok::<_, Error>("immediate") }
        }).await;

        assert_eq!(result.unwrap(), "immediate");
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_rate_limiter_burst() {
        let limiter = RateLimiter::new(100, 10); // 100/sec, burst of 10

        // Should allow burst
        for _ in 0..10 {
            assert!(limiter.try_acquire(1));
        }

        // 11th should fail
        assert!(!limiter.try_acquire(1));

        // After waiting, should replenish
        sleep(Duration::from_millis(20)).await;
        assert!(limiter.try_acquire(1));
    }

    #[tokio::test]
    async fn test_rate_limiter_replenishment() {
        let limiter = RateLimiter::new(100, 5); // 100 tokens/sec

        // Use all tokens
        assert!(limiter.try_acquire(5));
        assert_eq!(limiter.available_tokens() as u64, 0);

        // Wait 50ms, should have ~5 tokens
        sleep(Duration::from_millis(50)).await;
        let available = limiter.available_tokens();
        assert!(available >= 4.0 && available <= 6.0);
    }

    #[tokio::test]
    async fn test_rate_limiter_async_acquire() {
        let limiter = RateLimiter::new(1000, 10); // 1000/sec

        // Use all tokens
        assert!(limiter.try_acquire(10));

        // Time async acquire
        let start = Instant::now();
        limiter.acquire(5).await;
        let elapsed = start.elapsed();

        // Should wait approximately 5ms for 5 tokens at 1000/sec
        assert!(elapsed >= Duration::from_millis(4));
        assert!(elapsed < Duration::from_millis(20));
    }

    #[tokio::test]
    async fn test_rate_limiter_concurrent_access() {
        let limiter = Arc::new(RateLimiter::new(1000, 100));
        let mut handles = Vec::new();

        // Spawn 10 tasks each trying to acquire 10 tokens
        for _ in 0..10 {
            let limiter_clone = limiter.clone();
            let handle = tokio::spawn(async move {
                let mut acquired = 0;
                for _ in 0..20 {
                    if limiter_clone.try_acquire(1) {
                        acquired += 1;
                    }
                    sleep(Duration::from_millis(1)).await;
                }
                acquired
            });
            handles.push(handle);
        }

        // Wait for all tasks
        let results: Vec<u32> = futures::future::try_join_all(handles)
            .await
            .unwrap();

        // Total acquired should be reasonable given rate limit
        let total: u32 = results.iter().sum();
        assert!(total >= 100); // At least burst capacity
        assert!(total <= 150); // Not too much over
    }

    #[tokio::test]
    async fn test_rate_limiter_zero_tokens() {
        let limiter = RateLimiter::new(100, 10);
        
        // Acquiring 0 tokens should always succeed
        assert!(limiter.try_acquire(0));
        limiter.acquire(0).await; // Should return immediately
    }

    #[tokio::test]
    async fn test_shutdown_signal() {
        let mut signal = ShutdownSignal::new();
        
        // Trigger shutdown
        signal.shutdown();
        
        // Waiting should return immediately since shutdown was already triggered
        let start = Instant::now();
        signal.wait().await;
        assert!(start.elapsed() < Duration::from_millis(10));
    }
    
    #[tokio::test]
    async fn test_shutdown_signal_concurrent() {
        // Test concurrent shutdown scenario
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let (done_tx, done_rx) = tokio::sync::oneshot::channel::<&str>();
        
        // Create a separate task that waits for external shutdown signal
        tokio::spawn(async move {
            let signal = ShutdownSignal::new();
            // In real usage, something would call signal.shutdown()
            // For testing, we'll just wait for the signal
            tokio::select! {
                _ = signal.wait() => {
                    let _ = done_tx.send("internal shutdown");
                }
                _ = shutdown_rx => {
                    let _ = done_tx.send("external shutdown");
                }
            }
        });
        
        // Give the task time to start
        sleep(Duration::from_millis(10)).await;
        
        // Trigger external shutdown
        let _ = shutdown_tx.send(());
        
        // Wait for result
        let result = done_rx.await;
        assert_eq!(result.unwrap(), "external shutdown");
    }

    #[tokio::test]
    async fn test_shutdown_signal_multiple_calls() {
        let mut signal = ShutdownSignal::new();
        
        // First shutdown
        signal.shutdown();
        
        // Second shutdown should be safe (no panic)
        signal.shutdown();
        
        // Waiting should return immediately
        let start = Instant::now();
        signal.wait().await;
        assert!(start.elapsed() < Duration::from_millis(10));
    }

    #[tokio::test]
    #[should_panic(expected = "rate_per_second must be > 0")]
    async fn test_rate_limiter_zero_rate_panic() {
        let _ = RateLimiter::new(0, 10);
    }

    #[tokio::test]
    #[should_panic(expected = "burst_capacity must be > 0")]
    async fn test_rate_limiter_zero_capacity_panic() {
        let _ = RateLimiter::new(10, 0);
    }

    // Stress test
    #[tokio::test]
    async fn stress_test_rate_limiter() {
        let limiter = Arc::new(RateLimiter::new(10000, 1000));
        let duration = Duration::from_secs(1);
        let start = Instant::now();
        let mut handles = Vec::new();

        // 100 concurrent tasks
        for _ in 0..100 {
            let limiter_clone = limiter.clone();
            let handle = tokio::spawn(async move {
                let mut count = 0;
                let task_start = Instant::now();
                while task_start.elapsed() < duration {
                    if limiter_clone.try_acquire(1) {
                        count += 1;
                    } else {
                        sleep(Duration::from_micros(100)).await;
                    }
                }
                count
            });
            handles.push(handle);
        }

        let counts: Vec<u32> = futures::future::try_join_all(handles)
            .await
            .unwrap();

        let total: u32 = counts.iter().sum();
        let elapsed = start.elapsed();
        
        // Should be close to rate limit
        let expected = (10000.0 * elapsed.as_secs_f64()) as u32;
        let tolerance = expected / 10; // 10% tolerance
        
        assert!(
            total >= expected - tolerance && total <= expected + tolerance,
            "Total {} not within tolerance of expected {}",
            total, expected
        );
    }
}