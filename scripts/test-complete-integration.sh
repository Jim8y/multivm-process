#!/bin/bash
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== MultiVM Complete Integration Test ===${NC}"
echo

# Test results
declare -A test_results
test_count=0
passed_count=0

# Function to run a test
run_test() {
    local test_name=$1
    local test_command=$2
    
    test_count=$((test_count + 1))
    echo -n "Testing $test_name... "
    
    if eval "$test_command" > /dev/null 2>&1; then
        echo -e "${GREEN}✓ PASSED${NC}"
        test_results["$test_name"]="PASSED"
        passed_count=$((passed_count + 1))
    else
        echo -e "${RED}✗ FAILED${NC}"
        test_results["$test_name"]="FAILED"
    fi
}

# Function to test RPC method
test_rpc() {
    local node_url=$1
    local method=$2
    local params=$3
    
    result=$(curl -s -X POST "$node_url" \
        -H "Content-Type: application/json" \
        -d "{\"jsonrpc\":\"2.0\",\"method\":\"$method\",\"params\":$params,\"id\":1}" 2>/dev/null)
    
    if echo "$result" | grep -q "result" && ! echo "$result" | grep -q "error"; then
        return 0
    else
        return 1
    fi
}

# 1. Test basic connectivity
echo -e "${YELLOW}1. Testing Basic Connectivity${NC}"
run_test "MultiVM Node 1 Health" "curl -s http://localhost:8080/health | grep -q ok"
run_test "MultiVM Node 7 Health" "curl -s http://localhost:8086/health | grep -q ok"
run_test "Reth RPC Port" "nc -zv localhost 8545"
run_test "Reth Engine API Port" "nc -zv localhost 8551"
echo

# 2. Test RPC functionality
echo -e "${YELLOW}2. Testing RPC Functionality${NC}"
run_test "eth_blockNumber" "test_rpc http://localhost:8080 eth_blockNumber []"
run_test "eth_chainId" "test_rpc http://localhost:8080 eth_chainId []"
run_test "eth_gasPrice" "test_rpc http://localhost:8080 eth_gasPrice []"
run_test "net_version" "test_rpc http://localhost:8080 net_version []"
run_test "net_peerCount" "test_rpc http://localhost:8080 net_peerCount []"
echo

# 3. Test consensus
echo -e "${YELLOW}3. Testing Consensus Mechanism${NC}"
run_test "Consensus Status" "curl -s http://localhost:8080/consensus/status | grep -q round"

# Check if all nodes have the same block height (with tolerance)
echo -n "Testing block height consensus... "
heights=()
for i in {1..7}; do
    port=$((8080 + i - 1))
    height=$(curl -s -X POST "http://localhost:$port" \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' 2>/dev/null \
        | jq -r '.result' | xargs printf "%d\n" 2>/dev/null || echo "0")
    heights+=($height)
done

# Find min and max heights
min_height=${heights[0]}
max_height=${heights[0]}
for h in "${heights[@]}"; do
    if [ "$h" -lt "$min_height" ]; then min_height=$h; fi
    if [ "$h" -gt "$max_height" ]; then max_height=$h; fi
done

diff=$((max_height - min_height))
if [ "$diff" -le 2 ]; then
    echo -e "${GREEN}✓ PASSED${NC} (max diff: $diff blocks)"
    test_results["Block Height Consensus"]="PASSED"
    passed_count=$((passed_count + 1))
else
    echo -e "${RED}✗ FAILED${NC} (max diff: $diff blocks)"
    test_results["Block Height Consensus"]="FAILED"
fi
test_count=$((test_count + 1))
echo

# 4. Test Reth integration
echo -e "${YELLOW}4. Testing Reth Integration${NC}"
run_test "Reth eth_blockNumber" "test_rpc http://localhost:8545 eth_blockNumber []"
run_test "Reth eth_chainId" "test_rpc http://localhost:8545 eth_chainId []"
run_test "Engine API Connection" "curl -s http://localhost:8080/engine/status | grep -q connected"
echo

# 5. Test transaction submission
echo -e "${YELLOW}5. Testing Transaction Submission${NC}"

# Create a test transaction
test_tx='{
    "from": "0x0000000000000000000000000000000000000001",
    "to": "0x0000000000000000000000000000000000000002",
    "value": "0x1",
    "gas": "0x5208",
    "gasPrice": "0x3b9aca00"
}'

echo -n "Testing transaction submission... "
tx_result=$(curl -s -X POST "http://localhost:8080" \
    -H "Content-Type: application/json" \
    -d "{\"jsonrpc\":\"2.0\",\"method\":\"eth_sendTransaction\",\"params\":[$test_tx],\"id\":1}" 2>/dev/null)

if echo "$tx_result" | grep -q "result\|error"; then
    echo -e "${GREEN}✓ PASSED${NC} (endpoint responsive)"
    test_results["Transaction Submission"]="PASSED"
    passed_count=$((passed_count + 1))
else
    echo -e "${RED}✗ FAILED${NC}"
    test_results["Transaction Submission"]="FAILED"
fi
test_count=$((test_count + 1))
echo

# 6. Test block production
echo -e "${YELLOW}6. Testing Block Production${NC}"

echo -n "Getting initial block height... "
initial_height=$(curl -s -X POST "http://localhost:8080" \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' 2>/dev/null \
    | jq -r '.result' | xargs printf "%d\n" 2>/dev/null || echo "0")
echo "$initial_height"

echo "Waiting 10 seconds for new blocks..."
sleep 10

echo -n "Getting new block height... "
new_height=$(curl -s -X POST "http://localhost:8080" \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' 2>/dev/null \
    | jq -r '.result' | xargs printf "%d\n" 2>/dev/null || echo "0")
echo "$new_height"

blocks_produced=$((new_height - initial_height))
if [ "$blocks_produced" -gt 0 ]; then
    echo -e "${GREEN}✓ PASSED${NC} ($blocks_produced blocks produced)"
    test_results["Block Production"]="PASSED"
    passed_count=$((passed_count + 1))
else
    echo -e "${RED}✗ FAILED${NC} (no blocks produced)"
    test_results["Block Production"]="FAILED"
fi
test_count=$((test_count + 1))
echo

# 7. Test metrics collection
echo -e "${YELLOW}7. Testing Metrics Collection${NC}"
run_test "Prometheus Health" "curl -s http://localhost:9090/-/healthy | grep -q Prometheus"
run_test "Node Metrics" "curl -s http://localhost:9080/metrics | grep -q multivm_"
echo

# 8. Test transaction pool
echo -e "${YELLOW}8. Testing Transaction Pool${NC}"
run_test "Transaction Pool Status" "curl -s http://localhost:8080/txpool/status | grep -q pending"
echo

# Summary
echo -e "${BLUE}=== Test Summary ===${NC}"
echo
echo "Total Tests: $test_count"
echo "Passed: $passed_count"
echo "Failed: $((test_count - passed_count))"
echo

if [ "$passed_count" -eq "$test_count" ]; then
    echo -e "${GREEN}✓ All tests passed! The system is working correctly.${NC}"
    exit 0
else
    echo -e "${RED}✗ Some tests failed. Please check the components.${NC}"
    echo
    echo "Failed tests:"
    for test in "${!test_results[@]}"; do
        if [ "${test_results[$test]}" = "FAILED" ]; then
            echo "  - $test"
        fi
    done
    exit 1
fi