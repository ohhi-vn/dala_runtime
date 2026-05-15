# `dala_beam_loader` — BEAM File Parser

## What It Is

`dala_beam_loader` parses **.beam files** (the standard BEAM bytecode format)
into structured data that `dala_ir` can consume. It reads the binary chunk
format defined by EEP-46 and produces `BeamModule` structs containing
functions, exports, atoms, and metadata.

## How It Fits In the Pipeline

```
.beam file (binary)
    ↓
dala_beam_loader (parse chunks → BeamModule)
    ↓
dala_ir (build SSA IR from BeamModule)
    ↓
dala_codegen (generate native code)
```

## Module Structure

```
dala_beam_loader/src/
├── lib.rs              — BeamModule, loading functions, re-exports
├── bytecode.rs         — BeamFunction, BeamInstruction, BeamOperand, BeamRegister
├── chunk.rs            — BeamChunk, chunk IDs, section types
├── reader.rs           — BeamReader (low-level binary parsing)
└── error.rs            — BeamError, Result type alias
```

## BEAM File Format

BEAM files use a chunk-based binary format:

```
┌──────────────────────────────────────────────────────────┐
│ FOR1 header (4 bytes) + total size (4 bytes)             │
├──────────────────────────────────────────────────────────┤
│ BEAM header (4 bytes)                                    │
├──────────────────────────────────────────────────────────┤
│ Atom chunk      — Atom table (string interning)          │
│ Code chunk      — Function bytecode                      │
│ ExpT chunk      — Export table                           │
│ LitT chunk      — Literal table                          │
│ LocL chunk      — Local function table                   │
│ Line chunk      — Line number info                       │
│ Attr chunk      — Module attributes                      │
│ StrT chunk      — String table                           │
│ ImpT chunk      — Import table                           │
│ CInf chunk      — Compile info                           │
│ ...                                                       │
└──────────────────────────────────────────────────────────┘
```

### Chunk Layout

Each chunk has:
```
┌──────────┬──────────┬─────────────────────┐
│ ID (4B)  │ Size (4B)│ Data (size bytes)   │
└──────────┴──────────┴─────────────────────┘
```

## Key Types

### BeamModule

```rust
pub struct BeamModule {
    pub name: String,
    pub functions: HashMap<(String, u32), BeamFunction>,
    pub exports: Vec<(String, u32, u32)>,  // (name, arity, label)
    pub atoms: Vec<String>,
    pub attributes: Vec<(String, String)>,
    pub compile_info: Option<CompileInfo>,
}
```

### BeamFunction

```rust
pub struct BeamFunction {
    pub name: String,
    pub arity: u32,
    pub label: u32,
    pub code: Vec<BeamInstruction>,
}
```

### BeamInstruction

```rust
pub struct BeamInstruction {
    pub opcode: u32,
    pub operands: Vec<BeamOperand>,
    pub line: Option<u32>,
}

pub enum BeamOperand {
    Register(BeamRegister),  // X(n), Y(n), F(n)
    Label(u32),              // Jump target
    Integer(i64),            // Integer literal
    Float(f64),              // Float literal
    AtomIndex(u32),          // Atom table index
}

pub enum BeamRegister {
    X(u32),  // Argument/return registers
    Y(u32),  // Stack frame slots
    F(u32),  // Floating point registers
}
```

## Loading Functions

```rust
// From file path
let module = load_beam_file("path/to/module.beam")?;

// From bytes (useful for embedded/compiled-in modules)
let module = load_beam_bytes(&beam_data)?;

// From any reader
let module = load_beam(reader)?;
```

## BeamReader

The low-level binary parser:

```rust
pub struct BeamReader<R: Read + Seek> {
    reader: R,
    pub pos: u64,
}

impl<R: Read + Seek> BeamReader<R> {
    pub fn from_file(path: &str) -> Result<Self>;
    pub fn read_u8(&mut self) -> Result<u8>;
    pub fn read_u16(&mut self) -> Result<u16>;
    pub fn read_u32(&mut self) -> Result<u32>;
    pub fn read_u64(&mut self) -> Result<u64>;
    pub fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>>;
    pub fn read_string(&mut self) -> Result<String>;
    pub fn read_chunk(&mut self) -> Result<Option<(String, Vec<u8>)>>;
    pub fn read_module(&mut self) -> Result<BeamModule>;
}
```

All multi-byte values are **big-endian** (standard BEAM format).

## Chunk Parsing

### Atom Chunk
Contains the atom table — interned strings used throughout the module.
The first 4 bytes are the count, followed by length-prefixed strings.

### Code Chunk
Contains the actual bytecode. Structure:
```
sub_size: u32
instruction_start: u32
num_functions: u32
num_labels: u32
num_records: u32
// For each function:
  name_index: u32
  arity: u32
  label: u32
  num_instructions: u32
  // For each instruction:
    opcode: u32
    num_operands: u32
    // For each operand:
      operand_type: u8
      value: u32/u64/f64 (depending on type)
```

### Export Table
Maps (function_name, arity) to code labels:
```
count: u32
// For each export:
  name_index: u32
  arity: u32
  label: u32
```

## Error Handling

```rust
pub enum BeamError {
    IoError(String),       // File read error
    FormatError(String),   // Invalid BEAM format
    UnexpectedEof,         // Truncated file
    Unsupported(String),   // Unknown opcode or feature
}
```

## Tracing & Debugging

### Enable Loader Tracing

```bash
RUST_LOG=dala_beam_loader=trace cargo run --bin dala_aot -- inspect --input test.beam
```

### Inspect a BEAM File

```bash
# Show module structure
dala_aot inspect --input test.beam

# Disassemble bytecode
dala_aot disasm --input test.beam
```

### Programmatic Inspection

```rust
let module = load_beam_file("test.beam")?;
println!("Module: {}", module.name);
println!("Functions: {}", module.function_count());
println!("Exports: {:?}", module.exports);
println!("Atoms: {:?}", module.atoms);

for ((name, arity), func) in &module.functions {
    println!("  {}/{}: {} instructions", name, arity, func.code.len());
    for (i, inst) in func.code.iter().enumerate() {
        println!("    {:4}: opcode={} operands={:?}", i, inst.opcode, inst.operands);
    }
}
```

## Developing New Features

### Supporting New BEAM Opcodes

1. Parse the opcode in `reader.rs::parse_code_chunk()`
2. Map it to the appropriate `BeamOperand` types
3. In `dala_ir::builder.rs`, add a translation from the BEAM instruction
   to IR instructions

### Adding New Chunk Parsers

1. Add the chunk ID to `chunk.rs::chunk_ids`
2. Add a parsing method to `BeamReader`
3. Call it in `read_module()` when the chunk ID is encountered
