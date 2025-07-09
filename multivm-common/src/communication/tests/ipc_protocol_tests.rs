//! IPC protocol unit tests

use crate::communication::{
    ipc_protocol::{IpcProtocol, IpcProtocolConfig, IpcTransportConfig},
    CommunicationProtocol, CommunicationRequest, EngineType, ProtocolType,
};
use crate::types::ProcessId;
use std::time::Duration;
use uuid::Uuid;

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config(tcp_port: u16) -> IpcProtocolConfig {
        IpcProtocolConfig {
            target_process_id: ProcessId::Ethereum,
            source_process_id: ProcessId::Main,
            transport_config: IpcTransportConfig {
                use_unix_sockets: false, // Use TCP for tests
                tcp_host: "127.0.0.1".to_string(),
                tcp_port,
                unix_socket_path: None,
                connect_timeout: Duration::from_secs(5),
                read_timeout: Duration::from_secs(5),
                write_timeout: Duration::from_secs(5),
            },
            default_timeout: Duration::from_secs(10),
            retry_config: Default::default(),
            health_check_config: Default::default(),
        }
    }

    #[tokio::test]
    async fn test_ipc_protocol_creation() {
        let config = create_test_config(0);
        let protocol = IpcProtocol::new(EngineType::Ethereum, config);

        assert_eq!(protocol.protocol_type(), ProtocolType::Ipc);
        assert_eq!(protocol.engine_type(), EngineType::Ethereum);
        assert!(!protocol.is_connected());
    }

    #[tokio::test]
    async fn test_ipc_protocol_tcp_connection_failure() {
        let config = create_test_config(12345); // Port unlikely to be in use
        let mut protocol = IpcProtocol::new(EngineType::Ethereum, config);

        let result = protocol.connect().await;
        assert!(result.is_err());
        assert!(!protocol.is_connected());
    }

    #[tokio::test]
    async fn test_ipc_protocol_configuration() {
        let config = create_test_config(0);
        let protocol = IpcProtocol::new(EngineType::Ethereum, config);

        let config_value = protocol.get_configuration();
        assert!(config_value.is_object());
        assert_eq!(
            config_value["target_process_id"],
            serde_json::json!("Ethereum")
        );
    }

    #[tokio::test]
    async fn test_ipc_protocol_update_configuration() {
        let config = create_test_config(0);
        let mut protocol = IpcProtocol::new(EngineType::Ethereum, config);

        let new_config = serde_json::json!({
            "target_process_id": "Solana",
            "source_process_id": "Main",
            "transport_config": {
                "use_unix_sockets": false,
                "tcp_host": "127.0.0.1",
                "tcp_port": 9999,
                "unix_socket_path": null,
                "connect_timeout": { "secs": 10, "nanos": 0 },
                "read_timeout": { "secs": 10, "nanos": 0 },
                "write_timeout": { "secs": 10, "nanos": 0 }
            },
            "default_timeout": { "secs": 20, "nanos": 0 },
            "retry_config": {
                "max_retries": 5,
                "base_delay": { "secs": 0, "nanos": 200000000 },
                "max_delay": { "secs": 10, "nanos": 0 },
                "backoff_multiplier": 2.0
            },
            "health_check_config": {
                "interval": { "secs": 60, "nanos": 0 },
                "timeout": { "secs": 10, "nanos": 0 },
                "enable_auto_check": false
            }
        });

        let result = protocol.update_configuration(new_config).await;
        assert!(result.is_ok());

        let updated_config = protocol.get_configuration();
        assert_eq!(updated_config["transport_config"]["tcp_port"], 9999);
    }

    #[tokio::test]
    async fn test_ipc_request_without_connection() {
        let config = create_test_config(0);
        let protocol = IpcProtocol::new(EngineType::Ethereum, config);

        let request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: "get_health".to_string(),
            params: serde_json::Value::Null,
            timeout: Some(Duration::from_secs(5)),
            auth_context: None,
        };

        let result = protocol.send_request(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_ipc_health_check_without_connection() {
        let config = create_test_config(0);
        let protocol = IpcProtocol::new(EngineType::Ethereum, config);

        let result = protocol.health_check().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_ipc_connection_status() {
        let config = create_test_config(0);
        let protocol = IpcProtocol::new(EngineType::Ethereum, config);

        let status = protocol.get_connection_status().await.unwrap();
        assert!(!status.is_connected);
        assert_eq!(status.success_count, 0);
        assert_eq!(status.error_count, 0);
        assert!(status.last_success.is_none());
        assert!(status.latency_ms.is_none());
    }

    #[tokio::test]
    async fn test_ipc_subscribe_unsubscribe() {
        let config = create_test_config(0);
        let protocol = IpcProtocol::new(EngineType::Ethereum, config);

        let event_types = vec!["block".to_string(), "transaction".to_string()];

        // These should fail without connection
        let subscribe_result = protocol.subscribe(event_types.clone()).await;
        assert!(subscribe_result.is_err());

        let unsubscribe_result = protocol.unsubscribe(event_types).await;
        assert!(unsubscribe_result.is_err());
    }

    #[tokio::test]
    async fn test_ipc_disconnect_without_connection() {
        let config = create_test_config(0);
        let mut protocol = IpcProtocol::new(EngineType::Ethereum, config);

        // Should not fail even without connection
        let result = protocol.disconnect().await;
        assert!(result.is_ok());
        assert!(!protocol.is_connected());
    }

    #[tokio::test]
    async fn test_ipc_send_notification() {
        let config = create_test_config(0);
        let protocol = IpcProtocol::new(EngineType::Ethereum, config);

        let request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: "notify_event".to_string(),
            params: serde_json::json!({"event": "test"}),
            timeout: Some(Duration::from_secs(5)),
            auth_context: None,
        };

        let result = protocol.send_notification(request).await;
        assert!(result.is_err()); // Should fail without connection
    }

    #[tokio::test]
    async fn test_ipc_request_methods() {
        let config = create_test_config(0);
        let _protocol = IpcProtocol::new(EngineType::Ethereum, config);

        // Test various request types that the protocol should handle
        let test_requests = vec![
            CommunicationRequest {
                id: Uuid::new_v4().to_string(),
                method: "get_health".to_string(),
                params: serde_json::Value::Null,
                timeout: None,
                auth_context: None,
            },
            CommunicationRequest {
                id: Uuid::new_v4().to_string(),
                method: "get_state".to_string(),
                params: serde_json::Value::Null,
                timeout: Some(Duration::from_secs(10)),
                auth_context: None,
            },
            CommunicationRequest {
                id: Uuid::new_v4().to_string(),
                method: "process_block".to_string(),
                params: serde_json::json!({
                    "block_data": "test_data",
                    "blockchain_type": "ethereum",
                    "expect_response": true
                }),
                timeout: None,
                auth_context: None,
            },
            CommunicationRequest {
                id: Uuid::new_v4().to_string(),
                method: "rpc_call".to_string(),
                params: serde_json::json!({
                    "method": "eth_blockNumber",
                    "params": []
                }),
                timeout: None,
                auth_context: None,
            },
            CommunicationRequest {
                id: Uuid::new_v4().to_string(),
                method: "shutdown".to_string(),
                params: serde_json::json!({
                    "graceful": true,
                    "timeout_seconds": 30
                }),
                timeout: None,
                auth_context: None,
            },
        ];

        // Just verify the requests are well-formed
        for request in test_requests {
            assert!(!request.id.is_empty());
            assert!(!request.method.is_empty());
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_ipc_unix_socket_config() {
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        let config = IpcProtocolConfig {
            target_process_id: ProcessId::Ethereum,
            source_process_id: ProcessId::Main,
            transport_config: IpcTransportConfig {
                use_unix_sockets: true,
                tcp_host: "127.0.0.1".to_string(),
                tcp_port: 0,
                unix_socket_path: Some(socket_path.to_str().unwrap().to_string()),
                connect_timeout: Duration::from_secs(5),
                read_timeout: Duration::from_secs(5),
                write_timeout: Duration::from_secs(5),
            },
            default_timeout: Duration::from_secs(10),
            retry_config: Default::default(),
            health_check_config: Default::default(),
        };

        let mut protocol = IpcProtocol::new(EngineType::Ethereum, config);

        // Should fail to connect (no server)
        let result = protocol.connect().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_ipc_retry_mechanism() {
        let mut config = create_test_config(0);
        config.retry_config.max_retries = 2;
        config.retry_config.base_delay = Duration::from_millis(10);

        let protocol = IpcProtocol::new(EngineType::Ethereum, config);

        let request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: "get_health".to_string(),
            params: serde_json::Value::Null,
            timeout: Some(Duration::from_secs(1)),
            auth_context: None,
        };

        let start = std::time::Instant::now();
        let result = protocol.send_request(request).await;
        let elapsed = start.elapsed();

        assert!(result.is_err());
        // Should have retried with delays
        assert!(elapsed >= Duration::from_millis(20)); // At least 2 retry delays
    }
}
