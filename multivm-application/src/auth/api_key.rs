use super::jwt::UserRole;
use super::permissions::Permission;
use crate::error::{ApplicationError, AuthResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use uuid::Uuid;

/// API Key manager
#[derive(Debug)]
pub struct ApiKeyManager {
    keys: HashMap<String, ApiKeyInfo>,
    #[allow(dead_code)]
    storage_backend: ApiKeyStorage,
    environment_manager: Option<super::environment::EnvironmentApiKeyManager>,
    /// Rate limiting tracker
    rate_limiter: Arc<RwLock<RateLimiter>>,
}

/// Rate limiting tracker
#[derive(Debug, Default)]
struct RateLimiter {
    /// Tracks usage per key ID with sliding window
    usage_windows: HashMap<String, Vec<Instant>>,
}

/// Rate limit window configuration
struct RateLimitWindow {
    duration: Duration,
    max_requests: u32,
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
#[derive(Debug)]
pub enum ApiKeyStorage {
    Memory,
    Database,
    Redis,
    Environment,
}

impl ApiKeyManager {
    /// Create a new API key manager
    pub fn new(storage_backend: ApiKeyStorage) -> Self {
        let environment_manager = match storage_backend {
            ApiKeyStorage::Environment => {
                match super::environment::EnvironmentApiKeyManager::new() {
                    Ok(manager) => Some(manager),
                    Err(e) => {
                        eprintln!("Warning: Failed to load environment API keys: {}", e);
                        None
                    }
                }
            }
            _ => None,
        };

        Self {
            keys: HashMap::new(),
            storage_backend,
            environment_manager,
            rate_limiter: Arc::new(RwLock::new(RateLimiter::default())),
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
        // First check environment manager if available
        if let Some(env_manager) = &self.environment_manager {
            if let Ok(key_info) = env_manager.validate_api_key(api_key) {
                // Implement rate limiting for environment keys
                let (is_rate_limited, remaining_requests) =
                    self.check_rate_limit(&key_info.key_id).await;

                return Ok(ValidatedApiKey {
                    key_info: key_info.clone(),
                    is_expired: false, // Environment keys don't expire
                    is_rate_limited,
                    remaining_requests,
                });
            }
        }

        // Fall back to in-memory/database keys
        let key_hash = self.hash_api_key(api_key);

        for key_info in self.keys.values() {
            if key_info.key_hash == key_hash && key_info.is_active {
                let is_expired = key_info
                    .expires_at
                    .map(|exp| exp < Utc::now())
                    .unwrap_or(false);

                // Check rate limiting for regular API keys too
                let (is_rate_limited, remaining_requests) =
                    self.check_rate_limit(&key_info.key_id).await;

                return Ok(ValidatedApiKey {
                    key_info: key_info.clone(),
                    is_expired,
                    is_rate_limited,
                    remaining_requests,
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

    /// Check rate limiting for a key
    async fn check_rate_limit(&self, key_id: &str) -> (bool, u64) {
        let window = RateLimitWindow {
            duration: Duration::from_secs(60), // 1 minute window
            max_requests: 1000,                // Default limit
        };

        let mut rate_limiter = self.rate_limiter.write().await;
        let now = Instant::now();

        // Get or create usage window for this key
        let usage_times = rate_limiter
            .usage_windows
            .entry(key_id.to_string())
            .or_insert_with(Vec::new);

        // Remove old entries outside the window
        usage_times.retain(|&time| now.duration_since(time) <= window.duration);

        let current_count = usage_times.len() as u32;

        if current_count >= window.max_requests {
            // Rate limited
            (true, 0)
        } else {
            // Add current request
            usage_times.push(now);
            let remaining = window.max_requests - current_count - 1;
            (false, remaining as u64)
        }
    }

    /// Get rate limit information for a key
    pub async fn get_rate_limit_info(&self, key_id: &str) -> (u64, u64, Duration) {
        let window = RateLimitWindow {
            duration: Duration::from_secs(60),
            max_requests: 1000,
        };

        let rate_limiter = self.rate_limiter.read().await;
        let now = Instant::now();

        if let Some(usage_times) = rate_limiter.usage_windows.get(key_id) {
            let current_count = usage_times
                .iter()
                .filter(|&&time| now.duration_since(time) <= window.duration)
                .count() as u64;

            let remaining = window.max_requests.saturating_sub(current_count as u32) as u64;
            let reset_time = usage_times
                .first()
                .map(|&first_time| {
                    window
                        .duration
                        .saturating_sub(now.duration_since(first_time))
                })
                .unwrap_or(Duration::from_secs(0));

            (current_count, remaining, reset_time)
        } else {
            (0, window.max_requests as u64, Duration::from_secs(0))
        }
    }

    /// Reset rate limit for a key (admin function)
    pub async fn reset_rate_limit(&self, key_id: &str) {
        let mut rate_limiter = self.rate_limiter.write().await;
        rate_limiter.usage_windows.remove(key_id);
    }

    /// Clean up old rate limit entries
    pub async fn cleanup_rate_limits(&self) {
        let mut rate_limiter = self.rate_limiter.write().await;
        let now = Instant::now();
        let cleanup_duration = Duration::from_secs(3600); // 1 hour

        rate_limiter.usage_windows.retain(|_, usage_times| {
            usage_times.retain(|&time| now.duration_since(time) <= cleanup_duration);
            !usage_times.is_empty()
        });
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

