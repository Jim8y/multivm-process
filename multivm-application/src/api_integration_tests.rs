//! Comprehensive integration tests for MultiVM Application API

#[cfg(test)]
mod tests {
    use crate::{
        api::{
            rest::{create_app, AppState},
            graphql::{create_schema, MultiVmSchema},
        },
        auth::{
            manager::AuthManager,
            jwt::JwtManager,
            api_key::ApiKeyManager,
        },
        cache::{
            strategy::CacheStrategy,
            memory::MemoryCache,
        },
        config::ApplicationConfig,
        gateway::unified::UnifiedGateway,
        monitoring::{
            health::HealthMonitor,
            metrics::MetricsCollector,
        },
    };
    use multivm_common::{
        config::MultivmConfig,
        types::{ProcessId, health::HealthStatus},
        IpcMessage, IpcCommand,
    };
    use axum::{
        body::Body,
        http::{Request, Method, StatusCode, header},
        response::Response,
    };
    use serde_json::{json, Value};
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::test;
    use tower::ServiceExt;
    use tracing_test::traced_test;

    async fn create_test_app_state() -> AppState {
        let temp_dir = TempDir::new().unwrap();
        
        let mut config = ApplicationConfig::default();
        config.data_dir = temp_dir.path().to_path_buf();
        config.auth.jwt_secret = "test_jwt_secret".to_string();
        config.cache.strategy = "memory".to_string();
        
        let auth_manager = AuthManager::new(config.auth.clone()).await.unwrap();
        let cache = Box::new(MemoryCache::new(1000, Duration::from_secs(300))) as Box<dyn CacheStrategy>;
        let gateway = UnifiedGateway::new(config.clone()).await.unwrap();
        let health_monitor = HealthMonitor::new(Duration::from_secs(5)).await.unwrap();
        let metrics_collector = MetricsCollector::new().await.unwrap();
        
        AppState {
            config,
            auth_manager,
            cache,
            gateway,
            health_monitor,
            metrics_collector,
        }
    }

    #[traced_test]
    #[test]
    async fn test_health_endpoint() {
        let state = create_test_app_state().await;
        let app = create_app(state).await.unwrap();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
        
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let health_response: Value = serde_json::from_slice(&body).unwrap();
        
        assert_eq!(health_response["status"], "healthy");
        assert!(health_response["timestamp"].is_string());
        assert!(health_response["components"].is_object());
    }

    #[traced_test]
    #[test]
    async fn test_system_status_endpoint() {
        let state = create_test_app_state().await;
        let app = create_app(state).await.unwrap();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/system/status")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
        
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let status_response: Value = serde_json::from_slice(&body).unwrap();
        
        assert!(status_response["uptime"].is_number());
        assert!(status_response["version"].is_string());
        assert!(status_response["processes"].is_object());
    }

    #[traced_test]
    #[test]
    async fn test_authentication_middleware() {
        let state = create_test_app_state().await;
        let app = create_app(state).await.unwrap();

        // Test without authentication
        let request = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/admin/config")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // Test with invalid token
        let request = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/admin/config")
            .header(header::AUTHORIZATION, "Bearer invalid_token")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[traced_test]
    #[test]
    async fn test_accounts_endpoints() {
        let state = create_test_app_state().await;
        let app = create_app(state).await.unwrap();

        // Test account creation
        let create_account_body = json!({
            "vm_type": "svm",
            "address": "11111111111111111111111111111111",
            "metadata": {
                "name": "Test Account",
                "type": "user"
            }
        });

        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/accounts")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(create_account_body.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        // Test account retrieval
        let request = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/accounts/11111111111111111111111111111111")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let account_response: Value = serde_json::from_slice(&body).unwrap();
        
        assert_eq!(account_response["address"], "11111111111111111111111111111111");
        assert_eq!(account_response["vm_type"], "svm");
    }

    #[traced_test]
    #[test]
    async fn test_transaction_endpoints() {
        let state = create_test_app_state().await;
        let app = create_app(state).await.unwrap();

        // Test transaction submission
        let submit_tx_body = json!({
            "vm_type": "svm",
            "transaction_data": "base64_encoded_transaction_data",
            "priority": "high",
            "metadata": {
                "source": "api_test"
            }
        });

        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/transactions")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(submit_tx_body.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let tx_response: Value = serde_json::from_slice(&body).unwrap();
        
        assert!(tx_response["transaction_id"].is_string());
        assert_eq!(tx_response["status"], "pending");
    }

    #[traced_test]
    #[test]
    async fn test_cross_vm_transaction_flow() {
        let state = create_test_app_state().await;
        let app = create_app(state).await.unwrap();

        // Test cross-VM binding transaction
        let binding_tx_body = json!({
            "type": "account_binding",
            "source_vm": "svm",
            "target_vm": "evm",
            "source_address": "11111111111111111111111111111111",
            "target_address": "0x1234567890123456789012345678901234567890",
            "asset_type": "native",
            "amount": "1000000",
            "proof": "binding_proof_data"
        });

        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/multivm/bind")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(binding_tx_body.to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let binding_response: Value = serde_json::from_slice(&body).unwrap();
        
        assert!(binding_response["binding_id"].is_string());
        assert_eq!(binding_response["status"], "pending");
    }

    #[traced_test]
    #[test]
    async fn test_block_endpoints() {
        let state = create_test_app_state().await;
        let app = create_app(state).await.unwrap();

        // Test latest block retrieval
        let request = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/blocks/latest")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Test block by height
        let request = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/blocks/1")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        
        // May return 404 if no block exists, which is acceptable
        assert!(response.status() == StatusCode::OK || response.status() == StatusCode::NOT_FOUND);
    }

    #[traced_test]
    #[test]
    async fn test_graphql_endpoint() {
        let state = create_test_app_state().await;
        let schema = create_schema(state).await.unwrap();

        // Test GraphQL health query
        let query = json!({
            "query": "query { health { status timestamp } }"
        });

        let request = Request::builder()
            .method(Method::POST)
            .uri("/graphql")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(query.to_string()))
            .unwrap();

        let response = schema.execute_request(request).await;
        assert!(response.is_ok());
    }

    #[traced_test]
    #[test]
    async fn test_websocket_notifications() {
        let state = create_test_app_state().await;
        let app = create_app(state).await.unwrap();

        // Test WebSocket upgrade
        let request = Request::builder()
            .method(Method::GET)
            .uri("/ws/notifications")
            .header(header::CONNECTION, "Upgrade")
            .header(header::UPGRADE, "websocket")
            .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
            .header("Sec-WebSocket-Version", "13")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);
    }

    #[traced_test]
    #[test]
    async fn test_metrics_endpoint() {
        let state = create_test_app_state().await;
        let app = create_app(state).await.unwrap();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/metrics")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let metrics_response: Value = serde_json::from_slice(&body).unwrap();
        
        assert!(metrics_response["system"].is_object());
        assert!(metrics_response["application"].is_object());
        assert!(metrics_response["timestamp"].is_string());
    }

    #[traced_test]
    #[test]
    async fn test_error_handling() {
        let state = create_test_app_state().await;
        let app = create_app(state).await.unwrap();

        // Test invalid endpoint
        let request = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/invalid/endpoint")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        // Test invalid JSON
        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/transactions")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("invalid json"))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[traced_test]
    #[test]
    async fn test_rate_limiting() {
        let state = create_test_app_state().await;
        let app = create_app(state).await.unwrap();

        // Send multiple requests rapidly
        let mut responses = vec![];
        for _ in 0..100 {
            let request = Request::builder()
                .method(Method::GET)
                .uri("/api/v1/health")
                .body(Body::empty())
                .unwrap();

            let response = app.clone().oneshot(request).await.unwrap();
            responses.push(response.status());
        }

        // At least one request should succeed
        assert!(responses.iter().any(|&status| status == StatusCode::OK));
        
        // Some requests might be rate limited (429 Too Many Requests)
        let rate_limited_count = responses.iter()
            .filter(|&&status| status == StatusCode::TOO_MANY_REQUESTS)
            .count();
        
        println!("Rate limited requests: {}/100", rate_limited_count);
    }

    #[traced_test]
    #[test]
    async fn test_concurrent_api_calls() {
        let state = create_test_app_state().await;
        let app = create_app(state).await.unwrap();

        // Make concurrent API calls
        let mut handles = vec![];
        for i in 0..50 {
            let app_clone = app.clone();
            let handle = tokio::spawn(async move {
                let request = Request::builder()
                    .method(Method::GET)
                    .uri(&format!("/api/v1/health?id={}", i))
                    .body(Body::empty())
                    .unwrap();

                app_clone.oneshot(request).await
            });
            handles.push(handle);
        }

        // Wait for all requests to complete
        let mut success_count = 0;
        let mut error_count = 0;

        for handle in handles {
            match handle.await {
                Ok(Ok(response)) => {
                    if response.status() == StatusCode::OK {
                        success_count += 1;
                    } else {
                        error_count += 1;
                    }
                }
                _ => error_count += 1,
            }
        }

        // Most requests should succeed
        assert!(success_count > 40, "Only {} out of 50 requests succeeded", success_count);
        println!("Concurrent requests: {} succeeded, {} failed", success_count, error_count);
    }

    #[traced_test]
    #[test]
    async fn test_data_persistence() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create first app instance
        {
            let mut config = ApplicationConfig::default();
            config.data_dir = temp_dir.path().to_path_buf();
            
            let state = create_test_app_state().await;
            let app = create_app(state).await.unwrap();

            // Create an account
            let create_account_body = json!({
                "vm_type": "svm",
                "address": "persistent_test_account",
                "metadata": {
                    "name": "Persistent Test Account"
                }
            });

            let request = Request::builder()
                .method(Method::POST)
                .uri("/api/v1/accounts")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(create_account_body.to_string()))
                .unwrap();

            let response = app.oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::CREATED);
        }

        // Create second app instance with same data directory
        {
            let state = create_test_app_state().await;
            let app = create_app(state).await.unwrap();

            // Try to retrieve the account
            let request = Request::builder()
                .method(Method::GET)
                .uri("/api/v1/accounts/persistent_test_account")
                .body(Body::empty())
                .unwrap();

            let response = app.oneshot(request).await.unwrap();
            
            // Should be able to retrieve the persisted account
            assert_eq!(response.status(), StatusCode::OK);
        }
    }

    #[traced_test]
    #[test]
    async fn test_api_versioning() {
        let state = create_test_app_state().await;
        let app = create_app(state).await.unwrap();

        // Test v1 API
        let request = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Test unsupported version
        let request = Request::builder()
            .method(Method::GET)
            .uri("/api/v2/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[traced_test]
    #[test]
    async fn test_cache_functionality() {
        let state = create_test_app_state().await;
        let app = create_app(state).await.unwrap();

        // Make same request multiple times to test caching
        let uri = "/api/v1/system/status";
        
        let start_time = std::time::Instant::now();
        
        // First request (cache miss)
        let request = Request::builder()
            .method(Method::GET)
            .uri(uri)
            .body(Body::empty())
            .unwrap();

        let response1 = app.clone().oneshot(request).await.unwrap();
        let first_request_time = start_time.elapsed();
        assert_eq!(response1.status(), StatusCode::OK);

        // Second request (cache hit)
        let request = Request::builder()
            .method(Method::GET)
            .uri(uri)
            .body(Body::empty())
            .unwrap();

        let response2 = app.oneshot(request).await.unwrap();
        let second_request_time = start_time.elapsed() - first_request_time;
        assert_eq!(response2.status(), StatusCode::OK);

        // Cache hit should be faster (though this is not guaranteed in tests)
        println!("First request: {:?}, Second request: {:?}", 
                 first_request_time, second_request_time);
    }

    #[traced_test]
    #[test]
    async fn test_comprehensive_system_flow() {
        let state = create_test_app_state().await;
        let app = create_app(state).await.unwrap();

        // 1. Check system health
        let request = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/health")
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // 2. Create accounts
        let svm_account = json!({
            "vm_type": "svm",
            "address": "comprehensive_test_svm",
            "metadata": {"type": "test"}
        });

        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/accounts")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(svm_account.to_string()))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        // 3. Submit transaction
        let transaction = json!({
            "vm_type": "svm",
            "transaction_data": "comprehensive_test_tx_data",
            "priority": "normal"
        });

        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/transactions")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(transaction.to_string()))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);

        // 4. Check metrics
        let request = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/metrics")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}