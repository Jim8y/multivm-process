#!/bin/bash
set -e

echo "Starting MultiVM Node..."
echo "Node ID: ${NODE_ID:-unknown}"
echo "Node Type: ${NODE_TYPE:-validator}"

# Create necessary directories
mkdir -p /opt/multivm/data/consensus /opt/multivm/data/p2p /opt/multivm/logs

# Generate node key if it doesn't exist
if [ ! -f /opt/multivm/data/node_key.json ]; then
    echo "Generating node key..."
    # In a real implementation, this would generate proper crypto keys
    echo "{\"id\":\"${NODE_ID}\",\"key\":\"$(openssl rand -hex 32)\"}" > /opt/multivm/data/node_key.json
fi

# Wait for dependencies if not genesis node
if [ "${IS_GENESIS_VALIDATOR}" != "true" ]; then
    echo "Waiting for genesis node to be ready..."
    sleep 5
fi

# Start the MultiVM application
exec /opt/multivm/bin/multivm-application