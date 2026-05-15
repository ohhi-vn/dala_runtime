//! Streaming inference pipelines — actor-driven real-time AI.
//!
//! Pipelines chain multiple inference stages together with actor-style
//! message passing between stages.  This enables real-time AI workloads
//! like token streaming, video analysis, and sensor fusion.

use crate::ai::{AiError, InferencePriority, InferenceRequest, InferenceResult, Tensor};
use std::collections::VecDeque;

/// Configuration for a streaming pipeline.
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// Maximum buffer size between stages
    pub buffer_size: usize,
    /// Inference priority
    pub priority: InferencePriority,
    /// Whether to drop frames when the pipeline is full
    pub drop_on_full: bool,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            buffer_size: 16,
            priority: InferencePriority::Normal,
            drop_on_full: true,
        }
    }
}

/// A single stage in a pipeline.
#[derive(Debug)]
pub enum PipelineStage {
    /// Preprocessing (e.g. image resize, normalization)
    Preprocess {
        /// Stage name
        name: String,
    },
    /// Model inference
    Inference {
        /// Model ID
        model_id: u64,
    },
    /// Postprocessing (e.g. NMS, decoding)
    Postprocess {
        /// Stage name
        name: String,
    },
    /// Custom processing step
    Custom {
        /// Stage name
        name: String,
    },
}

/// A streaming inference pipeline.
pub struct Pipeline {
    /// Pipeline stages
    stages: Vec<PipelineStage>,
    /// Inter-stage buffers
    buffers: Vec<VecDeque<Tensor>>,
    /// Configuration
    config: StreamConfig,
    /// Whether the pipeline is running
    running: bool,
}

impl Pipeline {
    /// Create a new pipeline with the given stages.
    pub fn new(stages: Vec<PipelineStage>, config: StreamConfig) -> Self {
        let buffer_count = stages.len().saturating_sub(1).max(1);
        let buffers = (0..buffer_count)
            .map(|_| VecDeque::with_capacity(config.buffer_size))
            .collect();

        Self {
            stages,
            buffers,
            config,
            running: false,
        }
    }

    /// Start the pipeline.
    pub fn start(&mut self) {
        self.running = true;
    }

    /// Stop the pipeline.
    pub fn stop(&mut self) {
        self.running = false;
        for buf in &mut self.buffers {
            buf.clear();
        }
    }

    /// Push input into the first stage.
    pub fn push_input(&mut self, input: Tensor) -> Result<(), AiError> {
        if !self.running {
            return Err(AiError::PipelineError("pipeline not running".into()));
        }

        if self.buffers.is_empty() {
            return Err(AiError::PipelineError("single-stage pipeline".into()));
        }

        let buf = &mut self.buffers[0];
        if buf.len() >= self.config.buffer_size {
            if self.config.drop_on_full {
                buf.pop_front(); // Drop oldest
            } else {
                return Err(AiError::PipelineError("buffer full".into()));
            }
        }
        buf.push_back(input);
        Ok(())
    }

    /// Process one item through the pipeline.
    pub fn process_one(&mut self) -> Result<Option<Tensor>, AiError> {
        if !self.running {
            return Ok(None);
        }

        // In a full implementation, this would:
        // 1. Take input from the first buffer
        // 2. Run each stage sequentially
        // 3. Pass output to the next buffer
        // 4. Return the final output

        Ok(None)
    }

    /// Get the number of stages.
    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    /// Check if the pipeline is running.
    pub fn is_running(&self) -> bool {
        self.running
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_creation() {
        let stages = vec![
            PipelineStage::Preprocess { name: "resize".into() },
            PipelineStage::Inference { model_id: 1 },
            PipelineStage::Postprocess { name: "nms".into() },
        ];
        let pipeline = Pipeline::new(stages, StreamConfig::default());
        assert_eq!(pipeline.stage_count(), 3);
        assert!(!pipeline.is_running());
    }

    #[test]
    fn test_pipeline_start_stop() {
        let stages = vec![
            PipelineStage::Inference { model_id: 1 },
        ];
        let mut pipeline = Pipeline::new(stages, StreamConfig::default());
        pipeline.start();
        assert!(pipeline.is_running());
        pipeline.stop();
        assert!(!pipeline.is_running());
    }
}
