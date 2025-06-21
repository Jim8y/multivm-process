//! # MultiVM Application Layer Demo
//!
//! This example demonstrates the complete usage of the MultiVM Application Layer,
//! including REST API, GraphQL, WebSocket, and admin interfaces.

use multivm_application::{
    AdminServerConfig, ApplicationConfig, ApplicationServer, AuthConfig, CacheConfig,
    DatabaseConfig, FeatureConfig, GraphQLServerConfig, MonitoringConfig, MultivmClientConfig,
    PerformanceConfig, RateLimitingConfig, RestServerConfig, RethClientConfig, ServerConfig,
    SolanaClientConfig, VmClientConfig, WebSocketServerConfig,
};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for demo
    tracing_subscriber::fmt::init();

    info!("🚀 Starting MultiVM Application Layer Demo");

    // Step 1: Create demo configuration
    let config = create_demo_config();
    info!("✅ Created demo configuration");

    // Step 2: Validate configuration
    config.validate()?;
    info!("✅ Configuration validation passed");

    // Step 3: Create application server
    let server = ApplicationServer::new(config).await?;
    info!("✅ Application server created successfully");

    // Step 4: Check server status
    let status = server.get_status().await?;
    info!(
        "📊 Server status: running={}, version={}",
        status.is_running, status.version
    );

    // Step 5: Start server in background
    info!("🔄 Starting application server...");
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server.start().await {
            error!("Server error: {}", e);
        }
    });

    // Step 6: Wait for server startup
    sleep(Duration::from_secs(3)).await;
    info!("✅ Server startup complete");

    // Step 7: Demo API interactions
    demo_api_interactions().await?;

    // Step 8: Demo WebSocket interactions
    demo_websocket_interactions().await?;

    // Step 9: Demo admin interface
    demo_admin_interface().await?;

    // Step 10: Cleanup
    info!("🧹 Cleaning up demo...");
    server_handle.abort();

    info!("🎉 MultiVM Application Layer Demo completed successfully!");
    Ok(())
}

fn create_demo_config() -> ApplicationConfig {
    ApplicationConfig {
        server: ServerConfig {
            rest: RestServerConfig {
                host: "127.0.0.1".to_string(),
                port: 18080, // Different port for demo
                request_timeout: Duration::from_secs(30),
                max_body_size: 1024 * 1024,
                enable_cors: true,
                cors_origins: vec!["*".to_string()],
                enable_logging: true,
            },
            graphql: GraphQLServerConfig {
                host: "127.0.0.1".to_string(),
                port: 18081,
                enable_playground: true,
                max_query_depth: 15,
                max_query_complexity: 1000,
                query_timeout: Duration::from_secs(30),
            },
            websocket: WebSocketServerConfig {
                host: "127.0.0.1".to_string(),
                port: 18082,
                max_connections: 1000,
                connection_timeout: Duration::from_secs(60),
                heartbeat_interval: Duration::from_secs(30),
                max_subscriptions_per_connection: 100,
            },
            admin: AdminServerConfig {
                host: "127.0.0.1".to_string(),
                port: 18083,
                enable_ui: true,
                require_auth: false, // Disabled for demo
            },
        },
        database: DatabaseConfig {
            url: "sqlite::memory:".to_string(), // In-memory for demo
            max_connections: 10,
            min_connections: 1,
            connection_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(300),
            max_lifetime: Duration::from_secs(1800),
            enable_query_logging: true,
        },
        cache: CacheConfig {
            redis: multivm_application::RedisConfig {
                url: "".to_string(), // No Redis for demo
                max_connections: 10,
                connection_timeout: Duration::from_secs(5),
                command_timeout: Duration::from_secs(5),
                key_prefix: "demo:".to_string(),
            },
            memory: multivm_application::MemoryCacheConfig {
                max_items: 1000,
                max_memory_bytes: 10 * 1024 * 1024, // 10MB
                cleanup_interval: Duration::from_secs(60),
            },
            strategy: multivm_application::CacheStrategy::WriteThrough,
            default_ttl: Duration::from_secs(300),
        },
        auth: AuthConfig {
            jwt_secret: "demo-jwt-secret-key-that-is-long-enough-for-security".to_string(),
            jwt_expiration: Duration::from_secs(3600),
            enable_api_keys: true,
            api_key_validation: multivm_application::ApiKeyValidation::Database,
            admin_api_key: Some("demo-admin-key-123456".to_string()),
        },
        rate_limiting: RateLimitingConfig {
            enabled: true,
            default_rpm: 100, // Reduced for demo
            default_rph: 1000,
            default_rpd: 10000,
            storage: multivm_application::RateLimitStorage::Memory,
            burst_allowance: 10,
        },
        monitoring: MonitoringConfig {
            enable_metrics: true,
            metrics: multivm_application::MetricsConfig {
                host: "127.0.0.1".to_string(),
                port: 19090,
                format: multivm_application::MetricsFormat::Prometheus,
                collection_interval: Duration::from_secs(15),
            },
            health_check: multivm_application::HealthCheckConfig {
                host: "127.0.0.1".to_string(),
                port: 19091,
                check_interval: Duration::from_secs(30),
                check_timeout: Duration::from_secs(5),
            },
            tracing: multivm_application::TracingConfig {
                enabled: false, // Disabled for demo
                endpoint: None,
                service_name: "multivm-application-demo".to_string(),
                sampling_rate: 0.1,
            },
            log_level: "info".to_string(),
        },
        vm_clients: VmClientConfig {
            solana: SolanaClientConfig {
                rpc_url: "http://localhost:8899".to_string(),
                ws_url: "ws://localhost:8900".to_string(),
                timeout: Duration::from_secs(30),
                retry: multivm_application::RetryConfig {
                    max_retries: 3,
                    initial_delay: Duration::from_millis(100),
                    max_delay: Duration::from_secs(30),
                    backoff_multiplier: 2.0,
                },
            },
            reth: RethClientConfig {
                rpc_url: "http://localhost:8545".to_string(),
                ws_url: "ws://localhost:8546".to_string(),
                jwt_secret: "demo-reth-jwt-secret".to_string(),
                timeout: Duration::from_secs(30),
                retry: multivm_application::RetryConfig {
                    max_retries: 3,
                    initial_delay: Duration::from_millis(100),
                    max_delay: Duration::from_secs(30),
                    backoff_multiplier: 2.0,
                },
            },
            multivm: MultivmClientConfig {
                consensus_endpoint: "http://localhost:8100".to_string(),
                account_mapping_endpoint: "http://localhost:8101".to_string(),
                process_manager_endpoint: "http://localhost:8102".to_string(),
                timeout: Duration::from_secs(30),
                retry: multivm_application::RetryConfig {
                    max_retries: 3,
                    initial_delay: Duration::from_millis(100),
                    max_delay: Duration::from_secs(30),
                    backoff_multiplier: 2.0,
                },
            },
        },
        features: FeatureConfig {
            enable_graphql_subscriptions: true,
            enable_websocket_streaming: true,
            enable_cross_vm_operations: true,
            enable_admin_interface: true,
            enable_experimental: true, // Enabled for demo
            enable_request_batching: true,
            enable_compression: true,
        },
        performance: PerformanceConfig {
            worker_threads: Some(2), // Reduced for demo
            blocking_threads: Some(2),
            thread_stack_size: None,
            request_buffer_size: 4096,
            response_buffer_size: 4096,
            connection_pool_size: 10,
        },
    }
}

async fn demo_api_interactions() -> Result<(), Box<dyn std::error::Error>> {
    info!("📡 Demonstrating API interactions...");

    // Create HTTP client
    let client = reqwest::Client::new();

    // Test REST API endpoints
    let endpoints = vec![
        ("GET", "http://127.0.0.1:18080/health", "Health check"),
        ("GET", "http://127.0.0.1:18080/status", "System status"),
        ("GET", "http://127.0.0.1:19091/health", "Health endpoint"),
    ];

    for (method, url, description) in endpoints {
        match client.get(url).send().await {
            Ok(response) => {
                info!(
                    "✅ {} ({}): Status {}",
                    description,
                    method,
                    response.status()
                );
            }
            Err(_) => {
                info!(
                    "⚠️  {} ({}): Not available (expected in demo)",
                    description, method
                );
            }
        }
    }

    Ok(())
}

async fn demo_websocket_interactions() -> Result<(), Box<dyn std::error::Error>> {
    info!("🔌 Demonstrating WebSocket interactions...");

    // Note: In a real demo, we would connect to WebSocket and subscribe to events
    // For this demo, we'll just simulate the interaction

    info!("📡 Simulating WebSocket connection to ws://127.0.0.1:18082");
    info!("📦 Simulating subscription to block updates");
    info!("📦 Simulating subscription to transaction events");

    // Simulate some WebSocket events
    for i in 1..=3 {
        sleep(Duration::from_millis(500)).await;
        info!("📨 Simulated WebSocket event {}: New block notification", i);
    }

    Ok(())
}

async fn demo_admin_interface() -> Result<(), Box<dyn std::error::Error>> {
    info!("⚙️  Demonstrating admin interface...");

    let client = reqwest::Client::new();

    // Test admin endpoints
    let admin_endpoints = vec![
        "http://127.0.0.1:18083/admin/dashboard",
        "http://127.0.0.1:18083/admin/metrics",
        "http://127.0.0.1:18083/admin/config",
    ];

    for endpoint in admin_endpoints {
        info!("🔍 Testing admin endpoint: {}", endpoint);
        match client.get(endpoint).send().await {
            Ok(response) => {
                info!(
                    "✅ Admin endpoint {} returned status: {}",
                    endpoint,
                    response.status()
                );
                if response.status().is_success() {
                    // Try to parse JSON response
                    match response.text().await {
                        Ok(body) => {
                            if body.len() > 100 {
                                info!("📄 Response preview: {}...", &body[..100]);
                            } else {
                                info!("📄 Response: {}", body);
                            }
                        }
                        Err(_) => info!("📄 Response body could not be parsed as text"),
                    }
                }
            }
            Err(e) => {
                info!("⚠️  Admin endpoint {} not available: {}", endpoint, e);
            }
        }
    }

    Ok(())
}

/// Configuration examples for different deployment scenarios
pub fn production_config_example() -> ApplicationConfig {
    let mut config = ApplicationConfig::default();

    // Production-ready settings
    config.server.rest.host = "0.0.0.0".to_string();
    config.server.admin.require_auth = true;
    config.auth.jwt_secret = "production-secret-from-env-must-be-32-chars".to_string();
    config.rate_limiting.default_rpm = 10000;
    config.monitoring.enable_metrics = true;
    config.features.enable_experimental = false;

    config
}

pub fn development_config_example() -> ApplicationConfig {
    let mut config = ApplicationConfig::default();

    // Development-friendly settings
    config.server.rest.enable_cors = true;
    config.server.graphql.enable_playground = true;
    config.server.admin.require_auth = false;
    config.monitoring.tracing.enabled = true;
    config.features.enable_experimental = true;

    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demo_config_validation() {
        let config = create_demo_config();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_production_config_validation() {
        let config = production_config_example();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_development_config_validation() {
        let config = development_config_example();
        assert!(config.validate().is_ok());
    }
}
