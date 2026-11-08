// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! `build --emit deb` (#41): beside the program, a `.deb` that installs it
//! to `/usr/bin/<name>`. The archive is written in pure Rust (ar + tar + gzip,
//! reusing the tar writer `--emit oci` already has): a Debian package is an
//! `ar` archive of three members (`debian-binary`, `control.tar.gz`,
//! `data.tar.gz`), and nothing else is needed to assemble one. Every time in
//! it is zero and every owner root, so the same build writes the same bytes
//! (`dpkg-deb --info` and `dpkg-deb --contents` read it; the test suite does
//! not install it).

use crate::runtime::Target;
use crate::tar::{end, entry};

/// The Debian architecture of a target, or `None` for one that is not Linux
/// (or Linux on a CPU this compiler does not build for).
pub fn arch(triple: &str) -> Option<&'static str> {
    match triple {
        "x86_64-unknown-linux-musl" | "x86_64-unknown-linux-gnu" => Some("amd64"),
        "aarch64-unknown-linux-musl" | "aarch64-unknown-linux-gnu" => Some("arm64"),
        "armv7-unknown-linux-musleabihf" | "armv7-unknown-linux-gnueabihf" => Some("armhf"),
        "i686-unknown-linux-musl" | "i686-unknown-linux-gnu" => Some("i386"),
        _ => None,
    }
}

/// What a `.deb`'s control file names besides the program.
pub struct Meta {
    /// The package's name, sanitised to Debian's rules (`sanitise_name`).
    pub package: String,
    /// The version (`--pkg-version`, or `0.0.0`).
    pub version: String,
    /// The architecture (`arch`).
    pub arch: &'static str,
    /// The person who made the package (`--pkg-maintainer`, or a default).
    pub maintainer: String,
    /// What the package is (`--pkg-description`, or a default).
    pub description: String,
}

/// The package's name as Debian will accept it: lower case, digits, plus,
/// minus and dot, starting with a letter or digit, two characters or more.
/// Anything else is replaced with a minus; runs of minus are folded to one;
/// the first character, if not alphanumeric, is prefixed with `cig`.
pub fn sanitise_name(raw: &str) -> String {
    let mut out: String = raw
        .chars()
        .map(|c| c.to_ascii_lowercase())
        .map(|c| match c {
            'a'..='z' | '0'..='9' | '+' | '-' | '.' => c,
            _ => '-',
        })
        .collect();
    // Fold runs of `-` into one.
    while out.contains("--") {
        out = out.replace("--", "-");
    }
    out = out.trim_matches('-').to_string();
    if !out
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric())
    {
        out = format!("cig{out}");
    }
    if out.len() < 2 {
        out = format!("{out}0");
    }
    out
}

/// The `.deb`'s bytes: ar header, `debian-binary`, `control.tar.gz` and
/// `data.tar.gz`. The program goes to `/usr/bin/<name>`; the control file
/// names the package, its version, the architecture, the maintainer and the
/// description. A description runs across many lines, each continuation
/// prefixed by a space, so `dpkg-deb --info` prints it whole.
pub fn package(name: &str, program: &[u8], meta: &Meta) -> Vec<u8> {
    let data_tar = data_tar(name, program);
    let data_gz = crate::gzip::compress(&data_tar);
    let control_tar = control_tar(&control_file(meta));
    let control_gz = crate::gzip::compress(&control_tar);
    let mut out = Vec::with_capacity(data_gz.len() + control_gz.len() + 256);
    out.extend_from_slice(b"!<arch>\n");
    ar_member(&mut out, "debian-binary", b"2.0\n");
    ar_member(&mut out, "control.tar.gz", &control_gz);
    ar_member(&mut out, "data.tar.gz", &data_gz);
    out
}

/// Where a program's `.deb` goes: beside it, `<program>_<version>_<arch>.deb`,
/// as Debian names the file.
pub fn path_for(program: &std::path::Path, meta: &Meta) -> std::path::PathBuf {
    program.with_file_name(format!(
        "{}_{}_{}.deb",
        meta.package, meta.version, meta.arch
    ))
}

/// The tar that holds the control file.
fn control_tar(control: &str) -> Vec<u8> {
    let mut out = Vec::new();
    entry(&mut out, "./", 0o755, b'5', b"");
    entry(&mut out, "./control", 0o644, b'0', control.as_bytes());
    end(&mut out);
    out
}

/// The tar that holds the program at `/usr/bin/<name>`, with its parent
/// directories as `dpkg-deb --contents` shows them.
fn data_tar(name: &str, program: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    entry(&mut out, "./", 0o755, b'5', b"");
    entry(&mut out, "./usr/", 0o755, b'5', b"");
    entry(&mut out, "./usr/bin/", 0o755, b'5', b"");
    entry(&mut out, &format!("./usr/bin/{name}"), 0o755, b'0', program);
    end(&mut out);
    out
}

/// The control file's text. Debian wraps a long line at 80 columns with a
/// continuation prefixed by a space; the description is one line here, so
/// `dpkg-deb --info` prints it whole.
fn control_file(m: &Meta) -> String {
    let desc = m
        .description
        .lines()
        .next()
        .unwrap_or("A program compiled with cigcompile.");
    format!(
        "Package: {}\n\
         Version: {}\n\
         Section: utils\n\
         Priority: optional\n\
         Architecture: {}\n\
         Maintainer: {}\n\
         Description: {desc}\n",
        m.package, m.version, m.arch, m.maintainer,
    )
}

/// One member in an `ar` archive: a 60-byte header (BSD style: name
/// space-padded to 16, no trailing slash, as Debian writes them), then the
/// bytes, then a `\n` if the length is odd. The time is zero and the owners
/// root, so the archive is byte-for-byte reproducible.
fn ar_member(out: &mut Vec<u8>, name: &str, data: &[u8]) {
    assert!(name.len() <= 16, "an ar name fits in 16 bytes: {name}");
    let mut h = [b' '; 60];
    h[..name.len()].copy_from_slice(name.as_bytes());
    // mtime (12 bytes, decimal): 0.
    h[16] = b'0';
    // uid (6 bytes): 0.
    h[28] = b'0';
    // gid (6 bytes): 0.
    h[34] = b'0';
    // mode (8 bytes, octal): 100644 for a regular file.
    h[40..46].copy_from_slice(b"100644");
    // size (10 bytes, decimal).
    let size = data.len().to_string();
    h[48..48 + size.len()].copy_from_slice(size.as_bytes());
    // The trailer: 0x60 0x0a.
    h[58] = 0x60;
    h[59] = 0x0a;
    out.extend_from_slice(&h);
    out.extend_from_slice(data);
    if data.len() % 2 == 1 {
        out.push(b'\n');
    }
}

/// `meta` from the command-line arguments: the package name is the program's
/// name (sanitised), the version `--pkg-version` or `0.0.0`, the maintainer
/// `--pkg-maintainer` or `cigcompile <nobody@localhost>`, the description
/// `--pkg-description` or a default that names the program.
pub fn meta_for(
    name: &str,
    target: &Target,
    version: Option<&str>,
    maintainer: Option<&str>,
    description: Option<&str>,
) -> Result<Meta, cigscript::diagnostics::Diagnostic> {
    use cigscript::diagnostics::usage;
    let arch = arch(target.triple).ok_or_else(|| {
        usage(format!(
            "a .deb is for a Linux target, and this build is for {}",
            target.triple
        ))
        .with_hint(
            "build for Linux: --target x86_64-unknown-linux-musl (or aarch64-unknown-linux-musl, armv7-unknown-linux-musleabihf)",
        )
    })?;
    Ok(Meta {
        package: sanitise_name(name),
        version: version.unwrap_or("0.0.0").to_string(),
        arch,
        maintainer: maintainer
            .unwrap_or("cigcompile <nobody@localhost>")
            .to_string(),
        description: description
            .map(str::to_string)
            .unwrap_or_else(|| format!("{name}, a CigScript program compiled with cigcompile.")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_sanitised_to_debians_rules() {
        assert_eq!(sanitise_name("tidy-downloads"), "tidy-downloads");
        assert_eq!(sanitise_name("TidyDownloads"), "tidydownloads");
        assert_eq!(sanitise_name("tidy_downloads"), "tidy-downloads");
        assert_eq!(sanitise_name("tidy  downloads"), "tidy-downloads");
        assert_eq!(sanitise_name("--tidy--"), "tidy");
        assert_eq!(sanitise_name("1"), "10");
        assert_eq!(sanitise_name("_"), "cig");
    }

    #[test]
    fn target_triples_map_to_debian_architectures() {
        assert_eq!(arch("x86_64-unknown-linux-musl"), Some("amd64"));
        assert_eq!(arch("aarch64-unknown-linux-musl"), Some("arm64"));
        assert_eq!(arch("armv7-unknown-linux-musleabihf"), Some("armhf"));
        assert_eq!(arch("i686-unknown-linux-musl"), Some("i386"));
        assert_eq!(arch("x86_64-apple-darwin"), None);
        assert_eq!(arch("universal2-apple-darwin"), None);
    }

    /// The archive starts with the ar magic and holds exactly three members
    /// in the order Debian asks for them, and the same input writes the same
    /// bytes.
    #[test]
    fn a_deb_is_an_ar_of_three_members_in_order_and_reproducible() {
        let meta = Meta {
            package: "hello".to_string(),
            version: "0.0.0".to_string(),
            arch: "amd64",
            maintainer: "cigcompile <nobody@localhost>".to_string(),
            description: "A hello program.".to_string(),
        };
        let a = package("hello", b"\x7fELF program bytes", &meta);
        let b = package("hello", b"\x7fELF program bytes", &meta);
        assert_eq!(a, b, "the same input wrote different bytes");
        assert_eq!(&a[..8], b"!<arch>\n");
        let members = ar_members(&a[8..]);
        assert_eq!(
            members.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
            vec!["debian-binary", "control.tar.gz", "data.tar.gz"],
        );
        assert_eq!(members[0].1, b"2.0\n");
    }

    /// Parse an ar archive's members for the test above. Members are
    /// space-padded 16-byte names with a 60-byte header, each followed by the
    /// data and a `\n` padding when the length is odd.
    fn ar_members(mut b: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        while b.len() >= 60 {
            let name = std::str::from_utf8(&b[..16])
                .unwrap()
                .trim_end()
                .to_string();
            let size: usize = std::str::from_utf8(&b[48..58])
                .unwrap()
                .trim_end()
                .parse()
                .unwrap();
            b = &b[60..];
            out.push((name, b[..size].to_vec()));
            let step = size + size % 2;
            b = &b[step..];
        }
        out
    }
}
