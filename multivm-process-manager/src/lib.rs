#![allow(dead_code)]

use multivm_common::{config::LoggingConfig, MultivmError, MultivmResult};

// pub mod block_generator;  // Temporarily disabled due to dependency conflicts
// pub mod block_router;  // Temporarily disabled due to dependency conflicts
pub mod consensus_block_generator;
pub mod coordinator;
pub mod health;
pub mod ipc;
pub mod ipc_transport;
pub mod lock_ordering;
pub mod manager;
pub mod process;
pub mod resource_monitor;
pub mod transaction_batcher;
pub mod zombie_reaper;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod manager_tests;

#[cfg(test)]
mod health_tests;

#[cfg(test)]
mod process_tests;

#[cfg(test)]
mod resource_monitor_tests;

// Re-export key types
pub use manager::MultivmProcessManager;
pub use coordinator::{MultivmCoordinator, CoordinatorConfig};
pub use health::HealthMonitor;
// pub use block_router::BlockRouter;  // Temporarily disabled
pub use process::ProcessHandle;
pub use consensus_block_generator::{ConsensusBlockGenerator, ConsensusBlockGeneratorConfig};
pub use zombie_reaper::{ZombieReaper, ZombieReaperConfig, ZombieReaperStats};

/// Initialize logging system
fn init_logging(config: &LoggingConfig) -> MultivmResult<()> {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

    let level = match config.level.as_str() {
        "trace" => tracing::Level::TRACE,
        "debug" => tracing::Level::DEBUG,
        "info" => tracing::Level::INFO,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    };

    let mut layers = Vec::new();

    // Console layer - always enable with colors
    let console_layer = tracing_subscriber::fmt::layer()
        .with_ansi(true)
        .with_level(true)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true);
    layers.push(console_layer.boxed());

    // File layer
    if config.enable_file_logging {
        let file_path = config.log_directory.join("multivm.log");
        std::fs::create_dir_all(&config.log_directory).map_err(|e| {
            MultivmError::Configuration {
                component: "logging".to_string(),
                message: format!("Failed to create log directory: {e}"),
                validation_errors: None,
            }
        })?;

        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .map_err(|e| MultivmError::Configuration {
                component: "logging".to_string(),
                message: format!("Failed to open log file: {e}"),
                validation_errors: None,
            })?;

        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(file)
            .with_ansi(false)
            .with_target(true);
        layers.push(file_layer.boxed());
    }

    // Initialize subscriber
    tracing_subscriber::registry()
        .with(layers)
        .with(tracing_subscriber::filter::LevelFilter::from_level(level))
        .init();

    tracing::info!("Logging initialized");
    Ok(())
}

/// Setup signal handlers for graceful shutdown
async fn setup_signal_handlers(manager: &MultivmProcessManager) -> MultivmResult<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        let mut sigterm = signal(SignalKind::terminate()).map_err(|e| MultivmError::Process {
            process_id: "signal_handler".to_string(),
            message: format!("Failed to setup SIGTERM handler: {e}"),
            exit_code: None,
        })?;
        let mut sigint = signal(SignalKind::interrupt()).map_err(|e| MultivmError::Process {
            process_id: "signal_handler".to_string(),
            message: format!("Failed to setup SIGINT handler: {e}"),
            exit_code: None,
        })?;

        let manager_clone = manager.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = sigterm.recv() => {
                    tracing::info!("Received SIGTERM, shutting down gracefully");
                    if let Err(e) = manager_clone.shutdown(true).await {
                        tracing::error!("Error during graceful shutdown: {}", e);
                    }
                }
                _ = sigint.recv() => {
                    tracing::info!("Received SIGINT, shutting down gracefully");
                    if let Err(e) = manager_clone.shutdown(true).await {
                        tracing::error!("Error during graceful shutdown: {}", e);
                    }
                }
            }
        });
    }

    #[cfg(windows)]
    {
        use tokio::signal::windows::{ctrl_break, ctrl_c};

        let mut ctrl_c = ctrl_c().map_err(|e| MultivmError::Process {
            process_id: "signal_handler".to_string(),
            message: format!("Failed to setup Ctrl+C handler: {}", e),
            exit_code: None,
        })?;
        let mut ctrl_break = ctrl_break().map_err(|e| MultivmError::Process {
            process_id: "signal_handler".to_string(),
            message: format!("Failed to setup Ctrl+Break handler: {}", e),
            exit_code: None,
        })?;

        let manager_clone = manager.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = ctrl_c.recv() => {
                    tracing::info!("Received Ctrl+C, shutting down gracefully");
                    if let Err(e) = manager_clone.shutdown(true).await {
                        tracing::error!("Error during graceful shutdown: {}", e);
                    }
                }
                _ = ctrl_break.recv() => {
                    tracing::info!("Received Ctrl+Break, shutting down gracefully");
                    if let Err(e) = manager_clone.shutdown(true).await {
                        tracing::error!("Error during graceful shutdown: {}", e);
                    }
                }
            }
        });
    }

    Ok(())
}

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
