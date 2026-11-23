// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! The sha256 of every runtime this compiler accepts from a release: cig
//! v1.25.0's assets, as its release published them. Written by
//! `ci/digests.sh v1.25.0`; the version below must be the one Cargo.toml pins
//! (a test holds it to `cigscript::VERSION`).

/// The cig version these are for.
pub const VERSION: &str = "1.25.0";

/// Asset name and sha256, hex.
pub const DIGESTS: &[(&str, &str)] = &[
    (
        "cig-v1.25.0-freebsd-x86_64",
        "d02c0206d25d7e79de35642b7bd4052100ee85c72d3637099614d97fa7b94905",
    ),
    (
        "cig-v1.25.0-linux-aarch64",
        "a1f3487fe6f8ca7c4c21525e59a2975dd3f603b2164d1b7ae1ea738f47a56ddc",
    ),
    (
        "cig-v1.25.0-linux-arm",
        "e145f8e2ba8a8d6e9773189c5e36ce90ca4cec3fbcb61631fc72517b38bebc8f",
    ),
    (
        "cig-v1.25.0-linux-x86_64",
        "423d648340fd0db8fd541319f5dec3f1adf0329a387ebdf5d98c064232a90e46",
    ),
    (
        "cig-v1.25.0-macos-aarch64",
        "5920132f6dd38cf1c7cceb89c3c98c06db8d2ec874843e29ba58065378d81abd",
    ),
    (
        "cig-v1.25.0-macos-x86_64",
        "aa75cbc960c393092c7536983597f8a2d550018d6e7755d666459e2476b5b981",
    ),
    (
        "cig-v1.25.0-windows-aarch64.exe",
        "0c447ff62c317d1da6f7733066fb6bc5d59b17e9955a1522c05e7764d5a22dda",
    ),
    (
        "cig-v1.25.0-windows-x86_64.exe",
        "5b0ffa7540c7a52666fc2da44f1d1442f8a923c79a9adcc8422f0eccfb97e9f4",
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
