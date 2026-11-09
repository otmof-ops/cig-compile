# Changelog

All notable changes to CigCompile. The format follows Keep a Changelog; the
project follows semantic versioning.

## Unreleased

- **`build --sign` is all-or-nothing** (#66). A build with `--sign`
  checks the signer up front (`ssh-keygen` on this machine, the key file
  readable, E1004 before anything is written) and writes each output
  (program, `.sig`, SBOM, `.cdx.json.sig`, OCI image, `.deb`) to a
  staging folder beside `--out`. The files move into place only once
  every step has succeeded. A failing sign, SBOM or emit leaves `--out`
  exactly as it was, including an earlier signed program and its `.sig`
  byte for byte, and the error names what is still there (`nothing was
  written: out/fib is still the program from before (signed)`). The
  same rule holds for `--sbom` and `--emit oci`/`deb` without `--sign`.
  A program and its signature always match: a build that does not sign
  and replaces a program takes away the `.sig` (and the SBOM's) beside
  it, which signed the program from before.

- **Ten more benchmarks for #30.** `regex-ish`, `json-deep`, `strbuild`,
  `richards`, `deltablue`, `burn-mixed`, `orchestrate` and `startup-0`,
  `startup-1`, `startup-10` join the set, each with its `.expect` output
  and, where a language can say the same, Python, Node and Rust
  references that print the same bytes. `orchestrate` is
  cig-orchestrator's own 12.5k-line `orchestrate.cig` (the differential
  suite's copy) run with `help`, and a benchmark can now take
  arguments. The six that take a size read `args[0]` and ship a
  `.expect5x`: the harness runs them at 1x and 5x and reports `compute
  ms`, the slope between the medians, apart from start-up. No new CI
  jobs, no schedules.

## 0.2.0 — 2026-11-08

Programs for cig 1.18.0, checked, signed and read back for their
hardening; images and packages of them; two more passes; and the tools
the rest of the roadmap builds on.

- **Programs for cig 1.18.0.** The pinned runtime is cig 1.18.0's release:
  a faster interpreter (scopes and range loops, 10% to 40% on the
  benchmark set), `--seal-key-fd` and the rest of its tin, and a program
  puts back a stretch of its own runs with `--unburn-last N`,
  `--unburn-since AGE` and `--unburn-to <run>` (cig-script's notes for
  1.17.0 and 1.18.0 say it all).
- **CigScript's burn tests, run on compiled programs** (#6). A `burns` job
  on Linux and macOS runs cig-script's own pack, hop and wall suites, and
  the tin, seal, ledger, box, unburn and journal tests of its CLI suite,
  with every `cig run` building the script with this cigcompile and
  running the program. Skipped are the tests whose subject is the
  interpreter's own command line (a pinned script handed to another cig,
  a crash report's argv, golden text naming `cig run`).
- **`cigcompile verify <program>`** (#40) checks a program is what a build
  makes, part by part, and runs nothing: the runtime against the sha256 of
  cig's release assets this compiler pins (or a cig given with
  `--runtime`), the payload against its trailer and record (and the
  script, with `--script`), and the whole file against the bytes a build
  of those parts writes. Each part is `ok`, `DIFFERS` or `unchecked`; the
  exit is 1 when one differs.
- **`cigcompile audit <program>`** (#40) reads the hardening a program
  carries from its headers: ELF's PIE, RELRO, a stack that is not
  executable, CET or BTI and PAC; Mach-O's PIE, stack and code signature,
  each slice of a universal one; PE's ASLR, NX, Control Flow Guard and
  Authenticode signature. It only reports: the exit is 0 whatever it
  finds, and `--json` gives each part.
- **`build --sbom`** (#40) writes a CycloneDX 1.5 file beside each program,
  naming its runtime (cig's release asset, by version and sha256) and its
  script, with no time and no build path in it.
- **`build --timings[=trace.json]`** (#37) tables where a build's time went,
  phase by phase, and writes a Chrome trace on request.
- **`build --watch`** (#37) builds, then builds again each time a script
  is saved, until it is stopped.
- **`build --emit oci`** (#41) writes beside the program an OCI image of
  it, `<program>.oci.tar`: a scratch image of one file, the program, as
  its entrypoint, for Linux targets. `docker load -i` or `podman load -i`
  it, then run `localhost/<name>:latest`. Every time in it is zero and
  every owner root, so the same build writes the same bytes.
- **`build --json-progress`** (#12) says each step as it happens, one JSON
  object a line on stderr: the data the window will read.
- **`--emit cir` takes `-O0`, `-O1` and `-O2`, `--passes a,b,c` and
  `--passes-diff`** (#33): the passes by level or by name, in order, and a
  unified diff of what each one changed.
- **`cigcompile reduce <script>`** (#33, #39) cuts a script down to the
  smallest one that is still interesting, for a bug report: by default,
  that its program differs from it; with a command after `--`, that the
  command exits 0 on the candidate. Every candidate still passes `cig
  check`.
- **`cigcompile targets`** (#41) says each machine's stage, how CI proves
  its programs (run on its own runner, or built and not run), and what a
  program there lacks (the box is Linux's).
- **The differential suite reaches further** (#39): floats, the edges of
  i64 and their overflows, text past ASCII, closures made in loops, and
  values leaving through `finally`; three edge programs written by hand run
  in every suite, and 3,000 seeds are checked to make only programs `cig
  check` accepts.
- **Metamorphic tests** (#39): a generated program and variants of it that
  must mean the same (its variables renamed, its integer constants split
  into sums, dead declarations between its statements) must print the same
  and exit the same. At stage A that holds the interpreter to it; a later
  stage's programs will be held to it too.
- **`build --sign <key>` and `verify --signature <sig>`** (#10). A build
  with `--sign` writes a detached SSH signature of every program it made,
  `<program>.sig`, made with `ssh-keygen -Y sign` and the namespace
  `cigcompile-program`: a signature made here cannot be replayed as a git
  or generic file signature, and one made with another namespace fails to
  verify. `<key>` is an ssh private-key file, or a public-key file with
  `ssh-agent` holding the private one. With `--sbom`, the SBOM is signed
  too (`<program>.cdx.json.sig`).
- **`verify` gains a `signature` part.** `--signature <file>` holds the
  signature to a program by `ssh-keygen -Y verify` (with
  `--allowed-signers <file>` and `--identity <id>`) or by
  `-Y check-novalidate` (without one, reported `unchecked`). A bad
  signature exits 1 like any other difference.
- The Apple Developer ID and Authenticode parts of signing are a separate
  step: they need identities an operator holds, not something a compiler
  can build on its own (cig-script#153).

- **Two more CIR passes:** `copy-prop` folds chains of immutable copies
  (`stick a = b` uses `b`), and `sccp` empties the dead side of an `if`
  whose condition is a known boolean constant. Both run at `-O1` and
  `-O2`, between `fold-consts` and `dead-pure`, and `--passes` names them
  alongside the others. Neither changes behaviour: `copy-prop` only
  propagates through slots that are neither mutable nor captured, and
  `sccp` leaves a dead branch alone when emptying it would delete an
  effect (T3 catches that shape). Golden and negative tests sit under
  `tests/cir/copy-prop/` and `tests/cir/sccp/`.
- **`--emit cir-opt`, `--remarks` and `cigcompile bench <script>`** (#38).
  `--emit cir-opt` names the IR after the passes (at stage A it reads as
  `--emit cir`, so stage B can split them without a new flag). `--remarks`
  says on stderr what each pass did or left alone, a summary a pass and a
  line per change with its place, JSON lines with `--json`; a pass with no
  remarks of its own is said as having changed the IR or not.
  `cigcompile bench <script>` times one script, interpreted against
  compiled, through the same statistics as the benchmark set.
- **`build --emit deb`** (#41) writes beside a Linux program a `.deb` that
  installs it to `/usr/bin/<name>`: ar, tar and gzip, every time zero and
  every owner root, so the same build writes the same bytes; `--pkg-version`,
  `--pkg-maintainer` and `--pkg-description` fill its control file, and the
  architecture comes from the target. **`cigcompile package --brew` or
  `--scoop`** writes a Homebrew formula or a Scoop manifest for a release's
  assets, from flags or a JSON manifest.
- **Determinism checks and a real-world corpus** (#39): 34 float cases whose
  output IEEE-754 decides and 10 map insertion-order cases, each compared
  byte for byte between `cig run` and the compiled program; cig-orchestrator's
  scripts run in the differential suite at entry points that touch nothing.

## 0.1.2 — 2026-11-07

- **Programs for cig 1.16.1.** Its box makes the forwarder's listener
  before the hop starts, so a program's hop in the registry network that
  connects at once reaches the forwarder instead of being refused (a
  compiled program's forwarder is the program itself, and slower to start
  than `cig`).

## 0.1.1 — 2026-11-07

- **Programs for cig 1.16.0.** Its box records a refused connection before
  it answers it, so a program's hop that is refused is always said; run
  records can be pruned after every run (`keep_runs`, `keep_days`); and
  `verify_sample` checks objects from the whole tin after every burn.
- **A script whose name is not printable is refused** (E1009), before
  anything is written. cig refuses to run a program whose record holds a
  control or invisible character (otmof-ops/cig-script#137), and such a
  name can show on a terminal as another.

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
