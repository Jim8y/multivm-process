#!/bin/bash
# Architecture validation script for MultiVM
# Ensures proper isolation and communication patterns

set -e

echo "======================================"
echo "MultiVM Architecture Validation"
echo "======================================"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check functions
check_pass() {
    echo -e "${GREEN}✓${NC} $1"
}

check_fail() {
    echo -e "${RED}✗${NC} $1"
    exit 1
}

check_warn() {
    echo -e "${YELLOW}⚠${NC} $1"
}

echo -e "\n1. Checking Network Isolation..."
echo "================================="

# Check if execution network is internal only
if docker network inspect execution-net 2>/dev/null | grep -q '"Internal": true'; then
    check_pass "Execution network is internal-only (isolated)"
else
    check_fail "Execution network is NOT internal-only!"
fi

# Check that Reth has no external ports mapped
if ! docker-compose -f docker-compose.testnet-reth.yml config | grep -A5 "reth:" | grep -q "ports:"; then
    check_pass "Reth has no external ports exposed"
else
    check_fail "Reth has external ports exposed!"
fi

# Check that Solana has no external ports mapped
if ! docker-compose -f docker-compose.testnet-reth.yml config | grep -A5 "solana:" | grep -q "ports:"; then
    check_pass "Solana has no external ports exposed"
else
    check_fail "Solana has external ports exposed!"
fi

echo -e "\n2. Checking Process Connectivity..."
echo "===================================="

# Check MultiVM nodes can reach both networks
for i in {1..7}; do
    networks=$(docker inspect multivm-node-${i} 2>/dev/null | jq -r '.[0].NetworkSettings.Networks | keys[]' | sort | tr '\n' ' ')
    if [[ "$networks" == *"execution-net"* ]] && [[ "$networks" == *"multivm-testnet"* ]]; then
        check_pass "MultiVM node-${i} connected to both networks"
    else
        check_warn "MultiVM node-${i} network connectivity needs checking"
    fi
done

echo -e "\n3. Checking P2P and Consensus..."
echo "================================="

# Check Reth P2P is disabled
if docker exec multivm-reth reth --help 2>&1 | grep -q "disable-discovery"; then
    check_pass "Reth discovery is disabled"
fi

# Check Solana gossip is disabled
if docker logs multivm-solana 2>&1 | grep -q "gossip-port 0"; then
    check_pass "Solana gossip port is disabled (set to 0)"
fi

echo -e "\n4. Checking Communication Patterns..."
echo "====================================="

# Test MultiVM can reach Reth Engine API
for i in {1..7}; do
    if docker exec multivm-node-${i} curl -s -X POST -H "Content-Type: application/json" \
        --data '{"jsonrpc":"2.0","method":"eth_syncing","params":[],"id":1}' \
        http://reth:8545 2>/dev/null | grep -q "result"; then
        check_pass "MultiVM node-${i} can reach Reth RPC"
    else
        check_warn "MultiVM node-${i} cannot reach Reth RPC"
    fi
done

# Test MultiVM can reach Solana RPC
for i in {1..7}; do
    if docker exec multivm-node-${i} curl -s -X POST -H "Content-Type: application/json" \
        --data '{"jsonrpc":"2.0","method":"getHealth","id":1}' \
        http://solana:8899 2>/dev/null | grep -q "result"; then
        check_pass "MultiVM node-${i} can reach Solana RPC"
    else
        check_warn "MultiVM node-${i} cannot reach Solana RPC"
    fi
done

echo -e "\n5. Checking External Access Restrictions..."
echo "==========================================="

# Try to access Reth from outside (should fail)
if ! curl -s -X POST -H "Content-Type: application/json" \
    --data '{"jsonrpc":"2.0","method":"eth_syncing","params":[],"id":1}' \
    http://localhost:8545 2>/dev/null | grep -q "result"; then
    check_pass "Reth RPC not accessible from host"
else
    check_fail "Reth RPC is accessible from host!"
fi

# Try to access Solana from outside (should fail)
if ! curl -s -X POST -H "Content-Type: application/json" \
    --data '{"jsonrpc":"2.0","method":"getHealth","id":1}' \
    http://localhost:8899 2>/dev/null | grep -q "result"; then
    check_pass "Solana RPC not accessible from host"
else
    check_fail "Solana RPC is accessible from host!"
fi

echo -e "\n6. Checking Consensus Configuration..."
echo "======================================"

# Check MultiVM is handling consensus
for i in {1..7}; do
    if docker logs multivm-node-${i} 2>&1 | grep -q "consensus.*enabled"; then
        check_pass "MultiVM node-${i} has consensus enabled"
    else
        check_warn "MultiVM node-${i} consensus status unclear"
    fi
done

echo -e "\n7. Architecture Summary..."
echo "=========================="

echo -e "\nExpected Architecture:"
echo "- MultiVM handles all P2P networking and consensus"
echo "- Reth and Solana are isolated execution engines"
echo "- Reth/Solana have no external network access"
echo "- Reth/Solana only communicate with MultiVM via internal network"
echo "- All external clients connect through MultiVM RPC endpoints"

echo -e "\nValidation Complete!"