#!/usr/bin/env python3
"""size-by-crate.py BINARY: what a cig binary's code is made of, from its symbol table
(`nm -S --size-sort -C --defined-only`): bytes of code by crate, by module of `cigscript`,
and the largest functions. Reads only; needs binutils.

A symbol is counted for the crate its name starts with. `<T as Trait>::f` counts for T's
crate, and `core::ptr::drop_in_place<T>` for T's. Generic code is therefore counted where it
is named, which is not always where it is used: a `HashMap` method instantiated for a cig
type counts for `hashbrown`. C code (the C library, zstd, hand-written assembly) is grouped
by the prefix of its names."""
import re
import subprocess
import sys

binary = sys.argv[1]
out = subprocess.run(["nm", "-S", "--size-sort", "-C", "--defined-only", binary],
                     capture_output=True, text=True, check=True).stdout
sections = {}
for line in subprocess.run(["size", "-A", binary], capture_output=True, text=True, check=True).stdout.split("\n")[2:]:
    p = line.split()
    if len(p) >= 2 and p[1].isdigit():
        sections[p[0]] = int(p[1])

LEAD = re.compile(r"^(?:<|&|\*const |\*mut |mut |dyn |\[|\(|impl )+")
C_GROUPS = (("zstd (C)", ("ZSTD", "HUF_", "FSE_", "XXH", "ZDICT", "COVER_", "HIST_", "POOL_", "ERR_", "divsufsort", "FASTCOVER")),
            ("blake3 (C and assembly)", ("blake3", "_blake3")),
            ("ring and other assembly", ("ring_core", "aes", "gcm", "chacha", "poly1305", "sha256_block", "sha512_block", "bn_", "ecp_", "x25519", "OPENSSL", "CRYPTO", "p256", "p384", "LIMBS", "curve25519")))


def owner(name):
    n = name
    m = re.match(r"^core::ptr::drop_in_place<(.+)>$", n)
    if m:
        n = m.group(1)
    n = LEAD.sub("", n)
    if "::" not in n.split("<")[0].split(" ")[0]:
        for group, prefixes in C_GROUPS:
            if name.startswith(prefixes):
                return group, None
        return "the C library and other unprefixed symbols", None
    path = n.split("<")[0].split(" ")[0].split("::")
    crate = path[0]
    module = path[1] if crate == "cigscript" and len(path) > 2 else None
    if crate == "cigscript" and module is None:
        module = "(the crate's root)"
    return crate, module


code, data, mods, funcs = {}, {}, {}, []
for line in out.split("\n"):
    p = line.split(None, 3)
    if len(p) < 4:
        continue
    size, kind, name = int(p[1], 16), p[2], p[3]
    crate, module = owner(name)
    if kind in "Tt":
        code[crate] = code.get(crate, 0) + size
        funcs.append((size, name))
        if module:
            mods[module] = mods.get(module, 0) + size
    else:
        data[crate] = data.get(crate, 0) + size

text = sections.get(".text", 0)
named = sum(code.values())
print(f"`.text` is {text:,} bytes; the symbol table names {named:,} of them ({100 * named / text:.1f}%).\n")
print("| section | bytes |\n|---|---:|")
for s, n in sorted(sections.items(), key=lambda x: -x[1])[:10]:
    print(f"| `{s}` | {n:,} |")
print("\n| crate | code bytes | of the named code (%) |\n|---|---:|---:|")
rest = 0
for i, (c, n) in enumerate(sorted(code.items(), key=lambda x: -x[1])):
    if i < 30:
        print(f"| `{c}` | {n:,} | {100 * n / named:.1f} |")
    else:
        rest += n
print(f"| the other {max(0, len(code) - 30)} | {rest:,} | {100 * rest / named:.1f} |")
print("\n| module of `cigscript` | code bytes | of `cigscript` (%) |\n|---|---:|---:|")
total = sum(mods.values())
for m, n in sorted(mods.items(), key=lambda x: -x[1]):
    print(f"| `{m}` | {n:,} | {100 * n / total:.1f} |")
print("\n| the 25 largest functions | bytes |\n|---|---:|")
for size, name in sorted(funcs, reverse=True)[:25]:
    print(f"| `{name[:110]}` | {size:,} |")
