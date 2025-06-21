#!/bin/bash
# MultiVM Multi-Node Test Runner

set -e

# Environment variables
MULTIVM_NODES=${MULTIVM_NODES:-"multivm-node-1:8080,multivm-node-2:8080,multivm-node-3:8080,multivm-node-4:8080"}
TEST_TIMEOUT=${TEST_TIMEOUT:-300}
LOG_LEVEL=${LOG_LEVEL:-info}

# Convert node list to array
IFS=',' read -ra NODES <<< "$MULTIVM_NODES"

# Logging function
log() {
    echo "[$(date -Iseconds)] [TEST-RUNNER] $*"
}

# Test results directory
RESULTS_DIR="/opt/multivm/test-results"
mkdir -p "$RESULTS_DIR"

log "Starting MultiVM Multi-Node Tests"
log "Target Nodes: ${NODES[@]}"
log "Test Timeout: ${TEST_TIMEOUT}s"

# Function to check node health
check_node_health() {
    local node=$1
    local max_attempts=30
    local attempt=1
    
    log "Checking health of node: $node"
    
    while [ $attempt -le $max_attempts ]; do
        if curl -sf "http://$node/health" > /dev/null 2>&1; then
            log "Node $node is healthy"
            return 0
        fi
        
        log "Attempt $attempt/$max_attempts: Node $node not ready, waiting..."
        sleep 2
        attempt=$((attempt + 1))
    done
    
    log "ERROR: Node $node failed health check after $max_attempts attempts"
    return 1
}

# Function to run P2P connectivity tests
test_p2p_connectivity() {
    log "=== Testing P2P Connectivity ==="
    
    local results_file="$RESULTS_DIR/p2p_connectivity.json"
    echo "{" > "$results_file"
    echo "  \"test\": \"p2p_connectivity\"," >> "$results_file"
    echo "  \"timestamp\": \"$(date -Iseconds)\"," >> "$results_file"
    echo "  \"nodes\": [" >> "$results_file"
    
    local node_count=0
    for node in "${NODES[@]}"; do
        if [ $node_count -gt 0 ]; then
            echo "    ," >> "$results_file"
        fi
        
        log "Testing P2P connectivity for node: $node"
        
        # Get node info
        local node_info=$(curl -s "http://$node/api/v1/system/info" || echo '{}')
        local peer_count=$(curl -s "http://$node/api/v1/p2p/peers" | jq '.peers | length' 2>/dev/null || echo 0)
        
        echo "    {" >> "$results_file"
        echo "      \"node\": \"$node\"," >> "$results_file"
        echo "      \"peer_count\": $peer_count," >> "$results_file"
        echo "      \"node_info\": $node_info" >> "$results_file"
        echo "    }" >> "$results_file"
        
        log "Node $node has $peer_count peers"
        node_count=$((node_count + 1))
    done
    
    echo "  ]," >> "$results_file"
    echo "  \"status\": \"completed\"" >> "$results_file"
    echo "}" >> "$results_file"
    
    log "P2P connectivity test completed"
}

# Function to test consensus participation
test_consensus_participation() {
    log "=== Testing Consensus Participation ==="
    
    local results_file="$RESULTS_DIR/consensus_participation.json"
    echo "{" > "$results_file"
    echo "  \"test\": \"consensus_participation\"," >> "$results_file"
    echo "  \"timestamp\": \"$(date -Iseconds)\"," >> "$results_file"
    echo "  \"nodes\": [" >> "$results_file"
    
    local node_count=0
    for node in "${NODES[@]}"; do
        if [ $node_count -gt 0 ]; then
            echo "    ," >> "$results_file"
        fi
        
        log "Testing consensus participation for node: $node"
        
        # Get consensus stats
        local consensus_stats=$(curl -s "http://$node/api/v1/consensus/stats" || echo '{}')
        local block_height=$(echo "$consensus_stats" | jq '.current_height // 0' 2>/dev/null || echo 0)
        local total_blocks=$(echo "$consensus_stats" | jq '.total_blocks // 0' 2>/dev/null || echo 0)
        
        echo "    {" >> "$results_file"
        echo "      \"node\": \"$node\"," >> "$results_file"
        echo "      \"block_height\": $block_height," >> "$results_file"
        echo "      \"total_blocks\": $total_blocks," >> "$results_file"
        echo "      \"consensus_stats\": $consensus_stats" >> "$results_file"
        echo "    }" >> "$results_file"
        
        log "Node $node - Height: $block_height, Total Blocks: $total_blocks"
        node_count=$((node_count + 1))
    done
    
    echo "  ]," >> "$results_file"
    echo "  \"status\": \"completed\"" >> "$results_file"
    echo "}" >> "$results_file"
    
    log "Consensus participation test completed"
}

# Function to test block propagation
test_block_propagation() {
    log "=== Testing Block Propagation ==="
    
    local results_file="$RESULTS_DIR/block_propagation.json"
    
    # Capture initial heights
    local initial_heights=()
    for node in "${NODES[@]}"; do
        local height=$(curl -s "http://$node/api/v1/consensus/stats" | jq '.current_height // 0' 2>/dev/null || echo 0)
        initial_heights+=($height)
        log "Node $node initial height: $height"
    done
    
    # Wait for block production
    log "Waiting for block propagation (30 seconds)..."
    sleep 30
    
    # Capture final heights
    echo "{" > "$results_file"
    echo "  \"test\": \"block_propagation\"," >> "$results_file"
    echo "  \"timestamp\": \"$(date -Iseconds)\"," >> "$results_file"
    echo "  \"nodes\": [" >> "$results_file"
    
    local node_count=0
    local min_height=999999
    local max_height=0
    
    for i in "${!NODES[@]}"; do
        local node="${NODES[$i]}"
        local initial_height="${initial_heights[$i]}"
        
        if [ $node_count -gt 0 ]; then
            echo "    ," >> "$results_file"
        fi
        
        local final_height=$(curl -s "http://$node/api/v1/consensus/stats" | jq '.current_height // 0' 2>/dev/null || echo 0)
        local height_diff=$((final_height - initial_height))
        
        if [ $final_height -lt $min_height ]; then
            min_height=$final_height
        fi
        if [ $final_height -gt $max_height ]; then
            max_height=$final_height
        fi
        
        echo "    {" >> "$results_file"
        echo "      \"node\": \"$node\"," >> "$results_file"
        echo "      \"initial_height\": $initial_height," >> "$results_file"
        echo "      \"final_height\": $final_height," >> "$results_file"
        echo "      \"height_diff\": $height_diff" >> "$results_file"
        echo "    }" >> "$results_file"
        
        log "Node $node - Initial: $initial_height, Final: $final_height, Diff: $height_diff"
        node_count=$((node_count + 1))
    done
    
    local height_variance=$((max_height - min_height))
    
    echo "  ]," >> "$results_file"
    echo "  \"summary\": {" >> "$results_file"
    echo "    \"min_height\": $min_height," >> "$results_file"
    echo "    \"max_height\": $max_height," >> "$results_file"
    echo "    \"height_variance\": $height_variance" >> "$results_file"
    echo "  }," >> "$results_file"
    echo "  \"status\": \"completed\"" >> "$results_file"
    echo "}" >> "$results_file"
    
    log "Block propagation test completed - Variance: $height_variance blocks"
}

# Function to test cross-VM operations
test_cross_vm_operations() {
    log "=== Testing Cross-VM Operations ==="
    
    local results_file="$RESULTS_DIR/cross_vm_operations.json"
    local test_node="${NODES[0]}"
    
    log "Testing cross-VM operations via node: $test_node"
    
    # Test account mapping
    local mapping_result=$(curl -s -X POST "http://$test_node/api/v1/accounts/bind" \
        -H "Content-Type: application/json" \
        -d '{
            "solana_account": "11111111111111111111111111111111",
            "ethereum_account": "0x1111111111111111111111111111111111111111",
            "proof": "test-proof-data"
        }' || echo '{"error": "request_failed"}')
    
    # Test special transaction
    local tx_result=$(curl -s -X POST "http://$test_node/api/v1/transactions/submit" \
        -H "Content-Type: application/json" \
        -d '{
            "type": "cross_vm_transfer",
            "from_vm": "svm",
            "to_vm": "evm",
            "amount": 1000000,
            "from_account": "11111111111111111111111111111111",
            "to_account": "0x1111111111111111111111111111111111111111"
        }' || echo '{"error": "request_failed"}')
    
    echo "{" > "$results_file"
    echo "  \"test\": \"cross_vm_operations\"," >> "$results_file"
    echo "  \"timestamp\": \"$(date -Iseconds)\"," >> "$results_file"
    echo "  \"test_node\": \"$test_node\"," >> "$results_file"
    echo "  \"account_mapping\": $mapping_result," >> "$results_file"
    echo "  \"cross_vm_transfer\": $tx_result," >> "$results_file"
    echo "  \"status\": \"completed\"" >> "$results_file"
    echo "}" >> "$results_file"
    
    log "Cross-VM operations test completed"
}

# Main test execution
main() {
    log "Starting comprehensive MultiVM tests..."
    
    # Check all nodes are healthy
    local failed_nodes=()
    for node in "${NODES[@]}"; do
        if ! check_node_health "$node"; then
            failed_nodes+=("$node")
        fi
    done
    
    if [ ${#failed_nodes[@]} -gt 0 ]; then
        log "ERROR: The following nodes failed health checks: ${failed_nodes[*]}"
        exit 1
    fi
    
    log "All nodes are healthy, proceeding with tests..."
    
    # Run test suite
    test_p2p_connectivity
    test_consensus_participation
    test_block_propagation
    test_cross_vm_operations
    
    # Generate summary report
    local summary_file="$RESULTS_DIR/test_summary.json"
    echo "{" > "$summary_file"
    echo "  \"test_run\": {" >> "$summary_file"
    echo "    \"timestamp\": \"$(date -Iseconds)\"," >> "$summary_file"
    echo "    \"duration\": \"$(date +%s)\"," >> "$summary_file"
    echo "    \"nodes_tested\": ${#NODES[@]}," >> "$summary_file"
    echo "    \"tests_completed\": 4" >> "$summary_file"
    echo "  }," >> "$summary_file"
    echo "  \"results\": {" >> "$summary_file"
    echo "    \"p2p_connectivity\": \"$(test -f $RESULTS_DIR/p2p_connectivity.json && echo "completed" || echo "failed")\"," >> "$summary_file"
    echo "    \"consensus_participation\": \"$(test -f $RESULTS_DIR/consensus_participation.json && echo "completed" || echo "failed")\"," >> "$summary_file"
    echo "    \"block_propagation\": \"$(test -f $RESULTS_DIR/block_propagation.json && echo "completed" || echo "failed")\"," >> "$summary_file"
    echo "    \"cross_vm_operations\": \"$(test -f $RESULTS_DIR/cross_vm_operations.json && echo "completed" || echo "failed")\"" >> "$summary_file"
    echo "  }" >> "$summary_file"
    echo "}" >> "$summary_file"
    
    log "All tests completed! Results saved to: $RESULTS_DIR"
    log "Test summary:"
    cat "$summary_file"
    
    # Keep container running for result inspection
    log "Tests finished. Container will remain running for result inspection."
    log "Use 'docker exec -it multivm-test-runner /bin/bash' to inspect results."
    
    # Wait indefinitely
    tail -f /dev/null
}

# Execute main function
main "$@"