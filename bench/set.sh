#!/usr/bin/env sh
# SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
# SPDX-License-Identifier: FSL-1.1-ALv2
# The program set, read from the cig this compiler pins (#68): the
# programs of cig's `bench/compute` and their expected output, copied into
# OUTDIR after every file's sha256 is held to `bench/set.sha256`, which
# records the pinned cig's own. A file that differs, or one that is missing, is
# a refusal (exit 3): a set that is not the pinned cig's would make the runtime
# and the programs disagree. Usage:
#
#     set.sh OUTDIR
#
# The set is read from $CIG_BENCH_SET, else $CIGSCRIPT_SRC/bench/compute, else
# the installer's layout beside `cig` (<prefix>/share/cig/bench/compute). The
# manifest's env sets CIGSCRIPT_SRC to `../../CigScript`, the sibling checkout
# where the family's CI puts the pinned cig.
set -eu
out="${1:?usage: set.sh OUTDIR}"
here="$(cd "$(dirname "$0")" && pwd)"
sums="${CIG_BENCH_SUMS:-$here/set.sha256}"
set_dir=""
for d in "${CIG_BENCH_SET:-}" "${CIGSCRIPT_SRC:-}/bench/compute"; do
    if [ -n "$d" ] && [ -d "$d" ]; then
        set_dir="$d"
        break
    fi
done
if [ -z "$set_dir" ]; then
    bin="$(command -v cig 2>/dev/null || true)"
    if [ -n "$bin" ]; then
        d="$(cd "$(dirname "$bin")/.." && pwd)/share/cig/bench/compute"
        [ -d "$d" ] && set_dir="$d"
    fi
fi
if [ -z "$set_dir" ]; then
    echo "set.sh: the pinned cig's bench/compute was not found (set CIG_BENCH_SET or CIGSCRIPT_SRC)" >&2
    exit 3
fi
# The file goes in on stdin: with a name, sha256sum escapes one holding a
# backslash (every Windows path) and the line starts with `\`, not the sum.
if command -v sha256sum >/dev/null 2>&1; then
    sum256() { sha256sum; }
else
    sum256() { shasum -a 256; }
fi
mkdir -p "$out"
status=0
while read -r want name; do
    [ -n "$want" ] || continue
    have=""
    if [ -f "$set_dir/$name" ]; then
        have="$(sum256 < "$set_dir/$name" | cut -d' ' -f1)"
    fi
    if [ "$have" != "$want" ]; then
        echo "set.sh: $name is not the pinned cig's (sha256 ${have:-missing}, want $want)" >&2
        status=3
        continue
    fi
    cp "$set_dir/$name" "$out/$name"
done < "$sums"
exit "$status"
