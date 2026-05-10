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
