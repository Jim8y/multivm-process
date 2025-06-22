#!/bin/bash
# MultiVM Node Startup Script

set -e

# Environment variables with defaults
NODE_ID=${NODE_ID:-"unknown"}
NODE_TYPE=${NODE_TYPE:-"full"}
P2P_LISTEN_ADDR=${P2P_LISTEN_ADDR:-"0.0.0.0:26656"}
API_LISTEN_ADDR=${API_LISTEN_ADDR:-"0.0.0.0:8080"}
CONSENSUS_ROLE=${CONSENSUS_ROLE:-"observer"}
LOG_LEVEL=${LOG_LEVEL:-"info"}
BOOTSTRAP_NODES=${BOOTSTRAP_NODES:-""}
VALIDATOR_KEY=${VALIDATOR_KEY:-""}
BLOCK_GENERATION_ENABLED=${BLOCK_GENERATION_ENABLED:-"false"}
BLOCK_INTERVAL_MS=${BLOCK_INTERVAL_MS:-"2000"}
SVM_TX_PER_BLOCK=${SVM_TX_PER_BLOCK:-"3"}
EVM_TX_PER_BLOCK=${EVM_TX_PER_BLOCK:-"3"}

# Logging function
log() {
    echo "[$(date -Iseconds)] [$NODE_ID] $*"
}

# Create necessary directories
mkdir -p /opt/multivm/{data,logs,keys}

log "Starting MultiVM Node: $NODE_ID"
log "Node Type: $NODE_TYPE"
log "Consensus Role: $CONSENSUS_ROLE"
log "P2P Listen Address: $P2P_LISTEN_ADDR"
log "API Listen Address: $API_LISTEN_ADDR"
log "Block Generation Enabled: $BLOCK_GENERATION_ENABLED"
log "Block Interval: ${BLOCK_INTERVAL_MS}ms"

# Generate configuration file
cat > /opt/multivm/config/runtime.toml << EOF
[node]
id = "$NODE_ID"
type = "$NODE_TYPE"
role = "$CONSENSUS_ROLE"

[network]
p2p_listen_addr = "$P2P_LISTEN_ADDR"
api_listen_addr = "$API_LISTEN_ADDR"
bootstrap_nodes = ["$(echo $BOOTSTRAP_NODES | tr ',' '", "')"]

[consensus]
validator_key = "$VALIDATOR_KEY"
algorithm = "malachite"
timeout_propose_ms = 3000
timeout_prevote_ms = 1000
timeout_precommit_ms = 1000

[logging]
level = "$LOG_LEVEL"
file = "/opt/multivm/logs/${NODE_ID}.log"

[storage]
data_dir = "/opt/multivm/data"

[api]
rest_enabled = true
rest_addr = "$API_LISTEN_ADDR"
graphql_enabled = true
websocket_enabled = true

[execution]
solana_mode = "mock"
reth_mode = "mock"

[block_generation]
enabled = $BLOCK_GENERATION_ENABLED
interval_ms = $BLOCK_INTERVAL_MS
svm_tx_per_block = $SVM_TX_PER_BLOCK
evm_tx_per_block = $EVM_TX_PER_BLOCK
EOF

# Wait for bootstrap nodes if this is not the bootstrap node
if [ "$NODE_TYPE" != "bootstrap" ] && [ ! -z "$BOOTSTRAP_NODES" ]; then
    log "Waiting for bootstrap nodes to be ready..."
    
    for node in $(echo $BOOTSTRAP_NODES | tr ',' ' '); do
        node_host=$(echo $node | cut -d':' -f1)
        node_port=$(echo $node | cut -d':' -f2)
        
        log "Checking connectivity to $node_host:$node_port"
        
        # Wait for node to be reachable (simplified check)
        timeout=60
        count=0
        while [ $count -lt $timeout ]; do
            if nc -z $node_host $node_port 2>/dev/null; then
                log "Bootstrap node $node is reachable"
                break
            fi
            sleep 1
            count=$((count + 1))
        done
        
        if [ $count -eq $timeout ]; then
            log "WARNING: Could not connect to bootstrap node $node"
        fi
    done
    
    # Additional startup delay for network stabilization
    log "Waiting for network stabilization..."
    sleep 10
fi

# Start the MultiVM node
log "Starting MultiVM application..."

# Note: This would normally start the actual multivm-node binary
# For now, we'll start a simple HTTP server to simulate the node
exec /opt/multivm/bin/multivm-node \
    --config /opt/multivm/config/runtime.toml \
    --data-dir /opt/multivm/data \
    --log-level $LOG_LEVEL \
    2>&1 | tee -a /opt/multivm/logs/${NODE_ID}.log