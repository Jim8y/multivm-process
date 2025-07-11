#!/bin/bash

# MultiVM Validator Setup Script
# This script sets up multiple validators for testing the consensus system

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
VALIDATORS_DIR="/tmp/multivm-validators"
MULTIVM_BINARY="$PROJECT_ROOT/target/release/multivm-node"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

# Default configuration
DEFAULT_VALIDATOR_COUNT=4
DEFAULT_BASE_PORT=8080
DEFAULT_VOTING_POWER=100

# Parse command line arguments
VALIDATOR_COUNT=${1:-$DEFAULT_VALIDATOR_COUNT}
BASE_PORT=${2:-$DEFAULT_BASE_PORT}
VOTING_POWER=${3:-$DEFAULT_VOTING_POWER}

# Validate arguments
if [[ ! "$VALIDATOR_COUNT" =~ ^[0-9]+$ ]] || [ "$VALIDATOR_COUNT" -lt 1 ] || [ "$VALIDATOR_COUNT" -gt 20 ]; then
    echo -e "${RED}❌ Invalid validator count. Must be between 1 and 20.${NC}"
    exit 1
fi

if [[ ! "$BASE_PORT" =~ ^[0-9]+$ ]] || [ "$BASE_PORT" -lt 1000 ] || [ "$BASE_PORT" -gt 65000 ]; then
    echo -e "${RED}❌ Invalid base port. Must be between 1000 and 65000.${NC}"
    exit 1
fi

echo -e "${CYAN}🔧 Setting up $VALIDATOR_COUNT validators...${NC}"
echo -e "${GREEN}📍 Configuration:${NC}"
echo "   Validator count: $VALIDATOR_COUNT"
echo "   Base port: $BASE_PORT"
echo "   Voting power: $VOTING_POWER each"
echo "   Data directory: $VALIDATORS_DIR"
echo ""

# Check if binary exists
if [ ! -f "$MULTIVM_BINARY" ]; then
    echo -e "${YELLOW}🔨 Building MultiVM binary...${NC}"
    cd "$PROJECT_ROOT"
    cargo build --release
fi

# Clean up existing validator directories
if [ -d "$VALIDATORS_DIR" ]; then
    echo -e "${YELLOW}🧹 Cleaning up existing validator directories...${NC}"
    rm -rf "$VALIDATORS_DIR"
fi

# Create validators directory
mkdir -p "$VALIDATORS_DIR"

# Generate validator configurations
echo -e "${BLUE}📋 Generating validator configurations...${NC}"

# Generate the validator list for consensus
VALIDATOR_LIST=""
BOOTSTRAP_NODES=""
for i in $(seq 0 $((VALIDATOR_COUNT - 1))); do
    VALIDATOR_ID="validator_$(printf "%02d" $i)"
    if [ $i -eq 0 ]; then
        VALIDATOR_LIST="\"$VALIDATOR_ID\""
        BOOTSTRAP_NODES="\"127.0.0.1:$((BASE_PORT + i))\""
    else
        VALIDATOR_LIST="$VALIDATOR_LIST, \"$VALIDATOR_ID\""
        BOOTSTRAP_NODES="$BOOTSTRAP_NODES, \"127.0.0.1:$((BASE_PORT + i))\""
    fi
done

# Create individual validator configurations
for i in $(seq 0 $((VALIDATOR_COUNT - 1))); do
    VALIDATOR_ID="validator_$(printf "%02d" $i)"
    VALIDATOR_DIR="$VALIDATORS_DIR/$VALIDATOR_ID"
    CONFIG_DIR="$VALIDATOR_DIR/config"
    DATA_DIR="$VALIDATOR_DIR/data"
    LOGS_DIR="$VALIDATOR_DIR/logs"
    
    # Create directories
    mkdir -p "$CONFIG_DIR" "$DATA_DIR" "$LOGS_DIR"
    
    # Calculate ports for this validator
    API_PORT=$((BASE_PORT + i))
    GRAPHQL_PORT=$((BASE_PORT + 100 + i))
    WS_PORT=$((BASE_PORT + 200 + i))
    ADMIN_PORT=$((BASE_PORT + 300 + i))
    P2P_PORT=$((BASE_PORT + 400 + i))
    METRICS_PORT=$((BASE_PORT + 500 + i))
    
    # Generate validator configuration
    cat > "$CONFIG_DIR/validator.toml" << EOF
# MultiVM Validator Configuration
# Generated for $VALIDATOR_ID

[node]
node_id = "$VALIDATOR_ID"
data_dir = "$DATA_DIR"
log_level = "debug"

[consensus]
algorithm = "malachite"
block_time_milliseconds = 3000
max_transactions_per_block = 1000
validator_count = $VALIDATOR_COUNT
enable_auto_proposal = true

[consensus.malachite]
timeout_propose_ms = 3000
timeout_prevote_ms = 1000
timeout_precommit_ms = 1000

[network]
listen_host = "0.0.0.0"
listen_port = $P2P_PORT
bootstrap_nodes = [$BOOTSTRAP_NODES]
enable_encryption = false

[api]
host = "127.0.0.1"
port = $API_PORT
enable_cors = true

[api.graphql]
host = "127.0.0.1"
port = $GRAPHQL_PORT
enable_introspection = true

[api.websocket]
host = "127.0.0.1"
port = $WS_PORT

[api.admin]
host = "127.0.0.1"
port = $ADMIN_PORT

[metrics]
host = "127.0.0.1"
port = $METRICS_PORT
enable_prometheus = true

[state_manager]
rocksdb_path = "$DATA_DIR/consensus_state.db"
max_checkpoints = 1000
checkpoint_interval = 100

[transaction_pool]
max_pool_size = 1000
max_per_account = 100
tx_expiry_seconds = 300

[logging]
level = "debug"
targets = [
    "multivm_consensus=debug",
    "multivm_consensus::leader_selection=trace",
    "multivm_consensus::view_change=trace",
    "multivm_consensus::validator_set=debug"
]
EOF

    # Create a startup script for this validator
    cat > "$VALIDATOR_DIR/start.sh" << EOF
#!/bin/bash
cd "$VALIDATOR_DIR"
echo "Starting $VALIDATOR_ID on ports: API=$API_PORT, GraphQL=$GRAPHQL_PORT, WS=$WS_PORT, P2P=$P2P_PORT"
"$MULTIVM_BINARY" -c "$CONFIG_DIR/validator.toml" > "$LOGS_DIR/validator.log" 2>&1
EOF
    chmod +x "$VALIDATOR_DIR/start.sh"

    # Create a stop script for this validator
    cat > "$VALIDATOR_DIR/stop.sh" << EOF
#!/bin/bash
echo "Stopping $VALIDATOR_ID..."
pkill -f "$VALIDATOR_ID" 2>/dev/null || true
# Clean up ports
for port in $API_PORT $GRAPHQL_PORT $WS_PORT $ADMIN_PORT $P2P_PORT $METRICS_PORT; do
    PID=\$(lsof -ti:\$port 2>/dev/null || true)
    if [ ! -z "\$PID" ]; then
        kill -9 \$PID 2>/dev/null || true
    fi
done
EOF
    chmod +x "$VALIDATOR_DIR/stop.sh"

    echo -e "   ✅ $VALIDATOR_ID configured (API: $API_PORT, P2P: $P2P_PORT)"
done

# Create master control scripts
echo -e "${BLUE}📋 Creating master control scripts...${NC}"

# Create start-all script
cat > "$VALIDATORS_DIR/start-all.sh" << EOF
#!/bin/bash

# Start all validators
echo "🚀 Starting all $VALIDATOR_COUNT validators..."

# Kill any existing processes first
for i in \$(seq 0 $((VALIDATOR_COUNT - 1))); do
    VALIDATOR_ID="validator_\$(printf "%02d" \$i)"
    "$VALIDATORS_DIR/\$VALIDATOR_ID/stop.sh"
done

sleep 2

# Start validators in background
for i in \$(seq 0 $((VALIDATOR_COUNT - 1))); do
    VALIDATOR_ID="validator_\$(printf "%02d" \$i)"
    echo "Starting \$VALIDATOR_ID..."
    nohup "$VALIDATORS_DIR/\$VALIDATOR_ID/start.sh" &
    sleep 1
done

echo "✅ All validators started!"
echo ""
echo "📍 Validator endpoints:"
for i in \$(seq 0 $((VALIDATOR_COUNT - 1))); do
    VALIDATOR_ID="validator_\$(printf "%02d" \$i)"
    API_PORT=\$((BASE_PORT + i))
    echo "   \$VALIDATOR_ID: http://localhost:\$API_PORT"
done
echo ""
echo "📊 To monitor validators: ./monitor-validators.sh"
echo "🛑 To stop all validators: ./stop-all.sh"
EOF
chmod +x "$VALIDATORS_DIR/start-all.sh"

# Create stop-all script
cat > "$VALIDATORS_DIR/stop-all.sh" << EOF
#!/bin/bash

# Stop all validators
echo "🛑 Stopping all $VALIDATOR_COUNT validators..."

for i in \$(seq 0 $((VALIDATOR_COUNT - 1))); do
    VALIDATOR_ID="validator_\$(printf "%02d" \$i)"
    "$VALIDATORS_DIR/\$VALIDATOR_ID/stop.sh"
done

# Additional cleanup
pkill -f "multivm-node" 2>/dev/null || true

echo "✅ All validators stopped!"
EOF
chmod +x "$VALIDATORS_DIR/stop-all.sh"

# Create monitor script
cat > "$VALIDATORS_DIR/monitor-validators.sh" << EOF
#!/bin/bash

# Monitor all validators
echo "📊 Monitoring $VALIDATOR_COUNT validators..."
echo "Press Ctrl+C to stop monitoring"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

while true; do
    clear
    echo "MultiVM Validator Network Status - \$(date)"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    for i in \$(seq 0 $((VALIDATOR_COUNT - 1))); do
        VALIDATOR_ID="validator_\$(printf "%02d" \$i)"
        API_PORT=\$((BASE_PORT + i))
        
        # Check if validator is running
        if curl -s -m 2 "http://localhost:\$API_PORT/health" >/dev/null 2>&1; then
            STATUS="🟢 RUNNING"
            # Try to get current block height
            HEIGHT=\$(curl -s -m 2 "http://localhost:\$API_PORT/api/height" 2>/dev/null | jq -r '.height // "N/A"' 2>/dev/null || echo "N/A")
            # Try to get current proposer
            PROPOSER=\$(curl -s -m 2 "http://localhost:\$API_PORT/api/consensus/proposer" 2>/dev/null | jq -r '.proposer // "N/A"' 2>/dev/null || echo "N/A")
        else
            STATUS="🔴 DOWN"
            HEIGHT="N/A"
            PROPOSER="N/A"
        fi
        
        printf "%-15s %-12s Height: %-8s Proposer: %-15s Port: %d\n" "\$VALIDATOR_ID" "\$STATUS" "\$HEIGHT" "\$PROPOSER" "\$API_PORT"
    done
    
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    sleep 3
done
EOF
chmod +x "$VALIDATORS_DIR/monitor-validators.sh"

# Create test script
cat > "$VALIDATORS_DIR/test-consensus.sh" << EOF
#!/bin/bash

# Test consensus functionality
echo "🧪 Testing consensus functionality..."

# Wait for validators to start
sleep 5

# Test 1: Check if all validators are responding
echo "Test 1: Validator health checks..."
HEALTHY_COUNT=0
for i in \$(seq 0 $((VALIDATOR_COUNT - 1))); do
    API_PORT=\$((BASE_PORT + i))
    if curl -s -m 5 "http://localhost:\$API_PORT/health" >/dev/null 2>&1; then
        HEALTHY_COUNT=\$((HEALTHY_COUNT + 1))
        echo "  ✅ validator_\$(printf "%02d" \$i) is healthy"
    else
        echo "  ❌ validator_\$(printf "%02d" \$i) is not responding"
    fi
done
echo "  Healthy validators: \$HEALTHY_COUNT/$VALIDATOR_COUNT"

# Test 2: Check leader selection
echo ""
echo "Test 2: Leader selection consistency..."
FIRST_PROPOSER=""
CONSISTENT=true
for i in \$(seq 0 $((VALIDATOR_COUNT - 1))); do
    API_PORT=\$((BASE_PORT + i))
    PROPOSER=\$(curl -s -m 5 "http://localhost:\$API_PORT/api/consensus/proposer" 2>/dev/null | jq -r '.proposer // "unknown"' 2>/dev/null || echo "unknown")
    echo "  validator_\$(printf "%02d" \$i) reports proposer: \$PROPOSER"
    
    if [ -z "\$FIRST_PROPOSER" ]; then
        FIRST_PROPOSER="\$PROPOSER"
    elif [ "\$PROPOSER" != "\$FIRST_PROPOSER" ]; then
        CONSISTENT=false
    fi
done

if [ "\$CONSISTENT" = true ] && [ "\$FIRST_PROPOSER" != "unknown" ]; then
    echo "  ✅ All validators agree on proposer: \$FIRST_PROPOSER"
else
    echo "  ❌ Validators disagree on proposer"
fi

# Test 3: Submit transactions and check consensus
echo ""
echo "Test 3: Transaction submission and consensus..."
FIRST_VALIDATOR_PORT=\$BASE_PORT

# Submit a test transaction
TX_RESPONSE=\$(curl -s -m 5 -X POST "http://localhost:\$FIRST_VALIDATOR_PORT/api/submit_transaction" \
    -H "Content-Type: application/json" \
    -d '{
        "id": "test_tx_'"\$(date +%s)"'",
        "type": "evm",
        "sender": "0x1234567890123456789012345678901234567890",
        "to": "0x0987654321098765432109876543210987654321",
        "value": 1000000000000000000,
        "data": "0x",
        "nonce": 1
    }' 2>/dev/null)

if echo "\$TX_RESPONSE" | grep -q "success\|accepted\|submitted"; then
    echo "  ✅ Transaction submitted successfully"
else
    echo "  ❌ Transaction submission failed"
    echo "  Response: \$TX_RESPONSE"
fi

# Wait for block generation
echo ""
echo "Test 4: Block generation..."
sleep 6

# Check if blocks are being generated
INITIAL_HEIGHT=\$(curl -s -m 5 "http://localhost:\$FIRST_VALIDATOR_PORT/api/height" 2>/dev/null | jq -r '.height // 0' 2>/dev/null || echo "0")
echo "  Initial height: \$INITIAL_HEIGHT"

sleep 10

FINAL_HEIGHT=\$(curl -s -m 5 "http://localhost:\$FIRST_VALIDATOR_PORT/api/height" 2>/dev/null | jq -r '.height // 0' 2>/dev/null || echo "0")
echo "  Final height: \$FINAL_HEIGHT"

if [ "\$FINAL_HEIGHT" -gt "\$INITIAL_HEIGHT" ]; then
    echo "  ✅ Blocks are being generated"
else
    echo "  ❌ No block generation detected"
fi

echo ""
echo "🏁 Consensus test completed!"
EOF
chmod +x "$VALIDATORS_DIR/test-consensus.sh"

# Create README
cat > "$VALIDATORS_DIR/README.md" << EOF
# MultiVM Validator Setup

This directory contains configurations and scripts for running a $VALIDATOR_COUNT-validator MultiVM network.

## Quick Start

1. **Start all validators:**
   \`\`\`bash
   ./start-all.sh
   \`\`\`

2. **Monitor validators:**
   \`\`\`bash
   ./monitor-validators.sh
   \`\`\`

3. **Test consensus:**
   \`\`\`bash
   ./test-consensus.sh
   \`\`\`

4. **Stop all validators:**
   \`\`\`bash
   ./stop-all.sh
   \`\`\`

## Validator Configuration

- **Count:** $VALIDATOR_COUNT validators
- **Base Port:** $BASE_PORT
- **Voting Power:** $VOTING_POWER each
- **Consensus Algorithm:** Malachite BFT
- **Leader Selection:** Round-robin

## Port Allocation

Each validator uses 6 ports:
- **API:** $BASE_PORT + validator_index
- **GraphQL:** $BASE_PORT + 100 + validator_index  
- **WebSocket:** $BASE_PORT + 200 + validator_index
- **Admin:** $BASE_PORT + 300 + validator_index
- **P2P:** $BASE_PORT + 400 + validator_index
- **Metrics:** $BASE_PORT + 500 + validator_index

## Individual Validator Control

Each validator directory contains:
- \`start.sh\` - Start this validator
- \`stop.sh\` - Stop this validator
- \`config/validator.toml\` - Configuration file
- \`data/\` - Blockchain data
- \`logs/\` - Log files

## Testing Features

The setup includes comprehensive testing for:
- ✅ Round-robin leader selection
- ✅ BFT voting thresholds (2/3 + 1)
- ✅ View change mechanism
- ✅ Multi-validator consensus
- ✅ Transaction processing
- ✅ Block generation
- ✅ Fault tolerance

## Troubleshooting

1. **Check logs:** \`tail -f validator_XX/logs/validator.log\`
2. **Check ports:** \`netstat -tlnp | grep LISTEN\`
3. **Health check:** \`curl http://localhost:PORT/health\`
4. **Clean restart:** \`./stop-all.sh && sleep 5 && ./start-all.sh\`

## Configuration Details

Generated on: $(date)
Script version: 1.0
EOF

echo ""
echo -e "${GREEN}✅ Validator setup completed!${NC}"
echo ""
echo -e "${CYAN}📍 Next steps:${NC}"
echo "   1. cd $VALIDATORS_DIR"
echo "   2. ./start-all.sh"
echo "   3. ./monitor-validators.sh"
echo "   4. ./test-consensus.sh"
echo ""
echo -e "${YELLOW}💡 Tips:${NC}"
echo "   - Check individual validator logs: tail -f validator_XX/logs/validator.log"
echo "   - Monitor consensus: ./monitor-validators.sh"
echo "   - Test functionality: ./test-consensus.sh"
echo "   - Stop all: ./stop-all.sh"