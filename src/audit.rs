// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! `cigcompile audit <program>` (#40): the hardening a program carries,
//! read from its headers. A program is cig's runtime with a script
//! appended, so these are cig's own build's mitigations: PIE, RELRO and
//! BIND_NOW, a stack that is not executable, CET (IBT and shadow stack) or
//! AArch64's BTI and PAC, and on macOS and Windows a signature. Report only:
//! the exit is 0 whatever it finds, until cig's release turns them all on
//! (whether it may fail then is on otmof-ops/cig-script#153).

use crate::build::exit;
use cigscript::cli::Ctx;
use std::path::Path;

/// One mitigation: its name, whether it is there (`None`: does not apply
/// to this kind of program), and what was found.
pub struct Row {
    pub name: &'static str,
    pub on: Option<bool>,
    pub says: String,
}

fn row(name: &'static str, on: Option<bool>, says: impl Into<String>) -> Row {
    Row {
        name,
        on,
        says: says.into(),
    }
}

/// Little- or big-endian integers at an offset, `None` past the end.
struct Bytes<'a> {
    b: &'a [u8],
    big: bool,
}

impl Bytes<'_> {
    fn int(&self, at: usize, n: usize) -> Option<u64> {
        let s = self.b.get(at..at.checked_add(n)?)?;
        let f = |acc: u64, x: &u8| acc << 8 | u64::from(*x);
        Some(if self.big {
            s.iter().fold(0, f)
        } else {
            s.iter().rev().fold(0, f)
        })
    }
}

const PT_DYNAMIC: u64 = 2;
const PT_GNU_STACK: u64 = 0x6474_e551;
const PT_GNU_RELRO: u64 = 0x6474_e552;
const PT_GNU_PROPERTY: u64 = 0x6474_e553;

/// An ELF program's mitigations.
fn elf(b: &[u8]) -> Option<Vec<Row>> {
    let wide = *b.get(4)? == 2;
    let r = Bytes {
        b,
        big: *b.get(5)? == 2,
    };
    let e_type = r.int(16, 2)?;
    let machine = r.int(18, 2)?;
    let (phoff, phentsize, phnum) = if wide {
        (r.int(32, 8)?, r.int(54, 2)?, r.int(56, 2)?)
    } else {
        (r.int(28, 4)?, r.int(42, 2)?, r.int(44, 2)?)
    };
    let mut relro = false;
    let mut stack: Option<bool> = None;
    let mut dynamic: Option<(u64, u64)> = None;
    let mut property: Option<(u64, u64)> = None;
    for k in 0..phnum.min(512) {
        let at = usize::try_from(phoff + k * phentsize).ok()?;
        let p_type = r.int(at, 4)?;
        let (offset, filesz, flags) = if wide {
            (r.int(at + 8, 8)?, r.int(at + 32, 8)?, r.int(at + 4, 4)?)
        } else {
            (r.int(at + 4, 4)?, r.int(at + 16, 4)?, r.int(at + 24, 4)?)
        };
        match p_type {
            PT_GNU_RELRO => relro = true,
            PT_GNU_STACK => stack = Some(flags & 1 == 0),
            PT_DYNAMIC => dynamic = Some((offset, filesz)),
            PT_GNU_PROPERTY => property = Some((offset, filesz)),
            _ => {}
        }
    }
    // BIND_NOW: DT_BIND_NOW, DT_FLAGS's DF_BIND_NOW or DT_FLAGS_1's DF_1_NOW.
    let mut bind_now = false;
    if let Some((off, size)) = dynamic {
        let step = if wide { 16 } else { 8 };
        let half = step / 2;
        let mut at = usize::try_from(off).ok()?;
        let end = at.saturating_add(usize::try_from(size).ok()?);
        while at + step <= end {
            let tag = r.int(at, half)?;
            let val = r.int(at + half, half)?;
            match tag {
                0 => break,
                24 => bind_now = true,
                30 if val & 0x8 != 0 => bind_now = true,
                0x6fff_fffb if val & 0x1 != 0 => bind_now = true,
                _ => {}
            }
            at += step;
        }
    }
    // CET on x86-64 (IBT, SHSTK), BTI and PAC on AArch64: the GNU property
    // note's FEATURE_1_AND.
    let mut features: Option<u64> = None;
    if let Some((off, size)) = property {
        let mut at = usize::try_from(off).ok()?;
        let end = at.saturating_add(usize::try_from(size).ok()?);
        let align = if wide { 8 } else { 4 };
        while at + 12 <= end {
            let namesz = r.int(at, 4)? as usize;
            let descsz = r.int(at + 4, 4)? as usize;
            let mut p = at + 12 + namesz.div_ceil(4) * 4;
            let desc_end = p + descsz;
            while p + 8 <= desc_end {
                let pr_type = r.int(p, 4)?;
                let datasz = r.int(p + 4, 4)? as usize;
                if (pr_type == 0xc000_0002 || pr_type == 0xc000_0000) && datasz >= 4 {
                    features = Some(r.int(p + 8, 4)?);
                }
                p += 8 + datasz.div_ceil(align) * align;
            }
            at = desc_end.div_ceil(4) * 4;
        }
    }
    let is_static = dynamic.is_none();
    let mut rows = vec![
        row(
            "PIE",
            Some(e_type == 3),
            if e_type == 3 {
                "position-independent: the loader places it at a random address"
            } else {
                "a fixed address (ET_EXEC)"
            },
        ),
        row(
            "RELRO",
            Some(relro && (bind_now || is_static)),
            match (relro, bind_now, is_static) {
                (true, _, true) => "full: static, nothing bound at run time".to_string(),
                (true, true, false) => {
                    "full: read-only after relocation, every symbol bound at start (BIND_NOW)"
                        .to_string()
                }
                (true, false, false) => {
                    "partial: no BIND_NOW, so the GOT stays writable".to_string()
                }
                (false, _, _) => "none: no PT_GNU_RELRO".to_string(),
            },
        ),
        row(
            "NX stack",
            Some(stack.unwrap_or(false)),
            match stack {
                Some(true) => "the stack is not executable",
                Some(false) => "the stack is executable (PT_GNU_STACK asks for it)",
                None => "no PT_GNU_STACK: the kernel's default, executable on some",
            },
        ),
    ];
    match machine {
        0x3e => {
            let f = features.unwrap_or(0);
            rows.push(row(
                "CET",
                Some(f & 3 == 3),
                format!(
                    "IBT {}, shadow stack {}",
                    if f & 1 != 0 { "yes" } else { "no" },
                    if f & 2 != 0 { "yes" } else { "no" }
                ),
            ));
        }
        0xb7 => {
            let f = features.unwrap_or(0);
            rows.push(row(
                "BTI/PAC",
                Some(f & 3 == 3),
                format!(
                    "BTI {}, PAC {}",
                    if f & 1 != 0 { "yes" } else { "no" },
                    if f & 2 != 0 { "yes" } else { "no" }
                ),
            ));
        }
        _ => {}
    }
    Some(rows)
}

/// A thin Mach-O's mitigations.
fn macho(b: &[u8]) -> Option<Vec<Row>> {
    let r = Bytes { b, big: false };
    let flags = r.int(24, 4)?;
    let ncmds = r.int(16, 4)?;
    let wide = r.int(0, 4)? == 0xfeed_facf;
    let mut at = if wide { 32 } else { 28 };
    let mut signed = false;
    for _ in 0..ncmds.min(4096) {
        let cmd = r.int(at, 4)?;
        let size = r.int(at + 4, 4)? as usize;
        if cmd == 0x1d {
            signed = true;
        }
        if size == 0 {
            break;
        }
        at += size;
    }
    Some(vec![
        row("PIE", Some(flags & 0x0020_0000 != 0), "MH_PIE"),
        row(
            "NX stack",
            Some(flags & 0x0002_0000 == 0),
            if flags & 0x0002_0000 == 0 {
                "the stack is not executable"
            } else {
                "MH_ALLOW_STACK_EXECUTION"
            },
        ),
        row(
            "signed",
            Some(signed),
            if signed {
                "a code signature (ad hoc or by a developer: codesign -dv says which)"
            } else {
                "no code signature: Apple Silicon will not run it"
            },
        ),
    ])
}

/// A PE's mitigations.
fn pe(b: &[u8]) -> Option<Vec<Row>> {
    let r = Bytes { b, big: false };
    let pe_at = usize::try_from(r.int(0x3c, 4)?).ok()?;
    if b.get(pe_at..pe_at + 4)? != b"PE\0\0" {
        return None;
    }
    let opt = pe_at + 24;
    let magic = r.int(opt, 2)?;
    let dll = r.int(opt + 70, 2)?;
    let dirs = opt + if magic == 0x20b { 112 } else { 96 };
    let cert_size = r.int(dirs + 4 * 8 + 4, 4).unwrap_or(0);
    let bit = |m: u64| dll & m != 0;
    Some(vec![
        row(
            "ASLR",
            Some(bit(0x40) && bit(0x20)),
            format!(
                "DYNAMIC_BASE {}, HIGH_ENTROPY_VA {}",
                if bit(0x40) { "yes" } else { "no" },
                if bit(0x20) { "yes" } else { "no" }
            ),
        ),
        row("NX", Some(bit(0x100)), "NX_COMPAT"),
        row("CFG", Some(bit(0x4000)), "Control Flow Guard (GUARD_CF)"),
        row(
            "signed",
            Some(cert_size > 0),
            if cert_size > 0 {
                "an Authenticode signature"
            } else {
                "no Authenticode signature (cig-compile#10)"
            },
        ),
    ])
}

/// Each slice of a universal Mach-O, by its architecture.
fn fat(b: &[u8]) -> Option<Vec<(String, Vec<Row>)>> {
    let r = Bytes { b, big: true };
    let n = r.int(4, 4)?;
    let mut out = Vec::new();
    for k in 0..n.min(16) {
        let at = 8 + (k as usize) * 20;
        let cpu = r.int(at, 4)?;
        let offset = usize::try_from(r.int(at + 8, 4)?).ok()?;
        let size = usize::try_from(r.int(at + 12, 4)?).ok()?;
        let slice = b.get(offset..offset.checked_add(size)?)?;
        let name = match cpu {
            0x0100_000c => "arm64".to_string(),
            0x0100_0007 => "x86_64".to_string(),
            other => format!("cpu {other:#x}"),
        };
        out.push((name, macho(slice)?));
    }
    Some(out)
}

pub fn audit(ctx: &Ctx, program: &Path) -> i32 {
    let shown = program.display().to_string();
    let bytes = match std::fs::read(program) {
        Ok(b) => b,
        Err(e) => {
            let d = cigscript::diagnostics::usage(format!("cannot read {shown}: {e}"))
                .code("E1000")
                .with_hint("name a program, or any executable");
            cigscript::cli::report(ctx, &d, None, None);
            return exit::USAGE;
        }
    };
    let parts: Option<Vec<(String, Vec<Row>)>> = match crate::runtime::format_of(&bytes) {
        Some(crate::runtime::Format::Elf) => elf(&bytes).map(|r| vec![("ELF".to_string(), r)]),
        Some(crate::runtime::Format::Pe) => pe(&bytes).map(|r| vec![("PE".to_string(), r)]),
        Some(crate::runtime::Format::MachO) if bytes.starts_with(&[0xca, 0xfe, 0xba, 0xbe]) => {
            fat(&bytes)
        }
        Some(crate::runtime::Format::MachO) => {
            macho(&bytes).map(|r| vec![("Mach-O".to_string(), r)])
        }
        None => None,
    };
    let Some(parts) = parts else {
        let d = cigscript::diagnostics::usage(format!(
            "{shown} is not an executable this reads (ELF, Mach-O or PE)"
        ))
        .code("E1000")
        .with_hint("name a program cigcompile built");
        cigscript::cli::report(ctx, &d, None, None);
        return exit::USAGE;
    };
    if ctx.json {
        let v: Vec<serde_json::Value> = parts
            .iter()
            .map(|(kind, rows)| {
                serde_json::json!({
                    "kind": kind,
                    "mitigations": rows.iter().map(|r| serde_json::json!({"name": r.name, "on": r.on, "says": r.says})).collect::<Vec<_>>(),
                })
            })
            .collect();
        println!("{}", serde_json::json!({"program": shown, "parts": v}));
        return exit::OK;
    }
    println!("{shown}  (the runtime's own hardening: cig's build)");
    for (kind, rows) in &parts {
        println!("  {kind}");
        for r in rows {
            let mark = match r.on {
                Some(true) => ctx.green("yes"),
                Some(false) => ctx.yellow("no "),
                None => "n/a".to_string(),
            };
            println!("    {:<10} {mark}  {}", r.name, r.says);
        }
    }
    exit::OK
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 64-bit little-endian ELF header with the given program headers,
    /// each (p_type, p_flags).
    fn tiny_elf(e_type: u16, phdrs: &[(u32, u32)]) -> Vec<u8> {
        let mut b = vec![0u8; 64];
        b[..4].copy_from_slice(b"\x7fELF");
        b[4] = 2;
        b[5] = 1;
        b[16..18].copy_from_slice(&e_type.to_le_bytes());
        b[18..20].copy_from_slice(&0x3eu16.to_le_bytes());
        b[32..40].copy_from_slice(&64u64.to_le_bytes());
        b[54..56].copy_from_slice(&56u16.to_le_bytes());
        b[56..58].copy_from_slice(&(phdrs.len() as u16).to_le_bytes());
        for (t, f) in phdrs {
            let mut p = vec![0u8; 56];
            p[..4].copy_from_slice(&t.to_le_bytes());
            p[4..8].copy_from_slice(&f.to_le_bytes());
            b.extend(p);
        }
        b
    }

    fn on(rows: &[Row], name: &str) -> Option<bool> {
        rows.iter().find(|r| r.name == name).and_then(|r| r.on)
    }

    #[test]
    fn an_elf_without_relro_or_a_closed_stack_says_so() {
        let bare = elf(&tiny_elf(2, &[])).unwrap();
        assert_eq!(on(&bare, "PIE"), Some(false));
        assert_eq!(on(&bare, "RELRO"), Some(false));
        assert_eq!(on(&bare, "NX stack"), Some(false));
        assert_eq!(on(&bare, "CET"), Some(false));
        // Static-pie with RELRO and a stack that is not executable.
        let good = elf(&tiny_elf(
            3,
            &[(PT_GNU_RELRO as u32, 4), (PT_GNU_STACK as u32, 6)],
        ))
        .unwrap();
        assert_eq!(on(&good, "PIE"), Some(true));
        assert_eq!(
            on(&good, "RELRO"),
            Some(true),
            "static: nothing bound at run time"
        );
        assert_eq!(on(&good, "NX stack"), Some(true));
        let rwx = elf(&tiny_elf(3, &[(PT_GNU_STACK as u32, 7)])).unwrap();
        assert_eq!(on(&rwx, "NX stack"), Some(false));
    }
}
