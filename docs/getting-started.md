# Getting started

Ten minutes: install cigcompile, build a program from a one-line script,
run it on a machine with no cig, then build one for another machine and
check what came out.

## 1. Install cigcompile

[ciginstaller](https://github.com/otmof-ops/cig-installer) installs it,
and checks the download against the release's sha256 and attestation
first:

```sh
ciginstaller install pack builder      # cigcompile with the builder's other tools
# or: ciginstaller install carton      # the whole family at one tested version
cigcompile --version
```

No ciginstaller yet? [How to get it today](https://github.com/otmof-ops/cig-installer#install)
is in its README; once it has a release, that page shows its bootstrap
script. To update cigcompile later, run `ciginstaller update`; to remove
it, `ciginstaller uninstall cigcompile`.

cigcompile needs nothing else installed. A build for this machine uses the
`cig` of the pinned version when it's here, and fetches cig's release the
first time it isn't. On Windows and macOS an SSH client is only needed for
`--sign` and `verify --signature`; `--pass` and `verify --pass` need
nothing.

## 2. Build your first program

Any `.cig` script compiles. A one-line script:

```sh
cat > hello.cig <<'EOF'
exhale "hello from a compiled program"
EOF
cigcompile build hello.cig
```

The first build on a machine prints the fetch, once:

```
fetching cig-v1.24.0-linux-x86_64 from otmof-ops/cig-script (sha256 957ddff2c27c...)
kept /home/you/.cigscript/cigcompile/runtimes/1.24.0/linux-x86_64/cig (sha256 matches the pin)
compiled hello.cig -> hello (16.3 MB, cig 1.24.0 for x86_64-unknown-linux-musl, stage a)
  sha256 445143184b39c480f0934ce7339909d61fed286ed7a7f1ff0d6dab4285406af0
```

A second `cigcompile build` with nothing changed reads neither the
runtime nor the program, writes nothing, and takes about a
millisecond:

```
up to date: hello (16.3 MB)
  sha256 445143184b39c480f0934ce7339909d61fed286ed7a7f1ff0d6dab4285406af0
```

That's the build index at work; see [how it works](how-it-works.md).

## 3. Run it with nothing installed

```sh
./hello
```

```
hello from a compiled program
```

That program is the whole cig for this machine with the checked script
appended, so it carries its own runtime: there is no `cig` to install for
it to start. `./hello --help` lists the run flags cig gives every
program (`--dry-run`, `--no-rollback`, `--unburn`, and so on), and
`./hello --cig-program` says what it was compiled from:

```
hello  a CigScript program
  compiled from  hello.cig  (sha256 de4c85156932095faef8c6bb02e3d49759b4f41f0a6bb71f41d94396ebcb49e8)
  compiled by    cigcompile 0.5.0
  runs on        cig 1.24.0 (this binary)
  format         2, stage a
  built          opt 1 for x86_64-unknown-linux-musl
  runtime        sha256 957ddff2c27c9baa38b5aa43361e165e209ebb3d6a3c82cfa28d7b7777e91b3f
  sections
    source           39 bytes         39 decoded  stored   required  sha256 de4c85156932095faef8c6bb02e3d49759b4f41f0a6bb71f41d94396ebcb49e8
```

## 4. Check the program does what the script does

`cigcompile test` builds the program and compares its `--dry-run` with
the script's, step by step: the same plan, the same output, the same
exit. At stage A this holds by construction (the program *is* the
interpreter), so `test` on your own scripts is a fast assurance that a
build didn't go wrong in between:

```
$ cigcompile test hello.cig
up to date: hello (16.3 MB)
  sha256 445143184b39c480f0934ce7339909d61fed286ed7a7f1ff0d6dab4285406af0
same     hello.cig (dry run: the same plan, output and exit)
```

If any difference turned up, `test` would exit `1` and name the first
line that differed. [Troubleshooting](troubleshooting.md) says what to
try next.

## 5. Build for another machine

`--target` builds for any machine cig is released for, from any
machine. `cigcompile targets` lists them:

```
$ cigcompile targets
x86_64-unknown-linux-musl        linux-x86_64     stage A  run on its own runner                        this machine; runtime /home/you/.cigscript/cigcompile/runtimes/1.24.0/linux-x86_64/cig
aarch64-unknown-linux-musl       linux-aarch64    stage A  run on its own runner
armv7-unknown-linux-musleabihf   linux-arm        stage A  built, not run
aarch64-apple-darwin             macos-aarch64    stage A  run on its own runner  no box (E777, E763)
x86_64-apple-darwin              macos-x86_64     stage A  run on its own runner  no box (E777, E763)
universal2-apple-darwin          macos-universal  stage A  run on both Macs       no box (E777, E763)
x86_64-pc-windows-msvc           windows-x86_64   stage A  run on its own runner  no box (E777, E763)
aarch64-pc-windows-msvc          windows-aarch64  stage A  run on its own runner  no box (E777, E763)
x86_64-unknown-freebsd           freebsd-x86_64   stage A  built, not run         no box (E777, E763)
```

Each row says the stage, how CI proves its programs, and what a program
there lacks (the box is Linux's). To build a hello for Apple Silicon:

```
$ cigcompile build hello.cig -o bin-mac/hello --target aarch64-apple-darwin
fetching cig-v1.24.0-macos-aarch64 from otmof-ops/cig-script (sha256 15b92e1549a7...)
kept /home/you/.cigscript/cigcompile/runtimes/1.24.0/macos-aarch64/cig (sha256 matches the pin)
compiled hello.cig -> bin-mac/hello (10.5 MB, cig 1.24.0 for aarch64-apple-darwin, stage a)
  sha256 7a85c97fc18489313398c507a50f508928ea4b4bb578eeaf56202730ca289cb6
```

The runtime for another machine is cig's own release asset for it, of
the version this cigcompile pins; it is accepted only at the sha256
this compiler was built knowing (`src/digests.rs`). A tampered release
would not match, and `CIGCOMPILE_OFFLINE=1` forbids the fetch
altogether.

`cigcompile fetch --all` fetches every machine's runtime ahead of
time, for a build that will run offline later.

## 6. Verify what came out

`cigcompile verify` checks a program is what a build makes, part by
part, and runs nothing: the runtime against cig's release pin, the
record, each section against the sha256 the section table carries for
it, and the padding and the trailer against what a build writes.

```
$ cigcompile verify hello --script hello.cig
hello
  runtime          ok        cig's release asset cig-v1.24.0-linux-x86_64 (sha256 957ddff2c27c9baa)
  record           ok        hello: hello.cig for cig 1.24.0, built by cigcompile 0.5.0, stage a, -O1, x86_64-unknown-linux-musl
  section source   ok        hello.cig is the script it was built from (1 lines, sha256 de4c85156932095f)
  section table    ok        1 sections: source
  padding          ok        zeros, as the alignment needs
  trailer          ok        CIGPROG2, the head sha256 holds
```

Each part is `ok`, `DIFFERS` or `unchecked`. Any `DIFFERS` and the exit
is `1`; the first part that differs is named. See
[shipping a program](shipping.md) for signatures,
[commands](commands.md) for the full surface, and
[how it works](how-it-works.md) for what each part is.

## Next

- [Commands](commands.md): every flag, every exit code, every error.
- [Shipping a program](shipping.md): signatures, SBOMs, the hardening
  report, and the packages cigcompile can write.
- [Troubleshooting](troubleshooting.md): keyed by the error code a
  message prints.
