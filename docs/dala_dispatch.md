# `dala_dispatch` — Module Dispatch & Hot Code Loading

## What It Is

`dala_dispatch` manages **module registration, function lookup, and hot code
loading**. It is the bridge between compiled code and the runtime, providing
the dispatch layer that maps `(module, function, arity)` tuples to native code
pointers.

## How It Fits In the Pipeline

```
dala_codegen (produces CompiledFunctions)
    ↓
dala_dispatch (registers modules, builds export table)
    ↓
Runtime (looks up and calls functions via dispatch)
```

## Module Structure

```
dala_dispatch/src/
├── lib.rs              — DispatchManager, CompiledModule, HotCodeError
├── export_table.rs     — ExportTable, ExportKey, ExportEntry
└── hot_code.rs         — HotCodeManager, LazyFnRef
```

## Key Types

### DispatchManager

The central coordinator:

```rust
pub struct DispatchManager {
    modules: DashMap<u64, Arc<CompiledModule>>,
    export_table: ExportTable,
    hot_code: HotCodeManager,
    code_registry: CodeRegistry,
}
```

### CompiledModule

```rust
pub struct CompiledModule {
    pub name: u64,
    pub exports: Vec<ExportEntry>,
    pub ir_module: IRModule,
    pub metadata: ModuleMetadata,
}

pub struct ExportEntry {
    pub function: u64,
    pub arity: u32,
    pub code_ptr: CodePtr,
    pub lazy_ref: LazyFnRef,
}
```

### ExportTable

A concurrent hash map for lock-free function lookup:

```rust
pub struct ExportTable {
    entries: DashMap<ExportKey, ExportEntry>,
}

pub struct ExportKey {
    pub module: u64,
    pub function: u64,
    pub arity: u32,
}
```

### LazyFnRef

The key to hot code loading — an atomically swappable function pointer:

```rust
pub struct LazyFnRef {
    code: RwLock<CodePtr>,
    module: u64,
    function: u64,
    arity: u32,
}
```

Readers (executing code) use `get()` which acquires a read lock — never
blocks other readers. Writers (code loading) use `set()` which acquires a
write lock — only blocks during the brief pointer swap.

## Hot Code Loading

### The Protocol

```
1. New module is compiled (dala_codegen)
2. DispatchManager::hot_replace() is called
3. Validate exports match old module
4. Register new module in DashMap
5. Atomically update LazyFnRef pointers
6. Old code continues running; new calls use new code
```

### Why It Works

- `DashMap` provides concurrent read access — existing calls to the old
  code are not interrupted
- `RwLock` on `LazyFnRef` means readers never block each other
- The pointer swap is atomic at the `RwLock` level
- Old code finishes its current execution with its own stack/registers

### Validation

Before replacing a module, the dispatch manager validates that the new module
has the same exports as the old one. This prevents breaking callers that
expect certain functions to exist.

```rust
pub fn hot_replace(&self, module: CompiledModule) -> Result<(), HotCodeError> {
    let name = module.name;
    if let Some(old) = self.modules.get(&name) {
        let old_exports: Vec<_> = old.exports.iter()
            .map(|e| (e.function, e.arity)).collect();
        let new_exports: Vec<_> = module.exports.iter()
            .map(|e| (e.function, e.arity)).collect();
        if old_exports != new_exports {
            return Err(HotCodeError::ExportMismatch);
        }
    }
    // ... proceed with replacement
}
```

## Function Lookup

```rust
// Look up a function by (module, function, arity)
let code_ptr = dispatch.lookup_function(module, function, arity)?;

// The returned CodePtr can be called directly:
let result = unsafe { code_ptr.as_fn()(process, args) };
```

The lookup is O(1) via `DashMap` and lock-free for concurrent readers.

## CodeRegistry

The `CodeRegistry` manages the mapping from `(module, function, arity)` to
`FunctionEntry` metadata:

```rust
pub struct CodeRegistry {
    modules: RwLock<HashMap<u64, ModuleCode>>,
}

pub struct FunctionEntry {
    pub code: CodePtr,
    pub is_aot: bool,
    pub module: u64,
    pub function: u64,
    pub arity: u32,
    pub file: u64,
    pub line: u32,
}
```

## Tracing & Debugging

### Enable Dispatch Tracing

```bash
RUST_LOG=dala_dispatch=trace cargo run --bin dala_aot -- run --input test.beam
```

### Inspect Registered Modules

```rust
let modules = dispatch.code_registry().modules();
for module_name in modules {
    println!("Module: {}", module_name);
    if let Some(func) = dispatch.lookup_function(module_name, func_name, arity) {
        println!("  Code: {:?}, AOT: {}", func.as_usize(), func.is_aot);
    }
}
```

### Hot Code Loading in Production

```rust
// 1. Compile new module
let new_module = compiler.compile_beam_module(&ir_module)?;

// 2. Hot-replace
dispatch.hot_replace(new_module)?;

// 3. Verify
assert!(dispatch.lookup_function(module_name, func_name, arity).is_some());
```

## Developing New Features

### Adding a New Dispatch Strategy

1. Implement a new lookup method on `DispatchManager`
2. Add configuration to control which strategy is used
3. Benchmark against the default `DashMap`-based lookup

### Adding Module-Level Metadata

1. Add fields to `ModuleMetadata`
2. Populate during compilation in `dala_codegen`
3. Expose via `DispatchManager` API
