//! RPC integration tests

use multivm_rpc::{
    test_client::TestClient,
    proto::multivm::{
        CreateVmRequest, ListVmsRequest, 
        ResourceRequirements as ProtoResources,
    },
    server::{RpcServer, RpcServerConfig},
};
use multivm_core::{NodeId, VmId};
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::{sleep, timeout};

async fn setup_test_server() -> (String, TempDir, tokio::task::JoinHandle<()>) {
    let temp_dir = TempDir::new().unwrap();
    
    // Create RPC server config
    let config = RpcServerConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        enable_tls: false,
        tls_cert_path: None,
        tls_key_path: None,
        max_concurrent_requests: 100,
        request_timeout: Duration::from_secs(30),
    };
    
    // Create a simple mock orchestrator service
    let server = RpcServer::new(config);
    
    // Get the actual bound address
    let addr = server.local_addr();
    
    // Start server in background
    let server_handle = tokio::spawn(async move {
        server.serve().await.unwrap();
    });
    
    // Give server time to start
    sleep(Duration::from_millis(200)).await;
    
    let url = format!("http://{}", addr);
    (url, temp_dir, server_handle)
}

#[tokio::test]
async fn test_rpc_vm_lifecycle() {
    let (url, _temp_dir, _handle) = setup_test_server().await;
    let mut client = TestClient::connect(url.clone()).await.unwrap();
    
    // Create VM
    let create_req = CreateVmRequest {
        name: "test-vm".to_string(),
        resources: Some(ProtoResources {
            cpu_cores: 2.0,
            memory_mb: 1024,
            disk_gb: 10,
            network_mbps: Some(100),
        }),
        labels: std::collections::HashMap::new(),
    };
    
    let response = client.create_vm(create_req).await.unwrap();
    let vm_id = response.into_inner().vm_id;
    assert!(!vm_id.is_empty());
    
    // Get VM
    let response = client.get_vm(vm_id.clone()).await.unwrap();
    let vm = response.into_inner();
    assert_eq!(vm.vm_id, vm_id);
    assert_eq!(vm.name, "test-vm");
    
    // List VMs
    let list_req = ListVmsRequest {
        label_selector: std::collections::HashMap::new(),
        node_id: String::new(),
    };
    let response = client.list_vms(list_req).await.unwrap();
    let vms = response.into_inner().vms;
    assert_eq!(vms.len(), 1);
    assert_eq!(vms[0].vm_id, vm_id);
    
    // Start VM
    let response = client.start_vm(vm_id.clone()).await.unwrap();
    assert_eq!(response.into_inner(), ());
    
    // Stop VM
    let response = client.stop_vm(vm_id.clone(), false).await.unwrap();
    assert_eq!(response.into_inner(), ());
    
    // Delete VM
    let response = client.delete_vm(vm_id.clone()).await.unwrap();
    assert_eq!(response.into_inner(), ());
}

#[tokio::test]
async fn test_rpc_error_handling() {
    let (url, _temp_dir, _handle) = setup_test_server().await;
    let mut client = TestClient::connect(url).await.unwrap();
    
    // Try to get non-existent VM
    let result = client.get_vm(VmId::new().to_string()).await;
    assert!(result.is_err());
    
    let status = result.unwrap_err();
    assert_eq!(status.code(), tonic::Code::NotFound);
    
    // Try to start non-existent VM
    let result = client.start_vm(VmId::new().to_string()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_rpc_concurrent_requests() {
    let (url, _temp_dir, _handle) = setup_test_server().await;
    
    let mut handles = vec![];
    
    // Create multiple clients and send concurrent requests
    for i in 0..10 {
        let url_clone = url.clone();
        let handle = tokio::spawn(async move {
            let mut client = TestClient::connect(url_clone).await.unwrap();
            
            let create_req = CreateVmRequest {
                name: format!("concurrent-vm-{}", i),
                resources: Some(ProtoResources {
                    cpu_cores: 1.0,
                    memory_mb: 512,
                    disk_gb: 5,
                    network_mbps: Some(50),
                }),
                labels: std::collections::HashMap::new(),
            };
            
            let result = client.create_vm(create_req).await;
            result.is_ok()
        });
        handles.push(handle);
    }
    
    // Wait for all requests
    let results = futures::future::try_join_all(handles).await.unwrap();
    
    // All should succeed
    for (i, success) in results.iter().enumerate() {
        assert!(success, "Request {} should succeed", i);
    }
}

#[tokio::test]
async fn test_rpc_node_operations() {
    let (url, _temp_dir, _handle) = setup_test_server().await;
    let mut client = TestClient::connect(url).await.unwrap();
    
    // Get node status
    let response = client.get_node_status().await.unwrap();
    let status = response.into_inner();
    
    // Should have node ID
    assert!(!status.node_id.is_empty());
    assert!(!status.state.is_empty());
}

#[tokio::test]
async fn test_rpc_invalid_resources() {
    let (url, _temp_dir, _handle) = setup_test_server().await;
    let mut client = TestClient::connect(url).await.unwrap();
    
    // Try to create VM with invalid resources
    let invalid_requests = vec![
        CreateVmRequest {
            name: "invalid-cpu".to_string(),
            resources: Some(ProtoResources {
                cpu_cores: 0.0, // Invalid
                memory_mb: 512,
                disk_gb: 5,
                network_mbps: Some(50),
            }),
            labels: std::collections::HashMap::new(),
        },
        CreateVmRequest {
            name: "invalid-memory".to_string(),
            resources: Some(ProtoResources {
                cpu_cores: 1.0,
                memory_mb: 0, // Invalid
                disk_gb: 5,
                network_mbps: Some(50),
            }),
            labels: std::collections::HashMap::new(),
        },
    ];
    
    for req in invalid_requests {
        let result = client.create_vm(req).await;
        assert!(result.is_err(), "Invalid resources should be rejected");
    }
}

#[tokio::test]
async fn test_rpc_vm_migration() {
    let (url, _temp_dir, _handle) = setup_test_server().await;
    let mut client = TestClient::connect(url).await.unwrap();
    
    // Create a VM first
    let create_req = CreateVmRequest {
        name: "migration-test-vm".to_string(),
        resources: Some(ProtoResources {
            cpu_cores: 1.0,
            memory_mb: 512,
            disk_gb: 5,
            network_mbps: Some(50),
        }),
        labels: std::collections::HashMap::new(),
    };
    
    let response = client.create_vm(create_req).await.unwrap();
    let vm_id = response.into_inner().vm_id;
    
    // Start the VM
    client.start_vm(vm_id.clone()).await.unwrap();
    
    // Migrate VM to another node
    let target_node = NodeId::new().to_string();
    let result = client.migrate_vm(vm_id, target_node, true).await;
    
    // Migration might fail if no target nodes available, but RPC should work
    assert!(result.is_ok() || result.unwrap_err().code() == tonic::Code::FailedPrecondition);
}

#[tokio::test]
async fn test_rpc_heartbeat() {
    let (url, _temp_dir, _handle) = setup_test_server().await;
    let mut client = TestClient::connect(url).await.unwrap();
    
    // Send heartbeat
    let heartbeat_req = multivm_rpc::proto::multivm::HeartbeatRequest {
        node_id: NodeId::new().to_string(),
        resource_usage: Some(multivm_rpc::proto::multivm::ResourceUsage {
            cpu_percent: 25.5,
            memory_mb: 1024,
            disk_io_mbps: 100.0,
            network_mbps: 50.0,
        }),
    };
    
    let response = client.heartbeat(heartbeat_req).await;
    assert!(response.is_ok());
}

#[tokio::test]
async fn test_rpc_graceful_shutdown() {
    let temp_dir = TempDir::new().unwrap();
    
    let config = RpcServerConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        enable_tls: false,
        tls_cert_path: None,
        tls_key_path: None,
        max_concurrent_requests: 100,
        request_timeout: Duration::from_secs(30),
    };
    
    let server = RpcServer::new(config);
    let addr = server.local_addr();
    
    // Start server with shutdown signal
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    
    let server_handle = tokio::spawn(async move {
        server.serve_with_shutdown(async {
            shutdown_rx.await.ok();
        }).await.unwrap();
    });
    
    sleep(Duration::from_millis(200)).await;
    
    // Connect client
    let url = format!("http://{}", addr);
    let mut client = TestClient::connect(url.clone()).await.unwrap();
    
    // Make a request
    let response = client.get_node_status().await;
    assert!(response.is_ok());
    
    // Trigger shutdown
    shutdown_tx.send(()).unwrap();
    
    // Server should shut down gracefully
    timeout(Duration::from_secs(2), server_handle).await
        .expect("Server should shut down within timeout")
        .expect("Server task should complete successfully");
}