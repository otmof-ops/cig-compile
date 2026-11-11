// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! CIR: CigCompile's intermediate representation (#3), between the
//! checked AST and code generation, where the burn rule is structural: an
//! effect is a `burn.op` node, and the verifier proves every one sits in a
//! burn region (or in a function that may only be called from one) after
//! every pass. Stage B's bytecode is lowered from it.

pub mod effects;
pub mod ir;
pub mod lower;
pub mod pass;
pub mod print;
pub mod verify;

pub use ir::Module;
pub use verify::{verify, VerifyError};

/// Parse, check and lower a script. A script that does not parse or
/// check comes back as its first diagnostic, as `cig check` reports it.
pub fn lower_source(source: &str, file: &str) -> Result<Module, LowerError> {
    let program = cigscript::syntax::parse(source).map_err(LowerError::Diagnostic)?;
    let rep = cigscript::check::check(&program, &[]);
    if let Some(d) = rep.errors.into_iter().next() {
        return Err(LowerError::Diagnostic(d));
    }
    let resolver = effects::Resolver::new();
    lower::lower(&program, file, &resolver).map_err(LowerError::Internal)
}

#[derive(Debug)]
pub enum LowerError {
    /// The script does not parse or check.
    Diagnostic(cigscript::diagnostics::Diagnostic),
    /// The checker let through something lowering cannot place: a bug.
    Internal(String),
}

/// Lower, run the default passes, print: `--emit=cir`. The verifier's
/// warnings (V6) come back beside the text.
pub fn emit(source: &str, file: &str) -> Result<(String, Vec<VerifyError>), EmitError> {
    let mut m = lower_source(source, file).map_err(EmitError::Lower)?;
    let warnings = pass::run_passes(&mut m, &pass::default_passes()).map_err(EmitError::Verify)?;
    Ok((print::print(&m), warnings))
}

/// What `--emit=cir` printed, with what each pass changed when asked.
pub struct Emitted {
    pub text: String,
    pub warnings: Vec<VerifyError>,
    /// Each pass in the order it ran, with a unified diff of the printed
    /// IR before and after it, or `None` where it changed nothing.
    pub diffs: Vec<(&'static str, Option<String>)>,
    /// What each pass did or left alone, when `remarks` was asked for;
    /// empty otherwise. Ordered as the passes ran.
    pub remarks: Vec<pass::Remark>,
}

/// Lower, run `passes` (each between the verifier's checks), print. With
/// `diffs`, the printed IR is compared before and after every pass; with
/// `remarks`, each pass also says what it did or left alone. Both are
/// paid for only when asked.
pub fn emit_with(
    source: &str,
    file: &str,
    passes: &[&dyn pass::Pass],
    diffs: bool,
    remarks: bool,
) -> Result<Emitted, EmitError> {
    let mut m = lower_source(source, file).map_err(EmitError::Lower)?;
    let mut changed = Vec::new();
    let mut all_remarks: Vec<pass::Remark> = Vec::new();
    let mut warnings = pass::run_passes(&mut m, &[]).map_err(EmitError::Verify)?;
    for p in passes {
        let before = diffs.then(|| print::print(&m));
        if remarks {
            let (w, r) = pass::run_passes_with_remarks(&mut m, &[*p]).map_err(EmitError::Verify)?;
            warnings = w;
            all_remarks.extend(r);
        } else {
            warnings = pass::run_passes(&mut m, &[*p]).map_err(EmitError::Verify)?;
        }
        if let Some(before) = before {
            let after = print::print(&m);
            changed.push((p.name(), crate::diff::unified(&before, &after, 2)));
        }
    }
    Ok(Emitted {
        text: print::print(&m),
        warnings,
        diffs: changed,
        remarks: all_remarks,
    })
}

#[derive(Debug)]
pub enum EmitError {
    Lower(LowerError),
    Verify(pass::PassError),
}
