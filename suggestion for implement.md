BEAM AOT Compiler Architecture (HiPE-inspired, modern design)

What you are trying to build is essentially:

A second execution backend for BEAM

Instead of:

BEAM bytecode
   ↓
Interpreter / JIT
   ↓
CPU

you add:

BEAM bytecode
   ↓
AOT compiler
   ↓
Native ARM64/x86 code

The biggest design mistake people make:

treating this like “compile Erlang to machine code”

It is NOT.

You are actually building:

a BEAM-compatible runtime backend
with native execution

That changes everything architecturally.

1. Core Goals

Your AOT system should target:

Goal	Why
iOS compatibility	No JIT allowed
Deterministic startup	No runtime compilation
Smaller runtime overhead	Better mobile UX
Keep OTP compatibility	Existing ecosystem works
Mixed execution	Interpreter + AOT coexist
Native performance	CPU-intensive workloads
Rust integration	Easier ecosystem integration
2. High-Level Architecture

The clean architecture is:

                ┌────────────────┐
                │ Elixir/Erlang  │
                └───────┬────────┘
                        ↓
                ┌────────────────┐
                │ Core Erlang    │
                └───────┬────────┘
                        ↓
                ┌────────────────┐
                │ BEAM Compiler  │
                └───────┬────────┘
                        ↓
                ┌────────────────┐
                │ .beam files    │
                └───────┬────────┘
                        ↓
         ┌──────────────────────────┐
         │ AOT Native Compiler      │
         │ (Rust)                   │
         └───────┬──────────────────┘
                 ↓
        ┌─────────────────────┐
        │ ARM64/x86 binaries  │
        └─────────┬───────────┘
                  ↓
        ┌─────────────────────┐
        │ BEAM Runtime        │
        │ Scheduler + GC      │
        └─────────────────────┘

Important:

OTP runtime still exists
Scheduler still exists
GC still exists
Process model still exists

Only execution engine changes.

3. Requirements
3.1 VM Compatibility Requirements

You MUST preserve:

Feature	Required
Processes	YES
Message passing	YES
Reductions	YES
Preemption	YES
GC	YES
Pattern matching	YES
Exceptions	YES
Binaries	YES
NIF support	YES
ETS	YES
OTP semantics	YES

Otherwise:

it stops being BEAM-compatible.
4. Major Runtime Components

Your system actually has 5 big subsystems.

4.1 Loader

Responsible for:

reading .beam
extracting bytecode
metadata
literals
imports/exports

Produces:

BeamModule {
    functions,
    literals,
    atoms,
    imports,
    exports,
    debug_info
}

This stage is relatively easy.

4.2 IR Layer (VERY IMPORTANT)

Do NOT compile directly from BEAM instructions.

Create your own SSA IR.

Example:

BEAM
  ↓
Dala SSA IR
  ↓
Optimization
  ↓
Machine code

Why?

Because BEAM opcodes are:

stack/register oriented
difficult to optimize directly

SSA gives:

dead code elimination
constant propagation
inlining
register allocation
loop optimization

This is exactly what modern JITs do.

4.3 Runtime ABI

This is the hardest part.

Your generated code must follow:

BEAM calling convention
process state rules
GC visibility rules

Example process struct:

struct Process {
    heap_ptr: *mut Term,
    stack_ptr: *mut Term,
    reductions: usize,
    mailbox: Mailbox,
    catches: *mut CatchFrame,
}

Every compiled function receives:

fn compiled_fn(proc: *mut Process, args: *const Term)

This is critical.

4.4 Scheduler Integration

Compiled code CANNOT run forever.

You must preserve reductions:

loop:
    reductions -= 1

    if reductions <= 0:
        yield()

Without this:

schedulers break
responsiveness dies
OTP assumptions fail

This is non-negotiable.

4.5 GC Integration

BEAM GC needs:

visible roots
visible stack
visible registers

Before allocations:

save live terms
call allocator
GC may happen
restore terms

This requires:

stack maps
root tracking
safepoints

Very similar to:

JVM
.NET
V8
5. Native Code Strategy

You have 3 realistic options.

Option A — LLVM

Architecture:

BEAM → LLVM IR → native

Pros:

amazing optimization
mature
many targets

Cons:

huge
slow compile
difficult mobile integration
large binary size

Good for:

research
desktop

Not ideal for Dala mobile-first.

Option B — Cranelift (BEST CHOICE)

Architecture:

BEAM → SSA → Cranelift → native

Pros:

fast
simple
Rust-native
mobile friendly
easier embedding

Cons:

fewer optimizations

This is probably your best path.

Option C — Custom Backend

Architecture:

BEAM → custom assembler → machine code

Pros:

maximum control
smallest runtime

Cons:

extremely difficult
multi-arch nightmare

Not recommended initially.

6. Execution Modes

You should support:

Mode	Purpose
Interpreter	fallback/debug
JIT	desktop/Android
AOT	iOS/restricted
Mixed	production

Example:

Module A → native
Module B → interpreted
Module C → JIT

This is VERY important.

7. Function Dispatch Architecture

Do NOT directly call raw addresses everywhere.

Use indirection.

Example:

call export_table[module][function]

Why?
Because this enables:

hot swap
tracing
fallback
patching
instrumentation

HiPE relied heavily on this.

8. Hot Code Upgrade

This is one of the hardest problems.

BEAM assumes:

two module versions
live process migration
code replacement

AOT complicates this massively.

For mobile:

easiest solution is disabling hot upgrade

This is acceptable.

9. Binary Handling

Binaries are critical in BEAM.

Need:

refcounted large binaries
sub-binary support
zero-copy slicing

Your native code must understand:

ProcBin
HeapBin
SubBinary
MatchContext

Otherwise:

performance collapses.
10. Exception Handling

BEAM exceptions are NOT native exceptions.

Need:

catch stack
jump tables
unwind support

Typical model:

throw
  ↓
runtime exception handler
  ↓
restore process state
  ↓
jump to catch label
11. Closures / Fun Support

BEAM closures contain:

module
function
environment

Compiled representation:

struct Fun {
    entry: fn(...),
    env: Vec<Term>,
}

Need:

GC visibility
process-safe execution
12. Native Stack Design

Two choices:

Choice A — Native stack only

Fastest.

But:

difficult GC
difficult tracing
difficult debugging
Choice B — Hybrid stack (BEST)

Recommended:

BEAM stack = canonical
Native stack = temporary

Safer and more compatible.

13. iOS Deployment Model

Your actual deployment becomes:

mix compile
    ↓
beam files
    ↓
dala_aot
    ↓
arm64 objects
    ↓
Xcode static library
    ↓
signed app

No runtime code generation needed.

This fully avoids:

RWX pages
JIT restrictions
App Store rejection
14. Incremental Plan (VERY IMPORTANT)

Do NOT try full BEAM support immediately.

Phase 1 — Minimal Execution

Support:

arithmetic
function calls
tuples
pattern matching

No:

GC
exceptions
binaries

Goal:

native execution proof
Phase 2 — Scheduler Integration

Add:

reductions
yielding
process switching

Now:

real BEAM semantics begin
Phase 3 — GC Support

Add:

heap allocation
root maps
safepoints

Now:

real applications possible
Phase 4 — OTP Compatibility

Add:

binaries
exceptions
funs
ETS
ports

Now:

practical system
Phase 5 — Optimization

Add:

SSA optimizations
inlining
specialization
escape analysis

Now:

performance work
15. Smart Strategic Decision

The smartest thing you can do:

Reuse BEAM frontend

AND

Replace execution backend only

That means:

existing Elixir compiler works
macros work
tooling works
Mix works
ecosystem works

You only own:

runtime
compiler backend

This massively reduces complexity.

16. Realistic Difficulty

This project is:

easier than building a language
harder than building a compiler
basically building a mini JVM/.NET runtime

The hard parts are:

GC
scheduler
compatibility
stack maps
exceptions

NOT instruction translation.

Suggestion from Claude:
Overall Impression
This is an ambitious and well-structured project. The crate decomposition is clean, the choice of Cranelift is well-reasoned, and the phased implementation plan is realistic. The architecture diagram in the README is exactly right — this is a backend replacement, not a reimplementation of Erlang, which is the correct framing. Here are my suggestions organized by priority.

Architecture & Design
The "mixed execution" story needs more thought. You describe AOT-compiled and interpreted code coexisting, but this is the hardest part of any BEAM-compatible backend. When an AOT-compiled function calls an interpreted one (or vice versa), you need a uniform calling convention and a way to hand off process state. HiPE solved this with a "native stub" layer. You should document (or implement) a clear FFI boundary between native and interpreted frames, especially for stack inspection during GC.
GC safepoints with Cranelift are non-trivial. Cranelift doesn't natively support GC stack maps in the way LLVM's gc.statepoint intrinsics do. Your dala_ir crate will need to track every heap pointer live across a call and generate explicit root tables. Make sure you're not relying on conservative scanning — it breaks with copying collectors. Consider defining a StackMap IR node and emitting it as a side table in the object file.
Reduction counting placement matters. Reduction checks need to happen at backedges (loop headers) and function entries. If they're only at function entries, a tight recursive loop can starve the scheduler. Ensure the BEAM loader or IR lowering inserts yield points at all backedges.

Dependency Choices
cranelift version pin is slightly stale. 0.113 is fine, but Cranelift has been evolving quickly. Consider tracking it via a cranelift = { git = ... } for the development branch or bumping to the latest 0.11x release with better AArch64 instruction selection, especially important for iOS deployment.
dashmap = "5" for the dispatch table is a solid choice for concurrent module lookup, but watch out for deadlocks if you ever try to upgrade a module (hot code loading) while holding a read lock inside a NIF or BIF call. Consider a generation-counter + RCU pattern for the hot path.
bincode = "1" is showing its age. bincode 2.x has a completely different API and much better performance. If you're using this for serializing compiled artifacts to disk (which seems likely for the AOT use case), upgrading now is cheaper than migrating later.
Missing from dependencies: object crate (for emitting ELF/Mach-O object files) and gimli/addr2line (for DWARF debug info). Without DWARF, stack traces from AOT-compiled code will be opaque. Even basic line info makes debugging dramatically easier.

Correctness Concerns
Bignum / arbitrary-precision integers. BEAM's integers are arbitrary precision. The README doesn't mention bignums. If your term representation only supports 63-bit tagged integers, you'll get silent wraparound on values that BEAM would handle correctly. You need a fallback to heap-allocated bignums (via num-bigint or a custom implementation).
Binary handling is deferred (Phase 4) but it blocks almost everything. Most real Erlang/Elixir code — even basic HTTP servers — use binaries constantly. The sooner you have at least ref-counted heap binaries and sub-binary slices, the more testable the project becomes. Consider promoting this earlier in the roadmap.
Pattern matching compilation. BEAM's select_val / select_tuple_arity opcodes need to compile to efficient decision trees or jump tables, not chains of comparisons. If your dala_beam_loader is lowering these naively, you'll get correct but very slow code.

License Inconsistency
The README says Apache-2.0 / MIT dual license in one section, then says it changed to MPL-2.0 in another. The Cargo.toml says MPL-2.0. Pick one and remove the contradiction — this matters for downstream users and also for crates.io publishing (if that's ever a goal).

Testing & CI
No CI configuration is visible. For a project this complex, you need at minimum:

cargo test across all crates on every push
A golden-file test suite: compile a known .beam file, run it, assert output matches what the BEAM VM produces
Differential testing: the same Erlang program run on OTP and on Dala, comparing outputs

Consider adding a tests/ directory with a small Erlang corpus — factorial, fibonacci, a simple gen_server — and a shell script that compiles them with erlc then runs them through dala_aot.

Smaller Suggestions

The rust-version = "1.78" in Cargo.toml but edition = "2024" — edition 2024 requires Rust 1.85+. This will cause a build error on 1.78. Bump the MSRV.
Add #[deny(clippy::all)] in each crate and keep the lint clean. For a compiler project, clippy catches real bugs.
The docs/ folder is present but not mentioned in the README. If it has design notes, link to them — it's the most valuable part of a compiler project for contributors.
Consider publishing a dala_ir crate separately. A well-typed SSA IR for BEAM could be useful to the broader community (other AOT/JIT projects, static analyzers, etc.).

----------------

ChatGPT Suggestions:

What You Should Optimize FIRST

Most important insight:

AOT compiler quality matters less than runtime architecture quality.

Meaning:

instruction translation is easy
runtime correctness is hard

So focus architecture first.

1. Biggest Recommendation: Build Your Own SSA IR

If your compiler currently does:

BEAM opcode
   ↓
machine instruction

you should stop and add an IR layer.

You NEED:

BEAM
 ↓
CFG
 ↓
SSA IR
 ↓
Optimization passes
 ↓
Codegen

Without SSA:

optimization becomes nightmare
register allocation becomes messy
architecture portability becomes painful
inlining difficult
escape analysis impossible

Modern runtimes all use SSA:

JVM C2
.NET RyuJIT
V8 TurboFan
LuaJIT IR
Cranelift
BeamAsm internal lowering

This is probably your single most important improvement.

2. Separate Runtime From Compiler

A common early mistake:

compiler owns runtime logic

Bad.

Instead:

Compiler
  ↓
generates calls into
  ↓
Runtime ABI

Example:

call beam_allocate_heap
call beam_send_message
call beam_gc_safe_point

Why?
Because:

easier debugging
easier portability
interpreter + AOT can share runtime
future JIT reuse possible

This is extremely important.

3. Introduce Safepoints EARLY

If you do not already have:

scheduler safepoints
GC safepoints

add them immediately.

Every:

loop
call
allocation
receive

should potentially yield.

Example:

reductions -= 1

if reductions <= 0:
    beam_yield()

Without this:

long-running native code breaks BEAM fairness

This is the #1 thing that makes BEAM “BEAM”.

4. Use Stable Runtime ABI

You need something like:

extern "C" fn(
    proc: *mut Process,
    args: *const Term
) -> Term

Do NOT let generated code invent its own conventions.

Stable ABI enables:

interpreter fallback
mixed execution
hot patching
debugging
tracing
5. Build Proper Term Representation Layer

This is extremely important.

You should centralize:

tagging
boxing
immediate types
tuples
lists
atoms

Example:

enum TermKind {
    SmallInt,
    Atom,
    Tuple,
    Cons,
    Binary,
}

AND

Term = tagged machine word

Do NOT scatter term logic across compiler backend.

Otherwise:

GC integration becomes nightmare
bugs explode later
6. Do NOT Fully Compile All Instructions Initially

Very important.

You should tier your implementation.

Tier 1 — Native Fast Path

Compile:

arithmetic
comparisons
branches
tuple access
simple calls

Fallback:

complex instructions
Tier 2

Add:

binaries
maps
pattern matching
Tier 3

Add:

exceptions
closures
receive
process primitives

This dramatically improves progress speed.

7. Build CFG Explicitly

You NEED:

BasicBlock {
    instructions,
    successors,
    predecessors
}

BEAM bytecode is already near-CFG.

Without CFG:

optimization impossible
liveness impossible
SSA impossible
8. Implement Liveness Analysis Early

Before register allocation.

Need:

live-in
live-out
kill/use sets

Otherwise:

generated code bloats
register spills explode
9. Register Allocation Strategy

Do NOT write graph coloring allocator initially.

Use:

linear scan allocator

Much simpler.
Good enough initially.

Modern JITs often use linear scan.

10. Biggest Future Optimization: Specialization

BEAM is dynamically typed.

Huge wins come from:

specialized versions

Example:

add(integer, integer)

becomes:

ADD x0, x1

instead of generic runtime dispatch.

This is where massive performance appears.

11. Binary Handling Is Critical

You should architect binaries EARLY.

BEAM performance heavily depends on:

sub-binaries
refcounting
zero-copy slicing

Do NOT model binaries like generic blobs.

Need representations like:

HeapBinary
ProcBinary
SubBinary
MatchContext

This is essential.

12. Biggest Architectural Improvement

You should introduce:

Runtime Intrinsics

Instead of compiling everything.

Example:

beam_add
beam_tuple_get
beam_is_atom
beam_binary_match

Generated code calls intrinsics.

Advantages:

easier optimization
easier debugging
portable
interpreter reuse

Very important.

13. Consider Cranelift Instead of Direct ASM

If you currently emit assembly manually:

I strongly recommend moving toward:

BEAM
 ↓
SSA
 ↓
Cranelift IR
 ↓
ARM64/x86

Advantages:

ARM64 easier
x86 easier
register allocation free
relocations free
ABI support free

This saves YEARS.

Especially for:

iOS
Android
macOS ARM
14. iOS-Specific Advice

You likely care about this most.

Best deployment model:

mix compile
  ↓
beam
  ↓
aot compiler
  ↓
.o files
  ↓
static lib
  ↓
Xcode link

Do NOT generate code at runtime.

Even:

mmap executable pages
custom loaders
runtime patching

can trigger App Store problems.

Static signed code is safest.

15. Huge Opportunity: Shared Rust Runtime

You should think bigger:

Instead of:

only AOT

You can build:

Shared Runtime Layer

used by:

interpreter
JIT
AOT
WASM backend

Very similar to:

Dart VM
V8
JVM

This becomes powerful long-term.

16. Something VERY Smart You Can Borrow From BeamAsm

BeamAsm improved performance heavily partly because:

BEAM interpreter dispatch overhead vanished

You can borrow same idea:

Instead of:

generic opcode dispatcher

compile:

traces
hot paths
direct threaded execution

Example:

is_integer
  ↓
add
  ↓
branch

becomes:

one optimized native block

This is huge.

17. My Biggest Strategic Recommendation

You should NOT market this as:

“BEAM compiler”

You should market as:

Alternative BEAM execution backend

That framing is more correct technically.

And architecturally healthier.

18. What I Think Your Strongest Direction Is

Honestly, your strongest angle is probably:

AOT BEAM for mobile

because:

nobody really owns this space
iOS restrictions are real
Flutter/React Native dominate partly because BEAM lacks good mobile story
Dala + AOT BEAM is actually unique

That is strategically very interesting.
