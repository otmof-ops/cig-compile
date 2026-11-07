# CigCompile

CigCompile turns CigScript (and later CigarScript) into programs that run with nothing installed.

```sh
cigcompile build tools/tidy-downloads.cig      # -> ./tidy-downloads
./tidy-downloads ~/Downloads                     # no cig on the machine needed
```

Phase one compiles CigScript as it is. The interpreter is the reference: a compiled program must do exactly what `cig run` does, byte for byte, with the same burns, plan, journal, rollback and errors. The differential suite proves it on every script CigScript carries.

## How it works (stage A)

A program is the `cig` binary of the version this compiler pins, with the checked script appended. At start-up, `cig` finds the script at its own end and runs it through the same run as `cig run`. The payload format and the program's command line are in CigScript's [docs/PROGRAMS.md](https://github.com/otmof-ops/cig-script/blob/main/docs/PROGRAMS.md). Why this is the first stage, and what comes after it (a bytecode VM, then Cranelift), is in [docs/decisions/0001-backend.md](docs/decisions/0001-backend.md).

The runtime is built here, with the compiler: `cig-runtime` is `cig` of the pinned CigScript version, installed beside `cigcompile`. A program built with no `--runtime` uses the first of these:
1. `--runtime`, or `CIGCOMPILE_RUNTIME`;
2. `cig-runtime` beside `cigcompile`;
3. a `cig` of the same version on `PATH`, or in the humidor.

## Other machines

`cigcompile build tidy.cig --target aarch64-apple-darwin` builds a program for a Mac from any machine. Since cig 1.15.0 every cig is a runtime, so the runtime for another machine is cig's own release asset for it, of the version this compiler pins. It is:
- fetched once, into `<cig home>/cigcompile/runtimes/`;
- accepted only at the sha256 this compiler was built knowing (`src/digests.rs`, written by `ci/digests.sh` from the release's checksums).

`--target universal2-apple-darwin` makes one program for every Mac. Its runtime is the arm64 and x86_64 releases in one universal binary, made here as lipo makes one, from the two pinned assets. Each slice keeps its own signature, and the program's payload goes after both, where cig finds it.

A checksum fetched beside the asset would prove nothing against a tampered release; one built into the compiler does. `CIGCOMPILE_OFFLINE=1` forbids the fetch, and `--runtime` names a runtime by hand. CI builds for six machines and the universal Mac on one Linux runner, and runs each program on its own machine: the universal one on both Macs.

## The commands

| command | does |
|---|---|
| `cigcompile build [path] [-o out] [--target T] [--runtime cig] [--check]` | checks a script (or every `.cig` at the top of a folder) the way `cig check` does, then writes each as a program |
| `cigcompile test [path]` | builds, then runs each program and its script with `--dry-run` and compares them: the plan, the output and the exit |
| `cigcompile clean [path]` | removes the programs a build of this path wrote, and nothing else: a program there whose bytes are the ones the build index recorded. One of the same name from somewhere else is kept and named; `--force` removes it anyway |
| `cigcompile targets` | the machines cig is released for, and the runtime found for this one |
| `cigcompile fetch [--target T] [--all]` | fetches the runtime for other machines ahead of a build |
| `cigcompile bench [--runs N] [--budgets]` | the benchmark set, interpreted and compiled, and the budgets from #14 |

- **Where programs go.** One script builds to `./<name>`; a folder builds to `<folder>/bin/<name>`, and `<folder>/bin/<target>/` for another machine. `-o` says otherwise.
- **The write.** A program is written beside its place and renamed over it, so a build that stops halfway leaves the old program or none.
- **Clean.** It removes a file at one of a build's output paths only when that file is a program made from a script of the same name.
- **No-op rebuilds.** A rebuild that changes nothing touches nothing, not even the time, and takes about a millisecond: the build index in `<cig home>/cache/compile` remembers what each program was built from.
- **Reproducible.** The same script, compiler and runtime give the same bytes on any machine.
  - A program records its script's file name, never the path it was built from, and no time.
  - The runtime for another machine is cig's release, so builds for it agree everywhere. Builds for this machine agree too when its `cig` was installed from the release.
  - A `cig` built from source is the same version with bytes of its own, and so are the programs built on it.
  - CI builds one script for all six machines on five runners and checks that the bytes are identical.
- **Errors.** They are cig's, in the four kinds of no: the compiler's are `E1000` to `E1007`, the programs' `E1050` to `E1053`. `cig explain E1001` has the page.
- **Exit codes.** `0` ok, `1` a difference (`test`, `bench`), `2` a script that does not check, `3` usage.

## Building it

```sh
cargo build --release          # target/release/cigcompile and target/release/cig-runtime
cargo test                     # unit and end-to-end tests
CIGSCRIPT_SRC=../CigScript cargo test --release --test differential   # the differential suite
```

The differential suite reads its scripts from a CigScript checkout of the pinned version: the examples, the error corpus, `tests/scripts`, and the `cig` fences in the docs. It runs each under `cig run` and as a program, in a real run, a dry run, and with `--json` and `--plain`. It fails on any difference in output, exit code, the files left behind, or a real run's journal. It also holds every explain page a program shows to `cig explain`'s, in each output mode, and CI runs CigScript's portability probe twice, through cig and with every script built into a program (`ci/cig-compiled.sh`), and fails if any scenario differs.

Programs nobody wrote run too. `DIFF_RANDOM=<seed>:<count>` adds that many generated programs (`tests/differential/gen.rs`): functions, declarations, loops, `try`/`ashtray`/`finally`, pipelines, and burns under `out/` that now and then cough and roll back. A program that differs is shrunk to the fewest statements that still differ and printed with its seed, so `DIFF_RANDOM=<seed>:1` replays it; `DIFF_FILTER=random/` runs those alone. Every pull request runs 100 from a fixed seed, and every night `differential-random.yml` runs a thousand from a new one on Linux, macOS and Windows.

## Fuzzing

The compiler is the first reader of code that may come from someone else, so its front end is fuzzed (#16). The four targets live in `src/fuzz.rs`:
- `parse`: any text into the parser;
- `check`: a parsed program into the checker;
- `lower`: a program the checker accepts must lower to IR that verifies through every pass;
- `program`: any bytes as a program's trailer.

`fuzz/` holds them for cargo-fuzz, and `.github/workflows/fuzz.yml` runs each for five minutes every night, seeded with CigScript's own scripts. `tests/fuzz_smoke.rs` drives the same bodies on stable over a fixed sample on every test run, nesting past the parser's ceilings included. Each target runs on cig's 256 MB stack, as the compiler does: a script nested deeper than the parser allows is refused (E208), never a stack overflow.

```sh
cargo +nightly fuzz run lower fuzz/corpus/lower -- -max_total_time=60
```

## Status

Phase one, stage A. The plan, and what is left of it, is in the issues: #1 (the crate split), #3 (the IR), #10 (cross-compiling and signing), #12 (the window) and on.

## Licensing

- **Code** (`src/`, `tests/`, `bench/`, `ci/`, `fuzz/`): [Apache-2.0](LICENSE),
  the same license as CigScript. In plain words: use, copy, change, sell and
  embed cigcompile freely, including in closed products; keep the copyright
  notice and the [NOTICE](NOTICE) file with any copy; you get a patent license
  from every contributor; and you can't use the name to endorse your
  derivative.
- **Documentation** (`docs/`, this README and the other guides at the root):
  [CC BY 4.0](LICENSES/CC-BY-4.0.txt). Reuse and adapt with credit.
- **The programs it makes.** A program is cig with your script appended. Your
  script stays yours, under whatever license you give it. cig is CigScript's,
  under Apache-2.0, so whoever passes a program on gives the recipient a copy
  of that license and CigScript's
  [NOTICE](https://github.com/otmof-ops/cig-script/blob/main/NOTICE), as its
  section 4 asks of any copy of cig.
- **The name.** CigCompile and cigcompile are trademarks of Jay Taylor, as
  CigScript is. A fork needs a name of its own;
  [TRADEMARKS.md](TRADEMARKS.md) says what is free to do and what to ask about
  first.

Crates compiled into cigcompile are listed in
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md), all under permissive
licenses compatible with Apache-2.0. [REUSE.toml](REUSE.toml) maps every path
to its license. Contributions are accepted under the same terms, signed off
under the DCO; see [CONTRIBUTING.md](CONTRIBUTING.md). Security reports:
[SECURITY.md](SECURITY.md).

