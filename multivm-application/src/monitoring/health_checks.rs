//! Actual health check implementations

use crate::error::ApplicationResult;
use crate::ApplicationState;
use std::sync::Arc;
use std::time::Duration;

/// Check cache health by performing a test operation
pub async fn check_cache_health(
    state: &Arc<ApplicationState>,
) -> ApplicationResult<(bool, String)> {
    let test_key = "health_check_test";
    let test_value = "ok";

    // Try to set a value with short TTL
    match state
        .cache
        .set(test_key, &test_value, Duration::from_secs(1))
        .await
    {
        Ok(_) => {
            // Try to get it back
            match state.cache.get::<String>(test_key).await {
                Ok(Some(val)) if val == test_value => {
                    Ok((true, "Cache is operational".to_string()))
                }
                Ok(_) => Ok((false, "Cache read/write mismatch".to_string())),
                Err(e) => Ok((false, format!("Cache read failed: {e}"))),
            }
        }
        Err(e) => Ok((false, format!("Cache write failed: {e}"))),
    }
}

/// Check SVM (Solana) connection health
pub async fn check_svm_health(state: &Arc<ApplicationState>) -> ApplicationResult<(bool, String)> {
    match state.gateway.health_check().await {
        Ok(true) => {
            // Try to get latest block to ensure connection is functional
            match state.gateway.get_latest_block().await {
                Ok(_) => Ok((true, "SVM connection is healthy".to_string())),
                Err(e) => Ok((false, format!("SVM get latest block failed: {e}"))),
            }
        }
        Ok(false) => Ok((false, "SVM health check failed".to_string())),
        Err(e) => Ok((false, format!("SVM health check error: {e}"))),
    }
}

/// Check EVM (Ethereum) connection health
pub async fn check_evm_health(state: &Arc<ApplicationState>) -> ApplicationResult<(bool, String)> {
    // Similar to SVM but for Ethereum
    match state.gateway.health_check().await {
        Ok(true) => Ok((true, "EVM connection is healthy".to_string())),
        Ok(false) => Ok((false, "EVM health check failed".to_string())),
        Err(e) => Ok((false, format!("EVM health check error: {e}"))),
    }
}

/// Check database connection health
pub async fn check_database_health(
    state: &Arc<ApplicationState>,
) -> ApplicationResult<(bool, String)> {
    // Check database connectivity by performing a simple query
    let test_key = "health_check_db_test";
    let test_value = chrono::Utc::now().timestamp();

    // Try to write to cache (which uses database backend)
    match state
        .cache
        .set(test_key, &test_value, Duration::from_secs(60))
        .await
    {
        Ok(_) => {
            // Try to read it back
            match state.cache.get::<i64>(test_key).await {
                Ok(Some(val)) if val == test_value => {
                    Ok((true, "Database connection is healthy".to_string()))
                }
                Ok(_) => Ok((false, "Database read/write mismatch".to_string())),
                Err(e) => Ok((false, format!("Database read failed: {e}"))),
            }
        }
        Err(e) => Ok((false, format!("Database write failed: {e}"))),
    }
}

/// Check process manager health
pub async fn check_process_manager_health(
    state: &Arc<ApplicationState>,
) -> ApplicationResult<(bool, String)> {
    // Check execution engines status
    let engines_guard = state.execution_engines.read().await;
    let engines_healthy = engines_guard.is_healthy().await;

    if !engines_healthy {
        return Ok((false, "Execution engines are unhealthy".to_string()));
    }

    // Check if engines are responding
    match engines_guard.health_check().await {
        Ok(()) => Ok((
            true,
            "Process manager and execution engines are healthy".to_string(),
        )),
        Err(e) => Ok((false, format!("Process manager health check failed: {e}"))),
    }
}

/// Check consensus health
pub async fn check_consensus_health(
    state: &Arc<ApplicationState>,
) -> ApplicationResult<(bool, String)> {
    // Check if consensus manager is available
    let consensus_guard = state.consensus_manager.read().await;

    match consensus_guard.as_ref() {
        Some(_consensus) => {
            // For now, assume consensus is healthy if it exists
            // TODO: Add proper health check methods to MultiVMConsensusManager
            Ok((true, "Consensus manager is running".to_string()))
        }
        None => Ok((false, "Consensus manager not initialized".to_string())),
    }
}

/// Comprehensive system health check
pub async fn perform_full_health_check(
    state: &Arc<ApplicationState>,
) -> ApplicationResult<FullHealthReport> {
    let mut report = FullHealthReport {
        timestamp: chrono::Utc::now(),
        overall_health: true,
        checks: vec![],
    };

    // Run all health checks
    let checks = vec![
        ("cache", check_cache_health(state).await),
        ("svm", check_svm_health(state).await),
        ("evm", check_evm_health(state).await),
        ("database", check_database_health(state).await),
        ("process_manager", check_process_manager_health(state).await),
        ("consensus", check_consensus_health(state).await),
    ];

    for (name, result) in checks {
        match result {
            Ok((healthy, message)) => {
                if !healthy {
                    report.overall_health = false;
                }
                report.checks.push(ComponentHealth {
                    name: name.to_string(),
                    healthy,
                    message,
                });
            }
            Err(e) => {
                report.overall_health = false;
                report.checks.push(ComponentHealth {
                    name: name.to_string(),
                    healthy: false,
                    message: format!("Check failed: {e}"),
                });
            }
        }
    }

    Ok(report)
}

#[derive(Debug, serde::Serialize)]
pub struct FullHealthReport {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub overall_health: bool,
    pub checks: Vec<ComponentHealth>,
}

#[derive(Debug, serde::Serialize)]
pub struct ComponentHealth {
    pub name: String,
    pub healthy: bool,
    pub message: String,
}
