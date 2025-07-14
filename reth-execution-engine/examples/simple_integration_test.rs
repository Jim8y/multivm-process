//! Simple integration test for MultiVM Reth communication
//! 
//! This example tests basic connectivity without starting a new Reth instance.

use reth_execution_engine::{config_integration::load_configuration, engine::RethEngineError};
use serde_json::json;
use std::time::Duration;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info,reth_execution_engine=debug")
        .init();

    info!("🧪 Simple MultiVM Reth Integration Test");
    info!("==========================================");

    // Load configuration
    let config = match load_configuration() {
        Ok(config) => {
            info!("✅ Configuration loaded successfully");
            config.print_summary();
            config
        }
        Err(e) => {
            warn!("⚠️  Failed to load configuration: {}", e);
            info!("📝 Using default configuration");
            reth_execution_engine::config_integration::RethMultiVMConfig::default()
        }
    };

    // Test basic RPC connectivity
    info!("🌐 Testing RPC connectivity...");
    match test_rpc_connectivity(&config).await {
        Ok(_) => info!("✅ RPC connectivity test passed"),
        Err(e) => {
            error!("❌ RPC connectivity test failed: {}", e);
            return Err(e.into());
        }
    }

    // Test Engine API connectivity
    info!("🔐 Testing Engine API connectivity...");
    match test_engine_api_connectivity(&config).await {
        Ok(_) => info!("✅ Engine API connectivity test passed"),
        Err(e) => {
            error!("❌ Engine API connectivity test failed: {}", e);
            return Err(e.into());
        }
    }

    // Test some basic operations
    info!("⚡ Testing basic operations...");
    match test_basic_operations(&config).await {
        Ok(_) => info!("✅ Basic operations test passed"),
        Err(e) => {
            warn!("⚠️  Basic operations test failed: {}", e);
        }
    }

    info!("🎉 Simple integration test completed successfully!");
    Ok(())
}

/// Test basic RPC connectivity
async fn test_rpc_connectivity(
    config: &reth_execution_engine::config_integration::RethMultiVMConfig,
) -> Result<(), RethEngineError> {
    let client = reqwest::Client::new();
    let rpc_url = format!("http://{}:{}", config.rpc.host, config.rpc.port);

    let request = json!({
        "jsonrpc": "2.0",
        "method": "eth_chainId",
        "params": [],
        "id": 1
    });

    let response = client
        .post(&rpc_url)
        .header("Content-Type", "application/json")
        .json(&request)
        .timeout(Duration::from_secs(5))
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
async fn test_engine_api_connectivity(
    config: &reth_execution_engine::config_integration::RethMultiVMConfig,
) -> Result<(), RethEngineError> {
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

    let request = json!({
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
        .timeout(Duration::from_secs(5))
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

    if let Some(capabilities) = result.get("result").and_then(|r| r.as_array()) {
        info!("🔐 Engine API capabilities: {} methods", capabilities.len());
        for cap in capabilities.iter().take(5) {
            if let Some(method) = cap.as_str() {
                info!("  - {}", method);
            }
        }
        if capabilities.len() > 5 {
            info!("  ... and {} more", capabilities.len() - 5);
        }
        Ok(())
    } else {
        Err(RethEngineError::Rpc("Invalid Engine API response".to_string()))
    }
}

/// Test basic operations
async fn test_basic_operations(
    config: &reth_execution_engine::config_integration::RethMultiVMConfig,
) -> Result<(), RethEngineError> {
    let client = reqwest::Client::new();
    let rpc_url = format!("http://{}:{}", config.rpc.host, config.rpc.port);

    // Get current block number
    let block_request = json!({
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
    let network_request = json!({
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
    let client_request = json!({
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
    let header = json!({
        "alg": "HS256",
        "typ": "JWT"
    });

    // Create JWT payload with current timestamp
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| RethEngineError::Configuration(format!("System time error: {}", e)))?
        .as_secs();

    let payload = json!({
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