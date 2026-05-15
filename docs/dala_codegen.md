# `dala_codegen` — Code Generation

## What It Is

`dala_codegen` translates the optimized Dala IR into **native machine code**
using the Cranelift code generator. It supports both JIT (for desktop/Android)
and AOT (for iOS/restricted environments) compilation modes.

## How It Fits In the Pipeline

```
dala_ir (optimized SSA IR)
    ↓
dala_codegen (IR → Cranelift IR → machine code)
    ↓
CompiledFunction { code_ptr, code_size, stack_map, frame_size }
    ↓
dala_dispatch (register for execution)
```

## Module Structure

```
dala_codegen/src/
├── lib.rs              — Compiler driver, CodeGenerator, CodegenConfig
├── compiler.rs         — compile_beam_module(), translate_function()
├── intrinsics.rs       — Intrinsic enum, emit_intrinsic()
├── runtime_glue.rs     — RuntimeGlue, RuntimeFuncId
├── stack_map.rs        — StackMapRegistry, StackMapEntry
└── trap_sink.rs        — TrapSink, TrapSite
```

## Compilation Modes

| Mode | Description | Use Case |
|------|-------------|----------|
| `Jit` | Compile and execute immediately | Desktop, development |
| `AOT` | Compile to object file for later linking | iOS, App Store, embedded |

## CodeGenerator

The main entry point for code generation:

```rust
pub struct CodeGenerator {
    config: CodegenConfig,
}

impl CodeGenerator {
    pub fn new(config: CodegenConfig) -> Result<Self, CodegenError>;
    pub fn compile_function(&mut self, ir_func: &IRFunction) -> Result<CompiledFunction, CodegenError>;
}
```

### CompiledFunction

```rust
pub struct CompiledFunction {
    pub code_ptr: *const u8,       // Pointer to native code
    pub code_size: usize,          // Size in bytes
    pub stack_map: Option<Vec<u8>>, // GC stack map
    pub frame_size: usize,         // Stack frame size
    pub spill_count: usize,        // Number of spilled registers
}
```

## Runtime Glue

Compiled code calls back into the runtime via **glue functions**. These are
declared by `RuntimeGlue` and compiled code references them as external
function calls:

```rust
pub enum RuntimeFuncId {
    Alloc,              // Heap allocation
    ShouldYield,        // Reduction check
    ConsumeReductions,  // Preemption
    BifDispatch,        // Built-in function dispatch
    Throw,              // Exception throwing
    Send,               // Message sending
    Receive,            // Message receiving
    LoadLiteral,        // Literal loading
    MakeFun,            // Closure creation
    BinaryNew,          // Binary construction
    BinarySize,         // Binary size query
    BinaryExtract,      // Binary extraction
    ListCons,           // List construction
    ListHead,           // List head access
    ListTail,           // List tail access
    MapGet,             // Map lookup
    MapPut,             // Map insertion
    TupleElement,       // Tuple field access
    Raise,              // Exception raising
    Apply,              // Function application
}
```

### How Glue Works

1. `RuntimeGlue::declare_all()` declares all runtime functions in the
   Cranelift module with their signatures
2. When the codegen encounters a `CallBif`, `Alloc`, etc., it emits a call
   to the corresponding glue function
3. At runtime, the glue function executes the operation and returns

## Intrinsics

Intrinsics are special functions that the compiler knows about and can emit
inline code for, rather than making a regular function call:

```rust
pub enum Intrinsic {
    GetProcess,     // Get current process pointer
    GetReductions,  // Get reduction count
    ShouldYield,    // Check if should yield
    GetHeapPtr,     // Get heap pointer
    SetHeapPtr,     // Set heap pointer
    IsSmallInt,     // Type test: small integer
    IsAtom,         // Type test: atom
    IsTuple,        // Type test: tuple
    IsList,         // Type test: list
    IsFloat,        // Type test: float
    IsMap,          // Type test: map
    IsBinary,       // Type test: binary
    IsFun,          // Type test: function
    IsPid,          // Type test: PID
    IsPort,         // Type test: port
    TupleElement,   // Tuple field access
    MapGet,         // Map lookup
    MapPut,         // Map insert
    BinaryNew,      // Binary construction
    ListCons,       // List construction
    ListHead,       // List head
    ListTail,       // List tail
    Raise,          // Raise exception
    Error,          // Error
    Throw,          // Throw
    Apply,          // Apply function
    Send,           // Send message
    Receive,        // Receive message
    Unreachable,    // Unreachable code marker
}
```

### Inlineable Intrinsics

Some intrinsics can be inlined (emitted as direct machine instructions):

```rust
impl Intrinsic {
    pub fn is_inlineable(&self) -> bool {
        matches!(self,
            Intrinsic::IsSmallInt | Intrinsic::IsAtom | Intrinsic::IsTuple
            | Intrinsic::IsList | Intrinsic::IsFloat | Intrinsic::IsMap
            | Intrinsic::IsBinary | Intrinsic::IsFun | Intrinsic::IsPid
            | Intrinsic::IsPort | Intrinsic::GetReductions
            | Intrinsic::ShouldYield | Intrinsic::GetHeapPtr
            | Intrinsic::GetStackPtr | Intrinsic::ListHead
            | Intrinsic::ListTail | Intrinsic::TupleElement
        )
    }
}
```

For example, `IsSmallInt` becomes a single `AND` + `CMP` instruction
instead of a function call.

## Stack Maps

Stack maps describe which stack slots and registers contain heap pointers
at each GC safepoint. They are generated by the codegen and consumed by the
GC's root set scanner.

```rust
pub struct StackMapEntry {
    pub instruction_offset: u32,  // Native code offset
    pub live_registers: u64,      // Bitmask of live X registers
    pub live_stack_count: u32,    // Number of live stack slots
}

pub struct StackMapRegistry {
    maps: HashMap<u64, Vec<StackMapEntry>>,
}
```

### Stack Map Generation

The codegen generates stack maps at:
- Function calls (all arguments are live)
- GC safepoints (`GcSafe` IR instruction)
- Exception handlers (catch labels)

## Trap Handling

Traps are used for exception handling in native code. When a trap is hit, the
runtime walks the catch stack to find a handler.

```rust
pub struct TrapSite {
    pub offset: u32,       // Code offset of the trap
    pub trap_code: u32,    // Type of trap (overflow, badarg, etc.)
    pub beam_offset: u32,  // Source BEAM instruction
}

pub struct TrapSink {
    traps: Vec<TrapSite>,
}
```

## Compiler Driver

The `Compiler` struct orchestrates the full compilation pipeline:

```rust
pub struct Compiler {
    codegen: CodeGenerator,
    code_registry: CodeRegistry,
}

impl Compiler {
    pub fn new(config: CodegenConfig) -> Result<Self, CodegenError>;
    pub fn compile_beam_module(&mut self, ir_module: &IRModule) -> Result<Vec<CompiledFunction>, String>;
    pub fn translate_function(&self, func: &mut IRFunction) -> Result<(), String>;
    pub fn register_code(&mut self, module, name, arity, code_ptr, is_aot);
}
```

### Compilation Flow

```
1. Receive IRModule from dala_beam_loader
2. For each function in the module:
   a. Run optimization passes (via dala_ir::opt::optimize)
   b. Translate IR → Cranelift IR
   c. Generate machine code
   d. Collect stack maps and trap sites
3. Return Vec<CompiledFunction>
4. Register in CodeRegistry via dala_dispatch
```

## Tracing & Debugging

### Enable Codegen Tracing

```bash
RUST_LOG=dala_codegen=trace cargo run --bin dala_aot -- compile --input test.beam
```

### Inspect Compiled Functions

```rust
let compiled = codegen.compile_function(&ir_func)?;
println!("Code size: {} bytes", compiled.code_size);
println!("Frame size: {} bytes", compiled.frame_size);
println!("Spills: {}", compiled.spill_count);
if let Some(map) = &compiled.stack_map {
    println!("Stack map: {} bytes", map.len());
}
```

### Disassemble Output

```bash
# Compile to object file
dala_aot compile --input test.beam --output test.o --target x86_64 --mode aot

# Disassemble
objdump -d test.o
# or on macOS
otool -tv test.o
```

## Developing New Features

### Adding a New Intrinsic

1. Add variant to `Intrinsic` enum in `intrinsics.rs`
2. Update `is_inlineable()` and `may_gc()` if needed
3. In codegen, add a match arm in the instruction lowering:
   ```rust
   IRInstKind::CallIntrinsic { intrinsic: MyIntrinsic, args } => {
       emit_my_intrinsic(builder, args)?;
   }
   ```

### Adding a New Runtime Function

1. Add variant to `RuntimeFuncId` enum in `runtime_glue.rs`
2. Declare the function signature in `declare_all()`
3. Implement the function in `dala_runtime`
4. Reference it from codegen when lowering the corresponding IR instruction

### Supporting a New Target

Cranelift supports multiple targets. To add a new one:

1. Update `CodegenConfig.target`
2. Set the appropriate Cranelift ISA config:
   ```rust
   let isa = lookup(target_lexicon::Triple::host())?
       .finish(Builder::new())?;
   ```
3. Test with `--target aarch64` or `--target x86_64`
