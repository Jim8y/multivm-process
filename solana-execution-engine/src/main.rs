//! Solana Execution Engine Binary
//!
//! This binary provides a standalone Solana execution engine that can be run
//! as a separate process and communicates with the MultiVM coordinator via IPC.

use solana_execution_engine::simple_engine::SimpleSolanaEngine;

fn main() {
    println!("Solana Execution Engine (Simple)");
    

    // Example usage
    let _engine = SimpleSolanaEngine::new();
    println!("Simple Solana engine created successfully");
}
