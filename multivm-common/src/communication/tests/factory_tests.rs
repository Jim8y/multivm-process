//! Factory pattern unit tests

use crate::communication::{
    factory::{DefaultProtocolFactory, ProtocolConfigBuilder},
    ipc_protocol::IpcProtocolConfig,
    jwt_protocol::JwtProtocolConfig,
    rpc_protocol::RpcProtocolConfig,
    EngineType, ProtocolFactory, ProtocolType,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_default_factory_creation() {
        let factory = DefaultProtocolFactory::new();

        assert!(factory.supports_protocol(&ProtocolType::Ipc));
        assert!(factory.supports_protocol(&ProtocolType::Rpc));
        assert!(factory.supports_protocol(&ProtocolType::Jwt));

        let supported = factory.supported_protocols();
        assert_eq!(supported.len(), 3);
    }

    #[tokio::test]
    async fn test_factory_with_limited_protocols() {
        let factory =
            DefaultProtocolFactory::with_protocols(vec![ProtocolType::Ipc, ProtocolType::Rpc]);

        assert!(factory.supports_protocol(&ProtocolType::Ipc));
        assert!(factory.supports_protocol(&ProtocolType::Rpc));
        assert!(!factory.supports_protocol(&ProtocolType::Jwt));
    }

    #[tokio::test]
    async fn test_factory_create_ipc_protocol() {
        let factory = DefaultProtocolFactory::new();

        let protocol = factory
            .create_protocol(
                ProtocolType::Ipc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        assert_eq!(protocol.protocol_type(), ProtocolType::Ipc);
        assert_eq!(protocol.engine_type(), EngineType::Ethereum);
    }

    #[tokio::test]
    async fn test_factory_create_rpc_protocol() {
        let factory = DefaultProtocolFactory::new();

        let protocol = factory
            .create_protocol(
                ProtocolType::Rpc,
                EngineType::Solana,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        assert_eq!(protocol.protocol_type(), ProtocolType::Rpc);
        assert_eq!(protocol.engine_type(), EngineType::Solana);
    }

    #[tokio::test]
    async fn test_factory_create_jwt_protocol() {
        let factory = DefaultProtocolFactory::new();

        let custom_config = serde_json::json!({
            "jwt_secret": "test-secret-key-that-is-long-enough-for-security-requirements"
        });

        let protocol = factory
            .create_protocol(ProtocolType::Jwt, EngineType::Ethereum, custom_config)
            .await
            .unwrap();

        assert_eq!(protocol.protocol_type(), ProtocolType::Jwt);
        assert_eq!(protocol.engine_type(), EngineType::Ethereum);
    }

    #[tokio::test]
    async fn test_factory_unsupported_protocol() {
        let factory = DefaultProtocolFactory::with_protocols(vec![ProtocolType::Ipc]);

        let result = factory
            .create_protocol(
                ProtocolType::Rpc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            )
            .await;

        assert!(result.is_err());
        match result.err().unwrap() {
            crate::MultivmError::UnsupportedOperation { alternatives, .. } => {
                assert!(alternatives.is_some());
                assert!(alternatives.unwrap().contains(&"ipc".to_string()));
            }
            e => panic!("Expected UnsupportedOperation error, got: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_factory_custom_default_config() {
        let mut factory = DefaultProtocolFactory::new();

        let custom_default = serde_json::json!({
            "endpoint_url": "http://custom.endpoint.com",
            "connect_timeout": { "secs": 20, "nanos": 0 },
            "request_timeout": { "secs": 60, "nanos": 0 },
            "max_connections": 20,
            "retry_config": {
                "max_retries": 5,
                "base_delay": { "secs": 0, "nanos": 500000000 },
                "max_delay": { "secs": 30, "nanos": 0 },
                "backoff_multiplier": 3.0,
                "jitter": true
            },
            "health_check_config": {
                "health_method": "custom_health",
                "interval": { "secs": 120, "nanos": 0 },
                "timeout": { "secs": 15, "nanos": 0 },
                "enable_auto_check": false
            },
            "default_headers": {},
            "user_agent": "Custom-Agent/1.0",
            "use_http2": false,
            "tls_config": {
                "accept_invalid_certs": false,
                "accept_invalid_hostnames": false,
                "ca_cert_path": null,
                "client_cert_path": null,
                "client_key_path": null
            }
        });

        factory.set_default_config(ProtocolType::Rpc, custom_default);

        let protocol = factory
            .create_protocol(
                ProtocolType::Rpc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        let config = protocol.get_configuration();
        assert_eq!(config["endpoint_url"], "http://custom.endpoint.com");
    }

    #[tokio::test]
    async fn test_factory_engine_specific_config() {
        let mut factory = DefaultProtocolFactory::new();

        let solana_rpc_config = serde_json::json!({
            "endpoint_url": "http://solana.testnet.com",
            "health_check_config": {
                "health_method": "getVersion"
            }
        });

        factory.set_engine_config(ProtocolType::Rpc, EngineType::Solana, solana_rpc_config);

        let protocol = factory
            .create_protocol(
                ProtocolType::Rpc,
                EngineType::Solana,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        let config = protocol.get_configuration();
        assert_eq!(config["endpoint_url"], "http://solana.testnet.com");
    }

    #[tokio::test]
    async fn test_protocol_config_builder() {
        let ipc_config = IpcProtocolConfig {
            transport_config: crate::communication::ipc_protocol::IpcTransportConfig {
                tcp_port: 9999,
                ..Default::default()
            },
            ..Default::default()
        };

        let rpc_config = RpcProtocolConfig {
            endpoint_url: "http://custom.rpc.com".to_string(),
            ..Default::default()
        };

        let jwt_config = JwtProtocolConfig {
            endpoint_url: "http://custom.jwt.com".to_string(),
            jwt_secret: "custom-secret-key-that-is-long-enough-for-security".to_string(),
            ..Default::default()
        };

        let factory = ProtocolConfigBuilder::new()
            .ipc(EngineType::Ethereum, ipc_config)
            .rpc(EngineType::Ethereum, rpc_config)
            .jwt(EngineType::Solana, jwt_config)
            .build_factory();

        // Test IPC protocol creation
        let ipc_protocol = factory
            .create_protocol(
                ProtocolType::Ipc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        let ipc_config = ipc_protocol.get_configuration();
        assert_eq!(ipc_config["transport_config"]["tcp_port"], 9999);

        // Test RPC protocol creation
        let rpc_protocol = factory
            .create_protocol(
                ProtocolType::Rpc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        let rpc_config = rpc_protocol.get_configuration();
        assert_eq!(rpc_config["endpoint_url"], "http://custom.rpc.com");

        // Test JWT protocol creation
        let jwt_protocol = factory
            .create_protocol(
                ProtocolType::Jwt,
                EngineType::Solana,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        let jwt_config = jwt_protocol.get_configuration();
        assert_eq!(jwt_config["endpoint_url"], "http://custom.jwt.com");
    }

    #[tokio::test]
    async fn test_factory_config_override_priority() {
        let mut factory = DefaultProtocolFactory::new();

        // Set default config
        factory.set_default_config(
            ProtocolType::Rpc,
            serde_json::json!({
                "endpoint_url": "http://default.com",
                "max_connections": 5
            }),
        );

        // Set engine-specific config
        factory.set_engine_config(
            ProtocolType::Rpc,
            EngineType::Ethereum,
            serde_json::json!({
                "endpoint_url": "http://ethereum.com"
                // max_connections not specified, should inherit from default
            }),
        );

        // Create with user config
        let user_config = serde_json::json!({
            "request_timeout": { "secs": 45, "nanos": 0 }
            // endpoint_url not specified, should use engine-specific
        });

        let protocol = factory
            .create_protocol(ProtocolType::Rpc, EngineType::Ethereum, user_config)
            .await
            .unwrap();

        let config = protocol.get_configuration();
        assert_eq!(config["endpoint_url"], "http://ethereum.com"); // From engine-specific
        assert_eq!(config["max_connections"], 5); // From default
        assert_eq!(config["request_timeout"]["secs"], 45); // From user config
    }

    #[tokio::test]
    async fn test_factory_invalid_config_validation() {
        let factory = DefaultProtocolFactory::new();

        let invalid_config = serde_json::json!({
            "endpoint_url": 12345, // Should be a string
            "connect_timeout": "not a duration"
        });

        let result = factory
            .create_protocol(ProtocolType::Rpc, EngineType::Ethereum, invalid_config)
            .await;

        assert!(result.is_err());
        match result.err().unwrap() {
            crate::MultivmError::Configuration { .. } => {}
            e => panic!("Expected Configuration error, got: {:?}", e),
        }
    }

    #[test]
    fn test_config_builder_get_config() {
        let ipc_config = IpcProtocolConfig::default();

        let builder = ProtocolConfigBuilder::new().ipc(EngineType::Ethereum, ipc_config);

        let config = builder.get_config(&ProtocolType::Ipc, &EngineType::Ethereum);
        assert!(config.is_some());

        let missing_config = builder.get_config(&ProtocolType::Rpc, &EngineType::Ethereum);
        assert!(missing_config.is_none());
    }

    #[tokio::test]
    async fn test_factory_ethereum_defaults() {
        let factory = DefaultProtocolFactory::new();

        // Test Ethereum IPC defaults
        let eth_ipc = factory
            .create_protocol(
                ProtocolType::Ipc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        let ipc_config = eth_ipc.get_configuration();
        assert_eq!(ipc_config["transport_config"]["tcp_port"], 8545);
        assert_eq!(
            ipc_config["transport_config"]["unix_socket_path"],
            "/tmp/multivm-ethereum.sock"
        );

        // Test Ethereum RPC defaults
        let eth_rpc = factory
            .create_protocol(
                ProtocolType::Rpc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        let rpc_config = eth_rpc.get_configuration();
        assert_eq!(rpc_config["endpoint_url"], "http://127.0.0.1:8545");
        assert_eq!(
            rpc_config["health_check_config"]["health_method"],
            "eth_chainId"
        );
    }

    #[tokio::test]
    async fn test_factory_solana_defaults() {
        let factory = DefaultProtocolFactory::new();

        // Test Solana IPC defaults
        let sol_ipc = factory
            .create_protocol(
                ProtocolType::Ipc,
                EngineType::Solana,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        let ipc_config = sol_ipc.get_configuration();
        assert_eq!(ipc_config["transport_config"]["tcp_port"], 8899);
        assert_eq!(
            ipc_config["transport_config"]["unix_socket_path"],
            "/tmp/multivm-solana.sock"
        );

        // Test Solana RPC defaults
        let sol_rpc = factory
            .create_protocol(
                ProtocolType::Rpc,
                EngineType::Solana,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        let rpc_config = sol_rpc.get_configuration();
        assert_eq!(rpc_config["endpoint_url"], "http://127.0.0.1:8899");
        assert_eq!(
            rpc_config["health_check_config"]["health_method"],
            "getVersion"
        );
    }
}
