# Set-Theoretic Type System — Design & Implementation Guide

> How set-theoretic types work in Dala, why they matter for AOT optimization,
> what is currently implemented, and how to extend the system.

---

## Table of Contents

1. [What Are Set-Theoretic Types?](#1-what-are-set-theoretic-types)
2. [Why They Matter for Dala AOT](#2-why-they-matter-for-dala-aot)
3. [Type Algebra The Formal Model](#3-type-algebra-the-formal-model)
4. [Current Implementation](#4-current-implementation)
5. [The Lattice Join Meet Contains](#5-the-lattice-join-meet-contains)
6. [Type Narrowing and Pattern Matching](#6-type-narrowing-and-pattern-matching)
7. [Optimization Passes That Use Types](#7-optimization-passes-that-use-types)
8. [Native Layout Specialization](#8-native-layout-specialization)
9. [GC Integration](#9-gc-integration)
10. [Speculative Optimization](#10-speculative-optimization)
11. [How to Extend the Type System](#11-how-to-extend-the-type-system)
12. [Implementation Roadmap](#12-implementation-roadmap)

---

## 1. What Are Set-Theoretic Types?

A set-theoretic type system treats **types as sets of values**. Instead of
thinking about types as labels (int, string), you think about them as
the actual set of possible values:

```
integer()  = { ..., -2, -1, 0, 1, 2, ... }
atom()     = { nil, true, false, ok, error, ... }
boolean()  = { true, false }
```

This enables **boolean algebra on types**:

| Operation | Syntax | Meaning | Example |
|-----------|--------|---------|---------|
| Union | `A | B` | Values in A OR B | `integer() | float()` |
| Intersection | `A & B` | Values in BOTH A AND B | `integer() & non_neg_integer()` |
| Negation | `not A` | All values NOT in A | `atom() \\ nil` |
| Difference | `A \\ B` | Values in A but not B | `number() \\ float()` |

### Comparison With Other Type Systems

| Property | Nominal (Java) | Structural (TS) | Set-Theoretic (Dala) |
|----------|---------------|-----------------|---------------------|
| Subtyping | Type hierarchy | Shape compatibility | **Set containment** |
| Union types | Limited | Yes | **Full algebra** |
| Intersection | No | Yes | **Full algebra** |
| Negation | No | No | **Yes** |
| Exhaustiveness | Partial | Partial | **Mathematical proof** |
| Type narrowing | Limited | Control flow | **Mathematical refinement** |

---

## 2. Why They Matter for Dala AOT

Traditional BEAM execution assumes **everything can be anything**. So the
runtime constantly does:

```
type checks -> tag checks -> dynamic dispatch -> boxing/unboxing -> guard evaluation
```

This is expensive. Set-theoretic types let the compiler **prove** that a value
belongs to a smaller set, enabling:

```
Better type knowledge
  -> fewer runtime guards
  -> fewer generic BEAM operations
  -> more native specialization
  -> direct machine code generation
```

This is **runtime information compression** — the type system compresses what
the runtime needs to know about each value into a compact, mathematically
precise representation.

### The Key Insight

> Set-theoretic types are NOT mainly about static correctness.
> For Dala AOT, they are mainly about **making BEAM optimizable**.

---

## 3. Type Algebra The Formal Model

### 3.1 Semantic Subtyping

Subtyping is defined as **set containment**:

```
A subseteq B  iff  for all v. v in A implies v in B
```

This means `integer() subseteq number()` because every integer is a number. The
compiler can prove this mathematically, not just by following declaration
rules.

### 3.2 Union Types

```elixir
# A value that is either an integer or an atom
integer() | atom()

# A result type that is either {:ok, value} or {:error, reason}
{:ok, any()} | {:error, any()}
```

In the IR, this is `TypeKind::Union(Box<IRType>, Box<IRType>)`.

**Optimization impact**: The compiler can generate multi-version code:

```
if value in integer() -> fast integer path
if value in atom()    -> fast atom path
else                  -> generic fallback
```

### 3.3 Intersection Types

```elixir
# A value that is both an integer AND non-negative
integer() & non_neg_integer()
# Equivalent to: non_neg_integer()
```

**Optimization impact**: Intersections refine types. After a type test,
the compiler intersects the current type with the test type:

```
Before: x in any()
After is_integer(x): x in any() & integer() = integer()
```

### 3.4 Negation / Difference Types

```elixir
# All atoms except nil
atom() \\ nil

# All values except tuples
not tuple()
```

**Optimization impact**: Negation types enable precise branch elimination.
If the compiler knows `x in atom() \\ nil`, it can skip the nil check.

### 3.5 Exhaustiveness Checking

Because types are sets, the compiler can **mathematically prove** whether
all cases are covered:

```elixir
case x do
  :ok    -> ...
  :error -> ...
end

# Compiler checks: {:ok} | {:error} is a subset of the scrutinee type
# If yes -> exhaustive. If no -> warning.
```

---

## 4. Current Implementation

The type system lives in `dala_ir/src/type_system.rs`. Here is what is
currently implemented:

### 4.1 Type Kind Enum

```rust
pub enum TypeKind {
    Any,                              // Top of lattice — any value
    Bottom,                           // Unreachable code

    // Immediate types
    SmallInt, NonNegInt, Int64,       // Integers
    Float,                            // 64-bit float
    Atom, Boolean, Nil,               // Atoms, booleans, nil

    // Composite types
    Cons, List,                       // Linked lists
    Tuple { arity: u32 },             // Fixed-arity tuple
    StableTuple {                     // Typed, immutable tuple
        element_types: Vec<IRType>,
        immutable: bool,
    },
    Map, Binary,                      // Maps and binaries
    Fun { arity: u32 },               // Functions/closures
    Pid, Port, Reference,             // BEAM identity types

    // Dala-specific types
    Message { payload: Box<IRType>, priority: MessagePriority },
    Actor { accepts: Vec<IRType>, lifecycle: ActorLifecycle },
    Tensor { dtype: TensorDtype, shape: Vec<Option<u64>> },
    Capability { resource: NativeResourceKind, owned: bool, shareable: bool },

    // Set-theoretic connectives
    Union(Box<IRType>, Box<IRType>),  // A | B
    Constant(ConstantValue),          // A specific constant value
}
```

### 4.2 Feature Status

| Feature | Status | Location |
|---------|--------|----------|
| Union types (A | B) | Implemented | `type_system.rs` |
| Constant types (Constant(v)) | Implemented | `type_system.rs` |
| Join (least upper bound) | Implemented | `type_system.rs` |
| Meet (greatest lower bound) | Implemented | `type_system.rs` |
| Subtype check (contains) | Implemented | `type_system.rs` |
| Type narrowing in pattern matching | Implemented | `opt/pattern_match.rs` |
| Stable tuple shape tracking | Implemented | `type_system.rs` |
| TypeDescriptor with pointer_map | Implemented | `type_system.rs` |
| Native layout hints | Implemented | `type_system.rs` |
| Message types with priority | Implemented | `type_system.rs` |
| Actor types with lifecycle | Implemented | `type_system.rs` |
| Tensor types (shape + dtype) | Implemented | `type_system.rs` |
| Capability types (native resources) | Implemented | `type_system.rs` |
| Exhaustiveness checking | Partial | `opt/pattern_match.rs` |
| Full intersection types (A & B) | Partial | Via meet() |
| Negation types (not A) | Planned | — |
| Recursive types | Planned | — |
| Map shape types | Planned | — |
| Tallying constraint solver | Planned | — |
| Type inference from BEAM code | Planned | — |

### 4.3 Type Descriptors

Every heap-allocated type has a `TypeDescriptor` that the GC and AOT backend use:

```rust
pub struct TypeDescriptor {
    pub alloc_size: u32,              // Total bytes (header included)
    pub pointer_map: u64,             // Bitmap: which fields are GC pointers
    pub immutable: bool,              // Compiler-proven structural immutability
    pub native_layout: Option<NativeLayout>,  // For SIR compaction
    pub promotable_to_stable: bool,   // Can enter SIR
}
```

The `pointer_map` is critical — it tells the GC exactly which words in an
object are heap pointers, eliminating conservative scanning entirely.

---

## 5. The Lattice Join Meet Contains

The type system forms a **lattice** with three fundamental operations:

### 5.1 Join (Least Upper Bound) — A.join(B)

The join produces the **smallest type that contains both A and B**. Used
when control flow merges (e.g., after an if expression):

```rust
// Branch 1: x in integer()
// Branch 2: x in float()
// After merge: x in integer() | float()
let joined = IRType::new(TypeKind::SmallInt)
    .join(&IRType::new(TypeKind::Float));
// Result: Union(SmallInt, Float)
```

**Join rules (selected):**

| A | B | A.join(B) |
|---|---|-----------|
| Bottom | X | X |
| Any | X | Any |
| SmallInt | NonNegInt | SmallInt |
| Nil | Cons | List |
| StableTuple{a} | StableTuple{b} (same arity) | StableTuple{join(a,b)} |
| StableTuple{a} | StableTuple{b} (diff arity) | Tuple{max(a,b)} |
| Message{p1,pr1} | Message{p2,pr2} | Message{join(p1,p2), max(pr1,pr2)} |
| Union(A,B) | C | join(join(A,B), C) |
| Anything else | Anything else | Any |

### 5.2 Meet (Greatest Lower Bound) — A.meet(B)

The meet produces the **largest type contained in both A and B**. Used for
type refinement after tests:

```rust
// Before: x in any()
// After is_integer(x): x in any() & integer() = integer()
let refined = IRType::new(TypeKind::Any)
    .meet(&IRType::new(TypeKind::SmallInt));
// Result: SmallInt
```

**Meet rules (selected):**

| A | B | A.meet(B) |
|---|---|-----------|
| Bottom | X | Bottom |
| Any | X | X |
| Constant(c) | General (if c in General) | Constant(c) |
| Nil | List | Nil |
| Cons | List | Cons |
| NonNegInt | SmallInt | NonNegInt |
| Incompatible | Incompatible | Bottom |

### 5.3 Contains (Subtype Check) — A.contains(B)

Returns true if A is a subset of B (every value of type A is also a value of type B):

```rust
// SmallInt contains NonNegInt?
// No — SmallInt includes negative numbers
assert!(!small_int.contains(&non_neg_int));

// List contains Nil?
// Yes — nil is a valid list
assert!(list.contains(&nil));

// Any contains everything
assert!(any.contains(&any_type));
```

---

## 6. Type Narrowing and Pattern Matching

This is where set-theoretic types have the **biggest impact** on optimization.

### 6.1 Control Flow Narrowing

```elixir
# Before the test:
x in any()

if is_integer(x) do
  # After narrowing:
  x in any() & integer() = integer()

  # Compiler knows x is an integer — no runtime check needed
  x + 1  # Direct ADD instruction
end
```

In the IR, this is implemented by tracking the type of each SSA value and
refining it at branch points. The pattern matching optimization pass
(`opt/pattern_match.rs`) converts chains of type tests into efficient switch
dispatch.

### 6.2 Pattern Matching as Type Refinement

```elixir
case x do
  {:ok, value} ->
    # x in {:ok, any()}
    # Compiler knows: tuple of arity 2, first element is :ok
    # Can generate: CMP tag, TUPLE2; CMP elem0, :ok; BNE fail

  {:error, reason} ->
    # x in {:error, any()}
    # Compiler knows: tuple of arity 2, first element is :error
end
```

The pattern matching optimization pass converts type-test chains:

```
Before (generic):
  IsTuple(x) -> BrIf(tuple_block, fail)
  TupleSize(x, 2) -> BrIf(size_block, fail)
  TupleGet(x, 0) -> IsAtom(:ok) -> BrIf(ok_block, fail)

After (optimized):
  Switch type_tag(x):
    TUPLE2 -> Check first element atom tag -> Direct dispatch
    default -> fail
```

### 6.3 Receive Optimization

```elixir
receive do
  {:token, binary} ->
    # Message type: {:token, binary()}
    # Compiler can generate: dequeue_typed(mailbox, TAG_TOKEN)

  {:embedding, tensor} ->
    # Message type: {:embedding, tensor(f32, [batch, dim])}
    # Compiler can generate: dequeue_typed(mailbox, TAG_EMBEDDING)
end
```

The `Message` type in the IR models this:

```rust
TypeKind::Message {
    payload: Box::new(IRType::new(TypeKind::Tuple { arity: 2 })),
    priority: MessagePriority::Normal,
}
```

---

## 7. Optimization Passes That Use Types

### 7.1 Constant Propagation and Folding (opt/const_prop.rs)

Uses type information to determine when operands are constants:

```rust
// If type of x is Constant(Int(42)):
//   y = x + 1  ->  y = ConstSmallInt(43)
```

The type system enables this: when a value has type `Constant(v)`, the
compiler knows its exact value at compile time.

### 7.2 Dead Code Elimination (opt/dce.rs)

Uses type information to identify unreachable branches:

```rust
// If type of x is Constant(True):
//   if x then A else B  ->  A (B is dead)
```

### 7.3 Pattern Matching Optimization (opt/pattern_match.rs)

Converts type-test chains into switch dispatch, as described in section 6.

### 7.4 Common Subexpression Elimination (opt/cse.rs)

Uses type information to determine when two expressions are guaranteed to
produce the same result:

```rust
// If type of x is Constant(Int(5)):
//   y = x + 1  ->  6
//   z = x + 1  ->  6  (CSE can reuse the result)
```

### 7.5 CFG Simplification (opt/simplify_cfg.rs)

Uses type information to eliminate branches with known outcomes:

```rust
// If type of x is SmallInt (not a tuple):
//   if is_tuple(x) then A else B  ->  B (A is unreachable)
```

---

## 8. Native Layout Specialization

This is one of the **most important future opportunities**. With precise type
information, the compiler can generate compact native layouts instead of
generic boxed BEAM terms.

### 8.1 Stable Tuple to Native Struct

```
Without types:
  Generic boxed tuple: [tag | ptr0 | ptr1 | ptr2]
  Each element is a tagged pointer (8 bytes)

With set-theoretic typing:
  {:point, integer(), integer()}
  Could become:
  struct Point {
      int64_t x;    // 8 bytes, unboxed
      int64_t y;    // 8 bytes, unboxed
  }
  Total: 16 bytes vs 32 bytes boxed
```

### 8.2 TypeDescriptor Native Layout

The `TypeDescriptor` carries an optional `NativeLayout` that describes the
compact representation:

```rust
pub struct NativeLayout {
    pub fields: Vec<NativeField>,
    pub size: u32,
}

pub struct NativeField {
    pub offset: u32,
    pub kind: NativeFieldKind,  // I64, F64, Ptr, Bytes { len }
}
```

When a `StableTuple` is promoted to the SIR, it can be converted to its
native layout — eliminating tags, reducing memory, and improving cache locality.

### 8.3 Map Shape Specialization

BEAM maps are expensive because of arbitrary dynamic keys. Set-theoretic types
allow shape-specialized maps:

```elixir
%{
  id: integer(),
  name: binary()
}
```

Could become a hidden-class style layout:

```rust
struct Map_IdName {
    int64_t id;
    Binary* name;  // Known offset, no hash lookup
}
```

This is similar to V8 hidden classes, PyPy map strategies, and JS engine
shape optimization.

### 8.4 Escape Analysis

Set-theoretic types dramatically improve escape analysis:

```elixir
# If compiler proves this tuple never escapes:
{x, y} = compute_values()
# Then: no heap allocation needed — stack allocate or register allocate
```

The type system tracks whether references escape through the `Capability`
type and the `owned`/`shareable` flags.

---

## 9. GC Integration

Precise types help the GC enormously.

### 9.1 Pointer Maps

The `TypeDescriptor.pointer_map` is a bitmap where bit N is set if word N of
the object payload is a GC-traced pointer. This eliminates conservative
scanning entirely.

```
Object: [header | ptr0 | int | ptr1 | float]
Pointer map: 0b010101  (bits 0, 2, 4 are pointers)
```

### 9.2 SIR Promotion

The type system drives SIR promotion decisions:

1. Object survives N GC cycles -> increment `survival_count`
2. `survival_count >= THRESHOLD` AND `immutable == true` -> SIR candidate
3. SIR admission: walk subgraph, confirm no mutable children
4. Once in SIR: color = `stable-black` permanently, never rescanned

### 9.3 Stack Map Reduction

With precise type information, stack maps are smaller and faster to scan:

```
Without types:  "scan all slots conservatively"
With types:     "scan only slots where pointer_map bit is set"
```

This can reduce GC pause times by 60% or more for stable workloads.

---

## 10. Speculative Optimization

Types become **speculative guards** — the compiler generates specialized code
that assumes a type, with a fallback path if the assumption is wrong.

### 10.1 Speculative Arithmetic

```rust
// Compiler sees: x has type integer()
// Generates:
//   ADD x0, x1      // Fast path: direct integer add
//   BVS overflow    // Branch on overflow
//   JMP fallback    // Deoptimize to generic path
```

This is exactly what modern JITs (V8, JVM, LuaJIT) do. Set-theoretic types
make this possible in an AOT context.

### 10.2 Speculative Pattern Matching

```rust
// Compiler sees: x has type {:ok, integer()} | {:error, atom()}
// Generates:
//   CMP tag, TUPLE2    // Is it a tuple?
//   BNE fallback      // If not, generic path
//   CMP elem0, :ok    // Is first element :ok?
//   BEQ ok_path       // Direct dispatch
//   CMP elem0, :error // Is first element :error?
//   BEQ error_path    // Direct dispatch
// fallback:
//   ... generic pattern matching ...
```

### 10.3 Deoptimization

Even with types, BEAM remains dynamic. The compiler must generate fallback
paths:

```
Specialized path (fast)
    |
    | Type guard fails
    v
Generic path (slow but correct)
```

This is the same pattern used by V8, JVM, and other adaptive runtimes.

---

## 11. How to Extend the Type System

### 11.1 Adding a New Type Kind

1. Add variant to `TypeKind` in `type_system.rs`:

```rust
pub enum TypeKind {
    // ... existing variants ...
    MyNewType {
        field: u32,
    },
}
```

2. Implement `Display` for the new variant (in the `impl fmt::Display` block)
3. Add join rules in `IRType::join()` — what happens when this type merges
   with other types at control flow join points?
4. Add meet rules in `IRType::meet()` — what happens when this type is
   refined by a type test?
5. Add subtype rules in `IRType::contains()` — is this type a subtype of
   any existing type?
6. Add type predicates (e.g., `is_my_new_type()`) if needed by optimizations
7. Add tests in the `mod tests` block

### 11.2 Adding a New Type Test Instruction

1. Add variant to `IRInstKind` in `instruction.rs`:

```rust
IRInstKind::IsMyNewType { value: IRValueId },
```

2. In `opt/pattern_match.rs`, add the type test to the chain detection:

```rust
IRInstKind::IsMyNewType { value } if *value == test_value => {
    MY_NEW_TYPE_TAG
}
```

3. In codegen, emit the appropriate machine code (usually a tag check)

### 11.3 Adding a New Optimization Pass

1. Create `dala_ir/src/opt/my_pass.rs`
2. Implement: `pub fn optimize(func: &mut IRFunction) -> bool`
3. Use type information from `value.ty()` to make optimization decisions
4. Register in `opt/mod.rs`
5. Add tests

### 11.4 Adding Native Layout Support

1. Define the `NativeLayout` for your type in `TypeDescriptor`
2. In codegen, check `type_desc.native_layout` when emitting code
3. In the SIR promotion path, convert from tagged to native layout

---

## 12. Implementation Roadmap

### Phase 1 — Foundation (Done)
- [x] Union types (`A | B`)
- [x] Constant types (`Constant(v)`)
- [x] Join and meet operations
- [x] Subtype checking (`contains`)
- [x] Type narrowing in pattern matching
- [x] Stable tuple shape tracking
- [x] TypeDescriptor with pointer maps
- [x] Message, Actor, Tensor, Capability types

### Phase 2 — Optimization (In Progress)
- [x] Pattern matching type-test chain -> switch conversion
- [x] Stable tuple fast-path access
- [ ] Full intersection type support (currently via meet)
- [ ] Negation type support
- [ ] Exhaustiveness checking for case expressions
- [ ] Speculative arithmetic specialization
- [ ] Speculative pattern matching dispatch

### Phase 3 — Native Layouts (Planned)
- [ ] Stable tuple -> native struct conversion
- [ ] Map shape specialization (hidden classes)
- [ ] Escape analysis for stack allocation
- [ ] Unboxed integer/float in tuples
- [ ] SIR native layout compaction

### Phase 4 — Advanced Inference (Planned)
- [ ] Tallying constraint solver for type inference
- [ ] Recursive type support
- [ ] Gradual typing integration (dynamic() type)
- [ ] Type inference from BEAM bytecode
- [ ] Speculative trace compilation
- [ ] Adaptive deoptimization

### Phase 5 — AI Integration (Planned)
- [ ] Tensor type propagation through SSA
- [ ] Shape inference for tensor operations
- [ ] Typed actor message protocols
- [ ] Specialized receive for known message types
- [ ] Model metadata SIR promotion
