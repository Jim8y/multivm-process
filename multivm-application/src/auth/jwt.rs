use super::permissions::Permission;
use crate::error::{ApplicationError, AuthResult};
use chrono::{DateTime, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// JWT authentication handler
pub struct JwtAuth {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    algorithm: Algorithm,
    expiration: Duration,
}

impl std::fmt::Debug for JwtAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtAuth")
            .field("algorithm", &self.algorithm)
            .field("expiration", &self.expiration)
            .finish()
    }
}

/// JWT token claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenClaims {
    /// Subject (user ID)
    pub sub: String,

    /// Issued at timestamp
    pub iat: u64,

    /// Expiration timestamp
    pub exp: u64,

    /// Token ID
    pub jti: String,

    /// Issuer
    pub iss: String,

    /// User permissions
    pub permissions: Vec<Permission>,

    /// Additional user metadata
    pub metadata: TokenMetadata,
}

/// Additional token metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenMetadata {
    /// User email (optional)
    pub email: Option<String>,

    /// User role
    pub role: UserRole,

    /// API key ID (if authenticated via API key)
    pub api_key_id: Option<String>,

    /// Client information
    pub client_info: Option<ClientInfo>,
}

/// User role enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum UserRole {
    Guest,
    User,
    PowerUser,
    Admin,
    SuperAdmin,
}

/// Client information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub application_name: Option<String>,
}

/// Token validation result
#[derive(Debug, Clone)]
pub struct ValidatedToken {
    pub claims: TokenClaims,
    pub is_expired: bool,
    pub expires_in: Duration,
}

impl JwtAuth {
    /// Create a new JWT authentication handler
    pub fn new(secret: &str, expiration: Duration) -> AuthResult<Self> {
        if secret.len() < 32 {
            return Err(ApplicationError::ConfigurationError {
                component: "jwt".to_string(),
                message: "JWT secret must be at least 32 characters long".to_string(),
            });
        }

        let encoding_key = EncodingKey::from_secret(secret.as_bytes());
        let decoding_key = DecodingKey::from_secret(secret.as_bytes());

        Ok(Self {
            encoding_key,
            decoding_key,
            algorithm: Algorithm::HS256,
            expiration,
        })
    }

    /// Generate a new JWT token
    pub fn generate_token(
        &self,
        user_id: &str,
        permissions: Vec<Permission>,
        metadata: TokenMetadata,
    ) -> AuthResult<String> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| {
            ApplicationError::InternalError {
                component: "jwt".to_string(),
                message: format!("Failed to get current time: {}", e),
            }
        })?;

        let claims = TokenClaims {
            sub: user_id.to_string(),
            iat: now.as_secs(),
            exp: (now + self.expiration).as_secs(),
            jti: Uuid::new_v4().to_string(),
            iss: "multivm-application".to_string(),
            permissions,
            metadata,
        };

        let header = Header::new(self.algorithm);

        encode(&header, &claims, &self.encoding_key).map_err(|e| {
            ApplicationError::AuthenticationFailed {
                reason: format!("Failed to encode JWT: {}", e),
            }
        })
    }

    /// Validate and decode a JWT token
    pub fn validate_token(&self, token: &str) -> AuthResult<ValidatedToken> {
        let mut validation = Validation::new(self.algorithm);
        validation.set_issuer(&["multivm-application"]);

        let token_data =
            decode::<TokenClaims>(token, &self.decoding_key, &validation).map_err(|e| {
                ApplicationError::AuthenticationFailed {
                    reason: format!("Invalid JWT token: {}", e),
                }
            })?;

        let claims = token_data.claims;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let is_expired = claims.exp <= now;
        let expires_in = if is_expired {
            Duration::from_secs(0)
        } else {
            Duration::from_secs(claims.exp - now)
        };

        Ok(ValidatedToken {
            claims,
            is_expired,
            expires_in,
        })
    }

    /// Refresh a token (generate new token with same claims but updated timestamps)
    pub fn refresh_token(&self, token: &str) -> AuthResult<String> {
        let validated = self.validate_token(token)?;

        if validated.is_expired {
            return Err(ApplicationError::AuthenticationFailed {
                reason: "Cannot refresh expired token".to_string(),
            });
        }

        // Generate new token with same permissions and metadata
        self.generate_token(
            &validated.claims.sub,
            validated.claims.permissions,
            validated.claims.metadata,
        )
    }

    /// Extract token from Authorization header
    pub fn extract_token_from_header(auth_header: &str) -> AuthResult<&str> {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            Ok(token)
        } else {
            Err(ApplicationError::AuthenticationFailed {
                reason: "Invalid Authorization header format. Expected 'Bearer <token>'"
                    .to_string(),
            })
        }
    }

    /// Create token for admin user
    pub fn create_admin_token(&self, user_id: &str) -> AuthResult<String> {
        let metadata = TokenMetadata {
            email: None,
            role: UserRole::Admin,
            api_key_id: None,
            client_info: None,
        };

        self.generate_token(user_id, Permission::admin(), metadata)
    }

    /// Create token for regular user
    pub fn create_user_token(&self, user_id: &str, email: Option<String>) -> AuthResult<String> {
        let metadata = TokenMetadata {
            email,
            role: UserRole::User,
            api_key_id: None,
            client_info: None,
        };

        self.generate_token(user_id, Permission::default_user(), metadata)
    }

    /// Create token for power user
    pub fn create_power_user_token(
        &self,
        user_id: &str,
        email: Option<String>,
    ) -> AuthResult<String> {
        let metadata = TokenMetadata {
            email,
            role: UserRole::PowerUser,
            api_key_id: None,
            client_info: None,
        };

        self.generate_token(user_id, Permission::power_user(), metadata)
    }
}

impl TokenClaims {
    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.exp <= now
    }

    /// Get time until expiration
    pub fn expires_in(&self) -> Duration {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if self.exp > now {
            Duration::from_secs(self.exp - now)
        } else {
            Duration::from_secs(0)
        }
    }

    /// Get issued at time as DateTime
    pub fn issued_at(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.iat as i64, 0).unwrap_or_else(|| Utc::now())
    }

    /// Get expiration time as DateTime
    pub fn expires_at(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.exp as i64, 0).unwrap_or_else(|| Utc::now())
    }

    /// Check if user has specific permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission)
    }

    /// Check if user is admin
    pub fn is_admin(&self) -> bool {
        self.metadata.role == UserRole::Admin
            || self.metadata.role == UserRole::SuperAdmin
            || self.has_permission(&Permission::AdminAccess)
    }
}

impl UserRole {
    /// Get default permissions for role
    pub fn default_permissions(&self) -> Vec<Permission> {
        match self {
            UserRole::Guest => vec![Permission::ReadSystemStatus, Permission::ReadNetworkInfo],
            UserRole::User => Permission::default_user(),
            UserRole::PowerUser => Permission::power_user(),
            UserRole::Admin | UserRole::SuperAdmin => Permission::admin(),
        }
    }

    /// Check if role has elevated privileges
    pub fn is_elevated(&self) -> bool {
        matches!(self, UserRole::Admin | UserRole::SuperAdmin)
    }

    /// Get role priority (higher number = more privileges)
    pub fn priority(&self) -> u8 {
        match self {
            UserRole::Guest => 0,
            UserRole::User => 1,
            UserRole::PowerUser => 2,
            UserRole::Admin => 3,
            UserRole::SuperAdmin => 4,
        }
    }
}

impl Default for TokenMetadata {
    fn default() -> Self {
        Self {
            email: None,
            role: UserRole::User,
            api_key_id: None,
            client_info: None,
        }
    }
}

impl Default for ClientInfo {
    fn default() -> Self {
        Self {
            ip_address: None,
            user_agent: None,
            application_name: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_jwt_creation_and_validation() {
        let jwt_auth = JwtAuth::new(
            "test-secret-key-that-is-long-enough",
            Duration::from_secs(3600),
        )
        .expect("Failed to create JWT auth");

        let metadata = TokenMetadata::default();
        let token = jwt_auth
            .generate_token("test-user", Permission::default_user(), metadata)
            .expect("Failed to generate token");

        let validated = jwt_auth
            .validate_token(&token)
            .expect("Failed to validate token");

        assert_eq!(validated.claims.sub, "test-user");
        assert!(!validated.is_expired);
    }

    #[test]
    fn test_token_claims_methods() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = TokenClaims {
            sub: "test".to_string(),
            iat: now,
            exp: now + 3600,
            jti: Uuid::new_v4().to_string(),
            iss: "test".to_string(),
            permissions: vec![Permission::ReadSvmAccounts],
            metadata: TokenMetadata::default(),
        };

        assert!(!claims.is_expired());
        assert!(claims.has_permission(&Permission::ReadSvmAccounts));
        assert!(!claims.has_permission(&Permission::AdminAccess));
    }

    #[test]
    fn test_user_role_permissions() {
        assert!(UserRole::Admin.is_elevated());
        assert!(!UserRole::User.is_elevated());
        assert!(UserRole::Admin.priority() > UserRole::User.priority());
    }

    #[test]
    fn test_header_extraction() {
        let auth_header = "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9";
        let token =
            JwtAuth::extract_token_from_header(auth_header).expect("Failed to extract token");

        assert_eq!(token, "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9");

        let invalid_header = "InvalidHeader";
        assert!(JwtAuth::extract_token_from_header(invalid_header).is_err());
    }
}
