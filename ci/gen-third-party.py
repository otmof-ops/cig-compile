#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
# SPDX-License-Identifier: FSL-1.1-ALv2
"""Regenerate THIRD-PARTY-NOTICES.md from `cargo license --avoid-dev-deps --json`."""
import collections
import json
import pathlib
import subprocess

root = pathlib.Path(__file__).resolve().parent.parent
rows = json.loads(subprocess.check_output(["cargo", "license", "--avoid-dev-deps", "--json"], cwd=root))
rows = [r for r in rows if r["name"] != "cigcompile"]
fam = collections.OrderedDict()
for r in sorted(rows, key=lambda r: (r.get("license") or "unrecorded", r["name"])):
    fam.setdefault(r.get("license") or "unrecorded", []).append(r)
out = [
    "# Third-party notices",
    "",
    "cigcompile and cig-runtime link the Rust crates below, pulled as packages (nothing is vendored); "
    "cig-runtime is cig itself, built from the `cigscript` crate (CigScript, https://github.com/otmof-ops/cig-script, "
    "FSL-1.1-ALv2 from v1.21.0, Apache-2.0 for releases through v1.20.0). Every linked crate is under a permissive "
    "or source-available license whose use in cigcompile and in a compiled program is allowed; where a crate offers "
    "a choice (`OR`), the permissive option is the one taken. Holder names come from each crate's metadata; "
    "`unrecorded` means the crate publishes no author field. Regenerate with `cargo license --avoid-dev-deps --json` "
    "(cargo-license) and `ci/gen-third-party.py`.",
    "",
]
for lic, items in sorted(fam.items(), key=lambda kv: (-len(kv[1]), kv[0])):
    out += [f"## {lic}", "", "| crate | version | holder | source |", "|---|---|---|---|"]
    for r in items:
        holder = (r.get("authors") or "unrecorded").replace("|", ", ")
        out.append(f"| {r['name']} | {r['version']} | {holder} | {r.get('repository') or 'crates.io'} |")
    out.append("")
(root / "THIRD-PARTY-NOTICES.md").write_text("\n".join(out))
print(f"wrote THIRD-PARTY-NOTICES.md: {len(rows)} crates")
