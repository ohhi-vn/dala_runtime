# Dala Runtime — Technical Requirements & Implementation Plan

> A BEAM-inspired actor runtime with hybrid GC, stable subgraph optimization,
> typed layouts, and AI/mobile-first memory architecture.

---

## 1. Project Overview

**Dala** is a new language runtime targeting:
- Mobile (ARM-first)
- AI orchestration / ML pipelines
- UI reactive systems (LiveView-style)
- High-concurrency actor workloads

It is philosophically BEAM-derived but evolves the memory model to handle workloads BEAM was not designed for: large binaries, tensors, embeddings, stable long-lived immutable graphs, and native GPU/ML buffers.

---

## 2. Core Design Principles

| Principle | Detail |
|-----------|--------|
| Actor Isolation | Per-process heaps remain the foundation |
| Immutability-First | All optimizations exploit structural immutability |
| Adaptive GC | No single GC strategy — region + generation + stability |
| Typed Layouts | Set-theoretic types inform GC at compile-time |
| Mobile-Safe | Minimal write barriers; ARM cache-friendly |
| AI-Ready | Region/arena for tensor lifetimes; native buffer support |

---

## 3. Memory Architecture

### 3.1 Memory Spaces (4-tier)

```
┌──────────────────────────────────────────┐
│         Per-Process Young Heap           │  ← BEAM-style copying GC
│  fast bump allocation, tiny, isolated    │
└────────────────┬─────────────────────────┘
                 │ promotion
┌────────────────▼─────────────────────────┐
│           Per-Process Old Heap           │  ← concurrent mark + incremental sweep
│  longer-lived process-local structures   │
└────────────────┬─────────────────────────┘
                 │ stability detection
┌────────────────▼─────────────────────────┐
│        Stable Immutable Region (SIR)     │  ← blackened stable subgraphs
│  immutable graphs, config maps, UI trees │
│  rarely/never rescanned                  │
└──────────────────────────────────────────┘
┌──────────────────────────────────────────┐
│     Large Object / Native Buffer Space   │  ← refcount + arena
│  tensors, binaries, GPU buffers, arenas  │
└──────────────────────────────────────────┘
```

### 3.2 Young Heap (Tier 1)

**Algorithm:** Copying GC (semi-space)
**Trigger:** Heap exhaustion or actor yield point

Requirements:
- Bump-pointer allocator; O(1) allocation
- Semi-space copy on minor GC
- Roots: process stack, registers, mailbox references
- Survive threshold: configurable (default 2–3 collections)
- Process-local — zero global coordination
- Cache-line aligned object headers
- NO write barriers into same tier (immutability guarantee)

**Target pause:** < 500 µs per process

### 3.3 Old Heap (Tier 2)

**Algorithm:** Concurrent tri-color mark + incremental sweep
**Trigger:** Promotion from young heap

Requirements:
- Tri-color marking state per object header (2 bits)
- Concurrent marker thread per scheduler thread
- Incremental sweeper — interleaved with mutator
- Write barrier: only young→old references (remembered set)
- Stability counter per object: incremented on each survived GC
- Objects reaching `stability_threshold` (configurable, e.g. 5) become promotion candidates to SIR

**Target pause:** < 2 ms (incremental slices)

### 3.4 Stable Immutable Region — SIR (Tier 3)

**Algorithm:** Blackened Stable Subgraph (BSS) — no tracing, reference-counted roots only

Requirements:
- Objects enter SIR only when:
  - Survived N old-gen GC cycles
  - Graph contains no mutable pointers
  - Compiler-verified OR runtime-detected immutable
- Once in SIR: color = `stable-black` permanently
- Young GC and old GC skip deep traversal of stable-black graphs
- SIR roots tracked via a simple reference table (not pointer scanning)
- Optional: compact native layout conversion on SIR promotion
- Cross-region pointers: SIR→Young tracked in barrier; Young→SIR skipped (immutable)
- Eviction: aging heuristic — if not referenced after M collections, demote back to old heap

**Stability detection heuristics:**
- Survival counter ≥ threshold
- No writes detected (immutability marker)
- Graph size above minimum (avoid trivial promotions)
- Optionally: compiler annotation `@stable`

### 3.5 Large Object / Native Buffer Space (Tier 4)

**Algorithm:** Reference counting + arena/region allocation

Requirements:
- Large binaries (> 64 bytes default): always allocated here, shared via refcount
- Tensor buffers: arena-scoped, allocated per inference request
- GPU/native handles: RAII wrappers with Rust-style drop semantics
- Arena allocator API: `arena_new()`, `arena_alloc()`, `arena_drop()` — O(1) mass deallocation
- Arenas owned by actor or request scope — no tracing needed
- Actor request pattern:
  ```
  arena = arena_new()
  process_message(arena)
  arena_drop(arena)   ← entire region freed in O(1)
  ```

---

## 4. Blackened Stable Subgraph (BSS) — Detailed Spec

### 4.1 Object States

| Color | Meaning | Location |
|-------|---------|----------|
| White | Unvisited / dead | Young/Old heap |
| Gray | Discovered, scan pending | Old heap (GC cycle) |
| Black (transient) | Fully scanned this cycle | Old heap |
| **Stable-Black** | Permanently live, immutable, skip traversal | SIR |

### 4.2 Promotion Protocol

```
1. Object survives young GC → increment survival_count
2. survival_count >= YOUNG_THRESHOLD → promote to old heap
3. In old heap: track stability_count (survived old GC cycles)
4. stability_count >= OLD_THRESHOLD AND immutable_flag == true
   → candidate for SIR promotion
5. SIR admission check:
   a. Walk subgraph: confirm no mutable children
   b. Confirm no young-heap references within subgraph
   c. If pass: move to SIR, set color = stable-black
   d. Register in SIR root table
6. Future GC: when root encountered → skip subgraph traversal
```

### 4.3 Write Barrier Rules

| Pointer Direction | Barrier Needed | Reason |
|-------------------|---------------|--------|
| Young → Young | None | Same space copying |
| Young → Old | Yes (remembered set) | Old GC must know about young refs |
| Young → SIR | None | SIR is immutable |
| Old → Young | Yes (remembered set) | Young GC roots |
| Old → SIR | None | SIR is immutable |
| SIR → anything | Forbidden (immutable) | SIR objects cannot mutate |

### 4.4 Eviction from SIR

- If no living process holds a reference to a SIR root for M cycles → free
- Refcount on SIR roots (not graph nodes) — cheap
- Dead SIR objects bulk-freed by region deallocation (no per-object tracing)

---

## 5. Typed Layout System

### 5.1 Compiler Requirements

The compiler must emit, per type:

```
TypeDescriptor {
  size: usize,
  pointer_map: Bitmap,       // which fields are GC pointers
  immutable: bool,           // set-theoretic immutability proof
  stable_hint: bool,         // compiler annotation
  native_layout: Option<NativeTypeInfo>  // for SIR compaction
}
```

### 5.2 GC Interaction

- Pointer map eliminates conservative scanning
- `immutable: true` enables BSS promotion without runtime walk
- `native_layout` allows SIR objects to be compacted into flat, cache-friendly structures
- Non-pointer fields skipped during marking entirely

### 5.3 Example Types

```
// Compiler infers:
//   immutable = true
//   stable_hint = true (large, deep map)
@type Config :: %{
  routes: map(string, handler),
  schema: Schema.t(),
  metadata: binary()
}

// GC sees: stable candidate after few cycles
// BSS promotion: entire Config graph → SIR
// Future GCs: one pointer check, no traversal
```

---

## 6. Scheduler Integration

Requirements:
- GC safepoints at: actor yield, message receive, function calls (configurable interval)
- Young GC runs within scheduler thread of owning process — no cross-thread pause
- Old GC marker: background thread per scheduler
- SIR promotion: lazy, triggered during old GC cycle
- Arena drops: synchronous, O(1) — no scheduler impact
- GC work stealing: old-gen marker threads may assist each other (work queue)

**Scheduler must expose:**
- `gc_young(process_id)`
- `gc_old_cycle(scheduler_id)`
- `gc_sip_admit(object_ref)` — SIR promotion
- `arena_drop(arena_id)`

---

## 7. Native / AI Buffer Integration

### 7.1 Tensor Lifecycle

```
Inference Request
  ↓ arena_new()
  ↓ allocate input tensors
  ↓ allocate intermediate activations
  ↓ run inference
  ↓ extract output (copy to process heap if needed)
  ↓ arena_drop()   ← all temporaries freed O(1)
```

### 7.2 Static AI Metadata (BSS Candidate)

- Tokenizer vocab trees → SIR after first use
- Model graph metadata → SIR
- Embedding index structures → SIR
- Only dynamic activations use arena

### 7.3 GPU Buffer Handle

```
NativeBuffer {
  handle: *opaque,
  size: usize,
  drop_fn: fn(*opaque),   // platform dealloc
  refcount: AtomicUsize
}
```

GC treats NativeBuffer as:
- Opaque leaf node (no pointer children)
- Refcounted — GC decrements on last reference
- Drop function called when refcount == 0

---

## 8. Technical Requirements Summary

### 8.1 Functional Requirements

| ID | Requirement |
|----|-------------|
| FR-01 | Per-process bump-pointer young heap with semi-space copying GC |
| FR-02 | Per-process old heap with concurrent tri-color mark + incremental sweep |
| FR-03 | Stable Immutable Region (SIR) with stable-black subgraph classification |
| FR-04 | Write barrier only for young→old and old→young cross-references |
| FR-05 | Arena allocator with O(1) bulk deallocation |
| FR-06 | Native buffer API with refcounting and drop callbacks |
| FR-07 | Typed layout descriptors emitted by compiler (pointer map, immutability flag) |
| FR-08 | Survival counters on each object for tier promotion decisions |
| FR-09 | SIR admission walk: verify no mutable children before promotion |
| FR-10 | SIR root reference table (not pointer scanning) |
| FR-11 | Compiler annotation support: `@stable`, `@arena`, `@native` |
| FR-12 | Safepoint-based GC — no async interruption |
| FR-13 | Per-scheduler old-gen marker thread |
| FR-14 | GC introspection API for profiling/tuning |

### 8.2 Non-Functional Requirements

| ID | Requirement | Target |
|----|-------------|--------|
| NFR-01 | Young GC pause per process | < 500 µs |
| NFR-02 | Old GC incremental slice pause | < 2 ms |
| NFR-03 | Arena allocation/deallocation | O(1) |
| NFR-04 | Young heap allocation | O(1) bump pointer |
| NFR-05 | Write barrier overhead | < 3% throughput impact |
| NFR-06 | SIR traversal reduction | ≥ 60% for stable workloads |
| NFR-07 | ARM mobile CPU cache efficiency | Objects cache-line aligned |
| NFR-08 | Zero global stop-the-world | Required |
| NFR-09 | Process spawn/destroy overhead | Unchanged from BEAM baseline |

---

## 9. Implementation Plan

### Phase 0 — Foundations (Weeks 1–4)
- [ ] Define object header format (color bits, survival counter, type descriptor pointer, flags)
- [ ] Implement bump-pointer allocator
- [ ] Implement per-process heap structure (young semi-space)
- [ ] Basic copying GC (semi-space flip)
- [ ] Process isolation model: spawn, message-pass, destroy
- [ ] Test harness and GC correctness suite

### Phase 1 — Young Heap GC (Weeks 5–8)
- [ ] Root scanning (stack + registers + mailbox)
- [ ] Semi-space copying with forwarding pointers
- [ ] Survival counter tracking
- [ ] Young→Old remembered set (basic card table)
- [ ] Promotion to old heap stub
- [ ] Benchmarks: allocation throughput, pause time

### Phase 2 — Old Heap + Concurrent Marker (Weeks 9–14)
- [ ] Old heap allocator (free-list or region-based)
- [ ] Tri-color object state (2-bit field in header)
- [ ] Gray worklist (per-scheduler marker thread)
- [ ] Concurrent marking with safepoint coordination
- [ ] Incremental sweeping (interleaved with mutator)
- [ ] Write barrier implementation (card table dirty tracking)
- [ ] Benchmarks: old-gen pause, marker throughput

### Phase 3 — Stable Immutable Region (Weeks 15–22)
- [ ] SIR memory region allocator
- [ ] Stability detection: survival threshold, immutability check
- [ ] SIR admission protocol (subgraph walk + immutability validation)
- [ ] Stable-black state: GC traversal skip logic
- [ ] SIR root table (lightweight refcount)
- [ ] SIR eviction / aging heuristic
- [ ] Benchmarks: traversal reduction on stable workloads (maps, UI trees)

### Phase 4 — Arena & Native Buffers (Weeks 23–28)
- [ ] Arena allocator: `new`, `alloc`, `drop` — O(1) deallocation
- [ ] Actor-scoped and request-scoped arena lifecycle
- [ ] NativeBuffer handle: refcount + drop_fn
- [ ] GPU/tensor buffer integration API
- [ ] Arena drop correctness: ensure GC roots cleared on drop
- [ ] Benchmarks: inference pipeline, binary-heavy workloads

### Phase 5 — Typed Layouts (Weeks 29–35)
- [ ] TypeDescriptor format: pointer bitmap, immutability, native layout
- [ ] Compiler integration: emit descriptors per type
- [ ] GC scanning uses pointer bitmap (eliminate conservative fallbacks)
- [ ] Compiler `@stable` / `@arena` annotations
- [ ] SIR native layout compaction (flatten stable-black to packed layout)
- [ ] Benchmarks: pointer-map scan vs conservative scan

### Phase 6 — Scheduler Integration & Hardening (Weeks 36–42)
- [ ] Safepoint insertion in scheduler yield, message receive, call sites
- [ ] GC work stealing between scheduler marker threads
- [ ] GC profiling API (per-process stats, SIR hit rate, arena usage)
- [ ] Stress tests: millions of actors, large binaries, AI pipeline simulation
- [ ] Pause time regression suite
- [ ] Documentation: GC tuning guide

---

## 10. Risk Register

| Risk | Impact | Mitigation |
|------|--------|------------|
| Write barrier CPU overhead too high | High | Profile on ARM early; consider epoch-based alternatives |
| False stability promotion (objects promoted then die) | Medium | Aging heuristics + configurable thresholds |
| SIR admission walk too expensive for large graphs | Medium | Cap walk depth; use compiler immutability proof instead |
| Concurrent marker correctness (missed objects) | High | Snapshot-at-beginning (SATB) write barrier variant |
| Arena drop with dangling GC roots | Critical | Root clearing protocol before arena_drop() |
| TypeDescriptor incompatibility across compiler versions | Medium | Versioned descriptor format; runtime validation |

---

## 11. Key Metrics to Track

- **Young GC pause** (per process, p50/p99/p999)
- **Old GC incremental slice duration**
- **SIR hit rate** (% of GC roots skipped due to stable-black)
- **Traversal reduction** (nodes visited this GC vs. total live nodes)
- **Write barrier overhead** (% of CPU in barrier code)
- **Arena deallocation time** (should be O(1) / flat)
- **Process spawn/kill overhead** (regression vs. BEAM baseline)
- **Peak RSS** (memory overhead from GC metadata)
