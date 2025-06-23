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
// TODO: Re-enable when IPC types are available in multivm-common
// pub mod ipc_integration;
pub mod mapping;
pub mod special_tx;
pub mod storage;
pub mod validation;
pub mod vm_engines;

#[cfg(test)]
mod validation_tests;

// TODO: Re-enable when all dependencies are available
// #[cfg(test)]
// mod integration_tests;

// Re-exports for public API
pub use address::*;
pub use atomic_coordinator::*;
pub use cross_vm_coordinator::*;
pub use error::*;
// pub use ipc_integration::*;
pub use mapping::*;
pub use special_tx::*;
pub use storage::*;
pub use validation::*;
pub use vm_engines::*;

use multivm_common::MultivmResult;

/// Account mapping layer trait for integration with other system components
#[async_trait::async_trait]
pub trait AccountMappingLayer: Send + Sync {
    /// Process a special transaction (binding, cross-VM transfer, etc.)
    async fn process_special_transaction(
        &self,
        tx: SpecialTransaction,
    ) -> MultivmResult<SpecialTransactionResult>;

    /// Get account binding for a given address
    async fn get_account_binding(
        &self,
        address: &AccountAddress,
    ) -> MultivmResult<Option<AccountBinding>>;

    /// Get MultiVM account ID for a given VM-specific address
    async fn resolve_multivm_account(
        &self,
        address: &AccountAddress,
    ) -> MultivmResult<Option<MultivmAccountId>>;

    /// Get all bound addresses for a MultiVM account
    async fn get_bound_addresses(
        &self,
        multivm_id: &MultivmAccountId,
    ) -> MultivmResult<Vec<AccountAddress>>;

    /// Check if an account binding exists
    async fn has_binding(&self, address: &AccountAddress) -> MultivmResult<bool>;

    /// Create an automatic binding for a new account
    async fn add_auto_binding(&self, account: AccountAddress) -> MultivmResult<MultivmAccountId>;
}
