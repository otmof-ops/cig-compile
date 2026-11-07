// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! The sha256 of every runtime this compiler accepts from a release: cig
//! v1.16.0's assets, as its release published them. Written by
//! `ci/digests.sh v1.16.0`; the version below must be the one Cargo.toml pins
//! (a test holds it to `cigscript::VERSION`).

/// The cig version these are for.
pub const VERSION: &str = "1.16.0";

/// Asset name and sha256, hex.
pub const DIGESTS: &[(&str, &str)] = &[
    (
        "cig-v1.16.0-freebsd-x86_64",
        "02bc03877e3e00e3d90ffb7a558b8a69f992c2d57ad2f838dbec5263d4424395",
    ),
    (
        "cig-v1.16.0-linux-aarch64",
        "0aea153bcd023587494d7d88a0536c28df0efaa07cda077de892e2c7393ee6b2",
    ),
    (
        "cig-v1.16.0-linux-arm",
        "caf38eeb5fce49f68fc7749c4d38e03d2fe96e23bb98352cb0016a5c09e0a974",
    ),
    (
        "cig-v1.16.0-linux-x86_64",
        "8f0ea446c165f0028f3683b31be4b1b3de5db347a74b230bb49ee19f15579d9e",
    ),
    (
        "cig-v1.16.0-macos-aarch64",
        "9eddfe01097fcbff611ddf4c0eec9b10d76ef4a673e53a37aad60342e7ae0fb2",
    ),
    (
        "cig-v1.16.0-macos-x86_64",
        "b18e112bc3eb76d728df96b200b7802ca65e3d84b3fa968ab31efe90e9d0ec20",
    ),
    (
        "cig-v1.16.0-windows-aarch64.exe",
        "642f3bc25f9faa980d51dbe6d2eaff93a5c4b56c3c758024b9126b2d2e5d2d8f",
    ),
    (
        "cig-v1.16.0-windows-x86_64.exe",
        "ce3294ccaecd54016f052504fca01baeec067a2b740d84473851c7ad9ecb20e5",
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
