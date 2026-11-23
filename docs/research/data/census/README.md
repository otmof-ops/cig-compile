# A census of what cig's VM executes (cig 1.25.0)

The studies beside this folder keep asking the same things about cig itself: how settled are the types at a site, how many shapes does a field site see, how many callees a call site, how often is a function called with one row of argument types, how much of a run is bookkeeping. Nobody had counted. This folder is the count.

**Every figure here is a count of executed instructions. Nothing is timed.** A count is the same on any machine, so this needed no quiet box.

## What was run

- **The instrument:** [census.patch](census.patch), a diff against cig-script at the tag `v1.25.0` (`173aed5`). It adds `src/vm/census.rs` and a hook beside each of the four places the VM's own `bench-stats` counters already sit. It is built only with the `bench-stats` feature, which a release never has, and is on only when `CIG_VM_CENSUS` names a file. **It reads registers and changes nothing a script can see.** It is a research instrument and is not proposed for merging.
- **What it records,** for every instruction that completes, at its site (function index, pc), read before the instruction runs:
  - how often the site ran;
  - for arithmetic and comparisons, the pair of operand types;
  - for a field read or write, the map's **shape** (its keys in order, for a map of up to 16 keys; a larger map is a dictionary and its shape is its size class) and the position of the key in the map;
  - for an index, what was indexed by what;
  - for a call, which function was called and the row of its arguments' types; for a builtin and a method, its name and the receiver's type;
  - what was made: lists, maps, closures, interpolated strings.

  A site keeps up to 8 distinct observations and counts the rest, so "more than 8" is exact as a statement and its members are not listed. A function keeps up to 16 rows of argument types.
- **The programs:** the 25 programs of cig-script's `bench/compute` (21 compute programs, `hello` and three start-up programs) and `bench/engines/lines-500.cig`, each run once with `cig run --engine vm`; and **cigo's own script** (`orchestrate.cig` at cig-orchestrator `e0c21a7`: 1,038 functions, 115,268 instructions as lowered) for seven of the cases cigo's `bench/engines.toml` times, each a fresh process in a scratch project and home: `--version`, `--help`, `list` over 52 tasks, `audit` of one task, the agent hook on a benign command and on `curl | sh`, and a guard line check.
- **How:** [run.sh](run.sh) and [run-cigo.sh](run-cigo.sh), on a development machine (x86-64 Linux), 2026-11-23, in scratch folders with `CIG_SEAL_KEY=file`. The census build's sha256 was `eeb55c05…206f2a46`; the patch's is `ed03d3ef…ced21472`.

## How it was checked

- **Every program printed what it should.** The 26 scripts printed exactly their `.expect` files, and cigo's `list` and `audit` printed exactly cigo's fixtures ([verdicts.tsv](verdicts.tsv): exit code, output, engine). The other five cigo cases have no fixture and were held to exit code 0.
- **The VM ran every one** (a script the lowering declines runs on the tree-walker and leaves no census: none did).
- **The census counts what the VM counts.** For all 33 runs, the census's total equals the instruction total of the VM's own counter (`CIG_VM_STATS=1`).
- **The tables regenerate.** `python3 census.py <the runs' folder>` prints [report.md](report.md) from the per-site records a run writes. [summary.json](summary.json) is the same content as data. The per-site records themselves (33 files, 30 MB) are not kept in this repository: the commands at the end make them again, and a count is the same on any machine.

## What it found

The tables are in [report.md](report.md). The 21 compute programs executed 93,703,377 instructions in all.

### In the compute programs

1. **Step marks are a quarter of everything the VM executes.** `steps`, the instruction that charges the step budget, is 9.1% to 50.0% of a program's executed instructions, median 23.8%. It is the largest single group in three of them (`fannkuch`, `sieve`, `strbuild`). Any engine that keeps the budget exact pays for this, and it is the first thing a native tier has to make cheap.
2. **Types do not move.** Of 227 arithmetic and comparison sites, 226 saw one pair of operand types for the whole run (the other, in `trees`, saw two). 28,190,255 of 28,864,733 executions (97.7%) were at such a site. Arithmetic was 55.6% float with float, 42.2% integer with integer, 2.2% mixed, over 23,984,719 executions. In 15 of the 20 programs that do arithmetic, every arithmetic instruction ran on two integers.
3. **Every field site saw one shape.** All 74 `getfield` and `setfield` sites, 5,891,718 executions, saw one record shape and found their key at one position. Every index site (5,663,526 executions) saw one kind of container and index: a list by an integer, or a map by a string.
4. **Call sites call one function.** 28 of 29 `call` sites called one function for the whole run (the other is `dispatch`, written to call many). Every function was only ever called with one row of argument types. All 716,674 method calls were at a site with one receiver type.
5. **Half the moves copy a counted value.** A `mov` whose source is a string, list, map or function increments a reference count: the median program does that on 50.0% of its moves (0% to 100%).

These programs are benchmarks, written to do one thing in a loop. They say what a compute-bound script looks like, not what every script looks like. cigo's script is the other case.

### In cigo's script

6. **A cigo call is mostly its script's top level.** Every call runs the same 34,841 or so instructions outside any function, which declare the script's 1,038 functions and build its tables: 13,890 `loadk`, 8,795 `appendlist`, 4,517 `insertmap`, 2,323 `newlist`, 804 `newmap`, 662 `closure`. That is **94.3% of `cigo --version`** and 93.7% of `--help` (about 37,000 instructions each), and 42% to 52% of the five cases that do work (66,764 to 82,990). It makes 2,323 lists, 804 maps and 662 closures before the first line of the command runs, then the process ends and they are dropped.
7. **Nothing is hot.** A site runs 1.1 to 2.3 times on average. The ten busiest sites are 0.9% to 19.2% of a call. Between 13 and 80 of the 1,038 functions are entered, and 30.3% to 32.6% of the module's instructions run at all. A tier that waits for code to get hot has nothing to wait for here.
8. **Records come in two shapes.** 66% to 100% of field reads and writes were at a site that saw two shapes, and the key sat at one position for only 50% to 84% of them: the VM's cache cell, which remembers a position, misses the rest.
9. **Functions are called with more than one row of types.** In the two hook cases and the guard line check, 5% to 7% of calls went to a function that only ever saw one row of argument types; in `list` and `audit`, 72%. No function saw more than three rows.
10. **The work is in builtins and methods, not in arithmetic.** A hook call makes about 1,600 calls to script functions, 3,400 to builtins (`type_of` alone 3,149 times) and 1,600 method calls (`.push()` 1,282 times), against 175 arithmetic instructions. In the hook and guard cases, 96% to 99% of comparisons are string with string.
11. **Step marks are 1% to 13% of a cigo call,** against a quarter of a compute program: the top level is one long statement list, not a loop.

## What it says to the decisions

- **(b) inline caches.** A cache keyed on one shape or one type pair would hit at every site of every compute program. In the five cigo cases that do work it would hit on 20% to 34% of field accesses; a second entry would cover the rest. No site here saw more than two shapes, and no function more than three rows of argument types.
- **(c) counters.** A compute program concentrates: 4 to 91 sites make 90% of its instructions, so counting to a threshold finds them. In cigo it finds nothing. cigo's cost is start-up, and a profile kept between runs (which functions a command enters: 13 to 80 of 1,038) says more than a counter in one run.
- **(d) the step budget.** An exact budget costs a quarter of a compute program's instructions as the VM does it today. This is the figure a cheaper scheme has to beat.
- **(g) reference counts.** In the median compute program, half of the register moves copy a counted value. This is a floor: copies made by arguments, results and builtins are not in it.
- **(h) shapes.** Compute programs would be served by one shape a site; cigo by two.
- **(i) types.** In compute code a site's types are settled; inference or one observation gives the same answer. In cigo, functions see two or three rows.
- **Start-up (cig-compile#36, #71; cig-script#259).** The largest cost in cigo is building constant tables and closures at the top level on every call. A table whose elements are all constants is built by two instructions an element today.

## What it does not say

- **Time.** A `steps` and a `callb` of a regular expression are one instruction each. The shares are of instructions, not of milliseconds.
- **The tree-walker and compiled programs.** The census is in the VM only.
- **Other scripts.** 21 benchmarks and one real program. cigo is the only large cig script there is to measure.
- **All reference counting.** Only `mov` and `loadk` are looked at.
- **Rare kinds.** A site's ninth distinct observation and later are counted, not kept.

## Running it again

```sh
git -C <cig-script clone> worktree add --detach <scratch>/census v1.25.0
git -C <scratch>/census apply <this folder>/census.patch
cargo build --release --features bench-stats --manifest-path <scratch>/census/Cargo.toml
./run.sh <scratch>/census/target/release/cig <cig-script clone>/bench/compute <out>
./run-cigo.sh <scratch>/census/target/release/cig <cig-orchestrator clone> <out>
python3 census.py <out> --json <out>/summary.json > <out>/report.md
```

`run.sh` takes one folder of scripts: `lines-500.cig` and its `.expect` were copied in beside the compute programs for this run.
