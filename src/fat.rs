// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! A universal macOS binary (#10): the arm64 and x86_64 runtimes in one
//! file, as lipo writes it. A big-endian fat header and its table come
//! first, then each thin Mach-O at an offset aligned to 16 KiB. Each slice
//! keeps its own code signature, which covers that slice alone. A program's
//! payload goes after the last slice, where no signature reaches, as it
//! does after a thin binary; cig reads it from the end of its own file.

const FAT_MAGIC: u32 = 0xcafe_babe;
const MH_MAGIC_64: u32 = 0xfeed_facf;
/// 2^14: the page size of arm64 macOS, and what lipo uses for it.
const ALIGN: u32 = 14;

/// One slice in a fat binary's table.
#[derive(Debug, PartialEq, Eq)]
pub struct Slice {
    pub cputype: u32,
    pub cpusubtype: u32,
    pub offset: u32,
    pub size: u32,
    pub align: u32,
}

fn le(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn be(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn round_up(n: usize, to: usize) -> usize {
    n.div_ceil(to) * to
}

/// The fat binary holding these thin 64-bit Mach-O files, in this order.
/// The same slices always give the same bytes.
pub fn universal(thin: &[&[u8]]) -> Result<Vec<u8>, String> {
    let mut types = Vec::new();
    for (k, s) in thin.iter().enumerate() {
        if le(s, 0) != Some(MH_MAGIC_64) {
            return Err(format!("part {} is not a 64-bit Mach-O executable", k + 1));
        }
        let cputype = le(s, 4).unwrap_or(0);
        if types.iter().any(|(t, _)| *t == cputype) {
            return Err(format!("two parts are for the same CPU ({cputype:#x})"));
        }
        types.push((cputype, le(s, 8).unwrap_or(0)));
    }
    if thin.len() < 2 {
        return Err("a universal binary holds two or more parts".to_string());
    }
    let page = 1usize << ALIGN;
    let mut offset = round_up(8 + 20 * thin.len(), page);
    let mut table = Vec::new();
    for (s, (cputype, cpusubtype)) in thin.iter().zip(&types) {
        let (Ok(at), Ok(size)) = (u32::try_from(offset), u32::try_from(s.len())) else {
            return Err("a universal binary is limited to 4 GB".to_string());
        };
        table.push(Slice {
            cputype: *cputype,
            cpusubtype: *cpusubtype,
            offset: at,
            size,
            align: ALIGN,
        });
        offset = round_up(offset + s.len(), page);
    }
    let mut out = Vec::with_capacity(offset);
    out.extend(FAT_MAGIC.to_be_bytes());
    out.extend((thin.len() as u32).to_be_bytes());
    for t in &table {
        for v in [t.cputype, t.cpusubtype, t.offset, t.size, t.align] {
            out.extend(v.to_be_bytes());
        }
    }
    for (t, s) in table.iter().zip(thin) {
        out.resize(t.offset as usize, 0);
        out.extend_from_slice(s);
    }
    Ok(out)
}

/// The table of a fat binary, or None when `bytes` is not one.
pub fn slices(bytes: &[u8]) -> Option<Vec<Slice>> {
    if be(bytes, 0)? != FAT_MAGIC {
        return None;
    }
    let n = be(bytes, 4)? as usize;
    (0..n)
        .map(|k| {
            let at = 8 + 20 * k;
            Some(Slice {
                cputype: be(bytes, at)?,
                cpusubtype: be(bytes, at + 4)?,
                offset: be(bytes, at + 8)?,
                size: be(bytes, at + 12)?,
                align: be(bytes, at + 16)?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in thin Mach-O: the magic, the CPU type and subtype, then
    /// filler that says which it is.
    fn thin(cputype: u32, cpusubtype: u32, len: usize) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend(MH_MAGIC_64.to_le_bytes());
        b.extend(cputype.to_le_bytes());
        b.extend(cpusubtype.to_le_bytes());
        b.resize(len, cputype as u8);
        b
    }

    const ARM64: u32 = 0x0100_000c;
    const X86_64: u32 = 0x0100_0007;

    #[test]
    fn the_slices_sit_whole_at_page_aligned_offsets_in_order() {
        let a = thin(ARM64, 0, 40_000);
        let x = thin(X86_64, 0x8000_0003, 30_001);
        let fat = universal(&[&a, &x]).unwrap();
        let t = slices(&fat).unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!((t[0].cputype, t[0].cpusubtype), (ARM64, 0));
        assert_eq!((t[1].cputype, t[1].cpusubtype), (X86_64, 0x8000_0003));
        for (s, want) in t.iter().zip([&a, &x]) {
            assert_eq!(s.offset % (1 << 14), 0);
            assert_eq!(s.align, 14);
            let at = s.offset as usize;
            assert_eq!(&fat[at..at + s.size as usize], want.as_slice());
        }
        assert!(t[1].offset as usize >= t[0].offset as usize + a.len());
        assert_eq!(
            fat.len(),
            t[1].offset as usize + x.len(),
            "nothing after the last slice"
        );
        assert_eq!(
            fat,
            universal(&[&a, &x]).unwrap(),
            "the same parts, the same bytes"
        );
    }

    #[test]
    fn what_is_not_two_thin_mach_o_files_for_two_cpus_is_refused() {
        let a = thin(ARM64, 0, 100);
        assert!(universal(&[&a]).is_err());
        assert!(universal(&[&a, &thin(ARM64, 0, 200)]).is_err());
        assert!(universal(&[&a, b"\x7fELF and the rest"]).is_err());
        assert!(slices(&a).is_none());
    }
}
