# Shipping a program

A program made by cigcompile is a single file. This page is how to ship
one: sign it, carry an SBOM with it, read the hardening it got from
cig's own build, write it as an OCI image or a `.deb`, write a Homebrew
formula or a Scoop manifest for a release, build it the same bytes on
every machine, and the license that travels with it.

## All-or-nothing staging

A build that writes more than one thing — `--sign`, `--sbom`, `--emit
oci`, `--emit deb`, or any of these together — writes to a staging
folder beside `--out` first. The outputs move into place only when every
step has succeeded. A failing sign or emit leaves `--out` exactly as it
was, including an earlier signed program and its `.sig` byte for byte.
The error names what is still at `--out`:

```
nothing was written: out/fib is still the program from before (signed)
```

A build that does not sign and replaces a program takes away the `.sig`
and the SBOM's `.sig` beside it, which signed the program from before. A
program and its signature always match on disk.

## Signatures (`--sign`)

`cigcompile build --sign <key>` writes a detached SSH signature beside
each program, `<program>.sig`, made with `ssh-keygen -Y sign` and the
namespace `cigcompile-program`. The namespace means that a signature
made here cannot be replayed as a git or a generic file signature, and
one made with another namespace fails to verify here.

- `<key>` is an SSH private-key file, or a public-key file with
  `ssh-agent` holding the private one.
- The signer is checked up front: `ssh-keygen` must be on this machine
  (install OpenSSH: `openssh-client` on Debian and Ubuntu, `openssh` on
  macOS and Windows 10+), and the key file must be readable. A
  pre-check that fails exits `E1004` and nothing is written.
- With `--sbom`, the SBOM is signed too (`<program>.cdx.json.sig`).

A sign that fails after a build has written the program leaves `--out`
alone: the staging folder is discarded and the message names what is
still there.

### Verifying a signature

```
cigcompile verify <program> --signature <program>.sig [--allowed-signers <file>] [--identity <id>]
```

- With `--allowed-signers <file>`: the signer must be in that file (SSH
  allowed-signers format). `--identity <id>` names which signer; the
  default is `*`, any identity there.
- Without `--allowed-signers`: only the signature's integrity is checked
  (`-Y check-novalidate`) and the part is reported `unchecked`.

The exit is `1` when any part of the verify differs, as with every
other `verify`.

### Apple Developer ID and Authenticode

macOS code signatures and Windows Authenticode do not cover bytes
appended after signing. A program is cig with a script appended, so the
runtime's signature does not cover the payload. These identities are an
operator's to hold, and so adding them is a separate step; it tracks in
cig-script#153.

## SBOMs (`--sbom`)

`cigcompile build --sbom` writes a CycloneDX 1.5 file beside each
program, `<program>.cdx.json`. It names:

- The program as one component, by name and sha256.
- The runtime (cig's release asset), by version and sha256.
- The script, by name and sha256.

There is no time and no build path in it, so the same build writes the
same file. With `--sign`, the SBOM is signed too.

## The hardening report (`cigcompile audit`)

`cigcompile audit <program>` reads what a program carries from its
headers. These are cig's build's: a program is cig with a script
appended.

- **ELF** (Linux, FreeBSD): PIE, RELRO (full with BIND_NOW, or static),
  the stack not executable, CET (IBT and shadow stack) on x86-64 or
  AArch64's BTI and PAC.
- **Mach-O** (macOS): PIE, the stack, a code signature; each slice of a
  universal binary.
- **PE** (Windows): ASLR, NX, Control Flow Guard, an Authenticode
  signature.

Example, Linux:

```
$ cigcompile audit hello
hello  (the runtime's own hardening: cig's build)
  ELF
    PIE        yes  position-independent: the loader places it at a random address
    RELRO      yes  full: read-only after relocation, every symbol bound at start (BIND_NOW)
    NX stack   yes  the stack is not executable
    CET        no   IBT no, shadow stack no
```

`audit` is a report. The exit is `0` whatever it finds; `--json` gives
each part as a field.

## OCI images (`--emit oci`)

`cigcompile build --emit oci --target <linux>` writes an OCI image
beside the program, `<program>.oci.tar`: a scratch image of one file,
the program itself, as the image's entrypoint. The targets are Linux
ones (a program needs nothing else).

```
$ cigcompile build hello.cig --emit oci --target x86_64-unknown-linux-musl
compiled hello.cig -> hello (11.4 MB, cig 1.18.0 for x86_64-unknown-linux-musl)
  sha256 ...
  image  hello.oci.tar (docker load -i, then run localhost/hello:latest)
```

```sh
docker load -i hello.oci.tar          # or: podman load -i hello.oci.tar
docker run --rm localhost/hello:latest
```

Every time in the image is zero and every owner root, so the same build
writes the same bytes.

## Debian packages (`--emit deb`)

`cigcompile build --emit deb --target <linux>` writes a `.deb` beside
the program that installs it to `/usr/bin/<name>`. The archive is pure
ar + tar + gzip, written here, so no dpkg is needed to make one.

```
$ cigcompile build hello.cig --emit deb --target x86_64-unknown-linux-musl \
    --pkg-version 1.0.0 --pkg-description "greeting program"
compiled hello.cig -> hello (11.4 MB, cig 1.18.0 for x86_64-unknown-linux-musl)
  sha256 ...
  deb    hello_1.0.0_amd64.deb (hello_1.0.0_amd64.deb, dpkg -i to install to /usr/bin/hello)
```

The control file's fields come from the flags:

| flag | field | default |
|---|---|---|
| `--pkg-version V` | Version | `0.0.0` |
| `--pkg-maintainer "Name <email>"` | Maintainer | `cigcompile <nobody@localhost>` |
| `--pkg-description TEXT` | Description | `<name>, a CigScript program compiled with cigcompile.` |

The architecture comes from the target: `amd64`, `arm64`, `armhf`, or
`i386`. The Package field is the program's name, sanitized to Debian's
rules.

Every time in the `.deb` is zero and every owner root, so the same
inputs write the same bytes. `dpkg-deb --info` and `dpkg-deb
--contents` look inside without installing.

## Homebrew and Scoop (`cigcompile package`)

`cigcompile package` writes a Homebrew formula (`--brew`, a Ruby class)
or a Scoop manifest (`--scoop`, a JSON object) for a released program,
pointing at its release assets by URL and sha256. Nothing is built: the
formula or the manifest installs what the release already published.

Inputs come either from a JSON `--manifest` or from flags on the
command line; a flag overrides the manifest's. The five fields are
`--name`, `--version`, `--homepage`, `--license` and `--description`.
Per-target assets come as `--asset TARGET=URL,SHA256`, repeatable.

```sh
cigcompile package --brew \
  --name hello --version 0.1.0 \
  --homepage https://example.com/hello \
  --license Apache-2.0 \
  --description "A greeting program" \
  --asset x86_64-apple-darwin=https://example.com/hello-macos-x86_64,<sha256> \
  --asset aarch64-apple-darwin=https://example.com/hello-macos-aarch64,<sha256>
```

```ruby
class Hello < Formula
  desc "A greeting program"
  homepage "https://example.com/hello"
  version "0.1.0"
  license "Apache-2.0"

  on_macos do
    on_arm do
      url "https://example.com/hello-macos-aarch64"
      sha256 "..."
    end
    on_intel do
      url "https://example.com/hello-macos-x86_64"
      sha256 "..."
    end
  end
  ...
end
```

For Scoop, name Windows assets instead:

```sh
cigcompile package --scoop ... \
  --asset x86_64-pc-windows-msvc=https://example.com/hello.exe,<sha256>
```

`-o PATH` writes to a file; by default the formula or the manifest goes
to stdout, ready to pipe to a tap or a bucket.

A JSON manifest reads the shape
`{name, version, homepage, license, description, assets: [{target, url, sha256}]}`;
a `--asset` on the command line replaces the manifest's entry for the
same target. The same inputs write the same bytes.

## Reproducible builds

The same script, compiler and runtime give the same bytes on any
machine. In practice:

- A program records its script's file name only, not the path it was
  built from, and no build time.
- The runtime for another machine is cig's release asset, so builds for
  it agree everywhere. Builds for this machine agree too when its `cig`
  was installed from the release.
- A `cig` built from source is still that version, with bytes of its
  own; the programs built onto it share those bytes.
- CI builds the same `hello.cig` on five machines and checks that the
  bytes are identical.
- SBOMs, `.deb` packages and OCI images are all written with zero
  timestamps, so the same build writes the same files.

The build index makes a rebuild that would write identical bytes a
no-op: no read of the runtime, no read of the program, about a
millisecond.

## The license a program carries

A compiled program is cig with the script appended. Cig itself is
Apache-2.0 (CigScript's LICENSE), so a program passed on to someone
else carries the Apache-2.0 obligations for that runtime:

- Keep a copy of the Apache-2.0 LICENSE and the NOTICE file with the
  program (CigScript's
  [NOTICE](https://github.com/otmof-ops/cig-script/blob/main/NOTICE),
  as section 4 asks of any copy of cig).
- The script itself stays under the license you give it: your code,
  your choice. The program carries your script's license alongside
  cig's.

The SBOM (`--sbom`) names cig's release asset by version and sha256,
so a downstream consumer can look up its own licence information from
CigScript's release page.

### Picking up Apache-2.0 for downstream

A compiled program that will be redistributed should ship with:

- **The program's own license** (your script's), named in the file the
  program is distributed under or in the release's `LICENSE`.
- **CigScript's LICENSE and NOTICE**, as above.
- **cigcompile's own LICENSE and NOTICE** (Apache-2.0), when
  cigcompile itself ships alongside the program — not required for a
  program made with it.

The `--sbom` output gives a machine-readable hand-off: the runtime's
component carries its sha256 and the release it came from, which is
enough for a downstream scan.
