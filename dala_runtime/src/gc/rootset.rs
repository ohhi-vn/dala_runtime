//! Root set construction for GC.
//!
//! The root set consists of all references from the stack, registers,
//! and other GC-invisible locations that could point to live objects
//! on the heap.

use crate::process::Process;
use crate::term::Term;

/// The root set for a GC cycle.
pub struct RootSet<'a> {
    /// Stack roots
    pub stack_roots: Vec<*const Term>,
    /// Register roots (X registers)
    pub register_roots: Vec<*const Term>,
    /// Catch stack roots
    pub catch_roots: Vec<*const Term>,
    /// Process reference (prevents process from being freed)
    pub _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> RootSet<'a> {
    /// Build the root set from a process's current state.
    ///
    /// # Safety
    ///
    /// This must be called at a safepoint where all live values are
    /// visible on the stack or in registers.
    pub unsafe fn from_process(process: &'a mut Process) -> Self {
        let mut rootset = RootSet {
            stack_roots: Vec::new(),
            register_roots: Vec::new(),
            catch_roots: Vec::new(),
            _phantom: std::marker::PhantomData,
        };

        // Scan stack for roots
        // Stack grows downward: stack_ptr -> stack_top
        let stack_start = process.stack_ptr;
        let stack_end = process.stack_top;

        // In a real implementation, we'd use the stack map from the
        // compiled code to know exactly which slots are pointers.
        // For now, we conservatively scan all stack slots.
        let mut ptr = stack_start;
        while ptr < stack_end {
            let term = &*ptr;
            if term.is_boxed() || term.is_list() {
                rootset.stack_roots.push(ptr);
            }
            ptr = ptr.add(1);
        }

        // Scan X registers for roots
        for i in 0..256 {
            let term = &process.registers.x[i];
            if term.is_boxed() || term.is_list() {
                rootset
                    .register_roots
                    .push(&process.registers.x[i] as *const Term);
            }
        }

        // Scan catch stack for roots
        for frame in &process.catches {
            // Catch frames may contain saved X registers
            for i in 0..frame.x.len() {
                let term = &frame.x[i];
                if term.is_boxed() || term.is_list() {
                    rootset.catch_roots.push(&frame.x[i] as *const Term);
                }
            }
        }

        rootset
    }

    /// Total number of roots.
    pub fn len(&self) -> usize {
        self.stack_roots.len() + self.register_roots.len() + self.catch_roots.len()
    }

    /// Check if the root set is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
