use multivm_common::{MultivmError, MultivmResult};
/// Input validation utilities for MultiVM CLI
///
/// This module provides comprehensive input validation to prevent
/// security vulnerabilities like command injection, path traversal,
/// and other attacks through CLI parameters.
use std::path::{Path, PathBuf};

/// Validate and sanitize a file path to prevent directory traversal attacks
pub fn validate_file_path(path: &str, purpose: &str) -> MultivmResult<PathBuf> {
    // Basic validation
    if path.is_empty() {
        return Err(MultivmError::Configuration {
            component: "file_path".to_string(),
            message: format!("Empty path provided for {purpose}"),
            validation_errors: None,
        });
    }

    // Check for obvious malicious patterns
    if path.contains("..") || path.contains("~") {
        return Err(MultivmError::Configuration {
            component: "file_path".to_string(),
            message: format!("Invalid path for {purpose}: contains unsafe components"),
            validation_errors: None,
        });
    }

    // Convert to PathBuf and canonicalize if it exists
    let path_buf = PathBuf::from(path);

    // For existing paths, canonicalize to resolve any remaining issues
    if path_buf.exists() {
        match path_buf.canonicalize() {
            Ok(canonical) => Ok(canonical),
            Err(e) => Err(MultivmError::Configuration {
                component: "file_path".to_string(),
                message: format!("Failed to canonicalize path for {purpose}: {e}"),
                validation_errors: None,
            }),
        }
    } else {
        // For non-existing paths, validate the parent directory if it exists
        if let Some(parent) = path_buf.parent() {
            if parent.exists() {
                match parent.canonicalize() {
                    Ok(canonical_parent) => {
                        Ok(canonical_parent.join(path_buf.file_name().unwrap()))
                    }
                    Err(e) => Err(MultivmError::Configuration {
                        component: "file_path".to_string(),
                        message: format!("Invalid parent directory for {purpose}: {e}"),
                        validation_errors: None,
                    }),
                }
            } else {
                Err(MultivmError::Configuration {
                    component: "file_path".to_string(),
                    message: format!("Parent directory does not exist for {purpose}"),
                    validation_errors: None,
                })
            }
        } else {
            Err(MultivmError::Configuration {
                component: "file_path".to_string(),
                message: format!("Invalid path structure for {purpose}"),
                validation_errors: None,
            })
        }
    }
}

/// Validate a node ID to ensure it's safe and follows expected patterns
#[allow(dead_code)]
pub fn validate_node_id(node_id: &str) -> MultivmResult<String> {
    if node_id.is_empty() {
        return Err(MultivmError::Configuration {
            component: "node_id".to_string(),
            message: "Node ID cannot be empty".to_string(),
            validation_errors: None,
        });
    }

    if node_id.len() > 64 {
        return Err(MultivmError::Configuration {
            component: "node_id".to_string(),
            message: "Node ID too long (max 64 characters)".to_string(),
            validation_errors: None,
        });
    }

    // Only allow alphanumeric characters, hyphens, and underscores
    if !node_id
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(MultivmError::Configuration {
            component: "node_id".to_string(),
            message: "Node ID contains invalid characters (only alphanumeric, -, _ allowed)"
                .to_string(),
            validation_errors: None,
        });
    }

    Ok(node_id.to_string())
}

/// Validate a port number
#[allow(dead_code)]
pub fn validate_port(port: u16, purpose: &str) -> MultivmResult<u16> {
    match port {
        0 => Err(MultivmError::Configuration {
            component: "port".to_string(),
            message: format!("Port 0 is not valid for {purpose}"),
            validation_errors: None,
        }),
        1..=1023 => {
            eprintln!(
                "Warning: Using privileged port {port} for {purpose} (requires root)"
            );
            Ok(port)
        }
        1024..=65535 => Ok(port),
    }
}

/// Validate block generation interval
pub fn validate_block_interval(interval_ms: u64) -> MultivmResult<u64> {
    match interval_ms {
        0 => Err(MultivmError::Configuration {
            component: "block_interval".to_string(),
            message: "Block interval cannot be 0".to_string(),
            validation_errors: None,
        }),
        1..=100 => Err(MultivmError::Configuration {
            component: "block_interval".to_string(),
            message: "Block interval too fast (minimum 100ms)".to_string(),
            validation_errors: None,
        }),
        101..=3600000 => Ok(interval_ms), // 100ms to 1 hour
        _ => Err(MultivmError::Configuration {
            component: "block_interval".to_string(),
            message: "Block interval too long (maximum 1 hour)".to_string(),
            validation_errors: None,
        }),
    }
}

/// Validate validator count
pub fn validate_validator_count(count: usize) -> MultivmResult<usize> {
    match count {
        0 => Err(MultivmError::Configuration {
            component: "validator_count".to_string(),
            message: "Validator count cannot be 0".to_string(),
            validation_errors: None,
        }),
        1 => {
            eprintln!("Warning: Single validator mode - only for development/testing");
            Ok(count)
        }
        2..=1000 => Ok(count),
        _ => Err(MultivmError::Configuration {
            component: "validator_count".to_string(),
            message: "Validator count too high (maximum 1000)".to_string(),
            validation_errors: None,
        }),
    }
}

/// Validate environment variables to prevent injection attacks
#[allow(dead_code)]
pub fn validate_env_var(var_name: &str, var_value: &str) -> MultivmResult<String> {
    // Check for suspicious characters that could be used for injection
    let suspicious_chars = ['$', '`', ';', '|', '&', '>', '<', '\n', '\r'];

    if var_value.chars().any(|c| suspicious_chars.contains(&c)) {
        return Err(MultivmError::Configuration {
            component: "environment_variable".to_string(),
            message: format!(
                "Environment variable {var_name} contains suspicious characters"
            ),
            validation_errors: None,
        });
    }

    // Validate length
    if var_value.len() > 1024 {
        return Err(MultivmError::Configuration {
            component: "environment_variable".to_string(),
            message: format!(
                "Environment variable {var_name} is too long (max 1024 characters)"
            ),
            validation_errors: None,
        });
    }

    Ok(var_value.to_string())
}

/// Validate configuration file contents for basic safety
pub fn validate_config_file_safety(config_path: &Path) -> MultivmResult<()> {
    // Check file permissions (should not be world-writable)
    let metadata = config_path
        .metadata()
        .map_err(|e| MultivmError::Configuration {
            component: "config_file".to_string(),
            message: format!("Cannot read config file metadata: {e}"),
            validation_errors: None,
        })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = metadata.permissions();
        let mode = permissions.mode();

        // Check if world-writable (octal 002)
        if mode & 0o002 != 0 {
            return Err(MultivmError::Configuration {
                component: "config_file".to_string(),
                message: "Config file is world-writable, which is a security risk".to_string(),
                validation_errors: None,
            });
        }

        // Check if group-writable (octal 020) and warn
        if mode & 0o020 != 0 {
            eprintln!("Warning: Config file is group-writable");
        }
    }

    // Check file size (should be reasonable)
    if metadata.len() > 1024 * 1024 {
        // 1MB
        return Err(MultivmError::Configuration {
            component: "config_file".to_string(),
            message: "Config file is too large (max 1MB)".to_string(),
            validation_errors: None,
        });
    }

    Ok(())
}
