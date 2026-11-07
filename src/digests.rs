// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! The sha256 of every runtime this compiler accepts from a release: cig
//! v1.15.1's assets, as its release published them. Written by
//! `ci/digests.sh v1.15.1`; the version below must be the one Cargo.toml pins
//! (a test holds it to `cigscript::VERSION`).

/// The cig version these are for.
pub const VERSION: &str = "1.15.1";

/// Asset name and sha256, hex.
pub const DIGESTS: &[(&str, &str)] = &[
    (
        "cig-v1.15.1-freebsd-x86_64",
        "d4c4bf8f30adfc4f770e651ff0f5a9dbea6fe781cb8af43841be35fe55a05852",
    ),
    (
        "cig-v1.15.1-linux-aarch64",
        "c346eaf2e38171662bde4a591db007f36604eac5f814f4a2e740a68cf950fbfd",
    ),
    (
        "cig-v1.15.1-linux-arm",
        "55950f2a1c6178aca7de30e69abb3da231dc5f9783ce6d70fc9c0bdc5b044db3",
    ),
    (
        "cig-v1.15.1-linux-x86_64",
        "991a282f362fa40424cb216f0c1c4285ad193279ba36f4844c8d8ebd62445eef",
    ),
    (
        "cig-v1.15.1-macos-aarch64",
        "51eb2f8a2c755f419fc4759c27e3a79c73f48619495344d0743d5a2c298d109e",
    ),
    (
        "cig-v1.15.1-macos-x86_64",
        "1a3e0b3f246ea0bb537517f08297a8d0262cc63ebbfbae69676936ad9a5e9b2e",
    ),
    (
        "cig-v1.15.1-windows-aarch64.exe",
        "fcb5208a5a7ed643fa829f566f802224eb2fb8ddf1b52579d51f6ef0a6e9710e",
    ),
    (
        "cig-v1.15.1-windows-x86_64.exe",
        "c4870fc80b199ff8602594ef260ba2bee6a2950aef1ce844dcb3dd5466b0ecc3",
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
