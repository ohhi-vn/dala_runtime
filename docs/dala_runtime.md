# `dala_runtime` — Core Runtime

## What It Is

`dala_runtime` is the **execution engine** of Dala. It implements the actor
process model, QoS-aware scheduler, hybrid garbage collector, typed mailbox
system, multi-region memory management, AI runtime layer, and capability-based
native resource tracking.

## Module Structure

```
dala_runtime/src/
├── lib.rs              — RuntimeConfig, ExecutionMode, RuntimeError, init()
├── term.rs             — Term (tagged pointer), RegisterFile
├── process.rs          — Process, ProcessBuilder, ProcessFlags, CatchFrame
├── scheduler.rs        — Scheduler, QosClass, Governor, ThermalState, BatteryState
├── mailbox.rs          — Mailbox, MessageEnvelope, PriorityQueue, SimpleMailbox
├── gc/                 — Garbage collector
│   ├── mod.rs          — collect(), maybe_collect(), safepoint(), StackMap
│   ├── copy.rs         — Semi-space copying collection
│   ├── rootset.rs      — RootSet construction from process state
│   ├── sweep.rs        — Old heap sweeping, refcount management
│   └── header.rs       — ObjectHeader, GcColor, TypeDescriptor, TypeTable
├── memory/             — Memory regions
│   ├── mod.rs          — MemoryConfig, MemoryRegion trait
│   ├── arena.rs        — Arena allocator (bump-pointer, bulk-free)
│   └── regions.rs      — StableImmutableRegion, BinaryRegion, TensorRegion,
│                         NativeResourceRegion, NativeResourceId
├── ai/                 — AI runtime layer
│   ├── mod.rs          — AiConfig, InferencePriority, AiError
│   ├── tensor.rs       — Tensor, TensorDesc, TensorLocation
│   ├── inference.rs    — InferenceWorker, InferenceRequest, InferenceResult
│   ├── model.rs        — ModelRegistry, ModelHandle, ModelInfo
│   └── pipeline.rs     — Pipeline, PipelineStage, StreamConfig
├── atom.rs             — AtomTable, global atom interning
├── bif.rs              — Built-in functions (arithmetic, type tests, etc.)
├── code.rs             — CodePtr, CodeRegistry, FunctionEntry, ModuleCode
├── exception.rs        — Exception, Reason, Result helpers
├── port.rs             — Port, PortRegistry
└── trap.rs             — TrapFrame, TrapResult
```

## Term Representation

All values in Dala are represented as a **64-bit tagged pointer** (`Term`):

```
┌──────────────────────────────────────────────────────────┐
│ 64-bit Term (transparent u64)                            │
├──────────┬───────────────────────────────────────────────┤
│ Bits 0-1 │ Primary tag                                   │
│          │   00 = Boxed (pointer to heap object)         │
│          │   01 = List (cons cell pointer)               │
│          │   10 = Header (tuple, float, fun, map, etc.)  │
│          │   11 = Immediate                              │
├──────────┴───────────────────────────────────────────────┤
│ For immediates (tag=11):                                 │
│   Bits 2-5: Immed1 sub-tag                               │
│     0000 = Small integer (bits 6-63 = value >> 4)        │
│     0001 = PID                                           │
│     0010 = Port                                          │
│     0011 = Immed2 → atoms, registers, specials           │
│       Atom: bits 6-29 = atom index                       │
│       X reg: bits 6-29 = register index                  │
│       Special: nil, true, false                          │
└──────────────────────────────────────────────────────────┘
```

### Key Design Properties

- **Branch-free operations**: All type checks are bit manipulations
- **No heap allocation** for immediates (small ints, atoms, PIDs, nil, bools)
- **`#[repr(transparent)]`**: `Term` has the same layout as `u64`
- **Pattern matching**: `is_small()`, `is_atom()`, `is_list()`, etc. are single AND operations

## Process Model

Each Dala process is a self-contained execution unit with:

```rust
pub struct Process {
    pub pid: u64,
    // Heap
    pub heap_start: *mut Term,
    pub heap_ptr: *mut Term,
    pub heap_top: *mut Term,
    // Stack
    pub stack_ptr: *mut Term,
    pub stack_top: *mut Term,
    // State
    pub registers: RegisterFile,    // X[256], Y[1023], F[256]
    pub reductions: u32,            // Preemption counter
    pub max_reductions: u32,
    pub flags: ProcessFlags,
    pub mailbox: Mutex<Mailbox>,    // Priority-aware message queue
    pub catches: SmallVec<[CatchFrame; 4]>,
    pub current_function: (u64, u64, u32),  // (module, function, arity)
    pub code: CodePtr,
    pub priority: u8,
    pub qos: QosClass,              // QoS class for scheduling
    pub arena: Arena,               // Frame-scoped allocations
    pub stable_region: StableImmutableRegion,  // SIR
    pub status: ProcessStatus,
    pub exit_reason: Option<Term>,
}
```

### ProcessBuilder

Processes are created via the builder pattern:

```rust
let process = ProcessBuilder::new(pid)
    .heap_size(512)
    .reductions(2000)
    .priority(2)
    .group_leader(leader_pid)
    .initial_call(module, function, arity)
    .build()?;
```

### Heap Management

- **Allocation**: Bump-pointer, O(1). Grows by doubling on exhaustion.
- **GC trigger**: Heap exhaustion or reduction count exhaustion
- **Isolation**: Each process has its own heap — no shared mutable state

## Scheduler

### QoS-Aware Scheduling

The scheduler maintains **four run queues**, one per QoS class:

```
[Realtime]  → Voice, video, sensor fusion (highest priority)
[UserFacing] → UI, user interactions
[Utility]   → Data processing, caching
[Background] → Analytics, model updates (lowest priority)
```

The scheduler always picks from the highest-priority non-empty queue first.

### Thermal/Battery Governor

```rust
pub struct Governor {
    thermal: RwLock<ThermalState>,   // Nominal/Fair/Serious/Critical
    battery: RwLock<BatteryState>,   // {level: f32, charging: bool}
    throttling: AtomicBool,
}
```

The governor:
- **Scales reduction budgets** based on thermal state (100% → 80% → 50% → 25%)
- **Limits max QoS class** when thermal throttling is active
- **Rejects non-realtime inference** when device is hot

### Scheduler Loop

```rust
fn run(self) {
    loop {
        if shutting_down { break; }
        
        // Pick highest-priority process
        if let Some((pid, qos)) = self.global.pick_process() {
            self.run_process(pid, qos);
        } else {
            // Work stealing or sleep
            if let Some(work) = self.steal_work() { ... }
            else { sleep(100µs); }
        }
    }
}
```

## Mailbox System

### Architecture

```
┌──────────────────────────────────────────────────────────┐
│  Mailbox                                                 │
├──────────────────────────────────────────────────────────┤
│  [Critical]  Supervision, fault recovery                 │
│  [High]      UI events, control signals                  │
│  [Normal]    Standard actor messages                     │
│  [Low]       Telemetry, background work                  │
├──────────────────────────────────────────────────────────┤
│  Type Index: HashMap<type_tag, Vec<queue_idx>>          │
│  Overflow: VecDeque (back-pressure when queues full)     │
└──────────────────────────────────────────────────────────┘
```

### Message Envelope

```rust
pub struct MessageEnvelope {
    pub payload: Term,
    pub priority: MessagePriority,  // Critical/High/Normal/Low
    pub sender: u64,
    pub type_tag: Option<u32>,      // For fast-path matching
}
```

### Fast-Path Receive

When the compiler knows the expected message type:

```rust
// Instead of scanning all messages:
msg = mailbox.dequeue_typed(TAG_TOKEN)?;
// Uses type index for O(1) lookup
```

## Memory Regions

### Region Overview

| Region | Allocation | Deallocation | Use Case |
|--------|-----------|-------------|----------|
| Actor Heap | Bump pointer | Copying GC | Short-lived BEAM terms |
| SIR | Bump pointer | Never (actor-scoped) | UI trees, configs, schemas |
| Binary Region | `alloc()` | Refcount | Large binaries, shared data |
| Tensor Region | `alloc()` | Refcount | GPU/NN buffers |
| Native Resource | `register()` | `release()` | Files, sockets, GPU contexts |
| Arena | Bump pointer | `reset()` — O(1) bulk | Frame-scoped work |

### Arena Allocator

```rust
let arena = Arena::new(64 * 1024);     // 64 KB initial chunk
let ptr = arena.alloc(256);            // Bump pointer allocation
let buf = arena.alloc_aligned(64, 64); // 64-byte aligned
arena.reset();                          // Everything freed, O(1)
```

Arenas use chunked allocation: when the current chunk is full, a new chunk
(2× size, up to a max) is allocated. On `reset()`, only the first chunk is
kept; all others are freed.

### Stable Immutable Region (SIR)

```rust
let sir = StableImmutableRegion::new(RegionId(0), 1024 * 1024);
let ptr = sir.allocate_immutable(&Layout::new::<MyStruct>());
// Object is now in SIR — never rescanned by GC
```

Objects enter SIR when they:
1. Survive N GC cycles
2. Are compiler-proven immutable
3. Have no references to young-heap objects

### Native Resource Region

```rust
let region = NativeResourceRegion::new(RegionId(0));
let id = region.register(NativeResourceKind::GpuContext, handle, true, true, actor_pid)?;
region.transfer(id, new_owner)?;
region.release(id)?;  // Auto-cleanup if owned
```

## Garbage Collector

### Young Heap (Semi-Space Copying)

1. Trigger: heap exhaustion or yield point
2. Scan roots: stack (via stack maps), registers, mailbox
3. Copy live objects to new heap (2× live size)
4. Update forwarding pointers
5. Free old heap

**Target pause**: < 500 µs per process

### Old Heap (Concurrent Tri-Color Mark + Incremental Sweep)

1. Trigger: promotion from young heap
2. Concurrent marking from roots (background thread)
3. Write barrier: young→old references tracked in remembered set
4. Incremental sweep interleaved with mutator

**Target pause**: < 2 ms (incremental slices)

### SIR Integration

- Stable-black objects are never rescanned
- SIR roots tracked via lightweight reference table
- GC traversal skip reduces work by ≥60% for stable workloads

## AI Runtime Layer

### Inference Workers

```rust
let mut worker = InferenceWorker::new(WorkerConfig {
    max_concurrent: 2,
    enable_cache: true,
    thermal_threshold: 0.8,
});

let result = worker.submit(InferenceRequest {
    model_id: 1,
    inputs: vec![tensor_desc],
    priority: InferencePriority::UserFacing,
    timeout_ms: 5000,
})?;
```

Workers automatically throttle non-realtime requests when the device is hot.

### Tensor Resources

```rust
let desc = TensorDesc::image(TensorDtype::F32, 1, 3, 224, 224);
let tensor = Tensor::new(desc)?;
// tensor.data_ptr() → *mut u8
// tensor.as_slice::<f32>() → &[f32]
```

### Model Registry

```rust
let registry = ModelRegistry::new(256 * 1024 * 1024); // 256 MB max
let model_id = registry.load_model("resnet50", "/path/to/model.onnx")?;
let handle = registry.get(model_id).unwrap();
```

### Streaming Pipelines

```rust
let stages = vec![
    PipelineStage::Preprocess { name: "resize".into() },
    PipelineStage::Inference { model_id: 1 },
    PipelineStage::Postprocess { name: "nms".into() },
];
let mut pipeline = Pipeline::new(stages, StreamConfig::default());
pipeline.start();
pipeline.push_input(tensor)?;
while let Some(output) = pipeline.process_one()? { ... }
```

## Tracing & Debugging

### Enable Runtime Tracing

```bash
# Full trace
RUST_LOG=dala_runtime=trace cargo run --bin dala_aot -- run --input test.beam

# Scheduler only
RUST_LOG=dala_runtime::scheduler=debug cargo run -- ...

# GC only
RUST_LOG=dala_runtime::gc=debug cargo run -- ...

# Mailbox only
RUST_LOG=dala_runtime::mailbox=trace cargo run -- ...
```

### GC Statistics

```rust
// After a GC cycle:
println!("Heap before: {} words", stats.heap_words_before);
println!("Heap after: {} words", stats.heap_words_after);
println!("Roots scanned: {}", stats.roots_scanned);
println!("Objects copied: {}", stats.objects_copied);
println!("Time: {} ns", stats.time_ns);
```

### Process Inspection

```rust
println!("PID: {}", process.pid);
println!("Status: {:?}", process.status);
println!("Reductions: {}/{}", process.reductions, process.max_reductions);
println!("QoS: {:?}", process.qos);
println!("Mailbox: {} messages", process.mailbox.lock().len());
println!("Arena: {}/{} bytes used", process.arena.total_used(), process.arena.total_capacity());
```

### Memory Region Inspection

```rust
// SIR usage
println!("SIR: {}/{} bytes", sir.used(), sir.capacity());

// Tensor region GPU usage
println!("GPU: {} bytes", tensor_region.gpu_usage());

// Native resources
println!("Active resources: {}", resource_region.len());
```

## Developing New Features

### Adding a New BIF (Built-In Function)

1. Add the function to `bif.rs`:
   ```rust
   pub unsafe fn my_bif_2(proc: &mut Process, args: *const Term) -> Term {
       // Implementation
   }
   ```
2. Register in `register_all_bifs()`:
   ```rust
   bif("erlang", "my_bif", 2, my_bif_2);
   ```

### Adding a New Memory Region

1. Implement the `MemoryRegion` trait:
   ```rust
   pub struct MyRegion { ... }
   impl MemoryRegion for MyRegion {
       fn id(&self) -> RegionId { ... }
       fn capacity(&self) -> usize { ... }
       fn used(&self) -> usize { ... }
       fn try_allocate(&self, size: usize) -> *mut u8 { ... }
       unsafe fn deallocate(&self, ptr: *mut u8, size: usize) { ... }
   }
   ```
2. Add to `MemoryConfig` if needed
3. Integrate with `ProcessBuilder`

### Adding a New QoS Class

1. Add variant to `QosClass` enum
2. Update `Governor::max_qos()` and `Governor::reduction_budget()`
3. Update scheduler queue count (currently 4)
