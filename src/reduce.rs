// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! `cigcompile reduce` (#33, #39): a script cut down to the smallest one
//! that is still interesting, for a bug report. Interesting is, by default,
//! that its program differs from it as `cigcompile test` finds; or that a
//! command given after `--` exits 0 on it. Lines are removed in ever
//! smaller runs (delta debugging), and a candidate counts only when it
//! still passes `cig check`, so the result is a script anyone can run.

use crate::build::exit;
use cigscript::cli::Ctx;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct Opts<'a> {
    pub script: &'a Path,
    pub out: Option<&'a Path>,
    pub runtime: Option<&'a Path>,
    /// The interestingness test; empty for "its program differs".
    pub predicate: &'a [String],
}

pub fn reduce(ctx: &Ctx, o: &Opts) -> i32 {
    let fail = |d: cigscript::diagnostics::Diagnostic, code: i32| {
        cigscript::cli::report(ctx, &d, None, None);
        code
    };
    let file = o.script.to_string_lossy().to_string();
    let source = match std::fs::read_to_string(o.script) {
        Ok(s) => s,
        Err(e) => {
            return fail(
                cigscript::diagnostics::usage(format!("cannot read {file}: {e}"))
                    .code("E1000")
                    .with_hint("name a .cig script you can read"),
                exit::USAGE,
            )
        }
    };
    if let Some(d) = check_failure(&source) {
        cigscript::cli::report(ctx, &d, Some(&file), Some(&source));
        return exit::CHECK;
    }
    let name = o
        .script
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "script.cig".to_string());
    let work = match Scratch::new() {
        Ok(d) => d,
        Err(e) => {
            return fail(
                cigscript::diagnostics::usage(format!("no room to try candidates: {e}")),
                exit::USAGE,
            )
        }
    };
    let mut tester = Tester {
        candidate: work.path().join(&name),
        program: work
            .path()
            .join(format!("candidate{}", std::env::consts::EXE_SUFFIX)),
        runtime: o.runtime.map(Path::to_path_buf),
        predicate: o.predicate.to_vec(),
        seen: HashMap::new(),
        runs: 0,
    };
    let lines: Vec<String> = source.lines().map(str::to_string).collect();
    if !tester.interesting(&lines) {
        let what = if o.predicate.is_empty() {
            "its program does what it does (cigcompile test finds no difference)".to_string()
        } else {
            format!("`{}` does not exit 0 on it", o.predicate.join(" "))
        };
        return fail(
            cigscript::diagnostics::usage(format!("{file} is not interesting to begin with: {what}"))
                .with_hint("reduce cuts down a script that shows the problem; give the test after --, with {} for the script's path"),
            exit::USAGE,
        );
    }
    let before = lines.len();
    let reduced = ddmin(lines, &mut |c| tester.interesting(c));
    let out = o.out.map(Path::to_path_buf).unwrap_or_else(|| {
        let stem = o
            .script
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "script".to_string());
        o.script.with_file_name(format!("{stem}.reduced.cig"))
    });
    if let Err(e) = std::fs::write(&out, joined(&reduced)) {
        return fail(
            cigscript::diagnostics::usage(format!("cannot write {}: {e}", out.display())),
            exit::USAGE,
        );
    }
    if ctx.json {
        println!(
            "{}",
            serde_json::json!({"script": file, "out": out.to_string_lossy(), "lines_before": before, "lines_after": reduced.len(), "tests": tester.runs})
        );
    } else {
        println!(
            "reduced {file} from {before} lines to {} in {} tests: {}",
            reduced.len(),
            tester.runs,
            out.display()
        );
    }
    exit::OK
}

/// A folder of its own for the candidates, removed when the reduction ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> std::io::Result<Scratch> {
        let base = std::env::temp_dir();
        for n in 0..100u32 {
            let dir = base.join(format!("cigcompile-reduce-{}-{n}", std::process::id()));
            match std::fs::create_dir(&dir) {
                Ok(()) => return Ok(Scratch(dir)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(std::io::Error::other("every scratch name is taken"))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn joined(lines: &[String]) -> String {
    let mut s = lines.join("\n");
    s.push('\n');
    s
}

/// The first diagnostic `cig check` would give, if any.
fn check_failure(source: &str) -> Option<cigscript::diagnostics::Diagnostic> {
    match cigscript::syntax::parse(source) {
        Err(d) => Some(d),
        Ok(program) => cigscript::check::check(&program, &[])
            .errors
            .into_iter()
            .next(),
    }
}

struct Tester {
    candidate: PathBuf,
    program: PathBuf,
    runtime: Option<PathBuf>,
    predicate: Vec<String>,
    /// Each candidate tried, and what it came to.
    seen: HashMap<String, bool>,
    /// How many times the test ran.
    runs: usize,
}

impl Tester {
    fn interesting(&mut self, lines: &[String]) -> bool {
        let text = joined(lines);
        if let Some(known) = self.seen.get(&text) {
            return *known;
        }
        let verdict = check_failure(&text).is_none() && self.test(&text);
        self.seen.insert(text, verdict);
        verdict
    }

    fn test(&mut self, text: &str) -> bool {
        if std::fs::write(&self.candidate, text).is_err() {
            return false;
        }
        self.runs += 1;
        let path = self.candidate.to_string_lossy().to_string();
        let mut c = if self.predicate.is_empty() {
            let Ok(me) = std::env::current_exe() else {
                return false;
            };
            let mut c = Command::new(me);
            c.arg("test")
                .arg(&self.candidate)
                .arg("-o")
                .arg(&self.program);
            if let Some(rt) = &self.runtime {
                c.arg("--runtime").arg(rt);
            }
            c
        } else {
            let mut args: Vec<String> = self.predicate[1..]
                .iter()
                .map(|a| a.replace("{}", &path))
                .collect();
            if !self.predicate.iter().any(|a| a.contains("{}")) {
                args.push(path.clone());
            }
            let mut c = Command::new(&self.predicate[0]);
            c.args(args);
            c
        };
        let status = c
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        match status {
            Ok(s) if self.predicate.is_empty() => s.code() == Some(exit::DIFFERS),
            Ok(s) => s.success(),
            Err(_) => false,
        }
    }
}

/// Delta debugging over lines (Zeller's ddmin, by complements): remove
/// runs of lines, halving the run until single lines, keeping each removal
/// that leaves the script interesting. The result is 1-minimal: no single
/// line can go.
fn ddmin(mut lines: Vec<String>, interesting: &mut dyn FnMut(&[String]) -> bool) -> Vec<String> {
    let mut n = 2usize;
    while lines.len() >= 2 {
        let chunk = lines.len().div_ceil(n);
        let mut cut = false;
        let mut start = 0;
        while start < lines.len() {
            let end = (start + chunk).min(lines.len());
            let candidate: Vec<String> = lines[..start]
                .iter()
                .chain(&lines[end..])
                .cloned()
                .collect();
            if interesting(&candidate) {
                lines = candidate;
                n = (n - 1).max(2);
                cut = true;
                break;
            }
            start = end;
        }
        if !cut {
            if n >= lines.len() {
                break;
            }
            n = (n * 2).min(lines.len());
        }
    }
    if lines.len() == 1 && interesting(&[]) {
        lines.clear();
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::ddmin;

    /// The lines the test needs are all that is left, in their order.
    #[test]
    fn ddmin_keeps_exactly_what_the_test_needs() {
        let lines: Vec<String> = (0..40).map(|n| format!("line {n}")).collect();
        let mut calls = 0;
        let kept = ddmin(lines, &mut |c: &[String]| {
            calls += 1;
            c.iter().any(|l| l == "line 7") && c.iter().any(|l| l == "line 31")
        });
        assert_eq!(kept, vec!["line 7".to_string(), "line 31".to_string()]);
        assert!(calls < 200, "{calls} tests");
    }
}
