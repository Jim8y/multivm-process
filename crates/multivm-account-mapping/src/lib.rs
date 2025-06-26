//! # MultiVM Account Mapping Layer
//!
//! This module implements the account mapping layer for the MultiVM architecture,
//! enabling cross-VM account management and binding operations.
//!
//! ## Core Features
//!
//! - **Automatic Binding**: A↔M - When account A appears, automatically create MultiVM account M
//! - **User Binding**: A↔M↔B - Users can bind accounts across different VMs
//! - **Special Transactions**: Handle binding operations and cross-VM transfers
//! - **Address Translation**: Bidirectional address mapping between VMs
//!
//! ## Architecture
//!
//! ```text
//! Account A (SVM/EVM) ↔ MultiVM Account M ↔ Account B (EVM/SVM)
//! ```

pub mod address;
pub mod atomic_coordinator;
pub mod cross_vm_coordinator;
pub mod error;
pub mod ipc_integration;
pub mod mapping;
pub mod special_tx;
pub mod storage;
pub mod validation;
pub mod vm_engines;

// Re-export key types
pub use address::{AccountAddress, EthereumAddress, MultivmAccountId, SolanaAddress};
pub use error::{AccountMappingError, AccountMappingResult};
pub use mapping::{
    AccountBinding, AccountMapper, AccountMappingLayer, BindingMetadata, BindingProof,
};
pub use storage::{AccountMappingStorage, FileStorage, MemoryStorage, RocksDBStorage};

// Re-export common types for convenience
pub use multivm_common::{
    config::VmType, HealthStatus, Manager, ManagerState, ManagerStats, MultivmError, MultivmResult,
    ProcessingMetrics,
};

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Account mapping configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct AccountMappingConfig {
    /// Storage backend configuration
    pub storage: StorageConfig,
    /// IPC configuration for communication with other components
    pub ipc: IpcConfig,
    /// Validation settings
    pub validation: ValidationConfig,
    /// Cross-VM coordinator settings
    pub cross_vm: CrossVmConfig,
}

/// Storage configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StorageConfig {
    /// Storage backend type
    pub backend: StorageBackend,
    /// Data directory for file-based storage
    pub data_dir: std::path::PathBuf,
    /// RocksDB specific settings
    pub rocksdb: RocksDbConfig,
}

/// Storage backend types
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum StorageBackend {
    Memory,
    File,
    RocksDb,
}

/// RocksDB configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RocksDbConfig {
    /// Database path
    pub path: std::path::PathBuf,
    /// Enable compression
    pub enable_compression: bool,
    /// Cache size in MB
    pub cache_size_mb: u32,
    /// Write buffer size in MB
    pub write_buffer_size_mb: u32,
}

/// IPC configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IpcConfig {
    /// Enable IPC communication
    pub enabled: bool,
    /// IPC endpoint
    pub endpoint: String,
    /// Connection timeout in seconds
    pub timeout_seconds: u64,
    /// Maximum retries
    pub max_retries: u32,
}

/// Validation configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ValidationConfig {
    /// Enable strict validation
    pub strict_mode: bool,
    /// Require proof for all bindings
    pub require_proof: bool,
    /// Maximum binding age in seconds
    pub max_binding_age_seconds: u64,
    /// Enable address format validation
    pub validate_address_format: bool,
}

/// Cross-VM coordinator configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CrossVmConfig {
    /// Enable cross-VM operations
    pub enabled: bool,
    /// Transaction timeout in seconds
    pub transaction_timeout_seconds: u64,
    /// Maximum concurrent operations
    pub max_concurrent_operations: u32,
    /// Enable atomic operations
    pub enable_atomic_operations: bool,
}

// Default implementations

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: StorageBackend::Memory,
            data_dir: std::path::PathBuf::from("./data/account_mapping"),
            rocksdb: RocksDbConfig::default(),
        }
    }
}

impl Default for RocksDbConfig {
    fn default() -> Self {
        Self {
            path: std::path::PathBuf::from("./data/account_mapping.db"),
            enable_compression: true,
            cache_size_mb: 64,
            write_buffer_size_mb: 16,
        }
    }
}

impl Default for IpcConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            endpoint: "unix:///tmp/multivm_account_mapping.sock".to_string(),
            timeout_seconds: 30,
            max_retries: 3,
        }
    }
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            strict_mode: true,
            require_proof: true,
            max_binding_age_seconds: 86400, // 24 hours
            validate_address_format: true,
        }
    }
}

impl Default for CrossVmConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            transaction_timeout_seconds: 300, // 5 minutes
            max_concurrent_operations: 10,
            enable_atomic_operations: true,
        }
    }
}

impl AccountMappingConfig {
    /// Load configuration from file
    pub fn from_file(path: &std::path::Path) -> AccountMappingResult<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content).map_err(|e| AccountMappingError::Internal {
            message: format!("Failed to parse config: {}", e),
        })?;
        Ok(config)
    }

    /// Save configuration to file
    pub fn to_file(&self, path: &std::path::Path) -> AccountMappingResult<()> {
        let content = toml::to_string_pretty(self).map_err(|e| AccountMappingError::Internal {
            message: format!("Failed to serialize config: {}", e),
        })?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> AccountMappingResult<()> {
        // Validate storage configuration
        if matches!(
            self.storage.backend,
            StorageBackend::File | StorageBackend::RocksDb
        ) && !self.storage.data_dir.exists()
        {
            std::fs::create_dir_all(&self.storage.data_dir)?;
        }

        // Validate IPC configuration
        if self.ipc.enabled && self.ipc.endpoint.is_empty() {
            return Err(AccountMappingError::Internal {
                message: "IPC endpoint cannot be empty when IPC is enabled".to_string(),
            });
        }

        // Validate cross-VM configuration
        if self.cross_vm.enabled && self.cross_vm.max_concurrent_operations == 0 {
            return Err(AccountMappingError::Internal {
                message: "Max concurrent operations must be greater than 0".to_string(),
            });
        }

        Ok(())
    }
}

/// Utility functions for account mapping
pub mod utils {
    use super::*;

    /// Create a development configuration
    pub fn create_dev_config() -> AccountMappingConfig {
        let mut config = AccountMappingConfig::default();
        config.storage.backend = StorageBackend::Memory;
        config.validation.strict_mode = false;
        config.validation.require_proof = false;
        config
    }

    /// Create a production configuration
    pub fn create_prod_config() -> AccountMappingConfig {
        let mut config = AccountMappingConfig::default();
        config.storage.backend = StorageBackend::RocksDb;
        config.validation.strict_mode = true;
        config.validation.require_proof = true;
        config.cross_vm.enable_atomic_operations = true;
        config
    }

    /// Validate an address for a specific VM type
    pub fn validate_address(vm_type: VmType, address: &str) -> bool {
        match vm_type {
            VmType::Evm => {
                // Ethereum addresses are 40 hex characters (20 bytes) with optional 0x prefix
                if let Some(addr) = address.strip_prefix("0x") {
                    addr.len() == 40 && addr.chars().all(|c| c.is_ascii_hexdigit())
                } else {
                    address.len() == 40 && address.chars().all(|c| c.is_ascii_hexdigit())
                }
            }
            VmType::Svm => {
                // Solana addresses are base58 encoded, typically 32-44 characters
                address.len() >= 32
                    && address.len() <= 44
                    && address.chars().all(|c| {
                        "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".contains(c)
                    })
            }
        }
    }

    /// Generate a MultiVM account ID from an address
    pub fn generate_multivm_account_id(address: &str) -> String {
        use sha2::{Digest, Sha256};
        let hash = Sha256::digest(address.as_bytes());
        format!("multivm_{}", hex::encode(hash))
    }

    /// Check if two addresses can be bound together
    pub fn can_bind_addresses(
        source_vm: VmType,
        source_address: &str,
        target_vm: VmType,
        target_address: &str,
    ) -> bool {
        // Basic validation
        if source_vm == target_vm {
            return false; // Cannot bind addresses from the same VM
        }

        validate_address(source_vm, source_address) && validate_address(target_vm, target_address)
    }

    /// Format an address for display
    pub fn format_address(vm_type: VmType, address: &str) -> String {
        match vm_type {
            VmType::Evm => {
                if address.len() > 10 {
                    format!("{}...{}", &address[..6], &address[address.len() - 4..])
                } else {
                    address.to_string()
                }
            }
            VmType::Svm => {
                if address.len() > 12 {
                    format!("{}...{}", &address[..8], &address[address.len() - 4..])
                } else {
                    address.to_string()
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let config = AccountMappingConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_address_validation() {
        // Test EVM address validation
        assert!(utils::validate_address(
            VmType::Evm,
            "0x1234567890123456789012345678901234567890"
        ));
        assert!(!utils::validate_address(VmType::Evm, "invalid_address"));

        // Test SVM address validation
        assert!(utils::validate_address(
            VmType::Svm,
            "11111111111111111111111111111111"
        ));
        assert!(!utils::validate_address(VmType::Svm, "short"));
    }

    #[test]
    fn test_can_bind_addresses() {
        let evm_addr = "0x1234567890123456789012345678901234567890";
        let svm_addr = "11111111111111111111111111111111";

        // Should be able to bind different VM types
        assert!(utils::can_bind_addresses(
            VmType::Evm,
            evm_addr,
            VmType::Svm,
            svm_addr
        ));

        // Should not be able to bind same VM types
        assert!(!utils::can_bind_addresses(
            VmType::Evm,
            evm_addr,
            VmType::Evm,
            evm_addr
        ));
    }

    #[test]
    fn test_multivm_account_id_generation() {
        let address = "0x1234567890123456789012345678901234567890";
        let id = utils::generate_multivm_account_id(address);
        assert!(id.starts_with("multivm_"));
        assert_eq!(id.len(), 72); // "multivm_" + 64 hex chars
    }
}
