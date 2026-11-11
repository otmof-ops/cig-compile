#!/usr/bin/env sh
# SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
# SPDX-License-Identifier: FSL-1.1-ALv2
# A generated CigScript of N lines for the passes group (#68): constants to
# fold, copies to propagate and copies of them, an `exhale` to a unit of three lines, and
# `exhale 0` to fill N lines exactly.
# Nothing depends on the clock or the machine, so the file is the same bytes
# everywhere. Usage:
#
#     gen-script.sh N OUT
set -eu
n="${1:?usage: gen-script.sh N OUT}"
out="${2:?usage: gen-script.sh N OUT}"
mkdir -p "$(dirname "$out")"
awk -v n="$n" 'BEGIN {
    for (i = 1; i + 2 <= n; i += 3) {
        printf "stick c%d = %d * 2 + 3\n", i, i
        printf "stick d%d = c%d\n", i, i
        printf "exhale d%d\n", i
        lines += 3
    }
    while (lines < n) { print "exhale 0"; lines++ }
}' > "$out"
