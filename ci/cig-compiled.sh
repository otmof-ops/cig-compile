#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
# SPDX-License-Identifier: FSL-1.1-ALv2
# A `cig` that runs scripts as programs, for CigScript's portability probe
# (#7). `cig run [flags] <script> [-- args]` builds the script with
# cigcompile and runs the program with the same flags and arguments; every
# other command (unburn, runs, parcel, tin, ...) is the real cig's.
#
#   CIG=ci/cig-compiled.sh CIG_REAL=<cig-runtime> CIGCOMPILE=<cigcompile> bash probe.sh
set -u
real=${CIG_REAL:?CIG_REAL names the cig programs are appended to}
cc=${CIGCOMPILE:?CIGCOMPILE names cigcompile}
if [ "${1:-}" != run ]; then exec "$real" "$@"; fi
shift
flags=()
while [ $# -gt 0 ] && [ "${1#--}" != "$1" ] && [ "$1" != -- ]; do
  case "$1" in
    --max-steps) flags+=("$1" "$2"); shift 2 ;;
    *) flags+=("$1"); shift ;;
  esac
done
script=${1:?a script to run}
shift
# Programs and cigcompile's own index live outside the probe's folders and
# its CIGSCRIPT_HOME, so no scenario sees them.
work=${CIG_PROGRAMS:-${TMPDIR:-/tmp}/cig-compiled}
mkdir -p "$work"
prog="$work/$(basename "$script" .cig)-$$-$RANDOM"
case "$(uname -s)" in MINGW* | MSYS* | CYGWIN*) prog="$prog.exe" ;; esac
# The build says nothing unless it fails: a test reads the program's own
# output, and cigcompile's notes (a runtime linked against glibc) are not it.
built=$(CIGSCRIPT_HOME="$work/home" "$cc" build "$script" -o "$prog" --runtime "$real" 2>&1) || {
  code=$?
  printf '%s\n' "$built" >&2
  exit "$code"
}
exec "$prog" ${flags[@]+"${flags[@]}"} "$@"
