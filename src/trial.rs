// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! `cigcompile trial <new>` (#15): before a project trusts a newer cigcompile, this one and that
//! one both build the project and run its tests, and the two are compared. The test output and the
//! plan of every program (each run with `--dry-run`) are compared always; a corpus named with
//! `--differential` is built and really run by both when nothing codegen-relevant differs between
//! them. What differs is said first. Nothing is trusted here: the installer picks versions, so
//! this finds `<new>` by asking it, runs both as child processes in scratch copies of the project,
//! and never changes a pin (ADR 0003).

use crate::build::exit;
use crate::installer;
use cigscript::cli::Ctx;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// The options of one trial.
pub struct Opts<'a> {
    /// The carton (or pin) to try, as `ciginstaller pin carton` takes it.
    pub new: &'a str,
    /// The project: a folder of scripts (default the current folder).
    pub project: &'a Path,
    /// A folder of scripts both sides build and really run, when their codegen flags agree.
    pub differential: Option<&'a Path>,
}

/// The flags whose defaults or meaning shape what a compiler writes. When one differs between the
/// two, a run of the same scripts is no longer a like-for-like check, so the differential is skipped.
const CODEGEN_FLAGS: [&str; 5] = ["--target", "-O", "--passes", "--strength", "--emit"];

/// The most files and bytes copied into a scratch project: a folder that big is not a project.
const COPY_FILES: usize = 20_000;
const COPY_BYTES: u64 = 512 * 1024 * 1024;

/// One thing that differs: which check, about what, and the diff.
struct Difference {
    check: &'static str,
    subject: String,
    diff: String,
}

/// A run that could not be made: said once, exit 3.
struct Cannot(String);

fn cannot<T>(m: impl Into<String>) -> Result<T, Cannot> {
    Err(Cannot(m.into()))
}

pub fn trial(ctx: &Ctx, o: &Opts) -> i32 {
    match run_trial(ctx, o) {
        Ok(code) => code,
        Err(Cannot(m)) => {
            if ctx.json {
                println!("{}", serde_json::json!({"error": m}));
            } else {
                eprintln!("cigcompile trial: {m}");
            }
            if m.starts_with(NO_INSTALLER) {
                installer::MISSING
            } else {
                exit::USAGE
            }
        }
    }
}

const NO_INSTALLER: &str = "no ciginstaller";

/// Ask the installer where cigcompile of carton `new` is: `CIGI_CARTON=<new> ciginstaller which
/// cigcompile --path`. It never fetches for `which`, so a carton that is not installed is said with
/// the command that installs it.
fn locate(new: &str) -> Result<PathBuf, Cannot> {
    let Some(exe) = installer::find() else {
        return cannot(format!(
            "{NO_INSTALLER} here, and it is what says where carton {new} is\n{}",
            installer::missing_message("cigcompile trial", "ciginstaller which cigcompile --path")
        ));
    };
    let out = Command::new(&exe)
        .args(["which", "cigcompile", "--path"])
        .env("CIGI_CARTON", new)
        .stdin(Stdio::null())
        .output();
    let out = match out {
        Ok(o) => o,
        Err(e) => return cannot(format!("cannot start {}: {e}", exe.display())),
    };
    let said = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || path.is_empty() {
        let why = if said.is_empty() {
            "it named no cigcompile".to_string()
        } else {
            said
        };
        return cannot(format!(
            "ciginstaller has no cigcompile for carton {new}: {why}\nfetch it first: ciginstaller install carton --carton {new}"
        ));
    }
    let p = PathBuf::from(path.lines().next().unwrap_or_default());
    if !p.is_file() {
        return cannot(format!(
            "ciginstaller named {} for carton {new}, and it is not a file",
            p.display()
        ));
    }
    Ok(p)
}

fn scratch_dir() -> Result<PathBuf, Cannot> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let dir = std::env::temp_dir().join(format!("cigcompile-trial-{}-{nanos}", std::process::id()));
    std::fs::create_dir(&dir).map_err(|e| {
        Cannot(format!(
            "cannot make a scratch folder {}: {e}",
            dir.display()
        ))
    })?;
    Ok(dir)
}

/// A scratch copy of a project folder: files and folders, never a link, never `bin` (the programs
/// a build writes), `.git` or `target`. The user's own tree is not touched.
fn copy_project(from: &Path, to: &Path, seen: &mut (usize, u64), top: bool) -> Result<(), Cannot> {
    std::fs::create_dir_all(to)
        .map_err(|e| Cannot(format!("cannot write {}: {e}", to.display())))?;
    let rd = std::fs::read_dir(from)
        .map_err(|e| Cannot(format!("cannot read {}: {e}", from.display())))?;
    for e in rd.flatten() {
        let name = e.file_name();
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            let n = name.to_string_lossy();
            if top && matches!(n.as_ref(), "bin" | ".git" | "target") {
                continue;
            }
            copy_project(&e.path(), &to.join(&name), seen, false)?;
        } else if ft.is_file() {
            seen.0 += 1;
            seen.1 += e.metadata().map_or(0, |m| m.len());
            if seen.0 > COPY_FILES || seen.1 > COPY_BYTES {
                return cannot(format!(
                    "{} is too big to copy for a trial (over {COPY_FILES} files or {} MiB); name a folder of scripts",
                    from.display(),
                    COPY_BYTES / 1024 / 1024
                ));
            }
            std::fs::copy(e.path(), to.join(&name))
                .map_err(|e2| Cannot(format!("cannot copy {}: {e2}", e.path().display())))?;
        }
    }
    Ok(())
}

/// What a command printed and how it ended, with the scratch folder said as `<project>`.
struct Said {
    code: Option<i32>,
    text: String,
}

fn said(out: std::io::Result<Output>, scratch: &Path) -> Said {
    match out {
        Ok(o) => {
            let mut text = crate::test::normal(&o.stdout);
            let err = crate::test::normal(&o.stderr);
            if !err.is_empty() {
                text.push_str("--- stderr ---\n");
                text.push_str(&err);
            }
            Said {
                code: o.status.code(),
                text: text.replace(&scratch.to_string_lossy().into_owned(), "<project>"),
            }
        }
        Err(e) => Said {
            code: None,
            text: format!("could not be started: {e}\n"),
        },
    }
}

fn quiet(exe: &Path, cwd: &Path, args: &[&str]) -> std::io::Result<Output> {
    let mut c = Command::new(exe);
    c.args(args)
        .current_dir(cwd)
        .env("NO_COLOR", "1")
        .env("CIG_PLAIN", "1")
        .stdin(Stdio::null());
    retry_busy(&mut c)
}

/// `Command::output`, again for a moment while a program just written is still held open
/// (ETXTBSY on Linux and macOS), as the installer seam does.
fn retry_busy(c: &mut Command) -> std::io::Result<Output> {
    let mut tries = 0;
    loop {
        match c.output() {
            Err(e) if cfg!(unix) && e.raw_os_error() == Some(26) && tries < 50 => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            r => return r,
        }
    }
}

/// The option blocks of `build --help`, by option name: each block's text with whitespace
/// collapsed. clap prints an option on a line of its own starting with `-`, its description after.
fn flag_blocks(help: &str) -> BTreeMap<String, String> {
    let mut map: BTreeMap<String, String> = BTreeMap::new();
    let mut current: Vec<String> = Vec::new();
    for line in help.lines() {
        let t = line.trim();
        if t.starts_with('-') && line.starts_with("  ") {
            current = t
                .split_whitespace()
                .take_while(|w| w.starts_with('-'))
                .map(|w| w.trim_end_matches(',').to_string())
                .collect();
        } else if t.is_empty() || !line.starts_with("  ") {
            current.clear();
        }
        for name in &current {
            let e = map.entry(name.clone()).or_default();
            e.push_str(&t.split_whitespace().collect::<Vec<_>>().join(" "));
            e.push(' ');
        }
    }
    map
}

/// The codegen flags that are not the same on both sides, by name.
fn flags_that_differ(old: &Path, new: &Path, cwd: &Path) -> Vec<String> {
    let help = |exe: &Path| -> BTreeMap<String, String> {
        quiet(exe, cwd, &["build", "--help"])
            .map(|o| flag_blocks(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or_default()
    };
    let (a, b) = (help(old), help(new));
    CODEGEN_FLAGS
        .iter()
        .filter(|f| a.get(**f) != b.get(**f))
        .map(|f| (*f).to_string())
        .collect()
}

fn version_line(exe: &Path, cwd: &Path) -> Result<String, Cannot> {
    let o = quiet(exe, cwd, &["--version"])
        .map_err(|e| Cannot(format!("cannot start {}: {e}", exe.display())))?;
    if !o.status.success() {
        return cannot(format!("{} --version failed", exe.display()));
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// The scripts of a copy and where a build puts each program.
fn programs(dir: &Path) -> Result<Vec<crate::project::Job>, Cannot> {
    let target = crate::runtime::host();
    crate::project::jobs(Some(dir), None, target).map_err(|d| {
        Cannot(format!(
            "nothing to build in {}: {}",
            dir.display(),
            d.message
        ))
    })
}

/// A script as the project names it: relative to the scratch copy it was built in.
fn rel(copy: &Path, job: &crate::project::Job) -> String {
    job.script
        .strip_prefix(copy)
        .unwrap_or(&job.script)
        .to_string_lossy()
        .into_owned()
}

/// What one compiler says of the project: its `test` run, and the plan of each program.
struct Side {
    test: Said,
    plans: Vec<(String, Said)>,
    /// `compiler` and `cig` as each program records them (information, never a difference).
    versions: Vec<(String, String)>,
}

fn run_side(exe: &Path, copy: &Path) -> Result<Side, Cannot> {
    let test = said(quiet(exe, copy, &["test"]), copy);
    let mut plans = Vec::new();
    let mut versions = Vec::new();
    for job in programs(copy)? {
        let prog = if job.out.is_absolute() {
            job.out.clone()
        } else {
            copy.join(&job.out)
        };
        plans.push((
            rel(copy, &job),
            said(quiet(&prog, copy, &["--dry-run"]), copy),
        ));
        if let Ok(o) = quiet(&prog, copy, &["--cig-program", "--json"]) {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&o.stdout) {
                let field = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("?").to_string();
                versions.push((
                    rel(copy, &job),
                    format!("compiler {}, cig {}", field("compiler"), field("cig")),
                ));
            }
        }
    }
    Ok(Side {
        test,
        plans,
        versions,
    })
}

fn compare_said(check: &'static str, subject: &str, a: &Said, b: &Said, out: &mut Vec<Difference>) {
    let mut diff = String::new();
    if a.code != b.code {
        diff.push_str(&format!("exit: old {:?}, new {:?}\n", a.code, b.code));
    }
    if let Some(d) = cigcompile::diff::unified(&a.text, &b.text, 2) {
        diff.push_str(&d);
    }
    if !diff.is_empty() {
        out.push(Difference {
            check,
            subject: subject.to_string(),
            diff,
        });
    }
}

/// The differential: a corpus of scripts built by each side and really run, stdout and exit compared.
fn differential(
    old: &Path,
    new: &Path,
    corpus: &Path,
    root: &Path,
    out: &mut Vec<Difference>,
) -> Result<usize, Cannot> {
    let mut ran = 0;
    let mut results: Vec<Vec<(String, Said)>> = Vec::new();
    for (side, exe) in [("diff-old", old), ("diff-new", new)] {
        let copy = root.join(side);
        copy_project(corpus, &copy, &mut (0, 0), true)?;
        let _ = quiet(exe, &copy, &["build"]);
        let mut said_by = Vec::new();
        for job in programs(&copy)? {
            let prog = if job.out.is_absolute() {
                job.out.clone()
            } else {
                copy.join(&job.out)
            };
            said_by.push((rel(&copy, &job), said(quiet(&prog, &copy, &[]), &copy)));
        }
        results.push(said_by);
    }
    for (a, b) in results[0].iter().zip(results[1].iter()) {
        ran += 1;
        compare_said("differential", &a.0, &a.1, &b.1, out);
    }
    Ok(ran)
}

fn run_trial(ctx: &Ctx, o: &Opts) -> Result<i32, Cannot> {
    let old = std::env::current_exe()
        .map_err(|e| Cannot(format!("cannot find the running cigcompile: {e}")))?;
    let new = locate(o.new)?;
    if let (Ok(a), Ok(b)) = (old.canonicalize(), new.canonicalize()) {
        if a == b {
            return cannot(format!(
                "carton {} holds the cigcompile that is running; there is nothing to compare",
                o.new
            ));
        }
    }
    if !o.project.is_dir() {
        return cannot(format!("{} is not a folder", o.project.display()));
    }
    let root = scratch_dir()?;
    let result = trial_in(ctx, o, &old, &new, &root);
    let _ = std::fs::remove_dir_all(&root);
    result
}

fn trial_in(ctx: &Ctx, o: &Opts, old: &Path, new: &Path, root: &Path) -> Result<i32, Cannot> {
    let (old_v, new_v) = (version_line(old, root)?, version_line(new, root)?);
    let mut seen = (0, 0);
    let (copy_old, copy_new) = (root.join("old"), root.join("new"));
    copy_project(o.project, &copy_old, &mut seen, true)?;
    copy_project(o.project, &copy_new, &mut (0, 0), true)?;
    let (a, b) = (run_side(old, &copy_old)?, run_side(new, &copy_new)?);

    let mut found: Vec<Difference> = Vec::new();
    compare_said("test", "cigcompile test", &a.test, &b.test, &mut found);
    for ((file, pa), (_, pb)) in a.plans.iter().zip(b.plans.iter()) {
        compare_said("plan", file, pa, pb, &mut found);
    }
    let mut notes: Vec<String> = Vec::new();
    for ((file, va), (_, vb)) in a.versions.iter().zip(b.versions.iter()) {
        notes.push(format!(
            "{file}: old program records {va}; new records {vb}"
        ));
    }
    let mut checks = 1 + a.plans.len();
    match o.differential {
        None => notes.push(
            "differential: not run (name a folder of scripts to run for real: --differential <dir>)"
                .to_string(),
        ),
        Some(corpus) => {
            let differ = flags_that_differ(old, new, root);
            if differ.is_empty() {
                checks += differential(old, new, corpus, root, &mut found)?;
            } else {
                notes.push(format!(
                    "differential skipped: {} differs between the two, so the same scripts are not a like-for-like check",
                    differ.join(", ")
                ));
            }
        }
    }

    let new_label = format!("{new_v} ({})", new.display());
    if ctx.json {
        println!(
            "{}",
            serde_json::json!({
                "old": old_v, "new": new_v, "new_path": new.to_string_lossy(),
                "same": found.is_empty(), "checks": checks,
                "differences": found.iter().map(|d| serde_json::json!({"check": d.check, "subject": d.subject, "diff": d.diff})).collect::<Vec<_>>(),
                "notes": notes,
            })
        );
    } else {
        println!("cigcompile trial: this {old_v} against {new_label}");
        if found.is_empty() {
            println!("no differences in {checks} checks");
        } else {
            println!(
                "DIFFERENCES ({}), said before anything is trusted:",
                found.len()
            );
            for d in &found {
                println!("--- {} {}", d.check, d.subject);
                print!("{}", d.diff);
                if !d.diff.ends_with('\n') {
                    println!();
                }
            }
        }
        for n in &notes {
            println!("note: {n}");
        }
        if found.is_empty() {
            println!("{new_v} gives the same answers on this project; to trust it: ciginstaller pin carton {}", o.new);
        } else {
            println!(
                "{} difference{} in {checks} checks; nothing was pinned. Read them, then to trust it anyway: ciginstaller pin carton {}",
                found.len(),
                if found.len() == 1 { "" } else { "s" },
                o.new
            );
        }
    }
    Ok(if found.is_empty() {
        exit::OK
    } else {
        exit::DIFFERS
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn option_blocks_are_read_by_name() {
        let help = "Options:\n  -O <LEVEL>\n          The level.\n          [possible values: 0, 1, 2]\n\n      --passes <A,B>\n          Passes.\n\n  -h, --help\n          Print help\n";
        let m = flag_blocks(help);
        assert!(m["-O"].contains("possible values: 0, 1, 2"), "{m:?}");
        assert!(m["--passes"].contains("Passes."), "{m:?}");
        assert!(!m["--passes"].contains("The level"), "{m:?}");
        assert!(m.contains_key("--help") && m.contains_key("-h"));
    }
}
