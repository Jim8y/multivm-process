//! Comprehensive tests for IPC transport layer

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc_transport::{IpcTransport, IpcTransportConfig, SecureIpcTransport};
    use multivm_common::{
        IpcMessage, IpcCommand, IpcResponse, ProcessId,
        types::core::MessageId,
    };
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::test;
    use tracing_test::traced_test;

    fn create_test_config() -> (IpcTransportConfig, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test_ipc.sock");
        
        let config = IpcTransportConfig::UnixSocket {
            path: socket_path,
            permissions: 0o600,
            timeout: Duration::from_secs(10),
            buffer_size: 8192,
        };
        
        (config, temp_dir)
    }

    #[traced_test]
    #[test]
    async fn test_ipc_transport_creation() {
        let (config, _temp_dir) = create_test_config();
        
        let transport = IpcTransport::new(config).await;
        assert!(transport.is_ok(), "Failed to create IPC transport: {:?}", transport.err());
    }

    #[traced_test]
    #[test]
    async fn test_ipc_transport_lifecycle() {
        let (config, _temp_dir) = create_test_config();
        
        let mut transport = IpcTransport::new(config).await.unwrap();
        
        // Test start
        let start_result = transport.start().await;
        assert!(start_result.is_ok(), "Failed to start IPC transport: {:?}", start_result.err());
        
        // Test health check
        assert!(transport.is_healthy().await);
        
        // Test stop
        let stop_result = transport.stop().await;
        assert!(stop_result.is_ok(), "Failed to stop IPC transport: {:?}", stop_result.err());
    }

    #[traced_test]
    #[test]
    async fn test_message_send_receive() {
        let (config, _temp_dir) = create_test_config();
        
        let mut transport = IpcTransport::new(config).await.unwrap();
        transport.start().await.unwrap();

        // Create test message
        let test_message = IpcMessage::new(
            ProcessId::Main,
            ProcessId::Solana,
            IpcCommand::Ping
        );

        // Test send
        let send_result = transport.send_message(test_message.clone()).await;
        assert!(send_result.is_ok(), "Failed to send message: {:?}", send_result.err());

        // Test receive with timeout
        let receive_result = transport.receive_message(Duration::from_secs(1)).await;
        assert!(receive_result.is_ok(), "Failed to receive message: {:?}", receive_result.err());

        let received_message = receive_result.unwrap();
        assert_eq!(received_message.source, test_message.source);
        assert_eq!(received_message.destination, test_message.destination);

        transport.stop().await.unwrap();
    }

    #[traced_test]
    #[test]
    async fn test_request_response_pattern() {
        let (config, _temp_dir) = create_test_config();
        
        let mut transport = IpcTransport::new(config).await.unwrap();
        transport.start().await.unwrap();

        // Create request message
        let request = IpcMessage::new(
            ProcessId::Main,
            ProcessId::Solana,
            IpcCommand::GetHealth
        );

        // Send request and wait for response
        let response_result = transport.send_request(request, Duration::from_secs(5)).await;
        
        // In a real scenario, another process would handle this request
        // For testing, we'll simulate the response
        let test_response = IpcResponse::Health {
            status: multivm_common::types::health::HealthStatus::Healthy,
            details: "Test health response".to_string(),
        };

        // Test response handling
        let response_id = MessageId::new();
        let handle_result = transport.handle_response(response_id, test_response).await;
        assert!(handle_result.is_ok(), "Failed to handle response: {:?}", handle_result.err());

        transport.stop().await.unwrap();
    }

    #[traced_test]
    #[test]
    async fn test_concurrent_messages() {
        let (config, _temp_dir) = create_test_config();
        
        let mut transport = IpcTransport::new(config).await.unwrap();
        transport.start().await.unwrap();

        // Send multiple messages concurrently
        let mut handles = vec![];
        for i in 0..10 {
            let message = IpcMessage::new(
                ProcessId::Main,
                ProcessId::Solana,
                IpcCommand::Ping
            ).with_timeout(Duration::from_secs(5));

            let transport_ref = &transport;
            let handle = tokio::spawn(async move {
                transport_ref.send_message(message).await
            });
            handles.push(handle);
        }

        // Wait for all messages to be sent
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok(), "Failed to send concurrent message: {:?}", result.err());
        }

        transport.stop().await.unwrap();
    }

    #[traced_test]
    #[test]
    async fn test_message_timeout() {
        let (config, _temp_dir) = create_test_config();
        
        let mut transport = IpcTransport::new(config).await.unwrap();
        transport.start().await.unwrap();

        // Create message with short timeout
        let message = IpcMessage::new(
            ProcessId::Main,
            ProcessId::Solana,
            IpcCommand::GetHealth
        ).with_timeout(Duration::from_millis(1));

        // This should timeout quickly
        let start_time = std::time::Instant::now();
        let result = transport.send_request(message, Duration::from_millis(100)).await;
        let elapsed = start_time.elapsed();

        assert!(result.is_err(), "Request should have timed out");
        assert!(elapsed < Duration::from_millis(200), "Timeout should have occurred quickly");

        transport.stop().await.unwrap();
    }

    #[traced_test]
    #[test]
    async fn test_large_message_handling() {
        let (config, _temp_dir) = create_test_config();
        
        let mut transport = IpcTransport::new(config).await.unwrap();
        transport.start().await.unwrap();

        // Create large message payload
        let large_payload = vec![0u8; 1024 * 1024]; // 1MB
        let large_message = IpcMessage::new(
            ProcessId::Main,
            ProcessId::Solana,
            IpcCommand::Custom {
                command: "large_data".to_string(),
                payload: large_payload,
            }
        );

        let send_result = transport.send_message(large_message).await;
        assert!(send_result.is_ok(), "Failed to send large message: {:?}", send_result.err());

        transport.stop().await.unwrap();
    }

    #[traced_test]
    #[test]
    async fn test_connection_recovery() {
        let (config, _temp_dir) = create_test_config();
        
        let mut transport = IpcTransport::new(config).await.unwrap();
        transport.start().await.unwrap();

        // Force a connection error by stopping and restarting
        transport.stop().await.unwrap();

        // Attempt to send message (should trigger reconnection)
        let message = IpcMessage::new(
            ProcessId::Main,
            ProcessId::Solana,
            IpcCommand::Ping
        );

        // Restart transport
        transport.start().await.unwrap();
        
        let send_result = transport.send_message(message).await;
        assert!(send_result.is_ok(), "Failed to recover connection: {:?}", send_result.err());

        transport.stop().await.unwrap();
    }

    #[traced_test]
    #[test]
    async fn test_secure_ipc_transport() {
        let (config, _temp_dir) = create_test_config();
        
        // Create secure transport with encryption
        let mut secure_transport = SecureIpcTransport::new(config, "test_key".to_string()).await;
        assert!(secure_transport.is_ok(), "Failed to create secure transport: {:?}", secure_transport.err());
        
        let mut transport = secure_transport.unwrap();
        transport.start().await.unwrap();

        // Test encrypted message
        let message = IpcMessage::new(
            ProcessId::Main,
            ProcessId::Solana,
            IpcCommand::GetHealth
        );

        let send_result = transport.send_encrypted_message(message).await;
        assert!(send_result.is_ok(), "Failed to send encrypted message: {:?}", send_result.err());

        transport.stop().await.unwrap();
    }

    #[traced_test]
    #[test]
    async fn test_message_queuing() {
        let (config, _temp_dir) = create_test_config();
        
        let mut transport = IpcTransport::new(config).await.unwrap();
        
        // Send messages before transport is started (should queue)
        let message1 = IpcMessage::new(
            ProcessId::Main,
            ProcessId::Solana,
            IpcCommand::Ping
        );
        let message2 = IpcMessage::new(
            ProcessId::Main,
            ProcessId::Ethereum,
            IpcCommand::Ping
        );

        let queue_result1 = transport.queue_message(message1).await;
        let queue_result2 = transport.queue_message(message2).await;
        
        assert!(queue_result1.is_ok(), "Failed to queue message 1: {:?}", queue_result1.err());
        assert!(queue_result2.is_ok(), "Failed to queue message 2: {:?}", queue_result2.err());

        // Start transport and process queued messages
        transport.start().await.unwrap();
        
        let process_result = transport.process_queued_messages().await;
        assert!(process_result.is_ok(), "Failed to process queued messages: {:?}", process_result.err());

        transport.stop().await.unwrap();
    }

    #[traced_test]
    #[test]
    async fn test_transport_metrics() {
        let (config, _temp_dir) = create_test_config();
        
        let mut transport = IpcTransport::new(config).await.unwrap();
        transport.start().await.unwrap();

        // Send multiple messages to generate metrics
        for i in 0..5 {
            let message = IpcMessage::new(
                ProcessId::Main,
                ProcessId::Solana,
                IpcCommand::Ping
            );
            transport.send_message(message).await.unwrap();
        }

        // Get transport metrics
        let metrics = transport.get_metrics().await;
        assert!(metrics.is_ok(), "Failed to get transport metrics: {:?}", metrics.err());
        
        let transport_metrics = metrics.unwrap();
        assert!(transport_metrics.messages_sent >= 5);
        assert!(transport_metrics.total_bytes_sent > 0);
        assert_eq!(transport_metrics.connection_errors, 0);

        transport.stop().await.unwrap();
    }

    #[traced_test]
    #[test]
    async fn test_error_handling() {
        let (mut config, _temp_dir) = create_test_config();
        
        // Create invalid config
        if let IpcTransportConfig::UnixSocket { ref mut path, .. } = config {
            *path = std::path::PathBuf::from("/invalid/path/that/does/not/exist.sock");
        }
        
        let transport_result = IpcTransport::new(config).await;
        assert!(transport_result.is_err(), "Should have failed with invalid path");
        
        let error = transport_result.unwrap_err();
        assert!(matches!(error, multivm_common::MultivmError::Network { .. }));
    }

    #[traced_test]
    #[test]
    async fn test_transport_configuration_validation() {
        let temp_dir = TempDir::new().unwrap();
        
        // Test various invalid configurations
        let invalid_configs = vec![
            IpcTransportConfig::UnixSocket {
                path: temp_dir.path().join("test.sock"),
                permissions: 0o777, // Too permissive
                timeout: Duration::from_secs(0), // Invalid timeout
                buffer_size: 0, // Invalid buffer size
            },
            IpcTransportConfig::TcpSocket {
                host: "".to_string(), // Empty host
                port: 0, // Invalid port
                timeout: Duration::from_secs(60),
                buffer_size: 8192,
            },
        ];

        for config in invalid_configs {
            let result = IpcTransport::validate_config(&config);
            assert!(result.is_err(), "Should have failed validation for config: {:?}", config);
        }
    }

    #[traced_test]
    #[test]
    async fn test_high_frequency_messaging() {
        let (config, _temp_dir) = create_test_config();
        
        let mut transport = IpcTransport::new(config).await.unwrap();
        transport.start().await.unwrap();

        // Send many messages in rapid succession
        let start_time = std::time::Instant::now();
        let message_count = 1000;

        for i in 0..message_count {
            let message = IpcMessage::new(
                ProcessId::Main,
                ProcessId::Solana,
                IpcCommand::Custom {
                    command: format!("msg_{}", i),
                    payload: vec![i as u8; 100],
                }
            );
            
            let result = transport.send_message(message).await;
            assert!(result.is_ok(), "Failed to send message {}: {:?}", i, result.err());
        }

        let elapsed = start_time.elapsed();
        let messages_per_second = message_count as f64 / elapsed.as_secs_f64();
        
        println!("Sent {} messages in {:?} ({:.2} msgs/sec)", 
                 message_count, elapsed, messages_per_second);
        
        // Verify reasonable performance (at least 100 msgs/sec)
        assert!(messages_per_second > 100.0, 
                "Message throughput too low: {:.2} msgs/sec", messages_per_second);

        transport.stop().await.unwrap();
    }
}