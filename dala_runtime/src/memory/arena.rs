//! Arena allocator — fast, frame-scoped bulk-free allocation.
//!
//! Arenas are used for temporary allocations within a single execution
//! frame (e.g. during a function call or message handler).  Individual
//! frees are not supported; the entire arena is reset at once.
//!
//! This is critical for mobile performance: no fragmentation, no
//! individual free overhead, and excellent cache locality.

use std::alloc::{Layout, alloc, dealloc};
use std::cell::RefCell;
use std::ptr::NonNull;

/// A single chunk in the arena.
#[derive(Debug)]
struct Chunk {
    ptr: NonNull<u8>,
    size: usize,
    used: usize,
}

impl Chunk {
    fn new(size: usize) -> Self {
        let layout = Layout::from_size_align(size, std::mem::size_of::<usize>()).unwrap();
        let ptr = unsafe { NonNull::new(alloc(layout)).expect("arena alloc failed") };
        Self { ptr, size, used: 0 }
    }

    fn remaining(&self) -> usize {
        self.size - self.used
    }

    /// Try to allocate `size` bytes from this chunk.
    unsafe fn alloc(&mut self, size: usize) -> *mut u8 {
        if size > self.remaining() {
            return std::ptr::null_mut();
        }
        let ptr = self.ptr.as_ptr().add(self.used);
        self.used += size;
        ptr
    }
}

impl Drop for Chunk {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(self.size, std::mem::size_of::<usize>()).unwrap();
        unsafe { dealloc(self.ptr.as_ptr(), layout) };
    }
}

/// A fast arena allocator.
///
/// Allocations are bump-pointer within the current chunk.  When the
/// current chunk is full, a new chunk is allocated (twice the size of
/// the previous one, up to a maximum).
#[derive(Debug)]
pub struct Arena {
    chunks: RefCell<Vec<Chunk>>,
    chunk_size: usize,
    max_chunk_size: usize,
}

impl Arena {
    /// Create a new arena with the given initial chunk size.
    pub fn new(chunk_size: usize) -> Self {
        Self {
            chunks: RefCell::new(vec![Chunk::new(chunk_size)]),
            chunk_size,
            max_chunk_size: chunk_size * 64, // Cap at 64x initial
        }
    }

    /// Allocate `size` bytes from the arena.
    pub fn alloc(&self, size: usize) -> *mut u8 {
        self.alloc_aligned(size, std::mem::size_of::<usize>())
    }

    /// Allocate `size` bytes with the given alignment.
    pub fn alloc_aligned(&self, size: usize, align: usize) -> *mut u8 {
        let mut chunks = self.chunks.borrow_mut();

        // Try current chunk — align the current used offset
        let last_idx = chunks.len() - 1;
        unsafe {
            let chunk = &mut chunks[last_idx];
            let current = chunk.ptr.as_ptr().add(chunk.used);
            let aligned = (current as usize + align - 1) & !(align - 1);
            let padding = aligned - current as usize;
            if chunk.used + padding + size <= chunk.size {
                chunk.used += padding + size;
                return aligned as *mut u8;
            }
        }

        // Need a new chunk — allocate extra for alignment
        let new_size = (chunks[last_idx].size * 2)
            .max(size + align)
            .min(self.max_chunk_size);
        chunks.push(Chunk::new(new_size));

        let last_idx = chunks.len() - 1;
        unsafe {
            let chunk = &mut chunks[last_idx];
            let current = chunk.ptr.as_ptr();
            let aligned = (current as usize + align - 1) & !(align - 1);
            let padding = aligned - current as usize;
            chunk.used = padding + size;
            aligned as *mut u8
        }
    }

    /// Allocate with a specific layout.
    pub fn alloc_layout(&self, layout: Layout) -> *mut u8 {
        self.alloc_aligned(layout.size(), layout.align())
    }

    /// Reset the arena, freeing all allocations.
    pub fn reset(&self) {
        let mut chunks = self.chunks.borrow_mut();
        // Keep only the first chunk, reset its cursor
        for chunk in chunks.drain(1..) {
            drop(chunk);
        }
        if let Some(first) = chunks.first_mut() {
            first.used = 0;
        }
    }

    /// Get the total allocated bytes across all chunks.
    pub fn total_capacity(&self) -> usize {
        self.chunks.borrow().iter().map(|c| c.size).sum()
    }

    /// Get the total used bytes.
    pub fn total_used(&self) -> usize {
        self.chunks.borrow().iter().map(|c| c.used).sum()
    }

    /// Get the number of chunks.
    pub fn chunk_count(&self) -> usize {
        self.chunks.borrow().len()
    }
}

impl Default for Arena {
    fn default() -> Self {
        Self::new(64 * 1024) // 64 KB default
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_allocation() {
        let arena = Arena::new(1024);
        let ptr = arena.alloc(64);
        assert!(!ptr.is_null());
        assert_eq!(arena.total_used(), 64);
    }

    #[test]
    fn test_chunk_growth() {
        let arena = Arena::new(64);
        // Force multiple chunk allocations
        for _ in 0..20 {
            let ptr = arena.alloc(32);
            assert!(!ptr.is_null());
        }
        assert!(arena.chunk_count() > 1);
    }

    #[test]
    fn test_reset() {
        let arena = Arena::new(1024);
        for _ in 0..100 {
            arena.alloc(16);
        }
        assert!(arena.total_used() > 0);
        arena.reset();
        assert_eq!(arena.total_used(), 0);
        assert_eq!(arena.chunk_count(), 1);
    }

    #[test]
    fn test_aligned_allocation() {
        let arena = Arena::new(1024);
        let ptr = arena.alloc_aligned(32, 64);
        assert!(!ptr.is_null());
        assert_eq!(ptr as usize % 64, 0);
    }
}
