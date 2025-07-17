//! Account binding policies and security features

use crate::error::{AccountMappingError, AccountMappingResult};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Policy configuration for account binding operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingPolicy {
    /// Maximum number of accounts that can be bound to a single MultiVM account
    pub max_accounts_per_multivm: u8,
    /// Cooldown period between binding operations
    pub binding_cooldown: Duration,
    /// Whether multi-signature is required for binding
    pub require_multi_sig: bool,
    /// Minimum age of account before it can be bound
    pub min_account_age: Duration,
    /// Maximum number of binding attempts per hour
    pub max_binding_attempts_per_hour: u32,
    /// Whether to require secondary authentication
    pub require_secondary_auth: bool,
    /// Time lock period for unbinding operations
    pub unbinding_timelock: Duration,
}

impl Default for BindingPolicy {
    fn default() -> Self {
        Self {
            max_accounts_per_multivm: 10,
            binding_cooldown: Duration::from_secs(300), // 5 minutes
            require_multi_sig: false,
            min_account_age: Duration::from_secs(86400), // 24 hours
            max_binding_attempts_per_hour: 10,
            require_secondary_auth: false,
            unbinding_timelock: Duration::from_secs(604800), // 7 days
        }
    }
}

impl BindingPolicy {
    /// Create a strict policy for high-security environments
    pub fn strict() -> Self {
        Self {
            max_accounts_per_multivm: 5,
            binding_cooldown: Duration::from_secs(3600), // 1 hour
            require_multi_sig: true,
            min_account_age: Duration::from_secs(604800), // 7 days
            max_binding_attempts_per_hour: 3,
            require_secondary_auth: true,
            unbinding_timelock: Duration::from_secs(2592000), // 30 days
        }
    }

    /// Create a relaxed policy for development/testing
    pub fn relaxed() -> Self {
        Self {
            max_accounts_per_multivm: 50,
            binding_cooldown: Duration::from_secs(0),
            require_multi_sig: false,
            min_account_age: Duration::from_secs(0),
            max_binding_attempts_per_hour: 1000,
            require_secondary_auth: false,
            unbinding_timelock: Duration::from_secs(0),
        }
    }

    /// Validate if a binding operation is allowed under this policy
    pub fn validate_binding(
        &self,
        current_bound_accounts: usize,
        last_binding_time: Option<std::time::SystemTime>,
        account_creation_time: std::time::SystemTime,
        recent_attempts: u32,
    ) -> AccountMappingResult<()> {
        // Check maximum accounts limit
        if current_bound_accounts >= self.max_accounts_per_multivm as usize {
            return Err(AccountMappingError::PolicyViolation {
                reason: format!(
                    "Maximum accounts per MultiVM ID exceeded (limit: {})",
                    self.max_accounts_per_multivm
                ),
            });
        }

        // Check cooldown period
        if let Some(last_time) = last_binding_time {
            if let Ok(elapsed) = last_time.elapsed() {
                if elapsed < self.binding_cooldown {
                    return Err(AccountMappingError::PolicyViolation {
                        reason: format!(
                            "Binding cooldown period not met ({}s remaining)",
                            (self.binding_cooldown - elapsed).as_secs()
                        ),
                    });
                }
            }
        }

        // Check account age
        if let Ok(age) = account_creation_time.elapsed() {
            if age < self.min_account_age {
                return Err(AccountMappingError::PolicyViolation {
                    reason: format!(
                        "Account too new for binding (minimum age: {}s)",
                        self.min_account_age.as_secs()
                    ),
                });
            }
        }

        // Check rate limiting
        if recent_attempts >= self.max_binding_attempts_per_hour {
            return Err(AccountMappingError::PolicyViolation {
                reason: format!(
                    "Too many binding attempts (limit: {} per hour)",
                    self.max_binding_attempts_per_hour
                ),
            });
        }

        Ok(())
    }
}

/// Enhanced binding proof with additional security features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedBindingProof {
    /// The base binding proof
    pub proof: crate::mapping::BindingProof,
    /// Optional secondary authentication
    pub secondary_auth: Option<SecondaryAuth>,
    /// Risk score (0-100, where 0 is lowest risk)
    pub risk_score: u8,
    /// Additional metadata for security analysis
    pub security_metadata: SecurityMetadata,
}

/// Secondary authentication methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecondaryAuth {
    /// Time-based one-time password
    Totp { code: String },
    /// Hardware key signature
    HardwareKey {
        signature: Vec<u8>,
        device_id: String,
    },
    /// Email verification code
    EmailCode { code: String, email_hash: String },
    /// SMS verification code
    SmsCode { code: String, phone_hash: String },
    /// Multi-signature from other bound accounts
    MultiSig {
        signatures: Vec<(crate::address::AccountAddress, Vec<u8>)>,
    },
}

/// Security metadata for risk assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityMetadata {
    /// IP address of the request
    pub ip_address: Option<String>,
    /// User agent string
    pub user_agent: Option<String>,
    /// Geolocation data
    pub geolocation: Option<String>,
    /// Account activity score
    pub activity_score: u8,
    /// Previous suspicious activity count
    pub suspicious_activity_count: u32,
    /// Whether the account has been verified
    pub is_verified: bool,
}

impl SecurityMetadata {
    /// Calculate risk score based on metadata
    pub fn calculate_risk_score(&self) -> u8 {
        let mut score = 0u8;

        // Higher activity score reduces risk
        if self.activity_score > 80 {
            score = score.saturating_sub(20);
        } else if self.activity_score < 20 {
            score = score.saturating_add(20);
        }

        // Suspicious activity increases risk
        score = score.saturating_add((self.suspicious_activity_count * 10).min(50) as u8);

        // Verified accounts have lower risk
        if self.is_verified {
            score = score.saturating_sub(30);
        }

        // Missing metadata slightly increases risk
        if self.ip_address.is_none() || self.user_agent.is_none() {
            score = score.saturating_add(10);
        }

        score.min(100)
    }
}

/// Account recovery mechanisms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryConfig {
    /// Enable social recovery
    pub enable_social_recovery: bool,
    /// Minimum number of guardians required
    pub min_guardians: u8,
    /// Recovery threshold (percentage of guardians needed)
    pub recovery_threshold: u8,
    /// Recovery time lock period
    pub recovery_timelock: Duration,
    /// Enable backup key recovery
    pub enable_backup_key: bool,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            enable_social_recovery: true,
            min_guardians: 3,
            recovery_threshold: 66,                         // 66% of guardians
            recovery_timelock: Duration::from_secs(259200), // 3 days
            enable_backup_key: true,
        }
    }
}

/// Guardian for social recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Guardian {
    /// Guardian's account address
    pub address: crate::address::AccountAddress,
    /// When this guardian was added
    pub added_at: std::time::SystemTime,
    /// Optional label for the guardian
    pub label: Option<String>,
    /// Whether this guardian has been verified
    pub is_verified: bool,
}

/// Recovery request for social recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryRequest {
    /// The MultiVM account being recovered
    pub multivm_account: crate::address::MultivmAccountId,
    /// New account to bind after recovery
    pub new_account: crate::address::AccountAddress,
    /// Signatures from guardians
    pub guardian_signatures: Vec<(crate::address::AccountAddress, Vec<u8>)>,
    /// When the recovery was initiated
    pub initiated_at: std::time::SystemTime,
    /// When the recovery can be executed
    pub executable_at: std::time::SystemTime,
    /// Recovery request ID
    pub request_id: [u8; 32],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy() {
        let policy = BindingPolicy::default();
        assert_eq!(policy.max_accounts_per_multivm, 10);
        assert_eq!(policy.binding_cooldown.as_secs(), 300);
    }

    #[test]
    fn test_policy_validation() {
        let policy = BindingPolicy::default();

        // Should pass with valid conditions
        let result = policy.validate_binding(
            5,
            None,
            std::time::SystemTime::now() - Duration::from_secs(86401),
            5,
        );
        assert!(result.is_ok());

        // Should fail with too many accounts
        let result = policy.validate_binding(
            10,
            None,
            std::time::SystemTime::now() - Duration::from_secs(86401),
            5,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_risk_score_calculation() {
        let metadata = SecurityMetadata {
            ip_address: Some("127.0.0.1".to_string()),
            user_agent: Some("Test Agent".to_string()),
            geolocation: Some("US".to_string()),
            activity_score: 90,
            suspicious_activity_count: 0,
            is_verified: true,
        };

        let score = metadata.calculate_risk_score();
        assert!(score < 50); // Should be low risk
    }
}
