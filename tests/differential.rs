// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! The differential suite (#7): the interpreter is the reference. Every
//! script a CigScript checkout carries (its examples, its error corpus,
//! tests/scripts and the `cig` fences in its docs) runs under `cig` and as
//! the program cigcompile makes of it, and the two must say the same, exit
//! the same and leave the same files.
//!
//! `CIGSCRIPT_SRC` names the checkout (the version cigcompile pins; checked).
//! The interpreter is this build's `cig-runtime`: `cig` of that version.
//! `DIFF_FILTER=<text>` runs only the cases whose name holds it.
//! Without `CIGSCRIPT_SRC` the suite says so and passes: CI always sets it.

#[path = "differential/gen.rs"]
mod gen;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Deserialize)]
struct Manifest {
    case: Vec<Case>,
}

#[derive(Deserialize, Clone, Default)]
struct Case {
    script: String,
    /// The script's text, for a docs fence; read from the checkout otherwise.
    #[serde(skip)]
    inline: Option<String>,
    #[serde(default)]
    files: BTreeMap<String, String>,
    #[serde(default)]
    setup: Vec<Vec<String>>,
    #[serde(default)]
    needs: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    runs: Vec<Run>,
    /// What stdout must be, besides the same on both sides (tests/scripts' `.out`).
    #[serde(skip)]
    expect_stdout: Option<String>,
}

#[derive(Deserialize, Clone)]
struct Run {
    steps: Vec<Step>,
    #[serde(default = "both_modes")]
    modes: Vec<String>,
}

#[derive(Deserialize, Clone, Default)]
struct Step {
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    light: Option<String>,
}

fn both_modes() -> Vec<String> {
    words(&["run", "dry-run", "json", "plain"])
}

fn words(w: &[&str]) -> Vec<String> {
    w.iter().map(|s| s.to_string()).collect()
}

/// A mode's flags, given the same to `cig run` and to the program: `run`
/// is a real run, `dry-run json` is `--dry-run --json`.
fn mode_flags(mode: &str) -> Vec<String> {
    mode.split_whitespace()
        .filter(|w| *w != "run")
        .map(|w| format!("--{w}"))
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
struct Outcome {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

struct Tools {
    cig: PathBuf,
    cc: PathBuf,
}

#[derive(Default)]
struct Tally {
    passed: usize,
    skipped: Vec<String>,
    failed: Vec<String>,
}

fn main() {
    let Some(src) = std::env::var_os("CIGSCRIPT_SRC").map(PathBuf::from) else {
        println!("differential: CIGSCRIPT_SRC is not set; skipped (set it to a CigScript checkout of the version cigcompile pins)");
        return;
    };
    let tools = Tools {
        cig: PathBuf::from(env!("CARGO_BIN_EXE_cig-runtime")),
        cc: PathBuf::from(env!("CARGO_BIN_EXE_cigcompile")),
    };
    let toml_text =
        std::fs::read_to_string(src.join("Cargo.toml")).expect("the checkout's Cargo.toml");
    let theirs = toml_text
        .lines()
        .find_map(|l| {
            l.strip_prefix("version = \"")
                .and_then(|v| v.strip_suffix('"'))
        })
        .expect("a version in the checkout's Cargo.toml");
    assert_eq!(
        theirs,
        cigscript::VERSION,
        "CIGSCRIPT_SRC is CigScript {theirs}; cigcompile pins {}",
        cigscript::VERSION
    );
    let filter = std::env::var("DIFF_FILTER").unwrap_or_default();
    let cases: Vec<Case> = all_cases(&src)
        .into_iter()
        .filter(|c| c.script.contains(&filter))
        .collect();
    let mut tally = Tally::default();
    let started = Instant::now();
    for case in &cases {
        run_case(&tools, &src, case, &mut tally);
    }
    if "explain pages".contains(filter.as_str()) {
        explain_pages(&tools, &mut tally);
    }
    // Random programs (#7): DIFF_RANDOM=<seed>:<count>. A program that
    // differs is shrunk to the fewest statements that still differ.
    if let Some((seed, count)) = std::env::var("DIFF_RANDOM").ok().and_then(|v| {
        let (a, b) = v.split_once(':')?;
        Some((a.parse::<u64>().ok()?, b.parse::<u64>().ok()?))
    }) {
        for k in 0..count {
            let seed = seed.wrapping_add(k);
            let stmts = gen::program(seed);
            let case = random_case(seed, &stmts);
            if !case.script.contains(&filter) {
                continue;
            }
            let before = tally.failed.len();
            run_case(&tools, &src, &case, &mut tally);
            if tally.failed.len() > before {
                let smallest = shrink(&tools, &src, seed, stmts);
                println!(
                    "the smallest program that differs, from seed {seed}:\n{}\n",
                    smallest.join("\n")
                );
            }
        }
    }
    println!(
        "\ndifferential: {} same, {} skipped, {} differ, in {:.0} s",
        tally.passed,
        tally.skipped.len(),
        tally.failed.len(),
        started.elapsed().as_secs_f64()
    );
    for s in &tally.skipped {
        println!("  skipped {s}");
    }
    for f in &tally.failed {
        println!("  DIFFERS {f}");
    }
    if !tally.failed.is_empty() {
        std::process::exit(1);
    }
    if filter.is_empty() {
        assert!(
            tally.passed >= 40,
            "only {} cases ran: did a glob come back empty?",
            tally.passed
        );
    }
}

/// The manifest's cases, then the corpus, tests/scripts and the docs fences
/// found in the checkout. A script in examples/ without a case is a failure.
fn all_cases(src: &Path) -> Vec<Case> {
    let manifest: Manifest =
        toml::from_str(include_str!("differential.toml")).expect("tests/differential.toml");
    let mut cases = manifest.case;
    let named: Vec<String> = cases.iter().map(|c| c.script.clone()).collect();
    for e in cig_files(&src.join("examples")) {
        let rel = format!("examples/{}", e.file_name().unwrap().to_string_lossy());
        assert!(
            named.contains(&rel),
            "{rel} has no case in tests/differential.toml"
        );
    }
    for c in cig_files(&src.join("tests/corpus")) {
        let text = std::fs::read_to_string(&c).unwrap();
        let first = text.lines().next().unwrap_or("");
        // Each error said three ways: as it is, --json and --plain.
        let base = if first.contains("--dry-run") {
            "dry-run"
        } else {
            "run"
        };
        let modes = vec![
            base.to_string(),
            format!("{base} json"),
            format!("{base} plain"),
        ];
        let mut env = BTreeMap::new();
        env.insert("CIG_MAX_STEPS".to_string(), "100000".to_string());
        cases.push(Case {
            script: format!("tests/corpus/{}", c.file_name().unwrap().to_string_lossy()),
            env,
            runs: vec![Run {
                steps: vec![Step::default()],
                modes,
            }],
            ..Case::default()
        });
    }
    for s in cig_files(&src.join("tests/scripts")) {
        let out = s.with_extension("out");
        cases.push(Case {
            script: format!("tests/scripts/{}", s.file_name().unwrap().to_string_lossy()),
            expect_stdout: std::fs::read_to_string(&out).ok(),
            runs: vec![Run {
                steps: vec![Step::default()],
                modes: words(&["run", "json", "plain"]),
            }],
            ..Case::default()
        });
    }
    let mut docs: Vec<PathBuf> = std::fs::read_dir(src.join("docs"))
        .map(|d| {
            d.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "md"))
                .collect()
        })
        .unwrap_or_default();
    docs.push(src.join("README.md"));
    docs.sort();
    for d in docs {
        let text = std::fs::read_to_string(&d).unwrap_or_default();
        for (n, fence) in fences(&text).into_iter().enumerate() {
            let rel = d
                .strip_prefix(src)
                .unwrap_or(&d)
                .to_string_lossy()
                .replace('\\', "/");
            cases.push(Case {
                script: format!("{rel}#{}", n + 1),
                inline: Some(fence),
                // A fence is shown, not run for real: its plan is compared.
                runs: vec![Run {
                    steps: vec![Step::default()],
                    modes: words(&["dry-run", "dry-run json", "dry-run plain"]),
                }],
                ..Case::default()
            });
        }
    }
    cases
}

fn cig_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|d| {
            d.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "cig"))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// The ```cig fences of a Markdown file.
fn fences(md: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Option<String> = None;
    for line in md.lines() {
        let t = line.trim_start();
        match &mut cur {
            None if t == "```cig" => cur = Some(String::new()),
            Some(body) if t.starts_with("```") => {
                out.push(std::mem::take(body));
                cur = None;
            }
            Some(body) => {
                body.push_str(line);
                body.push('\n');
            }
            None => {}
        }
    }
    out
}

fn run_case(tools: &Tools, src: &Path, case: &Case, tally: &mut Tally) {
    for n in &case.needs {
        if which(n).is_none() {
            tally.skipped.push(format!("{} (needs {n})", case.script));
            return;
        }
    }
    let sb = tempfile::tempdir().expect("tempdir");
    let base = case
        .script
        .rsplit('/')
        .next()
        .unwrap()
        .replace(['#', '.'], "-")
        .trim_end_matches("-cig")
        .to_string();
    let script_name = format!("{base}.cig");
    let source = match &case.inline {
        Some(t) => t.clone(),
        None => std::fs::read_to_string(src.join(&case.script)).expect("the script"),
    };
    let work = sb.path().join("work");
    let prog = sb
        .path()
        .join("prog")
        .join(format!("{base}{}", std::env::consts::EXE_SUFFIX));
    let lay_out = || {
        let _ = std::fs::remove_dir_all(&work);
        let _ = std::fs::remove_dir_all(sb.path().join("home"));
        std::fs::create_dir_all(&work).unwrap();
        std::fs::write(work.join(&script_name), &source).unwrap();
        for (p, text) in &case.files {
            let f = work.join(p);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, text).unwrap();
        }
        for cmd in &case.setup {
            let ok = Command::new(&cmd[0])
                .args(&cmd[1..])
                .current_dir(&work)
                .env("HOME", sb.path().join("h"))
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success());
            assert!(ok, "setup {cmd:?} failed for {}", case.script);
        }
    };
    let env = |c: &mut Command| {
        c.current_dir(&work)
            .env("HOME", sb.path().join("h"))
            .env("CIGSCRIPT_HOME", sb.path().join("home"))
            .env("NO_COLOR", "1")
            .env("CIG_DOCTOR", "0")
            .env("CIG_SEAL_KEY", "file")
            .env("CIG_REPORTS", "never")
            .env("CIG_INTERNAL_TRACKER_DIR", sb.path().join("tracker"))
            .env("CIG_UPDATE_MANIFEST", sb.path().join("no-releases.json"))
            .env("CIGCOMPILE_RUNTIME", &tools.cig)
            .env("TZ", "UTC")
            .env_remove("CI")
            .env_remove("CIG_MAX_STEPS")
            .stdin(Stdio::null());
        for (k, v) in &case.env {
            c.env(k, v);
        }
    };
    // A fence that is a fragment is shown, not a program: the reference's
    // check decides.
    if case.inline.is_some() {
        lay_out();
        let mut c = Command::new(&tools.cig);
        env(&mut c);
        if !run(c.args(["check", &script_name]))
            .code
            .is_some_and(|c| c == 0)
        {
            tally.skipped.push(format!(
                "{} (a fragment: it does not check alone)",
                case.script
            ));
            return;
        }
    }
    // Compiled once. A script that does not check is refused by the
    // compiler exactly as `cig run` refuses it.
    lay_out();
    let mut c = Command::new(&tools.cc);
    env(&mut c);
    let built = run(c.args(["build", &script_name, "-o"]).arg(&prog));
    if built.code != Some(0) {
        let mut r = Command::new(&tools.cig);
        env(&mut r);
        let reference = run(r.args(["run", &script_name]));
        let same =
            reference.code == built.code && normal(&reference.stderr) == normal(&built.stderr);
        report(
            tally,
            &format!("{} (refused by both)", case.script),
            same,
            || format!("cig run: {reference:?}\ncigcompile build: {built:?}"),
        );
        return;
    }
    // The interpreter against itself, once for each mode.
    let mut controlled: Vec<String> = Vec::new();
    for (i, r) in case.runs.iter().enumerate() {
        for mode in &r.modes {
            let name = format!("{} run {} {mode}", case.script, i + 1);
            let json = mode.split_whitespace().any(|w| w == "json");
            let side = |program: bool| -> (Vec<Outcome>, String, String) {
                lay_out();
                let home = sb.path().join("home");
                let mut outs = Vec::new();
                let mut order: Vec<String> = Vec::new();
                for step in &r.steps {
                    let mut c = if program {
                        Command::new(&prog)
                    } else {
                        Command::new(&tools.cig)
                    };
                    env(&mut c);
                    if program {
                        c.args(mode_flags(mode));
                        if let Some(ch) = &step.light {
                            c.args(["--light", ch]);
                        }
                    } else {
                        c.arg(if step.light.is_some() { "light" } else { "run" });
                        c.args(mode_flags(mode));
                        c.arg(&script_name);
                        if let Some(ch) = &step.light {
                            c.arg(ch);
                        }
                    }
                    c.arg("--").args(&step.args);
                    outs.push(run(&mut c));
                    // The runs this step made, after the earlier steps':
                    // two runs in one millisecond still line up the same
                    // on both sides.
                    for f in run_folders(&home) {
                        if !order.contains(&f) {
                            order.push(f);
                        }
                    }
                }
                (outs, tree(&work), journals(&home, &order))
            };
            let (a, ta, ja) = side(false);
            if !controlled.contains(mode) {
                controlled.push(mode.clone());
                let (a2, ta2, ja2) = side(false);
                if canon(&a, false, json) != canon(&a2, false, json) || ta != ta2 || ja != ja2 {
                    report(
                        tally,
                        &format!("{name} (the interpreter against itself)"),
                        false,
                        || {
                            format!("the reference is not deterministic under the normalisation:\n{a:#?}\n{a2:#?}\nfiles:\n{ta}\n{ta2}\njournals:\n{ja}\n=====\n{ja2}")
                        },
                    );
                    continue;
                }
            }
            let (b, tb, jb) = side(true);
            let (ca, cb) = (canon(&a, true, json), canon(&b, false, json));
            let mut same = ca == cb && ta == tb && ja == jb;
            // The .out files are written with Unix paths (CigScript's own
            // tests skip the path-printing ones on Windows too).
            if let Some(want) = case
                .expect_stdout
                .as_ref()
                .filter(|_| cfg!(unix) && mode == "run")
            {
                same &= b.first().is_some_and(|o| &o.stdout == want);
            }
            report(tally, &name, same, || {
                let mut s = String::new();
                for (x, y) in ca.iter().zip(cb.iter()) {
                    if x != y {
                        s += &format!("interpreter: {x:#?}\nprogram:     {y:#?}\n");
                    }
                }
                if ta != tb {
                    s += &format!("files differ:\ninterpreter: {ta}\nprogram:     {tb}\n");
                }
                if ja != jb {
                    s += &format!("journals differ:\ninterpreter:\n{ja}\nprogram:\n{jb}\n");
                }
                if s.is_empty() {
                    s = "stdout differs from the .out file".to_string();
                }
                s
            });
        }
    }
}

/// A generated program as a case: one real run, from a tree with `out/`.
fn random_case(seed: u64, stmts: &[String]) -> Case {
    let mut files = BTreeMap::new();
    files.insert("out/.keep".to_string(), String::new());
    Case {
        script: format!("random/{seed}"),
        inline: Some(format!("{}\n", stmts.join("\n"))),
        files,
        runs: vec![Run {
            steps: vec![Step::default()],
            modes: words(&["run"]),
        }],
        ..Case::default()
    }
}

/// Drop statements, newest first, while the program still differs.
fn shrink(tools: &Tools, src: &Path, seed: u64, mut stmts: Vec<String>) -> Vec<String> {
    let mut k = stmts.len();
    while k > 0 {
        k -= 1;
        let mut candidate = stmts.clone();
        candidate.remove(k);
        let mut t = Tally::default();
        run_case(tools, src, &random_case(seed, &candidate), &mut t);
        if !t.failed.is_empty() {
            stmts = candidate;
        }
    }
    stmts
}

/// Every explain page a program shows (`--explain E…`) is cig's, as text,
/// `--plain` and `--json`: the pages CigScript pins in tests/golden come from
/// the one registry, and a program carries it whole. The codes are the ones
/// `cig explain` lists.
fn explain_pages(tools: &Tools, tally: &mut Tally) {
    let sb = tempfile::tempdir().expect("tempdir");
    let home = sb.path().join("home");
    let script = sb.path().join("x.cig");
    std::fs::write(&script, "exhale 1\n").expect("the script");
    let prog = sb.path().join(format!("x{}", std::env::consts::EXE_SUFFIX));
    let built = Command::new(&tools.cc)
        .arg("build")
        .arg(&script)
        .arg("-o")
        .arg(&prog)
        .env("CIGCOMPILE_RUNTIME", &tools.cig)
        .env("CIGCOMPILE_OFFLINE", "1")
        .env("CIGSCRIPT_HOME", &home)
        .output()
        .expect("cigcompile");
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let quiet = |c: &mut Command| {
        c.env("NO_COLOR", "1")
            .env("CIG_DOCTOR", "0")
            .env("CIG_REPORTS", "never")
            .env("CIG_SEAL_KEY", "file")
            .env("CIGSCRIPT_HOME", &home)
            .env_remove("CI")
            .stdin(Stdio::null());
    };
    let mut index = Command::new(&tools.cig);
    quiet(&mut index);
    let index = index.arg("explain").output().expect("cig explain");
    let code_re = regex::Regex::new(r"(?m)^\s+(E\d{3,4})\s").expect("a literal pattern");
    let codes: Vec<String> = code_re
        .captures_iter(&String::from_utf8_lossy(&index.stdout))
        .map(|c| c[1].to_string())
        .collect();
    assert!(
        codes.len() >= 100,
        "cig explain listed {} codes",
        codes.len()
    );
    for mode in [&[][..], &["--plain"][..], &["--json"][..]] {
        let mut differ = Vec::new();
        for code in &codes {
            let mut a = Command::new(&tools.cig);
            quiet(&mut a);
            let a = a
                .arg("explain")
                .args(mode)
                .arg(code)
                .output()
                .expect("cig explain");
            let mut b = Command::new(&prog);
            quiet(&mut b);
            let b = b
                .args(mode)
                .args(["--explain", code])
                .output()
                .expect("the program");
            if a.status.code() != b.status.code() || a.stdout != b.stdout || a.stderr != b.stderr {
                differ.push(code.clone());
            }
        }
        let name = format!(
            "explain pages ({} codes{})",
            codes.len(),
            mode.first().map(|m| format!(", {m}")).unwrap_or_default()
        );
        report(tally, &name, differ.is_empty(), || {
            format!("a program's --explain differs from cig explain for {differ:?}")
        });
    }
}

fn report(tally: &mut Tally, name: &str, same: bool, why: impl FnOnce() -> String) {
    if same {
        tally.passed += 1;
        println!("same     {name}");
    } else {
        println!("DIFFERS  {name}\n{}", why());
        tally.failed.push(name.to_string());
    }
}

/// Run with a time limit; the outcome, or the limit said as one.
fn run(c: &mut Command) -> Outcome {
    let started = Instant::now();
    let mut child = c
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start the command");
    let out_pipe = child.stdout.take();
    let err_pipe = child.stderr.take();
    let read = |p: Option<std::process::ChildStdout>| {
        std::thread::spawn(move || {
            let mut b = Vec::new();
            if let Some(mut p) = p {
                use std::io::Read;
                let _ = p.read_to_end(&mut b);
            }
            b
        })
    };
    let out_t = read(out_pipe);
    let err_t = std::thread::spawn(move || {
        let mut b = Vec::new();
        if let Some(mut p) = err_pipe {
            use std::io::Read;
            let _ = p.read_to_end(&mut b);
        }
        b
    });
    let code = loop {
        if let Some(s) = child.try_wait().expect("wait") {
            break s.code();
        }
        if started.elapsed() > Duration::from_secs(180) {
            let _ = child.kill();
            let _ = child.wait();
            break Some(-9);
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    Outcome {
        code,
        stdout: String::from_utf8_lossy(&out_t.join().unwrap()).to_string(),
        stderr: String::from_utf8_lossy(&err_t.join().unwrap()).to_string(),
    }
}

/// The outcomes with what may differ made the same: run ids, times,
/// durations; and, for the interpreter, its leading check warnings (a
/// program shows them when it is compiled, not on every start).
fn canon(outs: &[Outcome], interpreter: bool, json: bool) -> Vec<Outcome> {
    let records = |t: String| if json { json_normal(&t) } else { t };
    outs.iter()
        .map(|o| Outcome {
            code: o.code,
            stdout: records(normal(&o.stdout)),
            stderr: records(if interpreter {
                without_warnings(&normal(&o.stderr))
            } else {
                normal(&o.stderr)
            }),
        })
        .collect()
}

/// `--json` output, line by line, line ends kept. Only cig's own run
/// records are read: a line that is an object holding a `run` object. Any
/// other line, a script's own JSON included, is compared as it is. In a
/// record, what each process has its own of goes (`pid`, `pid_started`,
/// and `journal_head`, a hash chain over times), and a program's record is
/// read as the interpreter's.
fn json_normal(s: &str) -> String {
    s.split_inclusive('\n')
        .map(|l| {
            let body = l.trim_end_matches('\n');
            match serde_json::from_str::<serde_json::Value>(body) {
                Ok(mut v) if v.get("run").is_some_and(|r| r.is_object()) => {
                    if let Some(run) = v.get_mut("run") {
                        drop_own(run);
                        as_interpreted(run);
                    }
                    format!("{v}{}", &l[body.len()..])
                }
                _ => l.to_string(),
            }
        })
        .collect()
}

/// A run record's fields that each process has its own of.
fn drop_own(run: &mut serde_json::Value) {
    if let Some(m) = run.as_object_mut() {
        for k in ["pid", "pid_started", "journal_head"] {
            m.remove(k);
        }
    }
}

/// A program's run record names the program as its `script` and carries
/// `program`. It is read as the interpreter's when that name is the
/// program's and its sha256 and cig are the run's: `program` goes and
/// `program.file` becomes `script`, so the file name is compared with the
/// interpreter's. A mismatch is left in place, and differs.
fn as_interpreted(run: &mut serde_json::Value) {
    let Some(run) = run.as_object_mut() else {
        return;
    };
    let file = run.get("program").and_then(|p| {
        let same = p.get("name") == run.get("script")
            && p.get("sha256") == run.get("script_sha256")
            && p.get("cig") == run.get("version");
        if same {
            p.get("file").cloned()
        } else {
            None
        }
    });
    if let Some(f) = file {
        run.remove("program");
        run.insert("script".to_string(), f);
    }
}

/// What the runs left in cig's home, in the order they ran (`order`, the
/// run folders as the steps made them): each run's record, every journal
/// line and the after-state, without what is a process's own (ids, times,
/// pids, the hash chain; folder fingerprints masked), its snapshots by
/// content, and the names of everything in its folder. A dry run leaves no
/// run folder, so for dry runs this compares nothing.
fn journals(home: &Path, order: &[String]) -> String {
    let read = |p: PathBuf| {
        std::fs::read_to_string(p)
            .ok()
            .map(|t| t.replace("\r\n", "\n"))
    };
    let parse = |t: &str| serde_json::from_str::<serde_json::Value>(t).ok();
    let without = |mut v: serde_json::Value, keys: &[&str]| {
        if let Some(m) = v.as_object_mut() {
            for k in keys {
                m.remove(*k);
            }
        }
        // A folder's fingerprint hashes its entries' times (it is how
        // unburn sees newer work), and each side's tree is laid out afresh.
        // A file's sha256 is of its raw bytes, and a file can carry the time
        // it was written (a report's timestamp). Both values are masked and
        // their presence still compared; the contents are compared in the
        // tree and the snapshots, where such times are normalized.
        mask_everywhere(&mut v, "fingerprint");
        mask_everywhere(&mut v, "sha256");
        // A before-state's time is when a file the run wrote moments
        // earlier was written: a process's own, like the run's (found by
        // the random programs).
        mask_everywhere(&mut v, "mtime_ns");
        normal(&v.to_string())
    };
    let mut out = Vec::new();
    for name in order {
        let d = home.join("runs").join(name);
        let mut names: Vec<String> = std::fs::read_dir(&d)
            .map(|r| {
                r.flatten()
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        out.push(format!("run folder: {}", names.join(" ")));
        if let Some(mut v) = read(d.join("run.json")).and_then(|t| parse(&t)) {
            drop_own(&mut v);
            as_interpreted(&mut v);
            out.push(format!(
                "record: {}",
                without(v, &["id", "started", "finished"])
            ));
        }
        for l in read(d.join("journal.jsonl")).unwrap_or_default().lines() {
            match parse(l) {
                Some(v) => out.push(format!("journal: {}", without(v, &["run", "at", "prev"]))),
                None => out.push(format!("journal (not JSON): {l}")),
            }
        }
        if let Some(v) = read(d.join("after.json")).and_then(|t| parse(&t)) {
            out.push(format!("after: {}", without(v, &["recorded"])));
        }
        let snaps = tree(&d.join("snapshots"));
        if !snaps.is_empty() {
            out.push(format!("snapshots: {snaps}"));
        }
    }
    out.join("\n")
}

fn mask_everywhere(v: &mut serde_json::Value, key: &str) {
    match v {
        serde_json::Value::Object(m) => {
            if let Some(x) = m.get_mut(key) {
                *x = serde_json::Value::String(format!("<{key}>"));
            }
            m.values_mut().for_each(|x| mask_everywhere(x, key));
        }
        serde_json::Value::Array(a) => a.iter_mut().for_each(|x| mask_everywhere(x, key)),
        _ => {}
    }
}

/// The run folders in cig's home, by name; a step's new ones are those
/// not seen after the step before it.
fn run_folders(home: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(home.join("runs"))
        .map(|d| {
            d.flatten()
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn normal(s: &str) -> String {
    let rules = [
        (r"\d{8}T\d{6}-[0-9a-f]{6}", "<run>"),
        (
            r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(\.\d+)?(Z|[+-]\d\d:\d\d)?",
            "<time>",
        ),
        (r"\b\d+ ms\b", "<n> ms"),
    ];
    let mut out = s.replace("\r\n", "\n");
    for (re, to) in rules {
        out = regex::Regex::new(re)
            .unwrap()
            .replace_all(&out, to)
            .to_string();
    }
    out
}

fn without_warnings(s: &str) -> String {
    let mut lines = s.lines().peekable();
    while let Some(l) = lines.peek() {
        if !l.starts_with("warning[") {
            break;
        }
        lines.next();
        while lines
            .peek()
            .is_some_and(|n| n.is_empty() || n.starts_with(' '))
        {
            lines.next();
        }
    }
    let rest: Vec<&str> = lines.collect();
    let mut joined = rest.join("\n");
    if s.ends_with('\n') && !joined.is_empty() {
        joined.push('\n');
    }
    joined
}

/// The work tree, as one line per entry: path, and for a file its sha256
/// (and its mode on Unix). `.git` and cig's own state are left out.
fn tree(root: &Path) -> String {
    let mut lines = Vec::new();
    walk(root, root, &mut lines);
    lines.sort();
    lines.join("; ")
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let rel = p
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if rel == ".git" || rel == ".cigscript" || rel == ".config" {
            continue;
        }
        let Ok(meta) = std::fs::symlink_metadata(&p) else {
            continue;
        };
        if meta.is_dir() {
            out.push(format!("{rel}/"));
            walk(root, &p, out);
        } else {
            // Text is hashed as the streams are compared: a timestamp or a
            // run id written into a file (a report's `generated`) is not a
            // difference between the interpreter and the program.
            let sha = std::fs::read(&p)
                .map(|b| match String::from_utf8(b) {
                    Ok(t) => hex::encode(Sha256::digest(normal(&t).as_bytes())),
                    Err(e) => hex::encode(Sha256::digest(e.as_bytes())),
                })
                .unwrap_or_default();
            #[cfg(unix)]
            let mode = {
                use std::os::unix::fs::PermissionsExt;
                format!(" {:o}", meta.permissions().mode() & 0o777)
            };
            #[cfg(not(unix))]
            let mode = String::new();
            out.push(format!("{rel} {}{mode}", &sha[..12.min(sha.len())]));
        }
    }
}

fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(format!("{program}{}", std::env::consts::EXE_SUFFIX)))
        .find(|p| p.is_file())
}
