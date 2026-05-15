# `dala_aot` — CLI Tool

## What It Is

`dala_aot` is the **command-line interface** for the Dala compiler runtime. It
orchestrates the full pipeline from BEAM file to native code, providing
commands for compiling, inspecting, running, and disassembling BEAM modules.

## Commands

### `compile` — Compile a BEAM file to native code

```bash
dala_aot compile --input my_module.beam --output my_module.o --target x86_64 --mode aot
```

| Flag | Description | Default |
|------|-------------|---------|
| `--input` | Input .beam file path | (required) |
| `--output` | Output file path | (required) |
| `--target` | Target architecture (x86_64, aarch64) | x86_64 |
| `--mode` | Compilation mode (jit, aot) | aot |
| `-O` | Optimization level (none, less, default, aggressive) | default |

### `inspect` — Inspect a BEAM file

```bash
dala_aot inspect --input my_module.beam
```

Shows module name, exports, atom table, and function count.

### `disasm` — Disassemble BEAM bytecode

```bash
dala_aot disasm --input my_module.beam
```

Prints human-readable BEAM instructions with operands.

### `run` — Run a BEAM module

```bash
dala_aot run --input my_module.beam -- mixed
dala_aot run --input my_module.beam -- interpreted
dala_aot run --input my_module.beam -- native
```

| Flag | Description | Default |
|------|-------------|---------|
| `--input` | Input .beam file path | (required) |
| `--mode` | Execution mode (interpreted, mixed, native) | mixed |
| `args` | Arguments to pass to the module's main function | (none) |

## CLI Argument Definitions

```rust
#[derive(Parser)]
pub struct Cli {
    pub command: Commands,
}

pub enum Commands {
    Compile { input, output, target, mode, optimize },
    Inspect { input },
    Run { input, args, mode },
    Disasm { input },
}

pub enum CompilationMode { Jit, Aot }
pub enum OptLevel { None, Less, Default, Aggressive }
pub enum ExecutionMode { Interpreted, Mixed, Native }
```

## Usage Examples

### Compile for iOS (AOT, ARM64)

```bash
dala_aot compile   --input my_app.beam   --output my_app.o   --target aarch64   --mode aot   -O aggressive
```

### Inspect a Module

```bash
dala_aot inspect --input my_app.beam
# Output:
# Module: my_app
# Exports: [{start, 0}, {init, 1}]
# Functions: 42
# Atoms: 156
```

### Run with Debug Logging

```bash
RUST_LOG=dala_runtime=debug dala_aot run --input test.beam -- mixed
```

### Benchmark Compilation

```bash
# Compile with different optimization levels
for opt in none less default aggressive; do
    time dala_aot compile --input test.beam --output test_$opt.o -O $opt
done
```

## Tracing & Debugging

### Log Levels

| Level | What It Shows |
|-------|--------------|
| `error` | Only errors |
| `warn` | Warnings + errors |
| `info` | High-level pipeline progress |
| `debug` | Per-function compilation details |
| `trace` | Every IR instruction, every GC event |

### Enable Tracing

```bash
# Full trace
RUST_LOG=trace dala_aot compile --input test.beam --output test.o

# Per-module trace
RUST_LOG=dala_ir=trace,dala_codegen=debug dala_aot compile --input test.beam --output test.o

# GC trace
RUST_LOG=dala_runtime::gc=trace dala_aot run --input test.beam

# Scheduler trace
RUST_LOG=dala_runtime::scheduler=trace dala_aot run --input test.beam
```

### Lint and Check

```bash
cargo clippy --all-targets -- -D warnings
cargo fmt -- --check
```
