# Dala AOT — BEAM Ahead-of-Time Compiler

A Rust-based AOT compiler backend for the BEAM VM (Erlang/OTP), inspired by HiPE but with a modern architecture. This project implements a second execution backend for BEAM that compiles bytecode to native ARM64/x86 machine code.

## Architecture

```
Elixir/Erlang
    ↓
Core Erlang
    ↓
BEAM Compiler (existing)
    ↓
.beam files
    ↓
Dala AOT Compiler (this project)
    ↓
ARM64/x86 native binaries
    ↓
BEAM Runtime (scheduler + GC + process model)
```

**Key insight:** This is NOT "compile Erlang to machine code." It is building a BEAM-compatible runtime backend with native execution. OTP runtime, scheduler, GC, and process model all still exist — only the execution engine changes.

## Crates

| Crate | Description |
|-------|-------------|
| `dala_runtime` | Core runtime: process model, scheduler, GC, term representation, BIFs |
| `dala_ir` | SSA intermediate representation with optimization passes |
| `dala_beam_loader` | Parses .beam files into IR |
| `dala_codegen` | Cranelift-based native code generation (JIT + AOT) |
| `dala_dispatch` | Module dispatch, hot code loading, export tables |
| `dala_aot` | CLI tool orchestrating the full pipeline |

## Features

- **Process model**: Full BEAM process semantics (heap, stack, mailbox, reductions)
- **Scheduler**: SMP scheduler with work-stealing and reduction counting
- **GC**: Semi-space copying collector with stack maps and root set scanning
- **SSA IR**: Typed SSA IR with dead code elimination, constant propagation, CSE, CFG simplification
- **Code generation**: Cranelift-based native codegen supporting x86_64 and AArch64
- **Mixed execution**: AOT-compiled and interpreted code can coexist
- **BEAM compatibility**: All BEAM features preserved (pattern matching, exceptions, binaries, funs, ETS, NIFs)

## Phased Implementation

### Phase 1 — Minimal Execution (✓)
- Arithmetic, function calls, tuples, pattern matching

### Phase 2 — Scheduler Integration (✓)
- Reductions, yielding, process switching

### Phase 3 — GC Support (✓)
- Heap allocation, root maps, safepoints

### Phase 4 — OTP Compatibility (in progress)
- Binaries, exceptions, funs, ETS, ports

### Phase 5 — Optimization (planned)
- SSA optimizations, inlining, specialization, escape analysis

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

## Usage

```bash
# Compile a BEAM file to native code
dala_aot compile --input my_module.beam --output my_module.o --target x86_64 --mode aot

# Inspect a BEAM file
dala_aot inspect --input my_module.beam

# Run a BEAM module
dala_aot run --input my_module.beam -- mixed

# Disassemble BEAM bytecode
dala_aot disasm --input my_module.beam
```

## Design Decisions

### Why Cranelift?
- **Fast compilation** (critical for JIT)
- **Rust-native** (no C++ dependency)
- **Mobile-friendly** (works on iOS, unlike LLVM)
- **Simple embedding** (designed for JIT/embedded use)

### Why SSA IR?
BEAM opcodes are stack/register oriented and difficult to optimize directly. SSA form enables:
- Dead code elimination
- Constant propagation
- Inlining
- Register allocation
- Loop optimization

### Why not compile directly from BEAM?
Direct compilation from BEAM bytecode would lose optimization opportunities. The IR layer acts as a bridge that decouples the BEAM frontend from the native backend.

## iOS Deployment

```
mix compile
    ↓
beam files
    ↓
dala_aot --mode aot --target aarch64
    ↓
arm64 object files
    ↓
Xcode static library
    ↓
Signed IPA
```

No runtime code generation needed — fully avoids RWX pages, JIT restrictions, and App Store rejection.

## License

Apache-2.0 / MIT dual license (same as Rust).

## License Change

This project is now licensed under the Mozilla Public License, Version 2.0 (MPL-2.0).
See the LICENSE file for details.