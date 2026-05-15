# Getting Started Guide

This guide walks you through setting up, building, and running the Dala
Compiler Runtime step by step.

## Prerequisites

### Required Tools

- **Rust** (1.85 or later) — Install via [rustup](https://rustup.rs/):
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
- **CMake** (3.16+) — Required for building Cranelift:
  ```bash
  # macOS
  brew install cmake

  # Ubuntu/Debian
  sudo apt-get install cmake
  ```
- **Ninja** — Fast build system used by Cranelift:
  ```bash
  # macOS
  brew install ninja

  # Ubuntu/Debian
  sudo apt-get install ninja-build
  ```

### Optional Tools

- **Elixir/OTP** (1.15+) — For generating .beam files to compile
- **hexdump** or **xxd** — For inspecting binary output
- **objdump** — For disassembling generated object files

## Step 1: Clone the Repository

```bash
git clone https://github.com/manhvu/dala_compiler_runtime
cd dala_compiler_runtime
```

## Step 2: Build the Project

### Full Build (All Crates)

```bash
cargo build --release
```

This builds all six crates: `dala_runtime`, `dala_ir`, `dala_beam_loader`,
`dala_codegen`, `dala_dispatch`, and `dala_aot`.

### Build with JIT Support (Default)

```bash
cargo build --release --features jit
```

### Build for AOT-Only (No JIT — for iOS/App Store)

```bash
cargo build --release --features aot
```

### Development Build (Fast Iteration)

```bash
cargo build
RUST_LOG=debug cargo build
cargo test
```

## Step 3: Run Tests

```bash
# All tests
cargo test

# Specific crate
cargo test -p dala_ir
cargo test -p dala_runtime

# Specific test
cargo test -p dala_runtime -- mailbox::tests
cargo test -p dala_runtime -- scheduler::tests
cargo test -p dala_ir -- type_system::tests
```

## Step 4: Install the CLI Tool

```bash
cargo install --path dala_aot --release
dala_aot --version
```

## Step 5: Prepare a BEAM File

### Option A: Compile Your Own

```bash
cat > hello.erl << 'EOF'
-module(hello).
-export([world/0]).

world() ->
    io:format("Hello from Dala!~n"),
    42.
EOF
erlc hello.erl
```

### Option B: Use an Existing BEAM File

```bash
find /usr/local/lib/elixir -name "*.beam" | head -5
```

## Step 6: Inspect a BEAM File

```bash
dala_aot inspect --input hello.beam
```

## Step 7: Disassemble BEAM Bytecode

```bash
dala_aot disasm --input hello.beam
```

## Step 8: Compile to Native Code

### Compile to Object File (AOT Mode)

```bash
# x86_64
dala_aot compile --input hello.beam --output hello.o --target x86_64 --mode aot

# ARM64
dala_aot compile --input hello.beam --output hello.o --target aarch64 --mode aot
```

### Optimization Levels

```bash
dala_aot compile --input hello.beam --output hello.o -O none
dala_aot compile --input hello.beam --output hello.o -O less
dala_aot compile --input hello.beam --output hello.o -O default
dala_aot compile --input hello.beam --output hello.o -O aggressive
```

## Step 9: Run a BEAM Module

```bash
dala_aot run --input hello.beam -- mixed
dala_aot run --input hello.beam -- interpreted
dala_aot run --input hello.beam -- native
```

## Step 10: Link the Object File

```bash
ar rcs libhello.a hello.o
gcc main.o hello.o -o hello_program -ldala_runtime
```

## Step 11: Embed in an iOS Project (Optional)

```bash
dala_aot compile --input my_module.beam --output my_module.o --target aarch64 --mode aot
```

Add `my_module.o` to your Xcode project and link against `dala_runtime` as a
static library.

## Troubleshooting

| Issue | Solution |
|-------|----------|
| `cranelift` build fails | Ensure CMake and Ninja are installed |
| `Unsupported opcode` | The BEAM module uses an instruction not yet implemented |
| `Link error: undefined symbol` | Ensure dala_runtime is linked and compiled |
| `SIGSEGV in tests` | Some term tests have a known issue with num-bigint; use `cargo test --skip term::tests` |

### Debug Output

```bash
RUST_LOG=trace cargo run --bin dala_aot -- compile --input hello.beam ...
cargo clippy --all-targets -- -D warnings
```

## Next Steps

- Read the [Architecture Guide](architecture.md) for a deep dive into the system
- Read the [Reference](reference.md) for complete API documentation
- Read the subproject guides:
  - [`dala_ir`](dala_ir.md) — Intermediate representation
  - [`dala_runtime`](dala_runtime.md) — Core runtime
  - [`dala_codegen`](dala_codegen.md) — Code generation
  - [`dala_beam_loader`](dala_beam_loader.md) — BEAM file parser
  - [`dala_dispatch`](dala_dispatch.md) — Module dispatch & hot code loading
  - [`dala_aot`](dala_aot.md) — CLI tool
