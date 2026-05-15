//! Model lifecycle management — load, cache, version, and unload ML models.

use crate::ai::AiError;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Unique model identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModelId(pub u64);

/// A handle to a loaded model.
#[derive(Debug, Clone)]
pub struct ModelHandle {
    pub id: ModelId,
    pub info: ModelInfo,
    /// Reference-counted inner data
    inner: Arc<ModelInner>,
}

#[derive(Debug)]
struct ModelInner {
    /// Model weights/data pointer
    data: *mut u8,
    /// Size in bytes
    size: usize,
}

/// Model metadata.
#[derive(Debug, Clone)]
pub struct ModelInfo {
    /// Model name
    pub name: String,
    /// Model version
    pub version: String,
    /// Input tensor descriptions
    pub inputs: Vec<String>,
    /// Output tensor descriptions
    pub outputs: Vec<String>,
    /// Estimated memory usage (bytes)
    pub memory_usage: usize,
}

/// Model registry — manages loaded models with LRU eviction.
pub struct ModelRegistry {
    models: RwLock<HashMap<ModelId, ModelHandle>>,
    next_id: std::sync::atomic::AtomicU64,
    /// Maximum total model memory (bytes)
    max_memory: usize,
    /// Current memory usage
    current_memory: std::sync::atomic::AtomicUsize,
}

impl ModelRegistry {
    /// Create a new model registry.
    pub fn new(max_memory: usize) -> Self {
        Self {
            models: RwLock::new(HashMap::new()),
            next_id: std::sync::atomic::AtomicU64::new(1),
            max_memory,
            current_memory: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Load a model from a file path.
    pub fn load_model(&self, name: &str, path: &str) -> Result<ModelId, AiError> {
        // In a full implementation, this would:
        // 1. Read the model file
        // 2. Parse the model format (ONNX, CoreML, etc.)
        // 3. Allocate GPU memory for weights
        // 4. Register in the registry

        let id = ModelId(self.next_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst));
        let info = ModelInfo {
            name: name.to_string(),
            version: "1.0".to_string(),
            inputs: vec![],
            outputs: vec![],
            memory_usage: 0,
        };

        let handle = ModelHandle {
            id,
            info,
            inner: Arc::new(ModelInner {
                data: std::ptr::null_mut(),
                size: 0,
            }),
        };

        self.models.write().unwrap().insert(id, handle);
        Ok(id)
    }

    /// Get a model handle.
    pub fn get(&self, id: ModelId) -> Option<ModelHandle> {
        self.models.read().unwrap().get(&id).cloned()
    }

    /// Unload a model.
    pub fn unload(&self, id: ModelId) -> bool {
        let mut models = self.models.write().unwrap();
        if let Some(handle) = models.remove(&id) {
            self.current_memory.fetch_sub(
                handle.info.memory_usage,
                std::sync::atomic::Ordering::Relaxed,
            );
            true
        } else {
            false
        }
    }

    /// Get the number of loaded models.
    pub fn len(&self) -> usize {
        self.models.read().unwrap().len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.models.read().unwrap().is_empty()
    }
}

impl Drop for ModelInner {
    fn drop(&mut self) {
        if !self.data.is_null() && self.size > 0 {
            unsafe {
                let layout = std::alloc::Layout::from_size_align_unchecked(self.size, 64);
                std::alloc::dealloc(self.data, layout);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_registry() {
        let registry = ModelRegistry::new(1024 * 1024);
        assert!(registry.is_empty());

        let id = registry.load_model("test", "/tmp/test.model").unwrap();
        assert_eq!(registry.len(), 1);

        let handle = registry.get(id).unwrap();
        assert_eq!(handle.info.name, "test");

        assert!(registry.unload(id));
        assert!(registry.is_empty());
    }
}
