// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! `build --sign <key>` (#10): a detached SSH signature beside a built
//! program, `<program>.sig`, and beside the SBOM, `<program>.cdx.json.sig`.
//! `verify --signature <sig> --allowed-signers <file>` checks it. The
//! signatures use a namespace of their own, `cigcompile-program`, so one
//! made by this compiler cannot be replayed as a git or file signature,
//! and one made with another namespace fails to verify here. The identity
//! and key are the operator's to hold: Apple Developer ID and Authenticode
//! are a separate step (cig-script#153).
//!
//! `ssh-keygen -Y sign` and `-Y verify` are shelled out to: OpenSSH is on
//! every Linux, macOS and Windows 10+ host, so no new crate is compiled in.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The namespace every cigcompile signature carries. A signature made with
/// another namespace fails to verify through this verify, and one made with
/// this namespace is of no use to git or to the signed-files tool chain.
pub const NAMESPACE: &str = "cigcompile-program";

/// Where a program's or an SBOM's signature goes: beside it, with `.sig`
/// appended (the shape `ssh-keygen -Y sign` writes).
pub fn path_for(file: &Path) -> PathBuf {
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "signature".to_string());
    file.with_file_name(format!("{name}.sig"))
}

/// A build's pre-flight for `--sign <key>`: `ssh-keygen` is on this
/// machine and the key file is there and readable. Called before a build
/// starts, so a signer that is not there refuses the build up front and
/// nothing is written. The agent, when it holds the private half of a
/// public-key file, is not asked here: a key whose private half the agent
/// holds is still itself a readable file, and `ssh-keygen -Y sign` is
/// what says `agent refused signature` or `couldn't parse private key` at
/// sign time. With staging, a failure there leaves `--out` alone too.
pub fn precheck_signer(key: &Path) -> Result<(), String> {
    require_ssh_keygen()?;
    let md = std::fs::metadata(key).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!("no signing key at {}", key.display())
        } else {
            format!("cannot read the signing key at {}: {e}", key.display())
        }
    })?;
    if !md.is_file() {
        return Err(format!(
            "{} is not a file, so it is not an ssh key",
            key.display()
        ));
    }
    std::fs::File::open(key)
        .map_err(|e| format!("cannot read the signing key at {}: {e}", key.display()))?;
    Ok(())
}

/// Check `ssh-keygen` is on this machine. Called before a sign or a verify
/// so the message names the one missing thing, not a shell failure.
pub fn require_ssh_keygen() -> Result<(), String> {
    let out = Command::new("ssh-keygen")
        .arg("-?")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match out {
        Ok(_) => Ok(()),
        Err(e) => Err(format!(
            "ssh-keygen is not on this machine: {e}. Install OpenSSH (openssh-client on Debian and Ubuntu, openssh on macOS and Windows 10+) and try again"
        )),
    }
}

/// Sign `file` with the SSH key at `key` and leave the signature at
/// `<file>.sig`. The key is a private-key file, or a public-key file with
/// `ssh-agent` holding the private one: both are what `ssh-keygen -Y sign
/// -f` reads. The namespace is the one this compiler carries.
///
/// A signature kept here from a previous sign is replaced.
pub fn sign(file: &Path, key: &Path) -> Result<(), String> {
    require_ssh_keygen()?;
    // ssh-keygen would ask on stdin to overwrite an existing `.sig`; take
    // it out first so the run is uninterruptible.
    let sig = path_for(file);
    let _ = std::fs::remove_file(&sig);
    let out = Command::new("ssh-keygen")
        .arg("-Y")
        .arg("sign")
        .arg("-f")
        .arg(key)
        .arg("-n")
        .arg(NAMESPACE)
        .arg(file)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("could not run ssh-keygen: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let said = stderr.trim();
        return Err(if said.is_empty() {
            format!(
                "ssh-keygen -Y sign exited {}",
                out.status.code().unwrap_or(-1)
            )
        } else {
            format!("ssh-keygen -Y sign: {said}")
        });
    }
    if !sig.exists() {
        return Err(format!(
            "ssh-keygen -Y sign said nothing but {} is not there",
            sig.display()
        ));
    }
    Ok(())
}

/// The principal an allowed-signers file gives the key that made `sig`
/// (`ssh-keygen -Y find-principals`), or `None` when the file does not
/// list that key.
fn find_principal(allowed: &Path, sig: &Path) -> Result<Option<String>, String> {
    let out = Command::new("ssh-keygen")
        .arg("-Y")
        .arg("find-principals")
        .arg("-f")
        .arg(allowed)
        .arg("-s")
        .arg(sig)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("could not run ssh-keygen: {e}"))?;
    if !out.status.success() {
        return Ok(None);
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(str::to_string))
}

/// One part's verdict the way `verify` reports it.
pub enum Check {
    /// The signature is of these bytes and (with --allowed-signers) comes
    /// from the identity named.
    Ok(String),
    /// The signature is not of these bytes, or the signer is not allowed.
    Differs(String),
    /// A signature alone was checked (structure, namespace, signer's key
    /// used): who made it was not, because no allowed-signers was given.
    Unchecked(String),
}

/// Check the signature at `sig` against the bytes of `file`. With
/// `allowed`, the signer must be in that file and match `identity`
/// (`--identity *` when none is given); without `allowed`, only the
/// signature's structure, its namespace and that the signer's key was used
/// are checked (`-Y check-novalidate`), and the verdict is `Unchecked`.
pub fn check(
    file: &Path,
    sig: &Path,
    allowed: Option<&Path>,
    identity: Option<&str>,
) -> Result<Check, String> {
    require_ssh_keygen()?;
    if !sig.exists() {
        return Err(format!("no signature at {}", sig.display()));
    }
    // With no identity named, the one the allowed-signers file gives the
    // signature's key, as git does: `-I` is matched against each line's
    // principals, so a wildcard would match nothing ordinary.
    let found;
    let identity = match (allowed, identity) {
        (Some(allowed), None) => {
            found = find_principal(allowed, sig)?;
            match found.as_deref() {
                Some(id) => Some(id),
                None => {
                    return Ok(Check::Differs(format!(
                        "the key that made {} is not in {}",
                        sig.display(),
                        allowed.display()
                    )))
                }
            }
        }
        (_, id) => id,
    };
    let input =
        std::fs::File::open(file).map_err(|e| format!("cannot read {}: {e}", file.display()))?;
    let mut cmd = Command::new("ssh-keygen");
    cmd.arg("-Y");
    if let Some(allowed) = allowed {
        let id = identity.unwrap_or_default();
        cmd.arg("verify")
            .arg("-f")
            .arg(allowed)
            .arg("-I")
            .arg(id)
            .arg("-n")
            .arg(NAMESPACE)
            .arg("-s")
            .arg(sig);
    } else {
        cmd.arg("check-novalidate")
            .arg("-n")
            .arg(NAMESPACE)
            .arg("-s")
            .arg(sig);
    }
    // The program is ssh-keygen's stdin straight from the file: a program
    // is tens of megabytes, and nothing needs it in memory.
    let out = cmd
        .stdin(Stdio::from(input))
        .output()
        .map_err(|e| format!("could not run ssh-keygen: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if out.status.success() {
        let said = if !stdout.is_empty() { stdout } else { stderr };
        Ok(if allowed.is_some() {
            Check::Ok(said)
        } else {
            Check::Unchecked(format!(
                "{said} (no --allowed-signers: the signer was not checked)"
            ))
        })
    } else {
        let said = if !stderr.is_empty() { stderr } else { stdout };
        Ok(Check::Differs(said))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_for_puts_sig_beside() {
        let p = Path::new("/tmp/one/two/hi");
        assert_eq!(path_for(p), Path::new("/tmp/one/two/hi.sig"));
        let p = Path::new("hi.cdx.json");
        assert_eq!(path_for(p), Path::new("hi.cdx.json.sig"));
    }

    #[test]
    fn namespace_is_cigcompile_program() {
        // Changing this breaks every signature made so far: if it ever has
        // to change, the old one must still verify in a later cigcompile.
        assert_eq!(NAMESPACE, "cigcompile-program");
    }

    #[test]
    fn precheck_names_a_missing_key_file() {
        let d = tempfile::tempdir().unwrap();
        let e = precheck_signer(&d.path().join("not-here")).unwrap_err();
        assert!(e.contains("no signing key"), "{e}");
    }

    #[test]
    fn precheck_names_a_directory_as_not_a_key() {
        let d = tempfile::tempdir().unwrap();
        let e = precheck_signer(d.path()).unwrap_err();
        assert!(e.contains("not a file") || e.contains("cannot read"), "{e}");
    }
}
