// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! Stage B's lowering (#31), first part: CIR to the register bytecode,
//! verified, loaded and run in the cig runtime's VM, and held to the
//! tree-walker. Built only with the `vm` feature, against a cigscript
//! whose runtime carries the VM.
#![cfg(feature = "vm")]

#[path = "differential/gen.rs"]
#[allow(dead_code)]
mod gen;

use cigcompile::bc::{self, CompileError, Error};
use cigcompile::cir::ir::{
    Block, BurnNode, Const, FuncId, Kind, MapKey, OpKind, Piece, RegionId, ShortOp, StepSite,
    ValueId, VarId,
};
use cigcompile::cir::pass::{self, Pass};
use cigscript::burn::{Kernel, Mode};
use cigscript::cig_bytecode::{verify_module, Module};
use cigscript::diagnostics::Diagnostic;
use cigscript::interp::Interp;
use cigscript::syntax::ast::{AssignOp, BinOp, BurnClass, UnOp};
use cigscript::value::Effect;
use std::cell::RefCell;
use std::rc::Rc;

// ----- running both ways ---------------------------------------------------------

/// What a run left: what it wrote to the error stream (`log.info`), and
/// how it ended: cleanly, or with the error's code, message, line and
/// column.
#[derive(Debug, PartialEq)]
struct Outcome {
    logged: String,
    end: Option<(String, String, Option<u32>, Option<u32>)>,
}

/// The interpreter's error stream, kept for the comparison.
#[derive(Clone, Default)]
struct Log(Rc<RefCell<Vec<u8>>>);

impl std::io::Write for Log {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn interp(max_steps: u64) -> (Interp, Log) {
    let log = Log::default();
    let mut i = Interp::new(Kernel::ephemeral(Mode::Run));
    i.out = Box::new(std::io::sink());
    i.err = Box::new(log.clone());
    i.set_max_steps(max_steps);
    (i, log)
}

fn outcome(r: Result<(), Diagnostic>, log: &Log) -> Outcome {
    Outcome {
        logged: String::from_utf8_lossy(&log.0.borrow()).into_owned(),
        end: r
            .err()
            .map(|d| (d.code.to_string(), d.message, d.line, d.col)),
    }
}

/// The tree-walker, as `cig run` runs a script, and the steps it counted.
fn tree_walker(src: &str, max_steps: u64) -> (Outcome, u64) {
    let program = cigscript::syntax::parse(src).expect("parses");
    let (mut i, log) = interp(max_steps);
    let r = i.run_program(&program);
    (outcome(r, &log), i.steps)
}

/// The VM, through its public entry points: `verify_module`, `vm::load`,
/// `vm::run`. The top level returns null (a script has no top-level
/// `snuff`), so the values come out through `log.info`, which both call
/// through the same builtin.
fn vm(m: &Module, max_steps: u64) -> (Outcome, u64) {
    let verified = verify_module(m).unwrap_or_else(|e| panic!("does not verify: {e}"));
    let program =
        cigscript::vm::load(verified).unwrap_or_else(|e| panic!("does not load: {}", e.message));
    let (mut i, log) = interp(max_steps);
    let r = cigscript::vm::run(&mut i, &program).map(|v| {
        assert_eq!(v.repr(), "null", "the top level leaves null");
    });
    (outcome(r, &log), i.steps)
}

/// The tree-walker recurses on the native stack: deep recursion needs
/// more than a test thread's default.
fn big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(f)
        .expect("a thread")
        .join()
        .expect("the run does not panic")
}

fn compile(src: &str, level: u8) -> Module {
    match bc::compile(src, "t.cig", &pass::level(level)) {
        Ok(m) => m,
        Err(CompileError::Bytecode(e)) => panic!("-O{level}: {e}\n{src}"),
        Err(e) => panic!("-O{level}: {e:?}\n{src}"),
    }
}

// ----- the corpus -------------------------------------------------------------------

/// Small programs inside this part. Each says what it computed with
/// `log.info(repr(...))`, or stops on an error first.
fn corpus() -> Vec<(String, String)> {
    let mut c: Vec<(String, String)> = Vec::new();
    let mut add = |name: &str, src: &str| c.push((name.to_string(), src.to_string()));

    // Arithmetic at the i64 edges, and division by zero.
    add(
        "max-plus-one",
        "roll a = 9223372036854775807\nroll b = 1\nlog.info(repr(a + b))\n",
    );
    add(
        "min-minus-one",
        "roll a = -9223372036854775807 - 1\nroll b = 1\nlog.info(repr(a - b))\n",
    );
    add(
        "min-negated",
        "roll a = -9223372036854775807 - 1\nlog.info(repr(-a))\n",
    );
    add(
        "mul-overflow",
        "roll a = 3037000500\nlog.info(repr(a * a))\n",
    );
    add("mul-edge", "roll a = 3037000499\nlog.info(repr(a * a))\n");
    add(
        "min-div-minus-one",
        "roll a = -9223372036854775807 - 1\nroll b = -1\nlog.info(repr(a / b))\n",
    );
    add(
        "min-mod-minus-one",
        "roll a = -9223372036854775807 - 1\nroll b = -1\nlog.info(repr(a % b))\n",
    );
    add(
        "div-zero",
        "roll a = 7\nroll b = 0\nlog.info(repr(a / b))\n",
    );
    add(
        "mod-zero",
        "roll a = 7\nroll b = 0\nlog.info(repr(a % b))\n",
    );
    add(
        "float-div-zero",
        "roll a = 7.0\nroll b = 0.0\nlog.info(repr(a / b))\n",
    );
    add(
        "int-div",
        "roll a = -7\nroll b = 2\nlog.info(repr(a / b))\n",
    );
    add(
        "int-mod",
        "roll a = -7\nroll b = 2\nlog.info(repr(a % b))\n",
    );
    add(
        "mixed",
        "roll a = 7\nroll b = 2.5\nlog.info(repr(a * b - a / b))\n",
    );
    add(
        "strings",
        "roll a = \"ci\"\nroll b = \"g\"\nlog.info(repr(a + b))\n",
    );
    add(
        "str-plus-int",
        "roll a = \"n\"\nroll b = 1\nlog.info(repr(a + b))\n",
    );
    add(
        "big-constant",
        "roll a = 123456789012\nroll b = 40000\nlog.info(repr(a + b - 70000))\n",
    );
    add(
        "compound",
        "roll a = 10\na += 5\na -= 3\na *= 4\na /= 6\nlog.info(repr(a))\n",
    );

    // Every comparison across types.
    let operands = [
        "1", "2", "-3", "2.5", "0.0", "\"a\"", "\"b\"", "true", "false", "null",
    ];
    for (i, x) in operands.iter().enumerate() {
        for (j, y) in operands.iter().enumerate() {
            for op in ["==", "!=", "<", "<=", ">", ">="] {
                add(
                    &format!("cmp-{i}-{op}-{j}"),
                    &format!("roll x = {x}\nroll y = {y}\nlog.info(repr(x {op} y))\n"),
                );
            }
        }
    }
    add(
        "nan",
        "roll z = 0.0\nroll n = z / z\nroll o = 1.0\nlog.info(repr(n < o))\n",
    );
    add("not", "roll t = 0\nlog.info(repr(not t))\n");

    // Short-circuit logic and `??`.
    add(
        "short",
        "roll a = false and int(\"x\")\nroll b = true or int(\"x\")\nroll c = null ?? 3\nroll d = 4 ?? int(\"x\")\nroll e = 0 and 1\nroll f = \"\" or \"s\"\nroll g = 1 and 2\nroll h = false or null\nlog.info(repr(str(a) + str(b) + str(c) + str(d) + str(e) + f + str(g) + str(h)))\n",
    );

    // Nested `if`.
    add(
        "nested-if",
        "roll n = 0\nroll i = 0\nwhile i < 12 {\n  i += 1\n  if i % 2 == 0 {\n    if i % 3 == 0 {\n      n += 100\n    } else if i % 4 == 0 {\n      n += 10\n    } else {\n      n += 1\n    }\n  } else if i > 9 {\n    n -= 1\n  }\n}\nlog.info(repr(n))\n",
    );

    // `while` with `break` and `continue`, nested.
    add(
        "while-break-continue",
        "roll total = 0\nroll i = 0\nwhile i < 20 {\n  i += 1\n  if i == 3 {\n    continue\n  }\n  roll j = 0\n  while true {\n    j += 1\n    if j > i {\n      break\n    }\n    if j % 2 == 0 {\n      continue\n    }\n    total += j\n  }\n  if i == 15 {\n    break\n  }\n}\nlog.info(repr(total))\n",
    );
    add(
        "while-false",
        "roll n = 0\nwhile n > 0 {\n  n -= 1\n}\nlog.info(repr(n))\n",
    );

    // Pulls, lambdas, `snuff`, recursion (a pull that calls itself
    // through a parameter: one that names itself captures itself).
    add(
        "pulls",
        "pull add(a, b) {\n  snuff a + b\n}\npull none() {\n  roll x = 1\n}\nstick sq = pack(x) => x * x\nlog.info(repr(str(add(sq(3), 4)) + str(none())))\n",
    );
    add(
        "fact",
        "pull fact(self, n) {\n  if n <= 1 {\n    snuff 1\n  }\n  snuff n * self(self, n - 1)\n}\nlog.info(repr(fact(fact, 20)))\n",
    );
    add(
        "fact-overflow",
        "pull fact(self, n) {\n  if n <= 1 {\n    snuff 1\n  }\n  snuff n * self(self, n - 1)\n}\nlog.info(repr(fact(fact, 25)))\n",
    );
    add(
        "fib",
        "pull fib(self, n) {\n  if n < 2 {\n    snuff n\n  }\n  snuff self(self, n - 1) + self(self, n - 2)\n}\nlog.info(repr(fib(fib, 10)))\n",
    );
    add(
        "deep",
        "pull down(self, n) {\n  if n == 0 {\n    snuff 0\n  }\n  snuff 1 + self(self, n - 1)\n}\nlog.info(repr(down(down, 3000)))\n",
    );
    add(
        "too-deep",
        "pull down(self, n) {\n  if n == 0 {\n    snuff 0\n  }\n  snuff 1 + self(self, n - 1)\n}\nlog.info(repr(down(down, 5000)))\n",
    );
    add(
        "callee-type-error",
        "pull f(a) {\n  roll b = a * 2\n  snuff b + \"x\"\n}\nroll i = 0\nwhile i < 3 {\n  i += 1\n}\nlog.info(repr(f(i)))\n",
    );
    add(
        "arity",
        "pull f(a, b) {\n  snuff a\n}\nroll g = f\nlog.info(repr(g(1)))\n",
    );
    add("call-a-number", "roll g = 3\nlog.info(repr(g(1)))\n");
    add(
        "loop-in-callee",
        "pull spin(n) {\n  roll i = 0\n  while i < n {\n    i += 1\n  }\n  snuff i\n}\nroll t = 0\nroll k = 0\nwhile k < 4 {\n  k += 1\n  t += spin(k)\n}\nlog.info(repr(t))\n",
    );
    add(
        "early-snuff-in-loop",
        "pull find(n) {\n  roll i = 0\n  while true {\n    i += 1\n    if i * i > n {\n      snuff i\n    }\n  }\n}\nlog.info(repr(find(50) + find(1000)))\n",
    );
    add(
        "top-level-break-in-loop",
        "roll i = 0\nwhile true {\n  i += 1\n  if i == 7 {\n    log.info(repr(i))\n    break\n  }\n}\n",
    );
    add(
        "nested-pull",
        "pull outer(a) {\n  pull inner(b) {\n    snuff b + 1\n  }\n  snuff inner(a) * 2\n}\nlog.info(repr(outer(20)))\n",
    );

    // Builtin calls, and their errors.
    add(
        "builtins",
        "roll a = str(12)\nroll b = len(a)\nroll c = int(\"41\")\nroll d = type_of(2.5)\nlog.info(repr(a + str(b) + str(c + 1) + d))\n",
    );
    add("builtin-arity", "roll a = len()\nlog.info(repr(a))\n");
    add("builtin-error", "roll a = int(\"x\")\nlog.info(repr(a))\n");
    add(
        "assert",
        "roll n = 2\nassert(n == 3, \"n is not three\")\nlog.info(repr(n))\n",
    );

    // Nothing to say.
    add("silent", "roll n = 1\nn += 1\n");
    c
}

// ----- item 4: run it -----------------------------------------------------------

/// Every corpus program at -O0 and -O1: the value the top level leaves,
/// or the error's code, message, line and column, equal to the
/// tree-walker's; and the same number of steps.
#[test]
fn the_lowered_program_runs_like_the_interpreter() {
    big_stack(|| {
        let (mut runs, mut errors) = (0, 0);
        for (name, src) in corpus() {
            let (want, want_steps) = tree_walker(&src, 0);
            errors += usize::from(want.end.is_some());
            for level in [0, 1] {
                let m = compile(&src, level);
                let (got, got_steps) = vm(&m, 0);
                assert_eq!(got, want, "{name} -O{level}:\n{src}");
                assert_eq!(got_steps, want_steps, "{name} -O{level}: steps");
                runs += 1;
            }
        }
        println!(
            "{runs} runs agree ({} programs, {errors} of them stop on an error)",
            corpus().len()
        );
    });
}

/// For each corpus program and every `--max-steps` from 1 to its total
/// (and one past it): the same stop, line, column and code, or the same
/// value when the budget is enough.
#[test]
fn the_step_budget_stops_on_the_same_line_through_bytecode() {
    big_stack(|| {
        let mut budgets = 0u64;
        for (name, src) in corpus() {
            let (_, total) = tree_walker(&src, 0);
            let m = compile(&src, 1);
            // Every budget up to 2,000 steps; past that (the deep
            // recursions), every 97th and the last fifty, which keeps the
            // run linear.
            let budgets_of =
                (1..=total + 1).filter(|b| total <= 2000 || b % 97 == 0 || *b + 50 > total);
            for max in budgets_of {
                let (want, _) = tree_walker(&src, max);
                let (got, _) = vm(&m, max);
                assert_eq!(got, want, "{name} with --max-steps {max}:\n{src}");
                budgets += 1;
            }
        }
        println!("{budgets} budgets agree");
    });
}

/// The repository's program generator: every program either lowers to a
/// module that verifies, or is refused with a construct this part does
/// not cover. Each generated program is then shrunk the way the
/// differential suite shrinks one, dropping the top-level statement a
/// refusal (or a check that a drop broke) names until it lowers; what
/// lowers verifies, and runs in the VM as it runs in the tree-walker.
#[test]
fn every_lowered_module_verifies() {
    big_stack(|| {
        const SEEDS: u64 = 3000;
        let (mut lowered, mut refused) = (0, 0);
        let mut constructs: std::collections::BTreeMap<String, u32> = Default::default();
        let (mut shrunk, mut kept_stmts, mut ran) = (0, 0, 0);
        for seed in 0..SEEDS {
            let mut stmts = gen::program(seed);
            let text = stmts.join("\n") + "\n";
            match bc::compile(&text, "gen.cig", &pass::default_passes()) {
                Ok(m) => {
                    verify_module(&m).unwrap_or_else(|e| panic!("seed {seed}: {e}\n{text}"));
                    lowered += 1;
                }
                Err(CompileError::Bytecode(Error::Refused { construct, .. })) => {
                    *constructs.entry(construct).or_default() += 1;
                    refused += 1;
                }
                Err(e) => panic!("seed {seed}: {e:?}\n{text}"),
            }
            // Shrink to the part this lowering covers.
            loop {
                if stmts.is_empty() {
                    break;
                }
                let text = stmts.join("\n") + "\n";
                let line = match bc::compile(&text, "gen.cig", &pass::default_passes()) {
                    Ok(m) => {
                        verify_module(&m).unwrap_or_else(|e| panic!("seed {seed}: {e}\n{text}"));
                        shrunk += 1;
                        kept_stmts += stmts.len();
                        let (got, _) = vm(&m, 0);
                        let (want, _) = tree_walker(&text, 0);
                        assert_eq!(got, want, "seed {seed}, shrunk:\n{text}");
                        ran += 1;
                        break;
                    }
                    Err(CompileError::Bytecode(Error::Refused { span, .. })) => span.line,
                    Err(CompileError::Lower(cigcompile::cir::LowerError::Diagnostic(d))) => {
                        d.line.unwrap_or(1)
                    }
                    Err(e) => panic!("seed {seed}: {e:?}\n{text}"),
                };
                // The statement that holds that line.
                let mut first = 1;
                let mut drop = stmts.len() - 1;
                for (k, st) in stmts.iter().enumerate() {
                    let n = st.lines().count().max(1) as u32;
                    if line < first + n {
                        drop = k;
                        break;
                    }
                    first += n;
                }
                stmts.remove(drop);
            }
        }
        println!(
            "{lowered} of {SEEDS} generated programs lowered as they are, {refused} refused: {constructs:?}"
        );
        println!(
            "shrunk to this part: {shrunk} programs lowered ({kept_stmts} top-level statements), every one verified, {ran} ran like the tree-walker"
        );
        assert_eq!(lowered + refused, SEEDS);
        assert!(shrunk > SEEDS / 2, "{shrunk}");
    });
}

/// A construct outside this part stops the lowering with an ordinary
/// error naming it and its position.
#[test]
fn a_program_outside_this_part_is_refused_with_its_construct() {
    let cases = [
        ("roll a = 1\nroll l = [a]\n", "a list", 2, 10),
        ("roll m = {k: 1}\n", "a map", 1, 10),
        (
            "roll a = 1\nroll s = \"n=${a}\"\n",
            "string interpolation",
            2,
            10,
        ),
        ("for i in [1] {\n  roll x = i\n}\n", "a list", 1, 10),
        (
            "roll a = 0\nfor i in 0..3 {\n  a += i\n}\n",
            "a `..` range",
            2,
            10,
        ),
        ("exhale 1\n", "`exhale`", 1, 1),
        (
            "try {\n  roll a = 1\n} ashtray e {\n  roll b = 2\n}\n",
            "`try`",
            1,
            1,
        ),
        ("roll a = 1\ncough a\n", "`cough`", 2, 1),
        (
            "roll s = \"ab\"\nroll n = s.len()\n",
            "a method call",
            2,
            10,
        ),
        (
            "pull f(n) {\n  snuff f(n)\n}\nroll x = 1\n",
            "a function that captures `f` (a closure)",
            1,
            1,
        ),
        (
            "roll k = 2\nstick g = pack(x) => x * k\n",
            "a function that captures `k` (a closure)",
            2,
            11,
        ),
        (
            "roll f = len\n",
            "a builtin, module or `args` used as a value",
            1,
            10,
        ),
        ("burn {\n  fs.write_text(\"o\", \"x\")\n}\n", "`burn`", 1, 1),
    ];
    for (src, construct, line, col) in cases {
        match bc::compile(src, "t.cig", &pass::level(0)) {
            Err(CompileError::Bytecode(Error::Refused { construct: c, span })) => {
                assert_eq!(c, construct, "{src}");
                assert_eq!((span.line, span.col), (line, col), "{src}: {c}");
            }
            other => panic!("{src}: {:?}", other.map(|_| ())),
        }
    }
}

/// Every CIR instruction kind, lowered or refused: the next part's to-do
/// list. A new kind does not compile here until it is added.
#[test]
fn every_cir_kind_is_lowered_or_refused() {
    fn name(k: &Kind) -> &'static str {
        match k {
            Kind::Step(_) => "step",
            Kind::Const(_) => "const",
            Kind::Prelude(_) => "prelude",
            Kind::Load(_) => "load",
            Kind::Decl(..) => "decl",
            Kind::Store(..) => "store",
            Kind::Unary(..) => "unary",
            Kind::Binary(..) => "binary",
            Kind::Short { .. } => "short",
            Kind::MakeList(_) => "list",
            Kind::MakeMap(_) => "map",
            Kind::Interp(_) => "interp",
            Kind::Member { .. } => "member",
            Kind::Index { .. } => "index",
            Kind::SetIndex { .. } => "set.index",
            Kind::SetMember { .. } => "set.member",
            Kind::Closure(_) => "closure",
            Kind::BurnOp { .. } => "burn.op",
            Kind::CallBuiltin { .. } => "call.builtin",
            Kind::CallUser { .. } => "call.user",
            Kind::CallValue { .. } => "call.value",
            Kind::CallMethod { .. } => "call.method",
            Kind::MakeChain { .. } => "chain",
            Kind::PackDecl(_) => "pack",
            Kind::Exhale(_) => "exhale",
            Kind::Burn(_) => "burn",
            Kind::Try { .. } => "try",
            Kind::If { .. } => "if",
            Kind::Loop { .. } => "loop",
            Kind::ForEach { .. } => "foreach",
            Kind::Yield(_) => "yield",
            Kind::Ret(_) => "ret",
            Kind::Break => "break",
            Kind::Continue => "continue",
            Kind::Throw(_) => "throw",
        }
    }
    let v = ValueId(0);
    let kinds = vec![
        Kind::Step(StepSite::Stmt),
        Kind::Const(Const::Null),
        Kind::Prelude("args".into()),
        Kind::Load(VarId(0)),
        Kind::Decl(VarId(0), v),
        Kind::Store(VarId(0), v),
        Kind::Unary(UnOp::Neg, v),
        Kind::Binary(BinOp::Add, v, v),
        Kind::Short {
            op: ShortOp::And,
            subject: v,
            rhs: Block::default(),
        },
        Kind::MakeList(vec![]),
        Kind::MakeMap(vec![(MapKey::Static("k".into()), v)]),
        Kind::Interp(vec![Piece::Lit("x".into())]),
        Kind::Member {
            obj: v,
            name: "k".into(),
            safe: false,
        },
        Kind::Index { obj: v, idx: v },
        Kind::SetIndex {
            obj: v,
            idx: v,
            op: AssignOp::Set,
            val: v,
        },
        Kind::SetMember {
            obj: v,
            name: "k".into(),
            op: AssignOp::Set,
            val: v,
        },
        Kind::Closure(FuncId(1)),
        Kind::BurnOp {
            kind: OpKind::Write,
            callee: "fs.write_text".into(),
            effect: Effect::Write,
            args: vec![],
        },
        Kind::CallBuiltin {
            callee: "len".into(),
            args: vec![],
        },
        Kind::CallUser {
            func: FuncId(1),
            args: vec![],
        },
        Kind::CallValue { f: v, args: vec![] },
        Kind::CallMethod {
            recv: v,
            name: "len".into(),
            args: vec![],
        },
        Kind::MakeChain {
            name: VarId(0),
            steps: vec![],
        },
        Kind::PackDecl(vec![]),
        Kind::Exhale(vec![]),
        Kind::Burn(Box::new(BurnNode {
            region: RegionId(1),
            class: BurnClass::Normal,
            state: None,
            label: None,
            body: Block::default(),
            compensate: None,
            unburn_src: None,
        })),
        Kind::Try {
            body: Block::default(),
            catch: None,
            finally: None,
        },
        Kind::If {
            cond: v,
            then: Block::default(),
            otherwise: None,
        },
        Kind::Loop {
            cond: Block::default(),
            body: Block::default(),
        },
        Kind::ForEach {
            iter: v,
            var: VarId(0),
            body: Block::default(),
        },
        Kind::Yield(v),
        Kind::Ret(v),
        Kind::Break,
        Kind::Continue,
        Kind::Throw(v),
    ];
    let table: Vec<String> = kinds
        .iter()
        .map(|k| match bc::refused_kind(k) {
            None => format!("{} lowered", name(k)),
            Some(c) => format!("{} refused: {c}", name(k)),
        })
        .collect();
    let want = [
        "step lowered",
        "const lowered",
        "prelude refused: a builtin, module or `args` used as a value",
        "load lowered",
        "decl lowered",
        "store lowered",
        "unary lowered",
        "binary lowered",
        "short lowered",
        "list refused: a list",
        "map refused: a map",
        "interp refused: string interpolation",
        "member refused: a member read",
        "index refused: an index read",
        "set.index refused: an index assignment",
        "set.member refused: a member assignment",
        "closure lowered",
        "burn.op refused: a call that changes the world (a burn op)",
        "call.builtin lowered",
        "call.user lowered",
        "call.value lowered",
        "call.method refused: a method call",
        "chain refused: a chain",
        "pack refused: `pack`",
        "exhale refused: `exhale`",
        "burn refused: `burn`",
        "try refused: `try`",
        "if lowered",
        "loop lowered",
        "foreach refused: `for`",
        "yield lowered",
        "ret lowered",
        "break lowered",
        "continue lowered",
        "throw refused: `cough`",
    ];
    assert_eq!(table, want);
    // Lowered, but refused for what it names: a closure or `call.user` of
    // a function that captures, and the `..` range operator (see
    // a_program_outside_this_part_is_refused_with_its_construct).
}

// ----- operands past 8 and 16 bits ---------------------------------------------------

/// Three hundred slots: registers past 255 take the `WIDE` forms, and the
/// program still runs like the interpreter.
#[test]
fn a_frame_past_255_registers_uses_wide_operands() {
    let mut src = String::new();
    for k in 0..300 {
        src.push_str(&format!("roll v{k} = {k}\n"));
    }
    src.push_str("roll s = 0\n");
    for k in (0..300).step_by(7) {
        src.push_str(&format!("s += v{k} * 3 - v{} \n", 299 - k));
    }
    src.push_str("log.info(repr(s + len(str(v299))))\n");
    big_stack(move || {
        let m = compile(&src, 0);
        assert!(m.main.reg_count > 256, "{}", m.main.reg_count);
        let text = bc::disassemble(&m);
        assert!(text.contains(" w "), "a wide instruction");
        assert_eq!(vm(&m, 0), tree_walker(&src, 0));
    });
}

/// A loop whose body is longer than a 16-bit jump: the function is
/// lowered again with wide jumps.
#[test]
fn a_jump_past_16_bits_is_wide() {
    let mut src = String::from("roll n = 0\nroll i = 0\nwhile i < 3 {\n  i += 1\n");
    for _ in 0..7000 {
        src.push_str("  n += i\n");
    }
    src.push_str("}\nlog.info(repr(n))\n");
    big_stack(move || {
        let m = compile(&src, 0);
        assert!(m.main.instrs.len() > 32768, "{}", m.main.instrs.len());
        assert_eq!(vm(&m, 0), tree_walker(&src, 0));
    });
}

// ----- item 3: see it ---------------------------------------------------------------

/// `--emit bc`'s text, byte for byte: tests/bc/<name>.bc from
/// tests/bc/<name>.cig at -O1. After a deliberate change,
/// BC_UPDATE_GOLDEN=1 writes them again.
#[test]
fn the_disassembly_matches_its_golden_text() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/bc");
    let lf = |t: String| t.replace("\r\n", "\n");
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| {
            let p = e.unwrap().path();
            if p.extension()? != "cig" {
                return None;
            }
            Some(p.file_stem()?.to_string_lossy().into_owned())
        })
        .collect();
    names.sort();
    assert!(!names.is_empty());
    for name in names {
        let file = format!("{name}.cig");
        let src = lf(std::fs::read_to_string(dir.join(&file)).unwrap());
        let m = match bc::compile(&src, &file, &pass::default_passes()) {
            Ok(m) => m,
            Err(e) => panic!("{file}: {e:?}"),
        };
        let text = bc::disassemble(&m);
        let golden = dir.join(format!("{name}.bc"));
        if std::env::var_os("BC_UPDATE_GOLDEN").is_some() {
            std::fs::write(&golden, &text).unwrap();
        }
        assert_eq!(
            text,
            lf(std::fs::read_to_string(&golden).unwrap_or_default()),
            "{file}: BC_UPDATE_GOLDEN=1 writes it again"
        );
    }
}

/// The passes shape the bytecode: a constant expression -O1 folds is
/// one load, where -O0 computes it.
#[test]
fn the_level_shapes_the_bytecode() {
    let src = "stick a = 2 + 3\nlog.info(repr(a * 4))\n";
    let o0 = compile(src, 0);
    let o1 = compile(src, 1);
    let ops = |m: &Module| bc::disassemble(m).matches(" add ").count();
    assert_eq!(ops(&o0), 1);
    assert_eq!(ops(&o1), 0);
    let _: &[&dyn Pass] = &pass::level(1);
}

/// `cigcompile build --emit bc` prints the golden text; a script outside
/// this part is refused with its construct, its position and exit 3. Run
/// with a scratch home: no runtime is needed or looked for.
#[test]
fn emit_bc_prints_the_disassembly_and_refuses_what_it_does_not_cover() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let cc = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_cigcompile"))
            .args(args)
            .current_dir(dir.path())
            .env("HOME", &home)
            .env("CIGSCRIPT_HOME", home.join(".cigscript"))
            .env("XDG_CONFIG_HOME", home.join(".config"))
            .env("XDG_DATA_HOME", home.join(".local/share"))
            .env("XDG_CACHE_HOME", home.join(".cache"))
            .env("XDG_STATE_HOME", home.join(".local/state"))
            .env("NO_COLOR", "1")
            .env("CIG_DOCTOR", "0")
            .env("CIG_REPORTS", "never")
            .env("CIGCOMPILE_OFFLINE", "1")
            .output()
            .unwrap()
    };
    let golden = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/bc");
    let src = std::fs::read_to_string(golden.join("calls.cig"))
        .unwrap()
        .replace("\r\n", "\n");
    std::fs::write(dir.path().join("calls.cig"), src).unwrap();
    let o = cc(&["build", "calls.cig", "--emit", "bc"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let want = std::fs::read_to_string(golden.join("calls.bc"))
        .unwrap()
        .replace("\r\n", "\n");
    assert_eq!(String::from_utf8_lossy(&o.stdout), want);

    std::fs::write(dir.path().join("list.cig"), "roll a = 1\nroll l = [a]\n").unwrap();
    let o = cc(&["build", "list.cig", "--emit", "bc"]);
    assert_eq!(o.status.code(), Some(3));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(
        err.contains("a list is not lowered to bytecode yet") && err.contains("list.cig:2:10"),
        "{err}"
    );
}

/// More live slots than a frame can hold: an ordinary error at the
/// statement that crosses the limit, not a module the verifier refuses.
#[test]
fn a_frame_past_the_limit_is_an_error() {
    let mut src = String::new();
    for k in 0..65_540 {
        src.push_str(&format!("roll v{k} = 1\n"));
    }
    match bc::compile(&src, "t.cig", &pass::level(0)) {
        Err(CompileError::Bytecode(Error::Limit { what, span })) => {
            assert_eq!(
                what,
                "the function needs more than 65536 registers, the frame limit"
            );
            assert_eq!(span.line, 65_536);
        }
        other => panic!("{:?}", other.map(|_| ())),
    }
}
