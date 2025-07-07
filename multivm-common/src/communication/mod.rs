//! Unified communication protocols for execution engines
//!
//! This module provides a flexible abstraction layer for communicating with
//! execution engines (Reth, Solana) through multiple protocols:
//! - IPC (Inter-Process Communication) - Unix sockets, TCP sockets
//! - RPC (Remote Procedure Call) - JSON-RPC over HTTP/WebSocket
//! - JWT (JSON Web Token) - Authenticated API calls

use crate::{MultivmError, MultivmResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

pub mod config;
pub mod factory;
pub mod ipc_protocol;
pub mod jwt_protocol;
pub mod rpc_protocol;

/// Unique identifier for communication protocols
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProtocolType {
    /// Inter-Process Communication via Unix sockets or TCP
    Ipc,
    /// JSON-RPC over HTTP or WebSocket
    Rpc,
    /// JWT-authenticated API communication
    Jwt,
}

impl fmt::Display for ProtocolType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProtocolType::Ipc => write!(f, "IPC"),
            ProtocolType::Rpc => write!(f, "RPC"),
            ProtocolType::Jwt => write!(f, "JWT"),
        }
    }
}

/// Execution engine types that can be communicated with
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EngineType {
    /// Ethereum execution engine (Reth)
    Ethereum,
    /// Solana execution engine
    Solana,
}

impl fmt::Display for EngineType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineType::Ethereum => write!(f, "Ethereum"),
            EngineType::Solana => write!(f, "Solana"),
        }
    }
}

/// Generic communication request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationRequest {
    /// Unique request identifier
    pub id: String,
    /// Target method/command to execute
    pub method: String,
    /// Request parameters as JSON
    pub params: serde_json::Value,
    /// Request timeout
    pub timeout: Option<Duration>,
    /// Authentication context
    pub auth_context: Option<AuthContext>,
}

/// Generic communication response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationResponse {
    /// Request ID this response corresponds to
    pub request_id: String,
    /// Response result (success case)
    pub result: Option<serde_json::Value>,
    /// Error information (failure case)
    pub error: Option<CommunicationError>,
    /// Response metadata
    pub metadata: HashMap<String, String>,
}

/// Authentication context for requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthContext {
    /// JWT token for authenticated requests
    pub token: Option<String>,
    /// API key for simple authentication
    pub api_key: Option<String>,
    /// Additional authentication headers
    pub headers: HashMap<String, String>,
}

/// Communication error information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationError {
    /// Error code
    pub code: i32,
    /// Human-readable error message
    pub message: String,
    /// Additional error data
    pub data: Option<serde_json::Value>,
}

impl fmt::Display for CommunicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Communication Error {}: {}", self.code, self.message)
    }
}

/// Connection status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStatus {
    /// Whether the connection is active
    pub is_connected: bool,
    /// Last successful communication timestamp
    pub last_success: Option<std::time::SystemTime>,
    /// Connection latency in milliseconds
    pub latency_ms: Option<u64>,
    /// Number of successful requests
    pub success_count: u64,
    /// Number of failed requests
    pub error_count: u64,
}

/// Health check information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Overall health status
    pub is_healthy: bool,
    /// Engine version information
    pub version: Option<String>,
    /// Supported capabilities
    pub capabilities: Vec<String>,
    /// Performance metrics
    pub metrics: HashMap<String, serde_json::Value>,
    /// Last health check timestamp
    pub timestamp: std::time::SystemTime,
}

/// Main trait for communication protocols
#[async_trait]
pub trait CommunicationProtocol: Send + Sync {
    /// Get the protocol type
    fn protocol_type(&self) -> ProtocolType;

    /// Get the target engine type
    fn engine_type(&self) -> EngineType;

    /// Connect to the execution engine
    async fn connect(&mut self) -> MultivmResult<()>;

    /// Disconnect from the execution engine
    async fn disconnect(&mut self) -> MultivmResult<()>;

    /// Check if currently connected
    fn is_connected(&self) -> bool;

    /// Send a request and wait for response
    async fn send_request(
        &self,
        request: CommunicationRequest,
    ) -> MultivmResult<CommunicationResponse>;

    /// Send a request without waiting for response (fire-and-forget)
    async fn send_notification(&self, request: CommunicationRequest) -> MultivmResult<()>;

    /// Get current connection status
    async fn get_connection_status(&self) -> MultivmResult<ConnectionStatus>;

    /// Perform health check on the engine
    async fn health_check(&self) -> MultivmResult<HealthStatus>;

    /// Subscribe to engine events (if supported)
    async fn subscribe(&self, event_types: Vec<String>) -> MultivmResult<()>;

    /// Unsubscribe from engine events
    async fn unsubscribe(&self, event_types: Vec<String>) -> MultivmResult<()>;

    /// Get protocol-specific configuration
    fn get_configuration(&self) -> serde_json::Value;

    /// Update protocol configuration
    async fn update_configuration(&mut self, config: serde_json::Value) -> MultivmResult<()>;
}

/// Protocol factory trait for creating communication instances
#[async_trait]
pub trait ProtocolFactory: Send + Sync {
    /// Create a new protocol instance
    async fn create_protocol(
        &self,
        protocol_type: ProtocolType,
        engine_type: EngineType,
        config: serde_json::Value,
    ) -> MultivmResult<Box<dyn CommunicationProtocol>>;

    /// List supported protocol types
    fn supported_protocols(&self) -> Vec<ProtocolType>;

    /// Check if a protocol type is supported
    fn supports_protocol(&self, protocol_type: &ProtocolType) -> bool {
        self.supported_protocols().contains(protocol_type)
    }
}

/// Multi-protocol communication manager
pub struct CommunicationManager {
    /// Active protocol instances
    protocols: HashMap<(ProtocolType, EngineType), Box<dyn CommunicationProtocol>>,
    /// Protocol factory for creating new instances
    factory: Box<dyn ProtocolFactory>,
    /// Default protocol preferences
    protocol_preferences: HashMap<EngineType, Vec<ProtocolType>>,
}

impl CommunicationManager {
    /// Create a new communication manager
    pub fn new(factory: Box<dyn ProtocolFactory>) -> Self {
        let mut protocol_preferences = HashMap::new();

        // Default protocol preferences
        protocol_preferences.insert(
            EngineType::Ethereum,
            vec![ProtocolType::Jwt, ProtocolType::Rpc, ProtocolType::Ipc],
        );
        protocol_preferences.insert(
            EngineType::Solana,
            vec![ProtocolType::Rpc, ProtocolType::Jwt, ProtocolType::Ipc],
        );

        Self {
            protocols: HashMap::new(),
            factory,
            protocol_preferences,
        }
    }

    /// Add a protocol instance
    pub async fn add_protocol(
        &mut self,
        protocol_type: ProtocolType,
        engine_type: EngineType,
        config: serde_json::Value,
    ) -> MultivmResult<()> {
        let protocol = self
            .factory
            .create_protocol(protocol_type.clone(), engine_type.clone(), config)
            .await?;

        self.protocols
            .insert((protocol_type, engine_type), protocol);
        Ok(())
    }

    /// Get a protocol instance
    pub fn get_protocol(
        &self,
        protocol_type: &ProtocolType,
        engine_type: &EngineType,
    ) -> Option<&dyn CommunicationProtocol> {
        self.protocols
            .get(&(protocol_type.clone(), engine_type.clone()))
            .map(|p| p.as_ref())
    }

    /// Get a mutable protocol instance
    pub fn get_protocol_mut(
        &mut self,
        protocol_type: &ProtocolType,
        engine_type: &EngineType,
    ) -> Option<&mut (dyn CommunicationProtocol + 'static)> {
        self.protocols
            .get_mut(&(protocol_type.clone(), engine_type.clone()))
            .map(|p| p.as_mut())
    }

    /// Send request with automatic protocol selection and fallback
    pub async fn send_with_fallback(
        &self,
        engine_type: &EngineType,
        request: CommunicationRequest,
    ) -> MultivmResult<CommunicationResponse> {
        let preferences = self.protocol_preferences.get(engine_type).ok_or_else(|| {
            MultivmError::Configuration {
                component: "communication_manager".to_string(),
                message: format!("No protocol preferences configured for engine: {engine_type}"),
                validation_errors: None,
            }
        })?;

        let mut last_error = None;

        for protocol_type in preferences {
            if let Some(protocol) = self.get_protocol(protocol_type, engine_type) {
                if !protocol.is_connected() {
                    continue;
                }

                match protocol.send_request(request.clone()).await {
                    Ok(response) => return Ok(response),
                    Err(e) => {
                        tracing::warn!(
                            "Protocol {} failed for engine {}: {}",
                            protocol_type,
                            engine_type,
                            e
                        );
                        last_error = Some(e);
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| MultivmError::Network {
            message: format!("No available protocols for engine: {engine_type}"),
            endpoint: Some("communication_manager".to_string()),
            retry_after: None,
        }))
    }

    /// Get health status for all protocols
    pub async fn get_all_health_status(
        &self,
    ) -> HashMap<(ProtocolType, EngineType), MultivmResult<HealthStatus>> {
        let mut results = HashMap::new();

        for ((protocol_type, engine_type), protocol) in &self.protocols {
            let health_result = protocol.health_check().await;
            results.insert((protocol_type.clone(), engine_type.clone()), health_result);
        }

        results
    }

    /// Set protocol preferences for an engine
    pub fn set_protocol_preferences(
        &mut self,
        engine_type: EngineType,
        preferences: Vec<ProtocolType>,
    ) {
        self.protocol_preferences.insert(engine_type, preferences);
    }

    /// Get protocol preferences for an engine
    pub fn get_protocol_preferences(&self, engine_type: &EngineType) -> Option<&Vec<ProtocolType>> {
        self.protocol_preferences.get(engine_type)
    }
}

#[cfg(test)]
mod tests {
    // Include all test modules
    mod factory_tests;
    mod integration_tests;
    mod ipc_protocol_tests;
    mod jwt_protocol_tests;
    mod manager_tests;
    mod rpc_protocol_tests;
}
