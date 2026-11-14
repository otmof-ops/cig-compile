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
    /// The compiled side's peak with the program's file on a memory file
    /// system (`peak_rss_in_memory`): taken for the benchmark the memory
    /// budget is held to, `None` for the rest and where there is no such
    /// figure.
    compiled_peak_in_memory: Option<u64>,
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
        // The memory budget is held to a compiled hello (`budgets`): its
        // peak is taken once more with the program's file on a memory file
        // system, so the page cache's folio size is not in the figure.
        let compiled_peak_in_memory = if name == "hello" && built.is_ok() {
            peak_rss_in_memory(&bin, |copy| {
                let mut c = Command::new(copy);
                env(&mut c);
                c.current_dir(fresh("pm", 0)).args(args);
                c
            })
        } else {
            None
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
            compiled_peak_in_memory,
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

    let checks = budgets(&rows, noop_ms, orchestrate_ms, cigscript::VERSION);
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
    // The peak from one more run of its own (see `peak_rss`); the timed
    // runs' figure only where that run has none.
    let peak = peak_rss(&mut make(runs.max(1) + 1)).or(rss);
    Side::Timed(Stats::of(&ms, peak))
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

/// A command's peak resident set, from one more run that is not timed.
///
/// Linux folds the parent's peak into a child's `ru_maxrss` when the child
/// execs (the old address space's high-water mark is kept), so the peak a
/// timed run reports is at least the bench's own: the bench holds its code,
/// the runtime it read when it loaded it (15.8 MB for cig 1.21.0) and its
/// results. On Linux this run forks (no shared address space) and, in the
/// child before `exec`, sets the child's mark back to what it has resident
/// (`5` to `/proc/self/clear_refs`): a forked child shares no file pages and
/// little else, so the peak is the program's own. Elsewhere it is `wait4`'s
/// figure for the run.
fn peak_rss(c: &mut Command) -> Option<u64> {
    c.stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: the closure runs in the forked child before `exec` and
        // makes only async-signal-safe calls (open, write, close) on a
        // constant path, touching no memory of the parent's.
        unsafe {
            c.pre_exec(|| {
                let fd = libc::open(c"/proc/self/clear_refs".as_ptr(), libc::O_WRONLY);
                if fd >= 0 {
                    libc::write(fd, b"5".as_ptr().cast(), 1);
                    libc::close(fd);
                }
                Ok(())
            });
        }
    }
    let mut child = c.spawn().ok()?;
    let (ok, _, rss) = wait(&mut child);
    if ok {
        rss
    } else {
        None
    }
}

/// A compiled program's peak resident set with its file on a memory file
/// system: a copy of `program` is made in [`memory_folder`], run once
/// (untimed, as `make` builds the command for the copy) and removed.
///
/// Linux maps a program's file in the page cache's folios. A file system
/// that keeps large folios for a file just written (ext4 on Linux 7.0)
/// makes a 16 MB program resident whole: the peak of every small program
/// there is its file's size plus what the run itself allocated, 18.9 MB
/// for a hello whose own share is under 3 MB, on any runtime. A memory file
/// system maps the pages a run touches and no more, so the figure is the
/// program's and not the file system's. `None` where there is no such
/// folder (another system, a container without one), where a file there
/// cannot be executed, or where the run fails.
fn peak_rss_in_memory(program: &Path, make: impl FnOnce(&Path) -> Command) -> Option<u64> {
    let dir = memory_folder()?.join(format!("cigcompile-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).ok()?;
    let rss = program.file_name().and_then(|name| {
        let copy = dir.join(name);
        std::fs::copy(program, &copy).ok()?;
        peak_rss(&mut make(&copy))
    });
    let _ = std::fs::remove_dir_all(&dir);
    rss
}

/// A folder on a memory file system that this user can write: `/dev/shm`,
/// else the session's runtime folder. Linux only: elsewhere `wait4`'s peak
/// has no such share to take out.
fn memory_folder() -> Option<PathBuf> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let mounts = std::fs::read_to_string("/proc/self/mountinfo").ok()?;
    [
        Some(PathBuf::from("/dev/shm")),
        std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from),
    ]
    .into_iter()
    .flatten()
    .find(|d| d.is_dir() && on_memory_fs(&mounts, d))
}

/// Is `dir` on `tmpfs` or `ramfs`, by the text of `/proc/self/mountinfo`?
/// The mount whose point is the longest prefix of `dir` decides. A line is
/// `id parent major:minor root point options ... - fstype source options`.
fn on_memory_fs(mountinfo: &str, dir: &Path) -> bool {
    mountinfo
        .lines()
        .filter_map(|l| {
            let (head, tail) = l.split_once(" - ")?;
            let point = Path::new(head.split(' ').nth(4)?);
            let fstype = tail.split(' ').next()?;
            dir.starts_with(point)
                .then(|| (point.components().count(), fstype))
        })
        .max_by_key(|(depth, _)| *depth)
        .is_some_and(|(_, fstype)| matches!(fstype, "tmpfs" | "ramfs"))
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

/// A compiled hello's peak resident set, real run: 4.4 MB on cig 1.20.0's
/// runtime once the file is cold, 5.9 MB on the runtime whose heap starts in
/// 4 KiB pages; 11.5 MB straight after `cigcompile build` on a kernel that
/// maps the new file in large page-cache folios (Linux 6.17, ext4). cig
/// 1.21.0 as released peaked at 17.5 MB there (its heap in 2 MiB pages).
/// On Linux 7.0 with ext4 the just-built file is resident whole: 18.9 MB
/// for a 16.2 MB program and 16.7 MB for a 14.0 MB one, on any runtime,
/// which is why the budget's figure is taken with the file on a memory
/// file system (`peak_rss_in_memory`): 7 to 8 MB there for cig 1.22.0.
const HELLO_PEAK_BUDGET: u64 = 14_000_000;

/// Does this cig's runtime start a short program's heap in small pages? The
/// first release after 1.21.0 does. 1.21.0 as released keeps its heap in
/// 2 MiB pages, and a hello's peak there is 14 to 18 MB, by the machine.
fn heap_starts_small(runtime: &str) -> bool {
    let mut parts = runtime
        .split(['.', '-', '+'])
        .map(|p| p.parse::<u64>().ok());
    let v = (
        parts.next().flatten(),
        parts.next().flatten(),
        parts.next().flatten(),
    );
    matches!(v, (Some(a), Some(b), Some(c)) if (a, b, c) > (1, 21, 0))
}

fn budgets(
    rows: &[Row],
    noop_ms: Option<f64>,
    orchestrate_ms: Option<f64>,
    runtime: &str,
) -> Vec<Check> {
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
    // Compiled peak memory has a budget of its own (cig-compile#30): held on
    // Linux, where the peak is the program's own (see `peak_rss`), and from
    // the first runtime that can meet it (`heap_starts_small`); a side with
    // no figure is not measured, never missed. The figure is the one taken
    // with the program's file on a memory file system where there is one
    // (`peak_rss_in_memory`), so a kernel that keeps a just-written file in
    // large folios does not put the whole file in it.
    let in_memory = hello.and_then(|h| h.compiled_peak_in_memory);
    let hello_peak =
        in_memory.or_else(|| hello.and_then(|h| h.compiled.stats()).and_then(|s| s.rss));
    if let Some(bytes) = hello_peak {
        let small = heap_starts_small(runtime);
        checks.push((
            "peak memory of a compiled hello".into(),
            if small {
                format!("under {} MB (Linux)", HELLO_PEAK_BUDGET / 1_000_000)
            } else {
                format!(
                    "under {} MB (Linux, from the cig after 1.21.0)",
                    HELLO_PEAK_BUDGET / 1_000_000
                )
            },
            format!(
                "{:.1} MB{}",
                bytes as f64 / 1e6,
                if in_memory.is_some() {
                    " (its file on a memory file system)"
                } else {
                    ""
                }
            ),
            cfg!(target_os = "linux") && small,
            bytes < HELLO_PEAK_BUDGET,
        ));
    }
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

// ----- the front end of `cig bench` (#68) ---------------------------------------

/// What `cigcompile bench` hands to `cig bench` when it is a front end of it:
/// the manifest in `bench/bench.toml`, the shared statistics, the A/B, the live
/// mode and the debugging flags. The old harness above stays behind the flags
/// it has always had (`--ref`, `--budgets`, `--orchestrate`, `--alloc`,
/// `--set`, the JSON gate); a run that asks for none of them and names one of
/// the new ones comes here.
pub struct Shared<'a> {
    /// The manifest (default `bench/bench.toml`).
    pub manifest: Option<&'a Path>,
    /// The cig that has `bench`: the one given, else the runtime programs are
    /// appended to, so the interpreted side of every case is the runtime the
    /// compiled side carries.
    pub cig: &'a Path,
    /// `--runs`, as `cig bench --samples`.
    pub samples: Option<usize>,
    pub only: &'a [String],
    pub group: &'a [String],
    pub markdown: bool,
    pub json: bool,
    pub watch: bool,
    /// A git ref, a release or a binary: `cig bench --against`.
    pub against: Option<&'a str>,
    pub timings: bool,
    pub stats: bool,
    pub syscalls: bool,
}

/// The arguments `cig` is run with for a front-end run.
pub fn shared_argv(s: &Shared) -> Vec<String> {
    let mut a: Vec<String> = Vec::new();
    // `--json` is cig's global flag, so it goes before the subcommand.
    if s.json || s.syscalls {
        a.push("--json".into());
    }
    a.push("bench".into());
    a.push(
        s.manifest
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "bench/bench.toml".into()),
    );
    if let Some(n) = s.samples {
        a.push("--samples".into());
        a.push(n.to_string());
    }
    for o in s.only {
        a.push("--only".into());
        a.push(o.clone());
    }
    for g in s.group {
        a.push("--group".into());
        a.push(g.clone());
    }
    if s.markdown && !s.syscalls {
        a.push("--markdown".into());
    }
    if let Some(x) = s.against {
        a.push("--against".into());
        a.push(x.to_string());
    }
    for (on, flag) in [
        (s.watch, "--watch"),
        (s.timings, "--timings"),
        (s.stats, "--stats"),
        (s.syscalls, "--syscalls"),
    ] {
        if on {
            a.push(flag.into());
        }
    }
    a
}

/// Does this `cig` have a `bench` command? (A cig older than the release that
/// brought it says so; the front end says what to do instead of failing in a
/// way that reads like a bad manifest.)
pub fn cig_has_bench(cig: &Path) -> bool {
    Command::new(cig)
        .args(["bench", "--help"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// A folder to put first on the `PATH` of every case: it holds the runtime as
/// `cig`, so a case that says `cig run` runs the exact bytes a program built
/// here carries, and (through the `PATH` the harness keeps) `cigcompile`.
fn shim_dir(cig: &Path) -> std::io::Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("cigcompile-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(dir.join("cig"));
        std::os::unix::fs::symlink(std::fs::canonicalize(cig)?, dir.join("cig"))?;
    }
    #[cfg(not(unix))]
    {
        std::fs::copy(cig, dir.join("cig.exe"))?;
    }
    Ok(dir)
}

/// Run `cig bench` for a front-end run, and give its exit code.
pub fn shared(ctx: &Ctx, s: &Shared) -> i32 {
    if !cig_has_bench(s.cig) {
        let d = cigscript::diagnostics::usage(format!(
            "{} has no `bench` command, which `cigcompile bench` needs for {}",
            s.cig.display(),
            "--watch, --against <ref>, --timings, --stats, --syscalls and --manifest"
        ))
        .code("E1001")
        .with_hint(
            "give a cig that has `cig bench` with --cig, or use the flags the set harness has: --runs, --ref, --markdown, --json, --set, --runtime, --budgets, --orchestrate",
        );
        cigscript::cli::report(ctx, &d, None, None);
        return exit::USAGE;
    }
    let shim = match shim_dir(s.cig) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("cigcompile bench: cannot make a folder for the runtime: {e}");
            return exit::USAGE;
        }
    };
    let mut path_dirs = vec![shim.clone()];
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
    {
        path_dirs.push(dir);
    }
    path_dirs.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let path = std::env::join_paths(path_dirs).unwrap_or_default();
    let mut cmd = Command::new(s.cig);
    cmd.args(shared_argv(s)).env("PATH", path);
    let code = if s.syscalls {
        match cmd.stdin(Stdio::null()).stderr(Stdio::inherit()).output() {
            Ok(out) => {
                let text = String::from_utf8_lossy(&out.stdout).into_owned();
                if s.json {
                    print!("{text}");
                } else {
                    print_syscalls(&text);
                }
                out.status.code().unwrap_or(1)
            }
            Err(e) => {
                eprintln!("cigcompile bench: cannot run {}: {e}", s.cig.display());
                exit::USAGE
            }
        }
    } else {
        match cmd.status() {
            Ok(st) => st.code().unwrap_or(1),
            Err(e) => {
                eprintln!("cigcompile bench: cannot run {}: {e}", s.cig.display());
                exit::USAGE
            }
        }
    };
    let _ = std::fs::remove_dir_all(&shim);
    code
}

/// mmap and munmap calls from one case's `debug.syscalls` (the top ten by
/// time that `cig bench --syscalls --json` carries).
fn map_calls(case: &serde_json::Value) -> Option<(u64, u64)> {
    let top = case["debug"]["syscalls"]["top"].as_array()?;
    let n = |names: &[&str]| -> u64 {
        top.iter()
            .filter(|c| c["syscall"].as_str().is_some_and(|x| names.contains(&x)))
            .filter_map(|c| c["calls"].as_u64())
            .sum()
    };
    Some((n(&["mmap", "mmap2"]), n(&["munmap"])))
}

/// One compiled-against-interpreted pair whose gap the allocator explains.
#[derive(Debug, PartialEq)]
pub struct Gap {
    pub name: String,
    pub interpreted_ms: f64,
    pub compiled_ms: f64,
    pub interpreted_calls: u64,
    pub compiled_calls: u64,
}

impl Gap {
    pub fn line(&self) -> String {
        format!(
            "{}: interpreted {:.0} ms ({} mmap/munmap calls), compiled {:.0} ms ({}): allocator, not the compiler",
            self.name,
            self.interpreted_ms,
            self.interpreted_calls,
            self.compiled_ms,
            self.compiled_calls
        )
    }
}

/// The pairs in a `cig bench --syscalls --json` report (cases named
/// `interpreted: X` and `compiled: X`, both on the same runtime) where one side
/// maps and unmaps far more than the other: at least 100 mmap/munmap pairs
/// more, and ten times as many. Both sides run the one runtime, so the program
/// the compiler made is not what is faster or slower: the allocator's
/// behaviour is (cig-script#153). A pair that does not differ that way is left
/// alone, whatever its times say.
pub fn allocator_gaps(doc: &serde_json::Value) -> Vec<Gap> {
    let cases = doc["cases"].as_array().cloned().unwrap_or_default();
    let find = |name: &str| cases.iter().find(|c| c["name"].as_str() == Some(name));
    let median = |c: &serde_json::Value| c["outcome"]["median_ms"].as_f64();
    let mut out = Vec::new();
    for c in &cases {
        let Some(base) = c["name"]
            .as_str()
            .and_then(|n| n.strip_prefix("interpreted: "))
        else {
            continue;
        };
        let Some(d) = find(&format!("compiled: {base}")) else {
            continue;
        };
        let (Some((im, iu)), Some((cm, cu))) = (map_calls(c), map_calls(d)) else {
            continue;
        };
        let (i_pairs, c_pairs) = (im.min(iu), cm.min(cu));
        let (hi, lo) = (i_pairs.max(c_pairs), i_pairs.min(c_pairs));
        if hi >= lo + 100 && hi >= lo.saturating_mul(10) {
            if let (Some(a), Some(b)) = (median(c), median(d)) {
                out.push(Gap {
                    name: base.to_string(),
                    interpreted_ms: a,
                    compiled_ms: b,
                    interpreted_calls: im + iu,
                    compiled_calls: cm + cu,
                });
            }
        }
    }
    out
}

/// `--syscalls` in words: each case with its calls, and the allocator's gaps.
fn print_syscalls(json: &str) {
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(json) else {
        print!("{json}");
        return;
    };
    println!("syscalls: one extra run of each case under strace");
    for c in doc["cases"].as_array().into_iter().flatten() {
        let name = c["name"].as_str().unwrap_or("?");
        let sc = &c["debug"]["syscalls"];
        match sc["measured"].as_bool() {
            Some(true) => {
                let (m, u) = map_calls(c).unwrap_or((0, 0));
                let flag = if sc["thrash"].is_string() {
                    "  FLAG: mmap/munmap thrash"
                } else {
                    ""
                };
                println!(
                    "  {name}: {} calls, mmap {m}, munmap {u}{flag}",
                    sc["total"].as_u64().unwrap_or(0)
                );
            }
            _ => println!(
                "  {name}: {}",
                sc["note"].as_str().unwrap_or("not measured")
            ),
        }
    }
    for g in allocator_gaps(&doc) {
        println!("{}", g.line());
    }
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

    /// A child's peak memory is its own: the bench's earlier peak (here a
    /// 96 MiB buffer, touched and freed) does not show in the child's row.
    /// The probe runs in a process of its own: a forked child starts with
    /// what its parent has resident, and beside this test the harness runs
    /// others that hold a runtime's bytes each.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_benchmarks_peak_memory_is_its_own_not_the_benchs() {
        let exe = std::env::current_exe().expect("the test binary");
        let out = Command::new(exe)
            .args([
                "--exact",
                "bench::tests::the_peak_probe_in_a_process_of_its_own",
                "--ignored",
                "--test-threads=1",
            ])
            .stdin(Stdio::null())
            .output()
            .expect("run the probe");
        let said = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success() && said.contains("1 passed"),
            "the probe failed or did not run:\n{said}\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// The probe `a_benchmarks_peak_memory_is_its_own_not_the_benchs` starts.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "started by a_benchmarks_peak_memory_is_its_own_not_the_benchs, alone in its process"]
    fn the_peak_probe_in_a_process_of_its_own() {
        const BIG: usize = 96 << 20;
        let mut big = vec![0u8; BIG];
        for i in (0..BIG).step_by(4096) {
            big[i] = 1;
        }
        std::hint::black_box(&big);
        drop(big);
        let rss = peak_rss(Command::new("true").stdin(Stdio::null())).expect("a peak on Linux");
        assert!(
            rss < (BIG as u64) / 2,
            "the child's peak is {rss} bytes, which carries the bench's own {BIG}"
        );
    }

    /// A folder is on a memory file system when the mount that holds it is
    /// `tmpfs` or `ramfs`: the longest mount point that is a prefix of it
    /// decides, so a disk mounted under a memory file system is a disk.
    #[test]
    fn a_folder_is_on_a_memory_file_system_by_its_own_mount() {
        let mounts = "\
22 1 259:2 / / rw,noatime shared:1 - ext4 /dev/nvme0n1p2 rw
31 22 0:27 / /dev/shm rw,nosuid,nodev shared:4 - tmpfs tmpfs rw
40 22 0:35 / /run rw,nosuid,nodev shared:5 - tmpfs tmpfs rw,mode=755
55 40 0:60 / /run/user/1000 rw,nosuid,nodev,relatime shared:9 - tmpfs tmpfs rw,size=1G
60 31 8:1 / /dev/shm/disk rw - ext4 /dev/sda1 rw
";
        assert!(on_memory_fs(mounts, Path::new("/dev/shm")));
        assert!(on_memory_fs(
            mounts,
            Path::new("/dev/shm/cigcompile-bench-1")
        ));
        assert!(on_memory_fs(mounts, Path::new("/run/user/1000")));
        assert!(!on_memory_fs(mounts, Path::new("/tmp")));
        assert!(!on_memory_fs(mounts, Path::new("/dev/shm/disk/x")));
        assert!(!on_memory_fs("", Path::new("/dev/shm")));
        assert!(!on_memory_fs("garbage\n1 2 3\n", Path::new("/dev/shm")));
    }

    /// Compiled peak memory has an explicit budget (cig-compile#30): a hello
    /// over it is missed where it is held, one under it is met, and a hello
    /// with no figure has no row. It is held from the first cig after 1.21.0:
    /// on 1.21.0 as released the row is there and reads "not yet".
    #[test]
    fn a_compiled_hello_over_its_memory_budget_is_missed() {
        let row = |rss: Option<u64>| Row {
            name: "hello".into(),
            build_ms: 1.0,
            bytes: 1,
            interpreted: Side::Timed(Stats::of(&[5.0], rss)),
            compiled: Side::Timed(Stats::of(&[4.0], rss)),
            interpreted_5x: None,
            compiled_5x: None,
            refs: Vec::new(),
            refs_5x: Vec::new(),
            allocs: None,
            compiled_peak_in_memory: None,
        };
        let peak_on = |rss, runtime: &str| {
            budgets(&[row(rss)], None, None, runtime)
                .into_iter()
                .find(|c| c.0 == "peak memory of a compiled hello")
        };
        // The figure taken with the program's file on a memory file system
        // is the one the budget is held to: a hello whose resident set
        // counts its whole 16 MB file is met when its own peak is under.
        let whole_file = Row {
            compiled_peak_in_memory: Some(7_800_000),
            ..row(Some(18_900_000))
        };
        let own = budgets(&[whole_file], None, None, "1.22.0")
            .into_iter()
            .find(|c| c.0 == "peak memory of a compiled hello")
            .expect("a row");
        assert!(own.4 && own.2.contains("7.8 MB"), "{own:?}");
        assert!(own.2.contains("memory file system"), "{own:?}");
        let fat = Row {
            compiled_peak_in_memory: Some(15_500_000),
            ..row(Some(18_900_000))
        };
        let fat = budgets(&[fat], None, None, "1.22.0")
            .into_iter()
            .find(|c| c.0 == "peak memory of a compiled hello")
            .expect("a row");
        assert!(!fat.4, "{fat:?}");
        let peak = |rss| peak_on(rss, "1.22.0");
        let over = peak(Some(17_500_000)).expect("a row");
        assert_eq!(
            (over.3, over.4),
            (cfg!(target_os = "linux"), false),
            "{over:?}"
        );
        let under = peak(Some(5_000_000)).expect("a row");
        assert!(under.4, "{under:?}");
        assert!(peak(None).is_none());
        // cig 1.21.0 as released cannot meet it: the row is not held there
        let released = peak_on(Some(17_500_000), "1.21.0").expect("a row");
        assert_eq!((released.3, released.4), (false, false), "{released:?}");
        assert_eq!(verdict(&released), "not yet (a later stage)");
        assert!(released.1.contains("after 1.21.0"), "{released:?}");
        for (v, small) in [
            ("1.21.0", false),
            ("1.20.9", false),
            ("1.21.1", true),
            ("1.22.0", true),
            ("2.0.0", true),
            ("1.22.0-rc.1", true),
            ("nonsense", false),
        ] {
            assert_eq!(heap_starts_small(v), small, "{v}");
        }
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

    fn case(name: &str, ms: f64, mmap: u64, munmap: u64) -> serde_json::Value {
        serde_json::json!({
            "name": name,
            "outcome": { "state": "timed", "median_ms": ms },
            "debug": { "syscalls": {
                "measured": true,
                "total": mmap + munmap + 40,
                "top": [
                    { "syscall": "mmap", "calls": mmap, "ms": 1.0 },
                    { "syscall": "munmap", "calls": munmap, "ms": 1.0 },
                    { "syscall": "write", "calls": 40, "ms": 0.1 },
                ],
                "thrash": null,
            } },
        })
    }

    /// #68: a planted mmap thrash on one side of a pair, both on the one
    /// runtime, is the allocator's, not the compiler's.
    #[test]
    fn allocator_gap_is_named() {
        // The numbers of cig-script#153: nbody, 400,507 mmap/munmap calls
        // interpreted against 78 compiled, 1433 ms against 778 ms.
        let doc = serde_json::json!({ "cases": [
            case("interpreted: nbody", 1433.0, 200_254, 200_253),
            case("compiled: nbody", 778.0, 39, 39),
            // Both quiet: no gap to name, however far apart the times are.
            case("interpreted: fib", 900.0, 30, 30),
            case("compiled: fib", 500.0, 30, 30),
            // Both thrash alike: the runtime is the same on both sides.
            case("interpreted: trees", 900.0, 5_000, 5_000),
            case("compiled: trees", 880.0, 5_000, 5_000),
            // A compiled side with no interpreted partner is left alone.
            case("compiled: lonely", 10.0, 1, 1),
        ]});
        let gaps = allocator_gaps(&doc);
        assert_eq!(gaps.len(), 1, "{gaps:?}");
        assert_eq!(gaps[0].name, "nbody");
        assert_eq!(gaps[0].interpreted_calls, 400_507);
        assert_eq!(gaps[0].compiled_calls, 78);
        let line = gaps[0].line();
        assert!(line.contains("allocator, not the compiler"), "{line}");
        assert!(
            line.contains("400507") || line.contains("400,507") || line.contains("(400507"),
            "{line}"
        );

        // The planted thrash on the compiled side is named the same way.
        let doc = serde_json::json!({ "cases": [
            case("interpreted: x", 100.0, 20, 20),
            case("compiled: x", 400.0, 50_000, 50_000),
        ]});
        assert_eq!(allocator_gaps(&doc).len(), 1);
        // A strace that was not available says nothing: never a made-up flag.
        let doc = serde_json::json!({ "cases": [
            { "name": "interpreted: y", "outcome": { "median_ms": 1.0 },
              "debug": { "syscalls": { "measured": false, "note": "not measured: no strace" } } },
            { "name": "compiled: y", "outcome": { "median_ms": 1.0 },
              "debug": { "syscalls": { "measured": false, "note": "not measured: no strace" } } },
        ]});
        assert!(allocator_gaps(&doc).is_empty());
    }

    #[test]
    fn the_front_end_hands_cig_the_flags_it_was_given() {
        let only = vec!["fib".to_string()];
        let group = vec!["programs".to_string()];
        let s = Shared {
            manifest: None,
            cig: Path::new("cig"),
            samples: Some(3),
            only: &only,
            group: &group,
            markdown: true,
            json: false,
            watch: false,
            against: Some("origin/main"),
            timings: true,
            stats: false,
            syscalls: false,
        };
        assert_eq!(
            shared_argv(&s),
            [
                "bench",
                "bench/bench.toml",
                "--samples",
                "3",
                "--only",
                "fib",
                "--group",
                "programs",
                "--markdown",
                "--against",
                "origin/main",
                "--timings"
            ]
        );
        // `--json` is cig's global flag and goes before `bench`; `--syscalls`
        // needs the JSON to name the allocator's gaps.
        let s = Shared {
            json: true,
            markdown: false,
            against: None,
            timings: false,
            syscalls: true,
            ..s
        };
        assert_eq!(shared_argv(&s)[..2], ["--json", "bench"]);
        assert!(shared_argv(&s).contains(&"--syscalls".to_string()));
    }
}
