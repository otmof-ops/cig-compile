// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! The sha256 of every runtime this compiler accepts from a release: cig
//! v1.23.0's assets, as its release published them. Written by
//! `ci/digests.sh v1.23.0`; the version below must be the one Cargo.toml pins
//! (a test holds it to `cigscript::VERSION`).

/// The cig version these are for.
pub const VERSION: &str = "1.23.0";

/// Asset name and sha256, hex.
pub const DIGESTS: &[(&str, &str)] = &[
    (
        "cig-v1.23.0-freebsd-x86_64",
        "6446eae845a6ad5d9d8650281d0714597f075e8c32d7c217db22c051e025576f",
    ),
    (
        "cig-v1.23.0-linux-aarch64",
        "903f8ae773e016fbe376e0c4ee06deb64e020982b39ecdb5dcfaa2eba7210112",
    ),
    (
        "cig-v1.23.0-linux-arm",
        "4e17f803130d3c4eb3789b4cd53b2ae04f72ed955c163af3f2a77b7134e6ee50",
    ),
    (
        "cig-v1.23.0-linux-x86_64",
        "adffb3c84cd61cb27a47205ae3189d05ef7b1142665a9fafa89a7e5f5fee180d",
    ),
    (
        "cig-v1.23.0-macos-aarch64",
        "8ae2c137d2023ffca30c98cb21d6c25e3da60455777bc6f56f314191f9ca5321",
    ),
    (
        "cig-v1.23.0-macos-x86_64",
        "74e491614ed8764c1e16db96d68365ccc80fad9e23b59e0f3dbb65d18cc0f062",
    ),
    (
        "cig-v1.23.0-windows-aarch64.exe",
        "bbf2ad5ade32b410a3a8486c88976eefc3f894dadc84547f24a2ec796d6d4765",
    ),
    (
        "cig-v1.23.0-windows-x86_64.exe",
        "7e5d25e4a866ddaf0528989bd925306791a2e4d1af579c85c2fab4121baa5e95",
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
