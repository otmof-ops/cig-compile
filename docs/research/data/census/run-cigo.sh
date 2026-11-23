#!/usr/bin/env bash
# run-cigo.sh CIG CIGO_CLONE OUT_DIR: the census over cigo's own script (orchestrate.cig) for the cases
# cigo's bench/engines.toml times, each a fresh process on the VM in a scratch project and home.
# Keeps OUT_DIR/cigo-<case>.jsonl, .stats.txt and appends to verdicts.tsv.
set -uo pipefail
cig=$(realpath "$1"); cigo=$(realpath "$2"); out=$(realpath -m "$3")
mkdir -p "$out"
tool=$cigo/orchestrate.cig
one() { # name expect-file-or-- stdin-file-or-- args...
  local n=$1 expect=$2 stdin=$3; shift 3
  local w; w=$(mktemp -d /var/tmp/w36-cigo.XXXXXX)
  cp "$cigo/bench/cig-tasks.json" "$w/"; cp -r "$cigo/bench/fixtures" "$w/fixtures"
  rm -f "$out/cigo-$n.jsonl"
  local in=/dev/null; [ "$stdin" = - ] || in=$w/fixtures/$stdin
  ( cd "$w" && env -i PATH=/usr/bin:/bin HOME="$w/home" CIGSCRIPT_HOME="$w/home/.cigscript" CIGO_HOME="$w/home/.cig-orchestrator" \
      XDG_CONFIG_HOME="$w/home/.config" XDG_DATA_HOME="$w/home/.local/share" \
      XDG_CACHE_HOME="$w/home/.cache" XDG_STATE_HOME="$w/home/.local/state" \
      NO_COLOR=1 CIG_DOCTOR=0 CIG_SEAL_KEY=file CI= CIGO_REHEARSE=never CIGO_OSV=off \
      CIGO_OSV_API=file:///nonexistent/osv-api CIGO_OSV_DUMPS=file:///nonexistent/osv-dumps \
      CIGO_BLACKLIST_DIR=/nonexistent/cigo-bench/blacklist.d CIG_INTERNAL_TRACKER_DIR=/nonexistent/cigo-bench/tracker \
      CIG_VM_CENSUS="$out/cigo-$n.jsonl" CIG_VM_STATS=1 \
      timeout 600 "$cig" run --engine vm "$tool" -- "$@" < "$in" > "$w/stdout" 2> "$out/cigo-$n.stats.txt" )
  local rc=$? same=-
  if [ "$expect" != - ]; then
    if cmp -s "$w/stdout" "$w/fixtures/$expect"; then same=same; else same=DIFFERS; fi
  fi
  local ran=NOT-THE-VM; [ -s "$out/cigo-$n.jsonl" ] && ran=vm
  printf 'cigo-%s\t%s\t%s\t%s\n' "$n" "$rc" "$same" "$ran" | tee -a "$out/verdicts.tsv"
  rm -rf "$w"
}
one version - - --version
one help - - --help
one list list.expect - list
one audit-one audit-1.expect - audit task-01
one hook-benign - hook-benign.json agent hook claude-code
one hook-curl-sh - hook-curl-sh.json agent hook claude-code
one guard-line - guard-benign.in guard check-line
