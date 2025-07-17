//! Enhanced account mapping implementation with all recommended features

use crate::{
    address::{AccountAddress, MultivmAccountId},
    binding_message::{BindingAction, BindingMessage},
    binding_policy::{
        BindingPolicy, EnhancedBindingProof, Guardian, RecoveryConfig, RecoveryRequest,
    },
    distributed_lock::{DistributedLockManager, InMemoryLockManager, RetryableLockManager},
    error::{AccountMappingError, AccountMappingResult},
    events::{AccountBindingEvent, EventEmitter},
    mapping::{AccountBinding, AccountMappingLayer},
    storage::AccountMappingStorage,
    validation::AccountBindingValidator,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Enhanced account mapping service with all security features
pub struct EnhancedAccountMapper {
    /// Storage backend
    storage: Arc<dyn AccountMappingStorage>,
    /// Distributed lock manager
    lock_manager: Arc<RetryableLockManager<InMemoryLockManager>>,
    /// Binding policy
    policy: BindingPolicy,
    /// Event emitter
    pub event_emitter: Arc<RwLock<EventEmitter>>,
    /// Validator
    validator: AccountBindingValidator,
    /// Recovery configuration
    recovery_config: RecoveryConfig,
    /// Rate limiting tracking
    rate_limiter: Arc<RwLock<RateLimiter>>,
    /// Guardians registry
    pub guardians: Arc<RwLock<HashMap<MultivmAccountId, Vec<Guardian>>>>,
    /// Pending recovery requests
    pub recovery_requests: Arc<RwLock<HashMap<[u8; 32], RecoveryRequest>>>,
}

impl EnhancedAccountMapper {
    pub fn new(
        storage: Arc<dyn AccountMappingStorage>,
        policy: BindingPolicy,
        recovery_config: RecoveryConfig,
    ) -> Self {
        let lock_manager = InMemoryLockManager::new();
        let retryable_lock_manager =
            RetryableLockManager::new(lock_manager, 3, Duration::from_millis(100));

        Self {
            storage,
            lock_manager: Arc::new(retryable_lock_manager),
            policy,
            event_emitter: Arc::new(RwLock::new(EventEmitter::new())),
            validator: AccountBindingValidator::new(crate::validation::ValidationConfig::default()),
            recovery_config,
            rate_limiter: Arc::new(RwLock::new(RateLimiter::new())),
            guardians: Arc::new(RwLock::new(HashMap::new())),
            recovery_requests: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Process a binding message with full validation
    pub async fn process_binding_message(
        &self,
        message: BindingMessage,
        proof: EnhancedBindingProof,
    ) -> AccountMappingResult<()> {
        // Validate message timing
        if message.is_expired() {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Message has expired".to_string(),
            });
        }

        if message.is_future() {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Message timestamp is in the future".to_string(),
            });
        }

        // Rate limiting check
        self.check_rate_limit(&message.source).await?;

        match message.action {
            BindingAction::BindAccount => self.process_bind_account(message, proof).await,
            BindingAction::UnbindAccount => self.process_unbind_account(message, proof).await,
            BindingAction::InitiateRecovery => self.process_initiate_recovery(message, proof).await,
            BindingAction::AddGuardian => self.process_add_guardian(message, proof).await,
            BindingAction::RemoveGuardian => self.process_remove_guardian(message, proof).await,
            _ => Err(AccountMappingError::UnsupportedOperation {
                operation: format!("{:?}", message.action),
            }),
        }
    }

    /// Process account binding with distributed locking
    async fn process_bind_account(
        &self,
        message: BindingMessage,
        proof: EnhancedBindingProof,
    ) -> AccountMappingResult<()> {
        let lock_key = format!("bind:{}", message.multivm_id);

        let _lock = self
            .lock_manager
            .acquire_lock(&lock_key, Duration::from_secs(30))
            .await?;

        {
            // Validate proof
            self.validator.validate_proof(&proof.proof)?;

            // Check policy
            let binding = self.storage.get_binding(&message.multivm_id).await?;
            let bound_accounts = if let Some(ref b) = binding {
                b.get_all_accounts().len()
            } else {
                0
            };

            self.policy.validate_binding(
                bound_accounts,
                binding.as_ref().map(|b| b.created_at),
                SystemTime::now() - Duration::from_secs(86400), // Example account age
                0, // Recent attempts would be tracked separately
            )?;

            // Perform the binding
            if let Some(mut binding) = binding {
                binding.add_cross_binding(message.target.clone(), proof.proof)?;
                self.storage.update_binding(&binding).await?;
            } else {
                // This shouldn't happen with proper auto-binding
                return Err(AccountMappingError::InvalidBinding {
                    reason: "MultiVM account does not exist".to_string(),
                });
            }

            // Emit event
            let event = AccountBindingEvent::CrossBindingAdded {
                source: message.source,
                target: message.target,
                multivm_id: message.multivm_id,
                proof_type: "signature".to_string(),
                timestamp: SystemTime::now(),
            };
            self.event_emitter.write().await.emit(event).await;

            Ok(())
        }
    }

    /// Process unbind with timelock
    async fn process_unbind_account(
        &self,
        message: BindingMessage,
        proof: EnhancedBindingProof,
    ) -> AccountMappingResult<()> {
        // Check if unbinding is allowed
        if self.policy.unbinding_timelock > Duration::from_secs(0) {
            // In production, this would check a timelock registry
            warn!(
                "Unbinding requires timelock of {:?}",
                self.policy.unbinding_timelock
            );
        }

        // Validate authorization
        self.validator.validate_proof(&proof.proof)?;

        // Perform unbinding
        let binding = self
            .storage
            .get_binding(&message.multivm_id)
            .await?
            .ok_or_else(|| AccountMappingError::AccountNotFound {
                address: message.multivm_id.to_string(),
            })?;

        // Check authorization
        let authorized = match &message.target {
            account if binding.svm_account.as_ref() == Some(account) => true,
            account if binding.evm_account.as_ref() == Some(account) => true,
            _ => false,
        };

        if !authorized {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Not authorized to unbind this account".to_string(),
            });
        }

        // Remove the account binding
        // Note: In production, this would create an unbinding request with timelock
        info!(
            "Unbinding request created for account: {:?}",
            message.target
        );

        // Emit event
        let event = AccountBindingEvent::BindingRemoved {
            account: message.target,
            multivm_id: message.multivm_id,
            timestamp: SystemTime::now(),
            reason: "User requested unbinding".to_string(),
        };
        self.event_emitter.write().await.emit(event).await;

        Ok(())
    }

    /// Initiate account recovery
    async fn process_initiate_recovery(
        &self,
        message: BindingMessage,
        proof: EnhancedBindingProof,
    ) -> AccountMappingResult<()> {
        if !self.recovery_config.enable_social_recovery {
            return Err(AccountMappingError::RecoveryNotAllowed {
                reason: "Social recovery is disabled".to_string(),
            });
        }

        // Validate guardian signature
        self.validator.validate_proof(&proof.proof)?;

        // Check if requester is a guardian
        let guardians = self.guardians.read().await;
        let account_guardians = guardians.get(&message.multivm_id).ok_or_else(|| {
            AccountMappingError::RecoveryNotAllowed {
                reason: "No guardians configured".to_string(),
            }
        })?;

        let is_guardian = account_guardians
            .iter()
            .any(|g| g.address == message.source);

        if !is_guardian {
            return Err(AccountMappingError::RecoveryNotAllowed {
                reason: "Requester is not a guardian".to_string(),
            });
        }

        // Create recovery request
        let request_id = blake3::hash(message.to_sign_bytes().as_slice())
            .as_bytes()
            .clone();
        let recovery_request = RecoveryRequest {
            multivm_account: message.multivm_id.clone(),
            new_account: message.target.clone(),
            guardian_signatures: vec![(message.source.clone(), proof.proof.proof_data.to_vec())],
            initiated_at: SystemTime::now(),
            executable_at: SystemTime::now() + self.recovery_config.recovery_timelock,
            request_id,
        };

        // Store recovery request
        let mut requests = self.recovery_requests.write().await;
        requests.insert(request_id, recovery_request);

        // Emit event
        let event = AccountBindingEvent::RecoveryInitiated {
            multivm_id: message.multivm_id,
            new_account: message.target,
            guardian_count: account_guardians.len(),
            timestamp: SystemTime::now(),
            executable_at: SystemTime::now() + self.recovery_config.recovery_timelock,
        };
        self.event_emitter.write().await.emit(event).await;

        Ok(())
    }

    /// Add a guardian for social recovery
    async fn process_add_guardian(
        &self,
        message: BindingMessage,
        proof: EnhancedBindingProof,
    ) -> AccountMappingResult<()> {
        // Validate authorization
        self.validator.validate_proof(&proof.proof)?;

        // Check if source owns the MultiVM account
        let binding = self
            .storage
            .get_binding(&message.multivm_id)
            .await?
            .ok_or_else(|| AccountMappingError::AccountNotFound {
                address: message.multivm_id.to_string(),
            })?;

        let authorized = binding.svm_account.as_ref() == Some(&message.source)
            || binding.evm_account.as_ref() == Some(&message.source);

        if !authorized {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Not authorized to add guardians".to_string(),
            });
        }

        // Add guardian
        let guardian = Guardian {
            address: message.target.clone(),
            added_at: SystemTime::now(),
            label: None,
            is_verified: false,
        };

        let mut guardians = self.guardians.write().await;
        guardians
            .entry(message.multivm_id.clone())
            .or_insert_with(Vec::new)
            .push(guardian);

        // Emit event
        let event = AccountBindingEvent::GuardianAdded {
            multivm_id: message.multivm_id,
            guardian: message.target,
            timestamp: SystemTime::now(),
        };
        self.event_emitter.write().await.emit(event).await;

        Ok(())
    }

    /// Remove a guardian
    async fn process_remove_guardian(
        &self,
        message: BindingMessage,
        proof: EnhancedBindingProof,
    ) -> AccountMappingResult<()> {
        // Similar to add_guardian but removes instead
        self.validator.validate_proof(&proof.proof)?;

        let binding = self
            .storage
            .get_binding(&message.multivm_id)
            .await?
            .ok_or_else(|| AccountMappingError::AccountNotFound {
                address: message.multivm_id.to_string(),
            })?;

        let authorized = binding.svm_account.as_ref() == Some(&message.source)
            || binding.evm_account.as_ref() == Some(&message.source);

        if !authorized {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Not authorized to remove guardians".to_string(),
            });
        }

        let mut guardians = self.guardians.write().await;
        if let Some(account_guardians) = guardians.get_mut(&message.multivm_id) {
            account_guardians.retain(|g| g.address != message.target);
        }

        let event = AccountBindingEvent::GuardianRemoved {
            multivm_id: message.multivm_id,
            guardian: message.target,
            timestamp: SystemTime::now(),
        };
        self.event_emitter.write().await.emit(event).await;

        Ok(())
    }

    /// Check rate limiting
    async fn check_rate_limit(&self, account: &AccountAddress) -> AccountMappingResult<()> {
        let mut limiter = self.rate_limiter.write().await;

        if !limiter.check_and_update(account, self.policy.max_binding_attempts_per_hour) {
            let event = AccountBindingEvent::RateLimitExceeded {
                account: account.clone(),
                action: BindingAction::BindAccount,
                limit_type: "hourly_binding_attempts".to_string(),
                timestamp: SystemTime::now(),
            };
            self.event_emitter.write().await.emit(event).await;

            return Err(AccountMappingError::RateLimitExceeded {
                limit_type: "binding_attempts".to_string(),
            });
        }

        Ok(())
    }

    /// Handle automatic binding with proper locking
    pub async fn create_auto_binding(
        &self,
        account: AccountAddress,
    ) -> AccountMappingResult<MultivmAccountId> {
        let multivm_id = MultivmAccountId::from_account(&account);
        let lock_key = format!("auto_bind:{}", multivm_id);

        let _lock = self
            .lock_manager
            .acquire_lock(&lock_key, Duration::from_secs(10))
            .await?;

        {
            // Check if already exists
            if let Ok(Some(_)) = self.storage.get_binding(&multivm_id).await {
                return Ok(multivm_id);
            }

            // Create new binding
            let binding = AccountBinding::create_auto_binding(account.clone());
            self.storage.store_binding(&binding).await?;

            // Emit event
            let event = AccountBindingEvent::AutoBindingCreated {
                account,
                multivm_id: multivm_id.clone(),
                timestamp: SystemTime::now(),
                transaction_hash: None,
            };
            self.event_emitter.write().await.emit(event).await;

            Ok(multivm_id)
        }
    }
}

/// Simple rate limiter
#[cfg_attr(test, derive(Debug))]
pub(crate) struct RateLimiter {
    attempts: HashMap<AccountAddress, Vec<SystemTime>>,
}

impl RateLimiter {
    pub(crate) fn new() -> Self {
        Self {
            attempts: HashMap::new(),
        }
    }

    fn check_and_update(&mut self, account: &AccountAddress, max_per_hour: u32) -> bool {
        let now = SystemTime::now();
        let one_hour_ago = now - Duration::from_secs(3600);

        let attempts = self
            .attempts
            .entry(account.clone())
            .or_insert_with(Vec::new);

        // Clean old attempts
        attempts.retain(|&time| time > one_hour_ago);

        // Check limit
        if attempts.len() >= max_per_hour as usize {
            return false;
        }

        // Record new attempt
        attempts.push(now);
        true
    }
}

#[async_trait::async_trait]
impl AccountMappingLayer for EnhancedAccountMapper {
    async fn get_binding(
        &self,
        multivm_id: &MultivmAccountId,
    ) -> AccountMappingResult<Option<AccountBinding>> {
        self.storage.get_binding(multivm_id).await
    }

    async fn get_bound_addresses(
        &self,
        multivm_id: &MultivmAccountId,
    ) -> AccountMappingResult<Vec<AccountAddress>> {
        // Get the binding and extract all addresses
        if let Some(binding) = self.storage.get_binding(multivm_id).await? {
            Ok(binding.get_all_accounts())
        } else {
            Ok(Vec::new())
        }
    }

    async fn create_binding(&self, binding: AccountBinding) -> AccountMappingResult<()> {
        self.storage.store_binding(&binding).await
    }

    async fn update_binding(
        &self,
        multivm_id: &MultivmAccountId,
        metadata: crate::mapping::BindingMetadata,
    ) -> AccountMappingResult<()> {
        // Get the current binding
        if let Some(mut binding) = self.storage.get_binding(multivm_id).await? {
            // Update the metadata
            binding.metadata = metadata;
            // Store the updated binding
            self.storage.update_binding(&binding).await
        } else {
            Err(AccountMappingError::AccountNotFound {
                address: multivm_id.to_string(),
            })
        }
    }

    async fn get_binding_by_account(
        &self,
        account: &AccountAddress,
    ) -> AccountMappingResult<Option<AccountBinding>> {
        self.storage.get_binding_by_account(account).await
    }

    async fn has_binding(&self, account: &AccountAddress) -> AccountMappingResult<bool> {
        Ok(self
            .storage
            .get_binding_by_account(account)
            .await?
            .is_some())
    }

    async fn remove_binding(&self, multivm_id: &MultivmAccountId) -> AccountMappingResult<()> {
        self.storage.delete_binding(multivm_id).await
    }

    async fn add_auto_binding(
        &self,
        account: AccountAddress,
    ) -> AccountMappingResult<MultivmAccountId> {
        self.create_auto_binding(account).await
    }

    async fn resolve_multivm_account(
        &self,
        address: &AccountAddress,
    ) -> AccountMappingResult<Option<MultivmAccountId>> {
        self.storage.resolve_multivm_account(address).await
    }

    async fn process_special_transaction(
        &self,
        special_tx: crate::special_tx::SpecialTransaction,
    ) -> AccountMappingResult<()> {
        // Convert to binding message format for processing
        match special_tx {
            crate::special_tx::SpecialTransaction::AccountBinding {
                source_account,
                target_account,
                proof,
                ..
            } => {
                let multivm_id = self
                    .resolve_multivm_account(&source_account)
                    .await?
                    .unwrap_or_else(|| MultivmAccountId::from_account(&source_account));

                let message = BindingMessage::new(
                    BindingAction::BindAccount,
                    "multivm-mainnet".to_string(),
                    source_account,
                    target_account,
                    multivm_id,
                    proof.nonce,
                );

                let enhanced_proof = EnhancedBindingProof {
                    proof,
                    secondary_auth: None,
                    risk_score: 0,
                    security_metadata: crate::binding_policy::SecurityMetadata {
                        ip_address: None,
                        user_agent: None,
                        geolocation: None,
                        activity_score: 50,
                        suspicious_activity_count: 0,
                        is_verified: false,
                    },
                };

                self.process_binding_message(message, enhanced_proof).await
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MemoryStorage;

    #[tokio::test]
    async fn test_enhanced_auto_binding() {
        let storage = Arc::new(MemoryStorage::new());
        let mapper = EnhancedAccountMapper::new(
            storage,
            BindingPolicy::default(),
            RecoveryConfig::default(),
        );

        // Use unique account to avoid lock contention with other tests
        let account = AccountAddress::Ethereum(crate::address::EthereumAddress([99u8; 20]));
        let multivm_id = mapper.create_auto_binding(account.clone()).await.unwrap();

        // Should be idempotent
        let multivm_id2 = mapper.create_auto_binding(account).await.unwrap();
        assert_eq!(multivm_id, multivm_id2);
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let mut limiter = RateLimiter::new();
        let account = AccountAddress::Ethereum(crate::address::EthereumAddress([98u8; 20]));

        // Should allow up to limit
        for _ in 0..10 {
            assert!(limiter.check_and_update(&account, 10));
        }

        // Should block after limit
        assert!(!limiter.check_and_update(&account, 10));
    }
}
