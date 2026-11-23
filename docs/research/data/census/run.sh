#!/usr/bin/env bash
# run.sh CIG PROGRAMS_DIR OUT_DIR: run every .cig of PROGRAMS_DIR on the VM of a census build of cig
# (bench-stats feature with the census patch), each in a scratch folder with a scratch home, and keep
# OUT_DIR/<name>.jsonl (the census), <name>.stats.txt (the VM's own opcode table) and verdicts.tsv:
# whether the program printed what its .expect holds, and whether the VM ran it.
set -uo pipefail
cig=$(realpath "$1"); progs=$(realpath "$2"); out=$(realpath -m "$3")
mkdir -p "$out"
: > "$out/verdicts.tsv"
for f in "$progs"/*.cig; do
  n=$(basename "$f" .cig)
  w=$(mktemp -d /var/tmp/w36-census.XXXXXX)
  cp "$f" "$w/"
  rm -f "$out/$n.jsonl"
  ( cd "$w" && env -i PATH=/usr/bin:/bin HOME="$w/home" CIGSCRIPT_HOME="$w/home/.cigscript" \
      XDG_CONFIG_HOME="$w/home/.config" XDG_DATA_HOME="$w/home/.local/share" \
      XDG_CACHE_HOME="$w/home/.cache" XDG_STATE_HOME="$w/home/.local/state" \
      CIG_SEAL_KEY=file CIG_VM_CENSUS="$out/$n.jsonl" CIG_VM_STATS=1 \
      timeout 900 "$cig" run --engine vm "$n.cig" > "$w/stdout" 2> "$out/$n.stats.txt" )
  rc=$?
  same=-
  if [ -f "$progs/$n.expect" ]; then
    if cmp -s "$w/stdout" "$progs/$n.expect"; then same=same; else same=DIFFERS; fi
  fi
  if [ -s "$out/$n.jsonl" ]; then ran=vm; else ran=NOT-THE-VM; fi
  printf '%s\t%s\t%s\t%s\n' "$n" "$rc" "$same" "$ran" | tee -a "$out/verdicts.tsv"
  rm -rf "$w"
done
