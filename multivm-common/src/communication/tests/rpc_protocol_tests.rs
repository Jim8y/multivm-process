//! RPC protocol unit tests

use crate::communication::{
    rpc_protocol::{HealthCheckConfig, RetryConfig, RpcProtocol, RpcProtocolConfig, TlsConfig},
    CommunicationProtocol, CommunicationRequest, EngineType, ProtocolType,
};
use std::time::Duration;
use uuid::Uuid;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_config(endpoint_url: String) -> RpcProtocolConfig {
        RpcProtocolConfig {
            endpoint_url,
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(5),
            max_connections: 10,
            retry_config: RetryConfig {
                max_retries: 2,
                base_delay: Duration::from_millis(10),
                max_delay: Duration::from_secs(1),
                backoff_multiplier: 2.0,
                jitter: false,
            },
            health_check_config: HealthCheckConfig {
                health_method: "eth_chainId".to_string(),
                interval: Duration::from_secs(30),
                timeout: Duration::from_secs(5),
                enable_auto_check: false,
            },
            default_headers: Default::default(),
            user_agent: "MultiVM-Test/1.0".to_string(),
            use_http2: true,
            tls_config: TlsConfig::default(),
        }
    }

    #[tokio::test]
    async fn test_rpc_protocol_creation() {
        let config = create_test_config("http://127.0.0.1:8545".to_string()).await;
        let protocol = RpcProtocol::new(EngineType::Ethereum, config)
            .await
            .unwrap();

        assert_eq!(protocol.protocol_type(), ProtocolType::Rpc);
        assert_eq!(protocol.engine_type(), EngineType::Ethereum);
        assert!(!protocol.is_connected());
    }

    #[tokio::test]
    async fn test_rpc_protocol_successful_request() {
        let mock_server = MockServer::start().await;
        let config = create_test_config(mock_server.uri()).await;

        // Mock successful response
        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("content-type", "application/json"))
            .and(body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "eth_chainId",
                "params": []
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": "0x1"
            })))
            .mount(&mock_server)
            .await;

        let mut protocol = RpcProtocol::new(EngineType::Ethereum, config)
            .await
            .unwrap();
        let result = protocol.connect().await;
        assert!(result.is_ok());
        assert!(protocol.is_connected());

        let request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: "eth_chainId".to_string(),
            params: serde_json::json!([]),
            timeout: Some(Duration::from_secs(5)),
            auth_context: None,
        };

        let response = protocol.send_request(request).await.unwrap();
        assert!(response.error.is_none());
        assert_eq!(response.result, Some(serde_json::json!("0x1")));

        let status = protocol.get_connection_status().await.unwrap();
        assert!(status.is_connected);
        assert!(status.success_count > 0);
        assert!(status.latency_ms.is_some());
    }

    #[tokio::test]
    async fn test_rpc_protocol_error_response() {
        let mock_server = MockServer::start().await;
        let config = create_test_config(mock_server.uri()).await;

        // Mock error response
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "error": {
                    "code": -32601,
                    "message": "Method not found",
                    "data": null
                }
            })))
            .mount(&mock_server)
            .await;

        let protocol = RpcProtocol::new(EngineType::Ethereum, config)
            .await
            .unwrap();

        let request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: "invalid_method".to_string(),
            params: serde_json::json!([]),
            timeout: Some(Duration::from_secs(5)),
            auth_context: None,
        };

        let response = protocol.send_request(request).await.unwrap();
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, -32601);
    }

    #[tokio::test]
    async fn test_rpc_protocol_network_error() {
        let config = create_test_config("http://127.0.0.1:12345".to_string()).await;
        let mut protocol = RpcProtocol::new(EngineType::Ethereum, config)
            .await
            .unwrap();

        let result = protocol.connect().await;
        assert!(result.is_err());
        assert!(!protocol.is_connected());
    }

    #[tokio::test]
    async fn test_rpc_protocol_retry_mechanism() {
        let mock_server = MockServer::start().await;
        let config = create_test_config(mock_server.uri()).await;

        // First two attempts fail, third succeeds
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(500))
            .up_to_n_times(2)
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": "success"
            })))
            .mount(&mock_server)
            .await;

        let protocol = RpcProtocol::new(EngineType::Ethereum, config)
            .await
            .unwrap();

        let request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: "test_method".to_string(),
            params: serde_json::json!([]),
            timeout: Some(Duration::from_secs(5)),
            auth_context: None,
        };

        let response = protocol.send_request(request).await.unwrap();
        assert_eq!(response.result, Some(serde_json::json!("success")));
    }

    #[tokio::test]
    async fn test_rpc_protocol_rate_limiting() {
        let mock_server = MockServer::start().await;
        let config = create_test_config(mock_server.uri()).await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(429).set_body_string("Rate limit exceeded"))
            .mount(&mock_server)
            .await;

        let protocol = RpcProtocol::new(EngineType::Ethereum, config)
            .await
            .unwrap();

        let request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: "test_method".to_string(),
            params: serde_json::json!([]),
            timeout: Some(Duration::from_secs(5)),
            auth_context: None,
        };

        let result = protocol.send_request(request).await;
        assert!(result.is_err());
        match result.err().unwrap() {
            crate::MultivmError::RateLimited { .. } => {}
            e => panic!("Expected RateLimited error, got: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_rpc_protocol_custom_headers() {
        let mock_server = MockServer::start().await;
        let mut config = create_test_config(mock_server.uri()).await;
        config
            .default_headers
            .insert("X-API-Key".to_string(), "test-key".to_string());

        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("X-API-Key", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": "ok"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let protocol = RpcProtocol::new(EngineType::Ethereum, config)
            .await
            .unwrap();

        let request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: "test_method".to_string(),
            params: serde_json::json!([]),
            timeout: Some(Duration::from_secs(5)),
            auth_context: None,
        };

        let response = protocol.send_request(request).await.unwrap();
        assert_eq!(response.result, Some(serde_json::json!("ok")));
    }

    #[tokio::test]
    async fn test_rpc_protocol_notification() {
        let mock_server = MockServer::start().await;
        let config = create_test_config(mock_server.uri()).await;

        Mock::given(method("POST"))
            .and(path("/"))
            .and(body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": null,
                "method": "notify_event",
                "params": {"event": "test"}
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": null,
                "result": null
            })))
            .mount(&mock_server)
            .await;

        let protocol = RpcProtocol::new(EngineType::Ethereum, config)
            .await
            .unwrap();

        let request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: "notify_event".to_string(),
            params: serde_json::json!({"event": "test"}),
            timeout: Some(Duration::from_secs(5)),
            auth_context: None,
        };

        let result = protocol.send_notification(request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_rpc_protocol_health_check() {
        let mock_server = MockServer::start().await;
        let config = create_test_config(mock_server.uri()).await;

        Mock::given(method("POST"))
            .and(path("/"))
            .and(body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "eth_chainId",
                "params": []
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": "0x1"
            })))
            .mount(&mock_server)
            .await;

        let protocol = RpcProtocol::new(EngineType::Ethereum, config)
            .await
            .unwrap();

        let health = protocol.health_check().await.unwrap();
        assert!(health.is_healthy);
        assert!(health.capabilities.contains(&"json-rpc".to_string()));
    }

    #[tokio::test]
    async fn test_rpc_protocol_subscribe_unsubscribe() {
        let mock_server = MockServer::start().await;
        let config = create_test_config(mock_server.uri()).await;

        Mock::given(method("POST"))
            .and(path("/"))
            .and(body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "eth_subscribe",
                "params": ["newHeads", "logs"]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": "0x1234"
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/"))
            .and(body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "eth_unsubscribe",
                "params": ["newHeads", "logs"]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": true
            })))
            .mount(&mock_server)
            .await;

        let protocol = RpcProtocol::new(EngineType::Ethereum, config)
            .await
            .unwrap();

        let event_types = vec!["newHeads".to_string(), "logs".to_string()];

        let subscribe_result = protocol.subscribe(event_types.clone()).await;
        assert!(subscribe_result.is_ok());

        let unsubscribe_result = protocol.unsubscribe(event_types).await;
        assert!(unsubscribe_result.is_ok());
    }

    #[tokio::test]
    async fn test_rpc_protocol_solana_methods() {
        let mock_server = MockServer::start().await;
        let mut config = create_test_config(mock_server.uri()).await;
        config.health_check_config.health_method = "getVersion".to_string();

        Mock::given(method("POST"))
            .and(path("/"))
            .and(body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "getVersion",
                "params": []
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "solana-core": "1.14.0"
                }
            })))
            .mount(&mock_server)
            .await;

        let protocol = RpcProtocol::new(EngineType::Solana, config).await.unwrap();

        let health = protocol.health_check().await.unwrap();
        assert!(health.is_healthy);
        assert_eq!(health.version, Some("1.14.0".to_string()));
    }

    #[tokio::test]
    async fn test_rpc_protocol_configuration_update() {
        let mock_server = MockServer::start().await;
        let config = create_test_config(mock_server.uri()).await;
        let mut protocol = RpcProtocol::new(EngineType::Ethereum, config)
            .await
            .unwrap();

        let new_config = serde_json::json!({
            "endpoint_url": "http://new-endpoint.com",
            "connect_timeout": { "secs": 10, "nanos": 0 },
            "request_timeout": { "secs": 20, "nanos": 0 },
            "max_connections": 20,
            "retry_config": {
                "max_retries": 5,
                "base_delay": { "secs": 0, "nanos": 100000000 },
                "max_delay": { "secs": 10, "nanos": 0 },
                "backoff_multiplier": 3.0,
                "jitter": true
            },
            "health_check_config": {
                "health_method": "eth_blockNumber",
                "interval": { "secs": 60, "nanos": 0 },
                "timeout": { "secs": 10, "nanos": 0 },
                "enable_auto_check": false
            },
            "default_headers": {
                "X-Custom": "value"
            },
            "user_agent": "Updated-Agent/2.0",
            "use_http2": false,
            "tls_config": {
                "accept_invalid_certs": false,
                "accept_invalid_hostnames": false,
                "ca_cert_path": null,
                "client_cert_path": null,
                "client_key_path": null
            }
        });

        let result = protocol.update_configuration(new_config).await;
        assert!(result.is_ok());

        let updated_config = protocol.get_configuration();
        assert_eq!(updated_config["endpoint_url"], "http://new-endpoint.com");
    }
}
