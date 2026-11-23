# Stage `c`, first numbers (2026-11-23)

<!-- SPDX-License-Identifier: CC-BY-4.0 -->

What native code does for the functions stage `c` compiles today, on
three small programs whose work is inside such functions, beside the
same programs in Lua, LuaJIT and CPython. It is the first measurement
of stage `c`, made on the change that adds it, and it is narrow on
purpose: it says how fast the compiled functions are, and how little of
a real script they are yet.

**What it shows.** A function that works on numbers alone runs 8 to 22
times faster at stage `c` than at stage `b`, which is 3 to 7 times
faster than Lua 5.4 and between 0.6 and 1.8 times LuaJIT's time with its
compiler on. Decision
[0005](decisions/0005-what-goes-native-first.md)'s test for this step
was to at least halve stage `b`'s time on the code it covers.

**What it does not show.** Of the benchmark set's 29 programs only `fib`
has a function that compiles, and `--stage c` builds the others as stage
`b` programs: 19 keep all their code at the top level, which is not
compiled yet, and the rest work on lists, maps, strings or closures, or
divide an int by an int. cigo's script has 1,038 functions and none
compiles (the first one in the way is called with a string). Stage `c`
changes nothing for those until the top level, and then strings, lists
and maps ([#35](https://github.com/otmof-ops/cig-compile/issues/35)),
compile too.

## The programs

In [`bench/results/2026-11-23-stage-c/`](../bench/results/2026-11-23-stage-c/),
each in CigScript, Lua and Python:

| program | what it does | functions compiled |
|---|---|---|
| `fib` | `fib(35)`, the recursion of the benchmark set's `fib` with a larger argument | 1 of 1 |
| `mandel-fn` | counts the points of a 400 by 400 grid that stay in the Mandelbrot set for 100 turns: float arithmetic in a `while`, called from two `for`s | 2 of 2 |
| `churn` | sums `(i * i) % 7` for three million `i`: int arithmetic in a `for` | 1 of 1 |

## The times

Whole processes, start-up included, in milliseconds: the median of 9
runs, with the least and the most.

| median, ms | `fib` | `mandel-fn` | `churn` |
|---|---:|---:|---:|
| stage a | 3,205 (3,171 to 3,291) | 1,402 (1,383 to 1,435) | 210 (208 to 214) |
| stage b | 1,513 (1,474 to 1,741) | 226 (218 to 233) | 43.8 (43.0 to 50.7) |
| stage c, CIG_NATIVE=0 | 1,491 (1,453 to 1,640) | 224 (214 to 230) | 43.2 (42.8 to 50.2) |
| stage c | 68.1 (57.9 to 76.7) | 12.7 (12.5 to 12.9) | 5.6 (5.5 to 6.5) |
| Lua | 325 (319 to 337) | 85.6 (82.2 to 88.4) | 16.1 (16.0 to 16.2) |
| LuaJIT -joff | 282 (280 to 288) | 56.8 (56.6 to 57.6) | 14.5 (14.3 to 15.7) |
| LuaJIT | 38.2 (37.5 to 44.8) | 21.7 (21.6 to 21.8) | 3.9 (3.7 to 4.3) |
| CPython | 577 (570 to 597) | 332 (324 to 350) | 113 (110 to 116) |

| stage c's time ÷ theirs | `fib` | `mandel-fn` | `churn` |
|---|---:|---:|---:|
| stage a | 0.021 | 0.009 | 0.027 |
| stage b | 0.045 | 0.056 | 0.128 |
| stage c, CIG_NATIVE=0 | 0.046 | 0.057 | 0.130 |
| Lua | 0.209 | 0.148 | 0.350 |
| LuaJIT -joff | 0.241 | 0.223 | 0.386 |
| LuaJIT | 1.781 | 0.585 | 1.429 |
| CPython | 0.118 | 0.038 | 0.050 |

- **Against stage `b`:** 0.045, 0.056 and 0.128 of its time: 22, 18
  and 8 times faster. `churn` is the smallest gain because its whole
  run is 5.6 ms, and a program's start is part of that.
- **A stage `c` program without its native code** (`CIG_NATIVE=0`) runs
  at stage `b`'s speed: carrying the section costs nothing measurable.
- **Against LuaJIT with its compiler on:** `mandel-fn` takes 0.585 of
  its time; `fib` 1.78 times and `churn` 1.43 times its time.
- **The same counting as the interpreter.** The native code counts a
  step where the tree-walker does and checks every `+`, `-` and `*` for
  overflow, so these times are for code that stops at the same step
  budget and raises the same errors as stage `a`.

## How it was measured

- **Builds.** cigcompile from this change, release profile. The runtime
  is cig 1.25.0's source at the commit this compiler pins (the first
  with the VM's call into native code), built here in the release
  profile and linked against glibc: not cig's static release, which has
  no stage `c` yet. Each program was built with
  `cigcompile build <name>.cig --stage a|b|c` and run with
  `--max-steps 0` (`fib(35)` takes more than the default's ten million
  steps; the steps are still counted).
- **The others.** Lua 5.4.9, LuaJIT 2.1 (rolling, 2.1.1794386141) with
  and without `-joff`, and CPython 3.12.3.
- **The runs.** `measure.py` in that folder: the eight rows of a program
  one after another, nine times over, each run's output checked against
  the expected value, timed from before the process is started to after
  it is reaped. `times.json` holds every sample, and `report.py` makes
  the two tables above from it.
- **The machine.** One laptop: Intel Core Ultra 7 255H, 16 cores, Linux
  6.17 on x86-64, the `powersave` governor, on mains power, with a
  desktop session running. One machine and nine runs is a first reading,
  not a benchmark run of the kind
  [BENCHMARKS-2026-11-15.md](BENCHMARKS-2026-11-15.md) reports.

## Run it again

```sh
cargo build --release
d=$(mktemp -d)
for p in fib mandel-fn churn; do
  for s in a b c; do
    CIGCOMPILE_RUNTIME=target/release/cig-runtime target/release/cigcompile \
      build bench/results/2026-11-23-stage-c/$p.cig --stage $s -o $d/$p-$s
  done
done
python3 bench/results/2026-11-23-stage-c/measure.py $d
python3 bench/results/2026-11-23-stage-c/report.py $d/times.json
```

`LUA`, `LUAJIT` and `PYTHON` name the other implementations
(`lua5.4`, `luajit` and `python3` when they are not set).
