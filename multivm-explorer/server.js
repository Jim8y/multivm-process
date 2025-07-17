const express = require('express');
const cors = require('cors');
const axios = require('axios');
const cron = require('node-cron');
const { Server } = require('socket.io');
const http = require('http');
const path = require('path');

const app = express();
const server = http.createServer(app);
const io = new Server(server, {
  cors: {
    origin: "*",
    methods: ["GET", "POST"]
  }
});

// Middleware
app.use(cors());
app.use(express.json());
app.use(express.static(path.join(__dirname, 'public')));

// MultiVM node configurations - Use Docker hostnames when running in container
const MULTIVM_NODES = [
  { 
    id: 1, 
    name: 'Node 1', 
    multivm: 'http://multivm-node1:8080', 
    reth: 'http://multivm-node1:8545', 
    rethEngine: 'http://multivm-node1:8551',
    solana: 'http://multivm-node1:8899'  // Solana RPC endpoint
  },
  { 
    id: 2, 
    name: 'Node 2', 
    multivm: 'http://multivm-node2:8080', 
    reth: 'http://multivm-node2:8545', 
    rethEngine: 'http://multivm-node2:8551',
    solana: 'http://multivm-node2:8899'
  },
  { 
    id: 3, 
    name: 'Node 3', 
    multivm: 'http://multivm-node3:8080', 
    reth: 'http://multivm-node3:8545', 
    rethEngine: 'http://multivm-node3:8551',
    solana: 'http://multivm-node3:8899'
  },
  { 
    id: 4, 
    name: 'Node 4', 
    multivm: 'http://multivm-node4:8080', 
    reth: 'http://multivm-node4:8545', 
    rethEngine: 'http://multivm-node4:8551',
    solana: 'http://multivm-node4:8899'
  },
  { 
    id: 5, 
    name: 'Node 5', 
    multivm: 'http://multivm-node5:8080', 
    reth: 'http://multivm-node5:8545', 
    rethEngine: 'http://multivm-node5:8551',
    solana: 'http://multivm-node5:8899'
  },
  { 
    id: 6, 
    name: 'Node 6', 
    multivm: 'http://multivm-node6:8080', 
    reth: 'http://multivm-node6:8545', 
    rethEngine: 'http://multivm-node6:8551',
    solana: 'http://multivm-node6:8899'
  },
  { 
    id: 7, 
    name: 'Node 7', 
    multivm: 'http://multivm-node7:8080', 
    reth: 'http://multivm-node7:8545', 
    rethEngine: 'http://multivm-node7:8551',
    solana: 'http://multivm-node7:8899'
  }
];

// Global state
let networkStatus = {
  lastUpdate: new Date(),
  totalNodes: MULTIVM_NODES.length,
  healthyNodes: 0,
  latestBlock: 0,
  totalTransactions: 0,
  chainId: '1337',
  consensus: 'Malachite BFT',
  nodes: []
};

// Utility functions
async function makeRequest(url, timeout = 5000) {
  try {
    const response = await axios.get(url, { timeout });
    return { success: true, data: response.data };
  } catch (error) {
    return { success: false, error: error.message };
  }
}

async function makeRpcRequest(url, method, params = [], timeout = 5000) {
  try {
    const response = await axios.post(url, {
      jsonrpc: '2.0',
      method: method,
      params: params,
      id: 1
    }, { timeout });
    return { success: true, data: response.data.result };
  } catch (error) {
    return { success: false, error: error.message };
  }
}

// Check MultiVM node health
async function checkMultiVMHealth(node) {
  const result = await makeRequest(`${node.multivm}/health`);
  if (result.success && result.data?.data?.status === 'healthy') {
    return {
      healthy: true,
      version: result.data.data.version,
      timestamp: result.data.data.timestamp
    };
  }
  return { healthy: false, error: result.error };
}

// Check Reth node status
async function checkRethStatus(node) {
  const chainIdResult = await makeRpcRequest(node.reth, 'eth_chainId');
  const blockNumberResult = await makeRpcRequest(node.reth, 'eth_blockNumber');
  const syncingResult = await makeRpcRequest(node.reth, 'eth_syncing');
  
  if (chainIdResult.success && blockNumberResult.success) {
    return {
      healthy: true,
      chainId: parseInt(chainIdResult.data, 16),
      blockNumber: parseInt(blockNumberResult.data, 16),
      syncing: syncingResult.success ? syncingResult.data : false
    };
  }
  
  return { healthy: false };
}

// Check Solana node status
async function checkSolanaStatus(node) {
  const slotResult = await makeRpcRequest(node.solana, 'getSlot');
  const versionResult = await makeRpcRequest(node.solana, 'getVersion');
  const healthResult = await makeRpcRequest(node.solana, 'getHealth');
  
  if (slotResult.success) {
    return {
      healthy: true,
      slot: slotResult.data,
      version: versionResult.success ? versionResult.data : null,
      health: healthResult.success ? healthResult.data : 'unknown'
    };
  }
  
  return { healthy: false };
}

// Get latest block information (Ethereum)
async function getLatestBlock(node) {
  const blockResult = await makeRpcRequest(node.reth, 'eth_getBlockByNumber', ['latest', true]);
  if (blockResult.success && blockResult.data) {
    const block = blockResult.data;
    return {
      vm_type: 'ethereum',
      number: parseInt(block.number, 16),
      hash: block.hash,
      timestamp: parseInt(block.timestamp, 16),
      transactionCount: block.transactions ? block.transactions.length : 0,
      gasUsed: parseInt(block.gasUsed || '0x0', 16),
      gasLimit: parseInt(block.gasLimit || '0x0', 16),
      miner: block.miner,
      parentHash: block.parentHash,
      size: parseInt(block.size || '0x0', 16),
      transactions: block.transactions || []
    };
  }
  return null;
}

// Get latest Solana block information
async function getLatestSolanaBlock(node) {
  const slotResult = await makeRpcRequest(node.solana, 'getSlot');
  if (slotResult.success && slotResult.data) {
    const blockResult = await makeRpcRequest(node.solana, 'getBlock', [
      slotResult.data,
      { encoding: 'json', transactionDetails: 'full', rewards: false }
    ]);
    
    if (blockResult.success && blockResult.data) {
      const block = blockResult.data;
      return {
        vm_type: 'solana',
        slot: slotResult.data,
        blockhash: block.blockhash,
        previousBlockhash: block.previousBlockhash,
        parentSlot: block.parentSlot,
        blockTime: block.blockTime,
        transactionCount: block.transactions ? block.transactions.length : 0,
        transactions: block.transactions || []
      };
    }
  }
  return null;
}

// Get block by number
async function getBlockByNumber(nodeId, blockNumber) {
  const node = MULTIVM_NODES.find(n => n.id == nodeId);
  if (!node) return null;
  
  const hex = '0x' + blockNumber.toString(16);
  const blockResult = await makeRpcRequest(node.reth, 'eth_getBlockByNumber', [hex, true]);
  
  if (blockResult.success && blockResult.data) {
    const block = blockResult.data;
    return {
      number: parseInt(block.number, 16),
      hash: block.hash,
      timestamp: parseInt(block.timestamp, 16),
      transactionCount: block.transactions ? block.transactions.length : 0,
      gasUsed: parseInt(block.gasUsed || '0x0', 16),
      gasLimit: parseInt(block.gasLimit || '0x0', 16),
      miner: block.miner,
      parentHash: block.parentHash,
      size: parseInt(block.size || '0x0', 16),
      transactions: block.transactions || [],
      nonce: block.nonce,
      difficulty: block.difficulty,
      extraData: block.extraData
    };
  }
  return null;
}

// Get transaction by hash (Ethereum)
async function getTransactionByHash(nodeId, txHash) {
  const node = MULTIVM_NODES.find(n => n.id == nodeId);
  if (!node) return null;
  
  const txResult = await makeRpcRequest(node.reth, 'eth_getTransactionByHash', [txHash]);
  const receiptResult = await makeRpcRequest(node.reth, 'eth_getTransactionReceipt', [txHash]);
  
  if (txResult.success && txResult.data) {
    const tx = txResult.data;
    const receipt = receiptResult.success ? receiptResult.data : null;
    
    return {
      vm_type: 'ethereum',
      hash: tx.hash,
      blockNumber: parseInt(tx.blockNumber || '0x0', 16),
      blockHash: tx.blockHash,
      transactionIndex: parseInt(tx.transactionIndex || '0x0', 16),
      from: tx.from,
      to: tx.to,
      value: tx.value,
      gas: parseInt(tx.gas || '0x0', 16),
      gasPrice: tx.gasPrice,
      input: tx.input,
      nonce: parseInt(tx.nonce || '0x0', 16),
      status: receipt ? parseInt(receipt.status || '0x0', 16) : null,
      gasUsed: receipt ? parseInt(receipt.gasUsed || '0x0', 16) : null,
      logs: receipt ? receipt.logs : []
    };
  }
  return null;
}

// Get Solana transaction by signature
async function getSolanaTransactionBySignature(nodeId, signature) {
  const node = MULTIVM_NODES.find(n => n.id == nodeId);
  if (!node) return null;
  
  const txResult = await makeRpcRequest(node.solana, 'getTransaction', [
    signature,
    { encoding: 'json', commitment: 'confirmed' }
  ]);
  
  if (txResult.success && txResult.data) {
    const tx = txResult.data;
    
    return {
      vm_type: 'solana',
      signature: signature,
      slot: tx.slot,
      blockTime: tx.blockTime,
      confirmations: tx.confirmations || 0,
      err: tx.meta?.err || null,
      fee: tx.meta?.fee || 0,
      preBalances: tx.meta?.preBalances || [],
      postBalances: tx.meta?.postBalances || [],
      logMessages: tx.meta?.logMessages || [],
      accounts: tx.transaction?.message?.accountKeys || [],
      instructions: tx.transaction?.message?.instructions || []
    };
  }
  return null;
}

// Update network status
async function updateNetworkStatus() {
  console.log('🔄 Updating network status...');
  
  const nodeStatuses = await Promise.all(
    MULTIVM_NODES.map(async (node) => {
      const multivmHealth = await checkMultiVMHealth(node);
      const rethStatus = await checkRethStatus(node);
      const solanaStatus = await checkSolanaStatus(node);
      const latestEthBlock = rethStatus.healthy ? await getLatestBlock(node) : null;
      const latestSolanaBlock = solanaStatus.healthy ? await getLatestSolanaBlock(node) : null;
      
      return {
        id: node.id,
        name: node.name,
        multivm: {
          healthy: multivmHealth.healthy,
          version: multivmHealth.version,
          error: multivmHealth.error
        },
        ethereum: {
          healthy: rethStatus.healthy,
          chainId: rethStatus.chainId,
          blockNumber: rethStatus.blockNumber,
          syncing: rethStatus.syncing,
          latestBlock: latestEthBlock
        },
        solana: {
          healthy: solanaStatus.healthy,
          slot: solanaStatus.slot,
          version: solanaStatus.version,
          health: solanaStatus.health,
          latestBlock: latestSolanaBlock
        },
        lastChecked: new Date()
      };
    })
  );
  
  const healthyNodes = nodeStatuses.filter(node => 
    node.multivm.healthy && (node.ethereum.healthy || node.solana.healthy)
  ).length;
  
  // Get latest block numbers from both Ethereum and Solana
  const latestEthBlockNumber = Math.max(
    0,
    ...nodeStatuses
      .filter(node => node.ethereum.latestBlock)
      .map(node => node.ethereum.latestBlock.number)
  );
  
  const latestSolanaSlot = Math.max(
    0,
    ...nodeStatuses
      .filter(node => node.solana.latestBlock)
      .map(node => node.solana.latestBlock.slot)
  );
  
  // Calculate total transactions across all blocks
  let totalTransactions = 0;
  for (const node of nodeStatuses) {
    // Ethereum transactions
    if (node.ethereum.healthy && node.ethereum.blockNumber) {
      try {
        totalTransactions += node.ethereum.blockNumber * (node.ethereum.latestBlock?.transactionCount || 0);
      } catch (error) {
        console.error(`Error calculating Ethereum transactions for node ${node.id}:`, error);
      }
    }
    
    // Solana transactions
    if (node.solana.healthy && node.solana.slot) {
      try {
        totalTransactions += node.solana.slot * (node.solana.latestBlock?.transactionCount || 0);
      } catch (error) {
        console.error(`Error calculating Solana transactions for node ${node.id}:`, error);
      }
    }
  }
  
  networkStatus = {
    lastUpdate: new Date(),
    totalNodes: MULTIVM_NODES.length,
    healthyNodes: healthyNodes,
    ethereum: {
      latestBlock: latestEthBlockNumber,
      chainId: '1337'
    },
    solana: {
      latestSlot: latestSolanaSlot
    },
    totalTransactions: totalTransactions,
    consensus: 'Malachite BFT',
    nodes: nodeStatuses
  };
  
  // Emit to connected clients
  io.emit('networkStatus', networkStatus);
  
  console.log(`✅ Status updated: ${healthyNodes}/${MULTIVM_NODES.length} nodes healthy, ETH block: ${latestEthBlockNumber}, SOL slot: ${latestSolanaSlot}`);
}

// API Routes

// Network status
app.get('/api/status', (req, res) => {
  res.json(networkStatus);
});

// Node details
app.get('/api/nodes', (req, res) => {
  res.json(networkStatus.nodes);
});

app.get('/api/nodes/:nodeId', (req, res) => {
  const node = networkStatus.nodes.find(n => n.id == req.params.nodeId);
  if (!node) {
    return res.status(404).json({ error: 'Node not found' });
  }
  res.json(node);
});

// Block endpoints
app.get('/api/blocks/latest', async (req, res) => {
  const nodeId = req.query.node || 1;
  const node = MULTIVM_NODES.find(n => n.id == nodeId);
  if (!node) {
    return res.status(404).json({ error: 'Node not found' });
  }
  
  const block = await getLatestBlock(node);
  if (!block) {
    return res.status(500).json({ error: 'Failed to fetch latest block' });
  }
  
  res.json(block);
});

app.get('/api/blocks/:blockNumber', async (req, res) => {
  const nodeId = req.query.node || 1;
  const blockNumber = parseInt(req.params.blockNumber);
  
  if (isNaN(blockNumber)) {
    return res.status(400).json({ error: 'Invalid block number' });
  }
  
  const block = await getBlockByNumber(nodeId, blockNumber);
  if (!block) {
    return res.status(404).json({ error: 'Block not found' });
  }
  
  res.json(block);
});

// Get recent blocks
app.get('/api/blocks', async (req, res) => {
  const nodeId = req.query.node || 1;
  const limit = Math.min(parseInt(req.query.limit) || 10, 50);
  
  const node = MULTIVM_NODES.find(n => n.id == nodeId);
  if (!node) {
    return res.status(404).json({ error: 'Node not found' });
  }
  
  const latestBlock = await getLatestBlock(node);
  if (!latestBlock) {
    return res.status(500).json({ error: 'Failed to fetch latest block' });
  }
  
  const blocks = [];
  const startBlock = Math.max(0, latestBlock.number - limit + 1);
  
  for (let i = latestBlock.number; i >= startBlock && blocks.length < limit; i--) {
    const block = await getBlockByNumber(nodeId, i);
    if (block) {
      blocks.push(block);
    }
  }
  
  res.json(blocks);
});

// Transaction endpoints
app.get('/api/transactions/:txHash', async (req, res) => {
  const nodeId = req.query.node || 1;
  const txHash = req.params.txHash;
  const vmType = req.query.vm || 'ethereum'; // Default to Ethereum
  
  let transaction = null;
  
  if (vmType === 'solana') {
    // Solana transaction signature format
    transaction = await getSolanaTransactionBySignature(nodeId, txHash);
  } else {
    // Ethereum transaction hash format
    if (!/^0x[a-fA-F0-9]{64}$/.test(txHash)) {
      return res.status(400).json({ error: 'Invalid Ethereum transaction hash' });
    }
    transaction = await getTransactionByHash(nodeId, txHash);
  }
  
  if (!transaction) {
    return res.status(404).json({ error: 'Transaction not found' });
  }
  
  res.json(transaction);
});

// VM-specific endpoints
app.get('/api/ethereum/blocks/latest', async (req, res) => {
  const nodeId = req.query.node || 1;
  const node = MULTIVM_NODES.find(n => n.id == nodeId);
  if (!node) {
    return res.status(404).json({ error: 'Node not found' });
  }
  
  const block = await getLatestBlock(node);
  if (!block) {
    return res.status(500).json({ error: 'Failed to fetch latest Ethereum block' });
  }
  
  res.json(block);
});

app.get('/api/solana/blocks/latest', async (req, res) => {
  const nodeId = req.query.node || 1;
  const node = MULTIVM_NODES.find(n => n.id == nodeId);
  if (!node) {
    return res.status(404).json({ error: 'Node not found' });
  }
  
  const block = await getLatestSolanaBlock(node);
  if (!block) {
    return res.status(500).json({ error: 'Failed to fetch latest Solana block' });
  }
  
  res.json(block);
});

// Get VM health status
app.get('/api/vm-status/:nodeId', async (req, res) => {
  const nodeId = parseInt(req.params.nodeId);
  const node = MULTIVM_NODES.find(n => n.id === nodeId);
  
  if (!node) {
    return res.status(404).json({ error: 'Node not found' });
  }
  
  const multivmHealth = await checkMultiVMHealth(node);
  const ethereumHealth = await checkRethStatus(node);
  const solanaHealth = await checkSolanaStatus(node);
  
  res.json({
    nodeId: nodeId,
    nodeName: node.name,
    multivm: multivmHealth,
    ethereum: ethereumHealth,
    solana: solanaHealth,
    timestamp: new Date()
  });
});

// Search endpoint
app.get('/api/search/:query', async (req, res) => {
  const query = req.params.query;
  const nodeId = req.query.node || 1;
  
  // Try to determine what type of search this is
  if (/^0x[a-fA-F0-9]{64}$/.test(query)) {
    // Ethereum transaction hash or block hash
    const transaction = await getTransactionByHash(nodeId, query);
    if (transaction) {
      return res.json({ type: 'ethereum_transaction', vm_type: 'ethereum', data: transaction });
    }
    
    // Could be a block hash - would need to implement block by hash lookup
    return res.status(404).json({ error: 'Not found' });
  } else if (/^[1-9A-HJ-NP-Za-km-z]{32,44}$/.test(query)) {
    // Solana transaction signature (Base58 format)
    const transaction = await getSolanaTransactionBySignature(nodeId, query);
    if (transaction) {
      return res.json({ type: 'solana_transaction', vm_type: 'solana', data: transaction });
    }
    return res.status(404).json({ error: 'Solana transaction not found' });
  } else if (/^\d+$/.test(query)) {
    // Block number (Ethereum) or Slot (Solana)
    const blockNumber = parseInt(query);
    
    // Try Ethereum first
    const ethBlock = await getBlockByNumber(nodeId, blockNumber);
    if (ethBlock) {
      return res.json({ type: 'ethereum_block', vm_type: 'ethereum', data: ethBlock });
    }
    
    // Try Solana slot
    const node = MULTIVM_NODES.find(n => n.id == nodeId);
    if (node) {
      const blockResult = await makeRpcRequest(node.solana, 'getBlock', [
        blockNumber,
        { encoding: 'json', transactionDetails: 'full', rewards: false }
      ]);
      
      if (blockResult.success && blockResult.data) {
        return res.json({ 
          type: 'solana_block', 
          vm_type: 'solana', 
          data: {
            vm_type: 'solana',
            slot: blockNumber,
            ...blockResult.data
          }
        });
      }
    }
    
    return res.status(404).json({ error: 'Block/Slot not found' });
  } else if (/^0x[a-fA-F0-9]{40}$/.test(query)) {
    // Ethereum address
    return res.json({ 
      type: 'ethereum_address', 
      vm_type: 'ethereum', 
      data: { address: query, message: 'Ethereum address lookup not implemented yet' } 
    });
  } else if (/^[1-9A-HJ-NP-Za-km-z]{32,44}$/.test(query) && query.length <= 44) {
    // Solana address (Base58)
    return res.json({ 
      type: 'solana_address', 
      vm_type: 'solana', 
      data: { address: query, message: 'Solana address lookup not implemented yet' } 
    });
  }
  
  res.status(400).json({ error: 'Invalid search query format' });
});

// WebSocket connections
io.on('connection', (socket) => {
  console.log('🔌 Client connected to explorer');
  
  // Send current status immediately
  socket.emit('networkStatus', networkStatus);
  
  socket.on('disconnect', () => {
    console.log('🔌 Client disconnected from explorer');
  });
});

// Serve the frontend
app.get('/', (req, res) => {
  res.sendFile(path.join(__dirname, 'public', 'index.html'));
});

// Start the server
const PORT = process.env.PORT || 3000;

// Update network status every 10 seconds
cron.schedule('*/10 * * * * *', updateNetworkStatus);

// Initial status update
updateNetworkStatus();

server.listen(PORT, () => {
  console.log('🚀 MultiVM Blockchain Explorer started!');
  console.log(`📊 Dashboard: http://localhost:${PORT}`);
  console.log(`🔗 API: http://localhost:${PORT}/api/status`);
  console.log(`🌐 Monitoring ${MULTIVM_NODES.length} MultiVM nodes`);
  console.log('⏱️  Updating every 10 seconds');
});

module.exports = app;