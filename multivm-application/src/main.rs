use multivm_application::{ApplicationConfig, ApplicationServer};
use std::env;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting MultiVM Application Server");

    // Load configuration
    let config = load_configuration().await?;

    // Create and start the application server
    let server = ApplicationServer::new(config).await?;

    // Setup graceful shutdown
    let shutdown_signal = setup_shutdown_signal();

    // Start the server
    tokio::select! {
        result = server.start() => {
            error!("Server stopped unexpectedly: {:?}", result);
            result?;
        }
        _ = shutdown_signal => {
            info!("Shutdown signal received, stopping server...");
            server.stop().await?;
        }
    }

    info!("MultiVM Application Server stopped");
    Ok(())
}

async fn load_configuration() -> Result<ApplicationConfig, Box<dyn std::error::Error>> {
    // Try to load from config file first
    if let Ok(config_path) = env::var("MULTIVM_CONFIG_FILE") {
        info!("Loading configuration from file: {}", config_path);
        return Ok(ApplicationConfig::from_file(&config_path)?);
    }

    // Fall back to environment variables
    if env::var("MULTIVM_APP_SERVER__REST__HOST").is_ok() {
        info!("Loading configuration from environment variables");
        return Ok(ApplicationConfig::from_env()?);
    }

    // Use default configuration
    info!("Using default configuration");
    let config = ApplicationConfig::default();
    config.validate()?;
    Ok(config)
}

async fn setup_shutdown_signal() {
    use tokio::signal;

    #[cfg(unix)]
    {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut sigterm) => {
                match signal::unix::signal(signal::unix::SignalKind::interrupt()) {
                    Ok(mut sigint) => {
                        tokio::select! {
                            _ = sigterm.recv() => {
                                info!("Received SIGTERM");
                            }
                            _ = sigint.recv() => {
                                info!("Received SIGINT");
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to install SIGINT handler: {}", e);
                        // Fall back to just waiting for SIGTERM
                        let _ = sigterm.recv().await;
                        info!("Received SIGTERM");
                    }
                }
            }
            Err(e) => {
                error!(
                    "Failed to install signal handlers: {}. Shutting down immediately.",
                    e
                );
            }
        }
    }

    #[cfg(not(unix))]
    {
        match signal::ctrl_c().await {
            Ok(()) => info!("Received Ctrl+C"),
            Err(e) => error!(
                "Failed to listen for ctrl+c: {}. Shutting down immediately.",
                e
            ),
        }
    }
}
