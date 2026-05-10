# Architecture Guide

This guide explains the high-level architecture of the Dala AOT compiler, the design decisions behind it, and how the crates fit together.

## Overview

Dala AOT is a Rust-based Ahead-of-Time compiler backend for the BEAM VM (Erlang/OTP). It compiles BEAM bytecode to native machine code (ARM64/x86_64) while preserving the BEAM runtime semantics — scheduler, GC, process model, and all.

**Key insight:** This is NOT "compile Erlang to machine code." It replaces only the *execution engine* of BEAM. OTP's runtime, scheduler, GC, and process model remain intact.

## High-Level Pipeline

```
Elixir/Erlang Source
        ↓
   Erlang Compiler (OTP)
        ↓
   .beam Files (Standard BEAM bytecode)
        ↓
┌──────────────────────────────────────────────────┐
│  dala_beam_loader  — Parse .beam binary format   │
│  dala_ir           — Build SSA IR from bytecode   │
│  dala_ir::opt      — Optimize the IR              │
│  dala_codegen      — Generate native code (Cranelift) │
│  dala_dispatch     — Register & dispatch functions │
│  dala_runtime      — Execute (scheduler + GC + processes) │
└──────────────────────────────────────────────────┘
        ↓
   ARM64 / x86_64 Native Machine Code
        ↓
   BEAM Runtime (scheduler + GC + process model)
```

## Crate Architecture

### Crate Dependency Graph

```
dala_aot (CLI tool)
├── dala_runtime
│   ├── dala_beam_loader (for loading .beam files)
│   └── (system deps: parking_lot, crossbeam, dashmap)
├── dala_ir
├── dala_codegen
│   ├── dala_ir
│   └── dala_runtime
├── dala_dispatch
│   ├── dala_runtime
│   ├── dala_ir
│   └── dala_codegen
└── dala_aot
    ├── dala_runtime
    ├── dala_ir
    ├── dala_beam_loader
    ├── dala_codegen
    └── dala_dispatch
```

### Crate Responsibilities

| Crate | Purpose | Key Types |
|-------|---------|-----------|
| `dala_runtime` | Core BEAM runtime: processes, scheduler, GC, term representation | `Process`, `Scheduler`, `Term`, `TrapFrame` |
| `dala_ir` | SSA Intermediate Representation and optimization passes | `IRModule`, `IRFunction`, `IRInst`, `IRBuilder` |
| `dala_beam_loader` | Parses `.beam` binary files into structured data | `BeamModule`, `BeamFunction`, `BeamReader` |
| `dala_codegen` | Translates IR to native code via Cranelift | `CodeGenerator`, `CompiledFunction`, `RuntimeGlue` |
| `dala_dispatch` | Module registration, function lookup, hot code loading | `DispatchManager`, `ExportTable`, `HotCodeManager` |
| `dala_aot` | CLI tool orchestrating the full pipeline | `Cli`, `Commands` |

---

## Detailed Component Design

### 1. Term Representation (`dala_runtime::term`)

BEAM uses a **tagged pointer** scheme for all values. Dala implements this with a 64-bit word:

```
┌──────────────────────────────────────────────────────────┐
│ 64-bit Term (u64)                                        │
├──────────┬───────────────────────────────────────────────┤
│ 2-bit    │ Primary tag:                                  │
│ primary  │   00 = Boxed (pointer to heap)                │
│ tag      │   01 = List (cons cell pointer)               │
│          │   10 = Header (tuple, float, fun, etc.)       │
│          │   11 = Immediate                              │
├──────────┴───────────────────────────────────────────────┤
│ For immediates (primary=11):                             │
│   4-bit immed1 tag:                                      │
│     0000 = Small integer (value >> 4)                    │
│     0001 = PID                                            │
│     0010 = Port                                           │
│     0011 = Immed2 (atoms, catches, regs, specials)       │
│       For atoms (immed2=000): 24-bit atom index          │
│       For X regs (immed2=010): 24-bit register index     │
│       For specials: nil, true, false                     │
└──────────────────────────────────────────────────────────┘
```

**Why this matters:** All term operations are branch-free bit manipulations, critical for performance. The `Term` type is a transparent `u64` — no heap allocation needed for small integers, atoms, PIDs, etc.

### 2. Process Model (`dala_runtime::process`)

Each BEAM process is a self-contained execution unit:

```
┌──────────────────────────────────────────────────┐
│ Process                                          │
├──────────────────────────────────────────────────┤
│ pid: u64              — Unique process ID        │
│ heap_start/ptr/top    — Private heap (grows)     │
│ stack_ptr/top         — Private stack            │
│ heap_high_water       — GC high water mark       │
│ registers: [256]      — X0-X255, Y0-Y1023, F0-F255 │
│ reductions/max_red    — Preemption counter       │
│ mailbox: Mutex        — Message queue            │
│ catches: SmallVec     — Exception handlers       │
│ flags: bitflags       — TRAP_EXIT, TRACING, etc. │
│ status: ProcessStatus — Running/Runnable/Waiting │
└──────────────────────────────────────────────────┘
```

**Key design decisions:**
- Each process has its own heap — no shared mutable state between processes
- Heap grows by doubling (`grow_heap()`)
- `ProcessBuilder` pattern for configurable process creation
- `unsafe impl Send + Sync` because shared state is behind `Mutex`/`DashMap`

### 3. Scheduler (`dala_runtime::scheduler`)

The scheduler implements BEAM's **reduction-counting preemptive scheduling**:

```
┌──────────────────────────────────────────────────────────┐
│ GlobalState                                             │
├──────────────────────────────────────────────────────────┤
│ run_queues: [ParkingMutex<Vec<usize>>] — Per-scheduler  │
│ processes: DashMap<u64, Arc<Mutex<Process>>>             │
│ next_pid: AtomicU64                                      │
│ shutting_down: AtomicBool                                │
└──────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────┐
│ Scheduler (per thread)                                   │
├──────────────────────────────────────────────────────────┤
│ Main loop:                                               │
│   1. Pop from local run queue                            │
│   2. If empty → try work stealing from other schedulers  │
│   3. If still empty → sleep 100µs                        │
│   4. Run process until reductions exhausted              │
│   5. Re-enqueue if still runnable                        │
└──────────────────────────────────────────────────────────┘
```

**Key design decisions:**
- One scheduler thread per CPU core (configurable)
- Work-stealing for load balancing
- `parking_lot::Mutex` for low-overhead locking on run queues
- `dashmap::DashMap` for concurrent process access

### 4. Garbage Collector (`dala_runtime::gc`)

A **generational semi-space copying collector**:

```
GC Trigger Points:
  - Heap exhaustion (alloc request > available space)
  - Reduction count exhaustion
  - Explicit GC request

GC Phases:
  1. Root Set Scanning
     ├── Stack slots (via stack maps from compiler)
     ├── X registers (via RegisterFile)
     ├── Catch stack frames
     └── Mailbox messages
  2. Copying Collection
     ├── Allocate new heap (2× live size)
     ├── Trace from roots, copy live objects
     └── Update forwarding pointers
  3. Heap Swap
     ├── Update heap_start/heap_ptr
     └── Old heap freed on Drop
```

**Stack maps** (generated by `dala_codegen::stack_map`) tell the GC exactly which stack slots and registers hold heap pointers at each safepoint.

### 5. SSA IR (`dala_ir`)

The Intermediate Representation bridges BEAM bytecode and native code:

```
BEAM Instruction          IR Instruction
─────────────────         ───────────────
move X0, Y0          →    SetReg(X(0), GetReg(Y(0)))
add X0, X1, X2       →    SetReg(X(0), Add(GetReg(X(1)), GetReg(X(2))))
test is_integer, X0   →    BrIf(IsSmallInt(GetReg(X(0))), then, else)
call erlang:+, 2      →    CallBif(erlang, +, [a, b])
```

**IR Features:**
- **SSA form:** Each value defined exactly once (via `IRValueId`)
- **Typed:** Every value has an `IRType` (integer, float, term, etc.)
- **Control flow:** Basic blocks with `Br`, `BrIf`, `Switch`, `Ret`
- **Side effects tracking:** `SideEffects` struct tracks alloc, heap read/write, may_raise, calls, may_yield

### 6. Code Generation (`dala_codegen`)

Uses **Cranelift** as the backend:

```
IRModule
  └── For each IRFunction:
        ├── translate_instructions() — Map IR → Cranelift IR
        ├── compile_function()       — Machine code via Cranelift
        └── CompiledFunction { code_ptr, code_size, stack_map }
```

**Runtime Glue** (`RuntimeGlue`): Declares ~20 runtime functions that compiled code calls back into:
- `dala_alloc` — Heap allocation
- `dala_should_yield` — Reduction check
- `dala_consume_reductions` — Preemption
- `dala_bif_dispatch` — Built-in function dispatch
- `dala_send` / `dala_receive` — Message passing
- `dala_raise` / `dala_throw` — Exception handling

### 7. Hot Code Loading (`dala_dispatch`)

Supports atomic module replacement (like OTP's code server):

```
┌──────────────────────────────────────────────────┐
│ HotCodeManager                                    │
├──────────────────────────────────────────────────┤
│ modules: DashMap<u64, CompiledModule>             │
│                                                  │
│ hot_replace(module):                              │
│   1. Validate exports match old module            │
│   2. Register new module                          │
│   3. Atomically update LazyFnRef pointers         │
│      (RwLock allows concurrent reads)             │
└──────────────────────────────────────────────────┘
```

`LazyFnRef` uses `RwLock<CodePtr>` — readers (executing code) never block; writers (code loading) get exclusive access only during the brief swap.

---

## Execution Modes

| Mode | Description | Use Case |
|------|-------------|----------|
| `Interpreted` | Pure bytecode interpretation | Baseline, debugging |
| `Mixed` (default) | AOT-compiled + interpreter fallback | General use |
| `Aot` | Only AOT-compiled code, no interpreter | iOS/App Store (no RWX pages) |
| `Jit` | JIT compilation at runtime | Maximum performance |

---

## Design Decisions

### Why Cranelift?
- **Fast compilation** — critical for JIT
- **Rust-native** — no C++ dependency
- **Mobile-friendly** — works on iOS (unlike LLVM's JIT)
- **Simple embedding** — designed for JIT/embedded use

### Why SSA IR?
BEAM opcodes are stack/register-oriented and difficult to optimize directly. SSA enables:
- Dead code elimination
- Constant propagation
- Common subexpression elimination (CSE)
- Inlining
- Register allocation
- Loop optimization

### Why Not Compile Directly from BEAM?
Direct compilation would lose optimization opportunities. The IR layer decouples the BEAM frontend from the native backend, enabling:
- Re-optimization after hot code loading
- Target-independent optimizations
- Easier debugging and profiling

---

## Memory Layout

```
Process Memory Layout:
┌──────────────────────────────────────────┐ High address
│  Stack (grows downward)                  │
│    ┌─────────────────────────────────┐   │
│    │  Stack Frame N (current func)   │   │
│    │  Stack Frame N-1                │   │
│    │  ...                            │   │
│    │  Stack Frame 0                  │   │
│    └─────────────────────────────────┘   │
│         ↑ stack_ptr                      │
│  ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─  │
│         ↓ heap_ptr                       │
│  ┌─────────────────────────────────┐     │
│  │  Heap (grows upward)            │     │
│  │    Object 1 (tuple, list, etc.) │     │
│  │    Object 2                     │     │
│  │    Object 3                     │     │
│  │    ...                          │     │
│  └─────────────────────────────────┘     │
├──────────────────────────────────────────┤
│  Registers (RegisterFile)                │
│    X[0..255], Y[0..1023], F[0..255]     │
├──────────────────────────────────────────┤
│  Mailbox (message queue)                 │
└──────────────────────────────────────────┘ Low address
```