//! MultiVM RPC Service Binary

use multivm_rpc_service::{config::RpcServiceConfig, server::RpcServer};
use std::path::Path;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("multivm_rpc_service=info".parse()?)
        )
        .init();

    info!("Starting MultiVM RPC Service v{}", multivm_rpc_service::VERSION);

    // Load configuration
    let config_path = std::env::var("RPC_CONFIG_PATH")
        .unwrap_or_else(|_| "rpc-config.toml".to_string());

    let config = if Path::new(&config_path).exists() {
        info!("Loading configuration from: {}", config_path);
        match RpcServiceConfig::from_file(Path::new(&config_path)) {
            Ok(config) => {
                info!("Configuration loaded successfully");
                config
            }
            Err(e) => {
                error!("Failed to load configuration: {}", e);
                info!("Using default configuration");
                RpcServiceConfig::default()
            }
        }
    } else {
        info!("No configuration file found at {}, using defaults", config_path);
        RpcServiceConfig::default()
    };

    // Validate configuration
    if let Err(e) = config.validate() {
        error!("Invalid configuration: {}", e);
        std::process::exit(1);
    }

    info!("Configuration validated successfully");
    info!("Server will bind to: {}", config.server.bind_address);
    info!("Ethereum backends: {}", config.backends.ethereum.len());
    info!("Solana backends: {}", config.backends.solana.len());
    info!("Cache enabled: {}", config.cache.enabled);
    info!("Rate limiting enabled: {}", config.rate_limit.enabled);
    info!("Authentication enabled: {}", config.auth.enabled);

    // Create and start server
    let server = match RpcServer::new(config).await {
        Ok(server) => {
            info!("RPC server created successfully");
            server
        }
        Err(e) => {
            error!("Failed to create RPC server: {}", e);
            std::process::exit(1);
        }
    };

    // Handle shutdown gracefully
    let shutdown_signal = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C signal handler");
        info!("Shutdown signal received");
    };

    // Start server with graceful shutdown
    tokio::select! {
        result = server.start() => {
            match result {
                Ok(_) => info!("Server stopped normally"),
                Err(e) => error!("Server error: {}", e),
            }
        }
        _ = shutdown_signal => {
            info!("Shutting down server...");
        }
    }

    info!("MultiVM RPC Service stopped");
    Ok(())
}