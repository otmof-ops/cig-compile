# The benchmark run of 2026-11-15

<!-- SPDX-License-Identifier: CC-BY-4.0 -->

The raw data behind [`docs/BENCHMARKS-2026-11-15.md`](../../../docs/BENCHMARKS-2026-11-15.md) and the README's "How fast a program runs": run `w33-full` on the test machine, 16:21 to 16:44 UTC, cig 1.24.0 and cigcompile 0.5.0 as released. The same run is CigScript's [`bench/results/2026-11-15/`](https://github.com/otmof-ops/cig-script/tree/main/bench/results/2026-11-15), whose harness, `report.py` and `check.py` made and checked these numbers.

| file | what it holds |
|---|---|
| `pass1/`, `pass2/` | the two passes: every timed run (`samples.tsv`), every check (`checks.tsv`), three readings of each pair's peak memory (`memory.tsv`), each round's start, clock, temperature and cig's run history (`rounds.tsv`), the sha256 of every program, expected output, reference and built program (`inputs.tsv`), and the machine and the tools (`conditions.txt`) |
| `cigcompile-bench.md`, `cigcompile-bench.json` | `cigcompile bench --runtime <cig 1.24.0>` with cigcompile 0.5.0 as released, run between the two passes |
| `numbers.json` | every number, as `report.py` made it |
| `paper.md.in`, `readme.md.in` | the templates of the paper and of the README's section |

`inputs.tsv` holds the sha256 of each file as it ran. Two references' comments were reworded after the run, their code unchanged: `ref/nbody.js` and `ref/spectral.js` now say that JavaScript's `Math.round` takes no number of places (they said JavaScript has no rounding to a number of places, which `toFixed` does).

With a checkout of CigScript beside this one:

```sh
python3 ../cig-script/bench/compare/check.py \
  --pass bench/results/2026-11-15/pass1 --pass bench/results/2026-11-15/pass2 \
  --numbers bench/results/2026-11-15/numbers.json \
  --render bench/results/2026-11-15/paper.md.in docs/BENCHMARKS-2026-11-15.md \
  --contains docs/BENCHMARKS-2026-11-15.md bench/results/2026-11-15/cigcompile-bench.md
```
