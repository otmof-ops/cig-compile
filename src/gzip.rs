// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! gzip for `--emit deb`'s tars, through flate2, which the runtime already
//! compiles in. The header carries no time and no name and names no
//! operating system, so the same build of cigcompile writes the same bytes;
//! a later cigcompile may compress differently, as any compiler may.

use flate2::{Compression, GzBuilder};
use std::io::Write;

/// `data` as a gzip file, compressed at the best level.
pub fn compress(data: &[u8]) -> Vec<u8> {
    let mut gz = GzBuilder::new()
        .mtime(0)
        .operating_system(255)
        .write(Vec::with_capacity(data.len() / 2), Compression::best());
    gz.write_all(data).expect("writing to memory");
    gz.finish().expect("writing to memory")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn a_gzip_file_round_trips_and_is_reproducible() {
        let data: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let gz = compress(&data);
        assert_eq!(&gz[..3], &[0x1f, 0x8b, 8], "a gzip header, deflate");
        assert_eq!(&gz[4..8], &[0, 0, 0, 0], "no time in the header");
        assert_eq!(gz[9], 255, "no operating system named");
        assert!(gz.len() < data.len() / 4, "it compresses: {}", gz.len());
        let mut back = Vec::new();
        flate2::read::GzDecoder::new(&gz[..])
            .read_to_end(&mut back)
            .unwrap();
        assert_eq!(back, data);
        assert_eq!(compress(&data), gz, "the same bytes every time");
    }

    #[test]
    fn empty_input_is_a_valid_gzip_file() {
        let gz = compress(&[]);
        let mut back = Vec::new();
        flate2::read::GzDecoder::new(&gz[..])
            .read_to_end(&mut back)
            .unwrap();
        assert!(back.is_empty());
    }
}
