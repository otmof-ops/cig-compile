# Measurements the studies ask for

Made from the studies by `join.py`. Nothing was measured by the studies themselves; each line names what would decide an entry. A measurement of time belongs on the test machine; a count can be made anywhere. What this run did measure is in `data/`.

## Cranelift: its IR and its acyclic e-graph mid-end

[studies/01-cranelift-ir-and-mid-end.md](studies/01-cranelift-ir-and-mid-end.md)

- CIR-level LICM of `nbody` and `mandel` at stage B, before and after, on the test machine, with step counts the same and float bits the same (entry 8). `nbody`'s `dx * dx` reuse and `mandel`'s outer-constant hoist are the planted hoists.
- The egraph pass's share of `Context::optimize` on `orchestrate.cig` at stage C, `opt_level=speed`, broken down by `elaborate_licm_hoist`, `elaborate_remat`, `pure_inst_deduped`, `skeleton_inst_gvn`, `alias_analysis_removed_load` (entry 7). The counters are in `pass.stats`.
- Stage C compile time at every level on `hello.cig` (the 5 ms tree-walker baseline) and `orchestrate.cig` (the 46 MB peak-memory program), on the test machine, as `opt_level=none`, `speed`, `speed_and_size` (entry 15).
- Rebuild time of `orchestrate.cig` with one function changed, with and without `Context::compile_with_cache`, measured as wall-clock of the second build after a one-line edit (entry 16). The hit ratio on a representative edit distribution is the second number.
- `loop.cig` and `fib.cig` at stage C with the remat rules on and off, measured as wall-clock runtime of the program and as register-count output from the regalloc2 stats (entry 9).
- Alias analysis's share of `egraph_pass` time on `orchestrate.cig` at stage C, under `enable_alias_analysis=true` vs. `false`, wall-clock and `alias_analysis_removed_dead_store` count (entry 13).
- Lowering time of `orchestrate.cig` through `SSABuilder` against a hand-rolled parameter inserter, wall-clock and the number of block parameters the SSA builder inserts (entry 4).
- CIR-level inlining alone against CIR-level inlining plus `Context::inline`, for build time and the stripped `hello.cig` binary size (entry 14). The stripped baseline is 11 MB today at stage A; stage C targets 4 MB.
- LOC in `fold-consts` after a rewrite to rules against today's hand-written pass (`cig-script/src/lower/cir/pass/fold_consts.rs`) with the golden tests under `cig-script/tests/` held equal (entry 11).
- Chaos-mode seed shuffle over the compute set under `run_passes` with 16 different seeds, zero differences expected; the number of seeds that match is the signal (entry 20).
- Egraph pass time with `ECLASS_ENODE_LIMIT`, `MATCHES_LIMIT`, `REWRITE_LIMIT` set to the Cranelift defaults (5, 5, 5) and to doubled values (10, 10, 10), on `orchestrate.cig` at stage C, to see whether the defaults are tight (entry 7).
- Alias-region coverage on cig's stdlib operations (list, map, string, chain), measured as the fraction of lowered loads and stores that carry a region tag, with the egraph pass `alias_analysis_removed_load` counter as the dependent variable (entry 13).
- Cranelift verifier's share of `Context::compile` time with `enable_verifier=false` against the default `true`, on `orchestrate.cig`, to decide whether cig keeps the verifier on in release builds the way `cig-script/src/lower/cir/verify.rs` is kept on today (entry 5).

## Cranelift's back end and regalloc2

[studies/02-cranelift-back-end-and-regalloc2.md](studies/02-cranelift-back-end-and-regalloc2.md)

- Entry 2: LOC in `lower.rs` after rewriting ten opcodes to rules against the same ten as code, on the test machine, with golden tests holding.
- Entry 4: aarch64 build time of `orchestrate.cig` with the default island policy against a forced one-island-per-block policy, on the test machine.
- Entry 7: `orchestrate.cig` and `mandel.cig`, the cost of spilling the status byte through memory with `SystemV` (two regs) against returning it in `rcx` under `enable_llvm_abi_extensions` or `rdx` under `Tail`, on the test machine.
- Entry 8: `fib.cig` recursion depth with and without tail-call lowering at stage C, on the test machine.
- Entry 10: `mandel.cig` with and without `enable_pinned_reg`, plus the probe strategy's effect on a hot `fib.cig`, on the test machine.
- Entry 12: Ion's wall-clock cost on `orchestrate.cig` at stage C, on the test machine, against Fastalloc's cost.
- Entry 13: build time and code size of `orchestrate.cig` at each level, with each algorithm, on the test machine.
- Entry 19: code-size cost of `sign_return_address` on `orchestrate.cig` for aarch64, on the test machine.
- Entry 20: `cig-compile --timing orchestrate.cig -O2` once stage C is built, on the test machine, with the per-pass percentages.

## Wasmtime as an ahead-of-time embedder of Cranelift, and Winch

[studies/03-wasmtime-ahead-of-time-and-winch.md](studies/03-wasmtime-ahead-of-time-and-winch.md)

- **Fuel overhead.** Run the compute suite (`loop`, `fib`, `nbody`, `mandel`, `spectral`, `fannkuch`) on the test machine with and without the per-`Step` explicit check, against the tree-walker baseline. Decides entries 7 and 8.
- **Address-map size.** Build the `hello` program on the test machine with and without the address map, and record the sizes on x86_64-linux-musl and aarch64-apple-darwin. Decides entry 13.
- **Signals-vs-libcall trap cost.** Compile a trap-heavy program (division-by-zero on a hot path, bounds-check on a tight loop) twice on the test machine; measure the median wall time. Decides entry 11.
- **Stack-map size.** For a script that keeps heavy map work in a hot function, record the stack-map section's byte count against the whole `.cwasm`. Decides entry 15.
- **Incremental cache win.** Build `orchestrate.cig` on the test machine twice, once cold, once after a one-line edit, with and without a per-function cache enabled. Decides entry 4.

## SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers

[studies/04-spidermonkey-cacheir-and-warp.md](studies/04-spidermonkey-cacheir-and-warp.md)

- The site census the earlier draft asked for is done: `data/census/README.md` counted shapes per field site, receiver types per method site, callees per call site, operand-type pairs per arithmetic site, and rows of argument types per function, in the 21 compute programs and in cigo's `orchestrate.cig`. Entries 1, 4, 5, 6, 11 and 15 cite its findings; the shape-and-type counts answer the chain-cap, Megamorphic-threshold and shape-scheme questions the earlier draft left open.
- Per-op overhead of an interpreted stub against cig's hand-coded fast path, timed on fib, loop, sieve, mandel, a map-heavy and a `callm`-heavy workload. The census counted instructions only, not milliseconds. Decides entry 9: the kHybridICs split between interpreted ICs and static fast paths for cig's VM.
- Instruction count per field read and per `callm` with and without a shape-guarded stub, measured as `cig-script/docs/research/luajit.md` measures it (its run `w32-ljcount-j1`). Decides entry 6 against the status quo: the VM today takes 88 instructions per field read against 21 in LuaJIT's interpreter, and 97 per map read by key against 29.
- Startup cost of a trained AOT corpus against cold start, on a 500-line script. Decides entry 10: how much the tin should cache.
- Bailout frequency per function in a run with cherry forced on, for the differential suite. Decides entry 18's thresholds.
- Per-write cost of maintaining a cig shape as keys are inserted, and the shape's interaction with `IndexMap`'s rehash on grow. The census did not measure this. Decides entry 6's cost half against the already-measured win half.
- Memory per site for a cig stub against the current `Cell<u32>` in `Proto::hints`. Decides entries 1, 3 and 11: whether the per-site overhead fits the memory budget of `cig-compile#36`.
- Time per turn of a hot loop with and without an interpreted-IC chain, on fib, mandel and sieve. Decides entry 9: the point at which cig's VM gains more from a stub chain than it loses to the dispatch overhead.

For each measurement the entry format sets what is recorded and where. The compute set is the 21 programs under `cig-script/bench/compute/`; `orchestrate.cig` is the cigo workflow script that the census runs seven cases of. Every timed measurement is run on the test machine, with the step budget on and the differential suite checking that outputs match the tree-walker's.

## Lean 4, Koka and Lobster: reference counting made cheap

[studies/05-reference-counting-lean-koka-lobster.md](studies/05-reference-counting-lean-koka-lobster.md)

- Count of inc/dec a liveness-driven dec placement (entry 1) emits for `nbody`, `trees`, `closures` and `dispatch`, against the VM's current "clone always, drop at scope end": decides entries 1 and 2.
- Count of `.clone()` calls in `cig-script/src/vm/mod.rs`'s call setup for `dispatch` before and after a borrow-infer pass on `CallBuiltin` arguments only: decides entry 4.
- `.clone()` calls removed by static borrow signatures on `len`, `.contains`, `.get`, `.has`, `.keys`: decides entry 5.
- `strbuild`, `fannkuch` and `maps` at stage B with the uniqueness-write fast path (entry 15) against the clone-every-time path: decides entries 10 and 15.
- A nested-list drop at and past the measured 4,500,000-level threshold with and without a worklist drop for `Value::List` and `Value::Map`: decides entry 12. (Measured on cig 1.25.0, 2026-11-23: fine at 4,000,000 levels, aborts at 4,500,000 levels on both engines.)
- Count of inc/dec removed by a derived-borrow graph on `nbody` (field reads of a borrowed body record) and `fannkuch` (repeated indexing of a borrowed list), measured alongside entry 4: decides entry 17.
- Residual inc/dec count after entry 18 coalescing against entry 1 alone, on the four programs above: decides entry 18.
- The ratio of allocations that are strictly local (never escape the function) in cig's benchmark set, to see whether stage C would be served by escape analysis or whether entry 15's uniqueness-write already captures the win: this is the number cig-script#243's trick 6 names; it feeds entry 10 and entry 15.
- Count of `top` lattice states in the propagated-borrow result over cig's benchmark set, to decide whether the four-point lattice of entry 20 is the right shape or a three-point one suffices.
- Cost on the test machine of the `Rc::strong_count` single-compare that gates the uniqueness-write in entry 15, measured at a tight inner loop of `strbuild`, to confirm the branch stays predictable and the fallback clone is the only noticeable cost.

## Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR

[studies/06-swift-ownership-and-arc-roc.md](studies/06-swift-ownership-and-arc-roc.md)

- The count of implicit clones the tree-walker performs on `nbody`, `trees` and `fannkuch`, against the count a Clone/Drop/Borrow CIR would emit. The census (`data/census/README.md`) gives a floor for the VM: half of register moves copy a counted value in the median compute program, over 93,703,377 executed instructions across the 21 compute programs. It does not count copies made by arguments, results and builtins, and it does not measure the tree-walker. Still open: a hand-translated CIR for the hot loop of each program, with per-site clone counts.
- The hit rate of `Rc::strong_count(&l) == 1` at `SetIndex`, `SetMember` and `push` sites on `fannkuch`, `sieve`, `strbuild` and `maps`. The census (`data/census/README.md`) counts the per-site executions (for example `fannkuch` runs 1,060,163 `setindex` and `sieve` 607,827 `setindex`) but does not read the receiver's `Rc::strong_count` at the mutation site. Still open: a second instrument that reads the receiver's count at each such site. Decides whether entry 16 ships with just a run-time branch or with entry 10's loop-hoisting pass as well.
- The count of `Owned` parameters in the benchmark set whose matching drop dominates every return of the function: bounds entry 8's `owned-to-borrowed` signature pass. Below 20 percent and the pass is not worth building.
- The fraction of `Inc`/`Dec` pairs that would be `DecRef` instead on `trees` and `fannkuch`: bounds entry 11's non-recursive drop variant. A measured gain says whether cig's bytecode grows a `DEC_SHALLOW` opcode.
- The fraction of `Clone` sites whose source is a captured slot: bounds entry 15's exclusion. Confirms the elision pass is safe for most real programs.
- The size of the stack-promoted allocation set (entry 18) on `trees`, `nbody` and `dispatch`. A measure of what entry 18's escape analysis would catch, and the saved allocations per second.
- The number of inc/dec operations an insertion pass places per step of `dispatch` and `strbuild`, before and after the signature pass of entry 8 and the elision pass of entry 5. A per-pass breakdown is what the pipeline-order decision (entry 14's T5 to T7) is tuned against.
- The increase in CIR verifier time with T5 to T7 added to T1 to T4 (entry 14). cig's existing verifier runs after every pass; adding three rules should add a bounded fraction. The budget: not more than a 10 percent increase in total pass time, measured on a 1,000-line script.

## LLVM's optimization pipeline, and Alive2's translation validation

[studies/07-llvm-pipeline-and-alive2.md](studies/07-llvm-pipeline-and-alive2.md)

- At `-O2`, geomean runtime at stage B over `spectral`, `nbody`, `mandel`, `dispatch` for the pipeline shapes in entry 1 (one stage vs. two stages). (Decides entry 1.)
- At `-O2`, top-down vs. bottom-up inliner walk order on a cigo-scale corpus (the census, `data/census/README.md`, settles the compute case: 28 of 29 call sites are single-callee). (Decides entry 2.)
- At `-O2`, one `peephole` at the end against `peephole` after each heavy pass, following LLVM's "InstCombine after every heavy transform" pattern. (Decides entry 3.)
- At each level, stage B runtime and code size geomean over the benchmark corpus; budgets are `-O2 >= 1.5x -O0`, `-O3 >= 1.1x -O2`, `-Os <= 0.8x -O2` size. (Decides entry 4.)
- One-stage vs. two-stage on `orchestrate.cig` at `-O2`. (Decides entry 5.)
- `spectral`'s runtime stage B before and after a scoped CSE; then the extra instructions removed and compile time per 1000 lines of a global VN at `-O2`. (Decides entry 7.)
- `spectral`'s inner loop stage B before and after LICM, and separately the `MayRaise` safety test's frequency of allowing vs. refusing a hoist on the benchmark corpus; the census (`data/census/README.md`, finding 1) already fixes the floor on what `licm` may never move (step marks at 23.8% of a compute program's instructions). (Decides entry 8.)
- A golden-file test: 20 CIR snippets, each peephole-rewritten to fixpoint in one sweep. (Decides entry 9.)
- Thresholds 100, 200, 400 at `-O2` on the benchmark corpus, geomean runtime and code size; the census (`data/census/README.md`, finding 4) already suggests that near the lower end captures the compute-program gain. (Decides entry 10.)
- Keeping a call graph across `fold-consts`, `copy-prop`, `sccp` vs. rebuilding each time, on `orchestrate.cig`. (Decides entry 11.)
- `cigcompile bisect` wall time on a planted miscompile over a 1,000-line generated script; budget 30 s. (Decides entry 12.)
- `dispatch.cig` at `-O2` against `-O2 --profile-use=dispatch.cigprof`; budget `>= 1.15x`. (Decides entry 13.)
- A `.cigprof` file after a `fold-consts` change to the source, to confirm the CFG checksum detects the stale profile. (Decides entry 13.)
- A 20-function TV corpus; median solver time per function, budget 10 s. (Decides entries 14, 15, 16.)
- A planted arithmetic bug in `fold-consts` caught by TV but not by `verify_transition`. (Decides entry 17.)
- At unroll 2, 4, 8, and 32, TV refinement-check wall time on the 20-function corpus, Z3 as the solver, 10-second per-function cap, to pick the default. (Decides entry 16.)
- With and without the byte-equal pre/post shortcut from `tv/tv.cpp` (`verify`), TV time on a fuzz corpus of 2,000 CIR snippets after `fold-consts`, where most outputs equal inputs. (Decides entry 17.)
- `PassBuilderPipelines.cpp`'s shape at `-O1` vs `-O2` on `orchestrate.cig` at stage B: time, code size, and the number of instructions removed per pass invocation, so a future cig `-O1` vs `-O2` has a baseline. (Decides entry 4.)
- With and without the `PreservedAnalyses::preserveSet<CFGAnalyses>()` return for `fold-consts`, `copy-prop`, `sccp`, `dead-pure`: build-time cost of the dominator tree being kept against being rebuilt per pass, on `orchestrate.cig`. (Decides entry 11.)
- A run with the inliner at an extremely low threshold (`OptMinSizeThreshold = 5`, `-Oz`-style) against the full `-O2` setting, to measure how much size `cig-compile` saves when the inliner is suppressed. (Decides entries 4, 10.)
- A sampling of `alive2-v21.0/BugList.md` bugs reproduced against cig's `fold-consts` and `copy-prop` where applicable, as a sanity check that cig's existing passes do not share the same miscompiles. (Decides entry 17.)
- The cost of a second `peephole` run after a planned `gvn` at `-O2` on the benchmark corpus; LLVM's pattern of `InstCombine` after every heavy pass is cheap only because the worklist is small each time. (Decides entry 9.)
- The ratio of `MayRaise` operations that LICM would hoist with `AllowSpeculation=false` vs. `AllowSpeculation=true` on the benchmark corpus, to decide whether the two-step pattern is worth the extra pass. (Decides entry 8.)
- A `ProfileRecord` size budget: average bytes per 1000 CIR instructions in `.cigprof`, so the feature does not inflate stage B's payload. (Decides entry 13.)
- Byte-for-byte reproducibility of `.cigprof` across two instrumented runs on the same input (fixed clock, fixed seed), to confirm the file is a build input hashable into the program's own checksum. (Decides entry 13.)

## Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries

[studies/08-go-a-small-fast-compiler.md](studies/08-go-a-small-fast-compiler.md)

- Entry 2: how many cig passes depend on value order? Run the differential suite with a per-function value-order randomizer enabled, on cig's four passes (fold-consts, copy-prop, sccp, dead-pure), count the programs whose output changes.
- Entry 3: how many rules does it take to carry cig's `fold-consts` and `copy-prop` as rules? Count the lines in the two Rust modules and divide by the typical rule's compressed length; the figure decides whether a generator pays off at that scale.
- Entry 12: on a short benchmark run, what fraction of PGO edges match between two builds whose source has been reformatted but not functionally changed? The figure decides whether a `(caller span, callee name)` key is stable enough.
- Entry 16 (residual): for each program, which reached builtins are named by a static `CallBuiltin` only and which by a dynamic reference through `light` or a map. The size census in `data/size/README.md` names the drop targets (cli 2.0 MB, keyring 1.4 MB, zstd 821 KB, regex 617 KB, lowering 0.3 MB; 1.14 MB total for both engines, parser, lowering and standard library); the reachability matrix is still owed.
- Entry 17: on cig's benchmark set, the size of a pc-map (pc-delta, span-delta, inline-tree) per function, as a fraction of lowered code size. Go's figure is under 20 percent; the target is at that scale.
- Entry 18: the overhead of a 100 Hz sampling profiler against the VM's straight-line time on the benchmark set. Go's figure is low single-digit percentages; the target is at that scale.
- Entry 20: the disk and build-time cost of keeping a previous-cig binary in the cig-compile tree and using it as a bootstrap, against building cig-compile from scratch with the system Rust.

Taken off the owed list since the independent data run (`data/census/README.md`, `data/size/README.md`):

- Entry 7 (bounds-check elimination gain): the census shows every indexed access in the 21 compute programs sees one container-and-index kind (list by integer, or map by string) across 5.66 million executions; the Luajit gap of 38 to 15 instructions per read is the target that a prove-driven cut would close at every site.
- Entry 8 (Rc traffic): the census reports that the median compute program's `mov` instructions copy a counted value 50.0% of the time (range 0 to 100%), and that every function was called with one row of argument types.
- Entry 16 (which parts of the runtime can be dropped): the size census names the drops (above) and the instruction census names which builtin names cigo's script calls (`type_of` 3,149 times in one hook run, `.push()` 1,282 times); still owed is the static-vs-dynamic split per name.

## Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization

[studies/09-types-julia-dotnet-rustc.md](studies/09-types-julia-dotnet-rustc.md)

- A census-based measurement would count, from a run of each program, the number of distinct argument-type rows per cig function and the number of distinct receiver types at every call site. The existing census (`data/census/README.md`) counts observations on the VM; what is missing is the same count against an *inferred* type under the proposed stage-D pass.
- Entry 1: the fraction of CIR functions that end up with one inferred row of argument types under an interprocedural pass to a fixed point, on the compute programs and on `orchestrate.cig`.
- Entry 2: the fraction of records in `nbody` and in cigo's calls that are read only by their static keys (the condition for demoting to a shape).
- Entry 3: the maximum number of distinct argument-type rows any cig function would be specialized for, on the compute programs and `orchestrate.cig`, at a fixed point.
- Entry 5: the fraction of `CallBuiltin` sites eligible for constant folding under `effect_free + nothrow + terminates`.
- Entry 6: the distinct-signature count per function under the three widening rules, against the census's observed row count.
- Entry 7: the fraction of parameters that are "called" against "escape" against "neither", on the compute programs.
- Entry 8: the hit rate of a second tag test at `Union2` sites in cigo's hook and guard cases.
- Entry 10: the native-code size per cig function under cig-cherry; the share of functions hot enough to earn a cached blob between runs.
- Entry 12: the native-code size per method under one unified body for the four refcounted kinds, against four separate bodies.
- Entry 13: the hit rate of a single-guess guard at each site, and the code size growth of guarding every site that ran more than 100 times.
- Entry 14: the throughput of the `nbody` arithmetic loop under a promoted-record representation against today's `Map`-of-value.
- Entry 15: the size of the collected mono-item set under cig's rules, and the fraction of roots whose MIR walk reaches a cap.

## MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR

[studies/10-whole-program-mlton-and-unboxing-ghc.md](studies/10-whole-program-mlton-and-unboxing-ghc.md)

- **(entry 2)** compile-time and peak-RSS per pass as entries 3, 6, 7, 8, 14, 15 land, on the 21 compute programs, the lines-500 benchmark and cigo's script.
- **(entry 3)** code-size growth from signature-driven specialization on the same set, against cig-compile#34's `≤ 1.5× -O2's`.
- **(entry 5)** per-site candidate-set size of a 0CFA over CIR on the same set and cigo's script.
- **(entry 6)** fraction of `CallUser` targets whose static call-site count is one on cigo's script (contify candidates).
- **(entry 7)** count of `MakeMap` sites in cigo's script and the 21 compute programs whose keys are all static and whose readers are all `Member` with static names.
- **(entry 8)** fraction of `MakeMap` keys never read by `Member` or `Interp` on cigo's script.
- **(entry 9)** differential suite against 16-byte tagged and 8-byte NaN-boxed representations on the full set.
- **(entry 10)** per-call fraction of callers that already have an unboxed arg at the ww split, across the 21 compute programs.
- **(entry 11)** per-call fraction of boxed uses of a `Ty::Int` argument on the 21 compute programs.
- **(entry 12)** count of `Func`s whose every return is `MakeMap(static_keys)` across the 21 compute programs and cigo's script.
- **(entry 13)** register-count distribution at worker boundaries across the same set, against the chosen target ABI's cap.
- **(entry 14)** specialized-copy count on cigo's script against the `≤ 1.5× -O2's` ceiling.
- **(entry 15)** per-phase iteration count on a simplifier round on the 21 compute programs, to decide the phase count (one phase or three).
- **(entry 16)** how many of cig's current passes collapse into a single `shrink` wrapper, measured on the differential suite at zero differences.
- **(entry 17)** the time saved at each pipeline slot (early-specialize, simplifier, demand-and-ww, late-specialize) on the test machine's compile-time budget.
- **(cross-cut)** a one-table report of which passes touch each of the invariants `V1` to `V11` and `T1` to `T4` on an audit run, so a reviewer of a new pass sees the full invariant interaction at once.

## How a dynamic value is laid out, across twelve implementations

[studies/11-the-value-layout.md](studies/11-the-value-layout.md)

- **NaN-boxed 8 against tagged-16 against today's 24,** on `nbody`, `fannkuch`, `mandel`, `trees`, `sieve`, `loop`, counting callgrind machine instructions and peak RSS; decides the 15% threshold in #35 (entries 3, 15).
- **Paired-tag test on `num_add`,** before and after ordering `Value::Int` at discriminant 0, on `nbody`, `loop`, `fib`; decides entry 5's adaptation.
- **A copy of a 16-byte tagged `Value` through two registers,** against three words through memory for the 24-byte `Value`, on `fib` and on `loop`; decides entry 8.
- **The immortal-constants path on cigo,** against the baseline cigo start-up (`data/census/report.md` finding 6's 2,323 lists, 804 maps, 662 closures built every call); decides entry 12.

## V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end

[studies/12-v8-and-javascriptcore-tiers.md](studies/12-v8-and-javascriptcore-tiers.md)

- Entry 1 (polymorphic threshold): count the distinct shapes a `GETFIELD`/`SETFIELD` site sees in the compute programs and cigo's cases with a census run at a cherry-compiled build. The current census says every site saw one shape on the compute set; the extra question is whether cigo ever drifts.
- Entry 2 (tier thresholds): count how many of cigo's 1,038 functions reach any threshold on the cigo cases, with the four-threshold scheme (feedback allocation, tier 2, tier 3, OSR) and cig's current function sizes. Decides the tier-up values, which V8's 400 and 3000 are not likely to fit cig unchanged.
- Entry 3 (lattice width): count how many of the 227 arithmetic and comparison sites actually need more than one bit in the lattice (today one is "mixed"); if the number holds at near 1, cig's lattice can be narrower than planned. The lower bound on the lattice width is the eight primitive types plus the two "list of ints only" and "list of floats only" cases a numeric loop needs.
- Entry 4 (loop and call weighting): count call and loop turn pairs under `bench-stats` over the compute set; set `executionCounterIncrementFor{Entry,Loop}`'s cig analogues from the ratio. JSC's 15 and 1 are the reference starting point.
- Entry 5 (Maglev-shape compile time): compile time per function for the shape of entry 5 on the compute set and cigo's 1,038 functions, against today's `cig run --engine vm` start. Decides whether cig-cherry tier 2 compiles in time to beat the VM on cigo's cases, which are the shortest-lived workloads cig has.
- Entry 8 (exit-record size): bytes per exit point on cherry-compiled compute functions, against the compiled-size budget of cig-compile#36. Decides the open question in "Not settled" about translation-opcode stream against flat struct.
- Entry 9 (escape counts): the one cig-script#243 trick 6 owes, over the compute set and cigo.
- Entry 10 (exit-reason hotness): count exits by reason over the compute set with cherry forced on everything (threshold 0). Settles which reasons are hot; the design's reason list is cig's own (`WrongType`, `WrongShape`, `WrongReceiver`, `IntegerOverflow`, `TooManyCallees`, `StepBudgetBoundary`), and the measurement names the ones that stay and the ones that can be merged.
- Entry 11 (jettison threshold): the per-function exit count at which cig-cherry drops and recompiles with wider types. JSC's `osrExitCountForReoptimization` is 100 and `osrExitCountForReoptimizationFromLoop` is 5 (a loop exit is worth more because a loop runs many times); cig's analogues come from the measurement.
- Entry 12 (OSR-entry gain): loop time saved by switching in at a back-edge, on the loop-dominated compute programs (`loop`, `fannkuch`, `mandel`, `nbody`, `sieve`, `spectral` per the census). Decides whether OSR entry is in cig-cherry phase 4 at all, or deferred.
- Entry 13 (OSR-fuzz gate): the differential suite with cherry forced on and with cherry-fuzz injected must stay at zero differences. Not a measurement, a gate.
- Entry 14 (one-pass compile time): per function, on the compute set and cigo; against entry 5's number, so the tier-2 and tier-3 budgets are decided together.
- Entry 15 (per-tier compile budget): compile time per function at tier 2 and tier 3, on the test machine, against JSC's measured 5× to 10× ratio between the two.
- Entry 17 (sanity-check cost): time to run V8's eight-field sanity check on a cig cache entry of average size, as a fraction of cold cherry start-up. Decides whether the check runs on every cache hit or only on the first one per process.

## Proving the compiler right: Csmith, YARPGen, C-Vise, Cranelift's fuzzing and verifier, CompCert and diverse double-compiling

[studies/13-proving-the-compiler-right.md](studies/13-proving-the-compiler-right.md)

- **Policy bias (entry 4).** Compare pass fire-counts over 1,000 programs at fixed bias versus flat bias on a development machine using `cigcompile --remarks`; decide the ArithSSP-shape adoption.
- **Four generic reduce passes (entry 7).** On a seeded 500-statement differing program, measure the smallest script after `lines` alone versus `lines + balanced + peep + ints`; decide the pass set for `cigcompile reduce` 0.3.0.
- **Parallel reduce (entry 8).** Measure wall time and oracle calls per second at 1, 2, 4 and 8 workers on a 300-line differing script.
- **CIR generator (entry 10).** Measure iterations-to-fire for each of `broken.rs`'s pass twins under a CIR-level generator at 100,000 modules in a debug build.
- **CIR interpreter (entry 11).** Measure whether the four `broken.rs` twins each produce a divergence the interpreter sees, on a 50-line program.
- **Spec-per-term coverage (entry 13).** Count the CIR `Kind` variants (`cig-script/src/lower/cir/ir.rs`) and record how many have a `provide`/`require` comment on the day the discipline lands.
- **Regalloc2 checker cost (entry 14).** Once stage C is running, measure the per-compile overhead of enabling the checker on a 500-function module.
- **DDC (entry 17).** Once stage 1 exists, diff stage-2 binaries from three engines.
- **EMI transforms (carry-away).** Count transforms today (`cig-compile/tests/metamorphic.rs`: two) and target the Le et al. 2014 four (pull-inline, dead-code, statement reorder, constant split).
- **Fuzz throughput (entry 9).** Run each `cig-compile/fuzz/fuzz_targets/` target (`parse.rs`, `check.rs`, `lower.rs`, `program.rs`) for 60 seconds under libFuzzer on a development machine and record executions per second as the baseline for #39's "2,000 programs per PR" budget.
- **Bisect wall-time (entry 16).** On a planted miscompile in one of the four current passes, measure the wall-time from a failing `cigcompile test` to a named pass; the budget is 30 s per #33.
- **Reduce size target (entry 7).** For a 2,000-line generated program with a single-expression miscompile, measure the final reduced size and oracle-call count after a fixed-point driver with the four generic passes; #39 targets "30 lines in 2 minutes".
- **Engine pair coverage (entry 9).** Per engine pair today (tree-walker vs VM empty, tree-walker vs VM loaded, VM empty vs VM loaded), count cases in the differential suite and the rate of divergences that would have been caught.

## Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache

[studies/14-compiling-fast.md](studies/14-compiling-fast.md)

- **Entry 2.** For each cig CIR pass, the lines of Rust and the mean per-function walltime over the 21 compute programs; compared to QBE's `wc -l` per pass. Decides the Rust-pass-per-file target.
- **Entry 3.** The geometric mean of the 21 compute programs at stage C under cig's four passes against the same programs at stage A; the threshold is `cig-script/docs/ENGINES-1.25.0.md`'s 0.57 figure, which the VM already reaches. Decides whether QBE's "70% in 10%" shape is enough at stage C.
- **Entry 4.** The time to compile `fib` to native on a tier-2 translator against the VM's warm `fib` time (`cig-script/docs/ENGINES-1.25.0.md` reports 16.6 ms); the budget is a cold start no more than 10% slower per cig-script#256.
- **Entry 6.** The native bytes per stencil for each of cig's bytecode opcodes, cut by `rustc` at cig's build time, over the opcodes in `cig-script/crates/cig-bytecode/src/opcode.rs`. Decides template feasibility.
- **Entry 7.** The stamp-and-patch time per cig bytecode opcode against the VM's machine instructions per operation (900 in cig against 162 in Lua 5.4, `cig-script/docs/research/luajit.md`). Decides tier-2 payback.
- **Entry 9.** The guards planted per compute program at stage C under the CIR `Ty` lattice with and without a known-class state. Decides lattice width.
- **Entry 10.** The lines of Rust for a CIR-to-x86_64 baseline over a small instruction set, and the native bytes per CIR instruction. Decides whether a Rust-only baseline can stay bounded.
- **Entry 11.** The cache hit rate on cigo's `orchestrate.cig` after editing one function, under (a) a body-vs-signature split and (b) a whole-function hash.
- **Entry 12.** The per-node overwrite time for a one-function edit to `orchestrate.cig` against a cold link of the same.
- **Entry 13.** The rebuild time for a comment-only edit under a CIR-keyed per-function cache against today's `FileId`-keyed build index.
- **Entry 14.** Same input as 13, measured as cache state (green and red counts) rather than walltime.
- **Entry 15.** `orchestrate.cig` at stage C, 1 thread and 16 threads, codegen walltime; the test is `thread_count_does_not_change_bytes`.
- **Entry 16.** The warm-cache compile time per CIR function against a cold `cig-compile` run, over `orchestrate.cig`'s 1,038 functions (`data/census/README.md`). Today's cold rebuild is 0.06 to 0.16 s at stage A (`cig-compile/docs/decisions/0001-backend.md`); the floor for a no-op is 1.0 ms.

## Profiles that outlive a run, and code compiled ahead of time from them: ART, .NET, and Leyden

[studies/15-profiles-and-ahead-of-time-caches.md](studies/15-profiles-and-ahead-of-time-caches.md)

- The per-call size of a cigo-shaped profile with the entry-5 format, against the bytecode module's size, on the seven cigo cases (entry 5).
- The fraction of cigo call sites at which a second observation would be needed to cover the site: extend the census to keep 2 per site and read the distribution (entry 1).
- The cost in the VM of an extra `u16` increment (one load, one add, one store) at every `STEPS` for the hotness counter, measured against today's bench on the 21 compute programs where `steps` is 9.1% to 50.0% of executed instructions (entry 4).
- Cherry's per-function compile cost against the function's own run time for the ten busiest sites a cigo call enters (entry 2).
- A cold cigo run against a warm one (cherry entry exists), counted in instructions and in milliseconds, on the seven cigo cases (entry 7).
- Compile size per guarded inline against time saved on a hook call's 1,282 `.push()` method calls (entry 8).
- The rate of guard failures on cigo's `list` and `audit`, where functions see two or three rows of argument types (entry 14).
- Phase 2 of cig-script#256: fib and loop at least 3x faster warm, with a cold start no more than 10% slower than the VM's (the issue's own target).

## The shape of an IR: nanopass and Chez Scheme, Zig's two IRs, the sea of nodes and leaving it, flambda at a distance

[studies/16-ir-shapes-nanopass-zig-sea-of-nodes.md](studies/16-ir-shapes-nanopass-zig-sea-of-nodes.md)

1. The line count of a hand-declared grammar for today's CIR, with structural rules read off it, against today's `verify.rs`. Decides entry 1.
2. For each cig CIR pass (`fold-consts`, `copy-prop`, `sccp`, `dead-pure`), the fraction of its lines that walk `Kind` cases the pass does not change. Decides entry 3.
3. For today's CIR passes, the number of `verify.rs` errors that would have been a Rust type error at build time with language-typed `Instr`. Decides entry 4.
4. The length of a draft CIR-1 grammar as the difference from CIR-0: additions and removals as nanopass `(+ ...) (- ...)`. Decides entry 5 and entry 16.
5. For the 21 compute programs, the number of inlining attempts and the fraction that hit Chez's default `effort-limit` of 200. Decides entry 6.
6. For the 21 compute programs, the number of type checks (`Class::MayRaise` sites) an intraprocedural fixed-point lattice over the nine `Ty` kinds proves redundant. Decides entry 7.
7. Lowering times for the 21 compute programs, split between `lower.rs` (AST to CIR) and `bc/mod.rs` (CIR to bytecode), with and without a per-file cache of CIR-0. Decides entry 9.
8. Inference time at stage D's 500 ms budget on `orchestrate.cig`, with an intern pool of observed types compared with per-value fresh allocations. Decides entry 10.
9. Memory for a typed CIR as a tree of boxed `Instr` nodes against a `MultiArrayList`-style array of fixed-size records, on the `sieve` 300k program. Decides entry 11.
10. Size of a guard with an inline framestate against a pointer to a pin, for a test program with one guard per loop body. Decides entry 14.
11. Specialization rate: for the 21 compute programs, the number of closures observed to be called with a stable set of free-variable values. Decides entry 15.
12. For the CIR-0-to-CIR-1 lowering pass in a prototype, the line count against today's `bc/mod.rs` (3,544 lines) split into "CIR-0 to CIR-1" and "CIR-1 to bytecode" parts; the line count of a first Cranelift lowering from CIR-1. Decides entry 16.
13. For today's four CIR passes, the mean per-function run time on the 21 compute programs, against the same four passes written against a declared CIR grammar with autogenerated walks. Decides entry 3 and informs entry 16.
14. For a prototype static output grammar on CIR, the number of mistakes a planted pass would make that the Rust type checker catches versus the number the run-time verifier catches today. Decides entry 4.
15. For each of V8's reasons to leave the sea of nodes (`v8-leaving-the-sea-of-nodes.html` lists eight categories across the body), whether each reason applies to a cig redesign that chose a sea-of-nodes shape: only reason (g) (control-flow dependent typing) is weaker for cig than for V8, and only on the VM. Decides entry 13.
16. For a prototype verifier whose structural `V` rules are read from a grammar declaration, the line count against today's `verify.rs` (1,182 lines). Decides entries 1 and 4; informs entry 16.
17. For a `--remarks` report emitted by cig-compile's build on `orchestrate.cig` at `-O2`, the number of distinct inlining or specialization decisions compared with the per-round output flambda's chapter documents. Decides entry 15.
18. For the 21 compute programs under a cp0-like inliner at `score-limit` 20 and `effort-limit` 200, the number of inlining attempts that ran to completion (no bug-out), the number that hit `score-limit`, and the number that hit `effort-limit`. Decides entry 6 together with measurement 5.
19. For a typed CIR intern pool populated during a Sema-like pass on `orchestrate.cig`, the number of distinct type values interned and the RSS of the pool compared to a per-value allocation. Decides entry 10.

## From machine code to a program: object emission, embedded linking, size, debug tables, hardening, targets and the bootstrap

[studies/17-from-machine-code-to-a-program.md](studies/17-from-machine-code-to-a-program.md)

- **Per-function Cranelift compile time on the test machine (entry 1).** Run Cranelift on the CIR of every function in cigo (1,038 functions, `data/census/README.md`) at optimization level 2, collect a wall-time distribution and total; compare with the VM lowering time at the same level. Decides entry 1's parallelism target.
- **Embedded-linker lines of Rust for a minimal ELF/PE/Mach-O (entry 5).** From one object plus one archive, measure the lines in `cig-linker` to produce a working 2 MB static ELF, a working 2 MB PE and a working 2 MB Mach-O; the total is the cost of decision (e)'s "in-process" option.
- **`hello.cig` with hardening and release settings (entries 11, 15, 17).** Build cig as a static PIE with `-C opt-level=z -C strip=debuginfo -C link-arg=-Wl,--gc-sections -C link-arg=-Wl,--icf=safe -C link-arg=-Wl,-z,relro -C link-arg=-Wl,-z,now -C relocation-model=pic`, measure the executable's size against the released 17.0 MB. Decides entry 11's baseline.
- **ThinLTO summary size for cigo (entry 10).** Count the bytes a per-function summary (entry, callees, builtins, purity class, approximate size) would be for 1,038 functions; compare with the module's full bytes.
- **`.debug_line` and `.eh_frame` sizes for cigo at stage C (entries 12, 13).** Count the line-program rows per instruction and the number of CIEs/FDEs for 1,038 functions; compare with rustc's `.debug_line` on the rust clone's binaries.
- **Hardening overheads on cig's C dependencies (entry 16).** Compile `zstd` (`data/size/README.md`: 821,181 bytes of code) with `-fsanitize=cfi` and `-fsanitize=shadow-call-stack` (on aarch64), measure the time on `cigcompile bench`'s burn-heavy workloads.
- **The cost of `preserve_frame_pointers` on cig (entry 15).** On the test machine, the geometric mean slowdown on the 21 compute programs at stage C with `preserve_frame_pointers = true` against `= false`; cig-compile#38's pull-name backtrace needs it, and the cost has to be weighed against that benefit.
- **The weight of a symbol table (entry 9).** The symbol table of the released cig is 1,859,692 bytes (`data/size/README.md`); a program needs the symbol table to answer `--backtrace` in pull names (cig-compile#38) and `cigcompile audit` (cig-compile#40). Measure the subset reachable from one CigScript program's compiled code against the whole, so the runtime archive can keep only what a program can name.
- **A stage C hello's section count and layout (entries 5, 8).** On the test machine, the number of ELF sections and the file-offset layout of a stage C `hello` built through the embedded linker against the reserved-section-in-a-PIE-runtime route (entry 8), for each of the three formats; the number that keeps `signtool` and `codesign` happy is the one to pick.
