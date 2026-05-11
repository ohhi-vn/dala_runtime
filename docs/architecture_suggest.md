# Dala — Architecture Gaps & Benchmarking Strategy

---

## Part 1 — Architecture & Design Gaps

### Gap 1: Term Tagging Scheme — Undocumented & Untested

**The risk.** Term tagging is the most bug-prone part of any BEAM-like runtime. A single off-by-one in the tag bits silently corrupts GC root scanning, pattern matching, and arithmetic — with no error until a downstream crash.

**What needs to exist** (`dala_runtime/src/term/tags.rs`):

```
Pointer tag layout (low 3 bits of a word):
  000  → boxed pointer (heap object)
  001  → list (cons cell)
  010  → atom (index into atom table)
  011  → small integer (remaining 61 bits, sign-extended)
  100  → (reserved / pid)
  101  → port
  110  → reference
  111  → immediate 2 (float, special values)
```

Every tag variant needs:
- A constructor: `fn make_integer(i: i64) -> Term`
- A type check: `fn is_integer(t: Term) -> bool`
- A safe extractor: `fn as_integer(t: Term) -> Option<i64>`
- A unit test exercising all edge cases (e.g. `i64::MIN`, `i64::MAX`, `0`, `-1`)

The GC scanner's `TypeDescriptor::pointer_map` must agree exactly with this tagging — a word is a pointer iff its low bits are `000` or `001`. **Write the tag table first, then derive the pointer bitmap rules from it.**

---

### Gap 2: Mixed-Mode Execution Boundary

The README says:
> AOT-compiled and interpreted code can coexist

But the boundary protocol is unspecified. This is the hardest correctness problem in the project.

**The boundary involves:**

```
Native (AOT) frame
  ↓ call to uncompiled module
  ↓ transition to interpreter frame
  ↑ return value crosses the boundary
  ↑ back to native frame
```

**What must be defined:**

- **Calling convention adapter.** Native code uses hardware registers (ABI); the interpreter uses a value stack. Need a trampoline that marshals between them.
- **Stack map at call sites.** The GC must be able to scan a mixed-mode call stack. Each native frame needs a stack map entry marking which slots contain live GC roots at every call site.
- **Return value protocol.** Who owns the returned `Term`? Which heap was it allocated on? Can the callee's heap be collected while the caller holds a reference to a return value?
- **Exception unwind.** A `throw` in an interpreted callee must unwind through native frames. Cranelift does not generate DWARF unwind tables by default — you need to either enable them or implement a manual unwind protocol.

**Recommended design decision to make now:**

Define a `CallBoundary` enum and commit to it before writing more codegen:

```rust
pub enum CallBoundary {
    /// Both caller and callee are AOT-compiled. Direct call.
    NativeToNative,
    /// Caller is native, callee is interpreted. Trampoline needed.
    NativeToInterp { trampoline: TrampolineFn },
    /// Caller is interpreted, callee is native. Register marshal needed.
    InterpToNative { marshal: MarshalFn },
}
```

---

### Gap 3: Exception Handling in Native Code

BEAM exceptions (`throw`, `error`, `exit`) are pervasive. In interpreted BEAM, `catch` and `try` are implemented as stack markers. In native code, this needs a real mechanism.

**Three options — pick one now:**

| Option | Pros | Cons |
|--------|------|------|
| Setjmp/longjmp per process | Simple, no DWARF needed | Leaks C++ destructors; not `async-signal-safe` |
| Cranelift exception tables | Correct | Cranelift support is partial; needs manual integration |
| Explicit result threading | Safe, Rust-idiomatic | Every function returns `Result<Term, Exception>`; adds overhead |

**Recommendation:** Use explicit result threading for AOT functions (option 3). It's the most Rust-idiomatic, avoids unsafe unwinding through Cranelift frames, and the overhead is largely eliminated by inlining. Reserve `setjmp` for the interpreter boundary only.

---

### Gap 4: Tail Call Protocol

BEAM guarantees proper tail calls — this is fundamental to actor loop correctness (a GenServer's `handle_call` recursing forever must not grow the stack).

Cranelift supports tail calls via `return_call` / `return_call_indirect`, but they must be explicitly emitted. If your codegen emits regular `call` for tail positions, you silently break one of BEAM's core guarantees.

**What to add:**

- A tail-call analysis pass in `dala_ir` that marks tail-position `Call` instructions
- Codegen must lower tail-call `Call` nodes to Cranelift's `return_call`
- A test: a function that recurses 10,000,000 times must not stack-overflow

---

### Gap 5: ETS — Strategy Undefined

ETS (Erlang Term Storage) is used by virtually every real OTP application. The README lists it as a Phase 4 goal with no design.

**Key decisions to make:**

- **Storage backend.** A concurrent hash map (`dashmap`, already in deps) works for `set` and `bag`. Ordered sets need a concurrent B-tree (e.g. `crossbeam-skiplist`).
- **Memory ownership.** ETS tables outlive their owning process. Terms stored in ETS must be **copied out of the process heap** into a shared heap, or use the large-object refcounted space.
- **GC interaction.** The GC must treat ETS as a root set — terms in ETS tables keep process-heap objects alive if cross-references exist (they shouldn't, but must be enforced).
- **NIF access.** NIFs frequently access ETS directly via `enif_make_*`. The NIF ABI must expose ETS operations.

**Recommended minimal design:**

```
ETS table:
  storage: DashMap<Term, Vec<Term>>   // set semantics
  owner: ProcessId
  heap: SharedHeap                     // terms copied here on insert
  access: EtsAccess { public | protected | private }
```

---

### Gap 6: NIF ABI Bridge

NIFs (Native Implemented Functions) are how Erlang/Elixir calls C/Rust extensions. Without NIF compatibility, most hex.pm packages won't work.

**The NIF ABI is standardized by OTP** (`erl_nif.h`). Dala must expose the same function signature table.

**What's needed:**

- `enif_alloc` / `enif_free` — must use Dala's allocator, not libc malloc
- `enif_make_*` / `enif_get_*` — must produce/consume Dala's `Term` encoding
- `enif_send` — must integrate with Dala's scheduler
- `enif_self` — must return the current process's pid in Dala's format
- A loading protocol: `NifEntry` struct with function table, loaded via `dlopen`

**Biggest risk:** If your `Term` encoding (gap 1) diverges from OTP's, every existing NIF breaks. Either match OTP's encoding exactly, or provide a thin translation layer at the NIF boundary.

---

### Gap 7: Binary & Bitstring Handling

BEAM's binary matching (the `bs_*` opcodes) is one of the most complex parts of the VM, and it's listed as Phase 4 in-progress. Some design decisions to nail down now:

- **Small binary threshold.** Binaries ≤ 64 bytes: heap-allocated inline. Binaries > 64 bytes: refcounted in large-object space. This threshold affects GC scanning and copy semantics.
- **Sub-binary / match context.** BEAM's `bs_match` creates a match context pointing into a parent binary. This must not prevent GC of the parent if the context escapes. Use the refcount to keep the parent alive.
- **Bitstring alignment.** Bitstrings can have non-byte-aligned lengths. The `bit_size` field must be part of the binary header.

---

### Gap 8: Scheduler Safepoint Protocol — Not Specified

The README says safepoints exist, but the protocol is unspecified. This matters enormously for GC correctness.

**The minimal safepoint contract:**

```
Safepoint locations (where GC may run):
  1. Function call preamble (reduction decrement)
  2. Loop back-edges (if reduction count triggers)
  3. Message receive (process blocked anyway)
  4. Explicit yield (process:erlang.yield/0)

At each safepoint, the mutator guarantees:
  - All live GC roots are in known locations (stack slots or registers listed in the stack map)
  - No in-progress heap allocation (bump pointer is consistent)
  - No partially-constructed objects visible to GC
```

**What to add to `dala_codegen`:** Every call site must emit a stack map entry. Cranelift's `StackMap` API supports this. Without it, the GC's root scanner is guessing.

---

## Part 2 — Performance & Benchmarking Strategy

### Strategy Overview

Benchmarking a compiler+runtime has three distinct concerns:

```
1. Compiler throughput   — how fast does .beam → native take?
2. Runtime performance   — how fast does native code execute vs. BEAM interpreter?
3. GC behaviour          — pause times, allocation rates, promotion rates
```

Each needs different tooling and a different feedback loop.

---

### Benchmark 1: Compiler Throughput

**Goal:** Measure the `dala_beam_loader` + `dala_ir` + `dala_codegen` pipeline end-to-end.

**Setup** (`benches/compile_throughput.rs`):

```rust
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

fn bench_pipeline(c: &mut Criterion) {
    let fixtures = [
        ("tiny",   include_bytes!("fixtures/fib.beam").as_slice()),
        ("medium", include_bytes!("fixtures/json_parser.beam").as_slice()),
        ("large",  include_bytes!("fixtures/phoenix_router.beam").as_slice()),
    ];

    let mut group = c.benchmark_group("compile_pipeline");
    for (name, bytes) in &fixtures {
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(BenchmarkId::new("full", name), bytes, |b, bytes| {
            b.iter(|| {
                let ir    = dala_beam_loader::load_bytes(bytes).unwrap();
                let opt   = dala_ir::optimize(ir);
                let _code = dala_codegen::compile_aot(&opt).unwrap();
            });
        });

        // Also bench each stage in isolation
        group.bench_with_input(BenchmarkId::new("loader_only", name), bytes, |b, bytes| {
            b.iter(|| dala_beam_loader::load_bytes(bytes).unwrap());
        });
    }
    group.finish();
}

criterion_group!(benches, bench_pipeline);
criterion_main!(benches);
```

**What to look for:**
- Loader should be I/O-bound, not CPU-bound — if CPU dominates, the parser has a hot loop
- SSA construction + optimization should be linear in function size
- Codegen (Cranelift) will dominate; track it separately so you can tell if a Cranelift upgrade regresses it

---

### Benchmark 2: Runtime Performance vs. BEAM Interpreter

**Goal:** Quantify the speedup of AOT-compiled code over the OTP interpreter.

**Benchmark matrix:**

| Workload | Why |
|----------|-----|
| `fib(35)` recursive | Pure computation, no allocation |
| `lists:map/2` over 100k list | Allocation-heavy, tests GC interaction |
| Actor ping-pong (1M messages) | Scheduler + message passing |
| Binary pattern match | Tests `bs_*` codegen quality |
| Map operations (insert/lookup 10k keys) | Tests persistent data structure performance |
| Recursive descent parser | Mixed computation + allocation |

**Comparison harness:**

```rust
// benches/vs_beam.rs
// Requires erl_interface or an Erlang port process running OTP
fn bench_fib_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("fib_35");

    // Dala AOT path
    group.bench_function("dala_aot", |b| {
        let module = dala_runtime::load_native("fixtures/fib_native.so").unwrap();
        b.iter(|| module.call("fib", &[Term::from(35)]).unwrap());
    });

    // Dala interpreter path (for mixed-mode baseline)
    group.bench_function("dala_interp", |b| {
        b.iter(|| dala_runtime::interpret("fixtures/fib.beam", "fib", &[Term::from(35)]));
    });

    group.finish();
}
```

**Target:** AOT should be ≥ 3–5× faster than interpreter for pure computation workloads. If it isn't, the codegen is not producing good native code.

---

### Benchmark 3: GC Pause Instrumentation

**Goal:** Measure actual pause times per process, not just throughput.

**Add a GC metrics struct to `dala_runtime`:**

```rust
// dala_runtime/src/gc/metrics.rs
#[derive(Default, Debug)]
pub struct GcMetrics {
    // Young GC
    pub young_collections: u64,
    pub young_pause_ns_total: u64,
    pub young_pause_ns_max: u64,       // worst-case pause
    pub young_bytes_reclaimed: u64,

    // Old GC
    pub old_collections: u64,
    pub old_mark_ns_total: u64,
    pub old_sweep_ns_total: u64,

    // Promotion
    pub objects_promoted_to_old: u64,
    pub objects_promoted_to_sir: u64,
    pub sir_traversal_skips: u64,      // key metric: BSS effectiveness

    // Allocation
    pub alloc_bytes_total: u64,
    pub alloc_fast_path: u64,          // bump pointer hits
    pub alloc_slow_path: u64,          // heap extension needed
}
```

**Expose per-process metrics** so you can identify which actor is causing GC pressure:

```rust
pub fn gc_stats(pid: ProcessId) -> GcMetrics;
pub fn gc_stats_all() -> HashMap<ProcessId, GcMetrics>;
pub fn gc_reset_stats(pid: ProcessId);
```

**The most important metric to watch:** `young_pause_ns_max`. This is what determines whether Dala is suitable for soft-realtime (< 1ms) or hard-realtime workloads.

---

### Benchmark 4: Allocation Micro-Benchmarks

Isolate allocator performance before GC noise interferes:

```rust
fn bench_alloc(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocator");

    // Bump pointer — should be ~2–5 ns
    group.bench_function("bump_alloc_small", |b| {
        let mut heap = YoungHeap::new(1 << 20);
        b.iter(|| {
            let _ = heap.alloc(32, 8).unwrap(); // 32-byte object
        });
    });

    // Arena bulk drop — should be O(1) regardless of object count
    for n in [100, 10_000, 1_000_000] {
        group.bench_function(format!("arena_drop_{n}_objects"), |b| {
            b.iter(|| {
                let mut arena = Arena::new(n * 64);
                for _ in 0..n { let _ = arena.alloc(64, 8); }
                // drop(arena) must be constant time
            });
        });
    }

    group.finish();
}
```

**Targets:**
- Bump pointer alloc: < 5 ns
- Arena drop of 1M objects: < 1 µs (it's just a `munmap` or pointer reset)

---

### Benchmark 5: SIR Effectiveness (BSS Traversal Skip Rate)

This is the novel metric specific to Dala. It measures whether the Stable Immutable Region is actually reducing GC work.

```rust
fn bench_stable_subgraph(c: &mut Criterion) {
    // Simulate an actor with a large stable config map + a changing counter
    let config = build_large_map(10_000); // 10k key config, never changes
    let mut state = ActorState { config, counter: 0 };

    let mut group = c.benchmark_group("gc_stable_subgraph");

    group.bench_function("minor_gc_with_stable_config", |b| {
        b.iter(|| {
            state.counter += 1;
            // Force a minor GC — config should be in SIR and skipped
            trigger_young_gc(&state);
        });
    });

    // Baseline: same test but with SIR disabled
    group.bench_function("minor_gc_without_sir", |b| {
        with_sir_disabled(|| {
            b.iter(|| {
                state.counter += 1;
                trigger_young_gc(&state);
            });
        });
    });

    group.finish();
}
```

**Target:** GC pause with SIR enabled should be 5–10× lower than without when config map is large and stable.

---

### Tooling Setup

**Flamegraph integration** (profile where compile time goes):

```toml
# .cargo/config.toml
[profile.bench]
debug = 1  # keep symbols for perf/flamegraph
```

```bash
# Profile the compile pipeline
cargo flamegraph --bench compile_throughput -- --bench

# Profile runtime (actor-heavy workload)
cargo flamegraph --bin dala_aot -- run --input fixtures/pingpong.beam
```

**Heap profiler** — add `dhat` feature flag for allocation profiling:

```toml
[features]
dhat-heap = ["dhat"]

[dependencies]
dhat = { version = "0.3", optional = true }
```

```rust
#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;
```

**Regression CI** — add a step to GitHub Actions that runs benchmarks and fails if a key metric regresses by more than 10%:

```yaml
- name: Run benchmarks
  run: cargo bench --bench compile_throughput -- --output-format bencher | tee bench_output.txt

- name: Check for regressions
  uses: benchmark-action/github-action-benchmark@v1
  with:
    tool: cargo
    output-file-path: bench_output.txt
    alert-threshold: 110%   # fail if 10% slower
    fail-on-alert: true
    github-token: ${{ secrets.GITHUB_TOKEN }}
```

---

### Benchmarking Roadmap

| Phase | Benchmarks to Add | Blocking On |
|-------|-------------------|-------------|
| Now | Compiler throughput (loader, IR, codegen) | Nothing — just add `.beam` fixtures |
| After GC Tier 2 | Young GC pause, promotion rates | Old heap implementation |
| After SIR | BSS skip rate, stable subgraph pause | SIR admission protocol |
| After Arena | Arena alloc/drop O(1) proof | Arena crate |
| After mixed-mode | AOT vs interpreter speedup | Execution boundary (gap 2) |
| After NIF bridge | NIF call overhead | NIF ABI (gap 6) |
