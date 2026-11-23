// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2

//! Build the section: an entry wrapper for each body, then link every
//! body and every entry into one blob with Cranelift's text-section
//! builder.
//!
//! Each specialization is two functions: its body, and the entry that
//! calls it. Every body comes first, in the order of (module index,
//! row), then every entry in the same order. An entry's offset is what
//! [`cig_native::Entry::offset`] names.

use std::collections::BTreeMap;

use cranelift_codegen::control::ControlPlane;
use cranelift_codegen::ir::{
    types, AbiParam, ExtFuncData, ExternalName, Function, InstBuilder, MemFlagsData, Signature,
    UserExternalName, UserFuncName,
};
use cranelift_codegen::isa::TargetIsa;
use cranelift_codegen::{Context, RelocTarget};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};

use cig_native::{Arch, Entry, Section, Ty as NTy};
use cigscript::lower::cir::ir::FuncId;

use crate::lower::LowerOutput;
use crate::qualify::Scalar;
use crate::{Error, SpecKey};

/// Assemble the section from already-compiled bodies.
///
/// - `index` maps CIR `FuncId` to the bytecode module's function
///   index (entries carry this).
/// - `module` is the module's BLAKE3.
/// - `isa` is the target's ISA.
/// - `compiled` is the per-spec lowering output, in the order
///   specializations should appear.
pub fn assemble(
    index: &BTreeMap<FuncId, u32>,
    module: [u8; 32],
    arch: Arch,
    isa: &dyn TargetIsa,
    mut compiled: Vec<(SpecKey, LowerOutput)>,
) -> Result<(Section, Vec<SpecKey>), Error> {
    // By (module index, row): the order the section's entry table has.
    compiled.sort_by(|a, b| {
        let ai = index.get(&a.0 .0).copied().unwrap_or(u32::MAX);
        let bi = index.get(&b.0 .0).copied().unwrap_or(u32::MAX);
        (ai, &a.0 .1).cmp(&(bi, &b.0 .1))
    });

    // Compile each entry wrapper.
    let mut entries_lowered = Vec::with_capacity(compiled.len());
    for (key, out) in &compiled {
        let callee_sig = out.func.signature.clone();
        let entry = compile_entry_wrapper(isa, key, &callee_sig)?;
        entries_lowered.push(entry);
    }

    // Build the text section. Two labels per spec: body first, entry
    // second. So label index 2*i is the body, 2*i+1 is the entry.
    let align = match arch {
        Arch::X86_64 => 1u32,
        Arch::Aarch64 => 4u32,
    };
    let num_labels = compiled.len() * 2;
    let mut tsb = isa.text_section_builder(num_labels);
    let mut cp = ControlPlane::default();

    let mut body_offsets: Vec<u64> = Vec::with_capacity(compiled.len());
    let mut entry_offsets: Vec<u64> = Vec::with_capacity(compiled.len());

    // Append bodies first (labeled true, in the order they were given),
    // then entries. The relocations to other bodies' labels can be
    // resolved after all appends.
    for (_, out) in &compiled {
        let off = tsb.append(true, &out.code, align, &mut cp);
        body_offsets.push(off);
    }
    for entry in &entries_lowered {
        let off = tsb.append(true, &entry.code, align, &mut cp);
        entry_offsets.push(off);
    }

    // Resolve relocations: body calls to other bodies. The label index
    // for body i is i; for entry i is `compiled.len() + i`.
    // Each body knows its calls' `offset` relative to its own bytes:
    // add the body's placement to get the section-relative offset.
    let mut key_to_body_label: BTreeMap<SpecKey, usize> = BTreeMap::new();
    for (i, (key, _)) in compiled.iter().enumerate() {
        key_to_body_label.insert(key.clone(), i);
    }
    for (i, (_, out)) in compiled.iter().enumerate() {
        for call in &out.calls {
            let target = *key_to_body_label
                .get(&call.callee)
                .ok_or_else(|| Error::StrayReloc {
                    at: compiled[i].0.clone(),
                    what: format!("callee {:?} not in section", call.callee),
                })?;
            let section_offset = body_offsets[i] + call.offset as u64;
            let ok = tsb.resolve_reloc(section_offset, call.kind, call.addend, target);
            if !ok {
                return Err(Error::StrayReloc {
                    at: compiled[i].0.clone(),
                    what: format!("builder refused reloc {:?}", call.kind),
                });
            }
        }
    }
    // Entries call bodies too: resolve those relocations.
    for (i, entry) in entries_lowered.iter().enumerate() {
        for call in &entry.calls {
            let target = *key_to_body_label
                .get(&call.callee)
                .ok_or_else(|| Error::StrayReloc {
                    at: compiled[i].0.clone(),
                    what: format!("entry callee {:?} not in section", call.callee),
                })?;
            let section_offset = entry_offsets[i] + call.offset as u64;
            let ok = tsb.resolve_reloc(section_offset, call.kind, call.addend, target);
            if !ok {
                return Err(Error::StrayReloc {
                    at: compiled[i].0.clone(),
                    what: format!("builder refused entry reloc {:?}", call.kind),
                });
            }
        }
    }

    let code = tsb.finish(&mut cp);
    // Trim trailing zero-padding Cranelift added for alignment, if any
    // — the section format requires `code.len()` to be a multiple of
    // the architecture's instruction alignment, which Cranelift has
    // already ensured (`tsb.append` with the same align).

    // Build entries table.
    let mut section_entries = Vec::with_capacity(compiled.len());
    for (i, (key, out)) in compiled.iter().enumerate() {
        let func_idx = *index.get(&key.0).ok_or_else(|| {
            Error::Format(cig_native::FormatError::EntryOutsideCode {
                at: i,
                offset: 0,
                code_len: 0,
            })
        })?;
        // The row's types and the result's, as the language has them:
        // the VM tests arguments against the one and makes the result's
        // value by the other.
        let args: Vec<NTy> = key
            .1
             .0
            .iter()
            .map(|t| Scalar::from_ty(*t).map(Scalar::as_native))
            .collect::<Option<_>>()
            .ok_or_else(|| Error::Codegen {
                at: key.clone(),
                message: "a row type is not scalar".into(),
            })?;
        let ret_ty = out.ret.as_native();
        // The entry's offset in the code blob: section's entry
        // offset table points at the entry wrapper (not the body).
        let offset = u32::try_from(entry_offsets[i]).map_err(|_| {
            Error::Format(cig_native::FormatError::TooMany {
                what: "bytes of code",
                count: entry_offsets[i] as usize,
            })
        })?;
        section_entries.push(Entry {
            func: func_idx,
            offset,
            args,
            ret: ret_ty,
        });
    }
    // Entries must be sorted by function, then by argument types; the
    // sort we did on `compiled` ensures that if the index is
    // monotonic in func_id. We re-sort defensively.
    section_entries.sort_by(|a, b| (a.func, &a.args).cmp(&(b.func, &b.args)));

    // The sites of every function, one after another: each function
    // was compiled with the count of those before it (`site_base`), so
    // the index its code leaves with is its site's place in this table.
    let mut all_sites = Vec::new();
    for (_, out) in &compiled {
        all_sites.extend(out.sites.iter().cloned());
    }

    let section = Section {
        arch,
        module,
        code,
        entries: section_entries,
        sites: all_sites,
    };
    // Check the section is well-formed.
    let _ = section.to_bytes().map_err(Error::Format)?;

    let keys: Vec<SpecKey> = compiled.into_iter().map(|(k, _)| k).collect();
    Ok((section, keys))
}

/// The entry wrapper for one body, compiled as a Cranelift function.
///
/// Signature: `extern "C" fn(ctx: *mut Context, args: *const u64, ret: *mut u64) -> u32`.
pub struct EntryLowered {
    pub code: Vec<u8>,
    pub calls: Vec<crate::lower::CallReloc>,
}

fn compile_entry_wrapper(
    isa: &dyn TargetIsa,
    callee_key: &SpecKey,
    callee_sig: &Signature,
) -> Result<EntryLowered, Error> {
    let call_conv = isa.default_call_conv();
    let pointer_ty = isa.pointer_type();

    let mut sig = Signature::new(call_conv);
    sig.params.push(AbiParam::new(pointer_ty));
    sig.params.push(AbiParam::new(pointer_ty));
    sig.params.push(AbiParam::new(pointer_ty));
    sig.returns.push(AbiParam::new(types::I32));

    let mut func = Function::with_name_signature(UserFuncName::user(0, 0), sig);
    let mut ctx_fb = FunctionBuilderContext::new();
    let mut b = FunctionBuilder::new(&mut func, &mut ctx_fb);

    let entry = b.create_block();
    b.append_block_params_for_function_params(entry);
    b.switch_to_block(entry);
    let params = b.block_params(entry).to_vec();
    let ctx = params[0];
    let args_ptr = params[1];
    let ret_ptr = params[2];

    let n_args = callee_sig.params.len().saturating_sub(1);
    let mut call_args: Vec<cranelift_codegen::ir::Value> = Vec::with_capacity(n_args + 1);
    call_args.push(ctx);
    for i in 0..n_args {
        let ty = callee_sig.params[i + 1].value_type;
        let load_ty = if ty == types::F64 { types::I64 } else { ty };
        let raw = b
            .ins()
            .load(load_ty, MemFlagsData::trusted(), args_ptr, (i as i32) * 8);
        let v = if ty == types::F64 {
            b.ins().bitcast(types::F64, MemFlagsData::new(), raw)
        } else {
            raw
        };
        call_args.push(v);
    }

    // Import the callee.
    let sig_ref = b.import_signature(callee_sig.clone());
    let name = UserExternalName {
        namespace: 0,
        index: 0,
    };
    let name_ref = b.func.params.ensure_user_func_name(name);
    let func_ref = b.import_function(ExtFuncData {
        name: ExternalName::User(name_ref),
        signature: sig_ref,
        colocated: true,
        patchable: false,
    });
    let call = b.ins().call(func_ref, &call_args);
    let results = b.inst_results(call);
    let status = results[0];
    let retval = results[1];

    // On status 0, store retval into *ret.
    let ok = b.create_block();
    let done = b.create_block();
    let is_zero = b
        .ins()
        .icmp_imm_s(cranelift_codegen::ir::condcodes::IntCC::Equal, status, 0);
    b.ins().brif(is_zero, ok, &[], done, &[]);

    b.switch_to_block(ok);
    // retval is already an i64 slot: store whether the callee's return
    // was float or int.
    let slot: cranelift_codegen::ir::Value = {
        let rt = callee_sig.returns.get(1).map(|p| p.value_type);
        if rt == Some(types::F64) {
            b.ins().bitcast(types::I64, MemFlagsData::new(), retval)
        } else {
            retval
        }
    };
    b.ins().store(MemFlagsData::trusted(), slot, ret_ptr, 0);
    b.ins().jump(done, &[]);

    b.switch_to_block(done);
    b.ins().return_(&[status]);
    b.seal_all_blocks();
    b.finalize(isa.frontend_config());

    // Compile.
    let mut ctx = Context::for_function(func);
    let mut cp = ControlPlane::default();
    ctx.compile(isa, &mut cp).map_err(|e| Error::Codegen {
        at: callee_key.clone(),
        message: format!("entry wrapper: {e:?}"),
    })?;
    let compiled = ctx.take_compiled_code().ok_or_else(|| Error::Codegen {
        at: callee_key.clone(),
        message: "no compiled code".into(),
    })?;
    let buffer = &compiled.buffer;
    let code = buffer.data().to_vec();
    let mut calls = Vec::new();
    for r in buffer.relocs() {
        match &r.target {
            RelocTarget::ExternalName(ExternalName::User(_)) => {
                calls.push(crate::lower::CallReloc {
                    offset: r.offset,
                    callee: callee_key.clone(),
                    addend: r.addend,
                    kind: r.kind,
                });
            }
            other => {
                return Err(Error::StrayReloc {
                    at: callee_key.clone(),
                    what: format!("{other:?}"),
                });
            }
        }
    }
    if !buffer.traps().is_empty() {
        return Err(Error::Trap {
            at: callee_key.clone(),
            what: format!("{:?}", buffer.traps()),
        });
    }
    let _ = pointer_ty;
    Ok(EntryLowered { code, calls })
}
