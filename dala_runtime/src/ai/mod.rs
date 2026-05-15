//! AI Runtime Layer — integrated inference orchestration for Dala.
//!
//! This module provides first-class runtime support for AI workloads,
//! going far beyond "call a native ML library".  It includes:
//!
//! - **Inference Workers**: Dedicated actor-like workers that execute
//!   ML models with priority scheduling and thermal awareness.
//! - **Tensor Resources**: Managed tensor buffers with zero-copy interop
//!   to GPU/NN accelerators.
//! - **Streaming Pipelines**: Actor-driven streaming inference for
//!   real-time AI (e.g. token streaming, video analysis).
//! - **Model Lifecycle**: Load, cache, version, and unload ML models
//!   with proper resource cleanup.
//!
//! # Architecture
//!
//! ```text
//!  Actor ──► InferenceRequest ──► InferenceWorker
//!                                      │
//!                                      ▼
//!                              ModelRegistry
//!                                      │
//!                          ┌───────────┼───────────┐
//!                          ▼           ▼           ▼
//!                      Model v1    Model v2    Model v3
//!                          │
//!                          ▼
//!                    TensorBuffer ──► GPU/ANE
//! ```

pub mod inference;
pub mod model;
pub mod pipeline;
pub mod tensor;

pub use inference::{InferenceRequest, InferenceResult, InferenceWorker, WorkerConfig};
pub use model::{ModelHandle, ModelId, ModelInfo, ModelRegistry};
pub use pipeline::{Pipeline, PipelineStage, StreamConfig};
pub use tensor::{Tensor, TensorDesc};

/// AI runtime configuration.
#[derive(Debug, Clone)]
pub struct AiConfig {
    /// Maximum number of concurrent inference workers
    pub max_workers: usize,
    /// Default inference priority
    pub default_priority: InferencePriority,
    /// Enable thermal throttling
    pub thermal_throttling: bool,
    /// Maximum GPU memory usage (bytes)
    pub max_gpu_memory: usize,
    /// Model cache directory
    pub model_cache_dir: Option<String>,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            max_workers: 2,
            default_priority: InferencePriority::Normal,
            thermal_throttling: true,
            max_gpu_memory: 256 * 1024 * 1024, // 256 MB
            model_cache_dir: None,
        }
    }
}

/// Inference priority levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InferencePriority {
    /// Background model updates, batch processing
    Background = 0,
    /// Normal inference (e.g. image classification)
    Normal = 1,
    /// User-facing inference (e.g. autocomplete)
    UserFacing = 2,
    /// Real-time inference (e.g. voice activity detection)
    Realtime = 3,
}

/// Errors from the AI runtime.
#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("model not found: {0}")]
    ModelNotFound(String),
    #[error("model load failed: {0}")]
    ModelLoadError(String),
    #[error("inference failed: {0}")]
    InferenceError(String),
    #[error("tensor error: {0}")]
    TensorError(String),
    #[error("GPU error: {0}")]
    GpuError(String),
    #[error("thermal throttling active")]
    ThermalThrottling,
    #[error("out of GPU memory")]
    OutOfGpuMemory,
    #[error("pipeline error: {0}")]
    PipelineError(String),
}
