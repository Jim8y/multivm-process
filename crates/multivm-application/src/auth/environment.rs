//! Environment-based API key validation
//!
//! This module provides production-ready API key validation using environment variables
//! for secure, containerized deployments.

use super::api_key::{ApiKeyInfo, ApiKeyMetadata, UsageStats};
use super::jwt::UserRole;
use super::permissions::Permission;
use crate::error::{ApplicationError, AuthResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::env;

/// Environment-based API key configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentApiKey {
    /// API key value (will be hashed for security)
    pub key: String,
    /// User ID associated with this key
    pub user_id: String,
    /// Key name/description
    pub name: String,
    /// User role
    pub role: UserRole,
    /// Permissions granted to this key
    pub permissions: Vec<Permission>,
    /// Rate limit (requests per minute)
    pub rate_limit: Option<u64>,
    /// Contact email
    pub contact_email: Option<String>,
    /// IP address whitelist (comma-separated)
    pub ip_whitelist: Option<String>,
}

/// Environment API key manager
#[derive(Debug)]
pub struct EnvironmentApiKeyManager {
    /// Loaded API keys from environment
    keys: HashMap<String, ApiKeyInfo>,
}

impl EnvironmentApiKeyManager {
    /// Create a new environment API key manager and load keys from environment
    pub fn new() -> AuthResult<Self> {
        let mut manager = Self {
            keys: HashMap::new(),
        };

        manager.load_keys_from_environment()?;
        Ok(manager)
    }

    /// Load API keys from environment variables
    fn load_keys_from_environment(&mut self) -> AuthResult<()> {
        // Load admin API key if specified
        if let Ok(admin_key) = env::var("MULTIVM_ADMIN_API_KEY") {
            let admin_key_info = self.create_admin_key(&admin_key)?;
            let key_hash = self.hash_api_key(&admin_key);
            self.keys.insert(key_hash, admin_key_info);
        }

        // Load user API keys from environment
        // Format: MULTIVM_API_KEY_<NAME>=key:user_id:role:permissions:rate_limit:email:ip_whitelist
        for (key, value) in env::vars() {
            if key.starts_with("MULTIVM_API_KEY_") && key != "MULTIVM_ADMIN_API_KEY" {
                let key_name = key.strip_prefix("MULTIVM_API_KEY_").unwrap_or("unknown");

                match self.parse_api_key_definition(&value, key_name) {
                    Ok(api_key_info) => {
                        let key_hash = self.hash_api_key(&api_key_info.key);
                        let api_key_info = self.env_key_to_api_key_info(api_key_info)?;
                        self.keys.insert(key_hash, api_key_info);
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to parse API key {}: {}", key, e);
                        continue;
                    }
                }
            }
        }

        Ok(())
    }

    /// Create admin API key info
    fn create_admin_key(&self, api_key: &str) -> AuthResult<ApiKeyInfo> {
        Ok(ApiKeyInfo {
            key_id: "admin".to_string(),
            key_hash: self.hash_api_key(api_key),
            user_id: "admin".to_string(),
            name: "Admin API Key".to_string(),
            created_at: Utc::now(),
            last_used: None,
            expires_at: None,
            is_active: true,
            rate_limit: 10000, // Higher limit for admin
            permissions: Permission::all(),
            role: UserRole::SuperAdmin,
            usage_stats: UsageStats::default(),
            metadata: ApiKeyMetadata {
                application_name: Some("MultiVM Admin".to_string()),
                contact_email: env::var("MULTIVM_ADMIN_EMAIL").ok(),
                ip_whitelist: env::var("MULTIVM_ADMIN_IP_WHITELIST")
                    .ok()
                    .map(|ips| ips.split(',').map(|s| s.trim().to_string()).collect())
                    .unwrap_or_default(),
                allowed_origins: vec!["*".to_string()],
                tags: [("type".to_string(), "admin".to_string())]
                    .iter()
                    .cloned()
                    .collect(),
            },
        })
    }

    /// Parse API key definition from environment variable value
    /// Format: key:user_id:role:permissions:rate_limit:email:ip_whitelist
    fn parse_api_key_definition(
        &self,
        definition: &str,
        name: &str,
    ) -> AuthResult<EnvironmentApiKey> {
        let parts: Vec<&str> = definition.split(':').collect();

        if parts.len() < 4 {
            return Err(ApplicationError::ConfigurationError {
                component: "environment".to_string(),
                message: format!(
                    "Invalid API key definition format for {}. Expected: key:user_id:role:permissions[:rate_limit[:email[:ip_whitelist]]]", 
                    name
                ),
            });
        }

        let key = parts[0].to_string();
        let user_id = parts[1].to_string();
        let role = self.parse_role(parts[2])?;
        let permissions = self.parse_permissions(parts[3])?;
        let rate_limit = parts.get(4).and_then(|s| s.parse().ok());
        let contact_email = parts
            .get(5)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        let ip_whitelist = parts
            .get(6)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        Ok(EnvironmentApiKey {
            key,
            user_id,
            name: name.to_string(),
            role,
            permissions,
            rate_limit,
            contact_email,
            ip_whitelist,
        })
    }

    /// Parse user role from string
    fn parse_role(&self, role_str: &str) -> AuthResult<UserRole> {
        match role_str.to_lowercase().as_str() {
            "superadmin" | "super_admin" => Ok(UserRole::SuperAdmin),
            "admin" => Ok(UserRole::Admin),
            "poweruser" | "power_user" => Ok(UserRole::PowerUser),
            "user" => Ok(UserRole::User),
            "guest" => Ok(UserRole::Guest),
            _ => Err(ApplicationError::ConfigurationError {
                component: "environment".to_string(),
                message: format!(
                    "Invalid role: {}. Valid roles: superadmin, admin, poweruser, user, guest",
                    role_str
                ),
            }),
        }
    }

    /// Parse permissions from comma-separated string
    fn parse_permissions(&self, permissions_str: &str) -> AuthResult<Vec<Permission>> {
        if permissions_str == "*" || permissions_str.to_lowercase() == "all" {
            return Ok(Permission::all());
        }

        let permission_names: Vec<&str> = permissions_str.split(',').collect();
        let mut permissions = Vec::new();

        for perm_name in permission_names {
            let perm_name = perm_name.trim();
            match perm_name.to_lowercase().as_str() {
                "read_system_status" => permissions.push(Permission::ReadSystemStatus),
                "read_network_info" => permissions.push(Permission::ReadNetworkInfo),
                "read_blockchain_data" => permissions.push(Permission::ReadBlockchainData),
                "write_transactions" => permissions.push(Permission::WriteTransactions),
                "manage_api_keys" => permissions.push(Permission::ManageApiKeys),
                "admin_operations" => permissions.push(Permission::AdminOperations),
                "manage_users" => permissions.push(Permission::ManageUsers),
                "system_configuration" => permissions.push(Permission::SystemConfiguration),
                "cross_vm_operations" => permissions.push(Permission::CrossVmOperations),
                "manage_consensus" => permissions.push(Permission::ManageConsensus),
                "manage_p2p" => permissions.push(Permission::ManageP2P),
                "read_metrics" => permissions.push(Permission::ReadMetrics),
                "write_configuration" => permissions.push(Permission::WriteConfiguration),
                _ => {
                    return Err(ApplicationError::ConfigurationError {
                        component: "environment".to_string(),
                        message: format!("Unknown permission: {}", perm_name),
                    });
                }
            }
        }

        Ok(permissions)
    }

    /// Convert environment API key to ApiKeyInfo
    fn env_key_to_api_key_info(&self, env_key: EnvironmentApiKey) -> AuthResult<ApiKeyInfo> {
        let ip_whitelist = env_key
            .ip_whitelist
            .map(|ips| ips.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default();

        Ok(ApiKeyInfo {
            key_id: format!("env_{}", env_key.name),
            key_hash: self.hash_api_key(&env_key.key),
            user_id: env_key.user_id,
            name: env_key.name,
            created_at: Utc::now(),
            last_used: None,
            expires_at: None, // Environment keys don't expire
            is_active: true,
            rate_limit: env_key.rate_limit.unwrap_or(1000),
            permissions: env_key.permissions,
            role: env_key.role,
            usage_stats: UsageStats::default(),
            metadata: ApiKeyMetadata {
                application_name: Some("Environment Key".to_string()),
                contact_email: env_key.contact_email,
                ip_whitelist,
                allowed_origins: vec!["*".to_string()],
                tags: [("source".to_string(), "environment".to_string())]
                    .iter()
                    .cloned()
                    .collect(),
            },
        })
    }

    /// Validate an API key
    pub fn validate_api_key(&self, api_key: &str) -> AuthResult<&ApiKeyInfo> {
        let key_hash = self.hash_api_key(api_key);

        self.keys
            .get(&key_hash)
            .ok_or_else(|| ApplicationError::AuthenticationFailed {
                reason: "Invalid API key".to_string(),
            })
    }

    /// Hash API key for secure storage
    fn hash_api_key(&self, api_key: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(api_key.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Get all loaded API keys (for admin purposes)
    pub fn get_loaded_keys(&self) -> Vec<&ApiKeyInfo> {
        self.keys.values().collect()
    }

    /// Check if any keys are loaded
    pub fn has_keys(&self) -> bool {
        !self.keys.is_empty()
    }
}

impl Default for EnvironmentApiKeyManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            keys: HashMap::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_parse_role() {
        let manager = EnvironmentApiKeyManager {
            keys: HashMap::new(),
        };

        assert_eq!(manager.parse_role("admin").unwrap(), UserRole::Admin);
        assert_eq!(manager.parse_role("ADMIN").unwrap(), UserRole::Admin);
        assert_eq!(
            manager.parse_role("superadmin").unwrap(),
            UserRole::SuperAdmin
        );
        assert_eq!(manager.parse_role("user").unwrap(), UserRole::User);
    }

    #[test]
    fn test_parse_permissions() {
        let manager = EnvironmentApiKeyManager {
            keys: HashMap::new(),
        };

        let perms = manager
            .parse_permissions("read_system_status,read_network_info")
            .unwrap();
        assert_eq!(perms.len(), 2);
        assert!(perms.contains(&Permission::ReadSystemStatus));
        assert!(perms.contains(&Permission::ReadNetworkInfo));

        let all_perms = manager.parse_permissions("*").unwrap();
        assert!(all_perms.len() > 5); // Should have many permissions
    }

    #[test]
    fn test_parse_api_key_definition() {
        let manager = EnvironmentApiKeyManager {
            keys: HashMap::new(),
        };

        let definition = "test_key_123:user1:admin:read_system_status,admin_operations:5000:admin@example.com:192.168.1.1,10.0.0.1";
        let parsed = manager
            .parse_api_key_definition(definition, "test_key")
            .unwrap();

        assert_eq!(parsed.key, "test_key_123");
        assert_eq!(parsed.user_id, "user1");
        assert_eq!(parsed.role, UserRole::Admin);
        assert_eq!(parsed.permissions.len(), 2);
        assert_eq!(parsed.rate_limit, Some(5000));
        assert_eq!(parsed.contact_email, Some("admin@example.com".to_string()));
        assert_eq!(
            parsed.ip_whitelist,
            Some("192.168.1.1,10.0.0.1".to_string())
        );
    }

    #[tokio::test]
    async fn test_environment_key_loading() {
        // Set up test environment variables
        env::set_var("MULTIVM_ADMIN_API_KEY", "admin_key_123");
        env::set_var(
            "MULTIVM_API_KEY_TEST",
            "test_key:testuser:user:read_system_status:1000",
        );

        let manager = EnvironmentApiKeyManager::new().unwrap();

        assert!(manager.has_keys());
        assert_eq!(manager.get_loaded_keys().len(), 2);

        // Test admin key validation
        let admin_key_info = manager.validate_api_key("admin_key_123").unwrap();
        assert_eq!(admin_key_info.role, UserRole::SuperAdmin);
        assert!(admin_key_info.permissions.len() > 5);

        // Test user key validation
        let user_key_info = manager.validate_api_key("test_key").unwrap();
        assert_eq!(user_key_info.user_id, "testuser");
        assert_eq!(user_key_info.role, UserRole::User);

        // Clean up test environment variables
        env::remove_var("MULTIVM_ADMIN_API_KEY");
        env::remove_var("MULTIVM_API_KEY_TEST");
    }
}
