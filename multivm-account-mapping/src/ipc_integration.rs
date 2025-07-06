//! IPC Integration for Account Mapping
//!
//! This module provides integration with the MultiVM process manager
//! for coordinating account mapping operations across VM processes.

use crate::{
    address::AccountAddress,
    error::AccountMappingError,
    mapping::{AccountBinding, BindingProof},
};
use multivm_common::MultivmResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// IPC message types for account mapping operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum AccountMappingIpcMessage {
    /// Request to bind accounts across VMs
    BindAccounts {
        source_account: AccountAddress,
        target_account: AccountAddress,
        proof: BindingProof,
    },
    /// Response to bind accounts request
    BindAccountsResponse {
        success: bool,
        binding_id: Option<String>,
        error: Option<String>,
    },
    /// Request to unbind accounts
    UnbindAccounts { binding_id: String },
    /// Response to unbind accounts request
    UnbindAccountsResponse {
        success: bool,
        error: Option<String>,
    },
    /// Query account binding status
    QueryBinding { account: AccountAddress },
    /// Response to binding query
    QueryBindingResponse {
        binding: Option<AccountBinding>,
        error: Option<String>,
    },
    /// Notify of cross-VM transaction
    CrossVmTransaction {
        from_vm: String,
        to_vm: String,
        source_account: AccountAddress,
        target_account: AccountAddress,
        amount: u64,
        transaction_id: String,
    },
    /// Response to cross-VM transaction
    CrossVmTransactionResponse {
        success: bool,
        transaction_id: String,
        error: Option<String>,
    },
}

/// IPC client for account mapping operations
pub struct AccountMappingIpcClient {
    /// Connection to process manager
    connection: Arc<RwLock<Option<IpcConnection>>>,
    /// Pending requests
    #[allow(dead_code)]
    pending_requests:
        Arc<RwLock<HashMap<String, tokio::sync::oneshot::Sender<AccountMappingIpcMessage>>>>,
}

/// Mock IPC connection for development
pub struct IpcConnection {
    /// Connection ID
    pub id: String,
    /// Connected to process manager
    pub connected: bool,
}

impl AccountMappingIpcClient {
    /// Create new IPC client
    pub fn new() -> Self {
        Self {
            connection: Arc::new(RwLock::new(None)),
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Connect to process manager
    pub async fn connect(&self, endpoint: &str) -> MultivmResult<()> {
        let mut connection = self.connection.write().await;

        // Mock connection for development
        *connection = Some(IpcConnection {
            id: uuid::Uuid::new_v4().to_string(),
            connected: true,
        });

        tracing::info!("Connected to process manager at {}", endpoint);
        Ok(())
    }

    /// Disconnect from process manager
    pub async fn disconnect(&self) -> MultivmResult<()> {
        let mut connection = self.connection.write().await;
        *connection = None;

        tracing::info!("Disconnected from process manager");
        Ok(())
    }

    /// Send bind accounts request
    pub async fn bind_accounts(
        &self,
        source_account: AccountAddress,
        target_account: AccountAddress,
        proof: BindingProof,
    ) -> MultivmResult<String> {
        let connection = self.connection.read().await;
        if connection.is_none() {
            return Err(AccountMappingError::IpcError {
                message: "Not connected to process manager".to_string(),
            }
            .into());
        }

        let _message = AccountMappingIpcMessage::BindAccounts {
            source_account,
            target_account,
            proof,
        };

        let binding_id = uuid::Uuid::new_v4().to_string();

        tracing::info!("Binding accounts via IPC, binding_id: {}", binding_id);
        Ok(binding_id)
    }

    /// Send unbind accounts request
    pub async fn unbind_accounts(&self, binding_id: &str) -> MultivmResult<()> {
        let connection = self.connection.read().await;
        if connection.is_none() {
            return Err(AccountMappingError::IpcError {
                message: "Not connected to process manager".to_string(),
            }
            .into());
        }

        let _message = AccountMappingIpcMessage::UnbindAccounts {
            binding_id: binding_id.to_string(),
        };

        // Mock implementation
        tracing::info!("Unbinding accounts via IPC, binding_id: {}", binding_id);
        Ok(())
    }

    /// Query account binding
    pub async fn query_binding(
        &self,
        account: AccountAddress,
    ) -> MultivmResult<Option<AccountBinding>> {
        let connection = self.connection.read().await;
        if connection.is_none() {
            return Err(AccountMappingError::IpcError {
                message: "Not connected to process manager".to_string(),
            }
            .into());
        }

        let _message = AccountMappingIpcMessage::QueryBinding { account };

        // Mock implementation - return None for now
        tracing::info!("Querying account binding via IPC");
        Ok(None)
    }

    /// Notify of cross-VM transaction
    pub async fn notify_cross_vm_transaction(
        &self,
        from_vm: &str,
        to_vm: &str,
        source_account: AccountAddress,
        target_account: AccountAddress,
        amount: u64,
        transaction_id: &str,
    ) -> MultivmResult<()> {
        let connection = self.connection.read().await;
        if connection.is_none() {
            return Err(AccountMappingError::IpcError {
                message: "Not connected to process manager".to_string(),
            }
            .into());
        }

        let _message = AccountMappingIpcMessage::CrossVmTransaction {
            from_vm: from_vm.to_string(),
            to_vm: to_vm.to_string(),
            source_account,
            target_account,
            amount,
            transaction_id: transaction_id.to_string(),
        };

        // Mock implementation
        tracing::info!(
            "Notifying cross-VM transaction via IPC: {} -> {}, amount: {}, tx: {}",
            from_vm,
            to_vm,
            amount,
            transaction_id
        );
        Ok(())
    }

    /// Check if connected to process manager
    pub async fn is_connected(&self) -> bool {
        let connection = self.connection.read().await;
        connection.as_ref().is_some_and(|c| c.connected)
    }

    /// Get connection status
    pub async fn connection_status(&self) -> Option<String> {
        let connection = self.connection.read().await;
        connection.as_ref().map(|c| c.id.clone())
    }
}

impl Default for AccountMappingIpcClient {
    fn default() -> Self {
        Self::new()
    }
}

/// IPC message handler for account mapping
pub struct AccountMappingIpcHandler {
    /// Account mapping client
    client: Arc<AccountMappingIpcClient>,
}

impl AccountMappingIpcHandler {
    /// Create new IPC handler
    pub fn new(client: Arc<AccountMappingIpcClient>) -> Self {
        Self { client }
    }

    /// Handle incoming IPC message
    pub async fn handle_message(
        &self,
        message: AccountMappingIpcMessage,
    ) -> MultivmResult<AccountMappingIpcMessage> {
        match message {
            AccountMappingIpcMessage::BindAccounts {
                source_account,
                target_account,
                proof,
            } => {
                match self
                    .client
                    .bind_accounts(source_account, target_account, proof)
                    .await
                {
                    Ok(binding_id) => Ok(AccountMappingIpcMessage::BindAccountsResponse {
                        success: true,
                        binding_id: Some(binding_id),
                        error: None,
                    }),
                    Err(e) => Ok(AccountMappingIpcMessage::BindAccountsResponse {
                        success: false,
                        binding_id: None,
                        error: Some(e.to_string()),
                    }),
                }
            }
            AccountMappingIpcMessage::UnbindAccounts { binding_id } => {
                match self.client.unbind_accounts(&binding_id).await {
                    Ok(()) => Ok(AccountMappingIpcMessage::UnbindAccountsResponse {
                        success: true,
                        error: None,
                    }),
                    Err(e) => Ok(AccountMappingIpcMessage::UnbindAccountsResponse {
                        success: false,
                        error: Some(e.to_string()),
                    }),
                }
            }
            AccountMappingIpcMessage::QueryBinding { account } => {
                match self.client.query_binding(account).await {
                    Ok(binding) => Ok(AccountMappingIpcMessage::QueryBindingResponse {
                        binding,
                        error: None,
                    }),
                    Err(e) => Ok(AccountMappingIpcMessage::QueryBindingResponse {
                        binding: None,
                        error: Some(e.to_string()),
                    }),
                }
            }
            AccountMappingIpcMessage::CrossVmTransaction {
                from_vm,
                to_vm,
                source_account,
                target_account,
                amount,
                transaction_id,
            } => {
                match self
                    .client
                    .notify_cross_vm_transaction(
                        &from_vm,
                        &to_vm,
                        source_account,
                        target_account,
                        amount,
                        &transaction_id,
                    )
                    .await
                {
                    Ok(()) => Ok(AccountMappingIpcMessage::CrossVmTransactionResponse {
                        success: true,
                        transaction_id,
                        error: None,
                    }),
                    Err(e) => Ok(AccountMappingIpcMessage::CrossVmTransactionResponse {
                        success: false,
                        transaction_id,
                        error: Some(e.to_string()),
                    }),
                }
            }
            // Response messages are handled by the client
            _ => Err(AccountMappingError::IpcError {
                message: "Unexpected response message".to_string(),
            }
            .into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address::{EthereumAddress, SolanaAddress};
    use crate::mapping::ProofType;

    #[tokio::test]
    async fn test_ipc_client_connection() {
        let client = AccountMappingIpcClient::new();

        assert!(!client.is_connected().await);

        client.connect("mock://localhost:8080").await.unwrap();
        assert!(client.is_connected().await);

        client.disconnect().await.unwrap();
        assert!(!client.is_connected().await);
    }

    #[tokio::test]
    async fn test_bind_accounts_via_ipc() {
        let client = AccountMappingIpcClient::new();
        client.connect("mock://localhost:8080").await.unwrap();

        let source_account = AccountAddress::Solana(SolanaAddress([1u8; 32]));
        let target_account = AccountAddress::Ethereum(EthereumAddress([2u8; 20]));
        let proof = BindingProof {
            account: source_account.clone(),
            proof_type: ProofType::Signature {
                message: Box::new(b"test message".to_vec()),
                signature: Box::new(vec![1, 2, 3, 4]),
            },
            proof_data: Box::new(vec![5, 6, 7, 8]),
            timestamp: std::time::SystemTime::now(),
            nonce: 1,
        };

        let binding_id = client
            .bind_accounts(source_account, target_account, proof)
            .await
            .unwrap();
        assert!(!binding_id.is_empty());
    }
}
