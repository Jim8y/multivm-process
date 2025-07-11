#!/bin/bash
# MultiVM Node Startup Script

set -e

# Default configuration
CONFIG_FILE="${CONFIG_FILE:-/opt/multivm/config/config.toml}"
DATA_DIR="${DATA_DIR:-/opt/multivm/data}"
LOG_DIR="${LOG_DIR:-/opt/multivm/logs}"

# Generate secure JWT secret if not provided or if it's a test secret
if [ -z "$MULTIVM_JWT_SECRET" ] || [ "$MULTIVM_JWT_SECRET" = "test-secret-key-for-development-only-32chars" ]; then
    echo "Generating secure JWT secret..."
    MULTIVM_JWT_SECRET=$(dd if=/dev/urandom bs=32 count=1 2>/dev/null | base64 | tr -d '\n')
    export MULTIVM_JWT_SECRET
    echo "Warning: Generated random JWT secret. Save this for persistent deployments."
fi

# Create directories if they don't exist (handle permissions)
if [ -w "$(dirname "$DATA_DIR")" ] || [ -w "$DATA_DIR" ]; then
    mkdir -p "$DATA_DIR/consensus" "$DATA_DIR/p2p" "$DATA_DIR/execution" "$LOG_DIR" 2>/dev/null || true
else
    echo "Warning: Cannot create directories in $DATA_DIR (permission denied)"
    echo "Using existing directories..."
fi

# Ensure directories exist for non-root user
for dir in "$DATA_DIR" "$LOG_DIR" "$DATA_DIR/consensus" "$DATA_DIR/p2p" "$DATA_DIR/execution"; do
    if [ ! -d "$dir" ]; then
        echo "Creating directory: $dir"
        mkdir -p "$dir" 2>/dev/null || echo "Warning: Could not create $dir"
    fi
done

echo "========================================="
echo "Starting MultiVM Node"
echo "========================================="
echo "Node ID: ${NODE_ID:-unknown}"
echo "Node Type: ${NODE_TYPE:-validator}"
echo "Data Directory: ${DATA_DIR}"
echo "Log Directory: ${LOG_DIR}"
echo "P2P Address: ${MULTIVM_P2P_EXTERNAL_ADDRESS}"
echo "========================================="

# Generate node key if it doesn't exist
if [ ! -f "$DATA_DIR/node_key.json" ]; then
    echo "Generating secure node key..."
    # Generate a cryptographically secure ed25519 keypair for the node
    # Use /dev/urandom for better entropy
    NODE_KEY=$(dd if=/dev/urandom bs=32 count=1 2>/dev/null | xxd -p -c 32)
    echo "{\"id\":\"${NODE_ID}\",\"key\":\"${NODE_KEY}\"}" > "$DATA_DIR/node_key.json"
    chmod 600 "$DATA_DIR/node_key.json"  # Restrict permissions
fi

# Generate node-specific configuration if it doesn't exist
if [ ! -f "$CONFIG_FILE" ]; then
    echo "Generating configuration file..."
    mkdir -p "$(dirname "$CONFIG_FILE")"
    cat > "$CONFIG_FILE" <<EOF
# MultiVM Node Configuration
# Node ID: ${NODE_ID}

[node]
id = "${NODE_ID}"
type = "${NODE_TYPE}"
data_dir = "${DATA_DIR}"
log_dir = "${LOG_DIR}"

[api]
rest_port = ${MULTIVM_REST_PORT:-8080}
graphql_port = ${MULTIVM_GRAPHQL_PORT:-8081}
websocket_port = ${MULTIVM_WEBSOCKET_PORT:-8082}
admin_port = ${MULTIVM_ADMIN_PORT:-8083}
jwt_secret = "${MULTIVM_JWT_SECRET}"

[p2p]
listen_address = "${MULTIVM_P2P_LISTEN_ADDRESS}"
external_address = "${MULTIVM_P2P_EXTERNAL_ADDRESS}"
seeds = "${MULTIVM_P2P_SEEDS:-}"
max_peers = 50
enable_mdns = true
enable_kademlia = true
enable_gossipsub = true

[consensus]
listen_address = "${MULTIVM_CONSENSUS_LISTEN_ADDRESS}"
block_time_ms = 5000
transaction_pool_size = 10000
is_genesis_validator = ${IS_GENESIS_VALIDATOR:-false}

[monitoring]
enable_metrics = true
metrics_port = 9464
enable_tracing = true
enable_health_check = true
health_check_interval_secs = 30

[execution]
evm_enabled = true
svm_enabled = true
evm_ipc_path = "${DATA_DIR}/evm.ipc"
svm_ipc_path = "${DATA_DIR}/svm.ipc"

[cache]
redis_enabled = false
rocksdb_path = "${DATA_DIR}/rocksdb"
cache_size_mb = 1024

[security]
enable_encryption = true
enable_authentication = true
rate_limit_per_peer = 100
rate_limit_global = 1000
EOF
fi

# Wait for dependent services if not genesis node
if [ "${IS_GENESIS_VALIDATOR}" != "true" ] && [ -n "${MULTIVM_P2P_SEEDS}" ]; then
    echo "Waiting for seed nodes to be ready..."
    # Simple wait - in production, this would check actual connectivity
    sleep 10
fi

# Start the MultiVM application
echo "Starting MultiVM application..."
exec /opt/multivm/bin/multivm-application \
    --config "$CONFIG_FILE" \
    --node-id "${NODE_ID}" \
    2>&1 | tee -a "${LOG_DIR}/multivm.log"