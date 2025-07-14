//! Complete MultiVM Reth Integration Example
//!
//! This example demonstrates the full integration between the Reth execution engine
//! and the MultiVM setup scripts, including configuration loading, process management,
//! and Engine API communication.

use reth_execution_engine::{
    config_integration::{load_configuration, RethMultiVMConfig},
    engine::RethEngineError,
};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info,reth_execution_engine=debug")
        .init();

    info!("🚀 Starting Complete MultiVM Reth Integration Example");
    info!("================================================");

    // Step 1: Load configuration (supports both env vars and config files)
    info!("📁 Loading configuration...");
    let config = match load_configuration() {
        Ok(config) => {
            info!("✅ Configuration loaded successfully");
            config
        }
        Err(e) => {
            warn!("⚠️  Failed to load configuration: {}", e);
            info!("📝 Using default configuration");
            RethMultiVMConfig::default()
        }
    };

    // Print configuration summary
    config.print_summary();

    // Step 2: Create Reth engine with configuration
    info!("🔧 Creating Reth execution engine...");
    let mut reth_engine = config.create_reth_engine().await?;

    // Step 3: Initialize the engine (this will use setup scripts if available)
    info!("🚀 Initializing Reth engine...");
    match reth_engine.initialize().await {
        Ok(_) => {
            info!("✅ Reth engine initialized successfully");
        }
        Err(e) => {
            error!("❌ Failed to initialize Reth engine: {}", e);
            return Err(e.into());
        }
    }

    // Step 4: Verify engine is running
    info!("🔍 Verifying Reth process status...");
    if reth_engine.is_reth_running().await {
        if let Some(pid) = reth_engine.get_reth_process_pid().await {
            info!("✅ Reth process is running (PID: {})", pid);
        } else {
            info!("✅ Reth process is running");
        }
    } else {
        error!("❌ Reth process is not running");
        return Err("Reth process failed to start".into());
    }

    // Step 5: Test basic connectivity
    info!("🌐 Testing RPC connectivity...");
    match test_rpc_connectivity(&config).await {
        Ok(_) => info!("✅ RPC connectivity test passed"),
        Err(e) => {
            warn!("⚠️  RPC connectivity test failed: {}", e);
        }
    }

    // Step 6: Test Engine API with JWT authentication
    info!("🔐 Testing Engine API with JWT authentication...");
    match test_engine_api_connectivity(&config).await {
        Ok(_) => info!("✅ Engine API connectivity test passed"),
        Err(e) => {
            warn!("⚠️  Engine API connectivity test failed: {}", e);
        }
    }

    // Step 7: Demonstrate basic operations
    info!("⚡ Running basic operations...");
    if let Err(e) = demonstrate_basic_operations(&config).await {
        warn!("⚠️  Basic operations test failed: {}", e);
    }

    // Step 8: Monitor for a while
    info!("📊 Monitoring Reth process for 30 seconds...");
    let monitoring_duration = Duration::from_secs(30);
    let start_time = std::time::Instant::now();

    while start_time.elapsed() < monitoring_duration {
        if !reth_engine.is_reth_running().await {
            error!("❌ Reth process has stopped unexpectedly");
            break;
        }

        // Print status every 10 seconds
        if start_time.elapsed().as_secs() % 10 == 0 {
            info!("📈 Reth process is healthy (uptime: {:?})", start_time.elapsed());
        }

        sleep(Duration::from_secs(1)).await;
    }

    // Step 9: Graceful shutdown
    info!("🛑 Shutting down Reth engine...");
    match reth_engine.stop_reth_process().await {
        Ok(_) => info!("✅ Reth engine shut down successfully"),
        Err(e) => warn!("⚠️  Error during shutdown: {}", e),
    }

    info!("🎉 Complete MultiVM Reth Integration Example finished");
    Ok(())
}

/// Test RPC connectivity
async fn test_rpc_connectivity(config: &RethMultiVMConfig) -> Result<(), RethEngineError> {
    let client = reqwest::Client::new();
    let rpc_url = format!("http://{}:{}", config.rpc.host, config.rpc.port);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_chainId",
        "params": [],
        "id": 1
    });

    let response = client
        .post(&rpc_url)
        .header("Content-Type", "application/json")
        .json(&request)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| RethEngineError::Rpc(format!("RPC request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(RethEngineError::Rpc(format!(
            "RPC request failed with status: {}",
            response.status()
        )));
    }

    let result: serde_json::Value = response
        .json()
        .await
        .map_err(|e| RethEngineError::Rpc(format!("Failed to parse RPC response: {}", e)))?;

    if let Some(chain_id) = result.get("result").and_then(|r| r.as_str()) {
        info!("🔗 Connected to chain ID: {}", chain_id);
        Ok(())
    } else {
        Err(RethEngineError::Rpc("Invalid chain ID response".to_string()))
    }
}

/// Test Engine API connectivity with JWT authentication
async fn test_engine_api_connectivity(config: &RethMultiVMConfig) -> Result<(), RethEngineError> {
    // Load JWT secret
    let jwt_secret = match std::fs::read_to_string(&config.engine_api.jwt_secret_path) {
        Ok(secret) => secret.trim().to_string(),
        Err(e) => {
            return Err(RethEngineError::Configuration(format!(
                "Failed to read JWT secret: {}",
                e
            )));
        }
    };

    // Generate JWT token
    let jwt_token = generate_jwt_token(&jwt_secret)?;

    let client = reqwest::Client::new();
    let engine_url = format!("http://{}:{}", config.engine_api.host, config.engine_api.port);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "engine_exchangeCapabilities",
        "params": [[]],
        "id": 1
    });

    let response = client
        .post(&engine_url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", jwt_token))
        .json(&request)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| RethEngineError::Rpc(format!("Engine API request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(RethEngineError::Rpc(format!(
            "Engine API request failed with status: {}",
            response.status()
        )));
    }

    let result: serde_json::Value = response
        .json()
        .await
        .map_err(|e| RethEngineError::Rpc(format!("Failed to parse Engine API response: {}", e)))?;

    if result.get("result").is_some() {
        info!("🔐 Engine API JWT authentication successful");
        Ok(())
    } else {
        Err(RethEngineError::Rpc("Invalid Engine API response".to_string()))
    }
}

/// Demonstrate basic operations
async fn demonstrate_basic_operations(config: &RethMultiVMConfig) -> Result<(), RethEngineError> {
    let client = reqwest::Client::new();
    let rpc_url = format!("http://{}:{}", config.rpc.host, config.rpc.port);

    // Get current block number
    let block_request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_blockNumber",
        "params": [],
        "id": 1
    });

    let response = client
        .post(&rpc_url)
        .json(&block_request)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| RethEngineError::Rpc(format!("Block number request failed: {}", e)))?;

    if let Ok(result) = response.json::<serde_json::Value>().await {
        if let Some(block_hex) = result.get("result").and_then(|r| r.as_str()) {
            let block_number = u64::from_str_radix(block_hex.trim_start_matches("0x"), 16)
                .unwrap_or(0);
            info!("📦 Current block number: {}", block_number);
        }
    }

    // Get network version
    let network_request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "net_version",
        "params": [],
        "id": 2
    });

    let response = client
        .post(&rpc_url)
        .json(&network_request)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| RethEngineError::Rpc(format!("Network version request failed: {}", e)))?;

    if let Ok(result) = response.json::<serde_json::Value>().await {
        if let Some(net_version) = result.get("result").and_then(|r| r.as_str()) {
            info!("🌐 Network version: {}", net_version);
        }
    }

    // Get client version
    let client_request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "web3_clientVersion",
        "params": [],
        "id": 3
    });

    let response = client
        .post(&rpc_url)
        .json(&client_request)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| RethEngineError::Rpc(format!("Client version request failed: {}", e)))?;

    if let Ok(result) = response.json::<serde_json::Value>().await {
        if let Some(client_version) = result.get("result").and_then(|r| r.as_str()) {
            info!("🖥️  Client version: {}", client_version);
        }
    }

    Ok(())
}

/// Generate JWT token for Engine API authentication
fn generate_jwt_token(secret: &str) -> Result<String, RethEngineError> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Create JWT header
    let header = serde_json::json!({
        "alg": "HS256",
        "typ": "JWT"
    });

    // Create JWT payload with current timestamp
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| RethEngineError::Configuration(format!("System time error: {}", e)))?
        .as_secs();

    let payload = serde_json::json!({
        "iat": now,
        "exp": now + 300 // Token expires in 5 minutes
    });

    // Encode header and payload
    let header_b64 = URL_SAFE_NO_PAD.encode(
        serde_json::to_string(&header)
            .map_err(|e| RethEngineError::Configuration(format!("Failed to serialize header: {}", e)))?
            .as_bytes(),
    );

    let payload_b64 = URL_SAFE_NO_PAD.encode(
        serde_json::to_string(&payload)
            .map_err(|e| RethEngineError::Configuration(format!("Failed to serialize payload: {}", e)))?
            .as_bytes(),
    );

    // Create signature
    let message = format!("{}.{}", header_b64, payload_b64);
    let secret_bytes = hex::decode(secret)
        .map_err(|e| RethEngineError::Configuration(format!("Invalid JWT secret format: {}", e)))?;

    let mut mac = Hmac::<Sha256>::new_from_slice(&secret_bytes)
        .map_err(|e| RethEngineError::Configuration(format!("Failed to create HMAC: {}", e)))?;

    mac.update(message.as_bytes());
    let signature = mac.finalize().into_bytes();
    let signature_b64 = URL_SAFE_NO_PAD.encode(signature);

    // Combine into final JWT
    Ok(format!("{}.{}", message, signature_b64))
}

/// Example of how to use with environment variables (like the setup script)
#[allow(dead_code)]
fn example_with_env_vars() {
    // These would typically be set by the setup script
    std::env::set_var("RETH_DATA_DIR", "./reth-data");
    std::env::set_var("RETH_HTTP_PORT", "8545");
    std::env::set_var("RETH_ENGINE_PORT", "8551");
    std::env::set_var("RETH_CHAIN_ID", "1337");
    std::env::set_var("JWT_SECRET_PATH", "./reth-data/jwt.hex");
    std::env::set_var("MULTIVM_IPC_PATH", "/tmp/multivm-reth.sock");

    info!("Environment variables set for MultiVM Reth integration");
}

/// Example of manual configuration without setup scripts
#[allow(dead_code)]
fn example_manual_config() -> RethMultiVMConfig {
    let mut config = RethMultiVMConfig::default();
    
    // Customize for your environment
    config.reth.data_dir = "/var/lib/multivm/reth".to_string();
    config.rpc.port = 8545;
    config.engine_api.port = 8551;
    config.node.chain_id = 1337;
    config.reth.dev_mode = true;
    
    // Security settings for production
    config.security.enable_admin_api = false;
    config.security.allow_unsafe_methods = false;
    config.security.rate_limiting.enabled = true;
    
    info!("Manual configuration created");
    config
}