//! VM Engine Implementations for Atomic Cross-VM Transactions
//!
//! This module contains concrete implementations of the VmEngine trait
//! for different virtual machines (Solana, Ethereum, etc.)

pub mod ethereum_engine;
pub mod solana_engine;

pub use ethereum_engine::{
    CrossVmContractAbi, CrossVmContractCall, EthereumEngineConfig, EthereumEngineMetrics,
    EthereumLock, EthereumProcessEngine, EthereumRpcClient, EthereumTransactionBuilder,
    LockStatus as EthereumLockStatus,
};
pub use solana_engine::{
    LockStatus as SolanaLockStatus, SolanaEngineConfig, SolanaEngineMetrics, SolanaLock,
    SolanaProcessEngine, SolanaRpcClient, SolanaTransactionBuilder,
};
