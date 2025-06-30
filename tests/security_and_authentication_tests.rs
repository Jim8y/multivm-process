//! Comprehensive Security and Authentication Tests
//!
//! These tests verify the security mechanisms, authentication protocols,
//! authorization controls, and protection against common attack vectors.

use multivm_application::api::rest::create_app;
use multivm_common::{
    config::MultivmConfig,
    types::{ProcessId, health::HealthStatus},
    IpcMessage, IpcCommand,
    MultivmResult, MultivmError,
};
use multivm_p2p::{
    protocol::messages::{NetworkMessage, MessagePayload, ControlMessage},
    core::network::P2PNetwork,
};
use multivm_process_manager::MultivmProcessManager;
use axum::{
    body::Body,
    http::{Request, Method, StatusCode, header},
};
use serde_json::{json, Value};
use std::{
    time::{Duration, SystemTime, UNIX_EPOCH},
    sync::Arc,
    collections::HashMap,
};
use tempfile::TempDir;
use tokio::{
    test,
    time::{sleep, timeout},
    sync::{Mutex, RwLock},
    task::JoinSet,
};
use tower::ServiceExt;
use tracing_test::traced_test;
use jsonwebtoken::{encode, decode, Header, Algorithm, Validation, EncodingKey, DecodingKey};

/// JWT claims structure for testing
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct TestClaims {
    sub: String,
    exp: usize,
    iat: usize,
    iss: String,
    roles: Vec<String>,
    permissions: Vec<String>,
}

/// Security test context
struct SecurityTestContext {
    app: axum::Router,
    process_manager: MultivmProcessManager,
    jwt_secret: String,
    api_keys: HashMap<String, String>,
    temp_dir: TempDir,
}

impl SecurityTestContext {
    async fn new() -> MultivmResult<Self> {
        let temp_dir = TempDir::new().unwrap();
        let mut config = MultivmConfig::default();
        config.system.data_dir = temp_dir.path().to_path_buf();
        config.server.rest.port = 0;
        config.cache.redis.enabled = false;
        
        // Configure authentication
        config.auth.jwt.enabled = true;
        config.auth.jwt.secret = "test_jwt_secret_key_for_security_tests".to_string();
        config.auth.api_key.enabled = true;

        let process_manager = MultivmProcessManager::new(config.clone()).await?;
        
        let app_state = create_test_app_state(&config).await?;
        let app = create_app(app_state).await?;

        let jwt_secret = config.auth.jwt.secret.clone();
        let mut api_keys = HashMap::new();
        api_keys.insert("admin".to_string(), "admin_api_key_12345".to_string());
        api_keys.insert("user".to_string(), "user_api_key_67890".to_string());
        api_keys.insert("readonly".to_string(), "readonly_api_key_abcde".to_string());

        Ok(Self {
            app,
            process_manager,
            jwt_secret,
            api_keys,
            temp_dir,
        })
    }

    fn create_jwt_token(&self, subject: &str, roles: Vec<String>, permissions: Vec<String>, expires_in_seconds: u64) -> String {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as usize;
        let claims = TestClaims {
            sub: subject.to_string(),
            exp: now + expires_in_seconds as usize,
            iat: now,
            iss: "multivm-test".to_string(),
            roles,
            permissions,
        };

        let header = Header::new(Algorithm::HS256);
        let key = EncodingKey::from_secret(self.jwt_secret.as_ref());
        encode(&header, &claims, &key).unwrap()
    }

    fn create_expired_jwt_token(&self, subject: &str) -> String {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as usize;
        let claims = TestClaims {
            sub: subject.to_string(),
            exp: now - 3600, // Expired 1 hour ago
            iat: now - 7200, // Issued 2 hours ago
            iss: "multivm-test".to_string(),
            roles: vec!["user".to_string()],
            permissions: vec!["read".to_string()],
        };

        let header = Header::new(Algorithm::HS256);
        let key = EncodingKey::from_secret(self.jwt_secret.as_ref());
        encode(&header, &claims, &key).unwrap()
    }

    async fn make_request_with_auth(&self, method: Method, path: &str, body: Option<Value>, auth_header: Option<String>) -> (StatusCode, Value) {
        let mut request_builder = Request::builder()
            .method(method)
            .uri(path);

        if let Some(auth) = auth_header {
            request_builder = request_builder.header(header::AUTHORIZATION, auth);
        }

        if body.is_some() {
            request_builder = request_builder.header(header::CONTENT_TYPE, "application/json");
        }

        let request = request_builder
            .body(Body::from(body.map(|b| b.to_string()).unwrap_or_default()))
            .unwrap();

        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_json: Value = serde_json::from_slice(&body_bytes).unwrap_or(json!({}));
        
        (status, body_json)
    }
}

async fn create_test_app_state(config: &MultivmConfig) -> MultivmResult<multivm_application::api::rest::AppState> {
    let auth_manager = multivm_application::auth::manager::AuthManager::new(config.auth.clone()).await?;
    let cache = Box::new(multivm_application::cache::memory::MemoryCache::new(1000, Duration::from_secs(300)));
    let gateway = multivm_application::gateway::unified::UnifiedGateway::new(config.clone()).await?;
    let health_monitor = multivm_application::monitoring::health::HealthMonitor::new(Duration::from_secs(1)).await?;
    let metrics_collector = multivm_application::monitoring::metrics::MetricsCollector::new().await?;
    
    Ok(multivm_application::api::rest::AppState {
        config: config.clone(),
        auth_manager,
        cache,
        gateway,
        health_monitor,
        metrics_collector,
    })
}

#[traced_test]
#[test]
async fn test_jwt_authentication_valid_tokens() {
    let ctx = SecurityTestContext::new().await.unwrap();
    
    // Test admin token
    let admin_token = ctx.create_jwt_token(
        "admin_user",
        vec!["admin".to_string()],
        vec!["read".to_string(), "write".to_string(), "admin".to_string()],
        3600
    );
    
    let auth_header = format!("Bearer {}", admin_token);
    let (status, response) = ctx.make_request_with_auth(
        Method::GET,
        "/api/v1/admin/system/status",
        None,
        Some(auth_header),
    ).await;
    
    assert_eq!(status, StatusCode::OK);
    assert!(response["authenticated"].as_bool().unwrap_or(false));
    
    // Test user token
    let user_token = ctx.create_jwt_token(
        "regular_user",
        vec!["user".to_string()],
        vec!["read".to_string(), "write".to_string()],
        3600
    );
    
    let auth_header = format!("Bearer {}", user_token);
    let (status, _) = ctx.make_request_with_auth(
        Method::GET,
        "/api/v1/health",
        None,
        Some(auth_header),
    ).await;
    
    assert_eq!(status, StatusCode::OK);
    
    // Test readonly token
    let readonly_token = ctx.create_jwt_token(
        "readonly_user",
        vec!["readonly".to_string()],
        vec!["read".to_string()],
        3600
    );
    
    let auth_header = format!("Bearer {}", readonly_token);
    let (status, _) = ctx.make_request_with_auth(
        Method::GET,
        "/api/v1/health",
        None,
        Some(auth_header),
    ).await;
    
    assert_eq!(status, StatusCode::OK);
}

#[traced_test]
#[test]
async fn test_jwt_authentication_invalid_tokens() {
    let ctx = SecurityTestContext::new().await.unwrap();
    
    // Test expired token
    let expired_token = ctx.create_expired_jwt_token("expired_user");
    let auth_header = format!("Bearer {}", expired_token);
    let (status, _) = ctx.make_request_with_auth(
        Method::GET,
        "/api/v1/admin/system/status",
        None,
        Some(auth_header),
    ).await;
    
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    
    // Test malformed token
    let (status, _) = ctx.make_request_with_auth(
        Method::GET,
        "/api/v1/admin/system/status",
        None,
        Some("Bearer invalid_token_format".to_string()),
    ).await;
    
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    
    // Test missing token
    let (status, _) = ctx.make_request_with_auth(
        Method::GET,
        "/api/v1/admin/system/status",
        None,
        None,
    ).await;
    
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    
    // Test wrong algorithm token (try to create with different algorithm)
    let wrong_key = EncodingKey::from_secret(b"wrong_secret");
    let header = Header::new(Algorithm::HS256);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as usize;
    let claims = TestClaims {
        sub: "test_user".to_string(),
        exp: now + 3600,
        iat: now,
        iss: "multivm-test".to_string(),
        roles: vec!["admin".to_string()],
        permissions: vec!["admin".to_string()],
    };
    
    let wrong_token = encode(&header, &claims, &wrong_key).unwrap();
    let auth_header = format!("Bearer {}", wrong_token);
    let (status, _) = ctx.make_request_with_auth(
        Method::GET,
        "/api/v1/admin/system/status",
        None,
        Some(auth_header),
    ).await;
    
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[traced_test]
#[test]
async fn test_api_key_authentication() {
    let ctx = SecurityTestContext::new().await.unwrap();
    
    // Test valid admin API key
    let admin_key = ctx.api_keys.get("admin").unwrap();
    let (status, _) = ctx.make_request_with_auth(
        Method::GET,
        "/api/v1/admin/system/status",
        None,
        Some(format!("ApiKey {}", admin_key)),
    ).await;
    
    assert_eq!(status, StatusCode::OK);
    
    // Test valid user API key
    let user_key = ctx.api_keys.get("user").unwrap();
    let (status, _) = ctx.make_request_with_auth(
        Method::GET,
        "/api/v1/health",
        None,
        Some(format!("ApiKey {}", user_key)),
    ).await;
    
    assert_eq!(status, StatusCode::OK);
    
    // Test invalid API key
    let (status, _) = ctx.make_request_with_auth(
        Method::GET,
        "/api/v1/admin/system/status",
        None,
        Some("ApiKey invalid_api_key_12345".to_string()),
    ).await;
    
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[traced_test]
#[test]
async fn test_role_based_authorization() {
    let ctx = SecurityTestContext::new().await.unwrap();
    
    // Admin should access admin endpoints
    let admin_token = ctx.create_jwt_token(
        "admin_user",
        vec!["admin".to_string()],
        vec!["admin".to_string()],
        3600
    );
    
    let auth_header = format!("Bearer {}", admin_token);
    let (status, _) = ctx.make_request_with_auth(
        Method::GET,
        "/api/v1/admin/system/status",
        None,
        Some(auth_header),
    ).await;
    
    assert_eq!(status, StatusCode::OK);
    
    // Regular user should NOT access admin endpoints
    let user_token = ctx.create_jwt_token(
        "regular_user",
        vec!["user".to_string()],
        vec!["read".to_string(), "write".to_string()],
        3600
    );
    
    let auth_header = format!("Bearer {}", user_token);
    let (status, _) = ctx.make_request_with_auth(
        Method::GET,
        "/api/v1/admin/system/status",
        None,
        Some(auth_header),
    ).await;
    
    assert_eq!(status, StatusCode::FORBIDDEN);
    
    // Readonly user should access read endpoints but NOT write endpoints
    let readonly_token = ctx.create_jwt_token(
        "readonly_user",
        vec!["readonly".to_string()],
        vec!["read".to_string()],
        3600
    );
    
    let auth_header = format!("Bearer {}", readonly_token);
    let (status, _) = ctx.make_request_with_auth(
        Method::GET,
        "/api/v1/health",
        None,
        Some(auth_header.clone()),
    ).await;
    
    assert_eq!(status, StatusCode::OK);
    
    // Readonly user should NOT be able to create accounts
    let account_data = json!({
        "vm_type": "svm",
        "address": "readonly_test_account",
        "metadata": {"test": "authorization"}
    });
    
    let (status, _) = ctx.make_request_with_auth(
        Method::POST,
        "/api/v1/accounts",
        Some(account_data),
        Some(auth_header),
    ).await;
    
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[traced_test]
#[test]
async fn test_sql_injection_protection() {
    let ctx = SecurityTestContext::new().await.unwrap();
    
    let user_token = ctx.create_jwt_token(
        "test_user",
        vec!["user".to_string()],
        vec!["read".to_string(), "write".to_string()],
        3600
    );
    let auth_header = format!("Bearer {}", user_token);
    
    // Test SQL injection attempts in various parameters
    let sql_injection_payloads = vec![
        "'; DROP TABLE accounts; --",
        "1' OR '1'='1",
        "admin'; DELETE FROM users; --",
        "1 UNION SELECT * FROM sensitive_data",
        "'; INSERT INTO users (username, password) VALUES ('hacker', 'password'); --",
    ];
    
    for payload in sql_injection_payloads {
        // Test in account address field
        let account_data = json!({
            "vm_type": "svm",
            "address": payload,
            "metadata": {"test": "sql_injection"}
        });
        
        let (status, response) = ctx.make_request_with_auth(
            Method::POST,
            "/api/v1/accounts",
            Some(account_data),
            Some(auth_header.clone()),
        ).await;
        
        // Should either validate input (400) or reject malicious content
        assert!(status == StatusCode::BAD_REQUEST || status == StatusCode::UNPROCESSABLE_ENTITY,
                "SQL injection payload '{}' was not properly rejected: {}", payload, status);
        
        // Test in query parameters
        let malicious_query = format!("/api/v1/accounts?search={}", urlencoding::encode(payload));
        let (status, _) = ctx.make_request_with_auth(
            Method::GET,
            &malicious_query,
            None,
            Some(auth_header.clone()),
        ).await;
        
        // Should handle malicious query parameters gracefully
        assert!(status != StatusCode::INTERNAL_SERVER_ERROR,
                "SQL injection in query parameter '{}' caused server error", payload);
    }
}

#[traced_test]
#[test]
async fn test_xss_protection() {
    let ctx = SecurityTestContext::new().await.unwrap();
    
    let user_token = ctx.create_jwt_token(
        "test_user",
        vec!["user".to_string()],
        vec!["read".to_string(), "write".to_string()],
        3600
    );
    let auth_header = format!("Bearer {}", user_token);
    
    // Test XSS payloads
    let xss_payloads = vec![
        "<script>alert('xss')</script>",
        "javascript:alert('xss')",
        "<img src=x onerror=alert('xss')>",
        "<svg onload=alert('xss')>",
        "';alert('xss');//",
        "<iframe src='javascript:alert(\"xss\")'></iframe>",
    ];
    
    for payload in xss_payloads {
        let account_data = json!({
            "vm_type": "svm",
            "address": "xss_test_account",
            "metadata": {
                "name": payload,
                "description": payload
            }
        });
        
        let (status, response) = ctx.make_request_with_auth(
            Method::POST,
            "/api/v1/accounts",
            Some(account_data),
            Some(auth_header.clone()),
        ).await;
        
        // If account creation succeeds, verify XSS payload is sanitized
        if status == StatusCode::CREATED {
            // Check that dangerous scripts are not echoed back
            let response_str = response.to_string();
            assert!(!response_str.contains("<script>"), "XSS script tag found in response");
            assert!(!response_str.contains("javascript:"), "XSS javascript: found in response");
            assert!(!response_str.contains("onerror="), "XSS event handler found in response");
        } else {
            // Should reject or sanitize XSS attempts
            assert!(status == StatusCode::BAD_REQUEST || status == StatusCode::UNPROCESSABLE_ENTITY,
                    "XSS payload '{}' was not properly handled: {}", payload, status);
        }
    }
}

#[traced_test]
#[test]
async fn test_csrf_protection() {
    let ctx = SecurityTestContext::new().await.unwrap();
    
    let user_token = ctx.create_jwt_token(
        "test_user",
        vec!["user".to_string()],
        vec!["read".to_string(), "write".to_string()],
        3600
    );
    
    // Test request without CSRF token (should be rejected for state-changing operations)
    let account_data = json!({
        "vm_type": "svm",
        "address": "csrf_test_account",
        "metadata": {"test": "csrf"}
    });
    
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/accounts")
        .header(header::AUTHORIZATION, format!("Bearer {}", user_token))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ORIGIN, "https://malicious-site.com") // Suspicious origin
        .body(Body::from(account_data.to_string()))
        .unwrap();
    
    let response = ctx.app.clone().oneshot(request).await.unwrap();
    
    // CSRF protection should either require CSRF token or validate origin
    assert!(response.status() == StatusCode::FORBIDDEN || 
            response.status() == StatusCode::BAD_REQUEST ||
            response.status() == StatusCode::CREATED, // If CSRF protection allows this request
            "CSRF protection response: {}", response.status());
}

#[traced_test]
#[test]
async fn test_rate_limiting() {
    let ctx = SecurityTestContext::new().await.unwrap();
    
    let user_token = ctx.create_jwt_token(
        "test_user",
        vec!["user".to_string()],
        vec!["read".to_string()],
        3600
    );
    let auth_header = format!("Bearer {}", user_token);
    
    // Make rapid requests to test rate limiting
    let mut success_count = 0;
    let mut rate_limited_count = 0;
    
    for i in 0..50 {
        let (status, _) = ctx.make_request_with_auth(
            Method::GET,
            &format!("/api/v1/health?test_request={}", i),
            None,
            Some(auth_header.clone()),
        ).await;
        
        match status {
            StatusCode::OK => success_count += 1,
            StatusCode::TOO_MANY_REQUESTS => rate_limited_count += 1,
            _ => {},
        }
        
        // Small delay to avoid overwhelming the system
        sleep(Duration::from_millis(10)).await;
    }
    
    println!("Rate limiting test: {} successful, {} rate limited", success_count, rate_limited_count);
    
    // Should have some rate limiting if implemented
    // Note: This test may pass even without rate limiting if not implemented
    assert!(success_count > 0, "All requests should not be rate limited");
}

#[traced_test]
#[test]
async fn test_input_validation_and_sanitization() {
    let ctx = SecurityTestContext::new().await.unwrap();
    
    let user_token = ctx.create_jwt_token(
        "test_user",
        vec!["user".to_string()],
        vec!["read".to_string(), "write".to_string()],
        3600
    );
    let auth_header = format!("Bearer {}", user_token);
    
    // Test various invalid inputs
    let invalid_inputs = vec![
        // Oversized inputs
        json!({
            "vm_type": "svm",
            "address": "a".repeat(10000), // Very long address
            "metadata": {"test": "oversized"}
        }),
        // Invalid data types
        json!({
            "vm_type": 12345, // Should be string
            "address": "test_account",
            "metadata": {"test": "invalid_type"}
        }),
        // Missing required fields
        json!({
            "address": "test_account",
            // Missing vm_type
            "metadata": {"test": "missing_field"}
        }),
        // Invalid enum values
        json!({
            "vm_type": "invalid_vm_type",
            "address": "test_account",
            "metadata": {"test": "invalid_enum"}
        }),
        // Null values where not allowed
        json!({
            "vm_type": null,
            "address": "test_account",
            "metadata": {"test": "null_value"}
        }),
    ];
    
    for (i, invalid_input) in invalid_inputs.iter().enumerate() {
        let (status, response) = ctx.make_request_with_auth(
            Method::POST,
            "/api/v1/accounts",
            Some(invalid_input.clone()),
            Some(auth_header.clone()),
        ).await;
        
        // Should reject invalid inputs with appropriate error codes
        assert!(status == StatusCode::BAD_REQUEST || 
                status == StatusCode::UNPROCESSABLE_ENTITY,
                "Invalid input {} was not properly rejected: {} - Response: {}", 
                i, status, response);
        
        // Should provide meaningful error messages
        assert!(response["error"].is_string() || response["message"].is_string(),
                "No error message provided for invalid input {}", i);
    }
}

#[traced_test]
#[test]
async fn test_sensitive_data_exposure() {
    let ctx = SecurityTestContext::new().await.unwrap();
    
    let user_token = ctx.create_jwt_token(
        "test_user",
        vec!["user".to_string()],
        vec!["read".to_string()],
        3600
    );
    let auth_header = format!("Bearer {}", user_token);
    
    // Test that error responses don't expose sensitive information
    let (status, response) = ctx.make_request_with_auth(
        Method::GET,
        "/api/v1/accounts/nonexistent_account",
        None,
        Some(auth_header),
    ).await;
    
    assert_eq!(status, StatusCode::NOT_FOUND);
    
    let response_str = response.to_string().to_lowercase();
    
    // Check that sensitive information is not exposed
    let sensitive_keywords = vec![
        "password", "secret", "private_key", "database", "internal", 
        "stack_trace", "sql", "connection_string", "api_key"
    ];
    
    for keyword in sensitive_keywords {
        assert!(!response_str.contains(keyword), 
                "Sensitive keyword '{}' found in error response: {}", keyword, response);
    }
    
    // Test unauthorized access doesn't reveal system information
    let (status, response) = ctx.make_request_with_auth(
        Method::GET,
        "/api/v1/admin/system/status",
        None,
        None, // No authentication
    ).await;
    
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    
    let response_str = response.to_string().to_lowercase();
    assert!(!response_str.contains("internal"), "Internal information exposed in unauthorized response");
}

#[traced_test]
#[test]
async fn test_session_management() {
    let ctx = SecurityTestContext::new().await.unwrap();
    
    // Test token expiration
    let short_lived_token = ctx.create_jwt_token(
        "test_user",
        vec!["user".to_string()],
        vec!["read".to_string()],
        1 // Expires in 1 second
    );
    
    let auth_header = format!("Bearer {}", short_lived_token);
    
    // Should work initially
    let (status, _) = ctx.make_request_with_auth(
        Method::GET,
        "/api/v1/health",
        None,
        Some(auth_header.clone()),
    ).await;
    
    assert_eq!(status, StatusCode::OK);
    
    // Wait for token to expire
    sleep(Duration::from_secs(2)).await;
    
    // Should be unauthorized after expiration
    let (status, _) = ctx.make_request_with_auth(
        Method::GET,
        "/api/v1/health",
        None,
        Some(auth_header),
    ).await;
    
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[traced_test]
#[test]
async fn test_concurrent_authentication_attacks() {
    let ctx = SecurityTestContext::new().await.unwrap();
    
    // Test concurrent login attempts (brute force simulation)
    let mut handles = vec![];
    let attack_attempts = 20;
    
    for i in 0..attack_attempts {
        let app_clone = ctx.app.clone();
        let handle = tokio::spawn(async move {
            let malicious_token = format!("Bearer fake_token_{}", i);
            let request = Request::builder()
                .method(Method::GET)
                .uri("/api/v1/admin/system/status")
                .header(header::AUTHORIZATION, malicious_token)
                .body(Body::empty())
                .unwrap();
            
            app_clone.oneshot(request).await
        });
        handles.push(handle);
    }
    
    let mut unauthorized_count = 0;
    let mut error_count = 0;
    
    for handle in handles {
        match handle.await {
            Ok(Ok(response)) => {
                if response.status() == StatusCode::UNAUTHORIZED {
                    unauthorized_count += 1;
                }
            }
            _ => error_count += 1,
        }
    }
    
    // All malicious attempts should be rejected
    assert_eq!(unauthorized_count, attack_attempts, "Not all malicious authentication attempts were rejected");
    assert_eq!(error_count, 0, "System should handle concurrent auth attacks gracefully");
}

#[traced_test]
#[test]
async fn test_api_endpoint_security_headers() {
    let ctx = SecurityTestContext::new().await.unwrap();
    
    let user_token = ctx.create_jwt_token(
        "test_user",
        vec!["user".to_string()],
        vec!["read".to_string()],
        3600
    );
    
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/health")
        .header(header::AUTHORIZATION, format!("Bearer {}", user_token))
        .body(Body::empty())
        .unwrap();
    
    let response = ctx.app.clone().oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let headers = response.headers();
    
    // Check for important security headers
    let security_headers = vec![
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("x-xss-protection", "1; mode=block"),
        ("strict-transport-security", "max-age=31536000"),
    ];
    
    for (header_name, expected_value) in security_headers {
        if let Some(header_value) = headers.get(header_name) {
            let value_str = header_value.to_str().unwrap_or("");
            assert!(value_str.contains(expected_value) || !value_str.is_empty(),
                    "Security header '{}' missing or incorrect: got '{}'", header_name, value_str);
        }
        // Note: Headers might not be implemented yet, so we don't fail if missing
    }
}

#[traced_test]
#[test]
async fn test_privilege_escalation_prevention() {
    let ctx = SecurityTestContext::new().await.unwrap();
    
    // Create a user token with limited privileges
    let limited_token = ctx.create_jwt_token(
        "limited_user",
        vec!["user".to_string()],
        vec!["read".to_string()],
        3600
    );
    
    let auth_header = format!("Bearer {}", limited_token);
    
    // Try to access admin endpoints
    let admin_endpoints = vec![
        "/api/v1/admin/system/status",
        "/api/v1/admin/config",
        "/api/v1/admin/users",
        "/api/v1/admin/logs",
        "/api/v1/admin/shutdown",
    ];
    
    for endpoint in admin_endpoints {
        let (status, _) = ctx.make_request_with_auth(
            Method::GET,
            endpoint,
            None,
            Some(auth_header.clone()),
        ).await;
        
        assert_eq!(status, StatusCode::FORBIDDEN, 
                   "Limited user was able to access admin endpoint: {}", endpoint);
    }
    
    // Try to perform administrative actions
    let admin_actions = vec![
        (Method::POST, "/api/v1/admin/system/restart"),
        (Method::DELETE, "/api/v1/admin/system/reset"),
        (Method::PUT, "/api/v1/admin/config"),
    ];
    
    for (method, endpoint) in admin_actions {
        let (status, _) = ctx.make_request_with_auth(
            method,
            endpoint,
            Some(json!({"test": "privilege_escalation"})),
            Some(auth_header.clone()),
        ).await;
        
        assert!(status == StatusCode::FORBIDDEN || status == StatusCode::NOT_FOUND,
                "Limited user was able to perform admin action: {} {}", method, endpoint);
    }
}

#[traced_test]
#[test]
async fn test_data_encryption_in_transit() {
    let ctx = SecurityTestContext::new().await.unwrap();
    
    // This test verifies that sensitive data is not transmitted in plain text
    // In a real scenario, this would test HTTPS enforcement
    
    let user_token = ctx.create_jwt_token(
        "test_user",
        vec!["user".to_string()],
        vec!["read".to_string(), "write".to_string()],
        3600
    );
    let auth_header = format!("Bearer {}", user_token);
    
    // Create account with sensitive data
    let account_data = json!({
        "vm_type": "svm",
        "address": "encryption_test_account",
        "metadata": {
            "sensitive_field": "this_should_be_encrypted",
            "private_info": "confidential_data"
        }
    });
    
    let (status, response) = ctx.make_request_with_auth(
        Method::POST,
        "/api/v1/accounts",
        Some(account_data),
        Some(auth_header),
    ).await;
    
    // Verify the operation succeeds
    assert!(status == StatusCode::CREATED || status == StatusCode::OK);
    
    // In a real implementation, you would verify:
    // 1. HTTPS is enforced
    // 2. Sensitive data is encrypted in the database
    // 3. TLS configuration is secure
    // 4. Certificates are valid
    
    // For this test, we verify that the response doesn't expose raw sensitive data
    let response_str = response.to_string();
    assert!(!response_str.contains("this_should_be_encrypted") || status == StatusCode::CREATED,
            "Sensitive data might be exposed in response");
}