//! Integration tests for communication protocols

use crate::communication::{
    config::{CommunicationConfig, GlobalSettings, ProtocolConfigs, RetrySettings},
    factory::DefaultProtocolFactory,
    ipc_protocol::IpcProtocolConfig,
    jwt_protocol::JwtProtocolConfig,
    rpc_protocol::RpcProtocolConfig,
    CommunicationManager, CommunicationRequest, EngineType, ProtocolType,
};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_communication_config() -> CommunicationConfig {
        let mut protocol_preferences = HashMap::new();
        protocol_preferences.insert(
            EngineType::Ethereum,
            vec![ProtocolType::Ipc, ProtocolType::Rpc, ProtocolType::Jwt],
        );
        protocol_preferences.insert(
            EngineType::Solana,
            vec![ProtocolType::Jwt, ProtocolType::Rpc, ProtocolType::Ipc],
        );

        CommunicationConfig {
            protocol_preferences,
            protocol_configs: ProtocolConfigs {
                ipc: HashMap::new(),
                rpc: HashMap::new(),
                jwt: HashMap::new(),
            },
            global_settings: GlobalSettings {
                enable_health_checks: true,
                default_timeout_seconds: 30,
                max_concurrent_connections: 10,
                enable_fallback: true,
                retry_settings: RetrySettings {
                    max_connection_retries: 3,
                    connection_retry_delay_ms: 100,
                    max_retry_delay_ms: 5000,
                    retry_backoff_multiplier: 2.0,
                },
            },
        }
    }

    #[tokio::test]
    async fn test_integration_config_to_manager() {
        let mut config = create_test_communication_config();

        // Add specific protocol configs
        config.protocol_configs.ipc.insert(
            EngineType::Ethereum,
            IpcProtocolConfig {
                transport_config: crate::communication::ipc_protocol::IpcTransportConfig {
                    tcp_port: 8545,
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        config.protocol_configs.rpc.insert(
            EngineType::Ethereum,
            RpcProtocolConfig {
                endpoint_url: "http://localhost:8545".to_string(),
                ..Default::default()
            },
        );

        config.protocol_configs.jwt.insert(
            EngineType::Solana,
            JwtProtocolConfig {
                endpoint_url: "http://localhost:8900".to_string(),
                jwt_secret: "test-secret-key-that-is-long-enough-for-security-requirements"
                    .to_string(),
                ..Default::default()
            },
        );

        // Create manager manually (since from_config doesn't exist)
        let factory = Box::new(DefaultProtocolFactory::new());
        let mut manager = CommunicationManager::new(factory);

        // Add protocols from config
        for (engine_type, ipc_config) in config.protocol_configs.ipc {
            manager
                .add_protocol(
                    ProtocolType::Ipc,
                    engine_type,
                    serde_json::to_value(ipc_config).unwrap(),
                )
                .await
                .unwrap();
        }

        for (engine_type, rpc_config) in config.protocol_configs.rpc {
            manager
                .add_protocol(
                    ProtocolType::Rpc,
                    engine_type,
                    serde_json::to_value(rpc_config).unwrap(),
                )
                .await
                .unwrap();
        }

        for (engine_type, jwt_config) in config.protocol_configs.jwt {
            manager
                .add_protocol(
                    ProtocolType::Jwt,
                    engine_type,
                    serde_json::to_value(jwt_config).unwrap(),
                )
                .await
                .unwrap();
        }

        // Set preferences
        for (engine_type, prefs) in config.protocol_preferences {
            manager.set_protocol_preferences(engine_type, prefs);
        }

        // Verify protocols were created
        assert!(manager
            .get_protocol(&ProtocolType::Ipc, &EngineType::Ethereum)
            .is_some());
        assert!(manager
            .get_protocol(&ProtocolType::Rpc, &EngineType::Ethereum)
            .is_some());
        assert!(manager
            .get_protocol(&ProtocolType::Jwt, &EngineType::Solana)
            .is_some());

        // Verify preferences were set
        let eth_prefs = manager
            .get_protocol_preferences(&EngineType::Ethereum)
            .unwrap();
        assert_eq!(
            *eth_prefs,
            vec![ProtocolType::Ipc, ProtocolType::Rpc, ProtocolType::Jwt]
        );

        let sol_prefs = manager
            .get_protocol_preferences(&EngineType::Solana)
            .unwrap();
        assert_eq!(
            *sol_prefs,
            vec![ProtocolType::Jwt, ProtocolType::Rpc, ProtocolType::Ipc]
        );
    }

    #[tokio::test]
    async fn test_integration_protocol_lifecycle() {
        let factory = Box::new(DefaultProtocolFactory::new());
        let mut manager = CommunicationManager::new(factory);

        // Add protocol
        let config = serde_json::json!({
            "endpoint_url": "http://test.endpoint.com",
            "jwt_secret": "test-secret-key-that-is-long-enough-for-security-requirements"
        });

        manager
            .add_protocol(ProtocolType::Jwt, EngineType::Ethereum, config)
            .await
            .unwrap();

        // Verify it exists
        let protocol = manager.get_protocol(&ProtocolType::Jwt, &EngineType::Ethereum);
        assert!(protocol.is_some());

        // Update configuration
        let new_config = serde_json::json!({
            "endpoint_url": "http://updated.endpoint.com",
            "jwt_secret": "updated-secret-key-that-is-long-enough-for-security"
        });

        manager
            .add_protocol(ProtocolType::Jwt, EngineType::Ethereum, new_config)
            .await
            .unwrap();

        // Verify configuration was updated
        let updated_protocol = manager
            .get_protocol(&ProtocolType::Jwt, &EngineType::Ethereum)
            .unwrap();
        let config_value = updated_protocol.get_configuration();
        assert_eq!(config_value["endpoint_url"], "http://updated.endpoint.com");

        // Replace protocol by adding again
        manager
            .add_protocol(
                ProtocolType::Jwt,
                EngineType::Ethereum,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        // Verify it exists
        assert!(manager
            .get_protocol(&ProtocolType::Jwt, &EngineType::Ethereum)
            .is_some());
    }

    #[tokio::test]
    async fn test_integration_multi_engine_setup() {
        let mut config = create_test_communication_config();

        // Configure multiple protocols for multiple engines
        for engine in [EngineType::Ethereum, EngineType::Solana] {
            config.protocol_configs.ipc.insert(
                engine.clone(),
                IpcProtocolConfig {
                    transport_config: crate::communication::ipc_protocol::IpcTransportConfig {
                        tcp_port: if matches!(engine, EngineType::Ethereum) {
                            8545
                        } else {
                            8899
                        },
                        ..Default::default()
                    },
                    ..Default::default()
                },
            );

            config.protocol_configs.rpc.insert(
                engine.clone(),
                RpcProtocolConfig {
                    endpoint_url: format!(
                        "http://localhost:{}",
                        if matches!(engine, EngineType::Ethereum) {
                            8545
                        } else {
                            8899
                        }
                    ),
                    ..Default::default()
                },
            );

            config.protocol_configs.jwt.insert(
                engine.clone(),
                JwtProtocolConfig {
                    endpoint_url: format!(
                        "http://localhost:{}",
                        if matches!(engine, EngineType::Ethereum) {
                            8551
                        } else {
                            8900
                        }
                    ),
                    jwt_secret: "test-secret-key-that-is-long-enough-for-security-requirements"
                        .to_string(),
                    ..Default::default()
                },
            );
        }

        // Create manager manually
        let factory = Box::new(DefaultProtocolFactory::new());
        let mut manager = CommunicationManager::new(factory);

        // Add all protocols from config
        for (engine_type, ipc_config) in config.protocol_configs.ipc {
            manager
                .add_protocol(
                    ProtocolType::Ipc,
                    engine_type,
                    serde_json::to_value(ipc_config).unwrap(),
                )
                .await
                .unwrap();
        }

        for (engine_type, rpc_config) in config.protocol_configs.rpc {
            manager
                .add_protocol(
                    ProtocolType::Rpc,
                    engine_type,
                    serde_json::to_value(rpc_config).unwrap(),
                )
                .await
                .unwrap();
        }

        for (engine_type, jwt_config) in config.protocol_configs.jwt {
            manager
                .add_protocol(
                    ProtocolType::Jwt,
                    engine_type,
                    serde_json::to_value(jwt_config).unwrap(),
                )
                .await
                .unwrap();
        }

        // Verify all protocols for both engines
        for engine in [EngineType::Ethereum, EngineType::Solana] {
            for protocol in [ProtocolType::Ipc, ProtocolType::Rpc, ProtocolType::Jwt] {
                assert!(
                    manager.get_protocol(&protocol, &engine).is_some(),
                    "Missing {protocol} protocol for {engine} engine"
                );
            }
        }

        // Verify engine-specific configurations
        let eth_ipc = manager
            .get_protocol(&ProtocolType::Ipc, &EngineType::Ethereum)
            .unwrap();
        let eth_ipc_config = eth_ipc.get_configuration();
        assert_eq!(eth_ipc_config["transport_config"]["tcp_port"], 8545);

        let sol_rpc = manager
            .get_protocol(&ProtocolType::Rpc, &EngineType::Solana)
            .unwrap();
        let sol_rpc_config = sol_rpc.get_configuration();
        assert_eq!(sol_rpc_config["endpoint_url"], "http://localhost:8899");
    }

    #[tokio::test]
    async fn test_integration_error_scenarios() {
        let factory = Box::new(DefaultProtocolFactory::new());
        let manager = CommunicationManager::new(factory);

        // Test sending without any protocols
        let request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: "test".to_string(),
            params: serde_json::Value::Null,
            timeout: Some(Duration::from_secs(5)),
            auth_context: None,
        };

        let result = manager
            .send_with_fallback(&EngineType::Ethereum, request.clone())
            .await;
        assert!(result.is_err());

        // Test send with fallback without protocols
        let result = manager
            .send_with_fallback(&EngineType::Ethereum, request)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_integration_configuration_validation() {
        let mut config = create_test_communication_config();

        // Add invalid JWT config (short secret)
        config.protocol_configs.jwt.insert(
            EngineType::Ethereum,
            JwtProtocolConfig {
                jwt_secret: "short".to_string(), // Too short
                ..Default::default()
            },
        );

        // Create manager and try to add invalid JWT config
        let factory = Box::new(DefaultProtocolFactory::new());
        let mut manager = CommunicationManager::new(factory);

        // Try to add the invalid JWT config
        let result = manager
            .add_protocol(
                ProtocolType::Jwt,
                EngineType::Ethereum,
                serde_json::to_value(
                    config
                        .protocol_configs
                        .jwt
                        .get(&EngineType::Ethereum)
                        .unwrap(),
                )
                .unwrap(),
            )
            .await;

        // Should fail due to short secret
        assert!(result.is_err());
    }

    #[test]
    fn test_integration_config_serialization() {
        let config = create_test_communication_config();

        // Test serialization
        let serialized = serde_json::to_string(&config).unwrap();
        assert!(serialized.contains("protocol_preferences"));
        assert!(serialized.contains("global_settings"));

        // Test deserialization
        let deserialized: CommunicationConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(
            deserialized.global_settings.default_timeout_seconds,
            config.global_settings.default_timeout_seconds
        );
    }

    #[tokio::test]
    async fn test_integration_multiple_protocols() {
        let factory = Box::new(DefaultProtocolFactory::new());
        let mut manager = CommunicationManager::new(factory);

        // Add multiple protocols
        let protocols = vec![
            (
                ProtocolType::Ipc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            ),
            (
                ProtocolType::Rpc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            ),
            (
                ProtocolType::Jwt,
                EngineType::Solana,
                serde_json::json!({
                    "jwt_secret": "test-secret-key-that-is-long-enough-for-security-requirements"
                }),
            ),
        ];

        // Add protocols sequentially
        for (pt, et, config) in protocols {
            manager.add_protocol(pt, et, config).await.unwrap();
        }

        // Verify all protocols were added
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
}
