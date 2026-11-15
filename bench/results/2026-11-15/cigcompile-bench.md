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
