#!/usr/bin/env node

/**
 * MultiVM Explorer Connectivity Test
 * 
 * This script tests the explorer's ability to connect to validator nodes
 * and fetch chain data via RPC calls.
 */

const axios = require('axios');

// Test configuration
const TEST_NODES = [
  { 
    id: 1, 
    name: 'Local Test Node', 
    multivm: 'http://localhost:8080', 
    reth: 'http://localhost:8545', 
    solana: 'http://localhost:8899'
  }
];

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
    
    if (response.data.error) {
      return { success: false, error: response.data.error.message };
    }
    
    return { success: true, data: response.data.result };
  } catch (error) {
    return { success: false, error: error.message };
  }
}

async function testMultiVMHealth(node) {
  console.log(`\n🔍 Testing MultiVM Health for ${node.name}...`);
  const result = await makeRequest(`${node.multivm}/health`);
  
  if (result.success) {
    console.log(`✅ MultiVM Health: OK`);
    console.log(`   Status: ${result.data?.data?.status || 'unknown'}`);
    console.log(`   Version: ${result.data?.data?.version || 'unknown'}`);
    return true;
  } else {
    console.log(`❌ MultiVM Health: FAILED`);
    console.log(`   Error: ${result.error}`);
    return false;
  }
}

async function testEthereumRPC(node) {
  console.log(`\n🔍 Testing Ethereum RPC for ${node.name}...`);
  
  // Test chain ID
  const chainIdResult = await makeRpcRequest(node.reth, 'eth_chainId');
  if (chainIdResult.success) {
    console.log(`✅ Ethereum Chain ID: ${parseInt(chainIdResult.data, 16)}`);
  } else {
    console.log(`❌ Ethereum Chain ID: FAILED - ${chainIdResult.error}`);
    return false;
  }
  
  // Test latest block
  const blockResult = await makeRpcRequest(node.reth, 'eth_blockNumber');
  if (blockResult.success) {
    const blockNum = parseInt(blockResult.data, 16);
    console.log(`✅ Ethereum Latest Block: ${blockNum}`);
  } else {
    console.log(`❌ Ethereum Latest Block: FAILED - ${blockResult.error}`);
    return false;
  }
  
  // Test syncing status
  const syncResult = await makeRpcRequest(node.reth, 'eth_syncing');
  if (syncResult.success) {
    console.log(`✅ Ethereum Syncing: ${syncResult.data === false ? 'In sync' : 'Syncing'}`);
  } else {
    console.log(`⚠️  Ethereum Syncing Status: Unknown`);
  }
  
  return true;
}

async function testSolanaRPC(node) {
  console.log(`\n🔍 Testing Solana RPC for ${node.name}...`);
  
  // Test slot
  const slotResult = await makeRpcRequest(node.solana, 'getSlot');
  if (slotResult.success) {
    console.log(`✅ Solana Current Slot: ${slotResult.data}`);
  } else {
    console.log(`❌ Solana Current Slot: FAILED - ${slotResult.error}`);
    return false;
  }
  
  // Test version
  const versionResult = await makeRpcRequest(node.solana, 'getVersion');
  if (versionResult.success) {
    console.log(`✅ Solana Version: ${versionResult.data['solana-core']}`);
  } else {
    console.log(`⚠️  Solana Version: Unknown`);
  }
  
  // Test health
  const healthResult = await makeRpcRequest(node.solana, 'getHealth');
  if (healthResult.success) {
    console.log(`✅ Solana Health: ${healthResult.data}`);
  } else {
    console.log(`⚠️  Solana Health: Unknown`);
  }
  
  return true;
}

async function testChainDataFetching(node) {
  console.log(`\n🔍 Testing Chain Data Fetching for ${node.name}...`);
  
  // Test Ethereum block data
  const ethBlockResult = await makeRpcRequest(node.reth, 'eth_getBlockByNumber', ['latest', true]);
  if (ethBlockResult.success && ethBlockResult.data) {
    const block = ethBlockResult.data;
    console.log(`✅ Ethereum Block Data:`);
    console.log(`   Block Number: ${parseInt(block.number, 16)}`);
    console.log(`   Block Hash: ${block.hash}`);
    console.log(`   Transactions: ${block.transactions ? block.transactions.length : 0}`);
    console.log(`   Gas Used: ${parseInt(block.gasUsed || '0x0', 16)}`);
  } else {
    console.log(`❌ Ethereum Block Data: FAILED`);
  }
  
  // Test Solana block data
  const solSlotResult = await makeRpcRequest(node.solana, 'getSlot');
  if (solSlotResult.success) {
    const solBlockResult = await makeRpcRequest(node.solana, 'getBlock', [
      solSlotResult.data,
      { encoding: 'json', transactionDetails: 'full', rewards: false }
    ]);
    
    if (solBlockResult.success && solBlockResult.data) {
      const block = solBlockResult.data;
      console.log(`✅ Solana Block Data:`);
      console.log(`   Slot: ${solSlotResult.data}`);
      console.log(`   Block Hash: ${block.blockhash}`);
      console.log(`   Transactions: ${block.transactions ? block.transactions.length : 0}`);
      console.log(`   Block Time: ${new Date(block.blockTime * 1000).toISOString()}`);
    } else {
      console.log(`❌ Solana Block Data: FAILED`);
    }
  }
}

async function runConnectivityTests() {
  console.log('🚀 MultiVM Explorer Connectivity Test\n');
  console.log('This test verifies the explorer can connect to validator nodes');
  console.log('and fetch chain data via RPC calls.\n');
  
  const results = {
    total: 0,
    passed: 0,
    failed: 0
  };
  
  for (const node of TEST_NODES) {
    console.log(`\n${'='.repeat(60)}`);
    console.log(`Testing Node: ${node.name}`);
    console.log(`${'='.repeat(60)}`);
    
    const tests = [
      testMultiVMHealth,
      testEthereumRPC,
      testSolanaRPC,
      testChainDataFetching
    ];
    
    for (const test of tests) {
      results.total++;
      try {
        const success = await test(node);
        if (success !== false) {
          results.passed++;
        } else {
          results.failed++;
        }
      } catch (error) {
        console.log(`❌ Test failed with exception: ${error.message}`);
        results.failed++;
      }
    }
  }
  
  console.log(`\n${'='.repeat(60)}`);
  console.log('🏁 Test Results Summary');
  console.log(`${'='.repeat(60)}`);
  console.log(`Total Tests: ${results.total}`);
  console.log(`Passed: ${results.passed} ✅`);
  console.log(`Failed: ${results.failed} ❌`);
  console.log(`Success Rate: ${((results.passed / results.total) * 100).toFixed(1)}%`);
  
  if (results.failed === 0) {
    console.log('\n🎉 All tests passed! Explorer connectivity is working properly.');
    process.exit(0);
  } else {
    console.log('\n⚠️  Some tests failed. Check the validator node configurations.');
    console.log('\nTroubleshooting:');
    console.log('1. Ensure MultiVM nodes are running on the specified ports');
    console.log('2. Check that Reth is running and accessible on port 8545');
    console.log('3. Verify Solana is running and accessible on port 8899');
    console.log('4. Confirm network connectivity between explorer and nodes');
    process.exit(1);
  }
}

// Run the tests
if (require.main === module) {
  runConnectivityTests().catch((error) => {
    console.error('❌ Test runner failed:', error.message);
    process.exit(1);
  });
}

module.exports = {
  testMultiVMHealth,
  testEthereumRPC,
  testSolanaRPC,
  testChainDataFetching
};