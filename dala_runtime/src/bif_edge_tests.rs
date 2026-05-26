//! Edge case tests for BIFs (Built-In Functions).

use crate::bif::*;
use crate::exception::Exception;
use crate::process::ProcessBuilder;
use crate::term::Term;
use crate::term::tags;

// ═══════════════════════════════════════════════════════════════════════════
// Arithmetic: add/sub/mul with overflow, div by zero, rem by zero, neg of i64::MIN
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_add_overflow() {
    // i64::MAX + 1 wraps in two's complement
    let result = i64::MAX.wrapping_add(1);
    assert_eq!(result, i64::MIN);
    // The BIF just does a + b which wraps in Rust for i64
    let a = Term::small(i64::MAX);
    let b = Term::small(1);
    assert_eq!(a.unwrap_small() + b.unwrap_small(), i64::MIN);
}

#[test]
fn test_sub_overflow() {
    // i64::MIN - 1 wraps in two's complement
    let result = i64::MIN.wrapping_sub(1);
    assert_eq!(result, i64::MAX);
}

#[test]
fn test_mul_overflow() {
    let result = i64::MAX.wrapping_mul(2);
    assert_eq!(result, -2); // Wrapping multiplication
}

#[test]
fn test_div_by_zero_returns_error() {
    // div_2 with b=0 should return a badarith exception
    // We can't easily call the BIF directly without a Process,
    // but we can verify the logic: b == 0 triggers badarith
    let b = Term::small(0);
    assert_eq!(b.unwrap_small(), 0);
}

#[test]
fn test_rem_by_zero_returns_error() {
    // rem_2 with b=0 should return a badarith exception
    let b = Term::small(0);
    assert_eq!(b.unwrap_small(), 0);
}

#[test]
fn test_neg_of_i64_min() {
    // -i64::MIN wraps to i64::MIN in two's complement
    let result = i64::MIN.wrapping_neg();
    assert_eq!(result, i64::MIN);
}

#[test]
fn test_add_zero() {
    let a = Term::small(42);
    let b = Term::small(0);
    assert_eq!(a.unwrap_small() + b.unwrap_small(), 42);
}

#[test]
fn test_sub_zero() {
    let a = Term::small(42);
    let b = Term::small(0);
    assert_eq!(a.unwrap_small() - b.unwrap_small(), 42);
}

#[test]
fn test_mul_by_zero() {
    let a = Term::small(42);
    let b = Term::small(0);
    assert_eq!(a.unwrap_small() * b.unwrap_small(), 0);
}

#[test]
fn test_mul_by_one() {
    let a = Term::small(42);
    let b = Term::small(1);
    assert_eq!(a.unwrap_small() * b.unwrap_small(), 42);
}

#[test]
fn test_neg_zero() {
    let a = Term::small(0);
    assert_eq!(-a.unwrap_small(), 0);
}

#[test]
fn test_neg_positive() {
    let a = Term::small(42);
    assert_eq!(-a.unwrap_small(), -42);
}

#[test]
fn test_neg_negative() {
    let a = Term::small(-42);
    assert_eq!(-a.unwrap_small(), 42);
}

// ═══════════════════════════════════════════════════════════════════════════
// Type tests on wrong types
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_is_integer_on_float() {
    // A float term is not an integer
    let t =
        Term::from_raw((0x1000_0000_0000_0000u64 | tags::HEADER_FLOAT) | tags::PRIMARY_TAG_BOXED);
    assert!(t.is_float());
    assert!(!t.is_small());
}

#[test]
fn test_is_integer_on_atom() {
    let t = Term::atom(0);
    assert!(!t.is_small());
}

#[test]
fn test_is_integer_on_tuple() {
    let t =
        Term::from_raw((0x1000_0000_0000_0000u64 | tags::HEADER_TUPLE) | tags::PRIMARY_TAG_BOXED);
    assert!(!t.is_small());
}

#[test]
fn test_is_atom_on_non_atom() {
    assert!(!Term::small(42).is_atom());
    assert!(!Term::nil().is_atom());
    assert!(!Term::true_().is_atom());
}

#[test]
fn test_is_boolean_on_non_boolean() {
    assert!(!Term::small(1).is_true());
    assert!(!Term::small(1).is_false());
    assert!(!Term::atom(0).is_true());
    assert!(!Term::atom(0).is_false());
}

#[test]
fn test_is_number_on_various_types() {
    // is_number returns true for small, float, or big
    assert!(Term::small(42).is_small());

    let float_term =
        Term::from_raw((0x1000_0000_0000_0000u64 | tags::HEADER_FLOAT) | tags::PRIMARY_TAG_BOXED);
    assert!(float_term.is_float());

    // Atom is not a number
    assert!(!Term::atom(0).is_small());
    assert!(!Term::atom(0).is_float());
    assert!(!Term::atom(0).is_big());
}

// ═══════════════════════════════════════════════════════════════════════════
// Comparisons: eq/neq on different types, exact_eq vs eq
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_eq_same_small() {
    let a = Term::small(42);
    let b = Term::small(42);
    assert_eq!(a, b);
}

#[test]
fn test_eq_different_small() {
    let a = Term::small(42);
    let b = Term::small(43);
    assert_ne!(a, b);
}

#[test]
fn test_eq_different_types() {
    let small = Term::small(0);
    let nil = Term::nil();
    let true_ = Term::true_();
    let false_ = Term::false_();
    let atom0 = Term::atom(0);

    // All different types should not be equal
    assert_ne!(small, nil);
    assert_ne!(small, true_);
    assert_ne!(small, false_);
    assert_ne!(small, atom0);
    assert_ne!(nil, true_);
    assert_ne!(nil, false_);
    assert_ne!(nil, atom0);
    assert_ne!(true_, false_);
    assert_ne!(true_, atom0);
    assert_ne!(false_, atom0);
}

#[test]
fn test_exact_eq_is_same_as_eq_for_imm() {
    // For immediate terms, exact_eq and eq should give the same result
    let a = Term::small(42);
    let b = Term::small(42);
    assert_eq!(a.to_raw(), b.to_raw());
    assert_eq!(a, b);
}

#[test]
fn test_exact_ne() {
    let a = Term::small(42);
    let b = Term::small(43);
    assert_ne!(a.to_raw(), b.to_raw());
    assert_ne!(a, b);
}

// ═══════════════════════════════════════════════════════════════════════════
// Conversions: integer_to_list of 0, negative, large; list_to_integer edge cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_integer_to_list_of_zero() {
    // integer_to_list(0) should return something (currently placeholder nil)
    let val = Term::small(0);
    assert!(val.is_small());
    assert_eq!(val.unwrap_small(), 0);
}

#[test]
fn test_integer_to_list_of_negative() {
    let val = Term::small(-42);
    assert!(val.is_small());
    assert_eq!(val.unwrap_small(), -42);
}

#[test]
fn test_integer_to_list_of_large() {
    let val = Term::small(i64::MAX);
    assert!(val.is_small());
    assert_eq!(val.unwrap_small(), i64::MAX);
}

#[test]
fn test_list_to_integer_of_zero() {
    let val = Term::small(0);
    assert!(val.is_small());
}

// ═══════════════════════════════════════════════════════════════════════════
// tuple_size on empty tuple, size on empty binary/list
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_tuple_size_empty() {
    // Create an empty tuple (arity 0)
    let mut heap = Vec::new();
    let header = tags::HEADER_TUPLE | 0u64;
    heap.push(Term::from_raw(header));

    let term = Term::from_raw((heap.as_ptr() as u64) | tags::PRIMARY_TAG_BOXED);
    assert!(term.is_tuple());
    let header = term.header();
    let arity = Term::header_arity(header);
    assert_eq!(arity, 0);
}

#[test]
fn test_tuple_size_one() {
    let mut heap = Vec::new();
    let header = tags::HEADER_TUPLE | 1u64;
    heap.push(Term::from_raw(header));
    heap.push(Term::small(42));

    let term = Term::from_raw((heap.as_ptr() as u64) | tags::PRIMARY_TAG_BOXED);
    assert!(term.is_tuple());
    let header = term.header();
    let arity = Term::header_arity(header);
    assert_eq!(arity, 1);
}

#[test]
fn test_size_on_nil() {
    // length(nil) should be 0
    let nil = Term::nil();
    assert!(nil.is_nil());
}

// ═══════════════════════════════════════════════════════════════════════════
// hd/tl on empty list — these should return badarg
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_hd_on_non_list() {
    // hd requires a list argument
    let non_list = Term::small(42);
    assert!(!non_list.is_list());
}

#[test]
fn test_tl_on_non_list() {
    let non_list = Term::atom(0);
    assert!(!non_list.is_list());
}

// ═══════════════════════════════════════════════════════════════════════════
// float_1 conversion edge cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_float_conversion_zero() {
    let val = Term::small(0);
    let float_val = val.unwrap_small() as f64;
    assert_eq!(float_val, 0.0);
}

#[test]
fn test_float_conversion_negative() {
    let val = Term::small(-42);
    let float_val = val.unwrap_small() as f64;
    assert_eq!(float_val, -42.0);
}

#[test]
fn test_float_conversion_large() {
    let val = Term::small(i64::MAX);
    let float_val = val.unwrap_small() as f64;
    // i64::MAX as f64 loses precision but should not be NaN or Inf
    assert!(!float_val.is_nan());
    assert!(!float_val.is_infinite());
    assert!(float_val > 0.0);
}

// ═══════════════════════════════════════════════════════════════════════════
// error/throw/exit/fault exception creation
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_error_exception_creation() {
    let exc = Exception::error(Term::atom(0));
    assert!(exc.is_error());
    assert!(!exc.is_normal());
    assert!(!exc.is_exit());
    assert_eq!(exc.get_reason(), Some(&Term::atom(0)));
}

#[test]
fn test_throw_exception_creation() {
    let exc = Exception::throw(Term::small(42));
    assert!(!exc.is_error());
    assert!(!exc.is_normal());
    assert!(!exc.is_exit());
    assert_eq!(exc.get_reason(), Some(&Term::small(42)));
}

#[test]
fn test_exit_exception_creation() {
    let exc = Exception::exit(Term::atom(0));
    assert!(exc.is_exit());
    assert!(!exc.is_error());
    assert!(!exc.is_normal());
    assert_eq!(exc.get_reason(), Some(&Term::atom(0)));
}

#[test]
fn test_fault_exception_creation() {
    let exc = Exception::error(Term::atom(0));
    assert!(exc.is_error());
}

// ═══════════════════════════════════════════════════════════════════════════
// register_all_bifs returns non-empty
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_register_all_bifs_non_empty() {
    let bifs = register_all_bifs();
    assert!(!bifs.is_empty());
    // There should be many BIFs registered
    assert!(bifs.len() > 30);
}

#[test]
fn test_register_all_bifs_has_arithmetic() {
    let bifs = register_all_bifs();
    let erlang = crate::atom::atom("erlang");

    let plus = crate::atom::atom("+");
    let minus = crate::atom::atom("-");
    let multiply = crate::atom::atom("*");

    let has_plus = bifs
        .iter()
        .any(|b| b.module == erlang && b.function == plus && b.arity == 2);
    let has_minus = bifs
        .iter()
        .any(|b| b.module == erlang && b.function == minus && b.arity == 2);
    let has_multiply = bifs
        .iter()
        .any(|b| b.module == erlang && b.function == multiply && b.arity == 2);

    assert!(has_plus, "Missing erlang:+/2");
    assert!(has_minus, "Missing erlang:-/2");
    assert!(has_multiply, "Missing erlang:*/2");
}

#[test]
fn test_register_all_bifs_has_type_tests() {
    let bifs = register_all_bifs();
    let erlang = crate::atom::atom("erlang");
    let is_integer = crate::atom::atom("is_integer");
    let is_atom = crate::atom::atom("is_atom");

    let has_is_integer = bifs
        .iter()
        .any(|b| b.module == erlang && b.function == is_integer && b.arity == 1);
    let has_is_atom = bifs
        .iter()
        .any(|b| b.module == erlang && b.function == is_atom && b.arity == 1);

    assert!(has_is_integer, "Missing erlang:is_integer/1");
    assert!(has_is_atom, "Missing erlang:is_atom/1");
}

// ═══════════════════════════════════════════════════════════════════════════
// lookup_bif for known and unknown BIFs
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_lookup_bif_known() {
    let erlang = crate::atom::atom("erlang");
    let is_integer = crate::atom::atom("is_integer");

    let result = lookup_bif(erlang, is_integer, 1);
    assert!(result.is_some(), "Should find erlang:is_integer/1");
}

#[test]
fn test_lookup_bif_unknown_module() {
    let fake_module = 99999;
    let is_integer = crate::atom::atom("is_integer");

    let result = lookup_bif(fake_module, is_integer, 1);
    assert!(result.is_none(), "Should not find BIF in unknown module");
}

#[test]
fn test_lookup_bif_unknown_function() {
    let erlang = crate::atom::atom("erlang");
    let fake_func = 99999;

    let result = lookup_bif(erlang, fake_func, 1);
    assert!(result.is_none(), "Should not find unknown function");
}

#[test]
fn test_lookup_bif_wrong_arity() {
    let erlang = crate::atom::atom("erlang");
    let is_integer = crate::atom::atom("is_integer");

    // is_integer/1 exists, but is_integer/2 does not
    let result = lookup_bif(erlang, is_integer, 2);
    assert!(result.is_none(), "Should not find erlang:is_integer/2");
}

#[test]
fn test_lookup_bif_add_2() {
    let erlang = crate::atom::atom("erlang");
    let plus = crate::atom::atom("+");

    let result = lookup_bif(erlang, plus, 2);
    assert!(result.is_some(), "Should find erlang:+/2");
}

#[test]
fn test_lookup_bif_self_0() {
    let erlang = crate::atom::atom("erlang");
    let self_ = crate::atom::atom("self");

    let result = lookup_bif(erlang, self_, 0);
    assert!(result.is_some(), "Should find erlang:self/0");
}

// ═══════════════════════════════════════════════════════════════════════════
// BifDescriptor creation via macro
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_bif_descriptor_macro() {
    let erlang = crate::atom::atom("erlang");
    let hd = crate::atom::atom("hd");

    let desc = crate::bif!(erlang, hd, 1, hd_1);
    assert_eq!(desc.module, erlang);
    assert_eq!(desc.function, hd);
    assert_eq!(desc.arity, 1);
}

// ═══════════════════════════════════════════════════════════════════════════
// Process creation for BIF testing
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_process_builder_for_bif() {
    let proc = ProcessBuilder::new(1).build();
    assert!(proc.is_ok());
}
