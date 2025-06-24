# TODO Implementation Plan

## Summary
Found 16 TODO comments across 5 source files. Most are in the P2P module waiting for libp2p API stability.

## Priority 1: Configuration System (2 TODOs)

### 1. Deep Merge Logic for Config Overrides
**File**: `multivm-common/src/config/unified.rs:664`
**Implementation**:
```rust
pub fn merge_override(&mut self, path: &str) -> crate::MultivmResult<()> {
    let override_config = Self::from_file(path)?;
    
    // Deep merge system config
    if let Some(override_system) = override_config.system {
        self.system.get_or_insert_with(Default::default).merge(override_system);
    }
    
    // Deep merge blockchain configs
    for (chain_id, override_chain) in override_config.blockchains {
        self.blockchains.entry(chain_id)
            .or_insert_with(Default::default)
            .merge(override_chain);
    }
    
    // Merge other fields...
    Ok(())
}
```

### 2. Configuration Validation
**File**: `multivm-common/src/config/unified.rs:671`
**Implementation**:
```rust
pub fn validate(&self) -> crate::MultivmResult<()> {
    // Check port conflicts
    let mut used_ports = HashSet::new();
    
    if let Some(system) = &self.system {
        // Validate RPC ports
        if !used_ports.insert(system.rpc.port) {
            return Err(MultivmError::Config(format!("Port {} already in use", system.rpc.port)));
        }
        
        // Validate IPC paths exist
        if !Path::new(&system.ipc.socket_path).parent().map(|p| p.exists()).unwrap_or(false) {
            return Err(MultivmError::Config("IPC socket directory doesn't exist".into()));
        }
    }
    
    // Validate blockchain configs
    for (chain_id, config) in &self.blockchains {
        config.validate()?;
    }
    
    Ok(())
}
```

## Priority 2: Account Binding Validation (1 TODO)

### 3. Account Binding Validation
**File**: `multivm-account-mapping/src/cross_vm_coordinator.rs:368`
**Implementation**:
```rust
// Validate accounts exist
let source_binding = self.account_mapping
    .get_account_binding(&source_account)
    .await
    .map_err(|_| AtomicError::InvalidAccount(source_account.clone()))?;

let target_binding = self.account_mapping
    .get_account_binding(&target_account)
    .await
    .map_err(|_| AtomicError::InvalidAccount(target_account.clone()))?;

// Verify accounts are properly bound
if !source_binding.is_active() || !target_binding.is_active() {
    return Err(AtomicError::AccountNotActive);
}
```

## Priority 3: CLI Config Migration (2 TODOs)

### 4. Legacy Config Field Mappings
**File**: `multivm-cli/src/config_migration.rs:43`
**Implementation**:
```rust
// Map legacy fields to new structure
if let Some(rpc_port) = legacy_config.get("rpc_port") {
    new_config["system"]["rpc"]["port"] = rpc_port.clone();
}

if let Some(node_type) = legacy_config.get("node_type") {
    new_config["system"]["node"]["node_type"] = node_type.clone();
}

if let Some(consensus) = legacy_config.get("consensus_enabled") {
    new_config["consensus"]["enabled"] = consensus.clone();
}

// Map blockchain-specific configs
if let Some(eth_config) = legacy_config.get("ethereum") {
    new_config["blockchains"]["1"] = eth_config.clone();
}

if let Some(sol_config) = legacy_config.get("solana") {
    new_config["blockchains"]["900"] = sol_config.clone();
}
```

### 5. Detailed Migration Report
**File**: `multivm-cli/src/config_migration.rs:130`
**Implementation**:
```rust
pub fn generate_migration_report(
    legacy_path: &str,
    new_path: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut report = String::new();
    
    report.push_str("# Configuration Migration Report\n\n");
    report.push_str(&format!("**Date**: {}\n", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));
    report.push_str(&format!("**Source**: {}\n", legacy_path));
    report.push_str(&format!("**Target**: {}\n\n", new_path));
    
    report.push_str("## Migration Summary\n\n");
    
    // Load configs for comparison
    let legacy_config = load_legacy_config(legacy_path)?;
    let new_config = UnifiedConfig::from_file(new_path)?;
    
    // Field mapping analysis
    report.push_str("### Field Mappings\n");
    report.push_str("| Legacy Field | New Field | Status |\n");
    report.push_str("|--------------|-----------|--------|\n");
    
    // Check each legacy field
    for (key, value) in legacy_config.as_object().unwrap() {
        let status = if new_config.contains_equivalent(key) {
            "✅ Migrated"
        } else {
            "⚠️ Not mapped"
        };
        report.push_str(&format!("| {} | {} | {} |\n", key, find_new_field(key), status));
    }
    
    // Validation results
    report.push_str("\n### Validation Results\n");
    match new_config.validate() {
        Ok(_) => report.push_str("✅ New configuration is valid\n"),
        Err(e) => report.push_str(&format!("❌ Validation failed: {}\n", e)),
    }
    
    Ok(report)
}
```

## Priority 4: P2P Request-Response (9 TODOs)

These TODOs are waiting for libp2p API stability. The current implementation uses gossipsub as a reliable alternative. When libp2p's request-response API stabilizes, implement:

1. Define request/response message types
2. Implement codec for serialization
3. Add request-response behaviour to network
4. Handle request-response events
5. Implement direct peer messaging

**Note**: Current gossipsub implementation is production-ready and works well for the use case.

## Implementation Timeline

1. **Week 1**: Configuration system (TODOs 1-2)
2. **Week 1**: Account binding validation (TODO 3)
3. **Week 2**: CLI migration (TODOs 4-5)
4. **Future**: P2P request-response when libp2p API is stable

## Notes

- Most critical TODOs are in configuration and validation
- P2P TODOs are not blocking - gossipsub works well
- Account mapping TODOs need IPC types from multivm-common first