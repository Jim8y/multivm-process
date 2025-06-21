# Duplicate Struct Analysis Report

This report identifies struct definitions that are duplicated across different modules in the multivm-process codebase.

## Duplicated Structures

### 1. NetworkConfig
Found in 3 different modules with similar but not identical fields:

**multivm-consensus/src/manager.rs**
```rust
pub struct NetworkConfig {
    pub node_id: String,
    pub listen_address: String,
    // Bootstrap nodes for initial connection
}
```

**multivm-consensus/src/malachite.rs**
```rust
pub struct NetworkConfig {
    pub listen_addr: String,
    pub peers: Vec<String>,
}
```

**multivm-p2p/src/config.rs**
```rust
pub struct NetworkConfig {
    pub peer_id: Option<String>,
    pub listen_addresses: Vec<String>,
    // External addresses to advertise to other peers
}
```

**Recommendation**: Consolidate into a single `NetworkConfig` in `multivm-common` with optional fields for different use cases.

### 2. NetworkStats
Found in 2 different modules with different fields:

**multivm-p2p/src/lib.rs**
```rust
pub struct NetworkStats {
    pub connected_peers: usize,
    pub messages_sent: u64,
    pub messages_received: u64,
    // ...
}
```

**multivm-common/src/traits/monitoring.rs**
```rust
pub struct NetworkStats {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
}
```

**Recommendation**: Merge into a comprehensive `NetworkStats` struct in `multivm-common` that includes all metrics.

### 3. PeerInfo
Found in 2 different modules with different purposes:

**multivm-p2p/src/lib.rs**
```rust
pub struct PeerInfo {
    pub peer_id: String,
    pub addresses: Vec<multiaddr::Multiaddr>,
    // Protocol versions supported by the peer
}
```

**multivm-consensus/src/messages.rs**
```rust
pub struct PeerInfo {
    pub node_id: NodeId,
    pub address: String,
    pub status: NodeStatus,
    pub last_seen: SystemTime,
    pub protocol_version: u32,
}
```

**Recommendation**: Create a base `PeerInfo` in `multivm-common` and extend it in specific modules if needed.

### 4. ConnectionInfo
Found in 2 different modules with different fields:

**multivm-p2p/src/transport.rs**
```rust
pub struct ConnectionInfo {
    pub peer_id: PeerId,
    pub address: Multiaddr,
    pub established_at: std::time::Instant,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}
```

**multivm-common/src/ipc/secure_transport.rs**
```rust
pub struct ConnectionInfo {
    pub remote_process_id: Option<String>,
    pub authenticated: bool,
    pub connected_at: SystemTime,
    pub last_activity: SystemTime,
}
```

**Recommendation**: These serve different purposes (P2P vs IPC). Consider renaming to `P2PConnectionInfo` and `IpcConnectionInfo`.

### 5. AccountBindingInfo
Found in 2 different modules with different structures:

**multivm-application/src/gateway/multivm.rs**
```rust
pub struct AccountBindingInfo {
    pub multivm_id: String,
    pub svm_address: Option<String>,
    pub evm_address: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
}
```

**multivm-consensus/src/traits.rs**
```rust
pub struct AccountBindingInfo {
    pub multivm_account: MultivmAccountId,
    pub bound_addresses: Vec<String>,
    // Binding metadata
}
```

**Recommendation**: Move to `multivm-common` and standardize the structure.

### 6. SystemInfo
Found in 2 different modules with different fields:

**multivm-application/src/api/rest/handlers/system.rs**
```rust
pub struct SystemInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub build_info: BuildInfo,
    pub runtime_info: RuntimeInfo,
}
```

**multivm-application/src/api/graphql/mod.rs**
```rust
pub struct SystemInfo {
    pub version: String,
    pub name: String,
}
```

**Recommendation**: Use the REST API version as the canonical one and import it in GraphQL module.

## Common Patterns Observed

### Config Structs
- Multiple `*Config` structs across modules (GatewayConfig, WebSocketConfig, AdminConfig, etc.)
- Most are appropriately module-specific and don't need consolidation

### Stats Structs
- Various `*Stats` structs (GatewayStats, CacheStats, MemoryCacheStats, etc.)
- Most are appropriately module-specific but could benefit from a common trait

### Error Types
- Well-organized with module-specific error enums
- No significant duplication found

### Request/Response Structs
- API-specific request/response structs are appropriately separated
- No problematic duplication found

## Recommendations

1. **Create Common Types Module**: Add a `multivm-common/src/types/network.rs` for shared network-related types.

2. **Standardize Naming**: Use prefixes to distinguish similar structs serving different purposes (e.g., `P2PConnectionInfo` vs `IpcConnectionInfo`).

3. **Extract Common Traits**: Create traits for common patterns like `Stats`, `Config`, and `Info` to ensure consistency.

4. **Move Shared Types**: Move `AccountBindingInfo` and similar cross-module types to `multivm-common`.

5. **Document Purpose**: Add clear documentation to distinguish between similar structs serving different purposes.