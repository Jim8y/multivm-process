//! JWT Secret Rotation Service
//!
//! This module provides automated JWT secret rotation to enhance security.
//! Secrets are rotated based on configured intervals and old secrets are
//! retained for a grace period to allow validation of existing tokens.

use super::secret_manager::JwtSecretManager;
use crate::error::{ApplicationError, AuthResult};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// JWT secret rotation service
pub struct JwtRotationService {
    secret_manager: Arc<JwtSecretManager>,
    config: RotationServiceConfig,
    running: Arc<tokio::sync::RwLock<bool>>,
}

/// Configuration for the rotation service
#[derive(Debug, Clone)]
pub struct RotationServiceConfig {
    /// How often to check if rotation is needed
    pub check_interval: Duration,
    /// Whether to auto-rotate when needed
    pub auto_rotate: bool,
    /// Whether to auto-cleanup expired secrets
    pub auto_cleanup: bool,
    /// Maximum number of secrets to keep
    pub max_secrets: usize,
}

impl Default for RotationServiceConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(3600), // Check every hour
            auto_rotate: true,
            auto_cleanup: true,
            max_secrets: 10,
        }
    }
}

impl JwtRotationService {
    /// Create a new rotation service
    pub fn new(secret_manager: Arc<JwtSecretManager>, config: RotationServiceConfig) -> Self {
        Self {
            secret_manager,
            config,
            running: Arc::new(tokio::sync::RwLock::new(false)),
        }
    }

    /// Start the rotation service
    pub async fn start(&self) -> AuthResult<()> {
        let mut running = self.running.write().await;
        if *running {
            return Err(ApplicationError::ConfigurationError {
                component: "jwt_rotation_service".to_string(),
                message: "Rotation service is already running".to_string(),
            });
        }

        *running = true;
        drop(running);

        info!("Starting JWT secret rotation service");

        // Start the rotation task
        let secret_manager = Arc::clone(&self.secret_manager);
        let config = self.config.clone();
        let running_flag = Arc::clone(&self.running);

        tokio::spawn(async move {
            Self::rotation_task(secret_manager, config, running_flag).await;
        });

        Ok(())
    }

    /// Stop the rotation service
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        info!("Stopped JWT secret rotation service");
    }

    /// Check if the service is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Force rotation of secrets
    pub async fn force_rotation(&self) -> AuthResult<()> {
        info!("Forcing JWT secret rotation");
        let new_secret = self.secret_manager.rotate_secret()?;
        info!(
            "JWT secret rotated successfully. New secret ID: {}",
            new_secret.id
        );
        Ok(())
    }

    /// Force cleanup of expired secrets
    pub async fn force_cleanup(&self) -> AuthResult<u32> {
        info!("Forcing cleanup of expired JWT secrets");
        let deleted_count = self.secret_manager.cleanup_expired_secrets()?;
        info!("Cleaned up {} expired JWT secrets", deleted_count);
        Ok(deleted_count)
    }

    /// Get rotation statistics
    pub async fn get_statistics(&self) -> AuthResult<RotationStatistics> {
        let secret_stats = self.secret_manager.get_statistics()?;

        Ok(RotationStatistics {
            service_running: self.is_running().await,
            secret_statistics: secret_stats,
            config: self.config.clone(),
        })
    }

    /// Main rotation task
    async fn rotation_task(
        secret_manager: Arc<JwtSecretManager>,
        config: RotationServiceConfig,
        running: Arc<tokio::sync::RwLock<bool>>,
    ) {
        let mut interval = interval(config.check_interval);

        loop {
            interval.tick().await;

            // Check if we should stop
            {
                let is_running = running.read().await;
                if !*is_running {
                    debug!("JWT rotation service stopping");
                    break;
                }
            }

            // Check if rotation is needed
            if config.auto_rotate {
                match secret_manager.needs_rotation() {
                    Ok(true) => {
                        info!("JWT secret rotation needed, performing rotation");
                        match secret_manager.rotate_secret() {
                            Ok(new_secret) => {
                                info!(
                                    "JWT secret rotated successfully. New secret ID: {}",
                                    new_secret.id
                                );
                            }
                            Err(e) => {
                                error!("Failed to rotate JWT secret: {}", e);
                            }
                        }
                    }
                    Ok(false) => {
                        debug!("JWT secret rotation not needed");
                    }
                    Err(e) => {
                        error!("Failed to check if JWT secret rotation is needed: {}", e);
                    }
                }
            }

            // Cleanup expired secrets
            if config.auto_cleanup {
                match secret_manager.cleanup_expired_secrets() {
                    Ok(deleted_count) => {
                        if deleted_count > 0 {
                            info!("Cleaned up {} expired JWT secrets", deleted_count);
                        } else {
                            debug!("No expired JWT secrets to clean up");
                        }
                    }
                    Err(e) => {
                        error!("Failed to cleanup expired JWT secrets: {}", e);
                    }
                }
            }

            // Check secret count and warn if too many
            match secret_manager.get_statistics() {
                Ok(stats) => {
                    if stats.total_secrets > config.max_secrets {
                        warn!(
                            "Too many JWT secrets: {} > {}. Consider reducing retention period.",
                            stats.total_secrets, config.max_secrets
                        );
                    }

                    debug!(
                        "JWT secret statistics: {} total, {} active, {} expired, {} need rotation",
                        stats.total_secrets,
                        stats.active_secrets,
                        stats.expired_secrets,
                        stats.secrets_needing_rotation
                    );
                }
                Err(e) => {
                    error!("Failed to get JWT secret statistics: {}", e);
                }
            }
        }
    }
}

/// Rotation service statistics
#[derive(Debug, Clone)]
pub struct RotationStatistics {
    pub service_running: bool,
    pub secret_statistics: super::secret_manager::SecretStatistics,
    pub config: RotationServiceConfig,
}

/// JWT secret rotation scheduler
pub struct JwtRotationScheduler {
    rotation_service: JwtRotationService,
}

impl JwtRotationScheduler {
    /// Create a new rotation scheduler
    pub fn new(secret_manager: Arc<JwtSecretManager>, config: RotationServiceConfig) -> Self {
        Self {
            rotation_service: JwtRotationService::new(secret_manager, config),
        }
    }

    /// Start the scheduler
    pub async fn start(&self) -> AuthResult<()> {
        self.rotation_service.start().await
    }

    /// Stop the scheduler
    pub async fn stop(&self) {
        self.rotation_service.stop().await;
    }

    /// Schedule a one-time rotation
    pub async fn schedule_rotation(&self, delay: Duration) -> AuthResult<()> {
        let secret_manager = Arc::clone(&self.rotation_service.secret_manager);

        tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            match secret_manager.rotate_secret() {
                Ok(new_secret) => {
                    info!(
                        "Scheduled JWT secret rotated successfully. New secret ID: {}",
                        new_secret.id
                    );
                }
                Err(e) => {
                    error!("Scheduled JWT rotation failed: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Schedule periodic cleanup
    pub async fn schedule_cleanup(&self, interval: Duration) -> AuthResult<()> {
        let secret_manager = Arc::clone(&self.rotation_service.secret_manager);

        tokio::spawn(async move {
            let mut cleanup_interval = tokio::time::interval(interval);

            loop {
                cleanup_interval.tick().await;

                match secret_manager.cleanup_expired_secrets() {
                    Ok(deleted_count) => {
                        if deleted_count > 0 {
                            info!(
                                "Scheduled cleanup removed {} expired JWT secrets",
                                deleted_count
                            );
                        }
                    }
                    Err(e) => {
                        error!("Scheduled JWT cleanup failed: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Get scheduler statistics
    pub async fn get_statistics(&self) -> AuthResult<RotationStatistics> {
        self.rotation_service.get_statistics().await
    }
}

/// Builder for JWT rotation service configuration
pub struct RotationServiceBuilder {
    config: RotationServiceConfig,
}

impl RotationServiceBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: RotationServiceConfig::default(),
        }
    }

    /// Set check interval
    pub fn check_interval(mut self, interval: Duration) -> Self {
        self.config.check_interval = interval;
        self
    }

    /// Enable/disable auto rotation
    pub fn auto_rotate(mut self, enabled: bool) -> Self {
        self.config.auto_rotate = enabled;
        self
    }

    /// Enable/disable auto cleanup
    pub fn auto_cleanup(mut self, enabled: bool) -> Self {
        self.config.auto_cleanup = enabled;
        self
    }

    /// Set maximum number of secrets to keep
    pub fn max_secrets(mut self, max: usize) -> Self {
        self.config.max_secrets = max;
        self
    }

    /// Build the configuration
    pub fn build(self) -> RotationServiceConfig {
        self.config
    }
}

impl Default for RotationServiceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::super::secret_manager::{SecretManagerConfig, StorageBackend, StorageConfig};
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_rotation_service_creation() {
        let config = SecretManagerConfig {
            storage_backend: StorageBackend::File,
            storage_config: StorageConfig {
                file_path: Some("/tmp/test_jwt_secrets.json".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        let secret_manager = Arc::new(JwtSecretManager::new(config).unwrap());
        let rotation_config = RotationServiceConfig {
            check_interval: Duration::from_secs(1),
            auto_rotate: false,
            auto_cleanup: false,
            max_secrets: 5,
        };

        let service = JwtRotationService::new(secret_manager, rotation_config);
        assert!(!service.is_running().await);
    }

    #[tokio::test]
    async fn test_rotation_service_builder() {
        let config = RotationServiceBuilder::new()
            .check_interval(Duration::from_secs(30))
            .auto_rotate(true)
            .auto_cleanup(true)
            .max_secrets(15)
            .build();

        assert_eq!(config.check_interval, Duration::from_secs(30));
        assert!(config.auto_rotate);
        assert!(config.auto_cleanup);
        assert_eq!(config.max_secrets, 15);
    }
}
