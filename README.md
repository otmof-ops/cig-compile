<p align="center">
  <img src="docs/assets/logo.svg" alt="CigCompile" width="520">
</p>

<p align="center">
  <b>Your CigScript scripts, as programs that run with nothing installed.</b><br>
  <i>A tool from the maker of <a href="https://github.com/otmof-ops/cig-script">CigScript</a>: the same burns, plan and rollback as <code>cig run</code>, byte for byte.</i>
</p>

<p align="center">
  <a href="https://github.com/otmof-ops/cig-script"><img alt="a CigScript tool" src="https://img.shields.io/badge/a-CigScript%20tool-f97316"></a>
  <a href="https://github.com/otmof-ops/cig-compile/releases/latest"><img alt="release" src="https://img.shields.io/github/v/release/otmof-ops/cig-compile?color=f97316&label=release"></a>
  <a href="docs/commands.md#cigcompile-targets"><img alt="9 machines" src="https://img.shields.io/badge/machines-9-6b7280"></a>
  <a href="docs/roadmap.md"><img alt="stage A shipped" src="https://img.shields.io/badge/stage-A%20shipped-6b7280"></a>
  <a href="LICENSE"><img alt="license Apache-2.0" src="https://img.shields.io/badge/license-Apache--2.0-6b7280"></a>
  <img alt="installs needed: 0" src="https://img.shields.io/badge/installs%20needed-0-2ea44f">
</p>

A script is easy to write and awkward to hand over. Whoever runs it needs
the interpreter, the right version of it, and a reason to trust both: a
Python script needs Python, and a CigScript script needs `cig`. On a
server you don't own, a friend's laptop or a fresh CI runner, that is the
part that goes wrong.

**`cigcompile` makes the script a program.** One file, for any machine cig
is released for, that runs with nothing installed. The program is cig
itself, of the version this compiler pins, with your checked script sealed
onto its end, so it does exactly what `cig run` does: the same burns, the
same plan from `--dry-run`, the same journal, and the same undo. The
differential suite holds every script CigScript carries to that, byte for
byte, on every pull request.

<p align="center"><img src="docs/assets/stages.svg" alt="The script is checked, then appended to cig of the version this compiler pins, the runtime held to its sha256; the one file runs on every machine cig is released for. Stage A, the bundle, is shipped; a bytecode VM is next, then native code and types." width="820"></p>

## Thirty seconds

```sh
cigcompile build tidy-downloads.cig          # one file: ./tidy-downloads
./tidy-downloads --dry-run ~/Downloads       # the plan; nothing changes
./tidy-downloads ~/Downloads                 # for real, every move journaled
./tidy-downloads --unburn-last 1             # changed your mind: everything back
cigcompile build tidy-downloads.cig --target universal2-apple-darwin   # one program for every Mac
cigcompile verify tidy-downloads             # each part is what a build writes
```

[Getting started](docs/getting-started.md) walks through it in ten
minutes, install included.

## See it run

[`tidy-downloads.cig`](https://github.com/otmof-ops/cig-script/blob/main/examples/tidy-downloads.cig)
is one of CigScript's own examples: it sorts a folder's loose files into
folders by kind. Built into a program, it plans, runs, proves what it is,
and puts everything back, with no cig on the machine:

<p align="center"><img src="docs/assets/terminal.svg" alt="cigcompile build tidy-downloads.cig makes one program; the program shows its plan with --dry-run, moves five files for real, cigcompile verify finds every part as a build writes it, and --unburn-last 1 puts every file back" width="820"></p>

## What you get

| what you get | how |
|---|---|
| one file, nothing installed | a program is cig with your script appended: it needs no cig, no runtime and no network |
| every machine cig is released for | `--target` builds for Linux (x86_64, aarch64, arm), macOS (each, or one universal program), Windows (x86_64, aarch64) and FreeBSD, from any machine |
| what `cig run` does, exactly | burns, `--dry-run` plans, journals, rollback, the four kinds of no and every explain page are cig's own; the differential suite proves it |
| an undo inside the program | `--unburn-last N`, `--unburn-since AGE` and `--unburn-to <run>` put back a stretch of the program's own runs, and leave other programs' alone |
| programs you can check | `cigcompile verify` holds the runtime to the sha256 this compiler pins, the script to its record and the whole file to a rebuild; `--sign` adds an SSH signature, `--sbom` a CycloneDX SBOM, and `audit` reads the hardening from the headers |
| the same bytes everywhere | the same script, compiler and runtime make the same file on any machine; CI builds one script on five runners and compares them |
| ready to ship | `--emit oci` makes a scratch image of one file, `--emit deb` a Debian package, and `package --brew` or `--scoop` a Homebrew formula or a Scoop manifest |
| all or nothing | a build that signs, writes an SBOM or a package stages every output beside `--out` and moves them into place together, or leaves what was there |
| quick rebuilds | a rebuild that changes nothing touches nothing, in about a millisecond; `--watch` builds again on every save |
| a look inside | `--emit cir` prints the verified IR, `--remarks` says what each pass did, and `--timings` where the build's time went |
| the numbers | `cigcompile bench` runs 29 programs interpreted and compiled, beside Python, Node and Rust ([BENCHMARKS.md](docs/BENCHMARKS.md)) |

## Guides

| guide | read it when you want to |
|---|---|
| [Getting started](docs/getting-started.md) | install cigcompile, build your first program, and run it where there is no cig |
| [Shipping a program](docs/shipping.md) | sign it, check it, give it an SBOM, or make an image, a `.deb`, a formula or a manifest of it |
| [Commands](docs/commands.md) | look up any command, flag, exit code, environment variable or file cigcompile keeps |
| [Troubleshooting](docs/troubleshooting.md) | understand an `E10xx`, or a build that won't go |
| [How it works](docs/how-it-works.md) | read the compiler's insides before you change them |
| [Roadmap](docs/roadmap.md) | see the stages to come, and how a request gets there |
| [Benchmarks](docs/BENCHMARKS.md) | read the numbers, and run them yourself |
| [The backend decision](docs/decisions/0001-backend.md) | see why stage A is a bundle, and what B, C and D will be |

## Get help, ask for things, pitch in

| you want to | go here |
|---|---|
| get unstuck building or shipping a program | [a guidance request](https://github.com/otmof-ops/cig-compile/issues/new?template=guidance-request.yml) |
| build for a machine or an output it doesn't make yet | [a target request](https://github.com/otmof-ops/cig-compile/issues/new?template=target-request.yml) |
| get something your other compiler does better | [a quality-of-life request](https://github.com/otmof-ops/cig-compile/issues/new?template=qol-request.yml) |
| report something that went wrong | [a bug report](https://github.com/otmof-ops/cig-compile/issues/new?template=bug-report.yml) |
| report a problem in the language itself | [CigScript's issues](https://github.com/otmof-ops/cig-script/issues) |
| report a security problem | [SECURITY.md](SECURITY.md), never a public issue |
| send a fix | [CONTRIBUTING.md](CONTRIBUTING.md) |

[SUPPORT.md](SUPPORT.md) says what happens after you ask.

## Install

Each release has an asset for every machine cig is released for, with its
sha256 and a build provenance attestation:

```sh
v=v0.2.0 asset=linux-x86_64     # the latest release; or linux-aarch64, linux-arm, macos-aarch64, macos-x86_64, freebsd-x86_64
base=https://github.com/otmof-ops/cig-compile/releases/download/$v
curl -LO "$base/cigcompile-$v-$asset" -LO "$base/cigcompile-$v-$asset.sha256"
sha256sum -c "cigcompile-$v-$asset.sha256"                       # shasum -a 256 -c on macOS
gh attestation verify "cigcompile-$v-$asset" -R otmof-ops/cig-compile
install -m 755 "cigcompile-$v-$asset" ~/.local/bin/cigcompile
```

On Windows the assets are `cigcompile-<version>-windows-x86_64.exe` and
`-windows-aarch64.exe`. The compiler needs nothing else: the first build
for a machine fetches cig's release for it, once, and holds it to the
sha256 this compiler was built knowing. `cigcompile fetch --all` does that
ahead of time, for a machine that builds offline. The `LICENSE`, `NOTICE`,
`THIRD-PARTY-NOTICES.md` and the SBOM are beside the assets.

## Where it runs

Programs run on the nine machines `cigcompile targets` lists: Linux on
x86_64, aarch64 and 32-bit ARM, macOS on each architecture or as one
universal program, Windows on x86_64 and aarch64, and FreeBSD on x86_64.
A Linux program runs on any Linux, because its runtime is static. CI runs
each program on a runner of its own machine, the universal one on both
Macs; 32-bit ARM Linux and FreeBSD programs are built and checked there,
not run. The compiler itself is released for the same machines.

## Limits, honestly

- **A program carries cig whole.** A compiled hello is about 11 MB and
  starts in about 8 ms here, as cig does. A runtime that is only what a
  program uses waits on the crate split ([#1](https://github.com/otmof-ops/cig-compile/issues/1),
  [#4](https://github.com/otmof-ops/cig-compile/issues/4)).
- **A program runs at the interpreter's speed.** Stage A is the bundle; the
  speed comes with the bytecode VM and native code
  ([#31](https://github.com/otmof-ops/cig-compile/issues/31),
  [#32](https://github.com/otmof-ops/cig-compile/issues/32)).
- **The box is Linux's.** A program's boxed hop is refused or falls back
  elsewhere, as `cig`'s is (`E777`, `E763`).
- **Signing is SSH's.** Apple Developer ID and Windows Authenticode need
  identities you hold, and are a separate step.

Each of these is on the [roadmap](docs/roadmap.md).

## Part of CigScript

CigCompile compiles CigScript, the language at
[otmof-ops/cig-script](https://github.com/otmof-ops/cig-script), and it is
built on it: it reads, checks and lowers a script through the `cigscript`
library of the version it pins, and the programs it makes are cig of that
version with the script inside. The lexicon, the rules the kernel keeps and
the payload format a program carries ([PROGRAMS.md](https://github.com/otmof-ops/cig-script/blob/main/docs/PROGRAMS.md))
are there. Later, CigarScript will compile here too.

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

