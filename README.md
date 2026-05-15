# Dala Compiler Runtime — Actor-Native Mobile Runtime

A Rust-based actor-native mobile runtime with integrated AI orchestration and typed native execution. Inspired by BEAM semantics but designed from the ground up for mobile constraints: startup time, battery, thermal, memory locality, offline AI, UI synchronization, and native platform integration.

## Architecture

```
Elixir/Erlang
    ↓
BEAM SSA
    ↓
Dala Typed IR  ←─── Stable shapes, message types, tensor types, capabilities
    ↓
Optimization Passes ←─── Pattern matching, mailbox specialization, SIR promotion
    ↓
Backend Lowering
    ├── Interpreter (baseline)
    ├── Cranelift (JIT, mobile-friendly)
    └── LLVM (future AOT)
```

## Key Architectural Decisions

### 1. Dala Typed IR (Stable SSA)

The IR is the central hub. Every optimization, backend, and runtime feature flows through it:

- **Stable tuple shapes**: Fixed-layout tuples with known element types → compact native representation
- **Message types**: Expected message shapes for mailbox fast-path matching
- **Actor types**: Protocol-aware actor references with lifecycle metadata
- **Tensor types**: Shape + dtype for zero-copy AI interop
- **Capability types**: Typed native resource handles (GPU, files, sockets)

### 2. Semantic Layer vs Execution Backend

Clean separation between:

**Semantic Layer** (actors, mailboxes, supervisors, reductions, pattern matching, fault handling)
**Execution Backend** (SSA lowering, register allocation, vectorization, machine code)

This gives huge long-term flexibility — swap the execution backend without changing actor semantics.

### 3. Specialized Mailbox System

Four priority queues (Critical, High, Normal, Low) with:
- Type-tag indexed fast-path for `receive`
- Stable message layouts for zero-copy delivery
- Back-pressure when queues are full

### 4. Multiple Memory Regions

Instead of a single BEAM-style heap:

| Region | Purpose |
|--------|---------|
| Actor Heap | Short-lived BEAM terms, GC'd |
| Stable Immutable Region (SIR) | Long-lived, never rescanned (UI trees, configs, schemas) |
| Binary Region | Large binaries, refcounted |
| Tensor Region | GPU/NN buffers, zero-copy |
| Native Resource Region | Capability-tracked handles |
| Arena Allocators | Frame-scoped, bulk-free |

### 5. QoS-Aware Scheduler

Designed for mobile AI workloads:
- **Thermal-aware**: Reduces inference priority when device is hot
- **Battery-aware**: Deprioritizes background work when battery is low
- **QoS classes**: Realtime, UserFacing, Utility, Background
- **Inference-priority actors**: Deadline-aware scheduling for AI workers

### 6. AI Runtime Layer

First-class runtime support for AI:
- **Inference Workers**: Dedicated workers with thermal throttling
- **Tensor Resources**: Managed GPU/ANE buffers with zero-copy interop
- **Streaming Pipelines**: Actor-driven real-time inference
- **Model Lifecycle**: Load, cache, version, unload with proper cleanup

### 7. Capability-Based Native Resources

Instead of arbitrary native handles:
- Actor-owned capabilities with reference tracking
- Supervised native resources
- Transferable ownership
- Automatic cleanup on actor termination

### 8. Pattern Matching Optimization

Typed pattern matching + AOT:
- Tagged dispatch for known message shapes
- Specialized mailbox matching
- Stable tuple destructuring without runtime type checks
- Branch merging for identical pattern arms

## Crates

| Crate | Description |
|-------|-------------|
| `dala_runtime` | Core runtime: actors, scheduler, GC, mailboxes, memory regions, AI layer |
| `dala_ir` | Typed SSA IR with optimization passes |
| `dala_beam_loader` | Parses .beam files into IR |
| `dala_codegen` | Cranelift-based native code generation (JIT + AOT) |
| `dala_dispatch` | Module dispatch, hot code loading, export tables |
| `dala_aot` | CLI tool orchestrating the full pipeline |

## Building

```bash
# Build all crates
cargo build --release

# Build just the runtime
cd dala_runtime && cargo build --release

# Build with JIT support (default)
cargo build --features jit

# Build for AOT-only (iOS, no JIT)
cargo build --features aot
```

## License

This project is licensed under the Mozilla Public License, Version 2.0 (MPL-2.0).
See the LICENSE file for details.
