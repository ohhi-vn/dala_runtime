//! IR instructions - the building blocks of the SSA IR.
//!
//! Each instruction represents a single operation in the IR.
//! Instructions are organized into basic blocks and form a control
//! flow graph. The IR is in SSA form, meaning each value is defined
//! exactly once.

use crate::value::IRValueId;

/// A register or stack slot reference in the BEAM context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reg {
    /// X register (function argument / return value)
    X(u32),
    /// Y register (stack frame slot)
    Y(u32),
    /// F register (floating point)
    F(u32),
}

/// A label for a basic block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Label(pub u32);

/// A single IR instruction.
///
/// Each instruction has:
/// - A kind (what operation it performs)
/// - Operands (input values)
/// - Optional result (output value ID)
#[derive(Debug, Clone)]
pub struct IRInst {
    /// The kind of instruction
    pub kind: IRInstKind,
    /// The result value ID (if this instruction produces a value)
    pub result: Option<IRValueId>,
    /// Input operands
    pub operands: Vec<IRValueId>,
    /// The source BEAM instruction index (for debugging)
    pub beam_offset: u32,
    /// Side effects of this instruction
    pub side_effects: SideEffects,
}

/// Side effects that an instruction may have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SideEffects {
    /// May allocate memory on the process heap
    pub allocates: bool,
    /// May read from the process heap
    pub reads_heap: bool,
    /// May write to the process heap
    pub writes_heap: bool,
    /// May raise an exception
    pub may_raise: bool,
    /// May call other functions
    pub calls: bool,
    /// May yield (consume reductions)
    pub may_yield: bool,
}

impl SideEffects {
    /// No side effects at all.
    pub const NONE: Self = Self {
        allocates: false,
        reads_heap: false,
        writes_heap: false,
        may_raise: false,
        calls: false,
        may_yield: false,
    };

    /// All side effects.
    pub const ALL: Self = Self {
        allocates: true,
        reads_heap: true,
        writes_heap: true,
        may_raise: true,
        calls: true,
        may_yield: true,
    };
}

/// The different kinds of IR instructions.
#[derive(Debug, Clone)]
pub enum IRInstKind {
    // ===== Arithmetic =====
    /// Integer addition: result = a + b
    Add,
    /// Integer subtraction: result = a - b
    Sub,
    /// Integer multiplication: result = a * b
    Mul,
    /// Integer division: result = a / b
    Div,
    /// Integer remainder: result = a % b
    Rem,
    /// Integer negation: result = -a
    Neg,

    // ===== Bitwise =====
    /// Bitwise AND: result = a & b
    BitAnd,
    /// Bitwise OR: result = a | b
    BitOr,
    /// Bitwise XOR: result = a ^ b
    BitXor,
    /// Bitwise NOT: result = ~a
    BitNot,
    /// Left shift: result = a << b
    ShiftLeft,
    /// Right shift: result = a >> b
    ShiftRight,

    // ===== Comparison =====
    /// Equal comparison: result = (a == b)
    Eq,
    /// Not equal: result = (a != b)
    Ne,
    /// Greater than: result = (a > b)
    Gt,
    /// Greater than or equal: result = (a >= b)
    Ge,
    /// Less than: result = (a < b)
    Lt,
    /// Less than or equal: result = (a <= b)
    Le,

    // ===== Type Tests =====
    /// Test if value is a small integer
    IsSmallInt,
    /// Test if value is a float
    IsFloat,
    /// Test if value is an atom
    IsAtom,
    /// Test if value is a tuple
    IsTuple,
    /// Test if value is a list
    IsList,
    /// Test if value is a map
    IsMap,
    /// Test if value is a binary
    IsBinary,
    /// Test if value is a function
    IsFun,
    /// Test if value is a PID
    IsPid,
    /// Test if value is nil
    IsNil,
    /// Test if value is true
    IsTrue,
    /// Test if value is false
    IsFalse,

    // ===== Memory / Heap Operations =====
    /// Allocate heap space: result = pointer to allocated space
    Alloc {
        /// Number of words to allocate
        words: u32,
    },
    /// Load a value from heap: result = *base[offset]
    Load {
        /// Base pointer value
        base: IRValueId,
        /// Offset in words
        offset: u32,
    },
    /// Store a value to heap: *base[offset] = value
    Store {
        /// Base pointer value
        base: IRValueId,
        /// Offset in words
        offset: u32,
        /// Value to store
        value: IRValueId,
    },
    /// Get element from tuple: result = tuple[index]
    TupleGet {
        /// The tuple value
        tuple: IRValueId,
        /// Element index (0-based)
        index: u32,
    },
    /// Set element in tuple: tuple[index] = value
    TupleSet {
        /// The tuple value
        tuple: IRValueId,
        /// Element index
        index: u32,
        /// New value
        value: IRValueId,
    },

    // ===== Stack Operations =====
    /// Push value onto stack
    Push {
        /// Value to push
        value: IRValueId,
    },
    /// Pop value from stack
    Pop,
    /// Get stack pointer
    GetStackPtr,
    /// Set stack pointer
    SetStackPtr {
        /// New stack pointer value
        sp: IRValueId,
    },

    // ===== Register Operations =====
    /// Move value between registers
    Move {
        /// Source register
        src: Reg,
        /// Destination register
        dst: Reg,
    },
    /// Load value from register: result = reg
    GetReg {
        /// The register to read
        reg: Reg,
    },
    /// Store value to register: reg = value
    SetReg {
        /// The register to write
        reg: Reg,
        /// The value to store
        value: IRValueId,
    },

    // ===== Control Flow =====
    /// Unconditional branch to a block
    Br {
        /// Target label
        target: Label,
    },
    /// Conditional branch: if cond then true_block else false_block
    BrIf {
        /// Condition value
        cond: IRValueId,
        /// Target if true
        true_target: Label,
        /// Target if false
        false_target: Label,
    },
    /// Switch (jump table) on an integer value
    Switch {
        /// Value to switch on
        value: IRValueId,
        /// Default target
        default: Label,
        /// Jump table entries (value -> label)
        targets: Vec<(i64, Label)>,
    },
    /// Return from function with a value
    Ret {
        /// Return value
        value: IRValueId,
    },
    /// Indirect function call
    Call {
        /// Function to call
        func: IRValueId,
        /// Arguments
        args: Vec<IRValueId>,
    },
    /// Tail call (optimized for BEAM's last-call optimization)
    TailCall {
        /// Function to call
        func: IRValueId,
        /// Arguments
        args: Vec<IRValueId>,
    },
    /// Call a BIF (built-in function)
    CallBif {
        /// BIF module atom index
        module: IRValueId,
        /// BIF function atom index
        function: IRValueId,
        /// Arguments
        args: Vec<IRValueId>,
    },

    // ===== Exception Handling =====
    /// Install a catch handler
    Catch {
        /// Label to jump to on exception
        handler: Label,
    },
    /// Remove a catch handler
    CatchPop,
    /// Throw an exception
    Throw {
        /// Exception reason
        reason: IRValueId,
    },
    /// Resume after exception (landing pad)
    Resume {
        /// Exception value
        exception: IRValueId,
    },

    // ===== Process Operations =====
    /// Consume reductions and potentially yield
    ConsumeReductions {
        /// Number of reductions to consume
        count: u32,
    },
    /// Send a message to a process
    Send {
        /// Destination process
        dest: IRValueId,
        /// Message to send
        msg: IRValueId,
    },
    /// Receive a message from the mailbox
    Recv {
        /// Timeout in milliseconds (0 = infinite)
        timeout: u32,
    },

    // ===== Literal Operations =====
    /// Load a literal value from the literal table
    LoadLiteral {
        /// Literal table index
        index: u32,
    },
    /// Create a small integer constant
    ConstSmallInt {
        /// The integer value
        value: i64,
    },
    /// Create an atom constant
    ConstAtom {
        /// Atom table index
        index: u32,
    },
    /// Create a nil constant
    ConstNil,
    /// Create a true constant
    ConstTrue,
    /// Create a false constant
    ConstFalse,

    // ===== Binary Operations =====
    /// Create a heap binary from data
    BinaryNew {
        /// Data value (list or binary)
        data: IRValueId,
    },
    /// Get size of a binary
    BinarySize {
        /// Binary value
        binary: IRValueId,
    },
    /// Extract a sub-binary
    BinaryExtract {
        /// Source binary
        binary: IRValueId,
        /// Offset in bits
        offset: IRValueId,
        /// Size in bits
        size: IRValueId,
        /// Flags (signed, big/little endian, etc.)
        flags: u32,
    },

    // ===== Fun (Closure) Operations =====
    /// Create a function/closure
    MakeFun {
        /// Module name
        module: IRValueId,
        /// Function name
        function: IRValueId,
        /// Arity
        arity: u32,
        /// Free variables (environment)
        fvs: Vec<IRValueId>,
    },

    // ===== GC Safepoint =====
    /// GC safepoint - all live values must be on the stack
    GcSafe,

    // ===== Optimization =====
    /// No-op instruction (used as placeholder during optimization)
    Nop,
}

impl IRInst {
    /// Create a new instruction with no result.
    pub fn new(kind: IRInstKind) -> Self {
        Self {
            kind,
            result: None,
            operands: Vec::new(),
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        }
    }

    /// Create a new instruction with a result value.
    pub fn with_result(kind: IRInstKind, result: IRValueId) -> Self {
        Self {
            kind,
            result: Some(result),
            operands: Vec::new(),
            beam_offset: 0,
            side_effects: SideEffects::NONE,
        }
    }

    /// Add an operand to this instruction.
    pub fn add_operand(&mut self, operand: IRValueId) {
        self.operands.push(operand);
    }

    /// Set the side effects for this instruction.
    pub fn set_side_effects(&mut self, effects: SideEffects) {
        self.side_effects = effects;
    }

    /// Check if this instruction may trigger a GC.
    pub fn may_gc(&self) -> bool {
        self.side_effects.allocates
    }

    /// Check if this instruction may raise an exception.
    pub fn may_raise(&self) -> bool {
        self.side_effects.may_raise
    }

    /// Check if this instruction may yield to the scheduler.
    pub fn may_yield(&self) -> bool {
        self.side_effects.may_yield
    }
}
