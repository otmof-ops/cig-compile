#!/usr/bin/env bash
# Write src/digests.rs from a CigScript release's published checksums: the
# runtimes cigcompile accepts, pinned in the compiler itself (#10).
#
#   bash ci/digests.sh v1.15.0 > src/digests.rs
set -euo pipefail
tag=${1:?usage: digests.sh <tag>}
version=${tag#v}
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
gh release download "$tag" -R otmof-ops/CigScript -p 'cig-*.sha256' -D "$tmp" >/dev/null
cat <<HEAD
//! The sha256 of every runtime this compiler accepts from a release: cig
//! ${tag}'s assets, as its release published them. Written by
//! \`ci/digests.sh ${tag}\`; the version below must be the one Cargo.toml pins
//! (a test holds it to \`cigscript::VERSION\`).

/// The cig version these are for.
pub const VERSION: &str = "${version}";

/// Asset name and sha256, hex.
pub const DIGESTS: &[(&str, &str)] = &[
HEAD
for f in $(ls "$tmp" | sort); do
  read -r sum name < "$tmp/$f"
  name=${name#\*}
  printf '    ("%s", "%s"),\n' "$name" "$sum"
done
cat <<'TAIL'
];

/// The pinned sha256 of an asset, if this compiler knows it.
pub fn of(asset: &str) -> Option<&'static str> {
    DIGESTS.iter().find(|(n, _)| *n == asset).map(|(_, d)| *d)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_digests_are_for_the_pinned_cig() {
        assert_eq!(super::VERSION, cigscript::VERSION, "run ci/digests.sh for the pinned tag");
        assert!(super::DIGESTS.len() >= 6);
        assert!(super::DIGESTS.iter().all(|(_, d)| d.len() == 64));
    }
}
TAIL
