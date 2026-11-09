// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! Staging for `cigcompile build` (#66). A build that writes more than
//! one file (`--sign`, `--sbom`, `--emit oci`, `--emit deb`) assembles
//! each in a folder beside `--out`, on the same filesystem, so the final
//! step is a rename. A failing step leaves `--out` byte-for-byte as it
//! was, including an earlier signed program and its `.sig`; a success
//! moves each file into place.
//!
//! The rename itself is what `write_if_changed` has always done for a
//! single program: `std::fs::rename` is atomic on Unix, and on Windows
//! it is `MoveFileEx` with `MOVEFILE_REPLACE_EXISTING` (what Rust's
//! `rename` already asks for since Rust 1.5), so the old file is
//! replaced in one step on every target this compiler builds for.

use crate::project::Job;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// One job's staging. The build writes to `staged_out`, sign/emit beside
/// it; commit moves every file in `dir` into `final_out.parent()`.
pub struct Stage {
    /// Where the program finally lands (the original `job.out`).
    pub final_out: PathBuf,
    /// The staging folder, beside `final_out`, on the same filesystem.
    pub dir: PathBuf,
    /// The staged program path (`<dir>/<name>`), passed to the build.
    pub staged_out: PathBuf,
}

/// For each job, a staging folder beside `job.out`. The folder is a
/// dotted sibling of the final file so `ls` leaves it out of the
/// default listing, and so tools that look for `<name>` do not match it.
pub fn plan(jobs: &[Job]) -> io::Result<Vec<Stage>> {
    let pid = std::process::id();
    let mut stages = Vec::with_capacity(jobs.len());
    for (i, job) in jobs.iter().enumerate() {
        let parent = parent_or_dot(&job.out);
        fs::create_dir_all(&parent)?;
        let name = file_name_or(&job.out, "program");
        let dir = parent.join(format!(".{name}.cigcompile-{pid}-{i}.stage"));
        // A folder from a crashed build with the same pid and index is
        // unlikely but not impossible. Remove it so the create is clean.
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir(&dir)?;
        let staged_out = dir.join(&name);
        stages.push(Stage {
            final_out: job.out.clone(),
            dir,
            staged_out,
        });
    }
    Ok(stages)
}

/// Move each file in each staging folder into its final place, then drop
/// the staging folder. A file whose bytes are already at the final path,
/// byte for byte, is dropped from staging and not renamed, so a rebuild
/// that changes nothing does not touch mtime (the same rule
/// `write_if_changed` has for a single program). Says, per stage, whether
/// the program itself was replaced.
pub fn commit(stages: Vec<Stage>) -> io::Result<Vec<bool>> {
    let mut replaced = Vec::with_capacity(stages.len());
    for stage in &stages {
        let mut program_replaced = false;
        let final_parent = parent_or_dot(&stage.final_out);
        fs::create_dir_all(&final_parent)?;
        // Rename order does not matter for correctness, but the program
        // itself goes last so a reader that saw an older program sees
        // its matching `.sig`, SBOM and image at the same time or after.
        let mut files: Vec<PathBuf> = fs::read_dir(&stage.dir)?
            .flatten()
            .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
            .map(|e| e.path())
            .collect();
        let staged_name = stage.staged_out.file_name();
        files.sort_by_key(|p| p.file_name() == staged_name);
        for src in &files {
            let name = src
                .file_name()
                .ok_or_else(|| io::Error::other("a staged file has no name"))?;
            let dst = final_parent.join(name);
            if same_bytes(&dst, src)? {
                // The old file keeps its mtime; drop the staged copy.
                fs::remove_file(src)?;
                continue;
            }
            fs::rename(src, &dst)?;
            if src.file_name() == staged_name {
                program_replaced = true;
            }
        }
        fs::remove_dir(&stage.dir)?;
        replaced.push(program_replaced);
    }
    Ok(replaced)
}

/// Remove every staging folder. Called when a step failed, so `--out`
/// stays as it was.
pub fn discard(stages: &[Stage]) {
    for stage in stages {
        let _ = fs::remove_dir_all(&stage.dir);
    }
}

/// What to say in the error when a failing step did not write to
/// `--out`: the file in `--out` is the earlier program (signed, when a
/// `.sig` sits beside it), or there was no program there.
pub fn why_nothing_was_written(stages: &[Stage]) -> String {
    let mut parts = Vec::new();
    for stage in stages {
        let state = state_at(&stage.final_out);
        parts.push(format!("{}: {state}", stage.final_out.display()));
    }
    if parts.is_empty() {
        "nothing was written".to_string()
    } else {
        format!("nothing was written: {}", parts.join("; "))
    }
}

fn state_at(path: &Path) -> &'static str {
    let program = path.is_file();
    let sig = crate::sign::path_for(path).is_file();
    match (program, sig) {
        (false, _) => "there was no program there",
        (true, true) => "it is still the program from before (signed)",
        (true, false) => "it is still the program from before (unsigned)",
    }
}

fn same_bytes(a: &Path, b: &Path) -> io::Result<bool> {
    let (am, bm) = match (fs::metadata(a), fs::metadata(b)) {
        (Ok(am), Ok(bm)) => (am, bm),
        _ => return Ok(false),
    };
    if am.len() != bm.len() {
        return Ok(false);
    }
    // Two reads; cheap at the scale of a program, and only happens on
    // commit after a successful build.
    Ok(fs::read(a)? == fs::read(b)?)
}

fn parent_or_dot(p: &Path) -> PathBuf {
    match p.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

fn file_name_or(p: &Path, fallback: &str) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| fallback.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job_at(out: PathBuf) -> Job {
        Job {
            script: out.with_extension("cig"),
            file: out.file_name().unwrap().to_string_lossy().to_string(),
            name: out.file_stem().unwrap().to_string_lossy().to_string(),
            out,
        }
    }

    #[test]
    fn plan_opens_a_folder_beside_each_out_on_the_same_filesystem() {
        let d = tempfile::tempdir().unwrap();
        let jobs = vec![
            job_at(d.path().join("sub/a")),
            job_at(d.path().join("sub/b")),
        ];
        let stages = plan(&jobs).unwrap();
        for (job, stage) in jobs.iter().zip(&stages) {
            assert_eq!(stage.final_out, job.out);
            assert_eq!(stage.dir.parent().unwrap(), job.out.parent().unwrap());
            assert!(stage.dir.is_dir(), "{} not a folder", stage.dir.display());
            assert_eq!(stage.staged_out.parent().unwrap(), stage.dir);
            assert_eq!(stage.staged_out.file_name(), job.out.file_name());
        }
    }

    #[test]
    fn commit_moves_each_file_into_place() {
        let d = tempfile::tempdir().unwrap();
        let jobs = vec![job_at(d.path().join("sub/a"))];
        let stages = plan(&jobs).unwrap();
        let s = &stages[0];
        fs::write(&s.staged_out, b"program").unwrap();
        fs::write(s.dir.join("a.sig"), b"signature").unwrap();
        fs::write(s.dir.join("a.cdx.json"), b"{}").unwrap();
        commit(stages).unwrap();
        assert_eq!(fs::read(&jobs[0].out).unwrap(), b"program");
        assert_eq!(
            fs::read(jobs[0].out.with_file_name("a.sig")).unwrap(),
            b"signature"
        );
        assert_eq!(
            fs::read(jobs[0].out.with_file_name("a.cdx.json")).unwrap(),
            b"{}"
        );
        // Staging folder is gone.
        assert!(!d.path().join("sub").read_dir().unwrap().any(|e| e
            .as_ref()
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".stage")));
    }

    #[test]
    fn commit_keeps_mtime_of_a_file_whose_bytes_are_already_there() {
        let d = tempfile::tempdir().unwrap();
        let jobs = vec![job_at(d.path().join("sub/a"))];
        // The "before" program.
        fs::create_dir_all(jobs[0].out.parent().unwrap()).unwrap();
        fs::write(&jobs[0].out, b"same").unwrap();
        let mtime = fs::metadata(&jobs[0].out).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let stages = plan(&jobs).unwrap();
        fs::write(&stages[0].staged_out, b"same").unwrap();
        commit(stages).unwrap();
        assert_eq!(
            fs::metadata(&jobs[0].out).unwrap().modified().unwrap(),
            mtime,
            "unchanged bytes mean mtime does not move"
        );
    }

    #[test]
    fn discard_removes_the_folder_and_what_is_in_it() {
        let d = tempfile::tempdir().unwrap();
        let jobs = vec![job_at(d.path().join("a"))];
        let stages = plan(&jobs).unwrap();
        fs::write(&stages[0].staged_out, b"half").unwrap();
        fs::write(stages[0].dir.join("extra"), b"and this").unwrap();
        discard(&stages);
        assert!(!stages[0].dir.exists());
    }

    #[test]
    fn why_nothing_was_written_names_the_state_at_each_out() {
        let d = tempfile::tempdir().unwrap();
        let signed = d.path().join("s");
        fs::write(&signed, b"p").unwrap();
        fs::write(signed.with_file_name("s.sig"), b"sig").unwrap();
        let unsigned = d.path().join("u");
        fs::write(&unsigned, b"p").unwrap();
        let missing = d.path().join("m");
        let stages = vec![
            Stage {
                final_out: signed.clone(),
                dir: d.path().join(".tmp1"),
                staged_out: d.path().join(".tmp1/s"),
            },
            Stage {
                final_out: unsigned.clone(),
                dir: d.path().join(".tmp2"),
                staged_out: d.path().join(".tmp2/u"),
            },
            Stage {
                final_out: missing.clone(),
                dir: d.path().join(".tmp3"),
                staged_out: d.path().join(".tmp3/m"),
            },
        ];
        let s = why_nothing_was_written(&stages);
        assert!(s.contains("still the program from before (signed)"), "{s}");
        assert!(
            s.contains("still the program from before (unsigned)"),
            "{s}"
        );
        assert!(s.contains("there was no program there"), "{s}");
    }
}
