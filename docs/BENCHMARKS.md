# Benchmarks

`cigcompile bench` runs the set below interpreted (`cig run`) and compiled, from fresh folders, and says how the stage that exists meets the budgets in #14. The set and the harness are #30; the targets each later stage has to reach are in #42.

## How a benchmark is measured

1. **Output first.** Each implementation runs once, and its output is compared with the benchmark's `.expect` file. One that differs is WRONG, gets no time, and fails the run: a fast wrong answer is a bug, not a result.
2. **Samples.** Each side then runs until it has `--runs` samples (10 by default), or until it has three and has run for three seconds.
3. **Statistics.** The report gives the median, the 10th and 90th percentiles, the coefficient of variation, and the peak resident set (from `wait4`, on Unix). A side whose CV is over 5% is marked `~`, noisy.
4. **Conditions.** Every report starts with the CPU, cores, kernel, CPU governor, the cig and cigcompile versions, `CIG_SEAL_KEY` and the runtime's sha256.
5. **References.** `--ref` runs the Python, Node and Rust versions in `bench/ref/` too, where `python3`, `node` and `rustc` are installed (`rustc -O`), and gives cig's ratio to each. Each reference is held to the same `.expect` file.

`--json` is for tools and `--markdown` for a pull request comment. `--set DIR` measures a folder of `<name>.cig` and `<name>.expect` instead of the set. CI runs the set on five machines on every pull request; a WRONG output there fails the build. A nightly run (`.github/workflows/bench.yml`) adds the references and posts the table to its summary.

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

## What the later stages must reach

The full table is in #30 and #42. In short:

| target | stage B | stage C | stage D |
|---|---|---|---|
| set median against the tree-walker | at least 3× | at least 15× | at least 40× on typed code |
| against CPython, set median | parity | 3× faster | |
| `hello` start-up, compiled | 5 ms | 3 ms | 2 ms |
| `hello` size | 4 MB | 4 MB | 3 MB |

At stage A a pull request cannot change how fast a program runs: both sides are the pinned cig. So the regression gate (a benchmark 5% slower than `main`) starts with stage B (#31), where the compiler makes the code that runs.
