// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! The sha256 of every runtime this compiler accepts from a release: cig
//! v1.16.1's assets, as its release published them. Written by
//! `ci/digests.sh v1.16.1`; the version below must be the one Cargo.toml pins
//! (a test holds it to `cigscript::VERSION`).

/// The cig version these are for.
pub const VERSION: &str = "1.16.1";

/// Asset name and sha256, hex.
pub const DIGESTS: &[(&str, &str)] = &[
    (
        "cig-v1.16.1-freebsd-x86_64",
        "de5f60ff6c9e7cb0992889ad228ab2dc5d0c8f9461f965821a9df27e2ab06557",
    ),
    (
        "cig-v1.16.1-linux-aarch64",
        "301b5a82e31d777b12bcf3054a579c092674bf46997d9c3f7ceff0e79a7ecec2",
    ),
    (
        "cig-v1.16.1-linux-arm",
        "bbbfe600acc33aab67123182897d48e83ca0ed2f38982dfd7be6f023319697ed",
    ),
    (
        "cig-v1.16.1-linux-x86_64",
        "7c08355270ba1eef68591912193bb10a90dc4a8f3ea3aeceee1fa26c9f8ac775",
    ),
    (
        "cig-v1.16.1-macos-aarch64",
        "dad4128415f14b790aef1e5c686850967d90f280f1d8f783341d8693f2605983",
    ),
    (
        "cig-v1.16.1-macos-x86_64",
        "aa172467f3aa22a48ddd58f0a4d6f655370ffadd17a3de4f2fd41002d3c68c40",
    ),
    (
        "cig-v1.16.1-windows-aarch64.exe",
        "17fdc2cf5b33ea6b75415527df1e88376486b7a2400325a1ad16d82fe9ce670b",
    ),
    (
        "cig-v1.16.1-windows-x86_64.exe",
        "9089dc7ee2bb3325ae1d7eb2b2f072fa774d0e4d99140e02f29a314f0624eece",
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
