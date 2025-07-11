//! End-to-end integration tests for the MultiVM system

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::{sleep, timeout};
use tempfile::TempDir;
use multivm_core::{NodeId, VmId, VmState, ResourceRequirements};
use orchestrator::{OrchestratorConfig, Orchestrator};
use consensus::{Config as ConsensusConfig};
use tokio::sync::oneshot;

/// Test fixture for setting up a multi-node cluster
struct TestCluster {
    nodes: Vec<TestNode>,
    _temp_dirs: Vec<TempDir>,
}

struct TestNode {
    id: NodeId,
    orchestrator: Orchestrator,
    shutdown_tx: oneshot::Sender<()>,
    data_dir: PathBuf,
}

impl TestCluster {
    async fn new(num_nodes: usize) -> Self {
        let mut nodes = Vec::new();
        let mut temp_dirs = Vec::new();
        let mut node_ids = Vec::new();
        let mut addresses = Vec::new();
        
        // Generate node IDs and addresses
        for i in 0..num_nodes {
            node_ids.push(NodeId::new());
            addresses.push(format!("127.0.0.1:{}", 9000 + i).parse::<SocketAddr>().unwrap());
        }
        
        // Create cluster members
        let cluster_members: Vec<_> = node_ids.iter().zip(addresses.iter())
            .map(|(id, addr)| (*id, *addr))
            .collect();
        
        // Create nodes
        for i in 0..num_nodes {
            let temp_dir = TempDir::new().unwrap();
            let data_dir = temp_dir.path().to_path_buf();
            
            let config = OrchestratorConfig {
                node_id: Some(node_ids[i]),
                data_dir: data_dir.clone(),
                consensus: ConsensusConfig {
                    node_id: node_ids[i],
                    cluster_members: cluster_members.clone(),
                    election_timeout_min: Duration::from_millis(150),
                    election_timeout_max: Duration::from_millis(300),
                    heartbeat_interval: Duration::from_millis(50),
                    batch_size: 10,
                    snapshot_interval: 100,
                },
                rpc_addr: format!("127.0.0.1:{}", 9100 + i).parse().unwrap(),
                enable_tls: false, // Disable for testing
                tls_cert_path: None,
                tls_key_path: None,
                metrics_addr: format!("127.0.0.1:{}", 9200 + i).parse().unwrap(),
                admin_addr: format!("127.0.0.1:{}", 9300 + i).parse().unwrap(),
            };
            
            let (shutdown_tx, shutdown_rx) = oneshot::channel();
            let orchestrator = Orchestrator::new(config).await.unwrap();
            
            nodes.push(TestNode {
                id: node_ids[i],
                orchestrator,
                shutdown_tx,
                data_dir,
            });
            
            temp_dirs.push(temp_dir);
        }
        
        TestCluster { nodes, _temp_dirs: temp_dirs }
    }
    
    async fn start_all(&mut self) {
        for node in &mut self.nodes {
            // Start each node in a background task
            let orchestrator = node.orchestrator.clone();
            tokio::spawn(async move {
                orchestrator.run().await.unwrap();
            });
        }
        
        // Wait for cluster to stabilize
        sleep(Duration::from_secs(2)).await;
    }
    
    async fn find_leader(&self) -> Option<&TestNode> {
        for node in &self.nodes {
            if node.orchestrator.is_leader().await {
                return Some(node);
            }
        }
        None
    }
    
    async fn shutdown(self) {
        for node in self.nodes {
            let _ = node.shutdown_tx.send(());
        }
        // Give time for graceful shutdown
        sleep(Duration::from_millis(500)).await;
    }
}

#[tokio::test]
async fn test_cluster_formation() {
    let mut cluster = TestCluster::new(3).await;
    cluster.start_all().await;
    
    // Verify leader election
    let leader = timeout(Duration::from_secs(5), async {
        loop {
            if let Some(leader) = cluster.find_leader().await {
                return leader;
            }
            sleep(Duration::from_millis(100)).await;
        }
    }).await;
    
    assert!(leader.is_ok(), "Cluster should elect a leader");
    
    cluster.shutdown().await;
}

#[tokio::test]
async fn test_vm_lifecycle() {
    let mut cluster = TestCluster::new(3).await;
    cluster.start_all().await;
    
    // Wait for leader election
    let leader = timeout(Duration::from_secs(5), async {
        loop {
            if let Some(leader) = cluster.find_leader().await {
                return leader;
            }
            sleep(Duration::from_millis(100)).await;
        }
    }).await.expect("Should elect leader");
    
    // Create a VM
    let vm_id = VmId::new();
    let resources = ResourceRequirements::new(2.0, 1024, 10, Some(100)).unwrap();
    
    leader.orchestrator.create_vm(
        vm_id,
        "test-vm".to_string(),
        resources,
    ).await.expect("Should create VM");
    
    // Verify VM is created
    let vm_state = leader.orchestrator.get_vm_state(vm_id).await;
    assert!(vm_state.is_some());
    assert_eq!(vm_state.unwrap().state, VmState::Creating);
    
    // Start the VM
    leader.orchestrator.start_vm(vm_id).await.expect("Should start VM");
    
    // Wait for VM to be running
    timeout(Duration::from_secs(5), async {
        loop {
            if let Some(state) = leader.orchestrator.get_vm_state(vm_id).await {
                if state.state == VmState::Running {
                    return;
                }
            }
            sleep(Duration::from_millis(100)).await;
        }
    }).await.expect("VM should be running");
    
    // Stop the VM
    leader.orchestrator.stop_vm(vm_id).await.expect("Should stop VM");
    
    // Delete the VM
    leader.orchestrator.delete_vm(vm_id).await.expect("Should delete VM");
    
    cluster.shutdown().await;
}

#[tokio::test]
async fn test_leader_failover() {
    let mut cluster = TestCluster::new(5).await;
    cluster.start_all().await;
    
    // Find initial leader
    let initial_leader_id = timeout(Duration::from_secs(5), async {
        loop {
            if let Some(leader) = cluster.find_leader().await {
                return leader.id;
            }
            sleep(Duration::from_millis(100)).await;
        }
    }).await.expect("Should elect initial leader");
    
    // Create a VM on the leader
    let vm_id = VmId::new();
    let resources = ResourceRequirements::new(1.0, 512, 5, None).unwrap();
    
    if let Some(leader) = cluster.nodes.iter().find(|n| n.id == initial_leader_id) {
        leader.orchestrator.create_vm(
            vm_id,
            "failover-test-vm".to_string(),
            resources,
        ).await.expect("Should create VM");
    }
    
    // Simulate leader failure by shutting it down
    let leader_index = cluster.nodes.iter().position(|n| n.id == initial_leader_id).unwrap();
    let failed_node = cluster.nodes.remove(leader_index);
    let _ = failed_node.shutdown_tx.send(());
    
    // Wait for new leader election
    sleep(Duration::from_secs(2)).await;
    
    let new_leader = timeout(Duration::from_secs(5), async {
        loop {
            if let Some(leader) = cluster.find_leader().await {
                return leader;
            }
            sleep(Duration::from_millis(100)).await;
        }
    }).await.expect("Should elect new leader");
    
    assert_ne!(new_leader.id, initial_leader_id, "New leader should be different");
    
    // Verify VM state is preserved
    let vm_state = new_leader.orchestrator.get_vm_state(vm_id).await;
    assert!(vm_state.is_some(), "VM state should be preserved after failover");
    
    cluster.shutdown().await;
}

#[tokio::test]
async fn test_concurrent_vm_operations() {
    let mut cluster = TestCluster::new(3).await;
    cluster.start_all().await;
    
    let leader = timeout(Duration::from_secs(5), async {
        loop {
            if let Some(leader) = cluster.find_leader().await {
                return leader;
            }
            sleep(Duration::from_millis(100)).await;
        }
    }).await.expect("Should elect leader");
    
    // Create multiple VMs concurrently
    let mut handles = vec![];
    for i in 0..10 {
        let orchestrator = leader.orchestrator.clone();
        let handle = tokio::spawn(async move {
            let vm_id = VmId::new();
            let resources = ResourceRequirements::new(1.0, 256, 5, None).unwrap();
            orchestrator.create_vm(
                vm_id,
                format!("concurrent-vm-{}", i),
                resources,
            ).await
        });
        handles.push(handle);
    }
    
    // Wait for all operations to complete
    let results = futures::future::try_join_all(handles).await.unwrap();
    
    // Verify all VMs were created successfully
    for result in results {
        assert!(result.is_ok(), "All VM creations should succeed");
    }
    
    cluster.shutdown().await;
}

#[tokio::test]
async fn test_resource_scheduling() {
    let mut cluster = TestCluster::new(3).await;
    cluster.start_all().await;
    
    let leader = timeout(Duration::from_secs(5), async {
        loop {
            if let Some(leader) = cluster.find_leader().await {
                return leader;
            }
            sleep(Duration::from_millis(100)).await;
        }
    }).await.expect("Should elect leader");
    
    // Create VMs with different resource requirements
    let small_vm = VmId::new();
    let large_vm = VmId::new();
    
    leader.orchestrator.create_vm(
        small_vm,
        "small-vm".to_string(),
        ResourceRequirements::new(0.5, 256, 5, None).unwrap(),
    ).await.expect("Should create small VM");
    
    leader.orchestrator.create_vm(
        large_vm,
        "large-vm".to_string(),
        ResourceRequirements::new(4.0, 8192, 100, Some(1000)).unwrap(),
    ).await.expect("Should create large VM");
    
    // Verify both VMs are scheduled
    let small_state = leader.orchestrator.get_vm_state(small_vm).await;
    let large_state = leader.orchestrator.get_vm_state(large_vm).await;
    
    assert!(small_state.is_some());
    assert!(large_state.is_some());
    
    cluster.shutdown().await;
}

#[tokio::test]
async fn test_cluster_state_consistency() {
    let mut cluster = TestCluster::new(3).await;
    cluster.start_all().await;
    
    let leader = timeout(Duration::from_secs(5), async {
        loop {
            if let Some(leader) = cluster.find_leader().await {
                return leader;
            }
            sleep(Duration::from_millis(100)).await;
        }
    }).await.expect("Should elect leader");
    
    // Create several VMs
    let mut vm_ids = vec![];
    for i in 0..5 {
        let vm_id = VmId::new();
        let resources = ResourceRequirements::new(1.0, 512, 10, None).unwrap();
        leader.orchestrator.create_vm(
            vm_id,
            format!("consistency-test-vm-{}", i),
            resources,
        ).await.expect("Should create VM");
        vm_ids.push(vm_id);
    }
    
    // Wait for replication
    sleep(Duration::from_secs(1)).await;
    
    // Verify all nodes have consistent state
    for node in &cluster.nodes {
        for vm_id in &vm_ids {
            let state = node.orchestrator.get_vm_state(*vm_id).await;
            assert!(state.is_some(), "All nodes should have VM state");
        }
    }
    
    cluster.shutdown().await;
}