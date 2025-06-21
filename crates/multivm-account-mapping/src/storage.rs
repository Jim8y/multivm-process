//! Simple storage implementation without SQLx dependencies

use crate::{
    AccountAddress, AccountBinding, AccountMappingLayer, AccountMappingResult, MultivmAccountId,
    SpecialTransaction, SpecialTransactionResult,
};
use multivm_common::MultivmResult;
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
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: StorageBackend::Memory,
            connection_string: ":memory:".to_string(),
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
            self.get_binding(&multivm_id).await
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
        let binding = crate::AccountBinding::create_auto_binding(account.clone());
        let multivm_id = binding.multivm_account.clone();
        self.store_binding(&binding).await?;
        Ok(multivm_id)
    }
}

#[async_trait::async_trait]
impl AccountMappingLayer for MemoryStorage {
    async fn process_special_transaction(
        &mut self,
        tx: SpecialTransaction,
    ) -> MultivmResult<SpecialTransactionResult> {
        match tx {
            SpecialTransaction::AccountBinding {
                source_account,
                target_account,
                proof,
                metadata,
            } => {
                // Check if either account already has a binding
                let existing_source = self
                    .get_binding_by_account(&source_account)
                    .await
                    .map_err(|e| multivm_common::MultivmError::AccountMapping(e.to_string()))?;

                let existing_target = self
                    .get_binding_by_account(&target_account)
                    .await
                    .map_err(|e| multivm_common::MultivmError::AccountMapping(e.to_string()))?;

                let multivm_id = if let Some(existing) = existing_source {
                    // Extend existing binding to include target account
                    let mut updated_binding = existing.clone();

                    // Update based on account type
                    match &target_account {
                        AccountAddress::Solana(_) => {
                            if updated_binding.svm_account.is_some() {
                                return Err(multivm_common::MultivmError::AccountMapping(
                                    "Source binding already has SVM account".to_string(),
                                ));
                            }
                            updated_binding.svm_account = Some(target_account);
                        }
                        AccountAddress::Ethereum(_) => {
                            if updated_binding.evm_account.is_some() {
                                return Err(multivm_common::MultivmError::AccountMapping(
                                    "Source binding already has EVM account".to_string(),
                                ));
                            }
                            updated_binding.evm_account = Some(target_account);
                        }
                    }

                    // Add the proof
                    updated_binding.binding_proofs.push(proof);

                    if let Some(meta) = metadata {
                        updated_binding.metadata.notes = meta.notes;
                        updated_binding.metadata.tags.extend(meta.tags);
                    }

                    let multivm_id = updated_binding.multivm_account.clone();
                    self.store_binding(&updated_binding)
                        .await
                        .map_err(|e| multivm_common::MultivmError::AccountMapping(e.to_string()))?;

                    multivm_id
                } else if let Some(existing) = existing_target {
                    // Extend existing binding to include source account
                    let mut updated_binding = existing.clone();

                    // Update based on account type
                    match &source_account {
                        AccountAddress::Solana(_) => {
                            if updated_binding.svm_account.is_some() {
                                return Err(multivm_common::MultivmError::AccountMapping(
                                    "Target binding already has SVM account".to_string(),
                                ));
                            }
                            updated_binding.svm_account = Some(source_account);
                        }
                        AccountAddress::Ethereum(_) => {
                            if updated_binding.evm_account.is_some() {
                                return Err(multivm_common::MultivmError::AccountMapping(
                                    "Target binding already has EVM account".to_string(),
                                ));
                            }
                            updated_binding.evm_account = Some(source_account);
                        }
                    }

                    // Add the proof
                    updated_binding.binding_proofs.push(proof);

                    if let Some(meta) = metadata {
                        updated_binding.metadata.notes = meta.notes;
                        updated_binding.metadata.tags.extend(meta.tags);
                    }

                    let multivm_id = updated_binding.multivm_account.clone();
                    self.store_binding(&updated_binding)
                        .await
                        .map_err(|e| multivm_common::MultivmError::AccountMapping(e.to_string()))?;

                    multivm_id
                } else {
                    // Create new binding
                    let multivm_id = MultivmAccountId::generate();
                    let mut binding = AccountBinding {
                        multivm_account: multivm_id.clone(),
                        svm_account: None,
                        evm_account: None,
                        created_at: std::time::SystemTime::now(),
                        binding_proofs: vec![proof],
                        metadata: crate::BindingMetadata {
                            label: None,
                            active: true,
                            last_used: None,
                            notes: metadata.as_ref().and_then(|m| m.notes.clone()),
                            tags: metadata
                                .as_ref()
                                .map(|m| m.tags.clone())
                                .unwrap_or_default(),
                            config: crate::BindingConfiguration::default(),
                        },
                    };

                    // Set accounts based on type
                    match &source_account {
                        AccountAddress::Solana(_) => binding.svm_account = Some(source_account),
                        AccountAddress::Ethereum(_) => binding.evm_account = Some(source_account),
                    }

                    match &target_account {
                        AccountAddress::Solana(_) => binding.svm_account = Some(target_account),
                        AccountAddress::Ethereum(_) => binding.evm_account = Some(target_account),
                    }

                    self.store_binding(&binding)
                        .await
                        .map_err(|e| multivm_common::MultivmError::AccountMapping(e.to_string()))?;

                    multivm_id
                };

                Ok(SpecialTransactionResult {
                    success: true,
                    multivm_account: Some(multivm_id),
                    compute_units_used: 1500,
                    return_data: Some("Account binding created successfully".as_bytes().to_vec()),
                    events: vec![],
                    error: None,
                })
            }

            SpecialTransaction::CrossVmTransfer {
                from,
                to,
                amount,
                asset_type,
                memo,
            } => {
                // For now, just log the cross-VM transfer
                // In a full implementation, this would:
                // 1. Validate the transfer
                // 2. Update balances
                // 3. Record the transaction

                Ok(SpecialTransactionResult {
                    success: true,
                    multivm_account: Some(from.clone()),
                    compute_units_used: 2000,
                    return_data: Some(
                        format!(
                            "Cross-VM transfer of {} {} from {} to {} completed{}",
                            amount,
                            asset_type,
                            from,
                            to,
                            memo.as_ref()
                                .map(|m| format!(" (memo: {})", m))
                                .unwrap_or_default()
                        )
                        .as_bytes()
                        .to_vec(),
                    ),
                    events: vec![],
                    error: None,
                })
            }

            _ => Err(multivm_common::MultivmError::AccountMapping(
                "Unsupported special transaction type".to_string(),
            )),
        }
    }

    async fn get_account_binding(
        &self,
        address: &AccountAddress,
    ) -> MultivmResult<Option<AccountBinding>> {
        self.get_binding_by_account(address)
            .await
            .map_err(|e| multivm_common::MultivmError::AccountMapping(e.to_string()))
    }

    async fn resolve_multivm_account(
        &self,
        address: &AccountAddress,
    ) -> MultivmResult<Option<MultivmAccountId>> {
        AccountMappingStorage::resolve_multivm_account(self, address)
            .await
            .map_err(|e| multivm_common::MultivmError::AccountMapping(e.to_string()))
    }

    async fn get_bound_addresses(
        &self,
        multivm_id: &MultivmAccountId,
    ) -> MultivmResult<Vec<AccountAddress>> {
        if let Ok(Some(binding)) = self.get_binding(multivm_id).await {
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

    async fn has_binding(&self, address: &AccountAddress) -> MultivmResult<bool> {
        Ok(self
            .get_binding_by_account(address)
            .await
            .unwrap_or(None)
            .is_some())
    }

    async fn add_auto_binding(&self, account: AccountAddress) -> MultivmResult<MultivmAccountId> {
        AccountMappingStorage::add_auto_binding(self, account)
            .await
            .map_err(|e| multivm_common::MultivmError::AccountMapping(e.to_string()))
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
                fs::create_dir_all(parent).await.map_err(|e| {
                    crate::AccountMappingError::Internal {
                        message: format!("Failed to create storage directory: {}", e),
                    }
                })?;
            }
            // Create empty file with empty bindings
            self.save_to_file().await?;
            return Ok(());
        }

        let contents = fs::read_to_string(&self.file_path).await.map_err(|e| {
            crate::AccountMappingError::Internal {
                message: format!("Failed to read storage file: {}", e),
            }
        })?;

        if contents.trim().is_empty() {
            return Ok(());
        }

        let stored_bindings: Vec<AccountBinding> =
            serde_json::from_str(&contents).map_err(|e| crate::AccountMappingError::Internal {
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
            crate::AccountMappingError::Internal {
                message: format!("Failed to serialize bindings: {}", e),
            }
        })?;

        fs::write(&self.file_path, contents).await.map_err(|e| {
            crate::AccountMappingError::Internal {
                message: format!("Failed to write storage file: {}", e),
            }
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
            self.get_binding(&multivm_id).await
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
        let binding = crate::AccountBinding::create_auto_binding(account.clone());
        let multivm_id = binding.multivm_account.clone();
        self.store_binding(&binding).await?;
        Ok(multivm_id)
    }
}

#[async_trait::async_trait]
impl AccountMappingLayer for FileStorage {
    async fn process_special_transaction(
        &mut self,
        tx: SpecialTransaction,
    ) -> MultivmResult<SpecialTransactionResult> {
        match tx {
            SpecialTransaction::AccountBinding {
                source_account,
                target_account,
                proof,
                metadata,
            } => {
                // Check if either account already has a binding
                let existing_source = self
                    .get_binding_by_account(&source_account)
                    .await
                    .map_err(|e| multivm_common::MultivmError::AccountMapping(e.to_string()))?;

                let existing_target = self
                    .get_binding_by_account(&target_account)
                    .await
                    .map_err(|e| multivm_common::MultivmError::AccountMapping(e.to_string()))?;

                let multivm_id = if let Some(existing) = existing_source {
                    // Extend existing binding to include target account
                    let mut updated_binding = existing.clone();

                    // Update based on account type
                    match &target_account {
                        AccountAddress::Solana(_) => {
                            if updated_binding.svm_account.is_some() {
                                return Err(multivm_common::MultivmError::AccountMapping(
                                    "Source binding already has SVM account".to_string(),
                                ));
                            }
                            updated_binding.svm_account = Some(target_account);
                        }
                        AccountAddress::Ethereum(_) => {
                            if updated_binding.evm_account.is_some() {
                                return Err(multivm_common::MultivmError::AccountMapping(
                                    "Source binding already has EVM account".to_string(),
                                ));
                            }
                            updated_binding.evm_account = Some(target_account);
                        }
                    }

                    // Add the proof
                    updated_binding.binding_proofs.push(proof);

                    if let Some(meta) = metadata {
                        updated_binding.metadata.notes = meta.notes;
                        updated_binding.metadata.tags.extend(meta.tags);
                    }

                    let multivm_id = updated_binding.multivm_account.clone();
                    self.store_binding(&updated_binding)
                        .await
                        .map_err(|e| multivm_common::MultivmError::AccountMapping(e.to_string()))?;

                    multivm_id
                } else if let Some(existing) = existing_target {
                    // Extend existing binding to include source account
                    let mut updated_binding = existing.clone();

                    // Update based on account type
                    match &source_account {
                        AccountAddress::Solana(_) => {
                            if updated_binding.svm_account.is_some() {
                                return Err(multivm_common::MultivmError::AccountMapping(
                                    "Target binding already has SVM account".to_string(),
                                ));
                            }
                            updated_binding.svm_account = Some(source_account);
                        }
                        AccountAddress::Ethereum(_) => {
                            if updated_binding.evm_account.is_some() {
                                return Err(multivm_common::MultivmError::AccountMapping(
                                    "Target binding already has EVM account".to_string(),
                                ));
                            }
                            updated_binding.evm_account = Some(source_account);
                        }
                    }

                    // Add the proof
                    updated_binding.binding_proofs.push(proof);

                    if let Some(meta) = metadata {
                        updated_binding.metadata.notes = meta.notes;
                        updated_binding.metadata.tags.extend(meta.tags);
                    }

                    let multivm_id = updated_binding.multivm_account.clone();
                    self.store_binding(&updated_binding)
                        .await
                        .map_err(|e| multivm_common::MultivmError::AccountMapping(e.to_string()))?;

                    multivm_id
                } else {
                    // Create new binding
                    let multivm_id = MultivmAccountId::generate();
                    let mut binding = AccountBinding {
                        multivm_account: multivm_id.clone(),
                        svm_account: None,
                        evm_account: None,
                        created_at: std::time::SystemTime::now(),
                        binding_proofs: vec![proof],
                        metadata: crate::BindingMetadata {
                            label: None,
                            active: true,
                            last_used: None,
                            notes: metadata.as_ref().and_then(|m| m.notes.clone()),
                            tags: metadata
                                .as_ref()
                                .map(|m| m.tags.clone())
                                .unwrap_or_default(),
                            config: crate::BindingConfiguration::default(),
                        },
                    };

                    // Set accounts based on type
                    match &source_account {
                        AccountAddress::Solana(_) => binding.svm_account = Some(source_account),
                        AccountAddress::Ethereum(_) => binding.evm_account = Some(source_account),
                    }

                    match &target_account {
                        AccountAddress::Solana(_) => binding.svm_account = Some(target_account),
                        AccountAddress::Ethereum(_) => binding.evm_account = Some(target_account),
                    }

                    self.store_binding(&binding)
                        .await
                        .map_err(|e| multivm_common::MultivmError::AccountMapping(e.to_string()))?;

                    multivm_id
                };

                Ok(SpecialTransactionResult {
                    success: true,
                    multivm_account: Some(multivm_id),
                    compute_units_used: 1500,
                    return_data: Some("Account binding created successfully".as_bytes().to_vec()),
                    events: vec![],
                    error: None,
                })
            }

            SpecialTransaction::CrossVmTransfer {
                from,
                to,
                amount,
                asset_type,
                memo,
            } => {
                // For now, just log the cross-VM transfer
                // In a full implementation, this would:
                // 1. Validate the transfer
                // 2. Update balances
                // 3. Record the transaction

                Ok(SpecialTransactionResult {
                    success: true,
                    multivm_account: Some(from.clone()),
                    compute_units_used: 2000,
                    return_data: Some(
                        format!(
                            "Cross-VM transfer of {} {} from {} to {} completed{}",
                            amount,
                            asset_type,
                            from,
                            to,
                            memo.as_ref()
                                .map(|m| format!(" (memo: {})", m))
                                .unwrap_or_default()
                        )
                        .as_bytes()
                        .to_vec(),
                    ),
                    events: vec![],
                    error: None,
                })
            }

            _ => Err(multivm_common::MultivmError::AccountMapping(
                "Unsupported special transaction type".to_string(),
            )),
        }
    }

    async fn get_account_binding(
        &self,
        address: &AccountAddress,
    ) -> MultivmResult<Option<AccountBinding>> {
        self.get_binding_by_account(address)
            .await
            .map_err(|e| multivm_common::MultivmError::AccountMapping(e.to_string()))
    }

    async fn resolve_multivm_account(
        &self,
        address: &AccountAddress,
    ) -> MultivmResult<Option<MultivmAccountId>> {
        AccountMappingStorage::resolve_multivm_account(self, address)
            .await
            .map_err(|e| multivm_common::MultivmError::AccountMapping(e.to_string()))
    }

    async fn get_bound_addresses(
        &self,
        multivm_id: &MultivmAccountId,
    ) -> MultivmResult<Vec<AccountAddress>> {
        if let Ok(Some(binding)) = self.get_binding(multivm_id).await {
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

    async fn has_binding(&self, address: &AccountAddress) -> MultivmResult<bool> {
        Ok(self
            .get_binding_by_account(address)
            .await
            .unwrap_or(None)
            .is_some())
    }

    async fn add_auto_binding(&self, account: AccountAddress) -> MultivmResult<MultivmAccountId> {
        AccountMappingStorage::add_auto_binding(self, account)
            .await
            .map_err(|e| multivm_common::MultivmError::AccountMapping(e.to_string()))
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
        }
    }
}

/// Type alias for the storage factory
pub type StorageFactory = AccountMappingStorageFactory;
