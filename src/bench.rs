// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! `cigcompile bench` (#14): the benchmark set from #2, each program built,
//! then run interpreted (`cig run`) and compiled, from fresh folders, the
//! median of several runs. The budgets are held to the stage that exists:
//! one that stage A cannot meet is said as "not yet", never as passed.

use crate::build::exit;
use crate::runtime::Runtime;
use cigscript::cli::Ctx;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

/// The set: what #2 names (file walking, CSV, JSON, string building, a
/// tight numeric loop, a big burn) and a few more.
pub const SET: &[(&str, &str)] = &[
    ("hello", include_str!("../bench/hello.cig")),
    ("loop", include_str!("../bench/loop.cig")),
    ("fib", include_str!("../bench/fib.cig")),
    ("strings", include_str!("../bench/strings.cig")),
    ("maps", include_str!("../bench/maps.cig")),
    ("lists", include_str!("../bench/lists.cig")),
    ("json", include_str!("../bench/json.cig")),
    ("csv", include_str!("../bench/csv.cig")),
    ("files", include_str!("../bench/files.cig")),
    ("burn-big", include_str!("../bench/burn-big.cig")),
];

struct Row {
    name: &'static str,
    build_ms: f64,
    bytes: u64,
    interpreted_ms: f64,
    compiled_ms: f64,
    same: bool,
}

pub fn bench(
    ctx: &Ctx,
    runtime: &Runtime,
    runs: usize,
    budgets: bool,
    orchestrate: Option<&Path>,
) -> i32 {
    let me = std::env::current_exe().expect("this program's path");
    let root = std::env::temp_dir().join(format!("cigcompile-bench-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let env = |c: &mut Command| {
        c.env("CIGSCRIPT_HOME", root.join("home"))
            .env("CIGCOMPILE_RUNTIME", &runtime.path)
            .env("CIG_SEAL_KEY", "file")
            .env("CIG_MAX_STEPS", "0")
            .env("CIG_REPORTS", "never")
            .env("CIG_DOCTOR", "0")
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
    };
    let mut rows = Vec::new();
    for (name, source) in SET {
        let src = root.join("src");
        std::fs::create_dir_all(&src).expect("a temp folder");
        let script = src.join(format!("{name}.cig"));
        std::fs::write(&script, source).expect("write the script");
        let bin = root
            .join("bin")
            .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        let mut c = Command::new(&me);
        env(&mut c);
        c.arg("build").arg(&script).arg("-o").arg(&bin);
        let (build_ms, _) = timed(&mut c);
        let bytes = std::fs::metadata(&bin).map(|m| m.len()).unwrap_or(0);
        let times = |program: bool| -> (f64, String) {
            let mut ms = Vec::new();
            let mut out = String::new();
            for i in 0..runs {
                let work = root
                    .join("work")
                    .join(format!("{name}-{}-{i}", if program { "p" } else { "i" }));
                std::fs::create_dir_all(&work).expect("a run folder");
                std::fs::write(work.join(format!("{name}.cig")), source).expect("the script");
                let mut c = if program {
                    Command::new(&bin)
                } else {
                    Command::new(&runtime.path)
                };
                env(&mut c);
                c.current_dir(&work);
                if !program {
                    c.args(["run", &format!("{name}.cig")]);
                }
                let (t, o) = timed(&mut c);
                ms.push(t);
                out = o;
            }
            (median(ms), out)
        };
        let (interpreted_ms, a) = times(false);
        let (compiled_ms, b) = times(true);
        rows.push(Row {
            name,
            build_ms,
            bytes,
            interpreted_ms,
            compiled_ms,
            same: a == b,
        });
    }
    // A rebuild of hello that changes nothing.
    let mut c = Command::new(&me);
    env(&mut c);
    c.arg("build")
        .arg(root.join("src/hello.cig"))
        .arg("-o")
        .arg(
            root.join("bin")
                .join(format!("hello{}", std::env::consts::EXE_SUFFIX)),
        );
    let noop_ms = median((0..runs.max(3)).map(|_| timed(&mut c).0).collect());
    let orchestrate_ms = orchestrate.map(|p| {
        let mut c = Command::new(&me);
        env(&mut c);
        c.arg("build")
            .arg(p)
            .arg("-o")
            .arg(root.join("bin/orchestrate"));
        timed(&mut c).0
    });
    let _ = std::fs::remove_dir_all(&root);

    let hello = rows.iter().find(|r| r.name == "hello");
    let speedups: Vec<f64> = rows
        .iter()
        .map(|r| r.interpreted_ms / r.compiled_ms.max(0.001))
        .collect();
    let speedup = median(speedups);
    // (what, budget, measured, applies at stage A, met)
    let mut checks: Vec<(String, String, String, bool, bool)> = vec![
        (
            "incremental rebuild, nothing changed".into(),
            "under 50 ms".into(),
            format!("{noop_ms:.1} ms"),
            true,
            noop_ms < 50.0,
        ),
        (
            "start-up of a compiled hello".into(),
            "under 5 ms".into(),
            hello
                .map(|h| format!("{:.1} ms", h.compiled_ms))
                .unwrap_or_default(),
            false,
            hello.is_some_and(|h| h.compiled_ms < 5.0),
        ),
        (
            "compiled vs interpreted, median".into(),
            "at least 3x (stage B)".into(),
            format!("{speedup:.2}x"),
            false,
            speedup >= 3.0,
        ),
        (
            "hello binary size".into(),
            "under 4 MB (the trimmed runtime, #4)".into(),
            hello
                .map(|h| format!("{:.1} MB", h.bytes as f64 / 1e6))
                .unwrap_or_default(),
            false,
            hello.is_some_and(|h| h.bytes < 4_000_000),
        ),
    ];
    if let Some(ms) = orchestrate_ms {
        checks.push((
            "full build of orchestrate.cig".into(),
            "under 10 s".into(),
            format!("{:.2} s", ms / 1000.0),
            true,
            ms < 10_000.0,
        ));
    }
    let differ = rows.iter().filter(|r| !r.same).count();
    let missed = checks.iter().filter(|c| c.3 && !c.4).count();

    if ctx.json {
        let rows: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| serde_json::json!({"name": r.name, "build_ms": r.build_ms, "bytes": r.bytes, "interpreted_ms": r.interpreted_ms, "compiled_ms": r.compiled_ms, "same_output": r.same}))
            .collect();
        let checks: Vec<serde_json::Value> = checks
            .iter()
            .map(|c| serde_json::json!({"what": c.0, "budget": c.1, "measured": c.2, "applies": c.3, "met": c.4}))
            .collect();
        println!(
            "{}",
            serde_json::json!({"runs": runs, "stage": "A", "cig": runtime.version, "programs": rows, "budgets": checks})
        );
    } else {
        println!(
            "stage A (bundle) on cig {}, median of {runs} runs, CIG_SEAL_KEY=file",
            runtime.version
        );
        println!();
        println!(
            "{:<10} {:>10} {:>10} {:>14} {:>12} {:>8}  output",
            "program", "build ms", "size MB", "interpreted ms", "compiled ms", "ratio"
        );
        for r in &rows {
            println!(
                "{:<10} {:>10.1} {:>10.1} {:>14.1} {:>12.1} {:>7.2}x  {}",
                r.name,
                r.build_ms,
                r.bytes as f64 / 1e6,
                r.interpreted_ms,
                r.compiled_ms,
                r.interpreted_ms / r.compiled_ms.max(0.001),
                if r.same { "same" } else { "DIFFERS" }
            );
        }
        println!();
        println!("budgets (#14):");
        for c in &checks {
            let verdict = match (c.3, c.4) {
                (_, true) => "met",
                (true, false) => "MISSED",
                (false, false) => "not yet (a later stage)",
            };
            println!("  {:<38} {:<38} {:>10}  {verdict}", c.0, c.1, c.2);
        }
    }
    if differ > 0 || (budgets && missed > 0) {
        exit::DIFFERS
    } else {
        exit::OK
    }
}

/// Run and time a command; a command that fails is said, never timed as
/// if it had worked.
fn timed(c: &mut Command) -> (f64, String) {
    let started = Instant::now();
    let out = c.stderr(Stdio::piped()).output();
    let ms = started.elapsed().as_secs_f64() * 1000.0;
    match out {
        Ok(o) if o.status.success() => (ms, String::from_utf8_lossy(&o.stdout).to_string()),
        Ok(o) => {
            eprintln!(
                "bench: {:?} failed (exit {:?}):\n{}",
                c,
                o.status.code(),
                String::from_utf8_lossy(&o.stderr)
            );
            (f64::NAN, String::new())
        }
        Err(e) => {
            eprintln!("bench: {c:?} could not start: {e}");
            (f64::NAN, String::new())
        }
    }
}

fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v[v.len() / 2]
}

/// The path of a file given to `--orchestrate`, as it is.
pub fn path(p: &Option<PathBuf>) -> Option<&Path> {
    p.as_deref()
}
