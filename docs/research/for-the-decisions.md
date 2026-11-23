# The entries that feed each decision

Made from the studies' `Feeds` lines by `join.py`. The nine decisions are the ones the studies were written to inform; each is made once, for the VM, stage C and cig-cherry together.

## (a) the value representation and ABI

36 entries in 8 studies.

| study | | mechanism | for cig |
|---|---:|---|---|
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 6 | ABI framework: `ABIMachineSpec` and the per-target `MachineEnv` | take |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 7 | Calling conventions, cig's value size, and the sret fallback | adapt |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 1 | Lean's explicit reference-counting pass, inserted after a liveness analysis over owned and borrowed variables | adapt |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 2 | Koka's Perceus pass, with dup and drop placed precisely at last use, walked in reverse | adapt |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 11 | The object header: Lean's 32-bit count with sticky bands, Koka's 32-bit count with unique = 0 | adapt |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 12 | Freeing a deep structure without deep recursion: the header rewritten into a worklist pointer | adapt |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 13 | The fast path of a decrement is: "if count > 1, decrement and return; else cold path" | take |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 1 | The ownership kinds and the instructions that carry them | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 9 | Copy on write and the uniqueness primitive | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 10 | Hoisting a uniqueness check out of a loop: COWArrayOpt | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 11 | Roc's reference-count insertion with borrow inference | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 14 | The smallest ownership vocabulary that would let a CIR pass remove a clone/drop pair, with the verifier able to prove it | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 16 | Copy-on-write uniqueness as a VM primitive for cig's lists and maps | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 18 | Stack promotion of non-escaping allocations | adapt |
| [07](studies/07-llvm-pipeline-and-alive2.md) LLVM's optimization pipeline, and Alive2's translation validation | 6 | SROA, then Mem2Reg: how a frontend's memory becomes SSA | not applicable |
| [07](studies/07-llvm-pipeline-and-alive2.md) LLVM's optimization pipeline, and Alive2's translation validation | 10 | The inliner's cost model: thresholds and bonuses | adapt |
| [07](studies/07-llvm-pipeline-and-alive2.md) LLVM's optimization pipeline, and Alive2's translation validation | 15 | The IR encoding: `StateValue` and the fresh-undef-per-use rule | adapt |
| [08](studies/08-go-a-small-fast-compiler.md) Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries | 7 | The `prove` pass: bounds-check and nil-check elimination from facts about ranges | adapt |
| [09](studies/09-types-julia-dotnet-rustc.md) Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization | 12 | .NET shared generics: one body per reference, a copy per value type, a dictionary | adapt |
| [09](studies/09-types-julia-dotnet-rustc.md) Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization | 14 | First-class structs in registers | adapt |
| [10](studies/10-whole-program-mlton-and-unboxing-ghc.md) MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR | 7 | Flattening: tuples at every use site are passed flat; refs are packed into their containers | adapt |
| [10](studies/10-whole-program-mlton-and-unboxing-ghc.md) MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR | 9 | Packed representation: data layouts chosen per type at the SSA2 to RSSA boundary | adapt |
| [10](studies/10-whole-program-mlton-and-unboxing-ghc.md) MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR | 10 | GHC's worker and wrapper: a split in `WorkWrap.hs`, a safety proof in `DmdAnal.hs` | adapt |
| [10](studies/10-whole-program-mlton-and-unboxing-ghc.md) MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR | 13 | Unarisation: one source value becomes several machine values at STG time | adapt |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 1 | A tag word beside a payload word: Lua's 16-byte `TValue`, QuickJS on 64-bit, Go's `eface`, and cig's planned 16 | adapt |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 2 | One word, with tag bits in a pointer: V8, Ruby, OCaml, Lean, Koka, CPython's stack references | reject |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 3 | One word that is a double unless it is a NaN: LuaJIT, SpiderMonkey, JavaScriptCore, Wren, QuickJS on 32-bit | reject |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 4 | Testing "is it an integer": one byte compare, one mask-and-compare, one pair of mask-compares | take |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 5 | Testing "both integers" before an add | adapt |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 6 | Getting a double out and putting one back | adapt |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 7 | Getting a pointer out | take |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 8 | Copying a value between registers, or to memory, and passing one to a function | adapt |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 9 | Which layouts hold an `i64` unboxed, and which force a narrower integer or a heap number | adapt |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 10 | What each engine does when an integer does not fit | take |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 14 | Which layouts can be written in safe Rust, and which cannot | adapt |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 15 | QuickJS and Wren: both layouts behind one set of accessors | adapt |

## (b) one inline-cache and feedback format for the VM, stage C and cig-cherry

37 entries in 9 studies.

| study | | mechanism | for cig |
|---|---:|---|---|
| [03](studies/03-wasmtime-ahead-of-time-and-winch.md) Wasmtime as an ahead-of-time embedder of Cranelift, and Winch | 17 | Winch: one pass, a shadow stack, a bitset allocator | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 1 | CacheIR is a linear bytecode of guards, idempotent loads and one result op | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 2 | Each op declares itself in a YAML table that drives code generation | take |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 3 | Stub data lives apart from stub code, so one piece of code serves many sites | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 4 | OperandId types carry the type that a guard has already proved | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 5 | The generators look at actual values and emit guards then an action | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 7 | Shared code for baseline, Ion's own caches and the macro assembler | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 8 | Warp turns baseline's IR into optimizer IR with a transpiler | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 9 | The interpreter reads stubs with no native code: this is cig's situation | take |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 10 | Ahead-of-time stubs: a corpus baked into the runtime | take |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 11 | A site has three modes and a hard stub count | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 12 | Stub folding: several shape guards become one | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 13 | Health scoring: a per-op cost that reads out a site's happiness | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 14 | Counters and the next tier live together in ICScript | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 15 | Trial inlining gives a call site its own copy of the callee's caches | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 16 | The snapshot is taken on the main thread, from plain data | take |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 17 | Bailouts rebuild the lower tier's frame from snapshot plus recover instructions | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 18 | Invalidation keeps a function from bailing forever | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 19 | CacheIRCompiler's register allocator is a stub-wide SSA scheme | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 20 | Fuses let a hot guard leave the IR, then invalidate the code if it ever pops | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 13 | Roc's alias analysis and in-place list updates | adapt |
| [08](studies/08-go-a-small-fast-compiler.md) Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries | 11 | Devirtualization: static and profile-guided | reject |
| [09](studies/09-types-julia-dotnet-rustc.md) Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization | 3 | Widening makes inference terminate: named constants and `limit_type_size` | adapt |
| [09](studies/09-types-julia-dotnet-rustc.md) Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization | 13 | Guarded devirtualization: a guess and a guard, with a fallback | adapt |
| [12](studies/12-v8-and-javascriptcore-tiers.md) V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end | 1 | Feedback as a per-function vector with a kind per site and a state per slot | adapt |
| [12](studies/12-v8-and-javascriptcore-tiers.md) V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end | 3 | JavaScriptCore's value profiles and the speculated-type lattice as bits | adapt |
| [12](studies/12-v8-and-javascriptcore-tiers.md) V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end | 5 | Maglev: a CFG SSA built straight from bytecode using feedback, with specialized nodes | adapt |
| [12](studies/12-v8-and-javascriptcore-tiers.md) V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end | 19 | JavaScriptCore's inline cache compiler: a per-shape handler, patched in place | adapt |
| [13](studies/13-proving-the-compiler-right.md) Proving the compiler right: Csmith, YARPGen, C-Vise, Cranelift's fuzzing and verifier, CompCert and diverse double-compiling | 9 | Differential fuzzing: several oracles, named and configured | adapt |
| [13](studies/13-proving-the-compiler-right.md) Proving the compiler right: Csmith, YARPGen, C-Vise, Cranelift's fuzzing and verifier, CompCert and diverse double-compiling | 11 | An interpreter of the IR, as oracle for the code generator | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 4 | TinyCC: code generated while parsing | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 5 | TinyCC's value stack: a stand-in for an IR, and what one pass cannot do | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 6 | CPython's stencils: one template per bytecode instruction, cut at build time | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 7 | Stamping and patching holes at run time | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 8 | Page protections: write then execute, never both, and cig's rule 8 | reject |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 9 | The abstract interpreter over traces: a lattice for a dynamic language | adapt |
| [15](studies/15-profiles-and-ahead-of-time-caches.md) Profiles that outlive a run, and code compiled ahead of time from them: ART, .NET, and Leyden | 8 | ART's compiler filters and guarded inlines from profile data | adapt |

## (c) one counter and profile format

27 entries in 7 studies.

| study | | mechanism | for cig |
|---|---:|---|---|
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 2 | Each op declares itself in a YAML table that drives code generation | take |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 5 | The generators look at actual values and emit guards then an action | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 8 | Warp turns baseline's IR into optimizer IR with a transpiler | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 9 | The interpreter reads stubs with no native code: this is cig's situation | take |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 11 | A site has three modes and a hard stub count | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 13 | Health scoring: a per-op cost that reads out a site's happiness | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 14 | Counters and the next tier live together in ICScript | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 15 | Trial inlining gives a call site its own copy of the callee's caches | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 20 | Fuses let a hot guard leave the IR, then invalidate the code if it ever pops | adapt |
| [07](studies/07-llvm-pipeline-and-alive2.md) LLVM's optimization pipeline, and Alive2's translation validation | 3 | The function simplification pipeline, in order, with reasons | adapt |
| [07](studies/07-llvm-pipeline-and-alive2.md) LLVM's optimization pipeline, and Alive2's translation validation | 7 | EarlyCSE then GVN: cheap redundancy first, exhaustive redundancy later | adapt |
| [07](studies/07-llvm-pipeline-and-alive2.md) LLVM's optimization pipeline, and Alive2's translation validation | 13 | Instrumentation PGO: generate, lower, use; keyed by a function hash | adapt |
| [07](studies/07-llvm-pipeline-and-alive2.md) LLVM's optimization pipeline, and Alive2's translation validation | 17 | The `tv` plugin, and what a CIR verifier would own that `T1..T4` do not | adapt |
| [08](studies/08-go-a-small-fast-compiler.md) Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries | 12 | PGO: samples from an ordinary run, matched by source-relative line offset | adapt |
| [08](studies/08-go-a-small-fast-compiler.md) Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries | 18 | The built-in CPU profiler: samples the running program produces | adapt |
| [09](studies/09-types-julia-dotnet-rustc.md) Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization | 13 | Guarded devirtualization: a guess and a guard, with a fallback | adapt |
| [12](studies/12-v8-and-javascriptcore-tiers.md) V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end | 2 | The interrupt budget: counting scaled by code size, charged on the feedback cell | adapt |
| [12](studies/12-v8-and-javascriptcore-tiers.md) V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end | 3 | JavaScriptCore's value profiles and the speculated-type lattice as bits | adapt |
| [12](studies/12-v8-and-javascriptcore-tiers.md) V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end | 4 | JavaScriptCore's execution counter: counts scaled by code size and by how often a function was thrown back | adapt |
| [13](studies/13-proving-the-compiler-right.md) Proving the compiler right: Csmith, YARPGen, C-Vise, Cranelift's fuzzing and verifier, CompCert and diverse double-compiling | 9 | Differential fuzzing: several oracles, named and configured | adapt |
| [15](studies/15-profiles-and-ahead-of-time-caches.md) Profiles that outlive a run, and code compiled ahead of time from them: ART, .NET, and Leyden | 1 | ART's hotness counters and per-site inline caches (what the JIT records) | adapt |
| [15](studies/15-profiles-and-ahead-of-time-caches.md) Profiles that outlive a run, and code compiled ahead of time from them: ART, .NET, and Leyden | 2 | .NET's call-counting stubs and instrumented tiers | adapt |
| [15](studies/15-profiles-and-ahead-of-time-caches.md) Profiles that outlive a run, and code compiled ahead of time from them: ART, .NET, and Leyden | 3 | Counters placed on a spanning tree | take |
| [15](studies/15-profiles-and-ahead-of-time-caches.md) Profiles that outlive a run, and code compiled ahead of time from them: ART, .NET, and Leyden | 4 | cig counts steps already (rule 3): the step counter as the hotness counter | adapt |
| [15](studies/15-profiles-and-ahead-of-time-caches.md) Profiles that outlive a run, and code compiled ahead of time from them: ART, .NET, and Leyden | 5 | ART's profile on disk: format, merging, filtering | adapt |
| [15](studies/15-profiles-and-ahead-of-time-caches.md) Profiles that outlive a run, and code compiled ahead of time from them: ART, .NET, and Leyden | 16 | A proposed profile record and cache entry header for cig | take |
| [15](studies/15-profiles-and-ahead-of-time-caches.md) Profiles that outlive a run, and code compiled ahead of time from them: ART, .NET, and Leyden | 17 | ART's `profile_assistant` and "enough has changed to recompile" | take |

## (d) guards, snapshots and exits that keep the step count, the open tries and burns and the journal's position

31 entries in 9 studies.

| study | | mechanism | for cig |
|---|---:|---|---|
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 4 | `MachBuffer`: single-pass emission with peephole simplification, veneers and islands | take |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 5 | Side-tables beside the bytes: relocations, traps, call sites, srclocs | take |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 7 | Calling conventions, cig's value size, and the sret fallback | adapt |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 9 | `try_call` and exception tables | take |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 10 | Stack probes, frame pointers, and the pinned register | take |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 14 | The symbolic checker: regalloc2 proves its own output | take |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 15 | Unwind info: one abstract prologue, two formats | take |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 19 | Hardening settings: Spectre, BTI, pointer authentication, NaN canonicalization | take |
| [03](studies/03-wasmtime-ahead-of-time-and-winch.md) Wasmtime as an ahead-of-time embedder of Cranelift, and Winch | 7 | Fuel: an operator-cost table and a function-local counter | reject |
| [03](studies/03-wasmtime-ahead-of-time-and-winch.md) Wasmtime as an ahead-of-time embedder of Cranelift, and Winch | 8 | The fuel check, and the `out_of_gas` builtin | adapt |
| [03](studies/03-wasmtime-ahead-of-time-and-winch.md) Wasmtime as an ahead-of-time embedder of Cranelift, and Winch | 9 | Epochs: a cheap check against a cached deadline | reject |
| [03](studies/03-wasmtime-ahead-of-time-and-winch.md) Wasmtime as an ahead-of-time embedder of Cranelift, and Winch | 10 | Trap encoding: a native pc becomes a trap code | adapt |
| [03](studies/03-wasmtime-ahead-of-time-and-winch.md) Wasmtime as an ahead-of-time embedder of Cranelift, and Winch | 11 | Two ways a trap is raised: signals, or a libcall | adapt |
| [03](studies/03-wasmtime-ahead-of-time-and-winch.md) Wasmtime as an ahead-of-time embedder of Cranelift, and Winch | 14 | Unwinding and the exception table: handlers without stack walking | adapt |
| [03](studies/03-wasmtime-ahead-of-time-and-winch.md) Wasmtime as an ahead-of-time embedder of Cranelift, and Winch | 17 | Winch: one pass, a shadow stack, a bitset allocator | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 17 | Bailouts rebuild the lower tier's frame from snapshot plus recover instructions | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 18 | Invalidation keeps a function from bailing forever | adapt |
| [07](studies/07-llvm-pipeline-and-alive2.md) LLVM's optimization pipeline, and Alive2's translation validation | 8 | LICM's safety test, and why there are two LICM invocations around LoopRotate | take |
| [09](studies/09-types-julia-dotnet-rustc.md) Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization | 13 | Guarded devirtualization: a guess and a guard, with a fallback | adapt |
| [12](studies/12-v8-and-javascriptcore-tiers.md) V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end | 2 | The interrupt budget: counting scaled by code size, charged on the feedback cell | adapt |
| [12](studies/12-v8-and-javascriptcore-tiers.md) V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end | 5 | Maglev: a CFG SSA built straight from bytecode using feedback, with specialized nodes | adapt |
| [12](studies/12-v8-and-javascriptcore-tiers.md) V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end | 8 | The frame translation: per exit, where each interpreter register's value now lives | adapt |
| [12](studies/12-v8-and-javascriptcore-tiers.md) V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end | 9 | Materializing objects whose allocation the optimizer removed | adapt |
| [12](studies/12-v8-and-javascriptcore-tiers.md) V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end | 10 | Lazy against eager deopt, and what stops a function from deoptimizing forever | adapt |
| [12](studies/12-v8-and-javascriptcore-tiers.md) V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end | 11 | JavaScriptCore's OSR exit: recover the bytecode's variables from a stream of value sources | adapt |
| [12](studies/12-v8-and-javascriptcore-tiers.md) V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end | 12 | OSR entry: entering optimized code in the middle of a loop | adapt |
| [13](studies/13-proving-the-compiler-right.md) Proving the compiler right: Csmith, YARPGen, C-Vise, Cranelift's fuzzing and verifier, CompCert and diverse double-compiling | 1 | Csmith's effects: ruling out order-sensitive expressions at generation time | adapt |
| [13](studies/13-proving-the-compiler-right.md) Proving the compiler right: Csmith, YARPGen, C-Vise, Cranelift's fuzzing and verifier, CompCert and diverse double-compiling | 3 | YARPGen's generation-time evaluation: no safety wrappers, every value known | adapt |
| [13](studies/13-proving-the-compiler-right.md) Proving the compiler right: Csmith, YARPGen, C-Vise, Cranelift's fuzzing and verifier, CompCert and diverse double-compiling | 4 | Generation policies: biasing random programs toward the optimizer | take |
| [15](studies/15-profiles-and-ahead-of-time-caches.md) Profiles that outlive a run, and code compiled ahead of time from them: ART, .NET, and Leyden | 14 | When the profile is wrong: falling back and bounding the cost | take |
| [16](studies/16-ir-shapes-nanopass-zig-sea-of-nodes.md) The shape of an IR: nanopass and Chez Scheme, Zig's two IRs, the sea of nodes and leaving it, flambda at a distance | 14 | Graal's frame states: keep the idea, not the graph | adapt |

## (e) crate boundaries: the lowering, the shared code generator, the one unsafe loader

114 entries in 14 studies.

| study | | mechanism | for cig |
|---|---:|---|---|
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 1 | Block parameters in place of phi nodes | take |
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 2 | Data-flow graph kept apart from layout | adapt |
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 3 | Side-data rides on an instruction: source locations, memory flags, trap codes, exception tables, stack maps, debug tags | adapt |
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 4 | Variables to SSA at the front end (Braun et al. 2013) | adapt |
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 5 | The verifier: the invariant checker between passes | take |
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 6 | Pure nodes against the skeleton | adapt |
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 7 | The rewrite loop, its bounds and its cost function | adapt |
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 8 | GVN and LICM are not passes, they are elaboration | adapt |
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 9 | Rematerialization: the cheap op computed where it is used | adapt |
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 10 | `subsume`: a rewrite that drops a use | adapt |
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 11 | ISLE: a rule language compiled to Rust, with overlap and priority checked | adapt |
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 12 | How a rewrite is tested: one CLIF file per rule, filecheck pins the output | take |
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 13 | Alias analysis: one walk, four optimizations on the skeleton | adapt |
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 14 | The inliner is a library; the embedder is the policy | adapt |
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 17 | Lowering structured SSA with slots into CFG with block parameters | adapt |
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 18 | The dividing line: who runs which of cig-compile#33's passes, and why | adapt |
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 19 | The "a" in aegraph: an acyclic e-graph that never saturates | adapt |
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 20 | Chaos mode: a pseudo-randomness tap for proving that heuristics are only heuristics | adapt |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 1 | Lowering is a backward scan with instruction colors | adapt |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 2 | The ISLE lowering back end: pattern-matched rules per target | adapt |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 3 | One function at a time: `compile` drives lower-regalloc-check-emit, with edge blocks materialised | take |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 4 | `MachBuffer`: single-pass emission with peephole simplification, veneers and islands | take |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 6 | ABI framework: `ABIMachineSpec` and the per-target `MachineEnv` | take |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 8 | Tail calls: `return_call` and the `Tail` convention | adapt |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 10 | Stack probes, frame pointers, and the pinned register | take |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 11 | regalloc2's input: the `Function` trait and `Operand` | take |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 12 | Ion: the backtracking allocator | take |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 13 | Fastalloc, and the `regalloc_algorithm` setting that picks it | adapt |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 14 | The symbolic checker: regalloc2 proves its own output | take |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 15 | Unwind info: one abstract prologue, two formats | take |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 16 | User stack maps: GC roots the front end names, the back end records | reject |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 17 | Object files: `cranelift-module` and `cranelift-object` glue many functions into one `.o` | take |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 18 | JIT memory and W^X | take |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 19 | Hardening settings: Spectre, BTI, pointer authentication, NaN canonicalization | take |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 20 | Compile time: `timing` names every phase, `opt_level` is the gate | take |
| [03](studies/03-wasmtime-ahead-of-time-and-winch.md) Wasmtime as an ahead-of-time embedder of Cranelift, and Winch | 1 | The compiled artifact: an ELF with Wasmtime sections | adapt |
| [03](studies/03-wasmtime-ahead-of-time-and-winch.md) Wasmtime as an ahead-of-time embedder of Cranelift, and Winch | 2 | Sections that are decoded in place, not parsed | adapt |
| [03](studies/03-wasmtime-ahead-of-time-and-winch.md) Wasmtime as an ahead-of-time embedder of Cranelift, and Winch | 3 | Relocations: which are resolved when the image is built | adapt |
| [03](studies/03-wasmtime-ahead-of-time-and-winch.md) Wasmtime as an ahead-of-time embedder of Cranelift, and Winch | 4 | Reproducible builds, and the incremental Cranelift cache | adapt |
| [03](studies/03-wasmtime-ahead-of-time-and-winch.md) Wasmtime as an ahead-of-time embedder of Cranelift, and Winch | 5 | Loading without a page that is both writable and executable | adapt |
| [03](studies/03-wasmtime-ahead-of-time-and-winch.md) Wasmtime as an ahead-of-time embedder of Cranelift, and Winch | 10 | Trap encoding: a native pc becomes a trap code | adapt |
| [03](studies/03-wasmtime-ahead-of-time-and-winch.md) Wasmtime as an ahead-of-time embedder of Cranelift, and Winch | 12 | The builtin table on vmctx: calls into the runtime without relocations | adapt |
| [03](studies/03-wasmtime-ahead-of-time-and-winch.md) Wasmtime as an ahead-of-time embedder of Cranelift, and Winch | 13 | Address maps: a native pc becomes a source position | take |
| [03](studies/03-wasmtime-ahead-of-time-and-winch.md) Wasmtime as an ahead-of-time embedder of Cranelift, and Winch | 14 | Unwinding and the exception table: handlers without stack walking | adapt |
| [03](studies/03-wasmtime-ahead-of-time-and-winch.md) Wasmtime as an ahead-of-time embedder of Cranelift, and Winch | 15 | Stack maps: live roots at each callsite | adapt |
| [03](studies/03-wasmtime-ahead-of-time-and-winch.md) Wasmtime as an ahead-of-time embedder of Cranelift, and Winch | 17 | Winch: one pass, a shadow stack, a bitset allocator | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 7 | Shared code for baseline, Ion's own caches and the macro assembler | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 16 | The snapshot is taken on the main thread, from plain data | take |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 19 | CacheIRCompiler's register allocator is a stub-wide SSA scheme | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 1 | The ownership kinds and the instructions that carry them | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 2 | What the SIL ownership verifier proves, on every value, on every path | take |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 3 | Function parameter and result conventions | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 4 | Lexical lifetimes and deinit barriers: what may never move | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 6 | The older ARC sequence dataflow, and why ownership replaced most of it | reject |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 7 | Shortening a lifetime to its last use, and what forbids it | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 14 | The smallest ownership vocabulary that would let a CIR pass remove a clone/drop pair, with the verifier able to prove it | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 15 | Where cig's shared slots defeat the Swift model, and what Swift does about the same problem | adapt |
| [07](studies/07-llvm-pipeline-and-alive2.md) LLVM's optimization pipeline, and Alive2's translation validation | 1 | The pipeline's shape, a module through the default build | adapt |
| [07](studies/07-llvm-pipeline-and-alive2.md) LLVM's optimization pipeline, and Alive2's translation validation | 2 | The bottom-up walk and the inliner's module wrapper | adapt |
| [08](studies/08-go-a-small-fast-compiler.md) Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries | 1 | One pass table, in order, in one file, with the required passes marked | adapt |
| [08](studies/08-go-a-small-fast-compiler.md) Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries | 3 | Machine-independent rewrites: `generic.rules` | adapt |
| [08](studies/08-go-a-small-fast-compiler.md) Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries | 4 | Instruction selection written in the same rule language: `AMD64.rules` | reject |
| [08](studies/08-go-a-small-fast-compiler.md) Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries | 5 | The rule generator: `rulegen.go` and `main.go` | adapt |
| [08](studies/08-go-a-small-fast-compiler.md) Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries | 6 | Applying the rules to a fixed point | adapt |
| [08](studies/08-go-a-small-fast-compiler.md) Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries | 7 | The `prove` pass: bounds-check and nil-check elimination from facts about ranges | adapt |
| [08](studies/08-go-a-small-fast-compiler.md) Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries | 8 | Escape analysis with per-parameter summaries | adapt |
| [08](studies/08-go-a-small-fast-compiler.md) Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries | 10 | The inliner's budget and what cannot be inlined | adapt |
| [08](studies/08-go-a-small-fast-compiler.md) Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries | 14 | Concurrent function compilation and the unified export format | adapt |
| [08](studies/08-go-a-small-fast-compiler.md) Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries | 15 | The two phases of `walk`: lowering to runtime calls and respecting evaluation order | reject |
| [08](studies/08-go-a-small-fast-compiler.md) Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries | 16 | Linker dead-code elimination, reflection and the interface escape | adapt |
| [08](studies/08-go-a-small-fast-compiler.md) Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries | 17 | The pc-ln table: what every Go program carries for tracebacks and profiling | adapt |
| [10](studies/10-whole-program-mlton-and-unboxing-ghc.md) MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR | 4 | Defunctionalization: a closure is a tagged sum, a call is a case over the tag | adapt |
| [10](studies/10-whole-program-mlton-and-unboxing-ghc.md) MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR | 6 | Contification: a function called from one place becomes a block | adapt |
| [10](studies/10-whole-program-mlton-and-unboxing-ghc.md) MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR | 13 | Unarisation: one source value becomes several machine values at STG time | adapt |
| [10](studies/10-whole-program-mlton-and-unboxing-ghc.md) MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR | 15 | Specialise: a copy per dictionary, the type-directed analog | adapt |
| [10](studies/10-whole-program-mlton-and-unboxing-ghc.md) MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR | 16 | MLton's shrinker: the small local simplifier that runs inside every pass | adapt |
| [10](studies/10-whole-program-mlton-and-unboxing-ghc.md) MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR | 17 | GHC's multi-phase simplifier: a round of specialization, phases 2 to 0, then demand-and-ww | adapt |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 1 | A tag word beside a payload word: Lua's 16-byte `TValue`, QuickJS on 64-bit, Go's `eface`, and cig's planned 16 | adapt |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 3 | One word that is a double unless it is a NaN: LuaJIT, SpiderMonkey, JavaScriptCore, Wren, QuickJS on 32-bit | reject |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 8 | Copying a value between registers, or to memory, and passing one to a function | adapt |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 14 | Which layouts can be written in safe Rust, and which cannot | adapt |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 15 | QuickJS and Wren: both layouts behind one set of accessors | adapt |
| [12](studies/12-v8-and-javascriptcore-tiers.md) V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end | 5 | Maglev: a CFG SSA built straight from bytecode using feedback, with specialized nodes | adapt |
| [12](studies/12-v8-and-javascriptcore-tiers.md) V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end | 8 | The frame translation: per exit, where each interpreter register's value now lives | adapt |
| [12](studies/12-v8-and-javascriptcore-tiers.md) V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end | 15 | B3 and Air: a small SSA IR and a machine-level IR, three register allocators for three budgets | adapt |
| [13](studies/13-proving-the-compiler-right.md) Proving the compiler right: Csmith, YARPGen, C-Vise, Cranelift's fuzzing and verifier, CompCert and diverse double-compiling | 10 | Random IR valid by construction: cranelift-fuzzgen | adapt |
| [13](studies/13-proving-the-compiler-right.md) Proving the compiler right: Csmith, YARPGen, C-Vise, Cranelift's fuzzing and verifier, CompCert and diverse double-compiling | 11 | An interpreter of the IR, as oracle for the code generator | adapt |
| [13](studies/13-proving-the-compiler-right.md) Proving the compiler right: Csmith, YARPGen, C-Vise, Cranelift's fuzzing and verifier, CompCert and diverse double-compiling | 13 | Verified lowering rules: VeriISLE and Crocus | adapt |
| [13](studies/13-proving-the-compiler-right.md) Proving the compiler right: Csmith, YARPGen, C-Vise, Cranelift's fuzzing and verifier, CompCert and diverse double-compiling | 14 | A symbolic checker for the register allocator | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 1 | QBE's IL: a small SSA that leaves machine details out | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 2 | QBE's pass list: nine thousand lines of C across fourteen passes | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 3 | "70% of the performance in 10% of the code", in QBE's own words | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 4 | TinyCC: code generated while parsing | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 6 | CPython's stencils: one template per bytecode instruction, cut at build time | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 10 | Zig's debug back ends: typed IR straight to machine code, no optimizer between | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 11 | Incremental analysis: a per-kind dependency index | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 12 | Patching the output in place: Zig's mapped linker | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 13 | rustc's queries and fingerprints | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 15 | Parallel units of code generation | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 16 | Cranelift's per-function cache: hash the function stencil, store the compiled bytes | take |
| [16](studies/16-ir-shapes-nanopass-zig-sea-of-nodes.md) The shape of an IR: nanopass and Chez Scheme, Zig's two IRs, the sea of nodes and leaving it, flambda at a distance | 1 | Nanopass: a language is a grammar of record types | adapt |
| [16](studies/16-ir-shapes-nanopass-zig-sea-of-nodes.md) The shape of an IR: nanopass and Chez Scheme, Zig's two IRs, the sea of nodes and leaving it, flambda at a distance | 3 | Nanopass: `define-pass` generates the clauses a pass does not write | adapt |
| [16](studies/16-ir-shapes-nanopass-zig-sea-of-nodes.md) The shape of an IR: nanopass and Chez Scheme, Zig's two IRs, the sea of nodes and leaving it, flambda at a distance | 4 | Nanopass: a pass's output belongs to the output language, at build time | adapt |
| [16](studies/16-ir-shapes-nanopass-zig-sea-of-nodes.md) The shape of an IR: nanopass and Chez Scheme, Zig's two IRs, the sea of nodes and leaving it, flambda at a distance | 5 | Chez Scheme: a chain of twenty-eight intermediate languages, from `Lsrc` to `L16` | adapt |
| [16](studies/16-ir-shapes-nanopass-zig-sea-of-nodes.md) The shape of an IR: nanopass and Chez Scheme, Zig's two IRs, the sea of nodes and leaving it, flambda at a distance | 6 | Chez Scheme's cp0: a source-level optimizer bounded by two counters | take |
| [16](studies/16-ir-shapes-nanopass-zig-sea-of-nodes.md) The shape of an IR: nanopass and Chez Scheme, Zig's two IRs, the sea of nodes and leaving it, flambda at a distance | 9 | Zig's ZIR: an untyped per-file IR, cacheable, made in parallel | adapt |
| [16](studies/16-ir-shapes-nanopass-zig-sea-of-nodes.md) The shape of an IR: nanopass and Chez Scheme, Zig's two IRs, the sea of nodes and leaving it, flambda at a distance | 11 | Zig's AIR: typed, flat, liveness beside the IR, legalize per back end | adapt |
| [16](studies/16-ir-shapes-nanopass-zig-sea-of-nodes.md) The shape of an IR: nanopass and Chez Scheme, Zig's two IRs, the sea of nodes and leaving it, flambda at a distance | 15 | OCaml flambda: an IR built for inlining and closure specialization | adapt |
| [16](studies/16-ir-shapes-nanopass-zig-sea-of-nodes.md) The shape of an IR: nanopass and Chez Scheme, Zig's two IRs, the sea of nodes and leaving it, flambda at a distance | 16 | CIR's two ways to grow: one IR with more in it, or a chain where each link removes one thing | adapt |
| [17](studies/17-from-machine-code-to-a-program.md) From machine code to a program: object emission, embedded linking, size, debug tables, hardening, targets and the bootstrap | 1 | One object per codegen unit through `cranelift-object`; units compiled in parallel, functions serial inside a unit | take |
| [17](studies/17-from-machine-code-to-a-program.md) From machine code to a program: object emission, embedded linking, size, debug tables, hardening, targets and the bootstrap | 2 | What rustc_codegen_cranelift delegates to the system linker | adapt |
| [17](studies/17-from-machine-code-to-a-program.md) From machine code to a program: object emission, embedded linking, size, debug tables, hardening, targets and the bootstrap | 3 | mold's pipeline, and what a cig program needs of it | adapt |
| [17](studies/17-from-machine-code-to-a-program.md) From machine code to a program: object emission, embedded linking, size, debug tables, hardening, targets and the bootstrap | 4 | Section garbage collection and identical code folding | take |
| [17](studies/17-from-machine-code-to-a-program.md) From machine code to a program: object emission, embedded linking, size, debug tables, hardening, targets and the bootstrap | 5 | What `object` can write, and what it cannot | adapt |

## (f) the cache for lowered modules and native code

42 entries in 10 studies.

| study | | mechanism | for cig |
|---|---:|---|---|
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 15 | `Context::optimize`: the pipeline and its one gate | adapt |
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 16 | The incremental cache: one per-function key, one serialized compiled stencil | take |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 17 | Object files: `cranelift-module` and `cranelift-object` glue many functions into one `.o` | take |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 18 | JIT memory and W^X | take |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 20 | Compile time: `timing` names every phase, `opt_level` is the gate | take |
| [03](studies/03-wasmtime-ahead-of-time-and-winch.md) Wasmtime as an ahead-of-time embedder of Cranelift, and Winch | 1 | The compiled artifact: an ELF with Wasmtime sections | adapt |
| [03](studies/03-wasmtime-ahead-of-time-and-winch.md) Wasmtime as an ahead-of-time embedder of Cranelift, and Winch | 4 | Reproducible builds, and the incremental Cranelift cache | adapt |
| [03](studies/03-wasmtime-ahead-of-time-and-winch.md) Wasmtime as an ahead-of-time embedder of Cranelift, and Winch | 16 | The on-disk cache: a hashed key and an LRU worker | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 3 | Stub data lives apart from stub code, so one piece of code serves many sites | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 10 | Ahead-of-time stubs: a corpus baked into the runtime | take |
| [08](studies/08-go-a-small-fast-compiler.md) Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries | 13 | The build cache: hashing the inputs, storing by the output | adapt |
| [08](studies/08-go-a-small-fast-compiler.md) Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries | 14 | Concurrent function compilation and the unified export format | adapt |
| [09](studies/09-types-julia-dotnet-rustc.md) Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization | 6 | One compiled copy per argument-type combination, keyed after signature widening | take |
| [09](studies/09-types-julia-dotnet-rustc.md) Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization | 9 | The method cache: one per method instance, keyed by world range | adapt |
| [09](studies/09-types-julia-dotnet-rustc.md) Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization | 10 | Cached native code between runs: package images, not a separate format | adapt |
| [09](studies/09-types-julia-dotnet-rustc.md) Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization | 11 | Invalidation: backedges and binding partitions | reject |
| [09](studies/09-types-julia-dotnet-rustc.md) Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization | 15 | rustc monomorphization: a collector, a partitioner and two limits | adapt |
| [12](studies/12-v8-and-javascriptcore-tiers.md) V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end | 15 | B3 and Air: a small SSA IR and a machine-level IR, three register allocators for three budgets | adapt |
| [12](studies/12-v8-and-javascriptcore-tiers.md) V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end | 17 | Code kept between runs: a header of hashes, a sanity check before anything is used | take |
| [12](studies/12-v8-and-javascriptcore-tiers.md) V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end | 20 | The code-cache's two-step deserialize: a sanity check without the source, then with it | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 2 | QBE's pass list: nine thousand lines of C across fourteen passes | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 7 | Stamping and patching holes at run time | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 8 | Page protections: write then execute, never both, and cig's rule 8 | reject |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 10 | Zig's debug back ends: typed IR straight to machine code, no optimizer between | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 11 | Incremental analysis: a per-kind dependency index | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 12 | Patching the output in place: Zig's mapped linker | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 13 | rustc's queries and fingerprints | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 14 | Red and green: a result reused when its inputs did not change | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 15 | Parallel units of code generation | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 16 | Cranelift's per-function cache: hash the function stencil, store the compiled bytes | take |
| [15](studies/15-profiles-and-ahead-of-time-caches.md) Profiles that outlive a run, and code compiled ahead of time from them: ART, .NET, and Leyden | 7 | Leyden's training run and AOT cache (JEP 483 and JEP 515) | take |
| [15](studies/15-profiles-and-ahead-of-time-caches.md) Profiles that outlive a run, and code compiled ahead of time from them: ART, .NET, and Leyden | 10 | The oat header and what it checks | take |
| [15](studies/15-profiles-and-ahead-of-time-caches.md) Profiles that outlive a run, and code compiled ahead of time from them: ART, .NET, and Leyden | 11 | V8's code cache sanity check | take |
| [15](studies/15-profiles-and-ahead-of-time-caches.md) Profiles that outlive a run, and code compiled ahead of time from them: ART, .NET, and Leyden | 12 | ART's `oat_file_assistant` and the staleness statuses | take |
| [15](studies/15-profiles-and-ahead-of-time-caches.md) Profiles that outlive a run, and code compiled ahead of time from them: ART, .NET, and Leyden | 13 | ART's dual mapping of the JIT code cache (rule 8) | take |
| [15](studies/15-profiles-and-ahead-of-time-caches.md) Profiles that outlive a run, and code compiled ahead of time from them: ART, .NET, and Leyden | 15 | Julia's package images: how the cache is keyed to the flags that built it | adapt |
| [15](studies/15-profiles-and-ahead-of-time-caches.md) Profiles that outlive a run, and code compiled ahead of time from them: ART, .NET, and Leyden | 16 | A proposed profile record and cache entry header for cig | take |
| [15](studies/15-profiles-and-ahead-of-time-caches.md) Profiles that outlive a run, and code compiled ahead of time from them: ART, .NET, and Leyden | 17 | ART's `profile_assistant` and "enough has changed to recompile" | take |
| [15](studies/15-profiles-and-ahead-of-time-caches.md) Profiles that outlive a run, and code compiled ahead of time from them: ART, .NET, and Leyden | 18 | Off-thread deserialize: V8's model for a cache loaded while something else runs | adapt |
| [17](studies/17-from-machine-code-to-a-program.md) From machine code to a program: object emission, embedded linking, size, debug tables, hardening, targets and the bootstrap | 1 | One object per codegen unit through `cranelift-object`; units compiled in parallel, functions serial inside a unit | take |
| [17](studies/17-from-machine-code-to-a-program.md) From machine code to a program: object emission, embedded linking, size, debug tables, hardening, targets and the bootstrap | 4 | Section garbage collection and identical code folding | take |
| [17](studies/17-from-machine-code-to-a-program.md) From machine code to a program: object emission, embedded linking, size, debug tables, hardening, targets and the bootstrap | 5 | What `object` can write, and what it cannot | adapt |

## (g) reference counting made cheap: elision, borrowing, reuse

49 entries in 8 studies.

| study | | mechanism | for cig |
|---|---:|---|---|
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 13 | Alias analysis: one walk, four optimizations on the skeleton | adapt |
| [02](studies/02-cranelift-back-end-and-regalloc2.md) Cranelift's back end and regalloc2 | 16 | User stack maps: GC roots the front end names, the back end records | reject |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 1 | Lean's explicit reference-counting pass, inserted after a liveness analysis over owned and borrowed variables | adapt |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 2 | Koka's Perceus pass, with dup and drop placed precisely at last use, walked in reverse | adapt |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 3 | Lobster's compile-time lifetimes: LT_BORROW, LT_KEEP, LT_ANY on every AST node | reject |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 4 | Lean's borrow inference by fixed-point dataflow over an SCC | adapt |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 5 | Koka's borrow signatures carried with the module, consulted at every call | adapt |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 6 | Lean's reset-reuse: the freed cell of a matched constructor fed to the next same-shape allocation | reject |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 7 | Lean's reset-reuse expansion into hot and cold paths with dynamic derived borrows | reject |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 8 | Koka's reuse analysis: a dropped allocation of size N feeds the next same-size allocation | reject |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 9 | Koka's reuse specialization: skip the field store when the value written matches the pattern | reject |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 10 | Koka's drop specialization: a dropped value, branched on uniqueness, with children dup'd on the shared path | adapt |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 11 | The object header: Lean's 32-bit count with sticky bands, Koka's 32-bit count with unique = 0 | adapt |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 12 | Freeing a deep structure without deep recursion: the header rewritten into a worklist pointer | adapt |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 13 | The fast path of a decrement is: "if count > 1, decrement and return; else cold path" | take |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 14 | Koka's fip and fbip checks: static guarantees of in-place functional programming | reject |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 15 | The "count is one" check as the gate for in-place writes to strings, lists and maps | take |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 16 | What cig's mutable, shared `Rc<RefCell<..>>` containers change: most reuse is gone | adapt |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 17 | Derived borrows: a projection of a borrowed value is itself borrowed | adapt |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 18 | Coalescing inc and dec operations within a basic block | adapt |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 19 | Dead-code elimination of pure let-bindings as a prerequisite for counts | take |
| [05](studies/05-reference-counting-lean-koka-lobster.md) Lean 4, Koka and Lobster: reference counting made cheap | 20 | Propagating user-annotated borrows forward with a four-point lattice | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 1 | The ownership kinds and the instructions that carry them | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 2 | What the SIL ownership verifier proves, on every value, on every path | take |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 3 | Function parameter and result conventions | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 4 | Lexical lifetimes and deinit barriers: what may never move | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 5 | OSSA-era copy elision: why a `copy_value` of a `Guaranteed` can just vanish | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 6 | The older ARC sequence dataflow, and why ownership replaced most of it | reject |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 7 | Shortening a lifetime to its last use, and what forbids it | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 8 | Changing an owned parameter to guaranteed, lifting a retain/release out of the call pair | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 9 | Copy on write and the uniqueness primitive | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 10 | Hoisting a uniqueness check out of a loop: COWArrayOpt | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 11 | Roc's reference-count insertion with borrow inference | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 12 | Roc's reset/reuse and drop specialization: Perceus's deeper passes | reject |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 13 | Roc's alias analysis and in-place list updates | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 14 | The smallest ownership vocabulary that would let a CIR pass remove a clone/drop pair, with the verifier able to prove it | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 15 | Where cig's shared slots defeat the Swift model, and what Swift does about the same problem | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 16 | Copy-on-write uniqueness as a VM primitive for cig's lists and maps | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 17 | Move-only values and `drop_deinit` | reject |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 18 | Stack promotion of non-escaping allocations | adapt |
| [07](studies/07-llvm-pipeline-and-alive2.md) LLVM's optimization pipeline, and Alive2's translation validation | 6 | SROA, then Mem2Reg: how a frontend's memory becomes SSA | not applicable |
| [08](studies/08-go-a-small-fast-compiler.md) Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries | 8 | Escape analysis with per-parameter summaries | adapt |
| [08](studies/08-go-a-small-fast-compiler.md) Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries | 9 | The escape graph and its dataflow solver | not applicable |
| [08](studies/08-go-a-small-fast-compiler.md) Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries | 19 | The write barrier pass: how a tracing collector reaches into the compiler | not applicable |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 8 | Copying a value between registers, or to memory, and passing one to a function | adapt |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 11 | Where the reference count and the type of an object live | take |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 12 | Immortal and static objects that are never counted | adapt |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 13 | What a clone and a drop touch in memory | take |
| [12](studies/12-v8-and-javascriptcore-tiers.md) V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end | 9 | Materializing objects whose allocation the optimizer removed | adapt |

## (h) shapes for records

8 entries in 4 studies.

| study | | mechanism | for cig |
|---|---:|---|---|
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 1 | CacheIR is a linear bytecode of guards, idempotent loads and one result op | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 6 | A shape is a one-word summary of a map's layout that guards can check | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 12 | Stub folding: several shape guards become one | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 20 | Fuses let a hot guard leave the IR, then invalidate the code if it ever pops | adapt |
| [09](studies/09-types-julia-dotnet-rustc.md) Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization | 14 | First-class structs in registers | adapt |
| [10](studies/10-whole-program-mlton-and-unboxing-ghc.md) MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR | 7 | Flattening: tuples at every use site are passed flat; refs are packed into their containers | adapt |
| [10](studies/10-whole-program-mlton-and-unboxing-ghc.md) MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR | 9 | Packed representation: data layouts chosen per type at the SSA2 to RSSA boundary | adapt |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 11 | Where the reference count and the type of an object live | take |

## (i) the type lattice for stage D and for observed types

43 entries in 10 studies.

| study | | mechanism | for cig |
|---|---:|---|---|
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 2 | Data-flow graph kept apart from layout | adapt |
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 6 | Pure nodes against the skeleton | adapt |
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 9 | Rematerialization: the cheap op computed where it is used | adapt |
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 17 | Lowering structured SSA with slots into CFG with block parameters | adapt |
| [01](studies/01-cranelift-ir-and-mid-end.md) Cranelift: its IR and its acyclic e-graph mid-end | 18 | The dividing line: who runs which of cig-compile#33's passes, and why | adapt |
| [04](studies/04-spidermonkey-cacheir-and-warp.md) SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers | 4 | OperandId types carry the type that a guard has already proved | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 8 | Changing an owned parameter to guaranteed, lifting a retain/release out of the call pair | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 15 | Where cig's shared slots defeat the Swift model, and what Swift does about the same problem | adapt |
| [06](studies/06-swift-ownership-and-arc-roc.md) Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR | 17 | Move-only values and `drop_deinit` | reject |
| [09](studies/09-types-julia-dotnet-rustc.md) Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization | 1 | Inference is abstract interpretation of the call graph, not of a function alone | adapt |
| [09](studies/09-types-julia-dotnet-rustc.md) Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization | 2 | The lattice is seven layers composed, each adjoining one kind of refinement | adapt |
| [09](studies/09-types-julia-dotnet-rustc.md) Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization | 3 | Widening makes inference terminate: named constants and `limit_type_size` | adapt |
| [09](studies/09-types-julia-dotnet-rustc.md) Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization | 4 | One table of built-in result types and costs | take |
| [09](studies/09-types-julia-dotnet-rustc.md) Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization | 5 | Nine inferred effects per method, read at call sites | adapt |
| [09](studies/09-types-julia-dotnet-rustc.md) Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization | 6 | One compiled copy per argument-type combination, keyed after signature widening | take |
| [09](studies/09-types-julia-dotnet-rustc.md) Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization | 7 | The front-end widens too: a type the programmer never has to pay for | adapt |
| [09](studies/09-types-julia-dotnet-rustc.md) Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization | 8 | Union splitting at inlining sites | adapt |
| [09](studies/09-types-julia-dotnet-rustc.md) Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization | 12 | .NET shared generics: one body per reference, a copy per value type, a dictionary | adapt |
| [09](studies/09-types-julia-dotnet-rustc.md) Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization | 14 | First-class structs in registers | adapt |
| [09](studies/09-types-julia-dotnet-rustc.md) Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization | 15 | rustc monomorphization: a collector, a partitioner and two limits | adapt |
| [10](studies/10-whole-program-mlton-and-unboxing-ghc.md) MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR | 1 | The intermediate-language chain: XML, SXML, SSA, SSA2, RSSA | adapt |
| [10](studies/10-whole-program-mlton-and-unboxing-ghc.md) MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR | 3 | Monomorphization: copy a polymorphic function per type at which it is used | adapt |
| [10](studies/10-whole-program-mlton-and-unboxing-ghc.md) MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR | 4 | Defunctionalization: a closure is a tagged sum, a call is a case over the tag | adapt |
| [10](studies/10-whole-program-mlton-and-unboxing-ghc.md) MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR | 5 | 0CFA: the control-flow analysis that keeps the case small | adapt |
| [10](studies/10-whole-program-mlton-and-unboxing-ghc.md) MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR | 8 | Useless-component removal: a dependence graph, backwards from primitives | adapt |
| [10](studies/10-whole-program-mlton-and-unboxing-ghc.md) MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR | 10 | GHC's worker and wrapper: a split in `WorkWrap.hs`, a safety proof in `DmdAnal.hs` | adapt |
| [10](studies/10-whole-program-mlton-and-unboxing-ghc.md) MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR | 12 | CPR: Constructed Product Result, the symmetric split on function results | adapt |
| [10](studies/10-whole-program-mlton-and-unboxing-ghc.md) MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR | 14 | SpecConstr: specialize a recursive loop per data constructor at its call sites | adapt |
| [10](studies/10-whole-program-mlton-and-unboxing-ghc.md) MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR | 15 | Specialise: a copy per dictionary, the type-directed analog | adapt |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 4 | Testing "is it an integer": one byte compare, one mask-and-compare, one pair of mask-compares | take |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 5 | Testing "both integers" before an add | adapt |
| [11](studies/11-the-value-layout.md) How a dynamic value is laid out, across twelve implementations | 10 | What each engine does when an integer does not fit | take |
| [12](studies/12-v8-and-javascriptcore-tiers.md) V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end | 3 | JavaScriptCore's value profiles and the speculated-type lattice as bits | adapt |
| [13](studies/13-proving-the-compiler-right.md) Proving the compiler right: Csmith, YARPGen, C-Vise, Cranelift's fuzzing and verifier, CompCert and diverse double-compiling | 10 | Random IR valid by construction: cranelift-fuzzgen | adapt |
| [13](studies/13-proving-the-compiler-right.md) Proving the compiler right: Csmith, YARPGen, C-Vise, Cranelift's fuzzing and verifier, CompCert and diverse double-compiling | 13 | Verified lowering rules: VeriISLE and Crocus | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 1 | QBE's IL: a small SSA that leaves machine details out | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 9 | The abstract interpreter over traces: a lattice for a dynamic language | adapt |
| [14](studies/14-compiling-fast.md) Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache | 14 | Red and green: a result reused when its inputs did not change | adapt |
| [16](studies/16-ir-shapes-nanopass-zig-sea-of-nodes.md) The shape of an IR: nanopass and Chez Scheme, Zig's two IRs, the sea of nodes and leaving it, flambda at a distance | 5 | Chez Scheme: a chain of twenty-eight intermediate languages, from `Lsrc` to `L16` | adapt |
| [16](studies/16-ir-shapes-nanopass-zig-sea-of-nodes.md) The shape of an IR: nanopass and Chez Scheme, Zig's two IRs, the sea of nodes and leaving it, flambda at a distance | 7 | Chez Scheme's cptypes: type recovery over a lattice of predicates | adapt |
| [16](studies/16-ir-shapes-nanopass-zig-sea-of-nodes.md) The shape of an IR: nanopass and Chez Scheme, Zig's two IRs, the sea of nodes and leaving it, flambda at a distance | 10 | Zig's Sema: ZIR becomes AIR, with every compile-time thing folded on the way | adapt |
| [16](studies/16-ir-shapes-nanopass-zig-sea-of-nodes.md) The shape of an IR: nanopass and Chez Scheme, Zig's two IRs, the sea of nodes and leaving it, flambda at a distance | 11 | Zig's AIR: typed, flat, liveness beside the IR, legalize per back end | adapt |
| [16](studies/16-ir-shapes-nanopass-zig-sea-of-nodes.md) The shape of an IR: nanopass and Chez Scheme, Zig's two IRs, the sea of nodes and leaving it, flambda at a distance | 16 | CIR's two ways to grow: one IR with more in it, or a chain where each link removes one thing | adapt |
