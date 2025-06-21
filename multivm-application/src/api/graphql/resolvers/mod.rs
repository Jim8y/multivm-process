//! # GraphQL Resolvers
//!
//! GraphQL query and mutation resolvers for the MultiVM application.

pub mod mutation;
pub mod query;
pub mod subscription;

use crate::ApplicationState;
use async_graphql::{Context, Result as GraphQLResult};
use std::sync::Arc;

/// Get application state from GraphQL context
pub fn get_app_state<'a>(ctx: &'a Context<'_>) -> GraphQLResult<&'a Arc<ApplicationState>> {
    ctx.data::<Arc<ApplicationState>>()
        .map_err(|_| async_graphql::Error::new("Application state not found in context"))
}

/// Common resolver utilities
pub mod utils {
    use super::*;
    use async_graphql::ErrorExtensions;

    /// Convert application error to GraphQL error
    pub fn app_error_to_graphql(error: crate::error::ApplicationError) -> async_graphql::Error {
        match error {
            crate::error::ApplicationError::ValidationError { field, message } => {
                async_graphql::Error::new(format!(
                    "Validation error in field '{}': {}",
                    field, message
                ))
                .extend_with(|_, e| e.set("code", "VALIDATION_ERROR"))
            }
            crate::error::ApplicationError::AuthenticationFailed { reason } => {
                async_graphql::Error::new(format!("Authentication error: {}", reason))
                    .extend_with(|_, e| e.set("code", "AUTHENTICATION_ERROR"))
            }
            crate::error::ApplicationError::AuthorizationDenied { resource } => {
                async_graphql::Error::new(format!("Authorization error: {}", resource))
                    .extend_with(|_, e| e.set("code", "AUTHORIZATION_ERROR"))
            }
            crate::error::ApplicationError::ResourceNotFound {
                resource_type,
                identifier,
            } => async_graphql::Error::new(format!(
                "{} with id '{}' not found",
                resource_type, identifier
            ))
            .extend_with(|_, e| e.set("code", "NOT_FOUND")),
            crate::error::ApplicationError::RateLimitExceeded { limit, window } => {
                async_graphql::Error::new(format!(
                    "Rate limit of {} requests per {} seconds exceeded",
                    limit, window
                ))
                .extend_with(|_, e| e.set("code", "RATE_LIMIT_EXCEEDED"))
            }
            _ => async_graphql::Error::new("Internal server error")
                .extend_with(|_, e| e.set("code", "INTERNAL_ERROR")),
        }
    }

    /// Validate required fields
    pub fn validate_required_field<'a>(
        field_name: &str,
        value: &'a Option<String>,
    ) -> GraphQLResult<&'a String> {
        value.as_ref().ok_or_else(|| {
            async_graphql::Error::new(format!("Field '{}' is required", field_name))
                .extend_with(|_, e| e.set("code", "REQUIRED_FIELD"))
        })
    }

    /// Validate address format
    pub fn validate_address(address: &str, vm_type: &str) -> GraphQLResult<()> {
        match vm_type {
            "svm" => {
                if address.len() != 44 {
                    return Err(async_graphql::Error::new("Invalid SVM address format")
                        .extend_with(|_, e| e.set("code", "INVALID_ADDRESS")));
                }
            }
            "evm" => {
                if !address.starts_with("0x") || address.len() != 42 {
                    return Err(async_graphql::Error::new("Invalid EVM address format")
                        .extend_with(|_, e| e.set("code", "INVALID_ADDRESS")));
                }
            }
            _ => {
                return Err(async_graphql::Error::new("Invalid VM type")
                    .extend_with(|_, e| e.set("code", "INVALID_VM_TYPE")));
            }
        }
        Ok(())
    }

    /// Apply pagination limits
    pub fn apply_pagination_limits(limit: Option<i32>, offset: Option<i32>) -> (usize, usize) {
        let limit = limit.unwrap_or(50).max(1).min(1000) as usize;
        let offset = offset.unwrap_or(0).max(0) as usize;
        (limit, offset)
    }
}
