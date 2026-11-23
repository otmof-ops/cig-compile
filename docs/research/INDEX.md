# The studies, entry by entry

Made from the studies by `join.py`. A verdict is what the study proposes for cig; none is decided until its measurement exists.

| study | entries | take | adapt | reject | not applicable |
|---|---:|---:|---:|---:|---:|
| [Cranelift: its IR and its acyclic e-graph mid-end](studies/01-cranelift-ir-and-mid-end.md) | 20 | 4 | 16 | 0 | 0 |
| [Cranelift's back end and regalloc2](studies/02-cranelift-back-end-and-regalloc2.md) | 20 | 14 | 5 | 1 | 0 |
| [Wasmtime as an ahead-of-time embedder of Cranelift, and Winch](studies/03-wasmtime-ahead-of-time-and-winch.md) | 19 | 1 | 15 | 2 | 1 |
| [SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers](studies/04-spidermonkey-cacheir-and-warp.md) | 20 | 4 | 16 | 0 | 0 |
| [Lean 4, Koka and Lobster: reference counting made cheap](studies/05-reference-counting-lean-koka-lobster.md) | 20 | 3 | 11 | 6 | 0 |
| [Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR](studies/06-swift-ownership-and-arc-roc.md) | 18 | 1 | 14 | 3 | 0 |
| [LLVM's optimization pipeline, and Alive2's translation validation](studies/07-llvm-pipeline-and-alive2.md) | 17 | 2 | 14 | 0 | 1 |
| [Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries](studies/08-go-a-small-fast-compiler.md) | 20 | 0 | 15 | 3 | 2 |
| [Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization](studies/09-types-julia-dotnet-rustc.md) | 15 | 2 | 12 | 1 | 0 |
| [MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR](studies/10-whole-program-mlton-and-unboxing-ghc.md) | 17 | 0 | 17 | 0 | 0 |
| [How a dynamic value is laid out, across twelve implementations](studies/11-the-value-layout.md) | 15 | 5 | 8 | 2 | 0 |
| [V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end](studies/12-v8-and-javascriptcore-tiers.md) | 20 | 2 | 15 | 3 | 0 |
| [Proving the compiler right: Csmith, YARPGen, C-Vise, Cranelift's fuzzing and verifier, CompCert and diverse double-compiling](studies/13-proving-the-compiler-right.md) | 17 | 4 | 12 | 0 | 1 |
| [Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache](studies/14-compiling-fast.md) | 16 | 1 | 14 | 1 | 0 |
| [Profiles that outlive a run, and code compiled ahead of time from them: ART, .NET, and Leyden](studies/15-profiles-and-ahead-of-time-caches.md) | 18 | 9 | 9 | 0 | 0 |
| [The shape of an IR: nanopass and Chez Scheme, Zig's two IRs, the sea of nodes and leaving it, flambda at a distance](studies/16-ir-shapes-nanopass-zig-sea-of-nodes.md) | 16 | 1 | 13 | 2 | 0 |
| [From machine code to a program: object emission, embedded linking, size, debug tables, hardening, targets and the bootstrap](studies/17-from-machine-code-to-a-program.md) | 20 | 9 | 10 | 1 | 0 |
| **all 17** | **308** | **62** | **216** | **25** | **5** |

## Cranelift: its IR and its acyclic e-graph mid-end

[studies/01-cranelift-ir-and-mid-end.md](studies/01-cranelift-ir-and-mid-end.md)

| | mechanism | for cig | feeds |
|---:|---|---|---|
| 1 | Block parameters in place of phi nodes | take | cig-compile#32, cig-compile#33, (e) |
| 2 | Data-flow graph kept apart from layout | adapt | cig-compile#33, cig-compile#37, (e), (i) |
| 3 | Side-data rides on an instruction: source locations, memory flags, trap codes, exception tables, stack maps, debug tags | adapt | cig-compile#32, cig-compile#38, (e) |
| 4 | Variables to SSA at the front end (Braun et al. 2013) | adapt | cig-compile#32, (e) |
| 5 | The verifier: the invariant checker between passes | take | cig-compile#33, (e) |
| 6 | Pure nodes against the skeleton | adapt | cig-compile#33, (e), (i) |
| 7 | The rewrite loop, its bounds and its cost function | adapt | cig-compile#33, cig-compile#37, (e) |
| 8 | GVN and LICM are not passes, they are elaboration | adapt | cig-compile#33, (e) |
| 9 | Rematerialization: the cheap op computed where it is used | adapt | cig-compile#32, cig-compile#33, (e), (i) |
| 10 | `subsume`: a rewrite that drops a use | adapt | cig-compile#33, (e) |
| 11 | ISLE: a rule language compiled to Rust, with overlap and priority checked | adapt | cig-compile#33, (e) |
| 12 | How a rewrite is tested: one CLIF file per rule, filecheck pins the output | take | cig-compile#33, cig-compile#39, (e) |
| 13 | Alias analysis: one walk, four optimizations on the skeleton | adapt | cig-compile#32, cig-compile#33, cig-compile#35, (e), (g) |
| 14 | The inliner is a library; the embedder is the policy | adapt | cig-compile#32, cig-compile#33, (e) |
| 15 | `Context::optimize`: the pipeline and its one gate | adapt | cig-compile#32, cig-compile#33, cig-compile#37, (f) |
| 16 | The incremental cache: one per-function key, one serialized compiled stencil | take | cig-compile#37, cig-compile#32, (f) |
| 17 | Lowering structured SSA with slots into CFG with block parameters | adapt | cig-compile#32, (e), (i) |
| 18 | The dividing line: who runs which of cig-compile#33's passes, and why | adapt | cig-compile#32, cig-compile#33, cig-compile#37, (e), (i) |
| 19 | The "a" in aegraph: an acyclic e-graph that never saturates | adapt | cig-compile#33, (e) |
| 20 | Chaos mode: a pseudo-randomness tap for proving that heuristics are only heuristics | adapt | cig-compile#33, cig-compile#39, (e) |

## Cranelift's back end and regalloc2

[studies/02-cranelift-back-end-and-regalloc2.md](studies/02-cranelift-back-end-and-regalloc2.md)

| | mechanism | for cig | feeds |
|---:|---|---|---|
| 1 | Lowering is a backward scan with instruction colors | adapt | cig-compile#32, (e) |
| 2 | The ISLE lowering back end: pattern-matched rules per target | adapt | cig-compile#32, (e) |
| 3 | One function at a time: `compile` drives lower-regalloc-check-emit, with edge blocks materialised | take | cig-compile#32, cig-compile#37, (e) |
| 4 | `MachBuffer`: single-pass emission with peephole simplification, veneers and islands | take | cig-compile#32, cig-compile#41, (d), (e) |
| 5 | Side-tables beside the bytes: relocations, traps, call sites, srclocs | take | cig-compile#32, cig-compile#38, (d) |
| 6 | ABI framework: `ABIMachineSpec` and the per-target `MachineEnv` | take | cig-compile#32, cig-compile#41, (a), (e) |
| 7 | Calling conventions, cig's value size, and the sret fallback | adapt | cig-compile#32, cig-compile#35, (a), (d) |
| 8 | Tail calls: `return_call` and the `Tail` convention | adapt | cig-compile#32, (e) |
| 9 | `try_call` and exception tables | take | cig-compile#32, (d) |
| 10 | Stack probes, frame pointers, and the pinned register | take | cig-compile#32, cig-compile#40, (d), (e) |
| 11 | regalloc2's input: the `Function` trait and `Operand` | take | cig-compile#32, (e) |
| 12 | Ion: the backtracking allocator | take | cig-compile#32, cig-compile#37, (e) |
| 13 | Fastalloc, and the `regalloc_algorithm` setting that picks it | adapt | cig-compile#32, cig-compile#37, (e) |
| 14 | The symbolic checker: regalloc2 proves its own output | take | cig-compile#32, cig-compile#39, (d), (e) |
| 15 | Unwind info: one abstract prologue, two formats | take | cig-compile#32, cig-compile#38, (d), (e) |
| 16 | User stack maps: GC roots the front end names, the back end records | reject | cig-compile#32, (e), (g) |
| 17 | Object files: `cranelift-module` and `cranelift-object` glue many functions into one `.o` | take | cig-compile#32, cig-compile#41, (e), (f) |
| 18 | JIT memory and W^X | take | cig-compile#32, (e), (f) |
| 19 | Hardening settings: Spectre, BTI, pointer authentication, NaN canonicalization | take | cig-compile#32, cig-compile#40, cig-compile#41, (d), (e) |
| 20 | Compile time: `timing` names every phase, `opt_level` is the gate | take | cig-compile#32, cig-compile#37, (e), (f) |

## Wasmtime as an ahead-of-time embedder of Cranelift, and Winch

[studies/03-wasmtime-ahead-of-time-and-winch.md](studies/03-wasmtime-ahead-of-time-and-winch.md)

| | mechanism | for cig | feeds |
|---:|---|---|---|
| 1 | The compiled artifact: an ELF with Wasmtime sections | adapt | cig-compile#32, cig-compile#36, (e), (f) |
| 2 | Sections that are decoded in place, not parsed | adapt | cig-compile#32, cig-compile#36, cig-compile#37, (e) |
| 3 | Relocations: which are resolved when the image is built | adapt | cig-compile#32, cig-compile#40, (e) |
| 4 | Reproducible builds, and the incremental Cranelift cache | adapt | cig-compile#32, cig-compile#37, (e), (f) |
| 5 | Loading without a page that is both writable and executable | adapt | cig-compile#32, cig-compile#40, cig-script#256, (e) |
| 6 | The compatibility header: what is checked before a file is trusted | adapt | cig-compile#32, cig-compile#36, cig-compile#40, cig-script#256 |
| 7 | Fuel: an operator-cost table and a function-local counter | reject | cig-compile#32, cig-script#256, (d) |
| 8 | The fuel check, and the `out_of_gas` builtin | adapt | cig-compile#32, cig-script#256, (d) |
| 9 | Epochs: a cheap check against a cached deadline | reject | cig-compile#32, (d) |
| 10 | Trap encoding: a native pc becomes a trap code | adapt | cig-compile#32, cig-compile#40, (d), (e) |
| 11 | Two ways a trap is raised: signals, or a libcall | adapt | cig-compile#32, cig-compile#40, cig-script#256, (d) |
| 12 | The builtin table on vmctx: calls into the runtime without relocations | adapt | cig-compile#32, (e) |
| 13 | Address maps: a native pc becomes a source position | take | cig-compile#32, cig-compile#36, cig-compile#38, (e) |
| 14 | Unwinding and the exception table: handlers without stack walking | adapt | cig-compile#32, cig-compile#40, (d), (e) |
| 15 | Stack maps: live roots at each callsite | adapt | cig-compile#32, cig-compile#38, cig-script#256, (e) |
| 16 | The on-disk cache: a hashed key and an LRU worker | adapt | cig-compile#37, cig-script#256, (f) |
| 17 | Winch: one pass, a shadow stack, a bitset allocator | adapt | cig-compile#32, cig-script#256, (b), (d), (e) |
| 18 | Pulley: a bytecode interpreter for a 32-bit ARM target | not applicable | cig-compile#32, cig-compile#41 |
| 19 | Profilers and debuggers: perf jitdump and the GDB JIT interface | adapt | cig-compile#32, cig-compile#38, cig-compile#40, cig-script#256 |

## SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers

[studies/04-spidermonkey-cacheir-and-warp.md](studies/04-spidermonkey-cacheir-and-warp.md)

| | mechanism | for cig | feeds |
|---:|---|---|---|
| 1 | CacheIR is a linear bytecode of guards, idempotent loads and one result op | adapt | cig-compile#32, cig-script#256, (b), (h) |
| 2 | Each op declares itself in a YAML table that drives code generation | take | cig-compile#32, cig-compile#33, (b), (c) |
| 3 | Stub data lives apart from stub code, so one piece of code serves many sites | adapt | cig-compile#32, cig-compile#36, (b), (f) |
| 4 | OperandId types carry the type that a guard has already proved | adapt | cig-compile#34, cig-compile#35, (b), (i) |
| 5 | The generators look at actual values and emit guards then an action | adapt | cig-compile#32, cig-compile#34, (b), (c) |
| 6 | A shape is a one-word summary of a map's layout that guards can check | adapt | cig-compile#35, cig-compile#34, (h) |
| 7 | Shared code for baseline, Ion's own caches and the macro assembler | adapt | cig-compile#32, cig-script#256, (b), (e) |
| 8 | Warp turns baseline's IR into optimizer IR with a transpiler | adapt | cig-compile#32, cig-compile#33, cig-script#256, (b), (c) |
| 9 | The interpreter reads stubs with no native code: this is cig's situation | take | cig-compile#36, cig-script#256, (b), (c) |
| 10 | Ahead-of-time stubs: a corpus baked into the runtime | take | cig-compile#36, cig-script#256, (b), (f) |
| 11 | A site has three modes and a hard stub count | adapt | cig-compile#33, cig-compile#34, (b), (c) |
| 12 | Stub folding: several shape guards become one | adapt | cig-compile#33, cig-script#256, (b), (h) |
| 13 | Health scoring: a per-op cost that reads out a site's happiness | adapt | cig-compile#33, cig-compile#38, (b), (c) |
| 14 | Counters and the next tier live together in ICScript | adapt | cig-compile#33, cig-script#256, cig-script#243, (b), (c) |
| 15 | Trial inlining gives a call site its own copy of the callee's caches | adapt | cig-compile#33, cig-script#256, (b), (c) |
| 16 | The snapshot is taken on the main thread, from plain data | take | cig-compile#32, cig-script#256, (b), (e) |
| 17 | Bailouts rebuild the lower tier's frame from snapshot plus recover instructions | adapt | cig-compile#32, cig-script#256, cig-script#243, (b), (d) |
| 18 | Invalidation keeps a function from bailing forever | adapt | cig-compile#32, cig-compile#33, cig-script#256, (b), (d) |
| 19 | CacheIRCompiler's register allocator is a stub-wide SSA scheme | adapt | cig-compile#32, cig-script#256, (b), (e) |
| 20 | Fuses let a hot guard leave the IR, then invalidate the code if it ever pops | adapt | cig-compile#33, cig-script#256, (b), (c), (h) |

## Lean 4, Koka and Lobster: reference counting made cheap

[studies/05-reference-counting-lean-koka-lobster.md](studies/05-reference-counting-lean-koka-lobster.md)

| | mechanism | for cig | feeds |
|---:|---|---|---|
| 1 | Lean's explicit reference-counting pass, inserted after a liveness analysis over owned and borrowed variables | adapt | cig-compile#33, cig-compile#35, (a), (g) |
| 2 | Koka's Perceus pass, with dup and drop placed precisely at last use, walked in reverse | adapt | cig-compile#33, cig-compile#35, cig-script#243, (a), (g) |
| 3 | Lobster's compile-time lifetimes: LT_BORROW, LT_KEEP, LT_ANY on every AST node | reject | cig-compile#33, cig-compile#35, (g) |
| 4 | Lean's borrow inference by fixed-point dataflow over an SCC | adapt | cig-compile#33, cig-compile#35, (g) |
| 5 | Koka's borrow signatures carried with the module, consulted at every call | adapt | cig-compile#33, cig-compile#35, (g) |
| 6 | Lean's reset-reuse: the freed cell of a matched constructor fed to the next same-shape allocation | reject | cig-compile#35, (g) |
| 7 | Lean's reset-reuse expansion into hot and cold paths with dynamic derived borrows | reject | cig-compile#35, (g) |
| 8 | Koka's reuse analysis: a dropped allocation of size N feeds the next same-size allocation | reject | cig-compile#35, cig-compile#33, (g) |
| 9 | Koka's reuse specialization: skip the field store when the value written matches the pattern | reject | cig-compile#35, (g) |
| 10 | Koka's drop specialization: a dropped value, branched on uniqueness, with children dup'd on the shared path | adapt | cig-compile#33, cig-compile#35, (g) |
| 11 | The object header: Lean's 32-bit count with sticky bands, Koka's 32-bit count with unique = 0 | adapt | cig-compile#35, cig-compile#40, (a), (g) |
| 12 | Freeing a deep structure without deep recursion: the header rewritten into a worklist pointer | adapt | cig-compile#35, cig-compile#40, (a), (g) |
| 13 | The fast path of a decrement is: "if count > 1, decrement and return; else cold path" | take | cig-compile#35, (a), (g) |
| 14 | Koka's fip and fbip checks: static guarantees of in-place functional programming | reject | cig-compile#33, cig-compile#35, (g) |
| 15 | The "count is one" check as the gate for in-place writes to strings, lists and maps | take | cig-compile#35, cig-script#235, (g) |
| 16 | What cig's mutable, shared `Rc<RefCell<..>>` containers change: most reuse is gone | adapt | cig-compile#35, (g) |
| 17 | Derived borrows: a projection of a borrowed value is itself borrowed | adapt | cig-compile#33, cig-compile#35, (g) |
| 18 | Coalescing inc and dec operations within a basic block | adapt | cig-compile#33, (g) |
| 19 | Dead-code elimination of pure let-bindings as a prerequisite for counts | take | cig-compile#33, (g) |
| 20 | Propagating user-annotated borrows forward with a four-point lattice | adapt | cig-compile#33, cig-compile#35, (g) |

## Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR

[studies/06-swift-ownership-and-arc-roc.md](studies/06-swift-ownership-and-arc-roc.md)

| | mechanism | for cig | feeds |
|---:|---|---|---|
| 1 | The ownership kinds and the instructions that carry them | adapt | cig-compile#33, cig-compile#35, (a), (e), (g) |
| 2 | What the SIL ownership verifier proves, on every value, on every path | take | cig-compile#33, (e), (g) |
| 3 | Function parameter and result conventions | adapt | cig-compile#33, cig-compile#35, (e), (g) |
| 4 | Lexical lifetimes and deinit barriers: what may never move | adapt | cig-compile#33, (e), (g) |
| 5 | OSSA-era copy elision: why a `copy_value` of a `Guaranteed` can just vanish | adapt | cig-compile#33, cig-compile#35, (g) |
| 6 | The older ARC sequence dataflow, and why ownership replaced most of it | reject | cig-compile#33, (e), (g) |
| 7 | Shortening a lifetime to its last use, and what forbids it | adapt | cig-compile#33, (e), (g) |
| 8 | Changing an owned parameter to guaranteed, lifting a retain/release out of the call pair | adapt | cig-compile#33, (g), (i) |
| 9 | Copy on write and the uniqueness primitive | adapt | cig-compile#35, cig-compile#33, (a), (g) |
| 10 | Hoisting a uniqueness check out of a loop: COWArrayOpt | adapt | cig-compile#33, cig-compile#35, (a), (g) |
| 11 | Roc's reference-count insertion with borrow inference | adapt | cig-compile#33, cig-compile#35, cig-script#243, (a), (g) |
| 12 | Roc's reset/reuse and drop specialization: Perceus's deeper passes | reject | cig-compile#33, cig-compile#35, (g) |
| 13 | Roc's alias analysis and in-place list updates | adapt | cig-compile#33, cig-compile#35, cig-script#243, (b), (g) |
| 14 | The smallest ownership vocabulary that would let a CIR pass remove a clone/drop pair, with the verifier able to prove it | adapt | cig-compile#33, cig-compile#35, cig-script#243, (a), (e), (g) |
| 15 | Where cig's shared slots defeat the Swift model, and what Swift does about the same problem | adapt | cig-compile#33, cig-compile#35, (e), (g), (i) |
| 16 | Copy-on-write uniqueness as a VM primitive for cig's lists and maps | adapt | cig-compile#35, cig-compile#33, cig-script#243, (a), (g) |
| 17 | Move-only values and `drop_deinit` | reject | cig-compile#33, (g), (i) |
| 18 | Stack promotion of non-escaping allocations | adapt | cig-compile#33, cig-compile#35, (a), (g) |

## LLVM's optimization pipeline, and Alive2's translation validation

[studies/07-llvm-pipeline-and-alive2.md](studies/07-llvm-pipeline-and-alive2.md)

| | mechanism | for cig | feeds |
|---:|---|---|---|
| 1 | The pipeline's shape, a module through the default build | adapt | cig-compile#33, (e) |
| 2 | The bottom-up walk and the inliner's module wrapper | adapt | cig-compile#33, (e) |
| 3 | The function simplification pipeline, in order, with reasons | adapt | cig-compile#33, (c) |
| 4 | `-O0`, `-O1`, `-O2`, `-O3`, `-Os`, `-Oz`: what each changes | adapt | cig-compile#33, cig-compile#37 |
| 5 | The late module-optimization stage and why `LoopSink` is last | adapt | cig-compile#33, cig-compile#37 |
| 6 | SROA, then Mem2Reg: how a frontend's memory becomes SSA | not applicable | (a), (g) |
| 7 | EarlyCSE then GVN: cheap redundancy first, exhaustive redundancy later | adapt | cig-compile#33, (c) |
| 8 | LICM's safety test, and why there are two LICM invocations around LoopRotate | take | cig-compile#33, (d) |
| 9 | InstCombine: a worklist of rewrites that strictly simplify | adapt | cig-compile#33 |
| 10 | The inliner's cost model: thresholds and bonuses | adapt | cig-compile#33, (a) |
| 11 | The pass manager: analyses, invalidation, and what `PreservedAnalyses` says | adapt | cig-compile#33, cig-compile#37 |
| 12 | `-opt-bisect-limit`: one counter, pass by pass | take | cig-compile#33, cig-compile#39 |
| 13 | Instrumentation PGO: generate, lower, use; keyed by a function hash | adapt | cig-compile#33, (c) |
| 14 | Alive2's refinement check: six questions per pair of functions | adapt | cig-compile#39, cig-compile#33 |
| 15 | The IR encoding: `StateValue` and the fresh-undef-per-use rule | adapt | cig-compile#39, (a) |
| 16 | The bounds: what "bounded translation validation" bounds, and what it skips | adapt | cig-compile#39, cig-compile#33 |
| 17 | The `tv` plugin, and what a CIR verifier would own that `T1..T4` do not | adapt | cig-compile#39, (c) |

## Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries

[studies/08-go-a-small-fast-compiler.md](studies/08-go-a-small-fast-compiler.md)

| | mechanism | for cig | feeds |
|---:|---|---|---|
| 1 | One pass table, in order, in one file, with the required passes marked | adapt | cig-compile#33, (e) |
| 2 | Checking after every pass: the structural checker and the deterministic value-order randomizer | adapt | cig-compile#33 |
| 3 | Machine-independent rewrites: `generic.rules` | adapt | cig-compile#33, (e) |
| 4 | Instruction selection written in the same rule language: `AMD64.rules` | reject | cig-compile#32, (e) |
| 5 | The rule generator: `rulegen.go` and `main.go` | adapt | cig-compile#33, (e) |
| 6 | Applying the rules to a fixed point | adapt | cig-compile#33, (e) |
| 7 | The `prove` pass: bounds-check and nil-check elimination from facts about ranges | adapt | cig-compile#33, cig-compile#34, (a), (e) |
| 8 | Escape analysis with per-parameter summaries | adapt | cig-compile#33, cig-compile#34, cig-compile#35, (e), (g) |
| 9 | The escape graph and its dataflow solver | not applicable | cig-compile#33, (g) |
| 10 | The inliner's budget and what cannot be inlined | adapt | cig-compile#33, cig-compile#34, (e) |
| 11 | Devirtualization: static and profile-guided | reject | cig-compile#33, (b) |
| 12 | PGO: samples from an ordinary run, matched by source-relative line offset | adapt | cig-compile#33, (c) |
| 13 | The build cache: hashing the inputs, storing by the output | adapt | cig-compile#37, (f) |
| 14 | Concurrent function compilation and the unified export format | adapt | cig-compile#37, (e), (f) |
| 15 | The two phases of `walk`: lowering to runtime calls and respecting evaluation order | reject | cig-compile#33, (e) |
| 16 | Linker dead-code elimination, reflection and the interface escape | adapt | cig-compile#36, cig-compile#40, (e) |
| 17 | The pc-ln table: what every Go program carries for tracebacks and profiling | adapt | cig-compile#36, cig-compile#38, (e) |
| 18 | The built-in CPU profiler: samples the running program produces | adapt | cig-compile#38, (c) |
| 19 | The write barrier pass: how a tracing collector reaches into the compiler | not applicable | cig-compile#33, cig-compile#35, (g) |
| 20 | The compiler compiling itself: `cmd/dist`'s bootstrap | adapt | cig-compile#17 |

## Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization

[studies/09-types-julia-dotnet-rustc.md](studies/09-types-julia-dotnet-rustc.md)

| | mechanism | for cig | feeds |
|---:|---|---|---|
| 1 | Inference is abstract interpretation of the call graph, not of a function alone | adapt | cig-compile#34, (i) |
| 2 | The lattice is seven layers composed, each adjoining one kind of refinement | adapt | cig-compile#34, cig-script#256, (i) |
| 3 | Widening makes inference terminate: named constants and `limit_type_size` | adapt | cig-compile#34, cig-script#256, (b), (i) |
| 4 | One table of built-in result types and costs | take | cig-compile#34, cig-compile#37, (i) |
| 5 | Nine inferred effects per method, read at call sites | adapt | cig-compile#33, cig-compile#34, (i) |
| 6 | One compiled copy per argument-type combination, keyed after signature widening | take | cig-compile#34, cig-script#256, (f), (i) |
| 7 | The front-end widens too: a type the programmer never has to pay for | adapt | cig-compile#34, (i) |
| 8 | Union splitting at inlining sites | adapt | cig-compile#33, cig-compile#34, cig-script#256, (i) |
| 9 | The method cache: one per method instance, keyed by world range | adapt | cig-compile#36, cig-script#256, (f) |
| 10 | Cached native code between runs: package images, not a separate format | adapt | cig-compile#36, cig-compile#41, cig-script#256, (f) |
| 11 | Invalidation: backedges and binding partitions | reject | cig-compile#34, cig-script#256, (f) |
| 12 | .NET shared generics: one body per reference, a copy per value type, a dictionary | adapt | cig-compile#32, cig-compile#35, cig-compile#36, cig-script#256, (a), (i) |
| 13 | Guarded devirtualization: a guess and a guard, with a fallback | adapt | cig-compile#32, cig-compile#33, cig-script#256, (b), (c), (d) |
| 14 | First-class structs in registers | adapt | cig-compile#34, cig-compile#35, (a), (h), (i) |
| 15 | rustc monomorphization: a collector, a partitioner and two limits | adapt | cig-compile#33, cig-compile#34, cig-compile#37, cig-script#256, (f), (i) |

## MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR

[studies/10-whole-program-mlton-and-unboxing-ghc.md](studies/10-whole-program-mlton-and-unboxing-ghc.md)

| | mechanism | for cig | feeds |
|---:|---|---|---|
| 1 | The intermediate-language chain: XML, SXML, SSA, SSA2, RSSA | adapt | cig-compile#34, cig-compile#33, (i) |
| 2 | What whole-program compilation costs: time, memory, executable size | adapt | cig-compile#34, cig-compile#37 |
| 3 | Monomorphization: copy a polymorphic function per type at which it is used | adapt | cig-compile#34, (i) |
| 4 | Defunctionalization: a closure is a tagged sum, a call is a case over the tag | adapt | cig-compile#33, cig-compile#34, (e), (i) |
| 5 | 0CFA: the control-flow analysis that keeps the case small | adapt | cig-compile#33, cig-compile#34, (i) |
| 6 | Contification: a function called from one place becomes a block | adapt | cig-compile#33, (e) |
| 7 | Flattening: tuples at every use site are passed flat; refs are packed into their containers | adapt | cig-compile#35, cig-compile#33, cig-compile#34, (a), (h) |
| 8 | Useless-component removal: a dependence graph, backwards from primitives | adapt | cig-compile#33, cig-compile#34, (i) |
| 9 | Packed representation: data layouts chosen per type at the SSA2 to RSSA boundary | adapt | cig-compile#35, (a), (h) |
| 10 | GHC's worker and wrapper: a split in `WorkWrap.hs`, a safety proof in `DmdAnal.hs` | adapt | cig-compile#34, (a), (i) |
| 11 | Demand analysis as the safety proof: and what remains for a strict language | adapt | cig-compile#34, cig-compile#33 |
| 12 | CPR: Constructed Product Result, the symmetric split on function results | adapt | cig-compile#34, cig-compile#35, (i) |
| 13 | Unarisation: one source value becomes several machine values at STG time | adapt | cig-compile#34, cig-compile#35, (a), (e) |
| 14 | SpecConstr: specialize a recursive loop per data constructor at its call sites | adapt | cig-compile#34, cig-compile#33, (i) |
| 15 | Specialise: a copy per dictionary, the type-directed analog | adapt | cig-compile#34, (e), (i) |
| 16 | MLton's shrinker: the small local simplifier that runs inside every pass | adapt | cig-compile#33, (e) |
| 17 | GHC's multi-phase simplifier: a round of specialization, phases 2 to 0, then demand-and-ww | adapt | cig-compile#33, cig-compile#34, (e) |

## How a dynamic value is laid out, across twelve implementations

[studies/11-the-value-layout.md](studies/11-the-value-layout.md)

| | mechanism | for cig | feeds |
|---:|---|---|---|
| 1 | A tag word beside a payload word: Lua's 16-byte `TValue`, QuickJS on 64-bit, Go's `eface`, and cig's planned 16 | adapt | cig-compile#35, cig-compile#32, (a), (e) |
| 2 | One word, with tag bits in a pointer: V8, Ruby, OCaml, Lean, Koka, CPython's stack references | reject | cig-compile#35, cig-script#243, (a) |
| 3 | One word that is a double unless it is a NaN: LuaJIT, SpiderMonkey, JavaScriptCore, Wren, QuickJS on 32-bit | reject | cig-compile#35, (a), (e) |
| 4 | Testing "is it an integer": one byte compare, one mask-and-compare, one pair of mask-compares | take | cig-compile#35, cig-compile#32, (a), (i) |
| 5 | Testing "both integers" before an add | adapt | cig-compile#35, cig-compile#32, (a), (i) |
| 6 | Getting a double out and putting one back | adapt | cig-compile#35, (a) |
| 7 | Getting a pointer out | take | cig-compile#35, cig-compile#32, (a) |
| 8 | Copying a value between registers, or to memory, and passing one to a function | adapt | cig-compile#35, cig-compile#32, cig-script#243, cig-script#256, (a), (e), (g) |
| 9 | Which layouts hold an `i64` unboxed, and which force a narrower integer or a heap number | adapt | cig-compile#35, cig-compile#32, (a) |
| 10 | What each engine does when an integer does not fit | take | cig-compile#35, cig-compile#32, (a), (i) |
| 11 | Where the reference count and the type of an object live | take | cig-compile#35, (g), (h) |
| 12 | Immortal and static objects that are never counted | adapt | cig-compile#35, cig-compile#36, (g) |
| 13 | What a clone and a drop touch in memory | take | cig-compile#35, (g) |
| 14 | Which layouts can be written in safe Rust, and which cannot | adapt | cig-compile#35, (a), (e) |
| 15 | QuickJS and Wren: both layouts behind one set of accessors | adapt | cig-compile#35, (a), (e) |

## V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end

[studies/12-v8-and-javascriptcore-tiers.md](studies/12-v8-and-javascriptcore-tiers.md)

| | mechanism | for cig | feeds |
|---:|---|---|---|
| 1 | Feedback as a per-function vector with a kind per site and a state per slot | adapt | cig-compile#33, cig-script#256, (b) |
| 2 | The interrupt budget: counting scaled by code size, charged on the feedback cell | adapt | cig-compile#33, cig-compile#37, cig-script#256, cig-script#243, (c), (d) |
| 3 | JavaScriptCore's value profiles and the speculated-type lattice as bits | adapt | cig-compile#33, cig-script#256, (b), (c), (i) |
| 4 | JavaScriptCore's execution counter: counts scaled by code size and by how often a function was thrown back | adapt | cig-script#256, cig-script#243, cig-compile#33, (c) |
| 5 | Maglev: a CFG SSA built straight from bytecode using feedback, with specialized nodes | adapt | cig-compile#32, cig-compile#37, cig-script#256, (b), (d), (e) |
| 6 | Why V8 is leaving the sea of nodes | reject | cig-compile#33 |
| 7 | Turboshaft: operations in blocks in fixed order, a reducer stack, one phase per copy | adapt | cig-compile#33 |
| 8 | The frame translation: per exit, where each interpreter register's value now lives | adapt | cig-compile#32, cig-script#256, (d), (e) |
| 9 | Materializing objects whose allocation the optimizer removed | adapt | cig-compile#32, cig-compile#35, cig-script#256, (d), (g) |
| 10 | Lazy against eager deopt, and what stops a function from deoptimizing forever | adapt | cig-compile#32, cig-script#256, (d) |
| 11 | JavaScriptCore's OSR exit: recover the bytecode's variables from a stream of value sources | adapt | cig-compile#32, cig-script#256, (d) |
| 12 | OSR entry: entering optimized code in the middle of a loop | adapt | cig-script#256, (d) |
| 13 | OSR-exit fuzzing: force exits at random so the suite tests the exit path | take | cig-compile#39, cig-script#256 |
| 14 | Liftoff: a single pass over the Wasm bytecode with a virtual operand stack | adapt | cig-compile#37, cig-script#256 |
| 15 | B3 and Air: a small SSA IR and a machine-level IR, three register allocators for three budgets | adapt | cig-compile#32, cig-compile#37, cig-script#256, (e), (f) |
| 16 | Why JavaScriptCore replaced LLVM in its top tier, and what the figures were | reject | cig-compile#32, cig-compile#98 |
| 17 | Code kept between runs: a header of hashes, a sanity check before anything is used | take | cig-compile#36, cig-script#256, (f) |
| 18 | Sparkplug: a non-optimizing baseline that walks the bytecode once | reject | cig-compile#37, cig-script#256 |
| 19 | JavaScriptCore's inline cache compiler: a per-shape handler, patched in place | adapt | cig-compile#33, cig-script#256, (b) |
| 20 | The code-cache's two-step deserialize: a sanity check without the source, then with it | adapt | cig-compile#36, cig-script#256, (f) |

## Proving the compiler right: Csmith, YARPGen, C-Vise, Cranelift's fuzzing and verifier, CompCert and diverse double-compiling

[studies/13-proving-the-compiler-right.md](studies/13-proving-the-compiler-right.md)

| | mechanism | for cig | feeds |
|---:|---|---|---|
| 1 | Csmith's effects: ruling out order-sensitive expressions at generation time | adapt | cig-compile#39, (d) |
| 2 | Csmith's points-to facts: proving a dereference valid before emitting it | not applicable | cig-compile#39 |
| 3 | YARPGen's generation-time evaluation: no safety wrappers, every value known | adapt | cig-compile#39, cig-compile#33, (d) |
| 4 | Generation policies: biasing random programs toward the optimizer | take | cig-compile#39, cig-compile#33, (d) |
| 5 | Reporting the result: a checksum of the whole state | take | cig-compile#39, cig-compile#17 |
| 6 | Reduction: a fixed-point loop over many kinds of transformation | adapt | cig-compile#33, cig-compile#39 |
| 7 | Reduction passes: lines, balanced regions, peepholes, integers, syntax-aware | adapt | cig-compile#33, cig-compile#39 |
| 8 | Reduction's parallel candidate pool and the sanity gate | adapt | cig-compile#33, cig-compile#39 |
| 9 | Differential fuzzing: several oracles, named and configured | adapt | cig-compile#39, (b), (c) |
| 10 | Random IR valid by construction: cranelift-fuzzgen | adapt | cig-compile#33, cig-compile#39, (e), (i) |
| 11 | An interpreter of the IR, as oracle for the code generator | adapt | cig-compile#33, cig-compile#39, cig-compile#17, (b), (e) |
| 12 | Filetest kinds: run, compile, optimize, interpret | take | cig-compile#33, cig-compile#39, cig-compile#32 |
| 13 | Verified lowering rules: VeriISLE and Crocus | adapt | cig-compile#33, cig-compile#39, (e), (i) |
| 14 | A symbolic checker for the register allocator | adapt | cig-compile#32, cig-compile#33, cig-compile#39, (e) |
| 15 | CompCert: semantic preservation and what it did not cover | adapt | cig-compile#33, cig-compile#17 |
| 16 | Finding the pass at fault: counter-based bisect | take | cig-compile#33 |
| 17 | Trusting the bootstrap: diverse double-compiling | adapt | cig-compile#17 |

## Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache

[studies/14-compiling-fast.md](studies/14-compiling-fast.md)

| | mechanism | for cig | feeds |
|---:|---|---|---|
| 1 | QBE's IL: a small SSA that leaves machine details out | adapt | cig-compile#33, cig-compile#34, (e), (i) |
| 2 | QBE's pass list: nine thousand lines of C across fourteen passes | adapt | cig-compile#33, cig-compile#37, (e), (f) |
| 3 | "70% of the performance in 10% of the code", in QBE's own words | adapt | cig-compile#33, cig-script#256, (e) |
| 4 | TinyCC: code generated while parsing | adapt | cig-script#256, cig-compile#32, (b), (e) |
| 5 | TinyCC's value stack: a stand-in for an IR, and what one pass cannot do | adapt | cig-compile#32, cig-script#256, (b) |
| 6 | CPython's stencils: one template per bytecode instruction, cut at build time | adapt | cig-script#256, cig-compile#32, (b), (e) |
| 7 | Stamping and patching holes at run time | adapt | cig-script#256, cig-compile#32, (b), (f) |
| 8 | Page protections: write then execute, never both, and cig's rule 8 | reject | cig-script#256, cig-compile#32, cig-compile#40, (b), (f) |
| 9 | The abstract interpreter over traces: a lattice for a dynamic language | adapt | cig-compile#34, cig-script#256, (b), (i) |
| 10 | Zig's debug back ends: typed IR straight to machine code, no optimizer between | adapt | cig-compile#32, cig-script#256, (e), (f) |
| 11 | Incremental analysis: a per-kind dependency index | adapt | cig-compile#37, cig-compile#34, (e), (f) |
| 12 | Patching the output in place: Zig's mapped linker | adapt | cig-compile#37, cig-compile#32, (e), (f) |
| 13 | rustc's queries and fingerprints | adapt | cig-compile#37, cig-compile#32, (e), (f) |
| 14 | Red and green: a result reused when its inputs did not change | adapt | cig-compile#37, cig-compile#34, (f), (i) |
| 15 | Parallel units of code generation | adapt | cig-compile#37, cig-compile#32, (e), (f) |
| 16 | Cranelift's per-function cache: hash the function stencil, store the compiled bytes | take | cig-compile#37, cig-compile#32, cig-script#256, (e), (f) |

## Profiles that outlive a run, and code compiled ahead of time from them: ART, .NET, and Leyden

[studies/15-profiles-and-ahead-of-time-caches.md](studies/15-profiles-and-ahead-of-time-caches.md)

| | mechanism | for cig | feeds |
|---:|---|---|---|
| 1 | ART's hotness counters and per-site inline caches (what the JIT records) | adapt | cig-script#256, cig-script#243, cig-compile#33, (c) |
| 2 | .NET's call-counting stubs and instrumented tiers | adapt | cig-script#256, cig-script#243, (c) |
| 3 | Counters placed on a spanning tree | take | cig-compile#33, (c) |
| 4 | cig counts steps already (rule 3): the step counter as the hotness counter | adapt | cig-script#256, cig-script#243, (c) |
| 5 | ART's profile on disk: format, merging, filtering | adapt | cig-script#256, cig-compile#33, (c) |
| 6 | .NET's static profiles (.mibc) | adapt | cig-compile#33, cig-script#256 |
| 7 | Leyden's training run and AOT cache (JEP 483 and JEP 515) | take | cig-script#256, cig-compile#33, (f) |
| 8 | ART's compiler filters and guarded inlines from profile data | adapt | cig-script#256, cig-compile#33, (b) |
| 9 | ReadyToRun code replaced by tier-1 code; version resilience | adapt | cig-script#256, cig-compile#36 |
| 10 | The oat header and what it checks | take | cig-script#256, cig-script#254, (f) |
| 11 | V8's code cache sanity check | take | cig-script#256, (f) |
| 12 | ART's `oat_file_assistant` and the staleness statuses | take | cig-script#256, cig-script#254, (f) |
| 13 | ART's dual mapping of the JIT code cache (rule 8) | take | cig-script#256, (f) |
| 14 | When the profile is wrong: falling back and bounding the cost | take | cig-script#256, (d) |
| 15 | Julia's package images: how the cache is keyed to the flags that built it | adapt | cig-script#256, (f) |
| 16 | A proposed profile record and cache entry header for cig | take | cig-script#256, cig-script#254, cig-compile#33, (c), (f) |
| 17 | ART's `profile_assistant` and "enough has changed to recompile" | take | cig-script#256, cig-script#254, (c), (f) |
| 18 | Off-thread deserialize: V8's model for a cache loaded while something else runs | adapt | cig-script#256, (f) |

## The shape of an IR: nanopass and Chez Scheme, Zig's two IRs, the sea of nodes and leaving it, flambda at a distance

[studies/16-ir-shapes-nanopass-zig-sea-of-nodes.md](studies/16-ir-shapes-nanopass-zig-sea-of-nodes.md)

| | mechanism | for cig | feeds |
|---:|---|---|---|
| 1 | Nanopass: a language is a grammar of record types | adapt | cig-compile#3, cig-compile#33, (e) |
| 2 | Nanopass: a language is the difference from another language | adapt | cig-compile#3, cig-compile#34 |
| 3 | Nanopass: `define-pass` generates the clauses a pass does not write | adapt | cig-compile#33, (e) |
| 4 | Nanopass: a pass's output belongs to the output language, at build time | adapt | cig-compile#33, cig-compile#39, (e) |
| 5 | Chez Scheme: a chain of twenty-eight intermediate languages, from `Lsrc` to `L16` | adapt | cig-compile#3, cig-compile#32, cig-compile#33, cig-compile#34, (e), (i) |
| 6 | Chez Scheme's cp0: a source-level optimizer bounded by two counters | take | cig-compile#33, (e) |
| 7 | Chez Scheme's cptypes: type recovery over a lattice of predicates | adapt | cig-compile#34, cig-compile#33, (i) |
| 8 | Chez Scheme: the back end starts at `L16` with a single generator | adapt | cig-compile#32, cig-compile#33 |
| 9 | Zig's ZIR: an untyped per-file IR, cacheable, made in parallel | adapt | cig-compile#3, cig-compile#34, cig-compile#37, (e) |
| 10 | Zig's Sema: ZIR becomes AIR, with every compile-time thing folded on the way | adapt | cig-compile#34, (i) |
| 11 | Zig's AIR: typed, flat, liveness beside the IR, legalize per back end | adapt | cig-compile#32, cig-compile#34, cig-compile#39, (e), (i) |
| 12 | The sea of nodes: a graph where control and data have the same shape | reject | cig-compile#3, cig-compile#33 |
| 13 | V8's reasons for leaving the sea of nodes for a CFG (Turboshaft) | reject | cig-compile#3, cig-compile#32, cig-compile#33 |
| 14 | Graal's frame states: keep the idea, not the graph | adapt | cig-compile#32, cig-compile#34, (d) |
| 15 | OCaml flambda: an IR built for inlining and closure specialization | adapt | cig-compile#33, cig-compile#34, (e) |
| 16 | CIR's two ways to grow: one IR with more in it, or a chain where each link removes one thing | adapt | cig-compile#3, cig-compile#32, cig-compile#33, cig-compile#34, (e), (i) |

## From machine code to a program: object emission, embedded linking, size, debug tables, hardening, targets and the bootstrap

[studies/17-from-machine-code-to-a-program.md](studies/17-from-machine-code-to-a-program.md)

| | mechanism | for cig | feeds |
|---:|---|---|---|
| 1 | One object per codegen unit through `cranelift-object`; units compiled in parallel, functions serial inside a unit | take | cig-compile#32, cig-compile#4, (e), (f) |
| 2 | What rustc_codegen_cranelift delegates to the system linker | adapt | cig-compile#32, cig-compile#41, (e) |
| 3 | mold's pipeline, and what a cig program needs of it | adapt | cig-compile#32, cig-compile#36, cig-compile#41, (e) |
| 4 | Section garbage collection and identical code folding | take | cig-compile#36, cig-compile#4, cig-compile#1, (e), (f) |
| 5 | What `object` can write, and what it cannot | adapt | cig-compile#32, cig-compile#41, (e), (f) |
| 6 | Relocations across ELF, Mach-O and PE: what the generator emits | take | cig-compile#32, cig-compile#41 |
| 7 | The alternative with no linker at all: Wasmtime's image | reject | cig-compile#32 |
| 8 | A third way: the runtime as a position-independent executable with a reserved code section | adapt | cig-compile#32, cig-compile#40, cig-compile#41 |
| 9 | What a per-program runtime needs, from the measured cig 1.25.0 | take | cig-compile#36, cig-compile#4, cig-compile#1 |
| 10 | ThinLTO: whole-program optimization while modules still compile in parallel | adapt | cig-compile#32, cig-compile#33, cig-compile#37 |
| 11 | Small static binaries by default (Zig and release-mode settings) | adapt | cig-compile#36, cig-compile#41 |
| 12 | DWARF line tables from Rust: `.debug_line` written by gimli | take | cig-compile#38, cig-compile#32 |
| 13 | Unwind tables from Rust: `.eh_frame` written by gimli | adapt | cig-compile#38, cig-compile#32, cig-compile#4 |
| 14 | perf maps and perf jitdump for code made at run time | take | cig-compile#38, cig-script#256 |
| 15 | Cranelift's hardening settings | take | cig-compile#40, cig-compile#32 |
| 16 | CFI, SafeStack and ShadowCallStack: what each protects and what each costs | adapt | cig-compile#40 |
| 17 | PIE and full RELRO: a linker-side hardening pair | take | cig-compile#40, cig-compile#36 |
| 18 | Cross-compiling: one runtime per target, pinned by hash, fetched on demand | adapt | cig-compile#41, cig-compile#4 |
| 19 | Zig's bootstrap: a WebAssembly image of the compiler translated to C | adapt | cig-compile#17, cig-compile#41 |
| 20 | A staged bootstrap, and diverse double-compiling as its check | take | cig-compile#17, cig-compile#9 |
