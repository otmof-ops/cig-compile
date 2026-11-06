// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! The sha256 of every runtime this compiler accepts from a release: cig
//! v1.15.0's assets, as its release published them. Written by
//! `ci/digests.sh v1.15.0`; the version below must be the one Cargo.toml pins
//! (a test holds it to `cigscript::VERSION`).

/// The cig version these are for.
pub const VERSION: &str = "1.15.0";

/// Asset name and sha256, hex.
pub const DIGESTS: &[(&str, &str)] = &[
    (
        "cig-v1.15.0-freebsd-x86_64",
        "93d6941aa2353e58cdf15dfa6395d89ba3b7147fd68ea9185a378e7dbc93d91c",
    ),
    (
        "cig-v1.15.0-linux-aarch64",
        "4fb0c88627a568bf07ce9e56ce8b9cf48851b9216acb00cb7c952deaee7a88e4",
    ),
    (
        "cig-v1.15.0-linux-arm",
        "59131aaf8b6bd324d72f196bb57b2bb6426512fa629df52c527001302be89213",
    ),
    (
        "cig-v1.15.0-linux-x86_64",
        "243d8f4b67f644ff84a1aa4ad623c5bcd8664254d6774f00ea3c0bf288fb2fd7",
    ),
    (
        "cig-v1.15.0-macos-aarch64",
        "bc74cd89585d6094ee2c829e47ddd92af1956b3de47ec6bd5a12fe2a878f974d",
    ),
    (
        "cig-v1.15.0-macos-x86_64",
        "9c54461c5cc13f8251116cc2a64a333f9adc2ca6c98387a5fb61d86aa576ef06",
    ),
    (
        "cig-v1.15.0-windows-aarch64.exe",
        "38cff2dda31f68999f5f9d9c18df782828804c08edae988fd76195f740ffc99b",
    ),
    (
        "cig-v1.15.0-windows-x86_64.exe",
        "5c8ca624c37a498463a76377d2a4e5510e3293900afaa3e4df8aa058f5f3c293",
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
