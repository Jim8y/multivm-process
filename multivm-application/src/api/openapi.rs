//! OpenAPI documentation for MultiVM REST API

use utoipa::OpenApi;

/// OpenAPI 3.0 specification for MultiVM Application
#[derive(OpenApi)]
#[openapi(
    info(
        title = "MultiVM Application API",
        description = "REST API for the MultiVM blockchain platform supporting both EVM and SVM",
        version = "1.0.0",
        contact(
            name = "MultiVM Team",
            url = "https://github.com/multivm/multivm"
        ),
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT"
        )
    ),
    servers(
        (url = "http://localhost:8080/api/v1", description = "Local development server"),
        (url = "https://api.multivm.network/v1", description = "Production server")
    ),
    paths(
        crate::api::rest::handlers::system::health_check,
        crate::api::rest::handlers::system::metrics,
        crate::api::rest::handlers::accounts::get_account_binding,
        crate::api::rest::handlers::accounts::create_account_binding,
        crate::api::rest::handlers::transactions::submit_transaction,
        crate::api::rest::handlers::blocks::get_block,
        crate::api::rest::handlers::explorer::get_network_info
    ),
    components(
        schemas(
            crate::api::rest::types::ApiResponse,
            crate::api::rest::types::ApiError,
            crate::api::rest::types::HealthStatus,
            crate::api::rest::types::SystemMetrics,
            crate::api::rest::types::AccountBinding,
            crate::api::rest::types::TransactionSubmission,
            crate::api::rest::types::Block,
            crate::api::rest::types::NetworkInfo
        )
    ),
    tags(
        (name = "System", description = "System health and metrics endpoints"),
        (name = "Accounts", description = "Cross-VM account binding operations"),
        (name = "Transactions", description = "Transaction submission and querying"),
        (name = "Blocks", description = "Block information and history"),
        (name = "Explorer", description = "Blockchain explorer endpoints")
    ),
    security(
        ("api_key" = []),
        ("jwt_token" = [])
    )
)]
pub struct ApiDoc;

/// Security schemes for API authentication
pub mod security_schemes {
    use utoipa::openapi::security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme};
    
    /// API Key authentication scheme
    pub fn api_key_scheme() -> SecurityScheme {
        SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("X-API-Key")))
    }
    
    /// JWT Bearer token authentication scheme
    pub fn jwt_scheme() -> SecurityScheme {
        SecurityScheme::Http(
            HttpBuilder::new()
                .scheme(HttpAuthScheme::Bearer)
                .bearer_format("JWT")
                .build()
        )
    }
}

/// Common API response examples
pub mod examples {
    use serde_json::json;
    
    /// Example successful response
    pub fn success_response() -> serde_json::Value {
        json!({
            "data": {},
            "error": null,
            "metadata": {
                "request_id": "req_123456789",
                "timestamp": "2023-01-01T00:00:00Z",
                "response_time_ms": 42
            }
        })
    }
    
    /// Example error response
    pub fn error_response() -> serde_json::Value {
        json!({
            "data": null,
            "error": {
                "code": "VALIDATION_ERROR",
                "message": "Invalid request parameters",
                "details": {
                    "field": "account_id",
                    "issue": "Required field missing"
                }
            },
            "metadata": {
                "request_id": "req_123456789",
                "timestamp": "2023-01-01T00:00:00Z",
                "response_time_ms": 15
            }
        })
    }
}