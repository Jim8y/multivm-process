//! Comprehensive tests for core types

#[cfg(test)]
mod tests {
    use super::super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    #[test]
    fn test_node_id_creation() {
        let id1 = NodeId::new();
        let id2 = NodeId::new();
        assert_ne!(id1, id2, "NodeIds should be unique");
    }

    #[test]
    fn test_node_id_from_bytes() {
        let bytes = [1u8; 16];
        let id = NodeId::from_bytes(bytes);
        assert_eq!(id.as_bytes(), &bytes);
    }

    #[test]
    fn test_node_id_try_from_slice() {
        // Valid slice
        let bytes = vec![1u8; 16];
        let id = NodeId::try_from_slice(&bytes).unwrap();
        assert_eq!(id.as_bytes(), &[1u8; 16]);

        // Invalid slice - too short
        let short = vec![1u8; 15];
        assert!(NodeId::try_from_slice(&short).is_err());

        // Invalid slice - too long
        let long = vec![1u8; 17];
        assert!(NodeId::try_from_slice(&long).is_err());
    }

    #[test]
    fn test_resource_requirements_validation() {
        // Valid requirements
        let req = ResourceRequirements::new(2.0, 1024, 10, Some(100)).unwrap();
        assert_eq!(req.cpu_cores, 2.0);
        assert_eq!(req.memory_mb, 1024);
        assert_eq!(req.disk_gb, 10);
        assert_eq!(req.network_mbps, Some(100));

        // Invalid CPU
        assert!(ResourceRequirements::new(0.0, 1024, 10, None).is_err());
        assert!(ResourceRequirements::new(-1.0, 1024, 10, None).is_err());

        // Invalid memory
        assert!(ResourceRequirements::new(1.0, 0, 10, None).is_err());

        // Invalid disk
        assert!(ResourceRequirements::new(1.0, 1024, 0, None).is_err());
    }

    #[test]
    fn test_vm_state_transitions() {
        use VmState::*;

        // Valid transitions
        assert!(Creating.can_transition_to(&Running));
        assert!(Running.can_transition_to(&Paused));
        assert!(Running.can_transition_to(&Stopped));
        assert!(Running.can_transition_to(&Migrating));
        assert!(Paused.can_transition_to(&Running));
        assert!(Stopped.can_transition_to(&Running));
        assert!(Failed.can_transition_to(&Deleting));

        // Invalid transitions
        assert!(!Creating.can_transition_to(&Paused));
        assert!(!Running.can_transition_to(&Creating));
        assert!(!Deleting.can_transition_to(&Running));
        assert!(!Stopped.can_transition_to(&Paused));
    }

    #[test]
    fn test_vm_state_terminal() {
        assert!(!VmState::Creating.is_terminal());
        assert!(!VmState::Running.is_terminal());
        assert!(VmState::Failed.is_terminal());
        assert!(VmState::Deleting.is_terminal());
    }

    #[test]
    fn test_cluster_member() {
        let id = NodeId::new();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let member = ClusterMember::new(id, addr);

        assert_eq!(member.id, id);
        assert_eq!(member.address, addr);
        assert!(member.healthy);
        assert!(member.metadata.is_empty());

        // Test alive check
        let timeout = chrono::Duration::seconds(5);
        assert!(member.is_alive(timeout));

        // Test with old heartbeat
        let mut old_member = member.clone();
        old_member.last_heartbeat = chrono::Utc::now() - chrono::Duration::seconds(10);
        assert!(!old_member.is_alive(timeout));
    }

    #[test]
    fn test_version_increment() {
        let v1 = Version::INITIAL;
        assert_eq!(v1.0, 0);

        let v2 = v1.increment();
        assert_eq!(v2.0, 1);

        let v3 = v2.increment();
        assert_eq!(v3.0, 2);
    }

    #[test]
    fn test_vm_metadata_creation() {
        let vm_id = VmId::new();
        let node_id = NodeId::new();
        let resources = ResourceRequirements::new(1.0, 512, 5, None).unwrap();

        let metadata = VmMetadata {
            id: vm_id,
            name: "test-vm".to_string(),
            state: VmState::Creating,
            node_id: Some(node_id),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            resources,
            labels: std::collections::HashMap::new(),
        };

        assert_eq!(metadata.id, vm_id);
        assert_eq!(metadata.name, "test-vm");
        assert_eq!(metadata.state, VmState::Creating);
        assert_eq!(metadata.node_id, Some(node_id));
    }

    #[test]
    fn test_serialization() {
        // Test NodeId serialization
        let node_id = NodeId::new();
        let json = serde_json::to_string(&node_id).unwrap();
        let deserialized: NodeId = serde_json::from_str(&json).unwrap();
        assert_eq!(node_id, deserialized);

        // Test VmState serialization
        let state = VmState::Running;
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: VmState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deserialized);

        // Test ResourceRequirements serialization
        let req = ResourceRequirements::new(2.5, 2048, 20, Some(1000)).unwrap();
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: ResourceRequirements = serde_json::from_str(&json).unwrap();
        assert_eq!(req.cpu_cores, deserialized.cpu_cores);
        assert_eq!(req.memory_mb, deserialized.memory_mb);
    }

    #[test]
    fn test_display_implementations() {
        let node_id = NodeId::new();
        let display = format!("{}", node_id);
        assert!(!display.is_empty());

        let vm_id = VmId::new();
        let display = format!("{}", vm_id);
        assert!(!display.is_empty());
    }

    #[test]
    fn test_resource_usage() {
        let usage = ResourceUsage {
            cpu_percent: 75.5,
            memory_mb: 1500,
            disk_io_mbps: 100.0,
            network_mbps: 50.0,
        };

        assert_eq!(usage.cpu_percent, 75.5);
        assert_eq!(usage.memory_mb, 1500);
        assert_eq!(usage.disk_io_mbps, 100.0);
        assert_eq!(usage.network_mbps, 50.0);
    }

    #[test]
    fn test_common_config() {
        let config = CommonConfig::default();
        assert_eq!(config.data_dir, std::path::PathBuf::from("/var/lib/multivm"));
        assert_eq!(config.log_level, "info");
        assert_eq!(config.metrics_port, 9090);
        assert_eq!(config.admin_port, 8080);
    }
}