// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2

//! CIR specialization in, Cranelift IR and machine code out.
//!
//! The body of one specialization is one Cranelift function:
//!
//! - parameters `(ctx: pointer, one per argument)`, results
//!   `(status: i32, value: i64)`.
//! - status 0 means it returned `value`. Any non-zero status means it
//!   left: the context holds the site, the operands and `steps_left`.
//!
//! The entry wrapper, same signature on every target, is in
//! [`crate::link`].
//!
//! ## SSA
//!
//! - **Arguments and locals** are Cranelift `Variable`s, `def_var` /
//!   `use_var`.
//! - **`steps_left`** is one `Variable`; it is written to the context
//!   before every call, before every exit and before every return, and
//!   read back after a call that returned status 0.
//! - **Instruction results** live in an index `ValueId → ir::Value`.
//!
//! ## Exits
//!
//! An exit writes the context (`site`, `left`, `right`, `steps_left`)
//! and returns a non-zero status. In-loop flag bit is set on error
//! propagation from a nested call.

use std::collections::{BTreeMap, BTreeSet};

use cranelift_codegen::binemit::Reloc;
use cranelift_codegen::control::ControlPlane;
use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{
    types, AbiParam, Block as CbBlock, ExtFuncData, ExternalName, Function, InstBuilder,
    MemFlagsData, Signature, TrapCode, Type as CbType, UserExternalName, UserFuncName,
    Value as CbValue,
};
use cranelift_codegen::isa::TargetIsa;
use cranelift_codegen::{Context, RelocTarget};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};

use cig_native::{op as nop, Exit, Site, Ty as NTy};
use cigscript::lower::cir::infer::{Inference, Spec};
use cigscript::lower::cir::ir::{
    Block, Const, FuncId, InstrId, Kind, Module, ShortOp, Ty, ValueId, VarId,
};
use cigscript::syntax::ast::{BinOp, UnOp};
use cigscript::syntax::span::Span;

use crate::qualify::{counted_ranges, Scalar};
use crate::SpecKey;

// Where the context's fields lie: the layout `cig-native` gives, which
// the runtime that makes a context uses too.
const CTX_STEPS_LEFT: i32 = core::mem::offset_of!(cig_native::Context, steps_left) as i32;
const CTX_DEPTH_LEFT: i32 = core::mem::offset_of!(cig_native::Context, depth_left) as i32;
const CTX_SITE: i32 = core::mem::offset_of!(cig_native::Context, site) as i32;
const CTX_FLAGS: i32 = core::mem::offset_of!(cig_native::Context, flags) as i32;
const CTX_LEFT: i32 = core::mem::offset_of!(cig_native::Context, left) as i32;
const CTX_RIGHT: i32 = core::mem::offset_of!(cig_native::Context, right) as i32;

/// What [`body`] gives back.
pub struct LowerOutput {
    /// The compiled function's machine code.
    pub code: Vec<u8>,
    /// Calls to other specs: linking relocates them.
    pub calls: Vec<CallReloc>,
    /// Relocations that are not calls to other specs: any means a
    /// defect.
    pub stray_relocs: Vec<String>,
    /// Trap records Cranelift left in the code (should be empty).
    pub traps: Vec<String>,
    /// Sites met in source order.
    pub sites: Vec<Site>,
    /// The stack one call of it uses: its frame, with the saved frame
    /// pointer and the return address.
    pub frame_bytes: usize,
    /// The type of what it returns: its entry's result type.
    pub ret: Scalar,
    /// The finalized Cranelift function (kept for text-section
    /// linking).
    pub func: Function,
}

/// One call relocation this body emits.
#[derive(Clone, Debug)]
pub struct CallReloc {
    pub offset: u32,
    pub callee: SpecKey,
    pub addend: i64,
    pub kind: Reloc,
}

/// Lower one specialization's body. `site_base` is the number of
/// sites emitted by earlier specs in the section: each new site this
/// function records is written into the code as `site_base + local
/// index`, so the generated code reaches the right row of the
/// section's site table.
pub fn body(
    m: &Module,
    inf: &Inference,
    set: &BTreeSet<SpecKey>,
    spec: &Spec,
    isa: &dyn TargetIsa,
    site_base: u32,
) -> Result<LowerOutput, String> {
    let call_conv = isa.default_call_conv();
    let pointer_ty = isa.pointer_type();
    let f = &m.funcs[spec.func.0 as usize];

    let mut sig = Signature::new(call_conv);
    sig.params.push(AbiParam::new(pointer_ty));
    for t in &spec.row.0 {
        sig.params.push(AbiParam::new(scalar_ty(*t)));
    }
    sig.returns.push(AbiParam::new(types::I32));
    sig.returns.push(AbiParam::new(types::I64));

    let mut func = Function::with_name_signature(UserFuncName::user(0, 0), sig);
    let mut ctx_fb = FunctionBuilderContext::new();
    let mut b = FunctionBuilder::new(&mut func, &mut ctx_fb);

    let mut lower = Lower {
        m,
        inf,
        set,
        spec,
        counted: counted_ranges(m, spec),
        pointer_ty,
        values: BTreeMap::new(),
        vars: BTreeMap::new(),
        steps_var: Variable::from_u32(0),
        ctx_var: Variable::from_u32(0),
        loop_stack: Vec::new(),
        in_loop: 0,
        sites: Vec::new(),
        site_dedup: BTreeMap::new(),
        filled: false,
        user_name_to_spec: BTreeMap::new(),
        next_user_index: 0,
        site_base,
    };
    lower.emit(&mut b, f)?;
    b.seal_all_blocks();
    let frontend_config = isa.frontend_config();
    b.finalize(frontend_config);

    let mut ctx = Context::for_function(func);
    let mut cp = ControlPlane::default();
    ctx.compile(isa, &mut cp).map_err(|e| format!("{e:?}"))?;
    // Take the compiled code so we can access `ctx.func.params` while
    // reading the relocations.
    let compiled = ctx
        .take_compiled_code()
        .ok_or("Cranelift did not leave a compiled code")?;
    let buffer = &compiled.buffer;
    let code = buffer.data().to_vec();

    let mut calls = Vec::new();
    let mut stray = Vec::new();
    for r in buffer.relocs() {
        match &r.target {
            RelocTarget::ExternalName(ExternalName::User(user_ref)) => {
                let user = ctx
                    .func
                    .params
                    .user_named_funcs()
                    .get(*user_ref)
                    .ok_or_else(|| {
                        format!("reloc with unknown UserExternalNameRef {user_ref:?}")
                    })?;
                let key = lower
                    .user_name_to_spec
                    .get(&(user.namespace, user.index))
                    .cloned()
                    .ok_or_else(|| format!("no spec for name {user:?}"))?;
                calls.push(CallReloc {
                    offset: r.offset,
                    callee: key,
                    addend: r.addend,
                    kind: r.kind,
                });
            }
            other => stray.push(format!("{other:?}")),
        }
    }
    // The one trap Cranelift may record is the division inside a `srem`,
    // which no run reaches with a zero divisor: `%` tests its divisor
    // first and leaves. Any other record is an instruction that can
    // fault, which this generator does not emit.
    let traps: Vec<String> = buffer
        .traps()
        .iter()
        .filter(|t| t.code != TrapCode::INTEGER_DIVISION_BY_ZERO)
        .map(|t| format!("{t:?}"))
        .collect();

    // The stack one call uses. `frame_to_fp_offset` is the distance from
    // the stack pointer up to the frame pointer while the frame is
    // active: the clobbered callee-saved registers, the spill and stack
    // slots and the outgoing arguments (`sp_to_fp_offset` in Cranelift's
    // `machinst/abi.rs`). Above the frame pointer lie the saved frame
    // pointer and the return address, one pointer each on both targets
    // (pushed by `call` and the prologue on x86-64, stored as a pair by
    // the prologue on aarch64).
    let ptr = isa.pointer_bytes() as usize;
    let frame_bytes = buffer
        .frame_layout()
        .ok_or("Cranelift gave no frame layout")?
        .frame_to_fp_offset as usize
        + 2 * ptr;
    drop(compiled);

    Ok(LowerOutput {
        code,
        calls,
        stray_relocs: stray,
        traps,
        sites: lower.sites.clone(),
        frame_bytes,
        ret: Scalar::from_ty(spec.ret).ok_or("the result's type is not scalar")?,
        func: ctx.func,
    })
}

fn scalar_ty(t: Ty) -> CbType {
    match t {
        Ty::Float => types::F64,
        _ => types::I64,
    }
}

struct Lower<'a> {
    m: &'a Module,
    inf: &'a Inference,
    set: &'a BTreeSet<SpecKey>,
    spec: &'a Spec,
    counted: BTreeSet<InstrId>,
    pointer_ty: CbType,
    values: BTreeMap<ValueId, CbValue>,
    vars: BTreeMap<VarId, Variable>,
    steps_var: Variable,
    ctx_var: Variable,
    loop_stack: Vec<LoopFrame>,
    in_loop: u32,
    sites: Vec<Site>,
    site_dedup: BTreeMap<SiteKey, u32>,
    /// `true` when the current block is terminated. Tracks what
    /// `FunctionBuilder::is_filled` would say.
    filled: bool,
    user_name_to_spec: BTreeMap<(u32, u32), SpecKey>,
    next_user_index: u32,
    /// Section-global index of this spec's first site.
    site_base: u32,
}

#[derive(Clone, Copy)]
struct LoopFrame {
    cont: CbBlock,
    brk: CbBlock,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct SiteKey {
    start: u32,
    end: u32,
    line: u32,
    col: u32,
    kind: u8,
    op: u8,
    left: u8,
    right: u8,
    in_loop: bool,
}

impl<'a> Lower<'a> {
    fn switch_to(&mut self, b: &mut FunctionBuilder, block: CbBlock) {
        b.switch_to_block(block);
        self.filled = false;
    }

    fn terminate(&mut self) {
        self.filled = true;
    }

    fn emit(
        &mut self,
        b: &mut FunctionBuilder,
        f: &cigscript::lower::cir::ir::Func,
    ) -> Result<(), String> {
        let entry = b.create_block();
        b.append_block_params_for_function_params(entry);
        self.switch_to(b, entry);
        let params: Vec<CbValue> = b.block_params(entry).to_vec();
        self.ctx_var = b.declare_var(self.pointer_ty);
        b.def_var(self.ctx_var, params[0]);
        self.steps_var = b.declare_var(types::I64);
        let steps0 = b.ins().load(
            types::I64,
            MemFlagsData::trusted(),
            params[0],
            CTX_STEPS_LEFT,
        );
        b.def_var(self.steps_var, steps0);
        // Parameters -> slot variables.
        for (i, pid) in f.params.iter().enumerate() {
            let ty = self.spec.row.0[i];
            let sc = Scalar::from_ty(ty).ok_or("parameter type is not scalar")?;
            let var = b.declare_var(match sc {
                Scalar::Float => types::F64,
                _ => types::I64,
            });
            self.vars.insert(*pid, var);
            b.def_var(var, params[i + 1]);
        }
        // Locals: declare every slot the function owns and
        // initialize to zero so the SSA builder sees a definition on
        // every path.
        for (vid, t) in &self.spec.slots {
            if self.vars.contains_key(vid) {
                continue;
            }
            let sc = match Scalar::from_ty(*t) {
                Some(s) => s,
                None => continue,
            };
            let ty = match sc {
                Scalar::Float => types::F64,
                _ => types::I64,
            };
            let var = b.declare_var(ty);
            self.vars.insert(*vid, var);
            let zero = match sc {
                Scalar::Float => b.ins().f64const(0.0),
                _ => b.ins().iconst(types::I64, 0),
            };
            b.def_var(var, zero);
        }

        self.lower_block(b, &f.body)?;
        if !self.filled {
            // Fall-through: return null status 0. The CIR verifier
            // rejects a function whose body falls off the end, so this
            // is defensive.
            self.flush_steps(b);
            let s0 = b.ins().iconst(types::I32, 0);
            let z = b.ins().iconst(types::I64, 0);
            b.ins().return_(&[s0, z]);
            self.terminate();
        }
        Ok(())
    }

    fn lower_block(&mut self, b: &mut FunctionBuilder, block: &Block) -> Result<(), String> {
        for ins in &block.instrs {
            if self.filled {
                return Ok(());
            }
            self.lower_instr(b, ins)?;
        }
        Ok(())
    }

    fn lower_instr(
        &mut self,
        b: &mut FunctionBuilder,
        ins: &cigscript::lower::cir::ir::Instr,
    ) -> Result<(), String> {
        match &ins.kind {
            Kind::Step(_) => self.emit_step(b, ins.span),
            Kind::Const(c) => self.emit_const(b, ins, c),
            Kind::Load(v) => {
                let var = *self.vars.get(v).ok_or("load of an unknown slot")?;
                let val = b.use_var(var);
                self.values
                    .insert(ins.result.ok_or("load has no result")?, val);
                Ok(())
            }
            Kind::Decl(v, x) | Kind::Store(v, x) => {
                let var = *self.vars.get(v).ok_or("store to an unknown slot")?;
                let val = *self.values.get(x).ok_or("store of an unknown value")?;
                b.def_var(var, val);
                Ok(())
            }
            Kind::Unary(op, x) => self.emit_unary(b, ins, *op, *x),
            Kind::Binary(op, x, y) => self.emit_binary(b, ins, *op, *x, *y),
            Kind::Short { op, subject, rhs } => self.emit_short(b, ins, *op, *subject, rhs),
            Kind::If {
                cond,
                then,
                otherwise,
            } => self.emit_if(b, *cond, then, otherwise.as_ref()),
            Kind::Loop { cond, body } => self.emit_loop(b, cond, body),
            Kind::ForEach { iter, var, body } => self.emit_foreach(b, ins, *iter, *var, body),
            Kind::Break => self.emit_break(b),
            Kind::Continue => self.emit_continue(b),
            // What a `loop.cond` or a `short.rhs` block gives: the
            // instruction that owns the block reads it (`last_yield`).
            Kind::Yield(v) => self
                .values
                .get(v)
                .map(|_| ())
                .ok_or_else(|| "yield of an unknown value".to_string()),
            Kind::Ret(v) => {
                let val = *self.values.get(v).ok_or("ret of an unknown value")?;
                self.flush_steps(b);
                let zero = b.ins().iconst(types::I32, 0);
                let slot = self.as_slot(b, val);
                b.ins().return_(&[zero, slot]);
                self.terminate();
                Ok(())
            }
            Kind::CallUser { func, args } => self.emit_call(b, ins, *func, args),
            other => Err(format!(
                "instruction not compilable: {:?}",
                std::mem::discriminant(other)
            )),
        }
    }

    fn emit_const(
        &mut self,
        b: &mut FunctionBuilder,
        ins: &cigscript::lower::cir::ir::Instr,
        c: &Const,
    ) -> Result<(), String> {
        let v = match c {
            Const::Null => b.ins().iconst(types::I64, 0),
            Const::Bool(x) => b.ins().iconst(types::I64, i64::from(*x)),
            Const::Int(i) => b.ins().iconst(types::I64, *i),
            Const::Float(f) => b.ins().f64const(*f),
            Const::Str(_) => return Err("string constant is not compilable".into()),
        };
        if let Some(res) = ins.result {
            self.values.insert(res, v);
        }
        Ok(())
    }

    fn emit_step(&mut self, b: &mut FunctionBuilder, span: Span) -> Result<(), String> {
        let s = b.use_var(self.steps_var);
        let s1 = b.ins().iadd_imm_s(s, -1);
        b.def_var(self.steps_var, s1);
        let neg = b.ins().icmp_imm_s(IntCC::SignedLessThan, s1, 0);
        let exit = b.create_block();
        let cont = b.create_block();
        b.ins().brif(neg, exit, &[], cont, &[]);
        self.terminate();
        self.switch_to(b, exit);
        let site = self.site(span, Exit::Budget, 0, NTy::Null, NTy::Null);
        self.exit(b, Exit::Budget, site, None, None);
        self.terminate();
        self.switch_to(b, cont);
        Ok(())
    }

    fn emit_unary(
        &mut self,
        b: &mut FunctionBuilder,
        ins: &cigscript::lower::cir::ir::Instr,
        op: UnOp,
        x: ValueId,
    ) -> Result<(), String> {
        let xs = self.scalar_of_val(x)?;
        let xv = *self.values.get(&x).ok_or("unary of an unknown value")?;
        let out = match (op, xs) {
            (UnOp::Not, Scalar::Bool) => b.ins().bxor_imm_s(xv, 1),
            (UnOp::Not, Scalar::Null) => b.ins().iconst(types::I64, 1),
            (UnOp::Not, Scalar::Int) => {
                let z = b.ins().icmp_imm_s(IntCC::Equal, xv, 0);
                b.ins().uextend(types::I64, z)
            }
            (UnOp::Not, Scalar::Float) => {
                let zero = b.ins().f64const(0.0);
                let z = b.ins().fcmp(FloatCC::Equal, xv, zero);
                b.ins().uextend(types::I64, z)
            }
            (UnOp::Neg, Scalar::Int) => {
                let min = b.ins().iconst(types::I64, i64::MIN);
                let over = b.ins().icmp(IntCC::Equal, xv, min);
                let exit = b.create_block();
                let cont = b.create_block();
                b.ins().brif(over, exit, &[], cont, &[]);
                self.terminate();
                self.switch_to(b, exit);
                let site = self.site(ins.span, Exit::Negate, 0, NTy::Int, NTy::Null);
                self.exit(b, Exit::Negate, site, Some(xv), None);
                self.terminate();
                self.switch_to(b, cont);
                b.ins().ineg(xv)
            }
            (UnOp::Neg, Scalar::Float) => b.ins().fneg(xv),
            _ => return Err(format!("unary {op:?} on {xs:?}")),
        };
        if let Some(res) = ins.result {
            self.values.insert(res, out);
        }
        Ok(())
    }

    fn emit_binary(
        &mut self,
        b: &mut FunctionBuilder,
        ins: &cigscript::lower::cir::ir::Instr,
        op: BinOp,
        x: ValueId,
        y: ValueId,
    ) -> Result<(), String> {
        if matches!(op, BinOp::Range) {
            if !self.counted.contains(&ins.id) {
                return Err("a range that is not a counted for".into());
            }
            // The range-fits check mirrors `range_fits`:
            //   width = b as i128 - a as i128; width > 10_000_000 → Range exit.
            // In 64-bit we test with the two branches `b >= a` then
            // unsigned compare of `b - a`.
            let xv = *self.values.get(&x).ok_or("range a")?;
            let yv = *self.values.get(&y).ok_or("range b")?;
            let b_ge_a = b.ins().icmp(IntCC::SignedGreaterThanOrEqual, yv, xv);
            let do_check = b.create_block();
            let skip = b.create_block();
            b.ins().brif(b_ge_a, do_check, &[], skip, &[]);
            self.terminate();
            self.switch_to(b, do_check);
            let width = b.ins().isub(yv, xv);
            let too_wide = b
                .ins()
                .icmp_imm_s(IntCC::UnsignedGreaterThan, width, 10_000_000);
            let exit = b.create_block();
            b.ins().brif(too_wide, exit, &[], skip, &[]);
            self.terminate();
            self.switch_to(b, exit);
            let site = self.site(ins.span, Exit::Range, 0, NTy::Int, NTy::Int);
            self.exit(b, Exit::Range, site, Some(xv), Some(yv));
            self.terminate();
            self.switch_to(b, skip);
            return Ok(());
        }
        let xs = self.scalar_of_val(x)?;
        let ys = self.scalar_of_val(y)?;
        let xv = *self.values.get(&x).ok_or("binary a")?;
        let yv = *self.values.get(&y).ok_or("binary b")?;
        let out = match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul => match (xs, ys) {
                (Scalar::Int, Scalar::Int) => self.emit_int_overflow_op(b, ins.span, op, xv, yv)?,
                _ => {
                    let (xf, yf) = self.pair_to_floats(b, xs, xv, ys, yv);
                    match op {
                        BinOp::Add => b.ins().fadd(xf, yf),
                        BinOp::Sub => b.ins().fsub(xf, yf),
                        BinOp::Mul => b.ins().fmul(xf, yf),
                        _ => unreachable!(),
                    }
                }
            },
            BinOp::Rem => match (xs, ys) {
                (Scalar::Int, Scalar::Int) => {
                    let is_zero = b.ins().icmp_imm_s(IntCC::Equal, yv, 0);
                    let exit = b.create_block();
                    let cont = b.create_block();
                    b.ins().brif(is_zero, exit, &[], cont, &[]);
                    self.terminate();
                    self.switch_to(b, exit);
                    let site = self.site(ins.span, Exit::Operator, nop::REM, NTy::Int, NTy::Int);
                    self.exit(b, Exit::Operator, site, Some(xv), Some(yv));
                    self.terminate();
                    self.switch_to(b, cont);
                    // Divisor -1 gives 0 without dividing; otherwise `srem`.
                    let is_neg1 = b.ins().icmp_imm_s(IntCC::Equal, yv, -1);
                    let use_zero = b.create_block();
                    let do_rem = b.create_block();
                    let join = b.create_block();
                    b.append_block_param(join, types::I64);
                    b.ins().brif(is_neg1, use_zero, &[], do_rem, &[]);
                    self.terminate();
                    self.switch_to(b, use_zero);
                    let z = b.ins().iconst(types::I64, 0);
                    b.ins().jump(join, &[z.into()]);
                    self.terminate();
                    self.switch_to(b, do_rem);
                    let r = b.ins().srem(xv, yv);
                    b.ins().jump(join, &[r.into()]);
                    self.terminate();
                    self.switch_to(b, join);
                    b.block_params(join)[0]
                }
                _ => return Err("`%` with a float is not compiled".into()),
            },
            BinOp::Div => {
                let (xf, yf) = self.pair_to_floats(b, xs, xv, ys, yv);
                let zero = b.ins().f64const(0.0);
                let is_zero = b.ins().fcmp(FloatCC::Equal, yf, zero);
                let exit = b.create_block();
                let cont = b.create_block();
                b.ins().brif(is_zero, exit, &[], cont, &[]);
                self.terminate();
                self.switch_to(b, exit);
                let site = self.site(
                    ins.span,
                    Exit::Operator,
                    nop::DIV,
                    xs.as_native(),
                    ys.as_native(),
                );
                self.exit(b, Exit::Operator, site, Some(xv), Some(yv));
                self.terminate();
                self.switch_to(b, cont);
                b.ins().fdiv(xf, yf)
            }
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => match (xs, ys) {
                (Scalar::Int, Scalar::Int) => {
                    let cc = match op {
                        BinOp::Lt => IntCC::SignedLessThan,
                        BinOp::Le => IntCC::SignedLessThanOrEqual,
                        BinOp::Gt => IntCC::SignedGreaterThan,
                        _ => IntCC::SignedGreaterThanOrEqual,
                    };
                    let z = b.ins().icmp(cc, xv, yv);
                    b.ins().uextend(types::I64, z)
                }
                _ => {
                    let (xf, yf) = self.pair_to_floats(b, xs, xv, ys, yv);
                    let un = b.ins().fcmp(FloatCC::Unordered, xf, yf);
                    let exit = b.create_block();
                    let cont = b.create_block();
                    b.ins().brif(un, exit, &[], cont, &[]);
                    self.terminate();
                    self.switch_to(b, exit);
                    let op_code = match op {
                        BinOp::Lt => nop::LT,
                        BinOp::Le => nop::LE,
                        BinOp::Gt => nop::GT,
                        _ => nop::GE,
                    };
                    let site = self.site(
                        ins.span,
                        Exit::Operator,
                        op_code,
                        xs.as_native(),
                        ys.as_native(),
                    );
                    self.exit(b, Exit::Operator, site, Some(xv), Some(yv));
                    self.terminate();
                    self.switch_to(b, cont);
                    let cc = match op {
                        BinOp::Lt => FloatCC::LessThan,
                        BinOp::Le => FloatCC::LessThanOrEqual,
                        BinOp::Gt => FloatCC::GreaterThan,
                        _ => FloatCC::GreaterThanOrEqual,
                    };
                    let z = b.ins().fcmp(cc, xf, yf);
                    b.ins().uextend(types::I64, z)
                }
            },
            BinOp::Eq | BinOp::Ne => {
                let eq = self.emit_eq(b, xs, xv, ys, yv);
                if matches!(op, BinOp::Ne) {
                    b.ins().bxor_imm_s(eq, 1)
                } else {
                    eq
                }
            }
            BinOp::And | BinOp::Or | BinOp::Coalesce => {
                unreachable!("short-circuit is `Kind::Short`")
            }
            BinOp::Range => unreachable!("counted range handled above"),
        };
        if let Some(res) = ins.result {
            self.values.insert(res, out);
        }
        Ok(())
    }

    fn emit_int_overflow_op(
        &mut self,
        b: &mut FunctionBuilder,
        span: Span,
        op: BinOp,
        xv: CbValue,
        yv: CbValue,
    ) -> Result<CbValue, String> {
        let (res, of) = match op {
            BinOp::Add => b.ins().sadd_overflow(xv, yv),
            BinOp::Sub => b.ins().ssub_overflow(xv, yv),
            BinOp::Mul => b.ins().smul_overflow(xv, yv),
            _ => return Err(format!("not an int overflow op: {op:?}")),
        };
        let exit = b.create_block();
        let cont = b.create_block();
        b.ins().brif(of, exit, &[], cont, &[]);
        self.terminate();
        self.switch_to(b, exit);
        let site = self.site(
            span,
            Exit::Operator,
            match op {
                BinOp::Add => nop::ADD,
                BinOp::Sub => nop::SUB,
                _ => nop::MUL,
            },
            NTy::Int,
            NTy::Int,
        );
        self.exit(b, Exit::Operator, site, Some(xv), Some(yv));
        self.terminate();
        self.switch_to(b, cont);
        Ok(res)
    }

    fn emit_eq(
        &mut self,
        b: &mut FunctionBuilder,
        xs: Scalar,
        xv: CbValue,
        ys: Scalar,
        yv: CbValue,
    ) -> CbValue {
        match (xs, ys) {
            (Scalar::Null, Scalar::Null) => b.ins().iconst(types::I64, 1),
            (Scalar::Null, _) | (_, Scalar::Null) => b.ins().iconst(types::I64, 0),
            (Scalar::Bool, Scalar::Bool) => {
                let z = b.ins().icmp(IntCC::Equal, xv, yv);
                b.ins().uextend(types::I64, z)
            }
            (Scalar::Bool, _) | (_, Scalar::Bool) => b.ins().iconst(types::I64, 0),
            (Scalar::Int, Scalar::Int) => {
                let z = b.ins().icmp(IntCC::Equal, xv, yv);
                b.ins().uextend(types::I64, z)
            }
            _ => {
                let (xf, yf) = self.pair_to_floats(b, xs, xv, ys, yv);
                let z = b.ins().fcmp(FloatCC::Equal, xf, yf);
                b.ins().uextend(types::I64, z)
            }
        }
    }

    fn emit_short(
        &mut self,
        b: &mut FunctionBuilder,
        ins: &cigscript::lower::cir::ir::Instr,
        op: ShortOp,
        subject: ValueId,
        rhs: &Block,
    ) -> Result<(), String> {
        let sub_scalar = self.scalar_of_val(subject)?;
        let sub = *self.values.get(&subject).ok_or("short subject")?;
        let res_id = ins.result.ok_or("short has no result")?;
        let res_scalar =
            Scalar::from_ty(self.spec.vals[res_id.0 as usize]).ok_or("short result not scalar")?;
        let res_ty = match res_scalar {
            Scalar::Float => types::F64,
            _ => types::I64,
        };
        let res_var = b.declare_var(res_ty);

        let rhs_block = b.create_block();
        let done_block = b.create_block();
        let join = b.create_block();
        match op {
            ShortOp::And | ShortOp::Or => {
                let truth = self.truth_of(b, sub_scalar, sub);
                let (then_rhs, then_done) = if matches!(op, ShortOp::And) {
                    (rhs_block, done_block)
                } else {
                    (done_block, rhs_block)
                };
                b.ins().brif(truth, then_rhs, &[], then_done, &[]);
                self.terminate();
            }
            ShortOp::Coalesce => {
                let is_null = match sub_scalar {
                    Scalar::Null => b.ins().iconst(types::I8, 1),
                    _ => b.ins().iconst(types::I8, 0),
                };
                b.ins().brif(is_null, rhs_block, &[], done_block, &[]);
                self.terminate();
            }
            ShortOp::Safe => return Err("`?.` is not compiled".into()),
        }
        // Done (short) path: result is the subject.
        self.switch_to(b, done_block);
        let as_res = self.cast_val(b, sub, res_ty);
        b.def_var(res_var, as_res);
        b.ins().jump(join, &[]);
        self.terminate();
        // Rhs path.
        self.switch_to(b, rhs_block);
        self.lower_block(b, rhs)?;
        let yielded = last_yield(rhs).ok_or("a short-circuit's right side yields nothing")?;
        let yielded = *self
            .values
            .get(&yielded)
            .ok_or("a short-circuit's right side yields an unknown value")?;
        let as_res = self.cast_val(b, yielded, res_ty);
        b.def_var(res_var, as_res);
        b.ins().jump(join, &[]);
        self.terminate();
        // Join.
        self.switch_to(b, join);
        let final_val = b.use_var(res_var);
        self.values.insert(res_id, final_val);
        Ok(())
    }

    fn truth_of(&mut self, b: &mut FunctionBuilder, s: Scalar, v: CbValue) -> CbValue {
        match s {
            Scalar::Null => b.ins().iconst(types::I8, 0),
            Scalar::Bool | Scalar::Int => b.ins().icmp_imm_s(IntCC::NotEqual, v, 0),
            Scalar::Float => {
                let zero = b.ins().f64const(0.0);
                let eq0 = b.ins().fcmp(FloatCC::Equal, v, zero);
                b.ins().bxor_imm_s(eq0, 1)
            }
        }
    }

    fn emit_if(
        &mut self,
        b: &mut FunctionBuilder,
        cond: ValueId,
        then: &Block,
        otherwise: Option<&Block>,
    ) -> Result<(), String> {
        let cs = self.scalar_of_val(cond)?;
        let cv = *self.values.get(&cond).ok_or("if cond")?;
        let truth = self.truth_of(b, cs, cv);
        let then_b = b.create_block();
        let else_b = b.create_block();
        let join = b.create_block();
        b.ins().brif(truth, then_b, &[], else_b, &[]);
        self.terminate();
        self.switch_to(b, then_b);
        self.lower_block(b, then)?;
        if !self.filled {
            b.ins().jump(join, &[]);
            self.terminate();
        }
        self.switch_to(b, else_b);
        if let Some(e) = otherwise {
            self.lower_block(b, e)?;
        }
        if !self.filled {
            b.ins().jump(join, &[]);
            self.terminate();
        }
        self.switch_to(b, join);
        Ok(())
    }

    fn emit_loop(
        &mut self,
        b: &mut FunctionBuilder,
        cond: &Block,
        body: &Block,
    ) -> Result<(), String> {
        let header = b.create_block();
        let body_b = b.create_block();
        let post = b.create_block();
        b.ins().jump(header, &[]);
        self.terminate();
        self.switch_to(b, header);
        self.in_loop += 1;
        self.lower_block(b, cond)?;
        let cond_val_id = last_yield(cond).ok_or("loop condition has no yield")?;
        let cv = *self.values.get(&cond_val_id).ok_or("loop cond value")?;
        let cs = Scalar::from_ty(self.spec.vals[cond_val_id.0 as usize])
            .ok_or("loop cond not scalar")?;
        let truth = self.truth_of(b, cs, cv);
        b.ins().brif(truth, body_b, &[], post, &[]);
        self.terminate();
        self.switch_to(b, body_b);
        self.loop_stack.push(LoopFrame {
            cont: header,
            brk: post,
        });
        self.lower_block(b, body)?;
        if !self.filled {
            b.ins().jump(header, &[]);
            self.terminate();
        }
        self.loop_stack.pop();
        self.in_loop -= 1;
        self.switch_to(b, post);
        Ok(())
    }

    fn emit_foreach(
        &mut self,
        b: &mut FunctionBuilder,
        _ins: &cigscript::lower::cir::ir::Instr,
        iter: ValueId,
        var: VarId,
        body: &Block,
    ) -> Result<(), String> {
        let f = &self.m.funcs[self.spec.func.0 as usize];
        let (a, bexpr) = find_range_bounds(&f.body, iter)
            .ok_or("counted range: cannot find its defining instr")?;
        let av = *self.values.get(&a).ok_or("foreach a")?;
        let bv = *self.values.get(&bexpr).ok_or("foreach b")?;
        let slot = *self.vars.get(&var).ok_or("foreach var")?;
        // The count is the loop's own, not the script's variable: each
        // turn gives the variable the next number afresh, whatever the
        // body stored to it (`run_for_range`).
        let count = b.declare_var(types::I64);
        b.def_var(count, av);
        let header = b.create_block();
        let body_b = b.create_block();
        let latch = b.create_block();
        let post = b.create_block();
        b.ins().jump(header, &[]);
        self.terminate();
        self.switch_to(b, header);
        self.in_loop += 1;
        let i = b.use_var(count);
        let lt = b.ins().icmp(IntCC::SignedLessThan, i, bv);
        b.ins().brif(lt, body_b, &[], post, &[]);
        self.terminate();
        self.switch_to(b, body_b);
        b.def_var(slot, i);
        // `continue` goes on to the next number.
        self.loop_stack.push(LoopFrame {
            cont: latch,
            brk: post,
        });
        self.lower_block(b, body)?;
        if !self.filled {
            b.ins().jump(latch, &[]);
            self.terminate();
        }
        self.loop_stack.pop();
        self.switch_to(b, latch);
        // Below the bound, so below the largest int: it cannot overflow.
        let i = b.use_var(count);
        let next = b.ins().iadd_imm_s(i, 1);
        b.def_var(count, next);
        b.ins().jump(header, &[]);
        self.terminate();
        self.in_loop -= 1;
        self.switch_to(b, post);
        Ok(())
    }

    fn emit_break(&mut self, b: &mut FunctionBuilder) -> Result<(), String> {
        let frame = *self.loop_stack.last().ok_or("break outside a loop")?;
        b.ins().jump(frame.brk, &[]);
        self.terminate();
        Ok(())
    }

    fn emit_continue(&mut self, b: &mut FunctionBuilder) -> Result<(), String> {
        let frame = *self.loop_stack.last().ok_or("continue outside a loop")?;
        b.ins().jump(frame.cont, &[]);
        self.terminate();
        Ok(())
    }

    fn emit_call(
        &mut self,
        b: &mut FunctionBuilder,
        ins: &cigscript::lower::cir::ir::Instr,
        _func: FuncId,
        args: &[ValueId],
    ) -> Result<(), String> {
        let (callee_func, callee_row) = self
            .spec
            .calls
            .get(&ins.id)
            .cloned()
            .ok_or("call with no inferred callee")?;
        let callee_key: SpecKey = (callee_func, callee_row.clone());
        if !self.set.contains(&callee_key) {
            return Err("call of a spec not in the compiled set".into());
        }
        // depth_left <= 0 → Depth exit; else decrement in the context.
        let ctx = b.use_var(self.ctx_var);
        let depth = b
            .ins()
            .load(types::I64, MemFlagsData::trusted(), ctx, CTX_DEPTH_LEFT);
        let too_deep = b.ins().icmp_imm_s(IntCC::SignedLessThanOrEqual, depth, 0);
        let exit = b.create_block();
        let cont = b.create_block();
        b.ins().brif(too_deep, exit, &[], cont, &[]);
        self.terminate();
        self.switch_to(b, exit);
        let site = self.site(ins.span, Exit::Depth, 0, NTy::Null, NTy::Null);
        self.exit(b, Exit::Depth, site, None, None);
        self.terminate();
        self.switch_to(b, cont);
        let d1 = b.ins().iadd_imm_s(depth, -1);
        b.ins()
            .store(MemFlagsData::trusted(), d1, ctx, CTX_DEPTH_LEFT);
        self.flush_steps(b);
        // Signature.
        let callee_spec = self
            .inf
            .spec(callee_func, &callee_row)
            .ok_or("callee not found")?;
        let call_conv = b.func.signature.call_conv;
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(self.pointer_ty));
        for t in &callee_row.0 {
            sig.params.push(AbiParam::new(scalar_ty(*t)));
        }
        sig.returns.push(AbiParam::new(types::I32));
        sig.returns.push(AbiParam::new(types::I64));
        let sig_ref = b.import_signature(sig);
        let index = self.next_user_index;
        self.next_user_index += 1;
        let name = UserExternalName {
            namespace: 0,
            index,
        };
        let name_ref = b.func.params.ensure_user_func_name(name);
        self.user_name_to_spec
            .insert((0, index), callee_key.clone());
        let func_ref = b.import_function(ExtFuncData {
            name: ExternalName::User(name_ref),
            signature: sig_ref,
            colocated: true,
            patchable: false,
        });
        let mut call_args = vec![ctx];
        for (vid, t) in args.iter().zip(&callee_row.0) {
            // The row is the arguments' types: the inference made it
            // from them.
            let vs = self.scalar_of_val(*vid)?;
            let vv = *self.values.get(vid).ok_or("call arg")?;
            let want = Scalar::from_ty(*t).ok_or("call row type not scalar")?;
            if vs != want {
                return Err(format!(
                    "an argument is {vs:?} where the callee's row has {want:?}"
                ));
            }
            call_args.push(vv);
        }
        let call = b.ins().call(func_ref, &call_args);
        let results = b.inst_results(call);
        let status = results[0];
        let retval = results[1];
        let new_steps = b
            .ins()
            .load(types::I64, MemFlagsData::trusted(), ctx, CTX_STEPS_LEFT);
        b.def_var(self.steps_var, new_steps);
        let ok_b = b.create_block();
        let err_b = b.create_block();
        let after = b.create_block();
        b.append_block_param(after, types::I64);
        let is_zero = b.ins().icmp_imm_s(IntCC::Equal, status, 0);
        b.ins().brif(is_zero, ok_b, &[], err_b, &[]);
        self.terminate();
        // OK: depth += 1, pass retval to join.
        self.switch_to(b, ok_b);
        let ctx_ok = b.use_var(self.ctx_var);
        let d = b
            .ins()
            .load(types::I64, MemFlagsData::trusted(), ctx_ok, CTX_DEPTH_LEFT);
        let dp1 = b.ins().iadd_imm_s(d, 1);
        b.ins()
            .store(MemFlagsData::trusted(), dp1, ctx_ok, CTX_DEPTH_LEFT);
        b.ins().jump(after, &[retval.into()]);
        self.terminate();
        // Error: set in-loop bit, return (status, 0).
        self.switch_to(b, err_b);
        self.propagate_error(b, status);
        self.terminate();
        self.switch_to(b, after);
        let typed_i64 = b.block_params(after)[0];
        // If the callee returns a float, bitcast back.
        let callee_ret_ty = scalar_ty(callee_spec.ret);
        let typed = if callee_ret_ty == types::F64 {
            b.ins().bitcast(types::F64, MemFlagsData::new(), typed_i64)
        } else {
            typed_i64
        };
        if let Some(res) = ins.result {
            self.values.insert(res, typed);
        }
        Ok(())
    }

    fn scalar_of_val(&self, v: ValueId) -> Result<Scalar, String> {
        Scalar::from_ty(
            self.spec
                .vals
                .get(v.0 as usize)
                .copied()
                .ok_or("unknown value id")?,
        )
        .ok_or_else(|| format!("value {v:?} is not scalar"))
    }

    fn pair_to_floats(
        &mut self,
        b: &mut FunctionBuilder,
        xs: Scalar,
        xv: CbValue,
        ys: Scalar,
        yv: CbValue,
    ) -> (CbValue, CbValue) {
        let xf = match xs {
            Scalar::Float => xv,
            Scalar::Int => b.ins().fcvt_from_sint(types::F64, xv),
            _ => xv,
        };
        let yf = match ys {
            Scalar::Float => yv,
            Scalar::Int => b.ins().fcvt_from_sint(types::F64, yv),
            _ => yv,
        };
        (xf, yf)
    }

    fn as_slot(&mut self, b: &mut FunctionBuilder, v: CbValue) -> CbValue {
        let ty = b.func.dfg.value_type(v);
        if ty == types::F64 {
            b.ins().bitcast(types::I64, MemFlagsData::new(), v)
        } else {
            v
        }
    }

    fn cast_val(&mut self, b: &mut FunctionBuilder, v: CbValue, want: CbType) -> CbValue {
        let got = b.func.dfg.value_type(v);
        if got == want {
            v
        } else if want == types::F64 && got == types::I64 {
            b.ins().bitcast(types::F64, MemFlagsData::new(), v)
        } else if want == types::I64 && got == types::F64 {
            b.ins().bitcast(types::I64, MemFlagsData::new(), v)
        } else {
            v
        }
    }

    fn flush_steps(&mut self, b: &mut FunctionBuilder) {
        let ctx = b.use_var(self.ctx_var);
        let s = b.use_var(self.steps_var);
        b.ins()
            .store(MemFlagsData::trusted(), s, ctx, CTX_STEPS_LEFT);
    }

    fn site(&mut self, span: Span, kind: Exit, op: u8, left: NTy, right: NTy) -> u32 {
        let key = SiteKey {
            start: span.start as u32,
            end: span.end as u32,
            line: span.line,
            col: span.col,
            kind: kind as u8,
            op,
            left: left as u8,
            right: right as u8,
            in_loop: self.in_loop > 0,
        };
        if let Some(i) = self.site_dedup.get(&key) {
            return *i;
        }
        let i = self.sites.len() as u32;
        self.sites.push(Site {
            start: span.start as u32,
            end: span.end as u32,
            line: span.line,
            col: span.col,
            kind,
            op,
            left,
            right,
            in_loop: self.in_loop > 0,
        });
        self.site_dedup.insert(key, i);
        i
    }

    fn exit(
        &mut self,
        b: &mut FunctionBuilder,
        kind: Exit,
        site: u32,
        left_bits: Option<CbValue>,
        right_bits: Option<CbValue>,
    ) {
        self.flush_steps(b);
        let ctx = b.use_var(self.ctx_var);
        // Site indices in the code are section-global: add the base.
        let s = b.ins().iconst(types::I32, (self.site_base + site) as i64);
        b.ins().store(MemFlagsData::trusted(), s, ctx, CTX_SITE);
        if let Some(l) = left_bits {
            let slot = self.as_slot(b, l);
            b.ins().store(MemFlagsData::trusted(), slot, ctx, CTX_LEFT);
        }
        if let Some(r) = right_bits {
            let slot = self.as_slot(b, r);
            b.ins().store(MemFlagsData::trusted(), slot, ctx, CTX_RIGHT);
        }
        let status = b.ins().iconst(types::I32, kind as u8 as i64);
        let zero = b.ins().iconst(types::I64, 0);
        b.ins().return_(&[status, zero]);
    }

    fn propagate_error(&mut self, b: &mut FunctionBuilder, status: CbValue) {
        if self.in_loop > 0 {
            let ctx = b.use_var(self.ctx_var);
            let f = b
                .ins()
                .load(types::I32, MemFlagsData::trusted(), ctx, CTX_FLAGS);
            let or1 = b.ins().bor_imm_s(f, 1);
            b.ins().store(MemFlagsData::trusted(), or1, ctx, CTX_FLAGS);
        }
        let zero = b.ins().iconst(types::I64, 0);
        b.ins().return_(&[status, zero]);
    }
}

fn last_yield(block: &Block) -> Option<ValueId> {
    for ins in block.instrs.iter().rev() {
        if let Kind::Yield(v) = &ins.kind {
            return Some(*v);
        }
    }
    None
}

fn find_range_bounds(block: &Block, want: ValueId) -> Option<(ValueId, ValueId)> {
    for ins in &block.instrs {
        if ins.result == Some(want) {
            if let Kind::Binary(BinOp::Range, a, b) = ins.kind {
                return Some((a, b));
            }
        }
        for nested in ins.kind.blocks() {
            if let Some(r) = find_range_bounds(nested, want) {
                return Some(r);
            }
        }
    }
    None
}
