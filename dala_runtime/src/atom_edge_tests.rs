//! Edge case tests for AtomTable.

use crate::atom::*;

// ═══════════════════════════════════════════════════════════════════════════
// Empty table, lookup on empty, insert and lookup roundtrip
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_new_table_has_standard_atoms() {
    let table = AtomTable::new();
    // Standard atoms are pre-loaded
    assert!(table.len() > 0);
    assert!(!table.is_empty());
}

#[test]
fn test_lookup_on_empty_table() {
    let table = AtomTable::new();
    // "nonexistent_atom_xyz" is not a standard atom
    assert_eq!(table.lookup("nonexistent_atom_xyz"), None);
}

#[test]
fn test_insert_and_lookup_roundtrip() {
    let table = AtomTable::new();
    let idx = table.lookup_or_insert("hello");
    assert_eq!(table.lookup("hello"), Some(idx));
    assert_eq!(table.get_name(idx), Some("hello"));
}

#[test]
fn test_insert_empty_string() {
    let table = AtomTable::new();
    let idx = table.lookup_or_insert("");
    assert_eq!(table.lookup(""), Some(idx));
    assert_eq!(table.get_name(idx), Some(""));
}

#[test]
fn test_insert_unicode() {
    let table = AtomTable::new();
    let idx = table.lookup_or_insert("héllo_wörld_日本語");
    assert_eq!(table.lookup("héllo_wörld_日本語"), Some(idx));
    assert_eq!(table.get_name(idx), Some("héllo_wörld_日本語"));
}

#[test]
fn test_insert_long_string() {
    let table = AtomTable::new();
    let long_name = "a".repeat(10_000);
    let idx = table.lookup_or_insert(&long_name);
    assert_eq!(table.lookup(&long_name), Some(idx));
    assert_eq!(table.get_name(idx), Some(long_name.as_str()));
}

// ═══════════════════════════════════════════════════════════════════════════
// Duplicate insertion returns same index
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_duplicate_insertion_same_index() {
    let table = AtomTable::new();
    let idx1 = table.lookup_or_insert("duplicate_test");
    let idx2 = table.lookup_or_insert("duplicate_test");
    assert_eq!(idx1, idx2);
}

#[test]
fn test_duplicate_insertion_does_not_increase_len() {
    let table = AtomTable::new();
    let len_before = table.len();
    table.lookup_or_insert("unique_atom_abc");
    let len_after_first = table.len();
    assert_eq!(len_after_first, len_before + 1);

    table.lookup_or_insert("unique_atom_abc");
    let len_after_second = table.len();
    assert_eq!(len_after_second, len_after_first);
}

#[test]
fn test_multiple_duplicates() {
    let table = AtomTable::new();
    let idx1 = table.lookup_or_insert("multi_dup");
    let idx2 = table.lookup_or_insert("multi_dup");
    let idx3 = table.lookup_or_insert("multi_dup");
    assert_eq!(idx1, idx2);
    assert_eq!(idx2, idx3);
}

// ═══════════════════════════════════════════════════════════════════════════
// Lookup non-existent atom
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_lookup_nonexistent() {
    let table = AtomTable::new();
    assert_eq!(table.lookup("this_atom_does_not_exist_12345"), None);
}

#[test]
fn test_lookup_nonexistent_after_inserts() {
    let table = AtomTable::new();
    table.lookup_or_insert("existing");
    assert_eq!(table.lookup("not_existing"), None);
}

// ═══════════════════════════════════════════════════════════════════════════
// get_name on invalid index
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_get_name_invalid_index() {
    let table = AtomTable::new();
    let len = table.len() as u32;
    // Index beyond the table should return None
    assert_eq!(table.get_name(len + 1000), None);
}

#[test]
fn test_get_name_max_u32() {
    let table = AtomTable::new();
    assert_eq!(table.get_name(u32::MAX), None);
}

// ═══════════════════════════════════════════════════════════════════════════
// Rapid insertions
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_rapid_insertions() {
    let table = AtomTable::new();
    let count = 1000;
    let mut indices = Vec::new();

    for i in 0..count {
        let name = format!("rapid_atom_{}", i);
        let idx = table.lookup_or_insert(&name);
        indices.push(idx);
    }

    // All indices should be unique
    let mut unique = indices.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), count);

    // All should be retrievable
    for i in 0..count {
        let name = format!("rapid_atom_{}", i);
        assert!(table.lookup(&name).is_some());
    }
}

#[test]
fn test_rapid_insertions_len() {
    let table = AtomTable::new();
    let initial_len = table.len();

    for i in 0..100 {
        let name = format!("atom_{}", i);
        table.lookup_or_insert(&name);
    }

    assert_eq!(table.len(), initial_len + 100);
}

// ═══════════════════════════════════════════════════════════════════════════
// Standard atoms pre-loaded correctly
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_standard_atoms_preloaded() {
    let table = AtomTable::new();

    // These are the standard atoms from the implementation
    let standard = [
        "nil",
        "true",
        "false",
        "undefined",
        "error",
        "exit",
        "throw",
        "ok",
        "after",
        "normal",
        "shutdown",
        "badmatch",
        "case_clause",
        "if_clause",
        "try_clause",
        "badarg",
        "badarith",
        "badbool",
        "function_clause",
        "match",
        "multifile",
        "no_local",
        "on_load",
        "noconnection",
        "call_from_c",
        "file",
        "line",
        "badmap",
        "badkey",
    ];

    for name in &standard {
        assert!(
            table.lookup(name).is_some(),
            "Standard atom '{}' not found",
            name
        );
    }
}

#[test]
fn test_standard_atom_indices_are_sequential() {
    let table = AtomTable::new();

    // The first atom should have index 0, second index 1, etc.
    let idx_nil = table.lookup("nil").unwrap();
    let idx_true = table.lookup("true").unwrap();
    let idx_false = table.lookup("false").unwrap();

    assert_eq!(idx_nil, 0);
    assert_eq!(idx_true, 1);
    assert_eq!(idx_false, 2);
}

#[test]
fn test_standard_atom_count() {
    let table = AtomTable::new();
    // There are 29 standard atoms
    assert_eq!(table.len(), 29);
}

// ═══════════════════════════════════════════════════════════════════════════
// Global singleton behavior
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_global_singleton_consistency() {
    // The global atom table should always return the same instance
    let idx1 = atom("global_test_atom_1");
    let idx2 = atom("global_test_atom_1");
    assert_eq!(idx1, idx2);
}

#[test]
fn test_global_get_name() {
    let _ = atom("global_test_atom_2");
    let name = get_name(atom("global_test_atom_2"));
    assert_eq!(name, Some("global_test_atom_2"));
}

#[test]
fn test_global_atom_table_ref() {
    let table = get_atom_table();
    assert!(!table.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// AtomTable Default
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_atom_table_default() {
    let table: AtomTable = Default::default();
    assert!(!table.is_empty());
    assert!(table.lookup("nil").is_some());
}

// ═══════════════════════════════════════════════════════════════════════════
// is_empty
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_is_empty_false_for_new_table() {
    let table = AtomTable::new();
    // New table has standard atoms, so it's not empty
    assert!(!table.is_empty());
}
