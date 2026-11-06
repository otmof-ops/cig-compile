// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! Targets, and the runtime a program is appended to: the `cig` of the
//! version this compiler pins, built for the target.

use cigscript::diagnostics::{usage, Diagnostic};
use std::path::{Path, PathBuf};

/// What a target's executables look like, to catch a runtime for the
/// wrong machine before a program is made from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Elf,
    MachO,
    Pe,
}

/// A machine cig is released for.
#[derive(Debug)]
pub struct Target {
    /// The Rust target triple.
    pub triple: &'static str,
    /// The name of cig's release asset for it (`cig-v<version>-<asset>`).
    pub asset: &'static str,
    pub format: Format,
    /// For a target made of others, the triples of its parts: the
    /// universal macOS binary is the arm64 and the x86_64 runtime in one.
    pub parts: &'static [&'static str],
}

impl Target {
    /// What its executables end in.
    pub fn suffix(&self) -> &'static str {
        if self.format == Format::Pe {
            ".exe"
        } else {
            ""
        }
    }
}

/// Every target cig is released for (CigScript's release workflow).
pub static TARGETS: &[Target] = &[
    Target {
        triple: "x86_64-unknown-linux-musl",
        asset: "linux-x86_64",
        format: Format::Elf,
        parts: &[],
    },
    Target {
        triple: "aarch64-unknown-linux-musl",
        asset: "linux-aarch64",
        format: Format::Elf,
        parts: &[],
    },
    Target {
        triple: "armv7-unknown-linux-musleabihf",
        asset: "linux-arm",
        format: Format::Elf,
        parts: &[],
    },
    Target {
        triple: "aarch64-apple-darwin",
        asset: "macos-aarch64",
        format: Format::MachO,
        parts: &[],
    },
    Target {
        triple: "x86_64-apple-darwin",
        asset: "macos-x86_64",
        format: Format::MachO,
        parts: &[],
    },
    // Not a release asset: made here from the two above (src/fat.rs).
    Target {
        triple: "universal2-apple-darwin",
        asset: "macos-universal",
        format: Format::MachO,
        parts: &["aarch64-apple-darwin", "x86_64-apple-darwin"],
    },
    Target {
        triple: "x86_64-pc-windows-msvc",
        asset: "windows-x86_64",
        format: Format::Pe,
        parts: &[],
    },
    Target {
        triple: "aarch64-pc-windows-msvc",
        asset: "windows-aarch64",
        format: Format::Pe,
        parts: &[],
    },
    Target {
        triple: "x86_64-unknown-freebsd",
        asset: "freebsd-x86_64",
        format: Format::Elf,
        parts: &[],
    },
];

/// This machine, as the targets name it.
pub fn host() -> &'static Target {
    let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
    let triple = match (os, arch) {
        ("linux", "x86_64") => "x86_64-unknown-linux-musl",
        ("linux", "aarch64") => "aarch64-unknown-linux-musl",
        ("linux", "arm") => "armv7-unknown-linux-musleabihf",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        ("freebsd", "x86_64") => "x86_64-unknown-freebsd",
        _ => return &OTHER,
    };
    TARGETS.iter().find(|t| t.triple == triple).expect("listed")
}

/// A machine cig is not released for: cigcompile still runs there and
/// builds for it from a runtime given with `--runtime`.
static OTHER: Target = Target {
    triple: "this-machine",
    asset: "this-machine",
    format: if cfg!(windows) {
        Format::Pe
    } else if cfg!(target_os = "macos") {
        Format::MachO
    } else {
        Format::Elf
    },
    parts: &[],
};

/// The target named, or this machine's.
pub fn target(name: Option<&str>) -> Result<&'static Target, Diagnostic> {
    let Some(name) = name else {
        return Ok(host());
    };
    TARGETS
        .iter()
        .find(|t| t.triple == name || t.asset == name)
        .ok_or_else(|| {
            usage(format!("`{name}` is not a target cig is released for"))
                .code("E1006")
                .with_hint("cigcompile targets lists them, e.g. --target x86_64-unknown-linux-musl")
        })
}

/// A runtime, checked. Its bytes are read when a program is made from it,
/// never for a build that changes nothing.
pub struct Runtime {
    pub path: PathBuf,
    pub version: String,
    pub id: crate::cache::FileId,
    bytes: std::cell::OnceCell<Vec<u8>>,
}

impl Runtime {
    pub fn bytes(&self) -> std::io::Result<&[u8]> {
        if self.bytes.get().is_none() {
            let b = std::fs::read(&self.path)?;
            let _ = self.bytes.set(b);
        }
        Ok(self.bytes.get().expect("set above"))
    }
}

/// The cig version a program made here runs on: the one this compiler pins.
pub fn wanted() -> &'static str {
    cigscript::VERSION
}

/// Find the runtime for `target`: the one given (`--runtime`, or
/// `CIGCOMPILE_RUNTIME`), else for this machine `cig-runtime` beside this
/// compiler, then `cig` on PATH and the humidor's copy when they are this
/// compiler's version.
pub fn find(
    target: &Target,
    given: Option<&Path>,
    index: &mut crate::cache::Index,
) -> Result<Runtime, Diagnostic> {
    let from_env = std::env::var_os("CIGCOMPILE_RUNTIME").map(PathBuf::from);
    if let Some(p) = given.map(Path::to_path_buf).or(from_env) {
        return load(&p, target, index);
    }
    // Another machine's runtime: cig's own release for it, kept or fetched,
    // and accepted only at the sha256 this compiler pins (src/fetch.rs).
    if target.triple != host().triple {
        let fetched = crate::fetch::fetch(target, true)?;
        return load(&fetched, target, index);
    }
    let mut seen = Vec::new();
    for candidate in candidates(target) {
        match load(&candidate, target, index) {
            Ok(r) => return Ok(r),
            Err(d) => seen.push(format!("{} ({})", candidate.display(), d.message)),
        }
    }
    // Nothing here: cig's own release for this machine, when cig ships one.
    if host().triple != OTHER.triple {
        match crate::fetch::fetch(target, true) {
            Ok(p) => return load(&p, target, index),
            Err(d) => seen.push(format!("its release ({})", d.message)),
        }
    }
    let mut d = usage(format!(
        "no cig {} to append the program to was found on this machine",
        wanted()
    ))
    .code("E1001");
    d = if seen.is_empty() {
        d.with_hint(format!(
            "install cig {} (cig update, or its release), or pass one with --runtime",
            wanted()
        ))
    } else {
        d.with_hint(format!(
            "found, not usable: {}; pass cig {} with --runtime",
            seen.join("; "),
            wanted()
        ))
    };
    Err(d)
}

fn candidates(target: &Target) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
    {
        out.push(dir.join(format!("cig-runtime{}", target.suffix())));
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let p = dir.join(format!("cig{}", target.suffix()));
            if p.is_file() && !out.contains(&p) {
                out.push(p);
                break;
            }
        }
    }
    if let Some(v) = cigscript::update::Version::parse(wanted()) {
        if let Some(p) = cigscript::humidor::binary(&v) {
            out.push(p);
        }
    }
    out.into_iter().filter(|p| p.is_file()).collect()
}

/// Check a runtime is the cig this compiler pins, built for the target, and
/// not a program already: from the index when the file is unchanged since
/// it was checked, else by reading it.
pub fn load(
    path: &Path,
    target: &Target,
    index: &mut crate::cache::Index,
) -> Result<Runtime, Diagnostic> {
    let path = &std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if let Some(id) = crate::cache::FileId::of(path) {
        if let Some(version) = index.runtime_version(path, &id, target.triple) {
            if version == wanted() {
                return Ok(Runtime {
                    path: path.clone(),
                    version,
                    id,
                    bytes: std::cell::OnceCell::new(),
                });
            }
        }
    }
    let bytes = std::fs::read(path).map_err(|e| {
        usage(format!("cannot read the runtime {}: {e}", path.display()))
            .code("E1001")
            .with_hint("pass a cig binary you can read with --runtime")
    })?;
    if cigscript::program::carries_program(&bytes) {
        return Err(
            usage(format!("{} is a compiled program, not cig", path.display()))
                .code("E1005")
                .with_hint("give the cig binary itself with --runtime"),
        );
    }
    if !target.parts.is_empty() && crate::fat::slices(&bytes).is_none() {
        return Err(usage(format!(
            "{} is not a universal binary, which {} needs",
            path.display(),
            target.triple
        ))
        .code("E1002")
        .with_hint(format!(
            "leave out --runtime and cigcompile makes it from cig's releases for {}",
            target.parts.join(" and ")
        )));
    }
    if format_of(&bytes) != Some(target.format) {
        return Err(usage(format!(
            "{} is not an executable for {}",
            path.display(),
            target.triple
        ))
        .code("E1002")
        .with_hint(format!(
            "pass the cig built for {} (release asset {})",
            target.triple, target.asset
        )));
    }
    // The CPU too: a runtime for another machine of the same kind makes a
    // program that does not start there.
    let wrong = |what: String| {
        usage(format!("{} {what}", path.display()))
            .code("E1002")
            .with_hint(format!(
                "pass the cig built for {} (release asset {})",
                target.triple, target.asset
            ))
    };
    if target.parts.is_empty() {
        if let (Some(want), Some(got)) = (triple_arch(target.triple), arch_of(&bytes)) {
            if want != got {
                return Err(wrong(format!("is for {got}, not {}", target.triple)));
            }
        }
    } else {
        let have: Vec<&str> = crate::fat::slices(&bytes)
            .unwrap_or_default()
            .iter()
            .filter_map(|s| mach_arch(s.cputype))
            .collect();
        for part in target.parts {
            if let Some(want) = triple_arch(part) {
                if !have.contains(&want) {
                    return Err(wrong(format!(
                        "holds no {want} slice, which {} needs",
                        target.triple
                    )));
                }
            }
        }
    }
    let Some(version) = cigscript::program::runtime_version(&bytes) else {
        return Err(usage(format!(
            "{} carries no runtime version: a cig before 1.15.0, or not cig",
            path.display()
        ))
        .code("E1002")
        .with_hint(format!("pass cig {} with --runtime", wanted())));
    };
    if version != wanted() {
        return Err(usage(format!(
            "{} is cig {version}; this cigcompile makes programs for cig {}",
            path.display(),
            wanted()
        ))
        .code("E1002")
        .with_hint(format!(
            "pass cig {} with --runtime, or use the cigcompile built for cig {version}",
            wanted()
        )));
    }
    let id = crate::cache::FileId::of(path).ok_or_else(|| {
        usage(format!("cannot read the runtime {}", path.display()))
            .code("E1001")
            .with_hint("pass a cig binary you can read with --runtime")
    })?;
    index.note_runtime(path, &id, &version, target.triple);
    let cell = std::cell::OnceCell::new();
    let _ = cell.set(bytes);
    Ok(Runtime {
        path: path.clone(),
        version,
        id,
        bytes: cell,
    })
}

/// What kind of executable the bytes are, from their first bytes.
pub fn format_of(bytes: &[u8]) -> Option<Format> {
    match bytes.get(..4)? {
        [0x7f, b'E', b'L', b'F'] => Some(Format::Elf),
        [0xcf, 0xfa, 0xed, 0xfe] | [0xce, 0xfa, 0xed, 0xfe] | [0xca, 0xfe, 0xba, 0xbe] => {
            Some(Format::MachO)
        }
        [b'M', b'Z', _, _] => Some(Format::Pe),
        _ => None,
    }
}

/// The CPU an executable is for, from its header: ELF's `e_machine`, a
/// thin Mach-O's `cputype`, PE's `Machine`. None for a universal binary
/// (each slice has its own) or one that cannot be told.
pub fn arch_of(bytes: &[u8]) -> Option<&'static str> {
    let u16_le = |at: usize| Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?));
    let u32_le = |at: usize| Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
    match format_of(bytes)? {
        Format::Elf => {
            let m = if *bytes.get(5)? == 2 {
                u16::from_be_bytes(bytes.get(18..20)?.try_into().ok()?)
            } else {
                u16_le(18)?
            };
            match m {
                0x3e => Some("x86_64"),
                0xb7 => Some("aarch64"),
                0x28 => Some("arm"),
                _ => None,
            }
        }
        Format::MachO if crate::fat::slices(bytes).is_some() => None,
        Format::MachO => mach_arch(u32_le(4)?),
        Format::Pe => {
            let at = u32_le(0x3c)? as usize;
            if bytes.get(at..at + 4)? != b"PE\0\0" {
                return None;
            }
            match u16_le(at + 4)? {
                0x8664 => Some("x86_64"),
                0xaa64 => Some("aarch64"),
                _ => None,
            }
        }
    }
}

fn mach_arch(cputype: u32) -> Option<&'static str> {
    match cputype {
        0x0100_0007 => Some("x86_64"),
        0x0100_000c => Some("aarch64"),
        _ => None,
    }
}

/// The CPU a target's triple names; None for one made of others.
fn triple_arch(triple: &str) -> Option<&'static str> {
    match triple.split('-').next()? {
        "x86_64" => Some("x86_64"),
        "aarch64" => Some("aarch64"),
        "armv7" => Some("arm"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_executables_cpu_is_read_from_its_header() {
        let mut elf = vec![0x7f, b'E', b'L', b'F', 2, 1];
        elf.resize(18, 0);
        elf.extend(0xb7u16.to_le_bytes());
        assert_eq!(arch_of(&elf), Some("aarch64"));
        let mut mach = vec![0xcf, 0xfa, 0xed, 0xfe];
        mach.extend(0x0100_0007u32.to_le_bytes());
        assert_eq!(arch_of(&mach), Some("x86_64"));
        let mut pe = vec![b'M', b'Z'];
        pe.resize(0x3c, 0);
        pe.extend(0x40u32.to_le_bytes());
        pe.extend(b"PE\0\0");
        pe.extend(0xaa64u16.to_le_bytes());
        assert_eq!(arch_of(&pe), Some("aarch64"));
        assert_eq!(triple_arch("armv7-unknown-linux-musleabihf"), Some("arm"));
        assert_eq!(triple_arch("universal2-apple-darwin"), None);
        // This build's own cig is for this machine.
        let own = std::fs::read(std::env::current_exe().unwrap()).unwrap();
        assert_eq!(arch_of(&own), triple_arch(host().triple));
    }

    #[test]
    fn targets_are_found_by_triple_or_asset_and_others_refused() {
        assert_eq!(
            target(Some("linux-x86_64")).unwrap().triple,
            "x86_64-unknown-linux-musl"
        );
        assert_eq!(
            target(Some("aarch64-apple-darwin")).unwrap().asset,
            "macos-aarch64"
        );
        assert_eq!(
            target(Some("x86_64-pc-windows-msvc")).unwrap().suffix(),
            ".exe"
        );
        let e = target(Some("sparc-sun-solaris")).unwrap_err();
        assert_eq!(e.code, "E1006");
        assert_eq!(target(None).unwrap().triple, host().triple);
    }

    #[test]
    fn executable_formats_are_told_apart() {
        assert_eq!(format_of(b"\x7fELF\x02\x01"), Some(Format::Elf));
        assert_eq!(format_of(&[0xcf, 0xfa, 0xed, 0xfe, 7]), Some(Format::MachO));
        assert_eq!(format_of(b"MZ\x90\x00"), Some(Format::Pe));
        assert_eq!(format_of(b"#!/bin/sh"), None);
        assert_eq!(format_of(b"MZ"), None);
    }
}
