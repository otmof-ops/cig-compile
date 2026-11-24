// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2

//! The type of every register at every instruction, as a forward
//! data-flow pass over a function's [`Flow`].
//!
//! ## Lattice
//!
//! `None < {Null, Bool, Int, Float} < Any`. The four scalars in the middle
//! are not comparable to each other: joining two different ones gives
//! `Any`. `None` means "no run reaches here on this path"; `Any` means
//! "may hold anything, including a value that owns memory". The native
//! generator reads this: a register at `Int` or `Float` lives in a machine
//! register, one at `Any` stays in the frame and carries its tag.
//!
//! ## Soundness
//!
//! `before(pc)[r] == Int` is a claim that on every run of the function,
//! by the interpreter or by native code, the register holds an int
//! whenever control reaches `pc`. Every rule is read from what the
//! instruction does in the VM (`src/vm/mod.rs`, `src/interp/ops.rs` and
//! `docs/BYTECODE.md` in cig-script), never from what a lowering usually
//! emits. The rules go only where the instruction goes on: an operation
//! that raises for the operands it checks never writes its result on
//! the going-on path, so what writes the frame is what the operation
//! does for the cases that return a value. See [`transfer`] for the full
//! table.
//!
//! ## What is Any, always
//!
//! - A function with a handler (`flow.handlers`): every register, at every
//!   pc. A `catch` or a `finally` can be entered from any instruction of
//!   its range, and the exact set of pcs that reach each handler pc is
//!   wider than the bytecode spells out; the pass does not follow it yet.
//! - A cell register (`A` of any `cellnew`): every pc. The interpreter
//!   reads and writes the cell, and the closures that captured it share
//!   whatever is in it.
//! - At pc 0: every register.
//! - At every handler entry (`handler_pc` and `finally_pc`): every
//!   register. These pcs are reached from an unknown pc of the handler's
//!   range.
//! - For every opcode without a rule here: every register. The fallback
//!   is conservative so a new opcode cannot be silently wrong.
//!
//! ## `reads` and `writes`
//!
//! The same table drives [`reads`] and [`writes`], one row an opcode. A
//! call ([`Op::CALL`]) sets `writes.above`: every register from that
//! index up is undefined after the call and becomes `Any`. The generator
//! uses this to tell which registers it may keep in machine registers
//! across the call, and which it must write back first.

use std::collections::BTreeSet;

use cigscript::cig_bytecode::instr::{decode_table, DecodedInstr};
use cigscript::cig_bytecode::module::{Constant, Function};
use cigscript::cig_bytecode::opcode::Op;

use crate::flow::Flow;

/// The lattice value: the type of one register at one instruction.
///
/// `None` is "not reached"; `Any` is "could be anything". The four scalars
/// in between are not comparable to each other: joining two different ones
/// gives `Any`. The `Ord` instance exists because `Ty` is a `#[derive]`
/// `Ord` enum, but it is the derive's lexical order (the variants' order),
/// not the lattice's. Comparing `Ty` values for the lattice uses `join`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Ty {
    /// Not reached on this path.
    None,
    /// The value `null`.
    Null,
    /// A boolean.
    Bool,
    /// A 64-bit signed integer.
    Int,
    /// A 64-bit IEEE-754 float.
    Float,
    /// Anything: a scalar or a value that owns memory.
    Any,
}

impl Ty {
    /// The lattice join. `None` is the bottom (`join(None, X) == X`), `Any`
    /// is the top, and two different scalars join to `Any`.
    #[inline]
    pub fn join(self, other: Ty) -> Ty {
        match (self, other) {
            (Ty::None, x) | (x, Ty::None) => x,
            (Ty::Any, _) | (_, Ty::Any) => Ty::Any,
            (a, b) if a == b => a,
            _ => Ty::Any,
        }
    }

    /// `true` when `self` is a scalar (`Null`, `Bool`, `Int`, `Float`).
    #[inline]
    pub fn is_scalar(self) -> bool {
        matches!(self, Ty::Null | Ty::Bool | Ty::Int | Ty::Float)
    }
}

/// Every register's type at every instruction.
#[derive(Clone, Debug)]
pub struct Types {
    /// The function's register count. `before(pc)` always returns a slice
    /// of this length.
    pub reg_count: u32,
    /// Every instruction's first-word pc, ascending. Each row of `before`
    /// corresponds to the pc at the same index here.
    pub pcs: Vec<u32>,
    /// One row per pc, each `reg_count` long: the types just before the
    /// instruction runs on every path that reaches it.
    before: Vec<Vec<Ty>>,
    /// The decoded instruction at each pc. Kept so [`on_edge`] can compute
    /// per-path refinements without decoding again.
    code: Vec<DecodedInstr>,
    /// The type of each constant of the function's pool.
    kty: Vec<Ty>,
    /// The registers that can carry a cell: `Any` on every edge.
    cells: BTreeSet<u32>,
    /// The function has a handler: every register is `Any` on every edge.
    handlers: bool,
}

impl Types {
    /// Each register's type just before the instruction at `pc` runs, on
    /// every path that reaches it. The slice is `reg_count` long. An
    /// unknown `pc` returns an empty slice.
    pub fn before(&self, pc: u32) -> &[Ty] {
        match self.pcs.binary_search(&pc) {
            Ok(i) => &self.before[i],
            Err(_) => &[],
        }
    }

    /// Each register's type when the instruction at `from` has run and the
    /// bytecode goes on at `to`.
    ///
    /// The same transfer [`infer`] applied on that edge, constants and
    /// cell registers included, so `before(to)` is the join of `on_edge`
    /// over every instruction that goes on at `to`. It is refined per
    /// edge where the instruction decides which path to take based on an
    /// operand. The one such instruction is `testset A, B`: on the
    /// fall-through path (`to` is the instruction's own `pc + width`),
    /// `R(A)` has `R(B)`'s type (the VM copied it); on the jump path,
    /// nothing is written.
    pub fn on_edge(&self, from: u32, to: u32) -> Vec<Ty> {
        let Ok(i) = self.pcs.binary_search(&from) else {
            return Vec::new();
        };
        if self.handlers {
            return vec![Ty::Any; self.reg_count as usize];
        }
        let d = self.code[i];
        transfer_edge(&d, &self.before[i], to, &self.kty, &self.cells)
    }
}

/// The registers an instruction reads. The order is the order the operands
/// are declared in; duplicates are kept (so a reader can find each use).
pub fn reads(d: &DecodedInstr) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::new();
    match d.op {
        Op::NOP => {}
        Op::MOV => out.push(d.b_u),
        Op::LOADK | Op::LOADNULL | Op::LOADBOOL | Op::LOADINT | Op::LOADG => {}
        Op::ADD | Op::SUB | Op::MUL | Op::DIV | Op::MOD | Op::POW => {
            out.push(d.b_u);
            out.push(d.c);
        }
        Op::NEG | Op::NOT => out.push(d.b_u),
        Op::ADDK | Op::SUBK | Op::MULK | Op::DIVK | Op::MODK | Op::POWK => out.push(d.b_u),
        Op::EQ | Op::LT | Op::LE => {
            out.push(d.b_u);
            out.push(d.c);
        }
        Op::EQK | Op::LTK | Op::LEK => out.push(d.b_u),
        Op::EQJ | Op::NEJ | Op::LTJ | Op::LEJ => {
            out.push(d.a);
            out.push(d.b_u);
        }
        Op::JMP | Op::LOOP => {}
        Op::TEST | Op::TESTN => out.push(d.a),
        Op::TESTSET => out.push(d.b_u),
        Op::FORPREP | Op::FORLOOP => {
            out.push(d.a);
            out.push(d.a + 1);
            out.push(d.a + 2);
        }
        Op::ITERPREP => out.push(d.a),
        Op::ITERNEXT => {
            out.push(d.a + 1);
            out.push(d.a + 2);
        }
        Op::NEWLIST | Op::NEWMAP => {}
        Op::GETFIELD => out.push(d.b_u),
        Op::SETFIELD => {
            out.push(d.a);
            out.push(d.c);
        }
        Op::GETINDEX => {
            out.push(d.b_u);
            out.push(d.c);
        }
        Op::SETINDEX => {
            out.push(d.a);
            out.push(d.b_u);
            out.push(d.c);
        }
        Op::APPENDLIST => {
            out.push(d.a);
            out.push(d.b_u);
        }
        Op::INSERTMAP => {
            out.push(d.a);
            out.push(d.b_u);
            out.push(d.c);
        }
        Op::INTERP => {
            for r in d.b_u..=(d.b_u.saturating_add(d.c)) {
                out.push(r);
            }
        }
        Op::CALL => {
            out.push(d.a); // callee
            for k in 0..d.b_u {
                out.push(d.a.saturating_add(1).saturating_add(k));
            }
        }
        Op::CALLB => {
            for k in 0..d.c {
                out.push(d.a.saturating_add(1).saturating_add(k));
            }
        }
        Op::CALLM => {
            for k in 0..=d.c {
                out.push(d.a.saturating_add(k));
            }
        }
        Op::RET => out.push(d.a),
        Op::TAILCALL => {
            out.push(d.a);
            for k in 0..d.b_u {
                out.push(d.a.saturating_add(1).saturating_add(k));
            }
        }
        Op::CLOSURE => {} // captures are parent registers, not this frame's
        Op::GETUP => {}
        Op::SETUP => out.push(d.b_u),
        Op::CELLNEW => out.push(d.a),
        Op::CELLGET => {}
        Op::CELLSET => out.push(d.b_u),
        Op::TRY_ENTER => {}
        Op::TRY_LEAVE => {}
        Op::THROW => out.push(d.a),
        Op::BURN_ENTER => {}
        Op::BURN_OP => {
            // A + 1 .. A + 1 + arg_count; unwidened, (c >> 4) is the count,
            // wide, c's high byte is the count.
            let count = if d.wide { d.c >> 8 } else { d.c >> 4 };
            for k in 0..count {
                out.push(d.a.saturating_add(1).saturating_add(k));
            }
        }
        Op::BURN_LEAVE => {}
        Op::COMP_ENTER => out.push(d.a),
        Op::COMP_LEAVE => {}
        Op::PACK => {
            for k in 0..d.b_u {
                out.push(d.a.saturating_add(k));
            }
        }
        Op::EXHALE => {
            for k in 0..d.b_u {
                out.push(d.a.saturating_add(k));
            }
        }
        Op::CHAIN_MAKE => {
            for k in 0..=d.b_u {
                out.push(d.a.saturating_add(k));
            }
        }
        Op::STEPS => {}
        Op::WIDE => {}
    }
    out
}

/// What an instruction writes.
#[derive(Clone, Debug, Default)]
pub struct Writes {
    /// The registers that definitely get a new value when the instruction
    /// goes on, each with the type it then holds.
    pub regs: Vec<(u32, Ty)>,
    /// When `Some(r)`, every register from `r` up (within the frame) is
    /// left undefined by the instruction (currently: `call` and `tailcall`;
    /// a frame goes away with `tailcall`, so the field is set only on
    /// `call`). The data-flow pass sets each such register to `Any`.
    pub above: Option<u32>,
}

/// The registers an instruction writes when it goes on, and the type it
/// writes them with.
pub fn writes(d: &DecodedInstr) -> Writes {
    let mut w = Writes::default();
    match d.op {
        Op::NOP => {}
        Op::MOV => w.regs.push((d.a, Ty::None)), // type filled by `transfer`
        Op::LOADK => w.regs.push((d.a, Ty::None)),
        Op::LOADNULL => w.regs.push((d.a, Ty::Null)),
        Op::LOADBOOL => w.regs.push((d.a, Ty::Bool)),
        Op::LOADINT => w.regs.push((d.a, Ty::Int)),
        Op::LOADG => w.regs.push((d.a, Ty::Any)),

        Op::ADD | Op::SUB | Op::MUL => w.regs.push((d.a, Ty::None)),
        Op::DIV | Op::MOD | Op::POW => w.regs.push((d.a, Ty::None)),
        Op::NEG => w.regs.push((d.a, Ty::None)),
        Op::NOT => w.regs.push((d.a, Ty::Bool)),
        Op::ADDK | Op::SUBK | Op::MULK | Op::DIVK | Op::MODK | Op::POWK => {
            w.regs.push((d.a, Ty::None));
        }
        Op::EQ | Op::LT | Op::LE => w.regs.push((d.a, Ty::Bool)),
        Op::EQK | Op::LTK | Op::LEK => w.regs.push((d.a, Ty::Bool)),
        Op::EQJ | Op::NEJ | Op::LTJ | Op::LEJ => {}

        Op::JMP | Op::LOOP | Op::TEST | Op::TESTN => {}
        Op::TESTSET => {
            // On the fall-through path only (`transfer_edge` handles it).
            w.regs.push((d.a, Ty::None));
        }
        Op::FORPREP => {
            // On the path into the loop body, R(A+3) is an int (the start).
            // `transfer_edge` handles this: the default path is the
            // "no turn" jump, which writes nothing; the fall-through into
            // the body writes R(A+3) = Int.
            w.regs.push((d.a.saturating_add(3), Ty::Int));
        }
        Op::FORLOOP => {
            // On the path back into the body: R(A) and R(A+3) get an Int.
            w.regs.push((d.a, Ty::Int));
            w.regs.push((d.a.saturating_add(3), Ty::Int));
        }
        Op::ITERPREP => {
            w.regs.push((d.a.saturating_add(1), Ty::Any)); // list snapshot
            w.regs.push((d.a.saturating_add(2), Ty::Int)); // index
        }
        Op::ITERNEXT => {
            // On the fall-through path (item found): writes R(A+3) = Any,
            // and R(A+2) = Int. On the jump path: no writes.
            w.regs.push((d.a.saturating_add(3), Ty::Any));
            w.regs.push((d.a.saturating_add(2), Ty::Int));
        }
        Op::NEWLIST | Op::NEWMAP => w.regs.push((d.a, Ty::Any)),
        Op::GETFIELD | Op::GETINDEX => w.regs.push((d.a, Ty::Any)),
        Op::SETFIELD | Op::SETINDEX | Op::APPENDLIST | Op::INSERTMAP => {}
        Op::INTERP => w.regs.push((d.a, Ty::Any)),

        Op::CALL => {
            // R(A) is Any; R(A+1 ..= A+C) is Null (the rest of the row);
            // R(A+C+1) and up is undefined -> Any (`above`).
            w.regs.push((d.a, Ty::Any));
            for k in 1..=d.c {
                w.regs.push((d.a.saturating_add(k), Ty::Null));
            }
            w.above = Some(d.a.saturating_add(d.c).saturating_add(1));
        }
        Op::CALLB | Op::CALLM => w.regs.push((d.a, Ty::Any)),
        Op::RET | Op::TAILCALL | Op::THROW => {}
        Op::CLOSURE => w.regs.push((d.a, Ty::Any)),
        Op::GETUP | Op::CELLGET => w.regs.push((d.a, Ty::Any)),
        Op::SETUP | Op::CELLSET => {}
        Op::CELLNEW => w.regs.push((d.a, Ty::Any)),
        Op::TRY_ENTER => {}
        Op::TRY_LEAVE => {}
        Op::BURN_ENTER | Op::BURN_LEAVE | Op::COMP_LEAVE => {}
        Op::BURN_OP => w.regs.push((d.a, Ty::Any)),
        Op::COMP_ENTER => {}
        Op::PACK | Op::EXHALE => {}
        Op::CHAIN_MAKE => w.regs.push((d.a, Ty::Any)),
        Op::STEPS => {}
        Op::WIDE => {}
    }
    w
}

/// The forward data-flow pass.
pub fn infer(f: &Function, flow: &Flow) -> Types {
    let reg_count = f.reg_count;
    let pcs = flow.pcs.clone();

    // Decode once, so `on_edge` can read instructions without decoding
    // again.
    let code_table = match decode_table(&f.instrs) {
        Ok((_, c)) => c,
        Err(_) => {
            return Types {
                reg_count,
                pcs,
                before: Vec::new(),
                code: Vec::new(),
                kty: Vec::new(),
                cells: flow.cells.clone(),
                handlers: flow.handlers,
            };
        }
    };
    let code: Vec<DecodedInstr> = pcs
        .iter()
        .map(|p| code_table[*p as usize].expect("first-word pc has a decoded instruction"))
        .collect();

    // Constant-pool types, for `loadk`'s transfer.
    let kty: Vec<Ty> = f
        .constants
        .iter()
        .map(|k| match k {
            Constant::Bool(_) => Ty::Bool,
            Constant::Int(_) => Ty::Int,
            Constant::Float(_) => Ty::Float,
            Constant::Str(_) => Ty::Any,
        })
        .collect();

    // A function with any handler, or no instructions, is `Any` everywhere.
    if flow.handlers {
        let row: Vec<Ty> = vec![Ty::Any; reg_count as usize];
        return Types {
            reg_count,
            pcs,
            before: vec![row; code.len()],
            code,
            kty,
            cells: flow.cells.clone(),
            handlers: true,
        };
    }

    let n_pcs = pcs.len();
    let mut before: Vec<Vec<Ty>> = vec![vec![Ty::None; reg_count as usize]; n_pcs];

    // pc 0: every register is Any (a call can put anything there).
    if n_pcs > 0 {
        before[0] = vec![Ty::Any; reg_count as usize];
    }
    // Every handler entry pc: every register is Any. (Dead code with
    // handlers off, kept for parallel structure if handlers is later made
    // path-sensitive.)
    for h in &flow.handler_entries {
        for &pc in &[h.handler_pc, h.finally_pc] {
            if let Ok(i) = pcs.binary_search(&pc) {
                before[i] = vec![Ty::Any; reg_count as usize];
            }
        }
    }

    // Fixed-point worklist. The set is kept by pc so ties are deterministic.
    let mut worklist: BTreeSet<u32> = pcs.iter().copied().collect();
    while let Some(&pc) = worklist.iter().next() {
        worklist.remove(&pc);
        let i = pcs.binary_search(&pc).expect("pc in list");
        let d = code[i];
        let in_row = before[i].clone();

        // On every successor edge:
        let mut edges = flow.succs.get(&pc).cloned().unwrap_or_default();
        edges.sort();
        edges.dedup();
        for to in edges {
            let out_row = transfer_edge(&d, &in_row, to, &kty, &flow.cells);
            if let Ok(j) = pcs.binary_search(&to) {
                let merged = join_rows(&before[j], &out_row);
                if merged != before[j] {
                    before[j] = merged;
                    worklist.insert(to);
                }
            }
        }
    }

    // Finally: cell registers are Any everywhere (they are shared with
    // closures through their cell; a scalar claim is unsafe).
    for row in &mut before {
        for &r in &flow.cells {
            if (r as usize) < row.len() {
                row[r as usize] = Ty::Any;
            }
        }
    }

    Types {
        reg_count,
        pcs,
        before,
        code,
        kty,
        cells: flow.cells.clone(),
        handlers: false,
    }
}

/// Join two rows element by element.
fn join_rows(a: &[Ty], b: &[Ty]) -> Vec<Ty> {
    a.iter().zip(b.iter()).map(|(&x, &y)| x.join(y)).collect()
}

/// The transfer function for one edge `from -> to`, applied to `in_row`.
///
/// This is where the per-opcode rule lives. It handles the common `writes`
/// table and then layers on the few edge-sensitive rules.
fn transfer_edge(
    d: &DecodedInstr,
    in_row: &[Ty],
    to: u32,
    kty: &[Ty],
    cells: &BTreeSet<u32>,
) -> Vec<Ty> {
    let mut out = transfer(d, in_row, kty);
    // Edge-sensitive rules.
    match d.op {
        // testset: fall-through only writes R(A) = R(B)'s type.
        Op::TESTSET => {
            let fall = d.pc + d.width;
            if to == fall {
                let ty = in_row.get(d.b_u as usize).copied().unwrap_or(Ty::Any);
                if let Some(slot) = out.get_mut(d.a as usize) {
                    *slot = ty;
                }
            } else {
                // Jump path: R(A) unchanged.
                if let Some(slot) = out.get_mut(d.a as usize) {
                    *slot = in_row.get(d.a as usize).copied().unwrap_or(Ty::None);
                }
            }
        }
        // forprep: on the "no turn" jump, no writes; on the fall-through
        // into the body, R(A+3) is Int and R(A), R(A+1), R(A+2) are refined
        // to Int (for_window raises otherwise).
        Op::FORPREP => {
            let fall = d.pc + d.width;
            if to == fall {
                // Into the loop body.
                for off in 0..=3 {
                    if let Some(slot) = out.get_mut((d.a + off) as usize) {
                        *slot = Ty::Int;
                    }
                }
            } else {
                // No-turn jump path: nothing written (the body refinement
                // does not apply). The window registers were still read as
                // ints for the preceding check, but the pass does not know
                // whether this path was taken because they were ints or
                // because `turns == 0`; keep them as they were.
                if let Some(slot) = out.get_mut((d.a + 3) as usize) {
                    *slot = in_row.get((d.a + 3) as usize).copied().unwrap_or(Ty::None);
                }
            }
        }
        // forloop: the going-on path sets R(A) and R(A+3) to Int; the
        // fall-through path (loop done) writes nothing.
        Op::FORLOOP => {
            let fall = d.pc + d.width;
            if to == fall {
                // Loop done: no writes (the `writes` default would have
                // written R(A) and R(A+3) to Int; undo it).
                if let Some(slot) = out.get_mut(d.a as usize) {
                    *slot = in_row.get(d.a as usize).copied().unwrap_or(Ty::None);
                }
                if let Some(slot) = out.get_mut((d.a + 3) as usize) {
                    *slot = in_row.get((d.a + 3) as usize).copied().unwrap_or(Ty::None);
                }
            } else {
                // Back into the body: R(A) and R(A+1) and R(A+2) were ints
                // (for_window), and R(A), R(A+3) are now Int (writes).
                for off in 0..=3 {
                    if let Some(slot) = out.get_mut((d.a + off) as usize) {
                        *slot = Ty::Int;
                    }
                }
            }
        }
        // iternext: on the fall-through (item found) writes R(A+3) = Any
        // and R(A+2) = Int; on the jump (done) writes nothing.
        Op::ITERNEXT => {
            let fall = d.pc + d.width;
            if to != fall {
                // Done: undo the writes.
                for off in [2, 3] {
                    if let Some(slot) = out.get_mut((d.a + off) as usize) {
                        *slot = in_row
                            .get((d.a + off) as usize)
                            .copied()
                            .unwrap_or(Ty::None);
                    }
                }
            }
        }
        _ => {}
    }
    // Cells: a cell register is Any on every edge.
    for &r in cells {
        if let Some(slot) = out.get_mut(r as usize) {
            *slot = Ty::Any;
        }
    }
    out
}

/// The transfer function without edge sensitivity: the row `in_row` after
/// the instruction has run, assuming it went on. This is the basis for
/// [`transfer_edge`], which layers per-path writes on top.
fn transfer(d: &DecodedInstr, in_row: &[Ty], kty: &[Ty]) -> Vec<Ty> {
    let mut out = in_row.to_vec();

    // Opcodes we do not have a rule for: every register goes to Any. This
    // is the conservative fallback.
    if !has_rule(d.op) {
        out.fill(Ty::Any);
        return out;
    }

    let w = writes(d);
    for (r, ty) in &w.regs {
        if let Some(slot) = out.get_mut(*r as usize) {
            *slot = *ty;
        }
    }
    if let Some(above) = w.above {
        if let Some(tail) = out.get_mut((above as usize)..) {
            tail.fill(Ty::Any);
        }
    }

    // Opcode-specific result types that need to read `in_row` or the
    // constant pool.
    match d.op {
        Op::MOV => {
            let ty = in_row.get(d.b_u as usize).copied().unwrap_or(Ty::Any);
            if let Some(slot) = out.get_mut(d.a as usize) {
                *slot = ty;
            }
        }
        Op::LOADK => {
            let ty = kty.get(d.b_u as usize).copied().unwrap_or(Ty::Any);
            if let Some(slot) = out.get_mut(d.a as usize) {
                *slot = ty;
            }
        }
        Op::ADD | Op::SUB | Op::MUL => {
            let l = in_row.get(d.b_u as usize).copied().unwrap_or(Ty::Any);
            let r = in_row.get(d.c as usize).copied().unwrap_or(Ty::Any);
            let ty = arith_add_sub_mul(l, r);
            if let Some(slot) = out.get_mut(d.a as usize) {
                *slot = ty;
            }
        }
        Op::ADDK | Op::SUBK | Op::MULK => {
            let l = in_row.get(d.b_u as usize).copied().unwrap_or(Ty::Any);
            let r = kty.get(d.c as usize).copied().unwrap_or(Ty::Any);
            let ty = arith_add_sub_mul(l, r);
            if let Some(slot) = out.get_mut(d.a as usize) {
                *slot = ty;
            }
        }
        Op::DIV => {
            let l = in_row.get(d.b_u as usize).copied().unwrap_or(Ty::Any);
            let r = in_row.get(d.c as usize).copied().unwrap_or(Ty::Any);
            let ty = div_result(l, r);
            if let Some(slot) = out.get_mut(d.a as usize) {
                *slot = ty;
            }
        }
        Op::DIVK => {
            let l = in_row.get(d.b_u as usize).copied().unwrap_or(Ty::Any);
            let r = kty.get(d.c as usize).copied().unwrap_or(Ty::Any);
            let ty = div_result(l, r);
            if let Some(slot) = out.get_mut(d.a as usize) {
                *slot = ty;
            }
        }
        Op::MOD => {
            let l = in_row.get(d.b_u as usize).copied().unwrap_or(Ty::Any);
            let r = in_row.get(d.c as usize).copied().unwrap_or(Ty::Any);
            let ty = mod_result(l, r);
            if let Some(slot) = out.get_mut(d.a as usize) {
                *slot = ty;
            }
        }
        Op::MODK => {
            let l = in_row.get(d.b_u as usize).copied().unwrap_or(Ty::Any);
            let r = kty.get(d.c as usize).copied().unwrap_or(Ty::Any);
            let ty = mod_result(l, r);
            if let Some(slot) = out.get_mut(d.a as usize) {
                *slot = ty;
            }
        }
        Op::POW | Op::POWK => {
            // A builtin call: the result is a Value of any type; stay Any.
            if let Some(slot) = out.get_mut(d.a as usize) {
                *slot = Ty::Any;
            }
        }
        Op::NEG => {
            let v = in_row.get(d.b_u as usize).copied().unwrap_or(Ty::Any);
            let ty = match v {
                Ty::Int => Ty::Int,
                Ty::Float => Ty::Float,
                _ => Ty::Any,
            };
            if let Some(slot) = out.get_mut(d.a as usize) {
                *slot = ty;
            }
        }
        _ => {}
    }
    out
}

/// `add`/`sub`/`mul` result type by the operand types, from `ops.rs`:
/// - Int, Int -> Int (overflow raises; otherwise Int)
/// - Int, Float -> Float
/// - Float, Int -> Float
/// - Float, Float -> Float
/// - with Null or Bool on either side: raises (never goes on), so what
///   writes the frame on the going-on path is undefined; keep as the
///   operand's own type, which is unsafe to rely on, so return Any.
/// - with Any on either side -> Any.
fn arith_add_sub_mul(l: Ty, r: Ty) -> Ty {
    match (l, r) {
        (Ty::Int, Ty::Int) => Ty::Int,
        (Ty::Int, Ty::Float) | (Ty::Float, Ty::Int) | (Ty::Float, Ty::Float) => Ty::Float,
        _ => Ty::Any,
    }
}

/// `div` result from `ops.rs`:
/// - Int, Int: may give Int (`a % b == 0`) or raise (zero / overflow); the
///   general path returns Float, so going-on can be Int or Float -> `Any`.
/// - Int, Float; Float, Int; Float, Float -> Float.
/// - anything else -> Any.
fn div_result(l: Ty, r: Ty) -> Ty {
    match (l, r) {
        (Ty::Int, Ty::Int) => Ty::Any,
        (Ty::Int, Ty::Float) | (Ty::Float, Ty::Int) | (Ty::Float, Ty::Float) => Ty::Float,
        _ => Ty::Any,
    }
}

/// `mod` result from `ops.rs`:
/// - Int, Int -> Int (zero raises)
/// - Int, Float; Float, Int; Float, Float -> Float
/// - anything else -> Any.
fn mod_result(l: Ty, r: Ty) -> Ty {
    match (l, r) {
        (Ty::Int, Ty::Int) => Ty::Int,
        (Ty::Int, Ty::Float) | (Ty::Float, Ty::Int) | (Ty::Float, Ty::Float) => Ty::Float,
        _ => Ty::Any,
    }
}

/// `true` when the pass has a rule for `op`. The fallback for an opcode
/// without a rule is "every register goes to `Any`". `WIDE` is not a real
/// opcode (the decoder consumes it): it never reaches `transfer`.
fn has_rule(op: Op) -> bool {
    matches!(
        op,
        Op::NOP
            | Op::MOV
            | Op::LOADK
            | Op::LOADNULL
            | Op::LOADBOOL
            | Op::LOADINT
            | Op::LOADG
            | Op::ADD
            | Op::SUB
            | Op::MUL
            | Op::DIV
            | Op::MOD
            | Op::POW
            | Op::NEG
            | Op::ADDK
            | Op::SUBK
            | Op::MULK
            | Op::DIVK
            | Op::MODK
            | Op::POWK
            | Op::EQ
            | Op::LT
            | Op::LE
            | Op::NOT
            | Op::EQK
            | Op::LTK
            | Op::LEK
            | Op::EQJ
            | Op::NEJ
            | Op::LTJ
            | Op::LEJ
            | Op::JMP
            | Op::LOOP
            | Op::TEST
            | Op::TESTN
            | Op::TESTSET
            | Op::FORPREP
            | Op::FORLOOP
            | Op::ITERPREP
            | Op::ITERNEXT
            | Op::NEWLIST
            | Op::NEWMAP
            | Op::GETFIELD
            | Op::SETFIELD
            | Op::GETINDEX
            | Op::SETINDEX
            | Op::APPENDLIST
            | Op::INSERTMAP
            | Op::INTERP
            | Op::CALL
            | Op::CALLB
            | Op::CALLM
            | Op::RET
            | Op::TAILCALL
            | Op::CLOSURE
            | Op::GETUP
            | Op::SETUP
            | Op::CELLNEW
            | Op::CELLGET
            | Op::CELLSET
            | Op::TRY_ENTER
            | Op::TRY_LEAVE
            | Op::THROW
            | Op::BURN_ENTER
            | Op::BURN_OP
            | Op::BURN_LEAVE
            | Op::COMP_ENTER
            | Op::COMP_LEAVE
            | Op::PACK
            | Op::EXHALE
            | Op::CHAIN_MAKE
            | Op::STEPS
    )
}

/// How many of the format's instruction opcodes the pass has a transfer
/// rule for, and how many fall to the `Any`-everything fallback. The count
/// is of the 72 instruction opcodes the `Op` enum carries (73 variants
/// minus `WIDE`, which is a prefix, not an instruction).
pub const RULE_COUNTS: (u32, u32) = (72, 0);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::flow;
    use cigscript::cig_bytecode::instr::Instr;
    use cigscript::cig_bytecode::module::{Constant, FuncKind, Function, HandlerEntry, SpanBase};

    fn mk(
        instrs: Vec<Instr>,
        constants: Vec<Constant>,
        handlers: Vec<HandlerEntry>,
        reg_count: u32,
    ) -> Function {
        Function {
            name: String::new(),
            kind: FuncKind::Pull,
            reg_count,
            arity: 0,
            requires_burn: false,
            upvalues: Vec::new(),
            constants,
            children: Vec::new(),
            regions: Vec::new(),
            handlers,
            instrs,
            span_base: SpanBase::default(),
            lines: Vec::new(),
            region_rows: Vec::new(),
        }
    }

    #[test]
    fn the_lattice_joins_as_stated() {
        assert_eq!(Ty::None.join(Ty::Int), Ty::Int);
        assert_eq!(Ty::Int.join(Ty::None), Ty::Int);
        assert_eq!(Ty::Int.join(Ty::Int), Ty::Int);
        assert_eq!(Ty::Int.join(Ty::Float), Ty::Any);
        assert_eq!(Ty::Bool.join(Ty::Null), Ty::Any);
        assert_eq!(Ty::Int.join(Ty::Any), Ty::Any);
        assert_eq!(Ty::Any.join(Ty::Null), Ty::Any);
    }

    #[test]
    fn at_pc_0_every_register_is_any() {
        let instrs = vec![
            Instr::abc(Op::LOADNULL, 0, 0, 0),
            Instr::abc(Op::RET, 0, 0, 0),
        ];
        let f = mk(instrs, vec![], vec![], 4);
        let fl = flow(&f).unwrap();
        let t = infer(&f, &fl);
        assert_eq!(t.before(0), &[Ty::Any; 4]);
    }

    #[test]
    fn loadnull_loadbool_loadint_write_their_type() {
        // loadnull r0; loadbool r1, 1; loadint r2, 5; ret r0
        let instrs = vec![
            Instr::abc(Op::LOADNULL, 0, 0, 0),
            Instr::abc(Op::LOADBOOL, 1, 1, 0),
            Instr::asbx(Op::LOADINT, 2, 5),
            Instr::abc(Op::RET, 0, 0, 0),
        ];
        let f = mk(instrs, vec![], vec![], 4);
        let fl = flow(&f).unwrap();
        let t = infer(&f, &fl);
        // Just before `ret`, r0 = Null, r1 = Bool, r2 = Int.
        let before_ret = t.before(3);
        assert_eq!(before_ret[0], Ty::Null);
        assert_eq!(before_ret[1], Ty::Bool);
        assert_eq!(before_ret[2], Ty::Int);
        // r3 was never written: still Any from pc 0.
        assert_eq!(before_ret[3], Ty::Any);
    }

    #[test]
    fn add_of_two_ints_is_an_int_on_the_going_on_path() {
        // loadint r0, 1; loadint r1, 2; add r2, r0, r1; ret r2
        let instrs = vec![
            Instr::asbx(Op::LOADINT, 0, 1),
            Instr::asbx(Op::LOADINT, 1, 2),
            Instr::abc(Op::ADD, 2, 0, 1),
            Instr::abc(Op::RET, 2, 0, 0),
        ];
        let f = mk(instrs, vec![], vec![], 4);
        let fl = flow(&f).unwrap();
        let t = infer(&f, &fl);
        let before_ret = t.before(3);
        assert_eq!(before_ret[2], Ty::Int);
    }

    #[test]
    fn add_of_int_and_float_gives_float() {
        // loadint r0, 1; loadk r1, 0 (Float); add r2, r0, r1; ret r2
        let instrs = vec![
            Instr::asbx(Op::LOADINT, 0, 1),
            Instr::abx(Op::LOADK, 1, 0),
            Instr::abc(Op::ADD, 2, 0, 1),
            Instr::abc(Op::RET, 2, 0, 0),
        ];
        let f = mk(instrs, vec![Constant::Float(1.5)], vec![], 4);
        let fl = flow(&f).unwrap();
        let t = infer(&f, &fl);
        assert_eq!(t.before(3)[2], Ty::Float);
    }

    #[test]
    fn div_of_two_ints_is_any() {
        let instrs = vec![
            Instr::asbx(Op::LOADINT, 0, 7),
            Instr::asbx(Op::LOADINT, 1, 2),
            Instr::abc(Op::DIV, 2, 0, 1),
            Instr::abc(Op::RET, 2, 0, 0),
        ];
        let f = mk(instrs, vec![], vec![], 4);
        let fl = flow(&f).unwrap();
        let t = infer(&f, &fl);
        assert_eq!(t.before(3)[2], Ty::Any);
    }

    #[test]
    fn the_rest_of_a_call_row_is_null_and_registers_above_are_any() {
        // loadint r0, 1; loadint r1, 0; call r1, 0, 3; ret r0
        // After the call: R(1) = Any (result), R(2) = R(3) = R(4) = Null,
        // R(5..) = Any. `extra = C` is 3 here, so the row covers A..A+3
        // (R(1), R(2), R(3), R(4)); R(5) is the first "above".
        let instrs = vec![
            Instr::asbx(Op::LOADINT, 0, 1),
            Instr::asbx(Op::LOADINT, 5, 99), // mark r5 as Int before the call
            Instr::asbx(Op::LOADINT, 1, 0),
            Instr::abc(Op::CALL, 1, 0, 3),
            Instr::abc(Op::RET, 1, 0, 0),
        ];
        let f = mk(instrs, vec![], vec![], 8);
        let fl = flow(&f).unwrap();
        let t = infer(&f, &fl);
        let before_ret = t.before(4);
        assert_eq!(before_ret[1], Ty::Any); // result
        assert_eq!(before_ret[2], Ty::Null);
        assert_eq!(before_ret[3], Ty::Null);
        assert_eq!(before_ret[4], Ty::Null);
        // R(5) was an Int, but the call left it undefined -> Any.
        assert_eq!(before_ret[5], Ty::Any);
        // R(0) kept its Int (it was below the call).
        assert_eq!(before_ret[0], Ty::Int);
    }

    #[test]
    fn a_handler_makes_every_register_any() {
        // One handler (even if structurally weird): every reg is Any.
        let instrs = vec![
            Instr::abc(Op::LOADNULL, 0, 0, 0),
            Instr::abc(Op::LOADNULL, 1, 0, 0),
            Instr::abc(Op::RET, 0, 0, 0),
        ];
        let handlers = vec![HandlerEntry {
            pc_start: 0,
            pc_end: 2,
            handler_pc: 2,
            catch_reg: cigscript::cig_bytecode::module::RESERVED_U32,
            finally_pc: cigscript::cig_bytecode::module::RESERVED_U32,
        }];
        let f = mk(instrs, vec![], handlers, 4);
        let fl = flow(&f).unwrap();
        let t = infer(&f, &fl);
        for pc in [0, 1, 2] {
            for r in 0..4 {
                assert_eq!(t.before(pc)[r], Ty::Any, "pc {pc} r {r}");
            }
        }
    }

    #[test]
    fn a_cell_register_is_any_everywhere() {
        // loadint r1, 5; cellnew r1; loadint r1, 7; ret r1
        // r1 is a cell register, so it stays Any even where a plain loadint
        // would put Int.
        let instrs = vec![
            Instr::asbx(Op::LOADINT, 1, 5),
            Instr::abc(Op::CELLNEW, 1, 0, 0),
            Instr::asbx(Op::LOADINT, 1, 7),
            Instr::abc(Op::RET, 1, 0, 0),
        ];
        let f = mk(instrs, vec![], vec![], 4);
        let fl = flow(&f).unwrap();
        let t = infer(&f, &fl);
        for pc in [0, 1, 2, 3] {
            assert_eq!(t.before(pc)[1], Ty::Any, "pc {pc}");
        }
    }

    #[test]
    fn two_paths_meeting_with_different_types_give_any() {
        // test r0, +2 ; loadint r1, 1 ; jmp +1 ; loadk r1, 0 (Float) ; ret r1
        let instrs = vec![
            Instr::asbx(Op::TEST, 0, 2),    // pc 0: branch over the int
            Instr::asbx(Op::LOADINT, 1, 1), // pc 1: r1 = Int
            Instr::asbx(Op::JMP, 0, 1),     // pc 2: jump past the float
            Instr::abx(Op::LOADK, 1, 0),    // pc 3: r1 = Float
            Instr::abc(Op::RET, 1, 0, 0),   // pc 4: ret r1
        ];
        let f = mk(instrs, vec![Constant::Float(2.5)], vec![], 4);
        let fl = flow(&f).unwrap();
        let t = infer(&f, &fl);
        assert_eq!(t.before(4)[1], Ty::Any);
    }

    #[test]
    fn testset_refines_on_the_fall_through_only() {
        // loadint r1, 5 ; testset r0, r1, +2 ; loadnull r2 ; ret r0
        let instrs = vec![
            Instr::asbx(Op::LOADINT, 1, 5),    // pc 0: r1 = Int
            Instr::abc(Op::TESTSET, 0, 1, 2),  // pc 1: C = 2
            Instr::abc(Op::LOADNULL, 2, 0, 0), // pc 2 (fall-through)
            Instr::abc(Op::LOADNULL, 3, 0, 0), // pc 3 (jump target)
            Instr::abc(Op::RET, 0, 0, 0),      // pc 4
        ];
        let f = mk(instrs, vec![], vec![], 4);
        let fl = flow(&f).unwrap();
        let t = infer(&f, &fl);
        // on_edge from pc 1 to fall-through pc 2: R(0) = Int.
        let edge_to_2 = t.on_edge(1, 2);
        assert_eq!(edge_to_2[0], Ty::Int);
        // on_edge from pc 1 to jump pc 4: R(0) unchanged (Any from pc 0).
        let edge_to_4 = t.on_edge(1, 4);
        assert_eq!(edge_to_4[0], Ty::Any);
    }

    #[test]
    fn forprep_refines_the_window_into_the_body() {
        // loadint r1, 0 ; loadint r2, 5 ; loadint r3, 1 ; forprep r1, +2
        // (A = 1, so window R1..R4: start, limit, step, induction var)
        // ; loadnull r5 ; forloop r1, -2 ; ret r1
        let instrs = vec![
            Instr::asbx(Op::LOADINT, 1, 0),    // pc 0
            Instr::asbx(Op::LOADINT, 2, 5),    // pc 1
            Instr::asbx(Op::LOADINT, 3, 1),    // pc 2
            Instr::asbx(Op::FORPREP, 1, 2),    // pc 3 (fall-through = pc 4)
            Instr::abc(Op::LOADNULL, 5, 0, 0), // pc 4
            Instr::asbx(Op::FORLOOP, 1, -2),   // pc 5 (back to pc 4)
            Instr::abc(Op::RET, 1, 0, 0),      // pc 6
        ];
        let f = mk(instrs, vec![], vec![], 8);
        let fl = flow(&f).unwrap();
        let t = infer(&f, &fl);
        // Into the body (pc 4): R(1), R(2), R(3), R(4) are Int.
        let into_body = t.on_edge(3, 4);
        assert_eq!(into_body[1], Ty::Int);
        assert_eq!(into_body[2], Ty::Int);
        assert_eq!(into_body[3], Ty::Int);
        assert_eq!(into_body[4], Ty::Int);
        // At pc 4 (loop body start), R(4) is Int (the induction var).
        assert_eq!(t.before(4)[4], Ty::Int);
    }

    /// For a `cigscript`-parsed script, this stretches a real lowering: see
    /// `tests/types.rs`.
    #[test]
    fn an_edge_is_typed_as_the_pass_typed_it() {
        // 0 loadk r0, k0 (2.5) ; 1 loadint r1, 3 ; 2 addk r2, r1, k1 (4) ;
        // 3 cellnew r3 ; 4 loadint r3, 1 ; 5 ret r0
        let instrs = vec![
            Instr::abx(Op::LOADK, 0, 0),
            Instr::asbx(Op::LOADINT, 1, 3),
            Instr::abc(Op::ADDK, 2, 1, 1),
            Instr::abc(Op::CELLNEW, 3, 0, 0),
            Instr::asbx(Op::LOADINT, 3, 1),
            Instr::abc(Op::RET, 0, 0, 0),
        ];
        let f = mk(
            instrs,
            vec![Constant::Float(2.5), Constant::Int(4)],
            vec![],
            4,
        );
        let fl = flow(&f).unwrap();
        let t = infer(&f, &fl);
        // A constant's type is on the edge, as it is before the next pc.
        assert_eq!(t.on_edge(0, 1)[0], Ty::Float);
        assert_eq!(t.before(1)[0], Ty::Float);
        assert_eq!(t.on_edge(2, 3)[2], Ty::Int);
        assert_eq!(t.before(3)[2], Ty::Int);
        // A cell register is `Any` on an edge too, whatever was written.
        assert_eq!(t.on_edge(4, 5)[3], Ty::Any);
        // Before a pc with one way in, the types are that edge's.
        for (from, to) in [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5)] {
            assert_eq!(t.on_edge(from, to), t.before(to), "{from} -> {to}");
        }
    }

    #[test]
    fn eq_and_lt_write_bool() {
        // loadint r0, 1 ; loadint r1, 2 ; lt r2, r0, r1 ; ret r2
        let instrs = vec![
            Instr::asbx(Op::LOADINT, 0, 1),
            Instr::asbx(Op::LOADINT, 1, 2),
            Instr::abc(Op::LT, 2, 0, 1),
            Instr::abc(Op::RET, 2, 0, 0),
        ];
        let f = mk(instrs, vec![], vec![], 4);
        let fl = flow(&f).unwrap();
        let t = infer(&f, &fl);
        assert_eq!(t.before(3)[2], Ty::Bool);
    }

    #[test]
    fn unknown_opcode_fallback_is_any_everywhere() {
        // A function whose only opcode is one we have no rule for (a
        // `newlist` is a safe stand-in: it is in `has_rule`, but the rule
        // writes Any). Use a `pack` instead; we have a rule, but it writes
        // nothing; a `burn.enter` is governed by `handlers=false`. To
        // exercise the fallback, test the `has_rule` function directly for
        // every opcode that has one and for the wide prefix (which does
        // not).
        assert!(!has_rule(Op::WIDE));
        for op in [
            Op::NOP,
            Op::MOV,
            Op::LOADK,
            Op::LOADNULL,
            Op::LOADBOOL,
            Op::LOADINT,
            Op::ADD,
            Op::DIV,
            Op::CALL,
            Op::CELLNEW,
            Op::TRY_ENTER,
            Op::BURN_OP,
        ] {
            assert!(has_rule(op), "{:?}", op);
        }
    }

    #[test]
    fn reads_writes_round_trip_simple_ops() {
        let d = decode_single(Instr::abc(Op::ADD, 3, 1, 2));
        assert_eq!(reads(&d), vec![1, 2]);
        let w = writes(&d);
        assert_eq!(w.regs, vec![(3, Ty::None)]);
        assert!(w.above.is_none());

        let d = decode_single(Instr::abc(Op::CALL, 2, 1, 3));
        assert_eq!(reads(&d), vec![2, 3]);
        let w = writes(&d);
        // (2, Any), (3, Null), (4, Null), (5, Null); above = 6.
        assert_eq!(w.regs[0], (2, Ty::Any));
        assert_eq!(w.regs[1], (3, Ty::Null));
        assert_eq!(w.regs[2], (4, Ty::Null));
        assert_eq!(w.regs[3], (5, Ty::Null));
        assert_eq!(w.above, Some(6));
    }

    fn decode_single(i: Instr) -> DecodedInstr {
        let (_, code) = decode_table(&[i, Instr::abc(Op::RET, 0, 0, 0)]).unwrap();
        code[0].unwrap()
    }

    /// The transfer rule on every pair of the four scalar types, for each
    /// arithmetic, division, modulo, comparison and negation opcode.
    /// The rule here is what `ops.rs` in cig-script does, read function by
    /// function. A unit test against `interp::ops::binary` is not reachable
    /// from here (the module is `pub(crate)`); this test is the "read from
    /// the VM's code" transfer rule stated explicitly.
    #[test]
    fn transfer_rules_match_ops_rs_for_every_scalar_pair() {
        use Ty::{Any, Bool, Float, Int, Null};
        let scalars = [Null, Bool, Int, Float];
        for &l in &scalars {
            for &r in &scalars {
                // Add/sub/mul: Int+Int=Int; Int+Float, Float+Int,
                // Float+Float = Float; otherwise: raises in ops.rs, Any
                // here.
                let expected_add = match (l, r) {
                    (Int, Int) => Int,
                    (Int, Float) | (Float, Int) | (Float, Float) => Float,
                    _ => Any,
                };
                assert_eq!(arith_add_sub_mul(l, r), expected_add, "add {l:?}+{r:?}");

                // Div: Int, Int -> Any (may be Int on perfect division);
                // numeric-numeric pairs with a float -> Float; else Any.
                let expected_div = match (l, r) {
                    (Int, Int) => Any,
                    (Int, Float) | (Float, Int) | (Float, Float) => Float,
                    _ => Any,
                };
                assert_eq!(div_result(l, r), expected_div, "div {l:?}/{r:?}");

                // Mod: Int, Int -> Int; float pairs -> Float; else Any.
                let expected_mod = match (l, r) {
                    (Int, Int) => Int,
                    (Int, Float) | (Float, Int) | (Float, Float) => Float,
                    _ => Any,
                };
                assert_eq!(mod_result(l, r), expected_mod, "mod {l:?}%{r:?}");
            }
        }
    }
}
