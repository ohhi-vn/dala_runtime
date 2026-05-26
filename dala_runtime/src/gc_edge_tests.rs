//! Edge case tests for GC (Garbage Collector).

use crate::gc::*;

// ═══════════════════════════════════════════════════════════════════════════
// GCStats defaults
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_gc_stats_default() {
    let stats = GCStats::default();
    assert_eq!(stats.heap_words_before, 0);
    assert_eq!(stats.heap_words_after, 0);
    assert_eq!(stats.stack_words, 0);
    assert_eq!(stats.roots_scanned, 0);
    assert_eq!(stats.objects_copied, 0);
    assert_eq!(stats.time_ns, 0);
}

#[test]
fn test_gc_stats_clone() {
    let stats = GCStats {
        heap_words_before: 100,
        heap_words_after: 50,
        stack_words: 20,
        roots_scanned: 30,
        objects_copied: 25,
        time_ns: 1_000_000,
    };
    let stats2 = stats.clone();
    assert_eq!(stats.heap_words_before, stats2.heap_words_before);
    assert_eq!(stats.time_ns, stats2.time_ns);
}

#[test]
fn test_gc_stats_debug() {
    let stats = GCStats::default();
    let dbg = format!("{:?}", stats);
    assert!(dbg.contains("GCStats"), "Debug was: {}", dbg);
}

// ═══════════════════════════════════════════════════════════════════════════
// GCConfig defaults
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_gc_config_default() {
    let config = GCConfig::default();
    assert_eq!(config.nursery_size, 233);
    assert_eq!(config.max_copy, 7);
    assert_eq!(config.fullsweep_after, 65536);
}

#[test]
fn test_gc_config_clone() {
    let config = GCConfig::default();
    let config2 = config.clone();
    assert_eq!(config.nursery_size, config2.nursery_size);
    assert_eq!(config.max_copy, config2.max_copy);
    assert_eq!(config.fullsweep_after, config2.fullsweep_after);
}

// ═══════════════════════════════════════════════════════════════════════════
// StackMap: entries() on empty map
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_stack_map_empty_entries() {
    let map = StackMap {
        instruction_offset: 0,
        num_entries: 0,
        entries: [],
    };

    let entries = map.entries();
    assert!(entries.is_empty());
}

#[test]
fn test_stack_map_with_zero_entries() {
    let map = StackMap {
        instruction_offset: 42,
        num_entries: 0,
        entries: [],
    };

    assert_eq!(map.instruction_offset, 42);
    assert_eq!(map.num_entries, 0);
    assert!(map.entries().is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// StackMapEntry creation
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_stack_map_entry_creation() {
    let entry = StackMapEntry {
        offset: 4,
        is_pointer: true,
        value_type: StackMapType::ListPointer,
    };

    assert_eq!(entry.offset, 4);
    assert!(entry.is_pointer);
}

#[test]
fn test_stack_map_entry_copy() {
    let entry = StackMapEntry {
        offset: 8,
        is_pointer: false,
        value_type: StackMapType::Unknown,
    };
    let entry2 = entry;
    assert_eq!(entry.offset, entry2.offset);
    assert_eq!(entry.is_pointer, entry2.is_pointer);
}

#[test]
fn test_stack_map_entry_debug() {
    let entry = StackMapEntry {
        offset: 0,
        is_pointer: true,
        value_type: StackMapType::TuplePointer,
    };
    let dbg = format!("{:?}", entry);
    assert!(dbg.contains("StackMapEntry"), "Debug was: {}", dbg);
}

// ═══════════════════════════════════════════════════════════════════════════
// StackMapType variants
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_stack_map_type_variants() {
    let types = [
        StackMapType::Unknown,
        StackMapType::TuplePointer,
        StackMapType::ListPointer,
        StackMapType::BoxedPointer,
        StackMapType::FunPointer,
        StackMapType::MapPointer,
        StackMapType::BinaryPointer,
        StackMapType::MaybePointer,
    ];

    for ty in &types {
        let _ = format!("{:?}", ty);
    }
}

#[test]
fn test_stack_map_type_discriminants() {
    // Verify the repr(u8) discriminant values
    assert_eq!(StackMapType::Unknown as u8, 0);
    assert_eq!(StackMapType::TuplePointer as u8, 1);
    assert_eq!(StackMapType::ListPointer as u8, 2);
    assert_eq!(StackMapType::BoxedPointer as u8, 3);
    assert_eq!(StackMapType::FunPointer as u8, 4);
    assert_eq!(StackMapType::MapPointer as u8, 5);
    assert_eq!(StackMapType::BinaryPointer as u8, 6);
    assert_eq!(StackMapType::MaybePointer as u8, 7);
}

// ═══════════════════════════════════════════════════════════════════════════
// safepoint
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_safepoint_no_op() {
    // safepoint() should be a no-op in the current implementation
    safepoint();
}

// ═══════════════════════════════════════════════════════════════════════════
// StackMap with various instruction offsets
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_stack_map_instruction_offset_zero() {
    let map = StackMap {
        instruction_offset: 0,
        num_entries: 0,
        entries: [],
    };
    assert_eq!(map.instruction_offset, 0);
}

#[test]
fn test_stack_map_instruction_offset_max() {
    let map = StackMap {
        instruction_offset: u32::MAX,
        num_entries: 0,
        entries: [],
    };
    assert_eq!(map.instruction_offset, u32::MAX);
}

// ═══════════════════════════════════════════════════════════════════════════
// StackMapEntry with all value types
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_stack_map_entry_all_value_types() {
    let value_types = [
        StackMapType::Unknown,
        StackMapType::TuplePointer,
        StackMapType::ListPointer,
        StackMapType::BoxedPointer,
        StackMapType::FunPointer,
        StackMapType::MapPointer,
        StackMapType::BinaryPointer,
        StackMapType::MaybePointer,
    ];

    for (i, vt) in value_types.iter().enumerate() {
        let entry = StackMapEntry {
            offset: i as u32 * 8,
            is_pointer: matches!(
                vt,
                StackMapType::TuplePointer
                    | StackMapType::ListPointer
                    | StackMapType::BoxedPointer
                    | StackMapType::FunPointer
                    | StackMapType::MapPointer
                    | StackMapType::BinaryPointer
            ),
            value_type: *vt,
        };
        assert_eq!(entry.offset, i as u32 * 8);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// GCConfig custom values
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_gc_config_custom() {
    let config = GCConfig {
        nursery_size: 512,
        max_copy: 10,
        fullsweep_after: 100_000,
    };
    assert_eq!(config.nursery_size, 512);
    assert_eq!(config.max_copy, 10);
    assert_eq!(config.fullsweep_after, 100_000);
}
