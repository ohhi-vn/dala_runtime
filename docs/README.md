# Dala Compiler Runtime — Documentation Index

## Architecture & Design

| Document | Description |
|----------|-------------|
| [architecture.md](architecture.md) | **Start here.** High-level architecture, pipeline, crate map, design decisions. |
| [GC Requirements & Plan.md](GC%20Requirements%20&%20Plan.md) | GC design: multi-region memory, BSS, promotion protocol, implementation plan. |
| [set-theory type system.md](set-theory%20type%20system.md) | Set-theoretic types and their impact on AOT optimization. |
| [architecture_suggest.md](architecture_suggest.md) | Architecture gap analysis and benchmarking strategy. |
| [suggestion for implement.md](suggestion%20for%20implement.md) | Original implementation suggestions and design notes. |

## Subproject Guides

| Document | Description |
|----------|-------------|
| [dala_ir.md](dala_ir.md) | **IR crate.** Typed SSA IR, type system, instructions, optimization passes, how to write new passes. |
| [dala_runtime.md](dala_runtime.md) | **Runtime crate.** Processes, scheduler, GC, mailboxes, memory regions, AI layer, capabilities. |
| [dala_codegen.md](dala_codegen.md) | **Codegen crate.** Code generation, Cranelift backend, intrinsics, runtime glue, stack maps. |
| [dala_beam_loader.md](dala_beam_loader.md) | **Loader crate.** BEAM file format, chunk parsing, binary reader. |
| [dala_dispatch.md](dala_dispatch.md) | **Dispatch crate.** Module registration, export tables, hot code loading, LazyFnRef. |
| [dala_aot.md](dala_aot.md) | **CLI tool.** Commands, flags, usage examples, tracing. |

## API Reference

| Document | Description |
|----------|-------------|
| [reference.md](reference.md) | **Complete API reference.** All public types, methods, and constants across all crates. |

## Getting Started

| Document | Description |
|----------|-------------|
| [getting-started.md](getting-started.md) | **Setup guide.** Build, install, compile, run, troubleshoot. |

## Reading Path

### New to Dala?
1. [getting-started.md](getting-started.md) — Build and run
2. [architecture.md](architecture.md) — Understand the system
3. Subproject guide for your area of interest

### Implementing a New Feature?
1. [architecture.md](architecture.md) — Understand where it fits
2. Relevant subproject guide — Understand the code
3. [reference.md](reference.md) — API details
4. Subproject guide's "Developing New Features" section

### Debugging an Issue?
1. [dala_aot.md](dala_aot.md) — Enable tracing
2. Relevant subproject guide's "Tracing & Debugging" section
3. [reference.md](reference.md) — Check API usage
