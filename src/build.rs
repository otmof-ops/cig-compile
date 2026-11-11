// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! A build: each script checked as `cig check` checks it, its pin held to
//! the runtime, and the program written beside where it goes and moved into
//! place whole. A program whose bytes are already there is not touched,
//! not even its time.

use crate::project::Job;
use crate::runtime::{Runtime, Target};
use cigscript::cli::Ctx;
use cigscript::diagnostics::{usage, Diagnostic};
use cigscript::program::{self, Program};
use sha2::{Digest, Sha256};
use std::io::Write as _;
use std::path::Path;

/// Exit codes, the same as cig's for the same things.
pub mod exit {
    pub const OK: i32 = 0;
    pub const DIFFERS: i32 = 1;
    pub const CHECK: i32 = 2;
    pub const USAGE: i32 = 3;
}

/// What came of one script.
pub struct Built {
    pub job: Job,
    /// The program's sha256, hex.
    pub sha256: String,
    pub bytes: u64,
    /// False when the program was already there, byte for byte.
    pub written: bool,
}

fn read_script(ctx: &Ctx, job: &Job) -> Result<String, i32> {
    // A program records its script's name and prints it; cig refuses to run
    // one whose name holds a control or invisible character (E1050), so
    // none is made (otmof-ops/cig-script#137).
    let recorded = recorded_file(job);
    if !cigscript::program::printable(&recorded) {
        let d = usage(format!(
            "{}: the script's name holds a control or invisible character, and a program prints the name it was compiled from",
            recorded.escape_debug()
        ))
        .code("E1009")
        .with_hint("rename the script to a name that shows as it is, then build again");
        cigscript::cli::report(ctx, &d, None, None);
        return Err(exit::USAGE);
    }
    std::fs::read_to_string(&job.script).map_err(|e| {
        let d = usage(format!("cannot read {}: {e}", job.file))
            .code("E1000")
            .with_hint("name a .cig script you can read");
        cigscript::cli::report(ctx, &d, None, None);
        exit::USAGE
    })
}

/// Check a script: parse, then the checker, as `cig check` does. The
/// warnings are said here (and kept, to say again when a build changes
/// nothing); a program does not say them again on every start.
pub fn check(ctx: &Ctx, job: &Job, source: &str) -> Result<Vec<String>, i32> {
    let source = source.to_string();
    let parsed = match cigscript::syntax::parse(&source) {
        Ok(p) => p,
        Err(d) => {
            cigscript::cli::report(ctx, &d, Some(&job.file), Some(&source));
            return Err(exit::CHECK);
        }
    };
    let rep = cigscript::check::check(&parsed, &[]);
    if !rep.ok() {
        cigscript::cli::report_all(ctx, &rep.errors, &job.file, &source);
        return Err(exit::CHECK);
    }
    let warnings: Vec<String> = rep
        .warnings
        .iter()
        .map(|w| {
            w.render(Some(&job.file), Some(&source))
                .replacen("error[", "warning[", 1)
        })
        .collect();
    say_warnings(ctx, &warnings);
    Ok(warnings)
}

fn say_warnings(ctx: &Ctx, warnings: &[String]) {
    if !ctx.json {
        for w in warnings {
            eprint!("{}", ctx.yellow(w));
        }
    }
}

/// The script's pin, held to the cig the program will run on.
pub fn pin_holds(job: &Job, source: &str, runtime_version: &str) -> Result<(), Diagnostic> {
    let pin = match cigscript::pin::pin_for(&job.script, source) {
        Ok(Some(p)) => p,
        Ok(None) => return Ok(()),
        Err(e) => {
            return Err(usage(format!("{}: {e}", job.file))
                .code("E1003")
                .with_hint("fix the pin (`# cig >=1.15`), or remove it"))
        }
    };
    let v = cigscript::update::Version::parse(runtime_version)
        .expect("a runtime's version reads (runtime::load checked it)");
    if pin.constraint.satisfied_by(&v) {
        return Ok(());
    }
    Err(usage(format!(
        "{} asks for cig {}, and the program would run on cig {runtime_version}",
        job.file, pin.constraint
    ))
    .code("E1003")
    .with_hint(format!(
        "{}: build with the cigcompile for that cig, or move the pin once the script works on {runtime_version}",
        pin.describe(&job.script)
    )))
}

/// What a program records as its script: the file's name, never the path
/// it was built from. No folder of the build machine is in a program, and
/// a script gives the same bytes however it was named (#9).
pub fn recorded_file(job: &Job) -> String {
    job.script
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| job.file.clone())
}

/// The program's bytes: the runtime with the checked script appended.
pub fn program_bytes(job: &Job, source: &str, runtime: &Runtime) -> std::io::Result<Vec<u8>> {
    let p = Program::new(
        &job.name,
        &recorded_file(job),
        source,
        &format!("cigcompile {}", env!("CARGO_PKG_VERSION")),
    );
    Ok(program::append(runtime.bytes()?, &p))
}

/// Build every job; the first failure stops the build (a later script is
/// not built from a project that is not whole).
///
/// `stage_to`, when passed, redirects where the program (and the SBOM)
/// is written for each job: a staging folder beside `job.out` on the
/// same filesystem. The job's `.out` stays the final path (what the
/// compiler says and what the cache keys on), and a later commit step
/// (`crate::stage::commit`) moves each staged file into place, so a
/// failed sign or emit leaves `--out` as it was (#66). When `stage_to`
/// is set, the fresh-cache fast path is bypassed: a staged file is new,
/// and the up-to-date optimisation would not apply anyway.
pub fn build(
    ctx: &Ctx,
    jobs: &[Job],
    target: &Target,
    runtime: Option<&Runtime>,
    index: &mut crate::cache::Index,
    sbom: bool,
    stage_to: Option<&[std::path::PathBuf]>,
) -> Result<Vec<Built>, i32> {
    if let Some(s) = stage_to {
        assert_eq!(s.len(), jobs.len(), "stage_to must be parallel to jobs");
    }
    use crate::timings::{phase, progress};
    progress(
        serde_json::json!({"event": "start", "scripts": jobs.len(), "check_only": runtime.is_none()}),
    );
    let mut out = Vec::new();
    for (n, job) in jobs.iter().enumerate() {
        progress(
            serde_json::json!({"event": "script", "script": job.file, "n": n + 1, "of": jobs.len()}),
        );
        let source = phase(&job.file, "read", || read_script(ctx, job)).inspect_err(|_| {
            progress(serde_json::json!({"event": "failed", "script": job.file}));
        })?;
        let Some(runtime) = runtime else {
            phase(&job.file, "check", || check(ctx, job, &source)).inspect_err(|_| {
                progress(serde_json::json!({"event": "failed", "script": job.file}));
            })?;
            if let Err(d) = pin_holds(job, &source, crate::runtime::wanted()) {
                cigscript::cli::report(ctx, &d, None, None);
                return Err(exit::USAGE);
            }
            if ctx.json {
                println!("{}", serde_json::json!({"checked": job.file}));
            } else {
                println!("checked {}", job.file);
            }
            progress(serde_json::json!({"event": "checked", "script": job.file}));
            continue;
        };
        let key = crate::cache::key(&[
            env!("CARGO_PKG_VERSION"),
            cigscript::VERSION,
            target.triple,
            &runtime.path.to_string_lossy(),
            &format!("{:?}", runtime.id),
            &job.file,
            &recorded_file(job),
            &job.name,
            &hex::encode(Sha256::digest(source.as_bytes())),
        ]);
        // Where the program (and the SBOM) is written. With staging, a
        // path in the staging folder beside `job.out`; without, `job.out`
        // itself.
        let write_to: &std::path::Path = stage_to.map(|s| s[n].as_path()).unwrap_or(&job.out);
        // Unchanged since it was built: say the warnings again, check the
        // pin (a .cig-version may have come or gone), and touch nothing.
        // The fresh-cache fast path is bypassed with staging: the staged
        // path is a new empty folder, and the up-to-date optimisation
        // would not apply there.
        if stage_to.is_none() {
            if let Some((sha256, warnings)) =
                phase(&job.file, "index", || index.fresh(&job.out, &key))
            {
                say_warnings(ctx, &warnings);
                if let Err(d) = pin_holds(job, &source, &runtime.version) {
                    cigscript::cli::report(ctx, &d, None, None);
                    return Err(exit::USAGE);
                }
                let built = Built {
                    job: job.clone(),
                    sha256,
                    bytes: crate::cache::FileId::of(&job.out)
                        .map(|i| i.len)
                        .unwrap_or(0),
                    written: false,
                };
                if sbom {
                    if let Err(e) =
                        crate::sbom::write(job, &source, &built.sha256, runtime, &job.out)
                    {
                        let d = usage(format!("cannot write the SBOM beside the program: {e}"))
                            .code("E1004")
                            .with_hint("write it somewhere you can with -o");
                        cigscript::cli::report(ctx, &d, None, None);
                        return Err(exit::USAGE);
                    }
                }
                say(ctx, &built, target, runtime);
                progress(
                    serde_json::json!({"event": "built", "script": job.file, "program": job.out.to_string_lossy(), "written": false}),
                );
                out.push(built);
                continue;
            }
        }
        let warnings =
            phase(&job.file, "check", || check(ctx, job, &source)).inspect_err(|_| {
                progress(serde_json::json!({"event": "failed", "script": job.file}));
            })?;
        if let Err(d) = phase(&job.file, "pin", || {
            pin_holds(job, &source, &runtime.version)
        }) {
            cigscript::cli::report(ctx, &d, None, None);
            return Err(exit::USAGE);
        }
        let cannot_write = |e: std::io::Error, what: &str| {
            let d = usage(format!("cannot {what}: {e}"))
                .code("E1004")
                .with_hint(
                    "write it somewhere you can with -o, or free the space the message names",
                );
            cigscript::cli::report(ctx, &d, None, None);
            exit::USAGE
        };
        phase(&job.file, "runtime", || runtime.bytes().map(|_| ())).map_err(|e| {
            cannot_write(e, &format!("read the runtime {}", runtime.path.display()))
        })?;
        let bytes = phase(&job.file, "assemble", || {
            program_bytes(job, &source, runtime)
        })
        .map_err(|e| cannot_write(e, &format!("read the runtime {}", runtime.path.display())))?;
        let sha256 = phase(&job.file, "hash", || hex::encode(Sha256::digest(&bytes)));
        let written = phase(&job.file, "write", || write_if_changed(write_to, &bytes))
            .map_err(|e| cannot_write(e, &format!("write {}", write_to.display())))?;
        // The cache keys on the final path, not the staged one: a later
        // build sees the final file (the commit step is what puts it
        // there) and the sha256 noted here still matches.
        index.note_program(&job.out, &key, &sha256, &warnings);
        if sbom {
            phase(&job.file, "sbom", || {
                crate::sbom::write(job, &source, &sha256, runtime, write_to)
            })
            .map_err(|e| cannot_write(e, "write the SBOM beside the program"))?;
        }
        progress(
            serde_json::json!({"event": "built", "script": job.file, "program": job.out.to_string_lossy(), "written": written}),
        );
        let built = Built {
            job: job.clone(),
            sha256,
            bytes: bytes.len() as u64,
            written,
        };
        say(ctx, &built, target, runtime);
        out.push(built);
    }
    progress(serde_json::json!({"event": "done", "scripts": jobs.len()}));
    Ok(out)
}

fn say(ctx: &Ctx, b: &Built, target: &Target, runtime: &Runtime) {
    if ctx.json {
        println!(
            "{}",
            serde_json::json!({
                "script": b.job.file,
                "program": b.job.out.to_string_lossy(),
                "name": b.job.name,
                "target": crate::runtime::triple_of(target, runtime),
                "needs": runtime.needs.as_ref().filter(|_| crate::runtime::wants_static(target)),
                "runtime": {"path": runtime.path.to_string_lossy(), "cig": runtime.version},
                "bytes": b.bytes,
                "sha256": b.sha256,
                "written": b.written,
            })
        );
        return;
    }
    let size = format!("{:.1} MB", b.bytes as f64 / 1_000_000.0);
    if b.written {
        println!(
            "compiled {} -> {} ({size}, cig {} for {})",
            b.job.file,
            b.job.out.display(),
            runtime.version,
            crate::runtime::label(target, runtime)
        );
    } else {
        println!("up to date: {} ({size})", b.job.out.display());
    }
    println!("  sha256 {}", b.sha256);
}

/// Write `bytes` at `out` unless they are there already. A new program is
/// written beside its place and renamed over it, so a build that stops
/// halfway leaves the old program, or none, never half of one.
pub fn write_if_changed(out: &Path, bytes: &[u8]) -> std::io::Result<bool> {
    if let Ok(meta) = std::fs::metadata(out) {
        if meta.len() == bytes.len() as u64 && std::fs::read(out)? == bytes {
            return Ok(false);
        }
    }
    let dir = match out.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
        _ => std::path::PathBuf::from("."),
    };
    std::fs::create_dir_all(&dir)?;
    let name = out
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "program".to_string());
    let tmp = dir.join(format!(".{name}.cigcompile-{}.tmp", std::process::id()));
    let result = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            f.set_permissions(std::fs::Permissions::from_mode(0o755))?;
        }
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, out)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result.map(|()| true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rewrite_of_the_same_bytes_touches_nothing() {
        let d = tempfile::tempdir().unwrap();
        let out = d.path().join("sub").join("p");
        assert!(write_if_changed(&out, b"one").unwrap());
        let before = std::fs::metadata(&out).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert!(!write_if_changed(&out, b"one").unwrap());
        assert_eq!(std::fs::metadata(&out).unwrap().modified().unwrap(), before);
        assert!(write_if_changed(&out, b"two").unwrap());
        assert_eq!(std::fs::read(&out).unwrap(), b"two");
        let left: Vec<_> = std::fs::read_dir(d.path().join("sub"))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(left.len(), 1, "a temp file was left behind");
    }
}
