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

/// The stage every target's programs are built at: the checked script
/// appended to cig's runtime (stage A). Stage B is the bytecode (#31).
pub const STAGE: &str = "A";

impl Target {
    /// How cigcompile's CI proves its programs: run on a runner of that
    /// machine, or built and their bytes checked where no runner exists.
    pub fn ci(&self) -> &'static str {
        match self.triple {
            "armv7-unknown-linux-musleabihf" | "x86_64-unknown-freebsd" => "built, not run",
            "universal2-apple-darwin" => "run on both Macs",
            _ => "run on its own runner",
        }
    }

    /// What a program there lacks that it has on Linux: the box, cig's
    /// copy-on-write layer and namespaces (CigScript#106), where a boxed hop
    /// is refused or falls back to the tin (E777, E763).
    pub fn missing(&self) -> &'static str {
        if self.triple.contains("linux") {
            ""
        } else {
            "no box (E777, E763)"
        }
    }

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
    /// What a program made from it needs from the machine it runs on, when
    /// it is a dynamically linked ELF executable ([`needs_of`]).
    pub needs: Option<String>,
    bytes: std::cell::OnceCell<Vec<u8>>,
    sha256: std::cell::OnceCell<String>,
}

impl Runtime {
    pub fn bytes(&self) -> std::io::Result<&[u8]> {
        if self.bytes.get().is_none() {
            let b = std::fs::read(&self.path)?;
            let _ = self.bytes.set(b);
        }
        Ok(self.bytes.get().expect("set above"))
    }

    /// The runtime's sha256, hex, worked out once a build.
    pub fn sha256(&self) -> std::io::Result<&str> {
        if self.sha256.get().is_none() {
            use sha2::{Digest, Sha256};
            let h = hex::encode(Sha256::digest(self.bytes()?));
            let _ = self.sha256.set(h);
        }
        Ok(self.sha256.get().expect("set above"))
    }
}

/// The cig version a program made here runs on: the one this compiler pins.
pub fn wanted() -> &'static str {
    cigscript::VERSION
}

/// Does a program for `target` need a static runtime to run anywhere? cig's
/// Linux releases are static musl builds; a runtime linked to this
/// machine's glibc makes programs that start only where that glibc is
/// (#27).
pub fn wants_static(target: &Target) -> bool {
    target.triple.contains("-linux-musl")
}

/// The machine a program made from `rt` is for, as the build says it: the
/// target, or, for a static Linux target given a dynamically linked
/// runtime, the glibc triple and what the program needs (`x86_64-unknown-
/// linux-gnu (glibc 2.39)`).
pub fn label(target: &Target, rt: &Runtime) -> String {
    match (&rt.needs, wants_static(target)) {
        (Some(n), true) => format!("{} ({n})", triple_of(target, rt)),
        _ => target.triple.to_string(),
    }
}

/// The triple a program made from `rt` really is: `-gnu` when its runtime
/// links glibc.
pub fn triple_of(target: &Target, rt: &Runtime) -> String {
    match (&rt.needs, wants_static(target)) {
        (Some(n), true) if n.starts_with("glibc") => target.triple.replace("-musl", "-gnu"),
        _ => target.triple.to_string(),
    }
}

/// Find the runtime for `target`: the one given (`--runtime`, or
/// `CIGCOMPILE_RUNTIME`), else for this machine `cig-runtime` beside this
/// compiler, then `cig` on PATH and the humidor's copy when they are this
/// compiler's version, then cig's own release. For a static Linux target a
/// runtime found here that is dynamically linked is passed over (#27): its
/// programs would need this machine's glibc.
pub fn find(
    target: &Target,
    given: Option<&Path>,
    index: &mut crate::cache::Index,
) -> Result<Runtime, Diagnostic> {
    find_with(target, given, index, true)
}

/// [`find`], and with `fetch` false never the network: cig's release only
/// when it is kept here already (`cigcompile targets`).
pub fn find_with(
    target: &Target,
    given: Option<&Path>,
    index: &mut crate::cache::Index,
    fetch: bool,
) -> Result<Runtime, Diagnostic> {
    let from_env = std::env::var_os("CIGCOMPILE_RUNTIME").map(PathBuf::from);
    if let Some(p) = given.map(Path::to_path_buf).or(from_env) {
        return load(&p, target, index);
    }
    let release = |say: bool| {
        if fetch {
            crate::fetch::fetch(target, say)
        } else {
            crate::fetch::kept(target).ok_or_else(|| {
                usage(format!(
                    "{} is not kept here",
                    crate::fetch::asset_file(target)
                ))
                .code("E1001")
                .with_hint(format!("cigcompile fetch --target {}", target.triple))
            })
        }
    };
    // Another machine's runtime: cig's own release for it, kept or fetched,
    // and accepted only at the sha256 this compiler pins (src/fetch.rs).
    if target.triple != host().triple {
        let fetched = release(true)?;
        return load(&fetched, target, index);
    }
    let mut seen = Vec::new();
    let mut dynamic: Option<(PathBuf, String)> = None;
    for candidate in candidates(target) {
        match load(&candidate, target, index) {
            Ok(r) => match r.needs.clone().filter(|_| wants_static(target)) {
                Some(needs) => {
                    seen.push(format!(
                        "{} (dynamically linked: its programs need {needs})",
                        candidate.display()
                    ));
                    dynamic.get_or_insert((r.path, needs));
                }
                None => return Ok(r),
            },
            Err(d) => seen.push(format!("{} ({})", candidate.display(), d.message)),
        }
    }
    // Nothing here that will do: cig's own release for this machine, when
    // cig ships one.
    let mut why_not = None;
    if host().triple != OTHER.triple {
        match release(true) {
            Ok(p) => return load(&p, target, index),
            Err(d) => {
                seen.push(format!("its release ({})", d.message));
                why_not = Some(d.message);
            }
        }
    }
    if let Some((path, needs)) = dynamic {
        let why = why_not.map(|w| format!(" (here: {w})")).unwrap_or_default();
        return Err(usage(format!(
            "this machine's cig ({}) needs {needs}, so its programs would not start on most Linux machines",
            path.display()
        ))
        .code("E1008")
        .with_hint(format!(
            "cigcompile fetch for cig's static release{why}, or build cigcompile with cargo build --release --target {}",
            target.triple
        )));
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
        if let Some((version, needs)) = index.runtime_version(path, &id, target.triple) {
            if version == wanted() {
                return Ok(Runtime {
                    path: path.clone(),
                    version,
                    id,
                    needs,
                    bytes: std::cell::OnceCell::new(),
                    sha256: std::cell::OnceCell::new(),
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
    let needs = needs_of(&bytes);
    index.note_runtime(path, &id, &version, target.triple, needs.as_deref());
    let cell = std::cell::OnceCell::new();
    let _ = cell.set(bytes);
    Ok(Runtime {
        path: path.clone(),
        version,
        id,
        needs,
        bytes: cell,
        sha256: std::cell::OnceCell::new(),
    })
}

/// What a program made from this runtime needs from the machine it runs
/// on, when the runtime is a dynamically linked ELF executable: the newest
/// glibc its version needs name (`glibc 2.39`), else its loader (`the
/// loader /lib/ld-musl-x86_64.so.1`). `None` for a static executable, or
/// one that is not ELF.
pub fn needs_of(bytes: &[u8]) -> Option<String> {
    let elf = Elf::of(bytes)?;
    let interp = elf.interp()?;
    Some(match elf.glibc() {
        Some(v) => format!("glibc {v}"),
        None => format!("the loader {interp}"),
    })
}

/// An ELF file's headers, read where they lie.
struct Elf<'a> {
    b: &'a [u8],
    wide: bool,
    big: bool,
}

impl<'a> Elf<'a> {
    fn of(b: &'a [u8]) -> Option<Self> {
        (format_of(b)? == Format::Elf).then_some(())?;
        Some(Elf {
            b,
            wide: *b.get(4)? == 2,
            big: *b.get(5)? == 2,
        })
    }

    fn int(&self, at: usize, n: usize) -> Option<u64> {
        let s = self.b.get(at..at.checked_add(n)?)?;
        let f = |acc: u64, x: &u8| acc << 8 | u64::from(*x);
        Some(if self.big {
            s.iter().fold(0, f)
        } else {
            s.iter().rev().fold(0, f)
        })
    }

    /// A 64-bit file's field at `wide`, a 32-bit file's at `narrow`.
    fn field(&self, at: usize, wide: usize, narrow: usize) -> Option<u64> {
        if self.wide {
            self.int(at.checked_add(wide)?, 8)
        } else {
            self.int(at.checked_add(narrow)?, 4)
        }
    }

    /// A header table's offset, entry size and count.
    fn table(&self, which: Table) -> Option<(u64, u64, u64)> {
        let [off, size, count] = which[usize::from(!self.wide)];
        let base = self.int(off, if self.wide { 8 } else { 4 })?;
        Some((base, self.int(size, 2)?, self.int(count, 2)?))
    }

    /// Where entry `k` of a header table starts.
    fn entry(&self, k: u64, which: Table) -> Option<usize> {
        let (base, size, _) = self.table(which)?;
        usize::try_from(base.checked_add(k.checked_mul(size)?)?).ok()
    }

    fn count(&self, which: Table) -> Option<u64> {
        Some(self.table(which)?.2)
    }

    /// The `PT_INTERP` segment: the loader a dynamically linked executable
    /// names.
    fn interp(&self) -> Option<String> {
        for k in 0..self.count(PHDRS)? {
            let at = self.entry(k, PHDRS)?;
            if self.int(at, 4)? != 3 {
                continue;
            }
            let off = usize::try_from(self.field(at, 8, 4)?).ok()?;
            let len = usize::try_from(self.field(at, 32, 16)?).ok()?;
            let raw = self.b.get(off..off.checked_add(len)?)?;
            let path = raw.split(|c| *c == 0).next().unwrap_or_default();
            return Some(String::from_utf8_lossy(path).into_owned());
        }
        None
    }

    /// The newest `GLIBC_2.x` its version needs (`.gnu.version_r`) name.
    fn glibc(&self) -> Option<String> {
        let mut best: Option<Vec<u32>> = None;
        for k in 0..self.count(SHDRS)? {
            let at = self.entry(k, SHDRS)?;
            // SHT_GNU_verneed.
            if self.int(at + 4, 4)? != 0x6fff_fffe {
                continue;
            }
            let start = usize::try_from(self.field(at, 24, 16)?).ok()?;
            let link = self.int(at + if self.wide { 40 } else { 24 }, 4)?;
            let needs = self.int(at + if self.wide { 44 } else { 28 }, 4)?;
            let strings = usize::try_from(self.field(self.entry(link, SHDRS)?, 24, 16)?).ok()?;
            let mut vn = start;
            for _ in 0..needs.min(4096) {
                let mut aux = vn.checked_add(usize::try_from(self.int(vn + 8, 4)?).ok()?)?;
                for _ in 0..self.int(vn + 2, 2)?.min(4096) {
                    let name = strings.checked_add(usize::try_from(self.int(aux + 8, 4)?).ok()?)?;
                    let rest = self.b.get(name..)?;
                    let word = &rest[..rest.iter().position(|c| *c == 0)?];
                    let version: Option<Vec<u32>> = word
                        .strip_prefix(b"GLIBC_")
                        .and_then(|v| std::str::from_utf8(v).ok())
                        .and_then(|v| v.split('.').map(|p| p.parse().ok()).collect());
                    if let Some(v) = version {
                        if best.as_ref().is_none_or(|b| v > *b) {
                            best = Some(v);
                        }
                    }
                    match self.int(aux + 12, 4)? {
                        0 => break,
                        next => aux = aux.checked_add(usize::try_from(next).ok()?)?,
                    }
                }
                match self.int(vn + 12, 4)? {
                    0 => break,
                    next => vn = vn.checked_add(usize::try_from(next).ok()?)?,
                }
            }
        }
        best.map(|v| v.iter().map(u32::to_string).collect::<Vec<_>>().join("."))
    }
}

/// Where an ELF header keeps a table's offset, entry size and count: in a
/// 64-bit header, then in a 32-bit one.
type Table = [[usize; 3]; 2];
/// The program header table (`e_phoff`, `e_phentsize`, `e_phnum`).
const PHDRS: Table = [[32, 54, 56], [28, 42, 44]];
/// The section header table (`e_shoff`, `e_shentsize`, `e_shnum`).
const SHDRS: Table = [[40, 58, 60], [32, 46, 48]];

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

    /// An ELF with a `PT_INTERP` segment and, when `glibc` names any, a
    /// `.gnu.version_r` asking for them, 64-bit or 32-bit.
    fn dynamic_elf(wide: bool, glibc: &[&str]) -> Vec<u8> {
        let put = |b: &mut Vec<u8>, at: usize, n: usize, v: u64| {
            b[at..at + n].copy_from_slice(&v.to_le_bytes()[..n]);
        };
        let w = |a: usize, b: usize| if wide { a } else { b };
        let (head, ph, sh) = (w(64, 52), w(56, 32), w(64, 40));
        let interp = b"/lib/ld-test.so\0";
        let mut strings = b"\0libc.so.6\0".to_vec();
        let names: Vec<usize> = glibc
            .iter()
            .map(|v| {
                let at = strings.len();
                strings.extend_from_slice(format!("GLIBC_{v}\0").as_bytes());
                at
            })
            .collect();
        let (i_at, s_at) = (head + ph, head + ph + interp.len());
        let v_at = s_at + strings.len();
        let h_at = v_at + 16 + 16 * names.len();
        let mut b = vec![0u8; h_at + 3 * sh];
        b[..6].copy_from_slice(&[0x7f, b'E', b'L', b'F', w(2, 1) as u8, 1]);
        put(&mut b, w(32, 28), w(8, 4), head as u64);
        put(&mut b, w(40, 32), w(8, 4), h_at as u64);
        put(&mut b, w(54, 42), 2, ph as u64);
        put(&mut b, w(56, 44), 2, 1);
        put(&mut b, w(58, 46), 2, sh as u64);
        put(&mut b, w(60, 48), 2, if names.is_empty() { 0 } else { 3 });
        put(&mut b, head, 4, 3);
        put(&mut b, head + w(8, 4), w(8, 4), i_at as u64);
        put(&mut b, head + w(32, 16), w(8, 4), interp.len() as u64);
        b[i_at..s_at].copy_from_slice(interp);
        b[s_at..v_at].copy_from_slice(&strings);
        put(&mut b, v_at + 2, 2, names.len() as u64);
        put(&mut b, v_at + 4, 4, 1);
        put(&mut b, v_at + 8, 4, 16);
        for (k, name) in names.iter().enumerate() {
            let aux = v_at + 16 + 16 * k;
            put(&mut b, aux + 8, 4, *name as u64);
            put(
                &mut b,
                aux + 12,
                4,
                if k + 1 < names.len() { 16 } else { 0 },
            );
        }
        let strtab = h_at + sh;
        put(&mut b, strtab + 4, 4, 3);
        put(&mut b, strtab + w(24, 16), w(8, 4), s_at as u64);
        let verneed = h_at + 2 * sh;
        put(&mut b, verneed + 4, 4, 0x6fff_fffe);
        put(&mut b, verneed + w(24, 16), w(8, 4), v_at as u64);
        put(&mut b, verneed + w(40, 24), 4, 1);
        put(&mut b, verneed + w(44, 28), 4, 1);
        b
    }

    #[test]
    fn a_dynamically_linked_runtime_says_what_its_programs_need() {
        for wide in [true, false] {
            let b = dynamic_elf(wide, &["2.17", "2.34", "2.2.5", "PRIVATE"]);
            assert_eq!(needs_of(&b).as_deref(), Some("glibc 2.34"), "wide {wide}");
            let b = dynamic_elf(wide, &[]);
            assert_eq!(
                needs_of(&b).as_deref(),
                Some("the loader /lib/ld-test.so"),
                "wide {wide}"
            );
        }
        // A static ELF names no loader, and a Mach-O or PE is not asked.
        let mut elf = vec![0x7f, b'E', b'L', b'F', 2, 1];
        elf.resize(64, 0);
        assert_eq!(needs_of(&elf), None);
        assert_eq!(needs_of(b"MZ\x90\x00"), None);
        // This test binary: a glibc build needs its glibc; a musl, macOS or
        // Windows one needs nothing.
        let own = std::fs::read(std::env::current_exe().unwrap()).unwrap();
        let needs = needs_of(&own);
        if cfg!(all(target_os = "linux", target_env = "gnu")) {
            let n = needs.expect("a glibc build is dynamically linked");
            let v = n.strip_prefix("glibc ").unwrap_or_else(|| panic!("{n}"));
            assert!(v.split('.').all(|p| p.parse::<u32>().is_ok()), "{n}");
        } else if cfg!(any(target_os = "macos", windows, target_env = "musl")) {
            assert_eq!(needs, None);
        }
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
