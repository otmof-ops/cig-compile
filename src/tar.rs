// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! A minimal ustar tar writer shared by `--emit oci` (src/oci.rs) and
//! `--emit deb` (src/deb.rs). Every time is zero, every owner root, and
//! entries are written in the order they are given, so the same build writes
//! the same bytes.

/// One entry in a tar: a ustar header, the bytes, padding to 512. `kind` is
/// `b'0'` for a file and `b'5'` for a directory (whose data is empty).
pub fn entry(out: &mut Vec<u8>, name: &str, mode: u32, kind: u8, data: &[u8]) {
    let mut h = [0u8; 512];
    let n = name.as_bytes();
    assert!(n.len() < 100, "a short path: {name}");
    h[..n.len()].copy_from_slice(n);
    let mut octal = |at: usize, width: usize, v: u64| {
        let s = format!("{v:0w$o}", w = width - 1);
        h[at..at + width - 1].copy_from_slice(s.as_bytes());
    };
    octal(100, 8, u64::from(mode));
    octal(108, 8, 0);
    octal(116, 8, 0);
    octal(124, 12, data.len() as u64);
    octal(136, 12, 0);
    h[156] = kind;
    h[257..263].copy_from_slice(b"ustar\0");
    h[263..265].copy_from_slice(b"00");
    // The checksum is the header's bytes with its own field as spaces.
    h[148..156].copy_from_slice(b"        ");
    let sum: u32 = h.iter().map(|b| u32::from(*b)).sum();
    let s = format!("{sum:06o}\0 ");
    h[148..156].copy_from_slice(s.as_bytes());
    out.extend_from_slice(&h);
    out.extend_from_slice(data);
    let pad = (512 - data.len() % 512) % 512;
    out.extend(std::iter::repeat_n(0u8, pad));
}

/// Two zero blocks at the end of a tar archive.
pub fn end(out: &mut Vec<u8>) {
    out.extend(std::iter::repeat_n(0u8, 1024));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A header's checksum is what tar reads it to be, and a file's bytes
    /// are padded to the block.
    #[test]
    fn a_tar_entry_is_well_formed() {
        let mut t = Vec::new();
        entry(&mut t, "hello", 0o755, b'0', b"abc");
        assert_eq!(t.len(), 1024);
        let h = &t[..512];
        let stored = u32::from_str_radix(std::str::from_utf8(&h[148..154]).unwrap(), 8).unwrap();
        let mut copy = h.to_vec();
        copy[148..156].copy_from_slice(b"        ");
        assert_eq!(stored, copy.iter().map(|b| u32::from(*b)).sum::<u32>());
        assert_eq!(&h[124..135], b"00000000003");
        assert_eq!(&t[512..515], b"abc");
    }
}
