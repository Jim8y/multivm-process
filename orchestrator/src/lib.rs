//! Main orchestrator for the MultiVM system

pub mod orchestrator;
pub mod scheduler;
pub mod node_manager;
pub mod vm_manager;
pub mod config;

pub use orchestrator::{Orchestrator, OrchestratorStateMachine};
pub use config::OrchestratorConfig;
pub use scheduler::{Scheduler, SchedulingStrategy};
pub use node_manager::NodeManager;
pub use vm_manager::VmManager;

#[cfg(test)]
mod orchestrator_test;
