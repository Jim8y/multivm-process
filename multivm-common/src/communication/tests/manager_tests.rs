//! Communication manager unit tests

use crate::communication::{
    factory::DefaultProtocolFactory, CommunicationManager, CommunicationRequest, EngineType,
    ProtocolType,
};
use std::time::Duration;
use uuid::Uuid;

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_manager() -> CommunicationManager {
        let factory = Box::new(DefaultProtocolFactory::new());
        CommunicationManager::new(factory)
    }

    #[tokio::test]
    async fn test_manager_creation() {
        let manager = create_test_manager();

        // Should start empty
        assert!(manager
            .get_protocol(&ProtocolType::Ipc, &EngineType::Ethereum)
            .is_none());
        assert!(manager
            .get_protocol(&ProtocolType::Rpc, &EngineType::Solana)
            .is_none());
    }

    #[tokio::test]
    async fn test_manager_add_protocol() {
        let mut manager = create_test_manager();

        let config = serde_json::json!({
            "jwt_secret": "test-secret-key-that-is-long-enough-for-security-requirements"
        });

        let result = manager
            .add_protocol(ProtocolType::Jwt, EngineType::Ethereum, config)
            .await;

        assert!(result.is_ok());
        assert!(manager
            .get_protocol(&ProtocolType::Jwt, &EngineType::Ethereum)
            .is_some());
    }

    #[tokio::test]
    async fn test_manager_protocol_replacement() {
        let mut manager = create_test_manager();

        // Add a protocol
        manager
            .add_protocol(
                ProtocolType::Ipc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        assert!(manager
            .get_protocol(&ProtocolType::Ipc, &EngineType::Ethereum)
            .is_some());

        // Replace with new config by adding again
        let new_config = serde_json::json!({
            "transport_config": {
                "use_unix_sockets": false,
                "tcp_host": "127.0.0.1",
                "tcp_port": 9999,
                "unix_socket_path": null,
                "connect_timeout": { "secs": 5, "nanos": 0 },
                "read_timeout": { "secs": 5, "nanos": 0 },
                "write_timeout": { "secs": 5, "nanos": 0 }
            }
        });

        manager
            .add_protocol(ProtocolType::Ipc, EngineType::Ethereum, new_config)
            .await
            .unwrap();

        // Should still have the protocol
        assert!(manager
            .get_protocol(&ProtocolType::Ipc, &EngineType::Ethereum)
            .is_some());
    }

    #[tokio::test]
    async fn test_manager_set_protocol_preference() {
        let mut manager = create_test_manager();

        // Add multiple protocols for the same engine
        manager
            .add_protocol(
                ProtocolType::Ipc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        manager
            .add_protocol(
                ProtocolType::Rpc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        // Set preference
        manager.set_protocol_preferences(
            EngineType::Ethereum,
            vec![ProtocolType::Rpc, ProtocolType::Ipc],
        );

        let preferences = manager.get_protocol_preferences(&EngineType::Ethereum);
        assert!(preferences.is_some());
        assert_eq!(
            *preferences.unwrap(),
            vec![ProtocolType::Rpc, ProtocolType::Ipc]
        );
    }

    #[tokio::test]
    async fn test_manager_protocol_existence() {
        let mut manager = create_test_manager();

        // Add protocols
        manager
            .add_protocol(
                ProtocolType::Ipc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        manager
            .add_protocol(
                ProtocolType::Rpc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        // Set preferences
        manager.set_protocol_preferences(
            EngineType::Ethereum,
            vec![ProtocolType::Rpc, ProtocolType::Ipc],
        );

        // Verify protocols exist
        assert!(manager
            .get_protocol(&ProtocolType::Rpc, &EngineType::Ethereum)
            .is_some());
        assert!(manager
            .get_protocol(&ProtocolType::Ipc, &EngineType::Ethereum)
            .is_some());
    }

    #[tokio::test]
    async fn test_manager_send_no_protocol() {
        let manager = create_test_manager();

        let request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: "test".to_string(),
            params: serde_json::Value::Null,
            timeout: Some(Duration::from_secs(5)),
            auth_context: None,
        };

        // Try to send with non-existent protocol
        let result = manager
            .send_with_fallback(&EngineType::Ethereum, request)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_manager_protocol_verification() {
        let mut manager = create_test_manager();

        // Add a protocol
        manager
            .add_protocol(
                ProtocolType::Ipc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        // Verify protocol exists
        assert!(manager
            .get_protocol(&ProtocolType::Ipc, &EngineType::Ethereum)
            .is_some());

        // Verify non-existent protocol doesn't exist
        assert!(manager
            .get_protocol(&ProtocolType::Rpc, &EngineType::Ethereum)
            .is_none());
    }

    #[tokio::test]
    async fn test_manager_send_with_fallback() {
        let mut manager = create_test_manager();

        // Add multiple protocols
        manager
            .add_protocol(
                ProtocolType::Ipc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        manager
            .add_protocol(
                ProtocolType::Rpc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        // Set preference order
        manager.set_protocol_preferences(
            EngineType::Ethereum,
            vec![ProtocolType::Ipc, ProtocolType::Rpc],
        );

        let request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: "test".to_string(),
            params: serde_json::Value::Null,
            timeout: Some(Duration::from_secs(5)),
            auth_context: None,
        };

        // Will try IPC first, then RPC, both will fail without actual servers
        let result = manager
            .send_with_fallback(&EngineType::Ethereum, request)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_manager_get_all_protocols() {
        let mut manager = create_test_manager();

        // Add various protocols
        manager
            .add_protocol(
                ProtocolType::Ipc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        manager
            .add_protocol(
                ProtocolType::Rpc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        manager
            .add_protocol(
                ProtocolType::Jwt,
                EngineType::Solana,
                serde_json::json!({
                    "jwt_secret": "test-secret-key-that-is-long-enough-for-security-requirements"
                }),
            )
            .await
            .unwrap();

        // Verify all protocols exist by checking individual ones
        assert!(manager
            .get_protocol(&ProtocolType::Ipc, &EngineType::Ethereum)
            .is_some());
        assert!(manager
            .get_protocol(&ProtocolType::Rpc, &EngineType::Ethereum)
            .is_some());
        assert!(manager
            .get_protocol(&ProtocolType::Jwt, &EngineType::Solana)
            .is_some());
    }

    #[tokio::test]
    async fn test_manager_protocol_verification_per_engine() {
        let mut manager = create_test_manager();

        // Add protocols for different engines
        manager
            .add_protocol(
                ProtocolType::Ipc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        manager
            .add_protocol(
                ProtocolType::Rpc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        manager
            .add_protocol(
                ProtocolType::Jwt,
                EngineType::Solana,
                serde_json::json!({
                    "jwt_secret": "test-secret-key-that-is-long-enough-for-security-requirements"
                }),
            )
            .await
            .unwrap();

        // Verify Ethereum protocols
        assert!(manager
            .get_protocol(&ProtocolType::Ipc, &EngineType::Ethereum)
            .is_some());
        assert!(manager
            .get_protocol(&ProtocolType::Rpc, &EngineType::Ethereum)
            .is_some());
        assert!(manager
            .get_protocol(&ProtocolType::Jwt, &EngineType::Ethereum)
            .is_none());

        // Verify Solana protocols
        assert!(manager
            .get_protocol(&ProtocolType::Ipc, &EngineType::Solana)
            .is_none());
        assert!(manager
            .get_protocol(&ProtocolType::Rpc, &EngineType::Solana)
            .is_none());
        assert!(manager
            .get_protocol(&ProtocolType::Jwt, &EngineType::Solana)
            .is_some());
    }

    #[tokio::test]
    async fn test_manager_default_preferences() {
        let mut manager = create_test_manager();

        // Add protocols without setting preferences
        manager
            .add_protocol(
                ProtocolType::Rpc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        manager
            .add_protocol(
                ProtocolType::Ipc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        // Without explicit preferences, just verify protocols exist
        assert!(manager
            .get_protocol(&ProtocolType::Rpc, &EngineType::Ethereum)
            .is_some());
        assert!(manager
            .get_protocol(&ProtocolType::Ipc, &EngineType::Ethereum)
            .is_some());
    }

    #[tokio::test]
    async fn test_manager_multiple_engines() {
        let mut manager = create_test_manager();

        // Add protocols for different engines
        manager
            .add_protocol(
                ProtocolType::Ipc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        manager
            .add_protocol(
                ProtocolType::Rpc,
                EngineType::Solana,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        // Verify each engine has its protocol
        assert!(manager
            .get_protocol(&ProtocolType::Ipc, &EngineType::Ethereum)
            .is_some());
        assert!(manager
            .get_protocol(&ProtocolType::Rpc, &EngineType::Solana)
            .is_some());

        // Verify crossing doesn't work
        assert!(manager
            .get_protocol(&ProtocolType::Ipc, &EngineType::Solana)
            .is_none());
        assert!(manager
            .get_protocol(&ProtocolType::Rpc, &EngineType::Ethereum)
            .is_none());
    }
}
