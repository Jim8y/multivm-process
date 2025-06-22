#!/bin/bash
# Simulate single node MultiVM network with block generation

# Configuration
LOG_FILE="multivm-single-node-simulated-$(date +%Y%m%d_%H%M%S).log"
BLOCK_INTERVAL=2  # seconds
TOTAL_BLOCKS=50   # Total blocks to generate in simulation

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
PURPLE='\033[0;35m'
NC='\033[0m'

# Initialize log file
{
    echo "MultiVM Single Node Network - Simulated Log"
    echo "==========================================="
    echo "Started at: $(date)"
    echo "Node Configuration:"
    echo "  - Node ID: single-node"
    echo "  - Type: Bootstrap Validator"
    echo "  - Block Generation: Enabled"
    echo "  - Block Interval: ${BLOCK_INTERVAL}s"
    echo "  - SVM Transactions per block: 3"
    echo "  - EVM Transactions per block: 3"
    echo ""
} > "$LOG_FILE"

echo -e "${BLUE}MultiVM Single Node Network Simulator${NC}"
echo "====================================="
echo "Log file: $LOG_FILE"
echo ""

# Simulate startup sequence
simulate_startup() {
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    
    echo -e "${YELLOW}Starting MultiVM Node...${NC}"
    echo "[$timestamp] [INFO] Starting MultiVM Node..." | tee -a "$LOG_FILE"
    sleep 0.5
    
    echo "[$timestamp] [INFO] Loading configuration from /opt/multivm/config/multivm.toml" | tee -a "$LOG_FILE"
    sleep 0.3
    
    echo "[$timestamp] [INFO] Initializing execution engines..." | tee -a "$LOG_FILE"
    echo "[$timestamp] [INFO] Starting Solana execution engine (mock mode)" | tee -a "$LOG_FILE"
    echo "[$timestamp] [INFO] Starting Reth execution engine (mock mode)" | tee -a "$LOG_FILE"
    sleep 0.5
    
    echo "[$timestamp] [INFO] Initializing Malachite consensus..." | tee -a "$LOG_FILE"
    echo "[$timestamp] [INFO] Node ID: single-node, Role: validator" | tee -a "$LOG_FILE"
    sleep 0.3
    
    echo "[$timestamp] [INFO] Starting P2P network on 0.0.0.0:26656" | tee -a "$LOG_FILE"
    echo "[$timestamp] [INFO] Starting API server on 0.0.0.0:8080" | tee -a "$LOG_FILE"
    sleep 0.5
    
    echo -e "${GREEN}[$timestamp] [INFO] MultiVM Coordinator initialized${NC}" | tee -a "$LOG_FILE"
    echo "[$timestamp] [INFO] MultiVM Coordinator started" | tee -a "$LOG_FILE"
    sleep 0.5
    
    echo -e "${GREEN}[$timestamp] [INFO] Block generator started successfully${NC}" | tee -a "$LOG_FILE"
    echo "[$timestamp] [INFO] MultiVM Node is running with continuous block generation..." | tee -a "$LOG_FILE"
    echo "[$timestamp] [INFO] Generating blocks every ${BLOCK_INTERVAL} seconds" | tee -a "$LOG_FILE"
    echo ""
}

# Simulate block generation
simulate_block_generation() {
    local block_height=1
    local total_svm_tx=0
    local total_evm_tx=0
    
    echo -e "${PURPLE}Starting block generation simulation...${NC}"
    echo "======================================" | tee -a "$LOG_FILE"
    echo "" | tee -a "$LOG_FILE"
    
    while [ $block_height -le $TOTAL_BLOCKS ]; do
        local timestamp=$(date '+%Y-%m-%d %H:%M:%S.%3N')
        local svm_tx=3
        local evm_tx=3
        
        # Generate block log
        echo -e "${GREEN}[$timestamp] [INFO] Generated mock block $block_height with $svm_tx SVM and $evm_tx EVM transactions${NC}" | tee -a "$LOG_FILE"
        
        # Add some variety to the logs
        if [ $((block_height % 5)) -eq 0 ]; then
            echo "[$timestamp] [DEBUG] Block $block_height details:" | tee -a "$LOG_FILE"
            echo "[$timestamp] [DEBUG]   Previous hash: block_hash_$((block_height-1))" | tee -a "$LOG_FILE"
            echo "[$timestamp] [DEBUG]   State root: state_root_$block_height" | tee -a "$LOG_FILE"
            echo "[$timestamp] [DEBUG]   Transactions root: tx_root_$block_height" | tee -a "$LOG_FILE"
            echo "[$timestamp] [DEBUG]   Proposer: mock_validator" | tee -a "$LOG_FILE"
        fi
        
        # Simulate consensus messages periodically
        if [ $((block_height % 10)) -eq 0 ]; then
            echo "[$timestamp] [DEBUG] Consensus: View $block_height, Phase: Commit" | tee -a "$LOG_FILE"
            echo "[$timestamp] [INFO] Health check: All systems operational" | tee -a "$LOG_FILE"
            echo "[$timestamp] [INFO] Memory usage: $((50 + RANDOM % 50))MB" | tee -a "$LOG_FILE"
            echo "[$timestamp] [INFO] CPU usage: $((5 + RANDOM % 15))%" | tee -a "$LOG_FILE"
        fi
        
        # Update totals
        total_svm_tx=$((total_svm_tx + svm_tx))
        total_evm_tx=$((total_evm_tx + evm_tx))
        
        # Show progress
        if [ $((block_height % 20)) -eq 0 ]; then
            echo "" | tee -a "$LOG_FILE"
            echo "[$timestamp] [INFO] Progress Report:" | tee -a "$LOG_FILE"
            echo "[$timestamp] [INFO]   Blocks generated: $block_height" | tee -a "$LOG_FILE"
            echo "[$timestamp] [INFO]   Total SVM transactions: $total_svm_tx" | tee -a "$LOG_FILE"
            echo "[$timestamp] [INFO]   Total EVM transactions: $total_evm_tx" | tee -a "$LOG_FILE"
            echo "[$timestamp] [INFO]   Block generation rate: 0.5 blocks/second" | tee -a "$LOG_FILE"
            echo "" | tee -a "$LOG_FILE"
        fi
        
        # Increment block height
        block_height=$((block_height + 1))
        
        # Wait for next block (simulate block interval)
        sleep $BLOCK_INTERVAL
    done
    
    # Final summary
    echo "" | tee -a "$LOG_FILE"
    echo "======================================" | tee -a "$LOG_FILE"
    echo "Simulation Complete!" | tee -a "$LOG_FILE"
    echo "Total blocks generated: $((block_height - 1))" | tee -a "$LOG_FILE"
    echo "Total SVM transactions: $total_svm_tx" | tee -a "$LOG_FILE"
    echo "Total EVM transactions: $total_evm_tx" | tee -a "$LOG_FILE"
    echo "======================================" | tee -a "$LOG_FILE"
}

# Main execution
main() {
    # Simulate startup
    simulate_startup
    
    # Simulate block generation
    simulate_block_generation
    
    echo ""
    echo -e "${GREEN}✓ Simulation completed successfully!${NC}"
    echo ""
    echo "Full log saved to: $LOG_FILE"
    echo ""
    echo "To view the log file:"
    echo "  cat $LOG_FILE"
    echo ""
    echo "To see only block generation logs:"
    echo "  grep 'Generated mock block' $LOG_FILE"
    echo ""
    echo "To see block generation statistics:"
    echo "  grep -E '(Progress Report|blocks generated)' $LOG_FILE"
}

# Run simulation
main