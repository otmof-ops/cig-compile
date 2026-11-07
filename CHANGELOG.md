# Changelog

All notable changes to CigCompile. The format follows Keep a Changelog; the
project follows semantic versioning.

## 0.1.0 — 2026-11-07

Stage A: CigScript compiled into programs that run with nothing installed.
A program is the `cig` of the version this compiler pins (1.15.1) with the
checked script appended; it does exactly what `cig run` does, byte for
byte, through the same kernel, journal, plan and rollback.

- **`cigcompile build`** compiles a script, or every script in a folder,
  into a program (#9).
  - A build that changes nothing reads nothing and touches nothing: the
    build index.
  - `--check` checks the scripts and writes nothing.
  - `--emit=cir` prints the IR (#3).
- **Programs run with nothing installed** (#5). On Linux, a program's
  runtime is static, as cig's release is. A dynamically linked cig found
  here is passed over for the release, and the build says so when one is
  given by hand (#27).
- **Every machine cig is released for, from any machine** (#10).
  - `--target` builds for Linux x86_64, arm64 and armv7, macOS arm64 and
    x86_64, Windows x86_64 and arm64, and FreeBSD.
  - The runtime for another machine is cig's own release asset, accepted
    only at the sha256 this compiler pins.
  - `universal2-apple-darwin` makes one program for every Mac.
  - Each runtime is checked for the machine asked for: its kind of
    executable and its CPU.
- **The same bytes everywhere** (#9). A program records its script's file
  name only, so one script builds to the same bytes on any machine and by
  any path. CI builds on five machines and compares them.
- **`cigcompile test`** runs each program beside its script under
  `cig run` and says whether they agree. **`cigcompile clean`** removes
  only the programs a build of this path wrote (#28). **`cigcompile
  targets`**, **`fetch`** and **`bench`** list the machines, keep their
  runtimes, and measure the stage A budgets (#14).
- **The benchmark programme** (#30). `cigcompile bench` runs 19 benchmarks
  interpreted and compiled. Each output is checked first, then medians,
  spreads, the CV and peak memory are reported; `--ref` adds Python, Node
  and Rust references with the ratios, and `--markdown` and `--json` are
  the other formats. docs/BENCHMARKS.md has the set and the stage A numbers.
- **CIR** (#3): an IR where every effect is a structural `burn.op`, with a
  verifier (rules V1 to V10) and a transition check after every pass.
- **The differential suite** (#7). It runs every script CigScript carries
  under `cig run` and as a program, in a real run, a dry run, `--json` and
  `--plain`, and compares output, exit codes, the files left behind and
  the journals.
  - Every explain page, in each mode, is compared too.
  - So is CigScript's portability probe, compiled.
  - Every pull request runs 100 generated programs, and a nightly run
    takes a thousand from a new seed.
- **Fuzzing** (#16): the parser, the checker, the lowering and the program
  reader under libFuzzer, nightly, seeded with CigScript's own scripts.
- **The release** (#10). Assets for eight machines, each with its sha256
  and a build provenance attestation. Beside them: `LICENSE`, `NOTICE`,
  `THIRD-PARTY-NOTICES.md` and a CycloneDX SBOM of every crate compiled in.
