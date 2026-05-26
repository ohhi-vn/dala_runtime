//! Dala Runtime - BEAM-compatible runtime backend
//!
//! This crate implements the core BEAM runtime: process management,
//! scheduler, garbage collector, and term representation.
//! It serves as the execution engine for AOT-compiled BEAM code.

#![warn(missing_docs)]
#![deny(clippy::all)]

pub mod ai;
pub mod atom;
pub mod bif;
pub mod code;
pub mod exception;
pub mod gc;
pub mod mailbox;
pub mod memory;
pub mod port;
pub mod process;
pub mod scheduler;
pub mod term;
pub mod trap;

// Edge case test modules
#[cfg(test)]
mod atom_edge_tests;
#[cfg(test)]
mod bif_edge_tests;
#[cfg(test)]
mod code_edge_tests;
#[cfg(test)]
mod exception_edge_tests;
#[cfg(test)]
mod gc_edge_tests;
#[cfg(test)]
mod mailbox_edge_tests;
#[cfg(test)]
mod memory_edge_tests;
#[cfg(test)]
mod process_edge_tests;
#[cfg(test)]
mod scheduler_edge_tests;
#[cfg(test)]
mod term_edge_tests;

// Re-exports
pub use atom::AtomTable;
pub use exception::{Exception, Reason};
pub use process::{Process, ProcessFlags};
pub use scheduler::{Scheduler, SchedulerMessage};
pub use term::Term;
pub use trap::TrapFrame as Trap;

/// Runtime configuration
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Number of scheduler threads
    pub scheduler_count: usize,
    /// Initial heap size per process (in words)
    pub initial_heap_size: usize,
    /// Maximum heap size per process (in words)
    pub max_heap_size: usize,
    /// Maximum reductions before yielding
    pub reductions_per_yield: u32,
    /// Enable debug GC tracing
    pub debug_gc: bool,
    /// Execution mode
    pub execution_mode: ExecutionMode,
}

/// Execution mode for the runtime
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExecutionMode {
    /// Pure interpreter (baseline)
    Interpreted,
    /// Mixed: AOT + interpreter fallback
    Mixed,
    /// Full AOT (no JIT, no interpreter)
    Aot,
}

impl Default for ExecutionMode {
    fn default() -> Self {
        Self::Mixed
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            scheduler_count: num_cpus::get(),
            initial_heap_size: 233,
            max_heap_size: 16_384,
            reductions_per_yield: 2_000,
            debug_gc: false,
            execution_mode: ExecutionMode::default(),
        }
    }
}

/// Initialize the runtime with the given configuration.
/// Returns the global runtime instance.
pub fn init(config: RuntimeConfig) -> Result<(), RuntimeError> {
    Scheduler::global_init(config)
}

/// Runtime error types
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("scheduler error: {0}")]
    SchedulerError(String),
    #[error("memory allocation failed: {0}")]
    AllocationError(String),
    #[error("beam loader error: {0}")]
    BeamLoadError(String),
    #[error("codegen error: {0}")]
    CodegenError(String),
    #[error("link error: {0}")]
    LinkError(String),
    #[error("process crashed: {0}")]
    ProcessCrash(String),
}
