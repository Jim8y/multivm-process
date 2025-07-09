//! JWT protocol unit tests

use crate::communication::{
    jwt_protocol::{
        HealthCheckConfig, HttpClientConfig, JwtProtocol, JwtProtocolConfig, RetryConfig,
    },
    AuthContext, CommunicationProtocol, CommunicationRequest, EngineType, ProtocolType,
};
use std::time::Duration;
use uuid::Uuid;
use wiremock::matchers::{body_json, header, header_exists, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_config(endpoint_url: String) -> JwtProtocolConfig {
        JwtProtocolConfig {
            endpoint_url,
            jwt_secret: "test-secret-key-that-is-long-enough-for-security-requirements".to_string(),
            token_expiry: Duration::from_secs(3600),
            refresh_threshold: Duration::from_secs(300),
            http_config: HttpClientConfig {
                connect_timeout: Duration::from_secs(5),
                request_timeout: Duration::from_secs(5),
                user_agent: "MultiVM-Test/1.0".to_string(),
                use_http2: true,
                accept_invalid_certs: false,
                default_headers: Default::default(),
            },
            retry_config: RetryConfig {
                max_retries: 2,
                base_delay: Duration::from_millis(10),
                max_delay: Duration::from_secs(1),
                backoff_multiplier: 2.0,
                retry_auth_failures: true,
            },
            health_check_config: HealthCheckConfig {
                health_endpoint: "/health".to_string(),
                interval: Duration::from_secs(30),
                timeout: Duration::from_secs(5),
                enable_auto_check: false,
            },
            api_key: None,
            auth_headers: Default::default(),
        }
    }

    #[tokio::test]
    async fn test_jwt_protocol_creation() {
        let config = create_test_config("http://127.0.0.1:8080".to_string()).await;
        let protocol = JwtProtocol::new(EngineType::Ethereum, config)
            .await
            .unwrap();

        assert_eq!(protocol.protocol_type(), ProtocolType::Jwt);
        assert_eq!(protocol.engine_type(), EngineType::Ethereum);
        assert!(!protocol.is_connected());
    }

    #[tokio::test]
    async fn test_jwt_protocol_short_secret_validation() {
        let mut config = create_test_config("http://127.0.0.1:8080".to_string()).await;
        config.jwt_secret = "short".to_string();

        let result = JwtProtocol::new(EngineType::Ethereum, config).await;
        assert!(result.is_err());
        match result.err().unwrap() {
            crate::MultivmError::Configuration { message, .. } => {
                assert!(message.contains("32 characters"));
            }
            e => panic!("Expected Configuration error, got: {e:?}"),
        }
    }

    #[tokio::test]
    async fn test_jwt_token_generation() {
        let mock_server = MockServer::start().await;
        let config = create_test_config(mock_server.uri()).await;

        Mock::given(method("GET"))
            .and(path("/health"))
            .and(header_exists("authorization"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"status": "ok"})),
            )
            .mount(&mock_server)
            .await;

        let mut protocol = JwtProtocol::new(EngineType::Ethereum, config)
            .await
            .unwrap();
        let result = protocol.connect().await;
        assert!(result.is_ok());
        assert!(protocol.is_connected());
    }

    #[tokio::test]
    async fn test_jwt_protocol_successful_request() {
        let mock_server = MockServer::start().await;
        let config = create_test_config(format!("{}/api", mock_server.uri())).await;

        Mock::given(method("GET"))
            .and(path("/api/health"))
            .and(header_exists("authorization"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"status": "ok"})),
            )
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/api/test_method"))
            .and(header_exists("authorization"))
            .and(body_json(serde_json::json!({"param": "value"})))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": "success"})),
            )
            .mount(&mock_server)
            .await;

        let mut protocol = JwtProtocol::new(EngineType::Ethereum, config)
            .await
            .unwrap();
        protocol.connect().await.unwrap();

        let request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: "test_method".to_string(),
            params: serde_json::json!({"param": "value"}),
            timeout: Some(Duration::from_secs(5)),
            auth_context: None,
        };

        let response = protocol.send_request(request).await.unwrap();
        assert!(response.error.is_none());
        assert_eq!(
            response.result,
            Some(serde_json::json!({"result": "success"}))
        );
    }

    #[tokio::test]
    async fn test_jwt_protocol_auth_failure() {
        let mock_server = MockServer::start().await;
        let config = create_test_config(mock_server.uri()).await;

        Mock::given(method("POST"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
            .expect(3) // Initial + 2 retries
            .mount(&mock_server)
            .await;

        let protocol = JwtProtocol::new(EngineType::Ethereum, config)
            .await
            .unwrap();

        let request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: "/test".to_string(),
            params: serde_json::Value::Null,
            timeout: Some(Duration::from_secs(5)),
            auth_context: None,
        };

        let result = protocol.send_request(request).await;

        // JWT auth failures are returned as successful responses with error details
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.error.is_some());
        assert!(response
            .error
            .unwrap()
            .message
            .contains("Authentication failed"));
    }

    #[tokio::test]
    async fn test_jwt_protocol_with_api_key() {
        let mock_server = MockServer::start().await;
        let mut config = create_test_config(mock_server.uri()).await;
        config.jwt_secret = "".to_string(); // Disable JWT
        config.api_key = Some("test-api-key".to_string());

        Mock::given(method("POST"))
            .and(path("/test"))
            .and(header("x-api-key", "test-api-key"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": "ok"})),
            )
            .mount(&mock_server)
            .await;

        let protocol = JwtProtocol::new(EngineType::Ethereum, config)
            .await
            .unwrap();

        let request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: "/test".to_string(),
            params: serde_json::Value::Null,
            timeout: Some(Duration::from_secs(5)),
            auth_context: None,
        };

        let response = protocol.send_request(request).await.unwrap();
        assert_eq!(response.result, Some(serde_json::json!({"result": "ok"})));
    }

    #[tokio::test]
    async fn test_jwt_protocol_with_custom_auth_context() {
        let mock_server = MockServer::start().await;
        let config = create_test_config(mock_server.uri()).await;

        Mock::given(method("POST"))
            .and(path("/test"))
            .and(header("authorization", "Bearer custom-token"))
            .and(header("x-custom-header", "custom-value"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": "ok"})),
            )
            .mount(&mock_server)
            .await;

        let protocol = JwtProtocol::new(EngineType::Ethereum, config)
            .await
            .unwrap();

        let mut headers = std::collections::HashMap::new();
        headers.insert("x-custom-header".to_string(), "custom-value".to_string());

        let request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: "/test".to_string(),
            params: serde_json::Value::Null,
            timeout: Some(Duration::from_secs(5)),
            auth_context: Some(AuthContext {
                token: Some("custom-token".to_string()),
                api_key: None,
                headers,
            }),
        };

        let response = protocol.send_request(request).await.unwrap();
        assert_eq!(response.result, Some(serde_json::json!({"result": "ok"})));
    }

    #[tokio::test]
    async fn test_jwt_protocol_notification() {
        let mock_server = MockServer::start().await;
        let config = create_test_config(mock_server.uri()).await;

        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/notify"))
            .and(header_exists("authorization"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&mock_server)
            .await;

        let mut protocol = JwtProtocol::new(EngineType::Ethereum, config)
            .await
            .unwrap();
        protocol.connect().await.unwrap();

        let request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: "/notify".to_string(),
            params: serde_json::json!({"event": "test"}),
            timeout: Some(Duration::from_secs(5)),
            auth_context: None,
        };

        let result = protocol.send_notification(request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_jwt_protocol_health_check() {
        let mock_server = MockServer::start().await;
        let config = create_test_config(mock_server.uri()).await;

        Mock::given(method("POST"))
            .and(path("/health"))
            .and(header_exists("authorization"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "healthy",
                "version": "1.0.0"
            })))
            .mount(&mock_server)
            .await;

        let protocol = JwtProtocol::new(EngineType::Ethereum, config)
            .await
            .unwrap();

        let health = protocol.health_check().await.unwrap();
        assert!(health.is_healthy);
        assert!(health.capabilities.contains(&"jwt".to_string()));
        assert!(health.capabilities.contains(&"authenticated".to_string()));
    }

    #[tokio::test]
    async fn test_jwt_protocol_subscribe_unsubscribe() {
        let mock_server = MockServer::start().await;
        let config = create_test_config(mock_server.uri()).await;

        Mock::given(method("POST"))
            .and(path("/subscribe"))
            .and(header_exists("authorization"))
            .and(body_json(serde_json::json!({
                "event_types": ["blocks", "transactions"]
            })))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"subscribed": true})),
            )
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/unsubscribe"))
            .and(header_exists("authorization"))
            .and(body_json(serde_json::json!({
                "event_types": ["blocks", "transactions"]
            })))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"unsubscribed": true})),
            )
            .mount(&mock_server)
            .await;

        let protocol = JwtProtocol::new(EngineType::Ethereum, config)
            .await
            .unwrap();

        let event_types = vec!["blocks".to_string(), "transactions".to_string()];

        let subscribe_result = protocol.subscribe(event_types.clone()).await;
        assert!(subscribe_result.is_ok());

        let unsubscribe_result = protocol.unsubscribe(event_types).await;
        assert!(unsubscribe_result.is_ok());
    }

    #[tokio::test]
    async fn test_jwt_protocol_configuration_update() {
        let mock_server = MockServer::start().await;
        let config = create_test_config(mock_server.uri()).await;
        let mut protocol = JwtProtocol::new(EngineType::Ethereum, config)
            .await
            .unwrap();

        let new_config = serde_json::json!({
            "endpoint_url": "http://new-endpoint.com",
            "jwt_secret": "new-secret-key-that-is-long-enough-for-security-requirements",
            "token_expiry": { "secs": 7200, "nanos": 0 },
            "refresh_threshold": { "secs": 600, "nanos": 0 },
            "http_config": {
                "connect_timeout": { "secs": 10, "nanos": 0 },
                "request_timeout": { "secs": 20, "nanos": 0 },
                "user_agent": "Updated-Agent/2.0",
                "use_http2": false,
                "accept_invalid_certs": false,
                "default_headers": {
                    "X-Custom": "value"
                }
            },
            "retry_config": {
                "max_retries": 5,
                "base_delay": { "secs": 0, "nanos": 100000000 },
                "max_delay": { "secs": 10, "nanos": 0 },
                "backoff_multiplier": 3.0,
                "retry_auth_failures": false
            },
            "health_check_config": {
                "health_endpoint": "/status",
                "interval": { "secs": 60, "nanos": 0 },
                "timeout": { "secs": 10, "nanos": 0 },
                "enable_auto_check": false
            },
            "api_key": "backup-key",
            "auth_headers": {
                "X-Auth": "value"
            }
        });

        let result = protocol.update_configuration(new_config).await;
        assert!(result.is_ok());

        let updated_config = protocol.get_configuration();
        assert_eq!(updated_config["endpoint_url"], "http://new-endpoint.com");
        assert!(updated_config.get("jwt_secret").is_none()); // Should be hidden
    }

    #[tokio::test]
    async fn test_jwt_protocol_retry_with_token_refresh() {
        let mock_server = MockServer::start().await;
        let config = create_test_config(mock_server.uri()).await;

        // First attempt returns 401, second succeeds
        Mock::given(method("POST"))
            .and(path("/test"))
            .respond_with(ResponseTemplate::new(401))
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/test"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": "success"})),
            )
            .mount(&mock_server)
            .await;

        let protocol = JwtProtocol::new(EngineType::Ethereum, config)
            .await
            .unwrap();

        let request = CommunicationRequest {
            id: Uuid::new_v4().to_string(),
            method: "/test".to_string(),
            params: serde_json::Value::Null,
            timeout: Some(Duration::from_secs(5)),
            auth_context: None,
        };

        let response = protocol.send_request(request).await.unwrap();
        assert_eq!(
            response.result,
            Some(serde_json::json!({"result": "success"}))
        );
    }

    #[tokio::test]
    async fn test_jwt_protocol_network_error() {
        let config = create_test_config("http://127.0.0.1:12345".to_string()).await;
        let mut protocol = JwtProtocol::new(EngineType::Ethereum, config)
            .await
            .unwrap();

        let result = protocol.connect().await;
        assert!(result.is_err());
        match result.err().unwrap() {
            crate::MultivmError::Network { .. } => {}
            e => panic!("Expected Network error, got: {e:?}"),
        }
    }
}
