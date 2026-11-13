# Changelog

All notable changes to CigCompile. The format follows Keep a Changelog; the
project follows semantic versioning.

## Unreleased

- **Payload format 2 is decided (#31, #71, #8, #40).** One container
  holds a program's bytecode, its archives, its own flags and its
  provenance: a short record, a table of hashed sections and a 56-byte
  trailer, so a start hashes a constant amount whatever the script's
  size. The record, with the order the parts land in, is
  [docs/decisions/0004-payload-format-2.md](docs/decisions/0004-payload-format-2.md).
  No built program changes yet.
- **The bytecode lowering uses the IR's `key` and stored spans (#31).**
  A computed map key is checked where the interpreter checks it, before
  the entry's value, so a map with a computed key no longer needs to be
  refused when a later value can fail or run code. Index and store
  errors take their positions from the spans the IR stores, so a pass
  that replaces an operand no longer makes the lowering refuse it. A
  member store (`obj.k = v`) now stores its object's span too, and a
  store into something that is not a map is reported there.
- **The bytecode lowering, fourth part (#31).** Burns, their ops and
  compensations, `pack` and chains lower to the VM's burn instructions,
  so a whole script lowers: every generated program and every CigScript
  example and test script but one does. A burn is `burn.enter` with its
  region, `comp.enter` with its `unburn` block, `comp.leave` and
  `burn.leave`, left innermost first by `break` and `continue` across
  tries; an effect is `burn.op` with the VM's kind; a pull with an effect
  outside its own burns is `requires_burn`. `pack` roots and chain steps
  are checked as each is evaluated, as the interpreter checks them.
  Refused with their position: a burn whose label is not a string
  literal (`orchestrate.cig`'s `burn (s, name)`), and a `pack` root that
  runs a pull or a function. The IR's burn now keeps its `unburn`
  block's span. `tests/bc.rs` holds burns to the tree-walker on output,
  errors, steps, files, journal, plan and rollback, at every step budget.
- **The bytecode lowering, third part (#31).** `try`/`ashtray`/`finally`
  lower to the VM's handlers (`try.enter`, `try.leave`), with `break`,
  `continue` and `snuff` running every `finally` they leave; `cough` is
  `throw`; a builtin, a module or `args` used as a value (`roll f =
  len`) is `loadg`. Only burns, `pack` and chains are still refused.
  `tests/bc.rs` holds every way out of a `try` to the tree-walker,
  output, error, position and step count, at every step budget.
- **The bytecode lowering, second part (#31).** Closures that capture
  (cells, a fresh one per loop turn) and pulls that call themselves or
  each other by name; lists and maps with the interpreter's errors at
  its positions (an index's type at the index, a value that cannot be
  indexed at the object); `for` over ranges, lists, maps and text;
  interpolation, `exhale` and method calls. `tests/bc.rs` now compares
  printed output byte for byte, and counts how many of CigScript's own
  scripts lower.
- **CIR lowers to the register bytecode, first part (#31).** With the
  `vm` feature, `src/bc` lowers a verified CIR module to the bytecode the
  cig runtime's VM runs, and the format crate's verifier checks every
  module before it leaves: constants, `roll`/`stick` slots and moves,
  arithmetic, comparison and short-circuit logic, `if`/`else if`/`else`,
  `while`, `break`, `continue`, pulls and lambdas that capture nothing,
  calls to them and to builtins, `snuff`, and step marks with the line
  table. Anything else is refused with its construct and position.
  `--emit bc` prints the disassembly (goldens in `tests/bc/`).
  `tests/bc.rs` runs each lowered program in the VM and holds its values,
  errors, positions and step-budget stops to the tree-walker's.
- **Step marks in CIR (#33, #31).** Lowering opens every statement but a
  `pull` with a `step`, and every loop body with a `step.iter`, one for
  each tick of the interpreter's step counter, at its span; `--emit cir`
  prints them. The verifier keeps them: a loop body opens with exactly
  one `step.iter` (V11), and a pass that drops a mark from a block that
  keeps anything else breaks T3. `tests/cir.rs` has
  `the_marks_a_run_passes_are_the_steps_the_tree_walker_counts`, which
  checks a script's marks against cig's own count and, for every
  `--max-steps` short of the total, its stopping line.
- **Map literals and indexes keep the interpreter's error order and
  positions in CIR (#31).** A computed map key is checked by a new
  `key` instruction right after it, before the entry's value, as the
  tree-walker does, so a bad key raises before a later value can. An
  index, an indexed store and a map's computed keys carry their
  operands' own spans (the verifier's V2 checks them; `--emit cir`
  prints them as `idx@`, `obj@`, `key@`), so the right line and column
  survive copy propagation and folding. `tests/cir.rs` holds both to
  the tree-walker: `a_computed_key_is_checked_before_the_next_value`,
  `a_computed_key_runs_its_effects_before_the_next_value_reads_them`,
  `an_index_keeps_its_operands_positions_through_the_passes`.
- **The pass framework in its final shape (#33).** Each pass is its own
  module under `src/cir/pass/`. `Kind::class` sorts every instruction,
  from its kind and its operands' types, into pure, may raise, effect
  or step mark. T1 and T2 now hold only what is not pure, so pure code
  may move, merge or vanish. The new rule T4 removes an operation that
  may raise only where the verifier proves it cannot: constant operands
  that the interpreter's arithmetic folds to the constant left in its
  place. `dead-pure` prunes by class, so an unused `a / b` stays and
  raises where the interpreter raises. `fold-consts` and `dead-pure` have
  goldens (`tests/cir/fold-consts/`, `tests/cir/dead-pure/`) and broken
  twins. `each_pass_has_a_negative_test_that_is_caught` runs the twin of
  every pass in `pass::NAMES`, and `remarks_name_their_sites` holds
  `--remarks` to a golden.
- **The bench's peak memory is the program's own (#30; carton 0.2.0 row 10).** Linux folds
  the parent's peak into a child's at `exec`, and the bench held the runtime it had read
  (15.8 MB for cig 1.21.0), so every row's peak was at least the bench's: CI read 24.7 to
  25.0 MB on cig 1.20.0's runtime and 28.3 to 28.4 MB on 1.21.0's for hello and every other
  small program alike. That is the "+3.5 MB on every program" 0.4.0 shipped as an accepted
  exception; the programs' own peaks had moved more (hello 5.2 to 13.9 MB, a dry run). The
  bench now lets go of the runtime's bytes before it runs anything, and takes each side's
  peak from one more run that is not timed: on Linux it forks and sets the child's mark back
  to what it has resident (`/proc/self/clear_refs`) before `exec`. Timed runs start as
  before. `a_benchmarks_peak_memory_is_its_own_not_the_benchs` holds it (docs/BENCHMARKS.md,
  "How a benchmark is measured").
- **Compiled peak memory has a budget (CigCompile 1.0 row 6).** `cigcompile bench` reports
  "peak memory of a compiled hello", under 14 MB, held on Linux: 5.9 MB on the runtime whose
  heap starts in small pages once the file is cold, 11.5 MB straight after the build on a
  kernel that maps a new file in large page-cache folios, 17.5 MB on cig 1.21.0 as released.
  `a_compiled_hello_over_its_memory_budget_is_missed` holds it. The budget is held from the
  first cig after 1.21.0, whose runtime starts a short program's heap in small pages again;
  on cig 1.21.0 as released a hello peaks at 14 to 18 MB, by the machine, and the row reads
  `not yet`.

## 0.4.0 — 2026-11-12

Programs for cig 1.21.0; CigCompile joins the cig family's shared
plumbing, installed through ciginstaller (and brought back out the
same way), its crashes join cig's one queue, and its bench runs on
the family's shared harness with regression, memory and trend gates.
`.cigar` files build at a strength; `cigcompile trial <new>` compares
two compilers side by side when a project pins a newer carton; and
the licence moves to FSL-1.1-ALv2 for every release from this one on.

- **Programs for cig 1.21.0.** The pinned runtime moves to cig 1.21.0's
  release (cig-script's notes for 1.21.0 say what the interpreter gained).
  The runtime a compiled program embeds is cig 1.21.0, so its
  `cigcompile verify` row prints `cig-v1.21.0-<target>`, and the pass,
  the SBOM and `cigcompile audit` name the same version. **The licence
  travels with the runtime.** Cig 1.21.0 is the first FSL-1.1-ALv2
  cig release (cig-installer DECISIONS 19), so the cig inside a 0.4.0
  program carries FSL-1.1-ALv2 too, not Apache-2.0; the SBOM reports
  it (`src/sbom.rs::runtime_license`) and the shipping guide's
  licence block has the split by cig release. A program built on an
  older cig with `--runtime` still carries that cig's own licence.
- **`cigcompile bench` on the family's harness (#68).** `bench/bench.toml` measures
  build, programs (each of 21 compiled and interpreted, on one runtime), start-up, size,
  artefacts and passes, and `cigcompile bench` is a front end of `cig bench` for
  `--manifest`, `--watch`, `--timings`, `--stats`, `--syscalls`, `--group`, `--cig` and
  `--against <ref>`; every flag it had still runs the set harness unchanged (`--against` of
  a saved `.json`, `--current` and `--trend` stay the regression gate). The set is read from
  the pinned cig's `bench/compute`, each file held to `bench/set.sha256` (`bench/set.sh`);
  `bench/prep-program.sh` builds each program on the `cig` the interpreted side runs and
  refuses unless `cigcompile verify` finds those exact bytes in it; `--syscalls` names a
  compiled-against-interpreted gap that is the allocator's `allocator, not the compiler`
  (cig-script#153); a build writes its phases to `CIG_TIMINGS_FD` as v1 lines. A nightly run on
  cig-script's reusable workflow (`bench-nightly.yml`) keeps the history on `bench-history`.
  Timings are reported, never gated. `tests/bench_front.rs` and `src/bench.rs` have
  `cigcompile_bench_flags_still_work`, `set_comes_from_the_pinned_cig`,
  `same_runtime_for_both_sides`, `allocator_gap_is_named` and `build_phases_on_the_fd`
  (docs/BENCHMARKS.md, "On the family's harness").
- **A crash in cigcompile joins the family's one queue (cig-installer#10).** A panic is written
  into cig's ashtray as a report from `cig-compile`, handed over the way `cigo` does it (`cig
  crash submit --origin cig-compile --repo otmof-ops/cig-compile`), or, with no `cig` on the
  machine, written in the same schema (built with cig's own crash code, so the redaction is cig's:
  every path a shape, the command line only the words cigcompile knows). `ciginstaller crash list`
  shows it and `ciginstaller crash send` files it here. Nothing is sent by the crash itself, and
  `reports = never` (the installer's setting, `CIG_REPORTS`, `CIGI_REPORTS`, cig's own config, or a
  machine lock the installer reports) saves nothing. `src/crash.rs` has the tests: a deliberate
  panic in a child of the test binary leaves one report with `origin` `cig-compile`, and every way
  of saying `never` leaves none.
- **Pins choose the compiler, and a trial checks a new one (#15).** A project pinned with
  `cigar = "1.0"` in `cigar.toml` or a `.cigar-version` builds with the cigcompile of the carton
  that holds that cigar, after 1.1 is installed too: the installer picks (the shim and
  `ciginstaller which` read the key), cigcompile never reads a pin
  ([docs/decisions/0003-pins-and-trial.md](docs/decisions/0003-pins-and-trial.md)).
  `cigcompile trial <new>` asks the installer where carton `<new>`'s cigcompile is, builds a
  scratch copy of the project with both and runs its tests, and prints what differs first: the
  output of `test`, the plan of every program (`--dry-run`), and, with `--differential <dir>`
  and no codegen flag different, a corpus really run by both. It pins nothing and ends with
  `ciginstaller pin carton <new>`; exit `0` the same, `1` differs, `3` it could not run, `127` no
  installer. `tests/trial.rs` has the cases; a compiled program's `compiler` and `cig` fields are
  asserted in `tests/cli.rs` (#6).
- **`.cigar` files and `--strength` (cigar-script#7).** `cigcompile build x.cigar` checks
  the file with the `cigarscript` crate (a git dependency on cigar-script, pinned as cigscript
  is) at the strength it declares, and compiles the CigScript it lowers to; `--strength mild |
  medium | full` overrides the declaration (turning down lists every use above the strength,
  `E1101`, exit 2). A `.cig` script builds as before and ignores `--strength`; a folder build
  takes both kinds. A mild file may `use` a medium module, and a wrong-typed value at that
  boundary stops the program with `E1210`. `strength full` asks once per project (the answer is
  kept in the person's state folder, never in the project); a build with no terminal never asks
  and says so once; `strength_full_warning = off` in the person's config turns it off.
  `tests/cigar.rs` has the cases, `every_cigscript_script_builds_as_mild_unchanged` over the
  `CIGSCRIPT_SRC` corpus; `DIFF_AS=cigar` runs the differential suite on every script as a
  `.cigar` file.
- **The benchmark programme's gates (#30).** `cigcompile bench --against <main.json>` is the
  regression gate: exit 1 when a non-noisy benchmark is more than 5% slower than `main`'s, the
  set's geometric mean more than 2%, or peak memory or allocations more than 10% over, each by a
  one-sided Mann-Whitney test of the raw samples (`--json` now carries them as `samples_ms`), so
  noise does not trip it; `--current` compares two saved results without running, and `--against`
  repeats to pool interleaved rounds. `bench-gate.yml` runs the fast set (`--only`) on `main` and
  on the pull request round by round and comments the table; `bench.yml` keeps the nightly
  history as the `bench-history` artifact and `bench --trend <dir>` fails after five slower nights
  in a row. `--alloc <runtime>` adds allocation counts, from a bench-only runtime built with
  `--features alloc-count` (a counting global allocator; a release build has none). Every report
  names Python, Node and Rust with their versions, or says one is not installed. Renamed to the
  issue's names: `a_wrong_output_fails_before_timing`, `every_benchmark_has_an_expect_file`,
  `the_markdown_report_renders`; new: `the_regression_gate_trips_at_five_percent`,
  `a_noisy_benchmark_is_not_gated` and the tests beside them (docs/BENCHMARKS.md, "The gates").
- **License: FSL-1.1-ALv2 from the next release.** The code moves from Apache-2.0 to
  the Functional Source License 1.1 with an Apache-2.0 future license: free to
  use, study, change and share, not to sell or to offer in a competing
  product or service, and each version becomes Apache-2.0 two years after it
  is made available. Every release through v0.3.0 stays Apache-2.0 and can't be taken back; the last Apache-2.0
  commit is the tag `apache-2.0-last`, and [LICENSING.md](LICENSING.md) holds
  the table. `LICENSE` is the publisher's text, `LICENSES/FSL-1.1-ALv2.txt`
  carries it beside the others, and `NOTICE`, `REUSE.toml`, the Cargo
  `license` fields, every SPDX header, the README and the trademark policy
  say the same. Documentation stays CC BY 4.0. The cig inside a compiled program carries its own release's license; docs/shipping.md and the README say so, and leave the question of Competing Use to the License's text. Contributions now come in
  under the contributor agreement (`.github/CLA`, signed by a comment and
  recorded by the `cla` workflow) instead of the DCO. Decision record:
  `docs/decisions/0002-licence.md` (cig-installer#27).

- **Installed by ciginstaller.** The README and getting-started guide no
  longer teach a manual download, checksum and install; they name
  `ciginstaller install pack builder` (or `install carton`), `ciginstaller
  update` and `ciginstaller uninstall cigcompile`. Releases, asset names
  and checksums are unchanged.
- **`cigcompile __doctor [--json]`** (hidden): the checks only cigcompile
  can make, for `ciginstaller doctor --tools`. It is read-only, runs
  nothing, always exits 0, and reports the pinned cig runtime, the build
  index, the humidor, fetched runtimes and OpenSSH.
- **Bugs** go to `ciginstaller bug cigcompile` once the installer has it;
  the bug template and SUPPORT.md say so, and suggest `cigcompile reduce`.
- **`cigcompile report [args]`** is the short way in to `ciginstaller bug
  cigcompile`: it runs it with the arguments, stdio inherited, and passes
  its exit code through. With no ciginstaller it says so and exits 127.
  Messages that ask for a bug report name `ciginstaller bug cigcompile` (or
  `cigcompile report`), with the issues link as the fallback.

## 0.3.0 — 2026-11-09

Programs for cig 1.20.0; program passes; a signed build that writes
everything or nothing; ten more benchmarks; and guides, a logo and
diagrams that put CigCompile beside the rest of the cig family.

- **Programs for cig 1.20.0.** The pinned runtime is cig 1.20.0's release,
  with cig 1.19.0's under it: passes (checked once, trusted while what
  they read holds), the tin hidden from boxed hops, a seal key that
  rotates and wipes itself, a keys folder that is private and checked
  (`E721`), a run history that stays quick as it grows, and `cig runs
  --verify` reading every journal (cig-script's notes for 1.19.0 and
  1.20.0 say it all).
- **Program passes** (cig-script#186). `build --pass` writes
  `<program>.pass` beside each program: a sealed pass from this
  machine's tin naming the program by sha256, its script, its cig, what
  built it and its target, and the SBOM's sha256 with `--sbom`. `verify
  --pass` holds it to the program and says who made it; another machine
  trusts it through its allowed signers (`cig pass key --kinds program`
  prints the line), and checking makes no key on a machine without one.
  It goes through the same all-or-nothing staging as `--sign`, and a
  build without `--pass` that replaces the program takes the old pass
  away.
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
- **Guides and the family's look** (#70). A logo, a social preview, a
  diagram of a build's stages and a terminal of a real session, drawn in
  the cig family's colours; a README in the shape of cig-orchestrator's;
  and the guides it points to: getting started, every command, shipping
  a program, how it works, troubleshooting by error code, and the
  roadmap. Every output shown is a real run's.

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
