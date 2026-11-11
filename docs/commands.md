# Commands

```
cigcompile build [path] [-o out] [--target T] [--runtime cig] [--check]
                 [--emit cir|cir-opt|oci|deb] [-O 0|1|2] [--passes a,b,...] [--passes-diff]
                 [--remarks] [--timings[=FILE]] [--json-progress] [--sbom]
                 [--sign <key>] [--pass] [--watch]
                 [--pkg-version V] [--pkg-maintainer "Name <email>"] [--pkg-description TEXT]
cigcompile test [path] ...    # same build flags; then runs the plan both sides and compares
cigcompile clean [path] [-o out] [--target T] [--force]
cigcompile targets
cigcompile fetch [--target T]... [--all]
cigcompile bench [script] [--set DIR] [--only NAMES] [--runs N] [--ref] [--markdown] [--runtime cig] [--budgets] [--orchestrate SCRIPT] [--alloc RUNTIME] [--against JSON]... [--current JSON]...
cigcompile bench --trend DIR
cigcompile audit <program>
cigcompile verify <program> [--script s.cig] [--runtime cig]
                            [--signature <sig>] [--allowed-signers <file>] [--identity <id>]
                            [--pass <program>.pass]
cigcompile report [ciginstaller bug options]
cigcompile reduce <script> [-o out] [--runtime cig] [-- command...]
cigcompile package --brew|--scoop [--manifest FILE]
                   [--name N --version V --homepage URL --license SPDX --description TEXT]
                   [--asset TARGET=URL,SHA256]... [-o OUT]
cigcompile --version
cigcompile help [command]
```

Global flags apply to every command: `--json` for machine-readable
output on stdout, `--plain` for the same information without the
catchphrases (also `CIG_PLAIN`), `--no-color` to turn colour off (also
`NO_COLOR`).

A build names the machine by its target triple
(`x86_64-unknown-linux-musl`, `aarch64-apple-darwin`...), as
`cigcompile targets` lists them. With no `--target` a build goes to
this machine.

### `cigcompile report`

`cigcompile report [options]` is the short way in to
`ciginstaller bug cigcompile`: it runs that with your options, shows what
it prints and exits with its exit code. Run `cigcompile reduce` first, so the
report carries the smallest script that still shows the problem. With no
ciginstaller on `PATH`, in `$CIGSCRIPT_HOME/bin` or in `~/.cigscript/bin` it
says so, prints how to get one and exits `127`. Until your ciginstaller has
`bug`, file [a bug report](https://github.com/otmof-ops/cig-compile/issues/new?template=bug-report.yml).
Messages that ask you to report a bug name the same two commands.

## Installing, updating and removing

cigcompile has no commands for these; [ciginstaller](https://github.com/otmof-ops/cig-installer#install)
does them: `ciginstaller install pack builder`, `ciginstaller update` and
`ciginstaller uninstall cigcompile`. `ciginstaller doctor --tools` shows
what cigcompile makes of its own runtime and caches.

## Building

### `cigcompile build`

Check each script (as `cig check` does) and write it as a program. A
`.cig` path builds that one script; a folder builds every `.cig` at its
top level. The default output path is `./<name>` for one script, and
`<folder>/bin/<name>` for a folder; `-o` overrides it.

Where other machines' programs go when a folder builds for another
target: `<folder>/bin/<triple>/<name>`.

Flags:

| flag | does |
|---|---|
| `-o, --out <path>` | one script's program path, or the folder for a folder build |
| `--target <triple>` | the machine the program is for; `cigcompile targets` lists them |
| `--runtime <path>` | the cig to append to, instead of the one found |
| `--check` | parse and check; write nothing, need no runtime |
| `--emit cir \| cir-opt` | the verified IR instead of a program; see [IR and passes](#ir-and-passes) |
| `--emit oci` | the program, and beside it an OCI image of it; Linux targets only |
| `--emit deb` | the program, and beside it a `.deb` that installs to `/usr/bin/<name>`; Linux targets only |
| `-O 0 \| 1 \| 2` | the optimization level: `0` lowering only; `1` and `2` run every pass there is. At stage A this shapes `--emit` only (a program carries the script) |
| `--passes a,b,...` | run exactly these passes, in this order, for bisecting (`fold-consts`, `copy-prop`, `sccp`, `dead-pure`) |
| `--passes-diff` | with `--emit`, a unified diff of what each pass changed, or that it changed nothing |
| `--remarks` | each pass says on stderr what it did or left alone; `--json` writes them as JSON lines. Costs nothing when off |
| `--timings[=FILE]` | a phase-by-phase table on stderr (read, index, check, pin, runtime, assemble, hash, write, sbom); `=FILE` also writes a Chrome trace for `chrome://tracing` or Perfetto |
| `--json-progress` | one JSON object a line on stderr (`start`, `script`, `phase`, `built`, `done`), for an editor or a window |
| `--sbom` | a CycloneDX 1.5 file beside each program, `<program>.cdx.json`, naming its runtime and script by sha256, with no time and no build path in it |
| `--sign <key>` | a detached SSH signature beside each program (and the SBOM, with `--sbom`); see [shipping](shipping.md) |
| `--pass` | a program pass beside each program, `<program>.pass`: a sealed pass from this machine's tin naming the program by sha256, its script, its cig, what built it and its target (and the SBOM's sha256, with `--sbom`); see [shipping](shipping.md#program-passes---pass) |
| `--watch` | build, then build again on every save, until Ctrl-C |
| `--pkg-version V` | with `--emit deb`, the control file's Version (default `0.0.0`) |
| `--pkg-maintainer "Name <email>"` | with `--emit deb`, the control file's Maintainer (default `cigcompile <nobody@localhost>`) |
| `--pkg-description TEXT` | with `--emit deb`, the control file's Description |

Where programs go, and how a build decides:

- **One script**: `./<name>`, with `<name>` the script's stem, or the
  `.exe` of that for a Windows target. `-o` names it directly.
- **A folder**: every `.cig` at the top level becomes a program under
  `<folder>/bin/`. For other targets it goes to
  `<folder>/bin/<triple>/`. `-o DIR` renames the `bin` folder.
- Every write is to a temporary file beside the output and renamed over
  it. A build that stops halfway leaves the old program there, or none.
- A build with `--sign`, `--pass`, `--sbom`, `--emit oci` or `--emit deb` writes
  every output to a staging folder beside `--out` first and moves them
  into place only when every step has succeeded: see [shipping a
  program](shipping.md#all-or-nothing-staging).

Progress: `compiled <script> -> <path> (<size>, cig <v> for <triple>)`,
then `  sha256 <hex>`. A build that changed nothing says `up to date:
<path> (<size>)` instead; it reads neither the runtime nor the program.

### `cigcompile test`

Build, then run each program and its script with `--dry-run` and
compare the plan, the output and the exit. The same build flags apply,
so `cigcompile test <folder>` tests every script in it. A script that
agrees with its program:

```
same     hello.cig (dry run: the same plan, output and exit)
```

A script that differs is named, and the exit is `1`.

At stage A a program *is* the interpreter, so a difference is a bug in
the compiler, or in the runtime it fetched. Keep its output and
`cigcompile reduce` cuts the script down to the smallest one that still
shows the problem.

### `cigcompile clean`

Remove the programs a build of this path wrote, and nothing else. A
program whose bytes are not the ones the build index recorded there is
kept, and said:

```
kept bin/hello: a program, but not one this cigcompile wrote there
  (compiled from hello.cig, sha256 614523912f57...); --force removes it
```

`--force` takes a program compiled from a script of the same name even
when this cigcompile did not write it there (a build index that was
cleared, or one from another checkout).

### `cigcompile targets`

Every machine cig is released for, and the runtime cigcompile found for
this one:

```
x86_64-unknown-linux-musl        linux-x86_64     stage A  run on its own runner                        this machine; runtime /home/you/.local/bin/cig
aarch64-unknown-linux-musl       linux-aarch64    stage A  run on its own runner
armv7-unknown-linux-musleabihf   linux-arm        stage A  built, not run
aarch64-apple-darwin             macos-aarch64    stage A  run on its own runner  no box (E777, E763)
...
```

The columns: the target triple, the release-asset short name, the
stage, how CI proves its programs (run on its own runner, run on both
Macs for universal2, or built and not run where there is no runner),
and what a program there lacks (`no box` on every target that isn't
Linux; see [how it works](how-it-works.md#the-box-is-linuxs)).

`--json` prints the same as objects.

### `cigcompile fetch`

Fetch cig's release for one or more machines ahead of time, so a build
later needs no network. `--target` can be passed more than once;
`--all` fetches every machine.

Each asset is kept at
`<cig home>/cigcompile/runtimes/<version>/<asset>/cig` and accepted
only at the sha256 this compiler was built knowing. See [how it
works](how-it-works.md#pinned-digests).

### `cigcompile bench`

Run the benchmark set, interpreted and compiled, and say how the stage
that exists meets the budgets. Also takes one script to bench on its
own, with `cig run <script>`'s output as the expected result. See
[benchmarks](BENCHMARKS.md) for the set, the measurement and the
references.

Flags:

| flag | does |
|---|---|
| `[script]` | one script to bench on its own, through the same harness as the set |
| `--set DIR` | benchmarks from this folder (`<name>.cig` beside `<name>.expect`) instead of the set |
| `--runs N` | the most samples per side (default 10; stops sooner once it has three and has run three seconds) |
| `--ref` | run the Python, Node and Rust references too, where they are installed |
| `--markdown` | a Markdown table, for a pull request comment |
| `--runtime <path>` | the cig to build and run with |
| `--budgets` | exit `1` when a budget that applies to this stage is missed |
| `--orchestrate <script>` | also time a full build of this script (#14's 10 s budget for `orchestrate.cig`) |
| `--only a,b,c` | only these benchmarks of the set (the pull request job's fast set) |
| `--alloc <runtime>` | allocation counts: one interpreted run of each benchmark under a runtime built with `--features alloc-count`, as an `allocs` column (a runtime that does not count gives a dash, never a zero) |
| `--against <json>` | the regression gate: compare with a result saved from `main` by `--json`; repeat the flag to pool interleaved rounds. Exit `1` when it trips (below) |
| `--current <json>` | with `--against`: this pull request's saved results instead of a run; nothing is run |
| `--trend <dir>` | the nightly trend gate over a folder of dated `--json` files; exit `1` after five slower nights in a row. Nothing is run |

A WRONG output fails the run: a side whose program's output differs
from the `.expect` file is marked WRONG, gets no time and gives the run
exit `1`.

The gates are in [benchmarks](BENCHMARKS.md#the-gates): `--against`
exits `1` when a non-noisy benchmark is more than 5% slower than
`main`'s, the set's geometric mean more than 2%, or peak memory or
allocations more than 10% (each by a Mann-Whitney test of the
samples), and `--trend` when the set has been slower five nights in a
row. A result file that is not `--json` output is a usage error (`3`),
never a pass.

## Reading what a program carries

### `cigcompile audit`

Read the hardening a program carries from its headers, which is cig's
build's: a program is cig with a script appended.

- **ELF**: PIE, RELRO (full with BIND_NOW, or static), a stack that is
  not executable, CET (IBT and shadow stack) or AArch64's BTI and PAC.
- **Mach-O**: PIE, the stack, a code signature, each slice of a
  universal binary.
- **PE**: ASLR, NX, Control Flow Guard, an Authenticode signature.

`audit` is a report: the exit is `0` whatever it finds. `--json` gives
each part as a field.

### `cigcompile verify`

Check a program is what a build makes, part by part, and run nothing.
The parts:

- **runtime** — the runtime against the sha256 of cig's release assets
  this compiler pins. `--runtime <cig>` holds it to a specific cig
  instead (a cig built from source).
- **payload** — the payload (the checked script and its record)
  against the sha256 its trailer and its record carry. `--script
  <path>` adds a check against the script on disk.
- **signature** — with `--signature <file>`, an SSH signature of the
  program. Without `--allowed-signers` only the signature's integrity
  is checked (`-Y check-novalidate`) and the part is reported
  `unchecked`. With one, the signer must be in the file; `--identity
  <id>` names which signer (the default is `*`).
- **pass** — with `--pass <file>`, a program pass (as `build --pass`
  writes it): it holds here (this machine made it, or a signer trusted
  here for programs did) and it names this program by sha256. Who is
  trusted is `--allowed-signers` when given (a line's namespaces must
  name `cigpass-program`), else cig's own allowed signers; either file
  must be yours, a regular file and closed to others' writes, or it
  trusts nobody. Checking a pass makes no key on a machine that has none.
- **whole** — the whole file against the bytes a build of those parts
  writes.

Each part is `ok`, `DIFFERS` or `unchecked`. The exit is `1` when one
part differs; the first `DIFFERS` is named.

### `cigcompile reduce`

Cut a script down to the smallest one that still shows the problem,
for a bug report. By default, "interesting" means a program built from
it differs from its script (what `cigcompile test` would find); at
stage A that never happens, and `reduce` says so. With a command after
`--`, "interesting" means the command exits `0` on the candidate:
`{}` is the candidate's path, else it goes last. Every candidate must
still pass `cig check`.

```
$ cigcompile reduce many.cig -- grep -q "sum" {}
reduced many.cig from 5 lines to 4 in 5 tests: many.reduced.cig
```

`-o` names the output; the default is `<name>.reduced.cig` beside the
input.

## Packaging a release

### `cigcompile package`

Write a Homebrew formula (`--brew`) or a Scoop manifest (`--scoop`)
for a released program, pointing at its release assets by URL and
sha256. Nothing is built: the formula or the manifest installs what the
release already published.

Flags:

| flag | does |
|---|---|
| `--brew` | a Homebrew formula (a Ruby class) |
| `--scoop` | a Scoop manifest (a JSON object) |
| `--manifest FILE` | a JSON manifest of `{name, version, homepage, license, description, assets: [{target, url, sha256}]}` |
| `--name N`, `--version V`, `--homepage URL`, `--license SPDX`, `--description TEXT` | the five fields; override a manifest's if one is given |
| `--asset TARGET=URL,SHA256` | a per-target asset, repeatable; overrides the manifest's for that target |
| `-o PATH` | write to a file instead of stdout |

`--brew` needs macOS or Linux assets; `--scoop` needs Windows ones.

## Exit codes

cigcompile shares cig's exit codes for the same things:

| code | meaning |
|---|---|
| `0` | ok |
| `1` | a difference (`test` found one, `verify` found one, `bench` missed a budget) |
| `2` | a script did not check |
| `3` | a usage error (a flag, a path, a missing runtime) |

A deeper fault (a panic the thread wrapper catches) exits `70`.

## Error codes

All the codes cigcompile prints end in `cig explain E<code>`, which
has the long version. Compiler-side codes live in `E1000`-`E1009`;
the runtime a compiled program carries meets `E1050`-`E1053` of its
own. [Troubleshooting](troubleshooting.md) is keyed by the exact
message.

**The compiler** (`E1000`-`E1009`):

| code | when |
|---|---|
| `E1000` | nothing to compile: the path holds no `.cig` script, or it cannot be read |
| `E1001` | no runtime for the target: no cig of the pinned version was found here, and none was fetched |
| `E1002` | the runtime is another cig: a `--runtime` that isn't the version this compiler pins |
| `E1003` | the script's pin is not met: its `# cig` line, or `.cig-version`, asks for a cig the runtime does not satisfy |
| `E1004` | the program cannot be written: a filesystem fault (ENOENT, EACCES, ENOSPC...). The message says which |
| `E1005` | the runtime already carries a program: the file given with `--runtime` ends in a program's trailer |
| `E1006` | an unknown target: `--target` is not one cig is released for |
| `E1007` | two programs would share one name: scripts whose names differ only in case, on a filesystem that does not tell case apart |
| `E1008` | the runtime would tie programs to this machine's libc: the only cig here is dynamically linked, so a program made from it would start only where the same glibc is |
| `E1009` | a script whose name is not printable: the name holds a control or an invisible character, and cig would refuse to run such a program (`E1050`) |

**A compiled program at runtime** (`E1050`-`E1053`):

| code | when |
|---|---|
| `E1050` | a damaged program: its sha256 does not match the trailer, its record does not read, or its trailer is not at its end |
| `E1051` | a program for another cig: the payload was appended to a cig it was not compiled for |
| `E1052` | not one of the program's run flags: a flag before `--` that cig does not recognize, or an argument that is not valid UTF-8 |
| `E1053` | not this program's run: a run id passed to `--unburn` that was not made by this program |

## Environment variables

- `CIGCOMPILE_RUNTIME` — the path to the cig to append to, when no
  `--runtime` is given. Same precedence rules as `--runtime` (see
  [how it works](how-it-works.md#the-runtime-search)).
- `CIGCOMPILE_OFFLINE` — when set and not `0`, no release is ever
  fetched: a runtime not kept here already exits `E1001`, and the
  message says `CIGCOMPILE_OFFLINE forbids fetching it`.
- `CIGSCRIPT_HOME` — where cig keeps its state. The build index lives
  in `<home>/cache/compile/`, and fetched runtimes under
  `<home>/cigcompile/runtimes/`. Defaults to `~/.cigscript`.
- `NO_COLOR` — never colour (the same effect as `--no-color`).
- `CIG_PLAIN` — plain output without the catchphrases (the same
  effect as `--plain`).

A compiled program also reads cig's own variables when it runs, such
as `CIG_MAX_STEPS` and `CIG_SEAL_KEY`; see cig's documentation.

## The files cigcompile keeps

Under `CIGSCRIPT_HOME` (default `~/.cigscript`):

| path | holds |
|---|---|
| `cache/compile/index.json` | the build index: what each program was built from, so a rebuild that changes nothing touches nothing |
| `cigcompile/runtimes/<version>/<asset>/cig[.exe]` | fetched runtimes, each kept at the sha256 this compiler pins |

Beside each program:

| file | from |
|---|---|
| `<program>.cdx.json` | `--sbom`: the CycloneDX 1.5 SBOM |
| `<program>.sig` | `--sign`: the detached SSH signature |
| `<program>.cdx.json.sig` | `--sbom --sign`: the SBOM's signature |
| `<program>.pass` | `--pass`: the program pass |
| `<program>.oci.tar` | `--emit oci`: the OCI image |
| `<name>_<version>_<arch>.deb` | `--emit deb`: the Debian package |

The build index is a shortcut: lost or damaged, the next build does
the work and writes it again. Fetched runtimes are a cache: they can
be removed, and the next build that needs one fetches it.

## IR and passes

`--emit cir` prints the verified IR ([#3](https://github.com/otmof-ops/cig-compile/issues/3)) instead of a program. The
IR is shaped by `-O` and `--passes`:

- **`-O0`** lowering only. No pass runs.
- **`-O1`** (the default) runs every pass there is in order:
  `fold-consts`, `copy-prop`, `fold-consts`, `sccp`, `dead-pure`.
- **`-O2`** is the same at stage A; stage B's passes are what will
  set `-O2` apart.
- **`--passes a,b,...`** runs exactly these passes, in this order,
  for bisecting a bad result.

`--emit cir-opt` names the IR after the passes. At stage A it reads
the same as `cir` at the chosen level; `cir-opt` is reserved so that
stage B can split the two without a flag change.

`--passes-diff`, with `--emit`, adds a unified diff of what each pass
changed, or a line saying it changed nothing. `--remarks` says the
same thing in the other direction: one line per change with
`file:line:col`, or a summary a pass. `--json` writes the remarks as
JSON lines.

Stage A programs carry the script, so `-O` shapes `--emit` only; a
program's run is the same whichever level emitted the IR. See
[how it works](how-it-works.md#the-ir-and-the-passes) for the design.
