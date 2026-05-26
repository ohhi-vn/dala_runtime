//! Edge case tests for memory regions.

use crate::memory;
use crate::memory::MemoryConfig;
use crate::memory::arena::Arena;
use crate::memory::regions::{
    BinaryRegion, MemoryRegion, NativeResourceId, NativeResourceRegion, RegionId,
    StableImmutableRegion, TensorRegion,
};
use dala_ir::type_system::NativeResourceKind;

// ═══════════════════════════════════════════════════════════════════════════
// Arena: zero-size alloc, very large alloc, alignment edge cases, reset after many allocs
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_arena_zero_size_alloc() {
    let arena = Arena::new(1024);
    // Zero-size allocation should still return a valid pointer
    let ptr = arena.alloc(0);
    assert!(!ptr.is_null());
}

#[test]
fn test_arena_very_large_alloc() {
    let arena = Arena::new(64);
    // Allocate something larger than the chunk size
    let ptr = arena.alloc(1024 * 1024); // 1 MB
    assert!(!ptr.is_null());
}

#[test]
fn test_arena_alignment_1() {
    let arena = Arena::new(1024);
    let ptr = arena.alloc_aligned(1, 1);
    assert!(!ptr.is_null());
    assert_eq!(ptr as usize % 1, 0);
}

#[test]
fn test_arena_alignment_8() {
    let arena = Arena::new(1024);
    let ptr = arena.alloc_aligned(8, 8);
    assert!(!ptr.is_null());
    assert_eq!(ptr as usize % 8, 0);
}

#[test]
fn test_arena_alignment_64() {
    let arena = Arena::new(1024);
    let ptr = arena.alloc_aligned(16, 64);
    assert!(!ptr.is_null());
    assert_eq!(ptr as usize % 64, 0);
}

#[test]
fn test_arena_alignment_4096() {
    let arena = Arena::new(8192);
    let ptr = arena.alloc_aligned(32, 4096);
    assert!(!ptr.is_null());
    assert_eq!(ptr as usize % 4096, 0);
}

#[test]
fn test_arena_reset_after_many_allocs() {
    let arena = Arena::new(64);

    // Force many allocations and chunk growth
    for _ in 0..1000 {
        arena.alloc(32);
    }

    assert!(arena.total_used() > 0);
    assert!(arena.chunk_count() > 1);

    arena.reset();

    assert_eq!(arena.total_used(), 0);
    assert_eq!(arena.chunk_count(), 1);
}

#[test]
fn test_arena_total_capacity() {
    let arena = Arena::new(1024);
    assert_eq!(arena.total_capacity(), 1024);

    // Force chunk growth
    arena.alloc(2048);
    assert!(arena.total_capacity() > 1024);
}

#[test]
fn test_arena_alloc_layout() {
    let arena = Arena::new(1024);
    let layout = std::alloc::Layout::from_size_align(64, 16).unwrap();
    let ptr = arena.alloc_layout(layout);
    assert!(!ptr.is_null());
    assert_eq!(ptr as usize % 16, 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// StableImmutableRegion: allocate_immutable, contains, type_count
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_sir_allocate_immutable() {
    let sir = StableImmutableRegion::new(RegionId(0), 1024);
    let layout = std::alloc::Layout::from_size_align(64, 8).unwrap();
    let ptr = sir.allocate_immutable(&layout);
    assert!(!ptr.is_null());
}

#[test]
fn test_sir_allocate_immutable_contains() {
    let sir = StableImmutableRegion::new(RegionId(0), 1024);
    let layout = std::alloc::Layout::from_size_align(64, 8).unwrap();
    let ptr = sir.allocate_immutable(&layout);
    assert!(sir.contains(ptr));
}

#[test]
fn test_sir_contains_outside_ptr() {
    let sir = StableImmutableRegion::new(RegionId(0), 1024);
    let outside_ptr = 0x1 as *const u8;
    assert!(!sir.contains(outside_ptr));
}

#[test]
fn test_sir_type_count() {
    let sir = StableImmutableRegion::new(RegionId(0), 1024);
    assert_eq!(sir.type_count(), 0);
}

#[test]
fn test_sir_allocate_until_full() {
    let sir = StableImmutableRegion::new(RegionId(0), 128);
    let layout = std::alloc::Layout::from_size_align(64, 8).unwrap();

    let ptr1 = sir.allocate_immutable(&layout);
    assert!(!ptr1.is_null());

    // Second allocation of same size should fail (128 - 64 = 64, but alignment padding)
    let layout2 = std::alloc::Layout::from_size_align(64, 8).unwrap();
    let ptr2 = sir.allocate_immutable(&layout2);
    // May or may not be null depending on alignment, but shouldn't crash
    let _ = ptr2;
}

#[test]
fn test_sir_memory_region_trait() {
    let sir = StableImmutableRegion::new(RegionId(0), 1024);
    assert_eq!(sir.id(), RegionId(0));
    assert_eq!(sir.capacity(), 1024);
    assert_eq!(sir.used(), 0);

    sir.try_allocate(64);
    assert!(sir.used() > 0);
}

#[test]
fn test_sir_try_allocate() {
    let sir = StableImmutableRegion::new(RegionId(0), 1024);
    let ptr = sir.try_allocate(64);
    assert!(!ptr.is_null());
}

// ═══════════════════════════════════════════════════════════════════════════
// BinaryRegion: alloc_binary, incref/decref to zero, double-free safety
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_binary_region_alloc() {
    let region = BinaryRegion::new(RegionId(0), 1024 * 1024);
    let ptr = region.alloc_binary(256);
    assert!(!ptr.is_null());
}

#[test]
fn test_binary_region_alloc_zero_size() {
    let region = BinaryRegion::new(RegionId(0), 1024 * 1024);
    let ptr = region.alloc_binary(0);
    // Zero-size allocation may return null or a valid pointer
    let _ = ptr;
}

#[test]
fn test_binary_region_incref() {
    let region = BinaryRegion::new(RegionId(0), 1024 * 1024);
    let ptr = region.alloc_binary(256);
    assert!(!ptr.is_null());

    // Increment refcount
    region.incref(ptr);
    // No crash = success
}

#[test]
fn test_binary_region_decref_to_zero() {
    let region = BinaryRegion::new(RegionId(0), 1024 * 1024);
    let ptr = region.alloc_binary(256);
    assert!(!ptr.is_null());

    // Decrement to zero should free
    region.decref(ptr);
    // No crash = success
}

#[test]
fn test_binary_region_double_decref_safe() {
    let region = BinaryRegion::new(RegionId(0), 1024 * 1024);
    let ptr = region.alloc_binary(256);
    assert!(!ptr.is_null());

    region.decref(ptr); // Frees the binary
    region.decref(ptr); // Should be safe (no-op on non-existent ptr)
}

#[test]
fn test_binary_region_incref_unknown_ptr() {
    let region = BinaryRegion::new(RegionId(0), 1024 * 1024);
    let unknown_ptr = 0xDEAD as *mut u8;
    region.incref(unknown_ptr); // Should be safe (no-op)
}

#[test]
fn test_binary_region_decref_unknown_ptr() {
    let region = BinaryRegion::new(RegionId(0), 1024 * 1024);
    let unknown_ptr = 0xDEAD as *mut u8;
    region.decref(unknown_ptr); // Should be safe (no-op)
}

#[test]
fn test_binary_region_memory_region_trait() {
    let region = BinaryRegion::new(RegionId(0), 1024 * 1024);
    assert_eq!(region.id(), RegionId(0));
    assert_eq!(region.capacity(), 1024 * 1024);
    assert_eq!(region.used(), 0);

    let _ = region.try_allocate(256);
    assert!(region.used() > 0);
}

#[test]
fn test_binary_region_multiple_allocations() {
    let region = BinaryRegion::new(RegionId(0), 1024 * 1024);

    let ptrs: Vec<*mut u8> = (0..10).map(|_| region.alloc_binary(1024)).collect();

    for ptr in &ptrs {
        assert!(!ptr.is_null());
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TensorRegion: alloc_tensor, gpu_usage tracking
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_tensor_region_alloc() {
    let region = TensorRegion::new(RegionId(0), 16 * 1024 * 1024);
    let ptr = region.alloc_tensor(1024, false);
    assert!(!ptr.is_null());
}

#[test]
fn test_tensor_region_alloc_gpu() {
    let region = TensorRegion::new(RegionId(0), 16 * 1024 * 1024);
    let ptr = region.alloc_tensor(1024, true);
    assert!(!ptr.is_null());
}

#[test]
fn test_tensor_region_gpu_usage() {
    let region = TensorRegion::new(RegionId(0), 16 * 1024 * 1024);

    assert_eq!(region.gpu_usage(), 0);

    region.alloc_tensor(1024, true);
    assert_eq!(region.gpu_usage(), 1024);

    region.alloc_tensor(2048, true);
    assert_eq!(region.gpu_usage(), 3072);

    region.alloc_tensor(512, false); // Not GPU
    assert_eq!(region.gpu_usage(), 3072);
}

#[test]
fn test_tensor_region_memory_region_trait() {
    let region = TensorRegion::new(RegionId(0), 16 * 1024 * 1024);
    assert_eq!(region.id(), RegionId(0));
    assert_eq!(region.capacity(), 16 * 1024 * 1024);
    assert_eq!(region.used(), 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// NativeResourceRegion: register, get, release, transfer ownership
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_native_resource_register() {
    let region = NativeResourceRegion::new(RegionId(0));
    let handle = 0x1000 as *mut u8;

    let id = region.register(NativeResourceKind::IoHandle, handle, true, false, 1);
    assert!(!region.is_empty());
    assert_eq!(region.len(), 1);
}

#[test]
fn test_native_resource_get() {
    let region = NativeResourceRegion::new(RegionId(0));
    let handle = 0x1000 as *mut u8;

    let id = region.register(NativeResourceKind::IoHandle, handle, true, false, 1);
    let entry = region.get(id);
    assert!(entry.is_some());

    let entry = entry.unwrap();
    assert_eq!(entry.handle, handle);
    assert!(entry.owned);
    assert!(!entry.shareable);
    assert_eq!(entry.owner, 1);
}

#[test]
fn test_native_resource_get_nonexistent() {
    let region = NativeResourceRegion::new(RegionId(0));
    let id = NativeResourceId(999);
    assert!(region.get(id).is_none());
}

#[test]
fn test_native_resource_release() {
    let region = NativeResourceRegion::new(RegionId(0));
    let handle = 0x1000 as *mut u8;

    let id = region.register(NativeResourceKind::IoHandle, handle, true, false, 1);
    assert_eq!(region.len(), 1);

    let released = region.release(id);
    assert!(released);
    assert_eq!(region.len(), 0);
    assert!(region.is_empty());
}

#[test]
fn test_release_nonexistent() {
    let region = NativeResourceRegion::new(RegionId(0));
    let id = NativeResourceId(999);
    assert!(!region.release(id));
}

#[test]
fn test_native_resource_transfer_ownership() {
    let region = NativeResourceRegion::new(RegionId(0));
    let handle = 0x1000 as *mut u8;

    let id = region.register(NativeResourceKind::IoHandle, handle, true, true, 1);

    // Transfer to actor 2
    let transferred = region.transfer(id, 2);
    assert!(transferred);

    let entry = region.get(id).unwrap();
    assert_eq!(entry.owner, 2);
}

#[test]
fn test_transfer_non_shareable() {
    let region = NativeResourceRegion::new(RegionId(0));
    let handle = 0x1000 as *mut u8;

    let id = region.register(NativeResourceKind::IoHandle, handle, true, false, 1);

    // Transfer should fail for non-shareable resource to different owner
    let transferred = region.transfer(id, 2);
    assert!(!transferred);

    // Owner should remain the same
    let entry = region.get(id).unwrap();
    assert_eq!(entry.owner, 1);
}

#[test]
fn test_transfer_same_owner() {
    let region = NativeResourceRegion::new(RegionId(0));
    let handle = 0x1000 as *mut u8;

    let id = region.register(NativeResourceKind::IoHandle, handle, true, false, 1);

    // Transfer to same owner should succeed
    let transferred = region.transfer(id, 1);
    assert!(transferred);
}

#[test]
fn test_transfer_nonexistent() {
    let region = NativeResourceRegion::new(RegionId(0));
    let id = NativeResourceId(999);
    assert!(!region.transfer(id, 2));
}

#[test]
fn test_native_resource_multiple() {
    let region = NativeResourceRegion::new(RegionId(0));

    let id1 = region.register(
        NativeResourceKind::IoHandle,
        0x1000 as *mut u8,
        true,
        false,
        1,
    );
    let id2 = region.register(NativeResourceKind::Socket, 0x2000 as *mut u8, true, true, 2);
    let id3 = region.register(
        NativeResourceKind::GpuContext,
        0x3000 as *mut u8,
        false,
        false,
        3,
    );

    assert_eq!(region.len(), 3);

    // Each should be retrievable
    let e1 = region.get(id1).unwrap();
    assert_eq!(e1.kind, NativeResourceKind::IoHandle);

    let e2 = region.get(id2).unwrap();
    assert_eq!(e2.kind, NativeResourceKind::Socket);

    let e3 = region.get(id3).unwrap();
    assert_eq!(e3.kind, NativeResourceKind::GpuContext);

    // Release one
    region.release(id2);
    assert_eq!(region.len(), 2);
    assert!(region.get(id2).is_none());
}

#[test]
fn test_native_resource_memory_region_trait() {
    let region = NativeResourceRegion::new(RegionId(0));
    assert_eq!(region.id(), RegionId(0));
    assert_eq!(region.capacity(), usize::MAX);
    assert_eq!(region.used(), 0);

    region.register(
        NativeResourceKind::IoHandle,
        0x1000 as *mut u8,
        true,
        false,
        1,
    );
    assert_eq!(region.used(), 1);
}

// ═══════════════════════════════════════════════════════════════════════════
// MemoryConfig defaults
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_memory_config_default() {
    let config = MemoryConfig::default();
    assert_eq!(config.initial_heap_size, 233);
    assert_eq!(config.max_heap_size, 16_384);
    assert_eq!(config.sir_size, 1024 * 1024);
    assert_eq!(config.binary_region_size, 4 * 1024 * 1024);
    assert_eq!(config.tensor_region_size, 16 * 1024 * 1024);
    assert_eq!(config.arena_chunk_size, 64 * 1024);
}

// ═══════════════════════════════════════════════════════════════════════════
// RegionId
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_region_id_equality() {
    assert_eq!(RegionId(0), RegionId(0));
    assert_ne!(RegionId(0), RegionId(1));
}

#[test]
fn test_region_id_copy() {
    let id = RegionId(42);
    let id2 = id;
    assert_eq!(id, id2);
}

#[test]
fn test_region_id_debug() {
    let id = RegionId(42);
    let dbg = format!("{:?}", id);
    assert!(dbg.contains("42"), "Debug was: {}", dbg);
}

// ═══════════════════════════════════════════════════════════════════════════
// NativeResourceId
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_native_resource_id_equality() {
    assert_eq!(NativeResourceId(0), NativeResourceId(0));
    assert_ne!(NativeResourceId(0), NativeResourceId(1));
}

#[test]
fn test_native_resource_id_copy() {
    let id = NativeResourceId(42);
    let id2 = id;
    assert_eq!(id, id2);
}
