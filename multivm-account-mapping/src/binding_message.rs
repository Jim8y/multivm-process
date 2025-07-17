//! Standardized message formats for account binding operations

use crate::{
    address::{AccountAddress, MultivmAccountId},
    error::AccountMappingResult,
};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Standardized message for binding operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingMessage {
    /// The action being performed
    pub action: BindingAction,
    /// Chain ID to prevent cross-chain replay attacks
    pub chain_id: String,
    /// Source account (already bound)
    pub source: AccountAddress,
    /// Target account (to be bound)
    pub target: AccountAddress,
    /// MultiVM account ID
    pub multivm_id: MultivmAccountId,
    /// Nonce for replay protection
    pub nonce: u64,
    /// Unix timestamp
    pub timestamp: u64,
    /// Optional expiration time
    pub expires_at: Option<u64>,
    /// Message version for future compatibility
    pub version: u8,
}

/// Types of binding actions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BindingAction {
    /// Bind a new account
    BindAccount,
    /// Unbind an existing account
    UnbindAccount,
    /// Update binding configuration
    UpdateBinding,
    /// Initiate recovery
    InitiateRecovery,
    /// Add guardian for social recovery
    AddGuardian,
    /// Remove guardian
    RemoveGuardian,
}

impl BindingMessage {
    /// Create a new binding message
    pub fn new(
        action: BindingAction,
        chain_id: String,
        source: AccountAddress,
        target: AccountAddress,
        multivm_id: MultivmAccountId,
        nonce: u64,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            action,
            chain_id,
            source,
            target,
            multivm_id,
            nonce,
            timestamp,
            expires_at: Some(timestamp + 3600), // 1 hour expiration by default
            version: 1,
        }
    }

    /// Create the message bytes for signing (EIP-712 style)
    pub fn to_sign_bytes(&self) -> Vec<u8> {
        // Create a deterministic byte representation
        let mut bytes = Vec::new();
        
        // Add structured data prefix (similar to EIP-712)
        bytes.extend_from_slice(b"\x19\x01");
        
        // Add domain separator
        bytes.extend_from_slice(&self.domain_separator());
        
        // Add message hash
        bytes.extend_from_slice(&self.message_hash());
        
        bytes
    }

    /// Create domain separator for the message
    fn domain_separator(&self) -> [u8; 32] {
        use blake3::Hasher;
        
        let mut hasher = Hasher::new();
        hasher.update(b"MultiVM Account Binding");
        hasher.update(b"1"); // Version
        hasher.update(self.chain_id.as_bytes());
        hasher.update(b"multivm-account-binding");
        
        let hash = hasher.finalize();
        let mut result = [0u8; 32];
        result.copy_from_slice(hash.as_bytes());
        result
    }

    /// Create message hash
    fn message_hash(&self) -> [u8; 32] {
        use blake3::Hasher;
        
        let mut hasher = Hasher::new();
        
        // Hash action
        hasher.update(&self.action_to_bytes());
        
        // Hash addresses
        hasher.update(&self.source.to_bytes());
        hasher.update(&self.target.to_bytes());
        hasher.update(self.multivm_id.as_bytes());
        
        // Hash numeric values
        hasher.update(&self.nonce.to_le_bytes());
        hasher.update(&self.timestamp.to_le_bytes());
        
        if let Some(expires) = self.expires_at {
            hasher.update(&expires.to_le_bytes());
        }
        
        hasher.update(&[self.version]);
        
        let hash = hasher.finalize();
        let mut result = [0u8; 32];
        result.copy_from_slice(hash.as_bytes());
        result
    }

    /// Convert action to bytes
    fn action_to_bytes(&self) -> Vec<u8> {
        match self.action {
            BindingAction::BindAccount => vec![0x01],
            BindingAction::UnbindAccount => vec![0x02],
            BindingAction::UpdateBinding => vec![0x03],
            BindingAction::InitiateRecovery => vec![0x04],
            BindingAction::AddGuardian => vec![0x05],
            BindingAction::RemoveGuardian => vec![0x06],
        }
    }

    /// Create a human-readable message for wallet signing
    pub fn to_display_string(&self) -> String {
        format!(
            "MultiVM Account Binding Request\n\
             \n\
             Action: {}\n\
             Chain: {}\n\
             MultiVM Account: {}\n\
             Source Account: {}\n\
             Target Account: {}\n\
             Nonce: {}\n\
             Timestamp: {}\n\
             Expires: {}\n\
             \n\
             WARNING: Only sign this message if you initiated this action.",
            self.action_string(),
            self.chain_id,
            hex::encode(self.multivm_id.as_bytes()),
            self.source.as_string(),
            self.target.as_string(),
            self.nonce,
            self.timestamp,
            self.expires_at
                .map(|e| format!("{}", e))
                .unwrap_or_else(|| "Never".to_string())
        )
    }

    /// Get action as string
    fn action_string(&self) -> &str {
        match self.action {
            BindingAction::BindAccount => "Bind Account",
            BindingAction::UnbindAccount => "Unbind Account",
            BindingAction::UpdateBinding => "Update Binding",
            BindingAction::InitiateRecovery => "Initiate Recovery",
            BindingAction::AddGuardian => "Add Guardian",
            BindingAction::RemoveGuardian => "Remove Guardian",
        }
    }

    /// Validate message expiration
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            now > expires_at
        } else {
            false
        }
    }

    /// Validate message is not from the future
    pub fn is_future(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.timestamp > now + 300 // Allow 5 minutes clock skew
    }
}

/// Message builder for convenient construction
pub struct BindingMessageBuilder {
    action: Option<BindingAction>,
    chain_id: Option<String>,
    source: Option<AccountAddress>,
    target: Option<AccountAddress>,
    multivm_id: Option<MultivmAccountId>,
    nonce: Option<u64>,
    expires_in: Option<u64>,
}

impl BindingMessageBuilder {
    pub fn new() -> Self {
        Self {
            action: None,
            chain_id: None,
            source: None,
            target: None,
            multivm_id: None,
            nonce: None,
            expires_in: None,
        }
    }

    pub fn action(mut self, action: BindingAction) -> Self {
        self.action = Some(action);
        self
    }

    pub fn chain_id(mut self, chain_id: String) -> Self {
        self.chain_id = Some(chain_id);
        self
    }

    pub fn source(mut self, source: AccountAddress) -> Self {
        self.source = Some(source);
        self
    }

    pub fn target(mut self, target: AccountAddress) -> Self {
        self.target = Some(target);
        self
    }

    pub fn multivm_id(mut self, multivm_id: MultivmAccountId) -> Self {
        self.multivm_id = Some(multivm_id);
        self
    }

    pub fn nonce(mut self, nonce: u64) -> Self {
        self.nonce = Some(nonce);
        self
    }

    pub fn expires_in(mut self, seconds: u64) -> Self {
        self.expires_in = Some(seconds);
        self
    }

    pub fn build(self) -> AccountMappingResult<BindingMessage> {
        let action = self.action.ok_or(crate::error::AccountMappingError::InvalidInput {
            reason: "Action is required".to_string(),
        })?;
        let chain_id = self.chain_id.ok_or(crate::error::AccountMappingError::InvalidInput {
            reason: "Chain ID is required".to_string(),
        })?;
        let source = self.source.ok_or(crate::error::AccountMappingError::InvalidInput {
            reason: "Source account is required".to_string(),
        })?;
        let target = self.target.ok_or(crate::error::AccountMappingError::InvalidInput {
            reason: "Target account is required".to_string(),
        })?;
        let multivm_id = self.multivm_id.ok_or(crate::error::AccountMappingError::InvalidInput {
            reason: "MultiVM ID is required".to_string(),
        })?;
        let nonce = self.nonce.unwrap_or(0);

        let mut message = BindingMessage::new(action, chain_id, source, target, multivm_id, nonce);

        if let Some(expires_in) = self.expires_in {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            message.expires_at = Some(now + expires_in);
        }

        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address::{EthereumAddress, SolanaAddress};

    #[test]
    fn test_message_creation() {
        let source = AccountAddress::Ethereum(EthereumAddress([1u8; 20]));
        let target = AccountAddress::Solana(SolanaAddress([2u8; 32]));
        let multivm_id = MultivmAccountId::from_seed(b"test");

        let message = BindingMessage::new(
            BindingAction::BindAccount,
            "multivm-testnet".to_string(),
            source,
            target,
            multivm_id,
            1,
        );

        assert_eq!(message.version, 1);
        assert!(!message.is_expired());
        assert!(!message.is_future());
    }

    #[test]
    fn test_message_builder() {
        let source = AccountAddress::Ethereum(EthereumAddress([1u8; 20]));
        let target = AccountAddress::Solana(SolanaAddress([2u8; 32]));
        let multivm_id = MultivmAccountId::from_seed(b"test");

        let message = BindingMessageBuilder::new()
            .action(BindingAction::BindAccount)
            .chain_id("multivm-testnet".to_string())
            .source(source)
            .target(target)
            .multivm_id(multivm_id)
            .nonce(1)
            .expires_in(3600)
            .build()
            .unwrap();

        assert_eq!(message.action, BindingAction::BindAccount);
        assert!(message.expires_at.is_some());
    }

    #[test]
    fn test_message_signing_bytes() {
        let source = AccountAddress::Ethereum(EthereumAddress([1u8; 20]));
        let target = AccountAddress::Solana(SolanaAddress([2u8; 32]));
        let multivm_id = MultivmAccountId::from_seed(b"test");

        let message1 = BindingMessage::new(
            BindingAction::BindAccount,
            "multivm-testnet".to_string(),
            source.clone(),
            target.clone(),
            multivm_id.clone(),
            1,
        );

        let message2 = BindingMessage::new(
            BindingAction::BindAccount,
            "multivm-testnet".to_string(),
            source,
            target,
            multivm_id,
            1,
        );

        // Same messages should produce same signing bytes
        assert_eq!(message1.to_sign_bytes(), message2.to_sign_bytes());
    }
}