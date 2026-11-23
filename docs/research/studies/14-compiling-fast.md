# Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache

- **Serves.** cig-compile#37 (build speed), #32 (stage C); cig-script#256 (cig-cherry's tier 2), cig-script#259 (a first run on the VM pays for the lowering); decisions (e), (f).
- **Pins.** QBE `v1.3` at `c0818978` (MIT): `main.c`, `doc/il.txt`, `doc/llvm.txt`, every pass file, the per-target `isel`, `abi`, `emit`.
  TinyCC `release_0_9_27` at `d348a9a5` (LGPL-2.1): `tcc-doc.texi` (sections "Parser", "Code generation", "The value stack"); file names only for `tccgen.c` and `x86_64-gen.c`.
  CPython `v3.15.2` at `5e58eaad` (PSF-2.0): `Tools/jit/` (`template.c`, `_stencils.py`, `_targets.py`, `_writer.py`, `README.md`, `jit_infra.md`), `Python/jit.c`, `Python/jit_publish.c`, `Python/jit_unwind.c`, `Python/optimizer.c`, `Python/optimizer_analysis.c`, `Python/optimizer_symbols.c`.
  Zig `0.17.1` at `be248cc1` (MIT): `src/codegen.zig`, `src/codegen/x86_64/` (`CodeGen.zig`, `Mir.zig`, `Lower.zig`, `Emit.zig`, `abi.zig`), `src/register_manager.zig`, `src/InternPool.zig`, `src/Zcu.zig`, `src/Zcu/PerThread.zig`, `src/Compilation.zig`, `src/link.zig`, `src/link/Elf2.zig`, `src/link/MappedFile.zig`, `src/IncrementalDebugServer.zig`, `src/dev.zig`.
  rustc `1.100.0` at `0ef1bc38` (MIT OR Apache-2.0): `src/doc/rustc-dev-guide/src/queries/` (`incremental-compilation.md`, `incremental-compilation-in-detail.md`, `query-evaluation-model-in-detail.md`, `salsa.md`), `compiler/rustc_middle/src/dep_graph/` (`graph.rs`, `serialized.rs`), `compiler/rustc_incremental/src/persist/` (`save.rs`, `load.rs`, `fs.rs`, `clean.rs`, `file_format.rs`), `compiler/rustc_monomorphize/src/partitioning.rs`.
  Wasmtime `v50.0.1` at `c307ec81` (Apache-2.0 WITH LLVM-exception): `cranelift/codegen/src/incremental_cache.rs`.
- **License rule.** QBE, CPython, Zig, rustc and Cranelift: ideas freely; names, shapes, numbers. TinyCC (LGPL-2.1): ideas only; file names for pointing, no code structure to follow.
- **Read with.** `cig-script/docs/ENGINES-1.25.0.md` (the VM at 0.57 of the tree-walker's time on 21 compute programs) and `cig-script/docs/ENGINES-FIRST-RUNS-1.25.0.md` (a first run on the VM: `hello` 1.05 times the tree-walker, a 500-line script 3.14 times, cigo's script 8.45 times). `cig-compile/docs/decisions/0001-backend.md` (stage A's full build of `orchestrate.cig`, 12,540 lines, in 0.06 to 0.16 s, 46 MB; no-op rebuild 1.0 ms). The census beside this study: `data/census/README.md` and `data/census/report.md` (cigo's 1,038 functions, 115,268 instructions; a call enters 13 to 80 functions, runs 30% to 33% of the module's instructions, and spends 42% to 94% on the script's top level building tables).

## What cig should carry away

- **The unit of caching is the function after passes, keyed on everything that affects its lowered bytes.** Cranelift's `compute_cache_key` hashes the `FunctionStencil` with the target, the ISA flags and a `VersionMarker` (entry 16); rustc's dep nodes hash their inputs with the compiler's version (entries 13 and 14). For cig, that set is this cig's build and the lowering's `VERSION` (defined in `cig-script/src/lower/mod.rs` and used in the modcache's key per `cig-script/src/burn/modcache.rs`), the optimization level (today one level, `cig-script/src/lower/mod.rs: DEFAULT_LEVEL`), the function's CIR after passes, and the signatures of every function it calls, so a change to a callee's ABI invalidates the caller. A profile entry is part of the key only when the function was lowered under a profile. Entries 13, 14, 16.
- **A function's first run on the VM has to be cheaper than it is today.** cigo's module has 115,268 instructions across 1,038 functions; a call runs 30.3% to 32.6% of them and spends 42% to 94% on the script's top level (function 0, 34,841 to 34,911 instructions, about 30% of the module), building 2,323 lists, 804 maps and 662 closures before the first line of the command runs (`data/census/report.md`, "Where the work is"; `data/census/README.md` finding 6). `cig-script/docs/ENGINES-FIRST-RUNS-1.25.0.md` shows the first run of cigo on the VM at 305.9 ms (8.45 times the tree-walker's 36.2 ms, cig-script#259). Lowering a function at its first call would trade some lowering time for later invalidation work and would need rule 7 (verified bytecode, never rewritten) satisfied by verifying each function the first time it is lowered rather than the whole module up front. The top level alone is about 30% of the module and runs first on every call, so what a first run pays for is bounded below by it, whatever is lazy. The template approach (entries 6 to 8) gives a cheaper per-instruction cost. Entries 4, 6, 7, 11.
- **A single pass is the baseline tier, templates are the next step.** TinyCC's one-pass generator keeps only the value stack (entries 4, 5), and the cost is one AST walk; it is what cig-cherry's tier 2 (cig-script#256) can model if the back end is kept small. CPython's copy-and-patch (entries 6, 7) pays its compilation cost at CPython's own build time, so the run-time price is one memcpy and a handful of relocations per instruction; the file-backed, map-read-and-execute path described by `Python/jit.c: mark_executable` (entry 8) does not fit cig's rule 8, but the signed-file route cig-script#256 already names does. Entries 4, 5, 6, 7, 8.
- **Incremental means fingerprints, not file dates.** rustc hashes every query's inputs and outputs, and a query is skipped when all its inputs hash the same as last time (entries 13, 14; `compiler/rustc_middle/src/dep_graph/graph.rs: DepNodeColor` has `Green(DepNodeIndex)`, `Red`, `Unknown`). cig-compile's build index today keys on `(len, mtime_ns, ino)` (`cig-compile/src/cache.rs: FileId`); this is right for the whole-program cache at stage A, but at stage B and later the unit is the function and the key has to be its CIR (not its source bytes), so a cosmetic edit that leaves CIR the same is a hit. Entries 13, 14, 15, 16.
- **Patch the output in place.** Zig's new linker (`src/link/Elf2.zig`, built on `src/link/MappedFile.zig`) keeps the ELF mapped and swaps a changed function's bytes without rewriting the file (entry 12). For cig, stage C's archive of pinned runtimes (`cig-compile/docs/decisions/0001-backend.md`'s plan for stage C) and the per-function cache can be read through a similar node table, so that the final link is a relocation pass and nothing else. Entries 12, 16.
- **A type lattice earns its keep where types are not seen.** The census found 226 of 227 arithmetic and comparison sites in the 21 compute programs saw one pair of operand types, and functions in cigo see 2 or 3 argument-type rows (`data/census/README.md` finding 2). CPython's trace optimizer tracks a value at a point as one of `UNKNOWN`, `NULL`, `NON_NULL`, `TYPE_VERSION`, `KNOWN_CLASS`, `INT`, `TRUTHINESS`, `KNOWN_VALUE` and `BOTTOM` (`Python/optimizer_symbols.c`, entry 9), which is the shape cig needs for decision (i): a monotone lattice over the kinds cig holds today (`Int`, `Float`, `Str`, `List`, `Map`, `Func` in `cig-script/src/lower/cir/ir.rs: Ty`), with a known-value state for constants and a bottom state where a guard proved a type impossible. Entry 9.

## Design points

### 1. QBE's IL: a small SSA that leaves machine details out

- **What it does, and why.** QBE's intermediate language has four base types (`w` word, `l` long, `s` single, `d` double) and two sub-word extensions (`b` byte, `h` half-word) that appear only in data definitions (`doc/il.txt` Section 2 "Types", `BASETY := 'w' | 'l' | 's' | 'd'`, `EXTTY := BASETY | 'b' | 'h'`).
  The language has no pointer type: a pointer is `l` on 64-bit targets.
  A function is a list of blocks; a block has a label, a list of phi functions, a list of plain instructions and one terminator (jump, branch, ret, hlt).
  Temporaries are untyped names starting with `%`; globals start with `$`, user types with `:`, and block labels with `@`.
  The IL has no stack slots at the SSA level: a write to a local is a plain SSA `%v =w add %v, 1`, not three LLVM instructions (load, add, store).
  That is what `doc/llvm.txt` points to when it says "LLVM IL is more cluttered with memory operations".
- **For cig: adapt.** cig's IR is CIR (`cig-script/src/lower/`), which is structured SSA rather than a control-flow graph (`cig-script/src/lower/cir/ir.rs` has `Module`, `Func`, `Block`, `Instr`; no `Phi` kind).
  cig's rule 1 (burns never move) and rule 7 (verified bytecode) ask that passes keep a stable notion of a program, which CIR's `Pure`, `MayRaise`, `Effect`, `Step` classes already enforce.
  The lesson cig takes is the small type set: a dynamic language has one `Value` (`cig-script/src/value.rs: Value`), so the IR does not need QBE's four.
  Adapt means: at the stage where typed arithmetic enters CIR (planned in cig-compile#34), fast paths can be marked on a per-instruction kind without adding a new type dimension to CIR.
- **What would decide it.** Decided by rule 1.
- **Feeds.** cig-compile#33, cig-compile#34; decisions (e), (i).
- **Read.** `qbe-v1.3/doc/il.txt` (`BASETY`, `EXTTY`); `qbe-v1.3/doc/llvm.txt` (`cluttered with memory operations`); `qbe-v1.3/all.h` (`Blk`, `Phi`, `Ins`, `Fn`, `Tmp`, `Ref`); `cig-script/src/lower/cir/ir.rs` (`Module`, `Func`, `Block`, `Instr`); `cig-script/src/value.rs` (`Value`).

### 2. QBE's pass list: nine thousand lines of C across fourteen passes

- **What it does, and why.** `qbe-v1.3/main.c: func` runs each function through a fixed pipeline, with each pass in its own `.c` file.
  The pass list, with each file's line count (from `wc -l`) and the file each named function sits in, is: `parse.c` 1,449; the per-target ABI step `T.abi0` plus `amd64/sysv.c` 721, `arm64/abi.c` 852, `rv64/abi.c` 653; `mem.c` 488 (`promote` at line 9, `coalesce` at line 206); `ssa.c` 433 (`ssa` at line 316); `alias.c` 222 (`fillalias` at line 133); `load.c` 511 (`loadopt` at line 430); `copy.c` 436; `fold.c` 246 plus `gvn.c` 508 and `gcm.c` 460 (constant folding, global value numbering, global code motion); `simpl.c` 124; `ifopt.c` 121 (`ifconvert`); `cfg.c` 567 (`fillcfg`, `filldom`, `simplcfg`, `simpljmp`); `live.c` 144 (`filllive`); `spill.c` 531; `rega.c` 700; and finally per-target `isel.c` (amd64 944, arm64 316, rv64 255) and `emit.c` (amd64 881, arm64 692, rv64 581), with `util.c` 774 and `all.h` 631.
  The core (every `.c` and `.h` outside a per-target folder) is 9,081 lines; each per-target `isel` plus `emit` is 1,000 to 2,000 lines on top.
  `doc/llvm.txt` records the target of "less than 8 kloc"; the pin is a touch over that at 9 kLoC in the core.
- **For cig: adapt.** Decided by rules 1 and 2: cig's passes stay bounded by the verifier (`cig-script/src/lower/cir/verify.rs`, `T1` to `T4`), and QBE's "one small `.c` per pass" shape matches cig's layout (`cig-script/src/lower/cir/pass/` has `copy_prop`, `dead_pure`, `fold_consts`, `sccp`, each in its own file, named in `cig-script/src/lower/cir/pass/mod.rs: NAMES`).
  cig has no code generator today (stage C is planned in cig-compile#32; no Cranelift dependency exists anywhere in the clones), and QBE's pass sizes set a floor on what a native tier that leaves no passes out would cost cig at the lowered-function level: a full isel is 250 lines (rv64) to 950 (amd64) of rules per target.
- **What would decide it.** Not measured yet: for each cig CIR pass, the lines of Rust and the mean per-function walltime over the 21 compute programs; compared against QBE's per-pass file sizes.
- **Feeds.** cig-compile#33, cig-compile#37; decisions (e), (f).
- **Read.** `qbe-v1.3/main.c` (`func`, `T.abi0`, `fillcfg`, `filluse`, `filldom`, `gvn`, `simplcfg`, `gcm`, `ifconvert`, `T.abi1`, `simpl`, `T.isel`, `filllive`, `fillloop`, `fillcost`, `spill`, `rega`, `simpljmp`, `T.emitfn`, `T.emitfin`); `qbe-v1.3/parse.c`; `qbe-v1.3/mem.c` (`promote`, `coalesce`); `qbe-v1.3/ssa.c` (`ssa`); `qbe-v1.3/alias.c` (`fillalias`); `qbe-v1.3/load.c` (`loadopt`); `qbe-v1.3/copy.c`; `qbe-v1.3/gvn.c`; `qbe-v1.3/gcm.c`; `qbe-v1.3/spill.c`; `qbe-v1.3/rega.c`; `qbe-v1.3/amd64/isel.c`; `qbe-v1.3/amd64/sysv.c`; `qbe-v1.3/amd64/emit.c`; `cig-script/src/lower/cir/pass/mod.rs` (`NAMES`, `level`, `run_passes`).

### 3. "70% of the performance in 10% of the code", in QBE's own words

- **What it does, and why.** `qbe-v1.3/doc/llvm.txt` says, in its Scope section: "QBE is about the first 70%, not the last 30%. It attempts to pinpoint, in the extremely vast compilation literature, the optimizations that get you 70% of the performance in 10% of the code of full blown compilers. For example, copy propagation on SSA form is implemented in 160 lines of code in QBE!"
  The small core rests on one IL, one SSA builder, light types (four base types, no vectors, no aggregates in temporaries), no inliner, no aggressive loop pass and no vectorizer.
  `qbe-v1.3/copy.c` is 436 lines in all; the specific copy-propagation pass is the 160-line measure QBE quotes.
  The claim is comparative, not a figure from a specific benchmark: `doc/llvm.txt` is a position paper, and QBE's own tests (`qbe-v1.3/test/`) are its acceptance.
- **For cig: adapt.** Decided by rules 1 and 5 (identical behavior on every engine; passes keep classes).
  cig already picks four passes at `-O1` and `-O2` (`cig-script/src/lower/cir/pass/mod.rs: NAMES = ["fold-consts", "copy-prop", "sccp", "dead-pure"]`), and the QBE lesson is that this shape is enough to stay useful at stage C if each pass stays bounded to hundreds of lines.
  The compiler is one person's project, not one many years old; cig's four passes plus a lowering, a verifier and a bytecode encoder are already a few thousand lines of Rust across `cig-script/src/lower/`.
- **What would decide it.** Not measured yet: the geometric mean of the 21 compute programs at stage C under cig's four passes against the same programs at stage A; the threshold is `cig-script/docs/ENGINES-1.25.0.md`'s 0.57 figure, which the VM already reaches.
- **Feeds.** cig-compile#33, cig-script#256; decision (e).
- **Read.** `qbe-v1.3/doc/llvm.txt` (`70% of the performance`, `8 kloc`, `160 lines`); `qbe-v1.3/copy.c`; `qbe-v1.3/mem.c` (`coalesce`); `qbe-v1.3/README`; `cig-script/src/lower/cir/pass/mod.rs` (`NAMES`, `level`).

### 4. TinyCC: code generated while parsing

- **What it does, and why.** TinyCC's own documentation says (`tcc-doc.texi`, Parser section): "The parser is hardcoded (yacc is not necessary). It does only one pass, except" for two local look-arounds (an initialized array of unknown size, and reversed argument evaluation on some targets).
  The Code generation section says: "The TCC code generator directly generates linked binary code in one pass. It is rather unusual these days (see gcc for example which generates text assembly), but it can be very fast and surprisingly little complicated. The TCC code generator is register based. Optimization is only done at the expression level. No intermediate representation of expression is kept except the current values stored in the value stack."
  The gain is that there is no IR to lower, no IR verifier to run and no file to write between parse and code emission.
  The cost is that nothing beyond one expression can be optimized: no inlining, no global analysis, no post-hoc loop work; local look-arounds only.
- **For cig: adapt.** Decided by rules 1, 2 and 7 (burns never move; errors at the same place; verified bytecode never rewritten).
  A pure one-pass generator from source cannot satisfy rule 7 as written, because there is no verifiable bytecode in flight; the idea cig takes is narrower.
  cig's form: at cig-cherry's tier 2 (planned in cig-script#256), a function's register bytecode can be translated to native code in a single walk of its instructions, with the VM's `Proto::hints` (`cig-script/src/vm/mod.rs: Proto`) as the per-pc feedback that chooses a fast path.
  That is one-pass from bytecode to code, not from source to code; the module has been verified by `cig-script/src/lower/cir/verify.rs` and `cig-script/crates/cig-bytecode/src/verify.rs` before any native code is cut.
  Tier 3 then runs CIR passes on hot functions.
- **What would decide it.** Not measured yet: the time to compile `fib` to native on tier 2 against the warm VM time of `fib` on cig 1.25.0 as released (`cig-script/docs/ENGINES-1.25.0.md` reports 16.6 ms, which is the figure to beat); the budget in cig-script#256 is a cold start no more than 10% slower than the VM's.
- **Feeds.** cig-script#256, cig-compile#32; decisions (b), (e).
- **Read.** `tinycc-release_0_9_27/tcc-doc.texi` (`SValue`, `VT_CMP`, `VT_JMP`, `VT_LVAL`, `VT_MUSTCAST`); file names `tinycc-release_0_9_27/tccgen.c`, `tinycc-release_0_9_27/x86_64-gen.c`; `cig-script/src/vm/mod.rs` (`Proto`, `hints`); `cig-script/src/lower/cir/verify.rs`.

### 5. TinyCC's value stack: a stand-in for an IR, and what one pass cannot do

- **What it does, and why.** TinyCC's documentation (`tcc-doc.texi`, "The value stack" subsection) describes the shape: each expression's value is pushed on `vstack`, whose top is `vtop`; each entry is an `SValue` with a type field (`SValue.t`) and a location field (`SValue.r`) that is a register index or a flag.
  The flags are `VT_CONST`, `VT_LOCAL`, `VT_CMP` (the value is a comparison in CPU flags, 0 or 1), `VT_JMP`, `VT_LVAL`, `VT_SYM`, `VT_MUSTCAST` (lazy casting: a cast is emitted only when the value is used in a form that requires it) and others.
  A compare or a conditional jump is a flag on the top entry, not an instruction: the generator keeps the fact that the top value is a comparison, and the conditional jump is emitted only when the code that needs its value runs.
  What one pass cannot do, as the documentation says: nothing is kept after an expression's end except the register map, so no global analysis is possible; three temporary registers are used on x86, and when more are needed one is spilled.
- **For cig: adapt.** Decided by rule 1 (burns never move) and rule 2 (errors at the same place).
  The value stack is register allocation and expression-local constant folding in one data structure, and that is exactly what cig's register bytecode's temporary slots already serve (`cig-script/src/lower/bc/mod.rs`).
  The one lesson for cig is `VT_CMP` and `VT_JMP`: a compare-and-jump that cig's bytecode already has as one opcode (`EQJ`, `LTJ`, `LEJ`, `NEJ` named in `cig-script/docs/BYTECODE.md`) is TinyCC's shape, applied at the bytecode level.
  A native tier can carry the same idea: a compare produces a flag, the branch reads the flag, no materialized boolean is written to a register between the two.
- **What would decide it.** Decided by rule 1.
- **Feeds.** cig-compile#32, cig-script#256; decision (b).
- **Read.** `tinycc-release_0_9_27/tcc-doc.texi` (`SValue`, `VT_CMP`, `VT_JMP`, `VT_LVAL`, `VT_MUSTCAST`); `cig-script/src/lower/bc/mod.rs`; `cig-script/docs/BYTECODE.md`.

### 6. CPython's stencils: one template per bytecode instruction, cut at build time

- **What it does, and why.** CPython's JIT builds a template for each "uop" at CPython's own build time: `Tools/jit/template.c` (165 lines) holds the C source every uop body is compiled from.
  `Tools/jit/_targets.py: _Target` drives the build, calling clang (`Tools/jit/README.md` says "LLVM version 21 is the officially supported version") with the right target triple, parsing the emitted object files and extracting a `Stencil` from each one (`Tools/jit/_stencils.py: Stencil`): a `body` of machine code bytes plus a list of `Hole` records, one per relocation the object file says has to be patched.
  A `Hole` has an offset, a kind (the relocation type: `R_X86_64_64`, `R_AARCH64_CALL26` and so on, listed in `_PATCH_FUNCS`), a base value (`HoleValue.CODE`, `HoleValue.DATA`, `HoleValue.OPARG`, `HoleValue.OPERAND0`, `HoleValue.OPERAND1`, `HoleValue.JUMP_TARGET`), a symbol and an addend.
  The stencil is a per-uop block of code with its fix-up list; at run time, copying it in and running the patches gives a working piece of native code.
- **For cig: adapt.** Decided by rule 8 (no memory both writable and executable) and rule 9 (`forbid(unsafe_code)` everywhere but one small crate).
  cig could emit a Rust template for each of cig's bytecode opcodes (`cig-script/crates/cig-bytecode/src/opcode.rs: Op`), compile them with the Rust compiler at cig's own build time, and record the holes a trace stamp has to fill (the operand bytes, the branch targets, the field cache slot).
  Because the Rust compiler would be the one cutting the code, no run-time code generator lives in cig itself, and the one `unsafe` crate (rule 9) holds only the loader.
  The next-tier compiler (planned in cig-script#256) is a stamp and a patch: cheap enough that lowering lazily (a function at its first call) is credible, which pays the first-run cost `cig-script/docs/ENGINES-FIRST-RUNS-1.25.0.md` reports.
- **What would decide it.** Not measured yet: the native bytes per cig bytecode opcode (from a Rust compile of a template of each one, size recorded), and the stamp-and-patch time per opcode compared to the VM's time per opcode; the figure `cig-script/docs/research/luajit.md` names (900 machine instructions per operation in the VM against 162 in Lua 5.4) is the lower bound.
- **Feeds.** cig-script#256, cig-compile#32; decisions (b), (e).
- **Read.** `cpython-v3.15.2/Tools/jit/README.md`; `cpython-v3.15.2/Tools/jit/jit_infra.md`; `cpython-v3.15.2/Tools/jit/template.c`; `cpython-v3.15.2/Tools/jit/_stencils.py` (`Hole`, `HoleValue`, `Stencil`, `_PATCH_FUNCS`, `_HOLE_EXPRS`); `cpython-v3.15.2/Tools/jit/_targets.py` (`_Target`, `_compute_digest`); `cpython-v3.15.2/Tools/jit/_writer.py`; `papers/pep-0744.rst`; `papers/xu-2021-copy-and-patch-compilation.pdf`; `cig-script/crates/cig-bytecode/src/opcode.rs` (`Op`).

### 7. Stamping and patching holes at run time

- **What it does, and why.** `Python/jit.c` is the run-time half.
  For each uop in a trace the executor copies the stencil's body into a per-trace buffer (`jit_alloc` returns a page-aligned mapping) and then runs the per-hole patches: `patch_64` writes an absolute 64-bit value at the offset; `patch_x86_64_32rx` writes a RIP-relative 32-bit displacement that the relocation says is relaxing (a "load from GOT" that can be turned into a direct `mov` when the symbol is in range); `patch_aarch64_26r`, `patch_aarch64_21r`, `patch_aarch64_12` and friends write the various fixed-width fields of an AArch64 instruction, with `get_bits` and `set_bits` doing the bit splicing.
  The value a hole gets is computed from the trace's current operand: `_HOLE_EXPRS` in `_stencils.py` names them as `instruction->oparg`, `instruction->operand0`, `instruction->operand1`, `state->instruction_starts[instruction->jump_target]`.
  The whole trace fits in one allocation; the per-trace copy is contiguous.
- **For cig: adapt.** Decided by rule 8 and rule 9.
  cig's form would be a per-trace buffer that is a file in the tin (a signed, mapped artifact, not an anonymous page), with the template bytes memcpy'd in and the holes patched before the file is mapped read-and-execute.
  The hole kinds cig needs are narrower than CPython's: an operand byte in the bytecode, a branch target to another stencil, and a pointer into the runtime library (the stable C ABI planned in cig-compile#32).
  The three AArch64 patch functions (`patch_aarch64_26r` for `BL`/`B`, `patch_aarch64_21r` plus `patch_aarch64_12` for `ADRP`+`ADD`, `patch_aarch64_16a` through `patch_aarch64_16d` for `MOV` four-piece immediates) are the baseline list any AArch64 stamp has to cover.
- **What would decide it.** Not measured yet: the cost of one copy-and-patch per CIR instruction (median body size times page alignment) against the VM's mean dispatch time on the 21 compute programs (`cig-script/docs/ENGINES-1.25.0.md`'s 0.57 of the tree-walker).
- **Feeds.** cig-script#256, cig-compile#32; decisions (b), (f).
- **Read.** `cpython-v3.15.2/Python/jit.c` (`jit_alloc`, `jit_free`, `mark_executable`, `patch_64`, `patch_x86_64_32rx`, `patch_32r`, `patch_aarch64_26r`, `patch_aarch64_21r`, `patch_aarch64_12`, `patch_aarch64_16a`, `patch_aarch64_16b`, `patch_aarch64_16c`, `patch_aarch64_16d`, `get_bits`, `set_bits`); `cpython-v3.15.2/Tools/jit/_stencils.py` (`_HOLE_EXPRS`, `_PATCH_FUNCS`).

### 8. Page protections: write then execute, never both, and cig's rule 8

- **What it does, and why.** `Python/jit.c: mark_executable` does the transition: on POSIX it calls `__builtin___clear_cache` for the instruction-cache invalidate, then `mprotect(memory, size, PROT_EXEC | PROT_READ)`; on Windows it calls `FlushInstructionCache` and then `VirtualProtect(memory, size, PAGE_EXECUTE_READ, &old)`.
  The allocator (`jit_alloc`) returned the page as `PROT_READ | PROT_WRITE` (or Windows' `PAGE_READWRITE`) so the patches could be written; the transition flips to `PROT_EXEC | PROT_READ`.
  The comment in `mark_executable` is explicit: "Do NOT ever leave the memory writable! Also, don't forget to flush the i-cache".
  At no instant are both bits set: this is write-xor-execute at the page level, the common hardening rule.
- **For cig: reject.** Decided by rule 8 (no memory both writable and executable) and rule 10 (ahead of time by default, signed files mapped read-and-execute).
  CPython's flow (`PROT_READ|PROT_WRITE` then `PROT_EXEC|PROT_READ`) satisfies write-xor-execute at any instant, but hardened kernels, SELinux `execmem`, iOS and `fapolicyd` policies disallow even the briefest `PROT_WRITE` mapping that becomes `PROT_EXEC`.
  cig's rule 8 is stricter: it bans an anonymous mapping that was ever writable from turning executable.
  The route cig takes is strictly file-backed: write the code to a file, close it, then map it `PROT_READ|PROT_EXEC` only, with no `mprotect` transition.
  That is also what cig-script#256 names ("Native code is written to a file, then mapped read-and-execute").
  What cig takes from CPython is the hole-and-patch machinery; what it rejects is the anonymous-write-then-reprotect flow.
  cig's form: no mapping exists today (stage C is planned in cig-compile#32); the same file-backed signing scheme that `cig-script/src/burn/modcache.rs` already runs for the lowered module cache carries to compiled code.
- **What would decide it.** Decided by rule 8.
- **Feeds.** cig-script#256, cig-compile#32, cig-compile#40; decisions (b), (f).
- **Read.** `cpython-v3.15.2/Python/jit.c` (`mark_executable`, `jit_alloc`, `jit_free`); `cpython-v3.15.2/Python/jit_publish.c`; `cpython-v3.15.2/Python/jit_unwind.c`; `cig-script/src/burn/modcache.rs` (`DIR`, `KEY_FILE`, `MAGIC`, `FORMAT`, `HEADER_LEN`, `LEVEL`, `root`).

### 9. The abstract interpreter over traces: a lattice for a dynamic language

- **What it does, and why.** `Python/optimizer_symbols.c` describes a lattice over values on a trace.
  A symbol starts at `JIT_SYM_UNKNOWN_TAG` and can only move downward, never back up: `UNKNOWN` splits into `NULL`, `NON_NULL`, `RECORDED_VALUE`; `NON_NULL` narrows to `TYPE_VERSION`, then to `KNOWN_CLASS` (one of which is `INT`, a limited-range integer), to `KNOWN_VALUE`, and finally to `JIT_SYM_BOTTOM_TAG` when a contradiction is found.
  The comment reads: "If at any point we encounter a float guard on the same symbol, that would be a contradiction, and the symbol would be set to BOTTOM (indicating that the code is unreachable)."
  `Python/optimizer_analysis.c` runs a data-flow analysis over the trace, tracking the symbol at each point; the file cites Clang's data-flow analysis introduction.
  `Python/optimizer.c` drives the whole: it decides which uops to emit and which to drop given what the symbols say (`_Py_uop_analyze_and_optimize`, `MAX_EXECUTORS_SIZE`, `has_space_for_executor`).
- **For cig: adapt.** Decided by rule 5 (identical behavior on every engine): any inferred type has to be checked by a guard that the engine falls back on, which fits cig-cherry's shape (cig-script#256).
  cig's form for decision (i): a lattice over CIR values (today `Dynamic`, `Null`, `Bool`, `Int`, `Float`, `Str`, `List`, `Map`, `Func` in `cig-script/src/lower/cir/ir.rs: Ty`), extended with a known-class state and a known-value state for constants, and a bottom state for unreachable.
  The census' finding that 97.7% of arithmetic executions in the compute programs were at sites where types do not move (`data/census/report.md`) says a monotone lattice is well matched to what cig sees.
  CPython's `KNOWN_VALUE` is cig's `fold-consts` pass folded into the type lattice.
- **What would decide it.** Not measured yet: how many guards a stage C build of the 21 compute programs plants under a lattice with and without `KNOWN_CLASS`, and the compile time of each.
- **Feeds.** cig-compile#34, cig-script#256; decisions (b), (i).
- **Read.** `cpython-v3.15.2/Python/optimizer.c` (`_Py_uop_analyze_and_optimize`, `MAX_EXECUTORS_SIZE`, `has_space_for_executor`); `cpython-v3.15.2/Python/optimizer_analysis.c`; `cpython-v3.15.2/Python/optimizer_symbols.c` (`_PyUOpSymPrint`, `JIT_SYM_UNKNOWN_TAG`, `JIT_SYM_NULL_TAG`, `JIT_SYM_NON_NULL_TAG`, `JIT_SYM_BOTTOM_TAG`); `cig-script/src/lower/cir/ir.rs` (`Ty`).

### 10. Zig's debug back ends: typed IR straight to machine code, no optimizer between

- **What it does, and why.** `zig-0.17.1/src/codegen.zig` dispatches to per-target back ends (it publishes `aarch64` and `loongarch` as submodules, and the x86_64 back end lives in `zig-0.17.1/src/codegen/x86_64/CodeGen.zig`, 192,827 lines as a single file).
  The x86_64 back end walks AIR (Zig's typed IR after Sema) instruction by instruction, keeping the per-instruction result in a tracked register or on the stack.
  `zig-0.17.1/src/register_manager.zig` is a generic `RegisterManager(Function, Register, tracked_registers)` with an `AllocationError` that is `Error || error{OutOfRegisters}`; when a new value needs a register, the manager spills the oldest tracked instruction to the stack.
  The flow is AIR to Mir (`zig-0.17.1/src/codegen/x86_64/Mir.zig`: "These instructions have a 1:1 correspondence with machine code instructions for the target"), to Lower (`zig-0.17.1/src/codegen/x86_64/Lower.zig: max_result_insts` bounds a single MIR step's expansion), to Emit (`zig-0.17.1/src/codegen/x86_64/Emit.zig: emitMir`).
  No LLVM, no optimizer between AIR and MIR.
- **For cig: adapt.** Decided by rule 1 (burns never move), rule 9 (`unsafe` only in one crate) and rule 10 (ahead of time, read-and-execute).
  cig has no code generator today (stage C is planned in cig-compile#32); the Zig lesson is the shape a safe-Rust baseline can take.
  cig's form: a register manager over CIR temporaries (the `vals` list on `cig-script/src/lower/cir/ir.rs: Func`), feeding a per-target MIR that is 1-to-1 with the machine's instructions, with a Lower and an Emit step that produce bytes.
  The baseline can run under `#![forbid(unsafe_code)]` because the one place that needs `unsafe` is the loader that maps the file, not the generator.
  This is cig-cherry's tier 2 if the baseline is the function-local generator (entry 4), but Zig's version is more complete (it does register allocation that TinyCC does not).
- **What would decide it.** Not measured yet: the lines of Rust for a CIR-to-x86_64 baseline on a small set of CIR instructions (add, mov, load, store, branch, call), and the native bytes per CIR instruction.
- **Feeds.** cig-compile#32, cig-script#256; decisions (e), (f).
- **Read.** `zig-0.17.1/src/codegen.zig` (`aarch64`, `loongarch`); `zig-0.17.1/src/codegen/x86_64/CodeGen.zig`; `zig-0.17.1/src/codegen/x86_64/Mir.zig` (`Inst`, `FrameLoc`); `zig-0.17.1/src/codegen/x86_64/Lower.zig` (`max_result_insts`); `zig-0.17.1/src/codegen/x86_64/Emit.zig` (`emitMir`); `zig-0.17.1/src/register_manager.zig` (`RegisterManager`, `AllocationError`); `zig-0.17.1/src/codegen/x86_64/abi.zig`; `cig-script/src/lower/cir/ir.rs` (`Func`).

### 11. Incremental analysis: a per-kind dependency index

- **What it does, and why.** Zig does not use a general query engine.
  Its incremental analysis is held in `zig-0.17.1/src/InternPool.zig`, which keeps one dependency map per kind of thing the analysis depends on: `src_hash_deps` (a tracked ZIR instruction, declaration or function signature), `nav_val_deps` (a named value's value), `nav_ty_deps` (a named value's type), `func_ies_deps` (a function's inferred error set), `type_layout_deps` (a struct, union or enum's layout), `struct_defaults_deps` (a struct's default field values), `source_file_deps` (a `@import`), `embed_file_deps` (`@embedFile`), `namespace_deps` and `namespace_name_deps` (the names a namespace exposes).
  `addDependency(gpa, depender, dependee)` puts an entry in `dep_entries` and chains it in both the per-dependee map and the depender's own chain (`first_dependency`).
  `dependencyIterator(dependee)` walks every depender of one dependee; `markDependeeOutdated` (in `zig-0.17.1/src/Zcu.zig`) is called when the dependee's result changed and enters every depender into `outdated` or decrements its "potentially outdated" counter (`potentially_outdated`, `outdated_ready`).
- **For cig: adapt.** Decided by rule 1 (burns never move) and rule 5 (identical behavior).
  cig's form at the function level after passes: the dependency kinds are the function's own CIR hash, the signatures of every function it calls (the equivalent of `nav_ty_deps`), the layout of every record type it reads (planned in cig-compile#35), and the ABI version of the runtime library (`cig-compile/docs/decisions/0001-backend.md` names stage C's runtime library).
  A pure change to a callee's body with the same signature does not invalidate its callers; a signature change does.
  The brief recalled `Dependency` and `invalidate` in `src/Zcu.zig`; the pin's names are `Dependee` (in `src/InternPool.zig`) and `markDependeeOutdated` (in `src/Zcu.zig`), and the per-kind index lives in `InternPool.zig`.
- **What would decide it.** Not measured yet: for cigo's `orchestrate.cig` (1,038 functions, `data/census/README.md`), the cache hit rate after editing one function, with the body-vs-signature split against a whole-function hash.
- **Feeds.** cig-compile#37, cig-compile#34; decisions (e), (f).
- **Read.** `zig-0.17.1/src/InternPool.zig` (`Dependee`, `DependencyIterator`, `addDependency`, `dependencyIterator`, `src_hash_deps`, `nav_val_deps`, `nav_ty_deps`, `source_file_deps`, `namespace_deps`, `dep_entries`, `first_dependency`); `zig-0.17.1/src/Zcu.zig` (`markDependeeOutdated`, `outdated`, `potentially_outdated`, `outdated_ready`, `IncrementalDebugState`); `zig-0.17.1/src/Zcu/PerThread.zig`.

### 12. Patching the output in place: Zig's mapped linker

- **What it does, and why.** Zig's `zig-0.17.1/src/link.zig` has `Tag.elf2` and `Tag.macho2`, dispatched to `zig-0.17.1/src/link/Elf2.zig` and (for Mach-O) `src/link/MachO2.zig`.
  `Elf2.zig` is built on `zig-0.17.1/src/link/MappedFile.zig`, which keeps the output file mapped and tracks a tree of nodes (`MappedFile.Node.Index`).
  `Elf2.zig`'s `ni` field names one node each for the ELF header, section headers, program headers and the sections `rodata`, `text`, `data`, `data_rel_ro`, with optionals for `tls` and `gnu_eh_frame`.
  A function's code lives in a node; a changed function is written over its node's bytes (or a new node is allocated and the old one freed).
  The file is the output, not an in-memory representation: a compile then link produces a byte-identical file, and an incremental build changes only the nodes that moved.
  `zig-0.17.1/src/IncrementalDebugServer.zig` exposes a small TCP REPL on `port` 7623 (`spawn`, `runServer`) for inspecting the mapping, behind the `--debug-incremental` CLI flag.
- **For cig: adapt.** Decided by rule 8 (no memory both writable and executable) and rule 10 (ahead of time, signed files mapped read-and-execute).
  cig has no linker today (stage C is planned in cig-compile#32, which names an embedded linker written against the `object` crate; no such crate dependency exists in the clones yet).
  The Zig lesson for cig-compile#37's incremental linking is: keep the output mapped as it is written, so a per-function recompile is a per-node overwrite and nothing else moves.
  For cig, the output is the program (`cig run`'s binary carried with the payload; `cig-compile/docs/decisions/0001-backend.md`), and a per-function cache (cig-compile#37's "incremental linking") is the per-node overwrite.
- **What would decide it.** Not measured yet: the per-node overwrite time for a one-function edit to `orchestrate.cig` against a cold link of the same, on the test machine.
- **Feeds.** cig-compile#37, cig-compile#32; decisions (e), (f).
- **Read.** `zig-0.17.1/src/link.zig` (`Tag`, `elf2`, `macho2`); `zig-0.17.1/src/link/Elf2.zig` (`MappedFile`, `Node.Index`, `rodata`, `text`, `data`, `data_rel_ro`); `zig-0.17.1/src/link/MappedFile.zig` (`MappedFile`, `Alignment`, `memory_map`); `zig-0.17.1/src/IncrementalDebugServer.zig` (`port`, `spawn`, `runServer`); `zig-0.17.1/src/Compilation.zig` (`queued_jobs`, `link_queue`, `c_object_work_queue`).

### 13. rustc's queries and fingerprints

- **What it does, and why.** `rust-1.100.0/src/doc/rustc-dev-guide/src/queries/incremental-compilation.md` and its sibling "in detail" document say the model: every pure computation the compiler does is a query (`type_of`, `type_check_item`, and so on).
  Queries form a DAG: when `Q1` asks for `Q2`, an edge is recorded.
  The results of a query are fingerprinted (`Fingerprint` and `PackedFingerprint` in `rust-1.100.0/compiler/rustc_middle/src/dep_graph/graph.rs`; `rust-1.100.0/compiler/rustc_middle/src/dep_graph/serialized.rs` encodes two `Fingerprint`s per node with the serialized `DepKind`).
  Between builds the DAG is persisted (`rust-1.100.0/compiler/rustc_incremental/src/persist/save.rs: save_dep_graph`; `rust-1.100.0/compiler/rustc_incremental/src/persist/load.rs`).
  The on-disk format is in `rust-1.100.0/compiler/rustc_incremental/src/persist/file_format.rs` (`FILE_MAGIC` is `b"RSIC"`, with `HEADER_FORMAT_VERSION`), which carries the compiler's git commit hash so a different build invalidates the cache.
  A query result on disk is loaded only when its color on the current build is green.
- **For cig: adapt.** Decided by rule 1 (burns never move), rule 2 (errors at the same place) and rule 5 (identical behavior).
  A query engine is heavy for cig today, but the fingerprint idea maps directly: cig's per-function cache (planned in cig-compile#37) keys on the function's CIR after passes, hashed with the same inputs rustc hashes (the compiler's version, the target, the opt level).
  cig-compile's index today (`cig-compile/src/cache.rs: FileId`) keys on `(len, mtime_ns, ino)`, which is right at stage A because the whole program is the unit; at stage B the unit is the function and the key is the lowered CIR.
  The persistence shape (a magic header, a format version, a compiler commit hash) is what `cig-script/src/burn/modcache.rs` already uses (`MAGIC`, `FORMAT`, the build in the key), and the pattern carries to stage B's per-function cache.
- **What would decide it.** Not measured yet: the hit rate of a CIR-keyed per-function cache after editing one function of `orchestrate.cig`, against today's whole-program `FileId`-keyed build index's one miss.
- **Feeds.** cig-compile#37, cig-compile#32; decisions (e), (f).
- **Read.** `rust-1.100.0/src/doc/rustc-dev-guide/src/queries/incremental-compilation.md`; `rust-1.100.0/src/doc/rustc-dev-guide/src/queries/incremental-compilation-in-detail.md`; `rust-1.100.0/src/doc/rustc-dev-guide/src/queries/query-evaluation-model-in-detail.md`; `rust-1.100.0/src/doc/rustc-dev-guide/src/queries/salsa.md`; `rust-1.100.0/compiler/rustc_middle/src/dep_graph/graph.rs` (`Fingerprint`, `PackedFingerprint`, `DepGraphData`, `previous`); `rust-1.100.0/compiler/rustc_middle/src/dep_graph/serialized.rs` (`SerializedDepNodeIndex`, `DepKind`); `rust-1.100.0/compiler/rustc_incremental/src/persist/save.rs` (`save_dep_graph`, `par_join`); `rust-1.100.0/compiler/rustc_incremental/src/persist/load.rs`; `rust-1.100.0/compiler/rustc_incremental/src/persist/file_format.rs` (`FILE_MAGIC`, `HEADER_FORMAT_VERSION`); `cig-script/src/burn/modcache.rs` (`MAGIC`, `FORMAT`, `HEADER_LEN`, `LEVEL`).

### 14. Red and green: a result reused when its inputs did not change

- **What it does, and why.** `rust-1.100.0/compiler/rustc_middle/src/dep_graph/graph.rs: DepNodeColor` has three states: `Green(DepNodeIndex)` ("same as last session"), `Red` ("changed") and `Unknown`.
  The algorithm try-mark-green (named in the dev guide; the implementation sits in the `DepGraph` methods on `graph.rs`, which keep a `DepNodeColorMap` and set colors with `try_set_color`) walks a node's dependency edges: if every edge is green, the node is green without re-running the query; if any edge is red, the node is re-run and its new result is hashed, and a result whose hash is unchanged still marks the node green ("even if some inputs to a query changes, it may be that it still produces the same result as the previous compilation", `incremental-compilation.md`).
  A green node whose result sat on disk is loaded, not recomputed; a red node's downstream nodes are re-examined.
  Edges are walked in the order they were recorded (the dev guide's "The query DAG" section), so a run that branched on a changed input re-enters the same path.
- **For cig: adapt.** Decided by rule 5 (identical behavior on every engine) and rule 2 (errors at the same place).
  The red-green shape maps to cig's per-function cache: a function is green when the CIR it was compiled from hashes the same as last time and all its callee signatures are green.
  The "re-run and compare" step is what lets a cosmetic edit (a comment change that leaves CIR identical) stay a hit, which `cig-compile/src/cache.rs: FileId` cannot do today because its key depends on `mtime_ns`.
  For the shape of a per-function cache at stage B, the CIR hash is the fingerprint and the dependency edges are the callees and the record shapes (planned in cig-compile#35) the function reads.
- **What would decide it.** Not measured yet: for a comment-only edit of one function of `orchestrate.cig`, the cache state under (a) today's `FileId`-keyed build index and (b) a CIR-keyed per-function red-green scheme.
- **Feeds.** cig-compile#37, cig-compile#34; decisions (f), (i).
- **Read.** `rust-1.100.0/compiler/rustc_middle/src/dep_graph/graph.rs` (`DepNodeColor`, `Green`, `Red`, `Unknown`, `DepGraphData`, `DepNodeColorMap`, `try_set_color`); `rust-1.100.0/src/doc/rustc-dev-guide/src/queries/incremental-compilation.md`; `rust-1.100.0/src/doc/rustc-dev-guide/src/queries/incremental-compilation-in-detail.md`.

### 15. Parallel units of code generation

- **What it does, and why.** `rust-1.100.0/compiler/rustc_monomorphize/src/partitioning.rs` writes, in its own introductory comment: "the unit of codegen and optimization for LLVM is 'modules' or, how we call them 'codegen units'. ... There is no way for us to incrementally update an existing LLVM module and so we have to build any such module from scratch if it was affected by some change in the source code."
  The partitioner picks a codegen unit's shape: two units per source module (a stable one for non-generic code, a volatile one for monomorphized instances), balancing inlining opportunities against invalidation.
  A larger codegen unit lets LLVM inline across more functions; a smaller unit means fewer units need rebuilding after a change.
  The passes compile units in parallel (`par_join` is used in `rust-1.100.0/compiler/rustc_incremental/src/persist/save.rs`), and the dep-graph's green nodes decide which units to keep from the previous session.
- **For cig: adapt.** Decided by rule 1 (burns never move) and rule 8 (reproducibility: the same bytes from the same input on any machine).
  cig's "codegen unit" at stage C is the function, not the module: cig-compile#37 names "per-function codegen on a thread pool, deterministic order".
  That is simpler than rustc's shape because cig does not inline today (`cig-script/src/lower/cir/pass/mod.rs: NAMES` has no `inline`), so a function stands by itself.
  The lesson cig takes is the deterministic emission order, independent of thread count: rustc's parallel passes yield the same output because every unit is written in a known order, which is exactly what cig-compile#37's `thread_count_does_not_change_bytes` test names.
  At stage D, when types and inlining arrive (planned in cig-compile#34), a per-source-file unit may become the right granularity, but the function unit is the stage-B and stage-C shape.
- **What would decide it.** Not measured yet: the 1-thread and 16-thread codegen time on `orchestrate.cig` at stage C (planned, not yet built), per cig-compile#37's benchmark table.
- **Feeds.** cig-compile#37, cig-compile#32; decisions (e), (f).
- **Read.** `rust-1.100.0/compiler/rustc_monomorphize/src/partitioning.rs`; `rust-1.100.0/compiler/rustc_monomorphize/src/collector.rs`; `rust-1.100.0/compiler/rustc_incremental/src/persist/save.rs` (`par_join`, `save_dep_graph`); `cig-script/src/lower/cir/pass/mod.rs` (`NAMES`, `level`).

### 16. Cranelift's per-function cache: hash the function stencil, store the compiled bytes

- **What it does, and why.** `wasmtime-v50.0.1/cranelift/codegen/src/incremental_cache.rs` has three primitives.
  `compute_cache_key(isa, func) -> CacheKeyHash` hashes the `Function`'s `FunctionStencil` with `isa.name()`, `isa.triple()`, `isa.flags()` and `isa.isa_flags_hash_key()`; the SHA-256 of these is `CacheKeyHash([u8; 32])`.
  `serialize_compiled(stencil)` writes a compiled stencil as postcard-encoded bytes with a `VersionMarker` placed first ("the version marker must be first to ensure deserialization stops in case of a version mismatch before attempting to deserialize the actual compiled code", header comment).
  `try_finish_recompile(func, bytes) -> Result<CompiledCode, RecompileError>` deserializes a blob and, if its `VersionMarker` matches the current function's, returns a `CompiledCode` with the function's parameters applied (`result.stencil.apply_params(&func.params)`).
  `Context::compile_with_cache` is the easy path: it computes the key, asks the `CacheKvStore` for a hit, falls through to compile and store, and (under `enable_incremental_compilation_cache_checks()`) compiles anyway and asserts the compiled and cached bytes agree.
  A version mismatch is a miss, never a wrong program (`RecompileError::VersionMismatch`).
- **For cig: take.** Decided by rule 5 (identical behavior on every engine) and rule 10 (ahead of time by default).
  cig has no Cranelift dependency today (stage C is planned in cig-compile#32); the lesson is the exact shape of what cig's per-function cache would use once stage C is built.
  cig's form: `cig-script/src/burn/modcache.rs` is already a signed, MAC'd, file-backed cache, keyed on cig's build plus `LEVEL` plus the script's SHA-256 (the names `DIR`, `KEY_FILE`, `HEADER_LEN`, `root` lay out the scheme); stage C's per-function cache follows the same shape with the function's CIR after passes as the content being hashed, the target triple and isa flags folded in.
  `VersionMarker` is what `MAGIC` and `FORMAT` give cig today; `apply_params` is cig's "relocate to this function's own operand indices" step.
- **What would decide it.** Decided by rule 5 (a version mismatch is a miss, never a wrong program); the figure `cig-compile/docs/decisions/0001-backend.md` names (1.0 ms no-op rebuild) is the floor a per-function cache has to beat for a one-function edit.
- **Feeds.** cig-compile#37, cig-compile#32, cig-script#256; decisions (e), (f).
- **Read.** `wasmtime-v50.0.1/cranelift/codegen/src/incremental_cache.rs` (`compute_cache_key`, `CacheKeyHash`, `CacheKvStore`, `CachedFunc`, `VersionMarker`, `serialize_compiled`, `try_finish_recompile`, `RecompileError`, `compile_with_cache`); `cig-script/src/burn/modcache.rs` (`DIR`, `KEY_FILE`, `MAGIC`, `FORMAT`, `HEADER_LEN`, `LEVEL`, `root`); `cig-compile/src/cache.rs` (`FileId`, `Index`); `cig-compile/docs/decisions/0001-backend.md`.

## For the issues

- **cig-compile#32 (stage C).** The entries bear on how stage C chooses its back end's shape. QBE (entries 1 to 3) says a complete back end fits in 9 kLoC of C plus 1 to 2 kLoC per per-target isel and emit; Zig's own x86_64 back end (entry 10) is the Rust equivalent, built on a generic register manager. Cranelift's `incremental_cache` (entry 16) is the per-function cache stage C would use directly. The templates approach (entries 6 to 8) is the small-code-size path for cig-cherry (cig-script#256), which shares stage C's code generator per cig-script#256's "one code generator for both". CPython's page-protection flow (entry 8) does not fit cig's rule 8; the file-backed path is what stage C already plans.
- **cig-compile#37 (build speed).** The red-green model (entries 13, 14) and the per-kind dependency index (entry 11) are the frame for the per-function cache the issue names. The lesson for `cig-compile/src/cache.rs: FileId` is that at stage B the unit is the function and the key is CIR after passes, not `(len, mtime_ns, ino)` of the source. Zig's `Elf2` with `MappedFile` (entry 12) is the shape of the incremental linker the issue names ("Relinking unchanged objects is a concatenation and a relocation pass"). rustc's codegen-unit partitioner (entry 15) is why the issue's `thread_count_does_not_change_bytes` test can hold: a deterministic emission order is independent of thread count.
- **cig-script#256 (cig-cherry's tier 2).** TinyCC (entries 4, 5) and CPython's copy-and-patch (entries 6 to 8) are the two shapes tier 2 can take. The template route fits cig's rules better because the code is cut by the Rust compiler at cig's own build time, so the only run-time work is a copy and a patch, and the file-mapping path satisfies rule 8 without any `mprotect` transition. Zig's own back end (entry 10) is the full baseline alternative: more code, but fewer surprises (it does register allocation that TinyCC does not). CPython's lattice (entry 9) is the shape of the guards cherry can emit at tier 2 and tier 3.
- **cig-script#259 (a first run on the VM pays for the lowering).** The lowering happens whole: `cig-script/src/lower/mod.rs: module_bytes` lowers every function before any run, and the census (`data/census/report.md`, "Where the work is") shows cigo runs 30.3% to 32.6% of the module's 115,268 instructions on each call, the top level alone being about 30% (34,841 to 34,911 instructions). A lowering strategy that lowers lazily (a function on its first call) would skip the instructions in functions not entered: roughly two thirds of the module, not more than nine tenths. The top level must be lowered on every first run whatever is lazy, so what a first run pays for is bounded below by it. Zig's per-function analysis (entry 11) and Cranelift's per-function cache (entry 16) show the shape. Rule 7 (verified bytecode, never rewritten) is kept by verifying each function the first time it is lowered; `cig-script/src/lower/cir/verify.rs: verify` today takes the whole `Module`, so a per-function variant is work the scheme has to add.

## Measurements owed

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

## Not settled

- **Whether a file-backed template cache can be reproducible across machines.** `cpython-v3.15.2/Tools/jit/_targets.py: _compute_digest` hashes the triple and a few flags to decide a stencil's identity, but the stencil bytes are per clang build. For cig, the bytes would be per `rustc` build; `cig-compile/docs/decisions/0001-backend.md`'s reproducibility rule asks that a specific Rust compiler version is pinned, which would be part of the per-function cache key and the signed artifact. The pin confirms cig's reproducibility statement; the exact composition of the key is not fixed in the clones.
- **How wide the type lattice at entry 9 has to be for cigo's case.** The census (`data/census/README.md`) shows cigo's functions see 2 or 3 argument-type rows and 20% to 34% of field accesses see a second shape. A lattice with a known-class state plus two guarded paths would cover cigo; a narrower lattice would underfit. The exact threshold needs the stage-D build measured.
- **Whether lazy lowering fits rule 7 at reasonable cost.** The lowering's verifier (`cig-script/src/lower/cir/verify.rs: verify`) takes a whole `Module`; a per-function lazy path would verify a function at its first call, which puts verifier time on the critical path and asks a per-function variant of the verifier. The first run of cigo on the VM is 305.9 ms (`cig-script/docs/ENGINES-FIRST-RUNS-1.25.0.md`); the share of module **instructions** that have to be lowered on the first call is the share of instructions in entered functions, bounded below by the share of instructions actually executed, which `data/census/report.md` reports as 30.3% to 32.6% across the seven cigo cases. The top level alone is about 30% of the module (34,841 to 34,911 instructions on 115,268 total) and runs first on every call. The budget for a lazy scheme is that the sum of lowering plus verifying per entered function, totalled over entered functions, is less than the whole-module lowering and verifying time; the saving is at most two thirds, not more than nine tenths.
- **How the per-function cache invalidates on a callee signature change at stage D.** With cig-compile#34's types, a function's compiled bytes depend on each callee's signature; entry 11's `nav_ty_deps` is the model, but cig's own "signature" at stage D is not yet named. The brief's expected set was "callee signatures, captured slots, the runtime ABI version, the level": the pin confirms level, the captured slots are `cig-script/src/lower/cir/ir.rs: VarInfo.captured`, and `cig-compile/docs/decisions/0001-backend.md` names the runtime ABI version. Callee signatures are not yet named.
