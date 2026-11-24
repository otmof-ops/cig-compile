// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2

//! Types-pass tests on real scripts: lower them, parse the bytecode, run
//! the flow and the types passes, and assert on the register types at
//! named places.
//!
//! These check what the pass says about typical shapes; the per-opcode
//! transfer rules are held by the unit tests in `src/types.rs`.

use cig_codegen::flow::flow;
use cig_codegen::types::{infer, reads, writes, Ty};
use cigscript::cig_bytecode::instr::decode_table;
use cigscript::cig_bytecode::module::{Function, Module};
use cigscript::cig_bytecode::opcode::Op;
use cigscript::cig_bytecode::parse_module;
use cigscript::lower::module_bytes;
use cigscript::syntax::parse;

/// Lower a script to its bytecode module.
fn lower(src: &str) -> Module {
    let ast = parse(src).expect("the script parses");
    let bytes = module_bytes(&ast, "t.cig").expect("the script lowers");
    parse_module(&bytes).expect("the module parses")
}

/// Find the first function (at any depth of the tree) with `name`.
fn func<'a>(m: &'a Module, name: &str) -> &'a Function {
    fn walk<'a>(f: &'a Function, name: &str) -> Option<&'a Function> {
        if f.name == name {
            return Some(f);
        }
        for c in &f.children {
            if let Some(f) = walk(c, name) {
                return Some(f);
            }
        }
        None
    }
    walk(&m.main, name).unwrap_or_else(|| panic!("no function named `{name}`"))
}

/// The first first-word pc of the instruction whose opcode is `op`. For
/// control-flow instructions this is the instruction the body jumps back
/// to on each turn of a loop.
fn pc_of_op(f: &Function, op: Op) -> u32 {
    let (_, code) = decode_table(&f.instrs).expect("decode");
    for d in code.iter().flatten() {
        if d.op == op {
            return d.pc;
        }
    }
    panic!("no `{op:?}` in function `{}`", f.name);
}

#[test]
fn a_counted_loop_s_counter_and_accumulator_are_ints_inside_the_loop() {
    let src = "\
pull square_sum(n) {
  roll total = 0
  for i in 0..n {
    total = total + i * i % 7
  }
  snuff total
}
exhale square_sum(5)
";
    let m = lower(src);
    let f = func(&m, "square_sum");
    let fl = flow(f).expect("flow");
    let t = infer(f, &fl);
    // At the FORLOOP instruction's pc, the `A` register (the loop's running
    // start value) is Int; `A+3` is the induction variable, also Int. The
    // accumulator `total` was initialized to an Int outside and only
    // updated by `+ i * i % 7` (an Int by our transfer rules), so it is
    // Int when the FORLOOP runs.
    let forloop_pc = pc_of_op(f, Op::FORLOOP);
    let (_, code) = decode_table(&f.instrs).unwrap();
    let d = code[forloop_pc as usize].unwrap();
    let a = d.a;
    let row = t.before(forloop_pc);
    assert_eq!(row[a as usize], Ty::Int, "running value");
    assert_eq!(row[(a + 3) as usize], Ty::Int, "induction variable");
}

#[test]
fn int_plus_int_inside_a_counted_loop_is_int() {
    // Inside the loop body, both `total` (Int by initialization and only
    // updated with an Int result) and `i` (Int from `forprep`) are Int, so
    // the `ADD` or `ADDK` result is Int.
    let src = "\
pull sum_to(n) {
  roll total = 0
  for i in 0..n { total = total + i }
  snuff total
}
exhale sum_to(5)
";
    let m = lower(src);
    let f = func(&m, "sum_to");
    let fl = flow(f).expect("flow");
    let t = infer(f, &fl);
    let (_, code) = decode_table(&f.instrs).unwrap();
    // Collect every ADD/ADDK inside the loop's body range (between the
    // FORPREP and its FORLOOP, exclusive), and check the result is Int.
    let forprep_pc = pc_of_op(f, Op::FORPREP);
    let forloop_pc = pc_of_op(f, Op::FORLOOP);
    let mut tested = 0;
    for pc in &fl.pcs {
        if *pc <= forprep_pc || *pc >= forloop_pc {
            continue;
        }
        let d = code[*pc as usize].unwrap();
        if matches!(d.op, Op::ADD | Op::ADDK) {
            let after = *pc + d.width;
            let row = t.before(after);
            assert_eq!(row[d.a as usize], Ty::Int, "ADD/ADDK result in body");
            tested += 1;
        }
    }
    assert!(tested >= 1, "no ADD/ADDK inside the for loop");
}

#[test]
fn an_int_divided_by_an_int_is_any() {
    let src = "\
pull quo(a, b) { snuff a / b }
exhale quo(10, 3)
";
    let m = lower(src);
    let f = func(&m, "quo");
    let fl = flow(f).expect("flow");
    let t = infer(f, &fl);
    let (_, code) = decode_table(&f.instrs).unwrap();
    for pc in &fl.pcs {
        let d = code[*pc as usize].unwrap();
        if matches!(d.op, Op::DIV | Op::DIVK) {
            let row = t.before(*pc + d.width);
            assert_eq!(row[d.a as usize], Ty::Any, "int/int is Any");
            return;
        }
    }
    panic!("`quo` has no DIV/DIVK instruction");
}

#[test]
fn a_function_with_a_try_has_every_register_any_at_every_pc() {
    let src = "\
pull guarded(a) {
  try {
    snuff a + 1
  } ashtray e {
    snuff 0
  }
}
exhale guarded(5)
";
    let m = lower(src);
    let f = func(&m, "guarded");
    let fl = flow(f).expect("flow");
    let t = infer(f, &fl);
    assert!(fl.handlers);
    for pc in &fl.pcs {
        for r in 0..f.reg_count as usize {
            assert_eq!(t.before(*pc)[r], Ty::Any, "pc {pc} r {r}");
        }
    }
}

#[test]
fn the_same_function_gives_the_same_flow_and_types() {
    let src = "\
pull f(n) {
  roll s = 0
  for i in 0..n { s = s + i }
  snuff s
}
exhale f(3)
";
    let m1 = lower(src);
    let m2 = lower(src);
    let f1 = func(&m1, "f");
    let f2 = func(&m2, "f");
    let fl1 = flow(f1).unwrap();
    let fl2 = flow(f2).unwrap();
    assert_eq!(fl1.pcs, fl2.pcs);
    assert_eq!(fl1.succs, fl2.succs);
    assert_eq!(fl1.leaders, fl2.leaders);
    assert_eq!(fl1.loop_heads, fl2.loop_heads);
    assert_eq!(fl1.cells, fl2.cells);
    let t1 = infer(f1, &fl1);
    let t2 = infer(f2, &fl2);
    for pc in &fl1.pcs {
        assert_eq!(t1.before(*pc), t2.before(*pc), "pc {pc}");
    }
}

#[test]
fn a_register_after_a_call_is_any() {
    let src = "\
pull one() { snuff 1 }
pull uses(n) {
  roll x = n
  roll y = one()
  snuff x + y
}
exhale uses(2)
";
    let m = lower(src);
    let f = func(&m, "uses");
    let fl = flow(f).expect("flow");
    let t = infer(f, &fl);
    let (_, code) = decode_table(&f.instrs).unwrap();
    for pc in &fl.pcs {
        let d = code[*pc as usize].unwrap();
        if d.op == Op::CALL {
            let after = *pc + d.width;
            let row = t.before(after);
            assert_eq!(row[d.a as usize], Ty::Any, "call result is Any");
            return;
        }
    }
    panic!("`uses` has no CALL instruction");
}

#[test]
fn reads_and_writes_match_the_opcode_table() {
    // A handful of ops, decoded straight: reads and writes are the ones
    // BYTECODE.md declares.
    use cigscript::cig_bytecode::instr::Instr;
    let decode_one = |i: Instr| -> cigscript::cig_bytecode::instr::DecodedInstr {
        let (_, c) = decode_table(&[i, Instr::abc(Op::RET, 0, 0, 0)]).unwrap();
        c[0].unwrap()
    };
    let d = decode_one(Instr::abc(Op::MOV, 2, 3, 0));
    assert_eq!(reads(&d), vec![3]);
    assert_eq!(writes(&d).regs.len(), 1);

    let d = decode_one(Instr::abc(Op::EXHALE, 1, 3, 0));
    // A range of three registers: 1, 2, 3.
    assert_eq!(reads(&d), vec![1, 2, 3]);
    assert!(writes(&d).regs.is_empty());

    let d = decode_one(Instr::abc(Op::CALL, 2, 1, 0));
    // callee = R(2); one arg = R(3).
    assert_eq!(reads(&d), vec![2, 3]);
    let w = writes(&d);
    assert_eq!(w.regs[0], (2, Ty::Any)); // result
                                         // C = 0: no extra row registers.
    assert_eq!(w.above, Some(3));
}
