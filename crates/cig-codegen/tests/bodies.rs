// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2

//! Bodies from bytecode (decision 0008), on their own: which functions
//! get one and where it is entered, that the same input gives the same
//! bytes for either machine, and, on a machine with a loader, what a
//! body does to a real frame: a loop that runs to its end in machine
//! code, the step budget, an operand an inline form does not cover, a
//! stretch the interpreter runs, a call carried in one, an int divided
//! by an int, and a register that is not of its proven type.
//!
//! What a whole program does with its bodies is the differential
//! suite's (`tests/differential.rs`, `DIFF_BODIES`): every script, held
//! to the interpreter.

use cig_codegen::{bodies, bodies_as, Mode, Rule};
use cig_native::Arch;
use cigscript::cig_bytecode::instr::Instr;
use cigscript::cig_bytecode::module::{Constant, FuncKind, Function, Module, SpanBase};
use cigscript::cig_bytecode::opcode::Op;

fn function(reg_count: u32, arity: u16, instrs: Vec<Instr>) -> Function {
    Function {
        name: String::new(),
        kind: FuncKind::Main,
        reg_count,
        arity,
        requires_burn: false,
        upvalues: Vec::new(),
        constants: Vec::new(),
        children: Vec::new(),
        regions: Vec::new(),
        handlers: Vec::new(),
        instrs,
        span_base: SpanBase::default(),
        lines: Vec::new(),
        region_rows: Vec::new(),
    }
}

fn module(main: Function) -> Module {
    Module {
        source_name: String::new(),
        main,
    }
}

fn steps(nest: u8, n: u16) -> Instr {
    Instr::abx(Op::STEPS, nest, n)
}

/// `total = 0; for i in 0..10 { total = total + i }; return total`,
/// with every register an int the type pass can prove.
///
/// ```text
/// 0 loadint R0, 0      total
/// 1 loadint R1, 0      the window: start,
/// 2 loadint R2, 10       limit,
/// 3 loadint R3, 1        step
/// 4 forprep R1 -> 8
/// 5 steps 1, 1         <- the loop: three instructions a turn
/// 6 add R0, R0, R4
/// 7 forloop R1 -> 5
/// 8 ret R0
/// ```
fn sum_loop() -> Module {
    module(function(
        5,
        0,
        vec![
            Instr::asbx(Op::LOADINT, 0, 0),
            Instr::asbx(Op::LOADINT, 1, 0),
            Instr::asbx(Op::LOADINT, 2, 10),
            Instr::asbx(Op::LOADINT, 3, 1),
            Instr::asbx(Op::FORPREP, 1, 3),
            steps(1, 1),
            Instr::abc(Op::ADD, 0, 0, 4),
            Instr::asbx(Op::FORLOOP, 1, -3),
            Instr::abc(Op::RET, 0, 0, 0),
        ],
    ))
}

/// A loop of the same shape whose turn makes a list and calls it: no
/// instruction of it is faster as machine code.
fn call_loop() -> Module {
    module(function(
        7,
        0,
        vec![
            Instr::asbx(Op::LOADINT, 0, 0),
            Instr::asbx(Op::LOADINT, 1, 0),
            Instr::asbx(Op::LOADINT, 2, 10),
            Instr::asbx(Op::LOADINT, 3, 1),
            Instr::asbx(Op::FORPREP, 1, 4),
            steps(1, 1),
            Instr::abc(Op::NEWLIST, 5, 0, 0),
            Instr::abc(Op::CALL, 5, 0, 0),
            Instr::asbx(Op::FORLOOP, 1, -4),
            Instr::abc(Op::RET, 0, 0, 0),
        ],
    ))
}

/// `fn(a, b) { return a + b }`: its operands are whatever the caller
/// passed.
fn add_args() -> Module {
    module(function(
        3,
        2,
        vec![Instr::abc(Op::ADD, 2, 0, 1), Instr::abc(Op::RET, 2, 0, 0)],
    ))
}

/// `fn(xs) { total = 0; for i in 0..4 { total = total + xs[i] }; return total }`.
///
/// ```text
/// 0 loadint R1, 0      total
/// 1 loadint R2, 0      the window
/// 2 loadint R3, 4
/// 3 loadint R4, 1
/// 4 forprep R2 -> 10
/// 5 steps 1, 1         <- the loop
/// 6 getindex R6, R0, R5
/// 7 nop                the read's position
/// 8 add R1, R1, R6
/// 9 forloop R2 -> 5
/// 10 ret R1
/// ```
fn sum_list() -> Module {
    module(function(
        7,
        1,
        vec![
            Instr::asbx(Op::LOADINT, 1, 0),
            Instr::asbx(Op::LOADINT, 2, 0),
            Instr::asbx(Op::LOADINT, 3, 4),
            Instr::asbx(Op::LOADINT, 4, 1),
            Instr::asbx(Op::FORPREP, 2, 5),
            steps(1, 1),
            Instr::abc(Op::GETINDEX, 6, 0, 5),
            Instr::abc(Op::NOP, 0, 0, 0),
            Instr::abc(Op::ADD, 1, 1, 6),
            Instr::asbx(Op::FORLOOP, 2, -5),
            Instr::abc(Op::RET, 1, 0, 0),
        ],
    ))
}

/// `fn(xs) { for i in 0..4 { xs[i] = i } }`, the write followed by the
/// two `nop`s that carry its positions.
fn fill_list() -> Module {
    module(function(
        6,
        1,
        vec![
            Instr::asbx(Op::LOADINT, 1, 0),
            Instr::asbx(Op::LOADINT, 2, 4),
            Instr::asbx(Op::LOADINT, 3, 1),
            Instr::asbx(Op::FORPREP, 1, 5),
            steps(1, 1),
            Instr::abc(Op::SETINDEX, 0, 4, 4),
            Instr::abc(Op::NOP, 0, 0, 0),
            Instr::abc(Op::NOP, 0, 0, 0),
            Instr::asbx(Op::FORLOOP, 1, -5),
            Instr::abc(Op::RET, 0, 0, 0),
        ],
    ))
}

/// A loop of twenty proven adds, a list made and called:
///
/// ```text
/// 0 loadint R0, 0      total
/// 1..3                 the window: 0, 3, 1
/// 4 forprep R1 -> 29
/// 5 steps 1, 1         <- the loop
/// 6..25 add R0, R0, R4
/// 26 newlist R5        the interpreter's,
/// 27 call R5           and the VM's
/// 28 forloop R1 -> 5
/// 29 ret R0
/// ```
fn work_and_call() -> Module {
    let mut code = vec![
        Instr::asbx(Op::LOADINT, 0, 0),
        Instr::asbx(Op::LOADINT, 1, 0),
        Instr::asbx(Op::LOADINT, 2, 3),
        Instr::asbx(Op::LOADINT, 3, 1),
        Instr::asbx(Op::FORPREP, 1, 24),
        steps(1, 1),
    ];
    code.extend(std::iter::repeat_n(Instr::abc(Op::ADD, 0, 0, 4), 20));
    code.extend([
        Instr::abc(Op::NEWLIST, 5, 0, 0),
        Instr::abc(Op::CALL, 5, 0, 0),
        Instr::asbx(Op::FORLOOP, 1, -24),
        Instr::abc(Op::RET, 0, 0, 0),
    ]);
    module(function(7, 0, code))
}

/// `fn(xs) { c = 0; for b in xs { if b { c = c + 1 } }; return c }`.
///
/// ```text
/// 0 loadint R1, 0      c
/// 1 loadint R6, 1
/// 2 mov R2, R0         what the loop walks
/// 3 iterprep R2        R3 = its items, R4 = 0
/// 4 iternext R2 -> 9   <- the loop: R5 = the next item
/// 5 steps 1, 1
/// 6 testn R5 -> 8
/// 7 add R1, R1, R6
/// 8 loop -> 4
/// 9 ret R1
/// ```
fn count_true() -> Module {
    module(function(
        7,
        1,
        vec![
            Instr::asbx(Op::LOADINT, 1, 0),
            Instr::asbx(Op::LOADINT, 6, 1),
            Instr::abc(Op::MOV, 2, 0, 0),
            Instr::abc(Op::ITERPREP, 2, 0, 0),
            Instr::asbx(Op::ITERNEXT, 2, 4),
            steps(1, 1),
            Instr::asbx(Op::TESTN, 5, 1),
            Instr::abc(Op::ADD, 1, 1, 6),
            Instr::asbx(Op::LOOP, 0, -5),
            Instr::abc(Op::RET, 1, 0, 0),
        ],
    ))
}

/// `fn(xs) { for i in 0..4 { xs.push(i) }; return xs }`, the receiver
/// and the value moved to the call's registers as the lowering has
/// them.
///
/// ```text
/// 0..2                 the window: 0, 4, 1
/// 3 forprep R1 -> 9
/// 4 steps 1, 1         <- the loop
/// 5 mov R5, R0         the list
/// 6 mov R6, R4         the value
/// 7 callm R5, "push", 1
/// 8 forloop R1 -> 4
/// 9 ret R0
/// ```
fn push_all() -> Module {
    let mut f = function(
        7,
        1,
        vec![
            Instr::asbx(Op::LOADINT, 1, 0),
            Instr::asbx(Op::LOADINT, 2, 4),
            Instr::asbx(Op::LOADINT, 3, 1),
            Instr::asbx(Op::FORPREP, 1, 5),
            steps(1, 1),
            Instr::abc(Op::MOV, 5, 0, 0),
            Instr::abc(Op::MOV, 6, 4, 0),
            Instr::abc(Op::CALLM, 5, 0, 1),
            Instr::asbx(Op::FORLOOP, 1, -5),
            Instr::abc(Op::RET, 0, 0, 0),
        ],
    );
    f.constants = vec![Constant::Str("push".to_string())];
    module(f)
}

#[test]
fn a_loop_over_a_lists_items_and_a_loop_that_pushes_get_bodies() {
    let out = bodies(&count_true(), Rule::Loops, Arch::X86_64).expect("bodies");
    assert_eq!(out.bodies[0].resumes, vec![4]);
    assert_eq!(
        out.report[0].inline,
        (5, 10),
        "the next item, the mark, the test, the add and the jump back"
    );
    let out = bodies(&push_all(), Rule::Loops, Arch::X86_64).expect("bodies");
    assert_eq!(out.bodies[0].resumes, vec![4]);
    assert_eq!(out.report[0].inline, (5, 10));
}

/// A module whose top level only makes one function, `child`.
fn with_child(child: Function) -> Module {
    let mut main = function(
        1,
        0,
        vec![Instr::abx(Op::CLOSURE, 0, 0), Instr::abc(Op::RET, 0, 0, 0)],
    );
    main.children.push(Function {
        kind: FuncKind::Pull,
        name: "f".to_string(),
        ..child
    });
    module(main)
}

#[test]
fn a_functions_straight_code_gets_a_body_when_there_is_enough_of_it() {
    // `fn(a) { r = 1; r = r + r, forty times; return r }`: ints the
    // type pass proves from the second instruction on.
    let mut code = vec![Instr::asbx(Op::LOADINT, 1, 1)];
    code.extend(std::iter::repeat_n(Instr::abc(Op::ADD, 1, 1, 1), 40));
    code.push(Instr::abc(Op::RET, 1, 0, 0));
    let m = with_child(function(2, 1, code));
    let out = bodies(&m, Rule::Loops, Arch::X86_64).expect("bodies");
    assert_eq!(out.bodies.len(), 1);
    assert_eq!(out.bodies[0].func, 1);
    assert_eq!(
        out.bodies[0].resumes,
        vec![0],
        "entered at the function's first instruction"
    );
    assert_eq!(out.report[1].inline, (41, 42), "all but the return");
    // The top level runs once: its straight code is the bytecode's.
    assert_eq!(out.report[0].why, "it has no loop");
    // Two instructions do not pay for entering and leaving.
    let short = function(
        3,
        2,
        vec![Instr::abc(Op::ADD, 2, 0, 1), Instr::abc(Op::RET, 2, 0, 0)],
    );
    let out = bodies(&with_child(short), Rule::Loops, Arch::X86_64).expect("bodies");
    assert!(out.bodies.is_empty());
    assert!(out.report[1].weighed);
    assert_eq!(out.report[1].why, "it would not run faster as machine code");
}

#[test]
fn a_call_among_enough_native_work_is_carried_in_a_stretch() {
    let out = bodies(&work_and_call(), Rule::Loops, Arch::X86_64).expect("bodies");
    assert_eq!(out.bodies.len(), 1);
    // The loop's start, and the instruction after the stretch that
    // holds the call: where the bytecode comes back to machine code
    // when the runtime could not carry the call.
    assert_eq!(out.bodies[0].resumes, vec![5, 28]);
    assert_eq!(
        out.report[0].inline,
        (22, 30),
        "the step mark, the adds and the `forloop`"
    );
}

#[test]
fn a_loop_that_reads_a_list_by_an_int_gets_a_body() {
    let out = bodies(&sum_list(), Rule::Loops, Arch::X86_64).expect("bodies");
    assert_eq!(out.bodies.len(), 1);
    assert_eq!(out.bodies[0].resumes, vec![5]);
    // The loop's five instructions: the step mark, the read, its `nop`,
    // the add and the `forloop`.
    assert_eq!(out.report[0].inline, (5, 11));
    let out = bodies(&fill_list(), Rule::Loops, Arch::X86_64).expect("bodies");
    assert_eq!(out.bodies[0].resumes, vec![4]);
    assert_eq!(out.report[0].inline, (5, 10));
}

#[test]
fn the_rule_none_gives_no_function_a_body() {
    let out = bodies(&sum_loop(), Rule::None, Arch::X86_64).expect("bodies");
    assert!(out.bodies.is_empty() && out.code.is_empty());
    assert_eq!(out.rule, Rule::None);
    assert_eq!(out.report.len(), 1);
    assert_eq!(out.report[0].why, "the rule gives no function a body");
}

#[test]
fn a_top_level_without_a_loop_has_no_body() {
    // It runs once: nothing of it is weighed.
    let out = bodies(&add_args(), Rule::Loops, Arch::X86_64).expect("bodies");
    assert!(out.bodies.is_empty());
    assert_eq!(out.report[0].why, "it has no loop");
    assert!(!out.report[0].weighed);
}

#[test]
fn a_loop_of_proven_arithmetic_gets_a_body_entered_at_the_loop() {
    let out = bodies(&sum_loop(), Rule::Loops, Arch::X86_64).expect("bodies");
    assert_eq!(out.bodies.len(), 1);
    assert_eq!(out.bodies[0].func, 0);
    // The loop's start, and nothing else: the code before the loop and
    // the return after it run once and are the bytecode's.
    assert_eq!(out.bodies[0].resumes, vec![5]);
    let r = &out.report[0];
    assert!(r.compiled && r.why.is_empty());
    assert_eq!(r.inline, (3, 9), "the loop's three instructions of nine");
    assert_eq!(r.code_bytes as usize, out.code.len());
}

#[test]
fn a_loop_the_interpreter_runs_anyway_gets_no_body() {
    let out = bodies(&call_loop(), Rule::Loops, Arch::X86_64).expect("bodies");
    assert!(out.bodies.is_empty() && out.code.is_empty());
    let r = &out.report[0];
    assert!(!r.compiled && r.weighed);
    assert_eq!(r.why, "none of its loops would run faster as machine code");
}

#[test]
fn the_rule_every_gives_every_function_a_body_at_every_resume_point() {
    // The function's start, the loop's start, and (with a call) the
    // instruction after it.
    let out = bodies(&sum_loop(), Rule::Every, Arch::X86_64).expect("bodies");
    assert_eq!(out.bodies[0].resumes, vec![0, 5]);
    assert_eq!(out.report[0].inline, (8, 9), "all but the return");
    let out = bodies(&call_loop(), Rule::Every, Arch::X86_64).expect("bodies");
    assert_eq!(out.bodies[0].resumes, vec![0, 5, 8]);
    let out = bodies(&add_args(), Rule::Every, Arch::X86_64).expect("bodies");
    assert_eq!(out.bodies[0].resumes, vec![0]);
}

#[test]
fn bodies_are_in_the_modules_order_and_join_a_section_after_its_code() {
    let mut main = sum_loop().main;
    main.children.push(Function {
        kind: FuncKind::Pull,
        ..sum_loop().main
    });
    let m = module(main);
    let out = bodies(&m, Rule::Loops, Arch::X86_64).expect("bodies");
    assert_eq!(
        out.bodies.iter().map(|b| b.func).collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert_eq!(out.bodies[0].offset, 0);
    assert!(out.bodies[1].offset > 0);
    let first = out.bodies[1].offset;
    let mut section = cig_native::Section {
        arch: Arch::X86_64,
        module: [0; 32],
        code: vec![0xC3; 5],
        entries: Vec::new(),
        sites: Vec::new(),
        bodies: Vec::new(),
        rule: Rule::None,
    };
    let len = out.code.len();
    out.join(&mut section).expect("joined");
    assert_eq!(section.rule, Rule::Loops);
    // The code that was there, padded to the machine's alignment, then
    // the bodies, each offset moved by as much.
    let base = section.bodies[0].offset;
    assert!(base >= 5 && section.bodies[1].offset == base + first);
    assert_eq!(section.code.len(), base as usize + len);
    section.to_bytes().expect("a section the format takes");
}

/// The bytes of a body are a function of the module, the rule, the
/// mode and the target, on every host: a fingerprint holds the
/// generator to that. A change here is a change to the generator, and
/// is made on purpose.
#[test]
fn the_same_input_gives_the_same_bytes_for_either_machine_on_every_host() {
    fn fnv(bytes: &[u8]) -> u64 {
        bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    }
    let m = sum_loop();
    for (arch, len, print) in [
        (Arch::X86_64, X86_64_SUM_LOOP.0, X86_64_SUM_LOOP.1),
        (Arch::Aarch64, AARCH64_SUM_LOOP.0, AARCH64_SUM_LOOP.1),
    ] {
        let a = bodies(&m, Rule::Loops, arch).expect("bodies");
        let b = bodies(&m, Rule::Loops, arch).expect("bodies");
        assert_eq!(a.code, b.code, "{arch:?}: twice the same");
        assert_eq!(a.bodies, b.bodies);
        assert_eq!(
            (a.code.len(), fnv(&a.code)),
            (len, print),
            "{arch:?}: the generator's bytes changed"
        );
    }
    let arm = bodies(&m, Rule::Loops, Arch::Aarch64).expect("bodies");
    assert_eq!(arm.code.len() % 4, 0, "whole instructions");
    // A mode is part of the input: the same function, other bytes.
    let held = bodies_as(&m, Rule::Every, Arch::X86_64, Mode::Promoted).expect("bodies");
    let frame = bodies_as(&m, Rule::Every, Arch::X86_64, Mode::Frame).expect("bodies");
    let checked = bodies_as(&m, Rule::Every, Arch::X86_64, Mode::Checked).expect("bodies");
    assert_ne!(held.code, frame.code);
    assert_ne!(frame.code, checked.code);
    assert_eq!(held.bodies[0].resumes, checked.bodies[0].resumes);
}

/// `sum_loop` under `Rule::Loops`: the body's length and fingerprint.
const X86_64_SUM_LOOP: (usize, u64) = (1479, 18_395_959_923_850_581_249);
const AARCH64_SUM_LOOP: (usize, u64) = (1224, 17_820_732_550_131_740_509);

/// On Linux x86-64 and aarch64 a body is mapped and entered on a real
/// frame, with a host that records what the body asks of the runtime.
#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod runs {
    use super::*;
    use cig_native::{Context, Host, Mapped, Section, Stop, Tables};
    use cigscript::value::Value;
    use std::io::Write;

    /// A host that answers from a script and records how it was asked.
    #[derive(Default)]
    struct Recording {
        /// What `run` returns, one a call, in order.
        run_returns: Vec<Stop>,
        run_calls: Vec<(u32, u32)>,
        /// What `again` returns, one a call, in order.
        again_returns: Vec<Stop>,
        again_calls: Vec<u32>,
        /// The registers `list` was asked for, in order.
        list_calls: Vec<u32>,
        /// What `push`, `copy` and `clear` were asked for, in order.
        /// Each does what the runtime's does, on the frame.
        push_calls: Vec<(u32, u32, u32)>,
        copy_calls: Vec<(u32, u32)>,
        clear_calls: Vec<u32>,
        /// `push`, `copy` and `clear` say they cannot, and do nothing.
        refuses: bool,
        /// The frame the body runs on: where `list` finds a register.
        frame: Option<(*mut Value, usize)>,
    }

    impl Recording {
        /// The frame's registers.
        fn regs(&mut self) -> &mut [Value] {
            let (frame, len) = self.frame.expect("a frame");
            // SAFETY: `enter` set `frame` to the frame the body runs on,
            // which outlives the body's run and is touched by nothing
            // else while a helper runs.
            unsafe { std::slice::from_raw_parts_mut(frame, len) }
        }
    }

    impl Host for Recording {
        fn run(&mut self, _ctx: &mut Context, from: u32, to: u32) -> Stop {
            self.run_calls.push((from, to));
            assert!(!self.run_returns.is_empty(), "run was not expected");
            self.run_returns.remove(0)
        }
        fn list(&mut self, ctx: &mut Context, reg: u32) -> bool {
            self.list_calls.push(reg);
            let (frame, len) = self.frame.expect("a frame");
            assert!((reg as usize) < len);
            // SAFETY: `enter` set `frame` to the frame the body runs on,
            // which outlives the body's run and is touched by nothing
            // else while a helper runs.
            let value = unsafe { &*frame.add(reg as usize) };
            let Value::List(list) = value else {
                return false;
            };
            let Ok(mut elements) = list.try_borrow_mut() else {
                return false;
            };
            ctx.left = elements.as_mut_ptr() as u64;
            ctx.right = elements.len() as u64;
            true
        }
        fn again(&mut self, _ctx: &mut Context, pc: u32) -> Stop {
            self.again_calls.push(pc);
            assert!(!self.again_returns.is_empty(), "again was not expected");
            self.again_returns.remove(0)
        }
        fn push(&mut self, _ctx: &mut Context, list: u32, value: u32, dst: u32) -> bool {
            self.push_calls.push((list, value, dst));
            if self.refuses {
                return false;
            }
            let regs = self.regs();
            let Value::List(items) = &regs[list as usize] else {
                return false;
            };
            let items = items.clone();
            let v = regs[value as usize].clone();
            items.borrow_mut().push(v);
            regs[dst as usize] = Value::List(items);
            true
        }
        fn copy(&mut self, _ctx: &mut Context, dst: u32, src: u32) -> bool {
            self.copy_calls.push((dst, src));
            if self.refuses {
                return false;
            }
            let regs = self.regs();
            regs[dst as usize] = regs[src as usize].clone();
            true
        }
        fn clear(&mut self, _ctx: &mut Context, reg: u32) -> bool {
            self.clear_calls.push(reg);
            if self.refuses {
                return false;
            }
            self.regs()[reg as usize] = Value::Null;
            true
        }
    }

    /// `m`'s bodies under `rule` and `mode`, mapped and armed, with
    /// their rows.
    fn load(m: &Module, rule: Rule, mode: Mode) -> (cig_native::Code, Vec<cig_native::Body>) {
        let arch = Arch::host().expect("a machine with a loader");
        let out = bodies_as(m, rule, arch, mode).expect("bodies");
        let rows = out.bodies.clone();
        let section = Section {
            arch,
            module: [0u8; 32],
            code: out.code,
            entries: Vec::new(),
            sites: Vec::new(),
            bodies: out.bodies,
            rule: out.rule,
        };
        let body = section.to_bytes().expect("a section");
        let mut file = tempfile::NamedTempFile::new().expect("a temp file");
        file.write_all(&vec![0u8; 65_536]).expect("written");
        file.write_all(&body).expect("written");
        file.flush().expect("flushed");
        let read = std::fs::File::open(file.path()).expect("opened");
        let tables = Tables::parse(&body).expect("its tables");
        let code_len = tables.code_len as usize;
        let mapped = Mapped::map(&read, 65_536, code_len).expect("mapped");
        // SAFETY: the file is this test's own and nothing writes it
        // after `flush`; the code is this generator's fresh output for
        // this machine, with these tables.
        let code = unsafe { mapped.arm(&body[..code_len], &tables) }.expect("armed");
        (code, rows)
    }

    /// Enter body 0 at the resume point whose pc is `pc`, on `frame`,
    /// with `steps` of the budget left.
    fn enter(
        code: &cig_native::Code,
        rows: &[cig_native::Body],
        pc: u32,
        frame: &mut [Value],
        steps: i64,
        host: &mut Recording,
    ) -> (Stop, Context) {
        let at = rows[0]
            .resumes
            .iter()
            .position(|p| *p == pc)
            .unwrap_or_else(|| panic!("pc {pc} is no resume point of {:?}", rows[0].resumes));
        let mut ctx = Context::new(steps, 10);
        ctx.frame = frame.as_mut_ptr().cast();
        host.frame = Some((frame.as_mut_ptr(), frame.len()));
        let stop = code
            .enter(0, at as u32, &mut ctx, host)
            .expect("a body and a resume point it has");
        (stop, ctx)
    }

    /// Two lists made, then a number loaded, then a return.
    fn two_lists() -> Module {
        module(function(
            3,
            0,
            vec![
                Instr::abc(Op::NEWLIST, 0, 0, 0),
                Instr::abc(Op::NEWLIST, 1, 0, 0),
                Instr::asbx(Op::LOADINT, 2, 7),
                Instr::abc(Op::RET, 2, 0, 0),
            ],
        ))
    }

    fn ints(values: &[i64]) -> Vec<Value> {
        values.iter().map(|v| Value::Int(*v)).collect()
    }

    /// The frame of `sum_loop` as the interpreter leaves it at the
    /// loop's start, turn `i` with `total` so far.
    fn at_the_loop(total: i64, i: i64) -> Vec<Value> {
        ints(&[total, i, 10, 1, i])
    }

    #[test]
    fn a_loop_runs_to_its_end_in_machine_code_and_the_frame_is_whole_after() {
        for mode in [Mode::Promoted, Mode::Frame, Mode::Checked] {
            let (code, rows) = load(&sum_loop(), Rule::Loops, mode);
            let mut frame = at_the_loop(0, 0);
            let mut host = Recording::default();
            let (stop, ctx) = enter(&code, &rows, 5, &mut frame, 1_000, &mut host);
            // The bytecode goes on at the return, with ten turns' steps
            // spent and nothing asked of the runtime.
            assert_eq!(stop, Stop::At(8), "{mode:?}");
            assert_eq!(ctx.steps_left, 990, "{mode:?}");
            assert_eq!(ctx.nest, 1, "{mode:?}: the last `steps` set the nesting");
            assert!(host.run_calls.is_empty() && host.again_calls.is_empty());
            assert!(
                matches!(frame[0], Value::Int(45)),
                "{mode:?}: {}",
                frame[0].repr()
            );
            assert!(
                matches!(frame[4], Value::Int(9)),
                "{mode:?}: {}",
                frame[4].repr()
            );
        }
    }

    #[test]
    fn a_loop_entered_part_way_goes_on_from_the_frame_it_is_given() {
        let (code, rows) = load(&sum_loop(), Rule::Loops, Mode::Promoted);
        // Turn 7, with 0 + 1 + ... + 6 so far.
        let mut frame = at_the_loop(21, 7);
        let mut host = Recording::default();
        let (stop, ctx) = enter(&code, &rows, 5, &mut frame, 1_000, &mut host);
        assert_eq!(stop, Stop::At(8));
        assert_eq!(ctx.steps_left, 997);
        assert!(matches!(frame[0], Value::Int(45)), "{}", frame[0].repr());
    }

    #[test]
    fn the_step_budget_stops_a_loop_at_the_steps_instruction_that_spends_it() {
        let (code, rows) = load(&sum_loop(), Rule::Loops, Mode::Promoted);
        let mut frame = at_the_loop(0, 0);
        let mut host = Recording::default();
        // Three steps: three turns run, and the fourth's `steps` is the
        // interpreter's to raise at, with the frame as that turn finds
        // it and the budget untouched by the instruction that stopped.
        let (stop, ctx) = enter(&code, &rows, 5, &mut frame, 3, &mut host);
        assert_eq!(stop, Stop::At(5));
        assert_eq!(ctx.steps_left, 0);
        assert!(matches!(frame[0], Value::Int(3)), "{}", frame[0].repr());
        assert!(matches!(frame[4], Value::Int(3)), "{}", frame[4].repr());
        assert!(matches!(frame[1], Value::Int(3)), "{}", frame[1].repr());
        assert!(host.run_calls.is_empty() && host.again_calls.is_empty());
    }

    #[test]
    fn an_overflow_is_the_interpreters_and_the_loop_goes_on_after_it() {
        let (code, rows) = load(&sum_loop(), Rule::Loops, Mode::Promoted);
        // `total + 9` overflows on the last turn.
        let mut frame = at_the_loop(i64::MAX - 3, 9);
        let mut host = Recording {
            // The interpreter ran the add (it would raise; here it is
            // scripted to go on) and stands at the `forloop`.
            again_returns: vec![Stop::At(7)],
            ..Recording::default()
        };
        let (stop, _) = enter(&code, &rows, 5, &mut frame, 1_000, &mut host);
        assert_eq!(host.again_calls, vec![6], "the add, and only it");
        assert_eq!(stop, Stop::At(8));
        // The frame was whole when the runtime was called: the total
        // as it stood, not a wrapped sum.
        assert!(
            matches!(frame[0], Value::Int(v) if v == i64::MAX - 3),
            "{}",
            frame[0].repr()
        );
    }

    #[test]
    fn two_numbers_are_added_inline_and_anything_else_goes_through_again() {
        let (code, rows) = load(&add_args(), Rule::Every, Mode::Promoted);
        // Two ints.
        let mut frame = vec![Value::Int(2), Value::Int(3), Value::Null];
        let mut host = Recording::default();
        let (stop, _) = enter(&code, &rows, 0, &mut frame, 100, &mut host);
        assert_eq!(stop, Stop::At(1), "the return is the bytecode's");
        assert!(matches!(frame[2], Value::Int(5)), "{}", frame[2].repr());
        assert!(host.again_calls.is_empty());
        // An int and a float, as two floats.
        let mut frame = vec![Value::Int(2), Value::Float(0.5), Value::Null];
        let (stop, _) = enter(&code, &rows, 0, &mut frame, 100, &mut host);
        assert_eq!(stop, Stop::At(1));
        assert!(
            matches!(frame[2], Value::Float(v) if v == 2.5),
            "{}",
            frame[2].repr()
        );
        assert!(host.again_calls.is_empty());
        // A string: the interpreter's, and the body goes on where it
        // says the bytecode does.
        let mut frame = vec![Value::Str("a".into()), Value::Int(3), Value::Null];
        host.again_returns = vec![Stop::At(1)];
        let (stop, _) = enter(&code, &rows, 0, &mut frame, 100, &mut host);
        assert_eq!(stop, Stop::At(1));
        assert_eq!(host.again_calls, vec![0]);
        assert!(
            matches!(frame[2], Value::Null),
            "nothing was written inline"
        );
        // A register that holds a string is not written over by
        // machine code: the runtime frees what it held (`clear`), and
        // then the sum is stored.
        let mut frame = vec![Value::Int(2), Value::Int(3), Value::Str("held".into())];
        host.again_calls.clear();
        let (stop, _) = enter(&code, &rows, 0, &mut frame, 100, &mut host);
        assert_eq!(stop, Stop::At(1));
        assert!(host.again_calls.is_empty());
        assert_eq!(host.clear_calls, vec![2]);
        assert!(matches!(frame[2], Value::Int(5)), "{}", frame[2].repr());
        // Where the runtime cannot (the register carries a cell), the
        // interpreter runs the instruction, and nothing was written.
        let mut frame = vec![Value::Int(2), Value::Int(3), Value::Str("held".into())];
        host.refuses = true;
        host.again_returns = vec![Stop::At(1)];
        let (stop, _) = enter(&code, &rows, 0, &mut frame, 100, &mut host);
        assert_eq!(stop, Stop::At(1));
        assert_eq!(host.again_calls, vec![0]);
        assert!(matches!(&frame[2], Value::Str(s) if &**s == "held"));
        // And a string among the operands is the interpreter's before
        // anything is cleared: it must find the register as it was.
        let mut frame = vec![
            Value::Str("a".into()),
            Value::Int(3),
            Value::Str("held".into()),
        ];
        host.refuses = false;
        host.clear_calls.clear();
        host.again_calls.clear();
        host.again_returns = vec![Stop::At(1)];
        let (stop, _) = enter(&code, &rows, 0, &mut frame, 100, &mut host);
        assert_eq!(stop, Stop::At(1));
        assert_eq!(host.again_calls, vec![0]);
        assert!(host.clear_calls.is_empty());
        assert!(matches!(&frame[2], Value::Str(s) if &**s == "held"));
    }

    #[test]
    fn a_stop_at_the_instructions_own_pc_hands_the_frame_back() {
        // The runtime ran nothing (it has given the body up): the body
        // returns the stop as it is.
        let (code, rows) = load(&add_args(), Rule::Every, Mode::Promoted);
        let mut frame = vec![Value::Str("a".into()), Value::Int(3), Value::Null];
        let mut host = Recording {
            again_returns: vec![Stop::At(0)],
            ..Recording::default()
        };
        let (stop, _) = enter(&code, &rows, 0, &mut frame, 100, &mut host);
        assert_eq!(stop, Stop::At(0));
        // And an error a helper raised is returned as it is too.
        host.again_returns = vec![Stop::Raised(0)];
        let (stop, _) = enter(&code, &rows, 0, &mut frame, 100, &mut host);
        assert_eq!(stop, Stop::Raised(0));
    }

    #[test]
    fn instructions_with_no_inline_form_are_one_run_of_the_interpreter() {
        let (code, rows) = load(&two_lists(), Rule::Every, Mode::Promoted);
        let mut frame = vec![Value::Null, Value::Null, Value::Null];
        let mut host = Recording {
            // The interpreter made both lists and stands after them.
            run_returns: vec![Stop::At(2)],
            ..Recording::default()
        };
        let (stop, _) = enter(&code, &rows, 0, &mut frame, 100, &mut host);
        assert_eq!(host.run_calls, vec![(0, 2)], "both, in one call");
        // Then the load runs inline, and the return is the bytecode's.
        assert_eq!(stop, Stop::At(3));
        assert!(matches!(frame[2], Value::Int(7)), "{}", frame[2].repr());
        // A stop the body does not expect (an error, a call) is
        // returned as it is, with nothing run after it.
        let mut frame = vec![Value::Null, Value::Null, Value::Null];
        host.run_calls.clear();
        host.run_returns = vec![Stop::Raised(1)];
        let (stop, _) = enter(&code, &rows, 0, &mut frame, 100, &mut host);
        assert_eq!(stop, Stop::Raised(1));
        assert!(matches!(frame[2], Value::Null));
    }

    /// `fn(a, b) { return a / b }`.
    fn div_args() -> Module {
        module(function(
            3,
            2,
            vec![Instr::abc(Op::DIV, 2, 0, 1), Instr::abc(Op::RET, 2, 0, 0)],
        ))
    }

    /// The frame of `work_and_call` at the loop's start, turn `i` with
    /// `total` so far.
    fn working(total: i64, i: i64) -> Vec<Value> {
        let mut frame = ints(&[total, i, 3, 1, i]);
        frame.extend([Value::Null, Value::Null]);
        frame
    }

    #[test]
    fn a_call_in_a_native_loop_is_one_run_of_the_interpreter_a_turn() {
        let (code, rows) = load(&work_and_call(), Rule::Loops, Mode::Promoted);
        let mut frame = working(0, 0);
        let mut host = Recording {
            // The runtime made the list, carried the call to its return
            // and stands after it, three turns running.
            run_returns: vec![Stop::At(28); 3],
            ..Recording::default()
        };
        let (stop, ctx) = enter(&code, &rows, 5, &mut frame, 1_000, &mut host);
        assert_eq!(
            host.run_calls,
            vec![(26, 28); 3],
            "the list and the call, in one run a turn"
        );
        assert!(host.again_calls.is_empty());
        assert_eq!(stop, Stop::At(29), "the return is the bytecode's");
        assert_eq!(ctx.steps_left, 997);
        // Twenty adds of 0, of 1 and of 2.
        assert!(matches!(frame[0], Value::Int(60)), "{}", frame[0].repr());
        // The runtime could not carry the call and stands at it: the
        // body returns that stop, with the frame whole (the turn's
        // adds are in it), and the VM's own loop makes the call.
        let mut frame = working(5, 2);
        host.run_calls.clear();
        host.run_returns = vec![Stop::At(27)];
        let (stop, _) = enter(&code, &rows, 5, &mut frame, 1_000, &mut host);
        assert_eq!(stop, Stop::At(27));
        assert_eq!(host.run_calls, vec![(26, 28)]);
        assert!(matches!(frame[0], Value::Int(45)), "{}", frame[0].repr());
        // An error the callee raised and nobody under the body caught
        // is returned as it is.
        let mut frame = working(0, 0);
        host.run_returns = vec![Stop::Raised(27)];
        let (stop, _) = enter(&code, &rows, 5, &mut frame, 1_000, &mut host);
        assert_eq!(stop, Stop::Raised(27));
    }

    #[test]
    fn the_checked_build_leaves_every_call_to_the_vm() {
        // So the tests' two builds of a script make its calls both
        // ways: carried under the body, and by the VM's own loop.
        let (code, rows) = load(&work_and_call(), Rule::Every, Mode::Checked);
        assert!(rows[0].resumes.contains(&28), "{:?}", rows[0].resumes);
        let mut frame = working(0, 1);
        let mut host = Recording {
            // The interpreter made the list and stands at the call.
            run_returns: vec![Stop::At(27)],
            ..Recording::default()
        };
        let (stop, _) = enter(&code, &rows, 5, &mut frame, 1_000, &mut host);
        assert_eq!(host.run_calls, vec![(26, 27)], "the list alone");
        assert_eq!(stop, Stop::At(27), "the body is left at the call");
        // The same function with its registers held carries the call.
        let (code, rows) = load(&work_and_call(), Rule::Every, Mode::Promoted);
        let mut frame = working(0, 1);
        let mut host = Recording {
            run_returns: vec![Stop::At(27)],
            ..Recording::default()
        };
        let (stop, _) = enter(&code, &rows, 5, &mut frame, 1_000, &mut host);
        assert_eq!(host.run_calls, vec![(26, 28)], "the list and the call");
        assert_eq!(stop, Stop::At(27));
    }

    #[test]
    fn an_int_divided_by_an_int_is_an_int_when_it_divides_and_a_float_when_not() {
        for mode in [Mode::Promoted, Mode::Frame, Mode::Checked] {
            let (code, rows) = load(&div_args(), Rule::Every, mode);
            let mut host = Recording::default();
            let pairs = [
                (6, 3),
                (7, 2),
                (-7, 2),
                (0, 5),
                (6, -3),
                (1, 3),
                (i64::MIN, 2),
                (i64::MIN, 3),
                (i64::MAX, -1),
                (i64::MAX, i64::MAX),
                (-1, i64::MIN),
            ];
            for (a, b) in pairs {
                let mut frame = vec![Value::Int(a), Value::Int(b), Value::Null];
                let (stop, _) = enter(&code, &rows, 0, &mut frame, 100, &mut host);
                assert_eq!(stop, Stop::At(1), "{mode:?}: {a} / {b}");
                // The interpreter's rule: what divides exactly is an
                // int, anything else the two as floats.
                match a % b == 0 {
                    true => assert!(
                        matches!(frame[2], Value::Int(q) if q == a / b),
                        "{mode:?}: {a} / {b} = {}",
                        frame[2].repr()
                    ),
                    false => assert!(
                        matches!(frame[2], Value::Float(q) if q.to_bits() == (a as f64 / b as f64).to_bits()),
                        "{mode:?}: {a} / {b} = {}",
                        frame[2].repr()
                    ),
                }
            }
            assert!(host.again_calls.is_empty(), "{mode:?}");
            // By zero, and the least int by -1 (the one quotient that
            // does not fit): the interpreter's errors, with nothing
            // written.
            for (a, b) in [(1, 0), (0, 0), (i64::MIN, -1)] {
                let mut frame = vec![Value::Int(a), Value::Int(b), Value::Null];
                host.again_calls.clear();
                host.again_returns = vec![Stop::Raised(0)];
                let (stop, _) = enter(&code, &rows, 0, &mut frame, 100, &mut host);
                assert_eq!(stop, Stop::Raised(0), "{mode:?}: {a} / {b}");
                assert_eq!(host.again_calls, vec![0], "{mode:?}: {a} / {b}");
                assert!(matches!(frame[2], Value::Null), "{mode:?}: {a} / {b}");
            }
            // An int by a float and a float by an int are floats, and
            // a float by zero is the interpreter's.
            let mut frame = vec![Value::Int(7), Value::Float(2.0), Value::Null];
            let (stop, _) = enter(&code, &rows, 0, &mut frame, 100, &mut host);
            assert_eq!(stop, Stop::At(1));
            assert!(matches!(frame[2], Value::Float(q) if q == 3.5));
            let mut frame = vec![Value::Float(7.0), Value::Int(0), Value::Null];
            host.again_calls.clear();
            host.again_returns = vec![Stop::Raised(0)];
            let (stop, _) = enter(&code, &rows, 0, &mut frame, 100, &mut host);
            assert_eq!(stop, Stop::Raised(0));
            assert_eq!(host.again_calls, vec![0]);
        }
    }

    /// `fn(a) { b = a; return b }`.
    fn move_arg() -> Module {
        module(function(
            2,
            1,
            vec![Instr::abc(Op::MOV, 1, 0, 0), Instr::abc(Op::RET, 1, 0, 0)],
        ))
    }

    /// The frame of `count_true` at the loop's start: `items` is what
    /// `iterprep` made, `at` the count of those taken, `c` the count so
    /// far.
    fn counting(items: Value, at: i64, c: i64) -> Vec<Value> {
        vec![
            Value::Null,
            Value::Int(c),
            Value::Null,
            items,
            Value::Int(at),
            Value::Null,
            Value::Int(1),
        ]
    }

    #[test]
    fn a_loops_items_are_read_where_they_lie_and_the_list_is_asked_for_once() {
        for mode in [Mode::Promoted, Mode::Frame, Mode::Checked] {
            let (code, rows) = load(&count_true(), Rule::Loops, mode);
            let items = list_of(vec![
                Value::Bool(true),
                Value::Bool(false),
                Value::Int(7),
                Value::Null,
                Value::Float(0.5),
            ]);
            let mut frame = counting(items, 0, 0);
            let mut host = Recording::default();
            let (stop, ctx) = enter(&code, &rows, 4, &mut frame, 1_000, &mut host);
            assert_eq!(stop, Stop::At(9), "{mode:?}: the loop ran out of items");
            assert_eq!(host.list_calls, vec![3], "{mode:?}: asked once");
            assert!(host.again_calls.is_empty() && host.run_calls.is_empty());
            // True, 7 and 0.5 are truthy.
            assert!(
                matches!(frame[1], Value::Int(3)),
                "{mode:?}: {}",
                frame[1].repr()
            );
            assert!(
                matches!(frame[4], Value::Int(5)),
                "{mode:?}: {}",
                frame[4].repr()
            );
            assert_eq!(frame[5].repr(), "0.5", "{mode:?}: the last item");
            assert_eq!(ctx.steps_left, 995, "{mode:?}: a step a turn");
        }
    }

    #[test]
    fn an_item_that_owns_memory_and_a_window_that_is_no_list_are_the_interpreters() {
        let (code, rows) = load(&count_true(), Rule::Loops, Mode::Promoted);
        // A string among the items: only the interpreter shares it. It
        // is scripted to leave the loop.
        let items = list_of(vec![Value::Bool(true), Value::Str("s".into())]);
        let mut frame = counting(items, 0, 0);
        let mut host = Recording {
            again_returns: vec![Stop::At(9)],
            ..Recording::default()
        };
        let (stop, _) = enter(&code, &rows, 4, &mut frame, 1_000, &mut host);
        assert_eq!(host.again_calls, vec![4], "the `iternext`, at the string");
        assert_eq!(stop, Stop::At(9));
        assert!(matches!(frame[1], Value::Int(1)), "{}", frame[1].repr());
        assert!(
            matches!(frame[4], Value::Int(1)),
            "the count was the frame's when the interpreter was asked: {}",
            frame[4].repr()
        );
        // A count below zero is no item: the loop is over, with nothing
        // written.
        let mut frame = counting(list_of(vec![Value::Bool(true)]), -1, 0);
        let mut host = Recording::default();
        let (stop, _) = enter(&code, &rows, 4, &mut frame, 1_000, &mut host);
        assert_eq!(stop, Stop::At(9));
        assert!(matches!(frame[1], Value::Int(0)) && matches!(frame[5], Value::Null));
        // No list where `iterprep` left one: the interpreter's error.
        let mut frame = counting(Value::Int(3), 0, 0);
        let mut host = Recording {
            again_returns: vec![Stop::Raised(4)],
            ..Recording::default()
        };
        let (stop, _) = enter(&code, &rows, 4, &mut frame, 1_000, &mut host);
        assert_eq!(stop, Stop::Raised(4));
        assert_eq!(host.again_calls, vec![4]);
        // The item's register holds a string from a turn the
        // interpreter ran: the runtime frees it before the next item is
        // stored there.
        let mut frame = counting(list_of(vec![Value::Int(2)]), 0, 0);
        frame[5] = Value::Str("old".into());
        let mut host = Recording::default();
        let (stop, _) = enter(&code, &rows, 4, &mut frame, 1_000, &mut host);
        assert_eq!(stop, Stop::At(9));
        assert_eq!(host.clear_calls, vec![5]);
        assert!(matches!(frame[5], Value::Int(2)), "{}", frame[5].repr());
    }

    #[test]
    fn a_push_is_the_runtimes_and_a_list_is_moved_by_it() {
        let (code, rows) = load(&push_all(), Rule::Loops, Mode::Promoted);
        let xs = list_of(vec![]);
        let mut frame = vec![
            xs.clone(),
            Value::Int(0),
            Value::Int(4),
            Value::Int(1),
            Value::Int(0),
            Value::Null,
            Value::Null,
        ];
        let mut host = Recording::default();
        let (stop, _) = enter(&code, &rows, 4, &mut frame, 1_000, &mut host);
        assert_eq!(stop, Stop::At(9));
        assert_eq!(elements(&xs), ["0", "1", "2", "3"]);
        // The list is moved to the call's register by the runtime (it
        // is shared, and what the register held is freed), and pushed
        // to by it; the value, an int, is moved by machine code.
        assert_eq!(host.copy_calls, vec![(5, 0); 4]);
        assert_eq!(host.push_calls, vec![(5, 6, 5); 4]);
        assert!(host.again_calls.is_empty() && host.run_calls.is_empty());
        assert!(matches!(&frame[5], Value::List(_)), "{}", frame[5].repr());
        // Where the runtime cannot (a register carries a cell, the list
        // is being walked), the interpreter runs each instruction.
        let xs = list_of(vec![]);
        let mut frame = vec![
            xs.clone(),
            Value::Int(0),
            Value::Int(2),
            Value::Int(1),
            Value::Int(0),
            Value::Null,
            Value::Null,
        ];
        let mut host = Recording {
            refuses: true,
            again_returns: vec![Stop::At(6), Stop::At(8), Stop::At(6), Stop::At(8)],
            ..Recording::default()
        };
        let (stop, _) = enter(&code, &rows, 4, &mut frame, 1_000, &mut host);
        assert_eq!(stop, Stop::At(9));
        assert_eq!(host.again_calls, vec![5, 7, 5, 7]);
        assert!(
            elements(&xs).is_empty(),
            "the scripted interpreter does nothing"
        );
    }

    #[test]
    fn a_value_that_owns_memory_is_moved_by_the_runtime_and_a_number_by_machine_code() {
        let (code, rows) = load(&move_arg(), Rule::Every, Mode::Promoted);
        // A number over null: two stores.
        let mut frame = vec![Value::Int(5), Value::Null];
        let mut host = Recording::default();
        let (stop, _) = enter(&code, &rows, 0, &mut frame, 100, &mut host);
        assert_eq!(stop, Stop::At(1));
        assert!(matches!(frame[1], Value::Int(5)));
        assert!(host.copy_calls.is_empty() && host.again_calls.is_empty());
        // A string: the runtime shares it.
        let mut frame = vec![Value::Str("s".into()), Value::Null];
        let (stop, _) = enter(&code, &rows, 0, &mut frame, 100, &mut host);
        assert_eq!(stop, Stop::At(1));
        assert_eq!(host.copy_calls, vec![(1, 0)]);
        assert!(matches!(&frame[1], Value::Str(s) if &**s == "s"));
        // A number over a string: the runtime frees the string.
        let mut frame = vec![Value::Int(5), Value::Str("old".into())];
        host.copy_calls.clear();
        let (stop, _) = enter(&code, &rows, 0, &mut frame, 100, &mut host);
        assert_eq!(stop, Stop::At(1));
        assert_eq!(host.copy_calls, vec![(1, 0)]);
        assert!(matches!(frame[1], Value::Int(5)));
        assert!(host.again_calls.is_empty(), "no guess failed");
    }

    fn list_of(values: Vec<Value>) -> Value {
        Value::List(std::rc::Rc::new(std::cell::RefCell::new(values)))
    }

    fn elements(v: &Value) -> Vec<String> {
        match v {
            Value::List(l) => l.borrow().iter().map(Value::repr).collect(),
            other => panic!("not a list: {}", other.repr()),
        }
    }

    /// The frame of `sum_list` at the loop's start, turn `i`.
    fn summing(xs: Value, total: i64, i: i64) -> Vec<Value> {
        vec![
            xs,
            Value::Int(total),
            Value::Int(i),
            Value::Int(4),
            Value::Int(1),
            Value::Int(i),
            Value::Null,
        ]
    }

    #[test]
    fn a_list_of_numbers_is_read_where_it_lies_and_asked_for_once() {
        for mode in [Mode::Promoted, Mode::Frame, Mode::Checked] {
            let (code, rows) = load(&sum_list(), Rule::Loops, mode);
            let mut frame = summing(list_of(ints(&[10, 20, 30, 40])), 0, 0);
            let mut host = Recording::default();
            let (stop, ctx) = enter(&code, &rows, 5, &mut frame, 1_000, &mut host);
            assert_eq!(stop, Stop::At(10), "{mode:?}");
            assert!(
                matches!(frame[1], Value::Int(100)),
                "{mode:?}: {}",
                frame[1].repr()
            );
            assert_eq!(ctx.steps_left, 996, "{mode:?}");
            // Four reads, one question: nothing called the runtime in
            // between, so the list stayed in view.
            assert_eq!(host.list_calls, vec![0], "{mode:?}");
            assert!(host.again_calls.is_empty() && host.run_calls.is_empty());
        }
        // Floats come out as floats, and the add takes them.
        let (code, rows) = load(&sum_list(), Rule::Loops, Mode::Promoted);
        let floats = vec![
            Value::Float(0.5),
            Value::Float(1.5),
            Value::Int(2),
            Value::Float(4.0),
        ];
        let mut frame = summing(list_of(floats), 0, 0);
        let mut host = Recording::default();
        let (stop, _) = enter(&code, &rows, 5, &mut frame, 1_000, &mut host);
        // The total is anything once an element of unknown type is
        // added to it, so the add reads both tags: an int and a float
        // are two floats.
        assert_eq!(stop, Stop::At(10));
        assert!(host.again_calls.is_empty());
        assert!(
            matches!(frame[1], Value::Float(v) if v == 8.0),
            "{}",
            frame[1].repr()
        );
    }

    #[test]
    fn an_element_machine_code_may_not_copy_is_the_interpreters() {
        let (code, rows) = load(&sum_list(), Rule::Loops, Mode::Promoted);
        // The third element is a string: it is shared, and only the
        // interpreter counts shares.
        let xs = list_of(vec![
            Value::Int(1),
            Value::Int(2),
            Value::Str("three".into()),
            Value::Int(4),
        ]);
        let mut frame = summing(xs, 0, 0);
        let mut host = Recording {
            // The interpreter ran the read and stands past its `nop`.
            again_returns: vec![Stop::At(8)],
            ..Recording::default()
        };
        let (stop, _) = enter(&code, &rows, 5, &mut frame, 1_000, &mut host);
        assert_eq!(host.again_calls, vec![6], "the read of the string");
        // The body went on from past the `nop`, to the loop's end. (The
        // scripted interpreter wrote nothing, so the add saw the second
        // element again: 1 + 2 + 2 + 4.)
        assert_eq!(stop, Stop::At(10));
        assert!(matches!(frame[1], Value::Int(9)), "{}", frame[1].repr());
        // No list is in view once the interpreter has run: the fourth
        // read asked for it again.
        assert_eq!(host.list_calls, vec![0, 0]);
    }

    #[test]
    fn an_index_the_list_has_no_element_at_is_the_interpreters() {
        let (code, rows) = load(&sum_list(), Rule::Loops, Mode::Promoted);
        // Three elements, four turns: the last read is past the end.
        let mut frame = summing(list_of(ints(&[1, 2, 3])), 0, 0);
        let mut host = Recording {
            again_returns: vec![Stop::Raised(6)],
            ..Recording::default()
        };
        let (stop, _) = enter(&code, &rows, 5, &mut frame, 1_000, &mut host);
        assert_eq!(host.again_calls, vec![6]);
        assert_eq!(stop, Stop::Raised(6), "the interpreter's error");
        assert!(matches!(frame[1], Value::Int(6)), "{}", frame[1].repr());
        // A register that holds no list at all.
        let mut frame = summing(Value::Str("abcd".into()), 0, 0);
        host.again_calls.clear();
        host.again_returns = vec![Stop::At(6)];
        let (stop, _) = enter(&code, &rows, 5, &mut frame, 1_000, &mut host);
        assert_eq!(
            (stop, host.again_calls.as_slice()),
            (Stop::At(6), [6].as_slice())
        );
    }

    #[test]
    fn a_list_is_written_where_it_lies_when_nothing_has_to_be_freed() {
        let filling = |xs: Value| {
            vec![
                xs,
                Value::Int(0),
                Value::Int(4),
                Value::Int(1),
                Value::Int(0),
                Value::Null,
            ]
        };
        for mode in [Mode::Promoted, Mode::Frame, Mode::Checked] {
            let (code, rows) = load(&fill_list(), Rule::Loops, mode);
            let mut frame = filling(list_of(vec![
                Value::Null,
                Value::Bool(true),
                Value::Float(2.5),
                Value::Int(-1),
            ]));
            let mut host = Recording::default();
            let (stop, _) = enter(&code, &rows, 4, &mut frame, 1_000, &mut host);
            assert_eq!(stop, Stop::At(9), "{mode:?}");
            assert_eq!(elements(&frame[0]), ["0", "1", "2", "3"], "{mode:?}");
            assert_eq!(host.list_calls, vec![0], "{mode:?}");
            assert!(host.again_calls.is_empty());
        }
        // An element that is a string is not written over inline: the
        // interpreter frees it. It stands past the write's two `nop`s
        // after, and the loop goes on from there, asking for the list
        // again.
        let (code, rows) = load(&fill_list(), Rule::Loops, Mode::Promoted);
        let mut frame = filling(list_of(vec![
            Value::Int(9),
            Value::Str("old".into()),
            Value::Int(9),
            Value::Int(9),
        ]));
        let mut host = Recording {
            again_returns: vec![Stop::At(8)],
            ..Recording::default()
        };
        let (stop, _) = enter(&code, &rows, 4, &mut frame, 1_000, &mut host);
        assert_eq!(stop, Stop::At(9));
        assert_eq!(host.again_calls, vec![5]);
        assert_eq!(elements(&frame[0]), ["0", "\"old\"", "2", "3"]);
        assert_eq!(
            host.list_calls,
            vec![0, 0],
            "asked again after the interpreter ran"
        );
    }

    #[test]
    fn a_register_that_is_not_of_its_proven_type_is_a_defect() {
        // The pass proved `total` an int at the loop's start. A frame
        // that holds a float there is one no run of the bytecode makes:
        // a body that held it would compute with the wrong bits.
        for mode in [Mode::Promoted, Mode::Checked] {
            let (code, rows) = load(&sum_loop(), Rule::Loops, mode);
            let mut frame = at_the_loop(0, 0);
            frame[0] = Value::Float(1.5);
            let mut host = Recording::default();
            let (stop, _) = enter(&code, &rows, 5, &mut frame, 1_000, &mut host);
            assert!(
                matches!(stop, Stop::Defect(5) | Stop::Defect(6)),
                "{mode:?}: {stop:?}"
            );
            assert!(matches!(frame[0], Value::Float(v) if v == 1.5), "untouched");
        }
        // Read with its tag at every instruction, the same frame is no
        // defect: the add's inline form takes two numbers.
        let (code, rows) = load(&sum_loop(), Rule::Loops, Mode::Frame);
        let mut frame = at_the_loop(0, 9);
        frame[0] = Value::Float(1.5);
        let mut host = Recording::default();
        let (stop, _) = enter(&code, &rows, 5, &mut frame, 1_000, &mut host);
        assert_eq!(stop, Stop::At(8));
        assert!(
            matches!(frame[0], Value::Float(v) if v == 10.5),
            "{}",
            frame[0].repr()
        );
    }
}
