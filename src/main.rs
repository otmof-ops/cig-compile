// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! cigcompile: CigScript scripts as programs that run with nothing
//! installed. A program is the `cig` of the version this compiler pins,
//! with the checked script appended in payload format 2: at stage `b` with
//! the bytecode it lowers to (#31), which cig's VM runs, at stage `a` the
//! script alone, on the tree-walker. Either runs through the same run as
//! `cig run` (CigScript's docs/PROGRAMS.md).

mod audit;
mod bench;
mod benchgate;
mod build;
mod cache;
mod crash;
mod deb;
mod digests;
mod doctor;
mod fat;
mod fetch;
mod gzip;
mod installer;
mod oci;
mod pass;
mod pkg;
mod project;
mod reduce;
mod runtime;
mod sbom;
mod sign;
mod stage;
mod tar;
mod test;
mod timings;
mod trial;
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
    /// Before a project trusts a newer cigcompile: this one and that one both build the project and run its tests, and what differs is said first. The installer says where the new one is; nothing is pinned.
    Trial(TrialArgs),
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
    /// Generate a Homebrew formula or a Scoop manifest for a release, pointing at its release assets by URL and sha256. Nothing is built.
    Package(PackageArgs),
    /// Report a bug: hands over to `ciginstaller bug cigcompile`, with whatever follows.
    #[command(disable_help_flag = true)]
    Report {
        /// Passed to `ciginstaller bug cigcompile` as given.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
        args: Vec<String>,
    },
    /// The checks only cigcompile can make, for `ciginstaller doctor --tools` (read-only; --json for the findings).
    #[command(name = "__doctor", hide = true)]
    Doctor,
}

#[derive(Args)]
struct TrialArgs {
    /// The carton to try, as `ciginstaller pin carton` takes it (`0.2`, `0.2.0`); it must be installed (`ciginstaller install carton --carton <new>`).
    new: String,
    /// The project: a folder of scripts (default: this folder). It is copied; yours is not touched.
    #[arg(long, value_name = "DIR", default_value = ".")]
    project: PathBuf,
    /// A folder of scripts (a differential suite) that both build and really run, stdout and exit compared, when nothing codegen-relevant differs between them.
    #[arg(long, value_name = "DIR")]
    differential: Option<PathBuf>,
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
    /// A detached SSH signature of the program (as `build --sign` writes), to check against.
    #[arg(long, value_name = "FILE")]
    signature: Option<PathBuf>,
    /// An ssh allowed-signers file naming who may have signed the program; without it, only the signature's integrity is checked (`-Y check-novalidate`).
    #[arg(long, value_name = "FILE")]
    allowed_signers: Option<PathBuf>,
    /// The signer's identity to require in the allowed-signers file (default `*`, any identity there).
    #[arg(long, value_name = "ID")]
    identity: Option<String>,
    /// A program pass (as `build --pass` writes, <program>.pass) to check: it holds here (this machine made it, or a signer trusted here for programs did) and it names this program. --allowed-signers, when given, is who is trusted for it (a line's namespaces must name `cigpass-program`); else cig's own allowed signers.
    #[arg(long, value_name = "FILE")]
    pass: Option<PathBuf>,
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
    /// One user script (its `.cig`) to bench on its own, built and
    /// interpreted through the same harness as the set. The expected
    /// output is what `cig run <script>` prints with no arguments.
    #[arg(conflicts_with = "set")]
    script: Option<PathBuf>,
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
    /// Allocation counts (#30): a runtime built with `cargo build --release --bin cig-runtime --features alloc-count`. One interpreted run of each benchmark under it adds an allocations column.
    #[arg(long, value_name = "RUNTIME")]
    alloc: Option<PathBuf>,
    /// Only these benchmarks of the set (comma separated): the pull request job's fast set.
    #[arg(long, value_delimiter = ',', value_name = "NAMES")]
    only: Vec<String>,
    /// The regression gate (#30): compare with a `main` result saved by `bench --json`; repeat to pool interleaved rounds. Exit 1 when a non-noisy benchmark is over 5% slower, the geometric mean over 2%, or peak memory or allocations over 10%, each by a Mann-Whitney test of the samples. With `--current` nothing is run.
    #[arg(long, value_name = "JSON")]
    against: Vec<PathBuf>,
    /// This pull request's results, from `bench --json` (repeat to pool rounds), instead of running the set: the gate compares two saved results.
    #[arg(long, value_name = "JSON", requires = "against")]
    current: Vec<PathBuf>,
    /// The nightly trend gate (#30): a folder of nightly `bench --json` files named by date; exit 1 when the set has been slower five nights in a row. Nothing is run.
    #[arg(long, value_name = "DIR", conflicts_with_all = ["against", "current"])]
    trend: Option<PathBuf>,
    /// The shared harness (#68): `bench/bench.toml` through `cig bench`, with its statistics and its output. Asking for any of `--watch`, `--timings`, `--stats`, `--syscalls`, `--manifest`, `--group`, or `--against` with a git ref, a release or a binary (not a saved `.json`) runs it; `--runs` is its `--samples`, `--only`, `--markdown` and `--json` its own. The flags above that belong to the set harness (`--ref`, `--budgets`, `--orchestrate`, `--alloc`, `--set`, `--current`, `--trend`) are not part of it.
    #[arg(long, value_name = "FILE")]
    manifest: Option<PathBuf>,
    /// Rebuild and rerun the cases when a watched file changes (`cig bench --watch`).
    #[arg(long)]
    watch: bool,
    /// Where each case's time went, by phase, from the descriptor `CIG_TIMINGS_FD` names: cigcompile's build phases and the interpreter's.
    #[arg(long)]
    timings: bool,
    /// Each case's counters, from `CIG_STATS_FD`.
    #[arg(long)]
    stats: bool,
    /// One extra run of each case under strace: its calls, an mmap/munmap thrash flagged, and a compiled-against-interpreted gap that comes from the allocator named `allocator, not the compiler`.
    #[arg(long)]
    syscalls: bool,
    /// Only the cases of these groups (comma separated).
    #[arg(long, value_delimiter = ',', value_name = "GROUPS")]
    group: Vec<String>,
    /// The cig that has `cig bench` (default: the runtime programs are built on, so both sides of a case run the same bytes).
    #[arg(long, value_name = "CIG")]
    cig: Option<PathBuf>,
}

#[derive(Args)]
struct BuildArgs {
    /// A .cig script or a .cigar file, or a folder of them (default: this folder).
    path: Option<PathBuf>,
    /// For a .cigar file: check it as if it declared this strength (`mild`, `medium` or `full`). Turning down is checked: a medium file built at mild lists everything above mild. A .cig script is always mild and ignores it.
    #[arg(long, value_name = "mild|medium|full")]
    strength: Option<String>,
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
    /// `cir`: print the verified IR (#3) instead of a program, shaped by `-O`/`--passes`. `cir-opt`: the same IR, said in words: after the passes (at stage A, equivalent to `cir` at the chosen level; stage B may split them). `bc`: the bytecode that IR lowers to (#31), verified and disassembled: what a stage b program carries. `oci`: the program, and beside it an OCI image of it (`<program>.oci.tar`, a scratch image of one file; Linux targets). `deb`: the program, and beside it a `.deb` that installs it to `/usr/bin/<name>` (Linux targets; `--pkg-version`, `--pkg-maintainer`, `--pkg-description` shape the control file).
    #[arg(long, value_name = "WHAT", value_parser = ["cir", "cir-opt", "bc", "oci", "deb"])]
    emit: Option<String>,
    /// The optimisation level: 0 (lowering only), 1 or 2 (every pass there is), default 1. A stage b program's bytecode is lowered at it, and every program records it.
    #[arg(short = 'O', value_name = "LEVEL", value_parser = ["0", "1", "2"], conflicts_with = "passes")]
    opt: Option<String>,
    /// Run exactly these passes, in this order, for bisecting: a comma list of fold-consts, copy-prop, sccp, dead-pure. They shape --emit and --remarks; a program is lowered at -O's level.
    #[arg(long, value_name = "A,B", value_delimiter = ',')]
    passes: Option<Vec<String>>,
    /// `b`: the program runs its bytecode on cig's VM, and a script that does not lower is refused (E1010). `a` (the default, until stage b passes its speed gates): it runs its script on the tree-walker.
    #[arg(long, value_name = "a|b", value_parser = ["a", "b"])]
    stage: Option<String>,
    /// `direct`: every argument on the program's command line is its script's, flags included, and the program takes no run flags (as a script's `pack { own_flags: true }` says).
    #[arg(long, value_name = "direct", value_parser = ["direct"])]
    args: Option<String>,
    /// With --emit: after the IR, what each pass changed, as a unified diff of the printed IR.
    #[arg(long, requires = "emit")]
    passes_diff: bool,
    /// Say on stderr what each pass did or left alone and why: one line per remark, one summary a pass. `--json` writes the remarks as JSON lines. Cheap when off (no extra work is done).
    #[arg(long)]
    remarks: bool,
    /// Where the build's time went, phase by phase, on stderr; =FILE also writes a Chrome trace (chrome://tracing, Perfetto).
    #[arg(long, value_name = "FILE", num_args = 0..=1, require_equals = true)]
    timings: Option<Option<PathBuf>>,
    /// What the build is doing as it goes, one JSON object a line on stderr (for a window or an editor).
    #[arg(long)]
    json_progress: bool,
    /// A CycloneDX 1.5 SBOM beside each program, <program>.cdx.json: its runtime and its script, by sha256.
    #[arg(long)]
    sbom: bool,
    /// A detached SSH signature beside each program, <program>.sig (and the SBOM's, with --sbom), made with `ssh-keygen -Y sign` and the namespace `cigcompile-program`. KEY is an ssh private-key file, or a public-key file with an agent holding the private one.
    #[arg(long, value_name = "KEY")]
    sign: Option<PathBuf>,
    /// A program pass beside each program, <program>.pass: a sealed pass from this machine's tin naming the program by sha256, with its script, its cig, what built it and its target (and its SBOM's sha256, with --sbom). Another machine that trusts this one for programs checks it with `cigcompile verify --pass`; `cig pass key --kinds program` prints the line its allowed signers need.
    #[arg(long)]
    pass: bool,
    /// Build, then build again whenever a script changes (a script added to the folder too), until Ctrl-C.
    #[arg(long, conflicts_with_all = ["emit", "check"])]
    watch: bool,
    /// With --emit deb, the Version field of the control file (default 0.0.0).
    #[arg(long, value_name = "VERSION")]
    pkg_version: Option<String>,
    /// With --emit deb, the Maintainer field (default `cigcompile <nobody@localhost>`).
    #[arg(long, value_name = "NAME <email>")]
    pkg_maintainer: Option<String>,
    /// With --emit deb, the Description field (default `<name>, a CigScript program compiled with cigcompile.`).
    #[arg(long, value_name = "TEXT")]
    pkg_description: Option<String>,
}

#[derive(Args)]
struct PackageArgs {
    /// A Homebrew formula (a Ruby class) for the release.
    #[arg(long, conflicts_with = "scoop")]
    brew: bool,
    /// A Scoop manifest (a JSON object) for the release.
    #[arg(long)]
    scoop: bool,
    /// A JSON manifest with the release's name, version, homepage, license, description, and an assets array of {target, url, sha256}. Combined with any --asset given on the command line.
    #[arg(long, value_name = "FILE")]
    manifest: Option<PathBuf>,
    /// The program's name on the formula or the manifest (default: the manifest's `name`).
    #[arg(long)]
    name: Option<String>,
    /// The release's version (default: the manifest's `version`).
    #[arg(long)]
    version: Option<String>,
    /// The project's homepage (default: the manifest's `homepage`).
    #[arg(long)]
    homepage: Option<String>,
    /// The program's license, as SPDX (default: the manifest's `license`).
    #[arg(long)]
    license: Option<String>,
    /// A one-line description (default: the manifest's `description`).
    #[arg(long)]
    description: Option<String>,
    /// A release asset, `TARGET=URL,SHA256`, repeatable: the target triple, then the URL, then the hex sha256. For `--brew` give macOS and Linux assets; for `--scoop` give Windows assets.
    #[arg(long, value_name = "TARGET=URL,SHA256")]
    asset: Vec<String>,
    /// Where the formula or the manifest goes (default stdout).
    #[arg(short, long)]
    out: Option<PathBuf>,
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

/// The words a command line may say in a crash report: the parser's own subcommands and flags.
fn cli_words() -> Vec<String> {
    use clap::CommandFactory;
    fn walk(c: &clap::Command, out: &mut Vec<String>) {
        out.push(c.get_name().to_string());
        for a in c.get_arguments() {
            out.extend(a.get_long().map(|l| format!("--{l}")));
            out.extend(a.get_short().map(|s| format!("-{s}")));
        }
        for s in c.get_subcommands() {
            walk(s, out);
        }
    }
    let mut out = Vec::new();
    walk(&Cli::command(), &mut out);
    out
}

fn main() {
    // A crash joins the family's one queue (cig-installer#10); the hook is the first thing set.
    crash::install(cli_words());
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
        Some(Command::Build(a)) if a.watch => watch(&ctx, &a),
        Some(Command::Build(a)) => build_cmd(&ctx, &a).map(|_| exit::OK).unwrap_or_else(|c| c),
        Some(Command::Test(a)) => match build_cmd(&ctx, &a) {
            Ok(Some((built, rt))) => test::compare(&ctx, &built, &rt),
            Ok(None) => exit::OK,
            Err(c) => c,
        },
        Some(Command::Trial(a)) => trial::trial(
            &ctx,
            &trial::Opts {
                new: &a.new,
                project: &a.project,
                differential: a.differential.as_deref(),
            },
        ),
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
                signature: a.signature.as_deref(),
                allowed_signers: a.allowed_signers.as_deref(),
                identity: a.identity.as_deref(),
                pass: a.pass.as_deref(),
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
        Some(Command::Package(a)) => package_cmd(&ctx, &a),
        Some(Command::Doctor) => doctor::run(&ctx),
        Some(Command::Report { args }) => installer::report(&args),
        Some(Command::Bench(a)) if a.trend.is_some() || !a.current.is_empty() => {
            bench::gate_only(&ctx, a.trend.as_deref(), &a.against, &a.current, a.markdown)
        }
        Some(Command::Bench(a)) if bench_is_shared(&a) => {
            let cig = match &a.cig {
                Some(p) => Ok(p.clone()),
                None => {
                    let mut index = cache::Index::load();
                    let found = runtime::find(runtime::host(), a.runtime.as_deref(), &mut index);
                    index.save();
                    found.map(|rt| rt.path)
                }
            };
            match cig {
                Ok(cig) => bench::shared(
                    &ctx,
                    &bench::Shared {
                        manifest: a.manifest.as_deref(),
                        cig: &cig,
                        samples: (a.runs != 10).then_some(a.runs.max(1)),
                        only: &a.only,
                        group: &a.group,
                        markdown: a.markdown,
                        json: ctx.json,
                        watch: a.watch,
                        against: a.against.first().and_then(|p| p.to_str()),
                        timings: a.timings,
                        stats: a.stats,
                        syscalls: a.syscalls,
                    },
                ),
                Err(d) => {
                    cigscript::cli::report(&ctx, &d, None, None);
                    exit::USAGE
                }
            }
        }
        Some(Command::Bench(a)) => {
            let mut index = cache::Index::load();
            let found = runtime::find(runtime::host(), a.runtime.as_deref(), &mut index);
            index.save();
            match found {
                Ok(mut rt) => bench::bench(
                    &ctx,
                    &{
                        rt.release_bytes();
                        rt
                    },
                    &bench::Opts {
                        runs: a.runs.max(1),
                        budgets: a.budgets,
                        orchestrate: bench::path(&a.orchestrate),
                        refs: a.refs,
                        markdown: a.markdown,
                        set: a.set.as_deref(),
                        script: a.script.as_deref(),
                        alloc: a.alloc.as_deref(),
                        only: &a.only,
                        against: &a.against,
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

/// Does this `bench` run go through `cig bench` (the manifest) rather than the
/// set harness? The flags only the shared harness has, or `--against` given a
/// ref, a release or a binary instead of a saved `bench --json` result.
fn bench_is_shared(a: &BenchArgs) -> bool {
    let against_a_ref = !a.against.is_empty()
        && a.current.is_empty()
        && a.against
            .iter()
            .any(|p| p.extension().and_then(|e| e.to_str()) != Some("json"));
    a.watch
        || a.timings
        || a.stats
        || a.syscalls
        || a.manifest.is_some()
        || !a.group.is_empty()
        || against_a_ref
}

/// What a watched build looks at: each script it would build, and what
/// the file is (size, time, inode). A script added, removed or saved
/// changes it.
fn watched(
    a: &BuildArgs,
    target: &'static runtime::Target,
) -> Vec<(PathBuf, Option<cache::FileId>)> {
    project::jobs(a.path.as_deref(), a.out.as_deref(), target)
        .map(|jobs| {
            jobs.into_iter()
                .map(|j| {
                    let id = cache::FileId::of(&j.script);
                    (j.script, id)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `build --watch` (#37): build, then again whenever what it builds changes,
/// polled every tenth of a second. A failed build is said and waited out;
/// the next save tries again. The build index makes a script that did not
/// change cost about a millisecond, so every build is the whole one.
fn watch(ctx: &Ctx, a: &BuildArgs) -> i32 {
    let target = match runtime::target(a.target.as_deref()) {
        Ok(t) => t,
        Err(d) => {
            cigscript::cli::report(ctx, &d, None, None);
            return exit::USAGE;
        }
    };
    let mut seen = watched(a, target);
    loop {
        let built = build_cmd(ctx, a).is_ok();
        if !ctx.json {
            eprintln!(
                "watching {} script{}; Ctrl-C stops",
                seen.len(),
                if seen.len() == 1 { "" } else { "s" }
            );
        }
        let mut waited = 0u32;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(100));
            let now = watched(a, target);
            if now != seen {
                seen = now;
                break;
            }
            // Windows will not replace a program that is running: a build
            // that failed there is tried again every two seconds, so it goes
            // in once the program exits, without another save.
            waited += 1;
            if !built && cfg!(windows) && waited % 20 == 0 {
                break;
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
    let strength = strength_of(a).map_err(fail)?;
    let jobs = project::jobs(a.path.as_deref(), a.out.as_deref(), target).map_err(fail)?;
    let passes = passes_of(a).map_err(fail)?;
    // `cir` and `cir-opt` print the same IR at stage A: both are shaped by
    // `-O`/`--passes`, and `cir-opt` is the explicit "after-passes" name
    // reserved so that stage B can split the two without a flag change.
    if matches!(a.emit.as_deref(), Some("cir") | Some("cir-opt")) {
        emit_cir(ctx, &jobs, &passes, a.passes_diff, a.remarks, strength)?;
        return Ok(None);
    }
    if a.emit.as_deref() == Some("bc") {
        emit_bc(ctx, &jobs, &passes, strength)?;
        return Ok(None);
    }
    if a.remarks {
        // `--remarks` on a plain build: run the pass pipeline once per
        // script so each pass can say what it did or left, then let the
        // build go on. The IR is not kept and nothing else changes.
        remark_only(ctx, &jobs, &passes, strength)?;
    }
    let oci = a.emit.as_deref() == Some("oci");
    let deb = a.emit.as_deref() == Some("deb");
    if oci && oci::platform(target.triple).is_none() {
        return Err(fail(
            cigscript::diagnostics::usage(format!(
                "an OCI image is of a Linux program, and this build is for {}",
                target.triple
            ))
            .with_hint("build for Linux: --target x86_64-unknown-linux-musl (or aarch64-unknown-linux-musl, armv7-unknown-linux-musleabihf)"),
        ));
    }
    if deb && deb::arch(target.triple).is_none() {
        return Err(fail(
            cigscript::diagnostics::usage(format!(
                "a .deb is for a Linux target, and this build is for {}",
                target.triple
            ))
            .with_hint("build for Linux: --target x86_64-unknown-linux-musl (or aarch64-unknown-linux-musl, armv7-unknown-linux-musleabihf)"),
        ));
    }
    timings::enable(a.timings.is_some(), a.json_progress);
    let started = std::time::Instant::now();
    let finish = |built: &Result<_, i32>| {
        let total = started.elapsed().as_micros();
        if built.is_ok() {
            timings::write_fd();
        }
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
        let checked = build::build(
            ctx,
            &jobs,
            target,
            None,
            &mut index,
            build::Options {
                sbom: false,
                ..options_of(a, strength)
            },
            None,
        )
        .map(|_| ());
        finish(&checked);
        checked?;
        return Ok(None);
    }
    // Pre-flight: a `--sign` with no signer never writes a thing.
    // `ssh-keygen` must be on this machine and the key file must be
    // readable, before the runtime is loaded or a program is built.
    if let Some(key) = a.sign.as_deref() {
        if let Err(e) = sign::precheck_signer(key) {
            let d = cigscript::diagnostics::usage(e)
                .code("E1004")
                .with_hint("install OpenSSH (openssh-client on Debian and Ubuntu, openssh on macOS and Windows 10+), and name a key file you can read with --sign");
            cigscript::cli::report(ctx, &d, None, None);
            return Err(exit::USAGE);
        }
    }
    let found = runtime::find(target, a.runtime.as_deref(), &mut index);
    index.save();
    let rt = found.map_err(fail)?;
    // Stage every output beside `--out` when a build writes more than
    // one thing (`--sign`, `--sbom`, `--emit oci`, `--emit deb`). A
    // failing step then leaves `--out` as it was (#66): the staging
    // folder is removed and the error names what is still at `--out`.
    let want_staging = a.sign.is_some() || a.pass || a.sbom || oci || deb;
    let stages =
        if want_staging {
            match stage::plan(&jobs) {
                Ok(s) => s,
                Err(e) => return Err(fail(
                    cigscript::diagnostics::usage(format!(
                        "cannot open a staging folder beside --out: {e}"
                    ))
                    .code("E1004")
                    .with_hint(
                        "write it somewhere you can with -o, or free the space the message names",
                    ),
                )),
            }
        } else {
            Vec::new()
        };
    let stage_out: Vec<std::path::PathBuf> = stages.iter().map(|s| s.staged_out.clone()).collect();
    let stage_to = if want_staging {
        Some(stage_out.as_slice())
    } else {
        None
    };
    let built = build::build(
        ctx,
        &jobs,
        target,
        Some(&rt),
        &mut index,
        build::Options {
            sbom: a.sbom,
            ..options_of(a, strength)
        },
        stage_to,
    );
    index.save();
    finish(&built.as_ref().map(|_| ()).map_err(|c| *c));
    let built = match built {
        Ok(b) => b,
        Err(code) => {
            if want_staging {
                stage::discard(&stages);
            }
            return Err(code);
        }
    };
    // Each built's `b.job.out` is the FINAL path; `written_at[i]` is
    // where the file was written (a staging path when staging, the
    // final path otherwise). Sign and emit operate on the written-at
    // path, so a failure there does not touch `--out`.
    let written_at: Vec<std::path::PathBuf> = if want_staging {
        stage_out.clone()
    } else {
        built.iter().map(|b| b.job.out.clone()).collect()
    };
    if let Some(key) = a.sign.as_deref() {
        if let Err(code) = sign_built(ctx, &built, key, a.sbom, &written_at) {
            if want_staging {
                let why = stage::why_nothing_was_written(&stages);
                let d = cigscript::diagnostics::usage(why)
                    .code("E1004")
                    .with_hint("fix the signer and build again; nothing at --out was touched");
                cigscript::cli::report(ctx, &d, None, None);
                stage::discard(&stages);
            }
            return Err(code);
        }
    }
    if a.pass {
        let triple = runtime::triple_of(target, &rt);
        if let Err(code) = pass_built(ctx, &built, &triple, a.sbom, &written_at) {
            if want_staging {
                let why = stage::why_nothing_was_written(&stages);
                let d = cigscript::diagnostics::usage(why).code("E1004").with_hint(
                    "fix what the message names and build again; nothing at --out was touched",
                );
                cigscript::cli::report(ctx, &d, None, None);
                stage::discard(&stages);
            }
            return Err(code);
        }
    }
    warn_linkage(ctx, target, &rt);
    if oci {
        if let Err(code) = emit_oci(ctx, &built, target, &written_at) {
            if want_staging {
                let why = stage::why_nothing_was_written(&stages);
                let d = cigscript::diagnostics::usage(why)
                    .code("E1004")
                    .with_hint("write it somewhere you can with -o; nothing at --out was touched");
                cigscript::cli::report(ctx, &d, None, None);
                stage::discard(&stages);
            }
            return Err(code);
        }
    }
    if deb {
        if let Err(d) = emit_deb(ctx, a, target, &built, &written_at) {
            cigscript::cli::report(ctx, &d, None, None);
            if want_staging {
                let why = stage::why_nothing_was_written(&stages);
                let d = cigscript::diagnostics::usage(why)
                    .code("E1004")
                    .with_hint("write it somewhere you can with -o; nothing at --out was touched");
                cigscript::cli::report(ctx, &d, None, None);
                stage::discard(&stages);
            }
            return Err(exit::USAGE);
        }
    }
    let replaced: Vec<bool> = if want_staging {
        match stage::commit(stages) {
            Ok(r) => r,
            Err(e) => {
                let d = cigscript::diagnostics::usage(format!(
                    "cannot move built files into place: {e}"
                ))
                .code("E1004")
                .with_hint(
                    "write it somewhere you can with -o, or free the space the message names",
                );
                cigscript::cli::report(ctx, &d, None, None);
                return Err(exit::USAGE);
            }
        }
    } else {
        built.iter().map(|b| b.written).collect()
    };
    // A program and its signature always match: a build that does not sign
    // and replaced the program takes away the signatures beside it, which
    // signed the program from before.
    if a.sign.is_none() {
        for (b, r) in built.iter().zip(replaced.iter()) {
            if *r {
                drop_stale_signatures(ctx, &b.job.out);
            }
        }
    }
    // The same for a program pass: one that named the program from before
    // goes with it.
    if !a.pass {
        for (b, r) in built.iter().zip(replaced.iter()) {
            let stale = pass::path_for(&b.job.out);
            if *r && stale.is_file() && std::fs::remove_file(&stale).is_ok() && !ctx.json {
                println!(
                    "  removed {}: it named the program from before",
                    stale.display()
                );
            }
        }
    }
    Ok(Some((built, rt)))
}

/// Remove `<program>.sig` and the SBOM's `.sig` beside a program this
/// build replaced without signing, and say so.
fn drop_stale_signatures(ctx: &cigscript::cli::Ctx, program: &std::path::Path) {
    for sig in [
        sign::path_for(program),
        sign::path_for(&sbom::path_for(program)),
    ] {
        if sig.is_file() && std::fs::remove_file(&sig).is_ok() && !ctx.json {
            println!(
                "  removed {}: it signed the program from before",
                sig.display()
            );
        }
    }
}

/// `--emit deb`: beside each program, a `.deb` that installs it to
/// `/usr/bin/<name>`. Reproducible: the same build writes the same bytes
/// (src/deb.rs). Nothing is installed to make it.
///
/// `written_at[i]` is where the program was written (a staging path when
/// staging, else `b.job.out`); the `.deb` is written beside it. A failing
/// write there leaves the final `--out` as it was, since the commit step
/// is what moves either into place (#66).
fn emit_deb(
    ctx: &cigscript::cli::Ctx,
    a: &BuildArgs,
    target: &runtime::Target,
    built: &[build::Built],
    written_at: &[std::path::PathBuf],
) -> Result<(), cigscript::diagnostics::Diagnostic> {
    for (b, at) in built.iter().zip(written_at.iter()) {
        let meta = deb::meta_for(
            &b.job.name,
            target,
            a.pkg_version.as_deref(),
            a.pkg_maintainer.as_deref(),
            a.pkg_description.as_deref(),
        )?;
        let write_to = deb::path_for(at, &meta);
        let final_path = deb::path_for(&b.job.out, &meta);
        let program = std::fs::read(at).map_err(|e| {
            cigscript::diagnostics::usage(format!("cannot read {}: {e}", b.job.out.display()))
                .code("E1004")
                .with_hint("write it somewhere you can with -o")
        })?;
        let archive = deb::package(&b.job.name, &program, &meta);
        build::write_if_changed(&write_to, &archive).map_err(|e| {
            cigscript::diagnostics::usage(format!("cannot write {}: {e}", final_path.display()))
                .code("E1004")
                .with_hint("write it somewhere you can with -o")
        })?;
        if ctx.json {
            println!(
                "{}",
                serde_json::json!({
                    "deb": final_path.to_string_lossy(),
                    "program": b.job.out.to_string_lossy(),
                    "package": meta.package,
                    "version": meta.version,
                    "architecture": meta.arch,
                })
            );
        } else {
            println!(
                "  deb    {} ({}_{}_{}.deb, dpkg -i to install to /usr/bin/{})",
                final_path.display(),
                meta.package,
                meta.version,
                meta.arch,
                b.job.name,
            );
        }
    }
    Ok(())
}

/// `--emit oci`: beside each Linux program, an OCI image of it (#41).
///
/// `written_at[i]` is where the program was written (a staging path when
/// staging, else `b.job.out`); the image is written beside it. A failing
/// write there leaves the final `--out` as it was (#66).
fn emit_oci(
    ctx: &Ctx,
    built: &[build::Built],
    target: &runtime::Target,
    written_at: &[std::path::PathBuf],
) -> Result<(), i32> {
    let fail = |d: cigscript::diagnostics::Diagnostic| {
        cigscript::cli::report(ctx, &d, None, None);
        exit::USAGE
    };
    for (b, at) in built.iter().zip(written_at.iter()) {
        let image = std::fs::read(at)
            .ok()
            .and_then(|bytes| oci::image(&b.job.name, &bytes, target.triple));
        let write_to = oci::path_for(at);
        let final_path = oci::path_for(&b.job.out);
        let written = image.map(|i| build::write_if_changed(&write_to, &i));
        match written {
            Some(Ok(_)) if ctx.json => println!(
                "{}",
                serde_json::json!({"image": final_path.to_string_lossy(), "program": b.job.out.to_string_lossy()})
            ),
            Some(Ok(_)) => println!(
                "  image  {} (docker load -i, then run localhost/{}:latest)",
                final_path.display(),
                b.job.name
            ),
            _ => {
                return Err(fail(
                    cigscript::diagnostics::usage(format!("cannot write {}", final_path.display()))
                        .code("E1004")
                        .with_hint("write it somewhere you can with -o"),
                ))
            }
        }
    }
    Ok(())
}

/// `--sign <key>` (#10): a detached SSH signature of every program a
/// build wrote, and of its SBOM too when `--sbom` is on. The signature
/// goes beside the file, `<file>.sig`, with the namespace
/// `cigcompile-program`. ssh-keygen must be on this machine; a sign that
/// fails exits with a usage error that names what it said.
fn sign_built(
    ctx: &Ctx,
    built: &[build::Built],
    key: &std::path::Path,
    sbom: bool,
    written_at: &[std::path::PathBuf],
) -> Result<(), i32> {
    let fail = |message: String, hint: &str| {
        let d = cigscript::diagnostics::usage(message)
            .code("E1004")
            .with_hint(hint.to_string());
        cigscript::cli::report(ctx, &d, None, None);
        build::exit::USAGE
    };
    if let Err(e) = sign::require_ssh_keygen() {
        return Err(fail(
            e,
            "install OpenSSH (openssh-client on Debian and Ubuntu, openssh on macOS and Windows 10+)",
        ));
    }
    // The signed-at / signature-at path is what the sign step writes to
    // (staged when staging, the final path otherwise); the json/line
    // tells the operator the final path (`b.job.out`), which is where it
    // lands after the commit step.
    for (b, at) in built.iter().zip(written_at.iter()) {
        if let Err(e) = sign::sign(at, key) {
            return Err(fail(
                format!("cannot sign {}: {e}", b.job.out.display()),
                "give --sign an ssh private-key file, or a public-key file with ssh-agent holding the private one",
            ));
        }
        let sig = sign::path_for(&b.job.out);
        if ctx.json {
            println!(
                "{}",
                serde_json::json!({"signed": b.job.out.to_string_lossy(), "signature": sig.to_string_lossy(), "namespace": sign::NAMESPACE})
            );
        } else {
            println!(
                "  signed {} (ssh signature, namespace {})",
                sig.display(),
                sign::NAMESPACE
            );
        }
        if sbom {
            let sbom_at = sbom::path_for(at);
            if let Err(e) = sign::sign(&sbom_at, key) {
                let sbom_final = sbom::path_for(&b.job.out);
                return Err(fail(
                    format!("cannot sign {}: {e}", sbom_final.display()),
                    "give --sign an ssh private-key file, or a public-key file with ssh-agent holding the private one",
                ));
            }
            let sbom_final = sbom::path_for(&b.job.out);
            let sig = sign::path_for(&sbom_final);
            if ctx.json {
                println!(
                    "{}",
                    serde_json::json!({"signed": sbom_final.to_string_lossy(), "signature": sig.to_string_lossy(), "namespace": sign::NAMESPACE})
                );
            } else {
                println!(
                    "  signed {} (ssh signature, namespace {})",
                    sig.display(),
                    sign::NAMESPACE
                );
            }
        }
    }
    Ok(())
}

/// `build --pass`: a program pass beside each program written (a staging
/// path when staging), naming it by sha256, for the triple the program
/// really is (cig-script#186).
fn pass_built(
    ctx: &Ctx,
    built: &[build::Built],
    triple: &str,
    sbom: bool,
    written_at: &[std::path::PathBuf],
) -> Result<(), i32> {
    for (b, at) in built.iter().zip(written_at.iter()) {
        let sbom_at = sbom.then(|| sbom::path_for(at));
        let id = match pass::write(at, triple, sbom_at.as_deref()) {
            Ok(id) => id,
            Err(e) => {
                let d = cigscript::diagnostics::usage(format!(
                    "cannot make a program pass for {}: {e}",
                    b.job.out.display()
                ))
                .code("E1004")
                .with_hint("cig doctor says where this machine's tin key is kept, and why it cannot be read");
                cigscript::cli::report(ctx, &d, None, None);
                return Err(build::exit::USAGE);
            }
        };
        let to = pass::path_for(&b.job.out);
        if ctx.json {
            println!(
                "{}",
                serde_json::json!({"passed": b.job.out.to_string_lossy(), "pass": to.to_string_lossy(), "kind": pass::KIND, "id": id})
            );
        } else {
            println!(
                "  pass {} (a program pass from this machine, id {})",
                to.display(),
                &id[..12]
            );
        }
    }
    Ok(())
}

/// `cigcompile package`: a Homebrew formula or a Scoop manifest for a
/// released program, from flags and an optional JSON manifest. Nothing is
/// built; the formula or the manifest installs what the release already
/// published.
fn package_cmd(ctx: &cigscript::cli::Ctx, a: &PackageArgs) -> i32 {
    let fail = |d: cigscript::diagnostics::Diagnostic| {
        cigscript::cli::report(ctx, &d, None, None);
        exit::USAGE
    };
    if !a.brew && !a.scoop {
        return fail(
            cigscript::diagnostics::usage("name --brew or --scoop")
                .with_hint("--brew writes a Homebrew formula; --scoop writes a Scoop manifest"),
        );
    }
    let mut release = if let Some(path) = &a.manifest {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                return fail(
                    cigscript::diagnostics::usage(format!("cannot read {}: {e}", path.display()))
                        .code("E1000")
                        .with_hint("name a JSON manifest you can read with --manifest"),
                )
            }
        };
        match pkg::parse_manifest(&text) {
            Ok(r) => r,
            Err(d) => return fail(d),
        }
    } else {
        pkg::Release {
            name: String::new(),
            version: String::new(),
            homepage: String::new(),
            license: String::new(),
            description: String::new(),
            assets: Vec::new(),
        }
    };
    let overlay = |field: &mut String, flag: Option<&str>| {
        if let Some(v) = flag {
            *field = v.to_string();
        }
    };
    overlay(&mut release.name, a.name.as_deref());
    overlay(&mut release.version, a.version.as_deref());
    overlay(&mut release.homepage, a.homepage.as_deref());
    overlay(&mut release.license, a.license.as_deref());
    overlay(&mut release.description, a.description.as_deref());
    for s in &a.asset {
        match pkg::parse_asset(s) {
            Ok(asset) => {
                release.assets.retain(|x| x.target != asset.target);
                release.assets.push(asset);
            }
            Err(d) => return fail(d),
        }
    }
    for (field, name) in [
        (&release.name, "name"),
        (&release.version, "version"),
        (&release.homepage, "homepage"),
        (&release.license, "license"),
        (&release.description, "description"),
    ] {
        if field.is_empty() {
            return fail(
                cigscript::diagnostics::usage(format!("no {name}"))
                    .with_hint(format!("pass --{name}, or name it in the --manifest")),
            );
        }
    }
    if release.assets.is_empty() {
        return fail(
            cigscript::diagnostics::usage("no release assets to point at")
                .with_hint("pass --asset TARGET=URL,SHA256 for every per-target release asset"),
        );
    }
    let text = match if a.brew {
        pkg::brew(&release)
    } else {
        pkg::scoop(&release)
    } {
        Ok(t) => t,
        Err(d) => return fail(d),
    };
    match &a.out {
        Some(path) => {
            if let Err(e) = build::write_if_changed(path, text.as_bytes()) {
                return fail(
                    cigscript::diagnostics::usage(format!("cannot write {}: {e}", path.display()))
                        .code("E1004")
                        .with_hint("write it somewhere you can with -o"),
                );
            }
            if ctx.json {
                println!(
                    "{}",
                    serde_json::json!({
                        "kind": if a.brew { "brew" } else { "scoop" },
                        "out": path.to_string_lossy(),
                    })
                );
            } else {
                println!("wrote {}", path.display());
            }
        }
        None => {
            use std::io::Write as _;
            if let Err(e) = std::io::stdout().write_all(text.as_bytes()) {
                return fail(
                    cigscript::diagnostics::usage(format!("cannot write stdout: {e}"))
                        .code("E1004"),
                );
            }
        }
    }
    exit::OK
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
/// `--strength`, read: for a `.cigar` file (cigar-script#7).
fn strength_of(
    a: &BuildArgs,
) -> Result<Option<cigarscript::features::Strength>, cigscript::diagnostics::Diagnostic> {
    match a.strength.as_deref() {
        None => Ok(None),
        Some(s) => cigarscript::features::Strength::parse(s)
            .map(Some)
            .ok_or_else(|| {
                cigscript::diagnostics::usage(format!(
                    "--strength is `{s}`, want `mild`, `medium` or `full`"
                ))
                .code("E800")
                .with_hint("cigcompile build x.cigar --strength mild, medium or full")
            }),
    }
}

/// What a build is told: its strength, stage, level and `--args`.
fn options_of(a: &BuildArgs, strength: Option<cigarscript::features::Strength>) -> build::Options {
    build::Options {
        sbom: a.sbom,
        strength,
        stage: match a.stage.as_deref() {
            Some("a") => Some(cigscript::program::Stage::A),
            Some("b") => Some(cigscript::program::Stage::B),
            _ => None,
        },
        opt: level_of(a),
        args_direct: a.args.as_deref() == Some("direct"),
    }
}

/// `-O`'s level, 1 when it is not given.
fn level_of(a: &BuildArgs) -> u8 {
    a.opt.as_deref().unwrap_or("1").parse::<u8>().unwrap_or(1)
}

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
    Ok(pass::level(level_of(a)))
}

/// `--emit=cir` (or `--emit=cir-opt`): each script's IR, lowered, through
/// the passes asked for and verified after each, printed; with
/// `--passes-diff`, what each pass changed; with `--remarks`, each pass
/// also says what it did or left alone. A script that does not check is
/// said as `cig check` says it; a verifier failure is the compiler's bug.
fn emit_cir(
    ctx: &Ctx,
    jobs: &[project::Job],
    passes: &[&'static dyn cigcompile::cir::pass::Pass],
    diffs: bool,
    remarks: bool,
    strength: Option<cigarscript::features::Strength>,
) -> Result<(), i32> {
    use cigcompile::cir::{emit_with, EmitError, LowerError};
    for job in jobs {
        let source = build::read_for_ir(ctx, job, strength)?;
        match emit_with(&source, &job.file, passes, diffs, remarks) {
            Ok(e) => {
                // V6: an effectful pull called outside every burn, which
                // the run refuses with E701.
                for w in &e.warnings {
                    eprintln!("{}: {w}", job.file);
                }
                if remarks {
                    print_remarks(ctx, &job.file, &e.remarks);
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
                eprintln!("{}", installer::please_report());
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
                eprintln!("{}", installer::please_report());
                return Err(70);
            }
        }
    }
    Ok(())
}

/// `--emit=bc`: each script lowered to CIR, through the passes, to the
/// register bytecode (#31), verified and disassembled. A construct the
/// lowering does not cover yet is said with its position; a module that
/// does not verify is the compiler's bug.
fn emit_bc(
    ctx: &Ctx,
    jobs: &[project::Job],
    passes: &[&'static dyn cigcompile::cir::pass::Pass],
    strength: Option<cigarscript::features::Strength>,
) -> Result<(), i32> {
    use cigcompile::bc::{compile, disassemble, CompileError, Error};
    use cigcompile::cir::LowerError;
    for job in jobs {
        let source = build::read_for_ir(ctx, job, strength)?;
        let internal = |what: String| {
            let d = cigscript::diagnostics::internal(format!("lowering {}: {what}", job.file));
            cigscript::cli::report(ctx, &d, None, None);
            eprintln!("{}", installer::please_report());
            70
        };
        match compile(&source, &job.file, passes) {
            Ok(m) => print!("{}", disassemble(&m)),
            Err(CompileError::Lower(LowerError::Diagnostic(d))) => {
                cigscript::cli::report(ctx, &d, Some(&job.file), Some(&source));
                return Err(exit::CHECK);
            }
            Err(CompileError::Lower(LowerError::Internal(e))) => return Err(internal(e)),
            Err(CompileError::Verify(e)) => {
                let lines: Vec<String> = e.errors.iter().map(|x| x.to_string()).collect();
                return Err(internal(format!(
                    "the IR broke a rule after {}: {}",
                    e.pass,
                    lines.join("; ")
                )));
            }
            Err(CompileError::Bytecode(e)) => {
                let (what, span) = match e {
                    Error::Internal(e) => return Err(internal(e)),
                    Error::Refused { construct, span } => {
                        (format!("{construct} is not lowered to bytecode yet"), span)
                    }
                    Error::Limit { what, span } => (what, span),
                };
                let d = cigscript::diagnostics::usage(what).at(span).with_hint(
                    "--emit cir shows the IR; the lowering grows a part at a time (#31)",
                );
                cigscript::cli::report(ctx, &d, Some(&job.file), Some(&source));
                return Err(exit::USAGE);
            }
        }
    }
    Ok(())
}

/// `--remarks` on a plain build: lower each script and run the passes to
/// collect remarks; nothing else is kept. The IR is thrown away: the
/// build that follows is unchanged. A script that does not check or lower
/// is said as `cig check` says it.
fn remark_only(
    ctx: &Ctx,
    jobs: &[project::Job],
    passes: &[&'static dyn cigcompile::cir::pass::Pass],
    strength: Option<cigarscript::features::Strength>,
) -> Result<(), i32> {
    use cigcompile::cir::pass::run_passes_with_remarks;
    use cigcompile::cir::{lower_source, LowerError};
    for job in jobs {
        let source = build::read_for_ir(ctx, job, strength)?;
        match lower_source(&source, &job.file) {
            Ok(mut m) => match run_passes_with_remarks(&mut m, passes) {
                Ok((_, remarks)) => print_remarks(ctx, &job.file, &remarks),
                Err(e) => {
                    let lines: Vec<String> = e.errors.iter().map(|x| x.to_string()).collect();
                    let d = cigscript::diagnostics::internal(format!(
                        "the IR of {} broke a rule after {}: {}",
                        job.file,
                        e.pass,
                        lines.join("; ")
                    ));
                    cigscript::cli::report(ctx, &d, None, None);
                    eprintln!("{}", installer::please_report());
                    return Err(70);
                }
            },
            Err(LowerError::Diagnostic(d)) => {
                cigscript::cli::report(ctx, &d, Some(&job.file), Some(&source));
                return Err(exit::CHECK);
            }
            Err(LowerError::Internal(e)) => {
                let d = cigscript::diagnostics::internal(format!("lowering {}: {e}", job.file));
                cigscript::cli::report(ctx, &d, None, None);
                eprintln!("{}", installer::please_report());
                return Err(70);
            }
        }
    }
    Ok(())
}

/// Print the remarks a pass pipeline produced, prefixed with the file;
/// `--json` writes them as one JSON object a line, plain text otherwise.
fn print_remarks(ctx: &Ctx, file: &str, remarks: &[cigcompile::cir::pass::Remark]) {
    for r in remarks {
        if ctx.json {
            let line = if r.span.line > 0 {
                serde_json::json!({
                    "remark": r.pass,
                    "file": file,
                    "message": r.message,
                    "line": r.span.line,
                    "col": r.span.col,
                })
            } else {
                serde_json::json!({
                    "remark": r.pass,
                    "file": file,
                    "message": r.message,
                })
            };
            eprintln!("{line}");
        } else if r.span.line > 0 {
            eprintln!("{}:{}:{}: {}", file, r.span.line, r.span.col, r.message);
        } else {
            eprintln!("{}: {}", file, r.message);
        }
    }
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
            .filter(|p| p.file() == build::recorded_file(job))
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
                program.file(),
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
