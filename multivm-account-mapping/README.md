# MultiVM Account Mapping

Cross-VM account management and binding system for the MultiVM blockchain execution platform.

## Overview

The MultiVM Account Mapping module provides a sophisticated system for managing account relationships across different blockchain virtual machines (Solana VM and Ethereum VM). It enables seamless cross-VM operations through cryptographically secure account binding.

## Features

### 🔗 Account Binding
- **Automatic Binding**: Single-account MultiVM creation (A → M)
- **Cross-VM Binding**: Link accounts across VMs (A ↔ M ↔ B)
- **Proof Validation**: Cryptographic signature verification
- **Bidirectional Mapping**: Efficient lookup in both directions

### ✨ Special Transactions
- **Cross-VM Transfers**: Lock/mint/burn/unlock pattern for asset transfers
- **Binding Updates**: Modify binding configurations
- **Account Unbinding**: Safe removal of account bindings
- **Transaction Planning**: Multi-step execution with cost estimation

### 🛡️ Security
- **Ed25519 Signatures**: Solana account verification
- **ECDSA Signatures**: Ethereum account verification
- **Proof Requirements**: All bindings require cryptographic proof
- **Safety Checks**: Comprehensive validation before operations

## Architecture

```
┌─────────────────────────────────────────┐
│          Account Mapping Layer          │
├─────────────────────────────────────────┤
│   ┌─────────────┐    ┌─────────────┐   │
│   │   Storage   │    │ Validation  │   │
│   │  Abstraction│    │   Engine    │   │
│   └─────────────┘    └─────────────┘   │
├─────────────────────────────────────────┤
│   ┌─────────────┐    ┌─────────────┐   │
│   │   Special   │    │   Address   │   │
│   │Transactions │    │    Types    │   │
│   └─────────────┘    └─────────────┘   │
└─────────────────────────────────────────┘
```

## Usage

### Basic Account Binding

```rust
use multivm_account_mapping::*;

// Create storage layer
let storage = MemoryStorage::new();

// Create account addresses
let solana_addr = AccountAddress::Solana(SolanaAddress([1u8; 32]));
let ethereum_addr = AccountAddress::Ethereum(EthereumAddress([2u8; 20]));

// Auto-bind first account
let multivm_id = storage.add_auto_binding(solana_addr).await?;

// Create binding proof
let proof = BindingProof {
    account: ethereum_addr.clone(),
    proof_type: ProofType::Signature {
        message: b"bind_account".to_vec(),
        signature: signature_bytes,
    },
    proof_data: vec![],
    timestamp: SystemTime::now(),
};

// Cross-bind second account
storage.add_cross_binding(multivm_id, ethereum_addr, proof).await?;
```

### Cross-VM Transfer

```rust
// Create special transaction processor
let processor = SpecialTransactionProcessor::new(mapping_layer);

// Execute cross-VM transfer
let tx = SpecialTransaction::CrossVmTransfer {
    from: multivm_account_123,
    to: multivm_account_456,
    amount: 1_000_000,
    asset_type: AssetType::Native,
    memo: Some("Cross-VM transfer".to_string()),
};

let result = processor.process_transaction(tx).await?;
```

## Implementation Status

✅ **Complete and Production Ready**
- Full account binding implementation
- Complete special transaction processing
- Comprehensive validation framework
- Cross-VM transfer execution logic
- Account management operations

## API

### Core Traits

- `AccountMappingLayer` - Main interface for account operations
- `AccountBindingValidator` - Validation logic for bindings
- `SpecialTransactionProcessor` - Special transaction handling

### Key Types

- `MultivmAccountId` - Unique identifier for MultiVM accounts
- `AccountAddress` - VM-specific account addresses
- `AccountBinding` - Complete binding information
- `SpecialTransaction` - Cross-VM operation types
- `BindingProof` - Cryptographic ownership proofs

## Testing

```bash
# Run unit tests
cargo test -p multivm-account-mapping

# Run with logging
RUST_LOG=debug cargo test -p multivm-account-mapping
```

## License

Licensed under either Apache 2.0 or MIT license at your option.