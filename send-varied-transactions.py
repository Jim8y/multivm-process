#!/usr/bin/env python3
"""
Send varied types of transactions to MultiVM testnet
Includes transfers, contract deployments, and contract calls
"""

import json
import time
import random
import signal
import sys
from web3 import Web3
from eth_account import Account
from datetime import datetime

# Global flag for graceful shutdown
running = True

def signal_handler(sig, frame):
    global running
    print("\n\n🛑 Stopping transaction generator...")
    running = False

# Simple storage contract bytecode
STORAGE_CONTRACT_BYTECODE = "0x608060405234801561001057600080fd5b50610150806100206000396000f3fe608060405234801561001057600080fd5b50600436106100365760003560e01c80632e64cec11461003b5780636057361d14610059575b600080fd5b610043610075565b60405161005091906100d9565b60405180910390f35b610073600480360381019061006e919061009d565b61007e565b005b60008054905090565b8060008190555050565b60008135905061009781610103565b92915050565b6000602082840312156100b3576100b26100fe565b5b60006100c184828501610088565b91505092915050565b6100d3816100f4565b82525050565b60006020820190506100ee60008301846100ca565b92915050565b6000819050919050565b600080fd5b61010c816100f4565b811461011757600080fd5b5056fea2646970667358221220404e37f487a89a932dca5e77faaf6ca2de3b991f93d230604b1b8daaef64766264736f6c63430008070033"

# ERC20 token bytecode (simplified)
TOKEN_CONTRACT_BYTECODE = "0x60806040523480156100115760006000fd5b50604051610a5f380380610a5f8339818101604052810190610033919061011e565b8160039080519060200190610049929190610057565b50806004819055505061023b565b828054610063906101ae565b90600052602060002090601f01602090048101928261008557600085556100cc565b82601f1061009e57805160ff19168380011785556100cc565b828001600101855582156100cc579182015b828111156100cb5782518255916020019190600101906100b0565b5b5090506100d991906100dd565b5090565b5b808211156100f65760008160009055506001016100de565b5090565b60006101076101028461016f565b61014a565b905082815260208101848484011115610120576000005760006000fd5b61012b84828561017c565b509392505050565b600082601f83011261014557600080fd5b81516101558482602086016100fa565b91505092915050565b6000815190506101688161022a565b92915050565b6000604051905090565b600067ffffffffffffffff82111561019057600080fd5b610199826101f8565b9050602081019050919050565b600081519050919050565b60005b838110156101cf5780820151818401526020810190506101b4565b838111156101de576000848401525b50505050565b6000819050919050565b6000601f19601f8301169050919050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052602260045260246000fd5b610233816101e4565b811461023e57600080fd5b50565b610815806102506000396000f3fe608060405234801561001057600080fd5b50600436106100575760003560e01c806318160ddd1461005c57806370a082311461007a57806395d89b41146100aa578063a9059cbb146100c8578063dd62ed3e146100f8575b600080fd5b610064610128565b604051610071919061066d565b60405180910390f35b610094600480360381019061008f919061050a565b610132565b6040516100a1919061066d565b60405180910390f35b6100b261017b565b6040516100bf919061064b565b60405180910390f35b6100e260048036038101906100dd9190610537565b61020d565b6040516100ef9190610630565b60405180910390f35b610112600480360381019061010d9190610577565b610332565b60405161011f919061066d565b60405180910390f35b6000600454905090565b6000600160008373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff168152602001908152602001600020549050919050565b60606003805461018a9061077d565b80601f01602080910402602001604051908101604052809291908181526020018280546101b69061077d565b80156102035780601f106101d857610100808354040283529160200191610203565b820191906000526020600020905b8154815290600101906020018083116101e657829003601f168201915b5050505050905090565b6000600160003373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020016000205482111561025b57600080fd5b81600160003373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060008282546102aa9190610703565b9250508190555081600160008573ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060008282546103009190610688565b925050819055508273ffffffffffffffffffffffffffffffffffffffff163373ffffffffffffffffffffffffffffffffffffffff167fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef84604051610364919061066d565b60405180910390a36001905092915050565b60008060008473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060008373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002054905092915050565b60006103fe82610132565b9050919050565b600061041082610688565b9050919050565b600080fd5b600073ffffffffffffffffffffffffffffffffffffffff82169050919050565b60006104478261041c565b9050919050565b6104578161043c565b811461046257600080fd5b50565b6000813590506104748161044e565b92915050565b6000819050919050565b61048d8161047a565b811461049857600080fd5b50565b6000813590506104aa81610484565b92915050565b600080fd5b600080fd5b7f4e487b7100000000000000000000000000000000000000000000000000000000600052602260045260246000fd5b600060028204905060018216806104f157607f821691505b60208210811415610505576105046104ba565b5b50919050565b60006020828403121561052157610520610417565b5b600061052f84828501610465565b91505092915050565b6000806040838503121561054f5761054e610417565b5b600061055d85828601610465565b925050602061056e8582860161049b565b9150509250929050565b6000806040838503121561058f5761058e610417565b5b600061059d85828601610465565b92505060206105ae85828601610465565b9150509250929050565b600081519050919050565b600082825260208201905092915050565b60005b838110156105f25780820151818401526020810190506105d7565b83811115610601576000848401525b50505050565b6000601f19601f8301169050919050565b6000610623826105b8565b61062d81856105c3565b935061063d8185602086016105d4565b61064681610607565b840191505092915050565b6000602082019050818103600083015261066b8184610618565b905092915050565b6000602082019050610688600083018461047a565b92915050565b60006106998261047a565b91506106a48361047a565b9250827fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff038211156106d9576106d86107af565b5b828201905092915050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052601160045260246000fd5b600061071e8261047a565b91506107298361047a565b92508282101561073c5761073b6106e4565b5b828203905092915050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052602260045260246000fd5b600060028204905060018216806107955760708216915b5b602082108114156107a9576107a8610747565b5b50919050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052601160045260246000fd5b6000601f19601f8301169050919050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052602260045260246000fd5b600073ffffffffffffffffffffffffffffffffffffffff82169050919050565b6000819050919050565b600080fd5b6000601f19601f8301169050919050565b600080fd00a2646970667358221220a8b2e8e8a8e8e8e8e8e8e8e8e8e8e8e8e8e8e8e8e8e8e8e8e8e8e8e8e8e8e8e864736f6c63430008070033"

def get_transaction_type():
    """Randomly select a transaction type with weighted probability"""
    rand = random.random()
    if rand < 0.7:  # 70% simple transfers
        return "transfer"
    elif rand < 0.85:  # 15% contract deployments
        return "deploy"
    else:  # 15% contract calls
        return "contract_call"

def main():
    global running
    
    # Register signal handler
    signal.signal(signal.SIGINT, signal_handler)
    
    # Connect to MultiVM
    w3 = Web3(Web3.HTTPProvider("http://localhost:8545"))
    if not w3.is_connected():
        print("❌ Cannot connect to MultiVM RPC")
        return
    
    print("🎯 Varied Transaction Generator")
    print("==============================")
    print(f"✅ Connected to chain ID: {w3.eth.chain_id}")
    print("📍 Transaction types: Transfers (70%), Deployments (15%), Calls (15%)")
    print("⏱️  Sending 1 transaction per second")
    print("🛑 Press Ctrl+C to stop")
    print()
    
    # Dev account
    dev_private_key = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
    dev_account = Account.from_key(dev_private_key)
    dev_address = dev_account.address
    
    # Check balance
    balance = w3.eth.get_balance(dev_address)
    eth_balance = w3.from_wei(balance, 'ether')
    print(f"💰 Dev account balance: {eth_balance:.4f} ETH")
    
    # Load validators
    try:
        with open("testnet-production-complete/validators_complete.json", 'r') as f:
            validators = json.load(f)
    except:
        validators = [
            {"name": "Test1", "ethereum": {"address": "0x70997970C51812dc3A010C7d01b50e0d17dc79C8"}},
            {"name": "Test2", "ethereum": {"address": "0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC"}},
        ]
    
    print(f"📋 Using {len(validators)} validator addresses")
    print("🌐 View at: http://localhost:3000")
    print("\n" + "="*50 + "\n")
    
    # Counters
    nonce = w3.eth.get_transaction_count(dev_address)
    deployed_contracts = []
    stats = {
        "transfer": {"sent": 0, "success": 0},
        "deploy": {"sent": 0, "success": 0},
        "contract_call": {"sent": 0, "success": 0}
    }
    
    start_time = time.time()
    
    try:
        while running:
            tx_type = get_transaction_type()
            timestamp = datetime.now().strftime("%H:%M:%S")
            
            try:
                if tx_type == "transfer":
                    # Simple transfer
                    validator = random.choice(validators)
                    to_address = w3.to_checksum_address(validator["ethereum"]["address"])
                    amount = round(random.uniform(0.0001, 0.001), 6)
                    
                    transaction = {
                        'to': to_address,
                        'value': w3.to_wei(amount, 'ether'),
                        'gas': 21000,
                        'gasPrice': w3.to_wei(20, 'gwei'),
                        'nonce': nonce,
                        'chainId': 1337
                    }
                    
                    signed_txn = dev_account.sign_transaction(transaction)
                    raw_tx = signed_txn.rawTransaction if hasattr(signed_txn, 'rawTransaction') else signed_txn.raw_transaction
                    tx_hash = w3.eth.send_raw_transaction(raw_tx)
                    
                    stats[tx_type]["sent"] += 1
                    stats[tx_type]["success"] += 1
                    nonce += 1
                    
                    print(f"[{timestamp}] 💸 Transfer: {amount:.6f} ETH to {validator['name'][:8]:<8} | {tx_hash.hex()[:16]}...")
                    
                elif tx_type == "deploy":
                    # Deploy contract
                    bytecode = STORAGE_CONTRACT_BYTECODE if random.random() < 0.5 else TOKEN_CONTRACT_BYTECODE
                    contract_type = "Storage" if bytecode == STORAGE_CONTRACT_BYTECODE else "Token"
                    
                    transaction = {
                        'data': bytecode,
                        'gas': 500000,
                        'gasPrice': w3.to_wei(20, 'gwei'),
                        'nonce': nonce,
                        'chainId': 1337
                    }
                    
                    signed_txn = dev_account.sign_transaction(transaction)
                    raw_tx = signed_txn.rawTransaction if hasattr(signed_txn, 'rawTransaction') else signed_txn.raw_transaction
                    tx_hash = w3.eth.send_raw_transaction(raw_tx)
                    
                    # Wait for receipt to get contract address
                    receipt = w3.eth.wait_for_transaction_receipt(tx_hash, timeout=5)
                    if receipt.contractAddress:
                        deployed_contracts.append({
                            "address": receipt.contractAddress,
                            "type": contract_type
                        })
                    
                    stats[tx_type]["sent"] += 1
                    stats[tx_type]["success"] += 1
                    nonce += 1
                    
                    print(f"[{timestamp}] 📄 Deploy: {contract_type} contract | {tx_hash.hex()[:16]}...")
                    
                else:  # contract_call
                    # Call a deployed contract
                    if deployed_contracts:
                        contract = random.choice(deployed_contracts)
                        
                        # Simple storage set call (function selector for store(uint256))
                        function_selector = "0x6057361d"
                        value = random.randint(1, 1000)
                        data = function_selector + hex(value)[2:].zfill(64)
                        
                        transaction = {
                            'to': contract["address"],
                            'data': data,
                            'gas': 50000,
                            'gasPrice': w3.to_wei(20, 'gwei'),
                            'nonce': nonce,
                            'chainId': 1337
                        }
                        
                        signed_txn = dev_account.sign_transaction(transaction)
                        raw_tx = signed_txn.rawTransaction if hasattr(signed_txn, 'rawTransaction') else signed_txn.raw_transaction
                        tx_hash = w3.eth.send_raw_transaction(raw_tx)
                        
                        stats[tx_type]["sent"] += 1
                        stats[tx_type]["success"] += 1
                        nonce += 1
                        
                        print(f"[{timestamp}] 📞 Call: {contract['type']} set({value}) | {tx_hash.hex()[:16]}...")
                    else:
                        # No contracts deployed yet, do a transfer instead
                        tx_type = "transfer"
                        validator = random.choice(validators)
                        to_address = w3.to_checksum_address(validator["ethereum"]["address"])
                        amount = 0.0001
                        
                        transaction = {
                            'to': to_address,
                            'value': w3.to_wei(amount, 'ether'),
                            'gas': 21000,
                            'gasPrice': w3.to_wei(20, 'gwei'),
                            'nonce': nonce,
                            'chainId': 1337
                        }
                        
                        signed_txn = dev_account.sign_transaction(transaction)
                        raw_tx = signed_txn.rawTransaction if hasattr(signed_txn, 'rawTransaction') else signed_txn.raw_transaction
                        tx_hash = w3.eth.send_raw_transaction(raw_tx)
                        
                        stats[tx_type]["sent"] += 1
                        stats[tx_type]["success"] += 1
                        nonce += 1
                        
                        print(f"[{timestamp}] 💸 Transfer: {amount:.6f} ETH (no contracts yet) | {tx_hash.hex()[:16]}...")
                
                # Show summary every 20 transactions
                total_sent = sum(s["sent"] for s in stats.values())
                if total_sent % 20 == 0:
                    elapsed = time.time() - start_time
                    print(f"\n📊 Summary after {total_sent} transactions:")
                    for tx_type, stat in stats.items():
                        if stat["sent"] > 0:
                            rate = (stat["success"] / stat["sent"]) * 100
                            print(f"   {tx_type}: {stat['success']}/{stat['sent']} ({rate:.1f}%)")
                    print(f"   Deployed contracts: {len(deployed_contracts)}")
                    print(f"   Runtime: {int(elapsed)}s | TPS: {total_sent/elapsed:.2f}\n")
                    
            except Exception as e:
                stats[tx_type]["sent"] += 1
                print(f"[{timestamp}] ❌ {tx_type} failed: {str(e)[:40]}...")
                
                if "nonce too low" in str(e):
                    nonce = w3.eth.get_transaction_count(dev_address)
            
            # Wait 1 second
            time.sleep(1.0)
            
    except Exception as e:
        print(f"\n❌ Error: {e}")
    
    # Final statistics
    elapsed = time.time() - start_time
    total_sent = sum(s["sent"] for s in stats.values())
    total_success = sum(s["success"] for s in stats.values())
    
    if total_sent > 0:
        print("\n" + "="*50)
        print("📊 Final Statistics")
        print("="*50)
        print(f"⏱️  Runtime: {int(elapsed)}s")
        print(f"📤 Total transactions: {total_sent}")
        print(f"✅ Successful: {total_success} ({(total_success/total_sent)*100:.1f}%)")
        print("\nBreakdown by type:")
        for tx_type, stat in stats.items():
            if stat["sent"] > 0:
                rate = (stat["success"] / stat["sent"]) * 100
                print(f"  {tx_type}: {stat['success']}/{stat['sent']} ({rate:.1f}%)")
        print(f"\n📄 Contracts deployed: {len(deployed_contracts)}")
        print(f"⚡ Average TPS: {total_sent/elapsed:.2f}")
        
        # Final balance
        try:
            final_balance = w3.eth.get_balance(dev_address)
            final_eth = w3.from_wei(final_balance, 'ether')
            spent = eth_balance - final_eth
            print(f"\n💸 ETH spent: {spent:.6f} ETH")
            print(f"💰 Remaining: {final_eth:.4f} ETH")
        except:
            pass
        
        print("\n🌐 View all transactions at: http://localhost:3000")

if __name__ == "__main__":
    main()