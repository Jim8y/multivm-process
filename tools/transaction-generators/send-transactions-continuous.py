#!/usr/bin/env python3
"""
Send continuous valid transactions to MultiVM testnet
One transaction per second
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

def main():
    global running
    
    # Register signal handler for graceful shutdown
    signal.signal(signal.SIGINT, signal_handler)
    
    # Connect to MultiVM
    w3 = Web3(Web3.HTTPProvider("http://localhost:8545"))
    if not w3.is_connected():
        print("❌ Cannot connect to MultiVM RPC at http://localhost:8545")
        print("Make sure the testnet is running!")
        return
    
    print("🚀 Continuous Transaction Generator")
    print("==================================")
    print(f"✅ Connected to chain ID: {w3.eth.chain_id}")
    print("📍 Sending 1 transaction per second")
    print("🛑 Press Ctrl+C to stop")
    print()
    
    # Dev account with funds
    dev_private_key = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
    dev_account = Account.from_key(dev_private_key)
    dev_address = dev_account.address
    
    # Check initial balance
    balance = w3.eth.get_balance(dev_address)
    eth_balance = w3.from_wei(balance, 'ether')
    print(f"💰 Dev account balance: {eth_balance:.4f} ETH")
    
    if eth_balance < 0.1:
        print("⚠️  Warning: Low balance! You may run out of funds.")
    
    # Load validators to send to
    try:
        with open("testnet-production-complete/validators_complete.json", 'r') as f:
            validators = json.load(f)
    except FileNotFoundError:
        print("❌ validators_complete.json not found!")
        print("Using default test addresses...")
        validators = [
            {"name": "Test1", "ethereum": {"address": "0x70997970C51812dc3A010C7d01b50e0d17dc79C8"}},
            {"name": "Test2", "ethereum": {"address": "0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC"}},
            {"name": "Test3", "ethereum": {"address": "0x90F79bf6EB2c4f870365E785982E1f101E93b906"}},
        ]
    
    print(f"📋 Loaded {len(validators)} validator addresses")
    print("🌐 View transactions at: http://localhost:3000")
    print("\n" + "="*50 + "\n")
    
    # Transaction counters
    nonce = w3.eth.get_transaction_count(dev_address)
    total_sent = 0
    successful = 0
    failed = 0
    
    # Start time for statistics
    start_time = time.time()
    
    try:
        while running:
            # Pick random validator as recipient
            validator = random.choice(validators)
            to_address = w3.to_checksum_address(validator["ethereum"]["address"])
            
            # Random small amount (0.0001 to 0.001 ETH)
            amount = round(random.uniform(0.0001, 0.001), 6)
            
            # Timestamp for logging
            timestamp = datetime.now().strftime("%H:%M:%S")
            
            try:
                # Build transaction
                transaction = {
                    'to': to_address,
                    'value': w3.to_wei(amount, 'ether'),
                    'gas': 21000,
                    'gasPrice': w3.to_wei(20, 'gwei'),
                    'nonce': nonce,
                    'chainId': 1337
                }
                
                # Sign transaction
                signed_txn = dev_account.sign_transaction(transaction)
                
                # Handle different web3 versions
                try:
                    raw_tx = signed_txn.rawTransaction
                except AttributeError:
                    raw_tx = signed_txn.raw_transaction
                
                # Send transaction
                tx_hash = w3.eth.send_raw_transaction(raw_tx)
                tx_hash_hex = tx_hash.hex()
                
                total_sent += 1
                successful += 1
                nonce += 1
                
                # Success message with stats
                success_rate = (successful / total_sent) * 100
                print(f"[{timestamp}] ✅ TX #{total_sent}: {amount:.6f} ETH to {validator['name'][:10]:<10} | "
                      f"Hash: {tx_hash_hex[:16]}... | Success rate: {success_rate:.1f}%")
                
                # Every 10 transactions, show summary
                if total_sent % 10 == 0:
                    elapsed = time.time() - start_time
                    tps = total_sent / elapsed
                    print(f"\n📊 Summary: {successful}/{total_sent} successful ({success_rate:.1f}%) | "
                          f"TPS: {tps:.2f} | Runtime: {int(elapsed)}s\n")
                
            except Exception as e:
                failed += 1
                total_sent += 1
                print(f"[{timestamp}] ❌ TX #{total_sent}: Failed - {str(e)[:50]}...")
                
                # Check if it's a nonce issue and try to recover
                if "nonce too low" in str(e):
                    try:
                        nonce = w3.eth.get_transaction_count(dev_address)
                        print(f"    ↻ Recovered nonce: {nonce}")
                    except:
                        pass
            
            # Wait 1 second before next transaction
            time.sleep(1.0)
            
    except Exception as e:
        print(f"\n❌ Unexpected error: {e}")
    
    # Final statistics
    if total_sent > 0:
        elapsed = time.time() - start_time
        success_rate = (successful / total_sent) * 100
        avg_tps = total_sent / elapsed
        
        print("\n" + "="*50)
        print("📊 Final Statistics")
        print("="*50)
        print(f"⏱️  Total runtime: {int(elapsed)}s")
        print(f"📤 Total transactions: {total_sent}")
        print(f"✅ Successful: {successful}")
        print(f"❌ Failed: {failed}")
        print(f"📈 Success rate: {success_rate:.1f}%")
        print(f"⚡ Average TPS: {avg_tps:.2f}")
        print(f"💰 Approximate ETH spent: {successful * 0.0005:.4f} ETH")
        
        # Check final balance
        try:
            final_balance = w3.eth.get_balance(dev_address)
            final_eth = w3.from_wei(final_balance, 'ether')
            spent = eth_balance - final_eth
            print(f"💸 ETH spent (actual): {spent:.6f} ETH")
            print(f"💰 Remaining balance: {final_eth:.4f} ETH")
        except:
            pass
        
        print("\n🌐 View all transactions at: http://localhost:3000")
    else:
        print("\n📊 No transactions were sent")

if __name__ == "__main__":
    main()