#!/usr/bin/env sh
# SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
# SPDX-License-Identifier: FSL-1.1-ALv2
# Build one program of the set for the compiled-against-interpreted cases, and
# prove both sides run the same runtime (#68): the program is built on the `cig`
# that is first on the PATH, and `cigcompile verify` must find that cig's exact
# bytes in the front of the program. So `cig run NAME.cig` and the compiled
# program differ by the compiler's work and nothing else, and a runtime that
# differs is a refusal (exit 3), never a quiet comparison of two cigs. Usage:
#
#     prep-program.sh NAME OUTDIR
set -eu
name="${1:?usage: prep-program.sh NAME OUTDIR}"
out="${2:?usage: prep-program.sh NAME OUTDIR}"
src="generated/set/$name.cig"
rt="$(command -v cig)"
mkdir -p "$out"
cigcompile build "$src" --runtime "$rt" -o "$out/$name" > /dev/null
if ! cigcompile verify "$out/$name" --script "$src" --runtime "$rt" > /dev/null; then
    echo "prep-program.sh: $name does not carry the bytes of $rt" >&2
    exit 3
fi
