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
mod deb;
mod digests;
mod fat;
mod fetch;
mod gzip;
mod oci;
mod pkg;
mod project;
mod reduce;
mod runtime;
mod sbom;
mod tar;
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
    /// Generate a Homebrew formula or a Scoop manifest for a release, pointing at its release assets by URL and sha256. Nothing is built.
    Package(PackageArgs),
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
    /// `cir`: print the verified IR (#3) instead of a program, shaped by `-O`/`--passes`. `cir-opt`: the same IR, said in words: after the passes (at stage A, equivalent to `cir` at the chosen level; stage B may split them). `oci`: the program, and beside it an OCI image of it (`<program>.oci.tar`, a scratch image of one file; Linux targets). `deb`: the program, and beside it a `.deb` that installs it to `/usr/bin/<name>` (Linux targets; `--pkg-version`, `--pkg-maintainer`, `--pkg-description` shape the control file).
    #[arg(long, value_name = "WHAT", value_parser = ["cir", "cir-opt", "oci", "deb"])]
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
        Some(Command::Build(a)) if a.watch => watch(&ctx, &a),
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
        Some(Command::Package(a)) => package_cmd(&ctx, &a),
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
                        script: a.script.as_deref(),
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
    let jobs = project::jobs(a.path.as_deref(), a.out.as_deref(), target).map_err(fail)?;
    let passes = passes_of(a).map_err(fail)?;
    // `cir` and `cir-opt` print the same IR at stage A: both are shaped by
    // `-O`/`--passes`, and `cir-opt` is the explicit "after-passes" name
    // reserved so that stage B can split the two without a flag change.
    if matches!(a.emit.as_deref(), Some("cir") | Some("cir-opt")) {
        emit_cir(ctx, &jobs, &passes, a.passes_diff, a.remarks)?;
        return Ok(None);
    }
    if a.remarks {
        // `--remarks` on a plain build: run the pass pipeline once per
        // script so each pass can say what it did or left, then let the
        // build go on. The IR is not kept and nothing else changes.
        remark_only(ctx, &jobs, &passes)?;
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
    if oci {
        for b in &built {
            let image = std::fs::read(&b.job.out)
                .ok()
                .and_then(|bytes| oci::image(&b.job.name, &bytes, target.triple));
            let path = oci::path_for(&b.job.out);
            let written = image.map(|i| build::write_if_changed(&path, &i));
            match written {
                Some(Ok(_)) if ctx.json => println!(
                    "{}",
                    serde_json::json!({"image": path.to_string_lossy(), "program": b.job.out.to_string_lossy()})
                ),
                Some(Ok(_)) => println!(
                    "  image  {} (docker load -i, then run localhost/{}:latest)",
                    path.display(),
                    b.job.name
                ),
                _ => {
                    return Err(fail(
                        cigscript::diagnostics::usage(format!("cannot write {}", path.display()))
                            .code("E1004")
                            .with_hint("write it somewhere you can with -o"),
                    ))
                }
            }
        }
    }
    if deb {
        emit_deb(ctx, a, target, &built).map_err(fail)?;
    }
    Ok(Some((built, rt)))
}

/// `--emit deb`: beside each program, a `.deb` that installs it to
/// `/usr/bin/<name>`. Reproducible: the same build writes the same bytes
/// (src/deb.rs). Nothing is installed to make it.
fn emit_deb(
    ctx: &cigscript::cli::Ctx,
    a: &BuildArgs,
    target: &runtime::Target,
    built: &[build::Built],
) -> Result<(), cigscript::diagnostics::Diagnostic> {
    for b in built {
        let meta = deb::meta_for(
            &b.job.name,
            target,
            a.pkg_version.as_deref(),
            a.pkg_maintainer.as_deref(),
            a.pkg_description.as_deref(),
        )?;
        let path = deb::path_for(&b.job.out, &meta);
        let program = std::fs::read(&b.job.out).map_err(|e| {
            cigscript::diagnostics::usage(format!("cannot read {}: {e}", b.job.out.display()))
                .code("E1004")
                .with_hint("write it somewhere you can with -o")
        })?;
        let archive = deb::package(&b.job.name, &program, &meta);
        build::write_if_changed(&path, &archive).map_err(|e| {
            cigscript::diagnostics::usage(format!("cannot write {}: {e}", path.display()))
                .code("E1004")
                .with_hint("write it somewhere you can with -o")
        })?;
        if ctx.json {
            println!(
                "{}",
                serde_json::json!({
                    "deb": path.to_string_lossy(),
                    "program": b.job.out.to_string_lossy(),
                    "package": meta.package,
                    "version": meta.version,
                    "architecture": meta.arch,
                })
            );
        } else {
            println!(
                "  deb    {} ({}_{}_{}.deb, dpkg -i to install to /usr/bin/{})",
                path.display(),
                meta.package,
                meta.version,
                meta.arch,
                b.job.name,
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

/// `--remarks` on a plain build: lower each script and run the passes to
/// collect remarks; nothing else is kept. The IR is thrown away: the
/// build that follows is unchanged. A script that does not check or lower
/// is said as `cig check` says it.
fn remark_only(
    ctx: &Ctx,
    jobs: &[project::Job],
    passes: &[&'static dyn cigcompile::cir::pass::Pass],
) -> Result<(), i32> {
    use cigcompile::cir::pass::run_passes_with_remarks;
    use cigcompile::cir::{lower_source, LowerError};
    for job in jobs {
        let source = std::fs::read_to_string(&job.script).map_err(|e| {
            let d = cigscript::diagnostics::usage(format!("cannot read {}: {e}", job.file))
                .code("E1000")
                .with_hint("name a .cig script you can read");
            cigscript::cli::report(ctx, &d, None, None);
            exit::USAGE
        })?;
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
