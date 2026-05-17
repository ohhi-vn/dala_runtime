# Dala Set-Theoretic Type System — Internals Guide

> Deep reference for how the type system works, why it works that way,
> and how to reason about it.

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [The Lattice](#2-the-lattice)
3. [Type Kinds in Detail](#3-type-kinds-in-detail)
4. [Lattice Operations](#4-lattice-operations)
5. [Subtyping as Set Containment](#5-subtyping-as-set-containment)
6. [Compound Types](#6-compound-types)
7. [Type Normalization](#7-type-normalization)
8. [Exhaustiveness Checking](#8-exhaustiveness-checking)
9. [Speculative Types](#9-speculative-types)
10. [Recursive Types](#10-recursive-types)
11. [Dynamic Types](#11-dynamic-types)
12. [Optimization Passes](#12-optimization-passes)
13. [Profiling & Tracing](#13-profiling--tracing)
14. [Edge Cases & Invariants](#14-edge-cases--invariants)
15. [Extending the Type System](#15-extending-the-type-system)

---

## 1. Architecture Overview

The Dala type system lives in `dala_ir/src/type_system.rs` and forms the
mathematical backbone of the AOT compiler. Every SSA value in the IR has a
type, and the compiler uses type information to:

- **Eliminate dead code**: If a branch's condition type is `Bottom`, the branch is unreachable
- **Specialize operations**: If a value's type is `SmallInt`, emit direct integer arithmetic
- **Optimize pattern matching**: Convert type-test chains to switch dispatch
- **Stack-allocate objects**: If a value's type doesn't escape, skip heap allocation
- **Generate native layouts**: If a stable tuple has known shape, emit a packed struct

### Core Data Structures

```
TypeKind          — The enum of all possible type constructors
IRType            — A wrapper around TypeKind (the "type" of a value)
TypeDescriptor    — Runtime metadata (GC layout, native layout, etc.)
ConstantValue     — Specific constant values (Int(42), Atom(1), etc.)
SpeculativeGuard  — Runtime checks for speculative optimization
```

### Module Layout

```
type_system.rs              — Core type definitions and lattice operations
type_system_tests.rs        — Comprehensive edge-case tests
type_system_profiling.rs    — Profiling, tracing, complexity analysis
opt/escape_analysis.rs      — Escape analysis pass
opt/native_specialize.rs    — Native layout specialization pass
opt/type_inference.rs       — Constraint-based type inference
opt/speculative.rs          — Speculative optimization pass
```

---

## 2. The Lattice

The type system forms a **bounded lattice** `(T, ⊆, ∪, ∩)` where:

- **T** is the set of all types
- **⊆** is the subtyping relation (set containment)
- **∪` is join (least upper bound)
- **∩` is meet (greatest lower bound)

### Lattice Laws

The implementation maintains these mathematical properties:

| Law | Join | Meet |
|-----|------|------|
| Commutativity | A ∪ B = B ∪ A | A ∩ B = B ∩ A |
| Associativity | (A ∪ B) ∪ C = A ∪ (B ∪ C) | (A ∩ B) ∩ C = A ∩ (B ∩ C) |
| Idempotency | A ∪ A = A | A ∩ A = A |
| Absorption | A ∪ (A ∩ B) = A | A ∩ (A ∪ B) = A |

### Lattice Extremes

```
Any (⊤)  — Top of the lattice. Contains all values. Every type is a subtype of Any.
Bottom (⊥) — Bottom of the lattice. Contains no values. Subtype of every type.
```

### Partial Order Diagram (simplified)

```
                    Any (⊤)
                   / | \
          SmallInt  Float  Atom  ...
          /    \
    NonNegInt  Int64
         \    /
        Bottom (⊥)
```

---

## 3. Type Kinds in Detail

### Immediate Types

| Type | Values | Size |
|------|--------|------|
| `SmallInt` | Fixnum integers | 1 word |
| `NonNegInt` | Non-negative fixnums | 1 word |
| `Int64` | Full 64-bit integers | 1-2 words |
| `Float` | 64-bit IEEE 754 | 1-2 words |
| `Atom` | Erlang atoms | 1 word |
| `Boolean` | `true` or `false` | 1 word |
| `Nil` | Empty list `[]` | 1 word |

**Subtyping**: `NonNegInt ⊆ SmallInt ⊆ Int64`

### Composite Types

| Type | Description |
|------|-------------|
| `Cons` | Non-empty list cell |
| `List` | `Nil \| Cons` |
| `Tuple { arity }` | Fixed-size tuple |
| `StableTuple { element_types, immutable }` | Typed, immutable tuple |
| `Map` | Arbitrary key-value map |
| `MapShape { keys, values }` | Struct-like map with known keys |
| `Binary` | Byte sequence |
| `Fun { arity }` | Function/closure |

### BEAM Identity Types

`Pid`, `Port`, `Reference` — Opaque handles with no subtyping between them.

### Dala-Specific Types

| Type | Purpose |
|------|---------|
| `Message { payload, priority }` | Typed mailbox messages |
| `Actor { accepts, lifecycle }` | Typed actor references |
| `Tensor { dtype, shape }` | AI tensor types |
| `Capability { resource, owned, shareable }` | Native resource handles |

### Set-Theoretic Connectives

| Type | Notation | Meaning |
|------|----------|---------|
| `Union(A, B)` | A ∪ B | Values in A OR B |
| `Intersection(A, B)` | A ∩ B | Values in BOTH A AND B |
| `Difference(A, B)` | A \ B | Values in A but NOT in B |
| `Constant(v)` | {v} | A single constant value |

### Special Types

| Type | Purpose |
|------|---------|
| `RecursiveVar { id, bound }` | Type inference variable |
| `Dynamic` | Gradual typing boundary |
| `Speculative { assumed, actual, guard }` | Guard-based specialization |

---

## 4. Lattice Operations

### Join (Least Upper Bound) — `A.join(B)`

The join produces the **smallest type that contains both A and B**. Used when
control flow merges (e.g., after an if expression).

**Key rules:**

| A | B | A.join(B) |
|---|---|-----------|
| Bottom | X | X |
| Any | X | Any |
| SmallInt | NonNegInt | SmallInt |
| SmallInt | Float | Any |
| Nil | Cons | List |
| Constant(a) | Constant(b) | widen to supertype |
| StableTuple{a} | StableTuple{b} (same arity) | StableTuple{join(a,b)} |
| Message{p1,pr1} | Message{p2,pr2} | Message{join(p1,p2), max(pr1,pr2)} |
| Union(A,B) | C | flatten and simplify |
| Speculative{actual} | X | join(actual, X) |

### Meet (Greatest Lower Bound) — `A.meet(B)`

The meet produces the **largest type contained in both A and B**. Used for
type refinement after tests.

**Key rules:**

| A | B | A.meet(B) |
|---|---|-----------|
| Bottom | X | Bottom |
| Any | X | X |
| SmallInt | NonNegInt | NonNegInt |
| SmallInt | Float | Bottom |
| Constant(a) | Constant(b) | if a==b then Constant(a) else Bottom |
| Constant(c) | General(g) | if c ∈ g then Constant(c) else Bottom |
| StableTuple{a} | StableTuple{b} (same arity) | StableTuple{meet(a,b)} |
| Message{p1,pr1} | Message{p2,pr2} | Message{meet(p1,p2), min(pr1,pr2)} |
| Union(A,B) | C | distribute: meet(A,C) ∪ meet(B,C) |
| Speculative{assumed} | X | meet(assumed, X) |

### Contains (Subtype Check) — `A.contains(B)`

Returns true if A is a supertype of B (every value of type B is also a value of type A).

**Key rules:**

| A | B | A.contains(B) |
|---|---|---------------|
| Any | _ | true |
| _ | Bottom | true |
| List | Nil | true |
| List | Cons | true |
| SmallInt | NonNegInt | true |
| Int64 | SmallInt | true |
| StableTuple{..} | Tuple{arity} | if arity matches |
| Union(A,B) | C | A.contains(C) OR B.contains(C) |
| Intersection(A,B) | C | A.contains(C) AND B.contains(C) |
| Difference(A,B) | C | A.contains(C) AND C∩B = ∅ |
| Map | MapShape{..} | true |
| Speculative{assumed} | X | assumed.contains(X) |

---

## 5. Subtyping as Set Containment

The fundamental insight: **subtyping is set containment**.

```
A ⊆ B  iff  ∀v. v ∈ A → v ∈ B
```

This means:
- `NonNegInt ⊆ SmallInt` because every non-negative integer is an integer
- `Constant(42) ⊆ SmallInt` because 42 is an integer
- `Nil ⊆ List` because nil is a valid list
- `StableTuple{[SmallInt, Atom]} ⊆ Tuple{2}` because every stable tuple of 2 elements is a tuple of 2 elements

### Covariance and Contravariance

- **Message payload**: Covariant — `msg<SmallInt> ⊆ msg<Number>` if `SmallInt ⊆ Number`
- **Message priority**: Contravariant — `msg<A, High> ⊆ msg<A, Normal>` (higher priority is more specific)
- **Actor accepts**: Contravariant — an actor accepting more message types is a supertype
- **MapShape values**: Covariant — wider value types in the container

---

## 6. Compound Types

### Union Types (A ∪ B)

A value that is either of type A or type B.

```
integer() | float()    — a number that could be either
{:ok, T} | {:error, E} — a result type
nil | cons()            — a list
```

**Optimization**: The compiler can generate multi-version code:
```
if value ∈ integer() → fast integer path
if value ∈ float()   → fast float path
else                 → generic fallback
```

### Intersection Types (A ∩ B)

A value that is both of type A and type B.

```
integer() & non_neg_integer()  → non_neg_integer()
number() & float()             → float()
```

**Optimization**: Intersections refine types. After a type test:
```
Before: x ∈ any()
After is_integer(x): x ∈ any() & integer() = integer()
```

### Difference Types (A \ B)

Values in A that are not in B.

```
atom() \ nil       — all atoms except nil
integer() \ non_neg_integer()  — negative integers only
```

**Optimization**: Difference types enable precise branch elimination.
If the compiler knows `x ∈ atom() \ nil`, it can skip the nil check.

### Nested Compounds

The type system handles arbitrarily nested compounds:

```
(integer() | float()) & number()     → integer() | float()
(integer() | atom()) \ integer()     → atom()
```

The `normalize()` function flattens and simplifies:
```
(A | B) | C  →  A | B | C     (flatten)
SmallInt | Int64  →  Int64     (absorb subtypes)
```

---

## 7. Type Normalization

Normalization converts a type to canonical form:

1. **Flatten** nested unions: `(A | B) | C → A | B | C`
2. **Absorb** subtypes: `SmallInt | Int64 → Int64`
3. **Sort** alternatives for structural equality
4. **Simplify** intersections with Any/Bottom

**Idempotency**: `normalize(normalize(x)) == normalize(x)`

**Key property**: Two semantically equal types have identical normalized forms,
enabling simple `==` comparison.

---

## 8. Exhaustiveness Checking

Because types are sets, the compiler can **mathematically prove** whether
all cases are covered:

```elixir
case x do
  :ok    -> ...
  :error -> ...
end

# Compiler checks: {:ok} | {:error} ⊆ type_of(x)
# If yes → exhaustive. If no → warning.
```

**Algorithm**:
1. Compute the union of all pattern types
2. Check if the scrutinee type is a subset of that union
3. If not, `uncovered_by()` returns the missing type for error messages

---

## 9. Speculative Types

`Speculative { assumed, actual, guard }` represents a type assumption:

- **assumed**: What the fast path assumes (e.g., `SmallInt`)
- **actual**: What the value could actually be (e.g., `Any`)
- **guard**: The runtime check that validates the assumption

**Lattice behavior**:
- `join()` uses the **actual** type (conservative — don't assume)
- `meet()` uses the **assumed** type (optimistic — narrow for fast path)
- `contains()` uses the **assumed** type

**Example**:
```
x has type Speculative { assumed: SmallInt, actual: Any, guard: IsSmallInt }

Fast path:  GUARD IsSmallInt(x) ELSE deopt
            y = x + 1    -- direct integer add, no type check
            JMP done
Deopt:      y = generic_add(x, 1)
done:
```

### SpeculativeGuard Kinds

| Guard | Cost | Description |
|-------|------|-------------|
| `Trivial` | 0 | No check needed |
| `IsImmediate` | 1 | Tag check (integer, atom, etc.) |
| `IsConstant` | 1 | Value comparison |
| `IsComposite` | 2 | Type tag + structure check |
| `StableTupleShape` | 2+n | Tag + n element type checks |
| `MapShapeKeys` | 2+n | Tag + n key checks |
| `IsInUnion` | n | n alternative checks |
| `TensorSpec` | 3+k | dtype + k shape dims |

---

## 10. Recursive Types

`RecursiveVar { id, bound }` represents a type variable:

- **id**: De Bruijn index for the binder
- **bound**: Optional upper bound constraint

**Lattice behavior**:
- `join()` widens to the bound (or `Any` if unbounded)
- `meet()` narrows to the bound (or `Bottom` if unbounded)
- `contains()` checks against the bound

**Use case**: Type inference for recursive functions:
```
fun((X) -> X)  where X is RecursiveVar { id: 0, bound: None }
```

---

## 11. Dynamic Types

`Dynamic` represents an explicitly opted-out type for gradual typing:

- Behaves like `Any` in the lattice (join → `Any`, meet → other type)
- But carries a **runtime check obligation** at boundaries
- Used for interoperability with untyped BEAM code

**Key difference from `Any`**:
- `Any` is the top of the lattice — no runtime cost
- `Dynamic` generates runtime type checks at the boundary

---

## 12. Optimization Passes

### Escape Analysis (`opt/escape_analysis.rs`)

Determines which allocations can be stack-allocated:

1. **Find allocations**: Walk IR for `Alloc`, `AllocStable`, composite creation
2. **Check escape**: For each allocation, check if it's stored, sent, returned, or passed to calls
3. **Convert**: Non-escaping allocations become stack allocations

**Impact**: Eliminates GC pressure, improves cache locality.

### Native Layout Specialization (`opt/native_specialize.rs`)

Converts types to compact native layouts:

- `StableTuple {[SmallInt, SmallInt]}` → `struct { i64, i64 }` (16 bytes, unboxed)
- `MapShape {[id: SmallInt, name: Binary]}` → hidden-class struct (no hash lookup)

**Impact**: Reduces memory, eliminates tags, improves cache locality.

### Type Inference (`opt/type_inference.rs`)

Propagates type information through the IR:

1. **Generate constraints**: From each instruction (e.g., `IsTuple(x)` → `x : Tuple`)
2. **Solve**: Fixed-point iteration computing meets/joins
3. **Annotate**: Write inferred types back to IR values

### Speculative Optimization (`opt/speculative.rs`)

Generates specialized fast-path code:

1. **Identify**: Find type tests with high benefit/cost ratio
2. **Split**: Create fast/slow path blocks
3. **Guard**: Insert type guard at split point
4. **Specialize**: Fast path uses assumed type (no runtime checks)

---

## 13. Profiling & Tracing

### TypeProfiler

Tracks performance of type operations:

```rust
let mut profiler = TypeProfiler::new();
profiler.record_join(duration);
profiler.record_meet(duration);
println!("{}", profiler);  // Prints avg, p95, p99
```

### TypeTracer

Records type transformations:

```rust
let mut tracer = TypeTracer::new();
tracer.record(&ty, "before optimization");
let optimized = optimize(ty);
tracer.record(&optimized, "after optimization");
println!("{}", tracer.format_trace());
```

### TypeComplexity

Analyzes type structure:

```rust
let info = TypeComplexity::analyze(&ty);
// info.depth — nesting depth
// info.width — number of union alternatives
// info.can_simplify — would normalization help?
```

### Benchmarks

```rust
let result = benchmark_join(&a, &b, 10_000);
println!("{}", result);  // min, max, avg, p50, p95, p99
```

---

## 14. Edge Cases & Invariants

### Lattice Invariants (tested in `type_system_tests.rs`)

1. **Commutativity**: `A.join(B) == B.join(A)`, `A.meet(B) == B.meet(A)`
2. **Associativity**: `(A.join(B)).join(C) == A.join(B.join(C))`
3. **Idempotency**: `A.join(A) == A`, `A.meet(A) == A`
4. **Absorption**: `A.join(A.meet(B)) == A`, `A.meet(A.join(B)) == A`
5. **Upper bound**: `A.join(B).contains(A)` and `A.join(B).contains(B)`
6. **Lower bound**: `A.contains(A.meet(B))` and `B.contains(A.meet(B))`

### Subtyping Invariants

1. **Reflexivity**: `A.contains(A)` — always true
2. **Transitivity**: if `A.contains(B)` and `B.contains(C)`, then `A.contains(C)`
3. **Antisymmetry**: if `A.contains(B)` and `B.contains(A)`, then `A == B`

### Edge Cases

| Case | Result | Why |
|------|--------|-----|
| `A \ A` | `Bottom` | Removing all values leaves nothing |
| `A \ Bottom` | `A` | Removing nothing changes nothing |
| `A \ Any` | `Bottom` | Everything is removed |
| `A & A` | `A` | Intersecting with self is identity |
| `A & Bottom` | `Bottom` | Nothing is in both |
| `A & Any` | `A` | Everything contains A |
| `A \| A` | `A` | Union with self is identity |
| `A \| Bottom` | `A` | Adding nothing changes nothing |
| `A \| Any` | `Any` | Everything is in the union |
| `const(a) & const(b)` (a≠b) | `Bottom` | Different constants are disjoint |
| `const(a) \| const(a)` | `const(a)` | Same constant is idempotent |
| `normalize(normalize(x))` | `normalize(x)` | Normalization is idempotent |

---

## 15. Extending the Type System

### Adding a New Type Kind

1. Add variant to `TypeKind` enum
2. Implement `Display` for the new variant
3. Add join rules in `IRType::join()`:
   - What happens when this type merges with other types at control flow join points?
4. Add meet rules in `IRType::meet()`:
   - What happens when this type is refined by a type test?
5. Add subtype rules in `IRType::contains()`:
   - Is this type a subtype of any existing type?
6. Add type predicates (e.g., `is_my_new_type()`) if needed by optimizations
7. Add to `contains_constant_kind()` if the type can contain constants
8. Add to `native_field_for_type()` if the type has a native layout
9. Add to `boxed_size()` and `native_size()` for memory estimation
10. Add tests in `type_system_tests.rs`

### Adding a New Optimization Pass

1. Create `dala_ir/src/opt/my_pass.rs`
2. Implement: `pub fn optimize(func: &mut IRFunction) -> bool`
3. Use type information from `value.ty()` and the lattice operations
4. Register in `opt/mod.rs`
5. Add tests

### Adding a New Type Test Instruction

1. Add variant to `IRInstKind` in `instruction.rs`
2. In `opt/pattern_match.rs`, add the type test to chain detection
3. In `opt/type_inference.rs`, add constraint generation
4. In codegen, emit the appropriate machine code (usually a tag check)
