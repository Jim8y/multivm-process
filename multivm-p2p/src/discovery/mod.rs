//! Node discovery and gossip components

pub mod discovery;
pub mod gossip;

pub use discovery::{Discovery, DiscoveryConfig, DiscoveryEvent};
pub use gossip::{GossipConfig, GossipEvent, GossipManager};