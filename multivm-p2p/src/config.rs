//! Configuration management for P2P networking

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Main configuration for the P2P network layer
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct P2PConfig {
    /// Network configuration
    pub network: NetworkConfig,
    /// Transport configuration
    pub transport: TransportConfig,
    /// Discovery configuration
    pub discovery: DiscoveryConfig,
    /// Protocol configuration
    pub protocol: ProtocolConfig,
    /// Security configuration
    pub security: SecurityConfig,
    /// Logging configuration
    pub logging: LoggingConfig,
    /// Rate limiting configuration (alias for security.rate_limiting)
    pub rate_limiting: RateLimitConfig,
    /// Authentication configuration (alias for security.authentication)
    pub auth: AuthConfig,
}

/// Network layer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Local peer ID (optional, will be generated if not provided)
    pub peer_id: Option<String>,
    /// Listen addresses for incoming connections
    pub listen_addresses: Vec<String>,
    /// External addresses to advertise to other peers
    pub external_addresses: Vec<String>,
    /// Maximum number of connections
    pub max_connections: usize,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Keep-alive interval
    pub keep_alive_interval: Duration,
    /// Enable automatic NAT traversal
    pub enable_nat_traversal: bool,
    /// Enable relay support
    pub enable_relay: bool,
    /// Enable AutoNAT
    pub enable_autonat: bool,
    /// Maximum message size in bytes
    pub max_message_size: usize,
}

/// Transport layer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    /// Enable TCP transport
    pub enable_tcp: bool,
    /// Enable UDP/QUIC transport
    pub enable_quic: bool,
    /// Enable WebSocket transport
    pub enable_websocket: bool,
    /// TCP configuration
    pub tcp: TcpConfig,
    /// QUIC configuration
    pub quic: QuicConfig,
    /// WebSocket configuration
    pub websocket: WebSocketConfig,
}

/// TCP transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpConfig {
    /// TCP port range for listening
    pub port_range: (u16, u16),
    /// TCP nodelay option
    pub nodelay: bool,
    /// SO_REUSEADDR option
    pub reuse_addr: bool,
    /// Send buffer size
    pub send_buffer_size: Option<usize>,
    /// Receive buffer size
    pub recv_buffer_size: Option<usize>,
}

/// QUIC transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuicConfig {
    /// QUIC port range
    pub port_range: (u16, u16),
    /// Maximum concurrent streams
    pub max_concurrent_streams: usize,
    /// Maximum idle timeout
    pub max_idle_timeout: Duration,
    /// Keep alive interval
    pub keep_alive_interval: Duration,
}

/// WebSocket transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    /// WebSocket port range
    pub port_range: (u16, u16),
    /// Maximum frame size
    pub max_frame_size: usize,
    /// Enable compression
    pub enable_compression: bool,
}

/// Discovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    /// Enable mDNS discovery
    pub enable_mdns: bool,
    /// Enable Kademlia DHT
    pub enable_kademlia: bool,
    /// Bootstrap nodes for initial connection
    pub bootstrap_nodes: Vec<String>,
    /// Discovery interval
    pub discovery_interval: Duration,
    /// mDNS configuration
    pub mdns: MdnsConfig,
    /// Kademlia configuration
    pub kademlia: KademliaConfig,
}

/// mDNS discovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MdnsConfig {
    /// Service name for mDNS
    pub service_name: String,
    /// Query interval
    pub query_interval: Duration,
    /// TTL for mDNS records
    pub ttl: Duration,
}

/// Kademlia DHT configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KademliaConfig {
    /// DHT protocol name
    pub protocol_name: String,
    /// Replication factor
    pub replication_factor: usize,
    /// Query timeout
    pub query_timeout: Duration,
    /// Record TTL
    pub record_ttl: Duration,
    /// Republish interval
    pub republish_interval: Duration,
}

/// Protocol configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolConfig {
    /// Supported protocol versions
    pub supported_versions: Vec<u32>,
    /// Current protocol version
    pub current_version: u32,
    /// Enable protocol fallback
    pub enable_fallback: bool,
    /// Protocol-specific configurations
    pub protocol_specific: HashMap<String, ProtocolSpecificConfig>,
}

/// Protocol-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolSpecificConfig {
    /// Protocol name
    pub name: String,
    /// Protocol version
    pub version: String,
    /// Protocol parameters
    pub parameters: HashMap<String, String>,
    /// Enable/disable the protocol
    pub enabled: bool,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable noise encryption
    pub enable_noise: bool,
    /// Noise configuration
    pub noise: NoiseConfig,
    /// Rate limiting configuration
    pub rate_limiting: RateLimitConfig,
    /// Authentication configuration
    pub authentication: AuthConfig,
    /// Firewall configuration
    pub firewall: FirewallConfig,
}

/// Noise encryption configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoiseConfig {
    /// Noise protocol pattern
    pub pattern: String,
    /// Key derivation parameters
    pub key_derivation: KeyDerivationConfig,
}

/// Key derivation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyDerivationConfig {
    /// Algorithm used for key derivation
    pub algorithm: String,
    /// Salt for key derivation
    pub salt: Option<String>,
    /// Number of iterations
    pub iterations: u32,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Enable rate limiting
    pub enabled: bool,
    /// Maximum requests per peer per second
    pub max_requests_per_second: f64,
    /// Burst allowance
    pub burst_size: usize,
    /// Rate limit window
    pub window_duration: Duration,
    /// Penalty duration for rate limit violations
    pub penalty_duration: Duration,
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Enable peer authentication
    pub enabled: bool,
    /// Authentication method
    pub method: AuthMethod,
    /// Trusted peer list
    pub trusted_peers: Vec<String>,
    /// Authentication timeout
    pub timeout: Duration,
}

/// Authentication methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    /// No authentication
    None,
    /// Ed25519 signature-based authentication
    Ed25519,
    /// ECDSA signature-based authentication
    Ecdsa,
    /// Custom authentication
    Custom(String),
}

/// Firewall configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewallConfig {
    /// Enable firewall
    pub enabled: bool,
    /// Allowed peer IDs
    pub allowlist: Vec<String>,
    /// Blocked peer IDs
    pub blocklist: Vec<String>,
    /// Allowed IP ranges
    pub allowed_ips: Vec<String>,
    /// Blocked IP ranges
    pub blocked_ips: Vec<String>,
    /// Default firewall policy
    pub default_policy: FirewallPolicy,
}

/// Firewall policy enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FirewallPolicy {
    /// Allow all by default, block specific
    Allow,
    /// Block all by default, allow specific
    Block,
    /// Custom policy
    Custom,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    pub level: LogLevel,
    /// Enable structured logging
    pub structured: bool,
    /// Log format
    pub format: LogFormat,
    /// Enable metrics collection
    pub enable_metrics: bool,
    /// Metrics collection interval
    pub metrics_interval: Duration,
}

/// Log levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// Log formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogFormat {
    /// Plain text format
    Plain,
    /// JSON format
    Json,
    /// Custom format string
    Custom(String),
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            peer_id: None,
            listen_addresses: vec![
                "/ip4/0.0.0.0/tcp/0".to_string(),
                "/ip4/0.0.0.0/udp/0/quic-v1".to_string(),
            ],
            external_addresses: vec![],
            max_connections: 100,
            connection_timeout: Duration::from_secs(30),
            keep_alive_interval: Duration::from_secs(10),
            enable_nat_traversal: true,
            enable_relay: false,
            enable_autonat: true,
            max_message_size: 1024 * 1024, // 1MB
        }
    }
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            enable_tcp: true,
            enable_quic: true,
            enable_websocket: false,
            tcp: TcpConfig::default(),
            quic: QuicConfig::default(),
            websocket: WebSocketConfig::default(),
        }
    }
}

impl Default for TcpConfig {
    fn default() -> Self {
        Self {
            port_range: (10000, 65535),
            nodelay: true,
            reuse_addr: true,
            send_buffer_size: None,
            recv_buffer_size: None,
        }
    }
}

impl Default for QuicConfig {
    fn default() -> Self {
        Self {
            port_range: (10000, 65535),
            max_concurrent_streams: 100,
            max_idle_timeout: Duration::from_secs(30),
            keep_alive_interval: Duration::from_secs(10),
        }
    }
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            port_range: (8000, 9000),
            max_frame_size: 1024 * 1024, // 1MB
            enable_compression: true,
        }
    }
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            enable_mdns: true,
            enable_kademlia: true,
            bootstrap_nodes: vec![],
            discovery_interval: Duration::from_secs(60),
            mdns: MdnsConfig::default(),
            kademlia: KademliaConfig::default(),
        }
    }
}

impl Default for MdnsConfig {
    fn default() -> Self {
        Self {
            service_name: "_multivm._tcp.local".to_string(),
            query_interval: Duration::from_secs(60),
            ttl: Duration::from_secs(300),
        }
    }
}

impl Default for KademliaConfig {
    fn default() -> Self {
        Self {
            protocol_name: "/multivm/kad/1.0.0".to_string(),
            replication_factor: 20,
            query_timeout: Duration::from_secs(10),
            record_ttl: Duration::from_secs(3600),
            republish_interval: Duration::from_secs(1800),
        }
    }
}

impl Default for ProtocolConfig {
    fn default() -> Self {
        Self {
            supported_versions: vec![1],
            current_version: 1,
            enable_fallback: true,
            protocol_specific: HashMap::new(),
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_noise: true,
            noise: NoiseConfig::default(),
            rate_limiting: RateLimitConfig::default(),
            authentication: AuthConfig::default(),
            firewall: FirewallConfig::default(),
        }
    }
}

impl Default for NoiseConfig {
    fn default() -> Self {
        Self {
            pattern: "Noise_XX_25519_ChaChaPoly_BLAKE2s".to_string(),
            key_derivation: KeyDerivationConfig::default(),
        }
    }
}

impl Default for KeyDerivationConfig {
    fn default() -> Self {
        Self {
            algorithm: "HKDF-SHA256".to_string(),
            salt: None,
            iterations: 4096,
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_requests_per_second: 100.0,
            burst_size: 10,
            window_duration: Duration::from_secs(1),
            penalty_duration: Duration::from_secs(60),
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            method: AuthMethod::None,
            trusted_peers: vec![],
            timeout: Duration::from_secs(30),
        }
    }
}

impl Default for FirewallConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            allowlist: vec![],
            blocklist: vec![],
            allowed_ips: vec![],
            blocked_ips: vec![],
            default_policy: FirewallPolicy::Allow,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            structured: true,
            format: LogFormat::Json,
            enable_metrics: true,
            metrics_interval: Duration::from_secs(60),
        }
    }
}

impl P2PConfig {
    /// Load configuration from a file
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: P2PConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to a file
    pub fn to_file(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        // Validate network configuration
        if self.network.max_connections == 0 {
            return Err("max_connections must be greater than 0".to_string());
        }

        if self.network.listen_addresses.is_empty() {
            return Err("listen_addresses cannot be empty".to_string());
        }

        // Validate transport configuration
        if !self.transport.enable_tcp
            && !self.transport.enable_quic
            && !self.transport.enable_websocket
        {
            return Err("At least one transport must be enabled".to_string());
        }

        // Validate protocol configuration
        if self.protocol.supported_versions.is_empty() {
            return Err("supported_versions cannot be empty".to_string());
        }

        if !self
            .protocol
            .supported_versions
            .contains(&self.protocol.current_version)
        {
            return Err("current_version must be in supported_versions".to_string());
        }

        Ok(())
    }

    /// Create a minimal configuration for testing
    pub fn minimal() -> Self {
        Self {
            network: NetworkConfig {
                listen_addresses: vec!["/ip4/127.0.0.1/tcp/0".to_string()],
                max_connections: 10,
                ..Default::default()
            },
            discovery: DiscoveryConfig {
                enable_mdns: false,
                enable_kademlia: false,
                ..Default::default()
            },
            security: SecurityConfig {
                enable_noise: false,
                rate_limiting: RateLimitConfig {
                    enabled: false,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        }
    }
}
