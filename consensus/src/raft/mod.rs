//! Raft consensus algorithm implementation

mod config;
mod core;
mod election;
mod replication;
mod state;

pub use config::Config;
pub use core::{RaftNode, Command, Response};

#[cfg(test)]
mod state_test;