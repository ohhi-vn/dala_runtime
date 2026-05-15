# Architecture Guide

This guide explains the architecture of the **Dala Compiler Runtime** — an
actor-native mobile runtime with integrated AI orchestration and typed native
execution. It is philosophically BEAM-derived but designed from the ground up
for mobile constraints: startup time, battery, thermal, memory locality,
offline AI, UI synchronization, and native platform integration.

> **Key insight:** Dala is NOT "compile Erlang to machine code." It is a
> BEAM-compatible runtime backend with native execution. Actor semantics,
> supervision, and process isolation remain intact — only the execution
> engine changes.

---

## Table of Contents

1. [High-Level Pipeline](#high-level-pipeline)
2. [Crate Architecture](#crate-architecture)
3. [Semantic Layer vs Execution Backend](#semantic-layer-vs-execution-backend)
4. [Dala Typed IR](#dala-typed-ir)
5. [Memory Architecture](#memory-architecture)
6. [Mailbox System](#mailbox-system)
7. [Scheduler](#scheduler)
8. [Garbage Collector](#garbage-collector)
9. [AI Runtime Layer](#ai-runtime-layer)
10. [Capability-Based Native Resources](#capability-based-native-resources)
11. [Pattern Matching Optimization](#pattern-matching-optimization)
12. [Hot Code Loading](#hot-code-loading)
13. [Execution Modes](#execution-modes)
14. [Design Decisions](#design-decisions)

---

## High-Level Pipeline

```
Elixir/Erlang Source
        ↓
   Erlang Compiler (OTP)
        ↓
   .beam Files (Standard BEAM bytecode)
        ↓
┌──────────────────────────────────────────────────────────────────┐
│  dala_beam_loader  — Parse .beam binary format                  │
│  dala_ir           — Build Typed SSA IR from bytecode           │
│  dala_ir::opt      — Optimize (DCE, CSE, const-prop,            │
│                       pattern-match, tail-call, SIR promotion)  │
│  dala_codegen      — Generate native code (Cranelift/LLVM)      │
│  dala_dispatch     — Register, dispatch, hot code loading       │
│  dala_runtime      — Execute (scheduler + GC + processes + AI)  │
└──────────────────────────────────────────────────────────────────┘
        ↓
   ARM64 / x86_64 Native Machine Code
        ↓
   Dala Runtime (QoS scheduler + hybrid GC + actor model + AI)
```

---

## Crate Architecture

### Crate Dependency Graph

```
dala_aot (CLI tool)
├── dala_runtime
│   ├── dala_ir (typed types, message priority, actor lifecycle)
│   └── (system deps: parking_lot, crossbeam, dashmap, smallvec)
├── dala_ir
│   └── (no internal dependencies)
├── dala_codegen
│   ├── dala_ir
│   └── dala_runtime
├── dala_dispatch
│   ├── dala_runtime
│   ├── dala_ir
│   └── dala_codegen
└── dala_beam_loader
    └── (no internal dependencies)
```

### Crate Responsibilities

| Crate | Purpose | Key Types |
|-------|---------|-----------|
| `dala_runtime` | Core runtime: actors, QoS scheduler, hybrid GC, mailboxes, memory regions, AI layer | `Process`, `Scheduler`, `Mailbox`, `Tensor`, `InferenceWorker`, `Governor`, `Arena`, `StableImmutableRegion` |
| `dala_ir` | Typed SSA IR and optimization passes | `IRModule`, `IRFunction`, `IRInst`, `IRType`, `IRBuilder`, `TypeDescriptor` |
| `dala_beam_loader` | Parses `.beam` binary files into structured data | `BeamModule`, `BeamFunction`, `BeamReader` |
| `dala_codegen` | Translates IR to native code via Cranelift | `CodeGenerator`, `CompiledFunction`, `RuntimeGlue` |
| `dala_dispatch` | Module registration, function lookup, hot code loading | `DispatchManager`, `ExportTable`, `HotCodeManager` |
| `dala_aot` | CLI tool orchestrating the full pipeline | `Cli`, `Commands` |

---

## Semantic Layer vs Execution Backend

A core architectural principle is the clean separation between **what** the
runtime means (semantics) and **how** it executes (backend):

### Semantic Layer
Responsible for:
- **Actors**: spawn, link, monitor, exit signals
- **Mailboxes**: priority routing, typed message matching, back-pressure
- **Supervisors**: restart strategies, child specs
- **Reductions**: counting, yielding, preemption
- **Process semantics**: isolation, fault tolerance, scheduling QoS
- **Pattern matching**: receive blocks, case expressions
- **Fault handling**: catch/throw, exit propagation

### Execution Backend
Responsible for:
- **SSA lowering**: BEAM opcodes → typed IR
- **Register allocation**: virtual → physical registers
- **Vectorization**: SIMD for tensor/arithmetic operations
- **Machine code**: Cranelift/LLVM code generation
- **Memory layout**: object headers, pointer maps, native layouts

This separation means you can swap the execution backend (e.g., from
Cranelift to LLVM) without changing any actor semantics.

---

## Dala Typed IR

The IR is the central hub of the compiler. Every optimization, backend, and
runtime feature flows through it.

### Type Lattice

```
                    Any (top)
                 /     |     \
          StableTuple  |    Actor
          Message      |    Tensor
          Tuple        |    Capability
          List         |
          Map          |
          Binary       |
          Fun          |
          Pid          |
          Atom         |
          Integer      |
          Float        |
          Nil          |
                 \     |     /
                 Bottom (unreachable)
```

### Core Type Kinds

| Type | Description | Optimization Impact |
|------|-------------|-------------------|
| `StableTuple { elements, immutable }` | Fixed-layout tuple with known element types | Native struct layout, no runtime checks |
| `Message { payload, priority }` | Typed message pattern for mailbox | Fast-path dequeue, priority routing |
| `Actor { accepts, lifecycle }` | Protocol-aware actor reference | Compile-time send verification |
| `Tensor { dtype, shape }` | Typed tensor for AI workloads | Zero-copy GPU interop, shape checking |
| `Capability { resource, owned, shareable }` | Typed native resource handle | Automatic cleanup, ownership tracking |
| `SmallInt`, `Atom`, `Float`, ... | Standard BEAM types | Constant folding, specialization |

### Type Descriptors

Every heap-allocated type has a `TypeDescriptor` emitted by the compiler:

```rust
pub struct TypeDescriptor {
    pub alloc_size: u32,        // Total bytes (header included)
    pub pointer_map: u64,       // Bitmap: which fields are GC pointers
    pub immutable: bool,        // Compiler-proven structural immutability
    pub native_layout: Option<NativeLayout>,  // For SIR compaction
    pub promotable_to_stable: bool,           // Can enter SIR
}
```

The `pointer_map` eliminates conservative scanning — the GC knows exactly
which fields are pointers.

### New IR Instructions

Beyond standard arithmetic and control flow, the IR includes first-class
instructions for:

- **Actor operations**: `SpawnActor`, `SendTyped`, `RecvTyped`
- **Stable memory**: `AllocStable`, `PromoteStable`
- **Tensor operations**: `TensorNew`, `TensorOp` (Add, Mul, MatMul, Relu, Softmax, ...)
- **Capability operations**: `CapNew`, `CapRelease`, `CapTransfer`
- **AI operations**: `InferenceSubmit`, `InferenceAwait`
- **Arena operations**: `ArenaAlloc`, `ArenaReset`

---

## Memory Architecture

Dala uses a **hybrid managed memory model** — instead of a single BEAM-style
heap, it uses multiple regions optimized for different allocation patterns:

```
┌──────────────────────────────────────────────────────────┐
│  Actor Heap (Tier 1)                                     │
│  Short-lived BEAM terms, semi-space copying GC           │
│  Bump-pointer allocation, O(1)                           │
├──────────────────────────────────────────────────────────┤
│  Stable Immutable Region / SIR (Tier 2)                  │
│  Long-lived, structurally immutable objects              │
│  Never rescanned by GC after promotion                   │
│  UI trees, configs, schemas, static AI metadata          │
├──────────────────────────────────────────────────────────┤
│  Binary Region (Tier 3)                                  │
│  Large binaries, reference counted                       │
│  Shared across actors via refcount                       │
├──────────────────────────────────────────────────────────┤
│  Tensor Region (Tier 4)                                  │
│  GPU/NN buffers, 64-byte aligned                         │
│  Zero-copy interop with native ML frameworks             │
├──────────────────────────────────────────────────────────┤
│  Native Resource Region (Tier 5)                         │
│  Capability-tracked handles (files, sockets, GPU, etc.)  │
│  Actor-owned, automatic cleanup on termination           │
├──────────────────────────────────────────────────────────┤
│  Arena Allocators (Tier 6)                               │
│  Frame-scoped, bulk-free in O(1)                         │
│  Per-message handler, per-inference-request              │
└──────────────────────────────────────────────────────────┘
```

### Stable Immutable Region (SIR)

The SIR is one of Dala's most important optimizations. Objects enter the SIR
when they:
1. Survive N GC cycles (promotion threshold)
2. Are compiler-proven immutable (no mutable pointers)
3. Have no references to young-heap objects

Once in SIR:
- Color = `stable-black` permanently
- GC skips deep traversal (only checks root reference table)
- Optional: compact to native layout (flat, cache-friendly struct)
- Eviction: aging heuristic — demote if unreferenced for M cycles

### Arena Allocators

Arenas provide O(1) allocation and O(1) bulk deallocation:

```rust
let arena = Arena::new(64 * 1024);  // 64 KB chunks
let ptr = arena.alloc(256);         // Bump pointer
let ptr2 = arena.alloc_aligned(64, 64);  // Aligned allocation
arena.reset();                       // Everything freed, O(1)
```

Arenas are used for:
- Per-message handler temporary allocations
- Per-inference-request tensor scratch space
- Any frame-scoped work that can be bulk-freed

---

## Mailbox System

Each actor has a **typed, priority-aware mailbox** with four priority queues:

```
┌──────────────────────────────────────────────────────────┐
│  Mailbox                                                 │
├──────────────────────────────────────────────────────────┤
│  [Critical]  ──→  Supervision, fault recovery            │
│  [High]      ──→  UI events, control signals             │
│  [Normal]    ──→  Standard actor messages                │
│  [Low]       ──→  Telemetry, background work             │
├──────────────────────────────────────────────────────────┤
│  Type Index: type_tag → queue mapping for O(1) lookup   │
│  Overflow: back-pressure buffer when queues are full     │
└──────────────────────────────────────────────────────────┘
```

### Message Envelope

```rust
pub struct MessageEnvelope {
    pub payload: Term,              // The actual message
    pub priority: MessagePriority,  // Critical/High/Normal/Low
    pub sender: u64,                // Sender PID for reply routing
    pub type_tag: Option<u32>,      // Fast-path type matching
}
```

### Fast-Path Receive

When the compiler knows the expected message type, `receive` lowers to:

```rust
// Instead of scanning all messages:
msg = dequeue_typed(mailbox, TAG_TOKEN_OR_EMBEDDING);
switch element(1, msg):
    case atom(:token):     jump handle_token
    case atom(:embedding): jump handle_embedding
```

This avoids the generic pattern matching overhead entirely.

---

## Scheduler

The Dala scheduler is designed for **mobile AI workloads**, not telecom
fairness. It features:

### QoS Classes

| Class | Description | Reduction Budget |
|-------|-------------|-----------------|
| `Realtime` | Voice, video, sensor fusion | 500 reductions |
| `UserFacing` | UI, user interactions | 2000 reductions |
| `Utility` | Data processing, caching | 1000 reductions |
| `Background` | Analytics, model updates | 500 reductions |

### Thermal Governor

The scheduler integrates a thermal/battery governor:

```
Thermal State:  Nominal → Fair → Serious → Critical
                100%   → 80%  → 50%    → 25%  (reduction budget scaling)

Battery:        >20% normal, <20% deprioritize background,
                <10% only UserFacing and above
```

When thermal throttling is active:
- Background and Utility work is deprioritized
- Only Realtime and UserFacing actors get full budgets
- Inference workers reject non-realtime requests

### Per-QoS Run Queues

```rust
struct GlobalState {
    qos_queues: [QosQueue; 4],  // One per QoS class
    governor: Governor,          // Thermal/battery state
    // ...
}
```

The scheduler picks from the highest-priority non-empty queue first,
respecting the governor's thermal/battery limits.

---

## Garbage Collector

Dala uses a **hybrid generational copying collector** with SIR integration:

### Young Heap (Tier 1)
- **Algorithm**: Semi-space copying (Cheney)
- **Trigger**: Heap exhaustion or yield point
- **Pause target**: < 500 µs per process
- **Roots**: Stack (via stack maps), registers, mailbox

### Old Heap (Tier 2)
- **Algorithm**: Concurrent tri-color mark + incremental sweep
- **Trigger**: Promotion from young heap
- **Pause target**: < 2 ms (incremental slices)
- **Write barrier**: Young→Old references only

### SIR Integration
- Stable-black objects are never rescanned
- SIR roots tracked via lightweight reference table
- GC traversal skip reduces work by ≥60% for stable workloads

### Stack Maps

Generated by the codegen layer, stack maps tell the GC exactly which stack
slots and registers hold heap pointers at each safepoint:

```rust
pub struct StackMap {
    pub instruction_offset: u32,
    pub num_entries: u32,
    pub entries: [StackMapEntry],
}

pub struct StackMapEntry {
    pub offset: u32,
    pub is_pointer: bool,
    pub value_type: StackMapType,  // TuplePointer, ListPointer, etc.
}
```

---

## AI Runtime Layer

Dala provides first-class runtime support for AI workloads, going far beyond
"call a native ML library":

### Architecture

```
Actor ──► InferenceRequest ──► InferenceWorker
                                      │
                                      ▼
                              ModelRegistry (LRU cache)
                                      │
                          ┌───────────┼───────────┐
                          ▼           ▼           ▼
                      Model v1    Model v2    Model v3
                          │
                          ▼
                    TensorBuffer ──► GPU/ANE
```

### Inference Workers

```rust
pub struct InferenceWorker {
    config: WorkerConfig,
    active_requests: usize,
    throttled: bool,  // Thermal throttling
}
```

Workers are scheduled with QoS awareness and can be throttled based on
thermal state. Realtime inference requests are never throttled.

### Tensor Resources

```rust
pub struct Tensor {
    desc: TensorDesc,       // Shape + dtype
    data: *mut u8,          // Backing buffer
    location: TensorLocation,  // Host / GPU / ANE
    refcount: u32,
}
```

Tensors support zero-copy views and automatic reference counting.

### Streaming Pipelines

Multi-stage inference pipelines with actor-style message passing between stages:

```rust
let stages = vec![
    PipelineStage::Preprocess { name: "resize".into() },
    PipelineStage::Inference { model_id: 1 },
    PipelineStage::Postprocess { name: "nms".into() },
];
let mut pipeline = Pipeline::new(stages, StreamConfig::default());
pipeline.start();
pipeline.push_input(tensor)?;
```

### Model Lifecycle

```rust
pub struct ModelRegistry {
    models: RwLock<HashMap<ModelId, ModelHandle>>,
    max_memory: usize,
}
```

Models are loaded, cached, versioned, and unloaded with proper GPU memory
cleanup.

---

## Capability-Based Native Resources

Instead of arbitrary native handles, Dala uses **capability-typed resources**:

```rust
pub struct NativeResourceRegion {
    resources: Mutex<HashMap<NativeResourceId, NativeResourceEntry>>,
}

pub struct NativeResourceEntry {
    pub kind: NativeResourceKind,  // GpuContext, MlModel, IoHandle, Socket, ...
    pub handle: *mut u8,
    pub owned: bool,       // Responsible for cleanup
    pub shareable: bool,   // Can be transferred to other actors
    pub owner: u64,        // Owning actor PID
}
```

### Resource Kinds

| Kind | Description |
|------|-------------|
| `GpuContext` | Metal/Vulkan/CUDA compute context |
| `MlModel` | Pre-compiled ML model (weights + graph) |
| `TensorBuffer` | GPU/NN buffer (tensor storage) |
| `IoHandle` | File descriptor / I/O resource |
| `Socket` | Network socket |
| `SharedMemory` | Shared memory region |
| `UiSurface` | Platform-specific UI surface |
| `MediaDevice` | Camera / microphone |

### Ownership Transfer

```rust
// Register a new resource
let id = region.register(NativeResourceKind::GpuContext, handle, true, true, actor_pid);

// Transfer ownership
region.transfer(id, new_owner_pid)?;

// Release (auto-cleanup if owned)
region.release(id)?;
```

This fits BEAM's philosophy beautifully: resources are owned by actors,
supervised, and cleaned up on termination.

---

## Pattern Matching Optimization

The pattern matching optimization pass transforms typed `receive` blocks and
`case` expressions into optimized dispatch sequences:

### Type-Test Chain → Switch

Before (generic):
```text
  if is_tuple(msg) && tuple_size(msg) == 2:
    if element(1, msg) == atom(:token):
      handle_token(element(2, msg))
```

After (optimized):
```text
  switch type_tag(msg):
    case TUPLE2:
      switch element(1, msg):
        case atom(:token):     jump handle_token
        case atom(:embedding): jump handle_embedding
```

### Stable Tuple Destructuring

For stable tuples with known shapes, field access requires no runtime type
checks — the compiler emits direct offset access.

### Mailbox Fast-Path

Typed `receive` with known message types lowers to `dequeue_typed(mailbox,
type_tag)` which uses the type index for O(1) matching.

---

## Hot Code Loading

Dala supports atomic module replacement (like OTP's code server):

```rust
pub struct HotCodeManager {
    modules: DashMap<u64, CompiledModule>,
}

pub struct LazyFnRef {
    code: RwLock<CodePtr>,  // Atomically swappable
    module: u64,
    function: u64,
    arity: u32,
}
```

The protocol:
1. Validate new module exports match old module
2. Register new module in `DashMap`
3. Atomically update `LazyFnRef` pointers via `RwLock`
4. Old code continues running; new calls use new code

Readers (executing code) never block; writers (code loading) get exclusive
access only during the brief pointer swap.

---

## Execution Modes

| Mode | Description | Use Case |
|------|-------------|----------|
| `Interpreted` | Pure bytecode interpretation | Baseline, debugging |
| `Mixed` (default) | AOT-compiled + interpreter fallback | General use |
| `Aot` | Only AOT-compiled code, no interpreter | iOS/App Store (no RWX pages) |
| `Jit` | JIT compilation at runtime | Maximum performance (desktop) |

---

## Design Decisions

### Why Cranelift?
- **Fast compilation** — critical for JIT
- **Rust-native** — no C++ dependency
- **Mobile-friendly** — works on iOS (unlike LLVM's JIT)
- **Simple embedding** — designed for JIT/embedded use

### Why SSA IR?
BEAM opcodes are stack/register-oriented and hard to optimize directly. SSA
enables: DCE, constant propagation, CSE, inlining, register allocation, loop
optimization, and pattern matching specialization.

### Why Multiple Memory Regions?
A single BEAM-style heap is limiting for:
- **AI workloads**: tensors need GPU-resident, cache-aligned memory
- **Large binaries**: refcounting is more efficient than copying
- **Stable data**: UI trees and configs should not be rescanned
- **Frame-scoped work**: arenas provide O(1) bulk-free

### Why QoS-Aware Scheduling?
BEAM's scheduler optimizes for fairness (telecom workloads). Mobile AI
workloads need:
- Thermal awareness (prevent throttling)
- Battery awareness (conserve power)
- Inference priority (deadline-aware AI)
- UI responsiveness (user-facing priority)

### Why Capability-Based Resources?
Traditional BEAM has no concept of native resource ownership. Dala's
capability model:
- Prevents resource leaks (automatic cleanup on actor termination)
- Enables safe GPU/ML resource sharing
- Fits BEAM's actor ownership philosophy
- Provides compile-time safety for native interop
