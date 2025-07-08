 //! Example: Secure IPC Client for Reth Communication
//!
//! This example demonstrates how to use the SecureRethIpcClient with encryption,
//! message queuing, connection recovery, and health monitoring.

use reth_execution_engine::ipc_client::{
    IpcClientConfig, SecureRethIpcClient
};
use multivm_common::{IpcCommand, IpcResponse};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Starting Secure IPC Client Example");

    // Create configuration for secure IPC client
    let config = IpcClientConfig {
        address: "/tmp/reth.ipc".to_string(),
        connect_timeout_ms: 5000,
        request_timeout_ms: 30000,
        max_retries: 3,
        retry_delay_ms: 1000,
        enable_encryption: true,
        encryption_key: None, // Will auto-generate from address
        health_check_interval_ms: 10000,
        max_queue_size: 1000,
        recovery_interval_ms: 5000,
        enable_keepalive: true,
        keepalive_interval_ms: 30000,
    };

    // Create the secure IPC client
    let mut client = SecureRethIpcClient::new(config.clone())
        .expect("Failed to create secure IPC client");

    info!("Created secure IPC client for: {}", config.address);

    // Start the client (this will establish connection and start background tasks)
    match client.start().await {
        Ok(()) => {
            info!("IPC client started successfully");
        }
        Err(e) => {
            error!("Failed to start IPC client: {}", e);
            return Ok(());
        }
    }

    // Demonstrate basic health check
    demonstrate_health_monitoring(&client).await;

    // Demonstrate secure communication
    demonstrate_secure_communication(&client).await;

    // Demonstrate connection recovery
    demonstrate_connection_recovery(&client).await;

    // Wait for some operations to complete
    sleep(Duration::from_secs(5)).await;

    // Stop the client
    match client.stop().await {
        Ok(()) => {
            info!("IPC client stopped successfully");
        }
        Err(e) => {
            error!("Failed to stop IPC client: {}", e);
        }
    }

    info!("Secure IPC Client Example completed");
    Ok(())
}

/// Demonstrate health monitoring features
async fn demonstrate_health_monitoring(client: &SecureRethIpcClient) {
    info!("=== Demonstrating Health Monitoring ===");

    // Check initial health status
    let health_status = client.get_health_status().await;
    info!("Initial health status: {:?}", health_status.status);
    info!("Connection metrics: {:?}", health_status.metrics);

    // Check if the connection is healthy
    let is_healthy = client.is_healthy();
    info!("Connection is healthy: {}", is_healthy);

    // Get detailed metrics
    let metrics = client.get_metrics().await;
    info!("Total requests: {}", metrics.total_requests);
    info!("Successful requests: {}", metrics.successful_requests);
    info!("Failed requests: {}", metrics.failed_requests);
    info!("Connection count: {}", metrics.connection_count);
}

/// Demonstrate secure communication with encryption
async fn demonstrate_secure_communication(client: &SecureRethIpcClient) {
    info!("=== Demonstrating Secure Communication ===");

    // Create various IPC commands to test the communication
    let commands = vec![
        IpcCommand::GetState,
        IpcCommand::Ping,
        IpcCommand::GetHealth,
        IpcCommand::HealthCheck,
    ];

    for (index, command) in commands.iter().enumerate() {
        info!("Sending command {}: {:?}", index + 1, command);

        // Send command with automatic retry
        match client.send_command_with_retry(command.clone()).await {
            Ok(response) => {
                info!("Received response for command {}: {:?}", index + 1, response);
            }
            Err(e) => {
                warn!("Failed to send command {}: {}", index + 1, e);
            }
        }

        // Small delay between commands
        sleep(Duration::from_millis(500)).await;
    }
}

/// Demonstrate connection recovery features
async fn demonstrate_connection_recovery(client: &SecureRethIpcClient) {
    info!("=== Demonstrating Connection Recovery ===");

    // Monitor health status changes
    for i in 1..=5 {
        sleep(Duration::from_secs(2)).await;
        
        let health_status = client.get_health_status().await;
        info!("Health check {}: status = {:?}, healthy = {}", 
              i, health_status.status, client.is_healthy());

        // Try to send a command to test the connection
        match client.send_command(IpcCommand::GetHealth).await {
            Ok(_) => {
                info!("Command sent successfully during health check {}", i);
            }
            Err(e) => {
                warn!("Command failed during health check {}: {}", i, e);
            }
        }
    }
}

/// Helper function to create a test IPC response
#[allow(dead_code)]
fn create_test_response() -> IpcResponse {
    // Use a simple response type to avoid type complexity
    IpcResponse::Pong
}

/// Helper function to create various test commands
#[allow(dead_code)]
fn create_test_commands() -> Vec<IpcCommand> {
    use multivm_common::BlockchainType;
    vec![
        IpcCommand::GetState,
        IpcCommand::Ping,
        IpcCommand::GetHealth,
        IpcCommand::HealthCheck,
        IpcCommand::ProcessBlock { 
            block_data_bytes: Box::new(vec![1, 2, 3, 4]),
            blockchain_type: BlockchainType::Ethereum,
            expect_response: true,
        },
    ]
}

/// Demonstrate metrics collection
#[allow(dead_code)]
async fn demonstrate_metrics_collection(client: &SecureRethIpcClient) {
    info!("=== Demonstrating Metrics Collection ===");

    // Send multiple commands to generate metrics
    for i in 1..=10 {
        let command = IpcCommand::GetState;
        
        match client.send_command(command).await {
            Ok(_) => {
                info!("Command {} completed successfully", i);
            }
            Err(e) => {
                warn!("Command {} failed: {}", i, e);
            }
        }
    }

    // Display final metrics
    let metrics = client.get_metrics().await;
    info!("Final metrics:");
    info!("  Total requests: {}", metrics.total_requests);
    info!("  Successful requests: {}", metrics.successful_requests);
    info!("  Failed requests: {}", metrics.failed_requests);
    info!("  Success rate: {:.2}%", 
          (metrics.successful_requests as f64 / metrics.total_requests as f64) * 100.0);
    info!("  Average response time: {} ms", metrics.average_response_time_ms);
}

/// Demonstrate encryption and security features
#[allow(dead_code)]
async fn demonstrate_encryption_features() {
    info!("=== Demonstrating Encryption Features ===");

    // Create client with custom encryption key
    let custom_key: [u8; 32] = [
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
        17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32
    ];

    let secure_config = IpcClientConfig {
        address: "/tmp/reth_secure.ipc".to_string(),
        enable_encryption: true,
        encryption_key: Some(custom_key),
        ..Default::default()
    };

    match SecureRethIpcClient::new(secure_config) {
        Ok(mut secure_client) => {
            info!("Created client with custom encryption key");
            
            if let Err(e) = secure_client.start().await {
                warn!("Failed to start secure client: {}", e);
                return;
            }

            // Test encrypted communication
            let command = IpcCommand::GetState;
            match secure_client.send_command(command).await {
                Ok(response) => {
                    info!("Encrypted communication successful: {:?}", response);
                }
                Err(e) => {
                    warn!("Encrypted communication failed: {}", e);
                }
            }

            let _ = secure_client.stop().await;
        }
        Err(e) => {
            error!("Failed to create secure client: {}", e);
        }
    }
}

/// Configuration examples for different use cases
#[allow(dead_code)]
fn demonstrate_configuration_examples() {
    info!("=== Configuration Examples ===");

    // High-performance configuration
    let _high_perf_config = IpcClientConfig {
        address: "/tmp/reth_highperf.ipc".to_string(),
        connect_timeout_ms: 2000,
        request_timeout_ms: 10000,
        max_retries: 1,
        retry_delay_ms: 100,
        enable_encryption: false, // Disabled for max performance
        max_queue_size: 10000,
        health_check_interval_ms: 5000,
        recovery_interval_ms: 1000,
        enable_keepalive: true,
        keepalive_interval_ms: 10000,
        ..Default::default()
    };

    // High-security configuration
    let _secure_config = IpcClientConfig {
        address: "127.0.0.1:8551".to_string(), // TCP with authentication
        connect_timeout_ms: 10000,
        request_timeout_ms: 60000,
        max_retries: 5,
        retry_delay_ms: 2000,
        enable_encryption: true,
        encryption_key: Some([42; 32]), // Custom key
        max_queue_size: 100,
        health_check_interval_ms: 30000,
        recovery_interval_ms: 10000,
        enable_keepalive: true,
        keepalive_interval_ms: 60000,
        ..Default::default()
    };

    // Development configuration
    let _dev_config = IpcClientConfig {
        address: "/tmp/reth_dev.ipc".to_string(),
        connect_timeout_ms: 30000, // Longer timeout for debugging
        request_timeout_ms: 120000,
        max_retries: 10,
        retry_delay_ms: 5000,
        enable_encryption: false,
        max_queue_size: 50,
        health_check_interval_ms: 60000,
        recovery_interval_ms: 30000,
        enable_keepalive: false,
        keepalive_interval_ms: 0,
        ..Default::default()
    };

    info!("Configuration examples defined for different use cases");
}