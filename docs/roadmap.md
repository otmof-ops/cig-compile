# Roadmap

cigcompile is a staged compiler: each stage is useful on its own, and
each is measured against the one before. This page is the plan, in the
open, with issue links. It changes as people ask for things.

## Promises that don't change

- **Programs do what their scripts do.** The differential suite
  ([how it works](how-it-works.md#the-differential-suite)) holds every
  program byte for byte to `cig run`: the plan, the output, the exit,
  the files left behind. Zero differences at a release, every stage,
  every target, every level.
- **Pinned digests.** The runtime for another machine is cig's release
  asset, accepted only at the sha256 this compiler was built knowing
  ([pinned digests](how-it-works.md#pinned-digests)). A tampered
  release does not match.
- **Reproducible builds.** The same script, compiler and runtime give
  the same bytes on any machine, and CI holds it.
- **The next cig is pinned, not followed.** cigcompile pins one cig
  version. Bumping it is an audited step; see
  [pinning a new cig](how-it-works.md#pinning-a-new-cig).

## The stages

| stage | how | status |
|---|---|---|
| **A. bundle** | A program is cig for the target with the checked script appended. At start cig finds the script and runs it through the same run as `cig run`. | **shipped** in 0.1.0 ([#5](https://github.com/otmof-ops/cig-compile/issues/5), [#9](https://github.com/otmof-ops/cig-compile/issues/9)); hardened and packaged in 0.2.0 |
| **B. bytecode VM** | A register bytecode from the IR ([#3](https://github.com/otmof-ops/cig-compile/issues/3)) and a VM in the runtime. | next; set median at least 3x the tree-walker, CPython parity ([#31](https://github.com/otmof-ops/cig-compile/issues/31), [#14](https://github.com/otmof-ops/cig-compile/issues/14)) |
| **C. native code** | The IR lowered through Cranelift, calling the runtime library ([#4](https://github.com/otmof-ops/cig-compile/issues/4)) for dynamic values. | after B; set median at least 15x the tree-walker, 3x CPython ([#32](https://github.com/otmof-ops/cig-compile/issues/32)) |
| **D. typed lowering** | Code typed at medium strength skips the dynamic value path. | needs CigarScript's types; within 3x of Rust on typed numeric code ([#34](https://github.com/otmof-ops/cig-compile/issues/34)) |

[The backend record](decisions/0001-backend.md) is where these came
from, and the record itself is revisited at the end of each stage with
measured numbers beside it.

## The phases of #42

Issue [#42](https://github.com/otmof-ops/cig-compile/issues/42) is the overall plan: what a really good compiler does,
which issue covers each part, and the order they land in. The phases:

| phase | lands | needs | done when |
|---|---|---|---|
| **0. foundations** | [#30](https://github.com/otmof-ops/cig-compile/issues/30)'s harness and gates; [#27](https://github.com/otmof-ops/cig-compile/issues/27), [#28](https://github.com/otmof-ops/cig-compile/issues/28); cig-script's misuse-defaults table | — | every benchmark gated; the misuse table closed |
| **1. stage B** | [#31](https://github.com/otmof-ops/cig-compile/issues/31), with [#35](https://github.com/otmof-ops/cig-compile/issues/35)'s first value changes and [#33](https://github.com/otmof-ops/cig-compile/issues/33)'s first passes | [#3](https://github.com/otmof-ops/cig-compile/issues/3) | set median at least 3x the tree-walker; CPython parity as the stretch; zero differences |
| **2. the scaffolding a fast compiler needs** | [#39](https://github.com/otmof-ops/cig-compile/issues/39) (random programs, pull-request fuzz), [#38](https://github.com/otmof-ops/cig-compile/issues/38) (backtraces, `--emit=bytecode`, the profiler), [#37](https://github.com/otmof-ops/cig-compile/issues/37) (`--timings`, the parallel pool) | phase 1 | the pull-request fuzz job catches a planted miscompile |
| **3. small and quick** | [#36](https://github.com/otmof-ops/cig-compile/issues/36) with [#1](https://github.com/otmof-ops/cig-compile/issues/1) (crate split) and [#4](https://github.com/otmof-ops/cig-compile/issues/4) (runtime library); [cig-script#129](https://github.com/otmof-ops/cig-script/issues/129) | phase 1 | hello at 4 MB ([#14](https://github.com/otmof-ops/cig-compile/issues/14)), then 2 MB; 2 ms |
| **4. stage C** | [#32](https://github.com/otmof-ops/cig-compile/issues/32); [#35](https://github.com/otmof-ops/cig-compile/issues/35)'s shapes and typed lists; [#33](https://github.com/otmof-ops/cig-compile/issues/33)'s inlining, check elimination and rc elision; [#40](https://github.com/otmof-ops/cig-compile/issues/40)'s mitigations for native code | phases 1-3 | set median at least 15x the tree-walker; at least 3x CPython; within 2x of Node on compute |
| **5. stage D** | [#34](https://github.com/otmof-ops/cig-compile/issues/34) inference first, then CigarScript's declared types | phase 4; otmof-ops/cigar-script | within 3x of Rust on `loop`, `mandel`, `nbody`, `sieve`, `fib` |
| **6. reach** | [#41](https://github.com/otmof-ops/cig-compile/issues/41), [#10](https://github.com/otmof-ops/cig-compile/issues/10)'s signing, [#12](https://github.com/otmof-ops/cig-compile/issues/12) (the window), [#13](https://github.com/otmof-ops/cig-compile/issues/13) (`cigarscript` in cigo) | phase 4 | programs on every listed target and in every listed form |
| **7. the proof** | [#8](https://github.com/otmof-ops/cig-compile/issues/8) (cigo compiled), [#17](https://github.com/otmof-ops/cig-compile/issues/17) (self-hosting) | phases 4-5 | cigo ships as a compiled program; cigcompile builds itself to identical bytes |

## What's in today

Stage A, the whole thing: programs for every machine cig is released
for, the build index, the differential suite, the cross-compiler, the
benchmark harness, the IR with its passes, hardening reports,
signatures, SBOMs, OCI and `.deb` outputs, Homebrew and Scoop
manifests, `verify`, `audit`, `reduce`, and the fuzzing of the front
end. The release notes
([CHANGELOG.md](../CHANGELOG.md)) are the ledger.

## What's next

The next pieces of work, ordered against the phases above:

- **A bytecode VM** ([#31](https://github.com/otmof-ops/cig-compile/issues/31)). Phase 1 of [#42](https://github.com/otmof-ops/cig-compile/issues/42), the first time a
  cigcompile program runs code the compiler wrote.
- **The crate split** ([#1](https://github.com/otmof-ops/cig-compile/issues/1)). The runtime for a program should be only
  what the program uses, not all of cig. It's the way size and
  start-up come down together ([#36](https://github.com/otmof-ops/cig-compile/issues/36), [#14](https://github.com/otmof-ops/cig-compile/issues/14)).
- **cig-smith and metamorphic tests** ([#39](https://github.com/otmof-ops/cig-compile/issues/39)). Random programs, and
  variants of them that must mean the same, fuzzed and reduced. The
  correctness scaffolding stage B and stage C sit on.
- **Debugging and profiling in CigScript's terms** ([#38](https://github.com/otmof-ops/cig-compile/issues/38)). Backtraces
  in pull names, `gdb`/`lldb`/WinDbg against `.cig` lines, a built-in
  profiler, `--emit` at every stage.
- **More targets and outputs** ([#41](https://github.com/otmof-ops/cig-compile/issues/41)). WASI, RISC-V, Android, a
  library with a C header; the outputs that need operator identities
  (Apple Developer ID, Authenticode) wait on [#10](https://github.com/otmof-ops/cig-compile/issues/10)'s signing track.

## What waits on what

- **Stage B** waits on the IR ([#3](https://github.com/otmof-ops/cig-compile/issues/3)); phase 2 waits on stage B; stage
  C waits on phase 2 and on [#35](https://github.com/otmof-ops/cig-compile/issues/35)'s shapes; stage D waits on
  CigarScript's types.
- **A trimmed runtime** waits on the crate split and on the runtime
  library a compiled program links against ([#1](https://github.com/otmof-ops/cig-compile/issues/1), [#4](https://github.com/otmof-ops/cig-compile/issues/4)); the 4 MB
  budget needs both.
- **Debugging a compiled program in CigScript's terms** ([#38](https://github.com/otmof-ops/cig-compile/issues/38)) waits
  on a program that is more than a bundle (stage B onwards): at stage
  A the compiled program is the interpreter, so cig's own backtraces
  cover it.

## How a request gets here

1. Open the form that fits: a
   [bug report](https://github.com/otmof-ops/cig-compile/issues/new?template=bug-report.yml)
   when something went wrong; a
   [guidance request](https://github.com/otmof-ops/cig-compile/issues/new?template=guidance-request.yml)
   when you're stuck; a
   [target request](https://github.com/otmof-ops/cig-compile/issues/new?template=target-request.yml)
   for a machine or an output a program should support; a
   [quality-of-life request](https://github.com/otmof-ops/cig-compile/issues/new?template=qol-request.yml)
   for something another language or tool does better.
2. The form labels it, and I read it and reply there.
3. When it is accepted, it gets a line on this page.
4. When it ships, the [changelog](../CHANGELOG.md) says so, and the
   issue is closed with the version that has it.

See cigcompile's language counterpart ([CigScript](https://github.com/otmof-ops/cig-script))
for anything that is really the language's; a `.cig` script's own
syntax, kernel, or standard library belong there, not here.
