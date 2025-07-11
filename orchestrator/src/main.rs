//! MultiVM Orchestrator - Main Entry Point
//! 
//! This is the main binary for the MultiVM distributed VM orchestration system.
//! It provides a command-line interface for running the orchestrator node.

use clap::{Arg, Command};
use multivm_core::{Error, Result};
use multivm_orchestrator::{
    config::load_config,
    Orchestrator,
};
use std::path::PathBuf;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    init_tracing()?;

    let matches = Command::new("multivm-orchestrator")
        .version(env!("CARGO_PKG_VERSION"))
        .about("MultiVM Distributed VM Orchestration System")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path")
                .value_parser(clap::value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("data-dir")
                .short('d')
                .long("data-dir")
                .value_name("DIR")
                .help("Data directory path")
                .value_parser(clap::value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("node-id")
                .short('n')
                .long("node-id")
                .value_name("ID")
                .help("Node ID (will be generated if not specified)"),
        )
        .arg(
            Arg::new("cluster-addr")
                .long("cluster-addr")
                .value_name("ADDR")
                .help("Cluster listen address (default: 127.0.0.1:7000)"),
        )
        .arg(
            Arg::new("rpc-addr")
                .long("rpc-addr")
                .value_name("ADDR")
                .help("RPC listen address (default: 127.0.0.1:50051)"),
        )
        .arg(
            Arg::new("log-level")
                .short('l')
                .long("log-level")
                .value_name("LEVEL")
                .help("Log level (debug, info, warn, error)")
                .default_value("info"),
        )
        .get_matches();

    // Load configuration
    let config_path = matches.get_one::<PathBuf>("config");
    let config_path_str = match config_path {
        Some(p) => Some(p.to_str()
            .ok_or_else(|| Error::InvalidConfig("Invalid config path encoding".to_string()))?),
        None => None,
    };
    let mut config = load_config(config_path_str)
        .map_err(|e| Error::InvalidConfig(e.to_string()))?;

    // Apply command line overrides
    if let Some(data_dir) = matches.get_one::<PathBuf>("data-dir") {
        config.node.data_dir = data_dir.clone();
    }

    if let Some(node_id) = matches.get_one::<String>("node-id") {
        config.node.id = Some(node_id.clone());
    }

    if let Some(cluster_addr) = matches.get_one::<String>("cluster-addr") {
        config.cluster.listen_addr = cluster_addr.parse()
            .map_err(|e| Error::InvalidInput(format!("Invalid cluster address: {}", e)))?;
    }

    if let Some(rpc_addr) = matches.get_one::<String>("rpc-addr") {
        config.rpc.listen_addr = rpc_addr.parse()
            .map_err(|e| Error::InvalidInput(format!("Invalid RPC address: {}", e)))?;
    }

    if let Some(log_level) = matches.get_one::<String>("log-level") {
        config.node.log_level = log_level.clone();
    }

    // Validate configuration
    config.validate()
        .map_err(|e| Error::InvalidConfig(format!("Configuration validation failed: {}", e)))?;
    
    // Convert to orchestrator config
    let orchestrator_config = config.to_orchestrator_config();

    info!("Starting MultiVM Orchestrator");
    info!("Node ID: {}", orchestrator_config.node_id);
    info!("Cluster Address: {}", orchestrator_config.cluster_addr);
    info!("RPC Address: {}", orchestrator_config.rpc_addr);
    info!("Data Directory: {}", orchestrator_config.data_dir.display());

    // Create and start orchestrator
    let orchestrator = Orchestrator::new(orchestrator_config).await?;

    // Set up graceful shutdown
    let shutdown_orchestrator = orchestrator.clone();
    tokio::spawn(async move {
        if let Err(e) = wait_for_shutdown().await {
            error!("Error waiting for shutdown signal: {}", e);
        }
        
        info!("Shutdown signal received, stopping orchestrator...");
        shutdown_orchestrator.shutdown().await;
    });

    // Start the orchestrator (this blocks until shutdown)
    match orchestrator.start().await {
        Ok(()) => {
            info!("Orchestrator stopped gracefully");
            Ok(())
        }
        Err(e) => {
            error!("Orchestrator error: {}", e);
            Err(e)
        }
    }
}

/// Initialize tracing with structured logging
fn init_tracing() -> Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "multivm=info,warn".into());

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();

    Ok(())
}

/// Wait for shutdown signal (Ctrl+C or SIGTERM)
async fn wait_for_shutdown() -> Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        
        let mut sigterm = signal(SignalKind::terminate())
            .map_err(|e| Error::Other(format!("Failed to register SIGTERM handler: {}", e)))?;
        let mut sigint = signal(SignalKind::interrupt())
            .map_err(|e| Error::Other(format!("Failed to register SIGINT handler: {}", e)))?;

        tokio::select! {
            _ = sigterm.recv() => {
                info!("Received SIGTERM");
            }
            _ = sigint.recv() => {
                info!("Received SIGINT");
            }
        }
    }

    #[cfg(not(unix))]
    {
        signal::ctrl_c().await
            .map_err(|e| Error::Other(format!("Failed to wait for Ctrl+C: {}", e)))?;
        info!("Received Ctrl+C");
    }

    Ok(())
}