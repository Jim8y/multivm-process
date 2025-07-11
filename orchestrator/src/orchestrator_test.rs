//! Unit tests for the orchestrator module

#[cfg(test)]
mod tests {
    use super::super::*;
    use multivm_core::{NodeId, VmId, VmState, ResourceRequirements};
    use consensus::NodeState;
    use tempfile::TempDir;
    use std::time::Duration;
    use tokio::time::sleep;

    async fn create_test_orchestrator() -> (Orchestrator, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = OrchestratorConfig {
            node_id: Some(NodeId::new()),
            data_dir: temp_dir.path().to_path_buf(),
            consensus: consensus::Config {
                node_id: NodeId::new(),
                cluster_members: vec![],
                election_timeout_min: Duration::from_millis(150),
                election_timeout_max: Duration::from_millis(300),
                heartbeat_interval: Duration::from_millis(50),
                ..Default::default()
            },
            rpc_addr: "127.0.0.1:0".parse().unwrap(),
            enable_tls: false,
            ..Default::default()
        };
        
        let orchestrator = Orchestrator::new(config).await.unwrap();
        (orchestrator, temp_dir)
    }

    #[tokio::test]
    async fn test_orchestrator_creation() {
        let (orchestrator, _temp_dir) = create_test_orchestrator().await;
        assert!(!orchestrator.is_leader().await);
    }

    #[tokio::test]
    async fn test_vm_creation() {
        let (orchestrator, _temp_dir) = create_test_orchestrator().await;
        
        // Force leadership for testing
        orchestrator.consensus.force_leader().await;
        
        let vm_id = VmId::new();
        let resources = ResourceRequirements::new(2.0, 1024, 10, Some(100)).unwrap();
        
        let result = orchestrator.create_vm(
            vm_id,
            "test-vm".to_string(),
            resources,
        ).await;
        
        assert!(result.is_ok());
        
        // Verify VM was created
        let vm_state = orchestrator.get_vm_state(vm_id).await;
        assert!(vm_state.is_some());
        assert_eq!(vm_state.unwrap().name, "test-vm");
    }

    #[tokio::test]
    async fn test_vm_lifecycle_states() {
        let (orchestrator, _temp_dir) = create_test_orchestrator().await;
        orchestrator.consensus.force_leader().await;
        
        let vm_id = VmId::new();
        let resources = ResourceRequirements::new(1.0, 512, 5, None).unwrap();
        
        // Create VM
        orchestrator.create_vm(vm_id, "lifecycle-vm".to_string(), resources).await.unwrap();
        
        // Check initial state
        let state = orchestrator.get_vm_state(vm_id).await.unwrap();
        assert_eq!(state.state, VmState::Creating);
        
        // Start VM
        orchestrator.start_vm(vm_id).await.unwrap();
        let state = orchestrator.get_vm_state(vm_id).await.unwrap();
        assert!(matches!(state.state, VmState::Running | VmState::Creating));
        
        // Pause VM
        orchestrator.pause_vm(vm_id).await.unwrap();
        
        // Resume VM
        orchestrator.resume_vm(vm_id).await.unwrap();
        
        // Stop VM
        orchestrator.stop_vm(vm_id).await.unwrap();
        
        // Delete VM
        orchestrator.delete_vm(vm_id).await.unwrap();
        
        // VM should be gone or in deleting state
        let state = orchestrator.get_vm_state(vm_id).await;
        assert!(state.is_none() || state.unwrap().state == VmState::Deleting);
    }

    #[tokio::test]
    async fn test_list_vms() {
        let (orchestrator, _temp_dir) = create_test_orchestrator().await;
        orchestrator.consensus.force_leader().await;
        
        // Initially empty
        let vms = orchestrator.list_vms().await.unwrap();
        assert!(vms.is_empty());
        
        // Create multiple VMs
        let mut vm_ids = vec![];
        for i in 0..5 {
            let vm_id = VmId::new();
            let resources = ResourceRequirements::new(1.0, 256, 5, None).unwrap();
            orchestrator.create_vm(
                vm_id,
                format!("vm-{}", i),
                resources,
            ).await.unwrap();
            vm_ids.push(vm_id);
        }
        
        // List should contain all VMs
        let vms = orchestrator.list_vms().await.unwrap();
        assert_eq!(vms.len(), 5);
        
        for vm_id in vm_ids {
            assert!(vms.iter().any(|v| v.id == vm_id));
        }
    }

    #[tokio::test]
    async fn test_vm_migration() {
        let (orchestrator, _temp_dir) = create_test_orchestrator().await;
        orchestrator.consensus.force_leader().await;
        
        let vm_id = VmId::new();
        let resources = ResourceRequirements::new(1.0, 512, 10, None).unwrap();
        let target_node = NodeId::new();
        
        // Create and start VM
        orchestrator.create_vm(vm_id, "migration-vm".to_string(), resources).await.unwrap();
        orchestrator.start_vm(vm_id).await.unwrap();
        
        // Migrate VM
        let result = orchestrator.migrate_vm(vm_id, target_node).await;
        assert!(result.is_ok());
        
        // Check state
        let state = orchestrator.get_vm_state(vm_id).await.unwrap();
        assert_eq!(state.state, VmState::Migrating);
    }

    #[tokio::test]
    async fn test_node_management() {
        let (orchestrator, _temp_dir) = create_test_orchestrator().await;
        
        // List nodes (should have self)
        let nodes = orchestrator.list_nodes().await.unwrap();
        assert!(!nodes.is_empty());
        
        // Get self node info
        let node_id = orchestrator.config.node_id.unwrap();
        let node_info = orchestrator.get_node_info(node_id).await;
        assert!(node_info.is_some());
    }

    #[tokio::test]
    async fn test_invalid_vm_operations() {
        let (orchestrator, _temp_dir) = create_test_orchestrator().await;
        orchestrator.consensus.force_leader().await;
        
        let vm_id = VmId::new();
        
        // Operations on non-existent VM should fail
        assert!(orchestrator.start_vm(vm_id).await.is_err());
        assert!(orchestrator.stop_vm(vm_id).await.is_err());
        assert!(orchestrator.pause_vm(vm_id).await.is_err());
        assert!(orchestrator.resume_vm(vm_id).await.is_err());
        assert!(orchestrator.delete_vm(vm_id).await.is_err());
    }

    #[tokio::test]
    async fn test_resource_validation() {
        let (orchestrator, _temp_dir) = create_test_orchestrator().await;
        orchestrator.consensus.force_leader().await;
        
        let vm_id = VmId::new();
        
        // Invalid resources should fail
        let invalid_resources = vec![
            ResourceRequirements::new(0.0, 512, 10, None),    // Invalid CPU
            ResourceRequirements::new(1.0, 0, 10, None),      // Invalid memory
            ResourceRequirements::new(1.0, 512, 0, None),     // Invalid disk
        ];
        
        for resources in invalid_resources {
            assert!(resources.is_err());
        }
    }

    #[tokio::test]
    async fn test_concurrent_vm_operations() {
        let (orchestrator, _temp_dir) = create_test_orchestrator().await;
        let orchestrator = Arc::new(orchestrator);
        orchestrator.consensus.force_leader().await;
        
        let mut handles = vec![];
        
        // Create VMs concurrently
        for i in 0..10 {
            let orchestrator_clone = orchestrator.clone();
            let handle = tokio::spawn(async move {
                let vm_id = VmId::new();
                let resources = ResourceRequirements::new(1.0, 256, 5, None).unwrap();
                orchestrator_clone.create_vm(
                    vm_id,
                    format!("concurrent-vm-{}", i),
                    resources,
                ).await
            });
            handles.push(handle);
        }
        
        // All should succeed
        for handle in handles {
            assert!(handle.await.unwrap().is_ok());
        }
        
        // Verify all VMs exist
        let vms = orchestrator.list_vms().await.unwrap();
        assert_eq!(vms.len(), 10);
    }

    #[tokio::test]
    async fn test_leader_only_operations() {
        let (orchestrator, _temp_dir) = create_test_orchestrator().await;
        
        // Without leadership, write operations should fail
        let vm_id = VmId::new();
        let resources = ResourceRequirements::new(1.0, 512, 10, None).unwrap();
        
        let result = orchestrator.create_vm(vm_id, "test-vm".to_string(), resources).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not leader"));
    }

    #[tokio::test]
    async fn test_state_machine_persistence() {
        let (orchestrator, _temp_dir) = create_test_orchestrator().await;
        orchestrator.consensus.force_leader().await;
        
        // Create VMs
        let mut vm_ids = vec![];
        for i in 0..3 {
            let vm_id = VmId::new();
            let resources = ResourceRequirements::new(1.0, 256, 5, None).unwrap();
            orchestrator.create_vm(
                vm_id,
                format!("persistent-vm-{}", i),
                resources,
            ).await.unwrap();
            vm_ids.push(vm_id);
        }
        
        // Take snapshot
        let snapshot = orchestrator.state_machine.lock().await.snapshot().await.unwrap();
        assert!(!snapshot.is_empty());
        
        // Create new state machine and restore
        let mut new_state_machine = OrchestratorStateMachine::new();
        new_state_machine.restore(&snapshot).await.unwrap();
        
        // Verify VMs are restored
        for vm_id in vm_ids {
            let state = new_state_machine.vm_state.get(&vm_id.to_string());
            assert!(state.is_some());
        }
    }
}