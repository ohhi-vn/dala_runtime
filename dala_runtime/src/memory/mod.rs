//! Memory regions — hybrid managed memory model for Dala.
//!
//! Instead of a single BEAM-style heap, Dala uses multiple memory
//! regions, each optimized for a specific allocation pattern:
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────┐
//! │  Actor Heap          │ Short-lived BEAM terms, GC'd     │
//! ├──────────────────────┼───────────────────────────────────┤
//! │  Stable Immutable    │ Long-lived, never rescanned      │
//! │  Region (SIR)        │ UI trees, configs, schemas       │
//! ├──────────────────────┼───────────────────────────────────┤
//! │  Binary Region       │ Large binaries, refcounted       │
//! ├──────────────────────┼───────────────────────────────────┤
//! │  Tensor Region       │ GPU/NN buffers, zero-copy        │
//! ├──────────────────────┼───────────────────────────────────┤
//! │  Native Resource     │ Capability-tracked handles       │
//! │  Region              │ Files, sockets, GPU contexts     │
//! ├──────────────────────┼───────────────────────────────────┤
//! │  Arena Allocators    │ Frame-scoped, bulk-free          │
//! └──────────────────────┴───────────────────────────────────┘
//! ```
//!
//! This hybrid model gives the best of both worlds:
//! - BEAM-style GC for short-lived actor messages
//! - Stable regions for long-lived data (no GC pressure)
//! - Native regions for AI/interop (zero-copy, cache-friendly)
//! - Arena allocation for frame-scoped work (no individual frees)

pub mod arena;
pub mod regions;

pub use arena::Arena;
pub use regions::{
    BinaryRegion, MemoryRegion, NativeResourceRegion, RegionId, StableImmutableRegion,
    TensorRegion,
};

/// Configuration for the memory subsystem.
#[derive(Debug, Clone)]
pub struct MemoryConfig {
    /// Initial actor heap size (in words)
    pub initial_heap_size: usize,
    /// Maximum actor heap size (in words)
    pub max_heap_size: usize,
    /// Stable Immutable Region size (in bytes)
    pub sir_size: usize,
    /// Binary region size (in bytes)
    pub binary_region_size: usize,
    /// Tensor region size (in bytes)
    pub tensor_region_size: usize,
    /// Arena chunk size (in bytes)
    pub arena_chunk_size: usize,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            initial_heap_size: 233,
            max_heap_size: 16_384,
            sir_size: 1024 * 1024,       // 1 MB
            binary_region_size: 4 * 1024 * 1024, // 4 MB
            tensor_region_size: 16 * 1024 * 1024, // 16 MB
            arena_chunk_size: 64 * 1024,  // 64 KB
        }
    }
}
