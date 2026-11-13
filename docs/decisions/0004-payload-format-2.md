<!-- SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile) -->
<!-- SPDX-License-Identifier: CC-BY-4.0 -->

# 0004: Payload format 2, one container for bytecode, archives, own flags and provenance

- Status: accepted
- Date: 2026-11-13
- Issues: otmof-ops/cig-compile#31 (stage B), #71 (archives), #8 (own flags), #40 (provenance and the SBOM)
- Related: CigScript `docs/PROGRAMS.md` (format 1), CigScript `docs/BYTECODE.md` (the module), otmof-ops/cig-script#185 (the checked-tree archive)

## Context

A program today is the runtime with payload format 1 appended: one JSON record that holds the
script, then a 48-byte trailer (`CIGPROG1`, the payload's length, the payload's sha256). At
start the runtime hashes the whole payload and parses the whole script, so start-up grows with
the script (compiled cigo's `--version` is 66 ms against hello's 2.2 ms, #71).

Four pieces of work each change that payload:

| issue | wants in the payload |
|---|---|
| #31 stage B | the bytecode module with its own hash, the optimization level, the source kept for errors, `--cig-program` and `--interp` (the `stage` key is this record's, for #31's `--stage`) |
| #71 archives | named sections (`tree`, `snap`, the source compressed) with hashes computed at build, so start-up is flat in the script's size |
| #8 own flags | `own_flags`, so a program takes every argument without `--` (`pack { own_flags: true }`) |
| #40 trust | the target, the level and the runtime's sha256 in the record; the SBOM in the program |

Each could bump the format on its own. This record fixes one format that holds all four, and the
order in which they land on it.

Facts the design rests on, from the code as it is:

- The runtime runs a program only when the record's `cig` equals its own version (`E1051`).
  The writer is the cigscript library at the version cigcompile pins, so a runtime always knows
  every section its own writer emits. Unknown sections matter only to tools that read programs
  of other versions (`cigcompile verify`) and to programs moved between runtimes.
- A program knows it is one even with its trailer gone: its record starts at the runtime's
  image end with `{"format":` (#135). Any new layout must keep that, or a cut program could run
  as a general `cig`.
- An old runtime that meets a trailer it does not know falls back to that check; when the bytes
  at its image end start with `{"format":` it refuses the file as damaged (`E1050`), and when
  they do not, it runs as `cig`.
- A bytecode module carries its own 40-byte header: `CIGB`, a format version, and the BLAKE3-256
  of its body, which `parse_module` checks before `verify_module` and `vm::load`.
- The VM's spans are byte offsets into the source, so an error quoted with its line needs the
  source. `--chains` reads the source too.
- The VM does not yet run burns, compensations, `pack` or `CHAIN.MAKE` (`vm::NOT_BUILT`).
- Bytes appended after an executable's image are not mapped by the system loader. Zero-copy
  access to a section means the runtime maps its own file.

## Decision

### 1. The byte layout

A format 2 payload starts where the runtime's image ends (the payload start, `P`) and runs to the
end of the file, apart from the zero padding or PE certificate table format 1 already allows
after the trailer. All integers are little-endian. Offsets are from `P`.

```text
... cig ...                       the runtime, as released; its image ends at P
record                            JSON, starts with {"format":2, ...
zero padding                      to the first body's alignment
section bodies                    in table order, each starting at a file offset that is a multiple of 16
section table                     count x 64 bytes
trailer                           56 bytes, "CIGPROG2" first
```

**The trailer (56 bytes):**

| offset | size | field | value |
|---:|---:|---|---|
| 0 | 8 | magic | ASCII `CIGPROG2` |
| 8 | 8 | payload length (u64) | bytes from `P` to the trailer's first byte |
| 16 | 4 | record length (u32) | the record's bytes, from `P`; at most 65,536 |
| 20 | 2 | section count (u16) | 1 to 32 |
| 22 | 2 | flags (u16) | `0`; a reader refuses any other value (`E1050`) |
| 24 | 32 | head sha256 | sha256 of the record's bytes followed by the section table's bytes |

The table sits directly before the trailer, so its offset is `payload length - 64 x count`. The
record sits at `P`. Nothing else needs an offset.

**A section table entry (64 bytes):**

| offset | size | field | value |
|---:|---:|---|---|
| 0 | 8 | id | lower-case ASCII, zero-padded: `source`, `bytecode`, `tree`, `snap`, `sbom` |
| 8 | 2 | version (u16) | the body's own format: the bytecode's `FORMAT_VERSION`, the archive's `archive_format`, `1` for source and SBOM |
| 10 | 1 | encoding (u8) | `0` stored as is; `1` raw DEFLATE (RFC 1951) |
| 11 | 1 | flags (u8) | bit 0: required (a reader that does not know this id refuses the program); other bits `0` |
| 12 | 4 | decoded length (u32) | the body's length after decoding; equals `length` for encoding `0` |
| 16 | 8 | offset (u64) | from `P` |
| 24 | 8 | length (u64) | stored bytes |
| 32 | 32 | sha256 | of the stored bytes, computed at build |

Rules a reader checks on the table, each refused with `E1050`: ids are known characters and not
repeated; entries are in the fixed order `source`, `bytecode`, `tree`, `snap`, `sbom` (unknown
ids after them, in byte order of the id); bodies lie between the record's end and the table's
start, in table order, without overlapping; each body starts at a file offset that is a multiple
of 16; the bytes between bodies are zero; an encoding is `0` or `1`, and the entry's other flag
bits are zero; the payload is at most 64 MiB and a decoded body at most 64 MiB.

**The record** is one JSON object written without whitespace, its keys in this order, every
string printable (the format 1 rules for `name`, `file`, `compiler` and `cig` carry over):

| key | type | meaning | needed by |
|---|---|---|---|
| `format` | `2` | first, so a cut program is still recognized | all |
| `name` | string | the program's name | all |
| `file` | string | the script's file name as compiled; errors point here | all |
| `sha256` | hex | sha256 of the script's text, decoded; the same meaning as in format 1, so `cig runs` and `--unburn`'s ownership check (`E1053`) work across formats | all |
| `cig` | string | the cig version the program runs on | all |
| `compiler` | string | e.g. `cigcompile 0.4.0` | all |
| `stage` | `"a"` or `"b"` | which body runs: `a` the source on the tree-walker, `b` the bytecode on the VM | #31 |
| `opt` | string | the level it was built at: `"0"`, `"1"`, `"2"` (later `"3"`, `"s"`, #33) | #31, #40 |
| `target` | string | the target triple, at most 64 characters | #40 |
| `runtime_sha256` | hex | sha256 of the bytes before `P`: the runtime as it was appended to, which for a release asset is the asset's published sha256 | #40 |
| `own_flags` | `true` | present only when set: every argument is the program's, and the run flags come from `CIG_RUN_FLAGS` | #8 |

Readers ignore record keys they do not know, so a later optional key (a program's own step
budget, for one) does not need a format bump. A key that changes how a program runs is safe to
add the same way, because a program only runs on the cig that wrote it.

The record holds no section list: the table is the list, and `--cig-program` prints it (id,
bytes, encoding, sha256) beside the record.

### 2. The sections

| section | contents | stage `a` | stage `b` | read at start |
|---|---|---|---|---|
| `source` | the script's text; DEFLATE when the text is 4 KiB or more, stored as is below that | required | kept (see 3) | stage `a`, `--interp`, an error that quotes its line, `--chains` |
| `bytecode` | the module as `serialize_module` writes it, its `CIGB` header included | absent | required | always, at stage `b` without `--interp` |
| `tree` | the checked tree as cig's archive (cig-script#185) | optional | absent | when the tree-walker runs |
| `snap` | the top level's constant data, archived | optional, only with `tree` | absent | when the tree-walker runs |
| `sbom` | CycloneDX 1.5 JSON, DEFLATE | present in every program | the same | only for `--cig-program --sbom` |

- Every section the writer emits today is marked required except `sbom`, which a program can run
  without.
- A section a reader does not know: required, refused with `E1050` (`the program needs section
  <id>, which this cig does not read`); not required, skipped. Its sha256 is still covered by
  the head and still checked by `cigcompile verify`.
- A stage whose required section is missing is refused with `E1050`.
- `tree` and `snap` are the tree-walker's. A stage `b` program runs on the VM and does not carry
  them; a program whose script does not lower completely is stage `a` and may carry them.
- The SBOM lists the runtime (asset, version, `runtime_sha256`), its crates with versions and
  licenses, the compiler, and the script's sha256. The crate list is read from the runtime's own
  bytes, where cig writes it at build time beside its version mark, never from cigcompile's copy
  of the library, which is built under another lock file. The embedded document names the
  program by its record (its name, the script's sha256, `runtime_sha256`): it cannot hold the
  sha256 of the file that holds it. `build --sbom` keeps writing a sidecar beside the program:
  the embedded document plus the program file's own sha256, which only a file outside the
  program can carry.

### 3. Compatibility

- **A format 1 program on a runtime that reads format 2** reads, checks and runs as it does
  today: the reader looks for `CIGPROG1` (48 bytes) and `CIGPROG2` (56 bytes) at the end, after
  any zero padding or PE certificate table. Format 1 is read and run for as long as programs
  exist, since the code is small and `cigcompile verify` needs it for old programs.
  Format 1 is stage `a` with no own flags. A format 1 program for another cig is still `E1051`.
- **A format 2 program on a runtime that reads only format 1** (cig 1.15.1 through 1.21.x) is
  refused, never run as `cig`: the old reader does not know `CIGPROG2`, finds `{"format":` at
  its image end and stops with `E1050` ("its trailer is not at its end"). The wording is the
  damaged-program one, not "format 2". Making old runtimes say more would need a patch release of
  each, and such a pairing only arises when a payload is moved onto another cig, which is
  `E1051`'s case anyway, so the record accepts the wording.
- **A format 2 program for another cig** is `E1051`, before any section is read: the record is
  read and the `cig` key checked first (see 5).
- **The writer.** From the cig release that carries the reader, the writer makes format 2 for
  every program, stage `a` included, so that own flags, provenance and the SBOM are in every
  program from the same release. The library's format 1 writer stays one release so the
  differential suite can show that both formats run the same; there is no user-facing switch
  back to format 1. (#31's line "`--stage a` still makes format 1 programs" becomes "`--stage
  a` makes a stage `a` program in format 2".)
- **The source stays in a stage `b` program**: for `--interp`, for error text that quotes the
  line, and for `--chains` (`--explain <code>` needs no source: it reads the registry). It costs the
  deflated script, about 0.3x its size (#71's budget); for hello, under 100 bytes.

### 4. How a stage `b` program starts

1. Find the trailer, as in 5. Read the record and the table, check the head sha256.
2. Read the record; refuse raised rights (`E1054`); answer cig's internal calls; check `cig`
   (`E1051`).
3. Check the table's rules and the stage's required sections (`E1050`).
4. Read the command line: the run flags first, or, with `own_flags`, every argument the
   program's and the run flags from `CIG_RUN_FLAGS`. `--help`, `--explain` and `--cig-program`
   answer here and read no body.
5. Without `--interp`: read the `bytecode` body, `parse_module` (its magic, version and BLAKE3),
   `verify_module`, `vm::load` (builtins, prelude names, burn-op kinds), then run through the
   same kernel entry as the tree-walker. Any failure in this step is `E1050` with the rule and
   `fn<i>+<pc>`, as `docs/BYTECODE.md` words it.
6. With `--interp` (a run flag, and a word in `CIG_RUN_FLAGS`): read the `source` body, check its
   sha256 and the record's `sha256`, run it on the tree-walker. On a stage `a` program
   `--interp` changes nothing.

**No silent fallback.** A program whose bytecode is damaged, fails to verify or fails to load
refuses with `E1050`; it never runs its source instead. A fallback would hide a damaged program
and change both its speed and what the differential compares. The source runs only when asked,
with `--interp`.

**Which stage a build makes.** With the VM available, a build makes stage `b` when the whole
script lowers and verifies, and contains no instruction the pinned runtime's VM lists as not
built; otherwise it makes stage `a` and says so in a note. `--stage b` refuses a script that
cannot be stage `b` with a new cigcompile code (the next free one, `E1010`), naming the first
construct the lowering does not cover. `--stage a` always makes stage `a`.

**Errors while running.** A diagnostic quotes its line from the `source` body, which is read and
hashed the first time a diagnostic needs it. If it does not match, the program's own error is
still reported, without the quoted line and with a note that the source section is damaged
(`E1050`); the exit code stays the program's error's.

### 5. Integrity: what is hashed, and when

| what | at build | at every start | when used | `cigcompile verify` |
|---|---|---|---|---|
| record and table | head sha256 written into the trailer | sha256 of at most 66 KiB: **constant in the script's size** | | checked, named `record` or `section table` |
| `bytecode` | sha256 into the table; BLAKE3 in the module's own header | | BLAKE3 in `parse_module`, then verify and load (already linear in the module, within #31's 5 ms load budget for 12,500 lines) | sha256, parse and verify; with `--script`, re-lowered at the recorded `opt` and compared |
| `source` | sha256 into the table; the text's sha256 into the record | | sha256 when decoded (stage `a`, `--interp`, a quoted error, `--chains`) | sha256, and against `--script` |
| `tree`, `snap` | sha256 into the table; validated by the writer | | validated before use | sha256 and full validation |
| `sbom` | sha256 into the table | | sha256 on `--cig-program --sbom` | sha256 |
| runtime | `runtime_sha256` into the record | not hashed, as in format 1 | | against the record and against the pinned release digests, named `runtime` |
| padding, trailer | zeros; the trailer's fields | lengths and bounds checked | | the whole file against a rebuild, the first difference named by part |

- Start-up does no pass over the whole payload. Each body costs what using it costs, and a body
  the run does not use is never read.
- `CIG_VALIDATE=1` makes a start hash and validate every section before running, for tests.
- `verify` names the part that differs: `runtime`, `record`, `section table`, `section <id>`,
  `padding` or `trailer`. Format 1 programs keep today's names: `runtime`, `payload`, `script`
  and `whole`.
- `E1050` covers every integrity failure (head, table, a body, the bytecode's verifier and
  loader); `E1051` a program for another cig. The registry's `E1050` text gains "a section's
  bytes do not match the table, or the table does not read".
- As in format 1 the hashes are integrity, not authenticity: they catch damage, not an edit.
  Authenticity is signing (#40) and program passes.

### 6. Size

Estimated from the layouts above (the first package measures and records the real numbers):

| part | hello, stage `b` |
|---|---:|
| record (with `stage`, `opt`, `target`, `runtime_sha256`) | about 330 bytes |
| `source` body, stored as is | about 50 bytes |
| `bytecode` body (header 40, one function of about five instructions, one line row) | about 160 bytes |
| padding | at most 45 bytes |
| table, two entries (`source`, `bytecode`) | 128 bytes |
| trailer | 56 bytes |
| **without the SBOM** | **about 0.75 KB, against the 488 bytes decision 0001 measured in format 1** |
| `sbom` body, deflated from at most 20 KB (#40's budget), and its table entry | about 5 KB |
| **with the SBOM** | **about 6 KB** |

Against the roadmap's hello budgets of 4 MB and then 2 MB, format 2 is 0.15% and 0.3%. The
runtime is the size; the payload is not. For large scripts the budgets are #31's (payload at
most 3x the source, with the source now at about 0.3x of it) and #71's (`tree` and `snap` at most
4x the source).

### 7. The order of landing

The compiler pins a released cig, and its `vm` feature stays off until a release carries the
VM. So every part lands in CigScript first, ships in a cig release, and is then picked up by
cigcompile after it repins.

| step | repository | part | can it land before the next cig release? | tests it owes |
|---|---|---|---|---|
| 0 | cig-compile | this record | yes | none |
| 1 | cig-script | the container: reader for both formats, the format 2 writer with the `source` body, the record's keys (`stage`, `opt`, `target`, `runtime_sha256`, `own_flags`), the table rules, `CIG_VALIDATE`, `--cig-program` listing sections, `PROGRAMS.md`, the registry (`E1050` text, `E1010`) | it lands on main and ships in the next release | `archive_tamper_refused` (on `source` and a test-only section), plus `format_1_programs_still_run`, `an_unknown_required_section_is_refused`, `an_unknown_optional_section_is_skipped`, `the_head_is_all_a_start_hashes`, `the_same_inputs_give_the_same_bytes` |
| 2 | cig-script | own flags (#8): `pack { own_flags: true }`, the record key, `CIG_RUN_FLAGS` | same release as step 1 | `a_program_with_own_flags_gets_every_argument`, `run_flags_come_from_the_environment`, `an_old_program_keeps_its_bytes` (a format 1 fixture still runs) |
| 3 | cig-script | the SBOM's runtime half (#40): the crate list in the runtime's bytes, `--cig-program --sbom` | same release if ready, else the next; the container needs no change | `the_sbom_lists_the_runtime_crates`, the 20 KB budget |
| 4 | cig-script | stage `b` at run time (#31): the `bytecode` body, the start in 4, `--interp` | in the release that carries the VM; the container already reserves the section | `a_program_with_damaged_bytecode_is_e1050`, `interp_fallback_gives_the_same_bytes` |
| 5 | cig-compile | after the repin: the writer fills `stage`, `opt`, `target`, `runtime_sha256` and `own_flags` (`--args direct` is the build-side name for the same bit); the `sbom` section from `sbom.rs`, the sidecar as that document plus the program's own sha256; `verify` by section | no: it needs the release with steps 1 to 3 | `verify_detects_a_changed_byte` (runtime, record, table, each section, trailer), `args_direct`, `the_sbom_lists_the_runtime_crates` |
| 6 | cig-compile | after the repin to the VM release: `vm` on by default, `--stage a\|b`, the `bytecode` body at the recorded `opt` | no: it needs the release with step 4 | `differential_stage_b_is_identical`, `the_step_budget_stops_on_the_same_line`, `the_budget_table_holds` |
| 7 | both | `tree` and `snap` (#71, after cig-script#185's archive) | in a later release; additive, no format change | `programs_start_without_parsing`, `startup_flat_in_script_size`, `archive_tamper_refused` (on `tree`), `same_archive_as_cig`, `snapshot_restores_constants`, `impure_top_level_runs_live` |

Steps 1 and 2 touch the same two files (`src/program.rs`, `src/cli/compiled.rs`; step 2 the
parser too, for `own_flags`); they land one
after the other, step 1 first. Before a release exists, cigcompile's side of steps 5 and 6 can
be written and tested only against a CigScript branch outside main; nothing on cigcompile's main
depends on an unreleased cig.

### 8. Set aside

| option | why not |
|---|---|
| The bytecode as base64 inside the JSON record (#31's first sketch) | It adds a third to the bytecode, makes every start parse the whole record, and gives no aligned body for an archive. |
| Native object sections (ELF sections, a Mach-O segment, PE sections) written by rewriting the runtime's headers | It needs a header rewriter per format; it belongs to signing (#40), which can move this container into a section unchanged, since its offsets are relative to `P`. |
| A section table inside the JSON record | Tools would need a JSON parser to find a body, and the table's fixed entries are simpler to bound-check. |
| zstd for the source and the SBOM | The runtime links zstd for the tin, but a tool that reads a program from outside (a verifier or an auditor in another language) is likelier to have DEFLATE at hand, and on a few KB of text the gain is small. The encoding field leaves room: zstd can become encoding `2` later without a format change. |
| Falling back to the source when the bytecode does not load | It hides a damaged program; `--interp` is the only way the source runs at stage `b`. |
| Mixed programs (bytecode for some functions, the tree for the rest) | The VM can call interpreted functions, but the format would need a map from functions to tree nodes; revisit if the VM's coverage of burns and chains lags behind the rest. |
| Patch releases of older cigs that read format 2 | A program only runs on the cig that wrote it, so the only gain is a better wording of a refusal. |
| A `--format 1` build flag | Nothing needs format 1 programs once the writer and the runtime move together. |

## Consequences

- One format change, in one cig release; own flags, provenance, the SBOM, stage `b` and the
  archives all add keys or sections to it without another bump.
- Start-up's fixed cost becomes constant in the script's size; what remains linear is the work
  of using a body, which #71's archive and #31's VM then shrink.
- Every program's bytes change once, at the release that carries the container, as they do at
  any cig release.
- `verify` reads both formats and names parts by section.

## Questions settled with this record

1. **`tree` and `snap` at start: validated before use, every start.** It is safe without
   `unsafe` and memory-bound, and linear in the archive (on the order of a millisecond for a few
   MB), measured by `startup_flat_in_script_size`. Trusting the build-time hash and validating
   only under `CIG_VALIDATE=1` would read a damaged byte unchecked; it may be looked at again for
   signed programs, whose pages the system has checked.
2. **A stage `b` program always keeps its source** (about 0.3x the script). A build flag that
   leaves it out would lose `--interp`, quoted lines and `--chains`; it waits until someone asks
   for closed distribution.
3. **The format 1 writer stays in the library for one release, for the differential only.** There
   is no user-facing `--format 1`.
4. **The table's hash is sha256,** as format 1, `verify`, the release digests and the SBOM use.
   A start never hashes a large body with it, so BLAKE3's speed does not decide it.
5. **The SBOM is in every program** (about 5 KB deflated), as #40 asks; `build --sbom` still
   writes the sidecar.
