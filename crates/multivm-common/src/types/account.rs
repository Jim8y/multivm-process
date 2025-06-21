//! Account-related common types

use serde::{Deserialize, Serialize};

/// Account binding information used across modules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBindingInfo {
    /// MultiVM account identifier
    pub multivm_id: String,
    /// Solana (SVM) address if bound
    pub svm_address: Option<String>,
    /// Ethereum (EVM) address if bound
    pub evm_address: Option<String>,
    /// When the binding was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last activity on any bound address
    pub last_activity: chrono::DateTime<chrono::Utc>,
    /// Additional metadata for extensibility
    pub metadata: serde_json::Value,
}

impl AccountBindingInfo {
    /// Create a new account binding info
    pub fn new(multivm_id: String) -> Self {
        let now = chrono::Utc::now();
        Self {
            multivm_id,
            svm_address: None,
            evm_address: None,
            created_at: now,
            last_activity: now,
            metadata: serde_json::Value::Null,
        }
    }

    /// Check if the account has any bindings
    pub fn has_bindings(&self) -> bool {
        self.svm_address.is_some() || self.evm_address.is_some()
    }

    /// Get all bound addresses
    pub fn get_bound_addresses(&self) -> Vec<String> {
        let mut addresses = Vec::new();
        if let Some(ref addr) = self.svm_address {
            addresses.push(addr.clone());
        }
        if let Some(ref addr) = self.evm_address {
            addresses.push(addr.clone());
        }
        addresses
    }
}
