use super::jwt::UserRole;
use super::permissions::Permission;
use crate::error::{ApplicationError, AuthResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

/// API Key manager
#[derive(Debug)]
pub struct ApiKeyManager {
    keys: HashMap<String, ApiKeyInfo>,
}

/// API Key information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyInfo {
    /// Unique key ID
    pub key_id: String,

    /// Hashed API key (for security)
    pub key_hash: String,

    /// User ID associated with this key
    pub user_id: String,

    /// Key name/description
    pub name: String,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last used timestamp
    pub last_used: Option<DateTime<Utc>>,

    /// Expiration timestamp (optional)
    pub expires_at: Option<DateTime<Utc>>,

    /// Whether the key is active
    pub is_active: bool,

    /// Rate limit (requests per minute)
    pub rate_limit: u64,

    /// Permissions granted to this key
    pub permissions: Vec<Permission>,

    /// User role
    pub role: UserRole,

    /// Usage statistics
    pub usage_stats: UsageStats,

    /// Additional metadata
    pub metadata: ApiKeyMetadata,
}

/// Usage statistics for API keys
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageStats {
    /// Total requests made with this key
    pub total_requests: u64,

    /// Requests made today
    pub requests_today: u64,

    /// Requests made this month
    pub requests_this_month: u64,

    /// Last request timestamp
    pub last_request_at: Option<DateTime<Utc>>,

    /// Error count
    pub error_count: u64,
}

/// Additional metadata for API keys
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiKeyMetadata {
    /// Application or service name
    pub application_name: Option<String>,

    /// Contact email
    pub contact_email: Option<String>,

    /// IP address whitelist
    pub ip_whitelist: Vec<String>,

    /// Allowed origins for CORS
    pub allowed_origins: Vec<String>,

    /// Custom tags
    pub tags: HashMap<String, String>,
}

/// API Key creation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateApiKeyRequest {
    /// User ID
    pub user_id: String,

    /// Key name/description
    pub name: String,

    /// Expiration duration (optional)
    pub expires_in: Option<Duration>,

    /// Rate limit (requests per minute)
    pub rate_limit: Option<u64>,

    /// Permissions to grant
    pub permissions: Vec<Permission>,

    /// User role
    pub role: UserRole,

    /// Additional metadata
    pub metadata: Option<ApiKeyMetadata>,
}

/// API Key creation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateApiKeyResponse {
    /// Key ID
    pub key_id: String,

    /// The actual API key (only returned once!)
    pub api_key: String,

    /// Key information
    pub key_info: ApiKeyInfo,
}

/// API Key validation result
#[derive(Debug, Clone)]
pub struct ValidatedApiKey {
    pub key_info: ApiKeyInfo,
    pub is_expired: bool,
    pub is_rate_limited: bool,
    pub remaining_requests: u64,
}

/// Storage backend for API keys
pub enum ApiKeyStorage {
    Memory,
    Database,
    Redis,
}

impl ApiKeyManager {
    /// Create a new API key manager
    pub fn new(_storage_backend: ApiKeyStorage) -> Self {
        Self {
            keys: HashMap::new(),
        }
    }

    /// Create a new API key
    pub async fn create_api_key(
        &mut self,
        request: CreateApiKeyRequest,
    ) -> AuthResult<CreateApiKeyResponse> {
        let key_id = Uuid::new_v4().to_string();
        let api_key = format!("mvapp_{}", Uuid::new_v4().to_string().replace('-', ""));
        let key_hash = self.hash_api_key(&api_key);

        let key_info = ApiKeyInfo {
            key_id: key_id.clone(),
            key_hash,
            user_id: request.user_id,
            name: request.name,
            created_at: Utc::now(),
            last_used: None,
            expires_at: request
                .expires_in
                .map(|d| Utc::now() + chrono::Duration::from_std(d).unwrap_or_default()),
            is_active: true,
            rate_limit: request.rate_limit.unwrap_or(1000),
            permissions: request.permissions,
            role: request.role,
            usage_stats: UsageStats::default(),
            metadata: request.metadata.unwrap_or_default(),
        };

        self.keys.insert(key_id.clone(), key_info.clone());

        Ok(CreateApiKeyResponse {
            key_id,
            api_key,
            key_info,
        })
    }

    /// Validate an API key
    pub async fn validate_api_key(&self, api_key: &str) -> AuthResult<ValidatedApiKey> {
        let key_hash = self.hash_api_key(api_key);

        for key_info in self.keys.values() {
            if key_info.key_hash == key_hash && key_info.is_active {
                let is_expired = key_info
                    .expires_at
                    .map(|exp| exp < Utc::now())
                    .unwrap_or(false);

                return Ok(ValidatedApiKey {
                    key_info: key_info.clone(),
                    is_expired,
                    is_rate_limited: false,
                    remaining_requests: key_info.rate_limit,
                });
            }
        }

        Err(ApplicationError::AuthenticationFailed {
            reason: "Invalid API key".to_string(),
        })
    }

    /// List API keys for a user
    pub async fn list_user_keys(&self, user_id: &str) -> AuthResult<Vec<ApiKeyInfo>> {
        let keys: Vec<ApiKeyInfo> = self
            .keys
            .values()
            .filter(|key| key.user_id == user_id)
            .cloned()
            .collect();
        Ok(keys)
    }

    /// Revoke an API key
    pub async fn revoke_api_key(&mut self, key_id: &str) -> AuthResult<()> {
        if let Some(key_info) = self.keys.get_mut(key_id) {
            key_info.is_active = false;
            Ok(())
        } else {
            Err(ApplicationError::ResourceNotFound {
                resource_type: "api_key".to_string(),
                identifier: key_id.to_string(),
            })
        }
    }

    /// Update usage statistics
    pub async fn update_usage_stats(&mut self, key_id: &str) -> AuthResult<()> {
        if let Some(key_info) = self.keys.get_mut(key_id) {
            key_info.usage_stats.total_requests += 1;
            key_info.last_used = Some(Utc::now());
        }
        Ok(())
    }

    fn hash_api_key(&self, api_key: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(api_key.as_bytes());
        hex::encode(hasher.finalize())
    }
}

impl ApiKeyInfo {
    /// Check if the API key is expired
    pub fn is_expired(&self) -> bool {
        self.expires_at.map(|exp| exp < Utc::now()).unwrap_or(false)
    }

    /// Get time until expiration
    pub fn expires_in(&self) -> Option<Duration> {
        self.expires_at.map(|exp| {
            let now = Utc::now();
            if exp > now {
                (exp - now).to_std().unwrap_or_default()
            } else {
                Duration::from_secs(0)
            }
        })
    }

    /// Check if key has specific permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission)
    }

    /// Check if IP address is whitelisted
    pub fn is_ip_allowed(&self, ip: &str) -> bool {
        if self.metadata.ip_whitelist.is_empty() {
            true // No whitelist means all IPs are allowed
        } else {
            self.metadata.ip_whitelist.contains(&ip.to_string())
        }
    }

    /// Check if origin is allowed
    pub fn is_origin_allowed(&self, origin: &str) -> bool {
        if self.metadata.allowed_origins.is_empty() {
            true // No restriction means all origins are allowed
        } else {
            self.metadata.allowed_origins.contains(&origin.to_string())
                || self.metadata.allowed_origins.contains(&"*".to_string())
        }
    }
}

// Add rand dependency for random key generation

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_api_key_creation() {
        let mut manager = ApiKeyManager::new(ApiKeyStorage::Memory);

        let request = CreateApiKeyRequest {
            user_id: "test-user".to_string(),
            name: "Test Key".to_string(),
            expires_in: Some(Duration::from_secs(3600)),
            rate_limit: Some(500),
            permissions: Permission::default_user(),
            role: UserRole::User,
            metadata: None,
        };

        let response = manager
            .create_api_key(request)
            .await
            .expect("Failed to create API key");

        assert!(response.api_key.starts_with("mvapp_"));
        assert_eq!(response.key_info.name, "Test Key");
        assert_eq!(response.key_info.rate_limit, 500);
    }

    #[tokio::test]
    async fn test_api_key_validation() {
        let mut manager = ApiKeyManager::new(ApiKeyStorage::Memory);

        let request = CreateApiKeyRequest {
            user_id: "test-user".to_string(),
            name: "Test Key".to_string(),
            expires_in: None,
            rate_limit: None,
            permissions: Permission::default_user(),
            role: UserRole::User,
            metadata: None,
        };

        let response = manager
            .create_api_key(request)
            .await
            .expect("Failed to create API key");

        let validated = manager
            .validate_api_key(&response.api_key)
            .await
            .expect("Failed to validate API key");

        assert!(!validated.is_expired);
        assert_eq!(validated.key_info.user_id, "test-user");
    }

    #[test]
    fn test_api_key_info_methods() {
        let key_info = ApiKeyInfo {
            key_id: "test".to_string(),
            key_hash: "hash".to_string(),
            user_id: "user".to_string(),
            name: "Test".to_string(),
            created_at: Utc::now(),
            last_used: None,
            expires_at: Some(Utc::now() + chrono::Duration::hours(1)),
            is_active: true,
            rate_limit: 1000,
            permissions: vec![Permission::ReadSvmAccounts],
            role: UserRole::User,
            usage_stats: UsageStats::default(),
            metadata: ApiKeyMetadata::default(),
        };

        assert!(!key_info.is_expired());
        assert!(key_info.has_permission(&Permission::ReadSvmAccounts));
        assert!(!key_info.has_permission(&Permission::AdminAccess));
        assert!(key_info.is_ip_allowed("127.0.0.1")); // No whitelist
    }
}
