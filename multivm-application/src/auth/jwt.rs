use super::permissions::Permission;
use super::secret_manager::{JwtSecretManager, SecretManagerConfig};
use crate::error::{ApplicationError, AuthResult};
use chrono::{DateTime, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// JWT authentication handler with secret management
pub struct JwtAuth {
    secret_manager: Arc<JwtSecretManager>,
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

    /// Key ID used to sign this token
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_id: Option<String>,
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
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    /// Create a new JWT authentication handler with secret management
    pub fn new(
        secret_manager_config: SecretManagerConfig,
        expiration: Duration,
    ) -> AuthResult<Self> {
        let secret_manager = Arc::new(JwtSecretManager::new(secret_manager_config)?);

        Ok(Self {
            secret_manager,
            algorithm: Algorithm::HS256,
            expiration,
        })
    }

    /// Create a new JWT authentication handler with existing secret manager
    pub fn with_secret_manager(
        secret_manager: Arc<JwtSecretManager>,
        expiration: Duration,
    ) -> Self {
        Self {
            secret_manager,
            algorithm: Algorithm::HS256,
            expiration,
        }
    }

    /// Create a simple JWT handler (for backward compatibility)
    pub fn simple(secret: &str, expiration: Duration) -> AuthResult<Self> {
        use super::secret_manager::{StorageBackend, StorageConfig};

        if secret.len() < 32 {
            return Err(ApplicationError::ConfigurationError {
                component: "jwt".to_string(),
                message: "JWT secret must be at least 32 characters long".to_string(),
            });
        }

        // Set environment variable for simple mode
        std::env::set_var("MULTIVM_JWT_SECRET", secret);

        let config = SecretManagerConfig {
            storage_backend: StorageBackend::Environment,
            storage_config: StorageConfig::default(),
            ..Default::default()
        };

        Self::new(config, expiration)
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
                message: format!("Failed to get current time: {e}"),
            }
        })?;

        // Get current active secret
        let current_secret = self.secret_manager.get_current_secret()?;
        let encoding_key = EncodingKey::from_secret(current_secret.secret.as_bytes());

        let claims = TokenClaims {
            sub: user_id.to_string(),
            iat: now.as_secs(),
            exp: (now + self.expiration).as_secs(),
            jti: Uuid::new_v4().to_string(),
            iss: "multivm-application".to_string(),
            permissions,
            metadata,
            // Store the key ID in claims for validation
            key_id: Some(current_secret.id),
        };

        let header = Header::new(self.algorithm);

        encode(&header, &claims, &encoding_key).map_err(|e| {
            ApplicationError::AuthenticationFailed {
                reason: format!("Failed to encode JWT: {e}"),
            }
        })
    }

    /// Validate and decode a JWT token
    pub fn validate_token(&self, token: &str) -> AuthResult<ValidatedToken> {
        // First decode without verification to get key_id
        let token_data = decode::<TokenClaims>(
            token,
            &DecodingKey::from_secret(b"dummy"), // Dummy key for header extraction
            &Validation::new(self.algorithm),
        );

        // If decoding fails, try with any available secret (backward compatibility)
        let (claims, decoding_key) = if let Ok(data) = token_data {
            if let Some(key_id) = &data.claims.key_id {
                // Try to get the specific secret
                if let Some(secret) = self.secret_manager.get_secret(key_id)? {
                    let decoding_key = DecodingKey::from_secret(secret.secret.as_bytes());
                    (data.claims, decoding_key)
                } else {
                    return Err(ApplicationError::AuthenticationFailed {
                        reason: format!("Secret with key_id '{key_id}' not found"),
                    });
                }
            } else {
                // No key_id, try current secret (backward compatibility)
                let current_secret = self.secret_manager.get_current_secret()?;
                let decoding_key = DecodingKey::from_secret(current_secret.secret.as_bytes());
                (data.claims, decoding_key)
            }
        } else {
            // Failed to decode, try with current secret
            let current_secret = self.secret_manager.get_current_secret()?;
            let decoding_key = DecodingKey::from_secret(current_secret.secret.as_bytes());

            let mut validation = Validation::new(self.algorithm);
            validation.set_issuer(&["multivm-application"]);

            let token_data =
                decode::<TokenClaims>(token, &decoding_key, &validation).map_err(|e| {
                    ApplicationError::AuthenticationFailed {
                        reason: format!("Invalid JWT token: {e}"),
                    }
                })?;

            (token_data.claims, decoding_key)
        };

        // Now validate with the correct key
        let mut validation = Validation::new(self.algorithm);
        validation.set_issuer(&["multivm-application"]);

        let _validated_token =
            decode::<TokenClaims>(token, &decoding_key, &validation).map_err(|e| {
                ApplicationError::AuthenticationFailed {
                    reason: format!("Invalid JWT token: {e}"),
                }
            })?;

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

    /// Check if JWT secrets need rotation
    pub fn needs_rotation(&self) -> AuthResult<bool> {
        self.secret_manager.needs_rotation()
    }

    /// Rotate JWT secrets
    pub fn rotate_secrets(&self) -> AuthResult<()> {
        self.secret_manager.rotate_secret()?;
        Ok(())
    }

    /// Get secret management statistics
    pub fn get_secret_statistics(&self) -> AuthResult<super::secret_manager::SecretStatistics> {
        self.secret_manager.get_statistics()
    }

    /// Clean up expired secrets
    pub fn cleanup_expired_secrets(&self) -> AuthResult<u32> {
        self.secret_manager.cleanup_expired_secrets()
    }

    /// Get the secret manager (for advanced operations)
    pub fn secret_manager(&self) -> &Arc<JwtSecretManager> {
        &self.secret_manager
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
        DateTime::from_timestamp(self.iat as i64, 0).unwrap_or_else(Utc::now)
    }

    /// Get expiration time as DateTime
    pub fn expires_at(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.exp as i64, 0).unwrap_or_else(Utc::now)
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
