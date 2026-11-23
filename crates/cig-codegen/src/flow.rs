// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2

//! The bytecode's control-flow facts: for each instruction, every pc it can
//! go on to, which registers can carry a cell, which pcs are the targets of
//! backward jumps, and whether the function has a handler (a `try`).
//!
//! A function is decoded once ([`cig_bytecode::instr::decode_table`]) and
//! every pc named here is a first-word pc. Nothing in [`Flow`] holds an
//! unordered map: a `BTreeMap`/`BTreeSet` keeps the same function giving the
//! same [`Flow`] on any host.
//!
//! ## What a successor is
//!
//! For an instruction decoded at first-word pc `p` with width `w`, the next
//! sequential pc is `p + w`. The successor set is:
//!
//! - `ret`, `tailcall`, `throw`: empty (the frame leaves).
//! - `jmp`, `loop`: the jump target only.
//! - `test`, `testn`, `testset`, `eqj`, `nej`, `ltj`, `lej`, `forprep`,
//!   `forloop`, `iternext`: fall-through and the jump target.
//! - `try.leave A`: fall-through, and the `finally_pc` of every open handler
//!   whose body range contains the instruction, over-approximating a
//!   pending finally's resumption (see "Handlers" below).
//! - Every other instruction: fall-through only.
//!
//! ## Handlers
//!
//! A handler's `handler_pc` and `finally_pc` are reached from every
//! instruction of its body range when that instruction raises. Modelling
//! every raise as an extra edge would clutter `succs`; instead, the pcs are
//! surfaced as [`Flow::handler_entries`] so the types pass can mark them as
//! entry points (every register `Any` there). The `succs` map includes a
//! `try.leave` → `finally_pc` edge so the normal control-flow path (`A = 0`
//! with a finally) is closed, as the VM's `leave` does. This is a sound
//! over-approximation of the exact pcs a `try.leave` from a finally stage
//! resumes to (`Pending::Next` holds a dynamic pc); the types pass treats
//! every register as `Any` in a function with any handler anyway.
//!
//! ## Loop heads
//!
//! A pc is a loop head when it is the target of a backward jump (target pc
//! at or before the jumping instruction's first-word pc: an instruction
//! that jumps to itself is a loop). The jumps
//! that can go backward are the unconditional `jmp` and `loop`, the
//! conditional `test`/`testn`/`testset`/`eqj`/`nej`/`ltj`/`lej`, and
//! `iternext`. `forprep` and `forloop` are also included (a wide negative
//! offset is possible even if not usual) since what counts is the target pc.
//!
//! ## Leaders
//!
//! A leader is a first-word pc that starts a block. The set is:
//!
//! - pc 0;
//! - every jump target (including `forprep`/`forloop`/`iternext` targets);
//! - the pc after every instruction that branches (so the fall-through path
//!   starts a block too);
//! - every handler's `handler_pc` and `finally_pc`;
//! - the pc after a `jmp`, a `loop`, a `ret`, a `tailcall`, a `throw` and a
//!   `call` (nothing falls through the first five, and a frame is gone back
//!   to after the last).
//!
//! ## Resume points
//!
//! Where a function's native code can be entered (cig-compile's decision
//! 0008): pc 0, the pc after every `call`, every loop head, and every
//! handler's `handler_pc` and `finally_pc`. Each is a leader.

use std::collections::{BTreeMap, BTreeSet};

use cigscript::cig_bytecode::instr::{decode_table, DecodedInstr};
use cigscript::cig_bytecode::module::{Function, RESERVED_U32};
use cigscript::cig_bytecode::opcode::Op;

/// One handler of a function, as [`Flow`] reports it: the body's range, the
/// catch's entry (equal to the finally's entry when the handler has no
/// catch), and the finally's entry (or [`RESERVED_U32`] when there is none).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HandlerFlow {
    /// First pc of the body (inclusive, first-word).
    pub pc_start: u32,
    /// Pc after the body's last instruction.
    pub pc_end: u32,
    /// Where a raise in the body sends the frame (the catch's entry, or the
    /// finally's when the handler has no catch).
    pub handler_pc: u32,
    /// The finally's entry, or [`RESERVED_U32`] when there is none.
    pub finally_pc: u32,
    /// The register a catch binding takes, or [`RESERVED_U32`] when the
    /// handler has none.
    pub catch_reg: u32,
}

/// The bytecode's flow facts for one function.
#[derive(Clone, Debug)]
pub struct Flow {
    /// Every first-word pc that starts a block, ascending.
    pub leaders: Vec<u32>,
    /// For each instruction (by first-word pc): the pcs it can go on to, in
    /// the bytecode's own flow.
    pub succs: BTreeMap<u32, Vec<u32>>,
    /// The targets of backward jumps.
    pub loop_heads: BTreeSet<u32>,
    /// The registers that can carry a cell: the `A` of every `cellnew`.
    pub cells: BTreeSet<u32>,
    /// The function has a handler (a `try`).
    pub handlers: bool,
    /// The function's handlers, in the function's own handler-table order.
    /// Empty when `handlers` is false.
    pub handler_entries: Vec<HandlerFlow>,
    /// Where the function's native code can be entered, ascending: pc 0,
    /// the pc after every `call`, every loop head, every handler's
    /// `handler_pc` and `finally_pc`. The first is 0.
    pub resumes: Vec<u32>,
    /// Every first-word pc of the function. `succs`'s keys are a subset.
    pub pcs: Vec<u32>,
    /// The function's register count.
    pub reg_count: u32,
}

/// Build the flow of `f`.
///
/// Fails when the function does not decode (a bad opcode, a dangling or
/// doubled `WIDE`, an empty instruction stream). The error is a short
/// human-readable string: it names the pc.
pub fn flow(f: &Function) -> Result<Flow, String> {
    if f.instrs.is_empty() {
        return Err("empty function".to_string());
    }
    let (first_word_pcs, code) = decode_table(&f.instrs).map_err(|e| format!("{e:?}"))?;

    // Every instruction's first-word pc, ascending.
    let pcs: Vec<u32> = first_word_pcs.iter().collect();

    // Handlers, verbatim (sentinel `RESERVED_U32` for no catch_reg / no
    // finally_pc).
    let mut handler_entries = Vec::with_capacity(f.handlers.len());
    for h in &f.handlers {
        handler_entries.push(HandlerFlow {
            pc_start: h.pc_start,
            pc_end: h.pc_end,
            handler_pc: h.handler_pc,
            finally_pc: h.finally_pc,
            catch_reg: h.catch_reg,
        });
    }
    let handlers = !handler_entries.is_empty();

    // Successors.
    let mut succs: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for pc in &pcs {
        let Some(Some(d)) = code.get(*pc as usize) else {
            continue;
        };
        succs.insert(*pc, successors(d, &handler_entries));
    }

    // Cells: A of every cellnew.
    let mut cells: BTreeSet<u32> = BTreeSet::new();
    for d in code.iter().flatten() {
        if d.op == Op::CELLNEW {
            cells.insert(d.a);
        }
    }

    // Loop heads: targets of backward jumps.
    let mut loop_heads: BTreeSet<u32> = BTreeSet::new();
    for d in code.iter().flatten() {
        if let Some(target) = branch_target(d) {
            if target <= d.pc {
                loop_heads.insert(target);
            }
        }
    }

    // Leaders: pc 0; every branch target; the pc after every branching
    // instruction; every handler's handler_pc and finally_pc.
    let mut leader_set: BTreeSet<u32> = BTreeSet::new();
    leader_set.insert(0);
    for d in code.iter().flatten() {
        let next = d.pc + d.width;
        match d.op {
            Op::JMP | Op::LOOP => {
                if let Some(t) = branch_target(d) {
                    leader_set.insert(t);
                }
                // Nothing falls through a jump: what follows starts a
                // block of its own, reached or not.
                leader_set.insert(next);
            }
            Op::TEST
            | Op::TESTN
            | Op::TESTSET
            | Op::EQJ
            | Op::NEJ
            | Op::LTJ
            | Op::LEJ
            | Op::FORPREP
            | Op::FORLOOP
            | Op::ITERNEXT => {
                if let Some(t) = branch_target(d) {
                    leader_set.insert(t);
                }
                leader_set.insert(next);
            }
            Op::RET | Op::TAILCALL | Op::THROW => {
                // The frame leaves: `next` is not reached by fall-through.
                // It starts a block of its own, reached by a jump or not at
                // all.
                leader_set.insert(next);
            }
            // The frame is gone back to at the pc after a call.
            Op::CALL => {
                leader_set.insert(next);
            }
            Op::TRY_LEAVE => {
                leader_set.insert(next);
                for h in &handler_entries {
                    if (h.pc_start..h.pc_end).contains(&d.pc) && h.finally_pc != RESERVED_U32 {
                        leader_set.insert(h.finally_pc);
                    }
                }
            }
            _ => {}
        }
    }
    for h in &handler_entries {
        if h.handler_pc != RESERVED_U32 {
            leader_set.insert(h.handler_pc);
        }
        if h.finally_pc != RESERVED_U32 {
            leader_set.insert(h.finally_pc);
        }
    }
    // A leader must be a first-word pc; trim any that is not (a defensive
    // check, since the verifier already refuses a jump into a WIDE pair).
    let leaders: Vec<u32> = leader_set
        .into_iter()
        .filter(|pc| pcs.binary_search(pc).is_ok())
        .collect();

    // Resume points: every one is an instruction of the function, so a
    // call that ends the function has none after it.
    let mut resume_set: BTreeSet<u32> = loop_heads.clone();
    resume_set.insert(0);
    for d in code.iter().flatten() {
        if d.op == Op::CALL {
            resume_set.insert(d.pc + d.width);
        }
    }
    for h in &handler_entries {
        resume_set.extend([h.handler_pc, h.finally_pc]);
    }
    let resumes: Vec<u32> = resume_set
        .into_iter()
        .filter(|pc| pcs.binary_search(pc).is_ok())
        .collect();

    Ok(Flow {
        leaders,
        succs,
        resumes,
        loop_heads,
        cells,
        handlers,
        handler_entries,
        pcs,
        reg_count: f.reg_count,
    })
}

/// The pcs an instruction can go on to.
fn successors(d: &DecodedInstr, handlers: &[HandlerFlow]) -> Vec<u32> {
    let next = d.pc + d.width;
    let mut out: Vec<u32> = Vec::new();
    match d.op {
        Op::RET | Op::TAILCALL | Op::THROW => {}
        Op::JMP | Op::LOOP => {
            if let Some(t) = branch_target(d) {
                out.push(t);
            }
        }
        Op::TEST
        | Op::TESTN
        | Op::TESTSET
        | Op::EQJ
        | Op::NEJ
        | Op::LTJ
        | Op::LEJ
        | Op::FORPREP
        | Op::FORLOOP
        | Op::ITERNEXT => {
            out.push(next);
            if let Some(t) = branch_target(d) {
                if t != next {
                    out.push(t);
                }
            }
        }
        Op::TRY_LEAVE => {
            out.push(next);
            // Where a try.leave's pending finally resumes to is dynamic. A
            // sound over-approximation: every finally_pc of a handler whose
            // body range contains this instruction.
            for h in handlers {
                if (h.pc_start..h.pc_end).contains(&d.pc)
                    && h.finally_pc != RESERVED_U32
                    && !out.contains(&h.finally_pc)
                {
                    out.push(h.finally_pc);
                }
            }
        }
        _ => {
            out.push(next);
        }
    }
    out
}

/// The branch target of an instruction that carries one, or `None`. The
/// offset is in instruction words from the pc after the first word.
fn branch_target(d: &DecodedInstr) -> Option<u32> {
    match d.op {
        // AsBx jumps: offset is `b_s`.
        Op::JMP | Op::LOOP | Op::TEST | Op::TESTN | Op::FORPREP | Op::FORLOOP | Op::ITERNEXT => {
            jump_target(d.pc, d.b_s)
        }
        // ABC fused jumps: offset is `c_s`.
        Op::TESTSET | Op::EQJ | Op::NEJ | Op::LTJ | Op::LEJ => jump_target(d.pc, d.c_s),
        _ => None,
    }
}

/// `pc + 1 + offset` as the bytecode's rule has it, as a `u32`. `None` when
/// the arithmetic would wrap (the verifier has already refused that).
fn jump_target(pc: u32, offset: i32) -> Option<u32> {
    let i: i64 = i64::from(pc) + 1 + i64::from(offset);
    u32::try_from(i).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cigscript::cig_bytecode::instr::Instr;
    use cigscript::cig_bytecode::module::{FuncKind, Function, HandlerEntry, SpanBase};

    fn mk(instrs: Vec<Instr>, handlers: Vec<HandlerEntry>) -> Function {
        Function {
            name: String::new(),
            kind: FuncKind::Pull,
            reg_count: 8,
            arity: 0,
            requires_burn: false,
            upvalues: Vec::new(),
            constants: Vec::new(),
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
    fn a_ret_has_no_successors() {
        // loadnull r0; ret r0
        let instrs = vec![
            Instr::abc(Op::LOADNULL, 0, 0, 0),
            Instr::abc(Op::RET, 0, 0, 0),
        ];
        let f = mk(instrs, vec![]);
        let fl = flow(&f).unwrap();
        assert_eq!(fl.succs.get(&0), Some(&vec![1]));
        assert_eq!(fl.succs.get(&1), Some(&vec![]));
        assert!(!fl.handlers);
        assert_eq!(fl.pcs, vec![0, 1]);
        assert_eq!(fl.reg_count, 8);
    }

    #[test]
    fn a_conditional_jump_has_two_successors() {
        // test r0, +2 ; loadnull r1 ; loadnull r2 ; ret r0
        let instrs = vec![
            Instr::asbx(Op::TEST, 0, 2),
            Instr::abc(Op::LOADNULL, 1, 0, 0),
            Instr::abc(Op::LOADNULL, 2, 0, 0),
            Instr::abc(Op::RET, 0, 0, 0),
        ];
        let f = mk(instrs, vec![]);
        let fl = flow(&f).unwrap();
        // test at pc 0 with sBx=+2: fall-through pc 1, jump pc 3.
        let mut s0 = fl.succs.get(&0).unwrap().clone();
        s0.sort();
        assert_eq!(s0, vec![1, 3]);
        assert_eq!(fl.leaders, vec![0, 1, 3]);
        // A backward jump makes a loop head.
    }

    #[test]
    fn a_backward_jump_makes_a_loop_head() {
        // loadnull r0 ; jmp -2 ; ret r0
        let instrs = vec![
            Instr::abc(Op::LOADNULL, 0, 0, 0),
            Instr::asbx(Op::JMP, 0, -2),
            Instr::abc(Op::RET, 0, 0, 0),
        ];
        let f = mk(instrs, vec![]);
        let fl = flow(&f).unwrap();
        // jmp at pc 1 with offset -2: target = 1 + 1 + (-2) = 0.
        assert_eq!(fl.succs.get(&1), Some(&vec![0]));
        assert!(fl.loop_heads.contains(&0));
    }

    #[test]
    fn an_instruction_that_jumps_to_itself_is_a_loop_head() {
        // loadnull r0 ; jmp -1 (to itself) ; ret r0
        let instrs = vec![
            Instr::abc(Op::LOADNULL, 0, 0, 0),
            Instr::asbx(Op::JMP, 0, -1),
            Instr::abc(Op::RET, 0, 0, 0),
        ];
        let fl = flow(&mk(instrs, vec![])).unwrap();
        assert_eq!(fl.succs.get(&1), Some(&vec![1]));
        assert!(fl.loop_heads.contains(&1));
        // What follows a jump starts a block of its own.
        assert_eq!(fl.leaders, vec![0, 1, 2]);
        assert_eq!(fl.resumes, vec![0, 1]);
    }

    #[test]
    fn a_body_can_be_entered_at_its_start_after_a_call_and_at_a_loops_start() {
        // 0 loadnull r0 ; 1 call r0 (no arguments) ; 2 test r0, -2 (to 1) ;
        // 3 call r0 ; 4 ret r0
        let instrs = vec![
            Instr::abc(Op::LOADNULL, 0, 0, 0),
            Instr::abc(Op::CALL, 0, 0, 0),
            Instr::asbx(Op::TEST, 0, -2),
            Instr::abc(Op::CALL, 0, 0, 0),
            Instr::abc(Op::RET, 0, 0, 0),
        ];
        let fl = flow(&mk(instrs, vec![])).unwrap();
        // The start, the loop's start, and the pc after each call.
        assert_eq!(fl.resumes, vec![0, 1, 2, 4]);
        // Every resume point starts a block.
        for pc in &fl.resumes {
            assert!(fl.leaders.contains(pc), "{pc}");
        }
        // A call that ends the function has no instruction after it.
        let instrs = vec![
            Instr::abc(Op::LOADNULL, 0, 0, 0),
            Instr::abc(Op::CALL, 0, 0, 0),
        ];
        assert_eq!(flow(&mk(instrs, vec![])).unwrap().resumes, vec![0]);
    }

    #[test]
    fn cellnew_marks_the_cell_register() {
        let instrs = vec![
            Instr::abc(Op::LOADNULL, 2, 0, 0),
            Instr::abc(Op::CELLNEW, 2, 0, 0),
            Instr::abc(Op::RET, 2, 0, 0),
        ];
        let f = mk(instrs, vec![]);
        let fl = flow(&f).unwrap();
        assert!(fl.cells.contains(&2));
        assert!(!fl.cells.contains(&0));
    }

    #[test]
    fn a_handler_turns_the_flag_on_and_lists_its_pcs() {
        // Minimal shape: the try's body is one `nop`, then try.leave; the
        // handler_pc and finally_pc are later ret instructions. The verifier
        // is not run here (this is a Flow test), so the handler table just
        // names pcs.
        let instrs = vec![
            Instr::abc(Op::TRY_ENTER, 0, 0, 0), // pc 0
            Instr::abc(Op::NOP, 0, 0, 0),       // pc 1
            Instr::abc(Op::TRY_LEAVE, 0, 0, 0), // pc 2
            Instr::abc(Op::JMP, 0, 0, 0),       // pc 3 (over handler)
            Instr::abc(Op::NOP, 0, 0, 0),       // pc 4  (handler_pc)
            Instr::abc(Op::TRY_LEAVE, 0, 0, 0), // pc 5
            Instr::abc(Op::NOP, 0, 0, 0),       // pc 6  (finally_pc)
            Instr::abc(Op::TRY_LEAVE, 0, 0, 0), // pc 7
            Instr::abc(Op::RET, 0, 0, 0),       // pc 8
        ];
        let handlers = vec![HandlerEntry {
            pc_start: 1,
            pc_end: 3,
            handler_pc: 4,
            catch_reg: RESERVED_U32,
            finally_pc: 6,
        }];
        let f = mk(instrs, handlers);
        let fl = flow(&f).unwrap();
        assert!(fl.handlers);
        assert_eq!(fl.handler_entries.len(), 1);
        assert_eq!(fl.handler_entries[0].handler_pc, 4);
        assert_eq!(fl.handler_entries[0].finally_pc, 6);
        // try.leave at pc 2 goes on at pc 3 and can resume into finally_pc 6.
        let mut s2 = fl.succs.get(&2).unwrap().clone();
        s2.sort();
        assert_eq!(s2, vec![3, 6]);
        // The handler's handler_pc and finally_pc are leaders.
        assert!(fl.leaders.contains(&4));
        assert!(fl.leaders.contains(&6));
    }

    #[test]
    fn an_empty_function_fails_to_flow() {
        let f = mk(vec![], vec![]);
        assert!(flow(&f).is_err());
    }
}
