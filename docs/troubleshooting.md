# Troubleshooting

Every message cigcompile prints ends with what to try next. This page
is the longer version for the ones people meet most, keyed by the
error code the message carries. `cig explain E1001` prints the
canonical page for one of them.

Nothing here is a bug in your script until the compiler says so: every
message starts with the file and the kind of fault.

## Before a build starts

### `E1000 nothing to compile`

```
error[E1000 usage]: no-such-file.cig is not there
  = hint: name a .cig script, or a folder that holds them: cigcompile build tools/tidy.cig
```

The path is empty, not there, or holds no `.cig` script at its top
level. Name a `.cig` file, or `cd` into a folder whose top level has
one, or pass a folder with `cigcompile build <folder>`.

### `E1001 no runtime for the target`

```
error[E1001 usage]: no cig 1.20.0 to append the program to was found on this machine
  = hint: found, not usable: its release (cig-v1.20.0-linux-x86_64 is not kept here, and CIGCOMPILE_OFFLINE forbids fetching it); pass cig 1.20.0 with --runtime
```

cigcompile pins exactly one cig version. The runtime for this machine
is the first of:

1. `--runtime`, or `CIGCOMPILE_RUNTIME`;
2. `cig-runtime` beside `cigcompile` (what `cargo build` writes);
3. `cig` on `PATH` or in the humidor, when its version matches;
4. cig's static release, fetched once.

With none of the four, install cig of the pinned version (`cig
update`, or the release), or pass one with `--runtime`.

For another machine, the runtime is always cig's release asset for
it. `CIGCOMPILE_OFFLINE=1` forbids the fetch; `cigcompile fetch
--target <triple>` gets it ahead of time.

### `E1002 the runtime is another cig`

```
error[E1002 usage]: /home/you/old/cig is cig 1.16.1; this cigcompile makes programs for cig 1.20.0
  = hint: pass cig 1.20.0 with --runtime, or use the cigcompile built for cig 1.16.1
```

A cig given with `--runtime` must be the version this compiler pins
(`cigcompile --version` says which). Pass a cig of that version, or
install the cigcompile for the cig you have.

### `E1003 the script's pin is not met`

```
error[E1003 usage]: build.cig asks for cig >=1.21.0, and the program would run on cig 1.20.0
  = hint: pinned in build.cig (line 1): build with the cigcompile for that cig, or move the pin once the script works on 1.20.0
```

A `.cig-version`, a `# cig` header, or the manifest's `cig` key asks
for a newer cig than this cigcompile is for. Lower the pin, update
cigcompile to the one that pins the cig the script wants, or move the
pin once the script works on the installed cig.

### `E1004 the program cannot be written`

```
error[E1004 usage]: cannot write bin/hello: Permission denied (os error 13)
  = hint: write it somewhere you can with -o
```

A filesystem fault. The operating-system reason is in the message:
`ENOENT` (name a parent that exists, or a path `-o` creates), `EACCES`
(the folder's permissions, or a read-only mount), `ENOSPC` (free the
space the message names). The build left `--out` as it was.

### `E1005 the runtime already carries a program`

```
error[E1005 usage]: out/old is a compiled program (compiled from script.cig), not cig
  = hint: name cig (not a program) with --runtime
```

A file given with `--runtime` ends in a program's trailer: it was
compiled by another build. Pass cig itself; the runtime is the base,
not the output.

### `E1006 an unknown target`

```
error[E1006 usage]: `rustbsd` is not a target cig is released for
  = hint: cigcompile targets lists them, e.g. --target x86_64-unknown-linux-musl
```

`--target` names a target `cigcompile targets` lists. The listing has
the triples (`x86_64-unknown-linux-musl`) and the short asset names
(`linux-x86_64`); either works.

### `E1007 two programs would share one name`

A folder build whose scripts have names that differ only in case
(`Setup.cig` and `setup.cig`), on a filesystem that does not tell
case apart. Rename one, or move one of them out of the folder.

### `E1008 the runtime would tie programs to this machine's libc`

```
error[E1008 usage]: this machine's cig (/usr/bin/cig) needs glibc 2.39,
  so its programs would not start on most Linux machines
  = hint: cigcompile fetch for cig's static release, or build cigcompile
    with cargo build --release --target x86_64-unknown-linux-musl
```

The only cig here is dynamically linked, so a program made from it
needs the same glibc on every machine that runs it. cigcompile won't
make one: run `cigcompile fetch` (or let the build fetch), or pass a
static cig with `--runtime`. `cigcompile build --runtime <dynamic cig>`
uses one anyway, and warns.

### `E1009 a script whose name is not printable`

```
error[E1009 usage]: script\u{202e}.cig: the script's name holds a control
  or invisible character, and a program prints the name it was compiled from
```

A program carries its script's name and prints it in errors and in
`cig runs`. A name that carries a control (`\n`, `\t`) or an
invisible character (`U+202E`, a zero-width space) could show as
another name on a terminal; cig refuses to run such a program
(`E1050`), so cigcompile refuses to make one. Rename the script to a
name that shows as it is.

## While a program runs

A compiled program is cig for its target, so its errors are cig's
errors with a few of its own. These four are the program-side codes
`E1050`-`E1053`:

### `E1050 a damaged program`

The program's bytes don't match the sha256 in its trailer, its record
doesn't read, or its trailer is not at its end. It was cut short,
edited after compilation, or had bytes written after it. A sha256
catches damage, not tampering (programs are unsigned): if you built
it, compile it again (`cigcompile build`); if you downloaded it,
fetch it again. Zero padding, or a Windows certificate table from
signing, is fine after the trailer.

### `E1051 a program for another cig`

The program's payload was appended to a cig it was not compiled for.
This never happens to a build cigcompile wrote: it's what you get
from pasting a payload onto another cig by hand. Compile the script
again with the cigcompile for the cig you want.

### `E1052 not one of the program's run flags`

```
error[E1052]: a compiled program takes the run's flags first and its own arguments after them
```

A flag before `--` that cig does not recognize (a program accepts
the inherited run flags; `./program --help` lists them), a flag that
needs a value, or an argument that is not valid UTF-8. The program's
own arguments go after `--`: `./tool --dry-run -- --tag v2`.

### `E1053 not this program's run`

A `--unburn <id>` passed to a program for a run id that was made by
a different script or by another program. `cig unburn <id>` rolls
back any run; a program's own `--unburn` is its own runs only.

## After a build

### `cigcompile test` says a program differs

At stage A a program *is* the interpreter, so a difference is a bug
in the compiler, in the runtime, or in the kernel cig has here. The
failing script and command go in a bug report: `cigcompile reduce
<script>` cuts it down to the smallest script that still differs
(by default; with a command after `--`, to the smallest that makes
the command exit `0`), and every candidate still passes `cig
check`.

### `cigcompile verify` says `DIFFERS` on `whole`

The program's bytes aren't what a build of these parts writes. The
runtime and payload parts are `ok` on their own, so something was
written into the program after a build: a signing step, a packer,
or an edit by hand. A program and its signature always match on
disk after a `cigcompile build --sign`; if the signature is there
and the program differs, the signature is not for this file.

### `cigcompile verify` says `DIFFERS` on `script`

The payload's sha256 of the script doesn't match the script on
disk. The script changed after the program was compiled — a usual
sign the build is stale. Build again.

### `cigcompile verify --signature` says `DIFFERS` on `signature`

`ssh-keygen -Y verify` reported the signature does not belong to
this program. Either the program was replaced (rebuild) or the
`.sig` was replaced (fetch it again with the program).

### `warning: /usr/bin/cig is dynamically linked`

```
warning: /usr/bin/cig is dynamically linked, so its programs need glibc 2.39 to start
  and will not run on most Linux machines; cig's release for x86_64-unknown-linux-musl
  needs nothing (build without --runtime, or cigcompile fetch first)
```

`--runtime` named a dynamic cig. The build went ahead because you
asked for it by hand; the program will start only where the C
library the warning names is installed. Drop `--runtime`, or run
`cigcompile fetch` once and let the build pick the static release.

## Running offline

```
error[E1001 usage]: cig-v1.20.0-macos-aarch64 is not kept here, and CIGCOMPILE_OFFLINE forbids fetching it
  = hint: pass the cig for aarch64-apple-darwin with --runtime, or fetch it first: cigcompile fetch --target aarch64-apple-darwin
```

`CIGCOMPILE_OFFLINE=1` was set, and the runtime for the target wasn't
kept here already. Either unset it, run `cigcompile fetch` once with
network, or pass the runtime by hand with `--runtime`.

## Still stuck

Open a [guidance
request](https://github.com/otmof-ops/cig-compile/issues/new?template=guidance-request.yml)
with the command, what it printed, and `cigcompile --version`. If
it looks like a bug, a [bug
report](https://github.com/otmof-ops/cig-compile/issues/new?template=bug-report.yml)
gets it fixed, with a test so it stays fixed. For a `cigcompile
test` difference, the reduced script `cigcompile reduce` wrote
belongs in the report.
