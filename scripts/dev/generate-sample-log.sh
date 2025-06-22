#!/bin/bash
# Generate a sample log file showing expected MultiVM output

LOG_FILE="multivm-sample-output.log"

cat > "$LOG_FILE" << 'EOF'
MultiVM Single Node Network Log
===============================
Started at: 2024-06-22 15:20:00

[2024-06-22 15:20:00] 🚀 Starting MultiVM Single Node...
[2024-06-22 15:20:00] Node ID: single-node
[2024-06-22 15:20:00] Block Generation: true
[2024-06-22 15:20:00] Block Interval: 2000ms
[2024-06-22 15:20:00] -----------------------------------
[2024-06-22 15:20:00] Starting block generation every 2 seconds...

[2024-06-22 15:20:01.123] [INFO multivm_cli] Starting MultiVM Node...
[2024-06-22 15:20:01.234] [INFO multivm_cli] Configuration file: /opt/multivm/config/multivm.toml
[2024-06-22 15:20:01.345] [INFO multivm_cli] Data directory: /opt/multivm/data
[2024-06-22 15:20:01.456] [INFO multivm_cli] Configuration loaded successfully
[2024-06-22 15:20:01.567] [INFO multivm_process_manager::coordinator] Initializing MultiVM Coordinator
[2024-06-22 15:20:01.678] [INFO multivm_common::ipc] Starting IPC server on /tmp/multivm/svm.sock
[2024-06-22 15:20:01.789] [INFO multivm_common::ipc] Starting IPC server on /tmp/multivm/evm.sock
[2024-06-22 15:20:01.890] [INFO solana_execution_engine] Starting Solana execution engine in mock mode
[2024-06-22 15:20:01.901] [INFO reth_execution_engine] Starting Reth execution engine in mock mode
[2024-06-22 15:20:02.012] [INFO multivm_consensus::malachite] Initializing Malachite consensus engine
[2024-06-22 15:20:02.123] [INFO multivm_consensus::malachite] Node ID: single-node, Role: Validator
[2024-06-22 15:20:02.234] [INFO multivm_p2p::network] Starting P2P network on 0.0.0.0:26656
[2024-06-22 15:20:02.345] [INFO multivm_p2p::discovery] No bootstrap nodes configured, running as bootstrap node
[2024-06-22 15:20:02.456] [INFO multivm_application] Starting API server on 0.0.0.0:8080
[2024-06-22 15:20:02.567] [INFO multivm_process_manager::coordinator] MultiVM Coordinator initialized
[2024-06-22 15:20:02.678] [INFO multivm_cli] MultiVM Coordinator started
[2024-06-22 15:20:02.789] [INFO multivm_process_manager::block_generator] Starting block generator with 2000 ms intervals
[2024-06-22 15:20:02.890] [INFO multivm_process_manager::block_generator] Block generator started successfully
[2024-06-22 15:20:02.901] [INFO multivm_cli] MultiVM Node is running with continuous block generation...
[2024-06-22 15:20:02.912] [INFO multivm_cli] Using data directory: /opt/multivm/data
[2024-06-22 15:20:02.923] [INFO multivm_cli] Generating blocks every 2 seconds

[2024-06-22 15:20:04.001] [INFO multivm_process_manager::block_generator] Generated mock block 1 with 3 SVM and 3 EVM transactions
[2024-06-22 15:20:04.012] [DEBUG multivm_process_manager::coordinator] Processing block at height 1
[2024-06-22 15:20:04.023] [DEBUG multivm_process_manager::block_router] Decomposing block with 6 total transactions
[2024-06-22 15:20:04.034] [DEBUG multivm_consensus] Block 1 committed to consensus
[2024-06-22 15:20:04.045] [INFO multivm_process_manager::coordinator] Block 1 processed successfully in 44ms (6 txns)

[2024-06-22 15:20:06.001] [INFO multivm_process_manager::block_generator] Generated mock block 2 with 3 SVM and 3 EVM transactions
[2024-06-22 15:20:06.012] [DEBUG multivm_process_manager::coordinator] Processing block at height 2
[2024-06-22 15:20:06.023] [DEBUG multivm_process_manager::block_router] Decomposing block with 6 total transactions
[2024-06-22 15:20:06.034] [DEBUG multivm_consensus] Block 2 committed to consensus
[2024-06-22 15:20:06.045] [INFO multivm_process_manager::coordinator] Block 2 processed successfully in 44ms (6 txns)

[2024-06-22 15:20:08.001] [INFO multivm_process_manager::block_generator] Generated mock block 3 with 3 SVM and 3 EVM transactions
[2024-06-22 15:20:08.012] [DEBUG multivm_process_manager::coordinator] Processing block at height 3
[2024-06-22 15:20:08.023] [DEBUG multivm_process_manager::block_router] Decomposing block with 6 total transactions
[2024-06-22 15:20:08.034] [DEBUG multivm_consensus] Block 3 committed to consensus
[2024-06-22 15:20:08.045] [INFO multivm_process_manager::coordinator] Block 3 processed successfully in 44ms (6 txns)

[2024-06-22 15:20:10.001] [INFO multivm_process_manager::block_generator] Generated mock block 4 with 3 SVM and 3 EVM transactions
[2024-06-22 15:20:10.012] [DEBUG multivm_process_manager::coordinator] Processing block at height 4
[2024-06-22 15:20:10.023] [DEBUG multivm_process_manager::block_router] Decomposing block with 6 total transactions
[2024-06-22 15:20:10.034] [DEBUG multivm_consensus] Block 4 committed to consensus
[2024-06-22 15:20:10.045] [INFO multivm_process_manager::coordinator] Block 4 processed successfully in 44ms (6 txns)

[2024-06-22 15:20:12.001] [INFO multivm_process_manager::block_generator] Generated mock block 5 with 3 SVM and 3 EVM transactions
[2024-06-22 15:20:12.012] [DEBUG multivm_process_manager::coordinator] Processing block at height 5
[2024-06-22 15:20:12.023] [DEBUG multivm_process_manager::block_router] Decomposing block with 6 total transactions
[2024-06-22 15:20:12.034] [DEBUG multivm_consensus] Block 5 committed to consensus
[2024-06-22 15:20:12.045] [INFO multivm_process_manager::coordinator] Block 5 processed successfully in 44ms (6 txns)
[2024-06-22 15:20:12.056] [INFO multivm_process_manager] Health check: All systems operational
[2024-06-22 15:20:12.067] [INFO multivm_common::monitoring] CPU usage: 8.5%
[2024-06-22 15:20:12.078] [INFO multivm_common::monitoring] Memory usage: 75MB

[2024-06-22 15:20:14.001] [INFO multivm_process_manager::block_generator] Generated mock block 6 with 3 SVM and 3 EVM transactions
[2024-06-22 15:20:16.001] [INFO multivm_process_manager::block_generator] Generated mock block 7 with 3 SVM and 3 EVM transactions
[2024-06-22 15:20:18.001] [INFO multivm_process_manager::block_generator] Generated mock block 8 with 3 SVM and 3 EVM transactions
[2024-06-22 15:20:20.001] [INFO multivm_process_manager::block_generator] Generated mock block 9 with 3 SVM and 3 EVM transactions
[2024-06-22 15:20:22.001] [INFO multivm_process_manager::block_generator] Generated mock block 10 with 3 SVM and 3 EVM transactions

[2024-06-22 15:20:22.100] [INFO multivm_consensus] Consensus status: Height=10, View=10, State=NORMAL
[2024-06-22 15:20:22.111] [INFO multivm_process_manager] Progress Report:
[2024-06-22 15:20:22.122] [INFO multivm_process_manager]   Blocks generated: 10
[2024-06-22 15:20:22.133] [INFO multivm_process_manager]   Total SVM transactions: 30
[2024-06-22 15:20:22.144] [INFO multivm_process_manager]   Total EVM transactions: 30
[2024-06-22 15:20:22.155] [INFO multivm_process_manager]   Block generation rate: 0.5 blocks/second
[2024-06-22 15:20:22.166] [INFO multivm_process_manager]   Average block processing time: 44ms

[continues with similar pattern...]
EOF

echo "Sample log file generated: $LOG_FILE"
echo ""
echo "This shows what the actual MultiVM single node output would look like with:"
echo "- Continuous block generation every 2 seconds"
echo "- 3 SVM and 3 EVM transactions per block"
echo "- Coordinator processing and consensus messages"
echo "- Health monitoring and resource usage"
echo ""
echo "To view the sample log:"
echo "  cat $LOG_FILE"
echo ""
echo "To see only block generation:"
echo "  grep 'Generated mock block' $LOG_FILE"