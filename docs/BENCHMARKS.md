# Benchmarks

`cigcompile bench` runs the set below interpreted (`cig run`) and compiled, from fresh folders, and says how the stage that exists meets the budgets in #14. The set and the harness are #30; the targets each later stage has to reach are in #42.

## How a benchmark is measured

1. **Output first.** Each implementation runs once, and its output is compared with the benchmark's `.expect` file. One that differs is WRONG, gets no time, and fails the run: a fast wrong answer is a bug, not a result.
2. **Samples.** Each side then runs until it has `--runs` samples (10 by default), or until it has three and has run for three seconds.
3. **Statistics.** The report gives the median, the 10th and 90th percentiles, the coefficient of variation, and the peak resident set (from `wait4`, on Unix). A side whose CV is over 5% is marked `~`, noisy. **The peak is the program's own.** Linux folds the parent's peak into a child's at `exec`, so a run started by the bench reports at least the bench's own peak: before 0.4.1 every row did (the bench held the runtime it had read, 15.8 MB for cig 1.21.0, and CI's table read 24.7 to 25.0 MB on cig 1.20.0's runtime and 28.3 to 28.4 MB on 1.21.0's, the same figure for every program that uses less than the bench). Now the bench lets go of the runtime's bytes before it runs anything, and takes each side's peak from one more run that is not timed: on Linux that run forks (sharing no file pages with the bench) and sets the child's mark back to what it has resident (`5` to `/proc/self/clear_refs`) before `exec`; elsewhere it is that run's `wait4`. The timed runs are started as before. `a_benchmarks_peak_memory_is_its_own_not_the_benchs` holds it. A program just written by `cigcompile build` can map its code in large page-cache folios on a kernel that has them (Linux 6.17 on ext4: hello 10.9 MB just built, 5.4 MB once the file is rewritten); both sides of a gate run just-built programs, so they compare like with like.
4. **Conditions.** Every report starts with the CPU, cores, kernel, CPU governor, the cig and cigcompile versions, `CIG_SEAL_KEY` and the runtime's sha256.
5. **References.** `--ref` runs the Python, Node and Rust versions in `bench/ref/` too, where `python3`, `node` and `rustc` are installed (`rustc -O`), and gives cig's ratio to each. Each reference is held to the same `.expect` file.

`--json` is for tools (it carries every raw sample, which the gates read) and `--markdown` for a pull request comment. `--set DIR` measures a folder of `<name>.cig` and `<name>.expect` instead of the set. CI runs the set on five machines on every pull request; a WRONG output there fails the build. A nightly run (`.github/workflows/bench.yml`) adds the references, posts the table to its summary and keeps the trend ([the gates](#the-gates)).

## The set

| benchmark | stresses | references |
|---|---|---|
| `hello` | start-up | Python, Node, Rust |
| `loop` | integer arithmetic in a counted loop (400k) | Python, Node, Rust |
| `fib` | calls and returns: `fib(25)`, 243k calls | Python, Node, Rust |
| `strings` | string building | |
| `maps` | map reads and writes | |
| `lists` | list building and reads | |
| `json` | JSON encode and decode | |
| `csv` | CSV parsing | |
| `files` | walking a tree of files | |
| `burn-big` | a 1,000-write burn and its journal | |
| `nbody` | float arithmetic, map fields, 20k steps | Python, Node, Rust |
| `trees` | allocation and recursion, depth 12 | Python, Node |
| `spectral` | nested float loops and calls, n = 100 | Python, Node |
| `mandel` | a float loop with an early exit, 160 × 160 | Python, Node, Rust |
| `fannkuch` | list index reads, writes and swaps, n = 8 | Python, Node |
| `closures` | closures made and called, 100k | Python, Node |
| `dispatch` | lambdas kept in a map, called by key, 150k | Python, Node |
| `sieve` | a 300k-element list of booleans | Python, Node, Rust |
| `words` | split, map counting, string compare | Python, Node |
| `regex-ish` | string find, replace and split over a megabyte of text (5 MB at 5x) | Python, Node |
| `json-deep` | a six-level nested document, parse and walk | Python, Node |
| `strbuild` | `s += piece` 10,000 times (50,000 at 5x), the quadratic trap a compiler turns into a builder | Python, Node, Rust |
| `richards` | a priority scheduler of tasks, mixed maps and control flow | Python, Node |
| `deltablue` | a constraint chain, method-heavy | Python, Node |
| `burn-mixed` | 40 burns of five writes each (200 at 5x): burn entry and exit, not just ops | |
| `orchestrate` | cig-orchestrator's own `orchestrate.cig`, 12.5k lines, run with `help` | |
| `startup-0` | start-up, no stdlib module touched | Python, Node, Rust |
| `startup-1` | start-up, one stdlib module | Python, Node |
| `startup-10` | start-up, ten stdlib modules | Python, Node |

The first ten are I/O, the journal and the standard library, where a compiler must not regress. The next nine are the classic compute benchmarks that steer stages B to D; the last ten are what the "still to add" set in #30 names (#45 and this issue): bulk string work, deep JSON, the quadratic strbuild trap, the Richards and DeltaBlue classics, a many-burn stress, the biggest real CigScript program, and start-up with zero, one and ten stdlib modules touched.

Every report names the references it ran: each of Python, Node and Rust with its version, or `not installed`. A tool that is not there leaves its column empty (a dash): a row is never made up.

### Allocation counts: `--alloc`

`cargo build --release --bin cig-runtime --features alloc-count` makes a bench-only runtime whose global allocator counts every allocation and the bytes asked for (a `realloc` counts as one allocation of its new size), and writes the two numbers to `CIG_ALLOC_FILE` as the process exits. `cigcompile bench --alloc <that runtime>` runs each benchmark once interpreted under it, checks the output is still right, and adds an `allocs` column and an `allocs` object (`count`, `bytes`) to `--json`. The count does not vary from run to run, so one run is enough, and it is the number the memory gate trusts. A release build of the runtime has no counter.

### The 1x and 5x pass: compute time apart from start-up

A benchmark marked `scaled` reads `args[0]` as a size and the harness runs it at two sizes (1 and 5): the slope between the medians, `(ms5 - ms1) / 4`, is reported as **compute ms** beside **wall ms**. A 1.6 ms process start then does not flatter Rust or hide a compiler's gains on small inputs. The slope reuses the existing `Stats` summary (no new statistics code), and a benchmark that does not declare `<name>.expect5x` is unchanged.

The six benchmarks that take a size argument are `regex-ish`, `json-deep`, `strbuild`, `richards`, `deltablue` and `burn-mixed`. `orchestrate` and the `startup-*` benchmarks have no natural size: they are timed at 1x only.

Two sizes differ from #30's table, on purpose. `strbuild` appends 10,000 times, not 100,000: the copy is quadratic at stage A, so 100,000 (and 500,000 at 5x) would take seconds to minutes a sample; the 1x to 5x slope shows the trap as plainly. `burn-mixed` has no hops: a hop is a process start, which `files`, `burn-big` and cig-script's own sets time, and it would hide the burn's own entry and exit.

### `orchestrate`: the biggest real program

`orchestrate` is cig-orchestrator's own `orchestrate.cig`, the copy the differential suite already carries in `tests/corpus/cig-orchestrator/` (12.5k lines), run with `help`: the whole program is read, checked and started, and one command answers. Its `.expect` is that copy's help text. It times what a large program pays to start, interpreted and compiled; `cigcompile bench --orchestrate <file>` still times a full build of any copy.

## Stage A, measured

`cigcompile bench --ref --runs 5`, cig 1.15.0, on an Intel Core Ultra 7 255H (16 cores, Linux 6.17, governor powersave), Python 3.12, Node 18 and Rust stable; medians in ms:

| benchmark | interpreted | compiled | Python | Node | Rust | cig ÷ Python |
|---|---:|---:|---:|---:|---:|---:|
| `hello` | 9.8 | 8.9 | 24.0 | 150.6 | 1.8 | 0.4× |
| `loop` | 209.7 | 196.7 | 93.9 | 189.7 | 2.8 | 2.2× |
| `fib` | 166.1 | 158.7 | 40.2 | 162.7 | 2.3 | 4.1× |
| `nbody` | 1196.4 | 1175.2 | 177.8 | 186.4 | 4.2 | 6.7× |
| `trees` | 752.5 | 761.4 | 162.9 | 201.2 | | 4.6× |
| `spectral` | 508.0 | 512.5 | 113.8 | 192.4 | | 4.5× |
| `mandel` | 901.4 | 931.0 | 253.2 | 177.7 | 6.4 | 3.6× |
| `fannkuch` | 818.2 | 804.2 | 142.5 | 185.6 | | 5.7× |
| `closures` | 155.1 | 144.0 | 57.8 | 176.7 | | 2.7× |
| `dispatch` | 219.6 | 222.1 | 66.1 | 197.0 | | 3.3× |
| `sieve` | 448.1 | 442.1 | 113.3 | 186.8 | 3.3 | 4.0× |
| `words` | 116.8 | 103.8 | 46.5 | 193.3 | | 2.5× |

The tree-walking interpreter is 4.0× slower than CPython at the median of the eleven. It starts faster than both Python and Node. Compiled and interpreted are the same speed, as stage A must be: a program is the interpreter. (Node 18's start-up on this machine, about 150 ms, makes the Node column mostly start-up; #30's numbers were taken with Node 22.)

## The gates

Both sides of a gate are `--json` results; nothing in a gate runs a program.

| gate | runs | trips when | exit |
|---|---|---|---|
| regression, per benchmark (`--against`) | every pull request, the fast set | a non-noisy benchmark's median is more than **5%** slower than `main`'s and a one-sided Mann-Whitney test of the samples says it is real (p under 0.05) | 1 |
| regression, the set (`--against`) | every pull request | the geometric mean of the ratios, over the non-noisy rows, is more than **2%** slower and the same test over all their samples (each divided by its row's `main` median) says it is real | 1 |
| memory (`--against`) | every pull request | a benchmark's peak resident set, or its allocation count where both sides have one, is more than **10%** over `main`'s (and over 1 MiB, or 1,000 allocations: a few KB on a tiny program is not a leak) | 1 |
| trend (`--trend`) | nightly | the set's geometric mean is more than 0.5% slower than the night before, **five nights in a row** | 1 |

A benchmark whose CV is over 5% on either side is **noisy**: it is in the table, marked `noisy, not gated`, and never fails a run. A slowdown the test cannot tell from the samples' own spread is `not significant`, and not gated. A side with fewer than five samples is gated on its medians alone.

**Interleaved.** `bench-gate.yml` builds `main` and the pull request, runs the fast set on each in turn, three rounds (main, pull request, main, pull request, ...), and pools each side's rounds (`--against` and `--current` repeat). A runner that warms up, throttles or has a noisy neighbor does it under both. The table (Markdown) goes to the job summary and to the pull request. While `main` has no `--against` of its own, the job says there is nothing to compare with and does not pass by default.

**Trend.** `bench.yml` runs nightly: the set against its references, then `bench --json` into `history/<date>.json`, joined with the last run's history, kept as the `bench-history` artifact (thirty nights, 90 days), and `bench --trend history` fails the run at five slower nights. The step is 0.5% and the length five because #30 says "five nightlies in a row each slower" and a night-to-night difference under the harness's own noise is the same night. The nightly also runs `references_agree_with_the_expect_files`.

### Compiled peak memory: the budget

The memory gate holds a pull request to `main`; the budget holds a compiled program to a number (CigCompile 1.0 row 6). `cigcompile bench` reports it with the other budgets, and `--budgets` fails the run when it is missed:

| what | budget | held | measured |
|---|---|---|---|
| peak memory of a compiled hello, real run, its file on a memory file system | under 14 MB | Linux, from the first cig after 1.21.0 | 7 to 8 MB on cig 1.22.0; with the file where it was built: 5.9 MB once the file is cold, 11.5 MB just built on Linux 6.17, 18.9 MB just built on Linux 7.0 with ext4 (the whole 16.2 MB file is resident); 17.5 MB on cig 1.21.0 as released, whose heap was in 2 MiB pages |

**What the figure is.** A resident set counts the pages of the program's file that are mapped as well as what the program allocates, and how much of the file is mapped is the kernel's choice: it maps the page cache's folios, and for a file just written some file systems keep large ones. On Linux 7.0 with ext4 a 16.2 MB program is resident whole, so every small program peaks at its file's size plus its own two or three megabytes, on any runtime, and a number meant to catch a heap that grew would only measure the file system. For the budget's hello the bench therefore takes the peak once more from a copy of the program on a memory file system (`/dev/shm`, else the session's runtime folder), where the pages a run touches are mapped and no others; the row says so beside the figure. Where there is no such folder, or a file there cannot be executed, the figure is the one from where the program was built. The `RSS MB` column and the memory gate are unchanged: they are the resident set as the system counts it, compared with `main` measured the same way.

On cig 1.21.0 as released the row is shown and reads `not yet`: a hello peaks at 14 to 18 MB there, by the machine, and no change in the compiler can bring it under. The runtime's own share is held in cig-script (`tests/heap_pages.rs`: a dry-run hello under 9 MiB, and the heap's first 8 MiB in 4 KiB pages). Data-heavy programs are held by the memory gate, against `main`.

### Tests

| test | what it holds |
|---|---|
| `every_benchmark_has_an_expect_file` | every benchmark has its output, and each reference a benchmark |
| `a_wrong_output_fails_before_timing` | a benchmark that prints something else gets no time and fails the run |
| `a_noisy_benchmark_is_not_gated` | a 25% slowdown on a benchmark with a CV over 5% is said, not failed |
| `the_regression_gate_trips_at_five_percent` | a planted 8% slowdown in one of nineteen quiet benchmarks fails, 3% does not |
| `the_geomean_gate_trips_at_two_percent` | every benchmark 3% slower fails the set, 1% does not |
| `the_regression_gate_stays_quiet_on_noise` | 200 seeds of the same program measured twice: no failure |
| `the_memory_gate_trips_over_ten_percent` | peak RSS and allocations |
| `a_benchmarks_peak_memory_is_its_own_not_the_benchs` | a row's peak is the program's, not the bench's (Linux) |
| `a_compiled_hello_over_its_memory_budget_is_missed` | the compiled hello's memory budget |
| `the_regression_gate_trips_from_the_command_line` | `--against` and `--current` exit codes, pooling, a file that is not a result |
| `the_nightly_trend_fails_after_five_slower_nights` | five fail, four do not, a faster night ends the run |
| `the_markdown_report_renders` | the pull request table |
| `references_agree_with_the_expect_files` | Python, Node and Rust print the `.expect` output (nightly; `--ignored`) |

The gate tests plant their slowdowns in saved results, because at stage A both sides of a pull request are the pinned cig and no program gets faster or slower; stage B (#31) is what gives the gates a real change to see.

## On the family's harness: `bench/bench.toml`

`cigcompile bench` is a front end of `cig bench` ([docs/BENCH.md](https://github.com/otmof-ops/cig-script/blob/main/docs/BENCH.md) in cig-script), the one benchmark tool of the family. This repository's manifest, `bench/bench.toml`, measures what only a compiler has:

| group | cases | measures |
|---|---|---|
| `build` | `build` of hello and of the whole set, a rebuild with nothing changed, `-O0`, `-O1`, `-O2` | wall, rss, and with `--timings` the phases |
| `programs` | each of 21 programs of the set, interpreted (`cig run`) and compiled | wall, rss, and with `--syscalls` the calls |
| `start-up` | a compiled hello, and the interpreter's | wall |
| `size` | the file a build writes, for hello and nbody | `file:` bytes, reported, not gated (#14's 4 MB waits on stage B) |
| `artefacts` | `--sbom`, `--sign`, `--emit oci`, `--emit deb`, `verify`, `audit` | wall |
| `passes` | each pass alone on a generated 10,000-line script | wall |

```sh
cigcompile bench --manifest bench/bench.toml                  # every case
cigcompile bench --manifest bench/bench.toml --group programs # one group
cigcompile bench --group programs --syscalls                  # where the calls went
cigcompile bench --timings                                    # the build's phases
cigcompile bench --watch                                      # rebuild and rerun on a change
cigcompile bench --against origin/main                        # A/B against a ref, a release or a binary
```

Asking for any of `--watch`, `--timings`, `--stats`, `--syscalls`, `--manifest`, `--group`, or `--against` with a ref, a release or a binary (not a saved `.json`) runs the manifest through `cig bench`, with its statistics, its A/B and its live mode. `--runs` is its `--samples`, and `--only`, `--markdown` and `--json` are its own. **The flags the set harness had keep working, on the set harness, unchanged:** `--runs`, `--ref`, `--markdown`, `--json`, `--set`, `--runtime`, `--budgets`, `--orchestrate`, `--alloc`, `--only`, and `--against` with `--current` or `--trend` (the regression gate on saved results). `--cig PATH` names a cig that has `cig bench`; by default it is the runtime programs are built on. A cig without `bench` is said, with the flags that need no cig, and exits 3.

**The set is the pinned cig's.** The 21 programs and their expected output are cig's `bench/compute`; `bench/set.sh` copies them from `$CIGSCRIPT_SRC/bench/compute` (the manifest sets it to `../../CigScript`, where CI checks cig out) after holding every file to its sha256 in `bench/set.sha256`. A file that differs, or is missing, is a refusal naming it (exit 3). The `bench/*.cig` files the set harness reads are unchanged for now.

**Both sides run one runtime.** The front end puts the runtime first on the `PATH` as `cig`, so `cig run x.cig` runs the bytes a program built here carries; `bench/prep-program.sh` builds each program on that `cig` and refuses unless `cigcompile verify --runtime` finds exactly those bytes at its front. A gap between a program and its script is then the compiler's work and nothing else.

**The allocator is named.** `--syscalls` runs one extra run of each case under `strace`. A compiled-against-interpreted pair where one side maps and unmaps at least ten times as much as the other (and 100 pairs more) is reported as `allocator, not the compiler`: both sides run one runtime, so the program is not what is faster (cig-script#153's nbody: 400,507 mmap/munmap calls against 78). It is a sentence, never a failure.

**Phases.** A build writes the phases `--timings` prints (read, index, check, pin, runtime, assemble, hash, write, sbom) to the descriptor `CIG_TIMINGS_FD` names, as v1 lines (`{"v":1,"product":"cigcompile","phase":"check","ms":0.5}`), summed over the scripts. The descriptor alone is enough; with neither it nor `--timings` nothing is written. Time outside those phases (loading the runtime, mostly) is the reader's `other`.

**Not in the manifest yet:** `build --watch` reacting to one change (a case cannot wait on a process that never ends: use `cigcompile bench --watch`), the build of `orchestrate.cig` (`--orchestrate` keeps it), a program refusing with E1050 to E1055, the IR's size before and after a pass, and the 4 MB gate. The set harness is the default for a bare `cigcompile bench` until a cig with `cig bench` is the pin.

The nightly run is `.github/workflows/bench-nightly.yml`, on cig-script's reusable workflow, with the results on the `bench-history` branch. Timings are reported, never gated.

| test | what it holds |
|---|---|
| `cigcompile_bench_flags_still_work` | every flag of today's `--help` is still there and runs the set harness, none of them reaching `cig` |
| `the_new_flags_are_handed_to_cig_bench` | the new flags reach `cig bench` as written, with the runtime first on the `PATH` |
| `set_comes_from_the_pinned_cig` | the manifest's 21 programs are in `set.sha256`; the pinned cig's files match it; a changed byte or a missing file is refused |
| `same_runtime_for_both_sides` | the program carries the sha256 of the runtime `cig` is; another runtime is not accepted |
| `allocator_gap_is_named` | a planted mmap thrash on one side is reported as the allocator's, and a quiet pair is not |
| `build_phases_on_the_fd` | `CIG_TIMINGS_FD` gets the rows `--timings` prints, with the same times |

## What the later stages must reach

The full table is in #30 and #42. In short:

| target | stage B | stage C | stage D |
|---|---|---|---|
| set median against the tree-walker | at least 3× | at least 15× | at least 40× on typed code |
| against CPython, set median | parity | 3× faster | |
| `hello` start-up, compiled | 5 ms | 3 ms | 2 ms |
| `hello` size | 4 MB | 4 MB | 3 MB |

At stage A a pull request cannot change how fast a program runs: both sides are the pinned cig. So the regression gate (a benchmark 5% slower than `main`) is proven on planted slowdowns now, and sees a real one from stage B (#31), where the compiler makes the code that runs.
