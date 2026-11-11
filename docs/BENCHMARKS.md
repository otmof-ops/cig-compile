# Benchmarks

`cigcompile bench` runs the set below interpreted (`cig run`) and compiled, from fresh folders, and says how the stage that exists meets the budgets in #14. The set and the harness are #30; the targets each later stage has to reach are in #42.

## How a benchmark is measured

1. **Output first.** Each implementation runs once, and its output is compared with the benchmark's `.expect` file. One that differs is WRONG, gets no time, and fails the run: a fast wrong answer is a bug, not a result.
2. **Samples.** Each side then runs until it has `--runs` samples (10 by default), or until it has three and has run for three seconds.
3. **Statistics.** The report gives the median, the 10th and 90th percentiles, the coefficient of variation, and the peak resident set (from `wait4`, on Unix). A side whose CV is over 5% is marked `~`, noisy.
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
| `the_regression_gate_trips_from_the_command_line` | `--against` and `--current` exit codes, pooling, a file that is not a result |
| `the_nightly_trend_fails_after_five_slower_nights` | five fail, four do not, a faster night ends the run |
| `the_markdown_report_renders` | the pull request table |
| `references_agree_with_the_expect_files` | Python, Node and Rust print the `.expect` output (nightly; `--ignored`) |

The gate tests plant their slowdowns in saved results, because at stage A both sides of a pull request are the pinned cig and no program gets faster or slower; stage B (#31) is what gives the gates a real change to see.

## What the later stages must reach

The full table is in #30 and #42. In short:

| target | stage B | stage C | stage D |
|---|---|---|---|
| set median against the tree-walker | at least 3× | at least 15× | at least 40× on typed code |
| against CPython, set median | parity | 3× faster | |
| `hello` start-up, compiled | 5 ms | 3 ms | 2 ms |
| `hello` size | 4 MB | 4 MB | 3 MB |

At stage A a pull request cannot change how fast a program runs: both sides are the pinned cig. So the regression gate (a benchmark 5% slower than `main`) is proven on planted slowdowns now, and sees a real one from stage B (#31), where the compiler makes the code that runs.
