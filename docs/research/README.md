<!-- SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile) -->
<!-- SPDX-License-Identifier: CC-BY-4.0 -->

# Research notes

What other compilers do, read from their source at a pinned commit, and what each mechanism would be in cigcompile: the study #98 asks for, before stage C (#32), the optimizer (#33), stage D (#34) and the work beside them are built. Read the study for a compiler before proposing anything from it: these notes are why nobody reads the same file twice, and why a rejected idea is not proposed again.

**A verdict here is a proposal.** Each entry says take, adapt or reject for cig, and why, by cig's rules. It becomes a decision in [../decisions/](../decisions/) when its measurement exists, and the entry then carries the run that decided it.

## What is here

| | |
|---|---|
| [studies/](studies/) | The 17 studies: 15 to 20 mechanisms each, every one with what it does and why, a verdict for cig, what would decide it, the issues and decisions it feeds, and the files read at the pin. |
| [INDEX.md](INDEX.md) | Every study, entry by entry, with its verdict. |
| [for-the-decisions.md](for-the-decisions.md) | The entries that feed each of the nine design decisions, (a) the value layout to (i) the type lattice. |
| [for-the-issues.md](for-the-issues.md) | What the studies say to each issue (#32 to #41, #17, and cig-script's #243 and #256). |
| [measurements-owed.md](measurements-owed.md) | Every measurement an entry asks for. |
| [not-settled.md](not-settled.md) | What each study could not establish from the source. |
| [data/census/](data/census/) | A census of what cig 1.25.0's VM executes: the types, shapes and callees each site saw, in 21 compute programs and in cigo's own script. Counts, not times. |
| [data/size/](data/size/) | What the released 1.25.0 runtime's code is made of, by crate and by module. |

## The studies

| | study | entries |
|---:|---|---:|
| 01 | [Cranelift: its IR and its acyclic e-graph mid-end](studies/01-cranelift-ir-and-mid-end.md) | 20 |
| 02 | [Cranelift's back end and regalloc2](studies/02-cranelift-back-end-and-regalloc2.md) | 20 |
| 03 | [Wasmtime as an ahead-of-time embedder of Cranelift, and Winch](studies/03-wasmtime-ahead-of-time-and-winch.md) | 19 |
| 04 | [SpiderMonkey's CacheIR and Warp: one inline-cache language for four tiers](studies/04-spidermonkey-cacheir-and-warp.md) | 20 |
| 05 | [Lean 4, Koka and Lobster: reference counting made cheap](studies/05-reference-counting-lean-koka-lobster.md) | 20 |
| 06 | [Swift's SIL and its ARC optimizer, and Roc's reference counting: ownership written into the IR](studies/06-swift-ownership-and-arc-roc.md) | 18 |
| 07 | [LLVM's optimization pipeline, and Alive2's translation validation](studies/07-llvm-pipeline-and-alive2.md) | 17 |
| 08 | [Go's compiler: a short pass list, rewrites as data, a linker that drops what the program never uses, and a profiler the program carries](studies/08-go-a-small-fast-compiler.md) | 20 |
| 09 | [Types for speed: Julia's abstract interpretation, .NET's shared generics and guarded devirtualization, and rustc's monomorphization](studies/09-types-julia-dotnet-rustc.md) | 15 |
| 10 | [MLton's whole-program path and GHC's worker and wrapper: monomorphization, defunctionalization and unboxing, and what the "hot loop takes raw i64s" would be in CIR](studies/10-whole-program-mlton-and-unboxing-ghc.md) | 17 |
| 11 | [How a dynamic value is laid out, across twelve implementations](studies/11-the-value-layout.md) | 15 |
| 12 | [V8 and JavaScriptCore: feedback, tiers, leaving optimized code, and a lighter back end](studies/12-v8-and-javascriptcore-tiers.md) | 20 |
| 13 | [Proving the compiler right: Csmith, YARPGen, C-Vise, Cranelift's fuzzing and verifier, CompCert and diverse double-compiling](studies/13-proving-the-compiler-right.md) | 17 |
| 14 | [Compiling fast: QBE, TinyCC, CPython's copy-and-patch, Zig, rustc and Cranelift's per-function cache](studies/14-compiling-fast.md) | 16 |
| 15 | [Profiles that outlive a run, and code compiled ahead of time from them: ART, .NET, and Leyden](studies/15-profiles-and-ahead-of-time-caches.md) | 18 |
| 16 | [The shape of an IR: nanopass and Chez Scheme, Zig's two IRs, the sea of nodes and leaving it, flambda at a distance](studies/16-ir-shapes-nanopass-zig-sea-of-nodes.md) | 16 |
| 17 | [From machine code to a program: object emission, embedded linking, size, debug tables, hardening, targets and the bootstrap](studies/17-from-machine-code-to-a-program.md) | 20 |

`join.py studies .` checks every study's shape and rewrites the five indexes above.

## The rules every study follows

1. **Read the design, reimplement it in cig's terms, never paste code.** Names, and the shape of a mechanism in words.
2. **cig's rules decide every verdict,** and a rejection names the rule it breaks:
   1. burns never move: no pass reorders, merges, hoists or drops an effect;
   2. errors happen at the same place, with the same code and span as the tree-walker's;
   3. the step budget is exact;
   4. floats are IEEE-exact, and maps keep insertion order;
   5. identical behavior on every engine: the tree-walker is the oracle;
   6. deterministic cleanup: reference counting, and cleanup that can be observed never moves;
   7. verified bytecode, never rewritten at run time;
   8. no memory both writable and executable; programs stay reproducible and signable;
   9. `unsafe` in one small crate only, audited and fuzzed;
   10. ahead of time by default.
3. **Every name is found.** A path a study cites exists at the pin, and a name it relies on occurs in the file it gives: a program checked all 1,892 paths and 2,743 names. What a study says a function does is what was read, not what its name suggests.
4. **Every study was checked by a second reader** who was told only to find where it is wrong: every statement about cig against the two repositories, every number, every mechanism against the functions cited. The checks made 82 findings, each corrected from the source.
5. **Licenses:**

   | license | compilers | rule |
   |---|---|---|
   | MIT, BSD, Apache-2.0 (with or without an exception), UPL, HPND, NCSA, PSF | Cranelift and `regalloc2`, LLVM, rustc, Zig, Swift, GHC, Chez Scheme and the nanopass framework, MLton, .NET, Julia, Roc, QBE, mold, `egg`, Alive2, Csmith, YARPGen, C-Vise, ART, V8, `gimli`, `object`, Go, Lean 4, Koka, Lobster, Lua, LuaJIT, QuickJS, Wren, CPython, Ruby | ideas freely; if code is ever adapted rather than reimplemented, its notice goes in `THIRD-PARTY-NOTICES.md` |
   | MPL-2.0 | SpiderMonkey | ideas freely; reimplement, never adapt a file |
   | LGPL | OCaml, TinyCC, the LGPL files of JavaScriptCore | ideas only, reimplemented |
   | GPL | GCC, HotSpot, GraalVM (no source read: papers and documents only) | ideas only, never code |
   | non-commercial | CompCert (its papers only) | ideas only, never code |

## Pins

The source read, each tree as a shallow clone of exactly one commit. A study names a tree by this folder name and a file by its path inside it.

| tree | upstream | commit | license |
|---|---|---|---|
| `lua-v5.4.9` | https://github.com/lua/lua | `312b9efaa106` | MIT (stated in lua.h) |
| `luajit-v2.1` | https://github.com/LuaJIT/LuaJIT | `63c8f1a9b479` | MIT |
| `cpython-v3.15.2` | https://github.com/python/cpython | `5e58eaad7c58` | PSF-2.0 |
| `koka-v3.2.9` | https://github.com/koka-lang/koka | `facb7932ce68` | Apache-2.0 |
| `lean4-v4.35.0` | https://github.com/leanprover/lean4 | `eb6cafb7290e` | Apache-2.0 |
| `quickjs-ng-v0.18.0` | https://github.com/quickjs-ng/quickjs | `d7215b0ad3fc` | MIT |
| `wren-0.4.0` | https://github.com/wren-lang/wren | `4a18fc489f9e` | MIT |
| `node-v22.23.5` | https://github.com/nodejs/node | `60553651579e` | MIT (V8: BSD-3-Clause, deps/v8/LICENSE.v8) |
| `ruby-v3_4_12` | https://github.com/ruby/ruby | `366f5fdc00cb` | Ruby OR BSD-2-Clause (COPYING, BSDL) |
| `pypy-release-pypy3.12-v8.0.1` | https://github.com/pypy/pypy | `d90944c94ad6` | MIT |
| `wasmtime-v50.0.1` | https://github.com/bytecodealliance/wasmtime | `c307ec81de02` | Apache-2.0 WITH LLVM-exception |
| `go-go1.27.4` | https://github.com/golang/go | `7b2881024c95` | BSD-3-Clause |
| `lobster-v2026.8` | https://github.com/aardappel/lobster | `acc66d71978a` | Apache-2.0 (README.md, dev/LICENSE.txt) |
| `llvm-project-llvmorg-23.1.6` | https://github.com/llvm/llvm-project | `5b10d2d74929` | Apache-2.0 WITH LLVM-exception (LICENSE.TXT) |
| `rust-1.100.0` | https://github.com/rust-lang/rust | `0ef1bc386059` | MIT OR Apache-2.0 (COPYRIGHT) |
| `regalloc2-v0.15.3` | https://github.com/bytecodealliance/regalloc2 | `43dad15e4b27` | Apache-2.0 WITH LLVM-exception |
| `zig-0.17.1` | https://codeberg.org/ziglang/zig | `be248cc1fdc3` | MIT |
| `swift-swift-6.4.3-RELEASE` | https://github.com/swiftlang/swift | `51bb1b87378c` | Apache-2.0 WITH Swift-exception (LICENSE.txt) |
| `ghc-ghc-9.14.1-release` | https://github.com/ghc/ghc | `902339d332fb` | BSD-3-Clause (LICENSE) |
| `chezscheme-v10.4.1` | https://github.com/cisco/ChezScheme | `e95a7efbafa2` | Apache-2.0 |
| `nanopass-v1.9.4` | https://github.com/nanopass/nanopass-framework-scheme | `bb47b5695a5b` | MIT (Copyright) |
| `mlton-on-20241230-release` | https://github.com/MLton/mlton | `b15e2d289c3d` | HPND (LICENSE) |
| `ocaml-5.5.2` | https://github.com/ocaml/ocaml | `c7b6787eb4c2` | LGPL-2.1-only WITH OCaml-LGPL-linking-exception |
| `spidermonkey-FIREFOX_160_0_1_RELEASE` | https://github.com/mozilla-firefox/firefox | `58fed86b1b68` | MPL-2.0 (the source headers; LICENSE points at toolkit/content/license.html) |
| `webkit-wpewebkit-2.54.3` | https://github.com/WebKit/WebKit | `c01fbd906b1f` | BSD-2-Clause (the source headers), with LGPL-2.0-or-later files (Source/JavaScriptCore/COPYING.LIB) |
| `dotnet-runtime-v11.0.0` | https://github.com/dotnet/runtime | `deae2e32bf0e` | MIT (LICENSE.TXT) |
| `julia-v1.13.4` | https://github.com/JuliaLang/julia | `c1ac32ab7335` | MIT (LICENSE.md) |
| `roc-alpha4-rolling` | https://github.com/roc-lang/roc | `d73ea109cc21` | UPL-1.0 |
| `qbe-v1.3` | https://github.com/michaelforney/qbe | `c0818978acec` | MIT |
| `tinycc-release_0_9_27` | https://repo.or.cz/tinycc.git | `d348a9a51d32` | LGPL-2.1 (COPYING) |
| `mold-v3.0.0` | https://github.com/rui314/mold | `8de38c35a2df` | MIT |
| `egg-v0.11.0` | https://github.com/egraphs-good/egg | `fb6167957beb` | MIT |
| `alive2-v21.0` | https://github.com/AliveToolkit/alive2 | `913e1556032e` | MIT |
| `csmith-csmith-2.3.0` | https://github.com/csmith-project/csmith | `30dccd73b786` | BSD-2-Clause (COPYING) |
| `yarpgen-v1.1` | https://github.com/intel/yarpgen | `e83208352387` | Apache-2.0 (LICENSE.txt) |
| `cvise-v2.12.0` | https://github.com/marxin/cvise | `2d0889e738e4` | NCSA (COPYING) |
| `art-android-17.0.0_r1` | https://android.googlesource.com/platform/art | `b753cf97923c` | Apache-2.0 (NOTICE) |
| `v8-15.8.62.31` | https://github.com/v8/v8 | `b601a2347f00` | BSD-3-Clause (LICENSE, LICENSE.v8) |
| `gimli-v0.34.0` | https://github.com/gimli-rs/gimli | `86df12a630d4` | Apache-2.0 OR MIT |
| `object-v0.41.0` | https://github.com/gimli-rs/object | `b94077c46c16` | Apache-2.0 OR MIT |

The papers and design documents a study cites as `papers/...` are named in the study with their authors and venue; each was fetched from an openly published copy and its first page read against its title.
