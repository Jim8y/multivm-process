use super::api_key::{ApiKeyInfo, ApiKeyManager, ApiKeyStorage, ValidatedApiKey};
use super::jwt::{JwtAuth, TokenMetadata, UserRole, ValidatedToken};
use super::permissions::{Permission, PermissionChecker};
use crate::config::AuthConfig;
use crate::error::{ApplicationError, AuthResult};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Main authentication manager
#[derive(Debug)]
pub struct AuthManager {
    jwt_auth: Arc<JwtAuth>,
    api_key_manager: Arc<RwLock<ApiKeyManager>>,
    config: AuthConfig,
}

/// Authentication result containing user information
#[derive(Debug, Clone)]
pub struct AuthenticationResult {
    /// User ID
    pub user_id: String,

    /// User permissions
    pub permissions: Vec<Permission>,

    /// Permission checker for easy validation
    pub permission_checker: PermissionChecker,

    /// User role
    pub role: UserRole,

    /// Authentication method used
    pub auth_method: AuthMethod,

    /// Token metadata (if JWT was used)
    pub token_metadata: Option<TokenMetadata>,

    /// API key info (if API key was used)
    pub api_key_info: Option<ApiKeyInfo>,
}

/// Authentication method enum
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMethod {
    Jwt,
    ApiKey,
    Anonymous,
}

/// Authentication request
#[derive(Debug, Clone, PartialEq)]
pub enum AuthRequest {
    /// JWT token authentication
    JwtToken(String),

    /// API key authentication
    ApiKey(String),

    /// No authentication (anonymous)
    Anonymous,
}

impl AuthManager {
    /// Create a new authentication manager
    pub async fn new(config: &AuthConfig) -> AuthResult<Self> {
        // Initialize JWT authentication
        let jwt_auth = Arc::new(JwtAuth::new(&config.jwt_secret, config.jwt_expiration)?);

        // Initialize API key manager
        let storage_backend = match config.api_key_validation {
            crate::config::ApiKeyValidation::Database => ApiKeyStorage::Database,
            crate::config::ApiKeyValidation::Environment => ApiKeyStorage::Memory, // For now
            crate::config::ApiKeyValidation::Redis => ApiKeyStorage::Redis,
        };

        let api_key_manager = Arc::new(RwLock::new(ApiKeyManager::new(storage_backend)));

        Ok(Self {
            jwt_auth,
            api_key_manager,
            config: config.clone(),
        })
    }

    /// Authenticate a request
    pub async fn authenticate(&self, request: AuthRequest) -> AuthResult<AuthenticationResult> {
        match request {
            AuthRequest::JwtToken(token) => self.authenticate_jwt(&token).await,
            AuthRequest::ApiKey(api_key) => self.authenticate_api_key(&api_key).await,
            AuthRequest::Anonymous => self.authenticate_anonymous().await,
        }
    }

    /// Authenticate using JWT token
    async fn authenticate_jwt(&self, token: &str) -> AuthResult<AuthenticationResult> {
        let validated_token = self.jwt_auth.validate_token(token)?;

        if validated_token.is_expired {
            return Err(ApplicationError::AuthenticationFailed {
                reason: "JWT token has expired".to_string(),
            });
        }

        let claims = validated_token.claims;
        let permission_checker = PermissionChecker::new(claims.permissions.clone());

        Ok(AuthenticationResult {
            user_id: claims.sub.clone(),
            permissions: claims.permissions.clone(),
            permission_checker,
            role: claims.metadata.role.clone(),
            auth_method: AuthMethod::Jwt,
            token_metadata: Some(claims.metadata),
            api_key_info: None,
        })
    }

    /// Authenticate using API key
    async fn authenticate_api_key(&self, api_key: &str) -> AuthResult<AuthenticationResult> {
        let api_key_manager = self.api_key_manager.read().await;
        let validated_key = api_key_manager.validate_api_key(api_key).await?;

        if validated_key.is_expired {
            return Err(ApplicationError::AuthenticationFailed {
                reason: "API key has expired".to_string(),
            });
        }

        if validated_key.is_rate_limited {
            return Err(ApplicationError::RateLimitExceeded {
                limit: validated_key.key_info.rate_limit,
                window: "minute".to_string(),
            });
        }

        let key_info = validated_key.key_info;
        let permission_checker = PermissionChecker::new(key_info.permissions.clone());

        Ok(AuthenticationResult {
            user_id: key_info.user_id.clone(),
            permissions: key_info.permissions.clone(),
            permission_checker,
            role: key_info.role.clone(),
            auth_method: AuthMethod::ApiKey,
            token_metadata: None,
            api_key_info: Some(key_info),
        })
    }

    /// Handle anonymous authentication (limited permissions)
    async fn authenticate_anonymous(&self) -> AuthResult<AuthenticationResult> {
        let permissions = vec![Permission::ReadSystemStatus, Permission::ReadNetworkInfo];

        let permission_checker = PermissionChecker::new(permissions.clone());

        Ok(AuthenticationResult {
            user_id: "anonymous".to_string(),
            permissions,
            permission_checker,
            role: UserRole::Guest,
            auth_method: AuthMethod::Anonymous,
            token_metadata: None,
            api_key_info: None,
        })
    }

    /// Extract authentication from HTTP headers
    pub fn extract_auth_from_headers(
        &self,
        authorization: Option<&str>,
        api_key: Option<&str>,
    ) -> AuthRequest {
        // Check for API key first (header: X-API-Key)
        if let Some(key) = api_key {
            return AuthRequest::ApiKey(key.to_string());
        }

        // Check for JWT token in Authorization header
        if let Some(auth_header) = authorization {
            if let Ok(token) = JwtAuth::extract_token_from_header(auth_header) {
                return AuthRequest::JwtToken(token.to_string());
            }
        }

        // Default to anonymous
        AuthRequest::Anonymous
    }

    /// Create a new user token
    pub async fn create_user_token(
        &self,
        user_id: &str,
        role: UserRole,
        email: Option<String>,
    ) -> AuthResult<String> {
        match role {
            UserRole::Admin | UserRole::SuperAdmin => self.jwt_auth.create_admin_token(user_id),
            UserRole::PowerUser => self.jwt_auth.create_power_user_token(user_id, email),
            UserRole::User => self.jwt_auth.create_user_token(user_id, email),
            UserRole::Guest => Err(ApplicationError::AuthenticationFailed {
                reason: "Cannot create token for guest role".to_string(),
            }),
        }
    }

    /// Refresh a JWT token
    pub async fn refresh_token(&self, token: &str) -> AuthResult<String> {
        self.jwt_auth.refresh_token(token)
    }

    /// Create a new API key
    pub async fn create_api_key(
        &self,
        user_id: &str,
        name: &str,
        permissions: Vec<Permission>,
        role: UserRole,
    ) -> AuthResult<(String, String)> {
        let mut api_key_manager = self.api_key_manager.write().await;

        let request = super::api_key::CreateApiKeyRequest {
            user_id: user_id.to_string(),
            name: name.to_string(),
            expires_in: None,       // No expiration by default
            rate_limit: Some(1000), // Default rate limit
            permissions,
            role,
            metadata: None,
        };

        let response = api_key_manager.create_api_key(request).await?;
        Ok((response.key_id, response.api_key))
    }

    /// Revoke an API key
    pub async fn revoke_api_key(&self, key_id: &str) -> AuthResult<()> {
        let mut api_key_manager = self.api_key_manager.write().await;
        api_key_manager.revoke_api_key(key_id).await
    }

    /// List API keys for a user
    pub async fn list_user_api_keys(&self, user_id: &str) -> AuthResult<Vec<ApiKeyInfo>> {
        let api_key_manager = self.api_key_manager.read().await;
        api_key_manager.list_user_keys(user_id).await
    }

    /// Update API key usage statistics
    pub async fn update_api_key_usage(&self, key_id: &str) -> AuthResult<()> {
        let mut api_key_manager = self.api_key_manager.write().await;
        api_key_manager.update_usage_stats(key_id).await
    }

    /// Validate API key
    pub async fn validate_api_key(&self, api_key: &str) -> AuthResult<ValidatedApiKey> {
        let api_key_manager = self.api_key_manager.read().await;
        api_key_manager.validate_api_key(api_key).await
    }

    /// Validate JWT token
    pub async fn validate_jwt_token(&self, token: &str) -> AuthResult<ValidatedToken> {
        self.jwt_auth.validate_token(token)
    }

    /// Check if authentication is required for an endpoint
    pub fn is_auth_required(&self, path: &str, method: &str) -> bool {
        // Define public endpoints that don't require authentication
        let public_endpoints = vec![
            ("/health", "GET"),
            ("/metrics", "GET"),
            ("/status", "GET"),
            ("/", "GET"), // Root endpoint
        ];

        !public_endpoints.contains(&(path, method))
    }

    /// Validate permissions for an operation
    pub fn validate_permissions(
        &self,
        auth_result: &AuthenticationResult,
        required_permissions: &[Permission],
    ) -> AuthResult<()> {
        auth_result
            .permission_checker
            .require_any_permission(required_permissions)
    }

    /// Check if user is admin
    pub fn is_admin(&self, auth_result: &AuthenticationResult) -> bool {
        auth_result.permission_checker.is_admin() || auth_result.role.is_elevated()
    }
}

impl AuthenticationResult {
    /// Check if user has specific permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permission_checker.has_permission(permission)
    }

    /// Require specific permission (returns error if not present)
    pub fn require_permission(&self, permission: &Permission) -> AuthResult<()> {
        self.permission_checker.require_permission(permission)
    }

    /// Check if user is admin
    pub fn is_admin(&self) -> bool {
        self.permission_checker.is_admin() || self.role.is_elevated()
    }

    /// Get user identifier for logging/metrics
    pub fn user_identifier(&self) -> String {
        match &self.auth_method {
            AuthMethod::Jwt => format!("user:{}", self.user_id),
            AuthMethod::ApiKey => {
                if let Some(key_info) = &self.api_key_info {
                    format!("apikey:{}", key_info.key_id)
                } else {
                    format!("user:{}", self.user_id)
                }
            }
            AuthMethod::Anonymous => "anonymous".to_string(),
        }
    }
}

impl std::fmt::Display for AuthMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthMethod::Jwt => write!(f, "JWT"),
            AuthMethod::ApiKey => write!(f, "API_KEY"),
            AuthMethod::Anonymous => write!(f, "ANONYMOUS"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AuthConfig;
    use std::time::Duration;

    #[tokio::test]
    async fn test_auth_manager_creation() {
        let config = AuthConfig {
            jwt_secret: "test-secret-key-that-is-long-enough".to_string(),
            jwt_expiration: Duration::from_secs(3600),
            enable_api_keys: true,
            api_key_validation: crate::config::ApiKeyValidation::Database,
            admin_api_key: None,
        };

        let auth_manager = AuthManager::new(&config).await;
        assert!(auth_manager.is_ok());
    }

    #[tokio::test]
    async fn test_anonymous_authentication() {
        let config = AuthConfig {
            jwt_secret: "test-secret-key-that-is-long-enough".to_string(),
            jwt_expiration: Duration::from_secs(3600),
            enable_api_keys: true,
            api_key_validation: crate::config::ApiKeyValidation::Database,
            admin_api_key: None,
        };

        let auth_manager = AuthManager::new(&config).await.unwrap();
        let result = auth_manager
            .authenticate(AuthRequest::Anonymous)
            .await
            .unwrap();

        assert_eq!(result.auth_method, AuthMethod::Anonymous);
        assert_eq!(result.role, UserRole::Guest);
        assert!(result.has_permission(&Permission::ReadSystemStatus));
    }

    #[tokio::test]
    async fn test_auth_header_extraction() {
        let config = AuthConfig {
            jwt_secret: "test-secret-key-that-is-long-enough".to_string(),
            jwt_expiration: Duration::from_secs(3600),
            enable_api_keys: true,
            api_key_validation: crate::config::ApiKeyValidation::Database,
            admin_api_key: None,
        };

        let auth_manager = AuthManager::new(&config).await.unwrap();

        // Test API key extraction
        let auth_request = auth_manager
            .extract_auth_from_headers(None, Some("test-api-key"));

        match auth_request {
            AuthRequest::ApiKey(key) => assert_eq!(key, "test-api-key"),
            _ => panic!("Expected API key authentication"),
        }

        // Test JWT extraction
        let auth_request = auth_manager
            .extract_auth_from_headers(Some("Bearer test-jwt-token"), None);

        match auth_request {
            AuthRequest::JwtToken(token) => assert_eq!(token, "test-jwt-token"),
            _ => panic!("Expected JWT authentication"),
        }

        // Test anonymous
        let auth_request = auth_manager
            .extract_auth_from_headers(None, None);
        assert_eq!(auth_request, AuthRequest::Anonymous);
    }
}
