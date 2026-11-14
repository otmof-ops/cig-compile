// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! A build: each script checked as `cig check` checks it, its pin held to
//! the runtime, and the program written beside where it goes and moved into
//! place whole. A program whose bytes are already there is not touched,
//! not even its time.

use crate::project::Job;
use crate::runtime::{Runtime, Target};
use cigscript::cli::Ctx;
use cigscript::diagnostics::{usage, Diagnostic};
use cigscript::program::{self, Section, Stage};
use cigscript::syntax::span::Span;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::io::Write as _;
use std::path::Path;

/// Exit codes, the same as cig's for the same things.
pub mod exit {
    pub const OK: i32 = 0;
    pub const DIFFERS: i32 = 1;
    pub const CHECK: i32 = 2;
    pub const USAGE: i32 = 3;
}

/// What came of one script.
pub struct Built {
    pub job: Job,
    /// The program's sha256, hex.
    pub sha256: String,
    pub bytes: u64,
    /// False when the program was already there, byte for byte.
    pub written: bool,
    /// The stage the program is (read back from it when it was up to date).
    pub stage: Option<Stage>,
}

/// The stage of the program at `path`, from its record.
fn stage_at(path: &Path) -> Option<Stage> {
    let mut f = std::fs::File::open(path).ok()?;
    program::read(&mut f).ok().flatten().map(|l| l.stage())
}

pub fn read_script(ctx: &Ctx, job: &Job) -> Result<String, i32> {
    // A program records its script's name and prints it; cig refuses to run
    // one whose name holds a control or invisible character (E1050), so
    // none is made (otmof-ops/cig-script#137).
    let recorded = recorded_file(job);
    if !cigscript::program::printable(&recorded) {
        let d = usage(format!(
            "{}: the script's name holds a control or invisible character, and a program prints the name it was compiled from",
            recorded.escape_debug()
        ))
        .code("E1009")
        .with_hint("rename the script to a name that shows as it is, then build again");
        cigscript::cli::report(ctx, &d, None, None);
        return Err(exit::USAGE);
    }
    std::fs::read_to_string(&job.script).map_err(|e| {
        let d = usage(format!("cannot read {}: {e}", job.file))
            .code("E1000")
            .with_hint("name a .cig or .cigar script you can read");
        cigscript::cli::report(ctx, &d, None, None);
        exit::USAGE
    })
}

/// What a build does besides compile: the SBOM, the strength a `.cigar`
/// script is held to (`--strength`; a `.cig` script is always mild), the
/// stage asked for, the level the program is lowered at, and whether every
/// argument is the program's.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub sbom: bool,
    pub strength: Option<cigarscript::features::Strength>,
    /// `--stage`: `None` makes the default stage (`DEFAULT_STAGE`).
    pub stage: Option<Stage>,
    /// `-O`: the level the bytecode is lowered at, recorded as `opt`.
    pub opt: u8,
    /// `--args direct`: every argument is the program's, as a script's
    /// `pack { own_flags: true }` says.
    pub args_direct: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            sbom: false,
            strength: None,
            stage: None,
            opt: 1,
            args_direct: false,
        }
    }
}

/// Why a script cannot be a stage `b` program: the first thing that
/// stopped its lowering, and where, when it has a place in the script.
#[derive(Clone, Debug)]
pub struct NotB {
    pub what: String,
    pub span: Option<Span>,
}

impl NotB {
    /// `file:line:col: what`, or `file: what`.
    pub fn said(&self, file: &str) -> String {
        match self.span {
            Some(s) => format!("{file}:{}:{}: {}", s.line, s.col, self.what),
            None => format!("{file}: {}", self.what),
        }
    }
}

/// The bytecode a stage `b` program carries: the script lowered to CIR,
/// through the passes of level `opt`, to bytecode, verified, and loaded by
/// this cig's VM (its builtins, prelude names and burn-op kinds), with no
/// instruction the VM lists as not built. `Err` names the first thing that
/// stops it. The bytes are the module as `serialize_module` writes it.
pub fn bytecode_of(source: &str, file: &str, opt: u8) -> Result<Vec<u8>, NotB> {
    use cigcompile::bc::{compile, CompileError, Error};
    use cigcompile::cir::LowerError;
    use cigscript::cig_bytecode as bc;
    let passes = cigcompile::cir::pass::level(opt);
    let m = compile(source, file, &passes).map_err(|e| match e {
        CompileError::Lower(LowerError::Diagnostic(d)) => NotB {
            what: d.message,
            span: d.span,
        },
        CompileError::Lower(LowerError::Internal(e)) => NotB {
            what: format!("the lowering to IR failed ({e}), a cigcompile bug"),
            span: None,
        },
        CompileError::Verify(e) => NotB {
            what: format!(
                "the IR broke a rule after {} ({}), a cigcompile bug",
                e.pass,
                e.errors
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
            span: None,
        },
        CompileError::Bytecode(Error::Refused { construct, span }) => NotB {
            what: format!("{construct} is not lowered to bytecode"),
            span: Some(span),
        },
        CompileError::Bytecode(Error::Limit { what, span }) => NotB {
            what,
            span: Some(span),
        },
        CompileError::Bytecode(Error::Internal(e)) => NotB {
            what: format!("{e}, a cigcompile bug"),
            span: None,
        },
    })?;
    if let Some(n) = not_built(&m.main) {
        return Err(n);
    }
    let verified = bc::verify_module(&m).map_err(|e| NotB {
        what: format!("the bytecode does not verify: {e}, a cigcompile bug"),
        span: None,
    })?;
    cigscript::vm::load(verified).map_err(|d| NotB {
        what: format!("this cig's VM does not load it: {}", d.message),
        span: d.span,
    })?;
    Ok(bc::serialize_module(&m))
}

/// The first instruction, in pre-order, that this cig's VM lists as not
/// built (`vm::NOT_BUILT`), with its line from the function's line table.
fn not_built(f: &cigscript::cig_bytecode::Function) -> Option<NotB> {
    use cigscript::cig_bytecode::decode;
    let mut pc = 0u32;
    while (pc as usize) < f.instrs.len() {
        let Ok(d) = decode(&f.instrs, pc) else {
            break;
        };
        if cigscript::vm::NOT_BUILT.contains(&d.op) {
            let row = f.lines.iter().take_while(|r| r.pc <= pc).last();
            let span =
                row.map(|r| Span::new(0, 0, f.span_base.line.saturating_add(r.line_off), r.col));
            return Some(NotB {
                what: format!(
                    "it needs the instruction `{:?}`, which this cig's VM does not run",
                    d.op
                ),
                span,
            });
        }
        pc = d.pc + d.width;
    }
    f.children.iter().find_map(not_built)
}

/// The stage a build makes and, at stage `b`, its bytecode; with the note
/// a build says when it makes stage `a` of a script it was not told to.
pub struct Staged {
    pub stage: Stage,
    pub bytecode: Option<Vec<u8>>,
    pub note: Option<String>,
}

/// The stage a build makes without `--stage`: `a` until stage `b` passes
/// its speed gates (cig-compile#31), so a build with no flag never makes a
/// slower program. Set to `B`, a build with no flag makes stage `b` when
/// the script lowers and stage `a`, with a note naming why, when it does
/// not.
pub const DEFAULT_STAGE: Stage = Stage::A;

/// The stage of one script. `--stage a` never lowers; `--stage b` on a
/// script that does not lower is `E1010`, said here; no flag makes
/// [`DEFAULT_STAGE`].
pub fn stage_of(ctx: &Ctx, job: &Job, source: &str, opts: &Options) -> Result<Staged, i32> {
    let asked = opts.stage.or(match DEFAULT_STAGE {
        Stage::A => Some(Stage::A),
        Stage::B => None,
    });
    if asked == Some(Stage::A) {
        return Ok(Staged {
            stage: Stage::A,
            bytecode: None,
            note: None,
        });
    }
    match bytecode_of(source, &recorded_file(job), opts.opt) {
        Ok(b) => Ok(Staged {
            stage: Stage::B,
            bytecode: Some(b),
            note: None,
        }),
        Err(n) if opts.stage == Some(Stage::B) => {
            let mut d = usage(format!(
                "{} cannot be a stage b program: {}",
                job.file, n.what
            ))
            .code("E1010")
            .with_hint("build it without --stage b (a stage a program runs the same script on the tree-walker), or rewrite what the message names");
            if let Some(s) = n.span {
                d = d.at(s);
            }
            cigscript::cli::report(ctx, &d, Some(&job.file), Some(source));
            Err(exit::USAGE)
        }
        Err(n) => Ok(Staged {
            stage: Stage::A,
            bytecode: None,
            note: Some(format!(
                "note: {} is a stage a program: {}\n",
                job.file,
                n.said(&job.file)
            )),
        }),
    }
}

/// Whether every argument is the program's: the script's `pack` says
/// `own_flags: true`, or the build says `--args direct`.
pub fn own_flags(source: &str, opts: &Options) -> bool {
    opts.args_direct
        || cigscript::syntax::parse(source)
            .map(|p| p.own_flags())
            .unwrap_or(false)
}

/// A `.cigar` script, checked at its strength by `cigar`'s own checker and
/// lowered to the CigScript the program carries. A `.cig` script is
/// untouched. Errors are said here. Returns the script to compile, its
/// warnings (cigar's, kept to say again like cig's) and, for `.cigar`, the
/// strength it was checked at.
pub fn lower_cigar(
    ctx: &Ctx,
    job: &Job,
    text: &str,
    strength: Option<cigarscript::features::Strength>,
) -> Result<Option<cigarscript::prepare::Prepared>, i32> {
    if !crate::project::is_cigar(&job.script) {
        return Ok(None);
    }
    let render = cigarscript::prepare::Render {
        json: ctx.json,
        plain: ctx.plain,
        color: ctx.color,
    };
    cigarscript::prepare::prepare(&job.script, &job.file, text, strength, render)
        .map(Some)
        .map_err(|()| exit::CHECK)
}

/// The script as the IR emitters read it: a `.cigar` script's lowering,
/// anything else as written.
pub fn read_for_ir(
    ctx: &Ctx,
    job: &Job,
    strength: Option<cigarscript::features::Strength>,
) -> Result<String, i32> {
    let text = read_script(ctx, job)?;
    Ok(lower_cigar(ctx, job, &text, strength)?
        .map(|p| p.source)
        .unwrap_or(text))
}

/// The experience warning for `strength full` (cigar-script#7): asked once
/// per project, remembered in the person's state, never asked of a run
/// nobody can answer. `Err` when the person says no.
fn experience_asked(job: &Job, asked: &mut BTreeSet<std::path::PathBuf>) -> Result<(), i32> {
    use cigarscript::experience as ex;
    let root = ex::project_root(&job.script);
    if !asked.insert(root.clone()) {
        return Ok(());
    }
    let said = ex::confirm_full(
        &ex::Dirs::from_env(),
        &root,
        ex::interactive(),
        &mut std::io::stdin().lock(),
        &mut std::io::stderr(),
    );
    if said == ex::Outcome::Declined {
        return Err(exit::USAGE);
    }
    Ok(())
}

/// Check a script: parse, then the checker, as `cig check` does. The
/// warnings are said here (and kept, to say again when a build changes
/// nothing); a program does not say them again on every start.
pub fn check(ctx: &Ctx, job: &Job, source: &str) -> Result<Vec<String>, i32> {
    let source = source.to_string();
    let parsed = match cigscript::syntax::parse(&source) {
        Ok(p) => p,
        Err(d) => {
            cigscript::cli::report(ctx, &d, Some(&job.file), Some(&source));
            return Err(exit::CHECK);
        }
    };
    let rep = cigscript::check::check(&parsed, &[]);
    if !rep.ok() {
        cigscript::cli::report_all(ctx, &rep.errors, &job.file, &source);
        return Err(exit::CHECK);
    }
    let warnings: Vec<String> = rep
        .warnings
        .iter()
        .map(|w| {
            w.render(Some(&job.file), Some(&source))
                .replacen("error[", "warning[", 1)
        })
        .collect();
    say_warnings(ctx, &warnings);
    Ok(warnings)
}

fn say_warnings(ctx: &Ctx, warnings: &[String]) {
    if !ctx.json {
        for w in warnings {
            eprint!("{}", ctx.yellow(w));
        }
    }
}

/// The script's pin, held to the cig the program will run on.
pub fn pin_holds(job: &Job, source: &str, runtime_version: &str) -> Result<(), Diagnostic> {
    let pin = match cigscript::pin::pin_for(&job.script, source) {
        Ok(Some(p)) => p,
        Ok(None) => return Ok(()),
        Err(e) => {
            return Err(usage(format!("{}: {e}", job.file))
                .code("E1003")
                .with_hint("fix the pin (`# cig >=1.15`), or remove it"))
        }
    };
    let v = cigscript::update::Version::parse(runtime_version)
        .expect("a runtime's version reads (runtime::load checked it)");
    if pin.constraint.satisfied_by(&v) {
        return Ok(());
    }
    Err(usage(format!(
        "{} asks for cig {}, and the program would run on cig {runtime_version}",
        job.file, pin.constraint
    ))
    .code("E1003")
    .with_hint(format!(
        "{}: build with the cigcompile for that cig, or move the pin once the script works on {runtime_version}",
        pin.describe(&job.script)
    )))
}

/// What a program records as its script: the file's name, never the path
/// it was built from. No folder of the build machine is in a program, and
/// a script gives the same bytes however it was named (#9).
pub fn recorded_file(job: &Job) -> String {
    job.script
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| job.file.clone())
}

/// What a program is made of, beside its runtime: format 2's record keys
/// and its sections after `source`.
pub struct Made<'a> {
    pub staged: &'a Staged,
    pub opt: u8,
    pub target: &'a str,
    pub own_flags: bool,
}

/// The program's bytes: the runtime with the checked script appended in
/// format 2, its record's `stage`, `opt`, `target` and `own_flags` filled,
/// and at stage `b` the `bytecode` section beside `source`.
pub fn program_bytes(
    job: &Job,
    source: &str,
    runtime: &Runtime,
    made: &Made,
) -> std::io::Result<Vec<u8>> {
    let mut b = program::Build::new(
        &job.name,
        &recorded_file(job),
        source,
        &format!("cigcompile {}", env!("CARGO_PKG_VERSION")),
        made.target,
    );
    b.stage = made.staged.stage;
    b.opt = made.opt.to_string();
    b.own_flags = made.own_flags;
    if let Some(code) = &made.staged.bytecode {
        b.sections.push(Section {
            id: "bytecode".to_string(),
            version: cigscript::cig_bytecode::FORMAT_VERSION,
            encoding: program::Encoding::Stored,
            required: true,
            bytes: code.clone(),
        });
    }
    program::append2(runtime.bytes()?, &b).map_err(std::io::Error::other)
}

/// Build every job; the first failure stops the build (a later script is
/// not built from a project that is not whole).
///
/// `stage_to`, when passed, redirects where the program (and the SBOM)
/// is written for each job: a staging folder beside `job.out` on the
/// same filesystem. The job's `.out` stays the final path (what the
/// compiler says and what the cache keys on), and a later commit step
/// (`crate::stage::commit`) moves each staged file into place, so a
/// failed sign or emit leaves `--out` as it was (#66). When `stage_to`
/// is set, the fresh-cache fast path is bypassed: a staged file is new,
/// and the up-to-date optimisation would not apply anyway.
pub fn build(
    ctx: &Ctx,
    jobs: &[Job],
    target: &Target,
    runtime: Option<&Runtime>,
    index: &mut crate::cache::Index,
    opts: Options,
    stage_to: Option<&[std::path::PathBuf]>,
) -> Result<Vec<Built>, i32> {
    if let Some(s) = stage_to {
        assert_eq!(s.len(), jobs.len(), "stage_to must be parallel to jobs");
    }
    use crate::timings::{phase, progress};
    progress(
        serde_json::json!({"event": "start", "scripts": jobs.len(), "check_only": runtime.is_none()}),
    );
    let sbom = opts.sbom;
    let mut asked: BTreeSet<std::path::PathBuf> = BTreeSet::new();
    let mut out = Vec::new();
    for (n, job) in jobs.iter().enumerate() {
        progress(
            serde_json::json!({"event": "script", "script": job.file, "n": n + 1, "of": jobs.len()}),
        );
        let source = phase(&job.file, "read", || read_script(ctx, job)).inspect_err(|_| {
            progress(serde_json::json!({"event": "failed", "script": job.file}));
        })?;
        // A `.cigar` script is checked by cigar and compiled as what it
        // lowers to; `as_written` is the file as it is, for the SBOM.
        let as_written = source;
        let lowered = phase(&job.file, "lower", || {
            lower_cigar(ctx, job, &as_written, opts.strength)
        })
        .inspect_err(|_| {
            progress(serde_json::json!({"event": "failed", "script": job.file}));
        })?;
        let (source, cigar_warnings) = match lowered {
            Some(p) => {
                if runtime.is_some() && p.strength == cigarscript::features::Strength::Full {
                    experience_asked(job, &mut asked)?;
                }
                (p.source, Some(p.warnings))
            }
            None => (as_written.clone(), None),
        };
        let check = |ctx: &Ctx, job: &Job, source: &str| match &cigar_warnings {
            // cigar's check read the lowered text through cig's own already.
            Some(w) => {
                say_warnings(ctx, w);
                Ok(w.clone())
            }
            None => check(ctx, job, source),
        };
        let Some(runtime) = runtime else {
            phase(&job.file, "check", || check(ctx, job, &source)).inspect_err(|_| {
                progress(serde_json::json!({"event": "failed", "script": job.file}));
            })?;
            if let Err(d) = pin_holds(job, &source, crate::runtime::wanted()) {
                cigscript::cli::report(ctx, &d, None, None);
                return Err(exit::USAGE);
            }
            if ctx.json {
                println!("{}", serde_json::json!({"checked": job.file}));
            } else {
                println!("checked {}", job.file);
            }
            progress(serde_json::json!({"event": "checked", "script": job.file}));
            continue;
        };
        let key = crate::cache::key(&[
            env!("CARGO_PKG_VERSION"),
            cigscript::VERSION,
            target.triple,
            &runtime.path.to_string_lossy(),
            &format!("{:?}", runtime.id),
            &job.file,
            &recorded_file(job),
            &job.name,
            &hex::encode(Sha256::digest(source.as_bytes())),
            opts.stage.map_or("auto", Stage::as_str),
            &opts.opt.to_string(),
            if opts.args_direct {
                "args-direct"
            } else {
                "args"
            },
        ]);
        // Where the program (and the SBOM) is written. With staging, a
        // path in the staging folder beside `job.out`; without, `job.out`
        // itself.
        let write_to: &std::path::Path = stage_to.map(|s| s[n].as_path()).unwrap_or(&job.out);
        // Unchanged since it was built: say the warnings again, check the
        // pin (a .cig-version may have come or gone), and touch nothing.
        // The fresh-cache fast path is bypassed with staging: the staged
        // path is a new empty folder, and the up-to-date optimisation
        // would not apply there.
        if stage_to.is_none() {
            if let Some((sha256, warnings)) =
                phase(&job.file, "index", || index.fresh(&job.out, &key))
            {
                say_warnings(ctx, &warnings);
                if let Err(d) = pin_holds(job, &source, &runtime.version) {
                    cigscript::cli::report(ctx, &d, None, None);
                    return Err(exit::USAGE);
                }
                let built = Built {
                    job: job.clone(),
                    sha256,
                    bytes: crate::cache::FileId::of(&job.out)
                        .map(|i| i.len)
                        .unwrap_or(0),
                    written: false,
                    stage: stage_at(&job.out),
                };
                if sbom {
                    if let Err(e) =
                        crate::sbom::write(job, &as_written, &built.sha256, runtime, &job.out)
                    {
                        let d = usage(format!("cannot write the SBOM beside the program: {e}"))
                            .code("E1004")
                            .with_hint("write it somewhere you can with -o");
                        cigscript::cli::report(ctx, &d, None, None);
                        return Err(exit::USAGE);
                    }
                }
                say(ctx, &built, target, runtime);
                progress(
                    serde_json::json!({"event": "built", "script": job.file, "program": job.out.to_string_lossy(), "written": false}),
                );
                out.push(built);
                continue;
            }
        }
        let mut warnings =
            phase(&job.file, "check", || check(ctx, job, &source)).inspect_err(|_| {
                progress(serde_json::json!({"event": "failed", "script": job.file}));
            })?;
        if let Err(d) = phase(&job.file, "pin", || {
            pin_holds(job, &source, &runtime.version)
        }) {
            cigscript::cli::report(ctx, &d, None, None);
            return Err(exit::USAGE);
        }
        let staged = phase(&job.file, "bytecode", || stage_of(ctx, job, &source, &opts))
            .inspect_err(|_| {
                progress(serde_json::json!({"event": "failed", "script": job.file}));
            })?;
        if let Some(note) = &staged.note {
            // Kept with the warnings, so a build that changes nothing
            // says it again.
            say_warnings(ctx, std::slice::from_ref(note));
            warnings.push(note.clone());
        }
        let made = Made {
            staged: &staged,
            opt: opts.opt,
            target: &crate::runtime::triple_of(target, runtime),
            own_flags: own_flags(&source, &opts),
        };
        let cannot_write = |e: std::io::Error, what: &str| {
            let d = usage(format!("cannot {what}: {e}"))
                .code("E1004")
                .with_hint(
                    "write it somewhere you can with -o, or free the space the message names",
                );
            cigscript::cli::report(ctx, &d, None, None);
            exit::USAGE
        };
        phase(&job.file, "runtime", || runtime.bytes().map(|_| ())).map_err(|e| {
            cannot_write(e, &format!("read the runtime {}", runtime.path.display()))
        })?;
        let bytes = phase(&job.file, "assemble", || {
            program_bytes(job, &source, runtime, &made)
        })
        .map_err(|e| cannot_write(e, &format!("read the runtime {}", runtime.path.display())))?;
        let sha256 = phase(&job.file, "hash", || hex::encode(Sha256::digest(&bytes)));
        let written = phase(&job.file, "write", || write_if_changed(write_to, &bytes))
            .map_err(|e| cannot_write(e, &format!("write {}", write_to.display())))?;
        // The cache keys on the final path, not the staged one: a later
        // build sees the final file (the commit step is what puts it
        // there) and the sha256 noted here still matches.
        index.note_program(&job.out, &key, &sha256, &warnings);
        if sbom {
            phase(&job.file, "sbom", || {
                crate::sbom::write(job, &as_written, &sha256, runtime, write_to)
            })
            .map_err(|e| cannot_write(e, "write the SBOM beside the program"))?;
        }
        progress(
            serde_json::json!({"event": "built", "script": job.file, "program": job.out.to_string_lossy(), "written": written}),
        );
        let built = Built {
            job: job.clone(),
            sha256,
            bytes: bytes.len() as u64,
            written,
            stage: Some(staged.stage),
        };
        say(ctx, &built, target, runtime);
        out.push(built);
    }
    progress(serde_json::json!({"event": "done", "scripts": jobs.len()}));
    Ok(out)
}

fn say(ctx: &Ctx, b: &Built, target: &Target, runtime: &Runtime) {
    if ctx.json {
        println!(
            "{}",
            serde_json::json!({
                "script": b.job.file,
                "program": b.job.out.to_string_lossy(),
                "name": b.job.name,
                "target": crate::runtime::triple_of(target, runtime),
                "needs": runtime.needs.as_ref().filter(|_| crate::runtime::wants_static(target)),
                "runtime": {"path": runtime.path.to_string_lossy(), "cig": runtime.version},
                "bytes": b.bytes,
                "sha256": b.sha256,
                "written": b.written,
                "stage": b.stage.map(Stage::as_str),
            })
        );
        return;
    }
    let size = format!("{:.1} MB", b.bytes as f64 / 1_000_000.0);
    if b.written {
        println!(
            "compiled {} -> {} ({size}, cig {} for {}, stage {})",
            b.job.file,
            b.job.out.display(),
            runtime.version,
            crate::runtime::label(target, runtime),
            b.stage.map_or("?", Stage::as_str)
        );
    } else {
        println!("up to date: {} ({size})", b.job.out.display());
    }
    println!("  sha256 {}", b.sha256);
}

/// Write `bytes` at `out` unless they are there already. A new program is
/// written beside its place and renamed over it, so a build that stops
/// halfway leaves the old program, or none, never half of one.
pub fn write_if_changed(out: &Path, bytes: &[u8]) -> std::io::Result<bool> {
    if let Ok(meta) = std::fs::metadata(out) {
        if meta.len() == bytes.len() as u64 && std::fs::read(out)? == bytes {
            return Ok(false);
        }
    }
    let dir = match out.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
        _ => std::path::PathBuf::from("."),
    };
    std::fs::create_dir_all(&dir)?;
    let name = out
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "program".to_string());
    let tmp = dir.join(format!(".{name}.cigcompile-{}.tmp", std::process::id()));
    let result = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            f.set_permissions(std::fs::Permissions::from_mode(0o755))?;
        }
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, out)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result.map(|()| true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rewrite_of_the_same_bytes_touches_nothing() {
        let d = tempfile::tempdir().unwrap();
        let out = d.path().join("sub").join("p");
        assert!(write_if_changed(&out, b"one").unwrap());
        let before = std::fs::metadata(&out).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert!(!write_if_changed(&out, b"one").unwrap());
        assert_eq!(std::fs::metadata(&out).unwrap().modified().unwrap(), before);
        assert!(write_if_changed(&out, b"two").unwrap());
        assert_eq!(std::fs::read(&out).unwrap(), b"two");
        let left: Vec<_> = std::fs::read_dir(d.path().join("sub"))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(left.len(), 1, "a temp file was left behind");
    }
}
