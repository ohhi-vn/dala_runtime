//! Edge case tests for Exception/Reason.

use crate::exception::*;
use crate::term::Term;

// ═══════════════════════════════════════════════════════════════════════════
// All Reason variants, is_normal/is_error/is_exit
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_reason_normal() {
    let r = Reason::Normal;
    assert_eq!(format!("{}", r), "normal");
}

#[test]
fn test_reason_error() {
    let r = Reason::Error(Term::atom(0));
    let s = format!("{}", r);
    assert!(s.contains("error"), "Error display was: {}", s);
}

#[test]
fn test_reason_exit() {
    let r = Reason::Exit(Term::atom(0));
    let s = format!("{}", r);
    assert!(s.contains("exit"), "Exit display was: {}", s);
}

#[test]
fn test_reason_throw() {
    let r = Reason::Throw(Term::small(42));
    let s = format!("{}", r);
    assert!(s.contains("throw"), "Throw display was: {}", s);
}

// ═══════════════════════════════════════════════════════════════════════════
// get_reason on Normal returns None
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_get_reason_on_normal() {
    let exc = Exception::from(Reason::Normal);
    assert!(exc.get_reason().is_none());
}

#[test]
fn test_get_reason_on_error() {
    let exc = Exception::error(Term::small(42));
    assert_eq!(exc.get_reason(), Some(&Term::small(42)));
}

#[test]
fn test_get_reason_on_exit() {
    let exc = Exception::exit(Term::atom(0));
    assert_eq!(exc.get_reason(), Some(&Term::atom(0)));
}

#[test]
fn test_get_reason_on_throw() {
    let exc = Exception::throw(Term::nil());
    assert_eq!(exc.get_reason(), Some(&Term::nil()));
}

// ═══════════════════════════════════════════════════════════════════════════
// Exception from Reason conversion
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_exception_from_normal() {
    let exc: Exception = Reason::Normal.into();
    assert!(exc.is_normal());
    assert!(exc.stacktrace.is_empty());
}

#[test]
fn test_exception_from_error() {
    let exc: Exception = Reason::Error(Term::small(1)).into();
    assert!(exc.is_error());
    assert!(exc.stacktrace.is_empty());
}

#[test]
fn test_exception_from_exit() {
    let exc: Exception = Reason::Exit(Term::small(1)).into();
    assert!(exc.is_exit());
    assert!(exc.stacktrace.is_empty());
}

#[test]
fn test_exception_from_throw() {
    let exc: Exception = Reason::Throw(Term::small(1)).into();
    assert!(!exc.is_error());
    assert!(!exc.is_exit());
    assert!(!exc.is_normal());
}

// ═══════════════════════════════════════════════════════════════════════════
// Display formatting for all variants
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_exception_display_normal() {
    let exc = Exception::from(Reason::Normal);
    let s = format!("{}", exc);
    assert!(s.contains("normal"), "Normal exception display was: {}", s);
}

#[test]
fn test_exception_display_error() {
    let exc = Exception::error(Term::atom(0));
    let s = format!("{}", exc);
    assert!(s.contains("error"), "Error exception display was: {}", s);
}

#[test]
fn test_exception_display_with_stacktrace() {
    let mut exc = Exception::error(Term::atom(0));
    exc.stacktrace.push(StackFrame {
        module: 1,
        function: 2,
        arity: 3,
        file: 4,
        line: 42,
    });
    let s = format!("{}", exc);
    assert!(s.contains("Stacktrace"), "Stacktrace display was: {}", s);
    assert!(
        s.contains("error"),
        "Error in stacktrace display was: {}",
        s
    );
}

#[test]
fn test_exception_display_empty_stacktrace() {
    let exc = Exception::error(Term::atom(0));
    let s = format!("{}", exc);
    // Should not contain "Stacktrace:" when empty
    assert!(
        !s.contains("Stacktrace"),
        "Empty stacktrace display was: {}",
        s
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// StackFrame creation and display
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_stack_frame_creation() {
    let frame = StackFrame {
        module: 10,
        function: 20,
        arity: 3,
        file: 5,
        line: 100,
    };
    assert_eq!(frame.module, 10);
    assert_eq!(frame.function, 20);
    assert_eq!(frame.arity, 3);
    assert_eq!(frame.file, 5);
    assert_eq!(frame.line, 100);
}

#[test]
fn test_stack_frame_clone() {
    let frame = StackFrame {
        module: 1,
        function: 2,
        arity: 3,
        file: 4,
        line: 5,
    };
    let frame2 = frame.clone();
    assert_eq!(frame.module, frame2.module);
    assert_eq!(frame.function, frame2.function);
    assert_eq!(frame.arity, frame2.arity);
    assert_eq!(frame.file, frame2.file);
    assert_eq!(frame.line, frame2.line);
}

#[test]
fn test_stack_frame_debug() {
    let frame = StackFrame {
        module: 1,
        function: 2,
        arity: 3,
        file: 4,
        line: 5,
    };
    let dbg = format!("{:?}", frame);
    assert!(dbg.contains("StackFrame"), "Debug was: {}", dbg);
}

// ═══════════════════════════════════════════════════════════════════════════
// Result helpers
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_ok_term() {
    let result = ok_term(Term::small(42));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Term::small(42));
}

#[test]
fn test_error_term() {
    let result = error_term(Term::atom(0));
    assert!(result.is_err());
    let exc = result.unwrap_err();
    assert!(exc.is_error());
}

#[test]
fn test_exit_term() {
    let result = exit_term(Term::atom(0));
    assert!(result.is_err());
    let exc = result.unwrap_err();
    assert!(exc.is_exit());
}

#[test]
fn test_throw_term() {
    let result = throw_term(Term::small(42));
    assert!(result.is_err());
    let exc = result.unwrap_err();
    assert!(!exc.is_error());
    assert!(!exc.is_exit());
}

#[test]
fn test_exception_result() {
    let result: crate::exception::Result = exception_result(Reason::Error(Term::small(1)));
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// unwrap_result on Ok, is_exception, is_ok
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_is_exception_on_ok() {
    let result: Result = ok_term(Term::small(42));
    assert!(!is_exception(&result));
}

#[test]
fn test_is_exception_on_err() {
    let result: Result = error_term(Term::atom(0));
    assert!(is_exception(&result));
}

#[test]
fn test_is_ok_on_ok() {
    let result: Result = ok_term(Term::small(42));
    assert!(is_ok(&result));
}

#[test]
fn test_is_ok_on_err() {
    let result: Result = error_term(Term::atom(0));
    assert!(!is_ok(&result));
}

// ═══════════════════════════════════════════════════════════════════════════
// map_result, and_then_result chaining
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_map_result_ok() {
    let result: Result = ok_term(Term::small(5));
    let mapped = map_result(result, |t| {
        let val = t.unwrap_small();
        Term::small(val * 2)
    });
    assert!(mapped.is_ok());
    assert_eq!(mapped.unwrap().unwrap_small(), 10);
}

#[test]
fn test_map_result_err() {
    let result: Result = error_term(Term::atom(0));
    let mapped = map_result(result, |_t| Term::small(999));
    assert!(mapped.is_err());
}

#[test]
fn test_and_then_result_ok() {
    let result: Result = ok_term(Term::small(5));
    let chained = and_then_result(result, |t| {
        let val = t.unwrap_small();
        ok_term(Term::small(val + 1))
    });
    assert!(chained.is_ok());
    assert_eq!(chained.unwrap().unwrap_small(), 6);
}

#[test]
fn test_and_then_result_err() {
    let result: Result = error_term(Term::atom(0));
    let chained = and_then_result(result, |_t| ok_term(Term::small(999)));
    assert!(chained.is_err());
}

#[test]
fn test_and_then_result_chained_error() {
    let result: Result = ok_term(Term::small(5));
    let chained = and_then_result(result, |_t| error_term(Term::atom(0)));
    assert!(chained.is_err());
}

#[test]
fn test_propagate_ok() {
    let result: Result = ok_term(Term::small(42));
    let propagated = propagate(result);
    assert!(propagated.is_ok());
    assert_eq!(propagated.unwrap(), Term::small(42));
}

#[test]
fn test_propagate_err() {
    let result: Result = error_term(Term::atom(0));
    let propagated = propagate(result);
    assert!(propagated.is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// Exception is_normal / is_error / is_exit
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_is_normal() {
    let exc = Exception::from(Reason::Normal);
    assert!(exc.is_normal());
    assert!(!exc.is_error());
    assert!(!exc.is_exit());
}

#[test]
fn test_is_error() {
    let exc = Exception::error(Term::small(1));
    assert!(!exc.is_normal());
    assert!(exc.is_error());
    assert!(!exc.is_exit());
}

#[test]
fn test_is_exit() {
    let exc = Exception::exit(Term::small(1));
    assert!(!exc.is_normal());
    assert!(!exc.is_error());
    assert!(exc.is_exit());
}

// ═══════════════════════════════════════════════════════════════════════════
// Reason Clone and PartialEq
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_reason_clone() {
    let r = Reason::Error(Term::small(42));
    let r2 = r.clone();
    assert_eq!(r, r2);
}

#[test]
fn test_reason_partial_eq() {
    let r1 = Reason::Normal;
    let r2 = Reason::Normal;
    assert_eq!(r1, r2);

    let r3 = Reason::Error(Term::small(1));
    let r4 = Reason::Error(Term::small(1));
    assert_eq!(r3, r4);

    let r5 = Reason::Error(Term::small(1));
    let r6 = Reason::Error(Term::small(2));
    assert_ne!(r5, r6);
}

// ═══════════════════════════════════════════════════════════════════════════
// Exception Clone
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_exception_clone() {
    let mut exc = Exception::error(Term::small(42));
    exc.stacktrace.push(StackFrame {
        module: 1,
        function: 2,
        arity: 3,
        file: 4,
        line: 5,
    });
    let exc2 = exc.clone();
    assert_eq!(exc.reason, exc2.reason);
    assert_eq!(exc.stacktrace.len(), exc2.stacktrace.len());
}

// ═══════════════════════════════════════════════════════════════════════════
// Exception as std::error::Error
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_exception_as_error() {
    let exc = Exception::error(Term::atom(0));
    let err: &dyn std::error::Error = &exc;
    let _ = format!("{}", err);
}
