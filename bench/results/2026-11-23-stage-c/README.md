# Stage `c`, first numbers (2026-11-23)

<!-- SPDX-License-Identifier: CC-BY-4.0 -->

The programs and the raw data behind
[`docs/BENCHMARKS-2026-11-23-stage-c.md`](../../../docs/BENCHMARKS-2026-11-23-stage-c.md).

| file | what it holds |
|---|---|
| `fib.cig`, `mandel-fn.cig`, `churn.cig` | the three programs: their work is inside functions on numbers, which is what stage `c` compiles |
| `*.lua`, `*.py` | the same programs in Lua and Python |
| `measure.py` | times every row as a whole process, the rows in turn, and writes `times.json` |
| `times.json` | every sample of the run the paper reports, in nanoseconds |
| `report.py` | makes the paper's two tables from `times.json` |
