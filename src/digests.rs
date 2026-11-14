// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! The sha256 of every runtime this compiler accepts from a release: cig
//! v1.22.0's assets, as its release published them. Written by
//! `ci/digests.sh v1.22.0`; the version below must be the one Cargo.toml pins
//! (a test holds it to `cigscript::VERSION`).

/// The cig version these are for.
pub const VERSION: &str = "1.22.0";

/// Asset name and sha256, hex.
pub const DIGESTS: &[(&str, &str)] = &[
    (
        "cig-v1.22.0-freebsd-x86_64",
        "a62199bd123866e1415245c7c89951df58089c3748830785365b9b4d8003b164",
    ),
    (
        "cig-v1.22.0-linux-aarch64",
        "bf8c022a1ef2cd6a9e123a12717c9bfe01cddf1170c880307bd0f61de8a3d0c3",
    ),
    (
        "cig-v1.22.0-linux-arm",
        "4c17d4e508af573eca851a0fc619bd40219cda75e716be8692cf3400d90ed54a",
    ),
    (
        "cig-v1.22.0-linux-x86_64",
        "588076a2e4ef684113589942ac0c7f69f74b07361da04ccd09427fa617c04abe",
    ),
    (
        "cig-v1.22.0-macos-aarch64",
        "099178322690ad8aaa38c78dccd8487c50920ebc1488d33b8f4b3b677249878a",
    ),
    (
        "cig-v1.22.0-macos-x86_64",
        "7f97f9c56d9610f2e633e88ee7000c9cd2e3a53b4c6c70db7983b36d3c64e994",
    ),
    (
        "cig-v1.22.0-windows-aarch64.exe",
        "d58f32a56f61a91f67993e396815c9e01a9a2010d06ce5d494ee1d5b53a55ad6",
    ),
    (
        "cig-v1.22.0-windows-x86_64.exe",
        "f4eb5fcd7e9e9276db675108ce3e5f62429d6cae38913d595a1187c9b6cf97c3",
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
