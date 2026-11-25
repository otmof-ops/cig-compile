// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2

//! Which registers are read again, and which a loop only ever writes
//! numbers to.
//!
//! Machine code may store a number over a register only when what the
//! register held owns no memory: a string, a list or a map has to be
//! freed, and only the runtime can. An inline write therefore tests the
//! register's old value first, at every write, in every turn of a loop.
//!
//! Two facts make most of those tests unnecessary, and this module
//! computes both from the bytecode alone:
//!
//! - **[`Live`]: whether a register is read again before it is
//!   written.** A register nobody reads again holds a value nobody can
//!   see, so that value can be freed early, where a loop is entered,
//!   by one call of the runtime outside the loop.
//! - **[`plain_writer`]: whether an instruction can only leave a value
//!   that owns no memory in a register**, whoever runs it and whatever
//!   its operands turn out to be. `a - b` is a number or an error;
//!   `a + b` is a string when both are.
//!
//! A register that is dead where a loop starts, and that every
//! instruction of the loop that writes it writes plainly, owns no
//! memory anywhere in the loop once it has been freed at the entry:
//! every later write to it is a plain store.
//!
//! Both lean to the safe side: a register is live unless every path
//! shows it is not, and a writer is plain only when its own rules say
//! so for every operand it could meet.

use std::collections::BTreeMap;

use cigscript::cig_bytecode::instr::{decode_table, DecodedInstr};
use cigscript::cig_bytecode::module::{Function, UpvalKind};
use cigscript::cig_bytecode::opcode::Op;

use crate::flow::Flow;
use crate::types::{reads, writes, Ty};

/// For each instruction, the registers that are read again before they
/// are written, on some path from it.
#[derive(Clone, Debug)]
pub struct Live {
    /// Every first-word pc, ascending.
    pcs: Vec<u32>,
    /// The function's register count.
    reg_count: u32,
    /// One row a pc: a bit a register, set when it is live before the
    /// instruction runs.
    before: Vec<Vec<u64>>,
}

impl Live {
    /// Whether register `r` is live before the instruction at `pc`: it
    /// is read, on some path from there, before anything writes it. A
    /// pc or a register the function does not have is live.
    pub fn before(&self, pc: u32, r: u32) -> bool {
        if r >= self.reg_count {
            return true;
        }
        let Ok(i) = self.pcs.binary_search(&pc) else {
            return true;
        };
        self.before
            .get(i)
            .and_then(|row| row.get((r / 64) as usize))
            .is_none_or(|w| w & (1 << (r % 64)) != 0)
    }
}

fn set(row: &mut [u64], r: u32) {
    if let Some(w) = row.get_mut((r / 64) as usize) {
        *w |= 1 << (r % 64);
    }
}

/// The registers `f`'s instructions keep alive.
///
/// A backward pass to a fixed point over the bytecode's own flow. An
/// instruction keeps alive what it reads; one that writes a register
/// on every way out of it ends that register's life before it. A
/// `closure` reads the registers its function captures. Where the flow
/// or the code cannot be read, and in a function with a handler (an
/// error can go to it from anywhere, and it may read anything), every
/// register is live everywhere.
pub fn live(f: &Function, flow: &Flow) -> Live {
    let words = (f.reg_count as usize).div_ceil(64).max(1);
    let every = |n: usize| Live {
        pcs: flow.pcs.clone(),
        reg_count: f.reg_count,
        before: vec![vec![u64::MAX; words]; n],
    };
    let n = flow.pcs.len();
    let Ok((_, table)) = decode_table(&f.instrs) else {
        return every(n);
    };
    if flow.handlers {
        return every(n);
    }
    let index: BTreeMap<u32, usize> = flow
        .pcs
        .iter()
        .enumerate()
        .map(|(i, pc)| (*pc, i))
        .collect();
    // What each instruction reads, and what it writes whichever way it
    // goes on.
    let mut uses = vec![vec![0u64; words]; n];
    let mut kills = vec![vec![0u64; words]; n];
    for (i, pc) in flow.pcs.iter().enumerate() {
        let Some(Some(d)) = table.get(*pc as usize) else {
            return every(n);
        };
        for r in reads(d) {
            set(&mut uses[i], r);
        }
        if d.op == Op::CLOSURE {
            // The registers the new closure's function captures: a
            // cell where the register carries one, the value itself
            // where it does not.
            let Some(child) = f.children.get(d.b_u as usize) else {
                return every(n);
            };
            for up in &child.upvalues {
                if up.kind == UpvalKind::ParentLocal {
                    set(&mut uses[i], u32::from(up.index));
                }
            }
        }
        let w = writes(d);
        // These write on one of their ways out only (the turn taken,
        // the item found): on the other the register is what it was.
        if !matches!(d.op, Op::TESTSET | Op::FORPREP | Op::FORLOOP | Op::ITERNEXT) {
            for (r, _) in &w.regs {
                set(&mut kills[i], *r);
            }
        }
        // A call leaves the registers above its row undefined: what
        // they held is gone.
        if let Some(above) = w.above {
            for r in above..f.reg_count {
                set(&mut kills[i], r);
            }
        }
    }
    let mut before = vec![vec![0u64; words]; n];
    loop {
        let mut changed = false;
        for (i, pc) in flow.pcs.iter().enumerate().rev() {
            let mut row = vec![0u64; words];
            for s in flow.succs.get(pc).map(Vec::as_slice).unwrap_or(&[]) {
                match index.get(s) {
                    Some(j) => {
                        for (w, x) in row.iter_mut().zip(&before[*j]) {
                            *w |= x;
                        }
                    }
                    // Somewhere this pass cannot see: everything lives.
                    None => row.fill(u64::MAX),
                }
            }
            for ((w, k), u) in row.iter_mut().zip(&kills[i]).zip(&uses[i]) {
                *w = (*w & !k) | u;
            }
            if row != before[i] {
                before[i] = row;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    Live {
        pcs: flow.pcs.clone(),
        reg_count: f.reg_count,
        before,
    }
}

/// Whether `d`, when it writes register `r`, can only leave there a
/// value that owns no memory: null, a bool, an int or a float.
///
/// It holds for whoever runs the instruction (machine code, or the
/// interpreter where the inline form does not cover the operands) and
/// for any operands of the types `before` has, the function's register
/// types just before `d`; `konst` is the type of a constant of its
/// pool. An instruction that raises writes nothing.
///
/// - `-`, `/`, `%`, negation, the comparisons and `not` give a number
///   or a bool, or raise.
/// - `+` joins two strings and two lists: it is plain when one operand
///   is known to be neither.
/// - `*` repeats a string by an int: it is plain when one operand is a
///   float, a bool or null, or both are known to be no string.
/// - A constant, and a copy of a register, are what they copy.
/// - A counted loop writes ints; a walk over a list writes its index.
/// - A call leaves null in the rest of its row.
///
/// Everything else may write anything.
pub fn plain_writer(d: &DecodedInstr, r: u32, before: &[Ty], konst: &dyn Fn(u32) -> Ty) -> bool {
    let ty = |x: u32| before.get(x as usize).copied().unwrap_or(Ty::Any);
    // A string times an int is a string: neither side may be both
    // unknown and able to be the string.
    let no_text = |t: Ty| matches!(t, Ty::Float | Ty::Bool | Ty::Null);
    let times = |a: Ty, b: Ty| (a.is_scalar() && b.is_scalar()) || no_text(a) || no_text(b);
    match d.op {
        Op::LOADNULL | Op::LOADBOOL | Op::LOADINT => true,
        Op::LOADK => konst(d.b_u).is_scalar(),
        Op::MOV => ty(d.b_u).is_scalar(),
        Op::SUB | Op::DIV | Op::MOD | Op::SUBK | Op::DIVK | Op::MODK | Op::NEG => true,
        Op::NOT | Op::EQ | Op::LT | Op::LE | Op::EQK | Op::LTK | Op::LEK => true,
        Op::ADD => ty(d.b_u).is_scalar() || ty(d.c).is_scalar(),
        Op::ADDK => ty(d.b_u).is_scalar() || konst(d.c).is_scalar(),
        Op::MUL => times(ty(d.b_u), ty(d.c)),
        Op::MULK => times(ty(d.b_u), konst(d.c)),
        Op::FORPREP => r == d.a.saturating_add(3),
        Op::FORLOOP => r == d.a || r == d.a.saturating_add(3),
        Op::ITERPREP | Op::ITERNEXT => r == d.a.saturating_add(2),
        Op::CALL => r > d.a && r <= d.a.saturating_add(d.c),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::flow;
    use crate::types::infer;
    use cigscript::cig_bytecode::instr::Instr;
    use cigscript::cig_bytecode::module::{
        Constant, FuncKind, Function, HandlerEntry, SpanBase, UpvalDesc,
    };

    fn mk(instrs: Vec<Instr>, constants: Vec<Constant>, reg_count: u32) -> Function {
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
            handlers: Vec::new(),
            instrs,
            span_base: SpanBase::default(),
            lines: Vec::new(),
            region_rows: Vec::new(),
        }
    }

    fn alive(f: &Function, pc: u32) -> Vec<u32> {
        let l = live(f, &flow(f).expect("flows"));
        (0..f.reg_count).filter(|r| l.before(pc, *r)).collect()
    }

    #[test]
    fn a_register_lives_from_its_write_to_its_last_read() {
        // 0 loadint r0, 1; 1 loadint r1, 2; 2 add r2, r0, r1; 3 ret r2
        let f = mk(
            vec![
                Instr::asbx(Op::LOADINT, 0, 1),
                Instr::asbx(Op::LOADINT, 1, 2),
                Instr::abc(Op::ADD, 2, 0, 1),
                Instr::abc(Op::RET, 2, 0, 0),
            ],
            vec![],
            4,
        );
        assert_eq!(alive(&f, 0), Vec::<u32>::new());
        assert_eq!(alive(&f, 1), [0]);
        assert_eq!(alive(&f, 2), [0, 1]);
        assert_eq!(alive(&f, 3), [2]);
        // A pc and a register the function does not have are live.
        let l = live(&f, &flow(&f).expect("flows"));
        assert!(l.before(99, 0) && l.before(0, 99));
    }

    #[test]
    fn a_loop_keeps_alive_what_a_later_turn_reads() {
        // r0 = 0; r1 = 3; r2 = 1 (the counted loop's three registers);
        // r5 = 0, the sum.
        //  4 forprep r0, -> 8
        //  5 add r4, r3, r3      a scratch value, written before it is read
        //  6 add r5, r5, r4      the sum is read at every turn
        //  7 forloop r0, -> 5
        //  8 ret r5
        let f = mk(
            vec![
                Instr::asbx(Op::LOADINT, 0, 0),
                Instr::asbx(Op::LOADINT, 1, 3),
                Instr::asbx(Op::LOADINT, 2, 1),
                Instr::asbx(Op::LOADINT, 5, 0),
                Instr::asbx(Op::FORPREP, 0, 3),
                Instr::abc(Op::ADD, 4, 3, 3),
                Instr::abc(Op::ADD, 5, 5, 4),
                Instr::asbx(Op::FORLOOP, 0, -3),
                Instr::abc(Op::RET, 5, 0, 0),
            ],
            vec![],
            6,
        );
        // Where the loop starts: the loop's own three registers, its
        // variable and the sum live; the scratch register does not (it
        // is written before it is read, at every turn).
        assert_eq!(alive(&f, 5), [0, 1, 2, 3, 5]);
        // The loop's variable is written by the turn taken only: where
        // no turn is taken it is what it was, so it is not ended there.
        assert!(alive(&f, 4).contains(&0));
        // After the loop only the sum.
        assert_eq!(alive(&f, 8), [5]);
    }

    #[test]
    fn a_closure_reads_the_registers_its_function_captures() {
        //  0 closure r1, fn0   (captures this frame's r3)
        //  1 ret r1
        let mut f = mk(
            vec![Instr::abx(Op::CLOSURE, 1, 0), Instr::abc(Op::RET, 1, 0, 0)],
            vec![],
            4,
        );
        let mut child = mk(vec![Instr::abc(Op::RET, 0, 0, 0)], vec![], 1);
        child.upvalues = vec![
            UpvalDesc {
                kind: UpvalKind::ParentLocal,
                index: 3,
            },
            UpvalDesc {
                kind: UpvalKind::ParentUpvalue,
                index: 2,
            },
        ];
        f.children = vec![child];
        // The parent's register 3, and not its register 2: the second
        // capture is of the parent's own upvalue 2.
        assert_eq!(alive(&f, 0), [3]);
    }

    #[test]
    fn a_call_ends_the_registers_above_its_row() {
        //  0 call r1, n=0, c=0    r1 = the result; r2 and up are undefined after
        //  1 ret r1
        let f = mk(
            vec![Instr::abc(Op::CALL, 1, 0, 0), Instr::abc(Op::RET, 1, 0, 0)],
            vec![],
            5,
        );
        // Before the call only the callee is read; what r2..r4 held is
        // gone after it whoever would have read it.
        assert_eq!(alive(&f, 0), [1]);
    }

    #[test]
    fn a_handler_keeps_everything_alive() {
        let mut f = mk(
            vec![
                Instr::abc(Op::LOADNULL, 0, 0, 0),
                Instr::abc(Op::RET, 0, 0, 0),
            ],
            vec![],
            3,
        );
        f.handlers = vec![HandlerEntry {
            pc_start: 0,
            pc_end: 1,
            handler_pc: 1,
            catch_reg: cigscript::cig_bytecode::module::RESERVED_U32,
            finally_pc: cigscript::cig_bytecode::module::RESERVED_U32,
        }];
        assert_eq!(alive(&f, 0), [0, 1, 2]);
    }

    #[test]
    fn what_an_instruction_can_leave_in_a_register() {
        // The types before an instruction, as the pass would have them.
        let any = [Ty::Any, Ty::Any, Ty::Any];
        let int_first = [Ty::Any, Ty::Int, Ty::Any];
        let float_first = [Ty::Any, Ty::Float, Ty::Any];
        let ints = [Ty::Any, Ty::Int, Ty::Int];
        let konst = |i: u32| match i {
            0 => Ty::Int,
            1 => Ty::Float,
            _ => Ty::Any, // a string
        };
        let d = |op, a, b, c| {
            let (_, t) = decode_table(&[Instr::abc(op, a, b, c)]).expect("decodes");
            t[0].expect("an instruction")
        };
        let plain = |x: &DecodedInstr, before: &[Ty]| plain_writer(x, x.a, before, &konst);

        // A number or an error, whatever the operands.
        for op in [
            Op::SUB,
            Op::DIV,
            Op::MOD,
            Op::NEG,
            Op::NOT,
            Op::EQ,
            Op::LT,
            Op::LE,
        ] {
            assert!(plain(&d(op, 0, 1, 2), &any), "{op:?}");
        }
        // `+` joins two strings: plain only with a side that is none.
        assert!(!plain(&d(Op::ADD, 0, 1, 2), &any));
        assert!(plain(&d(Op::ADD, 0, 1, 2), &int_first));
        assert!(plain(&d(Op::ADDK, 0, 1, 0), &any), "a number is added");
        assert!(!plain(&d(Op::ADDK, 0, 1, 2), &any), "a string is added");
        assert!(plain(&d(Op::ADDK, 0, 1, 2), &int_first));
        // `*` repeats a string by an int: an int on one side is not
        // enough, a float is, and so are two that are no string.
        assert!(!plain(&d(Op::MUL, 0, 1, 2), &any));
        assert!(!plain(&d(Op::MUL, 0, 1, 2), &int_first));
        assert!(plain(&d(Op::MUL, 0, 1, 2), &float_first));
        assert!(plain(&d(Op::MUL, 0, 1, 2), &ints));
        assert!(!plain(&d(Op::MULK, 0, 1, 0), &any), "an int constant");
        assert!(plain(&d(Op::MULK, 0, 1, 1), &any), "a float constant");
        assert!(plain(&d(Op::MULK, 0, 1, 0), &int_first));
        // A copy is what it copies; a constant what it is.
        assert!(!plain(&d(Op::MOV, 0, 1, 0), &any));
        assert!(plain(&d(Op::MOV, 0, 1, 0), &int_first));
        let k = |i: u16| {
            let (_, t) = decode_table(&[Instr::abx(Op::LOADK, 0, i)]).expect("decodes");
            t[0].expect("an instruction")
        };
        assert!(plain(&k(0), &any) && !plain(&k(2), &any));
        // A field, an element, a call's result, a string built: anything.
        for op in [
            Op::GETFIELD,
            Op::GETINDEX,
            Op::CALLB,
            Op::INTERP,
            Op::NEWLIST,
        ] {
            assert!(!plain(&d(op, 0, 1, 2), &ints), "{op:?}");
        }
        // A counted loop writes its counter and its variable, ints; a
        // walk writes its index, and its item may be anything.
        let turn = d(Op::FORLOOP, 4, 0, 0);
        assert!(plain_writer(&turn, 4, &any, &konst) && plain_writer(&turn, 7, &any, &konst));
        let walk = d(Op::ITERNEXT, 4, 0, 0);
        assert!(plain_writer(&walk, 6, &any, &konst) && !plain_writer(&walk, 7, &any, &konst));
        // A call: null in the rest of its row, anything as its result.
        let call = d(Op::CALL, 3, 1, 2);
        assert!(!plain_writer(&call, 3, &any, &konst));
        assert!(plain_writer(&call, 4, &any, &konst) && plain_writer(&call, 5, &any, &konst));
        assert!(!plain_writer(&call, 6, &any, &konst));
    }

    #[test]
    fn the_pass_and_the_writers_agree_on_a_real_loop() {
        // The loop of `a_loop_keeps_alive_what_a_later_turn_reads`: the
        // scratch register is dead where the loop starts and both
        // writers of the loop's registers are plain once the pass has
        // the sum and the variable as ints.
        let f = mk(
            vec![
                Instr::asbx(Op::LOADINT, 0, 0),
                Instr::asbx(Op::LOADINT, 1, 3),
                Instr::asbx(Op::LOADINT, 2, 1),
                Instr::asbx(Op::LOADINT, 5, 0),
                Instr::asbx(Op::FORPREP, 0, 3),
                Instr::abc(Op::ADD, 4, 3, 3),
                Instr::abc(Op::ADD, 5, 5, 4),
                Instr::asbx(Op::FORLOOP, 0, -3),
                Instr::abc(Op::RET, 5, 0, 0),
            ],
            vec![],
            6,
        );
        let fl = flow(&f).expect("flows");
        let types = infer(&f, &fl);
        let (_, table) = decode_table(&f.instrs).expect("decodes");
        let konst = |_: u32| Ty::Any;
        for pc in [5u32, 6] {
            let d = table[pc as usize].expect("an instruction");
            assert!(plain_writer(&d, d.a, types.before(pc), &konst), "pc {pc}");
        }
        assert!(!live(&f, &fl).before(5, 4));
    }
}
