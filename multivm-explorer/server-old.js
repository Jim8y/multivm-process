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
  { id: 1, name: 'Node 1', multivm: 'http://multivm-node1:8080', reth: 'http://reth-node1:8545', rethEngine: 'http://reth-node1:8551' },
  { id: 2, name: 'Node 2', multivm: 'http://multivm-node2:8080', reth: 'http://reth-node2:8545', rethEngine: 'http://reth-node2:8551' },
  { id: 3, name: 'Node 3', multivm: 'http://multivm-node3:8080', reth: 'http://reth-node3:8545', rethEngine: 'http://reth-node3:8551' },
  { id: 4, name: 'Node 4', multivm: 'http://multivm-node4:8080', reth: 'http://reth-node4:8545', rethEngine: 'http://reth-node4:8551' },
  { id: 5, name: 'Node 5', multivm: 'http://multivm-node5:8080', reth: 'http://reth-node5:8545', rethEngine: 'http://reth-node5:8551' },
  { id: 6, name: 'Node 6', multivm: 'http://multivm-node6:8080', reth: 'http://reth-node6:8545', rethEngine: 'http://reth-node6:8551' },
  { id: 7, name: 'Node 7', multivm: 'http://multivm-node7:8080', reth: 'http://reth-node7:8545', rethEngine: 'http://reth-node7:8551' }
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

// Get latest block information
async function getLatestBlock(node) {
  const blockResult = await makeRpcRequest(node.reth, 'eth_getBlockByNumber', ['latest', true]);
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
      transactions: block.transactions || []
    };
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

// Get transaction by hash
async function getTransactionByHash(nodeId, txHash) {
  const node = MULTIVM_NODES.find(n => n.id == nodeId);
  if (!node) return null;
  
  const txResult = await makeRpcRequest(node.reth, 'eth_getTransactionByHash', [txHash]);
  const receiptResult = await makeRpcRequest(node.reth, 'eth_getTransactionReceipt', [txHash]);
  
  if (txResult.success && txResult.data) {
    const tx = txResult.data;
    const receipt = receiptResult.success ? receiptResult.data : null;
    
    return {
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

// Update network status
async function updateNetworkStatus() {
  console.log('🔄 Updating network status...');
  
  const nodeStatuses = await Promise.all(
    MULTIVM_NODES.map(async (node) => {
      const multivmHealth = await checkMultiVMHealth(node);
      const rethStatus = await checkRethStatus(node);
      const latestBlock = rethStatus.healthy ? await getLatestBlock(node) : null;
      
      return {
        id: node.id,
        name: node.name,
        multivm: {
          healthy: multivmHealth.healthy,
          version: multivmHealth.version,
          error: multivmHealth.error
        },
        reth: {
          healthy: rethStatus.healthy,
          chainId: rethStatus.chainId,
          blockNumber: rethStatus.blockNumber,
          syncing: rethStatus.syncing
        },
        latestBlock: latestBlock,
        lastChecked: new Date()
      };
    })
  );
  
  const healthyNodes = nodeStatuses.filter(node => 
    node.multivm.healthy && node.reth.healthy
  ).length;
  
  const latestBlockNumber = Math.max(
    ...nodeStatuses
      .filter(node => node.latestBlock)
      .map(node => node.latestBlock.number)
  );
  
  networkStatus = {
    lastUpdate: new Date(),
    totalNodes: MULTIVM_NODES.length,
    healthyNodes: healthyNodes,
    latestBlock: latestBlockNumber || 0,
    totalTransactions: nodeStatuses.reduce((sum, node) => 
      sum + (node.latestBlock?.transactionCount || 0), 0
    ),
    chainId: '1337',
    consensus: 'Malachite BFT',
    nodes: nodeStatuses
  };
  
  // Emit to connected clients
  io.emit('networkStatus', networkStatus);
  
  console.log(`✅ Status updated: ${healthyNodes}/${MULTIVM_NODES.length} nodes healthy, latest block: ${latestBlockNumber}`);
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
  
  if (!/^0x[a-fA-F0-9]{64}$/.test(txHash)) {
    return res.status(400).json({ error: 'Invalid transaction hash' });
  }
  
  const transaction = await getTransactionByHash(nodeId, txHash);
  if (!transaction) {
    return res.status(404).json({ error: 'Transaction not found' });
  }
  
  res.json(transaction);
});

// Search endpoint
app.get('/api/search/:query', async (req, res) => {
  const query = req.params.query;
  const nodeId = req.query.node || 1;
  
  // Try to determine what type of search this is
  if (/^0x[a-fA-F0-9]{64}$/.test(query)) {
    // Transaction hash or block hash
    const transaction = await getTransactionByHash(nodeId, query);
    if (transaction) {
      return res.json({ type: 'transaction', data: transaction });
    }
    
    // Could be a block hash - would need to implement block by hash lookup
    return res.status(404).json({ error: 'Not found' });
  } else if (/^\d+$/.test(query)) {
    // Block number
    const blockNumber = parseInt(query);
    const block = await getBlockByNumber(nodeId, blockNumber);
    if (block) {
      return res.json({ type: 'block', data: block });
    }
    return res.status(404).json({ error: 'Block not found' });
  } else if (/^0x[a-fA-F0-9]{40}$/.test(query)) {
    // Address - would need to implement address lookup
    return res.json({ type: 'address', data: { address: query, message: 'Address lookup not implemented yet' } });
  }
  
  res.status(400).json({ error: 'Invalid search query' });
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