// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2

//! Bodies from bytecode (decision 0008), on their own: which functions
//! get one and where it is entered, that the same input gives the same
//! bytes for either machine, and, on a machine with a loader, what a
//! body does to a real frame: a loop that runs to its end in machine
//! code, the step budget, an operand an inline form does not cover, a
//! stretch the interpreter runs, and a register that is not of its
//! proven type.
//!
//! What a whole program does with its bodies is the differential
//! suite's (`tests/differential.rs`, `DIFF_BODIES`): every script, held
//! to the interpreter.

use cig_codegen::{bodies, bodies_as, Mode, Rule};
use cig_native::Arch;
use cigscript::cig_bytecode::instr::Instr;
use cigscript::cig_bytecode::module::{FuncKind, Function, Module, SpanBase};
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

#[test]
fn the_rule_none_gives_no_function_a_body() {
    let out = bodies(&sum_loop(), Rule::None, Arch::X86_64).expect("bodies");
    assert!(out.bodies.is_empty() && out.code.is_empty());
    assert_eq!(out.rule, Rule::None);
    assert_eq!(out.report.len(), 1);
    assert_eq!(out.report[0].why, "the rule gives no function a body");
}

#[test]
fn a_function_without_a_loop_has_no_body() {
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
    }

    impl Host for Recording {
        fn run(&mut self, _ctx: &mut Context, from: u32, to: u32) -> Stop {
            self.run_calls.push((from, to));
            assert!(!self.run_returns.is_empty(), "run was not expected");
            self.run_returns.remove(0)
        }
        fn list(&mut self, _ctx: &mut Context, _reg: u32) -> bool {
            panic!("no body asks for a list's elements yet");
        }
        fn again(&mut self, _ctx: &mut Context, pc: u32) -> Stop {
            self.again_calls.push(pc);
            assert!(!self.again_returns.is_empty(), "again was not expected");
            self.again_returns.remove(0)
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
        // A register that holds a list is not written over inline:
        // only the interpreter frees what a register held.
        let mut frame = vec![Value::Int(2), Value::Int(3), Value::Str("held".into())];
        host.again_calls.clear();
        host.again_returns = vec![Stop::At(1)];
        let (stop, _) = enter(&code, &rows, 0, &mut frame, 100, &mut host);
        assert_eq!(stop, Stop::At(1));
        assert_eq!(host.again_calls, vec![0]);
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
