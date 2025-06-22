#![allow(dead_code)]

pub mod block_generator;
pub mod block_router;
pub mod consensus_block_generator;
pub mod coordinator;
pub mod health;
pub mod ipc;
pub mod ipc_transport;
pub mod manager;
pub mod process;
pub mod resource_monitor;

#[cfg(test)]
mod block_router_tests;

pub use block_generator::*;
pub use block_router::*;
pub use consensus_block_generator::*;
pub use coordinator::{
    CoordinatorConfig, CoordinatorState, MultivmCoordinator,
    SystemHealthStatus as CoordinatorHealthStatus, SystemMetrics,
};
pub use health::*;
pub use ipc_transport::*;
pub use manager::{MultivmProcessManager, SystemHealthStatus as ProcessManagerHealthStatus};
pub use process::*;
pub use resource_monitor::*;

// Re-export commonly used types from multivm-common
pub use multivm_common::{
    BlockProvider, BlockchainType, ExecutionEngine, HealthChecker, HealthStatus, IpcCommand,
    IpcMessage, IpcResponse, MetricsCollector, MultivmConfig, MultivmError, MultivmResult,
    ProcessId, ProcessManager as ProcessManagerTrait, ResourceMonitor as ResourceMonitorTrait,
};

/// Main entry point for the process manager
pub async fn run_multivm_system(config: MultivmConfig) -> MultivmResult<()> {
    tracing::info!("Starting MultiVM system with config: {:?}", config.system);

    // Initialize logging
    init_logging(&config.logging)?;

    // Create and start the process manager
    let manager = MultivmProcessManager::new(config).await?;
    manager.start().await?;

    // Setup signal handlers for graceful shutdown
    setup_signal_handlers(&manager).await?;

    // Main event loop
    manager.run().await?;

    Ok(())
}

/// Initialize logging system
fn init_logging(config: &multivm_common::LoggingConfig) -> MultivmResult<()> {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

    let level = match config.level {
        multivm_common::LogLevel::Trace => tracing::Level::TRACE,
        multivm_common::LogLevel::Debug => tracing::Level::DEBUG,
        multivm_common::LogLevel::Info => tracing::Level::INFO,
        multivm_common::LogLevel::Warn => tracing::Level::WARN,
        multivm_common::LogLevel::Error => tracing::Level::ERROR,
    };

    let mut layers = Vec::new();

    // Console layer
    if config.enable_colors {
        let console_layer = tracing_subscriber::fmt::layer()
            .with_ansi(true)
            .with_level(true)
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true);
        layers.push(console_layer.boxed());
    } else {
        let console_layer = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_level(true)
            .with_target(true);
        layers.push(console_layer.boxed());
    }

    // File layer
    if let Some(ref file_path) = config.file_path {
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                MultivmError::Configuration(format!("Failed to create log directory: {}", e))
            })?;
        }

        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)
            .map_err(|e| MultivmError::Configuration(format!("Failed to open log file: {}", e)))?;

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

        let mut sigterm = signal(SignalKind::terminate()).map_err(|e| {
            MultivmError::Process(format!("Failed to setup SIGTERM handler: {}", e))
        })?;
        let mut sigint = signal(SignalKind::interrupt())
            .map_err(|e| MultivmError::Process(format!("Failed to setup SIGINT handler: {}", e)))?;

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

        let mut ctrl_c = ctrl_c()
            .map_err(|e| MultivmError::Process(format!("Failed to setup Ctrl+C handler: {}", e)))?;
        let mut ctrl_break = ctrl_break().map_err(|e| {
            MultivmError::Process(format!("Failed to setup Ctrl+Break handler: {}", e))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.trim().is_empty());
    }

    #[tokio::test]
    async fn test_init_logging() {
        let config = multivm_common::LoggingConfig {
            level: multivm_common::LogLevel::Info,
            file_path: None,
            enable_json: false,
            enable_colors: false,
        };

        // This should not panic
        assert!(init_logging(&config).is_ok());
    }
}
