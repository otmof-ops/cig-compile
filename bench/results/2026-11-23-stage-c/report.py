#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
# SPDX-License-Identifier: FSL-1.1-ALv2
"""The tables of docs/BENCHMARKS-2026-11-23-stage-c.md, from times.json.

    python3 report.py [times.json]
"""
import json
import pathlib
import statistics
import sys

path = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else pathlib.Path(__file__).resolve().parent / "times.json"
rows = json.loads(path.read_text())
med = {(r["program"], r["what"]): statistics.median(r["samples_ns"]) / 1e6 for r in rows}
lo = {(r["program"], r["what"]): min(r["samples_ns"]) / 1e6 for r in rows}
hi = {(r["program"], r["what"]): max(r["samples_ns"]) / 1e6 for r in rows}
progs = list(dict.fromkeys(r["program"] for r in rows))
whats = list(dict.fromkeys(r["what"] for r in rows))


def ms(x):
    return f"{x:,.0f}" if x >= 100 else f"{x:.1f}"


print("| median, ms | " + " | ".join(f"`{p}`" for p in progs) + " |")
print("|---|" + "---:|" * len(progs))
for w in whats:
    print(f"| {w} | " + " | ".join(f"{ms(med[(p, w)])} ({ms(lo[(p, w)])} to {ms(hi[(p, w)])})" for p in progs) + " |")
print()
print("| stage c's time ÷ theirs | " + " | ".join(f"`{p}`" for p in progs) + " |")
print("|---|" + "---:|" * len(progs))
for w in whats:
    if w != "stage c":
        print(f"| {w} | " + " | ".join(f"{med[(p, 'stage c')] / med[(p, w)]:.3f}" for p in progs) + " |")
print()
print(f"runs a row: {len(rows[0]['samples_ns'])}")
