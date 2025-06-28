#!/bin/bash
set -euo pipefail

# Verify testnet configuration matches MultivmConfig structure
echo "Verifying testnet configuration..."

CONFIG_FILE="${1:-/tmp/multivm-testnet/config/testnet.toml}"

if [ ! -f "$CONFIG_FILE" ]; then
    echo "Config file not found: $CONFIG_FILE"
    echo "Run ./scripts/run-solo-testnet.sh first to generate config"
    exit 1
fi

# Required top-level sections
REQUIRED_SECTIONS=(
    "system"
    "server"
    "database"
    "cache"
    "blockchain"
    "consensus"
    "network"
    "ipc"
    "security"
    "monitoring"
    "logging"
)

echo "Checking required sections..."
for section in "${REQUIRED_SECTIONS[@]}"; do
    if grep -q "^\[$section\]" "$CONFIG_FILE"; then
        echo "✓ Found section: $section"
    else
        echo "✗ Missing section: $section"
        exit 1
    fi
done

# Check nested sections
echo ""
echo "Checking nested sections..."

# Server subsections
SERVER_SECTIONS=("rest" "graphql" "websocket" "admin")
for subsection in "${SERVER_SECTIONS[@]}"; do
    if grep -q "^\[server\.$subsection\]" "$CONFIG_FILE"; then
        echo "✓ Found server.$subsection"
    else
        echo "✗ Missing server.$subsection"
        exit 1
    fi
done

# Cache subsections
CACHE_SECTIONS=("redis" "memory")
for subsection in "${CACHE_SECTIONS[@]}"; do
    if grep -q "^\[cache\.$subsection\]" "$CONFIG_FILE"; then
        echo "✓ Found cache.$subsection"
    else
        echo "✗ Missing cache.$subsection"
        exit 1
    fi
done

# Blockchain subsections
BLOCKCHAIN_SECTIONS=("solana" "ethereum")
for subsection in "${BLOCKCHAIN_SECTIONS[@]}"; do
    if grep -q "^\[blockchain\.$subsection\]" "$CONFIG_FILE"; then
        echo "✓ Found blockchain.$subsection"
    else
        echo "✗ Missing blockchain.$subsection"
        exit 1
    fi
done

# Monitoring subsection
if grep -q "^\[monitoring\.health_check\]" "$CONFIG_FILE"; then
    echo "✓ Found monitoring.health_check"
else
    echo "✗ Missing monitoring.health_check"
    exit 1
fi

# IPC transport
if grep -q "^\[ipc\.transport\]" "$CONFIG_FILE"; then
    echo "✓ Found ipc.transport"
else
    echo "✗ Missing ipc.transport"
    exit 1
fi

echo ""
echo "Configuration structure verification PASSED!"
echo ""
echo "Note: This script only verifies the structure matches the MultivmConfig."
echo "The actual values are configured for a solo testnet environment."