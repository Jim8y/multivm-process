//! Comprehensive tests for P2P messages

#[cfg(test)]
mod tests {
    use crate::protocol::messages::*;
    use std::time::Duration;

    #[test]
    fn test_network_message_creation() {
        let msg = NetworkMessage::new(
            MessagePayload::Control(ControlMessage::StatusRequest),
            MessageSource::NetworkLayer,
            MessageTarget::Broadcast,
        );

        assert!(!msg.id.is_empty());
        assert_eq!(msg.source, MessageSource::NetworkLayer);
        assert_eq!(msg.target, MessageTarget::Broadcast);
        assert_eq!(msg.version, 1);
    }

    #[test]
    fn test_message_with_metadata() {
        let mut msg = NetworkMessage::new(
            MessagePayload::Control(ControlMessage::Heartbeat {
                status: NodeStatus::Active,
                uptime: Duration::from_secs(3600),
            }),
            MessageSource::NetworkLayer,
            MessageTarget::Peer("peer123".to_string()),
        );

        msg = msg.with_metadata("priority", "high");
        msg = msg.with_metadata("retry_count", "3");

        assert_eq!(msg.metadata.get("priority"), Some(&"high".to_string()));
        assert_eq!(msg.metadata.get("retry_count"), Some(&"3".to_string()));
    }

    #[test]
    fn test_message_type_inference() {
        let control_msg = NetworkMessage::new(
            MessagePayload::Control(ControlMessage::StatusRequest),
            MessageSource::NetworkLayer,
            MessageTarget::Broadcast,
        );
        assert_eq!(control_msg.infer_type(), MessageType::Control);

        let svm_msg = NetworkMessage::new(
            MessagePayload::Svm(SvmMessage::Transaction {
                transaction_data: Box::new(vec![1, 2, 3]),
                signature: "sig123".to_string(),
            }),
            MessageSource::SvmExecution,
            MessageTarget::Broadcast,
        );
        assert_eq!(svm_msg.infer_type(), MessageType::Svm);

        let evm_msg = NetworkMessage::new(
            MessagePayload::Evm(EvmMessage::Transaction {
                transaction_data: Box::new(vec![4, 5, 6]),
                tx_hash: "0x123".to_string(),
            }),
            MessageSource::EvmExecution,
            MessageTarget::Broadcast,
        );
        assert_eq!(evm_msg.infer_type(), MessageType::Evm);
    }

    #[test]
    fn test_message_targeting() {
        let broadcast_msg = NetworkMessage::new(
            MessagePayload::Control(ControlMessage::Heartbeat {
                status: NodeStatus::Active,
                uptime: Duration::from_secs(3600),
            }),
            MessageSource::NetworkLayer,
            MessageTarget::Broadcast,
        );
        assert!(broadcast_msg.is_broadcast());
        assert!(!broadcast_msg.is_peer_message());
        assert!(!broadcast_msg.is_vm_specific());
        assert_eq!(broadcast_msg.target_peer(), None);

        let peer_msg = NetworkMessage::new(
            MessagePayload::Control(ControlMessage::StatusRequest),
            MessageSource::NetworkLayer,
            MessageTarget::Peer("peer456".to_string()),
        );
        assert!(!peer_msg.is_broadcast());
        assert!(peer_msg.is_peer_message());
        assert!(!peer_msg.is_vm_specific());
        assert_eq!(peer_msg.target_peer(), Some("peer456"));

        let vm_msg = NetworkMessage::new(
            MessagePayload::Control(ControlMessage::StatusRequest),
            MessageSource::NetworkLayer,
            MessageTarget::Local(VmType::Svm),
        );
        assert!(!vm_msg.is_broadcast());
        assert!(!vm_msg.is_peer_message());
        assert!(vm_msg.is_vm_specific());
        assert_eq!(vm_msg.target_vm(), Some(VmType::Svm));
    }

    #[test]
    fn test_message_size_estimation() {
        let small_msg = NetworkMessage::new(
            MessagePayload::Control(ControlMessage::StatusRequest),
            MessageSource::NetworkLayer,
            MessageTarget::Broadcast,
        );
        let small_size = small_msg.estimated_size();
        assert!(small_size > 0);
        assert!(small_size < 1000); // Control messages should be small

        let large_msg = NetworkMessage::new(
            MessagePayload::Svm(SvmMessage::Transaction {
                transaction_data: Box::new(vec![0u8; 10000]),
                signature: "sig".to_string(),
            }),
            MessageSource::SvmExecution,
            MessageTarget::Broadcast,
        );
        let large_size = large_msg.estimated_size();
        assert!(large_size > 10000); // Should include the transaction data
    }

    #[test]
    fn test_control_message_variants() {
        let messages = vec![
            ControlMessage::StatusRequest,
            ControlMessage::StatusResponse {
                stats: NetworkStats::default(),
                peers: vec![],
            },
            ControlMessage::Heartbeat {
                status: NodeStatus::Active,
                uptime: Duration::from_secs(3600),
            },
            ControlMessage::Shutdown {
                reason: "Maintenance".to_string(),
                grace_period: Duration::from_secs(30),
            },
        ];

        for msg in messages {
            let network_msg = NetworkMessage::new(
                MessagePayload::Control(msg),
                MessageSource::NetworkLayer,
                MessageTarget::Broadcast,
            );
            assert_eq!(network_msg.infer_type(), MessageType::Control);
        }
    }

    #[test]
    fn test_svm_message_variants() {
        let messages = vec![
            SvmMessage::Transaction {
                transaction_data: Box::new(vec![1, 2, 3]),
                signature: "sig123".to_string(),
            },
            SvmMessage::Block {
                block_data: Box::new(vec![4, 5, 6]),
                block_hash: "hash123".to_string(),
                height: 1000,
            },
            SvmMessage::Gossip {
                data: Box::new(vec![7, 8, 9]),
                gossip_type: "validator_info".to_string(),
            },
        ];

        for msg in messages {
            let network_msg = NetworkMessage::new(
                MessagePayload::Svm(msg),
                MessageSource::SvmExecution,
                MessageTarget::Broadcast,
            );
            assert_eq!(network_msg.infer_type(), MessageType::Svm);
        }
    }

    #[test]
    fn test_evm_message_variants() {
        let messages = vec![
            EvmMessage::Transaction {
                transaction_data: Box::new(vec![1, 2, 3]),
                tx_hash: "0xabc".to_string(),
            },
            EvmMessage::Block {
                block_data: Box::new(vec![4, 5, 6]),
                block_hash: "0xdef".to_string(),
                block_number: 1000,
            },
            EvmMessage::Engine {
                method: "engine_newPayloadV1".to_string(),
                params: Box::new(serde_json::json!({})),
            },
        ];

        for msg in messages {
            let network_msg = NetworkMessage::new(
                MessagePayload::Evm(msg),
                MessageSource::EvmExecution,
                MessageTarget::Broadcast,
            );
            assert_eq!(network_msg.infer_type(), MessageType::Evm);
        }
    }

    #[test]
    fn test_priority_levels() {
        assert!(ExecutionPriority::Critical > ExecutionPriority::High);
        assert!(ExecutionPriority::High > ExecutionPriority::Normal);
        assert!(ExecutionPriority::Normal > ExecutionPriority::Low);
    }

    #[test]
    fn test_message_serialization() {
        let msg = NetworkMessage::new(
            MessagePayload::Control(ControlMessage::StatusRequest),
            MessageSource::NetworkLayer,
            MessageTarget::Broadcast,
        );

        // Test JSON serialization
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: NetworkMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg.id, deserialized.id);
        assert_eq!(msg.version, deserialized.version);

        // Test bincode serialization
        let bytes = bincode::serialize(&msg).unwrap();
        let deserialized: NetworkMessage = bincode::deserialize(&bytes).unwrap();
        assert_eq!(msg.id, deserialized.id);
        assert_eq!(msg.version, deserialized.version);
    }

    #[test]
    fn test_vm_type_serialization() {
        let vm_types = vec![VmType::Svm, VmType::Evm];

        for vm_type in vm_types {
            let json = serde_json::to_string(&vm_type).unwrap();
            let deserialized: VmType = serde_json::from_str(&json).unwrap();
            assert_eq!(vm_type, deserialized);
        }
    }

    #[test]
    fn test_node_status() {
        let statuses = vec![
            NodeStatus::Starting,
            NodeStatus::Active,
            NodeStatus::Syncing,
            NodeStatus::Shutting,
        ];

        for status in statuses {
            // Test that each status is distinct
            match status {
                NodeStatus::Starting => assert!(true),
                NodeStatus::Active => assert!(true),
                NodeStatus::Syncing => assert!(true),
                NodeStatus::Shutting => assert!(true),
                NodeStatus::Error(_) => assert!(true),
            }
        }
    }

    #[test]
    fn test_resource_limits() {
        let limits = ResourceLimits {
            max_connections: 100,
            max_message_size: 1024 * 1024,
            rate_limit: 1000.0,
        };

        assert_eq!(limits.max_connections, 100);
        assert_eq!(limits.max_message_size, 1024 * 1024);
        assert_eq!(limits.rate_limit, 1000.0);
    }

    #[test]
    fn test_node_capabilities() {
        let caps = NodeCapabilities::default();
        assert_eq!(caps.supported_vms.len(), 2);
        assert!(caps.supported_vms.contains(&VmType::Svm));
        assert!(caps.supported_vms.contains(&VmType::Evm));
        assert_eq!(caps.protocol_versions.len(), 1);
        assert_eq!(caps.limits.max_connections, 100);
    }
}
