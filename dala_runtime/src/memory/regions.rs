//! Memory region implementations.

use std::collections::HashMap;
use std::sync::{Mutex, RwLock};

use dala_ir::type_system::{NativeResourceKind, TypeDescriptor};

/// Unique identifier for a memory region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegionId(pub u32);

/// Trait for memory regions.
pub trait MemoryRegion: std::fmt::Debug + Send + Sync {
    /// The region ID.
    fn id(&self) -> RegionId;
    /// Total capacity in bytes.
    fn capacity(&self) -> usize;
    /// Used bytes.
    fn used(&self) -> usize;
    /// Try to allocate `size` bytes. Returns a pointer or null.
    fn try_allocate(&self, size: usize) -> *mut u8;
    /// Deallocate a previously allocated pointer.
    /// # Safety
    /// The pointer must have been allocated from this region.
    unsafe fn deallocate(&self, ptr: *mut u8, size: usize);
}

// ═══════════════════════════════════════════════════════════════════════════
// Stable Immutable Region (SIR)
// ═══════════════════════════════════════════════════════════════════════════

/// The Stable Immutable Region holds long-lived, structurally immutable
/// objects that are never rescanned by the GC.
///
/// Objects enter the SIR when they survive a promotion threshold AND
/// are marked immutable by the compiler.  Typical contents:
/// - UI tree snapshots
/// - Configuration maps
/// - Schema definitions
/// - Static actor state
#[derive(Debug)]
pub struct StableImmutableRegion {
    id: RegionId,
    /// Backing storage
    storage: Mutex<Vec<u8>>,
    /// Current allocation cursor
    cursor: std::sync::atomic::AtomicUsize,
    /// Type descriptors for objects in this region
    type_table: RwLock<Vec<TypeDescriptor>>,
    /// Total capacity
    capacity: usize,
}

impl StableImmutableRegion {
    pub fn new(id: RegionId, capacity: usize) -> Self {
        Self {
            id,
            storage: Mutex::new(vec![0u8; capacity]),
            cursor: std::sync::atomic::AtomicUsize::new(0),
            type_table: RwLock::new(Vec::new()),
            capacity,
        }
    }

    /// Allocate an immutable object with a known layout.
    /// Returns a pointer to the allocated space.
    pub fn allocate_immutable(&self, layout: &std::alloc::Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        // Align the cursor
        let current = self.cursor.load(std::sync::atomic::Ordering::Relaxed);
        let aligned = (current + align - 1) & !(align - 1);

        if aligned + size > self.capacity {
            return std::ptr::null_mut();
        }

        self.cursor
            .store(aligned + size, std::sync::atomic::Ordering::Release);

        let mut storage = self.storage.lock().unwrap();
        unsafe { storage.as_mut_ptr().add(aligned) }
    }

    /// Check if a pointer is within this region.
    pub fn contains(&self, ptr: *const u8) -> bool {
        let storage = self.storage.lock().unwrap();
        let start = storage.as_ptr();
        let end = unsafe { start.add(self.capacity) };
        ptr >= start && ptr < end
    }

    /// Get the number of registered type descriptors.
    pub fn type_count(&self) -> usize {
        self.type_table.read().unwrap().len()
    }
}

impl MemoryRegion for StableImmutableRegion {
    fn id(&self) -> RegionId {
        self.id
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn used(&self) -> usize {
        self.cursor.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn try_allocate(&self, size: usize) -> *mut u8 {
        self.allocate_immutable(
            &std::alloc::Layout::from_size_align(size, std::mem::size_of::<usize>()).unwrap(),
        )
    }

    unsafe fn deallocate(&self, _ptr: *mut u8, _size: usize) {
        // No-op: SIR objects are never individually freed.
        // The entire region is dropped when the actor is destroyed.
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Binary Region
// ═══════════════════════════════════════════════════════════════════════════

/// The Binary Region manages large binary data (heap binaries, refc bins).
/// Uses reference counting for shared binaries.
#[derive(Debug)]
pub struct BinaryRegion {
    id: RegionId,
    /// Allocated blocks: (pointer, size, refcount)
    blocks: Mutex<Vec<BlockEntry>>,
    /// Total capacity
    capacity: usize,
    /// Used bytes
    used: std::sync::atomic::AtomicUsize,
}

#[derive(Debug)]
struct BlockEntry {
    ptr: *mut u8,
    size: usize,
    refcount: u32,
}

// SAFETY: BlockEntry is Send+Sync because the Mutex provides synchronization
// and the pointer is only accessed while holding the lock.
unsafe impl Send for BlockEntry {}
unsafe impl Sync for BlockEntry {}

impl BinaryRegion {
    pub fn new(id: RegionId, capacity: usize) -> Self {
        Self {
            id,
            blocks: Mutex::new(Vec::new()),
            capacity,
            used: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Allocate a binary block with initial refcount 1.
    pub fn alloc_binary(&self, size: usize) -> *mut u8 {
        let layout = match std::alloc::Layout::from_size_align(size, 8) {
            Ok(l) => l,
            Err(_) => return std::ptr::null_mut(),
        };

        let ptr = unsafe { std::alloc::alloc(layout) };
        if ptr.is_null() {
            return std::ptr::null_mut();
        }

        self.blocks.lock().unwrap().push(BlockEntry {
            ptr,
            size,
            refcount: 1,
        });
        self.used
            .fetch_add(size, std::sync::atomic::Ordering::Relaxed);
        ptr
    }

    /// Increment the refcount for a binary.
    pub fn incref(&self, ptr: *mut u8) {
        let mut blocks = self.blocks.lock().unwrap();
        for entry in blocks.iter_mut() {
            if entry.ptr == ptr {
                entry.refcount += 1;
                return;
            }
        }
    }

    /// Decrement the refcount. Frees the binary if refcount reaches 0.
    pub fn decref(&self, ptr: *mut u8) {
        let mut blocks = self.blocks.lock().unwrap();
        if let Some(pos) = blocks.iter().position(|e| e.ptr == ptr) {
            blocks[pos].refcount -= 1;
            if blocks[pos].refcount == 0 {
                let entry = blocks.remove(pos);
                let size = entry.size;
                let p = entry.ptr;
                unsafe {
                    let layout = std::alloc::Layout::from_size_align_unchecked(size, 8);
                    std::alloc::dealloc(p, layout);
                }
                self.used
                    .fetch_sub(size, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }
}

impl MemoryRegion for BinaryRegion {
    fn id(&self) -> RegionId {
        self.id
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn used(&self) -> usize {
        self.used.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn try_allocate(&self, size: usize) -> *mut u8 {
        self.alloc_binary(size)
    }

    unsafe fn deallocate(&self, ptr: *mut u8, _size: usize) {
        self.decref(ptr);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tensor Region
// ═══════════════════════════════════════════════════════════════════════════

/// The Tensor Region manages GPU/NN buffer allocations for AI workloads.
/// Supports zero-copy interop with native ML frameworks.
#[derive(Debug)]
pub struct TensorRegion {
    id: RegionId,
    /// Allocated tensor buffers
    buffers: Mutex<Vec<TensorBuffer>>,
    /// Total capacity in bytes
    capacity: usize,
    /// Used bytes
    used: std::sync::atomic::AtomicUsize,
}

#[derive(Debug)]
struct TensorBuffer {
    ptr: *mut u8,
    size: usize,
    /// Whether this buffer is GPU-resident
    gpu_resident: bool,
    /// Reference count
    refcount: u32,
}

// SAFETY: TensorBuffer is Send+Sync because the Mutex provides synchronization.
unsafe impl Send for TensorBuffer {}
unsafe impl Sync for TensorBuffer {}

impl TensorRegion {
    pub fn new(id: RegionId, capacity: usize) -> Self {
        Self {
            id,
            buffers: Mutex::new(Vec::new()),
            capacity,
            used: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Allocate a tensor buffer.
    pub fn alloc_tensor(&self, size: usize, gpu: bool) -> *mut u8 {
        let layout = match std::alloc::Layout::from_size_align(size, 64) {
            // 64-byte aligned for SIMD/GPU
            Ok(l) => l,
            Err(_) => return std::ptr::null_mut(),
        };

        let ptr = unsafe { std::alloc::alloc(layout) };
        if ptr.is_null() {
            return std::ptr::null_mut();
        }

        self.buffers.lock().unwrap().push(TensorBuffer {
            ptr,
            size,
            gpu_resident: gpu,
            refcount: 1,
        });
        self.used
            .fetch_add(size, std::sync::atomic::Ordering::Relaxed);
        ptr
    }

    /// Get the total GPU-resident bytes.
    pub fn gpu_usage(&self) -> usize {
        self.buffers
            .lock()
            .unwrap()
            .iter()
            .filter(|b| b.gpu_resident)
            .map(|b| b.size)
            .sum()
    }
}

impl MemoryRegion for TensorRegion {
    fn id(&self) -> RegionId {
        self.id
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn used(&self) -> usize {
        self.used.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn try_allocate(&self, size: usize) -> *mut u8 {
        self.alloc_tensor(size, false)
    }

    unsafe fn deallocate(&self, ptr: *mut u8, _size: usize) {
        let mut buffers = self.buffers.lock().unwrap();
        if let Some(pos) = buffers.iter().position(|b| b.ptr == ptr) {
            buffers[pos].refcount -= 1;
            if buffers[pos].refcount == 0 {
                let buf = buffers.remove(pos);
                let layout = std::alloc::Layout::from_size_align_unchecked(buf.size, 64);
                std::alloc::dealloc(buf.ptr, layout);
                self.used
                    .fetch_sub(buf.size, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }
}

impl Drop for TensorRegion {
    fn drop(&mut self) {
        let mut buffers = self.buffers.lock().unwrap();
        for buf in buffers.drain(..) {
            unsafe {
                let layout = std::alloc::Layout::from_size_align_unchecked(buf.size, 64);
                std::alloc::dealloc(buf.ptr, layout);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Native Resource Region
// ═══════════════════════════════════════════════════════════════════════════

/// The Native Resource Region manages capability-tracked native handles
/// (file descriptors, sockets, GPU contexts, etc.).
#[derive(Debug)]
pub struct NativeResourceRegion {
    id: RegionId,
    /// Active resources
    resources: Mutex<HashMap<NativeResourceId, NativeResourceEntry>>,
    /// Next resource ID
    next_id: std::sync::atomic::AtomicU32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NativeResourceId(pub u32);

#[derive(Debug)]
pub(crate) struct NativeResourceEntry {
    pub kind: NativeResourceKind,
    pub handle: *mut u8,
    pub owned: bool,
    pub shareable: bool,
    /// The actor that owns this resource
    pub owner: u64,
}

// SAFETY: NativeResourceEntry is Send+Sync because the Mutex provides synchronization.
unsafe impl Send for NativeResourceEntry {}
unsafe impl Sync for NativeResourceEntry {}

impl NativeResourceRegion {
    pub fn new(id: RegionId) -> Self {
        Self {
            id,
            resources: Mutex::new(HashMap::new()),
            next_id: std::sync::atomic::AtomicU32::new(1),
        }
    }

    /// Register a new native resource. Returns a capability ID.
    pub fn register(
        &self,
        kind: NativeResourceKind,
        handle: *mut u8,
        owned: bool,
        shareable: bool,
        owner: u64,
    ) -> NativeResourceId {
        let id = NativeResourceId(
            self.next_id
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst),
        );
        self.resources.lock().unwrap().insert(
            id,
            NativeResourceEntry {
                kind,
                handle,
                owned,
                shareable,
                owner,
            },
        );
        id
    }

    /// Get a resource entry.
    pub fn get(&self, id: NativeResourceId) -> Option<NativeResourceEntry> {
        self.resources.lock().unwrap().get(&id).cloned()
    }

    /// Release a resource. If owned, the handle is deallocated.
    pub fn release(&self, id: NativeResourceId) -> bool {
        let mut resources = self.resources.lock().unwrap();
        if let Some(entry) = resources.remove(&id) {
            if entry.owned {
                // In a full implementation, this would call the
                // appropriate deallocator for the resource kind.
                let _ = entry.handle;
            }
            true
        } else {
            false
        }
    }

    /// Transfer ownership of a resource to a different actor.
    pub fn transfer(&self, id: NativeResourceId, new_owner: u64) -> bool {
        let mut resources = self.resources.lock().unwrap();
        if let Some(entry) = resources.get_mut(&id) {
            if entry.shareable || entry.owner == new_owner {
                entry.owner = new_owner;
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Get the number of active resources.
    pub fn len(&self) -> usize {
        self.resources.lock().unwrap().len()
    }

    /// Check if the region is empty.
    pub fn is_empty(&self) -> bool {
        self.resources.lock().unwrap().is_empty()
    }
}

impl MemoryRegion for NativeResourceRegion {
    fn id(&self) -> RegionId {
        self.id
    }

    fn capacity(&self) -> usize {
        usize::MAX // Unbounded — limited by OS
    }

    fn used(&self) -> usize {
        self.len()
    }

    fn try_allocate(&self, _size: usize) -> *mut u8 {
        std::ptr::null_mut() // Resources are registered, not allocated
    }

    unsafe fn deallocate(&self, _ptr: *mut u8, _size: usize) {
        // Resources are released via release()
    }
}

impl Clone for NativeResourceEntry {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind,
            handle: self.handle,
            owned: self.owned,
            shareable: self.shareable,
            owner: self.owner,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stable_immutable_region() {
        let sir = StableImmutableRegion::new(RegionId(0), 1024);
        let ptr = sir.allocate_immutable(&std::alloc::Layout::from_size_align(64, 8).unwrap());
        assert!(!ptr.is_null());
        assert!(sir.contains(ptr));
        assert_eq!(sir.used(), 64);
    }

    #[test]
    fn test_binary_region_refcounting() {
        let region = BinaryRegion::new(RegionId(1), 1024);
        let ptr = region.alloc_binary(256);
        assert!(!ptr.is_null());

        region.incref(ptr);
        region.decref(ptr); // refcount still 1
        region.decref(ptr); // refcount 0, freed
    }

    #[test]
    fn test_tensor_region() {
        let region = TensorRegion::new(RegionId(2), 1024 * 1024);
        let ptr = region.alloc_tensor(4096, true);
        assert!(!ptr.is_null());
        assert_eq!(region.gpu_usage(), 4096);
    }

    #[test]
    fn test_native_resource_region() {
        let region = NativeResourceRegion::new(RegionId(3));
        let handle = 0xDEADBEEF as *mut u8;
        let id = region.register(NativeResourceKind::IoHandle, handle, true, false, 42);

        let entry = region.get(id).unwrap();
        assert_eq!(entry.kind, NativeResourceKind::IoHandle);
        assert_eq!(entry.owner, 42);
        assert!(entry.owned);

        assert!(region.release(id));
        assert!(region.get(id).is_none());
    }

    #[test]
    fn test_resource_transfer() {
        let region = NativeResourceRegion::new(RegionId(4));
        let handle = 0x1234 as *mut u8;
        let id = region.register(NativeResourceKind::SharedMemory, handle, true, true, 1);

        assert!(region.transfer(id, 2));
        assert_eq!(region.get(id).unwrap().owner, 2);
    }
}
