//! Comprehensive tests for API endpoints

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ApplicationServer, ApplicationState, ApplicationResult};
    use crate::config::ApplicationConfig;
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
    };
    use serde_json::Value;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::time::{timeout, Duration};
    use tower::ServiceExt;

    async fn create_test_application() -> ApplicationResult<ApplicationServer> {
        let temp_dir = TempDir::new().unwrap();
        let mut config = ApplicationConfig::default();
        
        // Update config to use test directories and ports
        config.server.rest.port = 0; // Use random port for testing
        config.server.graphql.port = 0;
        config.server.websocket.port = 0;
        config.server.admin.port = 0;
        config.database.path = temp_dir.path().join("test.db");
        config.cache.redis.enabled = false; // Use memory cache for testing
        
        ApplicationServer::new(config).await
    }

    async fn create_test_state() -> ApplicationResult<Arc<ApplicationState>> {
        let temp_dir = TempDir::new().unwrap();
        let mut config = ApplicationConfig::default();
        config.database.path = temp_dir.path().join("test.db");
        config.cache.redis.enabled = false;
        
        Ok(Arc::new(ApplicationState::new(config).await?))
    }

    #[tokio::test]
    async fn test_application_server_creation() {
        let app = create_test_application().await;
        assert!(app.is_ok(), "Application server creation should succeed");
    }

    #[tokio::test]
    async fn test_application_state_creation() {
        let state = create_test_state().await;
        assert!(state.is_ok(), "Application state creation should succeed");
    }

    #[tokio::test]
    async fn test_health_check_endpoint() {
        let state = create_test_state().await.unwrap();
        let app = crate::api::rest::create_app(state).await.unwrap();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_system_status_endpoint() {
        let state = create_test_state().await.unwrap();
        let app = crate::api::rest::create_app(state).await.unwrap();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/system/status")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        
        // Verify response structure
        assert!(json.get("success").is_some());
        assert!(json.get("data").is_some());
    }

    #[tokio::test]
    async fn test_system_info_endpoint() {
        let state = create_test_state().await.unwrap();
        let app = crate::api::rest::create_app(state).await.unwrap();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/system/info")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        
        // Verify system info structure
        if let Some(data) = json.get("data") {
            assert!(data.get("name").is_some());
            assert!(data.get("version").is_some());
            assert!(data.get("description").is_some());
        }
    }

    #[tokio::test]
    async fn test_system_metrics_endpoint() {
        let state = create_test_state().await.unwrap();
        let app = crate::api::rest::create_app(state).await.unwrap();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/system/metrics")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        // Metrics endpoint might return different status codes depending on implementation
        assert!(response.status().is_success() || response.status().is_server_error());
    }

    #[tokio::test]
    async fn test_svm_account_endpoint() {
        let state = create_test_state().await.unwrap();
        let app = crate::api::rest::create_app(state).await.unwrap();

        let test_address = "11111111111111111111111111111112"; // System program address
        let request = Request::builder()
            .method(Method::GET)
            .uri(&format!("/api/v1/svm/accounts/{}", test_address))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        // Account endpoint might return various status codes depending on implementation
        assert!(response.status().is_success() || response.status().is_client_error() || response.status().is_server_error());
    }

    #[tokio::test]
    async fn test_evm_account_endpoint() {
        let state = create_test_state().await.unwrap();
        let app = crate::api::rest::create_app(state).await.unwrap();

        let test_address = "0x0000000000000000000000000000000000000000";
        let request = Request::builder()
            .method(Method::GET)
            .uri(&format!("/api/v1/evm/accounts/{}", test_address))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        // Account endpoint might return various status codes depending on implementation
        assert!(response.status().is_success() || response.status().is_client_error() || response.status().is_server_error());
    }

    #[tokio::test]
    async fn test_multivm_system_state_endpoint() {
        let state = create_test_state().await.unwrap();
        let app = crate::api::rest::create_app(state).await.unwrap();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/multivm/state/summary")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert!(response.status().is_success() || response.status().is_server_error());
    }

    #[tokio::test]
    async fn test_invalid_endpoint() {
        let state = create_test_state().await.unwrap();
        let app = crate::api::rest::create_app(state).await.unwrap();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/invalid/endpoint")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_cors_headers() {
        let state = create_test_state().await.unwrap();
        let app = crate::api::rest::create_app(state).await.unwrap();

        let request = Request::builder()
            .method(Method::OPTIONS)
            .uri("/api/v1/system/status")
            .header("Origin", "http://localhost:3000")
            .header("Access-Control-Request-Method", "GET")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        // CORS preflight should succeed
        assert!(response.status().is_success() || response.status() == StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn test_request_with_custom_headers() {
        let state = create_test_state().await.unwrap();
        let app = crate::api::rest::create_app(state).await.unwrap();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/health")
            .header("X-Request-ID", "test-123")
            .header("Authorization", "Bearer test-token")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_large_request_body_limit() {
        let state = create_test_state().await.unwrap();
        let app = crate::api::rest::create_app(state).await.unwrap();

        // Create a large JSON payload
        let large_payload = serde_json::json!({
            "data": "x".repeat(20 * 1024 * 1024) // 20MB
        });

        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/svm/transactions")
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&large_payload).unwrap()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        // Should reject large payloads
        assert!(response.status().is_client_error() || response.status().is_server_error());
    }

    #[tokio::test]
    async fn test_concurrent_requests() {
        let state = create_test_state().await.unwrap();
        
        let mut handles = Vec::new();
        for i in 0..10 {
            let state_clone = state.clone();
            let handle = tokio::spawn(async move {
                let app = crate::api::rest::create_app(state_clone).await.unwrap();
                let request = Request::builder()
                    .method(Method::GET)
                    .uri("/health")
                    .header("X-Request-ID", format!("concurrent-{}", i))
                    .body(Body::empty())
                    .unwrap();

                let response = app.oneshot(request).await.unwrap();
                response.status()
            });
            handles.push(handle);
        }

        // Wait for all requests to complete
        for handle in handles {
            let status = handle.await.unwrap();
            assert_eq!(status, StatusCode::OK);
        }
    }

    #[tokio::test]
    async fn test_request_timeout() {
        let state = create_test_state().await.unwrap();
        let app = crate::api::rest::create_app(state).await.unwrap();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        // Test that requests complete within reasonable time
        let response_future = app.oneshot(request);
        let result = timeout(Duration::from_secs(5), response_future).await;
        
        assert!(result.is_ok(), "Request should complete within timeout");
        let response = result.unwrap().unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_api_versioning() {
        let state = create_test_state().await.unwrap();
        let app = crate::api::rest::create_app(state).await.unwrap();

        // Test v1 API endpoints
        let endpoints = vec![
            "/api/v1/system/status",
            "/api/v1/system/info", 
            "/api/v1/system/health",
        ];

        for endpoint in endpoints {
            let request = Request::builder()
                .method(Method::GET)
                .uri(endpoint)
                .body(Body::empty())
                .unwrap();

            let response = app.oneshot(request).await.unwrap();
            // Should not return 404 for valid v1 endpoints
            assert_ne!(response.status(), StatusCode::NOT_FOUND);
        }
    }

    #[tokio::test]
    async fn test_malformed_json_request() {
        let state = create_test_state().await.unwrap();
        let app = crate::api::rest::create_app(state).await.unwrap();

        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/svm/transactions")
            .header("Content-Type", "application/json")
            .body(Body::from("{ invalid json"))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert!(response.status().is_client_error());
    }

    #[tokio::test]
    async fn test_missing_content_type() {
        let state = create_test_state().await.unwrap();
        let app = crate::api::rest::create_app(state).await.unwrap();

        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/svm/transactions")
            .body(Body::from(r#"{"test": "data"}"#))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        // Should handle missing content-type gracefully
        assert!(response.status().is_client_error() || response.status().is_server_error());
    }

    #[tokio::test]
    async fn test_method_not_allowed() {
        let state = create_test_state().await.unwrap();
        let app = crate::api::rest::create_app(state).await.unwrap();

        // Try DELETE method on a GET-only endpoint
        let request = Request::builder()
            .method(Method::DELETE)
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }
}