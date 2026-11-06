# Security policy

## Supported versions

Fixes go into the newest release. Please reproduce against it before
reporting: `cigcompile --version` says which you have.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting on this repository (**Security
→ Report a vulnerability**). Do not open a public issue for a security
problem. Include `cigcompile --version`, `cig --version`, the platform, and
the script or command that shows the problem. You'll get an acknowledgement
within a few days and a fix or a mitigation before anything is disclosed.

## What cigcompile promises and what it does not

- **A script is code, and so is the program made from it.** A compiled program
  runs its script through the same run as `cig run`: the same burns, journal
  and rollback, as you, with your permissions. Read a script from someone else
  before you build it, as you would before you run it.
- **The compiler is the first reader of untrusted input.** Its parser and
  checker are cig's own, and the front end is fuzzed: a sample on every test
  run, and libFuzzer on a schedule. A script nested past the parser's limits
  is refused (E208), not a crash.
- **The runtimes it fetches are pinned.** The runtime for another machine is
  cig's own release asset, accepted only at the sha256 compiled into
  cigcompile (`src/digests.rs`). A checksum fetched beside the asset is never
  trusted.
- **A runtime given with `--runtime` is yours to vouch for.** cigcompile checks
  that it is cig of the pinned version and the target's kind of executable,
  not that it is cig's release build.
- **Programs are unsigned.** macOS code signatures and Authenticode do not
  cover bytes appended after signing, so a program runs unsigned on both.
