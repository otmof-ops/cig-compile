#!/usr/bin/env bash
# The proof #5 asks for: programs built on the static runtime run in a
# container that holds nothing but them (FROM scratch: no cig, no shell, no
# libc), and say exactly what the interpreter says on the host.
#
#   bash ci/nothing-installed.sh <cigscript checkout> <dir with cigcompile and cig-runtime>
set -euo pipefail
src=$(cd "$1" && pwd)
bin=$(cd "$2" && pwd)
rt="$bin/cig-runtime"
cc="$bin/cigcompile"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
progs="$tmp/progs"
mkdir -p "$progs"

file "$rt"
file "$rt" | grep -Eq 'statically linked|static-pie linked' || { echo "the runtime is not static"; exit 1; }

# An image with an empty file system: no cig, no shell, no libc.
tar -cf - --files-from /dev/null | docker import - cigcompile-empty >/dev/null
# Nothing in it: no shell to run, no cig to find.
if docker run --rm --network none cigcompile-empty /bin/sh -c true 2>/dev/null; then
  echo "the empty image has a shell"; exit 1
fi

normal() { sed -E 's/[0-9]{8}T[0-9]{6}-[0-9a-f]{6}/<run>/g; s/[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9:.]+(Z|[+-][0-9:]+)?/<time>/g; s/[0-9]+ ms/<n> ms/g'; }

# lay_out <dir> <case>: the files a case starts from.
lay_out() {
  rm -rf "$1"; mkdir -p "$1"
  case "$2" in
    tidy) mkdir -p "$1/downloads/sub"; for f in a.JPG b.mp4 c.flac d.pdf e.tar f.weird; do echo "$f" > "$1/downloads/$f"; done; echo g > "$1/downloads/sub/g.png" ;;
    report) printf 'category,amount\nfood,12.50\nrent,800\nfood,7.25\n' > "$1/expenses.csv" ;;
    measure) printf 'series,value\ntemperature,21.4\ntemperature,22.0\npressure,101.2\n' > "$1/readings.csv" ;;
    manifest) mkdir -p "$1/data/sub"; echo alpha > "$1/data/a.txt"; printf 'x,y\n1,2\n' > "$1/data/sub/b.csv" ;;
  esac
}

failed=0
check() { # check <name> <script> <case> <steps separated by |, args space-separated>
  local name=$1 script=$2 case=$3 steps=$4
  "$cc" build "$src/$script" -o "$progs/$name" --runtime "$rt" >/dev/null
  local ref="$tmp/ref-$name" box="$tmp/box-$name"
  lay_out "$ref" "$case"; lay_out "$box" "$case"
  local i=0 IFS='|'
  for step in $steps; do
    i=$((i+1))
    IFS=' ' read -r -a args <<< "$step"
    (cd "$ref" && env -i PATH=/usr/bin:/bin CIGSCRIPT_HOME="$ref/.home" NO_COLOR=1 CIG_DOCTOR=0 CIG_SEAL_KEY=file CIG_REPORTS=never \
      "$rt" run "$src/$script" -- "${args[@]}" >"$tmp/r.out" 2>"$tmp/r.err"; echo $? >"$tmp/r.code") || true
    docker run --rm --network none --user "$(id -u):$(id -g)" --tmpfs /tmp \
      -v "$progs:/p:ro" -v "$box:/work" -w /work \
      -e CIGSCRIPT_HOME=/work/.home -e NO_COLOR=1 -e CIG_DOCTOR=0 -e CIG_SEAL_KEY=file -e CIG_REPORTS=never \
      cigcompile-empty "/p/$name" -- "${args[@]}" >"$tmp/b.out" 2>"$tmp/b.err" && echo 0 >"$tmp/b.code" || echo $? >"$tmp/b.code"
    # The interpreter's errors name the script by the path it was given;
    # the program by the path it was compiled from: the same here.
    for s in out err code; do
      if ! diff <(normal <"$tmp/r.$s") <(normal <"$tmp/b.$s") >"$tmp/d"; then
        echo "DIFFERS  $name step $i ($s):"; cat "$tmp/d"; failed=1
      fi
    done
  done
  if [ "$failed" = 0 ]; then echo "same     $name (in an empty container)"; fi
}

check hello examples/hello.cig none " |Ada"
check tidy examples/tidy-downloads.cig tidy "downloads"
check report examples/report.cig report "expenses.csv report.json"
check measure examples/lab-measurements.cig measure "readings.csv report.json"
check manifest examples/lab-manifest.cig manifest "data|data verify"
check usage examples/tidy-downloads.cig none " "
exit "$failed"
