//! Exception handling - BEAM-style error recovery.
//!
//! BEAM exceptions are NOT native exceptions. They use a catch stack
//! and explicit exception propagation, similar to setjmp/longjmp but
//! with full process state management.

use crate::term::Term;

/// Exception reason - the value that describes why an exception occurred.
#[derive(Debug, Clone, PartialEq)]
pub enum Reason {
    /// Normal return (process completed successfully)
    Normal,
    /// Error with a reason term
    Error(Term),
    /// Exit signal (process killed by another process)
    Exit(Term),
    /// Throw (explicit throw with a term)
    Throw(Term),
}

/// An exception that occurred during BEAM execution.
#[derive(Debug, Clone)]
pub struct Exception {
    /// The reason for the exception
    pub reason: Reason,
    /// The stacktrace / catch stack at the time of exception
    pub stacktrace: Vec<StackFrame>,
}

/// A single stack frame in the BEAM stacktrace.
#[derive(Debug, Clone)]
pub struct StackFrame {
    /// Module name atom index
    pub module: u64,
    /// Function name atom index
    pub function: u64,
    /// Arity
    pub arity: u32,
    /// Source file (atom index)
    pub file: u64,
    /// Line number
    pub line: u32,
}

impl Exception {
    /// Create a new error exception.
    pub fn error(reason: Term) -> Self {
        Self {
            reason: Reason::Error(reason),
            stacktrace: Vec::new(),
        }
    }

    /// Create a new exit exception.
    pub fn exit(reason: Term) -> Self {
        Self {
            reason: Reason::Exit(reason),
            stacktrace: Vec::new(),
        }
    }

    /// Create a new throw exception.
    pub fn throw(reason: Term) -> Self {
        Self {
            reason: Reason::Throw(reason),
            stacktrace: Vec::new(),
        }
    }

    /// Check if this is a normal return.
    pub fn is_normal(&self) -> bool {
        matches!(self.reason, Reason::Normal)
    }

    /// Check if this is an error.
    pub fn is_error(&self) -> bool {
        matches!(self.reason, Reason::Error(_))
    }

    /// Check if this is an exit signal.
    pub fn is_exit(&self) -> bool {
        matches!(self.reason, Reason::Exit(_))
    }

    /// Get the error reason term, if this is an error.
    pub fn get_reason(&self) -> Option<&Term> {
        match &self.reason {
            Reason::Error(t) | Reason::Exit(t) | Reason::Throw(t) => Some(t),
            Reason::Normal => None,
        }
    }
}

impl From<Reason> for Exception {
    fn from(reason: Reason) -> Self {
        Self {
            reason,
            stacktrace: Vec::new(),
        }
    }
}

impl std::fmt::Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Reason::Normal => write!(f, "normal"),
            Reason::Error(t) => write!(f, "error: {:?}", t),
            Reason::Exit(t) => write!(f, "exit: {:?}", t),
            Reason::Throw(t) => write!(f, "throw: {:?}", t),
        }
    }
}

impl std::fmt::Display for Exception {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.reason)?;
        if !self.stacktrace.is_empty() {
            writeln!(f, "\nStacktrace:")?;
            for frame in &self.stacktrace {
                writeln!(
                    f,
                    "  {}.{}/{}:{}:{}",
                    frame.module, frame.function, frame.arity, frame.file, frame.line
                )?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for Exception {}

// ===== Explicit Result Threading for AOT Code =====

/// The result type used by AOT-compiled functions for explicit exception
/// propagation.
///
/// Instead of using native unwinding (which is unsafe through Cranelift
/// frames), every AOT function returns `Result<Term, Exception>`.  The
/// codegen inserts explicit checks after each call site and propagates
/// errors through native frames via the `?` operator or the helpers below.
///
/// This is the recommended approach per the architecture doc (Gap 3):
/// it is Rust-idiomatic, avoids `setjmp`/`longjmp`, and the overhead is
/// largely eliminated by inlining.
pub type Result<T = Term> = std::result::Result<T, Exception>;

/// Propagate an exception through a native frame.
///
/// Use this helper at the boundary between AOT-compiled code and the
/// runtime.  If `result` is `Err`, the exception is returned immediately
/// (early-return), mimicking the BEAM's catch/throw mechanism.
#[inline(always)]
pub fn propagate<T>(result: Result<T>) -> Result<T> {
    result
}

/// Convert a raw exception reason into a `Result` that can be returned
/// from an AOT function.
#[inline(always)]
pub fn exception_result<T>(reason: Reason) -> Result<T> {
    Err(Exception::from(reason))
}

/// Create a successful result wrapping a `Term`.
#[inline(always)]
pub fn ok_term(term: Term) -> Result {
    Ok(term)
}

/// Create an error result from a term reason.
#[inline(always)]
pub fn error_term(reason: Term) -> Result {
    Err(Exception::error(reason))
}

/// Create an exit result from a term reason.
#[inline(always)]
pub fn exit_term(reason: Term) -> Result {
    Err(Exception::exit(reason))
}

/// Create a throw result from a term reason.
#[inline(always)]
pub fn throw_term(reason: Term) -> Result {
    Err(Exception::throw(reason))
}

/// Unwrap a `Result`, returning the `Term` on success or propagating
/// the exception on failure.  This is a convenience wrapper that can
/// be used in generated code.
#[inline(always)]
pub fn unwrap_result(result: Result) -> Term {
    match result {
        Ok(term) => term,
        Err(ref exc) => {
            // In a full implementation this would install the catch
            // handler.  For now we return a sentinel.
            panic!("uncaught exception: {}", exc);
        }
    }
}

/// Check whether a result is an exception.
#[inline(always)]
pub fn is_exception(result: &Result) -> bool {
    result.is_err()
}

/// Check whether a result is a normal return.
#[inline(always)]
pub fn is_ok(result: &Result) -> bool {
    result.is_ok()
}

/// Map over the success value of a result, leaving errors untouched.
#[inline(always)]
pub fn map_result<F>(result: Result, f: F) -> Result
where
    F: FnOnce(Term) -> Term,
{
    result.map(f)
}

/// Chain two results: if `first` is `Ok`, apply `f` to its value;
/// otherwise propagate the error.
#[inline(always)]
pub fn and_then_result<F>(first: Result, f: F) -> Result
where
    F: FnOnce(Term) -> Result,
{
    first.and_then(f)
}
