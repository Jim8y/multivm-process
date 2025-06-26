//! Simple storage implementation without SQLx dependencies

use crate::{
    address::{AccountAddress, MultivmAccountId},
    error::{AccountMappingError, AccountMappingResult},
    mapping::{AccountBinding, AccountMappingLayer, BindingMetadata},
    special_tx::SpecialTransaction,
};
use rocksdb::{Options, DB};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;

/// Configuration for the storage layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Storage backend type
    pub backend: StorageBackend,
    /// Connection string for the storage backend
    pub connection_string: String,
    /// Maximum number of cached entries
    pub cache_size: usize,
    /// Whether to enable write-through caching
    pub enable_cache: bool,
}

/// Supported storage backends
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageBackend {
    /// In-memory storage (for testing)
    Memory,
    /// File-based storage (simple JSON)
    File,
    /// RocksDB persistent storage (recommended for production)
    RocksDB,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: StorageBackend::RocksDB,
            connection_string: "/opt/multivm/data/account_mapping.db".to_string(),
            cache_size: 1000,
            enable_cache: true,
        }
    }
}

/// Trait for account mapping storage backends
#[async_trait::async_trait]
pub trait AccountMappingStorage: Send + Sync {
    /// Store a new account binding
    async fn store_binding(&self, binding: &AccountBinding) -> AccountMappingResult<()>;

    /// Retrieve an account binding by MultiVM account ID
    async fn get_binding(
        &self,
        multivm_id: &MultivmAccountId,
    ) -> AccountMappingResult<Option<AccountBinding>>;

    /// Retrieve an account binding by VM-specific account
    async fn get_binding_by_account(
        &self,
        account: &AccountAddress,
    ) -> AccountMappingResult<Option<AccountBinding>>;

    /// Update an existing account binding
    async fn update_binding(&self, binding: &AccountBinding) -> AccountMappingResult<()>;

    /// Delete an account binding
    async fn delete_binding(&self, multivm_id: &MultivmAccountId) -> AccountMappingResult<()>;

    /// List all bindings with pagination
    async fn list_bindings(
        &self,
        offset: usize,
        limit: usize,
    ) -> AccountMappingResult<Vec<AccountBinding>>;

    /// Count total number of bindings
    async fn count_bindings(&self) -> AccountMappingResult<usize>;

    /// Check if a binding exists
    async fn binding_exists(&self, multivm_id: &MultivmAccountId) -> AccountMappingResult<bool>;

    /// Resolve VM account to MultiVM account ID
    async fn resolve_multivm_account(
        &self,
        account: &AccountAddress,
    ) -> AccountMappingResult<Option<MultivmAccountId>>;

    /// Add an automatic binding for a new account
    async fn add_auto_binding(
        &self,
        account: AccountAddress,
    ) -> AccountMappingResult<MultivmAccountId>;
}

/// In-memory storage implementation
#[derive(Debug)]
pub struct MemoryStorage {
    bindings: Arc<RwLock<HashMap<MultivmAccountId, AccountBinding>>>,
    reverse_lookup: Arc<RwLock<HashMap<AccountAddress, MultivmAccountId>>>,
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryStorage {
    /// Create a new in-memory storage
    pub fn new() -> Self {
        Self {
            bindings: Arc::new(RwLock::new(HashMap::new())),
            reverse_lookup: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl AccountMappingStorage for MemoryStorage {
    async fn store_binding(&self, binding: &AccountBinding) -> AccountMappingResult<()> {
        let mut bindings = self.bindings.write().await;
        let mut reverse_lookup = self.reverse_lookup.write().await;

        // Store the binding
        bindings.insert(binding.multivm_account.clone(), binding.clone());

        // Update reverse lookup
        if let Some(ref svm_account) = binding.svm_account {
            reverse_lookup.insert(svm_account.clone(), binding.multivm_account.clone());
        }
        if let Some(ref evm_account) = binding.evm_account {
            reverse_lookup.insert(evm_account.clone(), binding.multivm_account.clone());
        }

        Ok(())
    }

    async fn get_binding(
        &self,
        multivm_id: &MultivmAccountId,
    ) -> AccountMappingResult<Option<AccountBinding>> {
        let bindings = self.bindings.read().await;
        Ok(bindings.get(multivm_id).cloned())
    }

    async fn get_binding_by_account(
        &self,
        account: &AccountAddress,
    ) -> AccountMappingResult<Option<AccountBinding>> {
        let reverse_lookup = self.reverse_lookup.read().await;
        if let Some(multivm_id) = reverse_lookup.get(account).cloned() {
            drop(reverse_lookup);
            AccountMappingStorage::get_binding(self, &multivm_id).await
        } else {
            Ok(None)
        }
    }

    async fn update_binding(&self, binding: &AccountBinding) -> AccountMappingResult<()> {
        self.store_binding(binding).await
    }

    async fn delete_binding(&self, multivm_id: &MultivmAccountId) -> AccountMappingResult<()> {
        let mut bindings = self.bindings.write().await;
        let mut reverse_lookup = self.reverse_lookup.write().await;

        if let Some(binding) = bindings.remove(multivm_id) {
            // Remove reverse lookups
            if let Some(ref svm_account) = binding.svm_account {
                reverse_lookup.remove(svm_account);
            }
            if let Some(ref evm_account) = binding.evm_account {
                reverse_lookup.remove(evm_account);
            }
        }

        Ok(())
    }

    async fn list_bindings(
        &self,
        offset: usize,
        limit: usize,
    ) -> AccountMappingResult<Vec<AccountBinding>> {
        let bindings = self.bindings.read().await;
        let bindings: Vec<_> = bindings
            .values()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect();
        Ok(bindings)
    }

    async fn count_bindings(&self) -> AccountMappingResult<usize> {
        let bindings = self.bindings.read().await;
        Ok(bindings.len())
    }

    async fn binding_exists(&self, multivm_id: &MultivmAccountId) -> AccountMappingResult<bool> {
        let bindings = self.bindings.read().await;
        Ok(bindings.contains_key(multivm_id))
    }

    async fn resolve_multivm_account(
        &self,
        account: &AccountAddress,
    ) -> AccountMappingResult<Option<MultivmAccountId>> {
        let reverse_lookup = self.reverse_lookup.read().await;
        Ok(reverse_lookup.get(account).cloned())
    }

    /// Add an automatic binding for a new account
    async fn add_auto_binding(
        &self,
        account: AccountAddress,
    ) -> AccountMappingResult<MultivmAccountId> {
        let binding = AccountBinding::create_auto_binding(account.clone());
        let multivm_id = binding.multivm_account.clone();
        self.store_binding(&binding).await?;
        Ok(multivm_id)
    }
}

#[async_trait::async_trait]
impl AccountMappingLayer for MemoryStorage {
    async fn create_binding(&self, binding: AccountBinding) -> AccountMappingResult<()> {
        self.store_binding(&binding).await
    }

    async fn get_binding(
        &self,
        multivm_id: &MultivmAccountId,
    ) -> AccountMappingResult<Option<AccountBinding>> {
        AccountMappingStorage::get_binding(self, multivm_id).await
    }

    async fn get_binding_by_account(
        &self,
        account: &AccountAddress,
    ) -> AccountMappingResult<Option<AccountBinding>> {
        AccountMappingStorage::get_binding_by_account(self, account).await
    }

    async fn update_binding(
        &self,
        multivm_id: &MultivmAccountId,
        metadata: BindingMetadata,
    ) -> AccountMappingResult<()> {
        if let Some(mut binding) = AccountMappingStorage::get_binding(self, multivm_id).await? {
            binding.metadata = metadata;
            self.store_binding(&binding).await
        } else {
            Err(AccountMappingError::Internal {
                message: format!("Binding not found: {}", multivm_id),
            })
        }
    }

    async fn remove_binding(&self, multivm_id: &MultivmAccountId) -> AccountMappingResult<()> {
        AccountMappingStorage::delete_binding(self, multivm_id).await
    }

    async fn add_auto_binding(
        &self,
        account: AccountAddress,
    ) -> AccountMappingResult<MultivmAccountId> {
        AccountMappingStorage::add_auto_binding(self, account).await
    }

    async fn resolve_multivm_account(
        &self,
        address: &AccountAddress,
    ) -> AccountMappingResult<Option<MultivmAccountId>> {
        AccountMappingStorage::resolve_multivm_account(self, address).await
    }

    async fn get_bound_addresses(
        &self,
        multivm_id: &MultivmAccountId,
    ) -> AccountMappingResult<Vec<AccountAddress>> {
        if let Some(binding) = AccountMappingStorage::get_binding(self, multivm_id).await? {
            let mut addresses = Vec::new();
            if let Some(ref svm) = binding.svm_account {
                addresses.push(svm.clone());
            }
            if let Some(ref evm) = binding.evm_account {
                addresses.push(evm.clone());
            }
            Ok(addresses)
        } else {
            Ok(Vec::new())
        }
    }

    async fn has_binding(&self, address: &AccountAddress) -> AccountMappingResult<bool> {
        Ok(AccountMappingStorage::get_binding_by_account(self, address)
            .await?
            .is_some())
    }

    async fn process_special_transaction(
        &self,
        special_tx: SpecialTransaction,
    ) -> AccountMappingResult<()> {
        match special_tx {
            SpecialTransaction::AccountBinding {
                source_account,
                target_account,
                proof,
                metadata: _,
            } => {
                // Create or update binding
                let mut binding = if let Some(existing) =
                    AccountMappingStorage::get_binding_by_account(self, &source_account).await?
                {
                    existing
                } else {
                    AccountBinding::create_auto_binding(source_account.clone())
                };

                // Add cross-binding
                binding.add_cross_binding(target_account, proof)?;

                // Note: metadata handling would need type conversion from SimpleBindingMetadata to BindingMetadata
                // For now, we skip metadata updates to avoid type mismatch

                self.create_binding(binding).await
            }
            SpecialTransaction::CrossVmTransfer { .. } => {
                // Cross-VM transfers are handled by the coordinator
                // This is just a placeholder for the trait implementation
                Ok(())
            }
            _ => {
                // Other special transactions
                Ok(())
            }
        }
    }
}

/// File-based storage implementation
#[derive(Debug)]
pub struct FileStorage {
    file_path: String,
    bindings: Arc<RwLock<HashMap<MultivmAccountId, AccountBinding>>>,
    reverse_lookup: Arc<RwLock<HashMap<AccountAddress, MultivmAccountId>>>,
}

impl FileStorage {
    /// Create a new file-based storage
    pub async fn new(file_path: String) -> AccountMappingResult<Self> {
        let storage = Self {
            file_path: file_path.clone(),
            bindings: Arc::new(RwLock::new(HashMap::new())),
            reverse_lookup: Arc::new(RwLock::new(HashMap::new())),
        };

        // Load existing data if file exists
        storage.load_from_file().await?;
        Ok(storage)
    }

    /// Load bindings from file
    async fn load_from_file(&self) -> AccountMappingResult<()> {
        let path = Path::new(&self.file_path);
        if !path.exists() {
            // Create parent directories if they don't exist
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .await
                    .map_err(|e| AccountMappingError::Internal {
                        message: format!("Failed to create storage directory: {}", e),
                    })?;
            }
            // Create empty file with empty bindings
            self.save_to_file().await?;
            return Ok(());
        }

        let contents = fs::read_to_string(&self.file_path).await.map_err(|e| {
            AccountMappingError::Internal {
                message: format!("Failed to read storage file: {}", e),
            }
        })?;

        if contents.trim().is_empty() {
            return Ok(());
        }

        let stored_bindings: Vec<AccountBinding> =
            serde_json::from_str(&contents).map_err(|e| AccountMappingError::Internal {
                message: format!("Failed to deserialize storage file: {}", e),
            })?;

        let mut bindings = self.bindings.write().await;
        let mut reverse_lookup = self.reverse_lookup.write().await;

        for binding in stored_bindings {
            // Update reverse lookup
            if let Some(ref svm_account) = binding.svm_account {
                reverse_lookup.insert(svm_account.clone(), binding.multivm_account.clone());
            }
            if let Some(ref evm_account) = binding.evm_account {
                reverse_lookup.insert(evm_account.clone(), binding.multivm_account.clone());
            }

            // Store binding
            bindings.insert(binding.multivm_account.clone(), binding);
        }

        Ok(())
    }

    /// Save bindings to file
    async fn save_to_file(&self) -> AccountMappingResult<()> {
        let bindings = self.bindings.read().await;
        let bindings_vec: Vec<_> = bindings.values().cloned().collect();

        let contents = serde_json::to_string_pretty(&bindings_vec).map_err(|e| {
            AccountMappingError::Internal {
                message: format!("Failed to serialize bindings: {}", e),
            }
        })?;

        fs::write(&self.file_path, contents)
            .await
            .map_err(|e| AccountMappingError::Internal {
                message: format!("Failed to write storage file: {}", e),
            })?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl AccountMappingStorage for FileStorage {
    async fn store_binding(&self, binding: &AccountBinding) -> AccountMappingResult<()> {
        let mut bindings = self.bindings.write().await;
        let mut reverse_lookup = self.reverse_lookup.write().await;

        // Store the binding
        bindings.insert(binding.multivm_account.clone(), binding.clone());

        // Update reverse lookup
        if let Some(ref svm_account) = binding.svm_account {
            reverse_lookup.insert(svm_account.clone(), binding.multivm_account.clone());
        }
        if let Some(ref evm_account) = binding.evm_account {
            reverse_lookup.insert(evm_account.clone(), binding.multivm_account.clone());
        }

        drop(bindings);
        drop(reverse_lookup);

        // Persist to file
        self.save_to_file().await?;
        Ok(())
    }

    async fn get_binding(
        &self,
        multivm_id: &MultivmAccountId,
    ) -> AccountMappingResult<Option<AccountBinding>> {
        let bindings = self.bindings.read().await;
        Ok(bindings.get(multivm_id).cloned())
    }

    async fn get_binding_by_account(
        &self,
        account: &AccountAddress,
    ) -> AccountMappingResult<Option<AccountBinding>> {
        let reverse_lookup = self.reverse_lookup.read().await;
        if let Some(multivm_id) = reverse_lookup.get(account).cloned() {
            drop(reverse_lookup);
            AccountMappingStorage::get_binding(self, &multivm_id).await
        } else {
            Ok(None)
        }
    }

    async fn update_binding(&self, binding: &AccountBinding) -> AccountMappingResult<()> {
        self.store_binding(binding).await
    }

    async fn delete_binding(&self, multivm_id: &MultivmAccountId) -> AccountMappingResult<()> {
        let mut bindings = self.bindings.write().await;
        let mut reverse_lookup = self.reverse_lookup.write().await;

        if let Some(binding) = bindings.remove(multivm_id) {
            // Remove reverse lookups
            if let Some(ref svm_account) = binding.svm_account {
                reverse_lookup.remove(svm_account);
            }
            if let Some(ref evm_account) = binding.evm_account {
                reverse_lookup.remove(evm_account);
            }
        }

        drop(bindings);
        drop(reverse_lookup);

        // Persist to file
        self.save_to_file().await?;
        Ok(())
    }

    async fn list_bindings(
        &self,
        offset: usize,
        limit: usize,
    ) -> AccountMappingResult<Vec<AccountBinding>> {
        let bindings = self.bindings.read().await;
        let bindings: Vec<_> = bindings
            .values()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect();
        Ok(bindings)
    }

    async fn count_bindings(&self) -> AccountMappingResult<usize> {
        let bindings = self.bindings.read().await;
        Ok(bindings.len())
    }

    async fn binding_exists(&self, multivm_id: &MultivmAccountId) -> AccountMappingResult<bool> {
        let bindings = self.bindings.read().await;
        Ok(bindings.contains_key(multivm_id))
    }

    async fn resolve_multivm_account(
        &self,
        account: &AccountAddress,
    ) -> AccountMappingResult<Option<MultivmAccountId>> {
        let reverse_lookup = self.reverse_lookup.read().await;
        Ok(reverse_lookup.get(account).cloned())
    }

    async fn add_auto_binding(
        &self,
        account: AccountAddress,
    ) -> AccountMappingResult<MultivmAccountId> {
        let binding = AccountBinding::create_auto_binding(account.clone());
        let multivm_id = binding.multivm_account.clone();
        self.store_binding(&binding).await?;
        Ok(multivm_id)
    }
}

#[async_trait::async_trait]
impl AccountMappingLayer for FileStorage {
    async fn create_binding(&self, binding: AccountBinding) -> AccountMappingResult<()> {
        self.store_binding(&binding).await
    }

    async fn get_binding(
        &self,
        multivm_id: &MultivmAccountId,
    ) -> AccountMappingResult<Option<AccountBinding>> {
        AccountMappingStorage::get_binding(self, multivm_id).await
    }

    async fn get_binding_by_account(
        &self,
        account: &AccountAddress,
    ) -> AccountMappingResult<Option<AccountBinding>> {
        AccountMappingStorage::get_binding_by_account(self, account).await
    }

    async fn update_binding(
        &self,
        multivm_id: &MultivmAccountId,
        metadata: BindingMetadata,
    ) -> AccountMappingResult<()> {
        if let Some(mut binding) = AccountMappingStorage::get_binding(self, multivm_id).await? {
            binding.metadata = metadata;
            self.store_binding(&binding).await
        } else {
            Err(AccountMappingError::Internal {
                message: format!("Binding not found: {}", multivm_id),
            })
        }
    }

    async fn remove_binding(&self, multivm_id: &MultivmAccountId) -> AccountMappingResult<()> {
        AccountMappingStorage::delete_binding(self, multivm_id).await
    }

    async fn add_auto_binding(
        &self,
        account: AccountAddress,
    ) -> AccountMappingResult<MultivmAccountId> {
        AccountMappingStorage::add_auto_binding(self, account).await
    }

    async fn resolve_multivm_account(
        &self,
        address: &AccountAddress,
    ) -> AccountMappingResult<Option<MultivmAccountId>> {
        AccountMappingStorage::resolve_multivm_account(self, address).await
    }

    async fn get_bound_addresses(
        &self,
        multivm_id: &MultivmAccountId,
    ) -> AccountMappingResult<Vec<AccountAddress>> {
        if let Some(binding) = AccountMappingStorage::get_binding(self, multivm_id).await? {
            let mut addresses = Vec::new();
            if let Some(ref svm) = binding.svm_account {
                addresses.push(svm.clone());
            }
            if let Some(ref evm) = binding.evm_account {
                addresses.push(evm.clone());
            }
            Ok(addresses)
        } else {
            Ok(Vec::new())
        }
    }

    async fn has_binding(&self, address: &AccountAddress) -> AccountMappingResult<bool> {
        Ok(AccountMappingStorage::get_binding_by_account(self, address)
            .await?
            .is_some())
    }

    async fn process_special_transaction(
        &self,
        special_tx: SpecialTransaction,
    ) -> AccountMappingResult<()> {
        match special_tx {
            SpecialTransaction::AccountBinding {
                source_account,
                target_account,
                proof,
                metadata: _,
            } => {
                // Create or update binding
                let mut binding = if let Some(existing) =
                    AccountMappingStorage::get_binding_by_account(self, &source_account).await?
                {
                    existing
                } else {
                    AccountBinding::create_auto_binding(source_account.clone())
                };

                // Add cross-binding
                binding.add_cross_binding(target_account, proof)?;

                // Note: metadata handling would need type conversion from SimpleBindingMetadata to BindingMetadata
                // For now, we skip metadata updates to avoid type mismatch

                self.create_binding(binding).await
            }
            SpecialTransaction::CrossVmTransfer { .. } => {
                // Cross-VM transfers are handled by the coordinator
                // This is just a placeholder for the trait implementation
                Ok(())
            }
            _ => {
                // Other special transactions
                Ok(())
            }
        }
    }
}

/// RocksDB storage implementation for production use
#[derive(Debug)]
pub struct RocksDBStorage {
    db: Arc<DB>,
}

impl RocksDBStorage {
    /// Create a new RocksDB storage instance
    pub fn new(db_path: &str) -> AccountMappingResult<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);

        let db = DB::open(&opts, db_path).map_err(|e| AccountMappingError::Internal {
            message: format!("Failed to open RocksDB: {}", e),
        })?;

        Ok(Self { db: Arc::new(db) })
    }

    /// Create key for bindings
    fn binding_key(multivm_id: &MultivmAccountId) -> Vec<u8> {
        let mut key = b"binding:".to_vec();
        key.extend_from_slice(&Self::serialize_multivm_id(multivm_id).unwrap_or_default());
        key
    }

    /// Create key for reverse lookup
    fn reverse_key(account: &AccountAddress) -> Vec<u8> {
        let mut key = b"reverse:".to_vec();
        key.extend_from_slice(&Self::serialize_address(account).unwrap_or_default());
        key
    }

    /// Serialize binding to bytes
    fn serialize_binding(binding: &AccountBinding) -> AccountMappingResult<Vec<u8>> {
        bincode::serialize(binding).map_err(|e| AccountMappingError::Internal {
            message: format!("Failed to serialize binding: {}", e),
        })
    }

    /// Deserialize binding from bytes
    fn deserialize_binding(data: &[u8]) -> AccountMappingResult<AccountBinding> {
        bincode::deserialize(data).map_err(|e| AccountMappingError::Internal {
            message: format!("Failed to deserialize binding: {}", e),
        })
    }

    /// Serialize account address to bytes
    fn serialize_address(address: &AccountAddress) -> AccountMappingResult<Vec<u8>> {
        bincode::serialize(address).map_err(|e| AccountMappingError::Internal {
            message: format!("Failed to serialize address: {}", e),
        })
    }

    /// Serialize multivm account id to bytes
    fn serialize_multivm_id(id: &MultivmAccountId) -> AccountMappingResult<Vec<u8>> {
        bincode::serialize(id).map_err(|e| AccountMappingError::Internal {
            message: format!("Failed to serialize multivm ID: {}", e),
        })
    }

    /// Deserialize multivm account id from bytes
    fn deserialize_multivm_id(data: &[u8]) -> AccountMappingResult<MultivmAccountId> {
        bincode::deserialize(data).map_err(|e| AccountMappingError::Internal {
            message: format!("Failed to deserialize multivm ID: {}", e),
        })
    }
}

#[async_trait::async_trait]
impl AccountMappingStorage for RocksDBStorage {
    async fn store_binding(&self, binding: &AccountBinding) -> AccountMappingResult<()> {
        let binding_key = Self::binding_key(&binding.multivm_account);
        let binding_value = Self::serialize_binding(binding)?;
        let multivm_id_bytes = Self::serialize_multivm_id(&binding.multivm_account)?;

        // Store the binding
        self.db
            .put(&binding_key, &binding_value)
            .map_err(|e| AccountMappingError::Internal {
                message: format!("Failed to store binding: {}", e),
            })?;

        // Update reverse lookup
        if let Some(ref svm_account) = binding.svm_account {
            let key = Self::reverse_key(svm_account);
            self.db
                .put(&key, &multivm_id_bytes)
                .map_err(|e| AccountMappingError::Internal {
                    message: format!("Failed to store reverse lookup: {}", e),
                })?;
        }

        if let Some(ref evm_account) = binding.evm_account {
            let key = Self::reverse_key(evm_account);
            self.db
                .put(&key, &multivm_id_bytes)
                .map_err(|e| AccountMappingError::Internal {
                    message: format!("Failed to store reverse lookup: {}", e),
                })?;
        }

        Ok(())
    }

    async fn get_binding(
        &self,
        multivm_id: &MultivmAccountId,
    ) -> AccountMappingResult<Option<AccountBinding>> {
        let key = Self::binding_key(multivm_id);

        match self.db.get(&key) {
            Ok(Some(data)) => {
                let data = &*data;
                let binding = Self::deserialize_binding(data)?;
                Ok(Some(binding))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(AccountMappingError::Internal {
                message: format!("Failed to get binding: {}", e),
            }),
        }
    }

    async fn get_binding_by_account(
        &self,
        account: &AccountAddress,
    ) -> AccountMappingResult<Option<AccountBinding>> {
        let key = Self::reverse_key(account);

        match self.db.get(&key) {
            Ok(Some(multivm_id_bytes)) => {
                let multivm_id_bytes = &*multivm_id_bytes;
                let multivm_id = Self::deserialize_multivm_id(multivm_id_bytes)?;
                AccountMappingStorage::get_binding(self, &multivm_id).await
            }
            Ok(None) => Ok(None),
            Err(e) => Err(AccountMappingError::Internal {
                message: format!("Failed to get binding by account: {}", e),
            }),
        }
    }

    async fn update_binding(&self, binding: &AccountBinding) -> AccountMappingResult<()> {
        // For RocksDB, update is the same as store
        self.store_binding(binding).await
    }

    async fn delete_binding(&self, multivm_id: &MultivmAccountId) -> AccountMappingResult<()> {
        // First get the binding to find reverse lookup keys
        if let Some(binding) = AccountMappingStorage::get_binding(self, multivm_id).await? {
            let binding_key = Self::binding_key(multivm_id);

            // Delete the binding
            self.db
                .delete(&binding_key)
                .map_err(|e| AccountMappingError::Internal {
                    message: format!("Failed to delete binding: {}", e),
                })?;

            // Delete reverse lookups
            if let Some(ref svm_account) = binding.svm_account {
                let key = Self::reverse_key(svm_account);
                self.db
                    .delete(&key)
                    .map_err(|e| AccountMappingError::Internal {
                        message: format!("Failed to delete reverse lookup: {}", e),
                    })?;
            }

            if let Some(ref evm_account) = binding.evm_account {
                let key = Self::reverse_key(evm_account);
                self.db
                    .delete(&key)
                    .map_err(|e| AccountMappingError::Internal {
                        message: format!("Failed to delete reverse lookup: {}", e),
                    })?;
            }
        }

        Ok(())
    }

    async fn list_bindings(
        &self,
        offset: usize,
        limit: usize,
    ) -> AccountMappingResult<Vec<AccountBinding>> {
        let iter = self.db.iterator(rocksdb::IteratorMode::Start);

        let bindings: Result<Vec<_>, _> = iter
            .filter_map(|result| {
                match result {
                    Ok((key, value)) => {
                        let value = &*value;
                        // Only process binding keys (not reverse lookup keys)
                        if key.starts_with(b"binding:") {
                            Some(Self::deserialize_binding(value))
                        } else {
                            None
                        }
                    }
                    Err(e) => Some(Err(AccountMappingError::Internal {
                        message: format!("Iterator error: {}", e),
                    })),
                }
            })
            .skip(offset)
            .take(limit)
            .collect();

        bindings
    }

    async fn count_bindings(&self) -> AccountMappingResult<usize> {
        let iter = self.db.iterator(rocksdb::IteratorMode::Start);
        let count = iter
            .filter(|result| match result {
                Ok((key, _)) => key.starts_with(b"binding:"),
                Err(_) => false,
            })
            .count();
        Ok(count)
    }

    async fn binding_exists(&self, multivm_id: &MultivmAccountId) -> AccountMappingResult<bool> {
        let key = Self::binding_key(multivm_id);

        match self.db.get(&key) {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(AccountMappingError::Internal {
                message: format!("Failed to check binding existence: {}", e),
            }),
        }
    }

    async fn resolve_multivm_account(
        &self,
        account: &AccountAddress,
    ) -> AccountMappingResult<Option<MultivmAccountId>> {
        let key = Self::reverse_key(account);

        match self.db.get(&key) {
            Ok(Some(multivm_id_bytes)) => {
                let multivm_id_bytes = &*multivm_id_bytes;
                let multivm_id = Self::deserialize_multivm_id(multivm_id_bytes)?;
                Ok(Some(multivm_id))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(AccountMappingError::Internal {
                message: format!("Failed to resolve multivm account: {}", e),
            }),
        }
    }

    async fn add_auto_binding(
        &self,
        account: AccountAddress,
    ) -> AccountMappingResult<MultivmAccountId> {
        let binding = AccountBinding::create_auto_binding(account.clone());
        let multivm_id = binding.multivm_account.clone();
        self.store_binding(&binding).await?;
        Ok(multivm_id)
    }
}

#[async_trait::async_trait]
impl AccountMappingLayer for RocksDBStorage {
    async fn create_binding(&self, binding: AccountBinding) -> AccountMappingResult<()> {
        self.store_binding(&binding).await
    }

    async fn get_binding(
        &self,
        multivm_id: &MultivmAccountId,
    ) -> AccountMappingResult<Option<AccountBinding>> {
        AccountMappingStorage::get_binding(self, multivm_id).await
    }

    async fn get_binding_by_account(
        &self,
        account: &AccountAddress,
    ) -> AccountMappingResult<Option<AccountBinding>> {
        AccountMappingStorage::get_binding_by_account(self, account).await
    }

    async fn update_binding(
        &self,
        multivm_id: &MultivmAccountId,
        metadata: BindingMetadata,
    ) -> AccountMappingResult<()> {
        if let Some(mut binding) = AccountMappingStorage::get_binding(self, multivm_id).await? {
            binding.metadata = metadata;
            self.store_binding(&binding).await
        } else {
            Err(AccountMappingError::Internal {
                message: format!("Binding not found: {}", multivm_id),
            })
        }
    }

    async fn remove_binding(&self, multivm_id: &MultivmAccountId) -> AccountMappingResult<()> {
        AccountMappingStorage::delete_binding(self, multivm_id).await
    }

    async fn add_auto_binding(
        &self,
        account: AccountAddress,
    ) -> AccountMappingResult<MultivmAccountId> {
        AccountMappingStorage::add_auto_binding(self, account).await
    }
    async fn resolve_multivm_account(
        &self,
        address: &AccountAddress,
    ) -> AccountMappingResult<Option<MultivmAccountId>> {
        AccountMappingStorage::resolve_multivm_account(self, address).await
    }

    async fn get_bound_addresses(
        &self,
        multivm_id: &MultivmAccountId,
    ) -> AccountMappingResult<Vec<AccountAddress>> {
        if let Some(binding) = AccountMappingStorage::get_binding(self, multivm_id).await? {
            let mut addresses = Vec::new();
            if let Some(ref svm) = binding.svm_account {
                addresses.push(svm.clone());
            }
            if let Some(ref evm) = binding.evm_account {
                addresses.push(evm.clone());
            }
            Ok(addresses)
        } else {
            Ok(Vec::new())
        }
    }

    async fn has_binding(&self, address: &AccountAddress) -> AccountMappingResult<bool> {
        Ok(AccountMappingStorage::get_binding_by_account(self, address)
            .await?
            .is_some())
    }

    async fn process_special_transaction(
        &self,
        special_tx: SpecialTransaction,
    ) -> AccountMappingResult<()> {
        match special_tx {
            SpecialTransaction::AccountBinding {
                source_account,
                target_account,
                proof,
                metadata: _,
            } => {
                // Create or update binding
                let mut binding = if let Some(existing) =
                    AccountMappingStorage::get_binding_by_account(self, &source_account).await?
                {
                    existing
                } else {
                    AccountBinding::create_auto_binding(source_account.clone())
                };

                // Add cross-binding
                binding.add_cross_binding(target_account, proof)?;

                // Note: metadata handling would need type conversion from SimpleBindingMetadata to BindingMetadata
                // For now, we skip metadata updates to avoid type mismatch

                self.create_binding(binding).await
            }
            SpecialTransaction::CrossVmTransfer { .. } => {
                // Cross-VM transfers are handled by the coordinator
                // This is just a placeholder for the trait implementation
                Ok(())
            }
            _ => {
                // Other special transactions
                Ok(())
            }
        }
    }
}

/// Storage factory
pub struct AccountMappingStorageFactory;

impl AccountMappingStorageFactory {
    /// Create a storage instance based on configuration
    pub async fn create(
        config: &StorageConfig,
    ) -> AccountMappingResult<Box<dyn AccountMappingStorage>> {
        match config.backend {
            StorageBackend::Memory => Ok(Box::new(MemoryStorage::new())),
            StorageBackend::File => {
                let file_storage = FileStorage::new(config.connection_string.clone()).await?;
                Ok(Box::new(file_storage))
            }
            StorageBackend::RocksDB => {
                let rocks_storage = RocksDBStorage::new(&config.connection_string)?;
                Ok(Box::new(rocks_storage))
            }
        }
    }
}

/// Type alias for the storage factory
pub type StorageFactory = AccountMappingStorageFactory;
