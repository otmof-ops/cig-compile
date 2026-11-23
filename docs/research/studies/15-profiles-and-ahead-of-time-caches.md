# Profiles that outlive a run, and code compiled ahead of time from them: ART, .NET, and Leyden

- **Serves.** cig-script#256 (cig-cherry), #254 (the module cache); cig-compile#33, #36; decisions (c), (f).
- **Pins.** ART `android-17.0.0_r1` at `b753cf97` (Apache-2.0): `runtime/jit/`, `runtime/oat/`, `compiler/optimizing/`, `dex2oat/`, `libprofile/profile/`, `profman/`. .NET runtime `v11.0.0` at `deae2e32` (MIT): `src/coreclr/vm/`, `src/coreclr/jit/`, `docs/design/`. V8 `15.8.62.31` at `b601a234` (BSD-3-Clause): `src/snapshot/`. Julia `v1.13.4` at `c1ac32ab` (MIT): `doc/src/devdocs/`. Leyden: JEP 483 and JEP 515 only (OpenJDK is GPL and has no tree here).
- **License rule.** ART, .NET, V8, Julia: ideas freely. Leyden: ideas only, described at the level of the JEP texts.
- **Read with.** `papers/compilers/jep-483.html`, `papers/compilers/jep-515.html`, `papers/compilers/source-android-art-jit.html`; `data/census/README.md` and `data/census/report.md` (the figure, in the compute programs and in cigo's script, that bears on counters and the profile).

## What cig should carry away

- **Nothing in cigo gets hot in one run.** 13 to 80 of 1,038 functions are entered, and a site runs 1.1 to 2.3 times on average: an in-process counter has nothing to watch (`data/census/README.md`). A profile between runs says more than any threshold would, as ART's does for apps and Leyden's does with a training run. The counter stays useful in a long compute program, where the census found 4 to 91 sites making 90% of the instructions (entry 4, entry 7).
- **The step counter the VM already keeps can be the hotness counter,** sampled at `STEPS`, and nothing else is charged on hot paths. Step marks are 9.1% to 50.0% of a compute program's executed instructions today (`data/census/README.md`), and that tick already exists; any new counter would sit beside it (entry 4).
- **One profile format,** per function and per instruction pc, that a script's run writes and a later `cigcompile build --profile-use` or `cig-cherry` reads. Carry the operand types pair, the map shape at a field site, the receiver type at a `callm`, and a hit count: a cache keyed on one shape or one type pair would hit at every site of every compute program and 20% to 34% of cigo's field accesses (entry 16).
- **A cache entry's header is where staleness is answered,** as ART, V8 and the oat header all do: magic, version, a hash of what the code describes (module bytes), a hash of what the generator decided from (the profile), the target CPU features and the compiler filter. cig's modcache already carries most of this (`cig-script/src/burn/modcache.rs`), and cig-cherry adds the CPU-features field and a compiler-filter field (entry 10, entry 16).
- **The rule the dual mapping holds.** ART writes to a non-executable view and runs from the executable view of the same memfd (`runtime/jit/jit_memory_region.cc`). A file on disk, mapped read-and-execute, keeps the same promise for cherry at no page-permission cost. Rule 8 forbids a page both writable and executable (entry 13).
- **Version resilience is not cig's problem.** ReadyToRun pays for every cross-module call and field access at a fixup, so a library can change without invalidating the code (`docs/design/coreclr/botr/readytorun-overview.md`). cig's cache is per machine, and a change to the script or to cig gives another key and another entry; the planned payload is reproducible bytes, not a library that other programs link against (entry 9).

## Design points

### 1. ART's hotness counters and per-site inline caches (what the JIT records)

- **What it does, and why.** Every warm method's `ProfilingInfo` is an array of 5-entry `InlineCache` records, one per `INVOKE` dex pc in the method, plus one `BranchCache` per branch dex pc; the method itself has a 16-bit `baseline_hotness_count_` that the baseline JIT decrements on entry and on a back-edge. `InlineCache::kIndividualCacheSize = 5`: the first five receiver classes seen at a call site are kept; a sixth makes the site megamorphic, at which point `number_of_types = kIndividualCacheSize` and the compiler will not inline. The baseline threshold defaults to `GetOptimizeThreshold() = 0xffff` (65535): a method must be baseline-compiled and then have the hotness count drop to zero before `optimizing/optimizing_compiler.cc` recompiles it with the profile. The branch cache has two 16-bit counts (`true_`, `false_`) that the compiler reads to lay hot blocks first. `ProfilingInfo::ComputeSize` lays out the inline caches and branch caches in memory following the struct, so the whole record is one allocation (`profiling_info.h:144-148`).

- **For cig: adapt.** The pairing (per-method counter, per-site type observation, branch counts) is right for cig; the sizes are too big. Decision (c) is one profile and one counter. cig's census found, in the compute programs, that 226 of 227 arithmetic and comparison sites saw one pair of operand types for the whole run, and that all 74 `getfield` and `setfield` sites (5,891,718 executions) saw one record shape and found their key at one position (`data/census/README.md` §3, "In the compute programs"). In cigo's script, 66% to 100% of field reads and writes were at a site that saw two shapes (`data/census/README.md` §6, "In cigo's script"). A per-site entry sized at 1 observation and a count of how many were seen covers every compute program; a second observation covers cigo. Keep it a byte for operand-type pair (`int,int`, `float,float`, `int,float`, `str,str`, `other`), a 16-bit map-shape id, a 16-bit key position, a 16-bit count: 7 bytes a site against ART's `sizeof(InlineCache) = 4 + 5 * sizeof(GcRoot<Class>)` of 24 bytes. cig's `Proto::hints` already holds one `Cell<u32>` per pc, used as a one-slot inline cache (`src/vm/mod.rs:275-285`): the per-function hot counter is a new field proposed on `Proto`, beside `hints`.

- **What would decide it.** Not measured yet: the fraction of cigo call sites at which a second observation is needed, measured by extending the census to keep 2 per site and reading the distribution. The census already says no site here saw more than two shapes.

- **Feeds.** cig-script#256 (what cherry compiles from); cig-script#243 A8 (counters); cig-compile#33 (profile for `--profile-use`); decision (c).

- **Read.** `art-android-17.0.0_r1/runtime/jit/profiling_info.h` (`InlineCache`, `BranchCache`, `ProfilingInfo`, `kIndividualCacheSize`, `baseline_hotness_count_`, `ComputeSize`); `art-android-17.0.0_r1/runtime/jit/jit.h` (`kIndividualSharedMethodHotnessThreshold`, `HotMethodThreshold`); `art-android-17.0.0_r1/runtime/jit/jit_options.cc` (`kJitDefaultOptimizeThreshold`, `kJitDefaultWarmupThreshold`); `cig-script/src/vm/mod.rs` (`Proto`, `hints`); `data/census/README.md`.

### 2. .NET's call-counting stubs and instrumented tiers

- **What it does, and why.** When a method is first called, a `CallCountingStub` is installed in front of its tier-0 entry point. The stub is a small block of hardcoded machine code with a `UINT16` cell beside it: it decrements the cell and jumps to tier-0, and when the cell reaches zero it jumps to `OnCallCountThresholdReachedStub`, which enqueues the method for tier-1 compilation on a background thread (`callcounting.h`, `callcounting.cpp`). Once the method has been promoted the method's code entry point is moved past the stub, so later calls do not run the counter at all (the stub itself is deleted later, in a batched cleanup: `callcounting.cpp:1046`). The default threshold is 30 (`tiered-compilation.md`), clamped to 1 to `UINT16_MAX` in `eeconfig.cpp`. There is a 100 ms tiering delay from the last tier-0 jit so start-up finishes before anything is optimized. The instrumented variant (`Tier0Instrumented`, `Tier1Instrumented`) runs between tier-0 and tier-1 for a method that has become hot, so even R2R code can collect a profile before its tier-1 rewrite (`DynamicPgo-InstrumentedTiers.md`).

- **For cig: adapt.** The idea of counting at a stub that goes away once the threshold is reached is the shape of a cheap counter. cig has no stubs, and will not get them: rule 7 forbids rewriting verified bytecode, and the step budget already runs at every statement. Instead, the VM's existing `STEPS` opcode is the counter's tick (entry 4), and the "threshold reached" path is a flag the dispatch loop reads at `STEPS` and that compiles the function in the background. .NET's "stub removed" is more precisely the method's code entry point being replaced so later calls skip the stub, with stub deletion batched later (`callcounting.cpp:1046`): cig does the same by moving the function's entry pointer to the compiled code when cherry finishes. The 100 ms startup delay has a cheaper form: cigo's start-up census says a call runs 34,841 or so instructions at the script's top level before the first line of the command (94.3% of `cigo --version`, `data/census/README.md` §6), so a counter that starts after the first `call` (leaving function 0 behind) ignores all that.

- **What would decide it.** Not measured yet: cherry's per-function compile cost against the function's own run time, for the ten busiest sites a cigo call enters (0.9% to 19.2% of a call, `data/census/report.md`).

- **Feeds.** cig-script#256 (cherry's decision); cig-script#243 A8; decision (c).

- **Read.** `dotnet-runtime-v11.0.0/src/coreclr/vm/callcounting.h` (`CallCountingStub`, `CallCountingInfo`, `TargetForThresholdReached`); `dotnet-runtime-v11.0.0/src/coreclr/vm/callcounting.cpp` (`OnCallCountThresholdReachedStub`); `dotnet-runtime-v11.0.0/src/coreclr/vm/tieredcompilation.cpp` (`TieringDelay`, `IsTieringDelayActive`); `dotnet-runtime-v11.0.0/src/coreclr/vm/eeconfig.cpp` (`TC_CallCountThreshold`); `dotnet-runtime-v11.0.0/docs/design/features/tiered-compilation.md`; `dotnet-runtime-v11.0.0/docs/design/features/DynamicPgo-InstrumentedTiers.md`.

### 3. Counters placed on a spanning tree

- **What it does, and why.** Rather than counting every block or every edge, .NET's JIT picks a spanning tree of the control-flow graph and instruments only the non-tree edges: tree-edge counts are reconstructed from the non-tree ones at read time. `fgprofile.cpp` evolves the tree in a maximal-cost depth-first order, walked by `WalkSpanningTree`, with throw blocks ignored to keep the shape stable (`fgprofile.cpp:811` names it "maximal cost depth first", a locally optimal choice at each step rather than a global maximum). A method with `B` blocks and `E` edges carries `E - B + 1` counters, often an order of magnitude fewer than a count on every edge. The comment there calls out that the DFST is deterministic so a later pass can rebuild the same tree.

- **For cig: take.** The idea is the right shape for cig-compile's `--profile-gen` build (cig-compile#33): CIR's `Block { instrs }` with its terminators gives the graph a counter wants. cig does not need it today for cherry, because cherry would sample at `STEPS` (entry 4), not place counters at every edge; but when cigcompile builds a profile-gen program it should use the spanning-tree placement for the `--profile-gen` instrumentation, rather than count every block. The determinism point matters for cig too: `-O2` and `-O2 --profile-gen` must agree on which edges carry counters, so the profile from a `-O2 --profile-gen` build can be read by the `-O2` build's instrumentation reader.

- **What would decide it.** Decided by rule N/A: it is an efficiency choice inside a planned pass, not a correctness one. The .NET doc says the aim is "reduce the number of counters" and it is what their fixed point uses.

- **Feeds.** cig-compile#33 (`--profile-gen`); decision (c).

- **Read.** `dotnet-runtime-v11.0.0/src/coreclr/jit/fgprofile.cpp` (`WalkSpanningTree`, `SpanningTreeVisitor`, `VisitNonTreeEdge`); `cig-script/src/lower/cir/ir.rs` (`Block`, `Instr`).

### 4. cig counts steps already (rule 3): the step counter as the hotness counter

- **What it does, and why.** The VM already charges one tick per statement and per loop iteration at a `STEPS` opcode, and raises `E515` at the mark's own span when the budget is spent (`cig-script/docs/BYTECODE.md`, `cig-script/src/lower/cir/`: `Step(StepSite)`). The census found `steps` is 9.1% to 50.0% of a compute program's executed instructions, median 23.8% (compute programs), and 1% to 13% of a cigo call (cigo's script, `data/census/README.md` §11): a running cost that would be paid anyway. Each `STEPS` is one place the VM already looks: a per-function counter sitting beside it adds one load, one add and one store at every `STEPS` (not zero, but on a path that already exists), provided the counter is a field of `Proto` (the per-function record) and not held in each frame.

- **For cig: adapt.** The step tick is exact and already pays: the hotness counter rides it. The difference between a step tick and a hotness counter is real: a step is per-statement and bounded by `E515`, so it never saturates; a hotness counter must saturate at a threshold and then stop. Keep two proposed values on `Proto`: `steps_here` (what the budget charges, unchanged, the VM already carries it) and a new `hot_count: Cell<u16>` beside `hints`, incremented from `STEPS` while below the threshold, left alone once reached. The increment is not free: it adds one more load, add and store per `STEPS`, on what is already 9.1% to 50.0% of a compute program's executed instructions (median 23.8%, `data/census/README.md` §1, "In the compute programs"); whether it is measurable against the VM's per-step cost is a measurement under "Measurements owed". What a shared counter does and does not tell: it tells which function is doing the work, because `STEPS` fires on every statement in it; it does not tell which site inside the function the work falls on. For that cig needs the per-site count of entry 1. In cigo there is no hot function in the one-run sense: the census says a site runs 1.1 to 2.3 times, in cigo's script. The counter is still the right thing for the function cache, because it is the signal `cig cherry warm` uses on a training run.

- **What would decide it.** Not measured yet: whether the increment under `STEPS` in the VM is measurable against today's bench (a count of a `u16` in `Proto` is one `cell` write per step); the census records a count, not a time.

- **Feeds.** cig-script#256 (cherry's trigger); cig-script#243 A8; decision (c).

- **Read.** `cig-script/src/lower/cir/ir.rs` (`Step`, `StepSite`); `cig-script/crates/cig-bytecode/src/opcode.rs` (`STEPS`); `cig-script/src/vm/mod.rs` (`Proto`, `hints`); `cig-script/docs/BYTECODE.md`; `data/census/README.md`.

### 5. ART's profile on disk: format, merging, filtering

- **What it does, and why.** ART's profile file starts with a 4-byte magic `'p','r','o','\0'` and a 4-byte version `'0','1','5','\0'` (or `'0','1','6','\0'` for a boot-image profile; `profile_compilation_info.cc:71-79`). The header is followed by six section types in the order `kDexFiles`, `kExtraDescriptors`, `kClasses`, `kMethods`, `kAggregationCounts`, `kClassesNoPreload` (`profile_compilation_info.cc:214-247`). Per dex file the profile stores the file's name, checksum, `num_type_ids`, `num_method_ids`: a dex file whose checksum or `num_method_ids` changed is not matched to its entries, and the profile silently ignores them (`profile_compilation_info.h:305-315`). Per method it stores a 32-bit flag word (`MethodHotness::Flag`: `kFlagHot`, `kFlagStartup`, `kFlagPostStartup`, `kFlag32bit`, `kFlag64bit`, `kFlagSensitiveThread`, `kFlagAmStartup`, `kFlagAmPostStartup`, startup bins for relative order) and an inline-cache map `DexPc -> DexPcData`, where each `DexPcData` holds up to `kIndividualInlineCacheSize = 5` type indices; a sixth distinct type sets `is_megamorphic`, and a type the profile cannot encode sets `is_missing_types` (`profile_compilation_info.h:136-166`; `kIsMegamorphicEncoding = 7`, `kIsMissingTypesEncoding = 6` in `.cc`). `MergeWith` combines two profiles for the same dex file; `ProfileFilterFn` discards entries whose dex location is not of interest to this compile (`profile_compilation_info.h:473-487`). The writer runs on a background thread (`profile_saver.cc`) whose constants sit in `profile_saver_options.h`: `kMinSavePeriodMs = 40 * 1000` (40 seconds, line 24), `kMinMethodsToSave = 10` (line 28), `kInlineCacheThreshold = 4000` (line 32), with an exponential back-off bounded by `kMinSavePeriodMs * 16` in the `profile_saver.cc:186` loop where JIT notifications are not available.
- **For cig: adapt.** cig's profile has fewer things to record: operand-type pairs for arithmetic, a map shape id for field sites, a receiver type for `callm`, a call-site callee for `call`, a hit count, and function flags (`hot`, `startup`). Reuse the shape: a magic, a 4-byte version, a section table, a per-module record naming the module's hash (cig has one module hash from `cig-bytecode`, no dex file), then per function a flag word and a per-pc map `pc -> siteData`. Three section types to start: a modules section (hash and lowering version), a functions section (flags), a sites section (per pc). Keep a megamorphic marker (`>8` observations) and a missing-types marker (a `pc` whose type would not fit the encoding); a second observation is enough for cigo, as the census says (entry 1). The writer does not need a background thread for `cig cherry warm` (the run is deliberate), and a plain `cig run` writes at process exit so the step budget is never charged for the save (rule 3).

- **What would decide it.** Not measured yet: the per-call size of a cigo-shaped profile with this format against the bytecode module's size (`cigcompile build` 4 MB budget, cig-compile#36). ART's HelloStreamWarmup adds 250 KB to the AOT cache (JEP 515).

- **Feeds.** cig-script#256; cig-compile#33 (`--profile-use`); decision (c).

- **Read.** `art-android-17.0.0_r1/libprofile/profile/profile_compilation_info.h` (`kProfileMagic`, `kProfileVersion`, `kIndividualInlineCacheSize`, `MethodHotness`, `DexPcData`); `art-android-17.0.0_r1/libprofile/profile/profile_compilation_info.cc` (`FileSectionType`, `FileHeader`, `kIsMegamorphicEncoding`, `kIsMissingTypesEncoding`); `art-android-17.0.0_r1/runtime/jit/profile_saver_options.h` (`kMinSavePeriodMs`, `kMinMethodsToSave`, `kInlineCacheThreshold`); `art-android-17.0.0_r1/runtime/jit/profile_saver.cc`; `art-android-17.0.0_r1/profman/profile_assistant.cc`.

### 6. .NET's static profiles (.mibc)

- **What it does, and why.** `dotnet-pgo create-mibc --trace trace.nettrace --output trace.mibc` turns an ETW trace of a .NET run into a `.mibc` file, and `dotnet publish -p:PublishReadyToRun=true -p:ReadyToRunOptimizationData=trace.mibc` passes it to the crossgen2 ahead-of-time compiler so the published R2R code is laid out from it. The profile schema is the same kind of thing as the dynamic one: block counts, type histograms at indirect-call sites (`pgo.cpp`, `src/coreclr/vm/pgo.h`). `merge` combines mibc files from multiple scenarios into one: the `merge` command exists because one mibc from one scenario would miss the application's warm paths.

- **For cig: adapt.** cig-compile#33's `--profile-use` is this role, and the comment cig-compile#33.newcomments ties it to cig-cherry's cache: one format, written by `cig run` or `cig cherry warm`, read by `cigcompile build --profile-use`. The merge command maps directly: `cig profile merge a.cigprof b.cigprof --out c.cigprof` for the cigo cases (version, help, list, audit, hook, guard), because one of them is not a representative run of the program.

- **What would decide it.** Not measured yet: whether a profile merged from the seven cigo cases makes the ten busiest sites cover enough of a hook call to be worth cherry's compile.

- **Feeds.** cig-compile#33 (`--profile-use`); cig-script#256 (one profile for both).

- **Read.** `dotnet-runtime-v11.0.0/docs/design/features/dotnet-pgo.md`; `dotnet-runtime-v11.0.0/src/coreclr/vm/pgo.h` (`Header`, `HeaderList`, `getPgoInstrumentationResultsInstance`); `dotnet-runtime-v11.0.0/src/coreclr/vm/pgo.cpp`.

### 7. Leyden's training run and AOT cache (JEP 483 and JEP 515)

- **What it does, and why.** JEP 483 adds an AOT cache that stores classes in a loaded and linked form, produced by a two-step command: a training run `-XX:AOTMode=record -XX:AOTConfiguration=app.aotconf` records the configuration, and `-XX:AOTMode=create -XX:AOTConfiguration=app.aotconf -XX:AOTCache=app.aot` builds the cache without running the program again. Production runs with `-XX:AOTCache=app.aot` get those classes for free; on a short program the JEP reports close to 600 JDK classes loaded, parsed and linked at startup, which is the work the training run avoids. The format of the configuration and cache files is explicitly not specified: the cache is treated as a per-JVM-version artifact that can be regenerated if the JVM changes. JEP 515 extends the same cache to method execution profiles collected during the training run, so the JIT can start compiling from them at startup instead of waiting to collect its own. The JEP notes that production profiles do not replace the cached ones: the JIT "fuses the benefits of AOT profiles, on-line profiling, and JIT compilation" so an application whose behavior in production diverges from training is still optimized correctly. JEP 515's HelloStreamWarmup example runs in 90 ms with an AOT cache without profiles and 73 ms with profiles, a 19% improvement; the cache with profiles is 250 KB larger (about 2.5% more).
- **For cig: take.** `cig cherry warm script.cig` is the training run, already named in cig-script#256; its output is cherry's profile and its compiled hot functions, stored under the tin and keyed as `cig-script/src/burn/modcache.rs` keys lowered modules today. Leyden's "two-step workflow" is what cig already does: lower once at build or at warm, run many times from the cache. The principle that production profiles keep updating the cached ones carries across: a later `cig run` whose observed types do not match the profile writes new entries beside the warm ones, and cherry recompiles on the next idle cycle. Nothing about the cache being a training-run artifact crosses rule 8: the file is mapped read-and-execute, never written and executed at once.

- **What would decide it.** Not measured yet: the difference between a cold cigo run (no cherry entry) and a warm one (an entry exists), on the seven cigo cases, counted in instructions and in milliseconds.

- **Feeds.** cig-script#256 (cherry's `warm`, the cache); cig-compile#33; decision (f).

- **Read.** `papers/compilers/jep-483.html`; `papers/compilers/jep-515.html`; `cig-script/src/burn/modcache.rs`.

### 8. ART's compiler filters and guarded inlines from profile data

- **What it does, and why.** `dex2oat` takes a compiler-filter argument that decides what it does: `kVerify` only verifies, `kSpeedProfile` compiles the methods the profile flags as hot (and startup methods on non-low-memory devices), `kSpeed` compiles everything that is not too big, `kEverything` compiles everything including the un-hot (`dex2oat/dex2oat.cc:1222`, `dex2oat/dex2oat_options.def`). The filter is recorded in the oat file's key-value store under `kCompilerFilter`. `compiler/optimizing/inliner.cc` turns a profile's five-entry `InlineCache` into a guarded inline: if the receiver is of the one class the profile saw (`kInlineCacheMonomorphic`), emit a type guard and the inlined body with a fallback call on miss; two to four classes (`kInlineCachePolymorphic`) give a chain of guards; `kInlineCacheMegamorphic` leaves the virtual call alone. `kUseAOTInlineCaches = true` keeps this on for ahead-of-time builds.

- **For cig: adapt.** cig has no virtual dispatch to devirtualize, but `callm` (a method) resolves against a receiver's type and today caches that in `Proto::hints`. The guarded-inline shape is the right thing for cherry: a `callm` or `callb` whose profile says one receiver (or two) becomes a type check and a direct call to the resolved builtin or function; a miss falls back to the VM at the bytecode instruction, with the frame rebuilt. The census found all 716,674 method calls were at a site with one receiver type, and 28 of 29 compute-program `call` sites called one function: the hit rate is high enough for one check to pay. The filter maps to a cherry decision: `verify` is today's VM run, `speed-profile` is cherry compiling from a profile, `speed` is `cigcompile build -O2`.

- **What would decide it.** Not measured yet: compile size per guarded inline against time saved, on a hook call's 1,282 `.push()` method calls (`data/census/report.md`).

- **Feeds.** cig-script#256 (cherry's inlining); cig-compile#33; decision (b).

- **Read.** `art-android-17.0.0_r1/dex2oat/dex2oat.cc` (`CompilerFilter::kSpeedProfile`, `kVerify`, `kSpeed`); `art-android-17.0.0_r1/dex2oat/dex2oat_options.def`; `art-android-17.0.0_r1/dex2oat/driver/compiler_driver.cc` (`ShouldCompileBasedOnProfile`, `IsHotMethod`, `IsStartupMethod`); `art-android-17.0.0_r1/compiler/optimizing/inliner.cc` (`GetInlineCacheType`, `TryInlineMonomorphicCall`, `TryInlinePolymorphicCall`, `kUseAOTInlineCaches`); `cig-script/src/vm/mod.rs` (`hints`, callm hint read).

### 9. ReadyToRun code replaced by tier-1 code; version resilience

- **What it does, and why.** ReadyToRun is a managed executable with native code embedded; the format is designed so a library's precompiled code stays valid when another library it calls is updated. The way it stays valid is to pay for every cross-module call through an indirection: a non-virtual call across modules goes through a fixup slot that `RUNTIME_ENTRY_FIXUP_METHOD` fills in lazily on first use (`readytorun-overview.md`). The field layout of `System.Object` and of public value types is fixed as a breaking-change rule, so a field access whose inheritance chain stays inside one "version bubble" has a native constant offset and needs no fixup; only cross-bubble inheritance adds a `SIZE_OF_BASECLASS` indirection as a data-section slot filled at module load (`readytorun-overview.md` §"Field access", around lines 177-212). Tiered compilation then watches the R2R code: when a method is called more than 30 times its R2R code is replaced by a tier-1 jit from the current profile, which is now free to inline and to bake in offsets that R2R had to keep indirect for the cross-bubble case (`tiered-compilation.md`, "Steady-State").

- **For cig: adapt.** The "tier 0 then tier 1 under the same entry point" shape maps to cherry: the VM is tier 0; `cig cherry warm` builds tier 2 (quick baseline); a tier-3 full-optimizer build happens for functions that stay hot across warm runs; a call site returns to the VM if a guard trips. The version-resilience fixups are rejected by rule 10 (ahead of time by default) and the fact that cig's cache is per machine and keyed to the exact script bytes: cig's cache is not a library that another program links against, and a change to cig, to the lowering, to the script or to the hardening level produces another key and another entry (entry 10).

- **What would decide it.** Decided by rule 10 for the version-resilience part. Not measured yet for the tiered shape: whether cherry's tier 2 beats the VM on fib with no guards, as cig-script#256's phase 2 target.

- **Feeds.** cig-script#256 (tiers); cig-compile#36 (startup and size).

- **Read.** `dotnet-runtime-v11.0.0/docs/design/coreclr/botr/readytorun-overview.md` (version bubbles, fixups, `RUNTIME_ENTRY_FIXUP_METHOD`); `dotnet-runtime-v11.0.0/docs/design/coreclr/botr/readytorun-format.md`; `dotnet-runtime-v11.0.0/docs/design/features/tiered-compilation.md` ("Steady-State").

### 10. The oat header and what it checks

- **What it does, and why.** Every .oat file starts with a fixed header: `kOatMagic = 'o','a','t','\n'`, `kOatVersion = '2','7','5','\0'`, a `uint32_t oat_checksum_`, `InstructionSet instruction_set_`, `uint32_t instruction_set_features_bitmap_`, a dex-file count, offsets to internal tables and to the key-value store (`runtime/oat/oat.h:47-220`). The key-value store names the compiler filter (`kCompilerFilter`), the class path (`kClassPathKey`), the boot class path and its checksums (`kBootClassPathKey`, `kBootClassPathChecksumsKey`), the apex versions (`kApexVersionsKey`), and whether the file assumes a particular SDK level (`kAssumeValueSdkIntKey`). The checksum covers the deterministic fields and the file body; non-deterministic fields (`kDex2OatCmdLineKey`, `kApexVersionsKey`) are padded to fixed lengths and excluded from the checksum, so the same inputs on different machines give the same checksum. `GetChecksum()` is read when the file is loaded.

- **For cig: take.** cig-cherry's cache-entry header is this kind of thing: a magic, a version, a checksum, the target ISA and features, the compiler filter, and a hash of what the code describes. cig's `modcache.rs` already carries `MAGIC = b"CIGMOD1\0"`, `FORMAT = 1`, a 16-byte build digest, the key, the module's length, and a MAC. Cherry extends it: add the target instruction set and feature bitmap, the compiler-filter name (`vm`, `speed-profile`, `speed`), and a hash of the profile the code was compiled from.

- **What would decide it.** Decided by rule 8 and rule 10: a signed, versioned header is how the mapped read-and-execute file is trusted. The specific fields are a straight translation of ART's fields that bear on code reuse.

- **Feeds.** cig-script#256 (the cache entry); cig-script#254 (the module cache); decision (f).

- **Read.** `art-android-17.0.0_r1/runtime/oat/oat.h` (`OatHeader`, `kOatMagic`, `kOatVersion`, `kCompilerFilter`, `kClassPathKey`, `kBootClassPathChecksumsKey`, `kApexVersionsKey`, `kAssumeValueSdkIntKey`, `GetChecksum`); `cig-script/src/burn/modcache.rs` (`HEADER_LEN`, `MAGIC`, `FORMAT`, `header`, `mac`).

### 11. V8's code cache sanity check

- **What it does, and why.** A V8 code cache (produced by `ScriptCompiler::CreateCodeCache`) carries a header with eight 32-bit or larger fields: `kMagicNumber`, `kVersionHash` (hash of the V8 version), a 32-byte `kSourceHash` (SHA-256 of the script source, wrapped arguments and origin options), `kFlagHash` (hash of the V8 flags that would affect code), `kReadOnlySnapshotChecksum` (hash of V8's read-only snapshot), `kPayloadLength`, `kTrustedPayloadLength`, `kChecksum` of the payload (`src/snapshot/code-serializer.h:111-130`). On load, `SanityCheckWithoutSource` (`code-serializer.cc:852-888`) tests seven things in order: header size (`size_ >= kHeaderSize`, else `kInvalidHeader`), magic number (`kMagicNumberMismatch`), version hash (`kVersionMismatch`), flag hash (`kFlagsMismatch`), read-only snapshot checksum (`kReadOnlySnapshotChecksumMismatch`), both length fields (`payload_length <= max_payload_length` and `trusted_payload_length <= payload_length`, both `kLengthMismatch`), and, when `v8_flags.verify_snapshot_checksum` is set, the payload's own checksum (`kChecksumMismatch`). `SanityCheckJustSource` then tests the source hash (`kSourceMismatch`). Any failure drops the cache silently.

- **For cig: take.** The set of things to hash and check is the clearest list I have read: magic, engine version, a flag hash (what cig's `--engine` and hardening level would be), a source hash, a payload length and a payload checksum. The flag hash is what `jit_options.cc` would be for ART if it had a cache; V8 computes it once per process from `FlagList::Hash()`. cig's cache would add it: `jit flags hash` of the `cig version`, `lowering version`, `level`, the target CPU features, and the compiler filter.

- **What would decide it.** Decided by rule 10 and the design of a cache entry: without a flag hash and a version hash, a cache entry can be loaded under different flags from the one that wrote it.

- **Feeds.** cig-script#256; decision (f).

- **Read.** `v8-15.8.62.31/src/snapshot/code-serializer.h` (`SerializedCodeData`, `SourceHash`, `kVersionHashOffset`, `kFlagHashOffset`, `kReadOnlySnapshotChecksumOffset`, `kChecksumOffset`); `v8-15.8.62.31/src/snapshot/code-serializer.cc` (`SanityCheck`, `SanityCheckWithoutSource`, `SanityCheckJustSource`, `FlagList::Hash`).

### 12. ART's `oat_file_assistant` and the staleness statuses

- **What it does, and why.** On a dex file's first load, `oat_file_assistant.cc` decides whether a precompiled oat file is still usable. The status is one of `kOatUpToDate`, `kOatCannotOpen`, `kOatDexOutOfDate` (the dex file's checksum does not match the oat's record), `kOatBootImageOutOfDate` (the boot image this oat built against has been updated), `kOatContextOutOfDate` (the class loader context has changed), `kOatAssumedValuesOutOfDate` (the assumed SDK level is no longer current). A stale oat is regenerated at the next idle cycle, from the dex and the current profile. `DexoptTrigger` compares the current filter with the target filter (`kComparingCompilerFilter`), and compiles only if the target is better.

- **For cig: take.** cig's cache entry needs these exact questions put to it on load: is the entry's magic right, is its header's build digest the current one, does the module hash match the script's bytes, is the entry's target ISA and features the current machine, is its compiler filter at least as good as the one we would ask for. The modcache already holds the first three; the ISA and filter checks are the ones cherry adds. The names map: `kOatUpToDate` is "hit", `kOatCannotOpen` and the checksum mismatches are "lower and store". `kOatContextOutOfDate` has no cig cognate yet: cig has no class loader context. `kOatAssumedValuesOutOfDate` maps to the hardening level: an entry built for Standard must not be used in Hardened.

- **What would decide it.** Decided by rule 8 (nothing executes that has not been verified against the input it was built from).

- **Feeds.** cig-script#256 (loader and fallback); cig-script#254 (module cache); decision (f).

- **Read.** `art-android-17.0.0_r1/runtime/oat/oat_file_assistant.h` (`OatStatus`, `kOatUpToDate`, `kOatCannotOpen`, `kOatDexOutOfDate`, `kOatBootImageOutOfDate`, `kOatContextOutOfDate`, `kOatAssumedValuesOutOfDate`, `DexoptComparator`, `DexoptTrigger`); `art-android-17.0.0_r1/runtime/oat/oat_file_assistant.cc`.

### 13. ART's dual mapping of the JIT code cache (rule 8)

- **What it does, and why.** `jit_memory_region.cc` creates one `memfd_create("jit-cache")`, truncates it to the cache's capacity, and maps it twice: once as RW (`non_exec_pages_`), once as RX (`exec_pages_`), both backed by the same bytes. Code updates are written to the RW mapping and read from the RX mapping; no page is ever both writable and executable. The ASCII diagram in the file labels one view "non exec code" and the other "exec code" and shows the VA pointing at one file-backed PA (`jit_memory_region.cc:115-138`). `MadviseDontFork` is applied so a fork does not duplicate the writable view, and on the zygote the writable mapping is sealed so no new one can be made. `JitMemoryRegion` holds `writable_data_pages_`, `data_pages_` (RO), `exec_pages_` (RX), `non_exec_pages_` (RW), and `TranslateAddress` converts a source pointer in one view to the address in its twin (`jit_memory_region.h:180-230`). Where `memfd_create` is unavailable or `rwx_memory_allowed` is set, the file falls back to one view and flips RWX to RX around each write; this is the case the code calls out as a last resort and refuses outright where `rwx_memory_allowed` is false, so the initialization returns `false` and the caller gets an error message.
- **For cig: take.** cig-cherry's native code is a file on disk (the tin), mapped read-and-execute (`mmap(..., PROT_READ | PROT_EXEC, MAP_PRIVATE, fd, 0)`). The file is written and signed before it is mapped, and the mapping has no write permission. ART's dual view is for a code cache that must be updated in place; cig does not update in place, so the dual view is not needed. What is taken is the design discipline: no path in the loader asks for a page both writable and executable, and the single-view RWX fallback is forbidden. The signing step and the MAC the modcache uses today (`modcache.rs`) is what the cherry entry carries too.

- **What would decide it.** Decided by rule 8.

- **Feeds.** cig-script#256 (loader); decision (f).

- **Read.** `art-android-17.0.0_r1/runtime/jit/jit_memory_region.cc` (`memfd_create`, `exec_pages`, `non_exec_pages`, `MadviseDontFork`, `rwx_memory_allowed`); `art-android-17.0.0_r1/runtime/jit/jit_memory_region.h` (`JitMemoryRegion`, `GetNonExecutableAddress`, `GetExecutableAddress`); `art-android-17.0.0_r1/runtime/jit/jit_scoped_code_cache_write.h`.

### 14. When the profile is wrong: falling back and bounding the cost

- **What it does, and why.** ART's guarded inline from an `InlineCache` leaves a fallback: a receiver class the profile did not record falls through the type guard to a general virtual call (`compiler/optimizing/inliner.cc`, `TryInlineMonomorphicCall` and the deoptimization path). .NET's tiered compilation throws the tier-1 code away and goes back to tier-0 if a profile-guided speculation is invalidated; on-stack replacement rewrites a running frame to the alternate tier (`OnStackReplacement.md`, patchpoints in `src/coreclr/jit/patchpoint.cpp`). .NET's prototype OSR work spells out the complications: the two frames may have different layouts, live locals must be mapped from the old frame to the new one, and the IL state at the OSR point must be reachable from both compilers' code models; these are the same questions cig faces for a snapshot. What bounds the cost: ART caps `baseline_hotness_count_` at a `uint16_t`, so a method that keeps re-collecting the profile hits the cap and stops being recompiled; .NET deletes a call-counting stub after the threshold is reached, and the method is not re-counted. A function that keeps falling back is not a native-speed function and both systems stop trying. ART's `optimizing_compiler.cc` marks a method as "not compilable" when the compiler returns an error for it twice, so dex2oat does not retry it.
- **For cig: take.** The fall-back shape is right for cherry: a failed type guard goes back to the VM at the bytecode instruction, with the frame rebuilt. The cost is bounded three ways: cherry does not compile below a threshold, cherry does not recompile a function that was just recompiled, cherry blacklists a function that keeps failing (cig-script#256 §3, Luajit's blacklisting is the model). The step budget crosses between tiers unchanged: `E515` still falls on the same statement, because cherry charges one tick per statement as the VM does (rule 3).

- **What would decide it.** Not measured yet: the rate of guard failures on cigo's `list` and `audit`, where functions see two or three rows of argument types (`data/census/report.md`).

- **Feeds.** cig-script#256 §3; decision (d).

- **Read.** `art-android-17.0.0_r1/compiler/optimizing/inliner.cc` (`TryInlineMonomorphicCall`, `GetInlineCacheType`, `kInlineCacheMegamorphic`); `dotnet-runtime-v11.0.0/docs/design/features/OnStackReplacement.md`; `dotnet-runtime-v11.0.0/src/coreclr/jit/patchpoint.cpp`; `cig-script/src/vm/mod.rs`.

### 15. Julia's package images: how the cache is keyed to the flags that built it

- **What it does, and why.** A Julia package image is a shared library of native code plus data, built once per package and loaded as a snapshot on `using`. The file is rejected unless certain flags match the Julia session's flags: `-g` (debug info), `--check-bounds`, `--inline`, `--pkgimages` must match exactly (they change code generation), `-O` must be at least as high as the file's (higher levels can still load lower-built files). The file also carries a `JULIA_CPU_TARGET` string, and multi-versioning lets one file carry code for several CPU microarchitectures. The file is actually linked at use time: Julia ships LLD so a user with no system linker still has one (`base/linking.jl`). The number of cached variants is capped by `JULIA_MAX_NUM_PRECOMPILE_FILES`.

- **For cig: adapt.** The flag-match rule is the practical form of entry 11's `kFlagsMismatch`: a short list of flags that affect code generation, matched exactly, and a short list that may differ up or down. cig's flags that matter: the engine (`vm` vs cherry), the hardening level (Standard, Hardened, Fortress), the optimization level. Cherry's cache entries carry each one and reject on mismatch. cig's cache is per machine, so one CPU target is enough: the multi-versioning idea is not needed here (and cig has no linker step for the user to be given).

- **What would decide it.** Decided by rule 10: the file on disk is signed and keyed to all the inputs that could change its bytes.

- **Feeds.** cig-script#256 (what to match before mapping); decision (f).

- **Read.** `julia-v1.13.4/doc/src/devdocs/pkgimg.md` (flag-match rules, multi-versioning, linking); `papers/compilers/jep-515.html` (ground this entry against for the AOT-cache comparison).

### 16. A proposed profile record and cache entry header for cig

- **What it does, and why.** Given entries 1 to 15, this is the synthesis the brief asks for: the record cig's VM writes, and the header cig's loader reads. Keep the profile as one file a script, next to the module cache's entry under the tin, with a stable section layout (entry 5).

Per function the profile stores: `flags` (`hot`, `startup`, `pure`, `has_burn`), `calls` (count), `hot_count` (saturating counter of entry 4). Per pc inside a function: one site record, sized to fit a small cache line.

| field | bytes | notes |
|---|---:|---|
| opcode | 1 | distinguishes arithmetic, field, index, call, callm, callb |
| observations_tag | 1 | 0 to 8 (observed count), 9 = megamorphic, 10 = missing-type (from ART) |
| hit_count | 2 | saturating u16 (as .NET's `CallCount`) |
| obs_1 | 2 or 4 | operand-pair id, map-shape id, or a function index for call sites |
| obs_2 | 2 or 4 | second observation; one is enough for compute, two for cigo field sites |

The cache entry's header (native code, a signed file in the tin). The "state" column says whether the field exists in cig today or is proposed for cherry; today's fields are read from `cig-script/src/burn/modcache.rs` lines 58-65 (`MAGIC = *b"CIGMOD1\0"`, `FORMAT = 1`, `HEADER_LEN = 8 + 4 + 4 + BUILD_LEN + 32 + 8 + 32`, `BUILD_LEN = 16`, MAC is a keyed BLAKE3 at `mac` line 158).

| field | bytes | state | source |
|---|---:|---|---|
| magic | 8 | today `CIGMOD1\0`; proposed to rename to a cherry-specific value when cherry ships | cig today (modcache line 58) |
| format version | 4 | today | cig today (modcache line 59) |
| flag bits | 4 | today (`FLAG_WARNED`); proposed to add engine, hardening, filter | cig today (modcache line 62); V8 `kFlagHashOffset` |
| flag hash | 4 | proposed | V8 (`kFlagHashOffset`), Julia (flag-match rules) |
| cig build digest | 16 | today | cig today (modcache `build()` line 84) |
| lowering version | 4 | today (in the key, not the header) | cig today (modcache `key_for` line 124) |
| hardening level | 1 | proposed | cig (Standard / Hardened / Fortress, cig-os#39) |
| compiler filter | 1 | proposed | ART (`kCompilerFilter`: `vm`, `speed-profile`, `speed`) |
| target ISA | 1 | proposed | ART (`instruction_set_`) and Julia (`JULIA_CPU_TARGET`) |
| ISA features | 8 | proposed | ART (`instruction_set_features_bitmap_`) and Julia (multi-versioning) |
| module hash | 32 | today (the key is this) | cig today (modcache header line 64); ART (dex checksum); V8 (`kSourceHash`) |
| profile hash | 32 | proposed | ART (profile versioning) and .NET (mibc hash in R2R data) |
| payload length | 8 | today | cig today (modcache line 65); V8 (`kPayloadLength`) |
| trusted payload length | 8 | proposed (for a cherry entry with signed and unsigned regions) | V8 (`kTrustedPayloadLength`) |
| payload BLAKE3 | 32 | today (in the module header the MAC covers) | cig today (modcache header comment lines 18-21) |
| payload checksum | 4 | proposed (a cheap check before the MAC) | V8 (`kChecksum`) |
| read-only snapshot checksum | 4 | proposed where cherry ships a baseline runtime image | V8 (`kReadOnlySnapshotChecksum`) |
| MAC | 32 | today (keyed BLAKE3 over header + module header) | cig today (modcache line 158) |

- **For cig: take.** This is the answer to decisions (c) and (f): one profile format, one cache-entry header. The profile's shape is from ART, scaled down by the census; the header's fields are from V8, ART and cig. The loader runs the same questions ART's `oat_file_assistant` runs (entry 12), and rejects on any mismatch.

- **What would decide it.** Decided by the entries above for each field. Not measured yet: the size of the profile against a cigo call's bytecode module.

- **Feeds.** cig-script#256; cig-script#254; cig-compile#33; decisions (c), (f).

- **Read.** `cig-script/src/burn/modcache.rs` (today's header and MAC); `art-android-17.0.0_r1/runtime/oat/oat.h` (`OatHeader`); `v8-15.8.62.31/src/snapshot/code-serializer.h` (`kVersionHashOffset`, `kSourceHashOffset`); `art-android-17.0.0_r1/libprofile/profile/profile_compilation_info.h` (`MethodHotness`, `DexPcData`); `data/census/README.md`.

### 17. ART's `profile_assistant` and "enough has changed to recompile"

- **What it does, and why.** `profman/profile_assistant.cc` is the glue between the running app's saved profile and `dex2oat`'s next idle compile. It compares the current profile with the reference profile that drove the last `dex2oat` and reports one of three decisions: `kSkipCompilationSmallDelta` when the profile has not changed enough to pay for a recompile, `kSkipCompilationEmptyProfiles` when no methods are marked hot, or go-ahead when the delta is above the thresholds. The thresholds are tuned per device class and read from the profile saver's constants: `kMinSavePeriodMs = 40 * 1000`, `kMinMethodsToSave = 10`, `kMinClassesToSave = 10`, `kInlineCacheThreshold = 4000` (`profile_saver_options.h`). The decision runs on the device, not on the build server: Android compiles against profiles the device's apps actually generated.

- **For cig: take.** cig's equivalent is a `cig tin maintain` step that compares the warm-run profile against the one cherry last compiled from: if the new profile has enough new hot functions (say 3, as a scaled-down version of ART's 10), recompile; if not, keep the entry. The thresholds are decisions (c) alongside the counter, and they are per-machine, as cig's cache is. The three decision kinds map one for one: "skip, small delta", "skip, empty", "go".

- **What would decide it.** Not measured yet: for cigo, the delta between a `--version` call's profile and a `list` call's profile, counted in new hot functions and new sites.

- **Feeds.** cig-script#256 (what triggers recompile); cig-script#254 (what the modcache's sweep does); decisions (c), (f).

- **Read.** `art-android-17.0.0_r1/profman/profile_assistant.cc`; `art-android-17.0.0_r1/profman/profile_assistant.h`; `art-android-17.0.0_r1/runtime/jit/profile_saver_options.h` (`kMinMethodsToSave`, `kInlineCacheThreshold`).

### 18. Off-thread deserialize: V8's model for a cache loaded while something else runs

- **What it does, and why.** V8's code cache is designed to be deserialized on a background thread while the main thread runs other JavaScript. `CodeSerializer::StartDeserializeOffThread` walks the cached bytes on a `LocalIsolate` without the main isolate's lock; when the source becomes available, `NotifySourceTextAvailable` runs the source-hash sanity check (`code-serializer.h:211-220`); `FinishOffThreadDeserialize` merges the deserialized script into the main heap on the main thread. A `BackgroundMergeTask` handles the merge, and the cache is dropped if any of the sanity checks fail (entry 11, seven checks in `SanityCheckWithoutSource` plus the source-hash check in `SanityCheckJustSource`). The off-thread path is what keeps a page's first paint close to the time the code cache took to load, instead of adding deserialization time to it.

- **For cig: adapt.** cig has no main-thread-and-background-thread model today (values are `Rc`, non-atomic, as `cig-script/src/value.rs` says). cig-cherry does add a background compile thread (cig-script#256 §2); the same thread deserializes a cache entry, and the main thread keeps running the VM. The sanity checks (entry 11) run on the background thread; a failed check does not fail the run, it just tells the VM there is no entry to switch to. This is the right shape for rule 5 (identical behavior on every engine): the main thread is the VM, and the background thread's success or failure never changes an observable thing a program can see.

- **What would decide it.** Not measured yet: whether cherry's background thread finishes compiling a hook function before the hook call ends (42% to 94% of the call is the script's top level, so there is time).

- **Feeds.** cig-script#256 §2 (the loader and the cache); decision (f).

- **Read.** `v8-15.8.62.31/src/snapshot/code-serializer.h` (`StartDeserializeOffThread`, `NotifySourceTextAvailable`, `FinishOffThreadDeserialize`, `BackgroundMergeTask`, `OffThreadDeserializeData`); `v8-15.8.62.31/src/snapshot/code-serializer.cc`.

## For the issues

- **cig-script#256 (cig-cherry).** The study maps cherry's tiers to ART's baseline-and-optimizing split (entry 1), its background compile to .NET's tiered compilation (entry 2), its cache to Leyden's AOT cache (entry 7) and ART's oat file (entry 10), its dual-mapping discipline to ART's memfd pair (entry 13), its fall-back to ART's guarded inlines and .NET's OSR (entry 14), and its profile to ART's `ProfilingInfo` and `ProfileCompilationInfo` (entries 1 and 5). Phase 2 of #256's plan (a leaf function with no guards) matches entry 1's shape with the per-site observation set to 1. The proposal for the cache entry header is entry 16.
- **cig-script#254 (the module cache).** The entries extend the modcache's existing key and MAC (`cig-script/src/burn/modcache.rs`) with target ISA, ISA features and a compiler-filter byte (entry 10, entry 16). cig's modcache already carries most of what an oat header or a V8 code cache carries; what it does not carry is the flag hash and the CPU features, both of which V8 (entry 11) and Julia (entry 15) show are the ones that go wrong on a cross-machine or cross-flag load.
- **cig-compile#33 (the optimizer and PGO).** `--profile-gen` writes a profile in the same format as cherry's (entry 5, entry 16); `--profile-use` reads it. Counter placement for `--profile-gen` follows .NET's spanning tree (entry 3). The `.mibc` merge command maps to `cig profile merge` (entry 6). The passes themselves are in the sibling studies.
- **cig-compile#36 (start-up and size).** Leyden's 19% HelloStreamWarmup improvement (entry 7) and the 250 KB cache overhead (2.5%) are the shape of the cost: cig's cache under the tin trades disk for faster warm runs on cigo's seven cases, where 94.3% of `cigo --version` is the top level (`data/census/README.md`).

## Measurements owed

- The per-call size of a cigo-shaped profile with the entry-5 format, against the bytecode module's size, on the seven cigo cases (entry 5).
- The fraction of cigo call sites at which a second observation would be needed to cover the site: extend the census to keep 2 per site and read the distribution (entry 1).
- The cost in the VM of an extra `u16` increment (one load, one add, one store) at every `STEPS` for the hotness counter, measured against today's bench on the 21 compute programs where `steps` is 9.1% to 50.0% of executed instructions (entry 4).
- Cherry's per-function compile cost against the function's own run time for the ten busiest sites a cigo call enters (entry 2).
- A cold cigo run against a warm one (cherry entry exists), counted in instructions and in milliseconds, on the seven cigo cases (entry 7).
- Compile size per guarded inline against time saved on a hook call's 1,282 `.push()` method calls (entry 8).
- The rate of guard failures on cigo's `list` and `audit`, where functions see two or three rows of argument types (entry 14).
- Phase 2 of cig-script#256: fib and loop at least 3x faster warm, with a cold start no more than 10% slower than the VM's (the issue's own target).

## Not settled

- The exact per-site layout on disk of ART's profile inline cache (varint encoding of dex-method-indices under `kIsMegamorphicEncoding`): I kept this at the level of the header comment and did not copy the varint code.
- Whether cig-compile's `--profile-gen` build uses the same bytes the VM writes at run, or a build-time instrumentation: the census shows the VM's `Proto::hints` already hold one `u32` a pc, so the two shapes are compatible but the question of which program writes the profile stays open.
- Whether a cig-cherry tier is worth running for functions that never leave cigo's top level (entry 7 says cigo's calls do not reach the hot regime in one run): the answer depends on the warm case, which I did not measure.
- Whether the hardening-level byte in the entry 16 header is the right place for the `fapolicyd`/IMA policy flag, or whether that belongs outside the file (a separate "allow native" signature file). I did not find a tree that keeps this outside the code file.
