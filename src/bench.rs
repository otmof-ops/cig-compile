// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! `cigcompile bench` (#14, #30): the benchmark set, each program built, then
//! run interpreted (`cig run`) and compiled, from fresh folders.
//!
//! - **Output first.** A benchmark whose output is not its `.expect` file is
//!   WRONG and gets no time: a fast wrong answer is a bug, not a result.
//! - **Statistics, not one number.** After a warm-up run, each side runs until
//!   it has `--runs` samples or three seconds have passed; the report gives
//!   the median, p10, p90 and the coefficient of variation, and a CV over 5%
//!   is `noisy`.
//! - **References.** `--ref` runs the Python, Node and Rust versions in
//!   `bench/ref` beside them, where those tools are installed, and gives the
//!   ratios.
//! - **The conditions are said:** the CPU, cores, kernel and governor, the
//!   versions, `CIG_SEAL_KEY` and the runtime's sha256.
//!
//! The budgets are held to the stage that exists: one that stage A cannot
//! meet is said as "not yet", never as passed.

use crate::benchgate;
use crate::build::exit;
use crate::runtime::Runtime;
use cigscript::cli::Ctx;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// A benchmark: its script, what it must print, and what it stresses.
/// `expect5x` is the output at scale 5 (`args[0] = "5"`); when it is `Some`
/// the harness times a second pass at that size, and the slope between the
/// two is reported as compute time, apart from start-up. `args` are what
/// the script is given (after `--` for `cig run`, as the program's own
/// arguments when compiled).
pub struct Bench {
    pub name: &'static str,
    pub source: &'static str,
    pub expect: &'static str,
    pub expect5x: Option<&'static str>,
    pub args: &'static [&'static str],
    pub stresses: &'static str,
}

macro_rules! bench {
    ($name:literal, $stresses:literal) => {
        Bench {
            name: $name,
            source: include_str!(concat!("../bench/", $name, ".cig")),
            expect: include_str!(concat!("../bench/", $name, ".expect")),
            expect5x: None,
            args: &[],
            stresses: $stresses,
        }
    };
    ($name:literal, $stresses:literal, from $path:literal, args $args:expr) => {
        Bench {
            name: $name,
            source: include_str!($path),
            expect: include_str!(concat!("../bench/", $name, ".expect")),
            expect5x: None,
            args: $args,
            stresses: $stresses,
        }
    };
    ($name:literal, $stresses:literal, scaled) => {
        Bench {
            name: $name,
            source: include_str!(concat!("../bench/", $name, ".cig")),
            expect: include_str!(concat!("../bench/", $name, ".expect")),
            expect5x: Some(include_str!(concat!("../bench/", $name, ".expect5x"))),
            args: &[],
            stresses: $stresses,
        }
    };
}

/// The set: what #2 names (file walking, CSV, JSON, string building, a tight
/// numeric loop, a big burn), and the classic benchmarks #30 adds. A `scaled`
/// benchmark reads `args[0]` as a size and is run at 1x and 5x, so the
/// compute slope is reported apart from the fixed start-up.
pub const SET: &[Bench] = &[
    bench!("hello", "start-up"),
    bench!("loop", "integer arithmetic in a counted loop"),
    bench!("fib", "calls and returns: fib(25)"),
    bench!("strings", "string building"),
    bench!("maps", "map reads and writes"),
    bench!("lists", "list building and reads"),
    bench!("json", "JSON encode and decode"),
    bench!("csv", "CSV parsing"),
    bench!("files", "walking a tree of files"),
    bench!("burn-big", "a 1,000-write burn and its journal"),
    bench!("nbody", "float arithmetic, map fields"),
    bench!("trees", "allocation and recursion"),
    bench!("spectral", "nested float loops and calls"),
    bench!("mandel", "a float loop with an early exit"),
    bench!("fannkuch", "list index reads, writes and swaps"),
    bench!("closures", "closures made and called"),
    bench!("dispatch", "lambdas kept in a map, called by key"),
    bench!("sieve", "a 300k-element list of booleans"),
    bench!("words", "split, map counting, string compare"),
    bench!("regex-ish", "string find, replace and split", scaled),
    bench!(
        "json-deep",
        "a six-level nested document, parse and walk",
        scaled
    ),
    bench!("strbuild", "s += piece, the quadratic trap", scaled),
    bench!("richards", "a priority scheduler of tasks", scaled),
    bench!("deltablue", "a constraint chain, method-heavy", scaled),
    bench!(
        "burn-mixed",
        "200 small burns of five writes each at 5x",
        scaled
    ),
    bench!(
        "orchestrate",
        "cig-orchestrator's own 12.5k-line orchestrate.cig: help",
        from "../tests/corpus/cig-orchestrator/orchestrate.cig",
        args &["help"]
    ),
    bench!("startup-0", "start-up, no stdlib module touched"),
    bench!("startup-1", "start-up, one stdlib module"),
    bench!("startup-10", "start-up, ten stdlib modules"),
];

/// The reference versions (`--ref`): language, file extension, source.
const REFS: &[(&str, &str, &str)] = &[
    ("hello", "py", include_str!("../bench/ref/hello.py")),
    ("hello", "js", include_str!("../bench/ref/hello.js")),
    ("hello", "rs", include_str!("../bench/ref/hello.rs")),
    ("loop", "py", include_str!("../bench/ref/loop.py")),
    ("loop", "js", include_str!("../bench/ref/loop.js")),
    ("loop", "rs", include_str!("../bench/ref/loop.rs")),
    ("fib", "py", include_str!("../bench/ref/fib.py")),
    ("fib", "js", include_str!("../bench/ref/fib.js")),
    ("fib", "rs", include_str!("../bench/ref/fib.rs")),
    ("nbody", "py", include_str!("../bench/ref/nbody.py")),
    ("nbody", "js", include_str!("../bench/ref/nbody.js")),
    ("nbody", "rs", include_str!("../bench/ref/nbody.rs")),
    ("trees", "py", include_str!("../bench/ref/trees.py")),
    ("trees", "js", include_str!("../bench/ref/trees.js")),
    ("spectral", "py", include_str!("../bench/ref/spectral.py")),
    ("spectral", "js", include_str!("../bench/ref/spectral.js")),
    ("mandel", "py", include_str!("../bench/ref/mandel.py")),
    ("mandel", "js", include_str!("../bench/ref/mandel.js")),
    ("mandel", "rs", include_str!("../bench/ref/mandel.rs")),
    ("fannkuch", "py", include_str!("../bench/ref/fannkuch.py")),
    ("fannkuch", "js", include_str!("../bench/ref/fannkuch.js")),
    ("closures", "py", include_str!("../bench/ref/closures.py")),
    ("closures", "js", include_str!("../bench/ref/closures.js")),
    ("dispatch", "py", include_str!("../bench/ref/dispatch.py")),
    ("dispatch", "js", include_str!("../bench/ref/dispatch.js")),
    ("sieve", "py", include_str!("../bench/ref/sieve.py")),
    ("sieve", "js", include_str!("../bench/ref/sieve.js")),
    ("sieve", "rs", include_str!("../bench/ref/sieve.rs")),
    ("words", "py", include_str!("../bench/ref/words.py")),
    ("words", "js", include_str!("../bench/ref/words.js")),
    ("regex-ish", "py", include_str!("../bench/ref/regex-ish.py")),
    ("regex-ish", "js", include_str!("../bench/ref/regex-ish.js")),
    ("json-deep", "py", include_str!("../bench/ref/json-deep.py")),
    ("json-deep", "js", include_str!("../bench/ref/json-deep.js")),
    ("strbuild", "py", include_str!("../bench/ref/strbuild.py")),
    ("strbuild", "js", include_str!("../bench/ref/strbuild.js")),
    ("strbuild", "rs", include_str!("../bench/ref/strbuild.rs")),
    ("richards", "py", include_str!("../bench/ref/richards.py")),
    ("richards", "js", include_str!("../bench/ref/richards.js")),
    ("deltablue", "py", include_str!("../bench/ref/deltablue.py")),
    ("deltablue", "js", include_str!("../bench/ref/deltablue.js")),
    ("startup-0", "py", include_str!("../bench/ref/startup-0.py")),
    ("startup-0", "js", include_str!("../bench/ref/startup-0.js")),
    ("startup-0", "rs", include_str!("../bench/ref/startup-0.rs")),
    ("startup-1", "py", include_str!("../bench/ref/startup-1.py")),
    ("startup-1", "js", include_str!("../bench/ref/startup-1.js")),
    (
        "startup-10",
        "py",
        include_str!("../bench/ref/startup-10.py"),
    ),
    (
        "startup-10",
        "js",
        include_str!("../bench/ref/startup-10.js"),
    ),
];

/// The languages `--ref` runs, as their columns are headed.
const LANGS: &[(&str, &str)] = &[("py", "Python"), ("js", "Node"), ("rs", "Rust")];

/// How a run was asked for.
pub struct Opts<'a> {
    /// The most samples per side.
    pub runs: usize,
    /// Exit 1 when a budget that applies to this stage is missed.
    pub budgets: bool,
    /// Also time a full build of this script.
    pub orchestrate: Option<&'a Path>,
    /// Run the reference versions too.
    pub refs: bool,
    /// A Markdown table, for a pull request comment.
    pub markdown: bool,
    /// Benchmarks from this folder (`<name>.cig` and `<name>.expect`)
    /// instead of the set.
    pub set: Option<&'a Path>,
    /// One user script to bench on its own: its expected output is what
    /// `cig run <script>` prints with no arguments. Overrides the set.
    pub script: Option<&'a Path>,
    /// A runtime built with `--features alloc-count`: one run of each
    /// benchmark under it gives the allocation counts.
    pub alloc: Option<&'a Path>,
    /// Only these benchmarks of the set (the pull request job's fast set).
    pub only: &'a [String],
    /// `main`'s saved results to gate against (`--against`).
    pub against: &'a [PathBuf],
}

/// How long one side may take, unless it has not got three samples yet.
const PER_SIDE: Duration = Duration::from_secs(3);
/// A coefficient of variation over this is noisy, and not gated.
const NOISY_CV: f64 = 0.05;

/// One side's samples, summarised.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stats {
    pub median: f64,
    pub p10: f64,
    pub p90: f64,
    pub cv: f64,
    pub samples: usize,
    /// Peak resident set of the largest sample, in bytes, where the system
    /// says it.
    pub rss: Option<u64>,
    /// Every sample, in ms, for the gates' Mann-Whitney test (#30).
    pub raw: Vec<f64>,
}

impl Stats {
    pub fn of(ms: &[f64], rss: Option<u64>) -> Stats {
        if ms.is_empty() {
            return Stats::default();
        }
        let mut v = ms.to_vec();
        v.sort_by(|a, b| a.total_cmp(b));
        let at = |q: f64| v[((v.len() - 1) as f64 * q).round() as usize];
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        let var = v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / v.len() as f64;
        Stats {
            median: at(0.5),
            p10: at(0.1),
            p90: at(0.9),
            cv: if mean > 0.0 { var.sqrt() / mean } else { 0.0 },
            samples: v.len(),
            rss,
            raw: ms.to_vec(),
        }
    }

    pub fn noisy(&self) -> bool {
        self.cv > NOISY_CV
    }
}

/// What happened to one implementation of one benchmark.
#[derive(Clone, Debug)]
pub enum Side {
    /// Its output was right; these are its times.
    Timed(Stats),
    /// Its output was not the `.expect` file's.
    Wrong(String),
    /// It did not run (a failed build, or the tool is not installed).
    Absent(String),
}

impl Side {
    fn stats(&self) -> Option<&Stats> {
        match self {
            Side::Timed(s) => Some(s),
            _ => None,
        }
    }
}

struct Row {
    name: String,
    build_ms: f64,
    bytes: u64,
    interpreted: Side,
    compiled: Side,
    /// The 5x pass, where the benchmark declared one (`expect5x`).
    interpreted_5x: Option<Side>,
    compiled_5x: Option<Side>,
    refs: Vec<(&'static str, Side)>,
    refs_5x: Vec<(&'static str, Side)>,
    /// Allocations and bytes allocated by one interpreted run under the
    /// counting runtime (`--alloc`).
    allocs: Option<(u64, u64)>,
}

/// One benchmark the harness measures: its source, its 1x expected output,
/// and the 5x expected output when the script takes a size argument.
struct Loaded {
    name: String,
    source: String,
    expect: String,
    expect5x: Option<String>,
    args: Vec<String>,
}

/// The benchmarks a run measures: the set, a folder's, or one user
/// script. A user script's expected output is what `cig run <script>`
/// prints with no arguments: a single trip through the interpreter that
/// pins what every later sample is compared to.
fn benchmarks(
    runtime: &Runtime,
    set: Option<&Path>,
    script: Option<&Path>,
) -> Result<Vec<Loaded>, String> {
    if let Some(path) = script {
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        if name.is_empty() {
            return Err(format!("{} has no .cig stem", path.display()));
        }
        let source =
            std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let expect = cig_run_output(runtime, path, &source)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        return Ok(vec![Loaded {
            name,
            source,
            expect,
            expect5x: None,
            args: Vec::new(),
        }]);
    }
    let Some(dir) = set else {
        return Ok(SET
            .iter()
            .map(|b| Loaded {
                name: b.name.to_string(),
                source: b.source.to_string(),
                expect: b.expect.to_string(),
                expect5x: b.expect5x.map(|s| s.to_string()),
                args: b.args.iter().map(|a| a.to_string()).collect(),
            })
            .collect());
    };
    let mut out = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut names: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "cig"))
        .collect();
    names.sort();
    for script in names {
        let name = script
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let source = std::fs::read_to_string(&script).map_err(|e| format!("{name}: {e}"))?;
        let expect = std::fs::read_to_string(script.with_extension("expect"))
            .map_err(|_| format!("{name}: no {name}.expect beside it"))?;
        let expect5x = std::fs::read_to_string(script.with_extension("expect5x")).ok();
        out.push(Loaded {
            name,
            source,
            expect,
            expect5x,
            args: Vec::new(),
        });
    }
    Ok(out)
}

/// Run `cig run <script>` once and keep its stdout: the output every
/// later sample of this script is compared with. The script is placed
/// in a scratch folder with the name the user passed, so relative file
/// operations behave as if the script were run there.
fn cig_run_output(runtime: &Runtime, path: &Path, source: &str) -> Result<String, String> {
    let scratch = std::env::temp_dir().join(format!(
        "cigcompile-bench-expect-{}-{}",
        std::process::id(),
        path.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
    ));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).map_err(|e| format!("a scratch folder: {e}"))?;
    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "script.cig".to_string());
    let placed = scratch.join(&file_name);
    std::fs::write(&placed, source).map_err(|e| format!("write {}: {e}", placed.display()))?;
    let mut c = Command::new(&runtime.path);
    c.current_dir(&scratch)
        .env("CIGSCRIPT_HOME", scratch.join("home"))
        .env("CIG_SEAL_KEY", "file")
        .env("CIG_MAX_STEPS", "0")
        .env("CIG_REPORTS", "never")
        .env("CIG_DOCTOR", "0")
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .args(["run", &file_name]);
    let m = measure(&mut c)?;
    let _ = std::fs::remove_dir_all(&scratch);
    Ok(m.stdout)
}

pub fn bench(ctx: &Ctx, runtime: &Runtime, o: &Opts) -> i32 {
    let set = match benchmarks(runtime, o.set, o.script) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("bench: {e}");
            return exit::USAGE;
        }
    };
    let set: Vec<Loaded> = if o.only.is_empty() {
        set
    } else {
        for n in o.only {
            if !set.iter().any(|b| b.name == *n) {
                eprintln!("bench: --only {n}: no such benchmark");
                return exit::USAGE;
            }
        }
        set.into_iter()
            .filter(|b| o.only.contains(&b.name))
            .collect()
    };
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
            .stdin(Stdio::null());
    };
    let tools = if o.refs {
        RefTools::find(&root)
    } else {
        RefTools::default()
    };
    let mut rows = Vec::new();
    for Loaded {
        name,
        source,
        expect,
        expect5x,
        args,
    } in &set
    {
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
        let built = measure(&mut c);
        let build_ms = built.as_ref().map(|m| m.ms).unwrap_or(f64::NAN);
        let bytes = std::fs::metadata(&bin).map(|m| m.len()).unwrap_or(0);
        let fresh = |side: &str, i: usize| {
            let work = root.join("work").join(format!("{name}-{side}-{i}"));
            std::fs::create_dir_all(&work).expect("a run folder");
            std::fs::write(work.join(format!("{name}.cig")), source).expect("the script");
            work
        };
        // The interpreter is given the size after `--`; the compiled
        // program takes it as `argv[1]`. Scale 1 passes nothing: a
        // benchmark that does not take a size argument is unchanged.
        let interpreted = sample(o.runs, expect, |i| {
            let mut c = Command::new(&runtime.path);
            env(&mut c);
            c.current_dir(fresh("i", i))
                .args(["run", &format!("{name}.cig")]);
            if !args.is_empty() {
                c.arg("--").args(args);
            }
            c
        });
        let compiled = match &built {
            Err(e) => Side::Absent(format!("the build failed: {e}")),
            Ok(_) => sample(o.runs, expect, |i| {
                let mut c = Command::new(&bin);
                env(&mut c);
                c.current_dir(fresh("p", i)).args(args);
                c
            }),
        };
        let (interpreted_5x, compiled_5x) = match expect5x.as_deref() {
            None => (None, None),
            Some(e5) => {
                let five = sample(o.runs, e5, |i| {
                    let mut c = Command::new(&runtime.path);
                    env(&mut c);
                    c.current_dir(fresh("i5", i))
                        .args(["run", &format!("{name}.cig"), "--", "5"]);
                    c
                });
                let fivec = match &built {
                    Err(_) => Side::Absent("the build failed".into()),
                    Ok(_) => sample(o.runs, e5, |i| {
                        let mut c = Command::new(&bin);
                        env(&mut c);
                        c.current_dir(fresh("p5", i)).arg("5");
                        c
                    }),
                };
                (Some(five), Some(fivec))
            }
        };
        let allocs = o
            .alloc
            .and_then(|rt| count_allocs(rt, &env, fresh("a", 0), name, expect, args));
        let refs: Vec<(&'static str, Side)> = LANGS
            .iter()
            .filter(|_| o.refs)
            .map(|(lang, _)| (*lang, tools.side(name, lang, o.runs, expect, 1)))
            .collect();
        let refs_5x: Vec<(&'static str, Side)> = match expect5x.as_deref() {
            Some(e5) if o.refs => LANGS
                .iter()
                .map(|(lang, _)| (*lang, tools.side(name, lang, o.runs, e5, 5)))
                .collect(),
            _ => Vec::new(),
        };
        rows.push(Row {
            name: name.clone(),
            build_ms,
            bytes,
            interpreted,
            compiled,
            interpreted_5x,
            compiled_5x,
            refs,
            refs_5x,
            allocs,
        });
    }
    // A rebuild of the first benchmark that changes nothing.
    let noop_ms = set.first().map(|Loaded { name, .. }| {
        let mut c = Command::new(&me);
        env(&mut c);
        c.arg("build")
            .arg(root.join(format!("src/{name}.cig")))
            .arg("-o")
            .arg(
                root.join("bin")
                    .join(format!("{name}{}", std::env::consts::EXE_SUFFIX)),
            );
        let times: Vec<f64> = (0..o.runs.max(3))
            .filter_map(|_| measure(&mut c).ok().map(|m| m.ms))
            .collect();
        Stats::of(&times, None).median
    });
    let orchestrate_ms = o.orchestrate.map(|p| {
        let mut c = Command::new(&me);
        env(&mut c);
        c.arg("build")
            .arg(p)
            .arg("-o")
            .arg(root.join("bin/orchestrate"));
        measure(&mut c).map(|m| m.ms).unwrap_or(f64::NAN)
    });
    let _ = std::fs::remove_dir_all(&root);

    let checks = budgets(&rows, noop_ms, orchestrate_ms);
    let wrong: Vec<&str> = rows
        .iter()
        .filter(|r| {
            let base = [&r.interpreted, &r.compiled]
                .into_iter()
                .chain(r.refs.iter().map(|(_, s)| s));
            let five = r
                .interpreted_5x
                .iter()
                .chain(r.compiled_5x.iter())
                .chain(r.refs_5x.iter().map(|(_, s)| s));
            base.chain(five).any(|s| matches!(s, Side::Wrong(_)))
                || matches!(r.compiled, Side::Absent(_))
        })
        .map(|r| r.name.as_str())
        .collect();
    let missed = checks.iter().filter(|c| c.3 && !c.4).count();
    let mut conditions = Conditions::here(runtime);
    if o.refs {
        conditions.refs = tools.versions();
    }
    if ctx.json {
        print_json(o, &rows, &checks, &conditions, runtime);
    } else if o.markdown {
        print_markdown(o, &rows, &checks, &conditions);
    } else {
        print_table(o, &rows, &checks, &conditions);
    }
    let mut gate_failed = false;
    if !o.against.is_empty() {
        let report = benchgate::Results::load(o.against).and_then(|base| {
            let mut cur = benchgate::Results::default();
            cur.add(&json_value(o, &rows, &checks, &conditions, runtime))?;
            Ok(benchgate::compare(&base, &cur))
        });
        match report {
            Ok(r) => gate_failed = print_gate(ctx.json, o.markdown, &r),
            Err(e) => {
                eprintln!("bench: --against: {e}");
                return exit::USAGE;
            }
        }
    }
    for r in &rows {
        for (what, s) in [("interpreted", &r.interpreted), ("compiled", &r.compiled)]
            .into_iter()
            .chain(r.refs.iter().map(|(l, s)| (*l, s)))
            .chain(
                r.interpreted_5x
                    .iter()
                    .map(|s| ("interpreted (5x)", s))
                    .chain(r.compiled_5x.iter().map(|s| ("compiled (5x)", s))),
            )
        {
            if let Side::Wrong(why) = s {
                eprintln!("bench: {} ({what}) is WRONG: {why}", r.name);
            }
        }
        if let Side::Absent(why) = &r.compiled {
            eprintln!("bench: {} (compiled): {why}", r.name);
        }
    }
    if !wrong.is_empty() || (o.budgets && missed > 0) || gate_failed {
        exit::DIFFERS
    } else {
        exit::OK
    }
}

/// One interpreted run of a benchmark under the counting runtime: how many
/// allocations it made and how many bytes it asked for. The runtime writes
/// its counts to `CIG_ALLOC_FILE` as it exits; a run whose output is wrong,
/// or a runtime that was not built with `--features alloc-count`, gives
/// nothing: never a zero.
fn count_allocs(
    runtime: &Path,
    env: &dyn Fn(&mut Command),
    work: PathBuf,
    name: &str,
    expect: &str,
    args: &[String],
) -> Option<(u64, u64)> {
    let file = work.join("alloc-count.txt");
    let mut c = Command::new(runtime);
    env(&mut c);
    c.current_dir(&work)
        .env("CIG_ALLOC_FILE", &file)
        .args(["run", &format!("{name}.cig")]);
    if !args.is_empty() {
        c.arg("--").args(args);
    }
    let m = measure(&mut c).ok()?;
    if lines(&m.stdout) != lines(expect) {
        return None;
    }
    let text = std::fs::read_to_string(&file).ok()?;
    let mut parts = text.split_whitespace().map(|w| w.parse::<u64>().ok());
    Some((parts.next()??, parts.next()??))
}

/// Say what the regression gate found; true when it fails the run. The
/// table goes to stdout, or to stderr when stdout carries `--json`.
fn print_gate(json: bool, markdown: bool, r: &benchgate::Report) -> bool {
    let text = benchgate::render(r, markdown);
    let failures = r.failures();
    let verdict = if failures.is_empty() {
        "the regression gate passes".to_string()
    } else {
        format!("the regression gate FAILS:\n  {}", failures.join("\n  "))
    };
    if json {
        eprintln!("\n{text}\n{verdict}");
    } else {
        println!("\n{text}\n{verdict}");
    }
    !failures.is_empty()
}

/// `bench --current` and `bench --trend`: the gates over saved results,
/// with nothing run and no runtime looked for.
pub fn gate_only(
    ctx: &Ctx,
    trend: Option<&Path>,
    against: &[PathBuf],
    current: &[PathBuf],
    markdown: bool,
) -> i32 {
    if let Some(dir) = trend {
        return match benchgate::trend(dir) {
            Ok(t) => {
                println!("{}", benchgate::render_trend(&t));
                if t.failed() {
                    exit::DIFFERS
                } else {
                    exit::OK
                }
            }
            Err(e) => {
                eprintln!("bench: --trend: {e}");
                exit::USAGE
            }
        };
    }
    match (
        benchgate::Results::load(against),
        benchgate::Results::load(current),
    ) {
        (Ok(base), Ok(cur)) => {
            if print_gate(ctx.json, markdown, &benchgate::compare(&base, &cur)) {
                exit::DIFFERS
            } else {
                exit::OK
            }
        }
        (Err(e), _) => {
            eprintln!("bench: --against: {e}");
            exit::USAGE
        }
        (_, Err(e)) => {
            eprintln!("bench: --current: {e}");
            exit::USAGE
        }
    }
}

/// Output as it is compared: line endings as Unix writes them, so a
/// `.expect` file checked out with CRLF on Windows still says the same.
fn lines(s: &str) -> String {
    s.replace("\r\n", "\n")
}

/// A warm-up run whose output must be `expect`, then samples until `runs`
/// or [`PER_SIDE`] (at least three, when there is time for them).
fn sample(runs: usize, expect: &str, mut make: impl FnMut(usize) -> Command) -> Side {
    let expect = &lines(expect);
    match measure(&mut make(0)) {
        Err(e) => return Side::Wrong(e),
        Ok(m) if lines(&m.stdout) != *expect => {
            return Side::Wrong(format!(
                "printed {:?}, not {:?}",
                clip(&m.stdout),
                clip(expect)
            ))
        }
        Ok(_) => {}
    }
    let started = Instant::now();
    let mut ms = Vec::new();
    let mut rss: Option<u64> = None;
    for i in 1..=runs.max(1) {
        if ms.len() >= 3 && started.elapsed() > PER_SIDE {
            break;
        }
        match measure(&mut make(i)) {
            Ok(m) if lines(&m.stdout) == *expect => {
                ms.push(m.ms);
                rss = rss.max(m.rss);
            }
            Ok(m) => {
                return Side::Wrong(format!(
                    "run {i} printed {:?}, not {:?}",
                    clip(&m.stdout),
                    clip(expect)
                ))
            }
            Err(e) => return Side::Wrong(e),
        }
    }
    Side::Timed(Stats::of(&ms, rss))
}

fn clip(s: &str) -> String {
    let mut t: String = s.chars().take(60).collect();
    if s.chars().count() > 60 {
        t.push('…');
    }
    t
}

/// One run: its wall time from start to exit, its stdout, and its peak
/// resident set where the system says it.
struct Measured {
    ms: f64,
    stdout: String,
    rss: Option<u64>,
}

/// Run a command to its end; a command that fails is an error, never a time.
fn measure(c: &mut Command) -> Result<Measured, String> {
    c.stdout(Stdio::piped()).stderr(Stdio::piped());
    let started = Instant::now();
    let mut child = c.spawn().map_err(|e| format!("could not start: {e}"))?;
    let mut stdout = child.stdout.take().expect("piped");
    let mut stderr = child.stderr.take().expect("piped");
    let err = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = std::io::Read::read_to_string(&mut stderr, &mut s);
        s
    });
    let mut out = String::new();
    let _ = std::io::Read::read_to_string(&mut stdout, &mut out);
    let (ok, code, rss) = wait(&mut child);
    let ms = started.elapsed().as_secs_f64() * 1000.0;
    let err = err.join().unwrap_or_default();
    if !ok {
        return Err(format!(
            "exit {}: {}",
            code.map_or("by a signal".to_string(), |c| c.to_string()),
            err.trim()
        ));
    }
    Ok(Measured {
        ms,
        stdout: out,
        rss,
    })
}

/// Wait for a child, and read its peak resident set from `wait4`.
#[cfg(unix)]
fn wait(child: &mut std::process::Child) -> (bool, Option<i32>, Option<u64>) {
    let pid = child.id() as libc::pid_t;
    let mut status = 0;
    // SAFETY: a zeroed rusage is valid, and wait4 writes only into the two
    // out-pointers it is given; the child is ours and not yet reaped.
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    let got = unsafe { libc::wait4(pid, &mut status, 0, &mut usage) };
    if got != pid {
        return match child.wait() {
            Ok(s) => (s.success(), s.code(), None),
            Err(_) => (false, None, None),
        };
    }
    // ru_maxrss is in bytes on macOS and in kilobytes elsewhere.
    let unit = if cfg!(target_os = "macos") { 1 } else { 1024 };
    let rss = u64::try_from(usage.ru_maxrss).ok().map(|k| k * unit);
    let exited = libc::WIFEXITED(status);
    let code = exited.then(|| libc::WEXITSTATUS(status));
    (exited && code == Some(0), code, rss)
}

#[cfg(not(unix))]
fn wait(child: &mut std::process::Child) -> (bool, Option<i32>, Option<u64>) {
    match child.wait() {
        Ok(s) => (s.success(), s.code(), None),
        Err(_) => (false, None, None),
    }
}

/// The reference tools found here, and the Rust references built once.
#[derive(Default)]
struct RefTools {
    python: Option<PathBuf>,
    node: Option<PathBuf>,
    rustc: Option<PathBuf>,
    dir: PathBuf,
}

impl RefTools {
    fn find(root: &Path) -> RefTools {
        let on_path = |names: &[&str]| {
            names.iter().find_map(|n| {
                std::env::var_os("PATH").and_then(|p| {
                    std::env::split_paths(&p)
                        .map(|d| d.join(format!("{n}{}", std::env::consts::EXE_SUFFIX)))
                        .find(|f| f.is_file())
                })
            })
        };
        let dir = root.join("ref");
        let _ = std::fs::create_dir_all(&dir);
        RefTools {
            python: on_path(&["python3", "python"]),
            node: on_path(&["node"]),
            rustc: on_path(&["rustc"]),
            dir,
        }
    }

    /// Each reference tool with its version, or `not installed`: a missing
    /// tool is said, and its column has no numbers.
    fn versions(&self) -> Vec<(&'static str, String)> {
        let ask = |tool: &Option<PathBuf>, arg: &str| -> String {
            let Some(t) = tool else {
                return "not installed".into();
            };
            Command::new(t)
                .arg(arg)
                .stdin(Stdio::null())
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| {
                    let text = String::from_utf8_lossy(if o.stdout.is_empty() {
                        &o.stderr
                    } else {
                        &o.stdout
                    })
                    .to_string();
                    text.lines().next().unwrap_or_default().trim().to_string()
                })
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "installed, version unknown".into())
        };
        vec![
            ("Python", ask(&self.python, "--version")),
            ("Node", ask(&self.node, "--version")),
            ("Rust", ask(&self.rustc, "--version")),
        ]
    }

    fn side(&self, name: &str, lang: &str, runs: usize, expect: &str, scale: usize) -> Side {
        let Some((_, _, source)) = REFS.iter().find(|(n, l, _)| *n == name && *l == lang) else {
            return Side::Absent("no reference".into());
        };
        let file = self.dir.join(format!("{name}.{lang}"));
        if std::fs::write(&file, source).is_err() {
            return Side::Absent("cannot write the reference".into());
        }
        let command: Box<dyn Fn() -> Command> = match lang {
            "py" => match &self.python {
                Some(py) => {
                    let (py, file) = (py.clone(), file.clone());
                    Box::new(move || {
                        let mut c = Command::new(&py);
                        c.arg(&file);
                        c
                    })
                }
                None => return Side::Absent("python3 is not installed".into()),
            },
            "js" => match &self.node {
                Some(node) => {
                    let (node, file) = (node.clone(), file.clone());
                    Box::new(move || {
                        let mut c = Command::new(&node);
                        c.arg(&file);
                        c
                    })
                }
                None => return Side::Absent("node is not installed".into()),
            },
            _ => {
                let Some(rustc) = &self.rustc else {
                    return Side::Absent("rustc is not installed".into());
                };
                let exe = self
                    .dir
                    .join(format!("{name}-rs{}", std::env::consts::EXE_SUFFIX));
                let ok = Command::new(rustc)
                    .args(["-O", "--edition", "2021", "-o"])
                    .arg(&exe)
                    .arg(&file)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .is_ok_and(|s| s.success());
                if !ok {
                    return Side::Absent("rustc could not build it".into());
                }
                Box::new(move || Command::new(&exe))
            }
        };
        sample(runs, expect, |_| {
            let mut c = command();
            c.stdin(Stdio::null());
            if scale > 1 {
                c.arg(scale.to_string());
            }
            c
        })
    }
}

/// The machine and the conditions, said with every report.
struct Conditions {
    cpu: String,
    cores: usize,
    kernel: String,
    governor: Option<String>,
    seal_key: String,
    runtime_sha256: String,
    /// The reference tools, each with its version or `not installed`.
    refs: Vec<(&'static str, String)>,
}

impl Conditions {
    fn here(runtime: &Runtime) -> Conditions {
        let read = |p: &str| {
            std::fs::read_to_string(p)
                .ok()
                .map(|s| s.trim().to_string())
        };
        let cpu = read("/proc/cpuinfo")
            .and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("model name") || l.starts_with("Model"))
                    .and_then(|l| l.split_once(':').map(|(_, v)| v.trim().to_string()))
            })
            .or_else(|| {
                Command::new("sysctl")
                    .args(["-n", "machdep.cpu.brand_string"])
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            })
            .unwrap_or_else(|| std::env::consts::ARCH.to_string());
        let kernel = read("/proc/sys/kernel/osrelease")
            .or_else(|| {
                Command::new("uname")
                    .arg("-r")
                    .output()
                    .ok()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            })
            .map(|r| format!("{} {r}", std::env::consts::OS))
            .unwrap_or_else(|| std::env::consts::OS.to_string());
        let runtime_sha256 = runtime
            .bytes()
            .map(|b| hex::encode(<sha2::Sha256 as sha2::Digest>::digest(b)))
            .unwrap_or_default();
        Conditions {
            cpu,
            cores: std::thread::available_parallelism().map_or(1, |n| n.get()),
            kernel,
            governor: read("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor"),
            seal_key: "file".into(),
            runtime_sha256,
            refs: Vec::new(),
        }
    }

    /// The reference tools and their versions, said with the report.
    fn refs_line(&self) -> Option<String> {
        (!self.refs.is_empty()).then(|| {
            let parts: Vec<String> = self.refs.iter().map(|(l, v)| format!("{l}: {v}")).collect();
            format!("references: {}", parts.join("; "))
        })
    }

    fn line(&self, runtime_version: &str) -> String {
        format!(
            "{}, {} cores, {}{}; cig {}, cigcompile {}, CIG_SEAL_KEY={}, runtime sha256 {}",
            self.cpu,
            self.cores,
            self.kernel,
            self.governor
                .as_ref()
                .map(|g| format!(", governor {g}"))
                .unwrap_or_default(),
            runtime_version,
            env!("CARGO_PKG_VERSION"),
            self.seal_key,
            &self.runtime_sha256[..self.runtime_sha256.len().min(16)]
        )
    }
}

type Check = (String, String, String, bool, bool);

fn budgets(rows: &[Row], noop_ms: Option<f64>, orchestrate_ms: Option<f64>) -> Vec<Check> {
    let median_of = |r: &Row, compiled: bool| {
        (if compiled {
            &r.compiled
        } else {
            &r.interpreted
        })
        .stats()
        .map(|s| s.median)
    };
    let hello = rows.iter().find(|r| r.name == "hello");
    let speedups: Vec<f64> = rows
        .iter()
        .filter_map(|r| Some(median_of(r, false)? / median_of(r, true)?.max(0.001)))
        .collect();
    let speedup = Stats::of(&speedups, None).median;
    let hello_ms = hello.and_then(|h| median_of(h, true));
    let mut checks: Vec<Check> = Vec::new();
    if let Some(ms) = noop_ms {
        checks.push((
            "incremental rebuild, nothing changed".into(),
            "under 50 ms".into(),
            format!("{ms:.1} ms"),
            true,
            ms < 50.0,
        ));
    }
    checks.extend([
        (
            "start-up of a compiled hello".into(),
            "under 5 ms".into(),
            hello_ms.map(|ms| format!("{ms:.1} ms")).unwrap_or_default(),
            false,
            hello_ms.is_some_and(|ms| ms < 5.0),
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
    ]);
    if let Some(ms) = orchestrate_ms {
        checks.push((
            "full build of orchestrate.cig".into(),
            "under 10 s".into(),
            format!("{:.2} s", ms / 1000.0),
            true,
            ms < 10_000.0,
        ));
    }
    checks
}

fn verdict(c: &Check) -> &'static str {
    match (c.3, c.4) {
        (_, true) => "met",
        (true, false) => "MISSED",
        (false, false) => "not yet (a later stage)",
    }
}

/// What a benchmark of the set stresses; empty for one from `--set`.
fn stresses(name: &str) -> &'static str {
    SET.iter()
        .find(|b| b.name == name)
        .map_or("", |b| b.stresses)
}

/// A side as a table cell: its median, or why it has none.
fn cell(s: &Side) -> String {
    match s {
        Side::Timed(st) if st.noisy() => format!("{:.1}~", st.median),
        Side::Timed(st) => format!("{:.1}", st.median),
        Side::Wrong(_) => "WRONG".into(),
        Side::Absent(_) => "-".into(),
    }
}

/// Compute time, apart from start-up: the slope between the 1x and 5x
/// medians, as `(ms5 - ms1) / 4`. Reuses [`Stats`] without a second
/// summary pass; empty when a benchmark did not declare a 5x.
fn compute_ms(one: &Side, five: Option<&Side>) -> Option<f64> {
    let (a, b) = (one.stats()?, five?.stats()?);
    Some(((b.median - a.median) / 4.0).max(0.0))
}

/// The compute-time slope as a table cell; a benchmark without a 5x shows a dash.
fn compute_cell(one: &Side, five: Option<&Side>) -> String {
    compute_ms(one, five)
        .map(|ms| format!("{ms:.1}"))
        .unwrap_or_else(|| "-".into())
}

/// cig's interpreted median over another side's.
fn ratio(r: &Row, other: &Side) -> String {
    match (r.interpreted.stats(), other.stats()) {
        (Some(a), Some(b)) if b.median > 0.0 => format!("{:.1}x", a.median / b.median),
        _ => "-".into(),
    }
}

/// Allocation count as a cell: the number, or a dash where there is none.
fn alloc_cell(a: Option<(u64, u64)>) -> String {
    a.map(|(n, _)| n.to_string()).unwrap_or_else(|| "-".into())
}

fn rss_cell(s: &Side) -> String {
    s.stats()
        .and_then(|st| st.rss)
        .map(|b| format!("{:.1}", b as f64 / 1e6))
        .unwrap_or_else(|| "-".into())
}

fn print_table(o: &Opts, rows: &[Row], checks: &[Check], c: &Conditions) {
    let version = cigscript::VERSION;
    println!(
        "stage A (bundle): up to {} samples a side after a warm-up, the median in ms; ~ is noisy (CV over 5%)",
        o.runs
    );
    println!("{}", c.line(version));
    if let Some(l) = c.refs_line() {
        println!("{l}");
    }
    println!();
    let mut head = format!(
        "{:<11} {:>9} {:>8} {:>12} {:>10} {:>10} {:>6} {:>8}",
        "program", "build ms", "size MB", "interpreted", "compiled", "compute ms", "CV", "RSS MB"
    );
    if o.alloc.is_some() {
        head.push_str(&format!(" {:>10}", "allocs"));
    }
    for (_, title) in LANGS.iter().filter(|_| o.refs) {
        head.push_str(&format!(" {title:>8}"));
    }
    if o.refs {
        head.push_str(&format!(" {:>8} {:>8}", "cig/py", "cig/node"));
    }
    println!("{head}");
    for r in rows {
        let cv = r
            .interpreted
            .stats()
            .map(|s| format!("{:.1}%", s.cv * 100.0))
            .unwrap_or_else(|| "-".into());
        let mut line = format!(
            "{:<11} {:>9.1} {:>8.1} {:>12} {:>10} {:>10} {:>6} {:>8}",
            r.name,
            r.build_ms,
            r.bytes as f64 / 1e6,
            cell(&r.interpreted),
            cell(&r.compiled),
            compute_cell(&r.interpreted, r.interpreted_5x.as_ref()),
            cv,
            rss_cell(&r.interpreted)
        );
        if o.alloc.is_some() {
            line.push_str(&format!(" {:>10}", alloc_cell(r.allocs)));
        }
        for (_, s) in &r.refs {
            line.push_str(&format!(" {:>8}", cell(s)));
        }
        if o.refs {
            let by = |l: &str| {
                r.refs
                    .iter()
                    .find(|(x, _)| *x == l)
                    .map(|(_, s)| ratio(r, s))
            };
            line.push_str(&format!(
                " {:>8} {:>8}",
                by("py").unwrap_or_default(),
                by("js").unwrap_or_default()
            ));
        }
        println!("{line}");
    }
    if o.refs {
        println!();
        println!("{}", medians_line(rows));
    }
    println!();
    println!("budgets (#14):");
    for ch in checks {
        println!("  {:<38} {:<38} {:>10}  {}", ch.0, ch.1, ch.2, verdict(ch));
    }
}

/// The set's median ratios to the references, without start-up's `hello`.
fn medians_line(rows: &[Row]) -> String {
    let mut parts = Vec::new();
    for (lang, title) in LANGS {
        let ratios: Vec<f64> = rows
            .iter()
            .filter(|r| r.name != "hello")
            .filter_map(|r| {
                let a = r.interpreted.stats()?.median;
                let b = r.refs.iter().find(|(l, _)| l == lang)?.1.stats()?.median;
                (b > 0.0).then_some(a / b)
            })
            .collect();
        if !ratios.is_empty() {
            parts.push(format!(
                "cig / {title}, median of {}: {:.1}x",
                ratios.len(),
                Stats::of(&ratios, None).median
            ));
        }
    }
    parts.join("; ")
}

fn print_markdown(o: &Opts, rows: &[Row], checks: &[Check], c: &Conditions) {
    println!("**cigcompile bench**: {}", c.line(cigscript::VERSION));
    if let Some(l) = c.refs_line() {
        println!("\n{l}");
    }
    println!();
    let mut head =
        "| benchmark | stresses | interpreted ms | compiled ms | compute ms | p10 to p90 | CV | RSS MB |"
            .to_string();
    let mut rule = "|---|---|---:|---:|---:|---|---:|---:|".to_string();
    if o.alloc.is_some() {
        head.push_str(" allocations |");
        rule.push_str("---:|");
    }
    for (_, title) in LANGS.iter().filter(|_| o.refs) {
        head.push_str(&format!(" {title} ms |"));
        rule.push_str("---:|");
    }
    if o.refs {
        head.push_str(" cig ÷ Python | cig ÷ Node |");
        rule.push_str("---:|---:|");
    }
    println!("{head}\n{rule}");
    for r in rows {
        let spread = r
            .interpreted
            .stats()
            .map(|s| format!("{:.1} to {:.1}", s.p10, s.p90))
            .unwrap_or_else(|| "-".into());
        let cv = r
            .interpreted
            .stats()
            .map(|s| format!("{:.1}%", s.cv * 100.0))
            .unwrap_or_else(|| "-".into());
        let mut line = format!(
            "| `{}` | {} | {} | {} | {} | {spread} | {cv} | {} |",
            r.name,
            stresses(&r.name),
            cell(&r.interpreted),
            cell(&r.compiled),
            compute_cell(&r.interpreted, r.interpreted_5x.as_ref()),
            rss_cell(&r.interpreted)
        );
        if o.alloc.is_some() {
            line.push_str(&format!(" {} |", alloc_cell(r.allocs)));
        }
        for (_, s) in &r.refs {
            line.push_str(&format!(" {} |", cell(s)));
        }
        if o.refs {
            let by = |l: &str| {
                r.refs
                    .iter()
                    .find(|(x, _)| *x == l)
                    .map(|(_, s)| ratio(r, s))
                    .unwrap_or_default()
            };
            line.push_str(&format!(" {} | {} |", by("py"), by("js")));
        }
        println!("{line}");
    }
    if o.refs {
        println!("\n{}", medians_line(rows));
    }
    println!("\n| budget (#14) | target | measured | verdict |\n|---|---|---:|---|");
    for ch in checks {
        println!("| {} | {} | {} | {} |", ch.0, ch.1, ch.2, verdict(ch));
    }
}

fn side_json(s: &Side) -> serde_json::Value {
    match s {
        Side::Timed(st) => serde_json::json!({
            "median_ms": st.median, "p10_ms": st.p10, "p90_ms": st.p90, "cv": st.cv,
            "samples": st.samples, "noisy": st.noisy(), "rss_bytes": st.rss,
            "samples_ms": st.raw,
        }),
        Side::Wrong(why) => serde_json::json!({"wrong": why}),
        Side::Absent(why) => serde_json::json!({"absent": why}),
    }
}

fn print_json(o: &Opts, rows: &[Row], checks: &[Check], c: &Conditions, runtime: &Runtime) {
    println!("{}", json_value(o, rows, checks, c, runtime));
}

fn json_value(
    o: &Opts,
    rows: &[Row],
    checks: &[Check],
    c: &Conditions,
    runtime: &Runtime,
) -> serde_json::Value {
    let rows: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            let refs: serde_json::Map<String, serde_json::Value> = r
                .refs
                .iter()
                .map(|(l, s)| (l.to_string(), side_json(s)))
                .collect();
            let refs_5x: serde_json::Map<String, serde_json::Value> = r
                .refs_5x
                .iter()
                .map(|(l, s)| (l.to_string(), side_json(s)))
                .collect();
            serde_json::json!({
                "name": r.name, "stresses": stresses(&r.name), "build_ms": r.build_ms, "bytes": r.bytes,
                "interpreted": side_json(&r.interpreted), "compiled": side_json(&r.compiled),
                "interpreted_5x": r.interpreted_5x.as_ref().map(side_json),
                "compiled_5x": r.compiled_5x.as_ref().map(side_json),
                "compute_ms": compute_ms(&r.interpreted, r.interpreted_5x.as_ref()),
                "allocs": r.allocs.map(|(n, b)| serde_json::json!({"count": n, "bytes": b})),
                "refs": refs, "refs_5x": refs_5x,
            })
        })
        .collect();
    let checks: Vec<serde_json::Value> = checks
        .iter()
        .map(|c| serde_json::json!({"what": c.0, "budget": c.1, "measured": c.2, "applies": c.3, "met": c.4}))
        .collect();
    {
        serde_json::json!({
            "stage": "A", "runs": o.runs, "cig": runtime.version, "cigcompile": env!("CARGO_PKG_VERSION"),
            "conditions": {"cpu": c.cpu, "cores": c.cores, "kernel": c.kernel, "governor": c.governor,
                           "seal_key": c.seal_key, "runtime_sha256": c.runtime_sha256},
            "references": c.refs.iter().map(|(l, v)| (l.to_string(), serde_json::Value::from(v.as_str()))).collect::<serde_json::Map<_, _>>(),
            "programs": rows, "budgets": checks,
        })
    }
}

/// The path of a file given to `--orchestrate`, as it is.
pub fn path(p: &Option<PathBuf>) -> Option<&Path> {
    p.as_deref()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statistics_are_medians_and_spreads_and_a_noisy_side_says_so() {
        let s = Stats::of(&[10.0, 10.0, 10.1, 9.9, 10.0], None);
        assert_eq!(s.median, 10.0);
        assert!(!s.noisy(), "{s:?}");
        let n = Stats::of(&[10.0, 14.0, 7.0, 12.0, 9.0], Some(5));
        assert!(n.noisy(), "a CV over 5% is noisy: {n:?}");
        assert_eq!((n.p10, n.median, n.p90), (7.0, 10.0, 14.0));
        assert_eq!(n.rss, Some(5));
        assert_eq!(Stats::of(&[], None), Stats::default());
    }

    #[test]
    fn every_benchmark_has_an_expect_file() {
        for b in SET {
            assert!(!b.expect.is_empty(), "{}: an empty .expect", b.name);
            assert!(!b.stresses.is_empty());
            if let Some(e5) = b.expect5x {
                assert!(!e5.is_empty(), "{}: an empty .expect5x", b.name);
                assert_ne!(
                    b.expect.trim(),
                    e5.trim(),
                    "{}: scaled benchmark whose 5x output matches its 1x",
                    b.name
                );
            }
        }
        for (name, lang, _) in REFS {
            assert!(
                SET.iter().any(|b| b.name == *name),
                "{name}.{lang}: not in the set"
            );
        }
    }

    /// The compute-time slope reuses [`Stats`] and reports non-negative ms
    /// derived from the 1x and 5x medians, with no slope when 5x is absent.
    #[test]
    fn compute_time_is_the_slope_between_the_one_and_five_times_medians() {
        let s1 = Side::Timed(Stats::of(&[10.0], None));
        let s5 = Side::Timed(Stats::of(&[42.0], None));
        assert_eq!(compute_ms(&s1, Some(&s5)), Some(8.0));
        // A 5x that came back faster clamps to zero rather than negative compute.
        let s5_fast = Side::Timed(Stats::of(&[6.0], None));
        assert_eq!(compute_ms(&s1, Some(&s5_fast)), Some(0.0));
        // No 5x means no slope.
        assert_eq!(compute_ms(&s1, None), None);
        // A 5x that is absent or wrong leaves the slope empty.
        assert_eq!(compute_ms(&s1, Some(&Side::Absent("x".into()))), None);
    }
}
