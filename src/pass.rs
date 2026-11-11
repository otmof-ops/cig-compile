// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! Program passes (otmof-ops/cig-script#186). `build --pass` writes a
//! sealed, shareable `program` pass from this machine's tin beside each
//! program, `<program>.pass`; `verify --pass` checks one. The pass names the
//! program by its sha256 and says what it is: the script it carries (by
//! name and sha256), the cig it runs on, what built it, the machine it is
//! for, and its SBOM's sha256 with `--sbom`. It records no path, so it reads
//! the same on any machine. Another machine that trusts this one for
//! programs (its allowed signers name this machine's pass key with the
//! namespace `cigpass-program`; `cig pass key --kinds program` prints the
//! line) knows who built the program, and that it is what was built.

use cigscript::pass::core::Pass;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// The kind of pass a program carries.
pub const KIND: &str = "program";

/// A pass is a line of text; anything longer than this is not one.
const MAX_PASS_FILE: u64 = 1024 * 1024;

/// Where a program's pass goes: `<program>.pass` beside it.
pub fn path_for(program: &Path) -> PathBuf {
    let name = program
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "program".to_string());
    program.with_file_name(format!("{name}.pass"))
}

/// What a program pass says about the program written at `at`, for
/// `target`, with its SBOM at `sbom_at` when there is one.
fn facts(at: &Path, target: &str, sbom_at: Option<&Path>) -> Result<serde_json::Value, String> {
    let bytes = std::fs::read(at).map_err(|e| format!("cannot read {}: {e}", at.display()))?;
    let mut c = std::io::Cursor::new(&bytes);
    let p = cigscript::program::read(&mut c)?
        .ok_or_else(|| format!("{} carries no program", at.display()))?;
    let mut f = serde_json::json!({
        "program_sha256": hex::encode(Sha256::digest(&bytes)),
        "name": p.name,
        "script": p.file,
        "script_sha256": p.sha256,
        "cig": p.cig,
        "compiler": p.compiler,
        "target": target,
    });
    if let Some(s) = sbom_at {
        let b = std::fs::read(s).map_err(|e| format!("cannot read {}: {e}", s.display()))?;
        f["sbom_sha256"] = hex::encode(Sha256::digest(&b)).into();
    }
    Ok(f)
}

/// Make the pass for the program written at `at` and write it beside it
/// (`<at>.pass`). Returns the pass's id, hex.
pub fn write(at: &Path, target: &str, sbom_at: Option<&Path>) -> Result<String, String> {
    let facts = facts(at, target, sbom_at)?;
    let sha = facts["program_sha256"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let p = cigscript::pass::issue_artifact_pass(KIND, sha.as_bytes(), &facts)?;
    let to = path_for(at);
    // A new file in the build's own staging folder: never one already
    // there, and never through a link.
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&to)
        .map_err(|e| format!("cannot write {}: {e}", to.display()))?;
    std::io::Write::write_all(&mut f, format!("{}\n", p.token()).as_bytes())
        .map_err(|e| format!("cannot write {}: {e}", to.display()))?;
    Ok(hex::encode(p.id()))
}

/// Check the pass at `pass_at` for a program whose bytes have the sha256
/// `program_sha256`. `Ok` says who made it, when, and what it says; `Err`
/// says why it does not hold here.
pub fn check(
    pass_at: &Path,
    program_sha256: &str,
    allowed_signers: Option<&Path>,
) -> Result<String, String> {
    let shown = pass_at.display();
    let len = std::fs::metadata(pass_at)
        .map_err(|e| format!("cannot read {shown}: {e}"))?
        .len();
    if len > MAX_PASS_FILE {
        return Err(format!("{shown} is not a pass: it is {len} bytes"));
    }
    let text = std::fs::read_to_string(pass_at).map_err(|e| format!("cannot read {shown}: {e}"))?;
    let p = Pass::from_token(text.trim()).map_err(|e| format!("{shown}: {e}"))?;
    let who = cigscript::pass::check_artifact_pass(
        &p,
        KIND,
        &[(
            "program_sha256",
            program_sha256,
            "it is about another program (the program's sha256 differs)",
        )],
        allowed_signers,
    )
    .map_err(|why| {
        if why.contains("signer is not allowed") {
            format!("{why} (on the machine that built it, `cig pass key --kinds program` prints the line --allowed-signers needs)")
        } else {
            why
        }
    })?;
    let said: serde_json::Value = serde_json::from_slice(&p.result).unwrap_or_default();
    let s = |k: &str| cigscript::pass::core::clean(said[k].as_str().unwrap_or("?"));
    Ok(format!(
        "made by {who} on {}: {} for cig {} on {}, built by {}",
        cigscript::pass::core::iso(p.issued_ms),
        s("script"),
        s("cig"),
        s("target"),
        s("compiler")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pass_goes_beside_its_program() {
        assert_eq!(
            path_for(Path::new("out/hello")),
            PathBuf::from("out/hello.pass")
        );
        assert_eq!(
            path_for(Path::new("out/hello.exe")),
            PathBuf::from("out/hello.exe.pass")
        );
    }

    #[test]
    fn a_file_that_is_not_a_pass_is_named() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("p.pass");
        std::fs::write(&f, "not a pass\n").unwrap();
        let e = check(&f, "00", None).unwrap_err();
        assert!(e.contains("not a pass"), "{e}");
    }
}
