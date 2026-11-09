// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! The sha256 of every runtime this compiler accepts from a release: cig
//! v1.20.0's assets, as its release published them. Written by
//! `ci/digests.sh v1.20.0`; the version below must be the one Cargo.toml pins
//! (a test holds it to `cigscript::VERSION`).

/// The cig version these are for.
pub const VERSION: &str = "1.20.0";

/// Asset name and sha256, hex.
pub const DIGESTS: &[(&str, &str)] = &[
    (
        "cig-v1.20.0-freebsd-x86_64",
        "d4b8f9b6bcc845de7653d4601c008d206d13b937265ab787ee82c5d94950b30b",
    ),
    (
        "cig-v1.20.0-linux-aarch64",
        "8b5c3d99fa589a41caf88057a702ed02c147afa9787cbfb58dfaa24b1922b255",
    ),
    (
        "cig-v1.20.0-linux-arm",
        "c44fc87e0b9a1a161bfb61ddb39bb3adf4060c794ba74d88424f71dbbee4bd4f",
    ),
    (
        "cig-v1.20.0-linux-x86_64",
        "4649fda09236ada8a2ade2460c33db9cfa4b2f4dba40186099118a6a3b2dc8e3",
    ),
    (
        "cig-v1.20.0-macos-aarch64",
        "ec803067ef2db62efc07f581acd9b8c34a948dda886758cbbe6ba4cbc8edd0d2",
    ),
    (
        "cig-v1.20.0-macos-x86_64",
        "2775d0cf06b0cf95d2dd9b7e501bc068d1df3c479c9828e1aab64fc9d49e7fc4",
    ),
    (
        "cig-v1.20.0-windows-aarch64.exe",
        "92135341a19594e8ea3cf98b50f0e153de8f4f39acb7a586202d7944f7fc7684",
    ),
    (
        "cig-v1.20.0-windows-x86_64.exe",
        "c919995c76a3a5396cb88817713796b192fd0ed5f83e994e2a749f1e76a6f7c1",
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
