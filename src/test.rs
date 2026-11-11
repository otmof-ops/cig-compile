// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! `cigcompile test`: each program beside its script, both with `--dry-run`
//! (nothing burns), on the runtime it was built from. The plan, the output
//! and the exit code must be the same.

use crate::build::{exit, Built};
use crate::runtime::Runtime;
use cigscript::cli::Ctx;
use std::path::Path;
use std::process::{Command, Output};

/// Compare each built program with its script; the number that differ.
pub fn compare(ctx: &Ctx, built: &[Built], runtime: &Runtime) -> i32 {
    let mut differ = 0;
    for b in built {
        let mut script = run(Command::new(&runtime.path).args(["run", "--dry-run", &b.job.file]));
        if let Some(o) = script.as_mut() {
            o.stdout = as_named(&o.stdout, &b.job);
            o.stderr = as_named(&o.stderr, &b.job);
        }
        let program = run(Command::new(absolute(&b.job.out)).arg("--dry-run"));
        let found = differences(&script, &program);
        if ctx.json {
            println!(
                "{}",
                serde_json::json!({"script": b.job.file, "program": b.job.out.to_string_lossy(), "same": found.is_empty(), "differences": found})
            );
        } else if found.is_empty() {
            println!(
                "same     {} (dry run: the same plan, output and exit)",
                b.job.file
            );
        } else {
            println!("DIFFERS  {}", b.job.file);
            for d in &found {
                println!("  {d}");
            }
        }
        if !found.is_empty() {
            differ += 1;
        }
    }
    if differ == 0 {
        exit::OK
    } else {
        exit::DIFFERS
    }
}

/// The interpreter names the script by the path it was given, and a
/// program by its file name (#9): `tools/x.cig:4:8` is said `x.cig:4:8`.
fn as_named(text: &[u8], job: &crate::project::Job) -> Vec<u8> {
    let name = crate::build::recorded_file(job);
    if job.file == name {
        return text.to_vec();
    }
    // The path as a whole word: `mytools/x.cig:4` is not `tools/x.cig:4`.
    let re = regex::Regex::new(&format!(r"(^|[^\w./\\-]){}:(\d)", regex::escape(&job.file)))
        .expect("an escaped path");
    re.replace_all(&String::from_utf8_lossy(text), |c: &regex::Captures| {
        format!("{}{name}:{}", &c[1], &c[2])
    })
    .into_owned()
    .into_bytes()
}

fn absolute(p: &Path) -> std::path::PathBuf {
    if p.is_absolute() || p.components().count() > 1 {
        p.to_path_buf()
    } else {
        Path::new(".").join(p)
    }
}

fn run(c: &mut Command) -> Option<Output> {
    c.env("NO_COLOR", "1")
        .stdin(std::process::Stdio::null())
        .output()
        .ok()
}

/// What differs between the script's run and the program's, after the
/// things that are allowed to: run ids, and the script's check warnings
/// (said when it was compiled; a program does not say them again).
pub fn differences(script: &Option<Output>, program: &Option<Output>) -> Vec<String> {
    let (Some(s), Some(p)) = (script, program) else {
        return vec!["one of the two could not be started".to_string()];
    };
    let mut out = Vec::new();
    if s.status.code() != p.status.code() {
        out.push(format!(
            "exit: script {:?}, program {:?}",
            s.status.code(),
            p.status.code()
        ));
    }
    let (so, po) = (normal(&s.stdout), normal(&p.stdout));
    if so != po {
        out.push(first_difference("stdout", &so, &po));
    }
    let (se, pe) = (without_warnings(&normal(&s.stderr)), normal(&p.stderr));
    if se != pe {
        out.push(first_difference("stderr", &se, &pe));
    }
    out
}

/// Run ids (`20261107T010203-abcdef`) become `<run>`.
pub fn normal(b: &[u8]) -> String {
    let re = regex::Regex::new(r"\d{8}T\d{6}-[0-9a-f]{6}").expect("a literal pattern");
    re.replace_all(&String::from_utf8_lossy(b), "<run>")
        .to_string()
}

/// `cig run`'s leading check warnings: each block from a `warning[` line
/// through its indented lines.
pub fn without_warnings(s: &str) -> String {
    let mut lines = s.lines().peekable();
    let mut out = Vec::new();
    while let Some(l) = lines.peek() {
        if l.starts_with("warning[") {
            lines.next();
            while let Some(n) = lines.peek() {
                if n.is_empty() || n.starts_with(' ') {
                    lines.next();
                } else {
                    break;
                }
            }
        } else {
            break;
        }
    }
    out.extend(lines);
    let mut joined = out.join("\n");
    if s.ends_with('\n') && !joined.is_empty() {
        joined.push('\n');
    }
    joined
}

fn first_difference(what: &str, a: &str, b: &str) -> String {
    let (al, bl): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    for i in 0..al.len().max(bl.len()) {
        let (x, y) = (al.get(i), bl.get(i));
        if x != y {
            return format!(
                "{what} line {}: script {:?}, program {:?}",
                i + 1,
                x.copied().unwrap_or("<end>"),
                y.copied().unwrap_or("<end>")
            );
        }
    }
    format!("{what}: the same lines, different line endings")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leading_warnings_go_and_the_rest_stays() {
        let s = "warning[E301 check]: unused\n  --> a.cig:2:3\n   |\n 2 | x\n\nwarning[E302 check]: also\n  = hint: h\nplan: write a.txt\n  more\n";
        assert_eq!(without_warnings(s), "plan: write a.txt\n  more\n");
        assert_eq!(without_warnings("plain\n"), "plain\n");
        assert_eq!(without_warnings(""), "");
    }

    #[test]
    fn run_ids_are_normalised() {
        assert_eq!(normal(b"run 20261107T010203-abcdef ok"), "run <run> ok");
    }

    #[test]
    fn the_typed_path_is_said_as_the_name_and_only_as_a_whole_word() {
        let job = crate::project::Job {
            script: std::path::PathBuf::from("tools/x.cig"),
            file: "tools/x.cig".to_string(),
            name: "x".to_string(),
            out: std::path::PathBuf::from("x"),
        };
        let text =
            "  --> tools/x.cig:4:8\nsee mytools/x.cig:4:8 and tools/x.cig:9:1\ntools/x.cig:1:1\n";
        assert_eq!(
            String::from_utf8(as_named(text.as_bytes(), &job)).unwrap(),
            "  --> x.cig:4:8\nsee mytools/x.cig:4:8 and x.cig:9:1\nx.cig:1:1\n"
        );
    }
}
