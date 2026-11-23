// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2

//! The Cranelift target for a section's architecture.
//!
//! Settings are fixed, justified in a comment each:
//!
//! - `opt_level=speed`: a stage-C program cares about the compiled code's
//!   speed, never the compile time. `speed_and_size` costs the budget's
//!   step count accuracy for a small size win we do not need.
//! - `is_pic=true`: the code is mapped where it lies; it must not embed
//!   an absolute address.
//! - `enable_verifier=true`: Cranelift's own verifier runs on every
//!   function. A verifier failure is a generator defect, caught here.
//! - `unwind_info=false`: no DWARF or SEH tables; the runtime never
//!   walks a native stack.
//! - `enable_probestack=true` with `probestack_strategy=inline`: a
//!   function with a stack frame lets the prologue touch each guard
//!   page. `MAX_FRAME` is the smallest guard page, so an inline sequence
//!   is at most a handful of instructions.
//! - `enable_nan_canonicalization=false`: the interpreter never
//!   canonicalizes NaN bits, and `ctx.left`/`right` on a NaN exit must
//!   be the operand's own bits. The canonicalization pass would rewrite
//!   that.
//! - No ISA flag is ever enabled beyond the baseline: the program must
//!   run on every x86-64 or aarch64 machine of its target.

use cranelift_codegen::isa::OwnedTargetIsa;
use cranelift_codegen::settings::{self, Configurable, Flags};
use target_lexicon::Triple;

use cig_native::Arch;

/// Why there is no ISA.
#[derive(Debug)]
pub enum IsaError {
    /// Cranelift's `lookup` refused the triple: support for this target
    /// was not compiled in.
    Lookup(cranelift_codegen::isa::LookupError),
    /// A setting could not be set.
    Setting(settings::SetError),
    /// Flags could not be finalized.
    Codegen(cranelift_codegen::CodegenError),
}

impl core::fmt::Display for IsaError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            IsaError::Lookup(e) => write!(f, "Cranelift has no ISA for this target: {e}"),
            IsaError::Setting(e) => write!(f, "Cranelift refused a setting: {e}"),
            IsaError::Codegen(e) => write!(f, "Cranelift refused its flags: {e}"),
        }
    }
}

impl std::error::Error for IsaError {}

/// The triple a `native` section of `arch` is for.
///
/// Programs must run on any machine of the architecture, so the triple
/// is `*-unknown-linux-gnu`; nothing the code does depends on the libc.
pub fn triple_of(arch: Arch) -> Triple {
    let s = match arch {
        Arch::X86_64 => "x86_64-unknown-linux-gnu",
        Arch::Aarch64 => "aarch64-unknown-linux-gnu",
    };
    s.parse()
        .unwrap_or_else(|_| unreachable!("a fixed valid triple"))
}

/// An owned Cranelift ISA for `arch` with the crate's fixed settings.
pub fn target(arch: Arch) -> Result<OwnedTargetIsa, IsaError> {
    let mut b = settings::builder();
    for (name, value) in [
        ("opt_level", "speed"),
        ("is_pic", "true"),
        ("enable_verifier", "true"),
        ("unwind_info", "false"),
        ("enable_probestack", "true"),
        ("probestack_strategy", "inline"),
        ("enable_nan_canonicalization", "false"),
    ] {
        b.set(name, value).map_err(IsaError::Setting)?;
    }
    let flags = Flags::new(b);
    let isa_b = cranelift_codegen::isa::lookup(triple_of(arch)).map_err(IsaError::Lookup)?;
    // No ISA flag is set beyond the baseline: the default builder has
    // every optional feature off.
    isa_b.finish(flags).map_err(IsaError::Codegen)
}
