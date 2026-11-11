// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! `cigcompile __doctor [--json]`: the checks only cigcompile can make, for
//! `ciginstaller doctor --tools` to show beside its own. Hidden, read-only
//! and non-recursive: it writes nothing, fetches nothing, runs no other
//! cigcompile or cig, and never calls `ciginstaller`. It always exits 0;
//! what it found is in what it printed.
//!
//! The shape is the installer doctor's own finding shape:
//! `{"tool","version","findings":[{check,severity,found,because,fix}]}`.

use crate::runtime;
use cigscript::cli::Ctx;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

struct Finding {
    check: &'static str,
    severity: &'static str,
    found: String,
    because: String,
    fix: Option<String>,
}

fn finding(
    check: &'static str,
    severity: &'static str,
    found: impl Into<String>,
    because: impl Into<String>,
    fix: Option<&str>,
) -> Finding {
    Finding {
        check,
        severity,
        found: found.into(),
        because: because.into(),
        fix: fix.map(str::to_string),
    }
}

/// Every check, in the order a build meets the things they look at.
fn findings() -> Vec<Finding> {
    let mut out = Vec::new();
    let home = cigscript::burn::runs::home_dir();
    let host = runtime::host();
    let wanted = runtime::wanted();

    // The runtime a build here would use, found the way a build finds it,
    // but never fetched: cig's release only when it is already kept.
    let mut index = crate::cache::Index::load();
    let resolved = runtime::find_with(host, None, &mut index, false);
    match &resolved {
        Ok(rt) => out.push(finding(
            "runtime",
            "ok",
            format!("cig {} at {} ({})", rt.version, rt.path.display(), source_of(&rt.path, &home)),
            "a build for this machine appends its program to this cig",
            None,
        )),
        Err(d) if host.triple == "this-machine" => out.push(finding(
            "runtime",
            "action",
            d.message.clone(),
            format!("cig is not released for this machine, so no runtime can be fetched; cigcompile {} needs cig {wanted}", env!("CARGO_PKG_VERSION")),
            Some(&format!("pass a cig {wanted} you built with --runtime, or set CIGCOMPILE_RUNTIME")),
        )),
        Err(d) if d.code == "E1001" && std::env::var_os("CIGCOMPILE_RUNTIME").is_none() => out.push(finding(
            "runtime",
            "note",
            format!("no cig {wanted} is kept here"),
            "the first build for this machine fetches cig's release for it, once, and checks it against the sha256 this cigcompile pins",
            Some("cigcompile fetch, to do it ahead of time (a machine that builds offline needs this)"),
        )),
        Err(d) => out.push(finding(
            "runtime",
            "action",
            d.message.clone(),
            "a build would stop here: the runtime it was pointed at, or found, is not usable",
            Some(&format!("pass cig {wanted} with --runtime, or unset CIGCOMPILE_RUNTIME to let a build find one")),
        )),
    }

    // The pin: what `cigcompile --version` says against the runtime in hand.
    let says = format!(
        "cigcompile {} makes programs for cig {wanted}",
        env!("CARGO_PKG_VERSION")
    );
    match &resolved {
        Ok(rt) if rt.version == wanted => out.push(finding(
            "runtime-pin",
            "ok",
            format!("{says}; the runtime is cig {}", rt.version),
            "the runtime and the pin agree",
            None,
        )),
        Ok(rt) => out.push(finding(
            "runtime-pin",
            "action",
            format!("{says}; the runtime is cig {}", rt.version),
            "programs would be built on a cig that is not the one this cigcompile was tested with",
            Some(&format!(
                "use cig {wanted}, or the cigcompile built for cig {}",
                rt.version
            )),
        )),
        Err(_) => out.push(finding(
            "runtime-pin",
            "note",
            says,
            "there is no runtime to compare it with yet",
            None,
        )),
    }

    // The build index: only a shortcut, lost or damaged it is written again.
    let cache = home.join("cache").join("compile").join("index.json");
    out.push(match std::fs::metadata(&cache) {
        Err(_) => finding(
            "cache",
            "ok",
            format!("no build index at {}", cache.display()),
            "nothing has been built here yet, or it was cleared; the next build writes it",
            None,
        ),
        Ok(m) => {
            let valid = std::fs::read(&cache)
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                .is_some_and(|v| v["format"] == 1);
            if valid {
                finding("cache", "ok", format!("build index {}, {}", cache.display(), size(m.len())), "it lets a rebuild that changes nothing touch nothing", None)
            } else {
                finding(
                    "cache",
                    "note",
                    format!("build index {} is not readable as one", cache.display()),
                    "it is only a shortcut: the next build does the work again and writes a good one",
                    Some(&format!("delete {}", cache.display())),
                )
            }
        }
    });

    // The humidor, cig's shelf of versions (cigcompile reads it, never writes).
    let shelf = cigscript::humidor::list();
    let bytes: u64 = shelf
        .iter()
        .filter_map(|(v, _)| cigscript::humidor::binary(v))
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum();
    let pinned_on_shelf = cigscript::update::Version::parse(wanted)
        .and_then(|v| cigscript::humidor::binary(&v))
        .is_some();
    out.push(finding(
        "humidor",
        "ok",
        format!(
            "{} version{} in {}, {}; cig {wanted} is {}",
            shelf.len(),
            if shelf.len() == 1 { "" } else { "s" },
            cigscript::humidor::dir().display(),
            size(bytes),
            if pinned_on_shelf { "there" } else { "not there" }
        ),
        "a build uses the humidor's cig of the pinned version when none is beside cigcompile or on PATH",
        None,
    ));

    // Runtimes fetched for this and other machines.
    let fetched = home.join("cigcompile").join("runtimes");
    let versions = subdirs(&fetched);
    if versions.is_empty() {
        out.push(finding(
            "fetched-runtimes",
            "ok",
            format!("none kept in {}", fetched.display()),
            "runtimes are fetched when a build needs one, and kept",
            None,
        ));
    } else {
        let total = tree_size(&fetched);
        let stale: Vec<String> = versions
            .iter()
            .filter(|v| v.as_str() != wanted)
            .cloned()
            .collect();
        let found = format!(
            "{} kept in {}, {} (cig {})",
            if versions.len() == 1 {
                "1 version".to_string()
            } else {
                format!("{} versions", versions.len())
            },
            fetched.display(),
            size(total),
            versions.join(", ")
        );
        if stale.is_empty() {
            out.push(finding(
                "fetched-runtimes",
                "ok",
                found,
                "these are the runtimes of the pinned cig",
                None,
            ));
        } else {
            out.push(finding(
                "fetched-runtimes",
                "note",
                found,
                format!(
                    "cig {} {} not the one this cigcompile pins, so no build uses {}",
                    stale.join(", "),
                    if stale.len() == 1 { "is" } else { "are" },
                    if stale.len() == 1 { "it" } else { "them" }
                ),
                Some(&format!(
                    "delete the folder of each old version under {}; they are only a cache",
                    fetched.display()
                )),
            ));
        }
    }

    // OpenSSH, for --sign and verify --signature only.
    out.push(match crate::sign::require_ssh_keygen() {
        Ok(()) => finding("openssh", "ok", "ssh-keygen is on this machine", "build --sign and verify --signature use it", None),
        Err(e) => finding(
            "openssh",
            "note",
            e,
            "only `build --sign` and `verify --signature` need it; --pass and everything else do not",
            Some("install OpenSSH (openssh-client on Debian and Ubuntu, openssh on macOS and Windows 10+) if you sign programs"),
        ),
    });
    out
}

/// Where a runtime came from, by where it is.
fn source_of(path: &Path, home: &Path) -> &'static str {
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let path = canon(path);
    if std::env::var_os("CIGCOMPILE_RUNTIME").is_some() {
        return "from CIGCOMPILE_RUNTIME";
    }
    if path.starts_with(canon(&home.join("cigcompile").join("runtimes"))) {
        return "fetched release";
    }
    if path.starts_with(canon(&cigscript::humidor::dir())) {
        return "the humidor";
    }
    let beside = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(canon));
    if path.parent().map(Path::to_path_buf) == beside {
        return "beside cigcompile";
    }
    "on PATH"
}

/// The names of the folders directly inside `dir`, sorted.
fn subdirs(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    v.sort();
    v
}

/// The bytes of the files under `dir`; links are not followed.
fn tree_size(dir: &Path) -> u64 {
    let mut total = 0;
    let mut stack: Vec<PathBuf> = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(e.path()),
                Ok(t) if t.is_file() => total += e.metadata().map(|m| m.len()).unwrap_or(0),
                _ => {}
            }
        }
    }
    total
}

fn size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut n = bytes as f64;
    let mut u = 0;
    while n >= 1024.0 && u < UNITS.len() - 1 {
        n /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{bytes} B")
    } else {
        format!("{n:.1} {}", UNITS[u])
    }
}

/// Print the findings: one JSON document with `--json`, else lines. Always 0.
pub fn run(ctx: &Ctx) -> i32 {
    let found = findings();
    if ctx.json {
        let list: Vec<Value> = found
            .iter()
            .map(|f| {
                json!({
                    "check": f.check,
                    "severity": f.severity,
                    "found": f.found,
                    "because": f.because,
                    "fix": f.fix,
                })
            })
            .collect();
        println!(
            "{}",
            json!({"tool": "cigcompile", "version": env!("CARGO_PKG_VERSION"), "findings": list})
        );
    } else {
        for f in &found {
            println!("{:<7} {}: {}", f.severity, f.check, f.found);
            if f.severity != "ok" {
                println!("        {}", f.because);
                if let Some(fix) = &f.fix {
                    println!("        fix: {fix}");
                }
            }
        }
    }
    crate::build::exit::OK
}
