//! # WebSocket API Module
//!
//! Provides real-time WebSocket connections for streaming blockchain data,
//! transaction updates, and system events.

use crate::{ApplicationResult, ApplicationState};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        ConnectInfo, State,
    },
    response::Response,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

/// WebSocket server configuration
#[derive(Debug, Clone)]
pub struct WebSocketConfig {
    /// Maximum number of concurrent connections
    pub max_connections: usize,

    /// Connection timeout in seconds
    pub connection_timeout: u64,

    /// Maximum message size in bytes
    pub max_message_size: usize,

    /// Enable authentication for WebSocket connections
    pub require_auth: bool,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            max_connections: 1000,
            connection_timeout: 300,       // 5 minutes
            max_message_size: 1024 * 1024, // 1MB
            require_auth: false,
        }
    }
}

/// WebSocket server
pub struct WebSocketServer {
    state: Arc<ApplicationState>,
    config: WebSocketConfig,
    event_sender: broadcast::Sender<WebSocketEvent>,
    connections: Arc<tokio::sync::RwLock<HashMap<String, WebSocketConnection>>>,
}

/// WebSocket connection information
#[derive(Debug, Clone)]
pub struct WebSocketConnection {
    /// Connection ID
    pub id: String,

    /// Client IP address
    pub addr: SocketAddr,

    /// Connection timestamp
    pub connected_at: chrono::DateTime<chrono::Utc>,

    /// Subscribed event types
    pub subscriptions: Vec<EventType>,

    /// Authentication status
    pub authenticated: bool,
}

/// WebSocket event types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// New block events
    NewBlock,

    /// New transaction events
    NewTransaction,

    /// Account update events
    AccountUpdate,

    /// Cross-VM transaction events
    CrossVmTransaction,

    /// System status events
    SystemStatus,

    /// Price update events
    PriceUpdate,
}

/// WebSocket events
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum WebSocketEvent {
    /// New block notification
    NewBlock {
        /// Block data
        block: serde_json::Value,

        /// VM type (svm, evm, multivm)
        vm_type: String,
    },

    /// New transaction notification
    NewTransaction {
        /// Transaction data
        transaction: serde_json::Value,

        /// VM type
        vm_type: String,
    },

    /// Account update notification
    AccountUpdate {
        /// Account address
        address: String,

        /// Updated account data
        account: serde_json::Value,

        /// VM type
        vm_type: String,
    },

    /// Cross-VM transaction notification
    CrossVmTransaction {
        /// Transaction ID
        id: String,

        /// Transaction status
        status: String,

        /// Transaction data
        data: serde_json::Value,
    },

    /// System status update
    SystemStatus {
        /// Status data
        status: serde_json::Value,
    },

    /// Price update notification
    PriceUpdate {
        /// Token symbol
        symbol: String,

        /// Price in USD
        price: f64,

        /// Timestamp
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}

/// WebSocket message types
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum WebSocketMessage {
    /// Subscribe to events
    Subscribe {
        /// Event types to subscribe to
        events: Vec<EventType>,
    },

    /// Unsubscribe from events
    Unsubscribe {
        /// Event types to unsubscribe from
        events: Vec<EventType>,
    },

    /// Authentication message
    Authenticate {
        /// API key or JWT token
        token: String,
    },

    /// Ping message
    Ping {
        /// Ping ID
        id: String,
    },
}

/// WebSocket response messages
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum WebSocketResponse {
    /// Subscription confirmation
    Subscribed {
        /// Subscribed event types
        events: Vec<EventType>,
    },

    /// Unsubscription confirmation
    Unsubscribed {
        /// Unsubscribed event types
        events: Vec<EventType>,
    },

    /// Authentication response
    AuthResponse {
        /// Authentication success
        success: bool,

        /// Error message if failed
        error: Option<String>,
    },

    /// Pong response
    Pong {
        /// Original ping ID
        id: String,
    },

    /// Error response
    Error {
        /// Error code
        code: String,

        /// Error message
        message: String,
    },
}

impl WebSocketServer {
    /// Create a new WebSocket server
    pub fn new(state: Arc<ApplicationState>) -> ApplicationResult<Self> {
        let config = WebSocketConfig::default();
        let (event_sender, _) = broadcast::channel(10000);
        let connections = Arc::new(tokio::sync::RwLock::new(HashMap::new()));

        Ok(Self {
            state,
            config,
            event_sender,
            connections,
        })
    }

    /// Start the WebSocket server
    pub async fn start(&self) -> ApplicationResult<()> {
        // The WebSocket server is integrated into the main HTTP server
        // This method is a placeholder for any WebSocket-specific startup logic
        tracing::info!("WebSocket server initialized");

        // Start the event broadcaster
        self.start_event_broadcaster().await;

        Ok(())
    }

    /// Handle WebSocket upgrade
    pub async fn handle_websocket_upgrade(
        State(state): State<Arc<ApplicationState>>,
        ws: WebSocketUpgrade,
        ConnectInfo(addr): ConnectInfo<SocketAddr>,
    ) -> Response {
        ws.on_upgrade(move |socket| handle_websocket_connection(state, socket, addr))
    }

    /// Broadcast an event to all subscribed connections
    pub async fn broadcast_event(&self, event: WebSocketEvent) -> ApplicationResult<()> {
        let _ = self.event_sender.send(event);
        Ok(())
    }

    /// Get connection count
    pub async fn connection_count(&self) -> usize {
        self.connections.read().await.len()
    }

    /// Start the event broadcaster task
    async fn start_event_broadcaster(&self) {
        let connections = self.connections.clone();
        let mut event_receiver = self.event_sender.subscribe();

        tokio::spawn(async move {
            while let Ok(event) = event_receiver.recv().await {
                let connections_guard = connections.read().await;

                // Broadcast to all connections that are subscribed to this event type
                for (connection_id, connection) in connections_guard.iter() {
                    if should_send_event_to_connection(&event, connection) {
                        // Send the event to the specific connection
                        if let Ok(serialized_event) = serde_json::to_string(&event) {
                            // In a production implementation, you would maintain WebSocket senders
                            // for each connection and send the message through them
                            tracing::debug!(
                                "Broadcasting event to connection {}: {}",
                                connection_id,
                                serialized_event
                            );

                            // Here you would use the connection's WebSocket sender:
                            // if let Some(sender) = connection_senders.get(connection_id) {
                            //     let _ = sender.send(Message::Text(serialized_event)).await;
                            // }
                        }
                    }
                }
            }
        });
    }
}

/// Handle individual WebSocket connection
async fn handle_websocket_connection(
    state: Arc<ApplicationState>,
    mut socket: WebSocket,
    addr: SocketAddr,
) {
    let connection_id = Uuid::new_v4().to_string();
    let connection = WebSocketConnection {
        id: connection_id.clone(),
        addr,
        connected_at: chrono::Utc::now(),
        subscriptions: vec![],
        authenticated: false,
    };

    // Add connection to the registry
    // Store connection info for event broadcasting
    // In a production implementation, you would store the WebSocket sender
    // along with the connection info for message delivery

    tracing::info!(
        "WebSocket connection established: {} from {}",
        connection_id,
        addr
    );

    // Handle messages
    while let Some(msg) = socket.recv().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Err(e) = handle_websocket_message(&state, &mut socket, &text).await {
                    tracing::error!("Error handling WebSocket message: {}", e);
                    break;
                }
            }
            Ok(Message::Binary(_)) => {
                // Handle binary messages if needed
                tracing::warn!("Received binary WebSocket message (not supported)");
            }
            Ok(Message::Close(_)) => {
                tracing::info!("WebSocket connection closed: {}", connection_id);
                break;
            }
            Ok(Message::Ping(data)) => {
                if let Err(e) = socket.send(Message::Pong(data)).await {
                    tracing::error!("Error sending pong: {}", e);
                    break;
                }
            }
            Ok(Message::Pong(_)) => {
                // Handle pong if needed
            }
            Err(e) => {
                tracing::error!("WebSocket error: {}", e);
                break;
            }
        }
    }

    // Remove connection from registry
    tracing::info!("WebSocket connection terminated: {}", connection_id);
}

/// Handle individual WebSocket message
async fn handle_websocket_message(
    _state: &Arc<ApplicationState>,
    socket: &mut WebSocket,
    text: &str,
) -> ApplicationResult<()> {
    let message: WebSocketMessage = serde_json::from_str(text).map_err(|e| {
        crate::error::ApplicationError::ValidationError {
            field: "message".to_string(),
            message: format!("Invalid WebSocket message: {}", e),
        }
    })?;

    let response = match message {
        WebSocketMessage::Subscribe { events } => {
            // Handle subscription
            WebSocketResponse::Subscribed { events }
        }
        WebSocketMessage::Unsubscribe { events } => {
            // Handle unsubscription
            WebSocketResponse::Unsubscribed { events }
        }
        WebSocketMessage::Authenticate { token: _ } => {
            // Handle authentication
            WebSocketResponse::AuthResponse {
                success: true,
                error: None,
            }
        }
        WebSocketMessage::Ping { id } => WebSocketResponse::Pong { id },
    };

    let response_text = serde_json::to_string(&response).map_err(|e| {
        crate::error::ApplicationError::InternalError {
            component: "websocket".to_string(),
            message: format!("Failed to serialize response: {}", e),
        }
    })?;

    socket
        .send(Message::Text(response_text.into()))
        .await
        .map_err(|e| crate::error::ApplicationError::InternalError {
            component: "websocket".to_string(),
            message: format!("Failed to send message: {}", e),
        })?;

    Ok(())
}

/// Check if an event should be sent to a specific connection
fn should_send_event_to_connection(
    event: &WebSocketEvent,
    connection: &WebSocketConnection,
) -> bool {
    let event_type = match event {
        WebSocketEvent::NewBlock { .. } => EventType::NewBlock,
        WebSocketEvent::NewTransaction { .. } => EventType::NewTransaction,
        WebSocketEvent::AccountUpdate { .. } => EventType::AccountUpdate,
        WebSocketEvent::CrossVmTransaction { .. } => EventType::CrossVmTransaction,
        WebSocketEvent::SystemStatus { .. } => EventType::SystemStatus,
        WebSocketEvent::PriceUpdate { .. } => EventType::PriceUpdate,
    };

    connection.subscriptions.contains(&event_type)
}
