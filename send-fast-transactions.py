#!/usr/bin/env python3
"""
Send transactions as fast as possible to stress test the network
Batches multiple transactions per second
"""

import json
import time
import random
import asyncio
import signal
from web3 import Web3
from eth_account import Account
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime

# Global flag for graceful shutdown
running = True

def signal_handler(sig, frame):
    global running
    print("\n\n🛑 Stopping fast transaction generator...")
    running = False

def send_transaction(w3, dev_account, to_address, amount, nonce):
    """Send a single transaction"""
    try:
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
        return True, tx_hash.hex()
    except Exception as e:
        return False, str(e)

async def main():
    global running
    
    # Register signal handler
    signal.signal(signal.SIGINT, signal_handler)
    
    # Connect to MultiVM
    w3 = Web3(Web3.HTTPProvider("http://localhost:8545"))
    if not w3.is_connected():
        print("❌ Cannot connect to MultiVM RPC")
        return
    
    print("⚡ Fast Transaction Generator")
    print("============================")
    print(f"✅ Connected to chain ID: {w3.eth.chain_id}")
    print("🚀 Sending transactions as fast as possible")
    print("📍 Target: Multiple transactions per second")
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
    
    if eth_balance < 1:
        print("⚠️  Warning: Low balance for fast sending!")
    
    # Create test addresses (using deterministic addresses for speed)
    test_addresses = []
    for i in range(10):
        private_key = f"0x{'0' * 63}{i+1}"
        account = Account.from_key(private_key)
        test_addresses.append(account.address)
    
    print(f"📋 Using {len(test_addresses)} test addresses")
    print("🌐 Monitor at: http://localhost:3000")
    print("\n" + "="*50 + "\n")
    
    # Statistics
    nonce = w3.eth.get_transaction_count(dev_address)
    total_sent = 0
    successful = 0
    failed = 0
    start_time = time.time()
    last_report_time = start_time
    
    # Thread pool for parallel sending
    executor = ThreadPoolExecutor(max_workers=5)
    
    try:
        while running:
            batch_start = time.time()
            futures = []
            
            # Send batch of 5 transactions
            batch_size = 5
            for i in range(batch_size):
                to_address = random.choice(test_addresses)
                amount = 0.0001  # Small fixed amount for speed
                
                future = executor.submit(
                    send_transaction,
                    w3, dev_account, to_address, amount, nonce + i
                )
                futures.append(future)
            
            # Collect results
            batch_success = 0
            batch_failed = 0
            
            for i, future in enumerate(futures):
                success, result = future.result()
                total_sent += 1
                
                if success:
                    successful += 1
                    batch_success += 1
                else:
                    failed += 1
                    batch_failed += 1
                    # Don't print individual failures to maintain speed
            
            # Update nonce based on successes
            nonce += batch_success
            
            # Print batch result
            batch_time = time.time() - batch_start
            batch_tps = batch_size / batch_time
            
            timestamp = datetime.now().strftime("%H:%M:%S")
            print(f"[{timestamp}] ⚡ Batch: {batch_success}/{batch_size} success | "
                  f"Batch TPS: {batch_tps:.1f} | Total: {successful}/{total_sent}")
            
            # Detailed report every 10 seconds
            current_time = time.time()
            if current_time - last_report_time >= 10:
                elapsed = current_time - start_time
                overall_tps = total_sent / elapsed
                success_rate = (successful / total_sent) * 100 if total_sent > 0 else 0
                
                print(f"\n📊 10-Second Report:")
                print(f"   Total sent: {total_sent}")
                print(f"   Successful: {successful} ({success_rate:.1f}%)")
                print(f"   Failed: {failed}")
                print(f"   Overall TPS: {overall_tps:.2f}")
                print(f"   Runtime: {int(elapsed)}s\n")
                
                last_report_time = current_time
            
            # Small delay to prevent overwhelming the network
            await asyncio.sleep(0.2)  # 200ms between batches = ~25 tx/sec theoretical max
            
    except Exception as e:
        print(f"\n❌ Error: {e}")
    finally:
        executor.shutdown(wait=True)
    
    # Final statistics
    elapsed = time.time() - start_time
    
    if total_sent > 0:
        success_rate = (successful / total_sent) * 100
        overall_tps = total_sent / elapsed
        
        print("\n" + "="*50)
        print("📊 Final Statistics")
        print("="*50)
        print(f"⏱️  Total runtime: {int(elapsed)}s")
        print(f"📤 Total transactions sent: {total_sent}")
        print(f"✅ Successful: {successful}")
        print(f"❌ Failed: {failed}")
        print(f"📈 Success rate: {success_rate:.1f}%")
        print(f"⚡ Average TPS: {overall_tps:.2f}")
        print(f"🚀 Peak batch TPS: ~{batch_size/0.2:.1f}")
        
        # Estimate ETH spent
        eth_spent = successful * 0.0001 * 1.00042  # amount + gas
        print(f"\n💸 Estimated ETH spent: {eth_spent:.6f} ETH")
        
        # Check final balance
        try:
            final_balance = w3.eth.get_balance(dev_address)
            final_eth = w3.from_wei(final_balance, 'ether')
            actual_spent = eth_balance - final_eth
            print(f"💸 Actual ETH spent: {actual_spent:.6f} ETH")
            print(f"💰 Remaining balance: {final_eth:.4f} ETH")
        except:
            pass
        
        print("\n🌐 View all transactions at: http://localhost:3000")
        print("📊 Check block production rate in the explorer")

if __name__ == "__main__":
    asyncio.run(main())