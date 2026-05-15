# Reference Guide

Complete API reference for all Dala Compiler Runtime crates.

## Table of Contents

- [dala_runtime](#dala_runtime)
  - [Term](#term)
  - [RegisterFile](#registerfile)
  - [Process](#process)
  - [Scheduler](#scheduler)
  - [QoS & Governor](#qos--governor)
  - [Mailbox](#mailbox)
  - [Memory Regions](#memory-regions)
  - [AI Runtime](#ai-runtime)
  - [GC](#gc)
  - [BIFs](#bifs)
  - [Exception](#exception)
  - [Trap](#trap)
- [dala_ir](#dala_ir)
  - [IRContext](#ircontext)
  - [IRModule](#irmodule)
  - [IRFunction](#irfunction)
  - [IRInst](#irinst)
  - [IRType](#irtype)
  - [Optimization Passes](#optimization-passes)
- [dala_codegen](#dala_codegen)
- [dala_beam_loader](#dala_beam_loader)
- [dala_dispatch](#dala_dispatch)
- [dala_aot CLI](#dala_aot-cli)

---

## dala_runtime

### Term

A **tagged pointer** — all BEAM values are represented as a `u64`.

```rust
#[repr(transparent)]
pub struct Term(u64);
```

#### Constants

| Constant | Value |
|----------|-------|
| `PRIMARY_TAG_MASK` | `0b11` |
| `PRIMARY_TAG_BOXED` | `0b00` |
| `PRIMARY_TAG_LIST` | `0b01` |
| `PRIMARY_TAG_HEADER` | `0b10` |
| `PRIMARY_TAG_IMMED1` | `0b11` |
| `IMMED1_SMALL` | Small integer tag |
| `IMMED1_PID` | PID tag |
| `IMMED1_PORT` | Port tag |
| `IMMED1_IMMED2` | Immed2 sub-tag |
| `IMMED2_ATOM` | Atom tag |
| `IMMED2_SPECIAL` | Special (nil/true/false) |
| `SPECIAL_NIL` | nil value |
| `SPECIAL_TRUE` | true value |
| `SPECIAL_FALSE` | false value |

#### Constructors

| Method | Description |
|--------|-------------|
| `Term::from_raw(bits)` | Create from raw u64 |
| `Term::nil()` | nil constant |
| `Term::true_()` | true constant |
| `Term::false_()` | false constant |
| `Term::small(i)` | Small integer |
| `Term::atom(idx)` | Atom by index |
| `Term::bool(b)` | Boolean |

#### Accessors

| Method | Returns |
|--------|---------|
| `is_small()` | `bool` — is this a small integer? |
| `is_atom()` | `bool` — is this an atom? |
| `is_list()` | `bool` — is this a cons cell? |
| `is_tuple()` | `bool` — is this a tuple? |
| `is_map()` | `bool` — is this a map? |
| `is_boxed()` | `bool` — is this a boxed pointer? |
| `is_float()` | `bool` — is this a float? |
| `is_binary()` | `bool` — is this a binary? |
| `is_nil()` | `bool` — is this nil? |
| `is_true()` | `bool` — is this true? |
| `is_false()` | `bool` — is this false? |
| `is_pid()` | `bool` — is this a PID? |
| `is_port()` | `bool` — is this a port? |
| `is_fun()` | `bool` — is this a function? |
| `get_small()` | `Option<i64>` — extract small integer |
| `get_atom_index()` | `Option<u32>` — extract atom index |
| `get_list_ptr()` | `*const Term` — list head pointer |
| `get_boxed_ptr()` | `*const Term` — boxed pointer |
| `header()` | `u64` — raw header word |
| `header_arity()` | `u32` — arity from header |
| `header_tag()` | `u64` — tag from header |
| `tuple_get(index)` | `Term` — element at index |
| `get_float()` | `Option<f64>` — extract float |

---

### RegisterFile

```rust
pub struct RegisterFile {
    pub x: [Term; 256],  // Argument/return registers
    pub y: [Term; 1023], // Stack frame slots
    pub f: [Term; 256],  // Floating point registers
}
```

---

### Process

```rust
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
    pub current_function: (u64, u64, u32),
    pub code: CodePtr,
    pub group_leader: u64,
    pub error_handler: Term,
    pub priority: u8,
    pub qos: QosClass,
    pub arena: Arena,
    pub stable_region: StableImmutableRegion,
    pub status: ProcessStatus,
    pub exit_reason: Option<Term>,
}
```

#### ProcessFlags

```rust
pub struct ProcessFlags: u32 {
    const TRAP_EXIT       = 0b0000_0001;
    const TRACING         = 0b0000_0010;
    const SYS_TRACE       = 0b0000_0100;
    const HEAP_SNAPSHOT   = 0b0000_1000;
    const SUSPENDED       = 0b0001_0000;
    const RUNNING         = 0b0010_0000;
    const RUNABLE         = 0b0100_0000;
    const DIRTY_CPU_SCHED = 0b1000_0000;
}
```

#### CatchFrame

```rust
pub struct CatchFrame {
    pub catch_label: u64,
    pub stack_pointer: usize,
    pub heap_pointer: usize,
    pub cp: u64,
}
```

#### ProcessStatus

```rust
pub enum ProcessStatus { Running, Runnable, Waiting, Suspended, Exiting }
```

#### ProcessBuilder

| Method | Description |
|--------|-------------|
| `new(pid)` | Create builder with given PID |
| `heap_size(size)` | Set initial heap size |
| `reductions(reds)` | Set reduction budget |
| `priority(p)` | Set priority |
| `group_leader(pid)` | Set group leader |
| `initial_call(m, f, a)` | Set initial function |
| `build()` | Build the Process |

#### Key Process Methods

| Method | Description |
|--------|-------------|
| `pid_term()` | Get PID as Term |
| `alloc(value)` | Allocate a single term on the heap |
| `alloc_words(count)` | Allocate raw words |
| `push(value)` | Push onto stack |
| `pop()` | Pop from stack |
| `stack_start()` | Stack start for GC |
| `stack_end()` | Stack end for GC |
| `heap_start()` | Heap start for GC |
| `heap_alloc_ptr()` | Heap pointer for GC |
| `set_high_water()` | Set GC high water mark |
| `consume_reductions(count)` | Consume reductions, return true if should yield |
| `reset_reductions()` | Reset to max |
| `push_catch(frame)` | Install catch handler |
| `pop_catch()` | Remove catch handler |
| `send(msg)` | Send message to mailbox |

---

### Scheduler

#### QosClass

```rust
pub enum QosClass {
    Background = 0,  // Analytics, cleanup
    Utility = 1,     // Data processing, caching
    UserFacing = 2,  // UI, user interactions
    Realtime = 3,    // Voice, video, sensor fusion
}
```

#### ThermalState

```rust
pub enum ThermalState { Nominal, Fair, Serious, Critical }
```

#### BatteryState

```rust
pub struct BatteryState {
    pub level: f32,      // 0.0 - 1.0
    pub charging: bool,
}
```

#### Governor

```rust
impl Governor {
    pub fn new() -> Self;
    pub fn set_thermal(state: ThermalState);
    pub fn set_battery(state: BatteryState);
    pub fn is_throttling() -> bool;
    pub fn max_qos() -> QosClass;           // Max allowed QoS under current conditions
    pub fn reduction_budget(qos: QosClass) -> u32;  // Scaled by thermal state
}
```

#### SchedulerMessage

```rust
pub enum SchedulerMessage {
    Spawn { pid, module, function, arity, args, qos: QosClass },
    Message { pid, msg },
    Kill(u64),
    UpdateThermal(ThermalState),
    UpdateBattery(BatteryState),
    Halt,
}
```

#### Key Scheduler Methods

| Method | Description |
|--------|-------------|
| `global_init(config)` | Initialize the global scheduler |
| `spawn(module, func, arity, args, qos)` | Spawn a new process |
| `send_message(pid, msg)` | Send a message to a process |
| `update_thermal(state)` | Update thermal state |
| `update_battery(state)` | Update battery state |

---

### RuntimeConfig

```rust
pub struct RuntimeConfig {
    pub scheduler_count: usize,
    pub initial_heap_size: usize,
    pub max_heap_size: usize,
    pub reductions_per_yield: u32,
    pub debug_gc: bool,
    pub execution_mode: ExecutionMode,
}

pub enum ExecutionMode { Interpreted, Mixed, Aot }
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

### Mailbox

#### MessageEnvelope

```rust
pub struct MessageEnvelope {
    pub payload: Term,
    pub priority: MessagePriority,
    pub sender: u64,
    pub type_tag: Option<u32>,
}

pub enum MessagePriority { Low, Normal, High, Critical }
```

#### Mailbox Methods

| Method | Description |
|--------|-------------|
| `new()` | Create empty mailbox (default capacity) |
| `with_capacity(max)` | Create with custom capacity |
`enqueue(msg)` | Add message (returns false if full) |
| `dequeue()` | Remove highest-priority message |
| `dequeue_typed(tag)` | Fast-path: remove message matching type tag |
| `peek()` | Peek at next message |
| `is_empty()` | Check if empty |
| `len()` | Total message count |
| `critical_count()` | Critical-priority message count |
| `high_count()` | High-priority message count |
| `drain()` | Remove all messages |

---

### Memory Regions

#### Arena

```rust
impl Arena {
    pub fn new(chunk_size: usize) -> Self;
    pub fn alloc(&self, size: usize) -> *mut u8;
    pub fn alloc_aligned(&self, size: usize, align: usize) -> *mut u8;
    pub fn alloc_layout(&self, layout: Layout) -> *mut u8;
    pub fn reset(&self);  // Bulk free, O(1)
    pub fn total_capacity(&self) -> usize;
    pub fn total_used(&self) -> usize;
    pub fn chunk_count(&self) -> usize;
}
```

#### StableImmutableRegion

```rust
impl StableImmutableRegion {
    pub fn new(id: RegionId, capacity: usize) -> Self;
    pub fn allocate_immutable(&self, layout: &Layout) -> *mut u8;
    pub fn contains(&self, ptr: *const u8) -> bool;
    pub fn type_count(&self) -> usize;
}
```

#### BinaryRegion

```rust
impl BinaryRegion {
    pub fn new(id: RegionId, capacity: usize) -> Self;
    pub fn alloc_binary(&self, size: usize) -> *mut u8;
    pub fn incref(&self, ptr: *mut u8);
    pub fn decref(&self, ptr: *mut u8);  // Frees when refcount = 0
}
```

#### TensorRegion

```rust
impl TensorRegion {
    pub fn new(id: RegionId, capacity: usize) -> Self;
    pub fn alloc_tensor(&self, size: usize, gpu: bool) -> *mut u8;
    pub fn gpu_usage(&self) -> usize;
}
```

#### NativeResourceRegion

```rust
impl NativeResourceRegion {
    pub fn new(id: RegionId) -> Self;
    pub fn register(kind, handle, owned, shareable, owner) -> NativeResourceId;
    pub fn get(id) -> Option<NativeResourceEntry>;
    pub fn release(id) -> bool;
    pub fn transfer(id, new_owner) -> bool;
    pub fn len() -> usize;
}
```

---

### AI Runtime

#### Tensor

```rust
pub struct Tensor {
    pub desc: TensorDesc,
    data: *mut u8,
    size: usize,
    location: TensorLocation,
    refcount: u32,
}

pub struct TensorDesc {
    pub dtype: TensorDtype,
    pub shape: Vec<u64>,
    pub num_elements: u64,
    pub size_bytes: u64,
}

pub enum TensorDtype { F32, F16, F64, I32, I64, U8, Bool }
pub enum TensorLocation { Host, Gpu, Ane }
```

#### InferenceWorker

```rust
pub struct InferenceWorker {
    config: WorkerConfig,
    active_requests: usize,
    throttled: bool,
}

pub struct WorkerConfig {
    pub max_concurrent: usize,
    pub enable_cache: bool,
    pub thermal_threshold: f32,
}

pub enum InferencePriority { Background, Normal, UserFacing, Realtime }
```

#### ModelRegistry

```rust
impl ModelRegistry {
    pub fn new(max_memory: usize) -> Self;
    pub fn load_model(name, path) -> Result<ModelId, AiError>;
    pub fn get(id: ModelId) -> Option<ModelHandle>;
    pub fn unload(id: ModelId) -> bool;
}
```

---

### GC

#### GCConfig

```rust
pub struct GCConfig {
    pub nursery_size: usize,
    pub max_copy: usize,
    pub fullsweep_after: usize,
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
    pub entries: [StackMapEntry],
}

pub struct StackMapEntry {
    pub offset: u32,
    pub is_pointer: bool,
    pub value_type: StackMapType,
}

pub enum StackMapType {
    Unknown, TuplePointer, ListPointer, BoxedPointer,
    FunPointer, MapPointer, BinaryPointer, MaybePointer,
}
```

#### Header (ObjectHeader)

```rust
pub struct ObjectHeader(AtomicU64);

pub enum GcColor { White, Gray, Black, StableBlack }

impl ObjectHeader {
    pub fn new(type_idx: u16, size_words: u8, immutable: bool) -> Self;
    pub fn is_forwarded(&self) -> bool;
    pub fn forward_ptr(&self) -> *mut u8;
    pub fn survival_count(&self) -> u8;
    pub fn gc_color(&self) -> GcColor;
    pub fn is_immutable(&self) -> bool;
    pub fn size_words(&self) -> u8;
    pub fn type_index(&self) -> u16;
    pub fn set_color(color: GcColor) -> GcColor;
    pub fn increment_survival(&self);
    pub fn should_promote_to_old(&self) -> bool;
    pub fn should_promote_to_stable(&self) -> bool;
}
```

---

### BIFs (Built-In Functions)

#### Registration

```rust
pub fn register_all_bifs(registry: &mut BifRegistry);
pub fn lookup_bif(module: u64, function: u64, arity: u32) -> Option<BifFn>;
```

#### BifDescriptor

```rust
pub struct BifDescriptor {
    pub module: u64,
    pub function: u64,
    pub arity: u32,
    pub implementation: BifFn,
}
```

#### Implemented BIFs

| Module | Function | Arity | Description |
|--------|----------|-------|-------------|
| erlang | + | 2 | Integer addition |
| erlang | - | 2 | Integer subtraction |
| erlang | * | 2 | Integer multiplication |
| erlang | / | 2 | Integer division |
| erlang | rem | 2 | Integer remainder |
| erlang | - | 1 | Negation |
| erlang | is_integer | 1 | Type test: integer |
| erlang | is_atom | 1 | Type test: atom |
| erlang | is_binary | 1 | Type test: binary |
| erlang | is_boolean | 1 | Type test: boolean |
| erlang | is_tuple | 1 | Type test: tuple |
| erlang | is_list | 1 | Type test: list |
| erlang | is_pid | 1 | Type test: PID |
| erlang | is_port | 1 | Type test: port |
| erlang | is_function | 1 | Type test: function |
| erlang | is_map | 1 | Type test: map |
| erlang | is_number | 1 | Type test: number |
| erlang | is_float | 1 | Type test: float |
| erlang | == | 2 | Equality |
| erlang | /= | 2 | Inequality |
| erlang | =:= | 2 | Exact equality |
| erlang | self | 0 | Self PID |
| erlang | spawn | 3 | Spawn process |
| erlang | send | 2 | Send message |
| erlang | error | 1 | Raise error |
| erlang | throw | 1 | Throw exception |
| erlang | exit | 1 | Exit process |
| erlang | tuple_size | 1 | Tuple size |
| erlang | size | 1 | Size of term |
| erlang | length | 1 | List length |
| erlang | hd | 1 | List head |
| erlang | tl | 1 | List tail |
| erlang | node | 0 | Node name |
| erlang | integer_to_list | 1 | Integer → string |
| erlang | list_to_integer | 1 | String → integer |
| erlang | atom_to_list | 1 | Atom → string |
| erlang | list_to_atom | 1 | String → atom |
| erlang | float | 1 | To float |

---

### Exception

```rust
pub enum Reason { Normal, Error(Term), Exit(Term), Throw(Term) }

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

| Function | Description |
|----------|-------------|
| `Exception::error(reason)` | Create error exception |
| `Exception::exit(reason)` | Create exit exception |
| `Exception::throw(reason)` | Create throw exception |

#### Result Helpers

| Function | Description |
|----------|-------------|
| `propagate(result)` | Propagate exception through native frame |
| `exception_result(reason)` | Convert reason to Result |
| `ok_term(term)` | Create successful Result |
| `error_term(reason)` | Create error Result |
| `is_exception(result)` | Check if Result is exception |

---

### Trap

```rust
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

```rust
pub struct IRContext {
    pub module: IRModule,
    pub functions: Vec<IRFunction>,
    pub constants: Vec<IRValue>,
    pub types: Vec<IRType>,
}

impl IRContext {
    pub fn new() -> Self;
    pub fn create_function(name, arity) -> IRFunctionId;
    pub fn get_function(id) -> &IRFunction;
    pub fn get_function(id) -> &mut IRFunction;
    pub fn create_type(ty) -> TypeId;
    pub fn get_type(id) -> &IRType;
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

```rust
pub struct IRModule {
    pub name: u64,
    pub functions: IndexMap<(u64, u32), IRFunctionId>,
    pub function_bodies: Vec<IRFunction>,
    pub exports: Vec<(u64, u32)>,
    pub imports: IndexMap<u64, Vec<(u64, u32)>>,
    pub attributes: Vec<(u64, u64)>,
    pub compile_info: CompileInfo,
    pub literals: Vec<u64>,
    pub line_info: Vec<(u32, u32)>,
}
```

| Method | Description |
|--------|-------------|
| `new(name)` | Create empty module |
| `add_function(name, arity)` | Add a function |
| `get_function(name, arity)` | Look up function ID |
| `get_function_body(id)` | Get function body |
| `add_export(name, arity)` | Add export |
| `add_import(module, func, arity)` | Add import |
| `add_literal(value)` | Add literal to table |
| `is_exported(name, arity)` | Check if exported |
| `exported_functions()` | Get all exports |
| `function_count()` | Number of functions |

---

### IRFunction

```rust
pub struct IRFunction {
    pub module: u64,
    pub name: u64,
    pub arity: u32,
    pub file: u64,
    pub line: u32,
    pub blocks: Vec<BasicBlock>,
    pub entry_block: BlockId,
    pub param_types: Vec<TypeId>,
    pub return_type: TypeId,
    pub locals: Vec<(Reg, IRValueId)>,
    pub compiled: bool,
    pub stack_maps: Vec<StackMapEntry>,
}
```

| Method | Description |
|--------|-------------|
| `new(module, name, arity)` | Create function |
| `create_block()` | Create new basic block |
| `get_block(id)` | Get block by ID |
| `block_count()` | Number of blocks |
| `name_str()` | Function name as string |
| `full_name()` | Fully qualified name |
| `add_param_type(ty)` | Add parameter type |
| `set_return_type(ty)` | Set return type |
| `add_stack_map(offset, live_regs, live_stack)` | Record stack map |

---

### IRInst

```rust
pub struct IRInst {
    pub kind: IRInstKind,
    pub result: Option<IRValueId>,
    pub operands: Vec<IRValueId>,
    pub beam_offset: u32,
    pub side_effects: SideEffects,
}
```

#### IRInstKind Enum (Selected)

**Arithmetic**: `Add`, `Sub`, `Mul`, `Div`, `Rem`, `Neg`

**Bitwise**: `BitAnd`, `BitOr`, `BitXor`, `BitNot`, `ShiftLeft`, `ShiftRight`

**Comparison**: `Eq`, `Ne`, `Gt`, `Ge`, `Lt`, `Le`

**Type Tests**: `IsSmallInt`, `IsFloat`, `IsAtom`, `IsTuple`, `IsList`, `IsMap`, `IsBinary`, `IsFun`, `IsPid`, `IsNil`, `IsTrue`, `IsFalse`

**Memory**: `Alloc { words }`, `Load { base, offset }`, `Store { base, offset, value }`, `TupleGet { tuple, index }`, `TupleSet { tuple, index, value }`

**Control Flow**: `Br { target }`, `BrIf { cond, true_target, false_target }`, `Switch { value, default, targets }`, `Ret { value }`, `Call { func, args }`, `TailCall { func, args }`, `CallBif { module, function, args }`

**Exceptions**: `Catch { handler }`, `CatchPop`, `Throw { reason }`, `Resume { exception }`

**Process**: `ConsumeReductions { count }`, `Send { dest, msg }`, `Recv { timeout }`

**Literals**: `LoadLiteral { index }`, `ConstSmallInt { value }`, `ConstAtom { index }`, `ConstNil`, `ConstTrue`, `ConstFalse`

**Binaries**: `BinaryNew { data }`, `BinarySize { binary }`, `BinaryExtract { binary, offset, size, flags }`

**Funs**: `MakeFun { module, function, arity, fvs }`

**Actor Operations**: `SpawnActor { module, args, qos }`, `SendTyped { target, msg, type_tag, priority }`, `RecvTyped { type_tag, timeout }`

**Stable Memory**: `AllocStable { type_desc, words }`, `PromoteStable { object }`

**Tensor Operations**: `TensorNew { desc_idx, gpu }`, `TensorOp { op, inputs }`

**Capability Operations**: `CapNew { resource_kind, owned }`, `CapRelease { cap }`, `CapTransfer { cap, new_owner }`

**AI Operations**: `InferenceSubmit { model_id, input, priority }`, `InferenceAwait { request }`

**Arena Operations**: `ArenaAlloc { arena, size, align }`, `ArenaReset { arena }`

**Other**: `GcSafe`, `Nop`

#### TensorOpKind

```rust
pub enum TensorOpKind { Add, Mul, MatMul, Relu, Softmax, Concat, Reshape, Transpose }
```

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
pub enum Reg { X(u32), Y(u32), F(u32) }
```

---

### IRType

```rust
pub struct IRType { pub kind: TypeKind }

pub enum TypeKind {
    Any, Bottom,
    SmallInt, NonNegInt, Int64, Float, Atom, Boolean, Nil,
    Cons, List,
    Tuple { arity: u32 },
    StableTuple { element_types: Vec<IRType>, immutable: bool },
    Map, Binary, Fun { arity: u32 },
    Pid, Port, Reference,
    Message { payload: Box<IRType>, priority: MessagePriority },
    Actor { accepts: Vec<IRType>, lifecycle: ActorLifecycle },
    Tensor { dtype: TensorDtype, shape: Vec<Option<u64>> },
    Capability { resource: NativeResourceKind, owned: bool, shareable: bool },
    Union(Box<IRType>, Box<IRType>),
    Constant(ConstantValue),
}
```

#### Lattice Operations

| Method | Description |
|--------|-------------|
| `join(&self, other)` | Least upper bound |
| `meet(&self, other)` | Greatest lower bound |
| `contains(&self, other)` | Subtype check |

#### Type Predicates

| Method | Description |
|--------|-------------|
| `is_definitely_small_int()` | Is this a small integer? |
| `is_definitely_atom()` | Is this an atom? |
| `is_definitely_tuple()` | Is this a tuple? |
| `is_definitely_list()` | Is this a list? |
| `is_definitely_map()` | Is this a map? |
| `is_definitely_float()` | Is this a float? |
| `is_definitely_fun()` | Is this a function? |
| `is_definitely_pid()` | Is this a PID? |
| `is_immutable()` | Is this compiler-proven immutable? |
| `is_message()` | Is this a message type? |
| `is_actor()` | Is this an actor type? |
| `is_tensor()` | Is this a tensor type? |
| `is_capability()` | Is this a capability type? |

---

### Optimization Passes

```rust
pub fn optimize(func: &mut IRFunction);  // Run all passes until convergence
pub fn run_pass(func: &mut IRFunction, pass_name: &str) -> bool;  // Run single pass
```

| Pass Name | File | Description |
|-----------|------|-------------|
| `dce` | `dce.rs` | Dead code elimination |
| `const-prop` | `const_prop.rs` | Constant propagation |
| `fold` | `const_prop.rs` | Constant folding |
| `cse` | `cse.rs` | Common subexpression elimination |
| `simplify-cfg` | `simplify_cfg.rs` | CFG simplification |
| `tail-call` | `tail_call.rs` | Tail call analysis |
| `pattern-match` | `pattern_match.rs` | Pattern matching optimization |

---

## dala_codegen

### CodeGenerator

```rust
pub struct CodeGenerator { config: CodegenConfig }

pub enum CompilationMode { Jit, Aot }

pub struct CodegenConfig {
    pub mode: CompilationMode,
    pub target: String,
    pub opt_level: &'static str,
    pub debug_assertions: bool,
    pub verbose: bool,
}

impl CodeGenerator {
    pub fn new(config: CodegenConfig) -> Result<Self, CodegenError>;
    pub fn compile_function(ir_func: &IRFunction) -> Result<CompiledFunction, CodegenError>;
}
```

### CompiledFunction

```rust
pub struct CompiledFunction {
    pub code_ptr: *const u8,
    pub code_size: usize,
    pub stack_map: Option<Vec<u8>>,
    pub frame_size: usize,
    pub spill_count: usize,
}
```

### CodegenError

```rust
pub enum CodegenError {
    TargetError(String),
    CompilationError(String),
    Unsupported(String),
    LinkError(String),
}
```

### RuntimeGlue

```rust
pub enum RuntimeFuncId {
    Alloc, ShouldYield, ConsumeReductions, BifDispatch,
    Throw, Send, Receive, LoadLiteral, MakeFun,
    BinaryNew, BinarySize, BinaryExtract,
    ListCons, ListHead, ListTail, MapGet, MapPut,
    TupleElement, Raise, Apply,
}
```

### Intrinsic

```rust
pub enum Intrinsic {
    GetProcess, GetReductions, SetReductions, ShouldYield,
    GetHeapPtr, SetHeapPtr, GetStackPtr, SetStackPtr,
    GcBarrier, IsSmallInt, IsAtom, IsTuple, IsList,
    IsFloat, IsMap, IsBinary, IsFun, IsPid, IsPort,
    TupleElement, MapGet, MapPut, BinaryNew, BinaryMatch,
    ListCons, ListHead, ListTail, Raise, Error, Throw,
    Apply, Send, Receive, Unreachable,
}
```

---

## dala_beam_loader

### BeamModule

```rust
pub struct BeamModule {
    pub name: String,
    pub functions: HashMap<(String, u32), BeamFunction>,
    pub exports: Vec<(String, u32, u32)>,
    pub atoms: Vec<String>,
    pub attributes: Vec<(String, String)>,
    pub compile_info: Option<CompileInfo>,
}
```

### Loading Functions

```rust
pub fn load_beam_file(path: &str) -> Result<BeamModule>;
pub fn load_beam_bytes(data: &[u8]) -> Result<BeamModule>;
pub fn load_beam<R: Read + Seek>(reader: R) -> Result<BeamModule>;
```

### BeamFunction

```rust
pub struct BeamFunction {
    pub name: String,
    pub arity: u32,
    pub label: u32,
    pub code: Vec<BeamInstruction>,
}
```

### BeamInstruction

```rust
pub struct BeamInstruction {
    pub opcode: u32,
    pub operands: Vec<BeamOperand>,
    pub line: Option<u32>,
}

pub enum BeamOperand {
    Register(BeamRegister),  // X(n), Y(n), F(n)
    Label(u32),
    Integer(i64),
    Float(f64),
    AtomIndex(u32),
}

pub enum BeamRegister { X(u32), Y(u32), F(u32) }
```

### BeamError

```rust
pub enum BeamError {
    IoError(String),
    FormatError(String),
    UnexpectedEof,
    Unsupported(String),
}
```

---

## dala_dispatch

### DispatchManager

```rust
pub struct DispatchManager {
    modules: DashMap<u64, Arc<CompiledModule>>,
    export_table: ExportTable,
    hot_code: HotCodeManager,
    code_registry: CodeRegistry,
}

impl DispatchManager {
    pub fn new() -> Self;
    pub fn register_module(module: CompiledModule) -> u64;
    pub fn lookup_function(module, function, arity) -> Option<CodePtr>;
    pub fn hot_replace(module: CompiledModule) -> Result<(), HotCodeError>;
    pub fn code_registry() -> &CodeRegistry;
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

pub struct ExportEntry {
    pub function: u64,
    pub arity: u32,
    pub code_ptr: CodePtr,
    pub lazy_ref: LazyFnRef,
}
```

### ExportTable

```rust
impl ExportTable {
    pub fn new() -> Self;
    pub fn register(module, function, arity, code_ptr);
    pub fn lookup(module, function, arity) -> Option<CodePtr>;
    pub fn remove(module, function, arity) -> bool;
    pub fn len() -> usize;
    pub fn module_exports(module) -> Vec<(u64, u32, CodePtr)>;
}
```

### HotCodeManager

```rust
impl HotCodeManager {
    pub fn new() -> Self;
    pub fn update_module(module_name, module);
    pub fn get_module(module_name) -> Option<CompiledModule>;
    pub fn has_module(module_name) -> bool;
    pub fn remove_module(module_name) -> bool;
}
```

### LazyFnRef

```rust
impl LazyFnRef {
    pub fn new(module, function, arity) -> Self;
    pub fn get() -> CodePtr;
    pub fn set(ptr: CodePtr);
    pub fn is_resolved() -> bool;
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

```rust
pub struct CodePtr { ptr: usize }

impl CodePtr {
    pub const fn null() -> Self;
    pub fn is_null() -> bool;
    pub fn from_raw(ptr: usize) -> Self;
    pub fn as_usize() -> usize;
}
unsafe impl Send for CodePtr {}
unsafe impl Sync for CodePtr {}
```

---

## dala_aot CLI

### Commands

| Command | Description |
|---------|-------------|
| `compile` | Compile BEAM file to native code |
| `inspect` | Inspect a BEAM file |
| `run` | Run a BEAM module |
| `disasm` | Disassemble BEAM bytecode |

### CompilationMode

```rust
pub enum CompilationMode { Jit, Aot }
```

### OptLevel

```rust
pub enum OptLevel { None, Less, Default, Aggressive }
```

### ExecutionMode

```rust
pub enum ExecutionMode { Interpreted, Mixed, Native }
```
