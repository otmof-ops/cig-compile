#!/usr/bin/env python3
"""census.py OUT_DIR [--json FILE]: read the census files a census build of cig wrote
(OUT_DIR/<program>.jsonl, one JSON object a line: `func`, `site`, `callee`, `made`) and print,
as Markdown tables, what the VM ran on: instructions by group, the types arithmetic and
comparisons saw, how many kinds of operand each site saw, shapes and key positions at field
sites, callees at call sites, argument-type rows per function, how concentrated the work is,
and what was made. Every figure is a count of executed instructions; nothing is timed.

A site is one instruction of one function (function index, pc). The census keeps up to 8
distinct observations per site and counts the rest (`sig_over`), so "more than 8" is exact
as a statement and its members are not listed."""
import gzip
import json
import os
import statistics
import sys

TAGS = ["null", "bool", "int", "float", "str", "list", "map", "func", "builtin", "module", "chain"]
ARITH = {"add", "sub", "mul", "div", "mod", "pow", "addk", "subk", "mulk", "divk", "modk", "powk"}
CMP = {"eq", "lt", "le", "eqk", "ltk", "lek", "eqj", "ltj", "lej", "nej"}
GROUPS = {
    "moves and constants": {"nop", "mov", "loadk", "loadnull", "loadbool", "loadint", "loadg"},
    "arithmetic": ARITH | {"neg"},
    "comparison and logic": CMP | {"not"},
    "control": {"jmp", "test", "testn", "testset", "forprep", "forloop", "loop", "iterprep", "iternext"},
    "step marks": {"steps"},
    "data": {"newlist", "newmap", "getfield", "setfield", "getindex", "setindex", "appendlist",
             "insertmap", "interp"},
    "calls and returns": {"call", "callb", "callm", "ret", "tailcall"},
    "closures and cells": {"closure", "getup", "setup", "cellnew", "cellget", "cellset"},
}
COUNTED = {4, 5, 6, 7}  # str, list, map, func: a copy is a reference-count increment
DICT = 1 << 63
BUILTIN = 1 << 62
OTHER = 1 << 61


def tag(t):
    return TAGS[t] if t < len(TAGS) else "none"


def rows(site, field):
    return [(int(w, 16), n) for w, n in site[field]]


def kinds(site, field="sig"):
    """How many distinct observations the site made: 9 stands for 'more than 8'."""
    return len(site[field]) + (1 if site[field + "_over"] else 0)


def pct(a, b):
    return 100.0 * a / b if b else None


def load(path):
    funcs, sites, callees, made = {}, [], [], {}
    opener = gzip.open if path.endswith(".gz") else open
    for line in opener(path, "rt", encoding="utf-8"):
        r = json.loads(line)
        k = r["kind"]
        if k == "func":
            funcs[r["f"]] = r
        elif k == "site":
            sites.append(r)
        elif k == "callee":
            callees.append(r)
        elif k == "made":
            for key in ("lists", "maps", "closures", "interpolated"):
                made[key] = made.get(key, 0) + r[key]
    return funcs, sites, callees, made


def weighted_single(sites, field="sig"):
    """Of the executions at these sites, how many were at a site that saw one kind only."""
    tot = sum(s["n"] for s in sites)
    one = sum(s["n"] for s in sites if kinds(s, field) == 1)
    return one, tot


def pairs(sites):
    """Operand type pairs over binary sites, by executions."""
    out = {}
    for s in sites:
        seen = 0
        for w, n in rows(s, "sig"):
            a, b = (w >> 4) & 15, w & 15
            if a == 2 and b == 2:
                k = "int, int"
            elif a == 3 and b == 3:
                k = "float, float"
            elif {a, b} == {2, 3}:
                k = "int and float"
            elif a == 4 and b == 4:
                k = "str, str"
            else:
                k = "other"
            out[k] = out.get(k, 0) + n
            seen += n
        if s["sig_over"]:
            out["other"] = out.get("other", 0) + s["sig_over"]
    return out


def measure(path):
    funcs, sites, callees, made = load(path)
    total = sum(s["n"] for s in sites)
    m = {"instructions": total, "sites": len(sites), "functions": len(funcs),
         "functions_entered": len({s["f"] for s in sites}),
         "module_instructions": sum(f.get("code", 0) for f in funcs.values())}
    by_op = {}
    for s in sites:
        by_op[s["op"]] = by_op.get(s["op"], 0) + s["n"]
    m["by_op"] = by_op
    grouped = {g: sum(n for op, n in by_op.items() if op in ops) for g, ops in GROUPS.items()}
    grouped["everything else"] = total - sum(grouped.values())
    m["groups"] = grouped
    top_ops = {}
    for s in sites:
        if s["f"] == 0:
            top_ops[s["op"]] = top_ops.get(s["op"], 0) + s["n"]
    m["top_level"] = {"n": sum(top_ops.values()),
                      "by_op": sorted(top_ops.items(), key=lambda x: -x[1])[:8]}

    arith = [s for s in sites if s["op"] in ARITH]
    cmp_ = [s for s in sites if s["op"] in CMP]
    m["arith"] = {"n": sum(s["n"] for s in arith), "pairs": pairs(arith)}
    m["cmp"] = {"n": sum(s["n"] for s in cmp_), "pairs": pairs(cmp_)}
    one, tot = weighted_single(arith + cmp_)
    m["typed_sites"] = {"n": tot, "one_kind": one,
                        "sites": len(arith + cmp_),
                        "sites_one_kind": sum(1 for s in arith + cmp_ if kinds(s) == 1)}

    idx = [s for s in sites if s["op"] in ("getindex", "setindex")]
    recv = {}
    for s in idx:
        for w, n in rows(s, "sig"):
            k = f"{tag((w >> 4) & 15)}[{tag(w & 15)}]"
            recv[k] = recv.get(k, 0) + n
        if s["sig_over"]:
            recv["other"] = recv.get("other", 0) + s["sig_over"]
    one, tot = weighted_single(idx)
    m["index"] = {"n": tot, "one_kind": one, "by": recv}

    fields = [s for s in sites if s["op"] in ("getfield", "setfield")]
    f = {"n": sum(s["n"] for s in fields), "sites": len(fields), "one_shape": 0, "two_shapes": 0,
         "up_to_8": 0, "over_8": 0, "one_position": 0, "on_dictionary": 0, "not_a_map": 0,
         "sites_one_shape": 0}
    for s in fields:
        k = kinds(s)
        f["one_shape" if k == 1 else "two_shapes" if k == 2 else "up_to_8" if k <= 8 else "over_8"] += s["n"]
        f["sites_one_shape"] += k == 1
        if kinds(s, "aux") == 1:
            f["one_position"] += s["n"]
        for w, n in rows(s, "sig"):
            if w & DICT:
                f["on_dictionary"] += n
            elif w < 16:
                f["not_a_map"] += n
    m["fields"] = f

    calls = [s for s in sites if s["op"] in ("call", "tailcall")]
    c = {"n": sum(s["n"] for s in calls), "sites": len(calls), "one_callee": 0, "to_function": 0,
         "to_builtin": 0, "to_other": 0, "sites_one_callee": 0}
    for s in calls:
        if kinds(s) == 1:
            c["one_callee"] += s["n"]
            c["sites_one_callee"] += 1
        for w, n in rows(s, "sig"):
            c["to_builtin" if w & BUILTIN else "to_other" if w & OTHER else "to_function"] += n
    m["calls"] = c

    def labelled(op):
        ss = [s for s in sites if s["op"] == op]
        top = {}
        for s in ss:
            top[s["label"]] = top.get(s["label"], 0) + s["n"]
        one, tot = weighted_single(ss)
        return {"n": tot, "one_kind": one,
                "top": sorted(top.items(), key=lambda x: -x[1])[:6]}
    m["callb"] = labelled("callb")
    m["callm"] = labelled("callm")

    fn = [r for r in callees if int(r["id"], 16) < OTHER]
    calls_to = [(sum(n for _, n in r["args"]) + r["args_over"],
                 len(r["args"]) + (1 if r["args_over"] else 0)) for r in fn]
    m["functions_called"] = {
        "functions": len(fn),
        "calls": sum(n for n, _ in calls_to),
        "calls_one_row": sum(n for n, k in calls_to if k == 1),
        "functions_one_row": sum(1 for _, k in calls_to if k == 1),
        "most_rows": max((k for _, k in calls_to), default=0),
    }

    order = sorted((s["n"] for s in sites), reverse=True)
    def top(k):
        return sum(order[:max(1, k)])
    need, acc = 0, 0
    for n in order:
        acc += n
        need += 1
        if acc >= 0.9 * total:
            break
    m["hot"] = {"top_10_sites": top(10), "top_tenth_of_sites": top(len(order) // 10),
                "sites_for_90_percent": need}

    m["made"] = made
    movs = [s for s in sites if s["op"] == "mov"]
    counted = sum(n for s in movs for w, n in rows(s, "sig") if w in COUNTED)
    loadk_str = sum(n for s in sites if s["op"] == "loadk" for w, n in rows(s, "sig") if w == 4)
    m["copies"] = {"mov": sum(s["n"] for s in movs), "mov_counted": counted, "loadk_str": loadk_str}

    rets = [s for s in sites if s["op"] == "ret"]
    one, tot = weighted_single(rets)
    m["returns"] = {"n": tot, "one_kind": one}
    return m


def fmt(x, digits=1):
    return "-" if x is None else f"{x:.{digits}f}"


def table(head, lines):
    print("| " + " | ".join(head) + " |")
    print("|" + "|".join(["---"] + ["---:"] * (len(head) - 1)) + "|")
    for r in lines:
        print("| " + " | ".join(str(c) for c in r) + " |")
    print()


def report(names, ms, title):
    print(f"## {title}\n")
    print("### Instructions executed, by group (percent of the program's instructions)\n")
    gs = list(GROUPS) + ["everything else"]
    table(["program", "instructions", "sites"] + gs,
          [[n, f"{m['instructions']:,}", m["sites"]] +
           [fmt(pct(m["groups"][g], m["instructions"])) for g in gs] for n, m in zip(names, ms)])

    print("### The script's top level (function 0)\n")
    print("What runs once a process before anything else can: the statements outside every function, which declare the script's functions and build its tables.\n")
    table(["program", "instructions", "in the top level", "of the run (%)", "its busiest instructions"],
          [[n, f"{m['instructions']:,}", f"{m['top_level']['n']:,}",
            fmt(pct(m["top_level"]["n"], m["instructions"])),
            ", ".join(f"`{op}` {c:,}" for op, c in m["top_level"]["by_op"][:6])] for n, m in zip(names, ms)])

    for key, word in (("arith", "Arithmetic"), ("cmp", "Comparisons")):
        print(f"### {word}: the operand types seen (percent of the group's executions)\n")
        cols = ["int, int", "float, float", "int and float", "str, str", "other"]
        table(["program", "executed"] + cols,
              [[n, f"{m[key]['n']:,}"] + [fmt(pct(m[key]["pairs"].get(c, 0), m[key]["n"])) for c in cols]
               for n, m in zip(names, ms)])

    print("### How settled a site's types are\n")
    print("Arithmetic and comparison sites. \"One kind\" is a site that saw one pair of operand types for the whole run.\n")
    table(["program", "sites", "sites with one kind", "executions", "at a site with one kind (%)"],
          [[n, m["typed_sites"]["sites"], m["typed_sites"]["sites_one_kind"], f"{m['typed_sites']['n']:,}",
            fmt(pct(m["typed_sites"]["one_kind"], m["typed_sites"]["n"]))] for n, m in zip(names, ms)])

    print("### Field reads and writes (`getfield`, `setfield`)\n")
    print("A shape is a record's keys in order (a map of up to 16 keys); a larger map is a dictionary. \"One position\" is a site where the key sat at one index for the whole run, which is what the VM's cache cell holds today.\n")
    table(["program", "executed", "sites", "sites with one shape", "one shape (%)", "two (%)", "3 to 8 (%)",
           "more than 8 (%)", "one position (%)", "on a dictionary (%)", "not on a map (%)"],
          [[n, f"{m['fields']['n']:,}", m["fields"]["sites"], m["fields"]["sites_one_shape"]] +
           [fmt(pct(m["fields"][k], m["fields"]["n"])) for k in
            ("one_shape", "two_shapes", "up_to_8", "over_8", "one_position", "on_dictionary", "not_a_map")]
           for n, m in zip(names, ms)])

    print("### Indexing (`getindex`, `setindex`): what was indexed by what (percent of executions)\n")
    keys = sorted({k for m in ms for k in m["index"]["by"]},
                  key=lambda k: -sum(m["index"]["by"].get(k, 0) for m in ms))[:6]
    table(["program", "executed", "at a site with one kind (%)"] + keys,
          [[n, f"{m['index']['n']:,}", fmt(pct(m["index"]["one_kind"], m["index"]["n"]))] +
           [fmt(pct(m["index"]["by"].get(k, 0), m["index"]["n"])) for k in keys] for n, m in zip(names, ms)])

    print("### Calls\n")
    print("`call` and `tailcall` call a value; `callb` calls a builtin the lowering resolved; `callm` calls a method on a value.\n")
    table(["program", "`call`", "sites", "sites with one callee", "at a site with one callee (%)",
           "to a function of the script (%)", "to a builtin value (%)", "`callb`", "`callm`",
           "`callm` at a site with one receiver type (%)"],
          [[n, f"{m['calls']['n']:,}", m["calls"]["sites"], m["calls"]["sites_one_callee"],
            fmt(pct(m["calls"]["one_callee"], m["calls"]["n"])),
            fmt(pct(m["calls"]["to_function"], m["calls"]["n"])),
            fmt(pct(m["calls"]["to_builtin"], m["calls"]["n"])),
            f"{m['callb']['n']:,}", f"{m['callm']['n']:,}",
            fmt(pct(m["callm"]["one_kind"], m["callm"]["n"]))] for n, m in zip(names, ms)])

    print("### Functions of the script: rows of argument types\n")
    print("For each function called through `call`: how many distinct rows of argument types it was called with (up to 16 are kept).\n")
    table(["program", "functions called", "called with one row", "calls", "calls to a function with one row (%)",
           "most rows at one function"],
          [[n, m["functions_called"]["functions"], m["functions_called"]["functions_one_row"],
            f"{m['functions_called']['calls']:,}",
            fmt(pct(m["functions_called"]["calls_one_row"], m["functions_called"]["calls"])),
            m["functions_called"]["most_rows"]] for n, m in zip(names, ms)])

    print("### Where the work is\n")
    print("The module is the script as lowered: every function it holds and every instruction in them. A site executed is an instruction that ran at least once.\n")
    table(["program", "functions in the module", "functions entered", "instructions in the module",
           "sites executed", "of the module (%)", "executions a site", "ten busiest sites (%)",
           "busiest tenth of the sites (%)", "sites that make 90% of the instructions"],
          [[n, m["functions"], m["functions_entered"], f"{m['module_instructions']:,}", f"{m['sites']:,}",
            fmt(pct(m["sites"], m["module_instructions"])),
            fmt(m["instructions"] / m["sites"] if m["sites"] else None),
            fmt(pct(m["hot"]["top_10_sites"], m["instructions"])),
            fmt(pct(m["hot"]["top_tenth_of_sites"], m["instructions"])), f"{m['hot']['sites_for_90_percent']:,}"]
           for n, m in zip(names, ms)])

    print("### What was made and copied (per 1,000 instructions)\n")
    print("A `mov` of a string, list, map or function, and a `loadk` of a string, each copy a counted value: one reference-count increment, and a decrement when the register is overwritten.\n")
    def per(x, m):
        return fmt(1000.0 * x / m["instructions"] if m["instructions"] else None, 2)
    table(["program", "lists", "maps", "closures", "interpolated strings", "`mov` of a counted value",
           "`loadk` of a string", "`mov` of a counted value (% of `mov`)"],
          [[n, per(m["made"].get("lists", 0), m), per(m["made"].get("maps", 0), m),
            per(m["made"].get("closures", 0), m), per(m["made"].get("interpolated", 0), m),
            per(m["copies"]["mov_counted"], m), per(m["copies"]["loadk_str"], m),
            fmt(pct(m["copies"]["mov_counted"], m["copies"]["mov"]))] for n, m in zip(names, ms)])

    print("### The builtins and methods called most\n")
    for n, m in zip(names, ms):
        b = ", ".join(f"`{l}` {c:,}" for l, c in m["callb"]["top"][:5]) or "none"
        me = ", ".join(f"`.{l}()` {c:,}" for l, c in m["callm"]["top"][:5]) or "none"
        print(f"- **{n}:** builtins: {b}. Methods: {me}.")
    print()


def main():
    args = sys.argv[1:]
    out_json = None
    if "--json" in args:
        i = args.index("--json")
        out_json = args[i + 1]
        del args[i:i + 2]
    d = args[0]
    files = {f.split(".jsonl")[0]: f for f in os.listdir(d) if f.endswith((".jsonl", ".jsonl.gz"))}
    names = sorted(files)
    ms = {n: measure(os.path.join(d, files[n])) for n in names}
    start = [n for n in names if n in ("hello", "startup-0", "startup-1", "startup-10", "lines-500")]
    cigo = [n for n in names if n.startswith("cigo-")]
    compute = [n for n in names if n not in start and n not in cigo]
    if compute:
        report(compute, [ms[n] for n in compute], f"The compute programs ({len(compute)})")
        typed = [pct(ms[n]["typed_sites"]["one_kind"], ms[n]["typed_sites"]["n"]) for n in compute]
        typed = [x for x in typed if x is not None]
        intint = [pct(ms[n]["arith"]["pairs"].get("int, int", 0), ms[n]["arith"]["n"]) for n in compute]
        intint = [x for x in intint if x is not None]
        print("### Across the compute programs\n")
        steps = [pct(ms[n]["groups"]["step marks"], ms[n]["instructions"]) for n in compute]
        print(f"- Step marks (`steps`) are {min(steps):.1f}% to {max(steps):.1f}% of a program's executed "
              f"instructions, median {statistics.median(steps):.1f}%.")
        moves = [pct(ms[n]["groups"]["moves and constants"], ms[n]["instructions"]) for n in compute]
        print(f"- Moves and constant loads are {min(moves):.1f}% to {max(moves):.1f}%, median "
              f"{statistics.median(moves):.1f}%.")
        print(f"- Arithmetic and comparison executions at a site with one kind of operands: "
              f"median {statistics.median(typed):.1f}%, lowest {min(typed):.1f}%.")
        print(f"- Arithmetic on two integers: median {statistics.median(intint):.1f}% of a program's arithmetic.")
        one = [pct(ms[n]["calls"]["one_callee"], ms[n]["calls"]["n"]) for n in compute]
        one = [x for x in one if x is not None]
        if one:
            print(f"- `call` executions at a site with one callee: median {statistics.median(one):.1f}%, "
                  f"lowest {min(one):.1f}% ({len(one)} programs make such calls).")
        shapes = [(ms[n]["fields"]["one_shape"], ms[n]["fields"]["n"]) for n in compute if ms[n]["fields"]["n"]]
        if shapes:
            print(f"- Field reads and writes at a site with one shape: {sum(a for a, _ in shapes):,} of "
                  f"{sum(b for _, b in shapes):,} executions in the {len(shapes)} programs that have any.")
        rows1 = [pct(ms[n]["functions_called"]["calls_one_row"], ms[n]["functions_called"]["calls"])
                 for n in compute if ms[n]["functions_called"]["calls"]]
        if rows1:
            print(f"- Calls to a function that was only ever called with one row of argument types: median "
                  f"{statistics.median(rows1):.1f}%, lowest {min(rows1):.1f}%.")
        print()
    if cigo:
        report(cigo, [ms[n] for n in cigo], "cigo's own script, one process a case")
    if start:
        report(start, [ms[n] for n in start], "Start-up programs")
    if out_json:
        with open(out_json, "w", encoding="utf-8") as f:
            json.dump(ms, f, indent=1, sort_keys=True)
            f.write("\n")


main()
