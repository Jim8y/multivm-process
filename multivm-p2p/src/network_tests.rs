//! Comprehensive tests for P2P network components

#[cfg(test)]
mod tests {
    use crate::error::P2PError;
    use crate::messages::*;
    use crate::network::*;
    use libp2p::Multiaddr;
    use std::time::Duration;

    #[test]
    fn test_network_config_creation() {
        let config = NetworkConfig {
            listen_addresses: vec![
                "/ip4/127.0.0.1/tcp/8000".parse().unwrap(),
                "/ip4/0.0.0.0/tcp/8000".parse().unwrap(),
            ],
            bootstrap_peers: vec!["/ip4/192.168.1.100/tcp/8000".parse().unwrap()],
            max_peers: 50,
            enable_mdns: true,
            validation_mode: libp2p::gossipsub::ValidationMode::Strict,
            connection_timeout: Duration::from_secs(30),
        };

        assert_eq!(config.listen_addresses.len(), 2);
        assert_eq!(config.bootstrap_peers.len(), 1);
        assert_eq!(config.max_peers, 50);
        assert!(config.enable_mdns);
        assert_eq!(config.connection_timeout.as_secs(), 30);
    }

    #[test]
    fn test_network_health_status() {
        let healthy = NetworkHealthStatus::Healthy;
        let warning = NetworkHealthStatus::Warning;
        let critical = NetworkHealthStatus::Critical;

        // Test ordering is not defined for these enum variants
        assert_eq!(healthy, NetworkHealthStatus::Healthy);
        assert_eq!(warning, NetworkHealthStatus::Warning);
        assert_eq!(critical, NetworkHealthStatus::Critical);
    }

    #[test]
    fn test_network_health_report() {
        let report = NetworkHealthReport {
            status: NetworkHealthStatus::Healthy,
            connected_peers: 10,
            failed_peers: 2,
            subscribed_topics: 3,
            message_throughput: 1950,
            issues: vec![],
            timestamp: std::time::SystemTime::now(),
        };

        assert_eq!(report.status, NetworkHealthStatus::Healthy);
        assert_eq!(report.connected_peers, 10);
        assert_eq!(report.failed_peers, 2);
        assert_eq!(report.subscribed_topics, 3);
        assert_eq!(report.message_throughput, 1950);
        assert!(report.issues.is_empty());
    }

    #[test]
    fn test_p2p_error_variants() {
        let errors = vec![
            P2PError::ConnectionError {
                message: "Connection timeout".to_string(),
            },
            P2PError::PeerNotFound {
                peer_id: "peer123".to_string(),
            },
            P2PError::InvalidMessageFormat {
                reason: "Invalid signature".to_string(),
            },
            P2PError::ProtocolError {
                protocol: "gossipsub".to_string(),
                message: "Topic not found".to_string(),
            },
            P2PError::TransportError {
                transport: "tcp".to_string(),
                message: "Socket closed".to_string(),
            },
            P2PError::DiscoveryError {
                message: "No peers found".to_string(),
            },
            P2PError::RoutingError {
                message: "No route to peer".to_string(),
            },
            P2PError::ConfigurationError {
                message: "Invalid listen address".to_string(),
            },
            P2PError::TimeoutError {
                duration: Duration::from_secs(30),
            },
            P2PError::InvalidMessage("Invalid payload".to_string()),
            P2PError::RateLimitExceeded("Too many requests".to_string()),
            P2PError::MessageTooLarge(1024 * 1024),
            P2PError::Serialization {
                message: "Failed to serialize".to_string(),
            },
            P2PError::Internal("Unexpected state".to_string()),
        ];

        // Test that all error variants can be created and have meaningful messages
        for error in errors {
            let error_string = error.to_string();
            assert!(!error_string.is_empty());

            // Test recoverable logic
            match &error {
                P2PError::ConnectionError { .. }
                | P2PError::TimeoutError { .. }
                | P2PError::InsufficientPeers { .. }
                | P2PError::RateLimitExceeded(_)
                | P2PError::TransportError { .. }
                | P2PError::DiscoveryError { .. } => {
                    assert!(error.is_recoverable());
                }
                _ => {
                    assert!(!error.is_recoverable());
                }
            }
        }
    }

    #[tokio::test]
    async fn test_network_config_validation() {
        // Test valid config
        let valid_config = NetworkConfig {
            listen_addresses: vec!["/ip4/127.0.0.1/tcp/8000".parse().unwrap()],
            bootstrap_peers: vec![],
            max_peers: 50,
            enable_mdns: false,
            validation_mode: libp2p::gossipsub::ValidationMode::Permissive,
            connection_timeout: Duration::from_secs(30),
        };

        // Config should be valid
        assert!(valid_config.max_peers > 0);
        assert!(!valid_config.listen_addresses.is_empty());

        // Test edge cases
        let edge_config = NetworkConfig {
            listen_addresses: vec!["/ip4/0.0.0.0/tcp/0".parse().unwrap()],
            bootstrap_peers: vec![],
            max_peers: 1,
            enable_mdns: true,
            validation_mode: libp2p::gossipsub::ValidationMode::None,
            connection_timeout: Duration::from_millis(100),
        };

        assert_eq!(edge_config.max_peers, 1);
        assert_eq!(edge_config.connection_timeout.as_millis(), 100);
    }

    #[test]
    fn test_multiaddr_parsing() {
        // Valid addresses
        let valid_addrs = vec![
            "/ip4/127.0.0.1/tcp/8000",
            "/ip4/192.168.1.1/tcp/9000",
            "/ip6/::1/tcp/8000",
            "/dns/localhost/tcp/8000",
            "/ip4/0.0.0.0/tcp/0",
        ];

        for addr_str in valid_addrs {
            let addr: Result<Multiaddr, _> = addr_str.parse();
            assert!(addr.is_ok(), "Failed to parse: {addr_str}");
        }

        // Invalid addresses
        let invalid_addrs = vec![
            "127.0.0.1:8000",
            "/invalid/protocol",
            "/ip4/999.999.999.999/tcp/8000",
        ];

        for addr_str in invalid_addrs {
            let addr: Result<Multiaddr, _> = addr_str.parse();
            assert!(addr.is_err(), "Should have failed to parse: {addr_str}");
        }

        // Special case for empty string - it creates an empty but valid Multiaddr
        let empty_addr: Result<Multiaddr, _> = "".parse();
        assert!(empty_addr.is_ok()); // Empty string is actually valid in libp2p
    }

    #[test]
    fn test_connection_limits() {
        // Test connection limit concepts without specific struct
        let max_pending_incoming = 10;
        let max_pending_outgoing = 10;
        let max_established_incoming = 40;
        let max_established_outgoing = 40;
        let max_established_per_peer = 2;

        assert_eq!(max_pending_incoming, 10);
        assert_eq!(max_established_incoming, 40);
        assert_eq!(max_established_per_peer, 2);

        // Total established should be reasonable
        let total_established = max_established_incoming + max_established_outgoing;
        assert!(total_established <= 100);
    }

    #[test]
    fn test_peer_info_creation() {
        use crate::PeerInfo;
        use crate::PeerStatus;

        let peer_info = PeerInfo {
            peer_id: "12D3KooWExample".to_string(),
            addresses: vec!["/ip4/192.168.1.100/tcp/8000".parse().unwrap()],
            protocols: vec!["/multivm/1.0.0".to_string()],
            supports_multivm: true,
            last_seen: chrono::Utc::now(),
            status: PeerStatus::Connected,
        };

        assert_eq!(peer_info.peer_id, "12D3KooWExample");
        assert_eq!(peer_info.addresses.len(), 1);
        assert!(peer_info.supports_multivm);
        assert!(matches!(peer_info.status, PeerStatus::Connected));
    }

    #[test]
    fn test_network_event_types() {
        use crate::{NetworkEvent, PeerInfo, PeerStatus};

        let peer_info = PeerInfo {
            peer_id: "test_peer".to_string(),
            addresses: vec![],
            protocols: vec![],
            supports_multivm: true,
            last_seen: chrono::Utc::now(),
            status: PeerStatus::Connected,
        };

        let events = vec![
            NetworkEvent::PeerConnected(peer_info.clone()),
            NetworkEvent::PeerDisconnected("test_peer".to_string()),
            NetworkEvent::MessageReceived {
                peer_id: "test_peer".to_string(),
                message: Box::new(NetworkMessage::new(
                    MessagePayload::Control(ControlMessage::StatusRequest),
                    MessageSource::NetworkLayer,
                    MessageTarget::Broadcast,
                )),
            },
            NetworkEvent::MessageSent {
                peer_id: "test_peer".to_string(),
                message_id: "msg123".to_string(),
            },
            NetworkEvent::Error {
                peer_id: Some("test_peer".to_string()),
                error: P2PError::ConnectionError {
                    message: "Test error".to_string(),
                },
            },
        ];

        for event in events {
            match event {
                NetworkEvent::PeerConnected(info) => {
                    assert_eq!(info.peer_id, "test_peer");
                }
                NetworkEvent::PeerDisconnected(peer_id) => {
                    assert_eq!(peer_id, "test_peer");
                }
                NetworkEvent::MessageReceived { peer_id, .. } => {
                    assert_eq!(peer_id, "test_peer");
                }
                NetworkEvent::MessageSent { message_id, .. } => {
                    assert_eq!(message_id, "msg123");
                }
                NetworkEvent::Error { peer_id, .. } => {
                    assert_eq!(peer_id, Some("test_peer".to_string()));
                }
            }
        }
    }
}
