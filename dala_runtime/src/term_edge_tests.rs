//! Edge case tests for Term representation.

use crate::term::tags;
use crate::term::{RegisterFile, Term};
use num_bigint::BigInt;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// ═══════════════════════════════════════════════════════════════════════════
// Tagged pointer encoding — boundary values for small ints
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_small_int_zero() {
    let t = Term::small(0);
    assert!(t.is_small());
    assert_eq!(t.unwrap_small(), 0);
}

#[test]
fn test_small_int_one() {
    let t = Term::small(1);
    assert!(t.is_small());
    assert_eq!(t.unwrap_small(), 1);
}

#[test]
fn test_small_int_negative_one() {
    let t = Term::small(-1);
    assert!(t.is_small());
    assert_eq!(t.unwrap_small(), -1);
}

#[test]
fn test_small_int_i64_max() {
    let t = Term::small(i64::MAX);
    assert!(t.is_small());
    assert_eq!(t.unwrap_small(), i64::MAX);
}

#[test]
fn test_small_int_i64_min() {
    let t = Term::small(i64::MIN);
    assert!(t.is_small());
    assert_eq!(t.unwrap_small(), i64::MIN);
}

#[test]
fn test_small_int_large_positive() {
    let t = Term::small(i64::MAX / 2);
    assert!(t.is_small());
    assert_eq!(t.unwrap_small(), i64::MAX / 2);
}

#[test]
fn test_small_int_large_negative() {
    let t = Term::small(i64::MIN / 2);
    assert!(t.is_small());
    assert_eq!(t.unwrap_small(), i64::MIN / 2);
}

// ═══════════════════════════════════════════════════════════════════════════
// Float special values
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_float_nan() {
    let nan = f64::NAN;
    assert!(nan.is_nan());
    // NaN != NaN by IEEE 754, but Term equality is bitwise
    let t1 = Term::from_raw(nan.to_bits());
    let t2 = Term::from_raw(nan.to_bits());
    assert_eq!(t1, t2);
}

#[test]
fn test_float_infinity() {
    let inf = f64::INFINITY;
    assert!(inf.is_infinite());
    assert!(inf.is_sign_positive());
    let t = Term::from_raw(inf.to_bits());
    assert_eq!(t.to_raw(), inf.to_bits());
}

#[test]
fn test_float_neg_infinity() {
    let neg_inf = f64::NEG_INFINITY;
    assert!(neg_inf.is_infinite());
    assert!(neg_inf.is_sign_negative());
    let t = Term::from_raw(neg_inf.to_bits());
    assert_eq!(t.to_raw(), neg_inf.to_bits());
}

#[test]
fn test_float_negative_zero() {
    let neg_zero = -0.0f64;
    assert!(neg_zero.is_sign_negative());
    // Negative zero == positive zero in normal comparison
    assert_eq!(neg_zero, 0.0f64);
    // But they have different bit patterns
    let t_neg = Term::from_raw(neg_zero.to_bits());
    let t_pos = Term::from_raw(0.0f64.to_bits());
    // -0.0 and 0.0 have different bit representations
    assert_ne!(t_neg.to_raw(), t_pos.to_raw());
}

#[test]
fn test_float_subnormal() {
    // Smallest positive subnormal f64
    let subnormal = f64::from_bits(0x0000_0000_0000_0001);
    assert!(subnormal.is_subnormal());
    let t = Term::from_raw(subnormal.to_bits());
    assert_eq!(t.to_raw(), subnormal.to_bits());
}

#[test]
fn test_float_max() {
    let max = f64::MAX;
    let t = Term::from_raw(max.to_bits());
    assert_eq!(t.to_raw(), max.to_bits());
}

#[test]
fn test_float_min_positive() {
    let min_pos = f64::MIN_POSITIVE;
    assert!(min_pos > 0.0);
    let t = Term::from_raw(min_pos.to_bits());
    assert_eq!(t.to_raw(), min_pos.to_bits());
}

// ═══════════════════════════════════════════════════════════════════════════
// BigInt edge cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_bigint_i64_min() {
    let big = BigInt::from(i64::MIN);
    let mut heap = Vec::new();
    let term = Term::bigint(&big, &mut heap).expect("Failed to create bignum");
    assert!(term.is_big());
    let recovered = term.get_bigint().expect("Failed to get bignum");
    assert_eq!(recovered, big);
}

#[test]
fn test_bigint_i64_max() {
    let big = BigInt::from(i64::MAX);
    let mut heap = Vec::new();
    let term = Term::bigint(&big, &mut heap).expect("Failed to create bignum");
    assert!(term.is_big());
    let recovered = term.get_bigint().expect("Failed to get bignum");
    assert_eq!(recovered, big);
}

#[test]
fn test_bigint_near_small_int_boundary() {
    // i64::MAX + 1 — just beyond small int range
    let big = BigInt::from(i64::MAX) + BigInt::from(1);
    let mut heap = Vec::new();
    let term = Term::bigint(&big, &mut heap).expect("Failed to create bignum");
    assert!(term.is_big());
    let recovered = term.get_bigint().expect("Failed to get bignum");
    assert_eq!(recovered, big);
}

#[test]
fn test_bigint_near_small_int_boundary_negative() {
    // i64::MIN - 1 — just beyond small int range
    let big = BigInt::from(i64::MIN) - BigInt::from(1);
    let mut heap = Vec::new();
    let term = Term::bigint(&big, &mut heap).expect("Failed to create bignum");
    assert!(term.is_big());
    let recovered = term.get_bigint().expect("Failed to get bignum");
    assert_eq!(recovered, big);
}

#[test]
fn test_bigint_one() {
    let big = BigInt::from(1);
    let mut heap = Vec::new();
    let term = Term::bigint(&big, &mut heap).expect("Failed to create bignum");
    assert!(term.is_big());
    let recovered = term.get_bigint().expect("Failed to get bignum");
    assert_eq!(recovered, big);
}

#[test]
fn test_bigint_negative_one() {
    let big = BigInt::from(-1);
    let mut heap = Vec::new();
    let term = Term::bigint(&big, &mut heap).expect("Failed to create bignum");
    assert!(term.is_big());
    let recovered = term.get_bigint().expect("Failed to get bignum");
    assert_eq!(recovered, big);
}

#[test]
fn test_bigint_very_large() {
    // A number that requires multiple 64-bit limbs
    let big = BigInt::parse_bytes(b"12345678901234567890123456789012345678901234567890", 10)
        .expect("Failed to parse");
    let mut heap = Vec::new();
    let term = Term::bigint(&big, &mut heap).expect("Failed to create bignum");
    assert!(term.is_big());
    let recovered = term.get_bigint().expect("Failed to get bignum");
    assert_eq!(recovered, big);
}

// ═══════════════════════════════════════════════════════════════════════════
// Type predicates on wrong types
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_is_small_on_non_small() {
    assert!(!Term::nil().is_small());
    assert!(!Term::true_().is_small());
    assert!(!Term::false_().is_small());
    assert!(!Term::atom(0).is_small());
    assert!(!Term::from_raw(0).is_small()); // boxed null pointer
}

#[test]
fn test_is_atom_on_non_atom() {
    assert!(!Term::small(42).is_atom());
    assert!(!Term::nil().is_atom());
    assert!(!Term::true_().is_atom());
    assert!(!Term::false_().is_atom());
    assert!(!Term::from_raw(0).is_atom());
}

#[test]
fn test_is_list_on_non_list() {
    assert!(!Term::small(42).is_list());
    assert!(!Term::nil().is_list());
    assert!(!Term::true_().is_list());
    assert!(!Term::atom(0).is_list());
}

#[test]
fn test_is_tuple_on_non_tuple() {
    assert!(!Term::small(42).is_tuple());
    assert!(!Term::nil().is_tuple());
    assert!(!Term::atom(0).is_tuple());
    assert!(!Term::true_().is_tuple());
}

#[test]
fn test_is_map_on_non_map() {
    assert!(!Term::small(42).is_map());
    assert!(!Term::nil().is_map());
    assert!(!Term::atom(0).is_map());
    assert!(!Term::true_().is_map());
}

#[test]
fn test_is_float_on_non_float() {
    assert!(!Term::small(42).is_float());
    assert!(!Term::nil().is_float());
    assert!(!Term::atom(0).is_float());
    assert!(!Term::true_().is_float());
}

#[test]
fn test_is_binary_on_non_binary() {
    assert!(!Term::small(42).is_binary());
    assert!(!Term::nil().is_binary());
    assert!(!Term::atom(0).is_binary());
    assert!(!Term::true_().is_binary());
}

#[test]
fn test_is_big_on_non_big() {
    assert!(!Term::small(42).is_big());
    assert!(!Term::nil().is_big());
    assert!(!Term::atom(0).is_big());
    assert!(!Term::true_().is_big());
}

#[test]
fn test_is_nil_on_non_nil() {
    assert!(!Term::small(0).is_nil());
    assert!(!Term::true_().is_nil());
    assert!(!Term::false_().is_nil());
    assert!(!Term::atom(0).is_nil());
}

#[test]
fn test_is_true_on_non_true() {
    assert!(!Term::small(1).is_true());
    assert!(!Term::nil().is_true());
    assert!(!Term::false_().is_true());
    assert!(!Term::atom(0).is_true());
}

#[test]
fn test_is_false_on_non_false() {
    assert!(!Term::small(0).is_false());
    assert!(!Term::nil().is_false());
    assert!(!Term::true_().is_false());
    assert!(!Term::atom(0).is_false());
}

#[test]
fn test_is_pid_on_non_pid() {
    assert!(!Term::small(42).is_pid());
    assert!(!Term::nil().is_pid());
    assert!(!Term::atom(0).is_pid());
    assert!(!Term::true_().is_pid());
}

#[test]
fn test_is_port_on_non_port() {
    assert!(!Term::small(42).is_port());
    assert!(!Term::nil().is_port());
    assert!(!Term::atom(0).is_port());
    assert!(!Term::true_().is_port());
}

#[test]
fn test_is_fun_on_non_fun() {
    assert!(!Term::small(42).is_fun());
    assert!(!Term::nil().is_fun());
    assert!(!Term::atom(0).is_fun());
    assert!(!Term::true_().is_fun());
}

#[test]
fn test_is_catch_on_non_catch() {
    assert!(!Term::small(42).is_catch());
    assert!(!Term::nil().is_catch());
    assert!(!Term::atom(0).is_catch());
    assert!(!Term::true_().is_catch());
}

// ═══════════════════════════════════════════════════════════════════════════
// get_small / get_atom_index / get_boxed_ptr on wrong types
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_get_small_on_non_small() {
    assert_eq!(Term::nil().get_small(), None);
    assert_eq!(Term::true_().get_small(), None);
    assert_eq!(Term::atom(0).get_small(), None);
    assert_eq!(Term::from_raw(0).get_small(), None);
}

#[test]
fn test_get_atom_index_on_non_atom() {
    assert_eq!(Term::small(42).get_atom_index(), None);
    assert_eq!(Term::nil().get_atom_index(), None);
    assert_eq!(Term::true_().get_atom_index(), None);
    assert_eq!(Term::from_raw(0).get_atom_index(), None);
}

#[test]
fn test_get_boxed_ptr_on_non_boxed() {
    // Non-boxed types should return a pointer derived from their raw bits
    // masked with BOXED_PTR_MASK — this is the actual behavior
    let small = Term::small(42);
    let ptr = small.get_boxed_ptr();
    // The pointer is (raw & BOXED_PTR_MASK) which for small ints
    // strips the lower 2 bits — it's not necessarily null
    // Just verify it doesn't panic
    let _ = ptr;

    let atom = Term::atom(5);
    let _ = atom.get_boxed_ptr();
}

// ═══════════════════════════════════════════════════════════════════════════
// tuple_get edge cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
#[should_panic]
fn test_tuple_get_out_of_bounds() {
    // Create a tuple with 2 elements on the heap
    let mut heap = Vec::new();
    // Header for tuple with arity 2
    let header = tags::HEADER_TUPLE | 2u64;
    heap.push(Term::from_raw(header));
    heap.push(Term::small(10));
    heap.push(Term::small(20));

    let term = Term::from_raw((heap.as_ptr() as u64) | tags::PRIMARY_TAG_BOXED);
    assert!(term.is_tuple());

    // Accessing index 0 and 1 should work
    assert_eq!(term.tuple_get(0).unwrap_small(), 10);
    assert_eq!(term.tuple_get(1).unwrap_small(), 20);

    // Accessing index 2 should panic (debug_assert)
    let _ = term.tuple_get(2);
}

#[test]
fn test_tuple_get_on_non_tuple() {
    // tuple_get uses debug_assert, so in release mode it won't panic
    // but will read garbage. In debug mode it panics.
    // We just verify the function can be called without UB on a non-tuple
    // boxed value.
    let mut heap = Vec::new();
    // A non-tuple header (e.g., HEADER_FLOAT)
    let header = tags::HEADER_FLOAT | 0x0003;
    heap.push(Term::from_raw(header));
    heap.push(Term::from_raw(0));

    let term = Term::from_raw((heap.as_ptr() as u64) | tags::PRIMARY_TAG_BOXED);
    assert!(!term.is_tuple());
    // This is technically UB if the heap doesn't have enough words,
    // but the debug_assert only fires in debug builds
    if cfg!(debug_assertions) {
        // In debug mode, this would panic — skip the call
    } else {
        let _ = term.tuple_get(0);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// header / header_arity / header_tag on various types
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_header_arity_tuple() {
    let header = tags::HEADER_TUPLE | 5u64;
    assert_eq!(Term::header_arity(header), 5);
}

#[test]
fn test_header_arity_zero() {
    let header = tags::HEADER_TUPLE | 0u64;
    assert_eq!(Term::header_arity(header), 0);
}

#[test]
fn test_header_arity_max() {
    // Max arity is 0x0000_FFFF_FFFF
    let header = tags::HEADER_TUPLE | tags::HEADER_ARITY_MASK;
    assert_eq!(Term::header_arity(header), tags::HEADER_ARITY_MASK as usize);
}

#[test]
fn test_header_tag_extraction() {
    // Verify header_tag returns the correct tag bits
    let tuple_header = tags::HEADER_TUPLE | 3u64;
    assert_eq!(tuple_header & tags::HEADER_TAG_MASK, tags::HEADER_TUPLE);

    let float_header = tags::HEADER_FLOAT | 0x0003;
    assert_eq!(float_header & tags::HEADER_TAG_MASK, tags::HEADER_FLOAT);

    let map_header = tags::HEADER_MAP | 2u64;
    assert_eq!(map_header & tags::HEADER_TAG_MASK, tags::HEADER_MAP);
}

// ═══════════════════════════════════════════════════════════════════════════
// Debug formatting for all types
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_debug_small_int() {
    let t = Term::small(42);
    let dbg = format!("{:?}", t);
    assert!(dbg.contains("42"), "Debug of small int 42 was: {}", dbg);
}

#[test]
fn test_debug_negative_small_int() {
    let t = Term::small(-1);
    let dbg = format!("{:?}", t);
    assert!(dbg.contains("-1"), "Debug of small int -1 was: {}", dbg);
}

#[test]
fn test_debug_nil() {
    let t = Term::nil();
    assert_eq!(format!("{:?}", t), "[]");
}

#[test]
fn test_debug_true() {
    let t = Term::true_();
    assert_eq!(format!("{:?}", t), "true");
}

#[test]
fn test_debug_false() {
    let t = Term::false_();
    assert_eq!(format!("{:?}", t), "false");
}

#[test]
fn test_debug_atom() {
    let t = Term::atom(42);
    let dbg = format!("{:?}", t);
    assert!(dbg.contains("atom"), "Debug of atom was: {}", dbg);
    assert!(dbg.contains("42"), "Debug of atom(42) was: {}", dbg);
}

#[test]
fn test_debug_list() {
    let t = Term::from_raw(0x0000_0000_0000_0001u64); // primary tag = 0b01 (list)
    let dbg = format!("{:?}", t);
    assert!(dbg.contains("list"), "Debug of list was: {}", dbg);
}

#[test]
fn test_debug_pid() {
    let t = Term::from_raw(tags::IMMED1_PID | 0x1234);
    let dbg = format!("{:?}", t);
    assert!(dbg.contains("pid"), "Debug of pid was: {}", dbg);
}

#[test]
fn test_debug_port() {
    let t = Term::from_raw(tags::IMMED1_PORT | 0x1234);
    let dbg = format!("{:?}", t);
    assert!(dbg.contains("port"), "Debug of port was: {}", dbg);
}

#[test]
fn test_debug_tuple() {
    let mut heap = Vec::new();
    let header = tags::HEADER_TUPLE | 3u64;
    heap.push(Term::from_raw(header));
    heap.push(Term::small(1));
    heap.push(Term::small(2));
    heap.push(Term::small(3));

    let term = Term::from_raw((heap.as_ptr() as u64) | tags::PRIMARY_TAG_BOXED);
    let dbg = format!("{:?}", term);
    assert!(dbg.contains("tuple"), "Debug of tuple was: {}", dbg);
    assert!(
        dbg.contains("3"),
        "Debug of tuple with arity 3 was: {}",
        dbg
    );
}

#[test]
fn test_debug_bigint() {
    let big = BigInt::from(123456789012345678901234567890i128);
    let mut heap = Vec::new();
    let term = Term::bigint(&big, &mut heap).expect("Failed to create bignum");
    let dbg = format!("{:?}", term);
    assert!(dbg.contains("big"), "Debug of bigint was: {}", dbg);
}

#[test]
fn test_debug_fun() {
    let t = Term::from_raw((0x1000_0000_0000_0000u64 | tags::HEADER_FUN) | tags::PRIMARY_TAG_BOXED);
    let dbg = format!("{:?}", t);
    assert!(dbg.contains("fun"), "Debug of fun was: {}", dbg);
}

#[test]
fn test_debug_map() {
    let t = Term::from_raw((0x1000_0000_0000_0000u64 | tags::HEADER_MAP) | tags::PRIMARY_TAG_BOXED);
    let dbg = format!("{:?}", t);
    assert!(dbg.contains("map"), "Debug of map was: {}", dbg);
}

#[test]
fn test_debug_boxed() {
    // A boxed value with an unrecognized header tag
    let t = Term::from_raw(
        (0x1000_0000_0000_0000u64 | (0xFFFFu64 << tags::HEADER_TAG_POS)) | tags::PRIMARY_TAG_BOXED,
    );
    let dbg = format!("{:?}", t);
    assert!(dbg.contains("boxed"), "Debug of boxed was: {}", dbg);
}

#[test]
fn test_debug_unknown_term() {
    // A term that doesn't match any known pattern
    let t = Term::from_raw(0xFFFF_FFFF_FFFF_FFF0u64);
    let dbg = format!("{:?}", t);
    assert!(dbg.contains("term"), "Debug of unknown term was: {}", dbg);
}

// ═══════════════════════════════════════════════════════════════════════════
// Hash consistency
// ═══════════════════════════════════════════════════════════════════════════

fn hash_of(term: &Term) -> u64 {
    let mut hasher = DefaultHasher::new();
    term.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn test_hash_consistency_same_value() {
    let t1 = Term::small(42);
    let t2 = Term::small(42);
    assert_eq!(hash_of(&t1), hash_of(&t2));
}

#[test]
fn test_hash_consistency_different_values() {
    let t1 = Term::small(42);
    let t2 = Term::small(43);
    // Different values should (almost certainly) have different hashes
    assert_ne!(hash_of(&t1), hash_of(&t2));
}

#[test]
fn test_hash_consistency_across_types() {
    // Different types should produce different hashes
    let small = Term::small(0);
    let nil = Term::nil();
    let true_ = Term::true_();
    let false_ = Term::false_();
    let atom0 = Term::atom(0);

    let hashes = vec![
        hash_of(&small),
        hash_of(&nil),
        hash_of(&true_),
        hash_of(&false_),
        hash_of(&atom0),
    ];

    // All should be unique
    for i in 0..hashes.len() {
        for j in (i + 1)..hashes.len() {
            assert_ne!(
                hashes[i], hashes[j],
                "Hash collision between types at indices {} and {}",
                i, j
            );
        }
    }
}

#[test]
fn test_hash_equal_terms() {
    // Terms that are equal should have the same hash
    let t1 = Term::atom(5);
    let t2 = Term::atom(5);
    assert_eq!(t1, t2);
    assert_eq!(hash_of(&t1), hash_of(&t2));
}

#[test]
fn test_hash_bool_values() {
    let t = Term::true_();
    let f = Term::false_();
    assert_ne!(hash_of(&t), hash_of(&f));
}

// ═══════════════════════════════════════════════════════════════════════════
// RegisterFile new / default
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_register_file_new() {
    let rf = RegisterFile::new();
    // All X registers should be nil
    for i in 0..256 {
        assert_eq!(rf.x[i], Term::nil(), "x[{}] should be nil", i);
    }
    // All Y registers should be nil
    for i in 0..1024 {
        assert_eq!(rf.y[i], Term::nil(), "y[{}] should be nil", i);
    }
    // All F registers should be 0.0
    for i in 0..256 {
        assert_eq!(rf.f[i], 0.0f64, "f[{}] should be 0.0", i);
    }
}

#[test]
fn test_register_file_default() {
    let rf: RegisterFile = Default::default();
    assert_eq!(rf.x[0], Term::nil());
    assert_eq!(rf.y[0], Term::nil());
    assert_eq!(rf.f[0], 0.0f64);
}

#[test]
fn test_register_file_clone() {
    let rf = RegisterFile::new();
    let rf2 = rf.clone();
    assert_eq!(rf.x[0], rf2.x[0]);
    assert_eq!(rf.y[0], rf2.y[0]);
    assert_eq!(rf.f[0], rf2.f[0]);
}

#[test]
fn test_register_file_debug() {
    let rf = RegisterFile::new();
    let dbg = format!("{:?}", rf);
    assert!(
        dbg.contains("RegisterFile"),
        "Debug of RegisterFile was: {}",
        dbg
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Term default
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_term_default_is_nil() {
    let t: Term = Default::default();
    assert!(t.is_nil());
}

// ═══════════════════════════════════════════════════════════════════════════
// Bool construction
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_bool_true() {
    let t = Term::bool(true);
    assert!(t.is_true());
    assert!(!t.is_false());
}

#[test]
fn test_bool_false() {
    let t = Term::bool(false);
    assert!(t.is_false());
    assert!(!t.is_true());
}

// ═══════════════════════════════════════════════════════════════════════════
// Raw roundtrip
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_raw_roundtrip() {
    let values = [0u64, 1, 0xFFFF_FFFF_FFFF_FFFF, 0xDEAD_BEEF_CAFE_BABE];
    for &val in &values {
        let t = Term::from_raw(val);
        assert_eq!(t.to_raw(), val);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Atom index boundaries
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_atom_index_zero() {
    let t = Term::atom(0);
    assert!(t.is_atom());
    assert_eq!(t.get_atom_index(), Some(0));
}

#[test]
fn test_atom_index_max() {
    let t = Term::atom(u32::MAX);
    assert!(t.is_atom());
    assert_eq!(t.get_atom_index(), Some(u32::MAX));
}

#[test]
fn test_atom_equality() {
    let a1 = Term::atom(5);
    let a2 = Term::atom(5);
    let a3 = Term::atom(6);
    assert_eq!(a1, a2);
    assert_ne!(a1, a3);
}
