//! Core P2P networking components

pub mod manager;
pub mod network;

pub use manager::{P2PManager, P2PManagerStats};
pub use network::P2PNetwork;