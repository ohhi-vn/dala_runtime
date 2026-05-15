# `dala_ir` — Intermediate Representation

## What It Is

`dala_ir` is the **typed SSA intermediate representation** that bridges BEAM
bytecode and native code generation. It is the central hub of the compiler —
every optimization, backend lowering, and runtime feature flows through it.

## How It Fits In the Pipeline

```
BEAM Bytecode
    ↓
dala_beam_loader (parses .beam → BeamModule)
    ↓
dala_ir (builds SSA IR from BeamModule)
    ↓
dala_ir::opt (optimizes the IR)
    ↓
dala_codegen (lowers IR → native machine code)
```

## Module Structure

```
dala_ir/src/
├── lib.rs              — IRContext, top-level re-exports
├── type_system.rs      — IRType, TypeKind, TypeDescriptor, lattice ops
├── instruction.rs      — IRInst, IRInstKind, SideEffects, Reg, Label
├── value.rs            — IRValue, IRValueId, ValueUse, ValueDef
├── function.rs         — IRFunction, BasicBlock, FunctionSignature
├── module.rs           — IRModule, CompilationUnit
├── builder.rs          — IRBuilder (constructs SSA IR from bytecode)
├── layout.rs           — FrameLayout, Slot, BeamCallingConvention
├── constant.rs         — Constant enum
└── opt/                — Optimization passes
    ├── mod.rs          — optimize() entry point, run_pass()
    ├── dce.rs          — Dead Code Elimination
    ├── const_prop.rs   — Constant Propagation & Folding
    ├── cse.rs          — Common Subexpression Elimination
    ├── simplify_cfg.rs — CFG Simplification (block merging, fallthrough)
    ├── tail_call.rs    — Tail Call Analysis
    ├── pattern_match.rs— Pattern Matching Optimization
    └── validation.rs   — IR Validation (SSA invariants, dominance)
```

## Type System

### The Lattice

All IR types form a lattice with `Any` at the top and `Bottom` at the bottom:

```
                         Any
                    /     |     \
              StableTuple  Actor  Tensor
              Message      Capability
              Tuple
              List         Map
              Binary       Fun
              Pid          Port
              Atom         Float
              Integer      Nil
                    \     |     /
                       Bottom
```

### Core Type Kinds

| Kind | Description |
|------|-------------|
| `Any` | Top of lattice — any possible term |
| `Bottom` | Unreachable code |
| `SmallInt` | Small integer (fixnum) |
| `NonNegInt` | Non-negative small integer |
| `Int64` | 64-bit integer |
| `Float` | 64-bit float |
| `Atom` | Interned atom |
| `Boolean` | true or false |
| `Nil` | Empty list |
| `Cons` | Non-empty list cell |
| `List` | nil or cons |
| `Tuple { arity }` | Tuple of known arity |
| `StableTuple { element_types, immutable }` | Fixed-layout, typed tuple |
| `Map` | Key-value map |
| `Binary` | Binary data |
| `Fun { arity }` | Function/closure |
| `Pid` | Process identifier |
| `Port` | Port identifier |
| `Message { payload, priority }` | Typed message pattern |
| `Actor { accepts, lifecycle }` | Typed actor reference |
| `Tensor { dtype, shape }` | Typed tensor for AI |
| `Capability { resource, owned, shareable }` | Native resource handle |
| `Union(A, B)` | Union of two types |
| `Constant(v)` | A specific constant value |

### Lattice Operations

```rust
// Join (least upper bound) — used when control flow merges
let joined = type_a.join(&type_b);

// Meet (greatest lower bound) — used for type refinement after tests
let refined = type_a.meet(&type_b);

// Subtype check
if type_a.contains(&type_b) { ... }
```

### Stable Tuples

`StableTuple` is a key optimization enabler. When the compiler knows a tuple's
exact shape and that it's immutable, it can:

1. Emit a compact native struct layout (no tagged pointers)
2. Skip runtime type checks on field access
3. Promote the tuple to the Stable Immutable Region (SIR)
4. Eliminate GC scanning of its fields

```rust
// A stable tuple: {integer, atom, float}
let ty = IRType::new(TypeKind::StableTuple {
    element_types: vec![
        IRType::new(TypeKind::SmallInt),
        IRType::new(TypeKind::Atom),
        IRType::new(TypeKind::Float),
    ],
    immutable: true,
});
```

### Type Descriptors

Every heap-allocated type has a `TypeDescriptor` that the GC and AOT backend use:

```rust
pub struct TypeDescriptor {
    pub alloc_size: u32,              // Total bytes including header
    pub pointer_map: u64,             // Bitmap of pointer fields
    pub immutable: bool,              // Compiler-proven immutability
    pub native_layout: Option<NativeLayout>,  // For SIR compaction
    pub promotable_to_stable: bool,   // Can enter SIR
}
```

The `pointer_map` is critical: it tells the GC exactly which words in an
object are heap pointers, eliminating conservative scanning.

## SSA Form

The IR is in **Static Single Assignment** form: each value is defined exactly
once. This enables powerful optimizations:

```
BEAM (stack/register):          SSA IR:
  move X0, Y0                   y0 = get_reg(Y(0))
  add X0, X1, X2                t0 = get_reg(X(1))
                                t1 = get_reg(X(2))
                                t2 = Add(t0, t1)
                                set_reg(X(0), t2)
```

### Value Identification

```rust
pub enum IRValue {
    Constant { value: ConstantValue, ty: IRType },
    InstResult { inst: InstId, result_index: u32, ty: IRType },
    Argument { index: u32, ty: IRType },
    Placeholder,  // Temporary during construction
}
```

### Instruction Structure

```rust
pub struct IRInst {
    pub kind: IRInstKind,
    pub result: Option<IRValueId>,  // What this instruction produces
    pub operands: Vec<IRValueId>,   // What this instruction consumes
    pub beam_offset: u32,           // Source BEAM instruction index
    pub side_effects: SideEffects,
}
```

### Side Effects Tracking

Every instruction carries a `SideEffects` bitfield:

```rust
pub struct SideEffects {
    pub allocates: bool,    // May allocate on the heap
    pub reads_heap: bool,   // May read from the heap
    pub writes_heap: bool,  // May write to the heap
    pub may_raise: bool,    // May raise an exception
    pub calls: bool,        // May call other functions
    pub may_yield: bool,    // May yield (consume reductions)
}
```

This is used by:
- **DCE**: instructions with no side effects and no used results are dead
- **Code motion**: side-effecting instructions can't be reordered
- **GC**: only instructions with `allocates` can trigger GC

## Control Flow

The IR uses **basic blocks** with explicit terminators:

```
Block 0 (entry):
  t0 = GetReg(X(0))
  t1 = ConstSmallInt(42)
  t2 = Eq(t0, t1)
  BrIf(t2, Block 1, Block 2)

Block 1:
  t3 = ConstAtom(:yes)
  Ret(t3)

Block 2:
  t4 = ConstAtom(:no)
  Ret(t4)
```

### Terminator Instructions

| Instruction | Description |
|-------------|-------------|
| `Br { target }` | Unconditional branch |
| `BrIf { cond, true_target, false_target }` | Conditional branch |
| `Switch { value, default, targets }` | Jump table dispatch |
| `Ret { value }` | Return from function |
| `TailCall { func, args }` | Tail call (stack reuse) |
| `Throw { reason }` | Throw exception |

## IR Builder

`IRBuilder` translates BEAM bytecode into SSA IR:

```rust
let mut builder = IRBuilder::new(module_id, func_name, arity);

// Emit constants
let val = builder.const_small_int(42);
let atom = builder.const_atom(atom_idx);

// Emit arithmetic
let sum = builder.emit_add(a, b);

// Emit control flow
let block1 = builder.create_block();
let block2 = builder.create_block();
builder.emit_br_if(cond, block1, block2);

// Emit return
builder.emit_ret(result);
```

### Register Mapping

The builder maintains mappings from BEAM registers (X, Y, F) to SSA values:

```rust
// Read X register
let x0 = builder.get_x_reg(0);

// Write X register
builder.set_x_reg(0, new_value);
```

## Optimization Passes

All passes are in `dala_ir/src/opt/`. The `optimize()` function runs them
iteratively until convergence (max 10 iterations).

### Pass Inventory

| Pass | File | What It Does |
|------|------|-------------|
| DCE | `dce.rs` | Removes unreachable blocks and unused instructions |
| Const Propagation | `const_prop.rs` | Replaces variables with known constants |
| Const Folding | `const_prop.rs` | Evaluates constant expressions at compile time |
| CSE | `cse.rs` | Eliminates redundant computations |
| CFG Simplification | `simplify_cfg.rs` | Merges blocks, removes fallthrough branches |
| Tail Call Analysis | `tail_call.rs` | Converts calls in tail position to `TailCall` |
| Pattern Matching | `pattern_match.rs` | Converts type-test chains to switch dispatch |
| Validation | `validation.rs` | Checks SSA invariants (debug builds) |

### Optimization Pipeline

```
Input IR
  ↓
┌─────────────────────────────────────────┐
│  Iterate until convergence (max 10x):   │
│    1. DCE                               │
│    2. Constant Propagation              │
│    3. Constant Folding                  │
│    4. CSE                               │
│    5. CFG Simplification                │
│    6. Tail Call Analysis                │
│    7. Pattern Matching Optimization     │
│                                         │
│  If any pass changed something, repeat. │
└─────────────────────────────────────────┘
  ↓
Optimized IR
```

### Running Individual Passes

For debugging and analysis, you can run a single pass:

```rust
use dala_ir::opt::run_pass;

let changed = run_pass(&mut func, "dce");
let changed = run_pass(&mut func, "pattern-match");
```

## New IR Instructions (Dala-Specific)

Beyond standard arithmetic and control flow, Dala's IR includes first-class
instructions for:

### Actor Operations

```rust
// Spawn a typed actor
IRInstKind::SpawnActor { module, args, qos }

// Send a typed message (fast-path)
IRInstKind::SendTyped { target, msg, type_tag, priority }

// Receive a typed message (fast-path)
IRInstKind::RecvTyped { type_tag, timeout }
```

### Stable Memory Operations

```rust
// Allocate in SIR
IRInstKind::AllocStable { type_desc, words }

// Promote existing object to SIR
IRInstKind::PromoteStable { object }
```

### Tensor Operations

```rust
// Create a tensor
IRInstKind::TensorNew { desc_idx, gpu }

// Tensor computation
IRInstKind::TensorOp { op: TensorOpKind, inputs }
// TensorOpKind: Add, Mul, MatMul, Relu, Softmax, Concat, Reshape, Transpose
```

### Capability Operations

```rust
// Create a native resource capability
IRInstKind::CapNew { resource_kind, owned }

// Release a capability
IRInstKind::CapRelease { cap }

// Transfer ownership
IRInstKind::CapTransfer { cap, new_owner }
```

### AI Runtime Operations

```rust
// Submit inference request
IRInstKind::InferenceSubmit { model_id, input, priority }

// Await inference result
IRInstKind::InferenceAwait { request }
```

### Arena Operations

```rust
// Arena allocation
IRInstKind::ArenaAlloc { arena, size, align }

// Bulk free
IRInstKind::ArenaReset { arena }
```

## Tracing & Debugging

### Enable Debug Output

```bash
RUST_LOG=dala_ir=trace cargo run --bin dala_aot -- compile --input test.beam
```

### Validate IR

In debug builds, the `validate_ir!()` macro automatically checks SSA invariants:

```rust
validate_ir!(func);  // Panics on any violation
```

### Inspect Optimized IR

```rust
// Print all blocks and instructions
for block in &func.blocks {
    println!("Block {:?}:", block.label);
    for inst in &block.instructions {
        println!("  {:?}", inst);
    }
}
```

## Developing New Passes

To add a new optimization pass:

1. Create `dala_ir/src/opt/my_pass.rs`
2. Implement a function with signature:
   ```rust
   pub fn my_optimization(func: &mut IRFunction) -> bool
   ```
   Return `true` if any changes were made.
3. Register in `opt/mod.rs`:
   ```rust
   pub mod my_pass;
   // In optimize():
   if my_pass::my_optimization(func) { changed = true; }
   // In run_pass():
   "my-pass" => my_pass::my_optimization(func),
   ```
4. Add validation after your pass (if it changes the IR structure):
   ```rust
   validate_ir!(func);
   ```

### Pass Development Tips

- **Always return `bool`** — the fixpoint loop needs to know if you changed anything
- **Run validation after** — catch SSA violations early
- **Test with small functions** — use `IRBuilder` to construct test cases
- **Check convergence** — your pass should be idempotent (running it twice should be a no-op)
