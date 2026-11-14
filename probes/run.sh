#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
# SPDX-License-Identifier: FSL-1.1-ALv2
# Where a compiled `strings` holds 3.5 MB more on cig 1.24.0 than on cig 1.21.0 (#90's bench gate:
# 26.5 to 30.0 MB, the only row over). For main (A) and the pull request's head (B): the same
# programs, interpreted and compiled, read at their exit for what is their own and what is their
# files', with transparent huge pages as the runner has them and turned off.
set -euo pipefail
A=${PROBE_A:-336a444}; B=${PROBE_B:-3a85c9e}
PROGS="hello strings lists maps words regex-ish csv strbuild"
here=$(cd "$(dirname "$0")" && pwd); work=$PWD/probe-work; mkdir -p "$work"; cd "$work"
echo "== the runner"
uname -r; nproc; grep -m1 "model name" /proc/cpuinfo
echo "THP enabled: $(cat /sys/kernel/mm/transparent_hugepage/enabled)  defrag: $(cat /sys/kernel/mm/transparent_hugepage/defrag)"
for f in /sys/kernel/mm/transparent_hugepage/hugepages-*/enabled; do [ -f "$f" ] && echo "$(basename "$(dirname "$f")"): $(cat "$f")"; done 2>/dev/null | tr '\n' ' '; echo
df -T /tmp "$work" | awk 'NR>1{print $1, $2, $7}'
echo "== sources"
git clone -q https://github.com/otmof-ops/cig-compile cc
for s in A B; do
  ref=$A; [ "$s" = B ] && ref=$B
  git -C cc worktree add -q "../cc-$s" "$ref"
  echo "$s: $(git -C "cc-$s" log --oneline -1 | cut -c1-90)   pins $(grep -o 'tag = "v[0-9.]*"' "cc-$s/Cargo.toml" | head -1)"
done
echo "== builds"
for s in A B; do
  ( cd "cc-$s" && (rustup show active-toolchain >/dev/null 2>&1 || rustup toolchain install >/dev/null 2>&1) && cargo build --release --locked --bin cigcompile 2>&1 | tail -1 )
done
env_for() { # the bench's own environment for a run
  export CIGSCRIPT_HOME=$work/home-$1 XDG_CACHE_HOME=$work/cache-$1 CIG_SEAL_KEY=file CIG_MAX_STEPS=0 CIG_REPORTS=never CIG_DOCTOR=0 NO_COLOR=1
}
echo "== programs"
for s in A B; do
  env_for "$s"; mkdir -p "bin-$s"
  for p in $PROGS; do
    "cc-$s/target/release/cigcompile" build "cc-$s/bench/$p.cig" -o "bin-$s/$p" >/dev/null 2>"build-$s-$p.log" || { echo "build of $p failed on $s:"; tail -5 "build-$s-$p.log"; exit 1; }
  done
  rt=$(find "$work/home-$s" "$work/cache-$s" "$HOME/.cigscript" "$HOME/.cache" -type f -name 'cig-v*-linux-x86_64' 2>/dev/null | head -1)
  [ -n "$rt" ] || { echo "the runtime cigcompile fetched was not found"; exit 1; }
  echo "$rt" > "rt-$s"; echo "$s: runtime $(basename "$rt") $(stat -c %s "$rt") bytes; a compiled hello $(stat -c %s "bin-$s/hello") bytes"
done
echo "== what each program held at its exit (MB)"
for p in $PROGS; do
  for s in A B; do
    env_for "$s"; rt=$(cat "rt-$s")
    for mode in interpreted compiled; do
      for thp in thp-as-is thp-off; do
        d=$work/run/$p-$s-$mode-$thp; mkdir -p "$d"; cp "cc-$s/bench/$p.cig" "$d/"
        flag=(); [ "$thp" = thp-off ] && flag=(--no-thp)
        if [ "$mode" = interpreted ]; then c=("$rt" run "$p.cig"); else c=("$work/bin-$s/$p"); fi
        printf '%-10s %s %-11s %-9s ' "$p" "$s" "$mode" "$thp"
        ( cd "$d" && python3 "$here/memsplit.py" "${flag[@]}" -- "${c[@]}" ) || true
      done
    done
  done
done
echo "== the same compiled strings, three more times each (is the figure stable?)"
for s in A B; do
  env_for "$s"
  for i in 1 2 3; do
    d=$work/run/again-$s-$i; mkdir -p "$d"
    printf 'strings    %s compiled    thp-as-is ' "$s"; ( cd "$d" && python3 "$here/memsplit.py" -- "$work/bin-$s/strings" ) || true
  done
done
