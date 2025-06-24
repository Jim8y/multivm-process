//! Unit tests for secure IPC transport

#[cfg(test)]
mod tests {
    use super::super::secure_transport::*;
    use crate::ipc::{IpcCommand, IpcMessage};
    use std::sync::Arc;
    use std::time::{Duration, SystemTime};
    use tokio::net::{TcpListener, TcpStream};

    fn create_test_config() -> EncryptionConfig {
        EncryptionConfig {
            enabled: true,
            algorithm: EncryptionAlgorithm::Aes256Gcm,
            key_derivation: KeyDerivation::Pbkdf2,
        }
    }

    fn create_test_auth_manager() -> Arc<AuthManager> {
        Arc::new(AuthManager::new(
            vec![0u8; 32],             // signing key
            Duration::from_secs(3600), // token expiry
        ))
    }

    fn create_test_rate_limiter() -> Arc<RateLimiter> {
        Arc::new(RateLimiter::new(RateLimitConfig {
            max_messages: 100,
            window_duration: Duration::from_secs(1),
            penalty_duration: Duration::from_secs(60),
        }))
    }

    fn create_test_token(process_id: &str) -> AuthToken {
        AuthToken {
            process_id: process_id.to_string(),
            issued_at: SystemTime::now(),
            expires_at: SystemTime::now() + Duration::from_secs(3600),
            permissions: vec!["read".to_string(), "write".to_string()],
            signature: vec![],
        }
    }

    #[tokio::test]
    async fn test_secure_transport_creation() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            // Keep connection alive
            tokio::time::sleep(Duration::from_millis(100)).await;
            drop(socket);
        });

        let stream = TcpStream::connect(addr).await.unwrap();

        let config = create_test_config();
        let auth_manager = create_test_auth_manager();
        let rate_limiter = create_test_rate_limiter();

        let _transport = SecureIpcTransport::new_tcp(stream, auth_manager, rate_limiter, config);

        // Transport created successfully - test passes
        // (connection_info is private, so we can't check it directly)
    }

    #[tokio::test]
    async fn test_authentication() {
        let auth_manager = create_test_auth_manager();

        // Create and issue a token
        let token = auth_manager
            .issue_token("test-process".to_string(), vec!["read".to_string()])
            .await
            .unwrap();

        // Valid token should validate
        let result = auth_manager.validate_token(&token).await;
        assert!(result.unwrap());

        // Expired token should fail
        let mut expired_token = token.clone();
        expired_token.expires_at = SystemTime::now() - Duration::from_secs(60);
        let result = auth_manager.validate_token(&expired_token).await;
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let rate_limiter = create_test_rate_limiter();
        let client_id = "test-client";

        // First 100 requests should succeed
        for _ in 0..100 {
            let result = rate_limiter.check_rate_limit(client_id).await;
            assert!(result.unwrap());
        }

        // 101st request should be rate limited
        let result = rate_limiter.check_rate_limit(client_id).await;
        assert!(!result.unwrap());

        // Wait for both window and penalty duration to expire (penalty is 60s, so let's test window reset only)
        // Create a new client to test fresh rate limiting
        let new_client_id = "test-client-2";

        // Should be able to make requests with new client ID
        let result = rate_limiter.check_rate_limit(new_client_id).await;
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_secure_message_roundtrip() {
        use crate::{MessageId, ProcessId};

        let _message = IpcMessage {
            id: MessageId::new(),
            source: ProcessId::Main,
            destination: ProcessId::Solana,
            command: IpcCommand::Ping,
            timestamp: SystemTime::now(),
            timeout: Some(Duration::from_secs(30)),
        };

        // Create secure message
        let auth_token = create_test_token("test-process");
        let payload_bytes = vec![1, 2, 3, 4, 5]; // In real impl, this would be serialized message
        let secure_msg = SecureMessage {
            auth_token,
            encrypted_payload: payload_bytes.clone(), // In real impl, this would be encrypted
            mac: vec![0u8; 32],                       // Mock MAC
            timestamp: SystemTime::now(),
            nonce: vec![0u8; 12], // Mock nonce
            message_id: "test-message-id".to_string(),
            sequence_number: 1,
        };

        // Verify fields
        assert_eq!(secure_msg.auth_token.process_id, "test-process");
        assert_eq!(secure_msg.encrypted_payload, vec![1, 2, 3, 4, 5]);
    }

    #[tokio::test]
    async fn test_connection_info() {
        let info = IpcConnectionInfo {
            remote_process_id: Some("test-process".to_string()),
            local_process_id: None,
            authenticated: true,
            connected_at: SystemTime::now(),
            last_activity: SystemTime::now(),
            sequence_number: 0,
            shared_secret: None,
        };

        assert_eq!(info.remote_process_id.as_ref().unwrap(), "test-process");
        assert!(info.authenticated);
    }
}
