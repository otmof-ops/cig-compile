// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! The sha256 of every runtime this compiler accepts from a release: cig
//! v1.24.0's assets, as its release published them. Written by
//! `ci/digests.sh v1.24.0`; the version below must be the one Cargo.toml pins
//! (a test holds it to `cigscript::VERSION`).

/// The cig version these are for.
pub const VERSION: &str = "1.24.0";

/// Asset name and sha256, hex.
pub const DIGESTS: &[(&str, &str)] = &[
    (
        "cig-v1.24.0-freebsd-x86_64",
        "1a718cd9e70b48e8754f6583d35447bb225b5c2ce05a0012409276c3c4ffd1a7",
    ),
    (
        "cig-v1.24.0-linux-aarch64",
        "31e0f07e4c3a6830d0a826146592d70da92a5d49f110efe72242d1bb8a47ce9b",
    ),
    (
        "cig-v1.24.0-linux-arm",
        "d373739f881debe8c9cc2cd5345f40d66058c1db3c0a674f51a3bc2fff2e024b",
    ),
    (
        "cig-v1.24.0-linux-x86_64",
        "957ddff2c27c9baa38b5aa43361e165e209ebb3d6a3c82cfa28d7b7777e91b3f",
    ),
    (
        "cig-v1.24.0-macos-aarch64",
        "15b92e1549a7b0b9ae658780f9383211ab8df76b85548d8e4c42246205a9c42e",
    ),
    (
        "cig-v1.24.0-macos-x86_64",
        "dbf822ead630851d179222694ced9e24fac47883681e12e9622562f489d91f48",
    ),
    (
        "cig-v1.24.0-windows-aarch64.exe",
        "8bc67b5cadf6140a656e7cbf8a0c98cdb0ef5c6a9e2d9a0daf998e2de1c524bf",
    ),
    (
        "cig-v1.24.0-windows-x86_64.exe",
        "96cd595cc3fb1b8f0639ec6436c49be9de8472e8a2582a10e6da061cb8fff45c",
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
