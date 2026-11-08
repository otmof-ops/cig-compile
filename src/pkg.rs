// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! `cigcompile package` (#41): a Homebrew formula and a Scoop manifest for a
//! released program, pointing at its release assets with their sha256. They
//! install what a release already published, so nothing is built here: the
//! input is the name, the version, the homepage, the license, and the URL
//! and sha256 of each per-target asset. Everything else is text, kept to
//! what Homebrew and Scoop ask for.

use cigscript::diagnostics::{usage, Diagnostic};

/// One release asset: where it lives, its sha256, and the target it is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    /// The target triple this asset is for (`x86_64-unknown-linux-musl`,
    /// `aarch64-apple-darwin`, …). The generators map it to each tool's
    /// idea of a platform.
    pub target: String,
    /// The asset's URL on the release server.
    pub url: String,
    /// The asset's sha256, hex.
    pub sha256: String,
}

/// What a formula or a manifest names besides the per-target assets.
#[derive(Clone, Debug)]
pub struct Release {
    pub name: String,
    pub version: String,
    pub homepage: String,
    pub license: String,
    pub description: String,
    pub assets: Vec<Asset>,
}

impl Release {
    /// Find the one asset for a target, or say so.
    fn asset(&self, target: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.target == target)
    }
}

/// The Homebrew formula (a Ruby class) for a release: one `on_macos` block
/// with an arm64 and an x86_64 branch, one `on_linux` block with the same,
/// and an `assert_predicate` test that the program runs. Only the targets
/// Homebrew supports are written: a release that names no macOS or Linux
/// asset is refused.
pub fn brew(r: &Release) -> Result<String, Diagnostic> {
    const TARGETS: &[(&str, &str, &str)] = &[
        ("aarch64-apple-darwin", "macos", "arm"),
        ("x86_64-apple-darwin", "macos", "intel"),
        ("aarch64-unknown-linux-musl", "linux", "arm"),
        ("x86_64-unknown-linux-musl", "linux", "intel"),
    ];
    let mut by_os: std::collections::BTreeMap<&str, Vec<(&str, &Asset)>> =
        std::collections::BTreeMap::new();
    for (triple, os, arch) in TARGETS {
        if let Some(a) = r.asset(triple) {
            by_os.entry(os).or_default().push((arch, a));
        }
    }
    if by_os.is_empty() {
        return Err(usage(
            "a Homebrew formula names a macOS or Linux asset, and none is in the release",
        )
        .with_hint(
            "give --asset for aarch64-apple-darwin, x86_64-apple-darwin, aarch64-unknown-linux-musl or x86_64-unknown-linux-musl",
        ));
    }
    let class = brew_class_name(&r.name);
    let desc = brew_string(&r.description);
    let homepage = brew_string(&r.homepage);
    let license = brew_string(&r.license);
    let version = brew_string(&r.version);
    let name = brew_string(&r.name);
    let mut out = String::new();
    out.push_str(&format!("class {class} < Formula\n"));
    out.push_str(&format!("  desc {desc}\n"));
    out.push_str(&format!("  homepage {homepage}\n"));
    out.push_str(&format!("  version {version}\n"));
    out.push_str(&format!("  license {license}\n"));
    out.push('\n');
    for (os, branches) in &by_os {
        out.push_str(&format!("  on_{os} do\n"));
        for (arch, a) in branches {
            out.push_str(&format!("    on_{arch} do\n"));
            out.push_str(&format!("      url {}\n", brew_string(&a.url)));
            out.push_str(&format!("      sha256 \"{}\"\n", a.sha256));
            out.push_str("    end\n");
        }
        out.push_str("  end\n");
    }
    out.push('\n');
    out.push_str("  def install\n");
    out.push_str(&format!("    bin.install Dir[\"*\"].first => {name}\n"));
    out.push_str("  end\n");
    out.push('\n');
    out.push_str("  test do\n");
    out.push_str(&format!("    assert_predicate bin/{name}, :exist?\n"));
    out.push_str("  end\n");
    out.push_str("end\n");
    Ok(out)
}

/// The Scoop manifest (a JSON object) for a release: a per-architecture
/// `architecture` block with a URL and a hash for each Windows target, and a
/// top-level `bin` so the program lands on PATH. A release that names no
/// Windows asset is refused.
pub fn scoop(r: &Release) -> Result<String, Diagnostic> {
    // Scoop names architectures: 64bit (x86_64), arm64 (aarch64). 32bit (i686)
    // too, when cigcompile is built for it later.
    const TARGETS: &[(&str, &str)] = &[
        ("x86_64-pc-windows-msvc", "64bit"),
        ("aarch64-pc-windows-msvc", "arm64"),
        ("i686-pc-windows-msvc", "32bit"),
    ];
    let mut archs = serde_json::Map::new();
    for (triple, arch) in TARGETS {
        if let Some(a) = r.asset(triple) {
            archs.insert(
                (*arch).to_string(),
                serde_json::json!({
                    "url": a.url,
                    "hash": a.sha256,
                }),
            );
        }
    }
    if archs.is_empty() {
        return Err(usage(
            "a Scoop manifest names a Windows asset, and none is in the release",
        )
        .with_hint(
            "give --asset for x86_64-pc-windows-msvc, aarch64-pc-windows-msvc or i686-pc-windows-msvc",
        ));
    }
    let bin = format!("{}.exe", r.name);
    let doc = serde_json::json!({
        "version": r.version,
        "description": r.description,
        "homepage": r.homepage,
        "license": r.license,
        "architecture": archs,
        "bin": bin,
    });
    Ok(serde_json::to_string_pretty(&doc).expect("a Scoop manifest serialises") + "\n")
}

/// The class name a Homebrew formula uses: the package name camel-cased,
/// with digits and non-letters stripped, so `tidy-downloads` becomes
/// `TidyDownloads`. Brew's own rule: the file is named after the package
/// and the class after the file's CamelCase.
fn brew_class_name(name: &str) -> String {
    let mut out = String::new();
    let mut up = true;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            if up {
                out.extend(c.to_uppercase());
                up = false;
            } else {
                out.push(c);
            }
        } else {
            up = true;
        }
    }
    if out.is_empty() || !out.chars().next().unwrap().is_ascii_alphabetic() {
        out.insert(0, 'F');
    }
    out
}

/// A Ruby double-quoted string: backslashes and the quote are escaped.
fn brew_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Read a release's inputs from a JSON manifest. The manifest is one object
/// with `name`, `version`, `homepage`, `license` and `description` strings,
/// and an `assets` array of `{target, url, sha256}` objects. It is combined
/// with any `--asset` passed on the command line.
pub fn parse_manifest(text: &str) -> Result<Release, Diagnostic> {
    let doc: serde_json::Value = serde_json::from_str(text).map_err(|e| {
        usage(format!("the manifest is not valid JSON: {e}")).with_hint(
            "name a .json file: {\"name\":\"…\",\"version\":\"…\", …, \"assets\":[{\"target\":\"…\",\"url\":\"…\",\"sha256\":\"…\"}]}",
        )
    })?;
    let field = |k: &str| {
        doc.get(k)
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| usage(format!("the manifest has no string `{k}`")).with_hint("add it"))
    };
    let name = field("name")?;
    let version = field("version")?;
    let homepage = field("homepage")?;
    let license = field("license")?;
    let description = field("description")?;
    let empty = Vec::new();
    let raw_assets = doc
        .get("assets")
        .and_then(serde_json::Value::as_array)
        .unwrap_or(&empty);
    let mut assets = Vec::new();
    for (n, a) in raw_assets.iter().enumerate() {
        let obj = a.as_object().ok_or_else(|| {
            usage(format!("asset {n} is not an object"))
                .with_hint("{\"target\":\"…\",\"url\":\"…\",\"sha256\":\"…\"}")
        })?;
        let get = |k: &str| {
            obj.get(k)
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| {
                    usage(format!("asset {n} has no string `{k}`"))
                        .with_hint("add target, url and sha256 to every asset")
                })
        };
        assets.push(Asset {
            target: get("target")?,
            url: get("url")?,
            sha256: get("sha256")?,
        });
    }
    Ok(Release {
        name,
        version,
        homepage,
        license,
        description,
        assets,
    })
}

/// Parse one `--asset TARGET=URL,SHA256`: the three fields in the order
/// Homebrew and Scoop ask for.
pub fn parse_asset(s: &str) -> Result<Asset, Diagnostic> {
    let (target, rest) = s.split_once('=').ok_or_else(|| {
        usage(format!("the asset {s:?} is not TARGET=URL,SHA256"))
            .with_hint("--asset x86_64-unknown-linux-musl=https://...,abc...")
    })?;
    let (url, sha256) = rest.split_once(',').ok_or_else(|| {
        usage(format!("the asset {s:?} has no sha256")).with_hint("--asset TARGET=URL,SHA256")
    })?;
    if sha256.len() != 64 || !sha256.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(usage(format!(
            "the asset {s:?} has a sha256 that is not 64 hex characters"
        ))
        .with_hint("name the hex sha256 the release published"));
    }
    Ok(Asset {
        target: target.to_string(),
        url: url.to_string(),
        sha256: sha256.to_ascii_lowercase(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Release {
        Release {
            name: "tidy".to_string(),
            version: "1.2.3".to_string(),
            homepage: "https://example.com/tidy".to_string(),
            license: "Apache-2.0".to_string(),
            description: "Tidy downloads for people who do not.".to_string(),
            assets: vec![
                Asset {
                    target: "aarch64-apple-darwin".to_string(),
                    url: "https://example.com/tidy-1.2.3-macos-aarch64".to_string(),
                    sha256: "a".repeat(64),
                },
                Asset {
                    target: "x86_64-unknown-linux-musl".to_string(),
                    url: "https://example.com/tidy-1.2.3-linux-x86_64".to_string(),
                    sha256: "b".repeat(64),
                },
                Asset {
                    target: "x86_64-pc-windows-msvc".to_string(),
                    url: "https://example.com/tidy-1.2.3-windows-x86_64.exe".to_string(),
                    sha256: "c".repeat(64),
                },
            ],
        }
    }

    #[test]
    fn a_brew_formula_names_the_class_the_url_the_sha256_and_a_test() {
        let s = brew(&fixture()).unwrap();
        assert!(s.contains("class Tidy < Formula"), "{s}");
        assert!(s.contains("desc \"Tidy downloads for people who do not.\""));
        assert!(s.contains("homepage \"https://example.com/tidy\""));
        assert!(s.contains("version \"1.2.3\""));
        assert!(s.contains("license \"Apache-2.0\""));
        assert!(s.contains("on_macos do"), "{s}");
        assert!(s.contains("    on_arm do"), "{s}");
        assert!(s.contains("on_linux do"));
        assert!(s.contains(&format!("\"{}\"", "a".repeat(64))));
        assert!(s.contains("assert_predicate"));
        assert_eq!(brew(&fixture()).unwrap(), s, "a rebuild wrote other bytes");
    }

    #[test]
    fn a_brew_formula_without_mac_or_linux_assets_is_refused() {
        let mut r = fixture();
        r.assets.retain(|a| a.target.contains("windows"));
        let e = brew(&r).unwrap_err();
        assert!(e.message.contains("macOS or Linux"), "{}", e.message);
    }

    #[test]
    fn a_scoop_manifest_names_the_hash_and_the_bin() {
        let s = scoop(&fixture()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["version"], "1.2.3");
        assert_eq!(v["architecture"]["64bit"]["hash"], "c".repeat(64));
        assert_eq!(
            v["architecture"]["64bit"]["url"],
            "https://example.com/tidy-1.2.3-windows-x86_64.exe"
        );
        assert_eq!(v["bin"], "tidy.exe");
        assert_eq!(scoop(&fixture()).unwrap(), s);
    }

    #[test]
    fn a_scoop_manifest_without_a_windows_asset_is_refused() {
        let mut r = fixture();
        r.assets.retain(|a| !a.target.contains("windows"));
        let e = scoop(&r).unwrap_err();
        assert!(e.message.contains("Windows"), "{}", e.message);
    }

    #[test]
    fn a_class_name_is_camelcase_and_safe() {
        assert_eq!(brew_class_name("tidy"), "Tidy");
        assert_eq!(brew_class_name("tidy-downloads"), "TidyDownloads");
        assert_eq!(brew_class_name("cig-script"), "CigScript");
        assert_eq!(brew_class_name("1up"), "F1up");
    }

    #[test]
    fn an_asset_is_parsed_from_target_equals_url_comma_sha256() {
        let a = parse_asset(&format!(
            "x86_64-unknown-linux-musl=https://x,{}",
            "a".repeat(64)
        ))
        .unwrap();
        assert_eq!(a.target, "x86_64-unknown-linux-musl");
        assert_eq!(a.url, "https://x");
        assert_eq!(a.sha256, "a".repeat(64));
        assert!(parse_asset("no-equals").is_err());
        assert!(parse_asset("t=u").is_err());
        assert!(parse_asset("t=u,short").is_err());
    }

    #[test]
    fn a_manifest_reads_the_release_and_assets() {
        let text = r#"{
            "name": "tidy",
            "version": "1.2.3",
            "homepage": "https://example.com",
            "license": "Apache-2.0",
            "description": "A thing.",
            "assets": [
                {
                    "target": "x86_64-unknown-linux-musl",
                    "url": "https://example.com/tidy-linux-x86_64",
                    "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                }
            ]
        }"#;
        let r = parse_manifest(text).unwrap();
        assert_eq!(r.name, "tidy");
        assert_eq!(r.assets.len(), 1);
        assert_eq!(r.assets[0].target, "x86_64-unknown-linux-musl");
        let bad = parse_manifest("{\"name\":\"x\"}");
        assert!(bad.is_err(), "a manifest missing fields was accepted");
    }
}
