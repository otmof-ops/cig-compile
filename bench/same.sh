#!/usr/bin/env sh
# SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
# SPDX-License-Identifier: FSL-1.1-ALv2
# Run one program of the set, interpreted (`cig run`) or compiled, and hold its
# output to the set's own .expect file (#68): exit 0 only when it exits 0 and
# prints exactly that. The harness reads an `expect.stdout` file before a
# case's `setup` has made it, so the check is made here, the same way for both
# sides; the shell and the compare are in both columns. Usage:
#
#     same.sh interpreted|compiled NAME
set -eu
kind="${1:?usage: same.sh interpreted|compiled NAME}"
name="${2:?usage: same.sh interpreted|compiled NAME}"
out="$(mktemp)"
trap 'rm -f "$out"' EXIT
case "$kind" in
    interpreted) cig run "generated/set/$name.cig" > "$out" ;;
    compiled) "generated/prog/$name" > "$out" ;;
    *) echo "same.sh: interpreted or compiled" >&2; exit 3 ;;
esac
cmp -s "$out" "generated/set/$name.expect"
