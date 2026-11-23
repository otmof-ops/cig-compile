#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
# SPDX-License-Identifier: FSL-1.1-ALv2
"""Time the three programs of this folder as whole processes.

    python3 measure.py BUILT [RUNS]

BUILT holds the programs `cigcompile build <name>.cig --stage <s> -o <name>-<s>`
made for s in a, b, c, and is where `times.json` is written. LUA, LUAJIT and
PYTHON name the other implementations (default: lua5.4, luajit, python3).
Every run's output is checked. The rows are taken in turn, RUNS times over
(9 by default), so drift on the machine lands on all of them alike.
"""
import json
import os
import pathlib
import statistics
import subprocess
import sys
import time

here = pathlib.Path(__file__).resolve().parent
built = pathlib.Path(sys.argv[1]).resolve()
runs = int(sys.argv[2]) if len(sys.argv) > 2 else 9
lua = os.environ.get("LUA", "lua5.4")
luajit = os.environ.get("LUAJIT", "luajit")
python = os.environ.get("PYTHON", "python3")
expect = {"fib": "9227465", "mandel-fn": "41295", "churn": "5999999"}
env = dict(os.environ)
env.pop("CI", None)

rows = []
for prog in expect:
    # `--max-steps 0`: no step budget (fib(35) takes more than the default's
    # ten million). The steps are still counted.
    cig = lambda stage: [str(built / f"{prog}-{stage}"), "--max-steps", "0"]
    rows += [
        (prog, "stage a", cig("a"), {}),
        (prog, "stage b", cig("b"), {}),
        (prog, "stage c, CIG_NATIVE=0", cig("c"), {"CIG_NATIVE": "0"}),
        (prog, "stage c", cig("c"), {}),
        (prog, "Lua", [lua, str(here / f"{prog}.lua")], {}),
        (prog, "LuaJIT -joff", [luajit, "-joff", str(here / f"{prog}.lua")], {}),
        (prog, "LuaJIT", [luajit, str(here / f"{prog}.lua")], {}),
        (prog, "CPython", [python, str(here / f"{prog}.py")], {}),
    ]
samples = {(p, w): [] for p, w, _, _ in rows}
for _ in range(runs):
    for p, w, cmd, extra in rows:
        e = dict(env)
        e.update(extra)
        t = time.perf_counter_ns()
        r = subprocess.run(cmd, env=e, cwd=built, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        ns = time.perf_counter_ns() - t
        if r.returncode != 0 or r.stdout.strip() != expect[p]:
            sys.exit(f"{p}, {w}: exit {r.returncode}, printed {r.stdout!r}\n{r.stderr}")
        samples[(p, w)].append(ns)
out = [{"program": p, "what": w, "samples_ns": samples[(p, w)]} for p, w, _, _ in rows]
(built / "times.json").write_text(json.dumps(out, indent=1) + "\n")
for row in out:
    ms = sorted(n / 1e6 for n in row["samples_ns"])
    print(f"{row['program']:10} {row['what']:22} median {statistics.median(ms):9.2f} ms  ({ms[0]:.2f} to {ms[-1]:.2f})")
