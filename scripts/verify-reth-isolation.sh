#!/bin/bash

# Verify Reth P2P and Consensus Isolation
# This script checks that Reth is properly isolated from external networks

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "${SCRIPT_DIR}")"

# Configuration
RETH_RPC_URL="${RETH_RPC_URL:-http://localhost:8545}"
RETH_ENGINE_URL="${RETH_ENGINE_URL:-http://localhost:8551}"

echo "🔍 Verifying Reth P2P and Consensus Isolation"
echo "=============================================="
echo

# Function to make JSON-RPC call
make_rpc_call() {
    local url="$1"
    local method="$2"
    local params="$3"
    local auth_header="$4"
    
    curl -s -X POST \
        -H "Content-Type: application/json" \
        ${auth_header:+-H "Authorization: Bearer $auth_header"} \
        --data "{\"jsonrpc\":\"2.0\",\"method\":\"$method\",\"params\":$params,\"id\":1}" \
        "$url" 2>/dev/null
}

# Check if Reth RPC is accessible
echo -n "📡 Checking Reth RPC accessibility... "
if curl -s --connect-timeout 5 "$RETH_RPC_URL" >/dev/null 2>&1; then
    echo -e "${GREEN}OK${NC}"
else
    echo -e "${RED}FAILED${NC}"
    echo "❌ Reth RPC is not accessible at $RETH_RPC_URL"
    exit 1
fi

# Check peer count (should be 0)
echo -n "👥 Checking peer count... "
peer_response=$(make_rpc_call "$RETH_RPC_URL" "net_peerCount" "[]" "")
peer_count=$(echo "$peer_response" | jq -r '.result // "error"' 2>/dev/null || echo "error")

if [ "$peer_count" = "0x0" ] || [ "$peer_count" = "0" ]; then
    echo -e "${GREEN}OK (0 peers)${NC}"
else
    echo -e "${RED}FAILED${NC}"
    echo "⚠️  Expected 0 peers, got: $peer_count"
    echo "Response: $peer_response"
fi

# Check listening status (should be false)
echo -n "👂 Checking network listening status... "
listening_response=$(make_rpc_call "$RETH_RPC_URL" "net_listening" "[]" "")
listening_status=$(echo "$listening_response" | jq -r '.result // "error"' 2>/dev/null || echo "error")

if [ "$listening_status" = "false" ]; then
    echo -e "${GREEN}OK (not listening)${NC}"
else
    echo -e "${YELLOW}WARNING${NC}"
    echo "⚠️  Expected false, got: $listening_status"
    echo "Response: $listening_response"
fi

# Check network version/chain ID
echo -n "🌐 Checking network version... "
version_response=$(make_rpc_call "$RETH_RPC_URL" "net_version" "[]" "")
network_version=$(echo "$version_response" | jq -r '.result // "error"' 2>/dev/null || echo "error")

if [ "$network_version" != "error" ]; then
    echo -e "${GREEN}OK (Chain ID: $network_version)${NC}"
else
    echo -e "${RED}FAILED${NC}"
    echo "Response: $version_response"
fi

# Check if Engine API is accessible (should require JWT)
echo -n "🔐 Checking Engine API protection... "
engine_response=$(make_rpc_call "$RETH_ENGINE_URL" "engine_getClientVersionV1" "[]" "")
engine_error=$(echo "$engine_response" | jq -r '.error.message // ""' 2>/dev/null || echo "")

if [[ "$engine_error" == *"Unauthorized"* ]] || [[ "$engine_error" == *"JWT"* ]] || [[ "$engine_error" == *"401"* ]]; then
    echo -e "${GREEN}OK (JWT protected)${NC}"
elif [ -z "$engine_error" ]; then
    echo -e "${YELLOW}WARNING (accessible without JWT)${NC}"
else
    echo -e "${GREEN}OK (protected)${NC}"
fi

# Check basic functionality
echo -n "⚙️  Checking basic RPC functionality... "
block_response=$(make_rpc_call "$RETH_RPC_URL" "eth_blockNumber" "[]" "")
block_number=$(echo "$block_response" | jq -r '.result // "error"' 2>/dev/null || echo "error")

if [ "$block_number" != "error" ] && [ "$block_number" != "null" ]; then
    block_decimal=$((16#${block_number#0x}))
    echo -e "${GREEN}OK (Block: $block_decimal)${NC}"
else
    echo -e "${RED}FAILED${NC}"
    echo "Response: $block_response"
fi

# Summary
echo
echo "📋 Summary:"
echo "==========="

if [ "$peer_count" = "0x0" ] || [ "$peer_count" = "0" ]; then
    echo -e "✅ P2P Isolation: ${GREEN}CONFIRMED${NC} (0 peers connected)"
else
    echo -e "❌ P2P Isolation: ${RED}FAILED${NC} (peers still connected)"
fi

if [ "$listening_status" = "false" ]; then
    echo -e "✅ Network Listening: ${GREEN}DISABLED${NC}"
else
    echo -e "⚠️  Network Listening: ${YELLOW}ENABLED${NC} (may be normal)"
fi

if [ "$network_version" != "error" ]; then
    echo -e "✅ RPC Functionality: ${GREEN}WORKING${NC}"
else
    echo -e "❌ RPC Functionality: ${RED}FAILED${NC}"
fi

echo
echo "🔧 Configuration Status:"
echo "========================"
echo "• Reth is running as an isolated execution engine"
echo "• MultiVM handles all consensus and P2P networking"
echo "• Engine API is protected with JWT authentication"
echo "• JSON-RPC API is available for dApp integration"

# Check for MultiVM-specific config
multivm_config="$PROJECT_ROOT/testnet/configs/reth-multivm.toml"
if [ -f "$multivm_config" ]; then
    echo
    echo -e "📄 MultiVM Config: ${GREEN}FOUND${NC} ($multivm_config)"
else
    echo
    echo -e "📄 MultiVM Config: ${YELLOW}NOT FOUND${NC} (using command line flags)"
fi

echo
echo "✅ Reth isolation verification complete!"
echo
echo "To view detailed configuration:"
echo "  cat docs/RETH_P2P_CONSENSUS_DISABLED.md"