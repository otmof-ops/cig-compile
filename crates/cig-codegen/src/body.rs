// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2

//! Bodies from bytecode (decision 0008): a function's native code, run
//! on the frame the VM made for the call.
//!
//! A **body** is entered at one of its function's resume points and
//! returns a stop: "the bytecode goes on at this pc", or what a helper
//! said. It never makes an error. Whatever it has no inline form for,
//! and whatever an inline form does not cover for the operands it
//! meets, the interpreter runs, on the same frame, through the `run`
//! helper reached from the context.
//!
//! ## What an instruction becomes
//!
//! - **Inline:** machine code. Each inline form is the interpreter's
//!   result for the operand types it tests for, and for any other
//!   operand it runs that one instruction through the interpreter
//!   (its *fall back*) before anything is written.
//! - **The interpreter's:** a run of consecutive instructions with no
//!   inline form is one call `run(ctx, from, to)`. When the stop is
//!   `to` the body goes on; when it is the last instruction's own jump
//!   target the body goes there; anything else is returned as it is.
//! - **A leave:** `call`, `tailcall` and `ret` return `At(pc)` at once:
//!   the VM's own instruction makes and ends frames.
//!
//! ## Where a register's value is
//!
//! In the frame, with its tag, unless the type pass ([`crate::types`])
//! proves it is an int, a float or a bool at that instruction: then,
//! in [`Mode::Promoted`], it is **held** in a machine register (a
//! Cranelift variable) and the frame's copy is stale. Three rules keep
//! that exact:
//!
//! 1. **Before anything else reads the frame, every held register is
//!    written to it:** before a call of `run`, and before the body
//!    returns.
//! 2. **A register is read back from the frame where its value comes
//!    from there:** at a resume point, and after the interpreter ran
//!    an instruction that wrote it. Its tag is compared with the type
//!    the pass proved; a mismatch is a defect in the pass, and the
//!    body returns `Defect(pc)`.
//! 3. **A held register's place in the frame never holds a value that
//!    owns memory.** A register starts being held at an inline write,
//!    and an inline write is taken only when the register's old value
//!    owns no memory. So writing a held register back is a plain
//!    store: nothing has to be freed.
//!
//! Where two paths meet that do not agree on a register's type, the
//! pass has it as "anything" there, and each path writes the register
//! to the frame on its way in.
//!
//! ## Determinism
//!
//! Functions are compiled in their module pre-order index, nothing
//! here holds an unordered map, and Cranelift is fed the same IR on
//! every host. The emitted code is a function of the module, the rule,
//! the mode and the target, and of nothing else.

use std::collections::{BTreeMap, BTreeSet};

use cranelift_codegen::control::ControlPlane;
use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{
    types, AbiParam, Block as CbBlock, Function, InstBuilder, JumpTableData, MemFlagsData, SigRef,
    Signature, Type as CbType, UserFuncName, Value as CbValue,
};
use cranelift_codegen::isa::TargetIsa;
use cranelift_codegen::Context;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};

use cig_native::{value, Arch, Body, Rule, Stop};
use cigscript::cig_bytecode::instr::{decode_table, DecodedInstr};
use cigscript::cig_bytecode::module::{Constant, Function as BcFunction, Module as BcModule};
use cigscript::cig_bytecode::opcode::Op;

use crate::flow::{self, Flow};
use crate::isa::target as target_isa;
use crate::plan::{self, Class, How, Plan};
use crate::types::{self as typed, Ty, Types};
use crate::Error;

const CTX_STEPS_LEFT: i32 = core::mem::offset_of!(cig_native::Context, steps_left) as i32;
const CTX_FRAME: i32 = core::mem::offset_of!(cig_native::Context, frame) as i32;
const CTX_HELPERS: i32 = core::mem::offset_of!(cig_native::Context, helpers) as i32;
const CTX_NEST: i32 = core::mem::offset_of!(cig_native::Context, nest) as i32;
const HLP_RUN: i32 = core::mem::offset_of!(cig_native::Helpers, run) as i32;
const HLP_AGAIN: i32 = core::mem::offset_of!(cig_native::Helpers, again) as i32;
const HLP_LIST: i32 = core::mem::offset_of!(cig_native::Helpers, list) as i32;
const HLP_PUSH: i32 = core::mem::offset_of!(cig_native::Helpers, push) as i32;
const HLP_COPY: i32 = core::mem::offset_of!(cig_native::Helpers, copy) as i32;
const HLP_CLEAR: i32 = core::mem::offset_of!(cig_native::Helpers, clear) as i32;
const CTX_LEFT: i32 = core::mem::offset_of!(cig_native::Context, left) as i32;
const CTX_RIGHT: i32 = core::mem::offset_of!(cig_native::Context, right) as i32;

/// The widest counted loop the interpreter runs (`range_fits`): a wider
/// one is the interpreter's to refuse.
const MAX_TURNS: i64 = 10_000_000;

/// How a body keeps its registers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Every register stays in the frame and is read with its tag.
    Frame,
    /// As [`Mode::Frame`], and each instruction first compares the tag
    /// of every register it reads or writes with what the type pass
    /// proved of it, returning `Defect(pc)` on a mismatch. It is how the
    /// pass is held to the VM's real registers.
    Checked,
    /// A register the type pass proves to be an int, a float or a bool
    /// is held in a machine register.
    Promoted,
}

/// One function's outcome: whether it got a body, and how big that body
/// is; or why it did not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyOutcome {
    /// The function's index in the bytecode module (0 is the top level).
    pub func: u32,
    /// `true` when the function has a body in [`Bodies::code`].
    pub compiled: bool,
    /// Why it has no body; empty when it does.
    pub why: String,
    /// The body's length in bytes. 0 when nothing was compiled.
    pub code_bytes: u32,
    /// How many of the function's instructions are machine code in the
    /// body, and how many it has in all.
    pub inline: (u32, u32),
    /// The plan weighed the function (its loops, and for any function
    /// but the top level its straight code) and left all of it to the
    /// bytecode: none would run faster as machine code.
    pub weighed: bool,
}

/// What [`bodies`] gives back: the bodies' machine code, each body's
/// row (an offset into this code and the function's resume points), the
/// rule the build used, and the report.
#[derive(Debug, Clone)]
pub struct Bodies {
    /// The machine the code is for.
    pub arch: Arch,
    /// Every body one after another, each starting on an instruction
    /// boundary.
    pub code: Vec<u8>,
    /// One row per body, `func` strictly ascending; `offset` is into
    /// `code`.
    pub bodies: Vec<Body>,
    /// Which rule the build used.
    pub rule: Rule,
    /// One outcome per function of the module, in pre-order.
    pub report: Vec<BodyOutcome>,
}

impl Bodies {
    /// No body: what a section without bodies is made with.
    pub fn none(arch: Arch) -> Bodies {
        Bodies {
            arch,
            code: Vec::new(),
            bodies: Vec::new(),
            rule: Rule::None,
            report: Vec::new(),
        }
    }

    /// Put the bodies into `section`, after the code it holds: each
    /// body's offset is moved by that code's length, and the section's
    /// rule becomes this one's.
    pub fn join(self, section: &mut cig_native::Section) -> Result<(), Error> {
        let align = self.arch.code_align() as usize;
        while !section.code.len().is_multiple_of(align) {
            section.code.push(0);
        }
        let base = u32::try_from(section.code.len()).map_err(|_| Error::BodyTooMany {
            what: "bytes of code",
            count: section.code.len(),
        })?;
        section.code.extend_from_slice(&self.code);
        for mut b in self.bodies {
            b.offset = b.offset.checked_add(base).ok_or(Error::BodyTooMany {
                what: "bytes of code",
                count: section.code.len(),
            })?;
            section.bodies.push(b);
        }
        section.rule = self.rule;
        Ok(())
    }
}

/// Compile a body for every function of `module` the rule gives one,
/// with registers of proven type held in machine registers.
pub fn bodies(module: &BcModule, rule: Rule, arch: Arch) -> Result<Bodies, Error> {
    bodies_as(module, rule, arch, Mode::Promoted)
}

/// [`bodies`], keeping registers as `mode` says.
///
/// The functions are taken in pre-order from the top level (index 0).
/// A function the rule covers gets a body unless its body's frame would
/// be larger than [`cig_native::MAX_BODY_FRAME`]: then it has no row,
/// and its outcome says so.
pub fn bodies_as(module: &BcModule, rule: Rule, arch: Arch, mode: Mode) -> Result<Bodies, Error> {
    let isa = target_isa(arch)?;
    let align = arch.code_align() as usize;

    let fns = enumerate_functions(module);
    let mut code = Vec::<u8>::new();
    let mut rows = Vec::<Body>::new();
    let mut report = Vec::<BodyOutcome>::with_capacity(fns.len());

    for (idx, f) in fns.iter().enumerate() {
        let func = idx as u32;
        let left = |why: String| BodyOutcome {
            func,
            compiled: false,
            why,
            code_bytes: 0,
            inline: (0, 0),
            weighed: false,
        };
        let flow = match flow::flow(f) {
            Ok(fl) => fl,
            Err(e) => {
                report.push(left(format!("the function does not decode: {e}")));
                continue;
            }
        };
        if rule == Rule::None {
            report.push(left("the rule gives no function a body".to_string()));
            continue;
        }
        // The top level runs once: only its loops are weighed.
        let top = idx == 0;
        if rule == Rule::Loops && top && flow.loop_heads.is_empty() {
            report.push(left("it has no loop".to_string()));
            continue;
        }
        let made = match compile_one(f, &flow, &*isa, mode, rule, top) {
            Ok(Some(c)) => c,
            // The plan found nothing that machine code would run faster.
            Ok(None) => {
                let why = match top {
                    true => "none of its loops would run faster as machine code",
                    false => "it would not run faster as machine code",
                };
                report.push(BodyOutcome {
                    weighed: true,
                    ..left(why.to_string())
                });
                continue;
            }
            Err(e) => {
                report.push(left(e));
                continue;
            }
        };
        if made.frame_bytes > cig_native::MAX_BODY_FRAME {
            report.push(left(format!(
                "its body's frame is larger than {} bytes",
                cig_native::MAX_BODY_FRAME
            )));
            continue;
        }
        if let Some(t) = made.traps.first() {
            return Err(Error::BodyTrap {
                func,
                what: t.clone(),
            });
        }
        if let Some(r) = made.stray_relocs.first() {
            return Err(Error::BodyStrayReloc {
                func,
                what: r.clone(),
            });
        }
        while !code.len().is_multiple_of(align) {
            code.push(0);
        }
        let offset = u32::try_from(code.len()).map_err(|_| Error::BodyTooMany {
            what: "bytes of body code",
            count: code.len(),
        })?;
        code.extend_from_slice(&made.code);
        rows.push(Body {
            func,
            offset,
            resumes: made.resumes,
        });
        report.push(BodyOutcome {
            func,
            compiled: true,
            why: String::new(),
            code_bytes: made.code.len() as u32,
            inline: made.inline,
            weighed: false,
        });
    }
    while !code.len().is_multiple_of(align) {
        code.push(0);
    }
    Ok(Bodies {
        arch,
        code,
        bodies: rows,
        rule,
        report,
    })
}

/// Pre-order enumeration of a module's functions: the top level first,
/// each function's children after it in their own order. It is the
/// order the VM numbers them in.
fn enumerate_functions(module: &BcModule) -> Vec<&BcFunction> {
    fn push<'a>(f: &'a BcFunction, out: &mut Vec<&'a BcFunction>) {
        out.push(f);
        for c in &f.children {
            push(c, out);
        }
    }
    let mut out = Vec::new();
    push(&module.main, &mut out);
    out
}

/// One compiled body's bytes and what is known of them.
struct OneBody {
    code: Vec<u8>,
    frame_bytes: usize,
    traps: Vec<String>,
    stray_relocs: Vec<String>,
    inline: (u32, u32),
    /// Where the VM enters it: the plan's.
    resumes: Vec<u32>,
}

/// One function's body, or `None` when the plan has nothing of it
/// native.
fn compile_one(
    f: &BcFunction,
    flow: &Flow,
    isa: &dyn TargetIsa,
    mode: Mode,
    rule: Rule,
    top: bool,
) -> Result<Option<OneBody>, String> {
    let call_conv = isa.default_call_conv();
    let ptr = isa.pointer_type();
    let (_, code) = decode_table(&f.instrs).map_err(|e| format!("{e:?}"))?;
    let types = typed::infer(f, flow);
    let end = f.instrs.len() as u32;

    // What machine code can do with each instruction, then who runs it.
    let mut class = BTreeMap::new();
    let mut after_interp = false;
    for d in code.iter().flatten() {
        let mut c = class_of(f, flow, &types, &code, d);
        // A `nop` after an instruction the interpreter runs carries a
        // position for it: the interpreter goes on past it, so it is
        // the interpreter's too.
        if d.op == Op::NOP && after_interp {
            c = Class::Interp;
        }
        after_interp = c == Class::Interp;
        class.insert(d.pc, c);
    }
    let asked = plan::Asked {
        straight: !top,
        // The checked build leaves every call to the VM's own loop:
        // the tests then run both ways of making one on every script.
        carry: mode != Mode::Checked,
    };
    let plan = plan::plan(rule, flow, &code, &class, end, asked);
    if plan.resumes.is_empty() {
        return Ok(None);
    }

    // `fn(ctx: *mut Context, at: u32) -> u64`.
    let mut sig = Signature::new(call_conv);
    sig.params.push(AbiParam::new(ptr));
    sig.params.push(AbiParam::new(types::I32));
    sig.returns.push(AbiParam::new(types::I64));
    let mut func = Function::with_name_signature(UserFuncName::user(0, 0), sig);
    let mut fbc = FunctionBuilderContext::new();
    let mut b = FunctionBuilder::new(&mut func, &mut fbc);

    // `run(ctx, from, to) -> u64`, as `cig_native::Helpers` has it.
    let mut run_sig = Signature::new(call_conv);
    run_sig.params.push(AbiParam::new(ptr));
    run_sig.params.push(AbiParam::new(types::I32));
    run_sig.params.push(AbiParam::new(types::I32));
    run_sig.returns.push(AbiParam::new(types::I64));
    let run_sig = b.import_signature(run_sig);
    // `again(ctx, pc) -> u64`.
    let mut again_sig = Signature::new(call_conv);
    again_sig.params.push(AbiParam::new(ptr));
    again_sig.params.push(AbiParam::new(types::I32));
    again_sig.returns.push(AbiParam::new(types::I64));
    let again_sig = b.import_signature(again_sig);
    // `list(ctx, reg) -> u32`.
    let mut list_sig = Signature::new(call_conv);
    list_sig.params.push(AbiParam::new(ptr));
    list_sig.params.push(AbiParam::new(types::I32));
    list_sig.returns.push(AbiParam::new(types::I32));
    let list_sig = b.import_signature(list_sig);
    // `push(ctx, list, value, dst) -> u32`, `copy(ctx, dst, src) -> u32`
    // and `clear(ctx, reg) -> u32`: a context and so many registers.
    let mut regs_sig = |n: usize| {
        let mut sig = Signature::new(call_conv);
        sig.params.push(AbiParam::new(ptr));
        for _ in 0..n {
            sig.params.push(AbiParam::new(types::I32));
        }
        sig.returns.push(AbiParam::new(types::I32));
        b.import_signature(sig)
    };
    let (push_sig, copy_sig, clear_sig) = (regs_sig(3), regs_sig(2), regs_sig(1));

    let entry = b.create_block();
    b.append_block_params_for_function_params(entry);
    b.switch_to_block(entry);
    let params = b.block_params(entry).to_vec();
    let ctx = b.declare_var(ptr);
    b.def_var(ctx, params[0]);
    let frame = b.declare_var(ptr);
    let steps = b.declare_var(types::I64);
    // Defined on every path before any use: each landing reads them.
    let null = b.ins().iconst(ptr, 0);
    b.def_var(frame, null);
    let zero = b.ins().iconst(types::I64, 0);
    b.def_var(steps, zero);
    // A view for each register a native index form takes its list
    // from: whether its elements are in view, where the first lies and
    // how many there are. Nothing is in view when the body is entered.
    let mut views = BTreeMap::new();
    for d in code.iter().flatten() {
        let list = match d.op {
            Op::GETINDEX => d.b_u,
            Op::SETINDEX => d.a,
            // The list `iterprep` made of what the loop walks.
            Op::ITERNEXT => d.a + 1,
            _ => continue,
        };
        if plan.how(d.pc) != How::Native || views.contains_key(&list) {
            continue;
        }
        let view = View {
            known: b.declare_var(types::I8),
            first: b.declare_var(ptr),
            count: b.declare_var(types::I64),
        };
        let no = b.ins().iconst(types::I8, 0);
        b.def_var(view.known, no);
        b.def_var(view.first, null);
        b.def_var(view.count, zero);
        views.insert(list, view);
    }

    let blocks: BTreeMap<u32, CbBlock> = plan
        .leaders
        .iter()
        .map(|pc| (*pc, b.create_block()))
        .collect();

    let mut e = Emit {
        f,
        flow,
        types: &types,
        code: &code,
        plan: &plan,
        mode,
        ptr,
        run_sig,
        again_sig,
        list_sig,
        push_sig,
        copy_sig,
        clear_sig,
        views,
        ctx,
        frame,
        steps,
        ivar: BTreeMap::new(),
        fvar: BTreeMap::new(),
        blocks,
        edges: BTreeMap::new(),
        later: Vec::new(),
        end,
        filled: false,
    };

    // The entry: a jump table on `at` over the resume points' landings.
    let defect = b.create_block();
    let mut landings = Vec::with_capacity(plan.resumes.len());
    for _ in &plan.resumes {
        landings.push(b.create_block());
    }
    let table: Vec<_> = landings
        .iter()
        .map(|blk| b.func.dfg.block_call(*blk, &[]))
        .collect();
    let default = b.func.dfg.block_call(defect, &[]);
    let jt = b.create_jump_table(JumpTableData::new(default, &table));
    b.ins().br_table(params[1], jt);

    b.switch_to_block(defect);
    b.set_cold_block(defect);
    let bits = b.ins().iconst(types::I64, Stop::Defect(0).bits() as i64);
    b.ins().return_(&[bits]);

    // A landing reads what a body finds when it is entered: the frame,
    // the steps left, and every register held at that pc.
    for (pc, blk) in plan.resumes.iter().zip(&landings) {
        b.switch_to_block(*blk);
        e.filled = false;
        e.reload_context(&mut b);
        let held: Vec<u32> = (0..flow.reg_count)
            .filter(|r| e.held(*pc, *r).is_some())
            .collect();
        e.read_back(&mut b, *pc, &held);
        let to = e.block(*pc)?;
        b.ins().jump(to, &[]);
    }

    // The blocks, in pc order.
    for (i, from) in plan.leaders.iter().enumerate() {
        let to = plan.leaders.get(i + 1).copied().unwrap_or(end);
        let blk = e.block(*from)?;
        b.switch_to_block(blk);
        e.filled = false;
        e.block_body(&mut b, *from, to)?;
        // The block is closed: the edge blocks it asked for can be
        // filled.
        e.fill_edges(&mut b);
    }

    b.seal_all_blocks();
    b.finalize(isa.frontend_config());
    let native = flow
        .pcs
        .iter()
        .filter(|pc| plan.how(**pc) == How::Native)
        .count();
    let inline = (native as u32, flow.pcs.len() as u32);

    let mut cx = Context::for_function(func);
    let mut cp = ControlPlane::default();
    cx.compile(isa, &mut cp).map_err(|e| format!("{e:?}"))?;
    let compiled = cx
        .take_compiled_code()
        .ok_or("Cranelift did not leave a compiled code")?;
    let buffer = &compiled.buffer;
    // A body calls nothing but the runtime's functions, through the
    // context: any relocation is a defect.
    let stray_relocs: Vec<String> = buffer
        .relocs()
        .iter()
        .map(|r| format!("{:?}", r.target))
        .collect();
    // The traps Cranelift may record are those of the machine's
    // division inside an `srem` and an `sdiv`: a zero divisor, and the
    // least int by -1. No run reaches either: `%` and `/` test for both
    // before they divide.
    let traps: Vec<String> = buffer
        .traps()
        .iter()
        .filter(|t| {
            t.code != cranelift_codegen::ir::TrapCode::INTEGER_DIVISION_BY_ZERO
                && t.code != cranelift_codegen::ir::TrapCode::INTEGER_OVERFLOW
        })
        .map(|t| format!("{t:?}"))
        .collect();
    let frame_bytes = buffer
        .frame_layout()
        .ok_or("Cranelift gave no frame layout")?
        .frame_to_fp_offset as usize
        + 2 * isa.pointer_bytes() as usize;
    Ok(Some(OneBody {
        code: buffer.data().to_vec(),
        frame_bytes,
        traps,
        stray_relocs,
        inline,
        resumes: plan.resumes.clone(),
    }))
}

/// An operand of an inline form.
#[derive(Clone, Copy)]
enum Opd {
    /// Null, known.
    Null,
    /// A bool, known: 0 or 1 in an `i64`.
    Bool(CbValue),
    /// An int, known.
    Int(CbValue),
    /// A float, known.
    Float(CbValue),
    /// A register in the frame, of whatever type its tag says.
    Reg(u32),
}

/// A condition known when the body is made, or only when it runs.
#[derive(Clone, Copy)]
enum Cond {
    Always,
    Never,
    If(CbValue),
}

/// A list's elements as machine code has them in view: the runtime said
/// where the first lies and how many there are, and that holds until
/// machine code next calls a function of the runtime that runs the
/// interpreter (it may grow the list, or put another in the register).
#[derive(Clone, Copy)]
struct View {
    /// 1 while the two below hold.
    known: Variable,
    first: Variable,
    count: Variable,
}

/// An edge block: the block, the registers it writes to the frame
/// (each with the type it is held as), and the block it goes on to.
type EdgeBlock = (CbBlock, Vec<(u32, Ty)>, CbBlock);

struct Emit<'a> {
    f: &'a BcFunction,
    flow: &'a Flow,
    types: &'a Types,
    code: &'a [Option<DecodedInstr>],
    plan: &'a Plan,
    mode: Mode,
    ptr: CbType,
    run_sig: SigRef,
    again_sig: SigRef,
    list_sig: SigRef,
    push_sig: SigRef,
    copy_sig: SigRef,
    clear_sig: SigRef,
    /// The view of each register a native index form takes a list from.
    views: BTreeMap<u32, View>,
    ctx: Variable,
    frame: Variable,
    steps: Variable,
    /// A held int's or bool's machine register, by register.
    ivar: BTreeMap<u32, Variable>,
    /// A held float's.
    fvar: BTreeMap<u32, Variable>,
    blocks: BTreeMap<u32, CbBlock>,
    /// Each register's type on an edge, as the pass has it.
    edges: BTreeMap<(u32, u32), Vec<Ty>>,
    /// Edge blocks made and not yet filled. A block is filled once the
    /// one being filled is closed.
    later: Vec<EdgeBlock>,
    end: u32,
    /// The block being filled has its last instruction.
    filled: bool,
}

fn flags() -> MemFlagsData {
    MemFlagsData::trusted()
}

fn is_held(t: Ty) -> bool {
    matches!(t, Ty::Int | Ty::Float | Ty::Bool)
}

fn tag_of(t: Ty) -> Option<u8> {
    Some(match t {
        Ty::Null => value::NULL,
        Ty::Bool => value::BOOL,
        Ty::Int => value::INT,
        Ty::Float => value::FLOAT,
        Ty::None | Ty::Any => return None,
    })
}

impl Emit<'_> {
    fn block(&self, pc: u32) -> Result<CbBlock, String> {
        self.blocks
            .get(&pc)
            .copied()
            .ok_or_else(|| format!("pc {pc} starts no block"))
    }

    fn instr(&self, pc: u32) -> Result<DecodedInstr, String> {
        self.code
            .get(pc as usize)
            .copied()
            .flatten()
            .ok_or_else(|| format!("pc {pc} starts no instruction"))
    }

    /// The type register `r` is held as before the instruction at `pc`:
    /// `None` when it is in the frame.
    fn held(&self, pc: u32, r: u32) -> Option<Ty> {
        if self.mode != Mode::Promoted {
            return None;
        }
        self.types
            .before(pc)
            .get(r as usize)
            .copied()
            .filter(|t| is_held(*t))
    }

    /// What the pass proved of `r` before `pc`, when the body may rely
    /// on it: nothing in the modes that keep every register in the
    /// frame and read each with its tag.
    fn proven(&self, pc: u32, r: u32) -> Ty {
        if self.mode != Mode::Promoted {
            return Ty::Any;
        }
        self.types
            .before(pc)
            .get(r as usize)
            .copied()
            .unwrap_or(Ty::Any)
    }

    /// The type `r` is held as when `from` has run and the bytecode
    /// goes on at `to`.
    fn held_on(&mut self, from: u32, to: u32, r: u32) -> Option<Ty> {
        if self.mode != Mode::Promoted {
            return None;
        }
        let types = self.types;
        self.edges
            .entry((from, to))
            .or_insert_with(|| types.on_edge(from, to))
            .get(r as usize)
            .copied()
            .filter(|t| is_held(*t))
    }

    fn var(&mut self, b: &mut FunctionBuilder, r: u32, t: Ty) -> Variable {
        if t == Ty::Float {
            *self
                .fvar
                .entry(r)
                .or_insert_with(|| b.declare_var(types::F64))
        } else {
            *self
                .ivar
                .entry(r)
                .or_insert_with(|| b.declare_var(types::I64))
        }
    }

    // ----- the frame ---------------------------------------------------------

    fn slot(r: u32) -> i32 {
        (r as i64 * value::SIZE as i64) as i32
    }

    fn tag(&self, b: &mut FunctionBuilder, r: u32) -> CbValue {
        let frame = b.use_var(self.frame);
        b.ins()
            .load(types::I8, flags(), frame, Self::slot(r) + value::TAG as i32)
    }

    fn tag_is(&self, b: &mut FunctionBuilder, r: u32, tag: u8) -> CbValue {
        let t = self.tag(b, r);
        b.ins().icmp_imm_s(IntCC::Equal, t, i64::from(tag))
    }

    /// `r`'s payload as an `i64`: an int, or a float's bits.
    fn bits(&self, b: &mut FunctionBuilder, r: u32) -> CbValue {
        let frame = b.use_var(self.frame);
        b.ins().load(
            types::I64,
            flags(),
            frame,
            Self::slot(r) + value::PAYLOAD as i32,
        )
    }

    fn float(&self, b: &mut FunctionBuilder, r: u32) -> CbValue {
        let frame = b.use_var(self.frame);
        b.ins().load(
            types::F64,
            flags(),
            frame,
            Self::slot(r) + value::PAYLOAD as i32,
        )
    }

    /// `r`'s bool, 0 or 1 in an `i64`.
    fn boolean(&self, b: &mut FunctionBuilder, r: u32) -> CbValue {
        let frame = b.use_var(self.frame);
        let byte = b.ins().load(
            types::I8,
            flags(),
            frame,
            Self::slot(r) + value::PAYLOAD as i32,
        );
        b.ins().uextend(types::I64, byte)
    }

    /// Store a known value in `r`'s place in the frame. The caller has
    /// seen to it that what was there owns no memory.
    fn store(&self, b: &mut FunctionBuilder, r: u32, v: Opd) {
        let frame = b.use_var(self.frame);
        let at = Self::slot(r);
        let (tag, payload) = match v {
            Opd::Null => (value::NULL, None),
            Opd::Bool(x) => (value::BOOL, Some(b.ins().ireduce(types::I8, x))),
            Opd::Int(x) => (value::INT, Some(x)),
            Opd::Float(x) => (value::FLOAT, Some(x)),
            Opd::Reg(_) => return,
        };
        let t = b.ins().iconst(types::I8, i64::from(tag));
        b.ins().store(flags(), t, frame, at + value::TAG as i32);
        if let Some(p) = payload {
            b.ins().store(flags(), p, frame, at + value::PAYLOAD as i32);
        }
    }

    // ----- the context -------------------------------------------------------

    /// What the body takes from the context again when the interpreter
    /// has run: the frame's address and the steps left. No list is in
    /// view after it: the interpreter may have grown one, or put
    /// another value in its register.
    fn reload_context(&self, b: &mut FunctionBuilder) {
        let ctx = b.use_var(self.ctx);
        let frame = b.ins().load(self.ptr, flags(), ctx, CTX_FRAME);
        b.def_var(self.frame, frame);
        let steps = b.ins().load(types::I64, flags(), ctx, CTX_STEPS_LEFT);
        b.def_var(self.steps, steps);
        self.forget_views(b);
    }

    /// The elements of the list in register `r`: the address of the
    /// first and how many there are. They are asked of the runtime when
    /// they are not in view, and it goes to `fb` when the register
    /// holds no list machine code may read where it lies (it holds
    /// something else, or the list is borrowed).
    ///
    /// `list` reads that one register, which is in the frame: a
    /// register that holds a list is never held in a machine register.
    /// It runs no instruction, so the other views and the frame's
    /// address stand.
    fn view(
        &mut self,
        b: &mut FunctionBuilder,
        r: u32,
        fb: CbBlock,
    ) -> Result<(CbValue, CbValue), String> {
        let view = *self
            .views
            .get(&r)
            .ok_or_else(|| format!("no view was made for register {r}"))?;
        let have = b.create_block();
        let ask = b.create_block();
        let known = b.use_var(view.known);
        b.ins().brif(known, have, &[], ask, &[]);
        b.switch_to_block(ask);
        b.set_cold_block(ask);
        let ctx = b.use_var(self.ctx);
        let helpers = b.ins().load(self.ptr, flags(), ctx, CTX_HELPERS);
        let list = b.ins().load(self.ptr, flags(), helpers, HLP_LIST);
        let reg = b.ins().iconst(types::I32, i64::from(r));
        let call = b.ins().call_indirect(self.list_sig, list, &[ctx, reg]);
        let got = b.inst_results(call)[0];
        let yes = b.create_block();
        b.ins().brif(got, yes, &[], fb, &[]);
        b.switch_to_block(yes);
        let first = b.ins().load(self.ptr, flags(), ctx, CTX_LEFT);
        let count = b.ins().load(types::I64, flags(), ctx, CTX_RIGHT);
        b.def_var(view.first, first);
        b.def_var(view.count, count);
        let one = b.ins().iconst(types::I8, 1);
        b.def_var(view.known, one);
        b.ins().jump(have, &[]);
        b.switch_to_block(have);
        Ok((b.use_var(view.first), b.use_var(view.count)))
    }

    /// The address of element `index` of the list whose elements are
    /// in view, an int index: to `fb` when the list has no such
    /// element (a negative index is one of those: it is compared as an
    /// unsigned number).
    fn element(
        &mut self,
        b: &mut FunctionBuilder,
        list: u32,
        index: CbValue,
        fb: CbBlock,
    ) -> Result<CbValue, String> {
        let (first, count) = self.view(b, list, fb)?;
        let inside = b.ins().icmp(IntCC::UnsignedLessThan, index, count);
        let ok = b.create_block();
        b.ins().brif(inside, ok, &[], fb, &[]);
        b.switch_to_block(ok);
        let offset = b.ins().imul_imm_s(index, value::SIZE as i64);
        Ok(b.ins().iadd(first, offset))
    }

    /// An index operand as an int: to `fb` when it is not one.
    fn index_of(&mut self, b: &mut FunctionBuilder, x: Opd, fb: CbBlock) -> CbValue {
        match self.is_int(b, x) {
            Cond::Always => {}
            Cond::If(c) => {
                let ok = b.create_block();
                b.ins().brif(c, ok, &[], fb, &[]);
                b.switch_to_block(ok);
            }
            Cond::Never => {
                let dead = b.create_block();
                b.ins().jump(fb, &[]);
                b.switch_to_block(dead);
            }
        }
        self.int_of(b, x)
    }

    /// `getindex A, B, C`: `R(A) = R(B)[R(C)]` for a list, an int
    /// index it has an element at, and an element that owns no memory
    /// (a number, a bool, null): the element is copied, tag and
    /// payload. Anything else is the interpreter's: a map, a string, an
    /// index past the end (its error), an element that is a string or
    /// a list (it is shared, and only the interpreter counts shares).
    fn getindex(&mut self, b: &mut FunctionBuilder, d: &DecodedInstr) -> Result<(), String> {
        let pc = d.pc;
        let fb = b.create_block();
        let join = b.create_block();
        let x = self.operand(b, pc, d.c);
        let index = self.index_of(b, x, fb);
        let at = self.element(b, d.b_u, index, fb)?;
        let tag = b.ins().load(types::I8, flags(), at, value::TAG as i32);
        let owns = b.ins().icmp_imm_s(
            IntCC::UnsignedGreaterThanOrEqual,
            tag,
            i64::from(value::OWNS),
        );
        let plain = b.create_block();
        b.ins().brif(owns, fb, &[], plain, &[]);
        b.switch_to_block(plain);
        let bits = b.ins().load(types::I64, flags(), at, value::PAYLOAD as i32);
        // The element is read whole before `R(A)` is made ready: `R(A)`
        // may hold the list itself (`x = x[i]`), and clearing it may
        // free the elements.
        self.over(b, pc, d.a, fb);
        // The type pass has `R(A)` as anything after a read: it is in
        // the frame.
        let frame = b.use_var(self.frame);
        let to = Self::slot(d.a);
        b.ins().store(flags(), tag, frame, to + value::TAG as i32);
        b.ins()
            .store(flags(), bits, frame, to + value::PAYLOAD as i32);
        b.ins().jump(join, &[]);
        self.fall_back(b, d, fb, join)?;
        b.switch_to_block(join);
        Ok(())
    }

    /// `setindex A, B, C`: `R(A)[R(B)] = R(C)` for a list, an int index
    /// it has an element at, a value that owns no memory, and an
    /// element that owns none (so nothing is freed and nothing shared):
    /// the value is stored where the element lies.
    fn setindex(&mut self, b: &mut FunctionBuilder, d: &DecodedInstr) -> Result<(), String> {
        let pc = d.pc;
        let fb = b.create_block();
        let join = b.create_block();
        let x = self.operand(b, pc, d.b_u);
        let index = self.index_of(b, x, fb);
        // The value, as a tag and a payload.
        let v = self.operand(b, pc, d.c);
        let (tag, payload) = match v {
            Opd::Null => (b.ins().iconst(types::I8, i64::from(value::NULL)), None),
            Opd::Bool(x) => (
                b.ins().iconst(types::I8, i64::from(value::BOOL)),
                Some(b.ins().ireduce(types::I8, x)),
            ),
            Opd::Int(x) => (b.ins().iconst(types::I8, i64::from(value::INT)), Some(x)),
            Opd::Float(x) => (b.ins().iconst(types::I8, i64::from(value::FLOAT)), Some(x)),
            Opd::Reg(r) => {
                let t = self.tag(b, r);
                let owns = b.ins().icmp_imm_s(
                    IntCC::UnsignedGreaterThanOrEqual,
                    t,
                    i64::from(value::OWNS),
                );
                let plain = b.create_block();
                b.ins().brif(owns, fb, &[], plain, &[]);
                b.switch_to_block(plain);
                (t, Some(self.bits(b, r)))
            }
        };
        let at = self.element(b, d.a, index, fb)?;
        let old = b.ins().load(types::I8, flags(), at, value::TAG as i32);
        let owns = b.ins().icmp_imm_s(
            IntCC::UnsignedGreaterThanOrEqual,
            old,
            i64::from(value::OWNS),
        );
        let plain = b.create_block();
        b.ins().brif(owns, fb, &[], plain, &[]);
        b.switch_to_block(plain);
        b.ins().store(flags(), tag, at, value::TAG as i32);
        if let Some(p) = payload {
            b.ins().store(flags(), p, at, value::PAYLOAD as i32);
        }
        b.ins().jump(join, &[]);
        self.fall_back(b, d, fb, join)?;
        b.switch_to_block(join);
        Ok(())
    }

    fn flush_steps(&self, b: &mut FunctionBuilder) {
        let ctx = b.use_var(self.ctx);
        let s = b.use_var(self.steps);
        b.ins().store(flags(), s, ctx, CTX_STEPS_LEFT);
    }

    /// Write every register held before `pc` to the frame.
    fn sync(&mut self, b: &mut FunctionBuilder, pc: u32) {
        for r in 0..self.flow.reg_count {
            if let Some(t) = self.held(pc, r) {
                let var = self.var(b, r, t);
                let v = b.use_var(var);
                self.store(b, r, known(t, v));
            }
        }
    }

    /// Read each of `regs` that is held before `pc` back from the
    /// frame, its tag compared with its proven type.
    fn read_back(&mut self, b: &mut FunctionBuilder, pc: u32, regs: &[u32]) {
        for r in regs {
            let Some(t) = self.held(pc, *r) else {
                continue;
            };
            let Some(tag) = tag_of(t) else {
                continue;
            };
            let ok = self.tag_is(b, *r, tag);
            let bad = b.create_block();
            let good = b.create_block();
            b.set_cold_block(bad);
            b.ins().brif(ok, good, &[], bad, &[]);
            b.switch_to_block(bad);
            let bits = b.ins().iconst(types::I64, Stop::Defect(pc).bits() as i64);
            b.ins().return_(&[bits]);
            b.switch_to_block(good);
            let v = match t {
                Ty::Float => self.float(b, *r),
                Ty::Bool => self.boolean(b, *r),
                _ => self.bits(b, *r),
            };
            let var = self.var(b, *r, t);
            b.def_var(var, v);
        }
    }

    /// Return `At(pc)`: the bytecode goes on there, with the frame
    /// whole.
    fn leave(&mut self, b: &mut FunctionBuilder, pc: u32) {
        self.sync(b, pc);
        self.flush_steps(b);
        let bits = b.ins().iconst(types::I64, Stop::At(pc).bits() as i64);
        b.ins().return_(&[bits]);
        self.filled = true;
    }

    /// Call `run(ctx, from, to)` with the frame whole, and take the
    /// frame's address and the steps left again: the stop's bits.
    fn run(&mut self, b: &mut FunctionBuilder, from: u32, to: u32) -> CbValue {
        self.sync(b, from);
        self.flush_steps(b);
        let ctx = b.use_var(self.ctx);
        let helpers = b.ins().load(self.ptr, flags(), ctx, CTX_HELPERS);
        let run = b.ins().load(self.ptr, flags(), helpers, HLP_RUN);
        let from_v = b.ins().iconst(types::I32, i64::from(from));
        let to_v = b.ins().iconst(types::I32, i64::from(to));
        let call = b
            .ins()
            .call_indirect(self.run_sig, run, &[ctx, from_v, to_v]);
        let stop = b.inst_results(call)[0];
        self.reload_context(b);
        stop
    }

    /// Call `again(ctx, pc)` with the frame whole: the interpreter runs
    /// the one instruction at `pc`, whose inline form did not cover its
    /// operands. The stop's bits.
    fn again(&mut self, b: &mut FunctionBuilder, pc: u32) -> CbValue {
        self.sync(b, pc);
        self.flush_steps(b);
        let ctx = b.use_var(self.ctx);
        let helpers = b.ins().load(self.ptr, flags(), ctx, CTX_HELPERS);
        let again = b.ins().load(self.ptr, flags(), helpers, HLP_AGAIN);
        let pc_v = b.ins().iconst(types::I32, i64::from(pc));
        let call = b.ins().call_indirect(self.again_sig, again, &[ctx, pc_v]);
        let stop = b.inst_results(call)[0];
        self.reload_context(b);
        stop
    }

    /// The registers the instructions of `from..to` can have written,
    /// or have a type proven of that they did not have before: what is
    /// read back from the frame after the interpreter ran them.
    fn touched(&self, from: u32, to: u32) -> Result<BTreeSet<u32>, String> {
        let mut out = BTreeSet::new();
        let mut pc = from;
        while pc < to {
            let d = self.instr(pc)?;
            let w = typed::writes(&d);
            out.extend(w.regs.iter().map(|(r, _)| *r));
            if let Some(above) = w.above {
                out.extend(above..self.flow.reg_count);
            }
            // A counted loop's window is proven by the instruction that
            // checked it, which writes only part of it.
            if matches!(d.op, Op::FORPREP | Op::FORLOOP) {
                out.extend(d.a..d.a.saturating_add(4).min(self.flow.reg_count));
            }
            pc += d.width;
        }
        Ok(out)
    }

    // ----- edges -------------------------------------------------------------

    /// The block an inline instruction at `from` goes to on its way to
    /// `to`, which goes on in `target`: `target` itself, or a block
    /// that first writes to the frame each register held on the way
    /// that `to` does not hold. The block is filled later
    /// ([`Emit::fill_edges`]): the one being filled is not closed yet.
    fn edge_into(
        &mut self,
        b: &mut FunctionBuilder,
        from: u32,
        to: u32,
        target: CbBlock,
    ) -> CbBlock {
        let mut spill = Vec::new();
        for r in 0..self.flow.reg_count {
            if let Some(t) = self.held_on(from, to, r) {
                if self.held(to, r) != Some(t) {
                    spill.push((r, t));
                }
            }
        }
        if spill.is_empty() {
            return target;
        }
        let blk = b.create_block();
        self.later.push((blk, spill, target));
        blk
    }

    /// [`Emit::edge_into`] the block of `to`.
    fn edge(&mut self, b: &mut FunctionBuilder, from: u32, to: u32) -> Result<CbBlock, String> {
        if to >= self.end {
            return Err(format!("pc {from} goes on past the function's end"));
        }
        let target = self.block(to)?;
        Ok(self.edge_into(b, from, to, target))
    }

    /// Fill the edge blocks made so far. The block being filled is
    /// closed when this is called.
    fn fill_edges(&mut self, b: &mut FunctionBuilder) {
        for (blk, spill, target) in std::mem::take(&mut self.later) {
            b.switch_to_block(blk);
            for (r, t) in spill {
                let var = self.var(b, r, t);
                let v = b.use_var(var);
                self.store(b, r, known(t, v));
            }
            b.ins().jump(target, &[]);
        }
    }

    /// Go on at `to` from the inline instruction at `from`.
    fn go(&mut self, b: &mut FunctionBuilder, from: u32, to: u32) -> Result<(), String> {
        let blk = self.edge(b, from, to)?;
        b.ins().jump(blk, &[]);
        self.filled = true;
        Ok(())
    }

    /// After the interpreter ran `from..to` and stood at `at`: read
    /// back what it touched that `at` holds, and jump there.
    fn after_run(
        &mut self,
        b: &mut FunctionBuilder,
        from: u32,
        to: u32,
        at: u32,
        target: CbBlock,
    ) -> Result<(), String> {
        let touched = self.touched(from, to)?;
        let regs: Vec<u32> = (0..self.flow.reg_count)
            .filter(|r| {
                self.held(at, *r).is_some()
                    && (touched.contains(r) || self.held(from, *r) != self.held(at, *r))
            })
            .collect();
        self.read_back(b, at, &regs);
        b.ins().jump(target, &[]);
        Ok(())
    }

    /// The interpreter ran `from..to` and stopped with `stop`. When it
    /// stands at one of `exits` the body goes on in that exit's block;
    /// any other stop is returned as it is (a call, a return, an error,
    /// or the frame the runtime takes back).
    fn hand_back(
        &mut self,
        b: &mut FunctionBuilder,
        stop: CbValue,
        from: u32,
        to: u32,
        exits: &[(u32, CbBlock)],
    ) -> Result<(), String> {
        for (at, target) in exits {
            let hit = b.create_block();
            let miss = b.create_block();
            let there = b.ins().icmp_imm_s(IntCC::Equal, stop, i64::from(*at));
            b.ins().brif(there, hit, &[], miss, &[]);
            b.switch_to_block(hit);
            self.after_run(b, from, to, *at, *target)?;
            b.switch_to_block(miss);
            b.set_cold_block(miss);
        }
        b.ins().return_(&[stop]);
        Ok(())
    }

    /// The block at `from` is the interpreter's, called from here: one
    /// `run` from `from` to the end of the stretch `from` is in, then
    /// on wherever the bytecode can leave that range for.
    fn stretch_from(&mut self, b: &mut FunctionBuilder, from: u32) -> Result<(), String> {
        let to = self.plan.stretch_end(from);
        let stop = self.run(b, from, to);
        // The stretch's end first: it is where most runs come out.
        let mut exits = Vec::new();
        let mut seen = BTreeSet::new();
        if to < self.end {
            seen.insert(to);
            exits.push((to, self.block(to)?));
        }
        let mut pc = from;
        while pc < to {
            let d = self.instr(pc)?;
            let outside: Vec<u32> = self
                .flow
                .succs
                .get(&pc)
                .into_iter()
                .flatten()
                .copied()
                .filter(|s| (*s < from || *s >= to) && *s < self.end)
                .collect();
            for s in outside {
                if seen.insert(s) {
                    exits.push((s, self.block(s)?));
                }
            }
            pc += d.width;
        }
        self.hand_back(b, stop, from, to, &exits)?;
        self.filled = true;
        Ok(())
    }

    // ----- a block -----------------------------------------------------------

    /// The block from `from` to `to`. Every instruction of it has the
    /// one answer of the plan: the plan's answers start blocks.
    fn block_body(&mut self, b: &mut FunctionBuilder, from: u32, to: u32) -> Result<(), String> {
        match self.plan.how(from) {
            // The VM's own loop runs the bytecode from here.
            How::Leave => {
                self.leave(b, from);
                Ok(())
            }
            How::Stretch => self.stretch_from(b, from),
            How::Native => {
                let mut pc = from;
                let mut last = from;
                while pc < to && !self.filled {
                    let d = self.instr(pc)?;
                    last = pc;
                    self.check(b, &d);
                    self.inline(b, &d)?;
                    pc += d.width;
                }
                if !self.filled {
                    // The edge writes what the next block does not hold.
                    self.go(b, last, to)?;
                }
                Ok(())
            }
        }
    }

    fn constant(&self, i: u32) -> Option<&Constant> {
        self.f.constants.get(i as usize)
    }

    /// [`Mode::Checked`]: every register the instruction reads or
    /// writes that the pass proved a type of has that type's tag, or
    /// the body returns `Defect(pc)`.
    fn check(&mut self, b: &mut FunctionBuilder, d: &DecodedInstr) {
        if self.mode != Mode::Checked {
            return;
        }
        let mut regs: BTreeSet<u32> = typed::reads(d).into_iter().collect();
        regs.extend(typed::writes(d).regs.iter().map(|(r, _)| *r));
        for r in regs {
            let claim = self.types.before(d.pc).get(r as usize).copied();
            let Some(tag) = claim.and_then(tag_of) else {
                continue;
            };
            let ok = self.tag_is(b, r, tag);
            let bad = b.create_block();
            let good = b.create_block();
            b.set_cold_block(bad);
            b.ins().brif(ok, good, &[], bad, &[]);
            b.switch_to_block(bad);
            let bits = b.ins().iconst(types::I64, Stop::Defect(d.pc).bits() as i64);
            b.ins().return_(&[bits]);
            b.switch_to_block(good);
        }
    }

    // ----- operands ----------------------------------------------------------

    fn operand(&mut self, b: &mut FunctionBuilder, pc: u32, r: u32) -> Opd {
        match self.held(pc, r) {
            Some(t) => {
                let var = self.var(b, r, t);
                known(t, b.use_var(var))
            }
            None if self.proven(pc, r) == Ty::Null => Opd::Null,
            None => Opd::Reg(r),
        }
    }

    fn konst(&self, b: &mut FunctionBuilder, i: u32) -> Result<Opd, String> {
        Ok(match self.constant(i) {
            Some(Constant::Bool(x)) => Opd::Bool(b.ins().iconst(types::I64, i64::from(*x))),
            Some(Constant::Int(x)) => Opd::Int(b.ins().iconst(types::I64, *x)),
            Some(Constant::Float(x)) => Opd::Float(b.ins().f64const(*x)),
            _ => return Err("a constant with no inline form".to_string()),
        })
    }

    fn is_int(&self, b: &mut FunctionBuilder, x: Opd) -> Cond {
        match x {
            Opd::Int(_) => Cond::Always,
            Opd::Reg(r) => Cond::If(self.tag_is(b, r, value::INT)),
            _ => Cond::Never,
        }
    }

    /// An int or a float.
    fn is_number(&self, b: &mut FunctionBuilder, x: Opd) -> Cond {
        match x {
            Opd::Int(_) | Opd::Float(_) => Cond::Always,
            Opd::Reg(r) => {
                // INT and FLOAT are next to each other.
                let t = self.tag(b, r);
                let off = b.ins().iadd_imm_s(t, -i64::from(value::INT));
                Cond::If(b.ins().icmp_imm_s(
                    IntCC::UnsignedLessThanOrEqual,
                    off,
                    i64::from(value::FLOAT - value::INT),
                ))
            }
            _ => Cond::Never,
        }
    }

    fn int_of(&self, b: &mut FunctionBuilder, x: Opd) -> CbValue {
        match x {
            Opd::Int(v) | Opd::Bool(v) => v,
            Opd::Reg(r) => self.bits(b, r),
            Opd::Float(v) => b.ins().bitcast(types::I64, MemFlagsData::new(), v),
            Opd::Null => b.ins().iconst(types::I64, 0),
        }
    }

    /// A number as a float: an int converted, as the interpreter's
    /// `as_f64` converts it.
    fn float_of(&self, b: &mut FunctionBuilder, x: Opd) -> CbValue {
        match x {
            Opd::Float(v) => v,
            Opd::Int(v) => b.ins().fcvt_from_sint(types::F64, v),
            Opd::Reg(r) => {
                let is_int = self.tag_is(b, r, value::INT);
                let bits = self.bits(b, r);
                let as_float = b.ins().bitcast(types::F64, MemFlagsData::new(), bits);
                let converted = b.ins().fcvt_from_sint(types::F64, bits);
                b.ins().select(is_int, converted, as_float)
            }
            Opd::Null | Opd::Bool(_) => b.ins().f64const(0.0),
        }
    }

    // ----- writing -----------------------------------------------------------

    /// Make `r`'s place in the frame one a number can be stored over.
    /// When it holds a value that owns memory, the runtime frees that
    /// value and leaves null there (`clear`); it goes to `fb` when the
    /// runtime cannot. Nothing to test when the pass proved what the
    /// register holds.
    ///
    /// It is called where the instruction can no longer fail: after
    /// every test of its operands and after every operand is read, and
    /// just before the store. An instruction the interpreter runs
    /// instead must find the register as it was (it may read it, and it
    /// may raise and leave it), so nothing is cleared before that is
    /// ruled out.
    fn over(&mut self, b: &mut FunctionBuilder, pc: u32, r: u32, fb: CbBlock) {
        if tag_of(self.proven(pc, r)).is_some() {
            return;
        }
        let t = self.tag(b, r);
        let owns = b
            .ins()
            .icmp_imm_s(IntCC::UnsignedGreaterThanOrEqual, t, i64::from(value::OWNS));
        let help = b.create_block();
        let ok = b.create_block();
        b.ins().brif(owns, help, &[], ok, &[]);
        b.switch_to_block(help);
        let reg = b.ins().iconst(types::I32, i64::from(r));
        let done = self.helper(b, HLP_CLEAR, self.clear_sig, &[reg]);
        self.forget_view(b, r);
        b.ins().brif(done, ok, &[], fb, &[]);
        b.switch_to_block(ok);
    }

    /// Call the runtime's function at `at` of its table with the
    /// context and `args`: its result. For the functions that run no
    /// instruction (`clear`, `copy`, `push`): the frame's address and
    /// the steps left stand after them.
    fn helper(
        &mut self,
        b: &mut FunctionBuilder,
        at: i32,
        sig: SigRef,
        args: &[CbValue],
    ) -> CbValue {
        let ctx = b.use_var(self.ctx);
        let helpers = b.ins().load(self.ptr, flags(), ctx, CTX_HELPERS);
        let f = b.ins().load(self.ptr, flags(), helpers, at);
        let mut all = vec![ctx];
        all.extend_from_slice(args);
        let call = b.ins().call_indirect(sig, f, &all);
        b.inst_results(call)[0]
    }

    /// Register `r` no longer holds the list machine code last saw
    /// there (the runtime put another value in it).
    fn forget_view(&self, b: &mut FunctionBuilder, r: u32) {
        if let Some(view) = self.views.get(&r) {
            let no = b.ins().iconst(types::I8, 0);
            b.def_var(view.known, no);
        }
    }

    /// No list is where machine code last saw it (the runtime added to
    /// one, which may have moved its elements).
    fn forget_views(&self, b: &mut FunctionBuilder) {
        for view in self.views.values() {
            let no = b.ins().iconst(types::I8, 0);
            b.def_var(view.known, no);
        }
    }

    /// `R(r) = v`, a known value, when the bytecode goes on at `to`
    /// after `pc`: into the machine register when `r` is held there,
    /// into the frame otherwise.
    fn write(&mut self, b: &mut FunctionBuilder, pc: u32, to: u32, r: u32, v: Opd) {
        let t = match v {
            Opd::Null => Ty::Null,
            Opd::Bool(_) => Ty::Bool,
            Opd::Int(_) => Ty::Int,
            Opd::Float(_) => Ty::Float,
            Opd::Reg(_) => return,
        };
        if self.held_on(pc, to, r) == Some(t) {
            let var = self.var(b, r, t);
            let x = match v {
                Opd::Bool(x) | Opd::Int(x) | Opd::Float(x) => x,
                _ => return,
            };
            b.def_var(var, x);
        } else {
            self.store(b, r, v);
        }
    }

    /// `R(r) = v` where the instruction can no longer fail: the
    /// register is made ready ([`Emit::over`]), then written.
    #[allow(clippy::too_many_arguments)]
    fn put(&mut self, b: &mut FunctionBuilder, pc: u32, to: u32, r: u32, fb: CbBlock, v: Opd) {
        self.over(b, pc, r, fb);
        self.write(b, pc, to, r, v);
    }

    // ----- the fall back -----------------------------------------------------

    /// Fill `fb`: the interpreter runs the instruction (`again`), then
    /// the body goes on in `join` when it stands at the next pc, at the
    /// instruction's own jump target when it stands there, and returns
    /// the stop otherwise. A stop at the instruction's own pc is the
    /// runtime taking the frame back with nothing run, never a jump.
    fn fall_back(
        &mut self,
        b: &mut FunctionBuilder,
        d: &DecodedInstr,
        fb: CbBlock,
        join: CbBlock,
    ) -> Result<(), String> {
        b.switch_to_block(fb);
        b.set_cold_block(fb);
        let (pc, next) = (d.pc, d.pc + d.width);
        let stop = self.again(b, pc);
        let mut exits = Vec::new();
        // The interpreter goes on past the `nop`s after an index
        // instruction: that pc starts a block (the plan made it one).
        if let Some(past) = plan::past_nops(self.code, d).filter(|p| *p != next && *p < self.end) {
            exits.push((past, self.block(past)?));
        }
        exits.push((next, join));
        if let Some(t) = flow::target_of(d).filter(|t| *t != next && *t != pc && *t < self.end) {
            exits.push((t, self.block(t)?));
        }
        self.hand_back(b, stop, pc, next, &exits)
    }

    // ----- inline forms ------------------------------------------------------

    fn inline(&mut self, b: &mut FunctionBuilder, d: &DecodedInstr) -> Result<(), String> {
        let pc = d.pc;
        let next = pc + d.width;
        match d.op {
            Op::NOP => Ok(()),
            Op::JMP | Op::LOOP => {
                let t = flow::target_of(d).ok_or("a jump before the function")?;
                self.go(b, pc, t)
            }
            Op::STEPS => {
                let s = b.use_var(self.steps);
                let left = b.ins().iadd_imm_s(s, -i64::from(d.b_u));
                let spent = b.ins().icmp_imm_s(IntCC::SignedLessThan, left, 0);
                let out = b.create_block();
                let on = b.create_block();
                b.set_cold_block(out);
                b.ins().brif(spent, out, &[], on, &[]);
                // The budget does not hold them all: the interpreter
                // counts them one by one and raises at the one that
                // spends it.
                b.switch_to_block(out);
                self.leave(b, pc);
                self.filled = false;
                b.switch_to_block(on);
                b.def_var(self.steps, left);
                let ctx = b.use_var(self.ctx);
                let nest = b.ins().iconst(types::I32, i64::from(d.a));
                b.ins().store(flags(), nest, ctx, CTX_NEST);
                Ok(())
            }
            Op::LOADNULL | Op::LOADBOOL | Op::LOADINT | Op::LOADK => {
                let v = match d.op {
                    Op::LOADNULL => Opd::Null,
                    Op::LOADBOOL => Opd::Bool(b.ins().iconst(types::I64, i64::from(d.b_u != 0))),
                    Op::LOADINT => Opd::Int(b.ins().iconst(types::I64, i64::from(d.b_s))),
                    _ => self.konst(b, d.b_u)?,
                };
                self.set(b, d, d.a, v)
            }
            Op::MOV => {
                if d.a == d.b_u {
                    return Ok(());
                }
                match self.operand(b, pc, d.b_u) {
                    Opd::Reg(src) => self.copy(b, d, src),
                    v => self.set(b, d, d.a, v),
                }
            }
            Op::ADD | Op::SUB | Op::MUL | Op::DIV | Op::MOD => {
                let x = self.operand(b, pc, d.b_u);
                let y = self.operand(b, pc, d.c);
                self.arith(b, d, x, y)
            }
            Op::ADDK | Op::SUBK | Op::MULK | Op::DIVK | Op::MODK => {
                let x = self.operand(b, pc, d.b_u);
                let y = self.konst(b, d.c)?;
                self.arith(b, d, x, y)
            }
            Op::NEG => {
                let x = self.operand(b, pc, d.b_u);
                self.negate(b, d, x)
            }
            Op::NOT => {
                let x = self.operand(b, pc, d.b_u);
                let fb = b.create_block();
                let join = b.create_block();
                let truth = self.truth(b, x, fb);
                let not = b.ins().bxor_imm_s(truth, 1);
                let v = b.ins().uextend(types::I64, not);
                self.over(b, pc, d.a, fb);
                self.write(b, pc, next, d.a, Opd::Bool(v));
                b.ins().jump(join, &[]);
                self.fall_back(b, d, fb, join)?;
                b.switch_to_block(join);
                Ok(())
            }
            Op::EQ | Op::LT | Op::LE | Op::EQK | Op::LTK | Op::LEK => {
                let x = self.operand(b, pc, d.b_u);
                let y = match d.op {
                    Op::EQ | Op::LT | Op::LE => self.operand(b, pc, d.c),
                    _ => self.konst(b, d.c)?,
                };
                let fb = b.create_block();
                let join = b.create_block();
                let holds = match d.op {
                    Op::EQ | Op::EQK => self.equal(b, x, y, fb),
                    Op::LT | Op::LTK => self.ordered(b, x, y, false, fb),
                    _ => self.ordered(b, x, y, true, fb),
                };
                let v = b.ins().uextend(types::I64, holds);
                self.over(b, pc, d.a, fb);
                self.write(b, pc, next, d.a, Opd::Bool(v));
                b.ins().jump(join, &[]);
                self.fall_back(b, d, fb, join)?;
                b.switch_to_block(join);
                Ok(())
            }
            Op::EQJ | Op::NEJ | Op::LTJ | Op::LEJ => {
                let x = self.operand(b, pc, d.a);
                let y = self.operand(b, pc, d.b_u);
                let fb = b.create_block();
                let join = b.create_block();
                let holds = match d.op {
                    Op::EQJ => self.equal(b, x, y, fb),
                    Op::NEJ => {
                        let eq = self.equal(b, x, y, fb);
                        b.ins().bxor_imm_s(eq, 1)
                    }
                    Op::LTJ => self.ordered(b, x, y, false, fb),
                    _ => self.ordered(b, x, y, true, fb),
                };
                self.branch(b, d, holds, join)?;
                self.fall_back(b, d, fb, join)?;
                b.switch_to_block(join);
                Ok(())
            }
            Op::TEST | Op::TESTN => {
                let x = self.operand(b, pc, d.a);
                let fb = b.create_block();
                let join = b.create_block();
                let truth = self.truth(b, x, fb);
                let jumps = if d.op == Op::TEST {
                    truth
                } else {
                    b.ins().bxor_imm_s(truth, 1)
                };
                self.branch(b, d, jumps, join)?;
                self.fall_back(b, d, fb, join)?;
                b.switch_to_block(join);
                Ok(())
            }
            Op::TESTSET => self.testset(b, d),
            Op::FORPREP => self.forprep(b, d),
            Op::FORLOOP => self.forloop(b, d),
            Op::GETINDEX => self.getindex(b, d),
            Op::SETINDEX => self.setindex(b, d),
            Op::ITERNEXT => self.iternext(b, d),
            Op::CALLM => self.push(b, d),
            other => Err(format!("{other:?} has no inline form")),
        }
    }

    /// A conditional jump: to the instruction's target when `jumps`
    /// holds, on to `join` otherwise.
    fn branch(
        &mut self,
        b: &mut FunctionBuilder,
        d: &DecodedInstr,
        jumps: CbValue,
        join: CbBlock,
    ) -> Result<(), String> {
        let next = d.pc + d.width;
        let t = flow::target_of(d).ok_or("a jump before the function")?;
        let there = self.edge(b, d.pc, t)?;
        // What a jump leaves held and the next pc does not is written
        // on the way on, as on the way to the target.
        let on = self.edge_into(b, d.pc, next, join);
        b.ins().brif(jumps, there, &[], on, &[]);
        Ok(())
    }

    /// `R(dst) = v`, a known value, then on.
    fn set(
        &mut self,
        b: &mut FunctionBuilder,
        d: &DecodedInstr,
        dst: u32,
        v: Opd,
    ) -> Result<(), String> {
        let fb = b.create_block();
        let join = b.create_block();
        self.over(b, d.pc, dst, fb);
        self.write(b, d.pc, d.pc + d.width, dst, v);
        b.ins().jump(join, &[]);
        self.fall_back(b, d, fb, join)?;
        b.switch_to_block(join);
        Ok(())
    }

    /// `mov A, B` of a frame register. A value that owns no memory,
    /// over one that owns none, is copied here, tag and payload. Where
    /// either owns some, the runtime makes the copy (`copy`): it shares
    /// the one and frees the other, which only it can.
    fn copy(&mut self, b: &mut FunctionBuilder, d: &DecodedInstr, src: u32) -> Result<(), String> {
        let fb = b.create_block();
        let join = b.create_block();
        let t = self.tag(b, src);
        let mut slow =
            b.ins()
                .icmp_imm_s(IntCC::UnsignedGreaterThanOrEqual, t, i64::from(value::OWNS));
        if tag_of(self.proven(d.pc, d.a)).is_none() {
            let old = self.tag(b, d.a);
            let owns = b.ins().icmp_imm_s(
                IntCC::UnsignedGreaterThanOrEqual,
                old,
                i64::from(value::OWNS),
            );
            slow = b.ins().bor(slow, owns);
        }
        let help = b.create_block();
        let plain = b.create_block();
        b.ins().brif(slow, help, &[], plain, &[]);
        b.switch_to_block(plain);
        let bits = self.bits(b, src);
        let frame = b.use_var(self.frame);
        let at = Self::slot(d.a);
        b.ins().store(flags(), t, frame, at + value::TAG as i32);
        b.ins()
            .store(flags(), bits, frame, at + value::PAYLOAD as i32);
        b.ins().jump(join, &[]);
        // Both registers are in the frame (a register held in a machine
        // register owns nothing), and the runtime touches no other.
        b.switch_to_block(help);
        let to = b.ins().iconst(types::I32, i64::from(d.a));
        let from = b.ins().iconst(types::I32, i64::from(src));
        let done = self.helper(b, HLP_COPY, self.copy_sig, &[to, from]);
        self.forget_view(b, d.a);
        b.ins().brif(done, join, &[], fb, &[]);
        self.fall_back(b, d, fb, join)?;
        b.switch_to_block(join);
        Ok(())
    }

    /// `+`, `-`, `*`, `/`, `%`: two ints (an overflow and a zero
    /// divisor are the interpreter's), or two numbers one of which is
    /// a float. An int divided by an int is an int when it divides
    /// exactly and a float otherwise, as the interpreter has it. A
    /// float's remainder is the interpreter's.
    fn arith(
        &mut self,
        b: &mut FunctionBuilder,
        d: &DecodedInstr,
        x: Opd,
        y: Opd,
    ) -> Result<(), String> {
        let (pc, next) = (d.pc, d.pc + d.width);
        let op = match d.op {
            Op::ADD | Op::ADDK => Op::ADD,
            Op::SUB | Op::SUBK => Op::SUB,
            Op::MUL | Op::MULK => Op::MUL,
            Op::DIV | Op::DIVK => Op::DIV,
            _ => Op::MOD,
        };
        let fb = b.create_block();
        let join = b.create_block();

        let ints = {
            let p = self.is_int(b, x);
            let q = self.is_int(b, y);
            both(b, p, q)
        };
        let float_path = !matches!(ints, Cond::Always) && op != Op::MOD;
        let numbers = if float_path {
            {
                let p = self.is_number(b, x);
                let q = self.is_number(b, y);
                both(b, p, q)
            }
        } else {
            Cond::Never
        };

        // Two ints.
        let rest = b.create_block();
        match ints {
            Cond::Always => {}
            Cond::If(c) => {
                let yes = b.create_block();
                b.ins().brif(c, yes, &[], rest, &[]);
                b.switch_to_block(yes);
            }
            Cond::Never => {
                b.ins().jump(rest, &[]);
            }
        }
        if !matches!(ints, Cond::Never) {
            let (xi, yi) = (self.int_of(b, x), self.int_of(b, y));
            let v = match op {
                Op::ADD | Op::SUB | Op::MUL => {
                    let (v, over) = match op {
                        Op::ADD => b.ins().sadd_overflow(xi, yi),
                        Op::SUB => b.ins().ssub_overflow(xi, yi),
                        _ => b.ins().smul_overflow(xi, yi),
                    };
                    let ok = b.create_block();
                    b.ins().brif(over, fb, &[], ok, &[]);
                    b.switch_to_block(ok);
                    v
                }
                Op::DIV => {
                    // `/`: a zero divisor is the interpreter's error,
                    // and the least int by -1 its overflow (the one
                    // quotient that does not fit, which the machine's
                    // division would trap on). What divides exactly is
                    // an int; anything else is the two as floats.
                    let zero = b.ins().icmp_imm_s(IntCC::Equal, yi, 0);
                    let ok = b.create_block();
                    b.ins().brif(zero, fb, &[], ok, &[]);
                    b.switch_to_block(ok);
                    let least = b.ins().icmp_imm_s(IntCC::Equal, xi, i64::MIN);
                    let minus_one = b.ins().icmp_imm_s(IntCC::Equal, yi, -1);
                    let over = b.ins().band(least, minus_one);
                    let fits = b.create_block();
                    b.ins().brif(over, fb, &[], fits, &[]);
                    b.switch_to_block(fits);
                    let rem = b.ins().srem(xi, yi);
                    let exact = b.ins().icmp_imm_s(IntCC::Equal, rem, 0);
                    let whole = b.create_block();
                    let part = b.create_block();
                    b.ins().brif(exact, whole, &[], part, &[]);
                    b.switch_to_block(part);
                    let xf = b.ins().fcvt_from_sint(types::F64, xi);
                    let yf = b.ins().fcvt_from_sint(types::F64, yi);
                    let q = b.ins().fdiv(xf, yf);
                    self.put(b, pc, next, d.a, fb, Opd::Float(q));
                    b.ins().jump(join, &[]);
                    b.switch_to_block(whole);
                    b.ins().sdiv(xi, yi)
                }
                _ => {
                    // `%`: a zero divisor is the interpreter's error;
                    // by -1 the remainder is 0 (`wrapping_rem`), which
                    // the machine's division would trap on for the
                    // least int.
                    let zero = b.ins().icmp_imm_s(IntCC::Equal, yi, 0);
                    let ok = b.create_block();
                    b.ins().brif(zero, fb, &[], ok, &[]);
                    b.switch_to_block(ok);
                    let minus_one = b.ins().icmp_imm_s(IntCC::Equal, yi, -1);
                    let one = b.ins().iconst(types::I64, 1);
                    let safe = b.ins().select(minus_one, one, yi);
                    b.ins().srem(xi, safe)
                }
            };
            self.put(b, pc, next, d.a, fb, Opd::Int(v));
            b.ins().jump(join, &[]);
        }

        // Two numbers, not both ints.
        b.switch_to_block(rest);
        match numbers {
            Cond::Never => {
                b.ins().jump(fb, &[]);
            }
            other => {
                if let Cond::If(c) = other {
                    let yes = b.create_block();
                    b.ins().brif(c, yes, &[], fb, &[]);
                    b.switch_to_block(yes);
                }
                let (xf, yf) = (self.float_of(b, x), self.float_of(b, y));
                let v = match op {
                    Op::ADD => b.ins().fadd(xf, yf),
                    Op::SUB => b.ins().fsub(xf, yf),
                    Op::MUL => b.ins().fmul(xf, yf),
                    _ => {
                        let zero = b.ins().f64const(0.0);
                        let by_zero = b.ins().fcmp(FloatCC::Equal, yf, zero);
                        let ok = b.create_block();
                        b.ins().brif(by_zero, fb, &[], ok, &[]);
                        b.switch_to_block(ok);
                        b.ins().fdiv(xf, yf)
                    }
                };
                self.put(b, pc, next, d.a, fb, Opd::Float(v));
                b.ins().jump(join, &[]);
            }
        }
        self.fall_back(b, d, fb, join)?;
        b.switch_to_block(join);
        Ok(())
    }

    /// Unary `-`: an int that is not the least, a float.
    fn negate(&mut self, b: &mut FunctionBuilder, d: &DecodedInstr, x: Opd) -> Result<(), String> {
        let (pc, next) = (d.pc, d.pc + d.width);
        let fb = b.create_block();
        let join = b.create_block();
        let floats = b.create_block();
        match self.is_int(b, x) {
            Cond::Never => {
                b.ins().jump(floats, &[]);
            }
            int => {
                if let Cond::If(c) = int {
                    let yes = b.create_block();
                    b.ins().brif(c, yes, &[], floats, &[]);
                    b.switch_to_block(yes);
                }
                let xi = self.int_of(b, x);
                let least = b.ins().icmp_imm_s(IntCC::Equal, xi, i64::MIN);
                let ok = b.create_block();
                b.ins().brif(least, fb, &[], ok, &[]);
                b.switch_to_block(ok);
                let v = b.ins().ineg(xi);
                self.put(b, pc, next, d.a, fb, Opd::Int(v));
                b.ins().jump(join, &[]);
            }
        }
        b.switch_to_block(floats);
        let float = match x {
            Opd::Float(_) => Cond::Always,
            Opd::Reg(r) => Cond::If(self.tag_is(b, r, value::FLOAT)),
            _ => Cond::Never,
        };
        match float {
            Cond::Never => {
                b.ins().jump(fb, &[]);
            }
            other => {
                if let Cond::If(c) = other {
                    let yes = b.create_block();
                    b.ins().brif(c, yes, &[], fb, &[]);
                    b.switch_to_block(yes);
                }
                let xf = match x {
                    Opd::Float(v) => v,
                    Opd::Reg(r) => self.float(b, r),
                    _ => b.ins().f64const(0.0),
                };
                let v = b.ins().fneg(xf);
                self.put(b, pc, next, d.a, fb, Opd::Float(v));
                b.ins().jump(join, &[]);
            }
        }
        self.fall_back(b, d, fb, join)?;
        b.switch_to_block(join);
        Ok(())
    }

    /// Whether `x` is truthy, as `Value::truthy` has it for null, a
    /// bool, an int and a float; any other value is the interpreter's.
    fn truth(&mut self, b: &mut FunctionBuilder, x: Opd, fb: CbBlock) -> CbValue {
        match x {
            Opd::Null => b.ins().iconst(types::I8, 0),
            Opd::Bool(v) | Opd::Int(v) => b.ins().icmp_imm_s(IntCC::NotEqual, v, 0),
            Opd::Float(v) => {
                let zero = b.ins().f64const(0.0);
                b.ins().fcmp(FloatCC::NotEqual, v, zero)
            }
            Opd::Reg(r) => {
                let t = self.tag(b, r);
                let out = b.declare_var(types::I8);
                let done = b.create_block();
                // A float.
                let is_float = b.ins().icmp_imm_s(IntCC::Equal, t, i64::from(value::FLOAT));
                let float = b.create_block();
                let not_float = b.create_block();
                b.ins().brif(is_float, float, &[], not_float, &[]);
                b.switch_to_block(float);
                let f = self.float(b, r);
                let zero = b.ins().f64const(0.0);
                let v = b.ins().fcmp(FloatCC::NotEqual, f, zero);
                b.def_var(out, v);
                b.ins().jump(done, &[]);
                // A value that owns memory.
                b.switch_to_block(not_float);
                let owns = b.ins().icmp_imm_s(
                    IntCC::UnsignedGreaterThanOrEqual,
                    t,
                    i64::from(value::OWNS),
                );
                let plain = b.create_block();
                b.ins().brif(owns, fb, &[], plain, &[]);
                // Null, a bool or an int: null is false, and the other
                // two are their payload, which a bool keeps in a byte.
                b.switch_to_block(plain);
                let is_int = b.ins().icmp_imm_s(IntCC::Equal, t, i64::from(value::INT));
                let whole = self.bits(b, r);
                let byte = self.boolean(b, r);
                let payload = b.ins().select(is_int, whole, byte);
                let nonzero = b.ins().icmp_imm_s(IntCC::NotEqual, payload, 0);
                let is_null = b.ins().icmp_imm_s(IntCC::Equal, t, i64::from(value::NULL));
                let no = b.ins().iconst(types::I8, 0);
                let v = b.ins().select(is_null, no, nonzero);
                b.def_var(out, v);
                b.ins().jump(done, &[]);
                b.switch_to_block(done);
                b.use_var(out)
            }
        }
    }

    /// `x == y`, as `Value::equals` has it: two numbers compare as
    /// numbers, null and a bool only with their own kind, and a value
    /// that owns memory is never equal to one that does not. Two that
    /// own memory are the interpreter's.
    fn equal(&mut self, b: &mut FunctionBuilder, x: Opd, y: Opd, fb: CbBlock) -> CbValue {
        let no = |b: &mut FunctionBuilder| b.ins().iconst(types::I8, 0);
        match (x, y) {
            (Opd::Null, Opd::Null) => b.ins().iconst(types::I8, 1),
            (Opd::Null, Opd::Reg(r)) | (Opd::Reg(r), Opd::Null) => self.tag_is(b, r, value::NULL),
            (Opd::Bool(p), Opd::Bool(q)) => b.ins().icmp(IntCC::Equal, p, q),
            (Opd::Bool(p), Opd::Reg(r)) | (Opd::Reg(r), Opd::Bool(p)) => {
                let is_bool = self.tag_is(b, r, value::BOOL);
                let q = self.boolean(b, r);
                let same = b.ins().icmp(IntCC::Equal, p, q);
                b.ins().band(is_bool, same)
            }
            (Opd::Int(p), Opd::Int(q)) => b.ins().icmp(IntCC::Equal, p, q),
            (Opd::Int(_) | Opd::Float(_), Opd::Int(_) | Opd::Float(_)) => {
                let (p, q) = (self.float_of(b, x), self.float_of(b, y));
                b.ins().fcmp(FloatCC::Equal, p, q)
            }
            (Opd::Int(_) | Opd::Float(_), Opd::Reg(r))
            | (Opd::Reg(r), Opd::Int(_) | Opd::Float(_)) => {
                // A number and a register: equal only to a number.
                let known = if matches!(x, Opd::Reg(_)) { y } else { x };
                let out = b.declare_var(types::I8);
                let done = b.create_block();
                let number = b.create_block();
                let other = b.create_block();
                match self.is_number(b, Opd::Reg(r)) {
                    Cond::If(c) => {
                        b.ins().brif(c, number, &[], other, &[]);
                    }
                    _ => {
                        b.ins().jump(number, &[]);
                    }
                }
                b.switch_to_block(other);
                let f = no(b);
                b.def_var(out, f);
                b.ins().jump(done, &[]);
                b.switch_to_block(number);
                let v = match known {
                    Opd::Int(p) => {
                        // Two ints compare as ints; an int and a float
                        // as floats.
                        let is_int = self.tag_is(b, r, value::INT);
                        let q = self.bits(b, r);
                        let ints = b.ins().icmp(IntCC::Equal, p, q);
                        let pf = b.ins().fcvt_from_sint(types::F64, p);
                        let qf = self.float_of(b, Opd::Reg(r));
                        let floats = b.ins().fcmp(FloatCC::Equal, pf, qf);
                        b.ins().select(is_int, ints, floats)
                    }
                    _ => {
                        let (p, q) = (self.float_of(b, known), self.float_of(b, Opd::Reg(r)));
                        b.ins().fcmp(FloatCC::Equal, p, q)
                    }
                };
                b.def_var(out, v);
                b.ins().jump(done, &[]);
                b.switch_to_block(done);
                b.use_var(out)
            }
            (Opd::Reg(p), Opd::Reg(q)) => {
                let (tp, tq) = (self.tag(b, p), self.tag(b, q));
                let out = b.declare_var(types::I8);
                let done = b.create_block();
                // Either owns memory: the interpreter's.
                let top = b.ins().bor(tp, tq);
                let owns = b.ins().icmp_imm_s(
                    IntCC::UnsignedGreaterThanOrEqual,
                    top,
                    i64::from(value::OWNS),
                );
                let plain = b.create_block();
                b.ins().brif(owns, fb, &[], plain, &[]);
                b.switch_to_block(plain);
                // Two numbers.
                let pn = self.is_number(b, Opd::Reg(p));
                let qn = self.is_number(b, Opd::Reg(q));
                let numbers = both(b, pn, qn);
                let number = b.create_block();
                let other = b.create_block();
                if let Cond::If(c) = numbers {
                    b.ins().brif(c, number, &[], other, &[]);
                }
                b.switch_to_block(number);
                let pi = self.tag_is(b, p, value::INT);
                let qi = self.tag_is(b, q, value::INT);
                let ints_both = b.ins().band(pi, qi);
                let (pb, qb) = (self.bits(b, p), self.bits(b, q));
                let ints = b.ins().icmp(IntCC::Equal, pb, qb);
                let (pf, qf) = (self.float_of(b, Opd::Reg(p)), self.float_of(b, Opd::Reg(q)));
                let floats = b.ins().fcmp(FloatCC::Equal, pf, qf);
                let v = b.ins().select(ints_both, ints, floats);
                b.def_var(out, v);
                b.ins().jump(done, &[]);
                // Null and bools: equal when the tags are and, for two
                // bools, the bytes are.
                b.switch_to_block(other);
                let same_tag = b.ins().icmp(IntCC::Equal, tp, tq);
                let is_null = b.ins().icmp_imm_s(IntCC::Equal, tp, i64::from(value::NULL));
                let (pb, qb) = (self.boolean(b, p), self.boolean(b, q));
                let same_byte = b.ins().icmp(IntCC::Equal, pb, qb);
                let same_value = b.ins().bor(is_null, same_byte);
                let v = b.ins().band(same_tag, same_value);
                b.def_var(out, v);
                b.ins().jump(done, &[]);
                b.switch_to_block(done);
                b.use_var(out)
            }
            // Null or a bool against a number or against each other.
            _ => no(b),
        }
    }

    /// `x < y` (`x <= y` with `or_equal`) on two numbers: two ints as
    /// ints, otherwise as floats, where a NaN is the interpreter's
    /// error. Anything else is the interpreter's (two strings order;
    /// the rest raises).
    fn ordered(
        &mut self,
        b: &mut FunctionBuilder,
        x: Opd,
        y: Opd,
        or_equal: bool,
        fb: CbBlock,
    ) -> CbValue {
        let (icc, fcc) = if or_equal {
            (IntCC::SignedLessThanOrEqual, FloatCC::LessThanOrEqual)
        } else {
            (IntCC::SignedLessThan, FloatCC::LessThan)
        };
        let ints = {
            let p = self.is_int(b, x);
            let q = self.is_int(b, y);
            both(b, p, q)
        };
        if let Cond::Always = ints {
            let (p, q) = (self.int_of(b, x), self.int_of(b, y));
            return b.ins().icmp(icc, p, q);
        }
        let out = b.declare_var(types::I8);
        let done = b.create_block();
        let rest = b.create_block();
        match ints {
            Cond::If(c) => {
                let yes = b.create_block();
                b.ins().brif(c, yes, &[], rest, &[]);
                b.switch_to_block(yes);
                let (p, q) = (self.int_of(b, x), self.int_of(b, y));
                let v = b.ins().icmp(icc, p, q);
                b.def_var(out, v);
                b.ins().jump(done, &[]);
            }
            _ => {
                b.ins().jump(rest, &[]);
            }
        }
        b.switch_to_block(rest);
        let numbers = {
            let p = self.is_number(b, x);
            let q = self.is_number(b, y);
            both(b, p, q)
        };
        match numbers {
            Cond::Never => {
                b.ins().jump(fb, &[]);
            }
            numbers => {
                if let Cond::If(c) = numbers {
                    let yes = b.create_block();
                    b.ins().brif(c, yes, &[], fb, &[]);
                    b.switch_to_block(yes);
                }
                let (p, q) = (self.float_of(b, x), self.float_of(b, y));
                let nan = b.ins().fcmp(FloatCC::Unordered, p, q);
                let ok = b.create_block();
                b.ins().brif(nan, fb, &[], ok, &[]);
                b.switch_to_block(ok);
                let v = b.ins().fcmp(fcc, p, q);
                b.def_var(out, v);
                b.ins().jump(done, &[]);
            }
        }
        b.switch_to_block(done);
        b.use_var(out)
    }

    /// `testset A, B`: a truthy `R(B)` is copied to `R(A)` and the
    /// bytecode goes on; a falsy one jumps.
    fn testset(&mut self, b: &mut FunctionBuilder, d: &DecodedInstr) -> Result<(), String> {
        let (pc, next) = (d.pc, d.pc + d.width);
        let x = self.operand(b, pc, d.b_u);
        let fb = b.create_block();
        let join = b.create_block();
        let truth = self.truth(b, x, fb);
        let t = flow::target_of(d).ok_or("a jump before the function")?;
        let jump = self.edge(b, pc, t)?;
        let stays = b.create_block();
        b.ins().brif(truth, stays, &[], jump, &[]);
        b.switch_to_block(stays);
        if d.a != d.b_u {
            match x {
                Opd::Reg(src) => {
                    // `truth` has seen that it owns no memory. It is
                    // read before `R(A)` is made ready.
                    let tag = self.tag(b, src);
                    let bits = self.bits(b, src);
                    self.over(b, pc, d.a, fb);
                    let frame = b.use_var(self.frame);
                    let at = Self::slot(d.a);
                    b.ins().store(flags(), tag, frame, at + value::TAG as i32);
                    b.ins()
                        .store(flags(), bits, frame, at + value::PAYLOAD as i32);
                }
                v => self.put(b, pc, next, d.a, fb, v),
            }
        }
        let on = self.edge_into(b, pc, next, join);
        b.ins().jump(on, &[]);
        self.fall_back(b, d, fb, join)?;
        b.switch_to_block(join);
        Ok(())
    }

    /// The three ints of a counted loop's window, `R(A)`, `R(A+1)` and
    /// `R(A+2)`, with a step of 1: any other window is the
    /// interpreter's.
    fn window(
        &mut self,
        b: &mut FunctionBuilder,
        d: &DecodedInstr,
        fb: CbBlock,
    ) -> (CbValue, CbValue) {
        let mut ints = Vec::with_capacity(3);
        for r in d.a..d.a + 3 {
            let x = self.operand(b, d.pc, r);
            match self.is_int(b, x) {
                Cond::Always => {}
                Cond::If(c) => {
                    let ok = b.create_block();
                    b.ins().brif(c, ok, &[], fb, &[]);
                    b.switch_to_block(ok);
                }
                Cond::Never => {
                    let dead = b.create_block();
                    b.ins().jump(fb, &[]);
                    b.switch_to_block(dead);
                }
            }
            ints.push(self.int_of(b, x));
        }
        let one = b.ins().icmp_imm_s(IntCC::Equal, ints[2], 1);
        let ok = b.create_block();
        b.ins().brif(one, ok, &[], fb, &[]);
        b.switch_to_block(ok);
        (ints[0], ints[1])
    }

    /// The window is ints on this way on: hold what the next pc holds.
    fn hold_window(
        &mut self,
        b: &mut FunctionBuilder,
        d: &DecodedInstr,
        to: u32,
        start: CbValue,
        limit: CbValue,
    ) {
        let one = b.ins().iconst(types::I64, 1);
        for (r, v) in [(d.a, start), (d.a + 1, limit), (d.a + 2, one)] {
            if self.held_on(d.pc, to, r) == Some(Ty::Int) && self.held(d.pc, r).is_none() {
                let var = self.var(b, r, Ty::Int);
                b.def_var(var, v);
            }
        }
    }

    /// `forprep A`: no turn jumps; otherwise `R(A+3)` is the start. A
    /// range wider than the interpreter runs is its to refuse.
    fn forprep(&mut self, b: &mut FunctionBuilder, d: &DecodedInstr) -> Result<(), String> {
        let (pc, next) = (d.pc, d.pc + d.width);
        let fb = b.create_block();
        let join = b.create_block();
        let (start, limit) = self.window(b, d, fb);
        let t = flow::target_of(d).ok_or("a jump before the function")?;
        let none = b.ins().icmp(IntCC::SignedGreaterThanOrEqual, start, limit);
        let runs = b.create_block();
        let skip = self.edge(b, pc, t)?;
        b.ins().brif(none, skip, &[], runs, &[]);
        b.switch_to_block(runs);
        // `limit > start`: the width is their difference as an
        // unsigned number, whatever their signs.
        let width = b.ins().isub(limit, start);
        let wide = b
            .ins()
            .icmp_imm_s(IntCC::UnsignedGreaterThan, width, MAX_TURNS);
        let fits = b.create_block();
        b.ins().brif(wide, fb, &[], fits, &[]);
        b.switch_to_block(fits);
        self.over(b, pc, d.a + 3, fb);
        self.hold_window(b, d, next, start, limit);
        self.write(b, pc, next, d.a + 3, Opd::Int(start));
        let on = self.edge_into(b, pc, next, join);
        b.ins().jump(on, &[]);
        self.fall_back(b, d, fb, join)?;
        b.switch_to_block(join);
        Ok(())
    }

    /// `forloop A`: the next number, when there is one below the
    /// limit, goes to `R(A)` and `R(A+3)` and the loop jumps back.
    fn forloop(&mut self, b: &mut FunctionBuilder, d: &DecodedInstr) -> Result<(), String> {
        let (pc, next) = (d.pc, d.pc + d.width);
        let fb = b.create_block();
        let join = b.create_block();
        let (cur, limit) = self.window(b, d, fb);
        let t = flow::target_of(d).ok_or("a jump before the function")?;
        // The next number: past the largest int there is none, and the
        // loop ends.
        let (n, over) = {
            let one = b.ins().iconst(types::I64, 1);
            b.ins().sadd_overflow(cur, one)
        };
        let below = b.ins().icmp(IntCC::SignedLessThan, n, limit);
        let not_over = b.ins().bxor_imm_s(over, 1);
        let more = b.ins().band(below, not_over);
        let again = b.create_block();
        let on = self.edge_into(b, pc, next, join);
        b.ins().brif(more, again, &[], on, &[]);
        b.switch_to_block(again);
        // `R(A)` is one of the window's three ints (the form saw so);
        // `R(A+3)` is the loop's variable, which the script may have
        // given anything.
        self.over(b, pc, d.a + 3, fb);
        self.hold_window(b, d, t, n, limit);
        self.write(b, pc, t, d.a, Opd::Int(n));
        self.write(b, pc, t, d.a + 3, Opd::Int(n));
        let back = self.edge(b, pc, t)?;
        b.ins().jump(back, &[]);
        self.fall_back(b, d, fb, join)?;
        b.switch_to_block(join);
        Ok(())
    }

    /// `iternext A`: the next item of the list `iterprep` made of what
    /// the loop walks (`R(A+1)`), by the count in `R(A+2)`. When there
    /// is one and it owns no memory (a number, a bool, null) it is
    /// copied to `R(A+3)` and the count goes up; when there is none
    /// left the loop jumps out, with nothing written. An item that owns
    /// memory is the interpreter's to share.
    fn iternext(&mut self, b: &mut FunctionBuilder, d: &DecodedInstr) -> Result<(), String> {
        let (pc, next) = (d.pc, d.pc + d.width);
        let fb = b.create_block();
        let join = b.create_block();
        let t = flow::target_of(d).ok_or("a jump before the function")?;
        let x = self.operand(b, pc, d.a + 2);
        let at = self.index_of(b, x, fb);
        let (first, count) = self.view(b, d.a + 1, fb)?;
        // A count below zero is no item either: it is compared as an
        // unsigned number.
        let more = b.ins().icmp(IntCC::UnsignedLessThan, at, count);
        let have = b.create_block();
        let out = self.edge(b, pc, t)?;
        b.ins().brif(more, have, &[], out, &[]);
        b.switch_to_block(have);
        let offset = b.ins().imul_imm_s(at, value::SIZE as i64);
        let item = b.ins().iadd(first, offset);
        let tag = b.ins().load(types::I8, flags(), item, value::TAG as i32);
        let owns = b.ins().icmp_imm_s(
            IntCC::UnsignedGreaterThanOrEqual,
            tag,
            i64::from(value::OWNS),
        );
        let plain = b.create_block();
        b.ins().brif(owns, fb, &[], plain, &[]);
        b.switch_to_block(plain);
        let bits = b
            .ins()
            .load(types::I64, flags(), item, value::PAYLOAD as i32);
        self.over(b, pc, d.a + 3, fb);
        // The pass has the item as anything: it is in the frame.
        let frame = b.use_var(self.frame);
        let to = Self::slot(d.a + 3);
        b.ins().store(flags(), tag, frame, to + value::TAG as i32);
        b.ins()
            .store(flags(), bits, frame, to + value::PAYLOAD as i32);
        let n = b.ins().iadd_imm_s(at, 1);
        self.write(b, pc, next, d.a + 2, Opd::Int(n));
        let on = self.edge_into(b, pc, next, join);
        b.ins().jump(on, &[]);
        self.fall_back(b, d, fb, join)?;
        b.switch_to_block(join);
        Ok(())
    }

    /// `callm A, "push", 1`: a list's own `push` of one value. The
    /// runtime adds `R(A+1)` to the list in `R(A)` and leaves the list
    /// there, which is what the call gives. Adding an element may move
    /// them all and the value may own memory, so machine code does not
    /// do it where the list lies; it asks, and when `R(A)` holds no
    /// list the runtime can add to, the interpreter runs the call.
    fn push(&mut self, b: &mut FunctionBuilder, d: &DecodedInstr) -> Result<(), String> {
        let pc = d.pc;
        let fb = b.create_block();
        let join = b.create_block();
        // The runtime reads the value's register: it is in the frame.
        self.sync(b, pc);
        let list = b.ins().iconst(types::I32, i64::from(d.a));
        let value = b.ins().iconst(types::I32, i64::from(d.a + 1));
        let done = self.helper(b, HLP_PUSH, self.push_sig, &[list, value, list]);
        // The list's elements may have moved.
        self.forget_views(b);
        b.ins().brif(done, join, &[], fb, &[]);
        self.fall_back(b, d, fb, join)?;
        b.switch_to_block(join);
        Ok(())
    }
}

/// Whether `d` is a method call machine code has a form for: a `push`
/// of one value (`xs.push(v)`), which a list answers.
fn is_push(f: &BcFunction, d: &DecodedInstr) -> bool {
    d.op == Op::CALLM
        && d.c == 1
        && matches!(f.constants.get(d.b_u as usize), Some(Constant::Str(name)) if name == "push")
}

/// What a `for` loop's item is, as far as the loop's own instructions
/// say: how sure machine code can be that it owns no memory.
///
/// The item of `for x in xs` is whatever the list holds. An inline
/// `iternext` copies a number, a bool or null, and anything else is the
/// interpreter's; a loop whose items are strings or maps would fail
/// that test every turn. So the instructions after the `iternext` are
/// read, up to the jump back: an item (or a register it was moved to)
/// that is the object of a field or an index, or the receiver of a
/// method, or called, owns memory for certain; one that is added,
/// multiplied or ordered is most often a number.
fn item_class(code: &[Option<DecodedInstr>], d: &DecodedInstr) -> Class {
    let mut names: BTreeSet<u32> = BTreeSet::from([d.a + 3]);
    let mut number = false;
    let mut pc = d.pc + d.width;
    for _ in 0..64 {
        let Some(Some(i)) = code.get(pc as usize) else {
            break;
        };
        let is = |r: u32| names.contains(&r);
        match i.op {
            Op::GETFIELD | Op::GETINDEX if is(i.b_u) => return Class::Interp,
            Op::SETFIELD | Op::SETINDEX | Op::CALLM | Op::CALL | Op::TAILCALL if is(i.a) => {
                return Class::Interp
            }
            Op::SUB | Op::MUL | Op::DIV | Op::MOD | Op::LT | Op::LE if is(i.b_u) || is(i.c) => {
                number = true
            }
            Op::SUBK | Op::MULK | Op::DIVK | Op::MODK | Op::LTK | Op::LEK | Op::NEG if is(i.b_u) => {
                number = true
            }
            Op::LTJ | Op::LEJ if is(i.a) || is(i.b_u) => number = true,
            _ => {}
        }
        // A register written is no longer the item, unless the item
        // was moved to it.
        let moved = i.op == Op::MOV && is(i.b_u);
        for (r, _) in typed::writes(i).regs {
            match moved && r == i.a {
                true => names.insert(r),
                false => names.remove(&r),
            };
        }
        // The jump back ends the turn.
        if names.is_empty() || flow::target_of(i).is_some_and(|t| t <= d.pc) {
            break;
        }
        pc += i.width;
    }
    Class::Index(if number { 95 } else { 80 })
}

/// What machine code can do with the instruction `d` ([`Class`]): which
/// of them have an inline form in [`Emit::inline`], and whether the
/// types the pass proved cover what that form tests when it runs. It
/// reads the pass's types whatever mode the body is built in, so one
/// function has one plan.
///
/// The estimates beside a guess are of scripts as they are written: a
/// register written over usually held a number or nothing before; a
/// value of unknown type that is added or ordered is usually a number;
/// one that is moved or compared for equality is more often a string, a
/// list or a map. They lean towards the guess holding: the runtime
/// gives up a body whose guesses keep failing, so a wrong estimate
/// costs microseconds, where a guess not taken costs the loop.
fn class_of(
    f: &BcFunction,
    flow: &Flow,
    types: &Types,
    code: &[Option<DecodedInstr>],
    d: &DecodedInstr,
) -> Class {
    match d.op {
        Op::CALL => return Class::Call,
        Op::RET | Op::TAILCALL => return Class::Exit,
        _ => {}
    }
    // A register that can carry a cell is read and written through it.
    let cell = |r: u32| flow.cells.contains(&r);
    if typed::reads(d).iter().any(|r| cell(*r))
        || typed::writes(d).regs.iter().any(|(r, _)| cell(*r))
    {
        return Class::Interp;
    }
    let before = types.before(d.pc);
    let ty = |r: u32| match before.get(r as usize).copied() {
        None | Some(Ty::None) => Ty::Any,
        Some(t) => t,
    };
    let konst = |i: u32| match f.constants.get(i as usize) {
        Some(Constant::Bool(_)) => Some(Ty::Bool),
        Some(Constant::Int(_)) => Some(Ty::Int),
        Some(Constant::Float(_)) => Some(Ty::Float),
        _ => None,
    };
    // A write is plain when what the register held is proven to own no
    // memory, and tests for that otherwise.
    let dst = |r: u32| match tag_of(ty(r)) {
        Some(_) => Class::Sure,
        None => Class::Guess(97),
    };
    let number = |t: Ty| matches!(t, Ty::Int | Ty::Float);
    let no_number = |t: Ty| matches!(t, Ty::Null | Ty::Bool);
    match d.op {
        Op::NOP | Op::JMP | Op::LOOP | Op::STEPS => Class::Sure,
        Op::LOADNULL | Op::LOADBOOL | Op::LOADINT => dst(d.a),
        Op::LOADK => match konst(d.b_u) {
            Some(_) => dst(d.a),
            None => Class::Interp,
        },
        Op::MOV if d.a == d.b_u => Class::Sure,
        Op::MOV => match ty(d.b_u) {
            Ty::Any => Class::Guess(70),
            _ => dst(d.a),
        },
        Op::ADD
        | Op::SUB
        | Op::MUL
        | Op::DIV
        | Op::MOD
        | Op::ADDK
        | Op::SUBK
        | Op::MULK
        | Op::DIVK
        | Op::MODK => {
            let x = ty(d.b_u);
            let y = match d.op {
                Op::ADD | Op::SUB | Op::MUL | Op::DIV | Op::MOD => ty(d.c),
                _ => match konst(d.c) {
                    Some(t) => t,
                    None => return Class::Interp,
                },
            };
            let rem = matches!(d.op, Op::MOD | Op::MODK);
            if no_number(x) || no_number(y) {
                // The interpreter's error, or its string.
                Class::Interp
            } else if rem && (x == Ty::Float || y == Ty::Float) {
                Class::Interp
            } else if number(x) && number(y) {
                dst(d.a)
            } else {
                Class::Guess(90)
            }
        }
        Op::NEG => match ty(d.b_u) {
            t if number(t) => dst(d.a),
            Ty::Any => Class::Guess(90),
            _ => Class::Interp,
        },
        Op::NOT => match ty(d.b_u) {
            Ty::Any => Class::Guess(90),
            _ => dst(d.a),
        },
        Op::EQ | Op::EQK => {
            let y = match d.op {
                Op::EQ => ty(d.c),
                _ => match konst(d.c) {
                    Some(t) => t,
                    None => return Class::Interp,
                },
            };
            // Only two values of unknown type can both own memory.
            match (ty(d.b_u), y) {
                (Ty::Any, Ty::Any) => Class::Guess(80),
                _ => dst(d.a),
            }
        }
        Op::LT | Op::LE | Op::LTK | Op::LEK => {
            let x = ty(d.b_u);
            let y = match d.op {
                Op::LT | Op::LE => ty(d.c),
                _ => match konst(d.c) {
                    Some(t) => t,
                    None => return Class::Interp,
                },
            };
            if no_number(x) || no_number(y) {
                Class::Interp
            } else if number(x) && number(y) {
                dst(d.a)
            } else {
                Class::Guess(90)
            }
        }
        Op::EQJ | Op::NEJ => match (ty(d.a), ty(d.b_u)) {
            (Ty::Any, Ty::Any) => Class::Guess(80),
            _ => Class::Sure,
        },
        Op::LTJ | Op::LEJ => {
            let (x, y) = (ty(d.a), ty(d.b_u));
            if no_number(x) || no_number(y) {
                Class::Interp
            } else if number(x) && number(y) {
                Class::Sure
            } else {
                Class::Guess(90)
            }
        }
        Op::TEST | Op::TESTN => match ty(d.a) {
            Ty::Any => Class::Guess(90),
            _ => Class::Sure,
        },
        Op::TESTSET => match ty(d.b_u) {
            Ty::Any => Class::Guess(80),
            _ if d.a == d.b_u => Class::Sure,
            _ => dst(d.a),
        },
        Op::FORPREP | Op::FORLOOP => {
            let last = d.a.saturating_add(3);
            if cell(last) {
                Class::Interp
            } else if (d.a..last).all(|r| ty(r) == Ty::Int) {
                dst(last)
            } else {
                Class::Guess(90)
            }
        }
        // A list's element, by an int: a proven int index is most often
        // into a list of numbers; an index of unknown type is as often
        // a map's key.
        Op::GETINDEX => match ty(d.c) {
            Ty::Int => Class::Index(90),
            Ty::Any => Class::Index(60),
            _ => Class::Interp,
        },
        Op::SETINDEX => match (ty(d.b_u), ty(d.c)) {
            (Ty::Null | Ty::Bool | Ty::Float, _) => Class::Interp,
            (Ty::Int, Ty::Any) => Class::Index(80),
            (Ty::Int, _) => Class::Index(90),
            (_, Ty::Any) => Class::Index(50),
            (_, _) => Class::Index(60),
        },
        // The next item of a `for` loop over a list.
        Op::ITERNEXT => item_class(code, d),
        // `xs.push(v)`: the runtime's own function for it.
        Op::CALLM if is_push(f, d) => Class::Push,
        _ => Class::Interp,
    }
}

fn known(t: Ty, v: CbValue) -> Opd {
    match t {
        Ty::Float => Opd::Float(v),
        Ty::Bool => Opd::Bool(v),
        _ => Opd::Int(v),
    }
}

fn both(b: &mut FunctionBuilder, x: Cond, y: Cond) -> Cond {
    match (x, y) {
        (Cond::Never, _) | (_, Cond::Never) => Cond::Never,
        (Cond::Always, c) | (c, Cond::Always) => c,
        (Cond::If(p), Cond::If(q)) => Cond::If(b.ins().band(p, q)),
    }
}
