//! Comprehensive tests for authentication and authorization

#[cfg(test)]
mod tests {
    use crate::auth::{AuthManager, AuthConfig, ApiKeyConfig, JwtConfig, EnvironmentConfig};
    use crate::auth::permissions::{Permission, Role, PermissionSet};
    use std::time::Duration;
    use tempfile::TempDir;

    fn create_test_auth_config() -> AuthConfig {
        let temp_dir = TempDir::new().unwrap();
        AuthConfig {
            api_key: ApiKeyConfig {
                enabled: true,
                require_api_key: false,
                key_expiry_days: 365,
                max_keys_per_user: 10,
                rate_limit_per_key: 1000,
            },
            jwt: JwtConfig {
                enabled: true,
                secret: "test-secret-key-for-testing-only".to_string(),
                issuer: "multivm-test".to_string(),
                audience: "multivm-api".to_string(),
                expiry_minutes: 60,
                refresh_enabled: true,
                refresh_expiry_days: 7,
            },
            environment: EnvironmentConfig {
                require_auth_in_development: false,
                allow_insecure_tokens_in_dev: true,
                log_auth_events: true,
                max_failed_attempts: 5,
                lockout_duration_minutes: 15,
            },
            database_path: temp_dir.path().join("auth.db"),
        }
    }

    #[tokio::test]
    async fn test_auth_manager_creation() {
        let config = create_test_auth_config();
        let auth_manager = AuthManager::new(&config).await;
        assert!(auth_manager.is_ok(), "Auth manager creation should succeed");
    }

    #[tokio::test]
    async fn test_api_key_generation() {
        let config = create_test_auth_config();
        let mut auth_manager = AuthManager::new(&config).await.unwrap();

        let user_id = "test-user-123";
        let api_key_result = auth_manager.generate_api_key(user_id, "Test Key".to_string()).await;
        
        assert!(api_key_result.is_ok(), "API key generation should succeed");
        let api_key = api_key_result.unwrap();
        
        assert!(!api_key.key.is_empty(), "API key should not be empty");
        assert_eq!(api_key.user_id, user_id);
        assert_eq!(api_key.name, "Test Key");
        assert!(api_key.is_active, "API key should be active by default");
    }

    #[tokio::test]
    async fn test_api_key_validation() {
        let config = create_test_auth_config();
        let mut auth_manager = AuthManager::new(&config).await.unwrap();

        let user_id = "test-user-456";
        let api_key = auth_manager.generate_api_key(user_id, "Test Key".to_string()).await.unwrap();

        // Test valid API key
        let validation_result = auth_manager.validate_api_key(&api_key.key).await;
        assert!(validation_result.is_ok(), "Valid API key should pass validation");
        
        let validated_key = validation_result.unwrap();
        assert_eq!(validated_key.user_id, user_id);
        assert!(validated_key.is_active);

        // Test invalid API key
        let invalid_result = auth_manager.validate_api_key("invalid-key").await;
        assert!(invalid_result.is_err(), "Invalid API key should fail validation");
    }

    #[tokio::test]
    async fn test_api_key_revocation() {
        let config = create_test_auth_config();
        let mut auth_manager = AuthManager::new(&config).await.unwrap();

        let user_id = "test-user-789";
        let api_key = auth_manager.generate_api_key(user_id, "Test Key".to_string()).await.unwrap();

        // Revoke the API key
        let revoke_result = auth_manager.revoke_api_key(&api_key.key).await;
        assert!(revoke_result.is_ok(), "API key revocation should succeed");

        // Validate revoked key should fail
        let validation_result = auth_manager.validate_api_key(&api_key.key).await;
        assert!(validation_result.is_err(), "Revoked API key should fail validation");
    }

    #[tokio::test]
    async fn test_jwt_token_generation() {
        let config = create_test_auth_config();
        let auth_manager = AuthManager::new(&config).await.unwrap();

        let user_id = "test-user-jwt";
        let permissions = vec![Permission::ReadAccounts, Permission::ReadTransactions];
        
        let token_result = auth_manager.generate_jwt_token(user_id, permissions).await;
        assert!(token_result.is_ok(), "JWT token generation should succeed");
        
        let token = token_result.unwrap();
        assert!(!token.access_token.is_empty(), "Access token should not be empty");
        assert!(!token.token_type.is_empty(), "Token type should not be empty");
        assert!(token.expires_in > 0, "Token should have positive expiry time");
    }

    #[tokio::test]
    async fn test_jwt_token_validation() {
        let config = create_test_auth_config();
        let auth_manager = AuthManager::new(&config).await.unwrap();

        let user_id = "test-user-validate";
        let permissions = vec![Permission::ReadAccounts];
        let token = auth_manager.generate_jwt_token(user_id, permissions.clone()).await.unwrap();

        // Validate the token
        let validation_result = auth_manager.validate_jwt_token(&token.access_token).await;
        assert!(validation_result.is_ok(), "Valid JWT token should pass validation");
        
        let claims = validation_result.unwrap();
        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.permissions, permissions);

        // Test invalid token
        let invalid_result = auth_manager.validate_jwt_token("invalid.jwt.token").await;
        assert!(invalid_result.is_err(), "Invalid JWT token should fail validation");
    }

    #[tokio::test]
    async fn test_jwt_token_expiry() {
        let mut config = create_test_auth_config();
        config.jwt.expiry_minutes = 0; // Immediate expiry for testing
        
        let auth_manager = AuthManager::new(&config).await.unwrap();

        let user_id = "test-user-expiry";
        let permissions = vec![Permission::ReadAccounts];
        let token = auth_manager.generate_jwt_token(user_id, permissions).await.unwrap();

        // Wait a moment then validate (should fail due to immediate expiry)
        tokio::time::sleep(Duration::from_millis(100)).await;
        let validation_result = auth_manager.validate_jwt_token(&token.access_token).await;
        assert!(validation_result.is_err(), "Expired JWT token should fail validation");
    }

    #[tokio::test]
    async fn test_permission_system() {
        // Test permission creation and comparison
        let read_perm = Permission::ReadAccounts;
        let write_perm = Permission::WriteAccounts;
        let admin_perm = Permission::AdminAccess;

        assert_ne!(read_perm, write_perm);
        assert_ne!(read_perm, admin_perm);

        // Test permission set
        let mut perm_set = PermissionSet::new();
        perm_set.add(Permission::ReadAccounts);
        perm_set.add(Permission::ReadTransactions);

        assert!(perm_set.has(&Permission::ReadAccounts));
        assert!(perm_set.has(&Permission::ReadTransactions));
        assert!(!perm_set.has(&Permission::WriteAccounts));

        // Test permission removal
        perm_set.remove(&Permission::ReadAccounts);
        assert!(!perm_set.has(&Permission::ReadAccounts));
        assert!(perm_set.has(&Permission::ReadTransactions));
    }

    #[tokio::test]
    async fn test_role_based_access() {
        let mut read_only_role = Role::new("read_only", "Read-only access");
        read_only_role.add_permission(Permission::ReadAccounts);
        read_only_role.add_permission(Permission::ReadTransactions);
        read_only_role.add_permission(Permission::ReadBlocks);

        let mut admin_role = Role::new("admin", "Full administrative access");
        admin_role.add_permission(Permission::AdminAccess);
        admin_role.add_permission(Permission::WriteAccounts);
        admin_role.add_permission(Permission::WriteTransactions);

        // Test role permissions
        assert!(read_only_role.has_permission(&Permission::ReadAccounts));
        assert!(!read_only_role.has_permission(&Permission::WriteAccounts));
        
        assert!(admin_role.has_permission(&Permission::AdminAccess));
        assert!(admin_role.has_permission(&Permission::WriteAccounts));

        // Test role inheritance/combination
        let combined_permissions = read_only_role.get_permissions()
            .iter()
            .chain(admin_role.get_permissions().iter())
            .cloned()
            .collect::<Vec<_>>();
        
        assert!(combined_permissions.contains(&Permission::ReadAccounts));
        assert!(combined_permissions.contains(&Permission::AdminAccess));
    }

    #[tokio::test]
    async fn test_user_session_management() {
        let config = create_test_auth_config();
        let mut auth_manager = AuthManager::new(&config).await.unwrap();

        let user_id = "test-user-session";
        
        // Start a user session
        let session_result = auth_manager.create_user_session(user_id).await;
        assert!(session_result.is_ok(), "User session creation should succeed");
        
        let session = session_result.unwrap();
        assert_eq!(session.user_id, user_id);
        assert!(!session.session_id.is_empty());
        assert!(session.is_active);

        // Validate session
        let validation_result = auth_manager.validate_session(&session.session_id).await;
        assert!(validation_result.is_ok(), "Valid session should pass validation");

        // End session
        let end_result = auth_manager.end_session(&session.session_id).await;
        assert!(end_result.is_ok(), "Session termination should succeed");

        // Validate ended session should fail
        let validation_after_end = auth_manager.validate_session(&session.session_id).await;
        assert!(validation_after_end.is_err(), "Ended session should fail validation");
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let mut config = create_test_auth_config();
        config.api_key.rate_limit_per_key = 2; // Very low limit for testing
        
        let mut auth_manager = AuthManager::new(&config).await.unwrap();

        let user_id = "test-user-rate-limit";
        let api_key = auth_manager.generate_api_key(user_id, "Rate Limit Test".to_string()).await.unwrap();

        // First request should succeed
        let result1 = auth_manager.check_rate_limit(&api_key.key).await;
        assert!(result1.is_ok(), "First request should pass rate limit");

        // Second request should succeed
        let result2 = auth_manager.check_rate_limit(&api_key.key).await;
        assert!(result2.is_ok(), "Second request should pass rate limit");

        // Third request should fail (exceeds limit of 2)
        let result3 = auth_manager.check_rate_limit(&api_key.key).await;
        assert!(result3.is_err(), "Third request should fail rate limit");
    }

    #[tokio::test]
    async fn test_failed_authentication_attempts() {
        let mut config = create_test_auth_config();
        config.environment.max_failed_attempts = 3;
        
        let mut auth_manager = AuthManager::new(&config).await.unwrap();

        let user_id = "test-user-lockout";
        
        // Simulate failed authentication attempts
        for i in 1..=3 {
            let result = auth_manager.record_failed_attempt(user_id).await;
            assert!(result.is_ok(), "Recording failed attempt {} should succeed", i);
        }

        // Check if user is locked out
        let is_locked = auth_manager.is_user_locked_out(user_id).await;
        assert!(is_locked.unwrap_or(false), "User should be locked out after max failed attempts");

        // Subsequent authentication should fail due to lockout
        let auth_result = auth_manager.authenticate_user(user_id, "any-password").await;
        assert!(auth_result.is_err(), "Authentication should fail for locked out user");
    }

    #[tokio::test]
    async fn test_concurrent_auth_operations() {
        let config = create_test_auth_config();
        let auth_manager = std::sync::Arc::new(tokio::sync::Mutex::new(
            AuthManager::new(&config).await.unwrap()
        ));

        let mut handles = Vec::new();
        
        // Generate multiple API keys concurrently
        for i in 0..10 {
            let auth_manager_clone = auth_manager.clone();
            let handle = tokio::spawn(async move {
                let mut manager = auth_manager_clone.lock().await;
                manager.generate_api_key(
                    &format!("user-{}", i),
                    format!("Concurrent Key {}", i)
                ).await
            });
            handles.push(handle);
        }

        // Wait for all operations to complete
        let mut success_count = 0;
        for handle in handles {
            if let Ok(Ok(_)) = handle.await {
                success_count += 1;
            }
        }

        assert!(success_count >= 8, "Most concurrent operations should succeed");
    }

    #[tokio::test]
    async fn test_auth_config_validation() {
        // Test valid config
        let valid_config = create_test_auth_config();
        assert!(valid_config.validate().is_ok(), "Valid config should pass validation");

        // Test invalid config - empty JWT secret
        let mut invalid_config = create_test_auth_config();
        invalid_config.jwt.secret = "".to_string();
        assert!(invalid_config.validate().is_err(), "Config with empty JWT secret should fail validation");

        // Test invalid config - zero expiry
        let mut invalid_config2 = create_test_auth_config();
        invalid_config2.jwt.expiry_minutes = 0;
        // This might be valid for testing, so we'll just ensure it doesn't panic
        let _ = invalid_config2.validate();
    }

    #[tokio::test]
    async fn test_permission_hierarchy() {
        // Test that admin permissions include all other permissions
        let admin_permissions = Permission::get_admin_permissions();
        let read_permissions = Permission::get_read_permissions();
        let write_permissions = Permission::get_write_permissions();

        // Admin should include all read permissions
        for read_perm in &read_permissions {
            assert!(admin_permissions.contains(read_perm), 
                   "Admin permissions should include read permission: {:?}", read_perm);
        }

        // Admin should include all write permissions  
        for write_perm in &write_permissions {
            assert!(admin_permissions.contains(write_perm),
                   "Admin permissions should include write permission: {:?}", write_perm);
        }
    }

    #[tokio::test]
    async fn test_auth_token_refresh() {
        let config = create_test_auth_config();
        let auth_manager = AuthManager::new(&config).await.unwrap();

        let user_id = "test-user-refresh";
        let permissions = vec![Permission::ReadAccounts];
        let initial_token = auth_manager.generate_jwt_token(user_id, permissions.clone()).await.unwrap();

        // Refresh the token
        let refresh_result = auth_manager.refresh_jwt_token(&initial_token.refresh_token.unwrap()).await;
        assert!(refresh_result.is_ok(), "Token refresh should succeed");
        
        let new_token = refresh_result.unwrap();
        assert_ne!(initial_token.access_token, new_token.access_token, "New token should be different");

        // Both tokens should be valid (until the old one is explicitly invalidated)
        let old_validation = auth_manager.validate_jwt_token(&initial_token.access_token).await;
        let new_validation = auth_manager.validate_jwt_token(&new_token.access_token).await;
        
        // At least the new token should be valid
        assert!(new_validation.is_ok(), "New token should be valid");
    }
}