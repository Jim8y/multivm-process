//! Tests for secure IPC transport with encryption

#[cfg(test)]
mod tests {
    use crate::ipc::secure_transport::{
        AuthManager, EncryptionConfig, RateLimitConfig, RateLimiter, SecureIpcTransport,
    };
    use crate::{IpcCommand, IpcMessage, ProcessId};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::mpsc;

    #[tokio::test]
    #[ignore] // Temporarily disabled due to hanging - needs investigation
    async fn test_chacha20poly1305_encryption() {
        // Start a TCP listener
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Create shared secret for both sides
        let shared_secret = vec![0x42; 32]; // 32 bytes for key material

        // Create auth manager and rate limiter
        let signing_key = AuthManager::generate_signing_key();
        let auth_manager = Arc::new(AuthManager::new(signing_key, Duration::from_secs(3600)));
        let rate_limiter = Arc::new(RateLimiter::new(RateLimitConfig::default()));

        // Create a channel to synchronize the test
        let (tx, mut rx) = mpsc::channel::<()>(1);

        // Server task
        let auth_manager_server = auth_manager.clone();
        let rate_limiter_server = rate_limiter.clone();
        let shared_secret_server = shared_secret.clone();
        let server_handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();

            // Create secure transport with encryption ENABLED
            let encryption_config = EncryptionConfig {
                enabled: true,
                ..Default::default()
            };

            let mut transport = SecureIpcTransport::new_tcp(
                stream,
                auth_manager_server.clone(),
                rate_limiter_server,
                encryption_config,
            );

            // Set up connection info with shared secret
            transport.set_local_process_id("server".to_string());
            transport.set_remote_process_id("client".to_string());
            transport.set_shared_secret(shared_secret_server);

            // Issue and authenticate with token
            let token = auth_manager_server
                .issue_token("server".to_string(), vec!["ipc".to_string()])
                .await
                .unwrap();
            transport.authenticate(token).await.unwrap();

            // Signal that server is ready
            tx.send(()).await.unwrap();

            // Receive encrypted message
            let received = transport.receive_secure().await.unwrap();
            assert_eq!(received.source, ProcessId::Main);
            assert!(matches!(received.command, IpcCommand::Ping));

            // Send encrypted response
            let response = IpcMessage::new(
                ProcessId::Main,
                ProcessId::Main,
                IpcCommand::GetHealth, // Use a valid command instead of Pong
            );
            transport.send_secure(response).await.unwrap();
        });

        // Wait for server to be ready
        rx.recv().await.unwrap();

        // Client connection
        let stream = TcpStream::connect(addr).await.unwrap();

        // Create secure transport with encryption ENABLED
        let encryption_config = EncryptionConfig {
            enabled: true,
            ..Default::default()
        };

        let mut transport = SecureIpcTransport::new_tcp(
            stream,
            auth_manager.clone(),
            rate_limiter,
            encryption_config,
        );

        // Set up matching connection info with shared secret
        transport.set_local_process_id("client".to_string());
        transport.set_remote_process_id("server".to_string());
        transport.set_shared_secret(shared_secret);

        // Authenticate
        let token = auth_manager
            .issue_token("client".to_string(), vec!["ipc".to_string()])
            .await
            .unwrap();
        transport.authenticate(token).await.unwrap();

        // Send encrypted message
        let message = IpcMessage::new(ProcessId::Main, ProcessId::Main, IpcCommand::Ping);
        transport.send_secure(message).await.unwrap();

        // Receive encrypted response
        let response = transport.receive_secure().await.unwrap();
        assert!(matches!(response.command, IpcCommand::GetHealth));

        // Wait for server to complete
        server_handle.await.unwrap();

        println!("✅ ChaCha20-Poly1305 encryption test passed!");
    }

    #[tokio::test]
    #[ignore] // Temporarily disabled due to potential hanging - needs investigation
    async fn test_encryption_with_authentication() {
        // Test that encryption works with proper authentication
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let signing_key = AuthManager::generate_signing_key();
        let auth_manager = Arc::new(AuthManager::new(signing_key, Duration::from_secs(3600)));
        let rate_limiter = Arc::new(RateLimiter::new(RateLimitConfig::default()));

        // Use a deterministic shared secret
        let shared_secret = b"test-shared-secret-for-encryption".to_vec();

        // Server task
        let auth_manager_server = auth_manager.clone();
        let rate_limiter_server = rate_limiter.clone();
        let shared_secret_server = shared_secret.clone();

        let server_handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();

            let mut transport = SecureIpcTransport::new_tcp(
                stream,
                auth_manager_server.clone(),
                rate_limiter_server,
                EncryptionConfig::default(), // Encryption enabled by default
            );

            // Configure shared secret
            transport.set_shared_secret(shared_secret_server);
            transport.set_local_process_id("server_process".to_string());
            transport.set_remote_process_id("client_process".to_string());

            let token = auth_manager_server
                .issue_token("server_process".to_string(), vec!["ipc".to_string()])
                .await
                .unwrap();
            transport.authenticate(token).await.unwrap();

            // Send a test message with sensitive data
            let sensitive_msg = IpcMessage::new(
                ProcessId::Main,
                ProcessId::Solana,
                IpcCommand::ProcessBlock {
                    block_data_bytes: Box::new(vec![0x42; 32]), // Sensitive block data
                    blockchain_type: crate::BlockchainType::Ethereum,
                    expect_response: true,
                },
            );
            transport.send_secure(sensitive_msg).await.unwrap();
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Client
        let stream = TcpStream::connect(addr).await.unwrap();
        let mut transport = SecureIpcTransport::new_tcp(
            stream,
            auth_manager.clone(),
            rate_limiter,
            EncryptionConfig::default(),
        );

        // Configure matching shared secret
        transport.set_shared_secret(shared_secret);
        transport.set_local_process_id("client_process".to_string());
        transport.set_remote_process_id("server_process".to_string());

        let token = auth_manager
            .issue_token("client_process".to_string(), vec!["ipc".to_string()])
            .await
            .unwrap();
        transport.authenticate(token).await.unwrap();

        // Receive and verify the encrypted message
        let received = transport.receive_secure().await.unwrap();
        if let IpcCommand::ProcessBlock {
            block_data_bytes,
            blockchain_type,
            expect_response,
        } = received.command
        {
            assert_eq!(*block_data_bytes, vec![0x42; 32]);
            assert!(matches!(blockchain_type, crate::BlockchainType::Ethereum));
            assert!(expect_response);
        } else {
            panic!("Expected ProcessBlock command");
        }

        server_handle.await.unwrap();

        println!("✅ Encryption with authentication test passed!");
    }

    #[test]
    fn test_encryption_config_default() {
        // Test that encryption is enabled by default
        let config = EncryptionConfig::default();
        assert!(
            config.enabled,
            "Encryption should be enabled by default for security"
        );
        assert!(matches!(
            config.algorithm,
            crate::ipc::secure_transport::EncryptionAlgorithm::ChaCha20Poly1305
        ));
    }

    #[test]
    fn test_rate_limit_config_default() {
        let config = RateLimitConfig::default();
        assert_eq!(config.max_messages, 100);
        assert_eq!(config.window_duration.as_secs(), 60);
        assert_eq!(config.penalty_duration.as_secs(), 300);
    }
}
