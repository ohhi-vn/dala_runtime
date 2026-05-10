# Reference Guide

This guide provides detailed API documentation for each module in the Dala AOT compiler.

---

## Table of Contents

- [dala_runtime](#dala_runtime)
  - [Term](#term)
  - [Process](#process)
  - [Scheduler](#scheduler)
  - [GC](#gc)
  - [BIFs](#bifs)
  - [Exception](#exception)
  - [Trap](#trap)
- [dala_ir](#dala_ir)
  - [IRContext](#ircontext)
  - [IRModule](#irmodule)
  - [IRFunction](#irfunction)
  - [IRInst](#irinst)
  - [IRBuilder](#irbuilder)
  - [IRValue](#irvalue)
  - [IRType](#irtype)
- [dala_codegen](#dala_codegen)
  - [CodeGenerator](#codegenerator)
  - [CompiledFunction](#compiledfunction)
  - [RuntimeGlue](#runtimeglue)
  - [Intrinsic](#intrinsic)
- [dala_beam_loader](#dala_beam_loader)
  - [BeamModule](#beammodule)
  - [BeamFunction](#beamfunction)
  - [BeamReader](#beamreader)
- [dala_dispatch](#dala_dispatch)
  - [DispatchManager](#dispatchmanager)
  - [ExportTable](#exporttable)
  - [HotCodeManager](#hotcodemanager)
  - [LazyFnRef](#lazyfnref)
- [dala_aot CLI](#dala_aot-cli)

---

## dala_runtime

### Term

The fundamental value type in the BEAM VM. A transparent wrapper around a 64-bit tagged word.

**File:** `dala_runtime/src/term.rs`

#### Type: `Term`

```rust
#[repr(transparent)]
pub struct Term(u64);
```

#### Constants

| Constant | Type | Description |
|----------|------|-------------|
| `Term::nil()` | `Term` | The empty list `[]` |
| `Term::true_()` | `Term` | Boolean `true` |
| `Term::false_()` | `Term` | Boolean `false` |

#### Constructors

```rust
// From raw bits
Term::from_raw(bits: u64) -> Term

// Small integer (fits in 63 bits, shifted left by 4)
Term::small(val: i64) -> Term

// Atom by index
Term::atom(index: u32) -> Term

// Boolean
Term::bool(b: bool) -> Term
```

#### Accessors

```rust
// Raw value
term.to_raw() -> u64

// Type checks
term.is_small() -> bool
term.is_atom() -> bool
term.is_list() -> bool
term.is_tuple() -> bool
term.is_map() -> bool
term.is_boxed() -> bool
term.is_float() -> bool
term.is_binary() -> bool
term.is_fun() -> bool
term.is_nil() -> bool
term.is_true() -> bool
term.is_false() -> bool
term.is_pid() -> bool
term.is_port() -> bool
term.is_catch() -> bool

// Value extraction (returns Option or panics)
term.get_small() -> Option<i64>
term.unwrap_small() -> i64
term.get_atom_index() -> Option<u32>
term.get_list_ptr() -> *const Term
term.get_list_ptr_mut() -> *mut Term
term.get_boxed_ptr() -> *const Term
term.get_boxed_ptr_mut() -> *mut Term
term.header() -> u64
term.header_tag() -> u64
term.get_float() -> Option<f64>
term.get_float_ptr() -> *const f64

// Tuple access
term.tuple_get(i: usize) -> Term
term.tuple_data_ptr() -> *const Term
term.tuple_data_ptr_mut() -> *mut Term
```

#### Static Methods

```rust
// Header utilities
Term::header_arity(header: u64) -> usize
```

#### Traits Implemented

- `Copy`, `Clone`, `Eq`, `PartialEq` (bitwise comparison)
- `Hash`
- `Debug` (human-readable representation)

---

### RegisterFile

The full set of BEAM registers.

```rust
pub struct RegisterFile {
    pub x: [Term; 256],   // X0-X255: function arguments and locals
    pub y: [Term; 1024],  // Y0-Y1023: stack frame slots
    pub f: [f64; 256],    // F0-F255: floating point registers
}
```

---

### Process

A BEAM process — the fundamental unit of concurrency.

**File:** `dala_runtime/src/process.rs`

#### Type: `Process`

```rust
#[repr(C)]
pub struct Process {
    pub pid: u64,
    pub heap_start: *mut Term,
    pub heap_ptr: *mut Term,
    pub heap_top: *mut Term,
    pub stack_ptr: *mut Term,
    pub stack_top: *mut Term,
    pub heap_high_water: *mut Term,
    pub registers: RegisterFile,
    pub reductions: u32,
    pub max_reductions: u32,
    pub flags: ProcessFlags,
    pub mailbox: Mutex<Mailbox>,
    pub catches: SmallVec<[CatchFrame; 4]>,
    pub current_function: (u64, u64, u32),  // (Module, Function, Arity)
    pub code: CodePtr,
    pub group_leader: u64,
    pub error_handler: Term,
    pub priority: u8,       // 0=low, 1=normal, 2=high, 3=max
    pub status: ProcessStatus,
    pub exit_reason: Option<Term>,
}
```

#### ProcessFlags

```rust
bitflags! {
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
```

#### CatchFrame

```rust
#[repr(C)]
pub struct CatchFrame {
    pub catch_label: u64,
    pub stack_pointer: usize,
    pub heap_pointer: usize,
    pub cp: u64,
}
```

#### ProcessStatus

```rust
pub enum ProcessStatus {
    Running,
    Runnable,
    Waiting,
    Suspended,
    Exiting,
}
```

#### ProcessBuilder

```rust
pub struct ProcessBuilder { /* private fields */ }

impl ProcessBuilder {
    pub fn new(pid: u64) -> Self
    pub fn heap_size(mut self, size: usize) -> Self
    pub fn reductions(mut self, reds: u32) -> Self
    pub fn priority(mut self, prio: u8) -> Self
    pub fn group_leader(mut self, leader: u64) -> Self
    pub fn initial_call(mut self, module: u64, function: u64, arity: u32) -> Self
    pub fn build(self) -> Result<Process, &'static str>
}
```

#### Key Process Methods

```rust
impl Process {
    pub fn pid_term(&self) -> Term
    pub fn alloc(&mut self, value: Term) -> *mut Term
    pub fn alloc_words(&mut self, count: usize) -> *mut Term
    pub fn push(&mut self, value: Term)
    pub fn pop(&mut self) -> Term
    pub fn stack_start(&self) -> *const Term
    pub fn stack_end(&self) -> *const Term
    pub fn heap_start(&self) -> *const Term
    pub fn heap_alloc_ptr(&self) -> *const Term
    pub fn set_high_water(&mut self)
    pub fn consume_reductions(&mut self, count: u32) -> bool  // true = should yield
    pub fn reset_reductions(&mut self)
    pub fn push_catch(&mut self, frame: CatchFrame)
    pub fn pop_catch(&mut self) -> Option<CatchFrame>
    pub fn send(&self, msg: Term)
}
```

---

### Scheduler

SMP scheduler with work-stealing.

**File:** `dala_runtime/src/scheduler.rs`

#### SchedulerMessage

```rust
pub enum SchedulerMessage {
    Spawn {
        pid: u64,
        module: u64,
        function: u64,
        arity: u32,
        args: Vec<Term>,
    },
    Message { pid: u64, msg: Term },
    Kill(u64),
    Halt,
}
```

#### Key Scheduler Methods

```rust
impl Scheduler {
    pub fn global_init(config: RuntimeConfig) -> Result<(), RuntimeError>
    pub fn spawn(&self, module: u64, function: u64, arity: u32, args: Vec<Term>) -> u64
    pub fn send_message(&self, pid: u64, msg: Term)
}
```

---

### RuntimeConfig

```rust
pub struct RuntimeConfig {
    pub scheduler_count: usize,            // defaults to num_cpus::get()
    pub initial_heap_size: usize,          // defaults to 233 words
    pub max_heap_size: usize,              // defaults to 16_384 words
    pub reductions_per_yield: u32,         // defaults to 2_000
    pub debug_gc: bool,                    // defaults to false
    pub execution_mode: ExecutionMode,     // defaults to Mixed
}

pub enum ExecutionMode {
    Interpreted,
    Mixed,      // default
    Aot,
}
```

#### Runtime Init

```rust
pub fn init(config: RuntimeConfig) -> Result<(), RuntimeError>
```

#### RuntimeError

```rust
pub enum RuntimeError {
    SchedulerError(String),
    AllocationError(String),
    BeamLoadError(String),
    CodegenError(String),
    LinkError(String),
    ProcessCrash(String),
}
```

---

### GC

**File:** `dala_runtime/src/gc/`

```rust
pub mod gc {
    pub fn collect(process: &mut Process, need_words: usize) -> Result<(), &'static str>
    pub unsafe fn maybe_collect(process: &mut Process, need_words: usize) -> *mut Term
    pub fn safepoint()
}
```

#### GCConfig

```rust
pub struct GCConfig {
    pub nursery_size: usize,       // defaults to 233
    pub max_copy: usize,           // defaults to 7
    pub fullsweep_after: usize,    // defaults to 65536
}
```

#### GCStats

```rust
pub struct GCStats {
    pub heap_words_before: usize,
    pub heap_words_after: usize,
    pub stack_words: usize,
    pub roots_scanned: usize,
    pub objects_copied: usize,
    pub time_ns: u64,
}
```

#### StackMap

```rust
pub struct StackMap {
    pub instruction_offset: u32,
    pub num_entries: u32,
    pub entries: [StackMapEntry; 0],  // flexible array
}

pub struct StackMapEntry {
    pub offset: u32,
    pub is_pointer: bool,
    pub value_type: StackMapType,
}

pub enum StackMapType {
    Unknown,
    TuplePointer,
    ListPointer,
    BoxedPointer,
    FunPointer,
    MapPointer,
    BinaryPointer,
    MaybePointer,
}
```

---

### BIFs (Built-In Functions)

**File:** `dala_runtime/src/bif.rs`

#### Registration

```rust
pub fn register_all_bifs() -> Vec<BifDescriptor>
pub fn lookup_bif(module: u64, function: u64, arity: u32) -> Option<BifFn>
```

#### BifDescriptor

```rust
pub struct BifDescriptor {
    pub module: u64,
    pub function: u64,
    pub arity: u32,
    pub implementation: BifFn,  // unsafe fn(&mut Process, &[Term]) -> BifResult
}
```

#### Implemented BIFs

| Module | Function | Arity | Description |
|--------|----------|-------|-------------|
| erlang | `+` | 2 | Integer addition |
| erlang | `-` | 2 | Integer subtraction |
| erlang | `*` | 2 | Integer multiplication |
| erlang | `div` | 2 | Integer division |
| erlang | `rem` | 2 | Integer remainder |
| erlang | `-` | 1 | Integer negation |
| erlang | `is_integer` | 1 | Type test |
| erlang | `is_atom` | 1 | Type test |
| erlang | `is_binary` | 1 | Type test |
| erlang | `is_boolean` | 1 | Type test |
| erlang | `is_tuple` | 1 | Type test |
| erlang | `is_list` | 1 | Type test |
| erlang | `is_pid` | 1 | Type test |
| erlang | `is_port` | 1 | Type test |
| erlang | `is_function` | 1 | Type test |
| erlang | `is_map` | 1 | Type test |
| erlang | `is_number` | 1 | Type test |
| erlang | `is_float` | 1 | Type test |
| erlang | `==` | 2 | Term equality |
| erlang | `/=` | 2 | Term inequality |
| erlang | `=:=` | 2 | Exact equality |
| erlang | `self` | 0 | Current PID |
| erlang | `spawn` | 3 | Spawn process |
| erlang | `send` | 2 | Send message |
| erlang | `error` | 1 | Raise error |
| erlang | `error` | 2 | Raise error with args |
| erlang | `throw` | 1 | Throw term |
| erlang | `exit` | 1 | Exit process |
| erlang | `fault` | 1 | Raise fault |
| erlang | `tuple_size` | 1 | Tuple size |
| erlang | `size` | 1 | Size of tuple/binary |
| erlang | `length` | 1 | List length |
| erlang | `hd` | 1 | List head |
| erlang | `tl` | 1 | List tail |
| erlang | `node` | 0 | Node name |
| erlang | `nodes` | 0 | Known nodes |
| erlang | `integer_to_list` | 1 | Int to list |
| erlang | `list_to_integer` | 1 | List to int |
| erlang | `atom_to_list` | 1 | Atom to list |
| erlang | `list_to_atom` | 1 | List to atom |
| erlang | `float` | 1 | Convert to float |

---

### Exception

**File:** `dala_runtime/src/exception.rs`

```rust
pub enum Reason {
    Normal,
    Error(Term),
    Exit(Term),
    Throw(Term),
}

pub struct Exception {
    pub reason: Reason,
    pub stacktrace: Vec<StackFrame>,
}

pub struct StackFrame {
    pub module: u64,
    pub function: u64,
    pub arity: u32,
    pub file: u64,
    pub line: u32,
}
```

#### Exception Constructors

```rust
Exception::error(reason: Term) -> Self
Exception::exit(reason: Term) -> Self
Exception::throw(reason: Term) -> Self
```

#### Trait Implementations

- `std::fmt::Display`
- `std::error::Error`

---

### Trap

**File:** `dala_runtime/src/trap.rs`

```rust
#[repr(C)]
pub struct TrapFrame {
    pub catch_label: u64,
    pub sp: usize,
    pub hp: usize,
    pub cp: u64,
    pub x: [Term; 10],
}

pub enum TrapResult {
    Caught { label: u64, sp: usize, hp: usize },
    Unhandled,
}
```

---

## dala_ir

### IRContext

**File:** `dala_ir/src/lib.rs`

The top-level container for all IR data in a compilation unit.

```rust
pub struct IRContext {
    pub module: IRModule,
    pub functions: Vec<IRFunction>,
    pub constants: Vec<IRValue>,
    pub types: Vec<IRType>,
}

impl IRContext {
    pub fn new() -> Self
    pub fn create_function(&mut self, name: String, ty: IRType) -> IRFunctionId
    pub fn get_function(&self, id: IRFunctionId) -> &IRFunction
    pub fn get_function_mut(&mut self, id: IRFunctionId) -> &mut IRFunction
    pub fn create_type(&mut self, ty: IRType) -> TypeId
    pub fn get_type(&self, id: TypeId) -> &IRType
}
```

#### Handle Types

```rust
pub struct IRFunctionId(pub usize);
pub struct ValueId(pub usize);
pub struct TypeId(pub usize);
pub struct BlockId(pub usize);
pub struct InstId(pub usize);
```

---

### IRModule

**File:** `dala_ir/src/module.rs`

```rust
pub struct IRModule {
    pub name: u64,
    pub functions: IndexMap<(u64, u32), IRFunctionId>,  // (name, arity) -> id
    pub function_bodies: Vec<IRFunction>,
    pub exports: Vec<(u64, u32)>,
    pub imports: IndexMap<u64, Vec<(u64, u32)>>,
    pub attributes: Vec<(u64, u64)>,
    pub compile_info: CompileInfo,
    pub literals: Vec<u64>,
    pub line_info: Vec<(u32, u32)>,
}

impl IRModule {
    pub fn new(name: u64) -> Self
    pub fn add_function(&mut self, name: u64, arity: u32) -> IRFunctionId
    pub fn get_function(&self, name: u64, arity: u32) -> Option<IRFunctionId>
    pub fn get_function_body(&self, id: IRFunctionId) -> &IRFunction
    pub fn get_function_body_mut(&mut self, id: IRFunctionId) -> &mut IRFunction
    pub fn add_export(&mut self, name: u64, arity: u32)
    pub fn add_import(&mut self, module: u64, function: u64, arity: u32)
    pub fn add_literal(&mut self, value: u64) -> u32
    pub fn is_exported(&self, name: u64, arity: u32) -> bool
    pub fn exported_functions(&self) -> &[(u64, u32)]
    pub fn function_count(&self) -> usize
}
```

---

### IRFunction

**File:** `dala_ir/src/function.rs`

Represents a single function in IR form, containing basic blocks and their instructions.

---

### IRInst

**File:** `dala_ir/src/instruction.rs`

```rust
pub struct IRInst {
    pub kind: IRInstKind,
    pub result: Option<IRValueId>,
    pub operands: Vec<IRValueId>,
    pub beam_offset: u32,
    pub side_effects: SideEffects,
}
```

#### IRInstKind Enum (full list)

**Arithmetic:** `Add`, `Sub`, `Mul`, `Div`, `Rem`, `Neg`

**Bitwise:** `BitAnd`, `BitOr`, `BitXor`, `BitNot`, `ShiftLeft`, `ShiftRight`

**Comparison:** `Eq`, `Ne`, `Gt`, `Ge`, `Lt`, `Le`

**Type Tests:** `IsSmallInt`, `IsFloat`, `IsAtom`, `IsTuple`, `IsList`, `IsMap`, `IsBinary`, `IsFun`, `IsPid`, `IsNil`, `IsTrue`, `IsFalse`

**Memory/Heap:**
- `Alloc { words: u32 }`
- `Load { base: IRValueId, offset: u32 }`
- `Store { base: IRValueId, offset: u32, value: IRValueId }`
- `TupleGet { tuple: IRValueId, index: u32 }`
- `TupleSet { tuple: IRValueId, index: u32, value: IRValueId }`

**Stack:** `Push { value }`, `Pop`, `GetStackPtr`, `SetStackPtr { sp }`

**Registers:** `Move { src, dst }`, `GetReg { reg }`, `SetReg { reg, value }`

**Control Flow:** `Br { target }`, `BrIf { cond, true_target, false_target }`, `Switch { value, default, targets }`, `Ret { value }`, `Call { func, args }`, `TailCall { func, args }`, `CallBif { module, function, args }`

**Exceptions:** `Catch { handler }`, `CatchPop`, `Throw { reason }`, `Resume { exception }`

**Process:** `ConsumeReductions { count }`, `Send { dest, msg }`, `Recv { timeout }`

**Literals:** `LoadLiteral { index }`, `ConstSmallInt { value }`, `ConstAtom { index }`, `ConstNil`, `ConstTrue`, `ConstFalse`

**Binary:** `BinaryNew { data }`, `BinarySize { binary }`, `BinaryExtract { binary, offset, size, flags }`

**Funs:** `MakeFun { module, function, arity, fvs }`

**GC:** `GcSafe`

**Other:** `Nop`

#### SideEffects

```rust
pub struct SideEffects {
    pub allocates: bool,
    pub reads_heap: bool,
    pub writes_heap: bool,
    pub may_raise: bool,
    pub calls: bool,
    pub may_yield: bool,
}
```

#### Register Type

```rust
pub enum Reg {
    X(u32),
    Y(u32),
    F(u32),
}
```

---

### IRBuilder

**File:** `dala_ir/src/builder.rs`

Used to construct IR functions programmatically.

---

### IRValue

**File:** `dala_ir/src/value.rs`

Represents a value in the IR — either a constant or a reference to an instruction result.

---

### IRType

**File:** `dala_ir/src/type_system.rs`

The type system for IR values.

```rust
pub struct IRType { /* private */ }

pub enum TypeKind {
    Integer,
    Float,
    Term,
    Tuple(Vec<TypeId>),
    List(Box<TypeId>),
    Map,
    Binary,
    Fun(Vec<TypeId>, Box<TypeId>),
    Bottom,  // uninhabited
    Top,     // any term
}
```

---

### Optimization Passes

**File:** `dala_ir/src/opt/`

```rust
pub fn optimize(func: &mut IRFunction)
```

Applies optimization passes including: dead code elimination, constant propagation, CSE, CFG simplification.

---

## dala_codegen

### CodeGenerator

**File:** `dala_codegen/src/lib.rs`

```rust
pub struct CodeGenerator { /* private */ }

pub enum CompilationMode {
    Jit,
    Aot,
}

pub struct CodegenConfig {
    pub mode: CompilationMode,
    pub target: String,        // "x86_64" or "aarch64"
    pub opt_level: String,     // "none", "less", "default", "aggressive"
    pub debug_assertions: bool,
    pub verbose: bool,
}

impl CodeGenerator {
    pub fn new(config: CodegenConfig) -> Result<Self, CodegenError>
    pub fn compile_function(&mut self, func: &IRFunction) -> Result<CompiledFunction, CodegenError>
}
```

### CompiledFunction

```rust
pub struct CompiledFunction {
    pub code_ptr: CodePtr,
    pub code_size: usize,
    pub stack_map: StackMap,
    pub frame_size: usize,
    pub spill_count: usize,
}

impl CompiledFunction {
    pub fn as_fn(&self) -> CompiledFn
}

pub type CompiledFn = unsafe extern "C" fn(proc: &mut Process, args: *const Term) -> Term;
```

### CodegenError

```rust
pub enum CodegenError {
    TargetError,
    CompilationError,
    Unsupported,
    LinkError,
}
```

### RuntimeGlue

**File:** `dala_codegen/src/runtime_glue.rs`

Declares runtime functions callable from generated code.

```rust
pub enum RuntimeFuncId {
    Alloc, ShouldYield, ConsumeReductions, BifDispatch,
    Throw, Send, Receive, LoadLiteral, MakeFun,
    BinaryNew, BinarySize, BinaryExtract,
    ListCons, ListHead, ListTail,
    MapGet, MapPut, TupleElement, Raise, Apply,
}

impl RuntimeGlue {
    pub fn new() -> Self
    pub fn declare_all(&mut self, module: &mut cranelift_module::Module)
    // Individual getters:
    pub fn get_alloc_fn(&self) -> FuncRef
    pub fn get_should_yield_fn(&self) -> FuncRef
    pub fn get_reductions_fn(&self) -> FuncRef
    pub fn get_bif_dispatch_fn(&self) -> FuncRef
    pub fn get_throw_fn(&self) -> FuncRef
    pub fn get_send_fn(&self) -> FuncRef
    pub fn get_recv_fn(&self) -> FuncRef
    pub fn get_load_literal_fn(&self) -> FuncRef
    pub fn get_make_fun_fn(&self) -> FuncRef
    pub fn get_binary_new_fn(&self) -> FuncRef
    pub fn get_binary_size_fn(&self) -> FuncRef
    pub fn get_binary_extract_fn(&self) -> FuncRef
    pub fn get_list_cons_fn(&self) -> FuncRef
    pub fn get_list_head_fn(&self) -> FuncRef
    pub fn get_list_tail_fn(&self) -> FuncRef
    pub fn get_map_get_fn(&self) -> FuncRef
    pub fn get_map_put_fn(&self) -> FuncRef
    pub fn get_tuple_element_fn(&self) -> FuncRef
    pub fn get_raise_fn(&self) -> FuncRef
    pub fn get_apply_fn(&self) -> FuncRef
}
```

### Intrinsic

**File:** `dala_codegen/src/intrinsics.rs`

```rust
pub enum Intrinsic {
    GetProcess, GetReductions, SetReductions, ShouldYield,
    GetHeapPtr, SetHeapPtr, GetStackPtr, SetStackPtr,
    GcBarrier,
    IsSmallInt, IsAtom, IsTuple, IsList, IsFloat, IsMap,
    IsBinary, IsFun, IsPid, IsPort,
    TupleElement, MapGet, MapPut,
    BinaryNew, BinaryMatch,
    ListCons, ListHead, ListTail,
    Raise, Error, Throw, Apply, Send, Receive,
    Unreachable,
}

impl Intrinsic {
    pub fn signature(&self) -> Signature
    pub fn is_inlineable(&self) -> bool
    pub fn may_gc(&self) -> bool
    pub fn may_yield(&self) -> bool
}

pub fn emit_intrinsic(builder: &mut FunctionBuilder, intrinsic: Intrinsic, args: &[Value]) -> Value
```

### Compiler (Driver)

**File:** `dala_codegen/src/compiler.rs`

```rust
pub struct Compiler {
    codegen: CodeGenerator,
    code_registry: CodeRegistry,
}

impl Compiler {
    pub fn new(config: CodegenConfig) -> Result<Self, CodegenError>
    pub fn compile_file<P: AsRef<Path>>(&mut self, path: P) -> Result<Vec<CompiledFunction>, String>
    pub fn compile_bytes(&mut self, data: &[u8]) -> Result<Vec<CompiledFunction>, String>
    pub fn compile_beam_module(&mut self, beam_module: &BeamModule) -> Result<Vec<CompiledFunction>, String>
    pub fn codegen(&self) -> &CodeGenerator
    pub fn codegen_mut(&mut self) -> &mut CodeGenerator
    pub fn code_registry(&self) -> &CodeRegistry
}
```

---

## dala_beam_loader

### BeamModule

**File:** `dala_beam_loader/src/lib.rs`

```rust
pub struct BeamModule {
    pub name: String,
    pub functions: HashMap<(String, u32), BeamFunction>,
    pub exports: Vec<(String, u32, u32)>,  // (name, arity, label)
    pub atoms: Vec<String>,
    pub attributes: Vec<(String, String)>,
    pub compile_info: Option<CompileInfo>,
}

pub struct CompileInfo {
    pub source_file: Option<String>,
    pub options: Vec<String>,
}
```

#### Loading Functions

```rust
pub fn load_beam_file(path: &str) -> Result<BeamModule, BeamError>
pub fn load_beam_bytes(data: &[u8]) -> Result<BeamModule, BeamError>
pub fn load_beam<R: Read + Seek>(reader: R) -> Result<BeamModule, BeamError>
```

#### BeamModule Methods

```rust
impl BeamModule {
    pub fn new(name: String) -> Self
    pub fn get_function(&self, name: &str, arity: u32) -> Option<&BeamFunction>
    pub fn exported_functions(&self) -> &[(String, u32, u32)]
    pub fn function_count(&self) -> usize
}
```

### BeamFunction

**File:** `dala_beam_loader/src/bytecode.rs`

```rust
pub struct BeamFunction {
    pub name: String,
    pub arity: u32,
    pub label: u32,
    pub code: Vec<BeamInstruction>,
}

pub struct BeamInstruction {
    pub opcode: u32,
    pub operands: Vec<BeamOperand>,
    pub line: Option<u32>,
}

pub enum BeamOperand {
    Register(BeamRegister),
    Label(u32),
    Integer(i64),
    Float(f64),
    AtomIndex(u32),
}

pub enum BeamRegister {
    X(u32),
    Y(u32),
    F(u32),
}
```

### BeamReader

**File:** `dala_beam_loader/src/reader.rs`

Low-level BEAM binary format reader.

### BeamError

**File:** `dala_beam_loader/src/error.rs`

```rust
pub enum BeamError { /* ... */ }
pub type Result<T> = std::result::Result<T, BeamError>;
```

---

## dala_dispatch

### DispatchManager

**File:** `dala_dispatch/src/lib.rs`

```rust
pub struct DispatchManager { /* private */ }

impl DispatchManager {
    pub fn new() -> Self
    pub fn register_module(&self, module: CompiledModule) -> u64
    pub fn lookup_function(&self, module: u64, function: u64, arity: u32) -> Option<CodePtr>
    pub fn hot_replace(&self, module: CompiledModule) -> Result<(), HotCodeError>
    pub fn code_registry(&self) -> &CodeRegistry
}
```

### CompiledModule

```rust
pub struct CompiledModule {
    pub name: u64,
    pub exports: Vec<ExportEntry>,
    pub ir_module: IRModule,
    pub metadata: ModuleMetadata,
}

pub struct ModuleMetadata {
    pub source_file: Option<String>,
    pub compiler_options: Vec<String>,
    pub code_size: usize,
}

pub struct ExportEntry {
    pub function: u64,
    pub arity: u32,
    pub code_ptr: CodePtr,
    pub lazy_ref: LazyFnRef,
}
```

### ExportTable

**File:** `dala_dispatch/src/export_table.rs`

```rust
pub struct ExportTable { /* private */ }

impl ExportTable {
    pub fn new() -> Self
    pub fn register(&self, module: u64, function: u64, arity: u32, code_ptr: CodePtr)
    pub fn lookup(&self, module: u64, function: u64, arity: u32) -> Option<CodePtr>
    pub fn remove(&self, module: u64, function: u64, arity: u32) -> bool
    pub fn len(&self) -> usize
    pub fn is_empty(&self) -> bool
    pub fn module_exports(&self, module: u64) -> Vec<(u64, u32, CodePtr)>
}
```

### HotCodeManager

**File:** `dala_dispatch/src/hot_code.rs`

```rust
pub struct HotCodeManager { /* private */ }

impl HotCodeManager {
    pub fn new() -> Self
    pub fn update_module(&self, module_name: u64, module: CompiledModule)
    pub fn get_module(&self, module_name: u64) -> Option<CompiledModule>
    pub fn has_module(&self, module_name: u64) -> bool
    pub fn remove_module(&self, module_name: u64) -> bool
}
```

### LazyFnRef

```rust
#[repr(C)]
pub struct LazyFnRef { /* private */ }

impl LazyFnRef {
    pub fn new(module: u64, function: u64, arity: u32) -> Self
    pub fn get(&self) -> CodePtr
    pub fn set(&self, ptr: CodePtr)
    pub fn is_resolved(&self) -> bool
}
```

### HotCodeError

```rust
pub enum HotCodeError {
    ExportMismatch,
    ModuleNotFound(u64),
    CompilationError(String),
}
```

---

## CodePtr

**File:** `dala_runtime/src/code.rs`

```rust
#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct CodePtr { ptr: usize }

impl CodePtr {
    pub const fn null() -> Self
    pub fn is_null(self) -> bool
    pub fn from_raw(ptr: usize) -> Self
    pub fn as_usize(self) -> usize
}

unsafe impl Send for CodePtr {}
unsafe impl Sync for CodePtr {}
```

---

## dala_aot CLI

**File:** `dala_aot/src/cli.rs`

### Commands

```
dala_aot compile --input <FILE> --output <FILE> [--target <ARCH>] [--mode <MODE>] [-O <LEVEL>]
dala_aot inspect --input <FILE>
dala_aot run --input <FILE> [-- <ARGS>...] [--mode <MODE>]
dala_aot disasm --input <FILE>
```

### CompilationMode

```rust
pub enum CompilationMode {
    Jit,
    Aot,
}
```

### OptLevel

```rust
pub enum OptLevel {
    None,
    Less,
    Default,
    Aggressive,
}
```

### ExecutionMode

```rust
pub enum ExecutionMode {
    Interpreted,
    Mixed,
    Native,
}
```