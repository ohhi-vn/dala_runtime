//! Tensor resource management for AI workloads.
//!
//! Tensors are the fundamental data type for ML inference.  This module
//! provides a managed tensor type that tracks:
//!
//! - Shape and dtype
//! - Memory region (host, GPU, ANE)
//! - Reference counting for shared buffers
//! - Zero-copy views into existing data

use crate::ai::AiError;
use dala_ir::type_system::TensorDtype;

/// Tensor descriptor — shape and dtype metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TensorDesc {
    /// Element data type
    pub dtype: TensorDtype,
    /// Shape dimensions
    pub shape: Vec<u64>,
    /// Total number of elements
    pub num_elements: u64,
    /// Total size in bytes
    pub size_bytes: u64,
}

impl TensorDesc {
    /// Create a new tensor descriptor.
    pub fn new(dtype: TensorDtype, shape: Vec<u64>) -> Self {
        let num_elements: u64 = shape.iter().product();
        let elem_size = tensor_dtype_size_bytes(&dtype);
        Self {
            dtype,
            shape,
            num_elements,
            size_bytes: num_elements * elem_size as u64,
        }
    }

    /// Create a 1D tensor (vector).
    pub fn vector(dtype: TensorDtype, len: u64) -> Self {
        Self::new(dtype, vec![len])
    }

    /// Create a 2D tensor (matrix).
    pub fn matrix(dtype: TensorDtype, rows: u64, cols: u64) -> Self {
        Self::new(dtype, vec![rows, cols])
    }

    /// Create a 4D image tensor (NCHW format).
    pub fn image(dtype: TensorDtype, batch: u64, channels: u64, height: u64, width: u64) -> Self {
        Self::new(dtype, vec![batch, channels, height, width])
    }

    /// Check if this is a scalar (0-dimensional).
    pub fn is_scalar(&self) -> bool {
        self.shape.is_empty() || self.shape.iter().all(|&d| d == 1)
    }

    /// Get the number of dimensions.
    pub fn ndim(&self) -> usize {
        self.shape.len()
    }
}

/// Tensor memory location.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TensorLocation {
    /// Host (CPU) memory
    Host,
    /// GPU memory (Metal, Vulkan, CUDA)
    Gpu,
    /// Apple Neural Engine
    Ane,
}

/// A managed tensor resource.
#[derive(Debug)]
pub struct Tensor {
    /// Shape and dtype metadata
    pub desc: TensorDesc,
    /// Backing data pointer
    data: *mut u8,
    /// Total allocated size
    size: usize,
    /// Memory location
    location: TensorLocation,
    /// Reference count
    refcount: u32,
}

impl Tensor {
    /// Create a new tensor with uninitialized host memory.
    pub fn new(desc: TensorDesc) -> Result<Self, AiError> {
        let size = desc.size_bytes as usize;
        let layout = std::alloc::Layout::from_size_align(size, 64)
            .map_err(|e| AiError::TensorError(e.to_string()))?;
        let data = unsafe { std::alloc::alloc(layout) };
        if data.is_null() {
            return Err(AiError::TensorError("allocation failed".into()));
        }
        Ok(Self {
            desc,
            data,
            size,
            location: TensorLocation::Host,
            refcount: 1,
        })
    }

    /// Create a tensor from existing data (zero-copy view).
    pub fn from_data(desc: TensorDesc, data: *mut u8) -> Self {
        Self {
            size: desc.size_bytes as usize,
            desc,
            data,
            location: TensorLocation::Host,
            refcount: 1,
        }
    }

    /// Get the data pointer.
    pub fn data_ptr(&self) -> *mut u8 {
        self.data
    }

    /// Get the data as a typed slice.
    pub fn as_slice<T>(&self) -> &[T] {
        unsafe {
            std::slice::from_raw_parts(self.data as *const T, self.size / std::mem::size_of::<T>())
        }
    }

    /// Get the data as a mutable typed slice.
    pub fn as_mut_slice<T>(&mut self) -> &mut [T] {
        unsafe {
            std::slice::from_raw_parts_mut(
                self.data as *mut T,
                self.size / std::mem::size_of::<T>(),
            )
        }
    }

    /// Get the memory location.
    pub fn location(&self) -> TensorLocation {
        self.location
    }

    /// Increment the reference count.
    pub fn incref(&mut self) {
        self.refcount += 1;
    }

    /// Decrement the reference count. Returns true if the tensor should be freed.
    pub fn decref(&mut self) -> bool {
        self.refcount -= 1;
        self.refcount == 0
    }
}

/// Get the size of a TensorDtype in bytes.
pub fn tensor_dtype_size_bytes(dtype: &TensorDtype) -> usize {
    match dtype {
        TensorDtype::F32 | TensorDtype::I32 => 4,
        TensorDtype::F16 => 2,
        TensorDtype::F64 | TensorDtype::I64 => 8,
        TensorDtype::U8 | TensorDtype::Bool => 1,
    }
}

impl Drop for Tensor {
    fn drop(&mut self) {
        if self.refcount <= 1 && !self.data.is_null() {
            unsafe {
                let layout = std::alloc::Layout::from_size_align_unchecked(self.size, 64);
                std::alloc::dealloc(self.data, layout);
            }
        }
    }
}

// SAFETY: Tensor data is owned and can be sent between threads.
unsafe impl Send for Tensor {}
unsafe impl Sync for Tensor {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tensor_desc() {
        let desc = TensorDesc::image(TensorDtype::F32, 1, 3, 224, 224);
        assert_eq!(desc.ndim(), 4);
        assert_eq!(desc.num_elements, 1 * 3 * 224 * 224);
        assert_eq!(desc.size_bytes, 1 * 3 * 224 * 224 * 4);
    }

    #[test]
    fn test_tensor_allocation() {
        let desc = TensorDesc::vector(TensorDtype::F32, 100);
        let tensor = Tensor::new(desc).unwrap();
        assert_eq!(tensor.size, 400);
        assert_eq!(tensor.location(), TensorLocation::Host);
    }

    #[test]
    fn test_dtype_sizes() {
        assert_eq!(tensor_dtype_size_bytes(&TensorDtype::F32), 4);
        assert_eq!(tensor_dtype_size_bytes(&TensorDtype::F16), 2);
        assert_eq!(tensor_dtype_size_bytes(&TensorDtype::F64), 8);
        assert_eq!(tensor_dtype_size_bytes(&TensorDtype::U8), 1);
    }
}
