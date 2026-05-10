//! Trap frame - for BEAM exception handling and catch blocks.
//!
//! When a BEAM process encounters an exception, the runtime walks the
//! catch stack to find a matching handler. The trap frame stores the
//! information needed to resume execution at the catch label.

use crate::term::Term;

/// A trap frame installed by a `catch` block.
#[repr(C)]
#[derive(Clone, Debug, Default)]
pub struct TrapFrame {
    /// The label to jump to when an exception is caught
    pub catch_label: u64,
    /// Stack pointer at the time of catch installation
    pub sp: usize,
    /// Heap pointer at the time of catch installation
    pub hp: usize,
    /// Continuation pointer (return address)
    pub cp: u64,
    /// X registers snapshot (for restoration after catch)
    pub x: [Term; 10],
}

impl TrapFrame {
    /// Create a new empty trap frame.
    pub fn new() -> Self {
        Self {
            catch_label: 0,
            sp: 0,
            hp: 0,
            cp: 0,
            x: [Term::nil(); 10],
        }
    }

    /// Check if this trap frame is valid (has been installed).
    pub fn is_valid(&self) -> bool {
        self.catch_label != 0
    }
}

/// The result of attempting to handle an exception.
pub enum TrapResult {
    /// Exception was caught, execution should resume at the catch label
    Caught {
        /// The label to jump to
        label: u64,
        /// The stack pointer to restore
        sp: usize,
        /// The heap pointer to restore
        hp: usize,
    },
    /// Exception was not caught by any handler
    Unhandled,
}
