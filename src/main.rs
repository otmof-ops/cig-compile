// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! cigcompile: CigScript scripts as programs that run with nothing
//! installed. Stage A (#2): a program is the `cig` of the version this
//! compiler pins, with the checked script appended; it runs through the
//! same run as `cig run` (CigScript's docs/PROGRAMS.md).

mod bench;
mod build;
mod cache;
mod digests;
mod fetch;
mod project;
mod runtime;
mod test;

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
    /// Runs per program and side; the median counts.
    #[arg(long, default_value_t = 5)]
    runs: usize,
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
        Some(Command::Bench(a)) => {
            let mut index = cache::Index::load();
            let found = runtime::find(runtime::host(), a.runtime.as_deref(), &mut index);
            index.save();
            match found {
                Ok(rt) => bench::bench(
                    &ctx,
                    &rt,
                    a.runs.max(1),
                    a.budgets,
                    bench::path(&a.orchestrate),
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
    if a.emit.is_some() {
        emit_cir(ctx, &jobs)?;
        return Ok(None);
    }
    let mut index = cache::Index::load();
    if a.check {
        build::build(ctx, &jobs, target, None, &mut index)?;
        return Ok(None);
    }
    let found = runtime::find(target, a.runtime.as_deref(), &mut index);
    index.save();
    let rt = found.map_err(fail)?;
    let built = build::build(ctx, &jobs, target, Some(&rt), &mut index);
    index.save();
    Ok(Some((built?, rt)))
}

/// `--emit=cir`: each script's IR, lowered, through the default passes
/// and verified after each, printed. A script that does not check is said
/// as `cig check` says it; a verifier failure is the compiler's bug.
fn emit_cir(ctx: &Ctx, jobs: &[project::Job]) -> Result<(), i32> {
    use cigcompile::cir::{emit, EmitError, LowerError};
    for job in jobs {
        let source = std::fs::read_to_string(&job.script).map_err(|e| {
            let d = cigscript::diagnostics::usage(format!("cannot read {}: {e}", job.file))
                .code("E1000")
                .with_hint("name a .cig script you can read");
            cigscript::cli::report(ctx, &d, None, None);
            exit::USAGE
        })?;
        match emit(&source, &job.file) {
            Ok((text, warnings)) => {
                // V6: an effectful pull called outside every burn, which
                // the run refuses with E701.
                for w in &warnings {
                    eprintln!("{}: {w}", job.file);
                }
                print!("{text}")
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
    for job in &jobs {
        let ours = std::fs::File::open(&job.out)
            .ok()
            .and_then(|mut f| cigscript::program::read(&mut f).ok().flatten())
            .is_some_and(|p| p.file == build::recorded_file(job));
        if ours {
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
    let here = runtime::find(host, None, &mut index).ok();
    index.save();
    for t in runtime::TARGETS {
        let this = t.triple == host.triple;
        if ctx.json {
            println!(
                "{}",
                serde_json::json!({
                    "triple": t.triple,
                    "asset": t.asset,
                    "this_machine": this,
                    "runtime": if this { here.as_ref().map(|r| r.path.to_string_lossy().to_string()) } else { None },
                })
            );
        } else {
            let note = match (this, &here) {
                (true, Some(r)) => format!("  this machine; runtime {}", r.path.display()),
                (true, None) => {
                    "  this machine; no runtime found (cigcompile build says why)".to_string()
                }
                _ => String::new(),
            };
            println!("{:<32} {:<16}{note}", t.triple, t.asset);
        }
    }
    exit::OK
}
