// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! cigcompile: CigScript scripts as programs that run with nothing
//! installed. Stage A (#2): a program is the `cig` of the version this
//! compiler pins, with the checked script appended; it runs through the
//! same run as `cig run` (CigScript's docs/PROGRAMS.md).

mod audit;
mod bench;
mod build;
mod cache;
mod digests;
mod fat;
mod fetch;
mod project;
mod reduce;
mod runtime;
mod sbom;
mod test;
mod timings;
mod verify;

use build::exit;
use cigscript::cli::Ctx;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "cigcompile",
    about = "CigScript scripts as programs that run with nothing installed",
    disable_version_flag = true
)]
struct Cli {
    /// Machine-readable output on stdout.
    #[arg(long, global = true)]
    json: bool,
    /// The same information without the catchphrases (also CIG_PLAIN).
    #[arg(long, global = true)]
    plain: bool,
    /// Never colour (also NO_COLOR).
    #[arg(long, global = true)]
    no_color: bool,
    /// Print the version and the cig it makes programs for.
    #[arg(long, short = 'V')]
    version: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Check a script, or every script at the top of a folder, and write each as a program.
    Build(BuildArgs),
    /// Build, then run each program and its script with --dry-run and compare them.
    Test(BuildArgs),
    /// Remove the programs a build of this path wrote.
    Clean(CleanArgs),
    /// The machines cig is released for, and the runtime for this one.
    Targets,
    /// The benchmark set, interpreted and compiled, and the budgets the stage that exists can meet.
    Bench(BenchArgs),
    /// Fetch the runtime (cig's own release asset) for other machines, checked against the sha256 this compiler pins.
    Fetch(FetchArgs),
    /// The hardening a program carries, read from its headers (cig's build's): PIE, RELRO, a closed stack, CET or BTI/PAC, a signature. Report only.
    Audit(AuditArgs),
    /// Check a program is what a build makes: its runtime against cig's release (or --runtime), its payload against its sha256 and the script (--script), the whole file byte for byte. Nothing runs.
    Verify(VerifyArgs),
    /// Cut a script down to the smallest one that is still interesting, for a bug report: by default, that its program differs from it (as `cigcompile test` finds); with a command after `--`, that the command exits 0 on it (`{}` is the candidate's path).
    Reduce(ReduceArgs),
}

#[derive(Args)]
struct AuditArgs {
    /// The program (or any executable).
    program: PathBuf,
}

#[derive(Args)]
struct VerifyArgs {
    /// The program.
    program: PathBuf,
    /// The script it was built from, to check the program carries it.
    #[arg(long)]
    script: Option<PathBuf>,
    /// The cig it was built on, when that is not cig's own release (a cig built from source).
    #[arg(long)]
    runtime: Option<PathBuf>,
}

#[derive(Args)]
struct ReduceArgs {
    /// The script to reduce.
    script: PathBuf,
    /// Where the reduced script goes (default <name>.reduced.cig beside it).
    #[arg(short, long)]
    out: Option<PathBuf>,
    /// The cig to build each candidate onto, when comparing (as build's --runtime).
    #[arg(long)]
    runtime: Option<PathBuf>,
    /// The test: a command and its arguments, with {} for the candidate's path (else it goes last); interesting when it exits 0.
    #[arg(last = true, value_name = "COMMAND")]
    predicate: Vec<String>,
}

#[derive(Args)]
struct FetchArgs {
    /// The machines to fetch for (`cigcompile targets` lists them).
    #[arg(long)]
    target: Vec<String>,
    /// Every machine cig is released for.
    #[arg(long)]
    all: bool,
}

#[derive(Args)]
struct BenchArgs {
    /// The most samples per program and side, after a warm-up run (a side
    /// stops sooner once it has three and has run three seconds).
    #[arg(long, default_value_t = 10)]
    runs: usize,
    /// Run the Python, Node and Rust references too, where they are installed.
    #[arg(long = "ref")]
    refs: bool,
    /// A Markdown table, for a pull request comment.
    #[arg(long)]
    markdown: bool,
    /// Benchmarks from this folder (`<name>.cig` beside `<name>.expect`)
    /// instead of the set.
    #[arg(long, value_name = "DIR")]
    set: Option<PathBuf>,
    /// The cig to build and run with (default: as build finds it).
    #[arg(long)]
    runtime: Option<PathBuf>,
    /// Exit 1 when a budget that applies to this stage is missed.
    #[arg(long)]
    budgets: bool,
    /// Also time a full build of this script (#14: orchestrate.cig under 10 s).
    #[arg(long, value_name = "SCRIPT")]
    orchestrate: Option<PathBuf>,
}

#[derive(Args)]
struct BuildArgs {
    /// A .cig script, or a folder of them (default: this folder).
    path: Option<PathBuf>,
    /// Where the program goes; for a folder, the folder the programs go to (default <folder>/bin).
    #[arg(short, long)]
    out: Option<PathBuf>,
    /// The machine the program is for (default: this one; `cigcompile targets` lists them).
    #[arg(long)]
    target: Option<String>,
    /// The cig to append the program to (default: the one of this compiler's version, found here).
    #[arg(long)]
    runtime: Option<PathBuf>,
    /// Check only: write nothing, need no runtime.
    #[arg(long)]
    check: bool,
    /// Print what a stage of the compiler makes instead of a program: `cir`, the verified IR (#3).
    #[arg(long, value_name = "WHAT", value_parser = ["cir"])]
    emit: Option<String>,
    /// The optimisation level: 0 (lowering only), 1 or 2 (every pass there is). Stage A programs carry the script, so the level shapes `--emit` until stage B.
    #[arg(short = 'O', value_name = "LEVEL", value_parser = ["0", "1", "2"], conflicts_with = "passes")]
    opt: Option<String>,
    /// Run exactly these passes, in this order, for bisecting: a comma list of fold-consts, dead-pure.
    #[arg(long, value_name = "A,B", value_delimiter = ',')]
    passes: Option<Vec<String>>,
    /// With --emit: after the IR, what each pass changed, as a unified diff of the printed IR.
    #[arg(long, requires = "emit")]
    passes_diff: bool,
    /// Where the build's time went, phase by phase, on stderr; =FILE also writes a Chrome trace (chrome://tracing, Perfetto).
    #[arg(long, value_name = "FILE", num_args = 0..=1, require_equals = true)]
    timings: Option<Option<PathBuf>>,
    /// What the build is doing as it goes, one JSON object a line on stderr (for a window or an editor).
    #[arg(long)]
    json_progress: bool,
    /// A CycloneDX 1.5 SBOM beside each program, <program>.cdx.json: its runtime and its script, by sha256.
    #[arg(long)]
    sbom: bool,
}

#[derive(Args)]
struct CleanArgs {
    /// The script or folder that was built (default: this folder).
    path: Option<PathBuf>,
    /// The -o the build was given.
    #[arg(short, long)]
    out: Option<PathBuf>,
    /// The target the build was for.
    #[arg(long)]
    target: Option<String>,
    /// Remove a program compiled from a script of this name even when this
    /// cigcompile did not write it there (a build index that was cleared).
    #[arg(long)]
    force: bool,
}

fn main() {
    // The same stack cig gives its parser and checker: their depth limits
    // (E208) are sized for it, and a deeply nested script must be refused,
    // not overflow the main thread's stack.
    let code = std::thread::Builder::new()
        .name("cigcompile".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(run)
        .expect("could not start the compiler thread")
        .join()
        .unwrap_or(70);
    std::process::exit(code);
}

fn run() -> i32 {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            let code = if e.use_stderr() {
                exit::USAGE
            } else {
                exit::OK
            };
            let _ = e.print();
            return code;
        }
    };
    use std::io::IsTerminal;
    let ctx = Ctx {
        json: cli.json,
        color: !cli.no_color
            && std::env::var_os("NO_COLOR").is_none()
            && std::io::stderr().is_terminal(),
        doctor: false,
        plain: cli.plain || cigscript::cli::plain_by_env(),
    };
    if cli.version {
        println!(
            "cigcompile {} (makes programs for cig {})",
            env!("CARGO_PKG_VERSION"),
            runtime::wanted()
        );
        return exit::OK;
    }
    match cli.command {
        None => {
            use clap::CommandFactory;
            let _ = Cli::command().print_help();
            exit::OK
        }
        Some(Command::Build(a)) => build_cmd(&ctx, &a).map(|_| exit::OK).unwrap_or_else(|c| c),
        Some(Command::Test(a)) => match build_cmd(&ctx, &a) {
            Ok(Some((built, rt))) => test::compare(&ctx, &built, &rt),
            Ok(None) => exit::OK,
            Err(c) => c,
        },
        Some(Command::Clean(a)) => clean(&ctx, &a),
        Some(Command::Targets) => targets(&ctx),
        Some(Command::Fetch(a)) => fetch_cmd(&ctx, &a),
        Some(Command::Audit(a)) => audit::audit(&ctx, &a.program),
        Some(Command::Verify(a)) => verify::verify(
            &ctx,
            &verify::Opts {
                program: &a.program,
                script: a.script.as_deref(),
                runtime: a.runtime.as_deref(),
            },
        ),
        Some(Command::Reduce(a)) => reduce::reduce(
            &ctx,
            &reduce::Opts {
                script: &a.script,
                out: a.out.as_deref(),
                runtime: a.runtime.as_deref(),
                predicate: &a.predicate,
            },
        ),
        Some(Command::Bench(a)) => {
            let mut index = cache::Index::load();
            let found = runtime::find(runtime::host(), a.runtime.as_deref(), &mut index);
            index.save();
            match found {
                Ok(rt) => bench::bench(
                    &ctx,
                    &rt,
                    &bench::Opts {
                        runs: a.runs.max(1),
                        budgets: a.budgets,
                        orchestrate: bench::path(&a.orchestrate),
                        refs: a.refs,
                        markdown: a.markdown,
                        set: a.set.as_deref(),
                    },
                ),
                Err(d) => {
                    cigscript::cli::report(&ctx, &d, None, None);
                    exit::USAGE
                }
            }
        }
    }
}

/// A build; `None` for a check-only one.
fn build_cmd(
    ctx: &Ctx,
    a: &BuildArgs,
) -> Result<Option<(Vec<build::Built>, runtime::Runtime)>, i32> {
    let fail = |d: cigscript::diagnostics::Diagnostic| {
        cigscript::cli::report(ctx, &d, None, None);
        exit::USAGE
    };
    let target = runtime::target(a.target.as_deref()).map_err(fail)?;
    let jobs = project::jobs(a.path.as_deref(), a.out.as_deref(), target).map_err(fail)?;
    let passes = passes_of(a).map_err(fail)?;
    if a.emit.is_some() {
        emit_cir(ctx, &jobs, &passes, a.passes_diff)?;
        return Ok(None);
    }
    timings::enable(a.timings.is_some(), a.json_progress);
    let started = std::time::Instant::now();
    let finish = |built: &Result<_, i32>| {
        let total = started.elapsed().as_micros();
        if a.timings.is_some() && built.is_ok() {
            timings::report(total);
            if let Some(Some(file)) = &a.timings {
                let trace = serde_json::to_string(&timings::trace(total)).unwrap_or_default();
                if let Err(e) = std::fs::write(file, trace) {
                    eprintln!("cannot write {}: {e}", file.display());
                }
            }
        }
    };
    let mut index = cache::Index::load();
    if a.check {
        let checked = build::build(ctx, &jobs, target, None, &mut index, false).map(|_| ());
        finish(&checked);
        checked?;
        return Ok(None);
    }
    let found = runtime::find(target, a.runtime.as_deref(), &mut index);
    index.save();
    let rt = found.map_err(fail)?;
    let built = build::build(ctx, &jobs, target, Some(&rt), &mut index, a.sbom);
    index.save();
    finish(&built.as_ref().map(|_| ()).map_err(|c| *c));
    let built = built?;
    warn_linkage(ctx, target, &rt);
    Ok(Some((built, rt)))
}

/// A runtime given by hand that links this machine's glibc makes programs
/// that start only where that glibc is: built, and said so (#27).
fn warn_linkage(ctx: &Ctx, target: &runtime::Target, rt: &runtime::Runtime) {
    let Some(needs) = rt.needs.as_ref().filter(|_| runtime::wants_static(target)) else {
        return;
    };
    if !ctx.json {
        eprint!(
            "{}",
            ctx.yellow(&format!(
                "warning: {} is dynamically linked, so its programs need {needs} to start and will not run on most Linux machines; cig's release for {} needs nothing (build without --runtime, or cigcompile fetch first)\n",
                rt.path.display(),
                target.triple
            ))
        );
    }
}

/// The passes `-O` or `--passes` asks for; `-O1` when neither is given.
fn passes_of(
    a: &BuildArgs,
) -> Result<Vec<&'static dyn cigcompile::cir::pass::Pass>, cigscript::diagnostics::Diagnostic> {
    use cigcompile::cir::pass;
    if let Some(names) = &a.passes {
        let mut out = Vec::new();
        for n in names.iter().map(|n| n.trim()).filter(|n| !n.is_empty()) {
            match pass::by_name(n) {
                Some(p) => out.push(p),
                None => {
                    return Err(
                        cigscript::diagnostics::usage(format!("there is no pass `{n}`"))
                            .with_hint(format!("the passes are {}", pass::NAMES.join(", "))),
                    )
                }
            }
        }
        return Ok(out);
    }
    let level = a.opt.as_deref().unwrap_or("1").parse::<u8>().unwrap_or(1);
    Ok(pass::level(level))
}

/// `--emit=cir`: each script's IR, lowered, through the passes asked for
/// and verified after each, printed; with `--passes-diff`, what each pass
/// changed. A script that does not check is said as `cig check` says it;
/// a verifier failure is the compiler's bug.
fn emit_cir(
    ctx: &Ctx,
    jobs: &[project::Job],
    passes: &[&'static dyn cigcompile::cir::pass::Pass],
    diffs: bool,
) -> Result<(), i32> {
    use cigcompile::cir::{emit_with, EmitError, LowerError};
    for job in jobs {
        let source = std::fs::read_to_string(&job.script).map_err(|e| {
            let d = cigscript::diagnostics::usage(format!("cannot read {}: {e}", job.file))
                .code("E1000")
                .with_hint("name a .cig script you can read");
            cigscript::cli::report(ctx, &d, None, None);
            exit::USAGE
        })?;
        match emit_with(&source, &job.file, passes, diffs) {
            Ok(e) => {
                // V6: an effectful pull called outside every burn, which
                // the run refuses with E701.
                for w in &e.warnings {
                    eprintln!("{}: {w}", job.file);
                }
                print!("{}", e.text);
                for (name, diff) in &e.diffs {
                    match diff {
                        Some(d) => print!(
                            "--- {} before {name}\n+++ {} after {name}\n{d}",
                            job.file, job.file
                        ),
                        None => println!("=== {name}: no change in {}", job.file),
                    }
                }
            }
            Err(EmitError::Lower(LowerError::Diagnostic(d))) => {
                cigscript::cli::report(ctx, &d, Some(&job.file), Some(&source));
                return Err(exit::CHECK);
            }
            Err(EmitError::Lower(LowerError::Internal(e))) => {
                let d = cigscript::diagnostics::internal(format!("lowering {}: {e}", job.file));
                cigscript::cli::report(ctx, &d, None, None);
                return Err(70);
            }
            Err(EmitError::Verify(e)) => {
                let lines: Vec<String> = e.errors.iter().map(|x| x.to_string()).collect();
                let d = cigscript::diagnostics::internal(format!(
                    "the IR of {} broke a rule after {}: {}",
                    job.file,
                    e.pass,
                    lines.join("; ")
                ));
                cigscript::cli::report(ctx, &d, None, None);
                return Err(70);
            }
        }
    }
    Ok(())
}

/// Remove the programs a build of this path wrote: only files that are
/// programs compiled from the same script, never anything else there.
fn clean(ctx: &Ctx, a: &CleanArgs) -> i32 {
    let fail = |d: cigscript::diagnostics::Diagnostic| {
        cigscript::cli::report(ctx, &d, None, None);
        exit::USAGE
    };
    let target = match runtime::target(a.target.as_deref()) {
        Ok(t) => t,
        Err(d) => return fail(d),
    };
    let jobs = match project::jobs(a.path.as_deref(), a.out.as_deref(), target) {
        Ok(j) => j,
        Err(d) => return fail(d),
    };
    // Only what a build of this path wrote is taken out (#28): a program at
    // the path whose bytes are the ones the build index recorded there. A
    // program of the same name from somewhere else is kept, and named.
    let mut index = cache::Index::load();
    for job in &jobs {
        let Some(program) = std::fs::File::open(&job.out)
            .ok()
            .and_then(|mut f| cigscript::program::read(&mut f).ok().flatten())
            .filter(|p| p.file == build::recorded_file(job))
        else {
            continue;
        };
        let sha256 = std::fs::read(&job.out)
            .map(|b| hex::encode(<sha2::Sha256 as sha2::Digest>::digest(b)))
            .unwrap_or_default();
        let ours = index.written(&job.out).as_deref() == Some(sha256.as_str());
        if !ours && !a.force {
            let why = format!(
                "a program, but not one this cigcompile wrote there (compiled from {}, sha256 {}...)",
                program.file,
                &sha256[..sha256.len().min(12)]
            );
            if ctx.json {
                println!(
                    "{}",
                    serde_json::json!({"kept": job.out.to_string_lossy(), "why": why})
                );
            } else {
                println!("kept {}: {why}; --force removes it", job.out.display());
            }
            continue;
        }
        {
            if let Err(e) = std::fs::remove_file(&job.out) {
                return fail(
                    cigscript::diagnostics::usage(format!(
                        "cannot remove {}: {e}",
                        job.out.display()
                    ))
                    .code("E1004")
                    .with_hint("check the folder's permissions"),
                );
            }
            index.forget_program(&job.out);
            if ctx.json {
                println!(
                    "{}",
                    serde_json::json!({"removed": job.out.to_string_lossy()})
                );
            } else {
                println!("removed {}", job.out.display());
            }
        }
    }
    index.save();
    exit::OK
}

fn fetch_cmd(ctx: &Ctx, a: &FetchArgs) -> i32 {
    let mut wanted: Vec<&'static runtime::Target> = Vec::new();
    if a.all {
        wanted.extend(runtime::TARGETS.iter());
    }
    for t in &a.target {
        match runtime::target(Some(t)) {
            Ok(t) => wanted.push(t),
            Err(d) => {
                cigscript::cli::report(ctx, &d, None, None);
                return exit::USAGE;
            }
        }
    }
    if wanted.is_empty() {
        wanted.push(runtime::host());
    }
    let mut code = exit::OK;
    for t in wanted {
        match fetch::fetch(t, !ctx.json) {
            Ok(p) => {
                if ctx.json {
                    println!(
                        "{}",
                        serde_json::json!({"target": t.triple, "runtime": p.to_string_lossy()})
                    );
                } else {
                    println!("{:<32} {}", t.triple, p.display());
                }
            }
            Err(d) => {
                cigscript::cli::report(ctx, &d, None, None);
                code = exit::USAGE;
            }
        }
    }
    code
}

fn targets(ctx: &Ctx) -> i32 {
    let host = runtime::host();
    let mut index = cache::Index::load();
    // What a build here would use, without fetching anything to say so.
    let here = runtime::find_with(host, None, &mut index, false);
    index.save();
    for t in runtime::TARGETS {
        let this = t.triple == host.triple;
        if ctx.json {
            println!(
                "{}",
                serde_json::json!({
                    "triple": t.triple,
                    "asset": t.asset,
                    "stage": runtime::STAGE,
                    "ci": t.ci(),
                    "missing": t.missing(),
                    "this_machine": this,
                    "runtime": if this { here.as_ref().ok().map(|r| r.path.to_string_lossy().to_string()) } else { None },
                })
            );
        } else {
            let note = match (this, &here) {
                (true, Ok(r)) => format!("  this machine; runtime {}", r.path.display()),
                (true, Err(d)) if d.code == "E1008" => format!(
                    "  this machine; {}: a build fetches cig's static release (cigcompile fetch)",
                    d.message
                ),
                (true, Err(_)) => {
                    "  this machine; no runtime kept here (cigcompile build fetches cig's release, or says why)".to_string()
                }
                _ => String::new(),
            };
            let line = format!(
                "{:<32} {:<16} stage {}  {:<22} {:<20}{note}",
                t.triple,
                t.asset,
                runtime::STAGE,
                t.ci(),
                t.missing()
            );
            println!("{}", line.trim_end());
        }
    }
    exit::OK
}
