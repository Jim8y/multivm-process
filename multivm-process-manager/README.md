# MultiVM Process Manager

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![Status](https://img.shields.io/badge/status-production--ready-green.svg)](PROJECT_STATUS.md)

The central coordinator for managing Solana and Ethereum execution engines in separate processes.

## Overview

The Process Manager serves as the main orchestrator for the multi-VM system. It provides:

- **Process Lifecycle Management**: Starting, stopping, and monitoring blockchain engine processes
- **Block Routing**: Intelligent routing of blocks to appropriate execution engines
- **Health Monitoring**: Continuous health checks with automatic recovery
- **Resource Monitoring**: CPU, memory, and system resource tracking
- **Event Management**: System-wide event handling and notifications
- **Configuration Management**: Centralized configuration for all components

## Architecture

```
External Block Provider
         │
         ▼
┌────────────────────┐
│   Process Manager  │
│                    │
│ ┌────────────────┐ │     ┌─────────────────┐
│ │ Block Router   │ │────▶│ Solana Process  │
│ └────────────────┘ │     └─────────────────┘
│                    │
│ ┌────────────────┐ │     ┌─────────────────┐
│ │ Health Monitor │ │────▶│ Ethereum Process│
│ └────────────────┘ │     └─────────────────┘
│                    │
│ ┌────────────────┐ │
│ │ IPC Controller │ │
│ └────────────────┘ │
└────────────────────┘
```

## Core Components

### Process Manager

The main struct that coordinates all operations:

```rust
pub struct MultivmProcessManager {
    inner: Arc<MultivmProcessManagerInner>,
}

struct MultivmProcessManagerInner {
    config: MultivmConfig,
    processes: RwLock<HashMap<ProcessId, ProcessHandle>>,
    health_monitor: HealthMonitor,
    block_router: BlockRouter,
    resource_monitor: Arc<Mutex<SystemResourceMonitor>>,
    metrics: Arc<Mutex<SystemMetrics>>,
    // ... other internal components
}
```

### Block Router

Routes incoming blocks to the appropriate engine process and handles cross-VM operations:

```rust
impl BlockRouter {
    pub async fn route_block(&self, block: MultiVMBlock) -> MultivmResult<()>;
    pub async fn distribute_to_engines(&self, block: &MultiVMBlock) -> MultivmResult<()>;
    // Handles cross-VM transaction processing and account mapping
}
```

### Health Monitor

Continuously monitors the health of engine processes:

```rust
impl HealthMonitor {
    pub async fn check_process_health(&self, handle: &ProcessHandle) -> MultivmResult<HealthStatus>;
    pub async fn start_monitoring(&self, processes: Arc<RwLock<HashMap<ProcessId, ProcessHandle>>>) -> MultivmResult<()>;
    // Provides comprehensive health tracking with automatic recovery
}
```

## API Reference

### Core Methods

#### `MultivmProcessManager::new(config: MultivmConfig) -> MultivmResult<Self>`
Creates a new process manager with the given configuration.

#### `MultivmProcessManager::start() -> MultivmResult<()>`
Starts all configured engine processes and begins monitoring.

#### `MultivmProcessManager::run() -> MultivmResult<()>`
Runs the main event loop until shutdown is requested.

#### `MultivmProcessManager::get_health_status() -> MultivmResult<SystemHealthStatus>`
Returns comprehensive health status of all processes and system metrics.

#### `MultivmProcessManager::shutdown(graceful: bool) -> MultivmResult<()>`
Shuts down all processes gracefully or forcefully with configurable timeout.

#### `MultivmProcessManager::register_process(handle: ProcessHandle) -> MultivmResult<()>`
Registers a new process handle with the manager for monitoring.

#### `MultivmProcessManager::unregister_process(process_id: ProcessId) -> MultivmResult<()>`
Unregisters a process from management and monitoring.

### Process Control

Internal process management methods:

#### `start_solana_engine() -> MultivmResult<()>` (internal)
Starts the Solana execution engine in a separate process if enabled in configuration.

#### `start_ethereum_engine() -> MultivmResult<()>` (internal)
Starts the Ethereum execution engine in a separate process if enabled in configuration.

#### Process Management
- Automatic process lifecycle management based on configuration
- Health monitoring with automatic restart capabilities
- Resource monitoring and enforcement
- Event-driven architecture for system coordination

### System Information

#### System Health Status
The `ProcessManagerHealthStatus` provides comprehensive system monitoring:

```rust
pub struct SystemHealthStatus {
    pub overall_healthy: bool,
    pub process_health: HashMap<ProcessId, HealthStatus>,
    pub system_uptime: Duration,
    pub total_blocks_processed: u64,
    pub active_processes: usize,
    pub timestamp: SystemTime,
}
```

#### Process Health Details
Each process provides detailed health information including:
- Process availability and responsiveness
- Block processing statistics
- Resource usage (CPU, memory)
- Error counts and last error details
- RPC server status

## Usage Examples

### Basic Setup and Usage

```rust
use multivm_process_manager::{MultivmProcessManager, run_multivm_system};
use multivm_common::MultivmConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Simple system startup
    let config = MultivmConfig::default();
    run_multivm_system(config).await?;
    Ok(())
}

// Or for more control:
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = MultivmConfig::default();
    
    // Create and start the process manager
    let manager = MultivmProcessManager::new(config).await?;
    manager.start().await?;
    
    // Monitor health
    let health = manager.get_health_status().await?;
    println!("System health: {:?}", health);
    
    // Run main event loop (blocks until shutdown)
    manager.run().await?;
    
    Ok(())
}
```

### System Monitoring

```rust
use multivm_process_manager::MultivmProcessManager;
use multivm_common::MultivmConfig;
use std::time::Duration;

async fn monitor_system() -> Result<(), Box<dyn std::error::Error>> {
    let config = MultivmConfig::default();
    let manager = MultivmProcessManager::new(config).await?;
    manager.start().await?;
    
    // Monitor system health
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            match manager.get_health_status().await {
                Ok(health) => {
                    println!("System healthy: {}", health.overall_healthy);
                    println!("Active processes: {}", health.active_processes);
                    println!("Total blocks processed: {}", health.total_blocks_processed);
                    
                    for (process_id, status) in health.process_health {
                        println!("{:?}: healthy={}, blocks={}", 
                               process_id, status.is_healthy, status.blocks_processed_total);
                    }
                }
                Err(e) => eprintln!("Health check failed: {}", e),
            }
        }
    });
    
    // Run main loop
    manager.run().await?;
    Ok(())
}
```

### Integration with Multivm System

The Process Manager integrates with other system components:

```rust
use multivm_process_manager::MultivmProcessManager;
use multivm_consensus::MultiVMConsensusManager;
use multivm_common::{MultivmConfig, MultiVMBlock};

async fn integrated_system() -> Result<(), Box<dyn std::error::Error>> {
    let config = MultivmConfig::default();
    
    // Start process manager
    let process_manager = MultivmProcessManager::new(config.clone()).await?;
    process_manager.start().await?;
    
    // The process manager automatically:
    // - Manages Solana and Ethereum engine processes
    // - Handles block routing through the BlockRouter
    // - Monitors system health and resources
    // - Provides event notifications
    
    // Run the system
    process_manager.run().await?;
    Ok(())
}
```

### Configuration

The process manager uses the central `MultivmConfig` structure:

```rust
use multivm_common::{MultivmConfig, SystemConfig, SolanaConfig, EthereumConfig};

let config = MultivmConfig {
    system: SystemConfig {
        data_dir: "./data".into(),
        health_check_interval: Duration::from_secs(30),
        shutdown_timeout: Duration::from_secs(30),
        max_processes: 10,
        resource_limits: Default::default(),
    },
    solana: SolanaConfig {
        enabled: true,
        data_dir: "./data/solana".into(),
        rpc_port: 8899,
        // ... other solana settings
    },
    ethereum: EthereumConfig {
        enabled: true,
        data_dir: "./data/ethereum".into(),
        rpc_port: 8545,
        // ... other ethereum settings
    },
    // ... other configuration sections
};
```

## Design Decisions

### Process Isolation
- **Rationale**: Separate processes prevent cascading failures and enable independent resource management
- **Implementation**: Uses tokio::process with secure IPC communication
- **Benefits**: Better fault tolerance, resource isolation, easier debugging, security boundaries

### Event-Driven Architecture
- **Rationale**: Loose coupling between components enables better scalability and maintainability
- **Implementation**: System-wide event handling with async/await patterns
- **Benefits**: Responsive system, clear component boundaries, extensible design

### Health Monitoring
- **Rationale**: Proactive monitoring prevents system degradation
- **Implementation**: Continuous health checks with comprehensive status reporting
- **Benefits**: Early problem detection, detailed system insights, automatic monitoring

### Resource Management
- **Rationale**: Prevent resource exhaustion from affecting system stability
- **Implementation**: System resource monitoring with configurable limits
- **Benefits**: System stability, predictable performance, resource awareness

## Configuration

### System Configuration

The process manager is configured through the central `MultivmConfig` structure:

```rust
// System-wide settings
system: SystemConfig {
    data_dir: PathBuf,
    health_check_interval: Duration,
    shutdown_timeout: Duration,
    max_processes: u32,
    resource_limits: ResourceLimits,
}

// Blockchain engine settings
solana: SolanaConfig {
    enabled: bool,
    data_dir: PathBuf,
    rpc_port: u16,
    // Additional Solana-specific configuration
}

ethereum: EthereumConfig {
    enabled: bool,
    data_dir: PathBuf,
    rpc_port: u16,
    // Additional Ethereum-specific configuration
}
```

### Resource Limits

```rust
resource_limits: ResourceLimits {
    max_memory_mb: Option<u64>,
    max_cpu_percent: Option<f64>,
    max_disk_usage_gb: Option<u64>,
    max_open_files: Option<u32>,
}
```

## Testing

### Running Tests

```bash
# Unit tests
cargo test --package multivm-process-manager

# All tests in the workspace
cd .. && cargo test --workspace

# With logging output
RUST_LOG=debug cargo test --package multivm-process-manager -- --nocapture
```

### Development

```bash
# Check compilation
cargo check --package multivm-process-manager

# Build with all features
cargo build --package multivm-process-manager --all-features

# Format code
cargo fmt --package multivm-process-manager

# Run clippy
cargo clippy --package multivm-process-manager
```

## Performance Considerations

### Architecture Benefits

#### Process Isolation
- Each engine runs in a separate process for fault isolation
- Independent memory spaces prevent cross-contamination
- Process crashes don't affect the main coordinator

#### Resource Monitoring
- Real-time tracking of CPU, memory, and disk usage
- Configurable resource limits with monitoring alerts
- System health metrics for operational visibility

#### Event-Driven Design
- Asynchronous event handling for responsive system behavior
- Clean separation of concerns between components
- Extensible architecture for future enhancements

#### Configuration Management
- Centralized configuration through MultivmConfig
- Environment-specific settings support
- Runtime configuration validation 