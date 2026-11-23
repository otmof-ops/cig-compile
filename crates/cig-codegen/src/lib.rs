// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2

//! # cig-codegen
//!
//! One job: typed specializations of CIR in, a native section out.
//!
//! The crate is small on purpose. It depends on Cranelift for the back
//! end and on [`cig_native`] for the format the VM and the code share;
//! it knows nothing of programs, payloads, files or the command line.
//!
//! ## What is compiled
//!
//! A specialization is a function and one row of argument types. One
//! qualifies ([`qualifies`]) when every value of it is an int, a float,
//! a bool or null and every instruction of its body is one native code
//! has. A call qualifies only when what it calls does, so the generator
//! takes the largest set closed under calls, compiles it, and reports
//! which specializations it did not compile and why.
//!
//! ## The code's behavior
//!
//! The tree-walker's (`src/interp/ops.rs` in cig-script), exactly.
//! Native code never makes an error: where the interpreter would raise
//! one, the code writes the site, the operands and `steps_left` to the
//! shared context and returns a non-zero status, and the VM has the
//! interpreter's own function make the error.
//!
//! ## Determinism
//!
//! The same inputs give the same bytes, on any host: the crate holds
//! no unordered map, reads no clock and detects no host CPU.
//! Specializations are sorted by `(module_index, row)` before
//! compilation; bodies come first, then entries.

#![deny(missing_docs)]
#![deny(unsafe_code)]

use std::collections::BTreeMap;

use cig_native::{Arch, Section};
use cigscript::lower::cir::infer::{Inference, Row};
use cigscript::lower::cir::ir::{FuncId, Module};

pub mod flow;
mod isa;
mod link;
mod lower;
mod qualify;
pub mod types;

pub use isa::{target as target_isa, triple_of, IsaError};
pub use qualify::{fixed_point, qualifies, Scalar, SpecKey};

/// What [`compile`] gives back.
pub struct Output {
    /// The section, ready for [`Section::to_bytes`].
    pub section: Section,
    /// Each specialization the inference found, compiled or not, and
    /// why not.
    pub report: Vec<Outcome>,
}

/// One specialization's outcome.
pub struct Outcome {
    /// The function this specialization is of.
    pub func: FuncId,
    /// Its row of argument types.
    pub row: Row,
    /// `true` when it is in the section.
    pub compiled: bool,
    /// Why it was left to the bytecode; empty when compiled.
    pub why: String,
}

/// The generator's own defects: cases the generator must handle and
/// does not. A specialization that does not qualify is an [`Outcome`],
/// never an [`Error`].
#[derive(Debug)]
pub enum Error {
    /// Cranelift's ISA could not be built for `arch`.
    Isa(IsaError),
    /// Cranelift refused to compile what the generator fed it: a
    /// verifier failure, an unsupported construction, or regalloc gave
    /// up. The `at` names the specialization.
    Codegen {
        /// The specialization that failed.
        at: SpecKey,
        /// Cranelift's own message.
        message: String,
    },
    /// A function produced a relocation this generator cannot resolve:
    /// anything but a call to another function of this section. A
    /// libcall is caught here.
    StrayReloc {
        /// The specialization that produced it.
        at: SpecKey,
        /// Cranelift's own description.
        what: String,
    },
    /// A function left a trap in its code. The generator compiles no
    /// trapping instruction, so this is a Cranelift inlay we did not
    /// expect; a defect to find.
    Trap {
        /// The specialization that produced it.
        at: SpecKey,
        /// Cranelift's trap code.
        what: String,
    },
    /// The section's own writer refused: a code length or table shape
    /// out of the format's rules. A defect in the generator.
    Format(cig_native::FormatError),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::Isa(e) => write!(f, "cannot build Cranelift ISA: {e}"),
            Error::Codegen { at, message } => {
                write!(
                    f,
                    "Cranelift refused spec @{:?} row {:?}: {message}",
                    at.0, at.1
                )
            }
            Error::StrayReloc { at, what } => write!(
                f,
                "spec @{:?} row {:?} has a relocation this generator does not resolve: {what}",
                at.0, at.1
            ),
            Error::Trap { at, what } => write!(
                f,
                "spec @{:?} row {:?} has a reachable trap: {what}",
                at.0, at.1
            ),
            Error::Format(e) => write!(f, "the section's writer refused: {e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Isa(e) => Some(e),
            Error::Format(e) => Some(e),
            _ => None,
        }
    }
}

impl From<IsaError> for Error {
    fn from(e: IsaError) -> Self {
        Error::Isa(e)
    }
}

impl From<cig_native::FormatError> for Error {
    fn from(e: cig_native::FormatError) -> Self {
        Error::Format(e)
    }
}

/// Compile every qualifying specialization of `inf` for `arch`.
///
/// - `m` is the module the inference is of.
/// - `index` gives each CIR function's index in the bytecode module (a
///   function not listed is not in the module and is not compiled).
/// - `module` is the module's BLAKE3, copied into the section's
///   footer.
///
/// The qualifying set is [`fixed_point`]'s. A frame's size is known
/// only of compiled code: a specialization whose frame is larger than
/// [`cig_native::MAX_FRAME`] is dropped, with whatever calls it, and
/// the rest is compiled again.
pub fn compile(
    m: &Module,
    inf: &Inference,
    index: &BTreeMap<FuncId, u32>,
    module: [u8; 32],
    arch: Arch,
) -> Result<Output, Error> {
    let isa = isa::target(arch)?;
    // The qualifying set, and why each of the others is not in it.
    let (mut set, mut outcomes) = qualify::fixed_point(m, inf, index);
    // Compile, measure, drop oversized frames, repeat. Specs are
    // compiled in sorted order so each one knows its site_base (the
    // section's index of its first site).
    let compiled = loop {
        // Sort the current set by (module_index, row) so site_base
        // counts in the final section's order.
        let mut ordered: Vec<SpecKey> = set.iter().cloned().collect();
        ordered.sort_by(|a, b| {
            let ai = index.get(&a.0).copied().unwrap_or(u32::MAX);
            let bi = index.get(&b.0).copied().unwrap_or(u32::MAX);
            (ai, &a.1).cmp(&(bi, &b.1))
        });
        let mut round = Vec::new();
        let mut oversized: Vec<SpecKey> = Vec::new();
        let mut site_base = 0u32;
        for key in &ordered {
            let s = match inf.spec(key.0, &key.1) {
                Some(s) => s,
                None => continue,
            };
            let out =
                lower::body(m, inf, &set, s, &*isa, site_base).map_err(|e| Error::Codegen {
                    at: key.clone(),
                    message: e,
                })?;
            if out.frame_bytes > cig_native::MAX_FRAME {
                oversized.push(key.clone());
                continue;
            }
            if let Some(r) = out.stray_relocs.first() {
                return Err(Error::StrayReloc {
                    at: key.clone(),
                    what: r.clone(),
                });
            }
            if let Some(t) = out.traps.first() {
                return Err(Error::Trap {
                    at: key.clone(),
                    what: t.clone(),
                });
            }
            site_base = site_base
                .checked_add(out.sites.len() as u32)
                .ok_or_else(|| Error::Codegen {
                    at: key.clone(),
                    message: "more than u32 sites in a section".into(),
                })?;
            round.push((key.clone(), out));
        }
        if oversized.is_empty() {
            break round;
        }
        for k in oversized {
            set.remove(&k);
            outcomes.push((
                k,
                format!("frame larger than {} bytes", cig_native::MAX_FRAME),
            ));
        }
        // What called one of those goes with it, and says so.
        let before = set.clone();
        set = prune_calls(inf, set);
        for k in before.difference(&set) {
            outcomes.push((k.clone(), "calls a function not compiled".into()));
        }
    };
    // Build the section.
    let (section, compiled_keys) = link::assemble(index, module, arch, &*isa, compiled)?;
    // Report: order of specs as the inference gives them (sorted by
    // `(FuncId, Row)`).
    let compiled_set: std::collections::BTreeSet<SpecKey> = compiled_keys.iter().cloned().collect();
    let mut report = Vec::new();
    for s in inf.specs() {
        let key = (s.func, s.row.clone());
        if compiled_set.contains(&key) {
            report.push(Outcome {
                func: key.0,
                row: key.1,
                compiled: true,
                why: String::new(),
            });
        } else {
            // Every spec outside the set was given its reason when it
            // left; one without is a defect, and still says something.
            let why = outcomes
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, w)| w.clone())
                .filter(|w| !w.is_empty())
                .unwrap_or_else(|| "not compiled".to_string());
            report.push(Outcome {
                func: key.0,
                row: key.1,
                compiled: false,
                why,
            });
        }
    }
    Ok(Output { section, report })
}

/// Shrink `set` by removing any member whose calls name a spec not in
/// the set. Used after an oversized frame drops a spec.
fn prune_calls(
    inf: &Inference,
    mut set: std::collections::BTreeSet<SpecKey>,
) -> std::collections::BTreeSet<SpecKey> {
    loop {
        let before = set.len();
        let mut removed = Vec::new();
        for key in &set {
            let s = match inf.spec(key.0, &key.1) {
                Some(s) => s,
                None => continue,
            };
            for (g, row) in s.calls.values() {
                let callee = (*g, row.clone());
                if !set.contains(&callee) {
                    removed.push(key.clone());
                    break;
                }
            }
        }
        for k in &removed {
            set.remove(k);
        }
        if set.len() == before {
            return set;
        }
    }
}
