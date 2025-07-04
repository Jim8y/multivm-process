//! Input validation utilities for the MultiVM application
//!
//! This module provides comprehensive validation for all user inputs across
//! REST API, GraphQL, and WebSocket endpoints.

use crate::error::{ApplicationError, ApplicationResult};
use hex;
use std::collections::HashSet;

/// Maximum allowed pagination limit
pub const MAX_PAGINATION_LIMIT: usize = 1000;

/// Maximum allowed transaction data size (1MB)
pub const MAX_TRANSACTION_SIZE: usize = 1024 * 1024;

/// Maximum allowed search query length
pub const MAX_SEARCH_QUERY_LENGTH: usize = 256;

/// Validation error types
#[derive(Debug, Clone)]
pub enum ValidationError {
    InvalidAddress {
        address: String,
        vm_type: String,
    },
    InvalidTransactionHash {
        hash: String,
    },
    InvalidTransactionData {
        reason: String,
    },
    InvalidPagination {
        field: String,
        value: i32,
    },
    InvalidBlockId {
        block_id: String,
    },
    InvalidVmType {
        vm_type: String,
    },
    InvalidAmount {
        amount: String,
    },
    InvalidSignature {
        reason: String,
    },
    InvalidToken {
        reason: String,
    },
    InvalidSearchQuery {
        reason: String,
    },
    TooLarge {
        field: String,
        size: usize,
        max_size: usize,
    },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::InvalidAddress { address, vm_type } => {
                write!(f, "Invalid {vm_type} address: {address}")
            }
            ValidationError::InvalidTransactionHash { hash } => {
                write!(f, "Invalid transaction hash: {hash}")
            }
            ValidationError::InvalidTransactionData { reason } => {
                write!(f, "Invalid transaction data: {reason}")
            }
            ValidationError::InvalidPagination { field, value } => {
                write!(f, "Invalid pagination parameter {field}: {value}")
            }
            ValidationError::InvalidBlockId { block_id } => {
                write!(f, "Invalid block ID: {block_id}")
            }
            ValidationError::InvalidVmType { vm_type } => {
                write!(f, "Invalid VM type: {vm_type}")
            }
            ValidationError::InvalidAmount { amount } => {
                write!(f, "Invalid amount: {amount}")
            }
            ValidationError::InvalidSignature { reason } => {
                write!(f, "Invalid signature: {reason}")
            }
            ValidationError::InvalidToken { reason } => {
                write!(f, "Invalid token: {reason}")
            }
            ValidationError::InvalidSearchQuery { reason } => {
                write!(f, "Invalid search query: {reason}")
            }
            ValidationError::TooLarge {
                field,
                size,
                max_size,
            } => {
                write!(f, "Field {field} too large: {size} bytes (max {max_size})")
            }
        }
    }
}

impl From<ValidationError> for ApplicationError {
    fn from(err: ValidationError) -> Self {
        ApplicationError::ValidationError {
            field: "input".to_string(),
            message: err.to_string(),
        }
    }
}

/// Address validation utilities
pub mod address {
    use super::*;
    use bs58;

    /// Validate Ethereum address format
    pub fn validate_ethereum_address(address: &str) -> ApplicationResult<()> {
        // Check if it starts with 0x
        if !address.starts_with("0x") {
            return Err(ValidationError::InvalidAddress {
                address: address.to_string(),
                vm_type: "ethereum".to_string(),
            }
            .into());
        }

        // Check length (0x + 40 hex characters = 42 total)
        if address.len() != 42 {
            return Err(ValidationError::InvalidAddress {
                address: address.to_string(),
                vm_type: "ethereum".to_string(),
            }
            .into());
        }

        // Check if all characters after 0x are valid hex
        let hex_part = &address[2..];
        if !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ValidationError::InvalidAddress {
                address: address.to_string(),
                vm_type: "ethereum".to_string(),
            }
            .into());
        }

        Ok(())
    }

    /// Validate Solana address format (Base58 encoded public key)
    pub fn validate_solana_address(address: &str) -> ApplicationResult<()> {
        // Solana addresses should be 32-44 characters in Base58
        if address.len() < 32 || address.len() > 44 {
            return Err(ValidationError::InvalidAddress {
                address: address.to_string(),
                vm_type: "solana".to_string(),
            }
            .into());
        }

        // Try to decode as Base58
        match bs58::decode(address).into_vec() {
            Ok(decoded) => {
                // Should decode to exactly 32 bytes
                if decoded.len() != 32 {
                    return Err(ValidationError::InvalidAddress {
                        address: address.to_string(),
                        vm_type: "solana".to_string(),
                    }
                    .into());
                }
            }
            Err(_) => {
                return Err(ValidationError::InvalidAddress {
                    address: address.to_string(),
                    vm_type: "solana".to_string(),
                }
                .into());
            }
        }

        Ok(())
    }

    /// Validate address for any VM type
    pub fn validate_address(address: &str, vm_type: &str) -> ApplicationResult<()> {
        match vm_type.to_lowercase().as_str() {
            "ethereum" | "evm" => validate_ethereum_address(address),
            "solana" | "svm" => validate_solana_address(address),
            "multivm" => {
                // For MultiVM, try both formats
                if validate_ethereum_address(address).is_ok()
                    || validate_solana_address(address).is_ok()
                {
                    Ok(())
                } else {
                    Err(ValidationError::InvalidAddress {
                        address: address.to_string(),
                        vm_type: vm_type.to_string(),
                    }
                    .into())
                }
            }
            _ => Err(ValidationError::InvalidVmType {
                vm_type: vm_type.to_string(),
            }
            .into()),
        }
    }
}

/// Transaction validation utilities
pub mod transaction {
    use super::*;

    /// Validate Ethereum transaction hash
    pub fn validate_ethereum_tx_hash(hash: &str) -> ApplicationResult<()> {
        // Should start with 0x and have 64 hex characters
        if !hash.starts_with("0x") || hash.len() != 66 {
            return Err(ValidationError::InvalidTransactionHash {
                hash: hash.to_string(),
            }
            .into());
        }

        // Check if all characters after 0x are valid hex
        let hex_part = &hash[2..];
        if !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ValidationError::InvalidTransactionHash {
                hash: hash.to_string(),
            }
            .into());
        }

        Ok(())
    }

    /// Validate Solana transaction signature
    pub fn validate_solana_signature(signature: &str) -> ApplicationResult<()> {
        // Solana signatures are Base58 encoded and typically 87-88 characters
        if signature.len() < 87 || signature.len() > 88 {
            return Err(ValidationError::InvalidTransactionHash {
                hash: signature.to_string(),
            }
            .into());
        }

        // Try to decode as Base58
        match bs58::decode(signature).into_vec() {
            Ok(decoded) => {
                // Should decode to 64 bytes (Ed25519 signature)
                if decoded.len() != 64 {
                    return Err(ValidationError::InvalidTransactionHash {
                        hash: signature.to_string(),
                    }
                    .into());
                }
            }
            Err(_) => {
                return Err(ValidationError::InvalidTransactionHash {
                    hash: signature.to_string(),
                }
                .into());
            }
        }

        Ok(())
    }

    /// Validate raw transaction data size
    pub fn validate_transaction_size(data: &[u8]) -> ApplicationResult<()> {
        if data.len() > MAX_TRANSACTION_SIZE {
            return Err(ValidationError::TooLarge {
                field: "transaction_data".to_string(),
                size: data.len(),
                max_size: MAX_TRANSACTION_SIZE,
            }
            .into());
        }
        Ok(())
    }

    /// Validate Ethereum transaction data structure
    pub fn validate_ethereum_transaction_data(data: &str) -> ApplicationResult<()> {
        // Check if it's valid hex
        if !data.starts_with("0x") {
            return Err(ValidationError::InvalidTransactionData {
                reason: "Transaction data must start with 0x".to_string(),
            }
            .into());
        }

        let hex_part = &data[2..];
        if !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ValidationError::InvalidTransactionData {
                reason: "Transaction data contains invalid hex characters".to_string(),
            }
            .into());
        }

        // Check size
        if let Ok(decoded) = hex::decode(hex_part) {
            validate_transaction_size(&decoded)?;
        } else {
            return Err(ValidationError::InvalidTransactionData {
                reason: "Failed to decode hex transaction data".to_string(),
            }
            .into());
        }

        Ok(())
    }
}

/// Pagination validation utilities
pub mod pagination {
    use super::*;

    /// Validate pagination parameters
    pub fn validate_pagination(
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> ApplicationResult<(usize, usize)> {
        let limit = match limit {
            Some(l) => {
                if l <= 0 {
                    return Err(ValidationError::InvalidPagination {
                        field: "limit".to_string(),
                        value: l,
                    }
                    .into());
                }
                if l > MAX_PAGINATION_LIMIT as i32 {
                    return Err(ValidationError::InvalidPagination {
                        field: "limit".to_string(),
                        value: l,
                    }
                    .into());
                }
                l as usize
            }
            None => 50, // Default limit
        };

        let offset = match offset {
            Some(o) => {
                if o < 0 {
                    return Err(ValidationError::InvalidPagination {
                        field: "offset".to_string(),
                        value: o,
                    }
                    .into());
                }
                o as usize
            }
            None => 0, // Default offset
        };

        Ok((limit, offset))
    }
}

/// Block validation utilities
pub mod block {
    use super::*;

    /// Validate block identifier (number, hash, or "latest")
    pub fn validate_block_id(block_id: &str, vm_type: &str) -> ApplicationResult<()> {
        // Check for special values
        if block_id == "latest" || block_id == "earliest" || block_id == "pending" {
            return Ok(());
        }

        // Check if it's a number
        if block_id.parse::<u64>().is_ok() {
            return Ok(());
        }

        // Check if it's a hash based on VM type
        match vm_type.to_lowercase().as_str() {
            "ethereum" | "evm" => {
                if block_id.starts_with("0x") && block_id.len() == 66 {
                    let hex_part = &block_id[2..];
                    if hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
                        return Ok(());
                    }
                }
            }
            "solana" | "svm" => {
                // Solana uses slot numbers, so block_id should be a number
                if block_id.parse::<u64>().is_ok() {
                    return Ok(());
                }
            }
            _ => {}
        }

        Err(ValidationError::InvalidBlockId {
            block_id: block_id.to_string(),
        }
        .into())
    }
}

/// Amount validation utilities
pub mod amount {
    use super::*;

    /// Validate amount string (supports both decimal and hex)
    pub fn validate_amount(amount: &str) -> ApplicationResult<u64> {
        if let Some(hex_part) = amount.strip_prefix("0x") {
            // Hex amount
            u64::from_str_radix(hex_part, 16).map_err(|_| {
                ValidationError::InvalidAmount {
                    amount: amount.to_string(),
                }
                .into()
            })
        } else {
            // Decimal amount
            amount.parse::<u64>().map_err(|_| {
                ValidationError::InvalidAmount {
                    amount: amount.to_string(),
                }
                .into()
            })
        }
    }
}

/// VM type validation
pub mod vm_type {
    use super::*;

    use once_cell::sync::Lazy;

    static VALID_VM_TYPES: Lazy<HashSet<&'static str>> = Lazy::new(|| {
        let mut set = HashSet::new();
        set.insert("svm");
        set.insert("evm");
        set.insert("multivm");
        set.insert("ethereum");
        set.insert("solana");
        set
    });

    /// Validate VM type string
    pub fn validate_vm_type(vm_type: &str) -> ApplicationResult<()> {
        if VALID_VM_TYPES.contains(&vm_type.to_lowercase().as_str()) {
            Ok(())
        } else {
            Err(ValidationError::InvalidVmType {
                vm_type: vm_type.to_string(),
            }
            .into())
        }
    }
}

/// Search query validation
pub mod search {
    use super::*;

    /// Validate search query parameters
    pub fn validate_search_query(query: &str) -> ApplicationResult<()> {
        if query.len() > MAX_SEARCH_QUERY_LENGTH {
            return Err(ValidationError::TooLarge {
                field: "search_query".to_string(),
                size: query.len(),
                max_size: MAX_SEARCH_QUERY_LENGTH,
            }
            .into());
        }

        // Check for potentially dangerous characters
        let dangerous_chars = ['<', '>', '"', '\'', '&', ';', '(', ')', '|', '`'];
        if query.chars().any(|c| dangerous_chars.contains(&c)) {
            return Err(ValidationError::InvalidSearchQuery {
                reason: "Search query contains potentially dangerous characters".to_string(),
            }
            .into());
        }

        Ok(())
    }
}

/// Authentication token validation
pub mod auth {
    use super::*;

    /// Validate JWT token format (basic structure validation)
    pub fn validate_jwt_format(token: &str) -> ApplicationResult<()> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(ValidationError::InvalidToken {
                reason: "JWT must have exactly 3 parts separated by dots".to_string(),
            }
            .into());
        }

        // Check if each part is valid Base64
        for (i, part) in parts.iter().enumerate() {
            if part.is_empty() {
                return Err(ValidationError::InvalidToken {
                    reason: format!("JWT part {} is empty", i + 1),
                }
                .into());
            }

            // Basic Base64 character check
            if !part
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
            {
                return Err(ValidationError::InvalidToken {
                    reason: format!("JWT part {} contains invalid characters", i + 1),
                }
                .into());
            }
        }

        Ok(())
    }

    /// Validate API key format
    pub fn validate_api_key(key: &str) -> ApplicationResult<()> {
        // API keys should be alphanumeric and at least 32 characters
        if key.len() < 32 {
            return Err(ValidationError::InvalidToken {
                reason: "API key too short (minimum 32 characters)".to_string(),
            }
            .into());
        }

        if !key
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(ValidationError::InvalidToken {
                reason: "API key contains invalid characters".to_string(),
            }
            .into());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ethereum_address_validation() {
        // Valid addresses
        assert!(
            address::validate_ethereum_address("0x742D35Cc6634C0532925a3b8D37E176E7f7ee456")
                .is_ok()
        );
        assert!(
            address::validate_ethereum_address("0x0000000000000000000000000000000000000000")
                .is_ok()
        );

        // Invalid addresses
        assert!(
            address::validate_ethereum_address("742D35Cc6634C0532925a3b8D37E176E7f7ee456").is_err()
        ); // No 0x
        assert!(
            address::validate_ethereum_address("0x742D35Cc6634C0532925a3b8D37E176E7f7ee45")
                .is_err()
        ); // Too short
        assert!(
            address::validate_ethereum_address("0x742D35Cc6634C0532925a3b8D37E176E7f7ee456G")
                .is_err()
        ); // Invalid char
    }

    #[test]
    fn test_solana_address_validation() {
        // Valid address
        assert!(address::validate_solana_address("11111111111111111111111111111112").is_ok());

        // Invalid addresses
        assert!(address::validate_solana_address("short").is_err()); // Too short
        assert!(
            address::validate_solana_address("0x742D35Cc6634C0532925a3b8D37E176E7f7ee456").is_err()
        ); // Not Base58
    }

    #[test]
    fn test_pagination_validation() {
        // Valid pagination
        assert!(pagination::validate_pagination(Some(10), Some(0)).is_ok());
        assert!(pagination::validate_pagination(None, None).is_ok());

        // Invalid pagination
        assert!(pagination::validate_pagination(Some(0), Some(0)).is_err()); // Limit too small
        assert!(pagination::validate_pagination(Some(-1), Some(0)).is_err()); // Negative limit
        assert!(pagination::validate_pagination(Some(10), Some(-1)).is_err()); // Negative offset
        assert!(pagination::validate_pagination(Some(2000), Some(0)).is_err()); // Limit too large
    }
}
