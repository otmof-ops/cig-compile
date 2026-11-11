<!-- SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile) -->
<!-- SPDX-License-Identifier: CC-BY-4.0 -->

# 0003: Pins choose the compiler through the installer, and a trial checks a new one

- Status: accepted
- Date: 2026-11-11
- Issue: otmof-ops/cig-compile#15
- Related: cig-installer decision 0001 (sections 3 and 5)

## Context

A project written for CigarScript 1.0 should keep building with the CigCompile that
shipped with 1.0 after 1.1 is installed, and a project should be able to see what a newer
CigCompile changes before it trusts it. The issue's first wording had cigcompile read the
pin and re-execute the pinned version. The family rule since then is that install, update,
pins and dispatch go through the installer, which already reads pin files, verifies what it
fetches and records what was tested together.

## Decision

1. **A cigar pin names a CigarScript version; the carton maps it to a compiler.**
   `cigar = "1.0"` at the top of `cigar.toml`, or a `.cigar-version`, is a pin on cigar. The
   installer selects the carton that holds a cigar meeting the pin and also holds
   `cigcompile`, and runs that carton's cigcompile. There is no table from cigcompile
   versions to cigar versions and no second version scheme: the carton manifest already says
   which builds were tested together. A `carton` key in the same `cigar.toml` wins, as a
   carton pin always does. When no installed carton fits, the whole carton is fetched under
   the `pin_fetch` policy (never a stick of cigar beside the machine's cigcompile, which
   would build with the wrong compiler) and refused with the command to run when fetching is
   off.
2. **The installer picks; cigcompile does not.** cigcompile never reads a pin and never runs
   another cigcompile. It reports its version and does what it is asked.
3. **`cigcompile trial <new>` is the one exception that lives here**, because it needs two
   compilers at once. `<new>` is a carton, as `ciginstaller pin carton` takes it. The trial
   asks the installer where that carton's cigcompile is (`CIGI_CARTON=<new> ciginstaller
   which cigcompile --path`), never fetching or verifying anything itself, and runs both
   compilers as child processes in scratch copies of the project. It compares the output
   of `test`, and the plan of every program run with `--dry-run`. A folder given with
   `--differential` is built and really run by both, and compared, only when the codegen
   flags (`--target`, `-O`, `--passes`, `--strength`, `--emit`) read the same on both sides;
   otherwise it says which one differs and skips. Differences print first. The trial never
   changes a pin; it ends with the command that would (`ciginstaller pin carton <new>`).
   Exit codes: `0` the same, `1` differences, `3` it could not run, `127` no installer.

## Options considered

| Option | Why not |
|---|---|
| cigcompile reads the pin and re-executes the pinned version | Duplicates the pin dialect and the verification the installer owns, so two programs would decide which build runs and check it. |
| A table of cigcompile versions per cigar version inside cigcompile | It drifts from the carton manifest, which is the record of what was tested together. |
| A stick of cigar next to the machine's cigcompile when the pin is not met | Builds with the newer compiler while looking pinned. |

## Consequences

- A cigar pin for a version no carton holds is refused rather than approximated.
- Until a carton holds both a released cigar and cigcompile, the behavior is tested with
  fixture cartons only.
- `trial` copies the project (never `bin`, `.git`, `target` or links) and builds there, so
  the project's tree is not written to; a `--differential` corpus really runs its scripts.
