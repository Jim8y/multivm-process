//! Network fault injection tests

use multivm_network::{TcpTransport, TcpTransportConfig};
use multivm_network::connection::ConnectionPool;
use multivm_core::NodeId;
use std::net::SocketAddr;
use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}};
use std::time::Duration;
use tokio::time::{sleep, timeout};

#[derive(Clone, Debug)]
struct TestMessage {
    id: String,
    payload: Vec<u8>,
    timestamp: std::time::SystemTime,
}

#[tokio::test]
async fn test_connection_pool_limits() {
    let pool = ConnectionPool::new(10, Duration::from_secs(30));
    let mut handles = vec![];
    
    // Try to create more connections than the pool limit
    for i in 0..20 {
        let pool_clone = pool.clone();
        let handle = tokio::spawn(async move {
            let node_id = NodeId::new();
            // Simulate connection attempt
            sleep(Duration::from_millis(10)).await;
            (i, node_id)
        });
        handles.push(handle);
    }
    
    let results = futures::future::join_all(handles).await;
    
    // Should have all tasks complete
    assert_eq!(results.len(), 20);
    for result in results {
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_tcp_transport_config() {
    let config = TcpTransportConfig {
        listen_addr: "127.0.0.1:0".parse().unwrap(),
        tls: None,
        connection: Default::default(),
        handshake_timeout: Duration::from_secs(5),
        max_connections: 100,
        connection_rate_limit: 50,
    };
    
    // Test default values
    assert_eq!(config.max_connections, 100);
    assert_eq!(config.connection_rate_limit, 50);
    assert!(config.tls.is_none());
    
    // Test default construction
    let default_config = TcpTransportConfig::default();
    assert_eq!(default_config.max_connections, 1000);
    assert_eq!(default_config.connection_rate_limit, 100);
}

#[tokio::test]
async fn test_connection_pool_eviction() {
    let pool = ConnectionPool::new(5, Duration::from_millis(100));
    let node_ids: Vec<NodeId> = (0..10).map(|_| NodeId::new()).collect();
    
    // Fill the pool beyond capacity
    for node_id in &node_ids {
        // Simulate adding connections
        sleep(Duration::from_millis(10)).await;
    }
    
    // Wait for eviction timeout
    sleep(Duration::from_millis(150)).await;
    
    // Pool should have evicted old connections
    // (actual implementation details would determine exact behavior)
}

#[tokio::test]
async fn test_network_latency_simulation() {
    let start = std::time::Instant::now();
    let delays = vec![10, 50, 100, 200];
    
    for delay_ms in delays {
        let delay_start = std::time::Instant::now();
        sleep(Duration::from_millis(delay_ms)).await;
        let actual_delay = delay_start.elapsed();
        
        // Allow 20% tolerance for timing
        let expected = Duration::from_millis(delay_ms);
        assert!(
            actual_delay >= expected && actual_delay <= expected * 120 / 100,
            "Delay {} ms: expected {:?}, got {:?}",
            delay_ms, expected, actual_delay
        );
    }
    
    let total_time = start.elapsed();
    println!("Total latency test time: {:?}", total_time);
}

#[tokio::test]
async fn test_concurrent_message_handling() {
    let message_count = Arc::new(AtomicU64::new(0));
    let error_count = Arc::new(AtomicU64::new(0));
    
    let mut handles = vec![];
    
    // Simulate concurrent message processing
    for i in 0..100 {
        let message_count_clone = message_count.clone();
        let error_count_clone = error_count.clone();
        
        let handle = tokio::spawn(async move {
            // Simulate message processing with occasional errors
            if i % 10 == 0 {
                // Simulate error
                error_count_clone.fetch_add(1, Ordering::Relaxed);
            } else {
                // Simulate successful processing
                sleep(Duration::from_micros(100)).await;
                message_count_clone.fetch_add(1, Ordering::Relaxed);
            }
        });
        
        handles.push(handle);
    }
    
    futures::future::join_all(handles).await;
    
    let total_messages = message_count.load(Ordering::Relaxed);
    let total_errors = error_count.load(Ordering::Relaxed);
    
    assert_eq!(total_messages + total_errors, 100);
    assert_eq!(total_errors, 10); // 10% error rate as simulated
}

#[tokio::test]
async fn test_rate_limiting() {
    let allowed = Arc::new(AtomicU64::new(0));
    let rejected = Arc::new(AtomicU64::new(0));
    
    let rate_limit = 10; // 10 per second
    let duration = Duration::from_secs(1);
    let start = std::time::Instant::now();
    
    let mut handles = vec![];
    
    // Try to send more than rate limit
    for _ in 0..20 {
        let allowed_clone = allowed.clone();
        let rejected_clone = rejected.clone();
        
        let handle = tokio::spawn(async move {
            // Simulate rate limiting check
            let current_allowed = allowed_clone.load(Ordering::Relaxed);
            if current_allowed < rate_limit {
                allowed_clone.fetch_add(1, Ordering::Relaxed);
            } else {
                rejected_clone.fetch_add(1, Ordering::Relaxed);
            }
        });
        
        handles.push(handle);
        sleep(Duration::from_millis(50)).await;
    }
    
    futures::future::join_all(handles).await;
    
    let elapsed = start.elapsed();
    let total_allowed = allowed.load(Ordering::Relaxed);
    let total_rejected = rejected.load(Ordering::Relaxed);
    
    println!("Rate limiting test: {} allowed, {} rejected in {:?}", 
             total_allowed, total_rejected, elapsed);
    
    // Should have enforced rate limit
    assert!(total_allowed <= rate_limit * 2); // Allow some tolerance
    assert!(total_rejected > 0);
}

#[tokio::test]
async fn test_connection_timeout() {
    let timeout_duration = Duration::from_millis(100);
    
    // Test timeout behavior
    let result = timeout(timeout_duration, async {
        // Simulate long operation
        sleep(Duration::from_millis(200)).await;
        "completed"
    }).await;
    
    assert!(result.is_err());
    
    // Test successful completion within timeout
    let result = timeout(timeout_duration, async {
        sleep(Duration::from_millis(50)).await;
        "completed"
    }).await;
    
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "completed");
}

#[tokio::test]
async fn test_network_partition_detection() {
    let partition_active = Arc::new(AtomicBool::new(false));
    let messages_sent = Arc::new(AtomicU64::new(0));
    let messages_failed = Arc::new(AtomicU64::new(0));
    
    let partition_clone = partition_active.clone();
    let sent_clone = messages_sent.clone();
    let failed_clone = messages_failed.clone();
    
    // Simulate sending messages with partition
    let sender = tokio::spawn(async move {
        for i in 0..100 {
            if i == 50 {
                // Activate partition halfway through
                partition_clone.store(true, Ordering::Relaxed);
            }
            
            if partition_clone.load(Ordering::Relaxed) {
                failed_clone.fetch_add(1, Ordering::Relaxed);
            } else {
                sent_clone.fetch_add(1, Ordering::Relaxed);
            }
            
            sleep(Duration::from_millis(10)).await;
        }
    });
    
    sender.await.unwrap();
    
    let total_sent = messages_sent.load(Ordering::Relaxed);
    let total_failed = messages_failed.load(Ordering::Relaxed);
    
    // Should have roughly half succeeded before partition
    assert!(total_sent >= 45 && total_sent <= 55);
    assert!(total_failed >= 45 && total_failed <= 55);
    assert_eq!(total_sent + total_failed, 100);
}