#!/usr/bin/env python3
"""flavor-patch.py CIG_SCRIPT_DIR [--revert]

Rewrites `Cargo.toml` and `src/main.rs` so mimalloc is gated by the `mi`
feature instead of `target_env = "musl"`. The same tree can then be built
four ways (gnu, gnu-mi, musl-mi, musl). No other file is touched.

The script is content-addressed (searches for exact strings), so it is
robust to line drift and idempotent: applying twice is a no-op, and
`--revert` puts the file back exactly as it was. On `apply` it writes
`*.pre-flavor` beside each file it changes (a byte-for-byte copy of the
input); on `--revert` it restores from that copy.
"""
import os, shutil, sys

ROOT = sys.argv[1]
REVERT = "--revert" in sys.argv[2:]

CARGO = os.path.join(ROOT, "Cargo.toml")
MAIN = os.path.join(ROOT, "src", "main.rs")

CARGO_OLD = """\
[target.'cfg(unix)'.dependencies]
libc = "0.2"
cig-vault = { path = "crates/cig-vault", version = "0.1.0" }

# The static musl build's own allocator makes every allocation-heavy script 2
# to 5x slower than the same code on glibc (cig-script#146), so musl alone
# links mimalloc. Every other target keeps the system allocator.
[target.'cfg(target_env = "musl")'.dependencies]
mimalloc = { version = "0.1", default-features = false }

[target.'cfg(windows)'.dependencies]
"""

CARGO_NEW = """\
[target.'cfg(unix)'.dependencies]
libc = "0.2"
cig-vault = { path = "crates/cig-vault", version = "0.1.0" }

# Probe only: mimalloc is a feature here, not bound to the target, so four
# flavors (gnu, gnu-mi, musl-mi, musl) build from one tree.
[target.'cfg(windows)'.dependencies]
"""

CARGO_OLD_DEPS = """\
[dependencies]
clap = { version = "4", features = ["derive"] }
"""

CARGO_NEW_DEPS = """\
[dependencies]
clap = { version = "4", features = ["derive"] }
mimalloc = { version = "0.1", default-features = false, optional = true }
"""

CARGO_OLD_FEATS = """\
[features]
# A counting allocator for `cig bench --stats` and `cig run --stats` (#179).
# Off by default and never on in a release.
bench-stats = []
"""

CARGO_NEW_FEATS = """\
[features]
# A counting allocator for `cig bench --stats` and `cig run --stats` (#179).
# Off by default and never on in a release.
bench-stats = []
# Probe only: mimalloc as the global allocator, with its arena, on any target.
mi = ["dep:mimalloc"]
"""

MAIN_SUBS = [
    ('#[cfg(all(target_env = "musl", not(feature = "bench-stats")))]',
     '#[cfg(all(feature = "mi", not(feature = "bench-stats")))]'),
    ('#[cfg(all(target_env = "musl", feature = "bench-stats"))]',
     '#[cfg(all(feature = "mi", feature = "bench-stats"))]'),
    ('#[cfg(all(not(target_env = "musl"), feature = "bench-stats"))]',
     '#[cfg(all(not(feature = "mi"), feature = "bench-stats"))]'),
    ('#[cfg(all(target_env = "musl", target_pointer_width = "64"))]',
     '#[cfg(all(feature = "mi", target_pointer_width = "64"))]'),
]

def apply(path, pairs):
    with open(path, encoding="utf-8") as f:
        text = f.read()
    changed = 0
    for old, new in pairs:
        if REVERT:
            old, new = new, old
        if old in text:
            text = text.replace(old, new, 1)
            changed += 1
        elif new in text:
            pass  # already applied / reverted
        else:
            print(f"ERROR in {path}: neither old nor new string present for {old[:60]!r}")
            sys.exit(2)
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)
    return changed

def backup(path):
    bak = path + ".pre-flavor"
    if not os.path.exists(bak):
        shutil.copy2(path, bak)

def restore(path):
    bak = path + ".pre-flavor"
    if os.path.exists(bak):
        shutil.copy2(bak, path)
        os.unlink(bak)

if REVERT:
    restore(CARGO)
    restore(MAIN)
    print("reverted")
    sys.exit(0)

backup(CARGO); backup(MAIN)

c = 0
c += apply(CARGO, [(CARGO_OLD, CARGO_NEW), (CARGO_OLD_DEPS, CARGO_NEW_DEPS), (CARGO_OLD_FEATS, CARGO_NEW_FEATS)])
c += apply(MAIN, MAIN_SUBS)
print(f"applied, {c} substitutions")
