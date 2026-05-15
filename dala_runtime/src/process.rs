//! Process implementation - the core unit of concurrency in BEAM.
//!
//! Every Erlang/Elixir process has:
//! - A private heap for term allocation
//! - A stack for function calls
//! - A mailbox for inter-process communication
//! - A reduction counter for preemption
//! - A trap handler for exception recovery

use parking_lot::Mutex;
use smallvec::SmallVec;

use crate::code::CodePtr;
use crate::mailbox::Mailbox;
use crate::term::{RegisterFile, Term};

#[doc = "Flags on a process"]
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ProcessFlags: u32 {
        const TRAP_EXIT        = 0b0000_0001;
        const TRACING          = 0b0000_0010;
        const SYS_TRACE        = 0b0000_0100;
        const HEAP_SNAPSHOT    = 0b0000_1000;
        const SUSPENDED        = 0b0001_0000;
        const RUNNING          = 0b0010_0000;
        const RUNABLE          = 0b0100_0000;
        const DIRTY_CPU_SCHED  = 0b1000_0000;
    }
}

/// A catch frame for BEAM exception handling.
#[repr(C)]
#[derive(Clone, Debug)]
pub struct CatchFrame {
    /// The label to jump to on exception
    pub catch_label: u64,
    /// Stack pointer at the time of catch installation
    pub stack_pointer: usize,
    /// Heap pointer at the time of catch installation
    pub heap_pointer: usize,
    /// The CP (continuation pointer) to restore
    pub cp: u64,
}

/// A BEAM process.
///
/// This is the fundamental unit of concurrency in BEAM.
/// Each process has its own heap, stack, and mailbox.
#[repr(C)]
pub struct Process {
    /// Process ID (unique identifier)
    pub pid: u64,

    /// Heap start - beginning of allocated heap (for GC and realloc)
    pub heap_start: *mut Term,

    /// Heap pointer - points to the next free word on the heap
    pub heap_ptr: *mut Term,

    /// Heap top - end of allocated heap
    pub heap_top: *mut Term,

    /// Stack pointer - points to the current stack frame
    pub stack_ptr: *mut Term,

    /// Stack top - end of allocated stack
    pub stack_top: *mut Term,

    /// Heap high water mark - for GC scanning
    pub heap_high_water: *mut Term,

    /// Register file (X, Y, F registers)
    pub registers: RegisterFile,

    /// Current reduction count (decremented each function call)
    pub reductions: u32,

    /// Maximum reductions before yielding
    pub max_reductions: u32,

    /// Process flags
    pub flags: ProcessFlags,

    /// Mailbox for message passing
    pub mailbox: Mutex<Mailbox>,

    /// Catch stack for exception handling
    pub catches: SmallVec<[CatchFrame; 4]>,

    /// Current function being executed
    pub current_function: (u64, u64, u32), // (Module, Function, Arity)

    /// Code pointer for the current module
    pub code: CodePtr,

    /// Group leader process (for I/O)
    pub group_leader: u64,

    /// Error handler (process dictionary key)
    pub error_handler: Term,

    /// Priority (0=low, 1=normal, 2=high, 3=max)
    pub priority: u8,

    /// Status (running, runnable, waiting, suspended, exiting)
    pub status: ProcessStatus,

    /// Exit reason (if exiting)
    pub exit_reason: Option<Term>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProcessStatus {
    Running,
    Runnable,
    Waiting,
    Suspended,
    Exiting,
}

/// A builder for creating new processes.
pub struct ProcessBuilder {
    pid: u64,
    initial_heap_size: usize,
    max_reductions: u32,
    priority: u8,
    group_leader: u64,
    initial_module: u64,
    initial_function: u64,
    initial_arity: u32,
}

impl ProcessBuilder {
    pub fn new(pid: u64) -> Self {
        Self {
            pid,
            initial_heap_size: 233,
            max_reductions: 2000,
            priority: 1,
            group_leader: 0,
            initial_module: 0,
            initial_function: 0,
            initial_arity: 0,
        }
    }

    pub fn heap_size(mut self, size: usize) -> Self {
        self.initial_heap_size = size;
        self
    }

    pub fn reductions(mut self, reds: u32) -> Self {
        self.max_reductions = reds;
        self
    }

    pub fn priority(mut self, prio: u8) -> Self {
        self.priority = prio;
        self
    }

    pub fn group_leader(mut self, leader: u64) -> Self {
        self.group_leader = leader;
        self
    }

    pub fn initial_call(mut self, module: u64, function: u64, arity: u32) -> Self {
        self.initial_module = module;
        self.initial_function = function;
        self.initial_arity = arity;
        self
    }

    pub fn build(self) -> Result<Process, &'static str> {
        // Allocate heap
        let heap_layout = std::alloc::Layout::array::<Term>(self.initial_heap_size)
            .map_err(|_| "invalid heap size")?;
        let heap_ptr = unsafe { std::alloc::alloc(heap_layout) as *mut Term };
        if heap_ptr.is_null() {
            return Err("heap allocation failed");
        }

        // Allocate stack
        let stack_size = 1024;
        let stack_layout =
            std::alloc::Layout::array::<Term>(stack_size).map_err(|_| "invalid stack size")?;
        let stack_ptr = unsafe { std::alloc::alloc(stack_layout) as *mut Term };
        if stack_ptr.is_null() {
            unsafe {
                std::alloc::dealloc(heap_ptr as *mut u8, heap_layout);
            }
            return Err("stack allocation failed");
        }

        Ok(Process {
            pid: self.pid,
            heap_start: heap_ptr,
            heap_ptr,
            heap_top: unsafe { heap_ptr.add(self.initial_heap_size) },
            stack_ptr,
            stack_top: unsafe { stack_ptr.add(stack_size) },
            heap_high_water: heap_ptr,
            registers: RegisterFile::new(),
            reductions: self.max_reductions,
            max_reductions: self.max_reductions,
            flags: ProcessFlags::empty(),
            mailbox: Mutex::new(Mailbox::new()),
            catches: SmallVec::new(),
            current_function: (
                self.initial_module,
                self.initial_function,
                self.initial_arity,
            ),
            code: CodePtr::null(),
            group_leader: self.group_leader,
            error_handler: Term::atom(0), // Default error handler
            priority: self.priority,
            status: ProcessStatus::Runnable,
            exit_reason: None,
        })
    }
}

impl Process {
    /// Get the PID as a Term.
    pub fn pid_term(&self) -> Term {
        Term::from_raw(self.pid)
    }

    /// Allocate a term on the process heap.
    pub fn alloc(&mut self, value: Term) -> *mut Term {
        if self.heap_ptr >= self.heap_top {
            // Need to grow heap or GC
            self.grow_heap();
        }
        let ptr = self.heap_ptr;
        unsafe {
            *ptr = value;
        }
        self.heap_ptr = unsafe { self.heap_ptr.add(1) };
        ptr
    }

    /// Allocate raw space on the heap (for tuples, etc.).
    pub fn alloc_words(&mut self, count: usize) -> *mut Term {
        if self.heap_ptr as usize + count * std::mem::size_of::<Term>() > self.heap_top as usize {
            self.grow_heap();
        }
        let ptr = self.heap_ptr;
        self.heap_ptr = unsafe { self.heap_ptr.add(count) };
        ptr
    }

    /// Push a term onto the stack.
    pub fn push(&mut self, value: Term) {
        self.stack_ptr = unsafe { self.stack_ptr.sub(1) };
        unsafe {
            *self.stack_ptr = value;
        }
    }

    /// Pop a term from the stack.
    pub fn pop(&mut self) -> Term {
        let value = unsafe { *self.stack_ptr };
        self.stack_ptr = unsafe { self.stack_ptr.add(1) };
        value
    }

    /// Get the stack pointer for GC root scanning.
    pub fn stack_start(&self) -> *const Term {
        self.stack_ptr
    }

    /// Get the stack end for GC root scanning.
    pub fn stack_end(&self) -> *const Term {
        self.stack_top
    }

    /// Get heap start for GC.
    pub fn heap_start(&self) -> *const Term {
        self.heap_start
    }

    /// Get heap pointer for GC.
    pub fn heap_alloc_ptr(&self) -> *const Term {
        self.heap_ptr
    }

    /// Set heap high water mark.
    pub fn set_high_water(&mut self) {
        self.heap_high_water = self.heap_ptr;
    }

    /// Consume reductions and return true if we should yield.
    pub fn consume_reductions(&mut self, count: u32) -> bool {
        if self.reductions > count {
            self.reductions -= count;
            false
        } else {
            self.reductions = 0;
            true
        }
    }

    /// Reset reductions for a new scheduling quantum.
    pub fn reset_reductions(&mut self) {
        self.reductions = self.max_reductions;
    }

    /// Install a catch handler on the catch stack.
    pub fn push_catch(&mut self, frame: CatchFrame) {
        self.catches.push(frame);
    }

    /// Remove the top catch handler.
    pub fn pop_catch(&mut self) -> Option<CatchFrame> {
        self.catches.pop()
    }

    /// Send a message to this process's mailbox.
    pub fn send(&self, msg: Term) {
        let mut mbox = self.mailbox.lock();
        mbox.enqueue(msg);
    }

    /// Grow the heap (double its size).
    fn grow_heap(&mut self) {
        let old_size = self.heap_top as usize - self.heap_start as usize;
        let new_size = (old_size * 2).max(233);

        let old_layout = unsafe {
            std::alloc::Layout::from_size_align_unchecked(
                old_size * std::mem::size_of::<Term>(),
                std::mem::align_of::<Term>(),
            )
        };

        let new_ptr = unsafe {
            std::alloc::realloc(
                self.heap_start as *mut u8,
                old_layout,
                new_size * std::mem::size_of::<Term>(),
            ) as *mut Term
        };

        // Update heap_ptr to point to the same offset in the new allocation
        let heap_offset = self.heap_ptr as usize - self.heap_start as usize;
        self.heap_start = new_ptr;
        self.heap_ptr = unsafe { new_ptr.add(heap_offset / std::mem::size_of::<Term>()) };
        self.heap_top = unsafe { new_ptr.add(new_size) };
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        let heap_size = self.heap_top as usize - self.heap_start as usize;
        let stack_size = self.stack_top as usize - self.stack_ptr as usize;

        unsafe {
            if heap_size > 0 {
                let heap_layout = std::alloc::Layout::from_size_align_unchecked(
                    heap_size * std::mem::size_of::<Term>(),
                    std::mem::align_of::<Term>(),
                );
                std::alloc::dealloc(self.heap_start as *mut u8, heap_layout);
            }

            if stack_size > 0 {
                let stack_layout = std::alloc::Layout::from_size_align_unchecked(
                    stack_size * std::mem::size_of::<Term>(),
                    std::mem::align_of::<Term>(),
                );
                std::alloc::dealloc(self.stack_ptr as *mut u8, stack_layout);
            }
        }
    }
}

// SAFETY: Process is Send because all shared state is behind Mutex or atomic types.
unsafe impl Send for Process {}
unsafe impl Sync for Process {}
