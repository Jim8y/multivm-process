//! JWT Secret Management with Key Rotation and Secure Storage
//!
//! This module provides secure management of JWT signing secrets including:
//! - Cryptographically secure key generation
//! - Key rotation capabilities
//! - Secure storage and retrieval
//! - Multiple key support for seamless rotation

use crate::error::{ApplicationError, AuthResult};
use chrono::{DateTime, Duration, Utc};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

/// JWT Secret manager for secure key handling and rotation
pub struct JwtSecretManager {
    /// Storage backend for secrets
    storage: Arc<dyn SecretStorage + Send + Sync>,
    /// Current active key ID
    current_key_id: Arc<RwLock<String>>,
    /// Cached secrets for performance
    key_cache: Arc<RwLock<HashMap<String, JwtSecret>>>,
    /// Configuration
    config: SecretManagerConfig,
}

/// Configuration for JWT secret management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretManagerConfig {
    /// How often to rotate keys (in hours)
    pub rotation_interval_hours: u32,
    /// How long to keep old keys for token validation (in hours)
    pub key_retention_hours: u32,
    /// Minimum secret length in bytes
    pub min_secret_length: usize,
    /// Storage backend type
    pub storage_backend: StorageBackend,
    /// Storage configuration
    pub storage_config: StorageConfig,
}

/// Storage backend options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageBackend {
    /// File-based storage (for development)
    File,
    /// Environment variables
    Environment,
    /// HashiCorp Vault
    Vault,
    /// AWS Secrets Manager
    AwsSecretsManager,
    /// Azure Key Vault
    AzureKeyVault,
}

/// Storage backend configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// File path for file storage
    pub file_path: Option<String>,
    /// Vault configuration
    pub vault_config: Option<VaultConfig>,
    /// AWS configuration
    pub aws_config: Option<AwsConfig>,
    /// Azure configuration
    pub azure_config: Option<AzureConfig>,
}

/// Vault configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    pub address: String,
    pub token: String,
    pub secret_path: String,
    pub mount_point: String,
}

/// AWS Secrets Manager configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsConfig {
    pub region: String,
    pub secret_name: String,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
}

/// Azure Key Vault configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureConfig {
    pub vault_url: String,
    pub secret_name: String,
    pub tenant_id: String,
    pub client_id: String,
    pub client_secret: Option<String>,
}

/// A JWT secret with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtSecret {
    /// Unique identifier for this secret
    pub id: String,
    /// The actual secret value
    pub secret: String,
    /// When this secret was created
    pub created_at: DateTime<Utc>,
    /// When this secret should be rotated
    pub rotate_at: DateTime<Utc>,
    /// When this secret expires and should be deleted
    pub expires_at: DateTime<Utc>,
    /// Whether this is the current active secret
    pub is_active: bool,
    /// Algorithm this secret is intended for
    pub algorithm: String,
    /// Additional metadata
    pub metadata: SecretMetadata,
}

/// Additional secret metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretMetadata {
    /// Who/what created this secret
    pub created_by: String,
    /// Environment this secret is for
    pub environment: String,
    /// Version number
    pub version: u32,
    /// Tags for organization
    pub tags: Vec<String>,
}

/// Trait for secret storage backends
pub trait SecretStorage {
    /// Store a secret
    fn store_secret(&self, secret: &JwtSecret) -> AuthResult<()>;

    /// Retrieve a secret by ID
    fn get_secret(&self, id: &str) -> AuthResult<Option<JwtSecret>>;

    /// List all secrets
    fn list_secrets(&self) -> AuthResult<Vec<JwtSecret>>;

    /// Delete a secret
    fn delete_secret(&self, id: &str) -> AuthResult<()>;

    /// Get the current active secret
    fn get_active_secret(&self) -> AuthResult<Option<JwtSecret>>;

    /// Set a secret as active
    fn set_active_secret(&self, id: &str) -> AuthResult<()>;
}

impl JwtSecretManager {
    /// Create a new JWT secret manager
    pub fn new(config: SecretManagerConfig) -> AuthResult<Self> {
        let storage = Self::create_storage(&config)?;

        let manager = Self {
            current_key_id: Arc::new(RwLock::new(String::new())),
            key_cache: Arc::new(RwLock::new(HashMap::new())),
            storage,
            config,
        };

        // Initialize with current secret or create one
        manager.initialize()?;

        Ok(manager)
    }

    /// Initialize the secret manager
    fn initialize(&self) -> AuthResult<()> {
        // Try to get current active secret
        if let Some(active_secret) = self.storage.get_active_secret()? {
            let mut current_key_id = self.current_key_id.write().unwrap();
            *current_key_id = active_secret.id.clone();

            let mut cache = self.key_cache.write().unwrap();
            cache.insert(active_secret.id.clone(), active_secret);
        } else {
            // No active secret exists, create one
            self.generate_new_secret(true)?;
        }

        // Load all existing secrets into cache
        self.refresh_cache()?;

        Ok(())
    }

    /// Generate a new JWT secret
    pub fn generate_new_secret(&self, set_as_active: bool) -> AuthResult<JwtSecret> {
        let secret_value = self.generate_secure_secret()?;
        let now = Utc::now();

        let secret = JwtSecret {
            id: Uuid::new_v4().to_string(),
            secret: secret_value,
            created_at: now,
            rotate_at: now + Duration::hours(self.config.rotation_interval_hours as i64),
            expires_at: now + Duration::hours(self.config.key_retention_hours as i64),
            is_active: set_as_active,
            algorithm: "HS256".to_string(),
            metadata: SecretMetadata {
                created_by: "jwt_secret_manager".to_string(),
                environment: std::env::var("RUST_ENV")
                    .unwrap_or_else(|_| "development".to_string()),
                version: 1,
                tags: vec!["jwt".to_string(), "application".to_string()],
            },
        };

        // Store the secret
        self.storage.store_secret(&secret)?;

        if set_as_active {
            self.storage.set_active_secret(&secret.id)?;
            let mut current_key_id = self.current_key_id.write().unwrap();
            *current_key_id = secret.id.clone();
        }

        // Update cache
        let mut cache = self.key_cache.write().unwrap();
        cache.insert(secret.id.clone(), secret.clone());

        Ok(secret)
    }

    /// Get the current active secret
    pub fn get_current_secret(&self) -> AuthResult<JwtSecret> {
        let current_key_id = self.current_key_id.read().unwrap();

        if current_key_id.is_empty() {
            return Err(ApplicationError::ConfigurationError {
                component: "jwt_secret_manager".to_string(),
                message: "No active JWT secret found".to_string(),
            });
        }

        // Try cache first
        {
            let cache = self.key_cache.read().unwrap();
            if let Some(secret) = cache.get(&*current_key_id) {
                return Ok(secret.clone());
            }
        }

        // Fall back to storage
        if let Some(secret) = self.storage.get_secret(&current_key_id)? {
            // Update cache
            let mut cache = self.key_cache.write().unwrap();
            cache.insert(secret.id.clone(), secret.clone());
            Ok(secret)
        } else {
            Err(ApplicationError::ConfigurationError {
                component: "jwt_secret_manager".to_string(),
                message: format!("Active JWT secret '{current_key_id}' not found"),
            })
        }
    }

    /// Get a secret by ID (for token validation)
    pub fn get_secret(&self, id: &str) -> AuthResult<Option<JwtSecret>> {
        // Try cache first
        {
            let cache = self.key_cache.read().unwrap();
            if let Some(secret) = cache.get(id) {
                return Ok(Some(secret.clone()));
            }
        }

        // Fall back to storage
        let secret = self.storage.get_secret(id)?;

        // Update cache if found
        if let Some(ref secret) = secret {
            let mut cache = self.key_cache.write().unwrap();
            cache.insert(secret.id.clone(), secret.clone());
        }

        Ok(secret)
    }

    /// Rotate the JWT secret (generate new and deactivate old)
    pub fn rotate_secret(&self) -> AuthResult<JwtSecret> {
        // Generate new secret
        let new_secret = self.generate_new_secret(true)?;

        // Clean up expired secrets
        self.cleanup_expired_secrets()?;

        Ok(new_secret)
    }

    /// Check if current secret needs rotation
    pub fn needs_rotation(&self) -> AuthResult<bool> {
        let current_secret = self.get_current_secret()?;
        Ok(Utc::now() >= current_secret.rotate_at)
    }

    /// Clean up expired secrets
    pub fn cleanup_expired_secrets(&self) -> AuthResult<u32> {
        let all_secrets = self.storage.list_secrets()?;
        let now = Utc::now();
        let mut deleted_count = 0;

        for secret in all_secrets {
            if now >= secret.expires_at && !secret.is_active {
                self.storage.delete_secret(&secret.id)?;

                // Remove from cache
                let mut cache = self.key_cache.write().unwrap();
                cache.remove(&secret.id);

                deleted_count += 1;
            }
        }

        Ok(deleted_count)
    }

    /// Refresh the secret cache
    pub fn refresh_cache(&self) -> AuthResult<()> {
        let all_secrets = self.storage.list_secrets()?;
        let mut cache = self.key_cache.write().unwrap();

        cache.clear();
        for secret in all_secrets {
            cache.insert(secret.id.clone(), secret);
        }

        Ok(())
    }

    /// Get secret statistics
    pub fn get_statistics(&self) -> AuthResult<SecretStatistics> {
        let all_secrets = self.storage.list_secrets()?;
        let now = Utc::now();

        let mut stats = SecretStatistics {
            total_secrets: all_secrets.len(),
            active_secrets: 0,
            expired_secrets: 0,
            secrets_needing_rotation: 0,
            oldest_secret_age: None,
            newest_secret_age: None,
        };

        let mut oldest_time = None;
        let mut newest_time = None;

        for secret in &all_secrets {
            if secret.is_active {
                stats.active_secrets += 1;
            }

            if now >= secret.expires_at {
                stats.expired_secrets += 1;
            }

            if now >= secret.rotate_at {
                stats.secrets_needing_rotation += 1;
            }

            // Track oldest and newest
            if oldest_time.is_none() || secret.created_at < oldest_time.unwrap() {
                oldest_time = Some(secret.created_at);
            }

            if newest_time.is_none() || secret.created_at > newest_time.unwrap() {
                newest_time = Some(secret.created_at);
            }
        }

        if let Some(oldest) = oldest_time {
            stats.oldest_secret_age = Some(now - oldest);
        }

        if let Some(newest) = newest_time {
            stats.newest_secret_age = Some(now - newest);
        }

        Ok(stats)
    }

    /// Generate a cryptographically secure secret
    fn generate_secure_secret(&self) -> AuthResult<String> {
        let length = self.config.min_secret_length.max(64); // Minimum 64 bytes
        let secret: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(length)
            .map(char::from)
            .collect();

        Ok(secret)
    }

    /// Create storage backend based on configuration
    fn create_storage(
        config: &SecretManagerConfig,
    ) -> AuthResult<Arc<dyn SecretStorage + Send + Sync>> {
        match config.storage_backend {
            StorageBackend::File => {
                let default_path = "jwt_secrets.json".to_string();
                let path = config
                    .storage_config
                    .file_path
                    .as_ref()
                    .unwrap_or(&default_path);
                Ok(Arc::new(FileSecretStorage::new(path)?))
            }
            StorageBackend::Environment => Ok(Arc::new(EnvironmentSecretStorage::new())),
            _ => Err(ApplicationError::ConfigurationError {
                component: "jwt_secret_manager".to_string(),
                message: format!(
                    "Storage backend {:?} not yet implemented",
                    config.storage_backend
                ),
            }),
        }
    }
}

/// Secret statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretStatistics {
    pub total_secrets: usize,
    pub active_secrets: usize,
    pub expired_secrets: usize,
    pub secrets_needing_rotation: usize,
    pub oldest_secret_age: Option<Duration>,
    pub newest_secret_age: Option<Duration>,
}

/// File-based secret storage (for development/testing)
pub struct FileSecretStorage {
    file_path: PathBuf,
}

impl FileSecretStorage {
    pub fn new(path: &str) -> AuthResult<Self> {
        Ok(Self {
            file_path: PathBuf::from(path),
        })
    }

    fn load_secrets(&self) -> AuthResult<HashMap<String, JwtSecret>> {
        if !self.file_path.exists() {
            return Ok(HashMap::new());
        }

        let content =
            fs::read_to_string(&self.file_path).map_err(|e| ApplicationError::InternalError {
                component: "file_secret_storage".to_string(),
                message: format!("Failed to read secrets file: {e}"),
            })?;

        let secrets: HashMap<String, JwtSecret> =
            serde_json::from_str(&content).map_err(|e| ApplicationError::InternalError {
                component: "file_secret_storage".to_string(),
                message: format!("Failed to parse secrets file: {e}"),
            })?;

        Ok(secrets)
    }

    fn save_secrets(&self, secrets: &HashMap<String, JwtSecret>) -> AuthResult<()> {
        // Create directory if it doesn't exist
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| ApplicationError::InternalError {
                component: "file_secret_storage".to_string(),
                message: format!("Failed to create secrets directory: {e}"),
            })?;
        }

        let content =
            serde_json::to_string_pretty(secrets).map_err(|e| ApplicationError::InternalError {
                component: "file_secret_storage".to_string(),
                message: format!("Failed to serialize secrets: {e}"),
            })?;

        fs::write(&self.file_path, content).map_err(|e| ApplicationError::InternalError {
            component: "file_secret_storage".to_string(),
            message: format!("Failed to write secrets file: {e}"),
        })?;

        Ok(())
    }
}

impl SecretStorage for FileSecretStorage {
    fn store_secret(&self, secret: &JwtSecret) -> AuthResult<()> {
        let mut secrets = self.load_secrets()?;
        secrets.insert(secret.id.clone(), secret.clone());
        self.save_secrets(&secrets)
    }

    fn get_secret(&self, id: &str) -> AuthResult<Option<JwtSecret>> {
        let secrets = self.load_secrets()?;
        Ok(secrets.get(id).cloned())
    }

    fn list_secrets(&self) -> AuthResult<Vec<JwtSecret>> {
        let secrets = self.load_secrets()?;
        Ok(secrets.into_values().collect())
    }

    fn delete_secret(&self, id: &str) -> AuthResult<()> {
        let mut secrets = self.load_secrets()?;
        secrets.remove(id);
        self.save_secrets(&secrets)
    }

    fn get_active_secret(&self) -> AuthResult<Option<JwtSecret>> {
        let secrets = self.load_secrets()?;
        Ok(secrets.values().find(|s| s.is_active).cloned())
    }

    fn set_active_secret(&self, id: &str) -> AuthResult<()> {
        let mut secrets = self.load_secrets()?;

        // Deactivate all secrets
        for secret in secrets.values_mut() {
            secret.is_active = false;
        }

        // Activate the specified secret
        if let Some(secret) = secrets.get_mut(id) {
            secret.is_active = true;
        } else {
            return Err(ApplicationError::ConfigurationError {
                component: "file_secret_storage".to_string(),
                message: format!("Secret '{id}' not found"),
            });
        }

        self.save_secrets(&secrets)
    }
}

/// Environment-based secret storage (reads from environment variables)
pub struct EnvironmentSecretStorage;

impl EnvironmentSecretStorage {
    pub fn new() -> Self {
        Self
    }
}

impl SecretStorage for EnvironmentSecretStorage {
    fn store_secret(&self, _secret: &JwtSecret) -> AuthResult<()> {
        Err(ApplicationError::ConfigurationError {
            component: "environment_secret_storage".to_string(),
            message: "Environment storage is read-only".to_string(),
        })
    }

    fn get_secret(&self, id: &str) -> AuthResult<Option<JwtSecret>> {
        // For environment storage, we use a simple mapping
        if id == "env_jwt_secret" {
            if let Ok(secret_value) = std::env::var("MULTIVM_JWT_SECRET") {
                let now = Utc::now();
                return Ok(Some(JwtSecret {
                    id: "env_jwt_secret".to_string(),
                    secret: secret_value,
                    created_at: now,
                    rotate_at: now + Duration::hours(24),
                    expires_at: now + Duration::hours(48),
                    is_active: true,
                    algorithm: "HS256".to_string(),
                    metadata: SecretMetadata {
                        created_by: "environment".to_string(),
                        environment: "production".to_string(),
                        version: 1,
                        tags: vec!["jwt".to_string(), "environment".to_string()],
                    },
                }));
            }
        }
        Ok(None)
    }

    fn list_secrets(&self) -> AuthResult<Vec<JwtSecret>> {
        if let Some(secret) = self.get_secret("env_jwt_secret")? {
            Ok(vec![secret])
        } else {
            Ok(vec![])
        }
    }

    fn delete_secret(&self, _id: &str) -> AuthResult<()> {
        Err(ApplicationError::ConfigurationError {
            component: "environment_secret_storage".to_string(),
            message: "Environment storage is read-only".to_string(),
        })
    }

    fn get_active_secret(&self) -> AuthResult<Option<JwtSecret>> {
        self.get_secret("env_jwt_secret")
    }

    fn set_active_secret(&self, _id: &str) -> AuthResult<()> {
        Err(ApplicationError::ConfigurationError {
            component: "environment_secret_storage".to_string(),
            message: "Environment storage is read-only".to_string(),
        })
    }
}

impl Default for SecretManagerConfig {
    fn default() -> Self {
        Self {
            rotation_interval_hours: 24 * 7, // Weekly rotation
            key_retention_hours: 24 * 30,    // Keep for 30 days
            min_secret_length: 64,
            storage_backend: StorageBackend::File,
            storage_config: StorageConfig {
                file_path: Some("jwt_secrets.json".to_string()),
                vault_config: None,
                aws_config: None,
                azure_config: None,
            },
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            file_path: Some("jwt_secrets.json".to_string()),
            vault_config: None,
            aws_config: None,
            azure_config: None,
        }
    }
}
