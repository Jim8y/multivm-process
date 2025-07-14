# MultiVM Tools

This directory contains utility tools for MultiVM development and testing.

## Directory Structure

### Transaction Generators (`transaction-generators/`)
Tools for generating test transactions and stress testing the network:

- **`send-transactions-continuous.py`** - Sends 1 transaction per second continuously
- **`send-varied-transactions.py`** - Sends mixed transaction types (transfers, contract deployments, calls)  
- **`send-fast-transactions.py`** - High-throughput batch transaction generator

### Account Management (`account-management/`)
Tools for managing validator accounts and keys:

- **`generate-validator-accounts.py`** - Generates complete validator account sets with Ethereum, Ed25519, and Solana keys

## Usage

### Transaction Generators

```bash
# Send continuous transactions (1 per second)
cd tools/transaction-generators
python3 send-transactions-continuous.py

# Send varied transaction types
python3 send-varied-transactions.py

# High-throughput stress testing
python3 send-fast-transactions.py
```

### Account Management

```bash
# Generate validator accounts for testnet
cd tools/account-management
python3 generate-validator-accounts.py
```

## Requirements

All tools require:
- Python 3.8+
- web3 library (`pip install web3`)
- eth-account library (`pip install eth-account`)

## Integration

These tools are designed to work with:
- The MultiVM testnet (started via `scripts/start-testnet.sh`)
- The blockchain explorer at `http://localhost:3000`
- The production testnet configuration in `testnet/`