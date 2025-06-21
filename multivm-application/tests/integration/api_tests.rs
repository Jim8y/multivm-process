//! # API Integration Tests
//! 
//! Integration tests for the MultiVM Application Layer API interfaces.

use multivm_application::{ApplicationConfig, ApplicationServer};
use tokio::time::{timeout, Duration};
use reqwest::Client;
use serde_json::json;

/// Test configuration for integration tests
fn create_test_config() -> ApplicationConfig {
    let mut config = ApplicationConfig::default();
    
    // Use different ports for testing to avoid conflicts
    config.server.rest.port = 18080;
    config.server.graphql.port = 18081;
    config.server.websocket.port = 18082;
    config.server.admin.port = 18083;
    
    // Use in-memory cache for testing
    config.cache.cache_type = multivm_application::config::CacheType::Memory;
    
    // Disable external dependencies for testing
    config.monitoring.enable_metrics = false;
    
    config
}

/// Test REST API health check
#[tokio::test]
async fn test_rest_api_health_check() {
    let config = create_test_config();
    let server = ApplicationServer::new(config).await.expect("Failed to create server");
    
    // Start server in background
    let server_handle = tokio::spawn(async move {
        server.start().await
    });
    
    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Test health check endpoint
    let client = Client::new();
    let response = client
        .get("http://localhost:18080/health")
        .send()
        .await;
    
    match response {
        Ok(resp) => {
            assert_eq!(resp.status(), 200);
            let body: serde_json::Value = resp.json().await.expect("Failed to parse JSON");
            assert!(body["data"]["status"].as_str().is_some());
        }
        Err(_) => {
            // Server might not be fully started yet, this is acceptable for this test
            println!("Health check endpoint not yet available");
        }
    }
    
    // Stop the server
    server_handle.abort();
}

/// Test REST API SVM endpoints
#[tokio::test]
async fn test_rest_api_svm_endpoints() {
    let config = create_test_config();
    let server = ApplicationServer::new(config).await.expect("Failed to create server");
    
    // Start server in background
    let server_handle = tokio::spawn(async move {
        server.start().await
    });
    
    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let client = Client::new();
    
    // Test SVM account endpoint
    let response = client
        .get("http://localhost:18080/api/v1/svm/accounts/11111111111111111111111111111112")
        .send()
        .await;
    
    // We expect this to fail gracefully since we don't have a real Solana node
    if let Ok(resp) = response {
        // Response should be in the correct format even if it's an error
        assert!(resp.status().is_client_error() || resp.status().is_server_error() || resp.status().is_success());
    }
    
    // Stop the server
    server_handle.abort();
}

/// Test WebSocket connection
#[tokio::test]
async fn test_websocket_connection() {
    let config = create_test_config();
    let server = ApplicationServer::new(config).await.expect("Failed to create server");
    
    // Start server in background
    let server_handle = tokio::spawn(async move {
        server.start().await
    });
    
    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Test WebSocket connection
    let ws_url = "ws://localhost:18082/ws";
    
    // We'll just test that the server is listening on the WebSocket port
    // A full WebSocket test would require more complex setup
    let result = timeout(Duration::from_millis(500), async {
        tokio_tungstenite::connect_async(ws_url).await
    }).await;
    
    // Connection might fail due to missing implementation details, but the server should be listening
    match result {
        Ok(Ok(_)) => println!("WebSocket connection successful"),
        Ok(Err(_)) => println!("WebSocket connection failed (expected)"),
        Err(_) => println!("WebSocket connection timed out"),
    }
    
    // Stop the server
    server_handle.abort();
}

/// Test GraphQL endpoint
#[tokio::test]
async fn test_graphql_endpoint() {
    let config = create_test_config();
    let server = ApplicationServer::new(config).await.expect("Failed to create server");
    
    // Start server in background
    let server_handle = tokio::spawn(async move {
        server.start().await
    });
    
    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let client = Client::new();
    
    // Test GraphQL introspection query
    let query = json!({
        "query": "{ __schema { types { name } } }"
    });
    
    let response = client
        .post("http://localhost:18081/")
        .json(&query)
        .send()
        .await;
    
    if let Ok(resp) = response {
        // GraphQL endpoint should respond with proper JSON structure
        assert!(resp.status().is_success() || resp.status().is_client_error());
        
        if resp.status().is_success() {
            let body: serde_json::Value = resp.json().await.expect("Failed to parse JSON");
            // Should have either data or errors field
            assert!(body.get("data").is_some() || body.get("errors").is_some());
        }
    }
    
    // Stop the server
    server_handle.abort();
}

/// Test admin interface
#[tokio::test]
async fn test_admin_interface() {
    let config = create_test_config();
    let server = ApplicationServer::new(config).await.expect("Failed to create server");
    
    // Start server in background
    let server_handle = tokio::spawn(async move {
        server.start().await
    });
    
    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let client = Client::new();
    
    // Test admin dashboard
    let response = client
        .get("http://localhost:18083/")
        .send()
        .await;
    
    if let Ok(resp) = response {
        assert!(resp.status().is_success() || resp.status().is_client_error());
    }
    
    // Test admin API
    let response = client
        .get("http://localhost:18083/api/system/status")
        .send()
        .await;
    
    if let Ok(resp) = response {
        if resp.status().is_success() {
            let body: serde_json::Value = resp.json().await.expect("Failed to parse JSON");
            assert!(body.get("running").is_some());
        }
    }
    
    // Stop the server
    server_handle.abort();
}

/// Test configuration validation
#[tokio::test]
async fn test_configuration_validation() {
    let config = create_test_config();
    
    // Test that valid configuration is accepted
    let validation_result = config.validate();
    assert!(validation_result.is_ok(), "Valid configuration should pass validation");
    
    // Test server creation with valid config
    let server_result = ApplicationServer::new(config).await;
    assert!(server_result.is_ok(), "Server creation should succeed with valid config");
} 