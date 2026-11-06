# 0001. How CigCompile makes programs

**Status:** accepted for stage A, which ships in 0.1.0. Stages B to D are the plan, decided again by measurement when each exists.
**Issue:** #2 (design question 5 of CigarScript and CigCompile).

## The question

How does CigCompile turn a CigScript program into something that runs with nothing installed? The design names three ways:
- Cranelift;
- LLVM;
- generating Rust for `rustc`.

CigScript is dynamically typed, and phase one has one rule above speed: a compiled program does exactly what `cig run` does, byte for byte (#7).

## The decision: a staged path, each stage useful on its own

| stage | how | status |
|---|---|---|
| **A. bundle** | The program is the `cig` binary of the version the compiler pins, built for the target, with the checked script appended (CigScript 1.15.0, `docs/PROGRAMS.md` there). At start-up `cig` finds the script and runs it through the same run as `cig run`. | **shipped** in 0.1.0 |
| **B. bytecode VM** | A register bytecode from the IR (#3) and a VM in the runtime. | next; target at least 3× over tree-walking (#14) |
| **C. native code** | The IR lowered to Cranelift, calling the runtime library (#4) for dynamic values. | after B |
| **D. typed lowering** | Code typed at medium strength skips the dynamic value path. | needs CigarScript's types |

**Why A first.** It gives #5 ("hello.cig compiles and runs with nothing installed") on the first day, and #6 ("the same kernel") by construction: the kernel *is* the interpreter's. It also gives the differential suite (#7) a working compiler to compare against, so every later stage is measured against a known-equal baseline.

**Why Cranelift for C.** It is written in Rust (one toolchain, like cig), compiles fast, and covers x86_64 and aarch64. Wasmtime uses it in production.
- **LLVM:** it makes faster code but is a heavy C++ dependency. It may be added later as an optional `--release=max` back end if the benchmarks justify it.
- **Generating Rust for `rustc`:** rejected. Every build would need `rustc` on the machine, and builds would be slow.

## Measurements

The benchmark set is in `bench/`; `cigcompile bench` runs it. It covers what #2 names (file walking, CSV, JSON, string building, a tight numeric loop, a big burn), plus recursion, maps, lists and hello.

Conditions for the figures below:
- An Intel Core Ultra 7 255H laptop, Linux 6.17, release builds.
- `CIG_SEAL_KEY=file`; the keyring's cost is in the findings below.
- The median of 5 runs per side, each from a fresh folder. Times include starting the process.

```text
program      build ms    size MB interpreted ms  compiled ms    ratio  output
hello            28.3       11.0            4.7          4.0    1.17x  same
loop             21.9       11.0           91.1         81.7    1.12x  same
fib              23.4       11.0           16.8         15.4    1.10x  same
strings          21.7       11.0           29.4         28.3    1.04x  same
maps             22.8       11.0           42.6         41.8    1.02x  same
lists            19.7       11.0           34.3         33.9    1.01x  same
json             18.5       11.0           93.3         87.9    1.06x  same
csv              19.7       11.0           49.3         42.9    1.15x  same
files            19.4       11.0          141.4        135.5    1.04x  same
burn-big         20.4       11.0          477.5        485.7    0.98x  same
```

A laptop's clock moves between runs. An earlier run of the same set gave hello at 7.9 and 7.3 ms, and `files` at 294 and 481 ms. The steady figure is the ratio: **about 1.0×, as stage A predicts**, because the program runs on the interpreter. Every program printed the same as its script.

- **Build:** about 20 ms per program, most of it writing 11 MB. A rebuild that changes nothing takes **1.0 ms**: the build index (#9) reads neither the runtime nor the program, and runs no check. A full build of cig-orchestrator's `orchestrate.cig` (12,540 lines) takes 0.06 to 0.16 s, with a peak resident size of 46 MB.
- **Size:** 10.96 MB for hello. That is the whole runtime, 10.957 MB, plus 488 bytes of payload. The runtime is all of cig, its command line included.
- **Memory:** a program's is cig's for the same script.

## Findings for later stages

- **Start-up is cig's.** Spawned directly (median of 15), a dry run of hello takes 1.6 ms. A real run takes 5.0 ms with the seal key in a file, and 15.0 ms with the default keyring. The table above includes the spawn the bench makes, so its figures are a little higher.
- **The keyring costs about 10 ms per real run.** It is paid even when nothing burns: the journal asks for the seal key when it is made, and the ledger asks again. That is CigScript's cost, paid by `cig run` and programs alike, and filed as otmof-ops/CigScript#129 (ask on the first journal line, once per process). The 5 ms budget (#14) turns on it.
- **Size needs the trimmed runtime.** The 4 MB budget (#14) is out of reach while the runtime is all of cig. The crate split (#1) and stdlib features (#4) make a runtime that is only what a program uses.
- **Signing appended data.** macOS code signatures and Windows Authenticode do not cover bytes appended after signing. Programs run unsigned on both, and CI proves it. `--sign` (#10) puts the payload in a Mach-O section, or appends before Authenticode signs.

## Consequences

- 0.1.0 ships stage A: `cigcompile build`, `test` and `clean`, programs that run with nothing installed, and the differential suite.
- #3 (the IR) is the next piece of real compiler work. Stage B is built on it, and this record is revisited with stage B's numbers beside stage A's.
