// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! `--emit=cir` with the compiler's own extras: a diff of the printed IR
//! around every pass, and each pass's remarks. The lowering itself is
//! `cigscript::lower`.

use cigscript::lower::cir::{lower_source, pass, print, EmitError, VerifyError};

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
