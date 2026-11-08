// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! The sha256 of every runtime this compiler accepts from a release: cig
//! v1.18.0's assets, as its release published them. Written by
//! `ci/digests.sh v1.18.0`; the version below must be the one Cargo.toml pins
//! (a test holds it to `cigscript::VERSION`).

/// The cig version these are for.
pub const VERSION: &str = "1.18.0";

/// Asset name and sha256, hex.
pub const DIGESTS: &[(&str, &str)] = &[
    (
        "cig-v1.18.0-freebsd-x86_64",
        "ed953956943b3d4f8934f378d31d1076b63d90b98618c10760769a162750fab2",
    ),
    (
        "cig-v1.18.0-linux-aarch64",
        "22f32f317960bcf8006c8b8ac13bc890fd9a6e972023b3b0ab2b27e85dd7d6f0",
    ),
    (
        "cig-v1.18.0-linux-arm",
        "7f5438727eaebff69d27a4dacb5d7924e8f43bf8543e8e1ba65c734b1fafcb16",
    ),
    (
        "cig-v1.18.0-linux-x86_64",
        "10077bd729b5d59c972f523ec9e724ef0af27e43095c74dd8cd4bb7cecf244bd",
    ),
    (
        "cig-v1.18.0-macos-aarch64",
        "e00872a534f7e048f8934f3c98666e531b817d6b3d5b16cf5abe7432fa30cfa2",
    ),
    (
        "cig-v1.18.0-macos-x86_64",
        "6dec79927e594101f4bf28e76a1c04ec7834a7c075d14df3b812705638d7f82c",
    ),
    (
        "cig-v1.18.0-windows-aarch64.exe",
        "97dfa90c3646839ad2f1adfe5e7122730ebfb869aac451d006a6be175659a64e",
    ),
    (
        "cig-v1.18.0-windows-x86_64.exe",
        "48f53d3c5732e1f2d3c8cc35685e92d528b6d6abd760060dae12275e4eb8bfd2",
    ),
];

/// The pinned sha256 of an asset, if this compiler knows it.
pub fn of(asset: &str) -> Option<&'static str> {
    DIGESTS.iter().find(|(n, _)| *n == asset).map(|(_, d)| *d)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_digests_are_for_the_pinned_cig() {
        assert_eq!(
            super::VERSION,
            cigscript::VERSION,
            "run ci/digests.sh for the pinned tag"
        );
        assert!(super::DIGESTS.len() >= 6);
        assert!(super::DIGESTS.iter().all(|(_, d)| d.len() == 64));
    }
}
