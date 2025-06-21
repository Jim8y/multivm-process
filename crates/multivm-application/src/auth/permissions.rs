use crate::error::{ApplicationError, AuthResult};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Permission types for different API operations
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    // SVM permissions
    ReadSvmAccounts,
    ReadSvmTransactions,
    ReadSvmBlocks,
    SubmitSvmTransactions,
    QuerySvmPrograms,

    // EVM permissions
    ReadEvmAccounts,
    ReadEvmTransactions,
    ReadEvmBlocks,
    SubmitEvmTransactions,
    QueryEvmContracts,

    // MultiVM permissions
    BindAccounts,
    UnbindAccounts,
    CrossVmTransfer,
    ReadAccountBindings,

    // System permissions
    ReadSystemStatus,
    ReadNetworkInfo,
    ReadMetrics,
    ReadHealthChecks,

    // Admin permissions
    AdminAccess,
    ManageApiKeys,
    ManageUsers,
    SystemConfiguration,

    // WebSocket permissions
    SubscribeToBlocks,
    SubscribeToTransactions,
    SubscribeToEvents,

    // Advanced permissions
    BatchOperations,
    AdvancedQueries,
    DebugAccess,
}

impl Permission {
    /// Get all available permissions
    pub fn all() -> Vec<Permission> {
        vec![
            // SVM permissions
            Permission::ReadSvmAccounts,
            Permission::ReadSvmTransactions,
            Permission::ReadSvmBlocks,
            Permission::SubmitSvmTransactions,
            Permission::QuerySvmPrograms,
            // EVM permissions
            Permission::ReadEvmAccounts,
            Permission::ReadEvmTransactions,
            Permission::ReadEvmBlocks,
            Permission::SubmitEvmTransactions,
            Permission::QueryEvmContracts,
            // MultiVM permissions
            Permission::BindAccounts,
            Permission::UnbindAccounts,
            Permission::CrossVmTransfer,
            Permission::ReadAccountBindings,
            // System permissions
            Permission::ReadSystemStatus,
            Permission::ReadNetworkInfo,
            Permission::ReadMetrics,
            Permission::ReadHealthChecks,
            // Admin permissions
            Permission::AdminAccess,
            Permission::ManageApiKeys,
            Permission::ManageUsers,
            Permission::SystemConfiguration,
            // WebSocket permissions
            Permission::SubscribeToBlocks,
            Permission::SubscribeToTransactions,
            Permission::SubscribeToEvents,
            // Advanced permissions
            Permission::BatchOperations,
            Permission::AdvancedQueries,
            Permission::DebugAccess,
        ]
    }

    /// Get default permissions for regular users
    pub fn default_user() -> Vec<Permission> {
        vec![
            Permission::ReadSvmAccounts,
            Permission::ReadSvmTransactions,
            Permission::ReadSvmBlocks,
            Permission::ReadEvmAccounts,
            Permission::ReadEvmTransactions,
            Permission::ReadEvmBlocks,
            Permission::ReadAccountBindings,
            Permission::ReadSystemStatus,
            Permission::ReadNetworkInfo,
            Permission::SubscribeToBlocks,
            Permission::SubscribeToTransactions,
        ]
    }

    /// Get default permissions for power users
    pub fn power_user() -> Vec<Permission> {
        let mut permissions = Self::default_user();
        permissions.extend(vec![
            Permission::SubmitSvmTransactions,
            Permission::SubmitEvmTransactions,
            Permission::BindAccounts,
            Permission::UnbindAccounts,
            Permission::CrossVmTransfer,
            Permission::QuerySvmPrograms,
            Permission::QueryEvmContracts,
            Permission::SubscribeToEvents,
            Permission::BatchOperations,
            Permission::AdvancedQueries,
        ]);
        permissions
    }

    /// Get admin permissions
    pub fn admin() -> Vec<Permission> {
        Self::all()
    }

    /// Get permission category
    pub fn category(&self) -> PermissionCategory {
        match self {
            Permission::ReadSvmAccounts
            | Permission::ReadSvmTransactions
            | Permission::ReadSvmBlocks
            | Permission::SubmitSvmTransactions
            | Permission::QuerySvmPrograms => PermissionCategory::Svm,

            Permission::ReadEvmAccounts
            | Permission::ReadEvmTransactions
            | Permission::ReadEvmBlocks
            | Permission::SubmitEvmTransactions
            | Permission::QueryEvmContracts => PermissionCategory::Evm,

            Permission::BindAccounts
            | Permission::UnbindAccounts
            | Permission::CrossVmTransfer
            | Permission::ReadAccountBindings => PermissionCategory::MultiVm,

            Permission::ReadSystemStatus
            | Permission::ReadNetworkInfo
            | Permission::ReadMetrics
            | Permission::ReadHealthChecks => PermissionCategory::System,

            Permission::AdminAccess
            | Permission::ManageApiKeys
            | Permission::ManageUsers
            | Permission::SystemConfiguration => PermissionCategory::Admin,

            Permission::SubscribeToBlocks
            | Permission::SubscribeToTransactions
            | Permission::SubscribeToEvents => PermissionCategory::WebSocket,

            Permission::BatchOperations | Permission::AdvancedQueries | Permission::DebugAccess => {
                PermissionCategory::Advanced
            }
        }
    }

    /// Check if permission is read-only
    pub fn is_read_only(&self) -> bool {
        matches!(
            self,
            Permission::ReadSvmAccounts
                | Permission::ReadSvmTransactions
                | Permission::ReadSvmBlocks
                | Permission::ReadEvmAccounts
                | Permission::ReadEvmTransactions
                | Permission::ReadEvmBlocks
                | Permission::ReadAccountBindings
                | Permission::ReadSystemStatus
                | Permission::ReadNetworkInfo
                | Permission::ReadMetrics
                | Permission::ReadHealthChecks
                | Permission::QuerySvmPrograms
                | Permission::QueryEvmContracts
        )
    }

    /// Check if permission requires elevated access
    pub fn requires_elevated_access(&self) -> bool {
        matches!(
            self,
            Permission::AdminAccess
                | Permission::ManageApiKeys
                | Permission::ManageUsers
                | Permission::SystemConfiguration
                | Permission::DebugAccess
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionCategory {
    Svm,
    Evm,
    MultiVm,
    System,
    Admin,
    WebSocket,
    Advanced,
}

/// Permission checker for validating user permissions
#[derive(Debug, Clone)]
pub struct PermissionChecker {
    user_permissions: HashSet<Permission>,
}

impl PermissionChecker {
    /// Create a new permission checker
    pub fn new(permissions: Vec<Permission>) -> Self {
        Self {
            user_permissions: permissions.into_iter().collect(),
        }
    }

    /// Check if user has a specific permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.user_permissions.contains(permission)
    }

    /// Check if user has all required permissions
    pub fn has_all_permissions(&self, permissions: &[Permission]) -> bool {
        permissions.iter().all(|p| self.has_permission(p))
    }

    /// Check if user has any of the required permissions
    pub fn has_any_permission(&self, permissions: &[Permission]) -> bool {
        permissions.iter().any(|p| self.has_permission(p))
    }

    /// Require a specific permission (returns error if not present)
    pub fn require_permission(&self, permission: &Permission) -> AuthResult<()> {
        if self.has_permission(permission) {
            Ok(())
        } else {
            Err(ApplicationError::AuthorizationDenied {
                resource: format!("{:?}", permission),
            })
        }
    }

    /// Require all specified permissions
    pub fn require_all_permissions(&self, permissions: &[Permission]) -> AuthResult<()> {
        for permission in permissions {
            self.require_permission(permission)?;
        }
        Ok(())
    }

    /// Require any of the specified permissions
    pub fn require_any_permission(&self, permissions: &[Permission]) -> AuthResult<()> {
        if self.has_any_permission(permissions) {
            Ok(())
        } else {
            Err(ApplicationError::AuthorizationDenied {
                resource: format!("Any of: {:?}", permissions),
            })
        }
    }

    /// Get all user permissions
    pub fn get_permissions(&self) -> Vec<Permission> {
        self.user_permissions.iter().cloned().collect()
    }

    /// Check if user is admin
    pub fn is_admin(&self) -> bool {
        self.has_permission(&Permission::AdminAccess)
    }

    /// Get permissions by category
    pub fn get_permissions_by_category(&self, category: PermissionCategory) -> Vec<Permission> {
        self.user_permissions
            .iter()
            .filter(|p| p.category() == category)
            .cloned()
            .collect()
    }
}

/// Permission set builder for easy permission management
pub struct PermissionSetBuilder {
    permissions: HashSet<Permission>,
}

impl PermissionSetBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            permissions: HashSet::new(),
        }
    }

    /// Add a single permission
    pub fn add(mut self, permission: Permission) -> Self {
        self.permissions.insert(permission);
        self
    }

    /// Add multiple permissions
    pub fn add_all(mut self, permissions: Vec<Permission>) -> Self {
        self.permissions.extend(permissions);
        self
    }

    /// Add default user permissions
    pub fn add_default_user(self) -> Self {
        self.add_all(Permission::default_user())
    }

    /// Add power user permissions
    pub fn add_power_user(self) -> Self {
        self.add_all(Permission::power_user())
    }

    /// Add admin permissions
    pub fn add_admin(self) -> Self {
        self.add_all(Permission::admin())
    }

    /// Add permissions by category
    pub fn add_category(mut self, category: PermissionCategory) -> Self {
        let category_permissions: Vec<Permission> = Permission::all()
            .into_iter()
            .filter(|p| p.category() == category)
            .collect();
        self.permissions.extend(category_permissions);
        self
    }

    /// Remove a permission
    pub fn remove(mut self, permission: Permission) -> Self {
        self.permissions.remove(&permission);
        self
    }

    /// Build the permission set
    pub fn build(self) -> Vec<Permission> {
        self.permissions.into_iter().collect()
    }
}

impl Default for PermissionSetBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_categories() {
        assert_eq!(
            Permission::ReadSvmAccounts.category(),
            PermissionCategory::Svm
        );
        assert_eq!(
            Permission::ReadEvmAccounts.category(),
            PermissionCategory::Evm
        );
        assert_eq!(
            Permission::AdminAccess.category(),
            PermissionCategory::Admin
        );
    }

    #[test]
    fn test_permission_checker() {
        let permissions = vec![
            Permission::ReadSvmAccounts,
            Permission::ReadEvmAccounts,
            Permission::AdminAccess,
        ];
        let checker = PermissionChecker::new(permissions);

        assert!(checker.has_permission(&Permission::ReadSvmAccounts));
        assert!(checker.has_permission(&Permission::AdminAccess));
        assert!(!checker.has_permission(&Permission::SubmitSvmTransactions));
        assert!(checker.is_admin());
    }

    #[test]
    fn test_permission_requirements() {
        let permissions = vec![Permission::ReadSvmAccounts];
        let checker = PermissionChecker::new(permissions);

        assert!(checker
            .require_permission(&Permission::ReadSvmAccounts)
            .is_ok());
        assert!(checker
            .require_permission(&Permission::AdminAccess)
            .is_err());
    }

    #[test]
    fn test_permission_builder() {
        let permissions = PermissionSetBuilder::new()
            .add(Permission::ReadSvmAccounts)
            .add(Permission::ReadEvmAccounts)
            .add_category(PermissionCategory::Admin)
            .build();

        assert!(permissions.contains(&Permission::ReadSvmAccounts));
        assert!(permissions.contains(&Permission::ReadEvmAccounts));
        assert!(permissions.contains(&Permission::AdminAccess));
    }

    #[test]
    fn test_default_permissions() {
        let default_user = Permission::default_user();
        let power_user = Permission::power_user();
        let admin = Permission::admin();

        assert!(default_user.len() < power_user.len());
        assert!(power_user.len() < admin.len());
        assert_eq!(admin.len(), Permission::all().len());
    }
}
