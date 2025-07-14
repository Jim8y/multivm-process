
🎯 STEP-BY-STEP VALIDATOR FUNDING GUIDE
=======================================

CURRENT SITUATION:
- ✅ MultiVM testnet is running with 7 validators
- ✅ Dev account has 1,000,000 ETH available  
- ✅ Blockchain explorer is running at http://localhost:3000
- ❌ Validator accounts need funding for real transactions

SOLUTION: Use MetaMask to Fund Validators
-----------------------------------------

STEP 1: Setup MetaMask
1. Open MetaMask browser extension
2. Click "Add Network" 
3. Add custom network with these settings:
   - Network Name: MultiVM Testnet
   - RPC URL: http://localhost:8545
   - Chain ID: 1337
   - Currency Symbol: ETH
   - Block Explorer: http://localhost:3000

STEP 2: Import Dev Account
1. In MetaMask, click "Import Account"
2. Select "Private Key"
3. Enter: ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
4. Confirm - you should see 1,000,000 ETH balance

STEP 3: Fund Each Validator (Send 5 ETH to each)

1. Send to validator-1:
   Address: 0xbad43e401b75a5aeba33835d3481f92b271231c4
   Amount: 5 ETH
   Gas: 21000 (default)

2. Send to validator-2:
   Address: 0xd9bd2d158197500c05d9f5fd6e54429e39bcff16
   Amount: 5 ETH
   Gas: 21000 (default)

3. Send to validator-3:
   Address: 0xdbc75afeb7892fd54877079f39602366fd3490a9
   Amount: 5 ETH
   Gas: 21000 (default)

4. Send to validator-4:
   Address: 0xfda0011aa977ae29b97ef3d53ceecc852886d54d
   Amount: 5 ETH
   Gas: 21000 (default)

5. Send to validator-5:
   Address: 0xa400dfda339ace504a92df76dcbe775b5d4dfa10
   Amount: 5 ETH
   Gas: 21000 (default)

6. Send to validator-6:
   Address: 0x3d5bcc755407a22baa663a0cca472874961971a0
   Amount: 5 ETH
   Gas: 21000 (default)

7. Send to validator-7:
   Address: 0x20dc3415cb5af3e799070f4a94f3d6fed094988e
   Amount: 5 ETH
   Gas: 21000 (default)

STEP 4: Verify Funding
1. Wait for all transactions to confirm
2. Run: python3 fund-accounts.py
3. All validators should show ✅ with 5+ ETH

STEP 5: Generate Transactions
1. Run: python3 demo-transactions.py
2. View activity: http://localhost:3000
3. Check blockchain explorer for real transactions

ALTERNATIVE: Automated Funding
------------------------------
If you have web3 Python library installed:
pip install web3 eth-account
python3 web3-fund-validators.py

TROUBLESHOOTING:
- If MetaMask shows 0 ETH: Check network is set to MultiVM Testnet
- If transactions fail: Increase gas price to 25-30 Gwei
- If RPC errors: Restart testnet with 'docker compose restart'
