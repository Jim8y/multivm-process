//! Example usage of the communication protocols
//!
//! This example demonstrates how to use the different communication protocols
//! (IPC, RPC, JWT) to interact with execution engines.

use multivm_common::communication::{
    factory::{DefaultProtocolFactory, ProtocolConfigBuilder},
    ipc_protocol::{IpcProtocolConfig, IpcTransportConfig},
    jwt_protocol::{HealthCheckConfig, HttpClientConfig, JwtProtocolConfig, RetryConfig},
    rpc_protocol::{RetryConfig as RpcRetryConfig, RpcProtocolConfig},
    CommunicationManager, CommunicationRequest, EngineType, ProtocolFactory, ProtocolType,
};
use multivm_common::MultivmResult;
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{error, info};
use uuid::Uuid;

#[tokio::main]
async fn main() -> MultivmResult<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting communication protocol examples");

    // Example 1: IPC Protocol
    example_ipc_protocol().await?;

    // Example 2: RPC Protocol
    example_rpc_protocol().await?;

    // Example 3: JWT Protocol
    example_jwt_protocol().await?;

    // Example 4: Protocol Manager with Fallback
    example_protocol_manager().await?;

    // Example 5: Custom Configuration
    example_custom_configuration().await?;

    info!("All examples completed successfully");
    Ok(())
}

/// Example 1: Using IPC Protocol for low-latency local communication
async fn example_ipc_protocol() -> MultivmResult<()> {
    info!("=== IPC Protocol Example ===");

    // Create IPC configuration
    let ipc_config = IpcProtocolConfig {
        transport_config: IpcTransportConfig {
            unix_socket_path: Some("/tmp/multivm-ethereum.sock".to_string()),
            tcp_host: "127.0.0.1".to_string(),
            tcp_port: 8545,
            use_unix_sockets: true,
            ..Default::default()
        },
        ..Default::default()
    };

    // Create protocol factory and IPC protocol
    let factory = DefaultProtocolFactory::new();
    let mut ipc_protocol = factory
        .create_protocol(
            ProtocolType::Ipc,
            EngineType::Ethereum,
            serde_json::to_value(ipc_config)?,
        )
        .await?;

    // Connect to the engine
    match ipc_protocol.connect().await {
        Ok(_) => info!("IPC protocol connected successfully"),
        Err(e) => {
            error!("Failed to connect IPC protocol: {}", e);
            return Ok(());
        }
    }

    // Send a request
    let request = CommunicationRequest {
        id: Uuid::new_v4().to_string(),
        method: "eth_chainId".to_string(),
        params: json!([]),
        timeout: Some(Duration::from_secs(10)),
        auth_context: None,
    };

    match ipc_protocol.send_request(request).await {
        Ok(response) => {
            info!("IPC Response: {:?}", response);
            if let Some(result) = response.result {
                info!("Chain ID: {}", result);
            }
        }
        Err(e) => error!("IPC request failed: {}", e),
    }

    ipc_protocol.disconnect().await?;
    Ok(())
}

/// Example 2: Using RPC Protocol for standard HTTP-based communication
async fn example_rpc_protocol() -> MultivmResult<()> {
    info!("=== RPC Protocol Example ===");

    // Create RPC configuration
    let rpc_config = RpcProtocolConfig {
        endpoint_url: "http://127.0.0.1:8545".to_string(),
        connect_timeout: Duration::from_secs(10),
        request_timeout: Duration::from_secs(30),
        max_connections: 10,
        retry_config: RpcRetryConfig {
            max_retries: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 2.0,
            jitter: true,
        },
        default_headers: HashMap::from([(
            "User-Agent".to_string(),
            "MultiVM-Example/1.0".to_string(),
        )]),
        use_http2: true,
        ..Default::default()
    };

    let factory = DefaultProtocolFactory::new();
    let mut rpc_protocol = factory
        .create_protocol(
            ProtocolType::Rpc,
            EngineType::Ethereum,
            serde_json::to_value(rpc_config)?,
        )
        .await?;

    // Connect to the RPC endpoint
    match rpc_protocol.connect().await {
        Ok(_) => info!("RPC protocol connected successfully"),
        Err(e) => {
            error!("Failed to connect RPC protocol: {}", e);
            return Ok(());
        }
    }

    // Send a request
    let request = CommunicationRequest {
        id: Uuid::new_v4().to_string(),
        method: "eth_blockNumber".to_string(),
        params: json!([]),
        timeout: Some(Duration::from_secs(10)),
        auth_context: None,
    };

    match rpc_protocol.send_request(request).await {
        Ok(response) => {
            info!("RPC Response: {:?}", response);
            if let Some(result) = response.result {
                info!("Current block number: {}", result);
            }
        }
        Err(e) => error!("RPC request failed: {}", e),
    }

    // Example: Subscribe to events (if supported)
    match rpc_protocol.subscribe(vec!["newHeads".to_string()]).await {
        Ok(_) => info!("Subscribed to newHeads events"),
        Err(e) => error!("Subscription failed: {}", e),
    }

    rpc_protocol.disconnect().await?;

    Ok(())
}

/// Example 3: Using JWT Protocol for authenticated communication
async fn example_jwt_protocol() -> MultivmResult<()> {
    info!("=== JWT Protocol Example ===");

    // Create JWT configuration
    let jwt_config = JwtProtocolConfig {
        endpoint_url: "http://127.0.0.1:8551".to_string(),
        jwt_secret: "your-secret-key-here-minimum-32-characters-long".to_string(),
        token_expiry: Duration::from_secs(3600),     // 1 hour
        refresh_threshold: Duration::from_secs(300), // 5 minutes
        http_config: HttpClientConfig {
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            user_agent: "MultiVM-JWT-Example/1.0".to_string(),
            ..Default::default()
        },
        retry_config: RetryConfig {
            max_retries: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 2.0,
            retry_auth_failures: true,
        },
        health_check_config: HealthCheckConfig {
            health_endpoint: "/health".to_string(),
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            enable_auto_check: true,
        },
        ..Default::default()
    };

    let factory = DefaultProtocolFactory::new();
    let mut jwt_protocol = factory
        .create_protocol(
            ProtocolType::Jwt,
            EngineType::Ethereum,
            serde_json::to_value(jwt_config)?,
        )
        .await?;

    // Connect to the JWT endpoint
    match jwt_protocol.connect().await {
        Ok(_) => info!("JWT protocol connected successfully"),
        Err(e) => {
            error!("Failed to connect JWT protocol: {}", e);
            return Ok(());
        }
    }

    // Send an authenticated request
    let request = CommunicationRequest {
        id: Uuid::new_v4().to_string(),
        method: "engine_getPayloadV1".to_string(),
        params: json!([]),
        timeout: Some(Duration::from_secs(10)),
        auth_context: None, // JWT token will be added automatically
    };

    match jwt_protocol.send_request(request).await {
        Ok(response) => {
            info!("JWT Response: {:?}", response);
            if let Some(result) = response.result {
                info!("Payload result: {}", result);
            }
        }
        Err(e) => error!("JWT request failed: {}", e),
    }

    jwt_protocol.disconnect().await?;
    Ok(())
}

/// Example 4: Using Protocol Manager with automatic fallback
async fn example_protocol_manager() -> MultivmResult<()> {
    info!("=== Protocol Manager Example ===");

    let factory = DefaultProtocolFactory::new();
    let mut manager = CommunicationManager::new(Box::new(factory));

    // Add multiple protocols for Ethereum
    let protocols = vec![
        (ProtocolType::Ipc, json!({})),
        (
            ProtocolType::Rpc,
            json!({"endpoint_url": "http://127.0.0.1:8545"}),
        ),
        (
            ProtocolType::Jwt,
            json!({"endpoint_url": "http://127.0.0.1:8551"}),
        ),
    ];

    for (proto_type, config) in protocols {
        match manager
            .add_protocol(proto_type.clone(), EngineType::Ethereum, config)
            .await
        {
            Ok(_) => {
                info!("Added {} protocol to manager", proto_type);
            }
            Err(e) => error!("Failed to create {} protocol: {}", proto_type, e),
        }
    }

    // Set protocol preference order
    manager.set_protocol_preferences(
        EngineType::Ethereum,
        vec![ProtocolType::Ipc, ProtocolType::Rpc, ProtocolType::Jwt],
    );

    // Send request with automatic fallback
    let request = CommunicationRequest {
        id: Uuid::new_v4().to_string(),
        method: "eth_syncing".to_string(),
        params: json!([]),
        timeout: Some(Duration::from_secs(10)),
        auth_context: None,
    };

    match manager
        .send_with_fallback(&EngineType::Ethereum, request)
        .await
    {
        Ok(response) => {
            info!("Response received (with fallback): {:?}", response);
            if let Some(metadata) = response.metadata.get("protocol") {
                info!("Protocol used: {}", metadata);
            }
        }
        Err(e) => error!("All protocols failed: {}", e),
    }

    // Check if protocols are available
    for protocol_type in &[ProtocolType::Ipc, ProtocolType::Rpc, ProtocolType::Jwt] {
        if let Some(protocol) = manager.get_protocol(protocol_type, &EngineType::Ethereum) {
            match protocol.health_check().await {
                Ok(health) => info!("{:?} - Healthy: {}", protocol_type, health.is_healthy),
                Err(e) => error!("{:?} - Health check failed: {}", protocol_type, e),
            }
        }
    }

    Ok(())
}

/// Example 5: Custom configuration using builder pattern
async fn example_custom_configuration() -> MultivmResult<()> {
    info!("=== Custom Configuration Example ===");

    // Use the configuration builder for easy setup
    let custom_ipc = IpcProtocolConfig {
        transport_config: IpcTransportConfig {
            unix_socket_path: Some("/var/run/multivm/ethereum.sock".to_string()),
            tcp_host: "0.0.0.0".to_string(),
            tcp_port: 9545,
            use_unix_sockets: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let custom_rpc = RpcProtocolConfig {
        endpoint_url: "https://ethereum-node.example.com:8545".to_string(),
        ..Default::default()
    };

    let builder = ProtocolConfigBuilder::new()
        .ipc(EngineType::Ethereum, custom_ipc)
        .rpc(EngineType::Ethereum, custom_rpc)
        .ipc(
            EngineType::Solana,
            IpcProtocolConfig {
                transport_config: IpcTransportConfig {
                    tcp_port: 8899,
                    ..Default::default()
                },
                ..Default::default()
            },
        );

    let factory = builder.build_factory();

    // Create protocols with custom configurations
    for engine in [EngineType::Ethereum, EngineType::Solana] {
        info!("Creating protocols for {} engine", engine);

        for protocol_type in factory.supported_protocols() {
            match factory
                .create_protocol(protocol_type.clone(), engine.clone(), json!({}))
                .await
            {
                Ok(_) => info!("  Created {} protocol", protocol_type),
                Err(e) => error!("  Failed to create {} protocol: {}", protocol_type, e),
            }
        }
    }

    Ok(())
}
