// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! Runtimes for other machines (#10). Since cig 1.15.0 every cig is a
//! runtime, so the runtime for a target is cig's own release asset for it,
//! of the version this compiler pins: fetched once into
//! `<cig home>/cigcompile/runtimes/<version>/<asset>/`, and accepted only
//! when its sha256 is the one this compiler was built knowing
//! (src/digests.rs). A checksum fetched from the same release as the asset
//! would prove nothing against a release that was tampered with; one built
//! into the compiler does.

use crate::runtime::{wanted, Target};
use cigscript::diagnostics::{usage, Diagnostic};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Where cig is released.
pub const REPO: &str = "otmof-ops/CigScript";

/// The release asset that is the runtime for `t`.
pub fn asset_file(t: &Target) -> String {
    format!("cig-v{}-{}{}", wanted(), t.asset, t.suffix())
}

/// Where a fetched runtime is kept.
pub fn cache_path(t: &Target) -> PathBuf {
    cigscript::burn::runs::home_dir()
        .join("cigcompile")
        .join("runtimes")
        .join(wanted())
        .join(t.asset)
        .join(format!("cig{}", t.suffix()))
}

fn sha256_file(p: &Path) -> Option<String> {
    std::fs::read(p)
        .ok()
        .map(|b| hex::encode(Sha256::digest(b)))
}

fn no(why: String, hint: String) -> Diagnostic {
    usage(why).code("E1001").with_hint(hint)
}

/// The runtime for `t`: the kept copy when its sha256 is the pinned one,
/// else fetched, checked against the pin, and kept.
pub fn fetch(t: &Target, say: bool) -> Result<PathBuf, Diagnostic> {
    if !t.parts.is_empty() {
        return assemble(t, say);
    }
    let name = asset_file(t);
    let dest = cache_path(t);
    let Some(want) = crate::digests::of(&name) else {
        return Err(no(
            format!(
                "cigcompile {} knows the runtimes of cig {} only, and none called {name}",
                env!("CARGO_PKG_VERSION"),
                crate::digests::VERSION
            ),
            format!("pass the cig for {} with --runtime", t.triple),
        ));
    };
    if sha256_file(&dest).as_deref() == Some(want) {
        return Ok(dest);
    }
    if std::env::var_os("CIGCOMPILE_OFFLINE").is_some_and(|v| v != "0") {
        return Err(no(
            format!("{name} is not kept here, and CIGCOMPILE_OFFLINE forbids fetching it"),
            format!("pass the cig for {} with --runtime, or fetch it first: cigcompile fetch --target {}", t.triple, t.triple),
        ));
    }
    if say {
        eprintln!("fetching {name} from {REPO} (sha256 {}...)", &want[..12]);
    }
    let rel = cigscript::update::find(REPO, &format!("v{}", wanted())).map_err(|e| {
        no(
            format!("cannot read cig v{}'s release: {e}", wanted()),
            format!("fetch {name} from {REPO}'s releases yourself and pass it with --runtime"),
        )
    })?;
    let Some(asset) = rel.asset(&name) else {
        return Err(no(
            format!("cig v{}'s release has no {name}", wanted()),
            format!("pass the cig for {} with --runtime", t.triple),
        ));
    };
    let dir = dest.parent().expect("a runtime has a folder");
    std::fs::create_dir_all(dir).map_err(|e| {
        no(
            format!("cannot make {}: {e}", dir.display()),
            "set CIGSCRIPT_HOME to a folder you can write".to_string(),
        )
    })?;
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    if let Err(e) = cigscript::update::download(&asset.url, &tmp) {
        let _ = std::fs::remove_file(&tmp);
        return Err(no(
            format!("cannot fetch {name}: {e}"),
            "check the network, or pass the runtime with --runtime".to_string(),
        ));
    }
    let got = sha256_file(&tmp).unwrap_or_default();
    if got != want {
        let _ = std::fs::remove_file(&tmp);
        return Err(no(
            format!("{name} as fetched has sha256 {got}; this cigcompile accepts only {want}"),
            "nothing was kept; fetch again, and report it if it persists".to_string(),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755));
    }
    std::fs::rename(&tmp, &dest).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        no(
            format!("cannot keep {name} at {}: {e}", dest.display()),
            "set CIGSCRIPT_HOME to a folder you can write".to_string(),
        )
    })?;
    if say {
        eprintln!("kept {} (sha256 matches the pin)", dest.display());
    }
    Ok(dest)
}

/// A target made of others (the universal macOS binary): each part fetched
/// and held to its own pin, then put together. What is kept is made again
/// whenever it is not exactly what the parts make, so it needs no pin of
/// its own.
fn assemble(t: &Target, say: bool) -> Result<PathBuf, Diagnostic> {
    let mut parts = Vec::new();
    for triple in t.parts {
        let part = crate::runtime::target(Some(triple)).expect("a listed part");
        let path = fetch(part, say)?;
        parts.push(std::fs::read(&path).map_err(|e| {
            no(
                format!("cannot read {}: {e}", path.display()),
                format!("fetch it again: cigcompile fetch --target {triple}"),
            )
        })?);
    }
    let refs: Vec<&[u8]> = parts.iter().map(Vec::as_slice).collect();
    let fat = crate::fat::universal(&refs).map_err(|e| {
        no(
            format!("cannot make the runtime for {}: {e}", t.triple),
            format!(
                "fetch the parts again: cigcompile fetch --target {}",
                t.parts.join(" --target ")
            ),
        )
    })?;
    let dest = cache_path(t);
    if std::fs::read(&dest).ok().as_deref() == Some(fat.as_slice()) {
        return Ok(dest);
    }
    let dir = dest.parent().expect("a runtime has a folder");
    let keep = || -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let tmp = dir.join(format!(".cig.{}.tmp", std::process::id()));
        std::fs::write(&tmp, &fat)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
        }
        std::fs::rename(&tmp, &dest).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })
    };
    keep().map_err(|e| {
        no(
            format!(
                "cannot keep the runtime for {} at {}: {e}",
                t.triple,
                dest.display()
            ),
            "set CIGSCRIPT_HOME to a folder you can write".to_string(),
        )
    })?;
    if say {
        eprintln!("made {} from {}", dest.display(), t.parts.join(" and "));
    }
    Ok(dest)
}
