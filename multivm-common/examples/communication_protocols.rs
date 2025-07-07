//! Example demonstrating the use of different communication protocols
//!
//! This example shows how to:
//! - Configure different protocols for Ethereum and Solana
//! - Use IPC for local communication
//! - Use RPC for remote communication
//! - Use JWT for authenticated communication
//! - Handle protocol fallback

use multivm_common::{
    communication::{
        config::{CommunicationConfig, GlobalSettings, ProtocolConfigs, RetrySettings},
        factory::DefaultProtocolFactory,
        ipc_protocol::IpcProtocolConfig,
        jwt_protocol::JwtProtocolConfig,
        rpc_protocol::RpcProtocolConfig,
        CommunicationManager, CommunicationRequest, EngineType, ProtocolType,
    },
    MultivmResult,
};
use std::collections::HashMap;
use std::time::Duration;
use tracing::info;
use uuid::Uuid;

#[tokio::main]
async fn main() -> MultivmResult<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Starting communication protocol example");

    // Create custom protocol configurations
    let mut ipc_configs = HashMap::new();
    ipc_configs.insert(
        EngineType::Ethereum,
        IpcProtocolConfig {
            target_process_id: multivm_common::types::ProcessId::Ethereum,
            source_process_id: multivm_common::types::ProcessId::Main,
            transport_config: Default::default(),
            default_timeout: Duration::from_secs(30),
            retry_config: Default::default(),
            health_check_config: Default::default(),
        },
    );

    let mut rpc_configs = HashMap::new();
    rpc_configs.insert(
        EngineType::Ethereum,
        RpcProtocolConfig {
            endpoint_url: "http://localhost:8545".to_string(),
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            max_connections: 10,
            retry_config: Default::default(),
            health_check_config: Default::default(),
            default_headers: HashMap::new(),
            user_agent: "MultiVM-Example/1.0".to_string(),
            use_http2: true,
            tls_config: Default::default(),
        },
    );

    let mut jwt_configs = HashMap::new();
    jwt_configs.insert(
        EngineType::Solana,
        JwtProtocolConfig {
            endpoint_url: "https://api.solana.example.com".to_string(),
            jwt_secret: "example-secret-key".to_string(),
            token_expiry: Duration::from_secs(3600),
            refresh_threshold: Duration::from_secs(300),
            http_config: Default::default(),
            retry_config: Default::default(),
            health_check_config: Default::default(),
            api_key: Some("backup-api-key".to_string()),
            auth_headers: HashMap::new(),
        },
    );

    // Create communication configuration
    let comm_config = CommunicationConfig {
        protocol_preferences: {
            let mut prefs = HashMap::new();
            // Ethereum: prefer IPC for local, RPC as fallback
            prefs.insert(
                EngineType::Ethereum,
                vec![ProtocolType::Ipc, ProtocolType::Rpc],
            );
            // Solana: prefer JWT for auth, RPC as fallback
            prefs.insert(
                EngineType::Solana,
                vec![ProtocolType::Jwt, ProtocolType::Rpc],
            );
            prefs
        },
        protocol_configs: ProtocolConfigs {
            ipc: ipc_configs,
            rpc: rpc_configs,
            jwt: jwt_configs,
        },
        global_settings: GlobalSettings {
            enable_health_checks: true,
            default_timeout_seconds: 30,
            max_concurrent_connections: 20,
            enable_fallback: true,
            retry_settings: RetrySettings {
                max_connection_retries: 3,
                connection_retry_delay_ms: 100,
                max_retry_delay_ms: 5000,
                retry_backoff_multiplier: 2.0,
            },
        },
    };

    // Create protocol factory and communication manager
    let factory = Box::new(DefaultProtocolFactory::new());
    let mut manager = CommunicationManager::new(factory);

    // Initialize protocols based on configuration
    info!("Initializing communication protocols");

    // Add IPC protocol for Ethereum
    if let Some(ipc_config) = comm_config.protocol_configs.ipc.get(&EngineType::Ethereum) {
        manager
            .add_protocol(
                ProtocolType::Ipc,
                EngineType::Ethereum,
                serde_json::to_value(ipc_config)?,
            )
            .await?;
        info!("Added IPC protocol for Ethereum");
    }

    // Add RPC protocol for both engines
    for engine_type in [EngineType::Ethereum, EngineType::Solana] {
        if let Some(rpc_config) = comm_config.protocol_configs.rpc.get(&engine_type) {
            manager
                .add_protocol(
                    ProtocolType::Rpc,
                    engine_type.clone(),
                    serde_json::to_value(rpc_config)?,
                )
                .await?;
            info!("Added RPC protocol for {}", engine_type);
        }
    }

    // Add JWT protocol for Solana
    if let Some(jwt_config) = comm_config.protocol_configs.jwt.get(&EngineType::Solana) {
        manager
            .add_protocol(
                ProtocolType::Jwt,
                EngineType::Solana,
                serde_json::to_value(jwt_config)?,
            )
            .await?;
        info!("Added JWT protocol for Solana");
    }

    // Example: Send a request to Ethereum using protocol fallback
    info!("Sending request to Ethereum engine");

    let eth_request = CommunicationRequest {
        id: Uuid::new_v4().to_string(),
        method: "eth_chainId".to_string(),
        params: serde_json::json!([]),
        timeout: Some(Duration::from_secs(10)),
        auth_context: None,
    };

    match manager
        .send_with_fallback(&EngineType::Ethereum, eth_request)
        .await
    {
        Ok(response) => {
            info!("Received response: {:?}", response);
            if let Some(result) = response.result {
                info!("Chain ID: {}", result);
            }
        }
        Err(e) => {
            info!("Request failed (expected in example): {}", e);
        }
    }

    // Example: Send a request to Solana with JWT auth
    info!("Sending authenticated request to Solana engine");

    let sol_request = CommunicationRequest {
        id: Uuid::new_v4().to_string(),
        method: "getVersion".to_string(),
        params: serde_json::json!([]),
        timeout: Some(Duration::from_secs(10)),
        auth_context: None,
    };

    match manager
        .send_with_fallback(&EngineType::Solana, sol_request)
        .await
    {
        Ok(response) => {
            info!("Received response: {:?}", response);
            if let Some(metadata) = response.metadata.get("protocol") {
                info!("Used protocol: {}", metadata);
            }
        }
        Err(e) => {
            info!("Request failed (expected in example): {}", e);
        }
    }

    // Demonstrate protocol preferences
    info!("\nProtocol preferences:");
    for engine_type in [EngineType::Ethereum, EngineType::Solana] {
        if let Some(preferences) = manager.get_protocol_preferences(&engine_type) {
            info!("{}: {:?}", engine_type, preferences);
        }
    }

    // Show available protocols
    info!("\nAvailable protocols:");
    for protocol_type in [ProtocolType::Ipc, ProtocolType::Rpc, ProtocolType::Jwt] {
        for engine_type in [EngineType::Ethereum, EngineType::Solana] {
            if manager.get_protocol(&protocol_type, &engine_type).is_some() {
                info!("  {} -> {}", engine_type, protocol_type);
            }
        }
    }

    info!("Communication protocol example completed");
    Ok(())
}
