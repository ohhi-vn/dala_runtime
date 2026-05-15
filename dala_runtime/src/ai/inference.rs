//! Inference worker — dedicated actor-like worker for ML inference.
//!
//! Inference workers are scheduled with QoS awareness and can be
//! throttled based on thermal state.  They communicate with actors
//! via typed message passing.

use crate::ai::{AiError, InferencePriority, Tensor, TensorDesc};
use dala_ir::type_system::MessagePriority;

/// An inference request sent to a worker.
#[derive(Debug, Clone)]
pub struct InferenceRequest {
    /// The model to run
    pub model_id: u64,
    /// Input tensors
    pub inputs: Vec<TensorDesc>,
    /// Priority for scheduling
    pub priority: InferencePriority,
    /// Timeout in milliseconds (0 = no timeout)
    pub timeout_ms: u32,
}

/// The result of an inference operation.
#[derive(Debug)]
pub struct InferenceResult {
    /// Output tensors
    pub outputs: Vec<Tensor>,
    /// Time taken in microseconds
    pub elapsed_us: u64,
    /// Whether the result was cached
    pub cached: bool,
}

/// Configuration for an inference worker.
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    /// Maximum concurrent requests
    pub max_concurrent: usize,
    /// Enable result caching
    pub enable_cache: bool,
    /// Thermal throttling threshold (0.0 - 1.0)
    pub thermal_threshold: f32,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 2,
            enable_cache: true,
            thermal_threshold: 0.8,
        }
    }
}

/// An inference worker — processes ML inference requests.
pub struct InferenceWorker {
    config: WorkerConfig,
    /// Current number of in-flight requests
    active_requests: usize,
    /// Whether thermal throttling is active
    throttled: bool,
}

impl InferenceWorker {
    /// Create a new inference worker.
    pub fn new(config: WorkerConfig) -> Self {
        Self {
            config,
            active_requests: 0,
            throttled: false,
        }
    }

    /// Submit an inference request.
    pub fn submit(&mut self, request: InferenceRequest) -> Result<InferenceResult, AiError> {
        if self.throttled && request.priority < InferencePriority::Realtime {
            return Err(AiError::ThermalThrottling);
        }

        if self.active_requests >= self.config.max_concurrent {
            return Err(AiError::InferenceError("worker overloaded".into()));
        }

        self.active_requests += 1;

        // In a full implementation, this would:
        // 1. Look up the model in the registry
        // 2. Prepare input tensors
        // 3. Execute on GPU/ANE
        // 4. Collect outputs

        self.active_requests -= 1;

        // Placeholder result
        Ok(InferenceResult {
            outputs: Vec::new(),
            elapsed_us: 0,
            cached: false,
        })
    }

    /// Check if the worker is available.
    pub fn is_available(&self) -> bool {
        self.active_requests < self.config.max_concurrent && !self.throttled
    }

    /// Set thermal throttling state.
    pub fn set_throttled(&mut self, throttled: bool) {
        self.throttled = throttled;
    }

    /// Get the BEAM message priority for this inference priority.
    pub fn beam_priority(priority: InferencePriority) -> MessagePriority {
        match priority {
            InferencePriority::Background => MessagePriority::Low,
            InferencePriority::Normal => MessagePriority::Normal,
            InferencePriority::UserFacing => MessagePriority::High,
            InferencePriority::Realtime => MessagePriority::Critical,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_creation() {
        let worker = InferenceWorker::new(WorkerConfig::default());
        assert!(worker.is_available());
    }

    #[test]
    fn test_thermal_throttling() {
        let mut worker = InferenceWorker::new(WorkerConfig::default());
        worker.set_throttled(true);

        let req = InferenceRequest {
            model_id: 1,
            inputs: vec![],
            priority: InferencePriority::Normal,
            timeout_ms: 1000,
        };
        assert!(matches!(worker.submit(req), Err(AiError::ThermalThrottling)));

        // Realtime requests should still go through
        let req = InferenceRequest {
            model_id: 1,
            inputs: vec![],
            priority: InferencePriority::Realtime,
            timeout_ms: 1000,
        };
        assert!(worker.submit(req).is_ok());
    }

    #[test]
    fn test_beam_priority_mapping() {
        assert_eq!(
            InferenceWorker::beam_priority(InferencePriority::Realtime),
            MessagePriority::Critical
        );
        assert_eq!(
            InferenceWorker::beam_priority(InferencePriority::Background),
            MessagePriority::Low
        );
    }
}
