# CigCompile

CigCompile turns CigScript (and later CigarScript) into programs that run with nothing installed.

```sh
cigcompile build tools/tidy-downloads.cig      # -> ./tidy-downloads
./tidy-downloads ~/Downloads                     # no cig on the machine needed
```

Phase one compiles CigScript as it is. The interpreter is the reference: a compiled program must do exactly what `cig run` does, byte for byte, with the same burns, plan, journal, rollback and errors. The differential suite proves it on every script CigScript carries.

## How it works (stage A)

A program is the `cig` binary of the version this compiler pins, with the checked script appended. At start-up, `cig` finds the script at its own end and runs it through the same run as `cig run`. The payload format and the program's command line are in CigScript's [docs/PROGRAMS.md](https://github.com/otmof-ops/CigScript/blob/main/docs/PROGRAMS.md). Why this is the first stage, and what comes after it (a bytecode VM, then Cranelift), is in [docs/decisions/0001-backend.md](docs/decisions/0001-backend.md).

The runtime is built here, with the compiler: `cig-runtime` is `cig` of the pinned CigScript version, installed beside `cigcompile`. A program built with no `--runtime` uses the first of these:
1. `--runtime`, or `CIGCOMPILE_RUNTIME`;
2. `cig-runtime` beside `cigcompile`;
3. a `cig` of the same version on `PATH`, or in the humidor.

## The commands

| command | does |
|---|---|
| `cigcompile build [path] [-o out] [--target T] [--runtime cig] [--check]` | checks a script (or every `.cig` at the top of a folder) the way `cig check` does, then writes each as a program |
| `cigcompile test [path]` | builds, then runs each program and its script with `--dry-run` and compares them: the plan, the output and the exit |
| `cigcompile clean [path]` | removes the programs a build of this path wrote, and nothing else |
| `cigcompile targets` | the machines cig is released for, and the runtime found for this one |
| `cigcompile bench [--runs N] [--budgets]` | the benchmark set, interpreted and compiled, and the budgets from #14 |

- **Where programs go.** One script builds to `./<name>`; a folder builds to `<folder>/bin/<name>`, and `<folder>/bin/<target>/` for another machine. `-o` says otherwise.
- **The write.** A program is written beside its place and renamed over it, so a build that stops halfway leaves the old program or none.
- **No-op rebuilds.** A rebuild that changes nothing touches nothing, not even the time, and takes about a millisecond: the build index in `<cig home>/cache/compile` remembers what each program was built from.
- **Reproducible.** The same script, compiler and runtime give the same bytes.
- **Errors.** They are cig's, in the four kinds of no: the compiler's are `E1000` to `E1007`, the programs' `E1050` to `E1053`. `cig explain E1001` has the page.
- **Exit codes.** `0` ok, `1` a difference (`test`, `bench`), `2` a script that does not check, `3` usage.

## Building it

```sh
cargo build --release          # target/release/cigcompile and target/release/cig-runtime
cargo test                     # unit and end-to-end tests
CIGSCRIPT_SRC=../CigScript cargo test --release --test differential   # the differential suite
```

The differential suite reads its scripts from a CigScript checkout of the pinned version: the examples, the error corpus, `tests/scripts`, and the `cig` fences in the docs. It runs each under `cig run` and as a program, and fails on any difference in output, exit code or files left behind.

## Status

Phase one, stage A. The plan, and what is left of it, is in the issues: #1 (the crate split), #3 (the IR), #10 (cross-compiling and signing), #12 (the window) and on.
