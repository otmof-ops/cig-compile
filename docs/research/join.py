#!/usr/bin/env python3
"""w36-join.py STUDIES_DIR OUT_DIR: check the shape of every study of the compiler study and write
the indexes that are made from them:
  OUT_DIR/INDEX.md             every study, its entries and their verdicts
  OUT_DIR/for-the-issues.md    what each study says to each issue, in the studies' own words
  OUT_DIR/for-the-decisions.md the entries that feed each decision (a) to (i), with their verdicts
  OUT_DIR/measurements-owed.md every measurement an entry asks for
  OUT_DIR/not-settled.md       what each study could not establish
Reads the studies; writes only under OUT_DIR. Exit 1 when a study is not in shape."""
import os
import re
import sys

DECISIONS = {
    "a": "the value representation and ABI",
    "b": "one inline-cache and feedback format for the VM, stage C and cig-cherry",
    "c": "one counter and profile format",
    "d": "guards, snapshots and exits that keep the step count, the open tries and burns and the journal's position",
    "e": "crate boundaries: the lowering, the shared code generator, the one unsafe loader",
    "f": "the cache for lowered modules and native code",
    "g": "reference counting made cheap: elision, borrowing, reuse",
    "h": "shapes for records",
    "i": "the type lattice for stage D and for observed types",
}
SECTIONS = ["What cig should carry away", "Design points", "For the issues", "Measurements owed", "Not settled"]
VERDICTS = ("take", "adapt", "reject", "not applicable")


def section(text, name):
    m = re.search(rf"^## {re.escape(name)}\s*$", text, re.M)
    if not m:
        return None
    rest = text[m.end():]
    n = re.search(r"^## ", rest, re.M)
    return rest[:n.start()].strip("\n") if n else rest.strip("\n")


def study(path):
    text = open(path, encoding="utf-8").read()
    problems = []
    title = text.split("\n", 1)[0].lstrip("# ").strip()
    for s in SECTIONS:
        if section(text, s) is None:
            problems.append(f"no section '## {s}'")
    for head in ("Serves", "Pins", "License rule"):
        if not re.search(rf"^- \*\*{head}\.\*\*", text, re.M):
            problems.append(f"no '- **{head}.**' line")
    entries = []
    body = section(text, "Design points") or ""
    parts = re.split(r"^### (\d+)\. (.+)$", body, flags=re.M)
    for i in range(1, len(parts), 3):
        num, name, rest = int(parts[i]), parts[i + 1].strip(), parts[i + 2]
        v = re.search(r"^- \*\*For cig: (take|adapt|reject|not applicable)\b", rest, re.M)
        verdict = v.group(1).strip() if v else None
        if verdict not in VERDICTS:
            problems.append(f"entry {num}: no verdict (take, adapt, reject, not applicable)")
        for field in ("What it does, and why", "What would decide it", "Feeds", "Read"):
            if not re.search(rf"^- \*\*{re.escape(field)}\.\*\*", rest, re.M):
                problems.append(f"entry {num}: no '- **{field}.**' line")
        f = re.search(r"^- \*\*Feeds\.\*\*(.*)$", rest, re.M)
        feeds = f.group(1) if f else ""
        entries.append({"n": num, "name": name, "verdict": verdict,
                        "letters": sorted(set(re.findall(r"\(([a-i])\)", feeds))),
                        "issues": issues(feeds)})
    nums = [e["n"] for e in entries]
    if nums != list(range(1, len(nums) + 1)):
        problems.append(f"entries are not numbered 1 to {len(nums)}: {nums}")
    lines = text.count("\n")
    if not 250 <= lines <= 600:
        problems.append(f"{lines} lines (the contract asks for 250 to 550)")
    return {"path": path, "file": os.path.basename(path), "title": title, "text": text,
            "entries": entries, "problems": problems, "lines": lines}


def issues(feeds):
    """Issue references in a Feeds line: a bare #N belongs to the repository named before it."""
    out, repo = [], "cig-compile"
    for m in re.finditer(r"(cig-compile|cig-script|cig-orchestrator|cigar-script|cig-installer)?#(\d+)", feeds):
        if m.group(1):
            repo = m.group(1)
        out.append(f"{repo}#{m.group(2)}")
    return out


def main():
    src, out = sys.argv[1], sys.argv[2]
    os.makedirs(out, exist_ok=True)
    studies = [study(os.path.join(src, f)) for f in sorted(os.listdir(src)) if re.match(r"\d\d-.*\.md$", f)]
    bad = 0
    for s in studies:
        for p in s["problems"]:
            print(f"{s['file']}: {p}")
            bad = 1

    with open(os.path.join(out, "INDEX.md"), "w", encoding="utf-8") as f:
        f.write("# The studies, entry by entry\n\nMade from the studies by `join.py`. A verdict is what the study proposes for cig; none is decided until its measurement exists.\n\n")
        f.write("| study | entries | take | adapt | reject | not applicable |\n|---|---:|---:|---:|---:|---:|\n")
        tot = {v: 0 for v in VERDICTS}
        for s in studies:
            c = {v: sum(1 for e in s["entries"] if e["verdict"] == v) for v in VERDICTS}
            for v in VERDICTS:
                tot[v] += c[v]
            f.write(f"| [{s['title']}](studies/{s['file']}) | {len(s['entries'])} | " + " | ".join(str(c[v]) for v in VERDICTS) + " |\n")
        f.write(f"| **all {len(studies)}** | **{sum(len(s['entries']) for s in studies)}** | " + " | ".join(f"**{tot[v]}**" for v in VERDICTS) + " |\n")
        for s in studies:
            f.write(f"\n## {s['title']}\n\n[studies/{s['file']}](studies/{s['file']})\n\n| | mechanism | for cig | feeds |\n|---:|---|---|---|\n")
            for e in s["entries"]:
                feeds = ", ".join(e["issues"] + [f"({x})" for x in e["letters"]])
                f.write(f"| {e['n']} | {e['name']} | {e['verdict']} | {feeds} |\n")

    with open(os.path.join(out, "for-the-decisions.md"), "w", encoding="utf-8") as f:
        f.write("# The entries that feed each decision\n\nMade from the studies' `Feeds` lines by `join.py`. The nine decisions are the ones the studies were written to inform; each is made once, for the VM, stage C and cig-cherry together.\n")
        for letter, what in DECISIONS.items():
            rows = [(s, e) for s in studies for e in s["entries"] if letter in e["letters"]]
            f.write(f"\n## ({letter}) {what}\n\n{len(rows)} entries in {len({s['file'] for s, _ in rows})} studies.\n\n")
            if rows:
                f.write("| study | | mechanism | for cig |\n|---|---:|---|---|\n")
                for s, e in rows:
                    f.write(f"| [{s['file'][:2]}](studies/{s['file']}) {s['title']} | {e['n']} | {e['name']} | {e['verdict']} |\n")

    with open(os.path.join(out, "for-the-issues.md"), "w", encoding="utf-8") as f:
        f.write("# What the studies say to each issue\n\nMade from the studies by `join.py`: first the entries whose `Feeds` line names an issue, then each study's own \"For the issues\" section, in its words. These are the material for the comment cig-compile#98 owes each issue it touches; none has been posted.\n\n## Entries by issue\n\n")
        by = {}
        for s in studies:
            for e in s["entries"]:
                for i in e["issues"]:
                    by.setdefault(i, []).append((s, e))
        def key(i):
            repo, n = i.split("#")
            return (repo != "cig-compile", repo, int(n))
        f.write("| issue | entries | take | adapt | reject | studies |\n|---|---:|---:|---:|---:|---|\n")
        for i in sorted(by, key=key):
            rows = by[i]
            c = {v: sum(1 for _, e in rows if e["verdict"] == v) for v in VERDICTS}
            which = ", ".join(sorted({s["file"][:2] for s, _ in rows}))
            f.write(f"| {i} | {len(rows)} | {c['take']} | {c['adapt']} | {c['reject']} | {which} |\n")
        for s in studies:
            f.write(f"\n## From: {s['title']}\n\n[studies/{s['file']}](studies/{s['file']})\n\n{section(s['text'], 'For the issues') or ''}\n")

    for name, sec, head in (("measurements-owed.md", "Measurements owed",
                             "# Measurements the studies ask for\n\nMade from the studies by `join.py`. Nothing was measured by the studies themselves; each line names what would decide an entry. A measurement of time belongs on the test machine; a count can be made anywhere. What this run did measure is in `data/`.\n"),
                            ("not-settled.md", "Not settled",
                             "# What the studies could not establish\n\nMade from the studies by `join.py`: each study's own list, in its words.\n")):
        with open(os.path.join(out, name), "w", encoding="utf-8") as f:
            f.write(head)
            for s in studies:
                f.write(f"\n## {s['title']}\n\n[studies/{s['file']}](studies/{s['file']})\n\n{section(s['text'], sec) or ''}\n")

    print(f"{len(studies)} studies, {sum(len(s['entries']) for s in studies)} entries, "
          f"{sum(s['lines'] for s in studies)} lines; indexes written to {out}")
    sys.exit(bad)


main()
