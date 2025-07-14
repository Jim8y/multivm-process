// MultiVM Explorer - Enhanced JavaScript
const socket = io();

// Global state
let currentView = 'overview';
let networkStatus = null;
let selectedNode = 1;

// Utility functions
function formatNumber(num) {
    return new Intl.NumberFormat().format(num);
}

function formatHash(hash, length = 16) {
    if (!hash) return 'N/A';
    return hash.substring(0, length) + '...';
}

function formatTimestamp(timestamp) {
    if (!timestamp) return 'N/A';
    const date = new Date(timestamp * 1000);
    return date.toLocaleString();
}

function formatTimeAgo(timestamp) {
    if (!timestamp) return 'N/A';
    const seconds = Math.floor(Date.now() / 1000 - timestamp);
    
    if (seconds < 60) return `${seconds}s ago`;
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
    if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;
    return `${Math.floor(seconds / 86400)}d ago`;
}

function formatEther(wei) {
    if (!wei) return '0';
    // Simple conversion - for display purposes
    const eth = parseInt(wei, 16) / 1e18;
    return eth.toFixed(6);
}

// Navigation functions
function showOverview() {
    setActiveTab('overview');
    currentView = 'overview';
    renderOverview();
}

function showBlocks() {
    setActiveTab('blocks');
    currentView = 'blocks';
    renderBlocks();
}

function showTransactions() {
    setActiveTab('transactions');
    currentView = 'transactions';
    renderTransactions();
}

function showNodes() {
    setActiveTab('nodes');
    currentView = 'nodes';
    renderNodes();
}

function showBlockDetails(blockNumber) {
    currentView = 'blockDetails';
    renderBlockDetails(blockNumber);
}

function showTransactionDetails(txHash) {
    currentView = 'transactionDetails';
    renderTransactionDetails(txHash);
}

function setActiveTab(tabName) {
    document.querySelectorAll('.tab').forEach(tab => {
        tab.classList.remove('active');
    });
    
    const tabs = ['overview', 'blocks', 'transactions', 'nodes'];
    const index = tabs.indexOf(tabName);
    if (index >= 0) {
        document.querySelectorAll('.tab')[index].classList.add('active');
    }
}

// Search functionality
function handleSearch(event) {
    if (event.key === 'Enter') {
        const query = event.target.value.trim();
        if (!query) return;
        
        performSearch(query);
    }
}

async function performSearch(query) {
    try {
        const response = await fetch(`/api/search/${query}?node=${selectedNode}`);
        const result = await response.json();
        
        if (response.ok) {
            if (result.type === 'block') {
                showBlockDetails(result.data.number);
            } else if (result.type === 'transaction') {
                showTransactionDetails(result.data.hash);
            } else if (result.type === 'address') {
                alert('Address search coming soon!');
            }
        } else {
            alert('Search failed: ' + result.error);
        }
    } catch (error) {
        console.error('Search error:', error);
        alert('Search error: ' + error.message);
    }
}

// Render functions
function renderOverview() {
    if (!networkStatus) {
        document.getElementById('mainContent').innerHTML = '<div class="loading"><div class="spinner"></div></div>';
        return;
    }
    
    const content = `
        <div class="stats-grid">
            <div class="stat-card">
                <div class="stat-label">Latest Block</div>
                <div class="stat-value">#${formatNumber(networkStatus.latestBlock)}</div>
                <div class="stat-change positive">
                    <i class="fas fa-arrow-up"></i>
                    <span>Block time: ~3s</span>
                </div>
            </div>
            
            <div class="stat-card">
                <div class="stat-label">Total Transactions</div>
                <div class="stat-value">${formatNumber(networkStatus.totalTransactions)}</div>
                <div class="stat-change positive">
                    <i class="fas fa-arrow-up"></i>
                    <span>Active</span>
                </div>
            </div>
            
            <div class="stat-card">
                <div class="stat-label">Network Health</div>
                <div class="stat-value">${networkStatus.healthyNodes}/${networkStatus.totalNodes}</div>
                <div class="stat-change ${networkStatus.healthyNodes === networkStatus.totalNodes ? 'positive' : 'negative'}">
                    <i class="fas fa-${networkStatus.healthyNodes === networkStatus.totalNodes ? 'check' : 'exclamation'}-circle"></i>
                    <span>${networkStatus.healthyNodes === networkStatus.totalNodes ? 'All nodes healthy' : 'Some nodes down'}</span>
                </div>
            </div>
            
            <div class="stat-card">
                <div class="stat-label">Consensus</div>
                <div class="stat-value">${networkStatus.consensus}</div>
                <div class="stat-change positive">
                    <i class="fas fa-shield-alt"></i>
                    <span>Byzantine Fault Tolerant</span>
                </div>
            </div>
        </div>
        
        <div class="data-section">
            <div class="section-header">
                <h3 class="section-title">Latest Blocks</h3>
                <a href="#" class="view-all-btn" onclick="showBlocks(); return false;">View All</a>
            </div>
            <div id="latestBlocksList">
                <div class="loading"><div class="spinner"></div></div>
            </div>
        </div>
        
        <div class="data-section">
            <div class="section-header">
                <h3 class="section-title">Recent Transactions</h3>
                <a href="#" class="view-all-btn" onclick="showTransactions(); return false;">View All</a>
            </div>
            <div id="recentTransactionsList">
                <div class="loading"><div class="spinner"></div></div>
            </div>
        </div>
    `;
    
    document.getElementById('mainContent').innerHTML = content;
    
    // Load latest blocks and transactions
    loadLatestBlocks();
    loadRecentTransactions();
}

async function loadLatestBlocks() {
    try {
        const response = await fetch(`/api/blocks?node=${selectedNode}&limit=5`);
        const blocks = await response.json();
        
        let html = '<table class="data-table"><thead><tr>';
        html += '<th>Block</th><th>Age</th><th>Txns</th><th>Gas Used</th><th>Hash</th>';
        html += '</tr></thead><tbody>';
        
        blocks.forEach(block => {
            html += `
                <tr>
                    <td><a href="#" class="hash-link" onclick="showBlockDetails(${block.number}); return false;">#${formatNumber(block.number)}</a></td>
                    <td>${formatTimeAgo(block.timestamp)}</td>
                    <td>${block.transactionCount}</td>
                    <td>${formatNumber(block.gasUsed)}</td>
                    <td><span class="hash-link" onclick="navigator.clipboard.writeText('${block.hash}')" style="cursor: pointer;" title="Click to copy">${formatHash(block.hash)}</span></td>
                </tr>
            `;
        });
        
        html += '</tbody></table>';
        document.getElementById('latestBlocksList').innerHTML = html;
    } catch (error) {
        console.error('Error loading blocks:', error);
        document.getElementById('latestBlocksList').innerHTML = '<div class="empty-state"><i class="fas fa-exclamation-triangle"></i><p>Failed to load blocks</p></div>';
    }
}

async function loadRecentTransactions() {
    try {
        const response = await fetch(`/api/blocks?node=${selectedNode}&limit=3`);
        const blocks = await response.json();
        
        let transactions = [];
        blocks.forEach(block => {
            if (block.transactions) {
                block.transactions.forEach(tx => {
                    transactions.push({
                        ...tx,
                        blockNumber: block.number,
                        timestamp: block.timestamp
                    });
                });
            }
        });
        
        if (transactions.length === 0) {
            document.getElementById('recentTransactionsList').innerHTML = '<div class="empty-state"><i class="fas fa-inbox"></i><p>No recent transactions</p></div>';
            return;
        }
        
        let html = '<table class="data-table"><thead><tr>';
        html += '<th>Tx Hash</th><th>Block</th><th>From</th><th>To</th><th>Value</th>';
        html += '</tr></thead><tbody>';
        
        transactions.slice(0, 10).forEach(tx => {
            html += `
                <tr>
                    <td><a href="#" class="hash-link" onclick="showTransactionDetails('${tx.hash}'); return false;">${formatHash(tx.hash)}</a></td>
                    <td><a href="#" class="hash-link" onclick="showBlockDetails(${tx.blockNumber}); return false;">#${formatNumber(tx.blockNumber)}</a></td>
                    <td><span class="address-link" onclick="navigator.clipboard.writeText('${tx.from}')" style="cursor: pointer;" title="Click to copy">${formatHash(tx.from, 12)}</span></td>
                    <td><span class="address-link" onclick="navigator.clipboard.writeText('${tx.to || 'Contract Creation'}')" style="cursor: pointer;" title="Click to copy">${tx.to ? formatHash(tx.to, 12) : 'Contract Creation'}</span></td>
                    <td>${formatEther(tx.value)} ETH</td>
                </tr>
            `;
        });
        
        html += '</tbody></table>';
        document.getElementById('recentTransactionsList').innerHTML = html;
    } catch (error) {
        console.error('Error loading transactions:', error);
        document.getElementById('recentTransactionsList').innerHTML = '<div class="empty-state"><i class="fas fa-exclamation-triangle"></i><p>Failed to load transactions</p></div>';
    }
}

function renderBlocks() {
    const content = `
        <div class="data-section">
            <div class="section-header">
                <h3 class="section-title">All Blocks</h3>
                <select onchange="selectedNode = this.value; renderBlocks();" style="background: var(--card-bg); color: var(--text-primary); border: 1px solid var(--border-color); padding: 0.5rem; border-radius: 0.375rem;">
                    ${networkStatus ? networkStatus.nodes.map(node => 
                        `<option value="${node.id}" ${node.id == selectedNode ? 'selected' : ''}>${node.name}</option>`
                    ).join('') : ''}
                </select>
            </div>
            <div id="blocksList">
                <div class="loading"><div class="spinner"></div></div>
            </div>
        </div>
    `;
    
    document.getElementById('mainContent').innerHTML = content;
    loadAllBlocks();
}

async function loadAllBlocks() {
    try {
        const response = await fetch(`/api/blocks?node=${selectedNode}&limit=20`);
        const blocks = await response.json();
        
        let html = '<table class="data-table"><thead><tr>';
        html += '<th>Block</th><th>Age</th><th>Transactions</th><th>Miner</th><th>Gas Used</th><th>Gas Limit</th><th>Hash</th>';
        html += '</tr></thead><tbody>';
        
        blocks.forEach(block => {
            const gasPercent = ((block.gasUsed / block.gasLimit) * 100).toFixed(1);
            html += `
                <tr>
                    <td><a href="#" class="hash-link" onclick="showBlockDetails(${block.number}); return false;">#${formatNumber(block.number)}</a></td>
                    <td>${formatTimeAgo(block.timestamp)}</td>
                    <td>${block.transactionCount}</td>
                    <td><span class="address-link" onclick="navigator.clipboard.writeText('${block.miner}')" style="cursor: pointer;" title="Click to copy">${formatHash(block.miner, 12)}</span></td>
                    <td>${formatNumber(block.gasUsed)} (${gasPercent}%)</td>
                    <td>${formatNumber(block.gasLimit)}</td>
                    <td><span class="hash-link" onclick="navigator.clipboard.writeText('${block.hash}')" style="cursor: pointer;" title="Click to copy">${formatHash(block.hash, 16)}</span></td>
                </tr>
            `;
        });
        
        html += '</tbody></table>';
        document.getElementById('blocksList').innerHTML = html;
    } catch (error) {
        console.error('Error loading blocks:', error);
        document.getElementById('blocksList').innerHTML = '<div class="empty-state"><i class="fas fa-exclamation-triangle"></i><p>Failed to load blocks</p></div>';
    }
}

function renderTransactions() {
    const content = `
        <div class="data-section">
            <div class="section-header">
                <h3 class="section-title">All Transactions</h3>
                <select onchange="selectedNode = this.value; renderTransactions();" style="background: var(--card-bg); color: var(--text-primary); border: 1px solid var(--border-color); padding: 0.5rem; border-radius: 0.375rem;">
                    ${networkStatus ? networkStatus.nodes.map(node => 
                        `<option value="${node.id}" ${node.id == selectedNode ? 'selected' : ''}>${node.name}</option>`
                    ).join('') : ''}
                </select>
            </div>
            <div id="transactionsList">
                <div class="loading"><div class="spinner"></div></div>
            </div>
        </div>
    `;
    
    document.getElementById('mainContent').innerHTML = content;
    loadAllTransactions();
}

async function loadAllTransactions() {
    try {
        const response = await fetch(`/api/blocks?node=${selectedNode}&limit=10`);
        const blocks = await response.json();
        
        let transactions = [];
        blocks.forEach(block => {
            if (block.transactions) {
                block.transactions.forEach(tx => {
                    transactions.push({
                        ...tx,
                        blockNumber: block.number,
                        timestamp: block.timestamp
                    });
                });
            }
        });
        
        if (transactions.length === 0) {
            document.getElementById('transactionsList').innerHTML = '<div class="empty-state"><i class="fas fa-inbox"></i><p>No transactions found</p></div>';
            return;
        }
        
        let html = '<table class="data-table"><thead><tr>';
        html += '<th>Tx Hash</th><th>Block</th><th>Age</th><th>From</th><th>To</th><th>Value</th><th>Gas</th>';
        html += '</tr></thead><tbody>';
        
        transactions.forEach(tx => {
            html += `
                <tr>
                    <td><a href="#" class="hash-link" onclick="showTransactionDetails('${tx.hash}'); return false;">${formatHash(tx.hash, 16)}</a></td>
                    <td><a href="#" class="hash-link" onclick="showBlockDetails(${tx.blockNumber}); return false;">#${formatNumber(tx.blockNumber)}</a></td>
                    <td>${formatTimeAgo(tx.timestamp)}</td>
                    <td><span class="address-link" onclick="navigator.clipboard.writeText('${tx.from}')" style="cursor: pointer;" title="Click to copy">${formatHash(tx.from, 12)}</span></td>
                    <td><span class="address-link" onclick="navigator.clipboard.writeText('${tx.to || 'Contract Creation'}')" style="cursor: pointer;" title="Click to copy">${tx.to ? formatHash(tx.to, 12) : 'Contract Creation'}</span></td>
                    <td>${formatEther(tx.value)} ETH</td>
                    <td>${parseInt(tx.gas, 16)}</td>
                </tr>
            `;
        });
        
        html += '</tbody></table>';
        document.getElementById('transactionsList').innerHTML = html;
    } catch (error) {
        console.error('Error loading transactions:', error);
        document.getElementById('transactionsList').innerHTML = '<div class="empty-state"><i class="fas fa-exclamation-triangle"></i><p>Failed to load transactions</p></div>';
    }
}

function renderNodes() {
    if (!networkStatus) {
        document.getElementById('mainContent').innerHTML = '<div class="loading"><div class="spinner"></div></div>';
        return;
    }
    
    const content = `
        <div class="stats-grid">
            ${networkStatus.nodes.map(node => `
                <div class="stat-card">
                    <div class="stat-label">${node.name}</div>
                    <div class="stat-value" style="font-size: 1.5rem;">
                        ${node.multivm.healthy && node.reth.healthy ? 
                            '<i class="fas fa-check-circle text-success"></i> Healthy' : 
                            '<i class="fas fa-exclamation-circle text-danger"></i> Down'}
                    </div>
                    <div class="detail-item">
                        <div class="detail-label">MultiVM</div>
                        <div class="detail-value">${node.multivm.healthy ? 
                            `<span class="text-success">Online</span> - ${node.multivm.version || 'Unknown version'}` : 
                            `<span class="text-danger">Offline</span>`}
                        </div>
                    </div>
                    <div class="detail-item">
                        <div class="detail-label">Reth</div>
                        <div class="detail-value">${node.reth.healthy ? 
                            `<span class="text-success">Block #${node.reth.blockNumber}</span>` : 
                            `<span class="text-danger">Offline</span>`}
                        </div>
                    </div>
                    ${node.reth.syncing ? '<div class="badge badge-pending">Syncing</div>' : ''}
                </div>
            `).join('')}
        </div>
    `;
    
    document.getElementById('mainContent').innerHTML = content;
}

async function renderBlockDetails(blockNumber) {
    const content = `
        <a href="#" class="back-button" onclick="showBlocks(); return false;">
            <i class="fas fa-arrow-left"></i> Back to Blocks
        </a>
        
        <div class="detail-container">
            <div class="detail-header">
                <h2 class="detail-title">Block #${formatNumber(blockNumber)}</h2>
                <p class="detail-subtitle">Block details and transactions</p>
            </div>
            <div id="blockDetailsContent">
                <div class="loading"><div class="spinner"></div></div>
            </div>
        </div>
    `;
    
    document.getElementById('mainContent').innerHTML = content;
    
    try {
        const response = await fetch(`/api/blocks/${blockNumber}?node=${selectedNode}`);
        const block = await response.json();
        
        if (!response.ok) {
            throw new Error(block.error || 'Failed to load block');
        }
        
        const detailsHtml = `
            <div class="detail-grid">
                <div class="detail-item">
                    <div class="detail-label">Block Hash</div>
                    <div class="detail-value">${block.hash}</div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">Parent Hash</div>
                    <div class="detail-value">${block.parentHash}</div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">Timestamp</div>
                    <div class="detail-value">${formatTimestamp(block.timestamp)} (${formatTimeAgo(block.timestamp)})</div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">Transactions</div>
                    <div class="detail-value">${block.transactionCount} transactions</div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">Miner</div>
                    <div class="detail-value">${block.miner}</div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">Gas Used</div>
                    <div class="detail-value">${formatNumber(block.gasUsed)} / ${formatNumber(block.gasLimit)} (${((block.gasUsed / block.gasLimit) * 100).toFixed(2)}%)</div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">Size</div>
                    <div class="detail-value">${formatNumber(block.size)} bytes</div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">Nonce</div>
                    <div class="detail-value">${block.nonce || '0x0'}</div>
                </div>
            </div>
            
            ${block.transactions && block.transactions.length > 0 ? `
                <div class="data-section" style="margin-top: 2rem;">
                    <div class="section-header">
                        <h3 class="section-title">Transactions</h3>
                    </div>
                    <table class="data-table">
                        <thead>
                            <tr>
                                <th>Index</th>
                                <th>Hash</th>
                                <th>From</th>
                                <th>To</th>
                                <th>Value</th>
                            </tr>
                        </thead>
                        <tbody>
                            ${block.transactions.map((tx, index) => `
                                <tr>
                                    <td>${index}</td>
                                    <td><a href="#" class="hash-link" onclick="showTransactionDetails('${tx.hash}'); return false;">${formatHash(tx.hash, 20)}</a></td>
                                    <td><span class="address-link">${formatHash(tx.from, 12)}</span></td>
                                    <td><span class="address-link">${tx.to ? formatHash(tx.to, 12) : 'Contract Creation'}</span></td>
                                    <td>${formatEther(tx.value)} ETH</td>
                                </tr>
                            `).join('')}
                        </tbody>
                    </table>
                </div>
            ` : '<div class="empty-state"><i class="fas fa-inbox"></i><p>No transactions in this block</p></div>'}
        `;
        
        document.getElementById('blockDetailsContent').innerHTML = detailsHtml;
    } catch (error) {
        console.error('Error loading block details:', error);
        document.getElementById('blockDetailsContent').innerHTML = `
            <div class="empty-state">
                <i class="fas fa-exclamation-triangle"></i>
                <p>Error loading block: ${error.message}</p>
            </div>
        `;
    }
}

async function renderTransactionDetails(txHash) {
    const content = `
        <a href="#" class="back-button" onclick="showTransactions(); return false;">
            <i class="fas fa-arrow-left"></i> Back to Transactions
        </a>
        
        <div class="detail-container">
            <div class="detail-header">
                <h2 class="detail-title">Transaction Details</h2>
                <p class="detail-subtitle">${txHash}</p>
            </div>
            <div id="transactionDetailsContent">
                <div class="loading"><div class="spinner"></div></div>
            </div>
        </div>
    `;
    
    document.getElementById('mainContent').innerHTML = content;
    
    try {
        const response = await fetch(`/api/transactions/${txHash}?node=${selectedNode}`);
        const tx = await response.json();
        
        if (!response.ok) {
            throw new Error(tx.error || 'Failed to load transaction');
        }
        
        const statusBadge = tx.status === 1 ? 
            '<span class="badge badge-success">Success</span>' : 
            tx.status === 0 ? 
            '<span class="badge badge-failed">Failed</span>' : 
            '<span class="badge badge-pending">Pending</span>';
        
        const detailsHtml = `
            <div class="detail-grid">
                <div class="detail-item">
                    <div class="detail-label">Transaction Hash</div>
                    <div class="detail-value">${tx.hash}</div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">Status</div>
                    <div class="detail-value">${statusBadge}</div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">Block</div>
                    <div class="detail-value">
                        <a href="#" class="hash-link" onclick="showBlockDetails(${tx.blockNumber}); return false;">
                            #${formatNumber(tx.blockNumber)}
                        </a>
                    </div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">From</div>
                    <div class="detail-value">${tx.from}</div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">To</div>
                    <div class="detail-value">${tx.to || 'Contract Creation'}</div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">Value</div>
                    <div class="detail-value">${formatEther(tx.value)} ETH</div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">Gas Limit</div>
                    <div class="detail-value">${formatNumber(tx.gas)}</div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">Gas Used</div>
                    <div class="detail-value">${tx.gasUsed ? formatNumber(tx.gasUsed) : 'Pending'}</div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">Gas Price</div>
                    <div class="detail-value">${parseInt(tx.gasPrice, 16) / 1e9} Gwei</div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">Nonce</div>
                    <div class="detail-value">${tx.nonce}</div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">Position</div>
                    <div class="detail-value">${tx.transactionIndex}</div>
                </div>
            </div>
            
            ${tx.input && tx.input !== '0x' ? `
                <div class="detail-item" style="margin-top: 2rem;">
                    <div class="detail-label">Input Data</div>
                    <div class="detail-value" style="word-break: break-all; font-family: monospace; font-size: 0.875rem;">
                        ${tx.input}
                    </div>
                </div>
            ` : ''}
            
            ${tx.logs && tx.logs.length > 0 ? `
                <div class="data-section" style="margin-top: 2rem;">
                    <div class="section-header">
                        <h3 class="section-title">Event Logs</h3>
                    </div>
                    <table class="data-table">
                        <thead>
                            <tr>
                                <th>Index</th>
                                <th>Address</th>
                                <th>Topics</th>
                            </tr>
                        </thead>
                        <tbody>
                            ${tx.logs.map((log, index) => `
                                <tr>
                                    <td>${index}</td>
                                    <td><span class="address-link">${formatHash(log.address, 20)}</span></td>
                                    <td style="font-size: 0.875rem;">${log.topics.map(t => formatHash(t, 16)).join(', ')}</td>
                                </tr>
                            `).join('')}
                        </tbody>
                    </table>
                </div>
            ` : ''}
        `;
        
        document.getElementById('transactionDetailsContent').innerHTML = detailsHtml;
    } catch (error) {
        console.error('Error loading transaction details:', error);
        document.getElementById('transactionDetailsContent').innerHTML = `
            <div class="empty-state">
                <i class="fas fa-exclamation-triangle"></i>
                <p>Error loading transaction: ${error.message}</p>
            </div>
        `;
    }
}

// WebSocket handlers
socket.on('networkStatus', (status) => {
    networkStatus = status;
    
    // Update chain ID in header
    document.getElementById('chainId').textContent = status.chainId;
    
    // Update current view if needed
    if (currentView === 'overview') {
        renderOverview();
    } else if (currentView === 'nodes') {
        renderNodes();
    }
});

socket.on('connect', () => {
    console.log('Connected to explorer WebSocket');
});

socket.on('disconnect', () => {
    console.log('Disconnected from explorer WebSocket');
});

// Initialize
document.addEventListener('DOMContentLoaded', () => {
    // Load initial status
    fetch('/api/status')
        .then(response => response.json())
        .then(status => {
            networkStatus = status;
            showOverview();
        })
        .catch(error => {
            console.error('Error loading initial status:', error);
            document.getElementById('mainContent').innerHTML = `
                <div class="empty-state">
                    <i class="fas fa-exclamation-triangle"></i>
                    <p>Failed to connect to explorer API</p>
                </div>
            `;
        });
});

// Export functions for global access
window.showOverview = showOverview;
window.showBlocks = showBlocks;
window.showTransactions = showTransactions;
window.showNodes = showNodes;
window.showBlockDetails = showBlockDetails;
window.showTransactionDetails = showTransactionDetails;
window.handleSearch = handleSearch;