#!/usr/bin/env python3
"""bench.py WORK SET_DIR ROUNDS: time whole processes, interleaved rounds, outputs checked.

Sides, for each compute program: each flavor's tree-walker (`cig run`) and VM (its stage b
program), and four switches that change only the page size a side's memory gets:
  gnu-vm+hugetlb      glibc's malloc asks for huge pages (GLIBC_TUNABLES=glibc.malloc.hugetlb=1)
  gnu-vm+nothp        transparent huge pages off for the process (a control: glibc's malloc
                      does not ask for them, so nothing should change where THP is `madvise`)
  musl-mi-vm+nothp    the same switch on the release flavor, whose arena does ask for them
  musl-mi-tw+nothp    and on its tree-walker
A program that burns nothing is run as a dry run on every side. After the timed rounds, one
more run of each side reads its page faults and its user and system time."""
import ctypes, glob, math, os, statistics, subprocess, sys, time

work, set_dir, rounds = sys.argv[1], sys.argv[2], int(sys.argv[3])
FLAVORS = ["gnu", "gnu-mi", "musl-mi", "musl"]
libc = ctypes.CDLL(None, use_errno=True)
PR_SET_THP_DISABLE = 41


def cig_of(f):
    t = "x86_64-unknown-linux-gnu" if f.startswith("gnu") else "x86_64-unknown-linux-musl"
    return f"{work}/target-{f}/{t}/release/cig"


def pre(nothp):
    def f():
        if nothp:
            libc.prctl(PR_SET_THP_DISABLE, 1, 0, 0, 0)   # kept across exec
    return f


def run(side, name, want_out=False):
    d = os.path.join(work, "runs", f"{name}.{side['side']}")
    os.makedirs(d, exist_ok=True)
    for e in os.scandir(d):
        if e.is_file() or e.is_symlink():
            os.unlink(e.path)
    env = dict(os.environ)
    env.update(side.get("env", {}))
    t = time.perf_counter()
    p = subprocess.Popen(side["cmd"], cwd=d, stdin=subprocess.DEVNULL,
                         stdout=subprocess.PIPE if want_out else subprocess.DEVNULL,
                         stderr=subprocess.DEVNULL, env=env, preexec_fn=pre(side.get("nothp", False)))
    got = p.stdout.read() if want_out else b""
    _, status, ru = os.wait4(p.pid, 0)
    ms = (time.perf_counter() - t) * 1000
    p.returncode = os.waitstatus_to_exitcode(status)
    if want_out:
        p.stdout.close()
    return ms, p.returncode, got, ru


def pct(v, q):
    s = sorted(v)
    return s[min(len(s) - 1, int(q * len(s)))]


def gmean(v):
    return math.exp(sum(math.log(x) for x in v) / len(v)) if v else float("nan")


# the switch itself, shown working
p = subprocess.run(["grep", "THP_enabled", "/proc/self/status"], capture_output=True, text=True, preexec_fn=pre(True))
print("under the switch:", p.stdout.strip() or "(this kernel does not print THP_enabled)")
p = subprocess.run(["grep", "THP_enabled", "/proc/self/status"], capture_output=True, text=True, preexec_fn=pre(False))
print("without it:      ", p.stdout.strip() or "(this kernel does not print THP_enabled)")

programs = []
for f in sorted(glob.glob(set_dir + "/*.cig")):
    n = os.path.basename(f)[:-4]
    if n == "hello" or n.startswith("startup-"):
        continue
    burns = any(l.lstrip().startswith("burn") for l in open(f, encoding="utf-8"))
    dry = [] if burns else ["--dry-run"]
    sides = []
    for fl in FLAVORS:
        sides.append({"side": f"{fl}-tw", "cmd": [cig_of(fl), "run"] + dry + [f]})
        b = f"{work}/bin-{fl}/{n}.b"
        if os.path.exists(b):
            sides.append({"side": f"{fl}-vm", "cmd": [b] + dry})
    g, m = f"{work}/bin-gnu/{n}.b", f"{work}/bin-musl-mi/{n}.b"
    if os.path.exists(g):
        sides.append({"side": "gnu-vm+hugetlb", "cmd": [g] + dry, "env": {"GLIBC_TUNABLES": "glibc.malloc.hugetlb=1"}})
        sides.append({"side": "gnu-vm+nothp", "cmd": [g] + dry, "nothp": True})
    if os.path.exists(m):
        sides.append({"side": "musl-mi-vm+nothp", "cmd": [m] + dry, "nothp": True})
    sides.append({"side": "musl-mi-tw+nothp", "cmd": [cig_of("musl-mi"), "run"] + dry + [f], "nothp": True})
    programs.append({"name": n, "expect": f"{set_dir}/{n}.expect", "sides": sides, "dry": not burns})
print("real runs (they burn):", [p["name"] for p in programs if not p["dry"]])
print(len(programs), "programs,", sum(len(p["sides"]) for p in programs), "sides,", rounds, "rounds")

med, flt, cpu = {}, {}, {}
rows = []
for prog in programs:
    name = prog["name"]
    expect = open(prog["expect"], "rb").read()
    ok, times = {}, {}
    for s in prog["sides"]:
        try:
            _, rc, got, _ = run(s, name, want_out=True)
            ok[s["side"]] = rc == 0 and got == expect
            if not ok[s["side"]]:
                print(f"WRONG {name} {s['side']}: exit {rc}, got {got[:100]!r}", flush=True)
        except Exception as e:  # a side that cannot start is reported, the rest go on
            ok[s["side"]] = False
            print(f"FAILED {name} {s['side']}: {e}", flush=True)
        times[s["side"]] = []
    for _ in range(rounds):
        for s in prog["sides"]:
            if ok[s["side"]]:
                times[s["side"]].append(run(s, name)[0])
    for s in prog["sides"]:
        v = times[s["side"]]
        if not v:
            rows.append([name, s["side"], "WRONG"] + [""] * 9)
            continue
        _, _, _, ru = run(s, name)
        med[(name, s["side"])] = statistics.median(v)
        flt[(name, s["side"])] = ru.ru_minflt
        cpu[(name, s["side"])] = (ru.ru_utime * 1000, ru.ru_stime * 1000)
        rows.append([name, s["side"], "ok", f"{statistics.median(v):.2f}", f"{min(v):.2f}", f"{pct(v, 0.1):.2f}",
                     f"{pct(v, 0.9):.2f}", str(ru.ru_minflt), str(ru.ru_majflt), str(ru.ru_maxrss),
                     f"{ru.ru_utime * 1000:.1f}", f"{ru.ru_stime * 1000:.1f}"])
    print("done", name, " ".join(f"{s['side']}={med[(name, s['side'])]:.1f}" for s in prog["sides"] if (name, s["side"]) in med), flush=True)

print("\n== rows: program, side, ok, median ms, min, p10, p90, minor faults, major faults, max RSS KiB (the launcher's is its floor), user ms, system ms")
for r in rows:
    print("\t".join(r))

names = [p["name"] for p in programs]


def table(title, cols, key=med, fmt="{:.1f}"):
    print(f"\n== {title}")
    print("| program | " + " | ".join(cols) + " |")
    print("|---|" + "---:|" * len(cols))
    for n in names:
        print(f"| {n} | " + " | ".join(fmt.format(key[(n, c)]) if (n, c) in key else "" for c in cols) + " |")


def ratio_line(label, a, b):
    v = [(n, med[(n, a)] / med[(n, b)]) for n in names if (n, a) in med and (n, b) in med]
    if not v:
        return
    r = [x for _, x in v]
    worst = max(v, key=lambda t: t[1])
    best = min(v, key=lambda t: t[1])
    print(f"{label}: geometric mean {gmean(r):.3f}, median {statistics.median(r):.3f}, "
          f"highest {worst[0]} {worst[1]:.2f}, lowest {best[0]} {best[1]:.2f} ({len(r)} programs)")


vm = [f"{f}-vm" for f in FLAVORS] + ["gnu-vm+hugetlb", "gnu-vm+nothp", "musl-mi-vm+nothp"]
tw = [f"{f}-tw" for f in FLAVORS] + ["musl-mi-tw+nothp"]
table("the VM, median ms", vm)
table("the tree-walker, median ms", tw)
table("the VM, minor page faults of one run", vm, key=flt, fmt="{}")
table("the tree-walker, minor page faults of one run", tw, key=flt, fmt="{}")
sysms = {k: v[1] for k, v in cpu.items()}
table("the VM, system time of one run, ms", vm, key=sysms)

print("\n== ratios of medians (first over second)")
ratio_line("VM: gnu over musl-mi", "gnu-vm", "musl-mi-vm")
ratio_line("VM: gnu-mi over musl-mi (the libc and the linking alone)", "gnu-mi-vm", "musl-mi-vm")
ratio_line("VM: gnu over gnu-mi (the allocator alone, on glibc)", "gnu-vm", "gnu-mi-vm")
ratio_line("VM: musl over musl-mi (the allocator alone, on musl)", "musl-vm", "musl-mi-vm")
ratio_line("VM: gnu with huge pages asked for, over gnu", "gnu-vm+hugetlb", "gnu-vm")
ratio_line("VM: gnu without THP over gnu (control)", "gnu-vm+nothp", "gnu-vm")
ratio_line("VM: musl-mi without THP over musl-mi", "musl-mi-vm+nothp", "musl-mi-vm")
ratio_line("tree-walker: gnu over musl-mi", "gnu-tw", "musl-mi-tw")
ratio_line("tree-walker: gnu-mi over musl-mi", "gnu-mi-tw", "musl-mi-tw")
ratio_line("tree-walker: musl over musl-mi", "musl-tw", "musl-mi-tw")
ratio_line("tree-walker: musl-mi without THP over musl-mi", "musl-mi-tw+nothp", "musl-mi-tw")
for f in FLAVORS:
    ratio_line(f"{f}: tree-walker over VM", f"{f}-tw", f"{f}-vm")
bad = [r for r in rows if r[2] != "ok"]
print(f"\n{len(rows) - len(bad)} sides timed, {len(bad)} wrong or failed")
sys.exit(1 if bad else 0)
