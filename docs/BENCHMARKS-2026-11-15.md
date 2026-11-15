# Benchmarks, 2026-11-15: programs cigcompile builds, against `cig run`, Lua, LuaJIT, CPython, Node and Rust

<!-- SPDX-License-Identifier: CC-BY-4.0 -->

What a program cigcompile 0.5.0 builds runs at: the 25 programs of the benchmark set, built at stage `a` and at stage `b` on cig 1.24.0 as released, timed on one machine beside `cig run` and beside the same programs written in Lua, Python, JavaScript and Rust. This is the compiled side of a run CigScript's paper reports in full, [cig-script's `docs/BENCHMARKS-2026-11-15.md`](https://github.com/otmof-ops/cig-script/blob/main/docs/BENCHMARKS-2026-11-15.md): the programs in the other languages, the rule they are held to and the harness are there. This run's raw samples are in [`bench/results/2026-11-15/`](../bench/results/2026-11-15/).

Every measured number below is made from those samples by CigScript's `bench/compare/report.py` and made again by its `bench/compare/check.py`, a second program written apart from the first to the same definitions. The two agree on every number and every character of this file: none was typed by hand.

## In short

- **A stage `b` program takes 0.52 of `cig run`'s time** on the 21 compute programs, 1.91 times as fast, and is faster on all 21; on the start-up programs it takes 0.61 of it. Stage `b`'s target ([#31](https://github.com/otmof-ops/cig-compile/issues/31)) is a third of the interpreter's time: not reached yet.
- **Against CPython 3.13 it takes 0.82 of the time**, 1.22 times as fast (faster on 12 of 21, level on 2, slower on 7), and against Node 22 0.84, Node taking 27.2 ms just to start.
- **Lua is faster**: a stage `b` program takes 1.97 times Lua 5.4's time, 2.77 times LuaJIT's interpreter's and 5.20 times LuaJIT's with its compiler on.
- **A stage `a` program**, what a build makes by default, runs on the interpreter as `cig run` does: it takes 0.93 of `cig run`'s time.
- **A program starts in 3.7 ms** (`hello` at stage `b`; 3.7 ms at stage `a`), where `cig run` takes 5.8 ms, CPython 13.5 ms and Node 27.2 ms.
- **cig's start-up grows with its run history** ([cig-script#252](https://github.com/otmof-ops/cig-script/issues/252)), and so does a program's: `hello` at stage `b` took 2.8 ms over the first pass, while the history grew from 76 to 2356 runs, and 4.8 ms over the second (2736 to 5016).

## Against each language

A cell is the time on that row divided by the other language's: the geometric mean, over the programs both have, of the ratio of their median times. Under 1, the cig side took less time. In brackets: how many of those programs it was faster on, level with (within 3%) and slower on.

| time ÷ theirs | Lua | LuaJIT `-joff` | LuaJIT | CPython | Node | Rust |
|---|---:|---:|---:|---:|---:|---:|
| `cig run` | 4.03 (1/2/15) | 5.66 (1/0/17) | 10.6 (1/0/17) | 1.56 (4/0/17) | 1.65 (9/0/11) | 67.5 (0/0/6) |
| stage a | 3.77 (3/1/14) | 5.30 (2/1/15) | 9.95 (2/0/16) | 1.44 (4/1/16) | 1.53 (9/0/11) | 63.4 (0/0/6) |
| stage b | 1.97 (4/0/14) | 2.77 (3/0/15) | 5.20 (2/0/16) | 0.82 (12/2/7) | 0.84 (10/1/9) | 26.2 (0/0/6) |

- **`cig run`**: the script on cig's tree-walking interpreter.
- **A stage `a` program**: `cigcompile build`, the default; cig itself with the script inside, run by the same interpreter.
- **A stage `b` program**: `cigcompile build --stage b`; the script's bytecode, run on cig's VM.

The start-up programs:

| time ÷ theirs | Lua | LuaJIT `-joff` | LuaJIT | CPython | Node | Rust |
|---|---:|---:|---:|---:|---:|---:|
| `cig run` | 6.67 (0/0/3) | 6.17 (0/0/3) | 6.27 (0/0/3) | 0.38 (4/0/0) | 0.22 (4/0/0) | 6.84 (0/0/2) |
| stage a | 4.18 (0/0/3) | 3.87 (0/0/3) | 3.93 (0/0/3) | 0.24 (4/0/0) | 0.14 (4/0/0) | 4.29 (0/0/2) |
| stage b | 4.08 (0/0/3) | 3.78 (0/0/3) | 3.84 (0/0/3) | 0.23 (4/0/0) | 0.13 (4/0/0) | 4.26 (0/0/2) |

## Every program, on cig's sides

The median time of each program, in milliseconds, over 31 rounds in each of 2 passes, as a run and as a dry run (`--dry-run`, which runs the program and keeps no run record):

| program | `cig run` | `cig run --dry-run` | stage a | stage a, `--dry-run` | stage b | stage b, `--dry-run` |
|---|---:|---:|---:|---:|---:|---:|
| `closures` | 53.1 | 49.6 | 52.2 | 50.0 | 40.4 | 40.2 |
| `csv` | 63.3 | – | 61.4 | – | 55.6 | – |
| `deltablue` | 11.3 | 9.0 | 9.2 | 8.6 | 4.6 | 4.3 |
| `dispatch` | 78.4 | 75.6 | 76.2 | 76.8 | 35.7 | 35.9 |
| `fannkuch` | 327.8 | 331.3 | 322.0 | 316.9 | 76.3 | 75.3 |
| `fib` | 62.6 | 58.2 | 59.9 | 56.4 | 34.4 | 32.8 |
| `json` | 74.0 | 69.9 | 69.8 | 69.6 | 66.6 | 65.8 |
| `json-deep` | 8.4 | 6.1 | 6.1 | 5.9 | 5.4 | 5.1 |
| `lists` | 32.9 | 29.9 | 29.6 | 29.4 | 27.8 | 26.7 |
| `loop` | 64.8 | 61.6 | 62.0 | 60.8 | 17.6 | 17.4 |
| `mandel` | 374.5 | 354.9 | 357.3 | 362.2 | 71.8 | 70.7 |
| `maps` | 41.8 | 38.0 | 38.3 | 37.9 | 29.4 | 29.4 |
| `nbody` | 474.2 | 458.9 | 474.9 | 491.3 | 202.3 | 193.2 |
| `regex-ish` | 42.4 | 41.4 | 39.1 | 38.5 | 32.6 | 31.8 |
| `richards` | 25.2 | 22.4 | 23.2 | 22.7 | 12.1 | 11.5 |
| `sieve` | 164.6 | 171.8 | 163.9 | 161.1 | 69.7 | 67.1 |
| `spectral` | 216.6 | 210.3 | 212.8 | 200.2 | 76.6 | 78.3 |
| `strbuild` | 10.0 | 7.7 | 7.8 | 7.3 | 6.6 | 6.1 |
| `strings` | 25.2 | 22.4 | 22.9 | 22.5 | 17.4 | 16.9 |
| `trees` | 397.5 | 382.2 | 376.9 | 369.4 | 260.0 | 244.9 |
| `words` | 39.2 | 36.0 | 38.2 | 35.4 | 25.4 | 25.1 |
| `hello` | 5.8 | 3.5 | 3.7 | 3.3 | 3.7 | 3.2 |
| `startup-0` | 6.0 | 3.6 | 3.7 | 3.2 | 3.7 | 3.3 |
| `startup-1` | 6.1 | 3.6 | 3.8 | 3.3 | 3.6 | 3.3 |
| `startup-10` | 6.1 | 3.8 | 3.8 | 3.2 | 3.7 | 3.3 |

What the run record costs, over the programs that have both:

|  | programs | run ÷ dry run | run − dry run, median ms |
|---|---:|---:|---:|
| `cig run` | 24 | 1.17 | 2.8 |
| stage a | 24 | 1.05 | 0.5 |
| stage b | 24 | 1.04 | 0.4 |

## Memory

The peak resident memory of each program, in MB, read as it exits (the median of every reading, three in each pass). It counts the pages of the program's own file with its allocations: a compiled program carries the whole cig runtime in its file (about 16 MB), the same binary `cig run` is.

| program | `cig run` | stage b | Lua | LuaJIT `-joff` | LuaJIT | CPython | Node | Rust |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `closures` | 15.9 | 14.2 | 2.3 | 2.4 | 2.6 | 9.9 | 70.1 | – |
| `csv` | 35.9 | 31.1 | – | – | – | 23.7 | – | – |
| `deltablue` | 15.9 | 14.2 | 2.3 | 2.5 | 2.6 | 9.9 | 61.5 | – |
| `dispatch` | 15.9 | 14.2 | 2.4 | 2.5 | 2.6 | 9.9 | 69.4 | – |
| `fannkuch` | 16.9 | 14.2 | 2.4 | 2.5 | 2.6 | 10.0 | 70.5 | – |
| `fib` | 15.9 | 14.2 | 2.3 | 2.4 | 2.6 | 9.9 | 68.0 | 2.1 |
| `json` | 69.6 | 69.2 | – | – | – | 27.3 | 78.5 | – |
| `json-deep` | 17.0 | 14.3 | – | – | – | 12.0 | 61.7 | – |
| `lists` | 26.1 | 24.7 | 4.2 | 3.4 | 3.5 | 14.2 | 72.3 | – |
| `loop` | 15.9 | 14.3 | 2.3 | 2.4 | 2.6 | 9.8 | 69.2 | 2.1 |
| `mandel` | 15.9 | 14.2 | 2.5 | 2.5 | 2.7 | 9.9 | 70.0 | 2.1 |
| `maps` | 28.3 | 26.1 | 10.4 | 7.9 | 8.1 | 16.3 | 76.8 | – |
| `nbody` | 16.0 | 14.1 | 2.7 | 2.7 | 2.8 | 10.6 | 70.8 | 2.1 |
| `regex-ish` | 33.6 | 33.0 | 15.5 | 15.7 | 16.0 | 23.7 | 103.5 | – |
| `richards` | 15.9 | 14.1 | 2.3 | 2.4 | 2.6 | 10.0 | 68.3 | – |
| `sieve` | 51.3 | 47.6 | 10.6 | 6.4 | 6.8 | 12.1 | 86.0 | 2.3 |
| `spectral` | 15.9 | 14.2 | 2.7 | 2.6 | 2.8 | 10.2 | 70.6 | – |
| `strbuild` | 15.9 | 14.2 | 2.7 | 2.7 | 2.7 | 9.9 | 61.8 | 2.1 |
| `strings` | 24.9 | 26.7 | 9.7 | 7.6 | 7.9 | 14.7 | 77.1 | – |
| `trees` | 17.6 | 14.5 | 6.0 | 7.9 | 7.7 | 11.0 | 73.3 | – |
| `words` | 25.2 | 25.5 | 4.8 | 3.9 | 3.9 | 14.7 | 76.0 | – |
| `hello` | 15.9 | 14.2 | 2.2 | 2.4 | 2.5 | 9.9 | 61.1 | 2.1 |
| `startup-0` | 15.9 | 14.2 | 2.2 | 2.4 | 2.5 | 9.8 | 61.2 | 2.1 |
| `startup-1` | 15.9 | 14.2 | 2.2 | 2.4 | 2.5 | 9.9 | 61.1 | – |
| `startup-10` | 15.9 | 14.5 | – | – | – | 17.0 | 64.7 | – |

## cigcompile's own suite

`cigcompile bench`, cigcompile 0.5.0 as released, run between the two passes, as it prints it (its JSON, with every sample, is in the results folder). It builds its programs at stage `a`, the default, so its "compiled" column is a stage `a` program, and its budget line "compiled vs interpreted" is stage `a`'s; the stage `b` programs are in the tables above.

**cigcompile bench**: Intel(R) Core(TM) i5-8265U CPU @ 1.60GHz, 8 cores, linux 7.2.11+parrot7.1-amd64, governor powersave; cig 1.24.0, cigcompile 0.5.0, CIG_SEAL_KEY=file, runtime sha256 957ddff2c27c9baa

| benchmark | stresses | interpreted ms | compiled ms | compute ms | p10 to p90 | CV | RSS MB |
|---|---|---:|---:|---:|---|---:|---:|
| `hello` | start-up | 1.9 | 1.7 | - | 1.8 to 2.0 | 2.9% | 11.4 |
| `loop` | integer arithmetic in a counted loop | 58.3 | 58.1 | - | 58.1 to 58.4 | 0.2% | 11.5 |
| `fib` | calls and returns: fib(25) | 53.7 | 52.8 | - | 53.5 to 53.8 | 0.8% | 11.5 |
| `strings` | string building | 20.8 | 20.4 | - | 20.6 to 21.0 | 0.9% | 24.9 |
| `maps` | map reads and writes | 35.8 | 36.1 | - | 35.7 to 36.0 | 0.5% | 25.4 |
| `lists` | list building and reads | 27.4 | 31.4~ | - | 27.3 to 27.5 | 0.2% | 23.0 |
| `json` | JSON encode and decode | 67.9 | 67.9 | - | 67.4 to 68.7 | 1.0% | 69.7 |
| `csv` | CSV parsing | 41.4 | 41.4 | - | 41.1 to 41.8 | 0.8% | 35.9 |
| `files` | walking a tree of files | 14.7 | 14.3~ | - | 14.6 to 14.8 | 0.7% | 19.0 |
| `burn-big` | a 1,000-write burn and its journal | 52.9 | 52.8 | - | 52.9 to 53.1 | 0.3% | 23.8 |
| `nbody` | float arithmetic, map fields | 444.9 | 443.4 | - | 444.2 to 451.8 | 1.8% | 11.8 |
| `trees` | allocation and recursion | 361.1 | 361.1 | - | 359.9 to 366.2 | 0.8% | 13.5 |
| `spectral` | nested float loops and calls | 197.7 | 200.7 | - | 197.3 to 200.5 | 0.8% | 13.8 |
| `mandel` | a float loop with an early exit | 340.3 | 343.2 | - | 340.0 to 364.0 | 2.9% | 11.8 |
| `fannkuch` | list index reads, writes and swaps | 295.2 | 310.0 | - | 294.9 to 296.9 | 0.3% | 11.8 |
| `closures` | closures made and called | 46.7 | 47.6 | - | 46.6 to 46.8 | 0.2% | 13.8 |
| `dispatch` | lambdas kept in a map, called by key | 72.2 | 72.0 | - | 71.8 to 72.5 | 0.5% | 11.8 |
| `sieve` | a 300k-element list of booleans | 157.9 | 157.7~ | - | 157.6 to 158.3 | 0.2% | 48.3 |
| `words` | split, map counting, string compare | 34.1 | 33.4 | - | 34.0 to 34.2 | 0.2% | 24.9 |
| `regex-ish` | string find, replace and split | 37.3 | 37.0~ | 33.1 | 37.1 to 37.4 | 0.2% | 33.6 |
| `json-deep` | a six-level nested document, parse and walk | 5.4 | 4.7 | 1.1 | 5.3 to 5.5 | 1.5% | 13.1 |
| `strbuild` | s += piece, the quadratic trap | 7.0 | 6.1 | 16.9 | 6.9 to 7.0 | 0.8% | 12.1 |
| `richards` | a priority scheduler of tasks | 23.1 | 22.3 | 20.6 | 23.1 to 23.2 | 0.3% | 11.9 |
| `deltablue` | a constraint chain, method-heavy | 8.9 | 7.7 | 5.9 | 8.8 to 9.0 | 1.3% | 12.6 |
| `burn-mixed` | 200 small burns of five writes each at 5x | 15.3 | 14.5 | 11.9 | 15.1 to 15.3 | 0.6% | 15.8 |
| `orchestrate` | cig-orchestrator's own 12.5k-line orchestrate.cig: help | 46.5 | 44.2 | - | 46.0 to 46.6 | 1.5% | 49.2 |
| `startup-0` | start-up, no stdlib module touched | 3.1 | 2.2 | - | 3.0 to 3.2 | 2.7% | 12.2 |
| `startup-1` | start-up, one stdlib module | 3.2 | 2.2 | - | 3.0 to 3.2 | 3.0% | 12.2 |
| `startup-10` | start-up, ten stdlib modules | 3.6 | 2.4 | - | 3.4 to 3.7 | 3.2% | 13.1 |

| budget (#14) | target | measured | verdict |
|---|---|---:|---|
| incremental rebuild, nothing changed | under 50 ms | 1.8 ms | met |
| start-up of a compiled hello | under 5 ms | 1.7 ms | met |
| compiled vs interpreted, median | at least 3x (stage B) | 1.01x | not yet (a later stage) |
| hello binary size | under 4 MB (the trimmed runtime, #4) | 16.3 MB | not yet (a later stage) |
| peak memory of a compiled hello | under 14 MB (Linux) | 8.3 MB (2.6 MB of its own, 5.7 MB of its files' pages) | met |

## Conditions

|  | pass 1 | pass 2 |
|---|---|---|
| started (UTC) | 2026-11-15T16:21:07Z | 2026-11-15T16:34:06Z |
| processor | Intel(R) Core(TM) i5-8265U CPU @ 1.60GHz | Intel(R) Core(TM) i5-8265U CPU @ 1.60GHz |
| logical cores | 8 | 8 |
| highest clock, MHz | 3900 | 3900 |
| kernel | 7.2.11+parrot7.1-amd64 | 7.2.11+parrot7.1-amd64 |
| frequency driver | intel_pstate | intel_pstate |
| CPU governor | powersave | powersave |
| energy preference | balance_performance | balance_performance |
| turbo | on | on |
| power profile | balanced | balanced |
| transparent huge pages | [always] madvise never | [always] madvise never |
| on mains power | no | no |
| load average at the start | 0.68 0.39 0.17 | 1.01 1.07 0.74 |
| package °C at the start | 60 | 64 |
| cig's run records at the start | 0 | 2660 |
| harness | Python 3.13.5 | Python 3.13.5 |
| rounds | 31 | 31 |
| seed of the rounds' order | 1 | 2 |
| `true`, median ms | 0.633 | 0.813 |
| cig's run records at the last round | 2356 | 5016 |
| mean clock at a round's start, lowest, MHz | 787 | 738 |
| mean clock at a round's start, highest, MHz | 3357 | 3400 |
| package °C at a round's start, highest | 92 | 87 |
| cig | cig 1.24.0 | cig 1.24.0 |
| cigcompile | cigcompile 0.5.0 (makes programs for cig 1.24.0) | cigcompile 0.5.0 (makes programs for cig 1.24.0) |
| Lua | Lua 5.4.9  Copyright (C) 1994-2026 Lua.org, PUC-Rio | Lua 5.4.9  Copyright (C) 1994-2026 Lua.org, PUC-Rio |
| LuaJIT | LuaJIT 2.1.1794386141 -- Copyright (C) 2005-2026 Mike Pall. https://luajit.org/ | LuaJIT 2.1.1794386141 -- Copyright (C) 2005-2026 Mike Pall. https://luajit.org/ |
| CPython | Python 3.13.5 | Python 3.13.5 |
| Node | v22.23.5 | v22.23.5 |
| rustc | rustc 1.100.0 (0ef1bc386 2026-11-09) | rustc 1.100.0 (0ef1bc386 2026-11-09) |
| sha256 of `cig-v1.24.0-linux-x86_64` | `957ddff2c27c9baa38b5aa43361e165e209ebb3d6a3c82cfa28d7b7777e91b3f` | `957ddff2c27c9baa38b5aa43361e165e209ebb3d6a3c82cfa28d7b7777e91b3f` |
| sha256 of `cigcompile-v0.5.0-linux-x86_64` | `4ba7d3e10d5c82b118d7f6c792b6c96375b09de68897dfb9a02e947bb77c7ef0` | `4ba7d3e10d5c82b118d7f6c792b6c96375b09de68897dfb9a02e947bb77c7ef0` |
| sha256 of `lua5.4` | `adc4b6a990abb309187207b1650489b02173bd40e69710130e37a77245cfd93f` | `adc4b6a990abb309187207b1650489b02173bd40e69710130e37a77245cfd93f` |
| sha256 of `luajit` | `e525f77c526139de63f130ae8a72aca04d41ec4249c375f96185d581a4a0e5c1` | `e525f77c526139de63f130ae8a72aca04d41ec4249c375f96185d581a4a0e5c1` |
| sha256 of `python3` | `889c603f0d17cb54060951bcf4c4f9b8c9ebd9e52b392c70209bbb9755d797d9` | `889c603f0d17cb54060951bcf4c4f9b8c9ebd9e52b392c70209bbb9755d797d9` |
| sha256 of `node` | `2f822d544601dfff107f57d945b649d15c5bf6302595735a6bd9cdc7ee9669bd` | `2f822d544601dfff107f57d945b649d15c5bf6302595735a6bd9cdc7ee9669bd` |
| sha256 of `rustc` | `5073422953727c9427a76b7fce03aa9404da46315e5a1e4965389512a41e8db0` | `5073422953727c9427a76b7fce03aa9404da46315e5a1e4965389512a41e8db0` |

## What this does not show

- **One machine.** A 2018 laptop processor, on battery and on its everyday settings. Another processor, another operating system or another power setting moves every number; the ratios move less than the times.
- **Small programs, timed whole.** Most of these programs run for tens of milliseconds, so start-up is a large part of every time; it is the time a program's user waits, not the speed of a long-running program.
- **cig as it ships today**: cig 1.24.0 and cigcompile 0.5.0, both released. The work merged since and in progress on the VM ([cig-script#243](https://github.com/otmof-ops/cig-script/issues/243)) is not in these numbers.
- **Not every program in every language.** Rust has eight of the 25 programs; Lua has no `json`, `json-deep`, `csv` or `startup-10`; Node has no `csv`. The three counts beside each geometric mean add up to the programs it covers.
