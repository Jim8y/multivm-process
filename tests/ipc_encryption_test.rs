//! Test IPC encryption functionality

use multivm_common::{
    config::{IpcConfig, IpcTransportConfig},
    ipc::secure_transport::{
        AuthManager, EncryptionConfig, RateLimitConfig, RateLimiter, SecureIpcTransport,
    },
    IpcCommand, IpcMessage, ProcessId,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};

#[tokio::test]
async fn test_ipc_encryption_tcp() {
    // Start a TCP listener
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Create auth manager and rate limiter
    let signing_key = AuthManager::generate_signing_key();
    let auth_manager = Arc::new(AuthManager::new(signing_key, Duration::from_secs(3600)));
    let rate_limiter = Arc::new(RateLimiter::new(RateLimitConfig::default()));

    // Server task
    let auth_manager_server = auth_manager.clone();
    let rate_limiter_server = rate_limiter.clone();
    let server_handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        
        // Create secure transport with encryption enabled
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

        // Issue and authenticate with token
        let token = auth_manager_server
            .issue_token("test_process".to_string(), vec!["ipc".to_string()])
            .await
            .unwrap();
        transport.authenticate(token).await.unwrap();

        // Receive encrypted message
        let received = transport.receive_secure().await.unwrap();
        assert_eq!(received.source, ProcessId::Main);
        assert!(matches!(received.command, IpcCommand::Ping));

        // Send encrypted response
        let response = IpcMessage::new(ProcessId::Main, ProcessId::Main, IpcCommand::Pong);
        transport.send_secure(response).await.unwrap();
    });

    // Client connection
    let stream = TcpStream::connect(addr).await.unwrap();
    
    // Create secure transport with encryption enabled
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

    // Authenticate
    let token = auth_manager
        .issue_token("test_process".to_string(), vec!["ipc".to_string()])
        .await
        .unwrap();
    transport.authenticate(token).await.unwrap();

    // Send encrypted message
    let message = IpcMessage::new(ProcessId::Main, ProcessId::Main, IpcCommand::Ping);
    transport.send_secure(message).await.unwrap();

    // Receive encrypted response
    let response = transport.receive_secure().await.unwrap();
    assert!(matches!(response.command, IpcCommand::Pong));

    // Wait for server to complete
    server_handle.await.unwrap();
    
    println!("✅ IPC encryption test passed with ChaCha20-Poly1305!");
}

#[tokio::test]
async fn test_ipc_encryption_disabled() {
    // Test with encryption disabled to ensure backward compatibility
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let signing_key = AuthManager::generate_signing_key();
    let auth_manager = Arc::new(AuthManager::new(signing_key, Duration::from_secs(3600)));
    let rate_limiter = Arc::new(RateLimiter::new(RateLimitConfig::default()));

    // Server with encryption disabled
    let auth_manager_server = auth_manager.clone();
    let rate_limiter_server = rate_limiter.clone();
    let server_handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        
        let encryption_config = EncryptionConfig {
            enabled: false, // Disabled for testing
            ..Default::default()
        };
        
        let mut transport = SecureIpcTransport::new_tcp(
            stream,
            auth_manager_server.clone(),
            rate_limiter_server,
            encryption_config,
        );

        let token = auth_manager_server
            .issue_token("test_process".to_string(), vec!["ipc".to_string()])
            .await
            .unwrap();
        transport.authenticate(token).await.unwrap();

        let received = transport.receive_secure().await.unwrap();
        assert_eq!(received.source, ProcessId::Main);
        assert!(matches!(received.command, IpcCommand::Ping));
    });

    // Client with encryption disabled
    let stream = TcpStream::connect(addr).await.unwrap();
    
    let encryption_config = EncryptionConfig {
        enabled: false, // Disabled for testing
        ..Default::default()
    };
    
    let mut transport = SecureIpcTransport::new_tcp(
        stream,
        auth_manager.clone(),
        rate_limiter,
        encryption_config,
    );

    let token = auth_manager
        .issue_token("test_process".to_string(), vec!["ipc".to_string()])
        .await
        .unwrap();
    transport.authenticate(token).await.unwrap();

    let message = IpcMessage::new(ProcessId::Main, ProcessId::Main, IpcCommand::Ping);
    transport.send_secure(message).await.unwrap();

    server_handle.await.unwrap();
    
    println!("✅ IPC without encryption test passed (backward compatibility)!");
}