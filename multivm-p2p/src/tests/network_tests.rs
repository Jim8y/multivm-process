//! Production-ready tests for P2P network components

#[cfg(test)]
mod tests {
    use crate::core::manager::NetworkEvent;
    use crate::error::P2PError;
    use crate::protocol::messages::*;
    use libp2p::{Multiaddr, PeerId};
    use std::time::Duration;

    #[test]
    fn test_network_event_creation() {
        let peer_id = PeerId::random();
        let address: Multiaddr = "/ip4/127.0.0.1/tcp/8000".parse().unwrap();

        // Test PeerConnected event
        let connect_event = NetworkEvent::PeerConnected {
            peer_id,
            address: address.clone(),
        };

        match connect_event {
            NetworkEvent::PeerConnected {
                peer_id: _,
                address: _,
            } => {
                // Event created successfully
            }
            _ => panic!("Wrong event type"),
        }

        // Test PeerDisconnected event
        let disconnect_event = NetworkEvent::PeerDisconnected {
            peer_id,
            reason: "Test disconnection".to_string(),
        };

        match disconnect_event {
            NetworkEvent::PeerDisconnected { peer_id: _, reason } => {
                assert_eq!(reason, "Test disconnection");
            }
            _ => panic!("Wrong event type"),
        }

        // Test MessageReceived event
        let message_event = NetworkEvent::MessageReceived {
            peer_id,
            message: vec![1, 2, 3, 4],
        };

        match message_event {
            NetworkEvent::MessageReceived {
                peer_id: _,
                message,
            } => {
                assert_eq!(message, vec![1, 2, 3, 4]);
            }
            _ => panic!("Wrong event type"),
        }

        // Test Error event
        let error_event = NetworkEvent::Error {
            error: P2PError::ConnectionError {
                message: "Test error".to_string(),
            },
        };

        match error_event {
            NetworkEvent::Error { error } => match error {
                P2PError::ConnectionError { message } => {
                    assert_eq!(message, "Test error");
                }
                _ => panic!("Wrong error type"),
            },
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_message_creation() {
        // Test creating different message types
        let heartbeat_msg = NetworkMessage::new(
            MessagePayload::Control(ControlMessage::Heartbeat {
                status: NodeStatus::Active,
                uptime: Duration::from_secs(3600),
            }),
            MessageSource::NetworkLayer,
            MessageTarget::Broadcast,
        );

        // Verify message has correct fields
        assert!(!heartbeat_msg.id.is_empty());
        assert_eq!(heartbeat_msg.source, MessageSource::NetworkLayer);
        assert_eq!(heartbeat_msg.target, MessageTarget::Broadcast);
        assert_eq!(heartbeat_msg.version, 1);

        // Test SVM message
        let svm_msg = NetworkMessage::new(
            MessagePayload::Svm(SvmMessage::Transaction {
                transaction_data: Box::new(vec![1, 2, 3, 4]),
                signature: "test_signature".to_string(),
            }),
            MessageSource::SvmExecution,
            MessageTarget::Broadcast,
        );

        assert_eq!(svm_msg.source, MessageSource::SvmExecution);
        match svm_msg.payload {
            MessagePayload::Svm(SvmMessage::Transaction { signature, .. }) => {
                assert_eq!(signature, "test_signature");
            }
            _ => panic!("Wrong message payload type"),
        }

        // Test EVM message
        let evm_msg = NetworkMessage::new(
            MessagePayload::Evm(EvmMessage::Transaction {
                transaction_data: Box::new(vec![5, 6, 7, 8]),
                tx_hash: "0xtest_hash".to_string(),
            }),
            MessageSource::EvmExecution,
            MessageTarget::Broadcast,
        );

        assert_eq!(evm_msg.source, MessageSource::EvmExecution);
        match evm_msg.payload {
            MessagePayload::Evm(EvmMessage::Transaction { tx_hash, .. }) => {
                assert_eq!(tx_hash, "0xtest_hash");
            }
            _ => panic!("Wrong message payload type"),
        }
    }

    #[test]
    fn test_message_metadata() {
        let mut msg = NetworkMessage::new(
            MessagePayload::Control(ControlMessage::StatusRequest),
            MessageSource::NetworkLayer,
            MessageTarget::Broadcast,
        );

        // Test adding metadata
        msg = msg.with_metadata("key1", "value1");
        msg = msg.with_metadata("key2", "value2");

        assert_eq!(msg.metadata.get("key1"), Some(&"value1".to_string()));
        assert_eq!(msg.metadata.get("key2"), Some(&"value2".to_string()));
        assert_eq!(msg.metadata.len(), 2);
    }

    #[test]
    fn test_message_serialization() {
        let msg = NetworkMessage::new(
            MessagePayload::Control(ControlMessage::StatusRequest),
            MessageSource::NetworkLayer,
            MessageTarget::Broadcast,
        );

        // Test serialization
        let serialized = bincode::serialize(&msg);
        assert!(serialized.is_ok(), "Failed to serialize message");

        // Test deserialization
        let serialized_data = serialized.unwrap();
        let deserialized: Result<NetworkMessage, _> = bincode::deserialize(&serialized_data);
        assert!(deserialized.is_ok(), "Failed to deserialize message");

        let deserialized_msg = deserialized.unwrap();
        assert_eq!(msg.id, deserialized_msg.id);
        assert_eq!(msg.source, deserialized_msg.source);
        assert_eq!(msg.target, deserialized_msg.target);
        assert_eq!(msg.version, deserialized_msg.version);
    }
}
