// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! The sha256 of every runtime this compiler accepts from a release: cig
//! v1.21.0's assets, as its release published them. Written by
//! `ci/digests.sh v1.21.0`; the version below must be the one Cargo.toml pins
//! (a test holds it to `cigscript::VERSION`).

/// The cig version these are for.
pub const VERSION: &str = "1.21.0";

/// Asset name and sha256, hex.
pub const DIGESTS: &[(&str, &str)] = &[
    (
        "cig-v1.21.0-freebsd-x86_64",
        "0c71870a83e48a656b46ceb43f147ed7cf4754d960a4e9c17464b65e8ca90ff6",
    ),
    (
        "cig-v1.21.0-linux-aarch64",
        "6a767256c202cda687b3365cf59eb4260d46caa42d6b1a51247b86181af07d1d",
    ),
    (
        "cig-v1.21.0-linux-arm",
        "792bf2573417f62b7066e38afe9852e452685f51272e0d436e4425f239441083",
    ),
    (
        "cig-v1.21.0-linux-x86_64",
        "46bf00f607c63dc5b3df04654282328c083df0ff388b9f44aef2b658fa9501a4",
    ),
    (
        "cig-v1.21.0-macos-aarch64",
        "24904f03ae2e34d9ec838ddc236729f9faa234c3ed6c2b5163f81eb2e0a846dc",
    ),
    (
        "cig-v1.21.0-macos-x86_64",
        "cf7736ce62a10677dfa333bd4e86f5b00487bc8b35941d7470e3d848546c36c1",
    ),
    (
        "cig-v1.21.0-windows-aarch64.exe",
        "08423952131d7c6a55f5acb439be37b60691eab6657158596aaca2f56123d10c",
    ),
    (
        "cig-v1.21.0-windows-x86_64.exe",
        "1f2d0b5604bfefb2b4dfed24f4503234a19b019a791d50912d224d6dd9889da6",
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
