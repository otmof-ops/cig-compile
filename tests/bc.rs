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
use cigscript::syntax::span::Span;
use cigscript::value::Effect;
use std::cell::RefCell;
use std::rc::Rc;

// ----- running both ways ---------------------------------------------------------

/// What a run left: the bytes it printed (`exhale`) and wrote to the
/// error stream (`log`), and how it ended: cleanly, with an `exit` code,
/// or with the error's code, message, hint, line and column.
#[derive(Debug, PartialEq)]
struct Outcome {
    printed: String,
    logged: String,
    exit: Option<i32>,
    end: Option<Stop>,
}

/// An error's code, message, hint, line and column.
type Stop = (String, String, Option<String>, Option<u32>, Option<u32>);

/// One of the interpreter's streams, kept for the comparison.
#[derive(Clone, Default)]
struct Stream(Rc<RefCell<Vec<u8>>>);

impl std::io::Write for Stream {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct Run {
    interp: Interp,
    out: Stream,
    err: Stream,
}

fn interp(max_steps: u64) -> Run {
    let (out, err) = (Stream::default(), Stream::default());
    let mut i = Interp::new(Kernel::ephemeral(Mode::Run));
    i.out = Box::new(out.clone());
    i.err = Box::new(err.clone());
    i.set_max_steps(max_steps);
    Run {
        interp: i,
        out,
        err,
    }
}

fn outcome(r: Result<(), Diagnostic>, run: &Run) -> (Outcome, u64) {
    let exit = run.interp.exit_requested;
    let text = |s: &Stream| String::from_utf8_lossy(&s.0.borrow()).into_owned();
    (
        Outcome {
            printed: text(&run.out),
            logged: text(&run.err),
            exit,
            // `exit()` stops the tree-walker cleanly and the VM with the
            // request recorded: both are the exit, not an error.
            end: r
                .err()
                .filter(|_| exit.is_none())
                .map(|d| (d.code.to_string(), d.message, d.hint, d.line, d.col)),
        },
        run.interp.steps,
    )
}

/// The tree-walker, as `cig run` runs a script, and the steps it counted.
fn tree_walker(src: &str, max_steps: u64) -> (Outcome, u64) {
    let program = cigscript::syntax::parse(src).expect("parses");
    let mut run = interp(max_steps);
    let r = run.interp.run_program(&program);
    outcome(r, &run)
}

/// The VM, through its public entry points: `verify_module`, `vm::load`,
/// `vm::run`. A script has no top-level `snuff`, so the top level leaves
/// null; what it computed comes out through `exhale`, which both write
/// through the interpreter's one writer.
fn vm(m: &Module, max_steps: u64) -> (Outcome, u64) {
    let verified = verify_module(m).unwrap_or_else(|e| panic!("does not verify: {e}"));
    let program =
        cigscript::vm::load(verified).unwrap_or_else(|e| panic!("does not load: {}", e.message));
    let mut run = interp(max_steps);
    let r = cigscript::vm::run(&mut run.interp, &program).map(|v| {
        assert_eq!(v.repr(), "null", "the top level leaves null");
    });
    outcome(r, &run)
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

/// Small programs inside what the lowering covers. Each prints what it
/// computed with `exhale`, or stops on an error first.
fn corpus() -> Vec<(String, String)> {
    let mut c: Vec<(String, String)> = Vec::new();
    let mut add = |name: &str, src: &str| c.push((name.to_string(), src.to_string()));

    // Arithmetic at the i64 edges, and division by zero.
    add(
        "max-plus-one",
        "roll a = 9223372036854775807\nroll b = 1\nexhale a + b\n",
    );
    add(
        "min-minus-one",
        "roll a = -9223372036854775807 - 1\nroll b = 1\nexhale a - b\n",
    );
    add(
        "min-negated",
        "roll a = -9223372036854775807 - 1\nexhale -a\n",
    );
    add("mul-overflow", "roll a = 3037000500\nexhale a * a\n");
    add("mul-edge", "roll a = 3037000499\nexhale a * a\n");
    add(
        "min-div-minus-one",
        "roll a = -9223372036854775807 - 1\nroll b = -1\nexhale a / b\n",
    );
    add(
        "min-mod-minus-one",
        "roll a = -9223372036854775807 - 1\nroll b = -1\nexhale a % b\n",
    );
    add("div-zero", "roll a = 7\nroll b = 0\nexhale a / b\n");
    add("mod-zero", "roll a = 7\nroll b = 0\nexhale a % b\n");
    add(
        "float-div-zero",
        "roll a = 7.0\nroll b = 0.0\nexhale a / b\n",
    );
    add("int-div", "roll a = -7\nroll b = 2\nexhale a / b\n");
    add("int-mod", "roll a = -7\nroll b = 2\nexhale a % b\n");
    add("mixed", "roll a = 7\nroll b = 2.5\nexhale a * b - a / b\n");
    add("strings", "roll a = \"ci\"\nroll b = \"g\"\nexhale a + b\n");
    add("str-plus-int", "roll a = \"n\"\nroll b = 1\nexhale a + b\n");
    add(
        "big-constant",
        "roll a = 123456789012\nroll b = 40000\nexhale a + b - 70000\n",
    );
    add(
        "compound",
        "roll a = 10\na += 5\na -= 3\na *= 4\na /= 6\nexhale a\n",
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
                    &format!("roll x = {x}\nroll y = {y}\nexhale x {op} y\n"),
                );
            }
        }
    }
    add(
        "nan",
        "roll z = 0.0\nroll n = z / z\nroll o = 1.0\nexhale n < o\n",
    );
    add("not", "roll t = 0\nexhale not t\n");

    // Short-circuit logic and `??`.
    add(
        "short",
        "roll a = false and int(\"x\")\nroll b = true or int(\"x\")\nroll c = null ?? 3\nroll d = 4 ?? int(\"x\")\nroll e = 0 and 1\nroll f = \"\" or \"s\"\nroll g = 1 and 2\nroll h = false or null\nexhale str(a) + str(b) + str(c) + str(d) + str(e) + f + str(g) + str(h)\n",
    );

    // Nested `if`.
    add(
        "nested-if",
        "roll n = 0\nroll i = 0\nwhile i < 12 {\n  i += 1\n  if i % 2 == 0 {\n    if i % 3 == 0 {\n      n += 100\n    } else if i % 4 == 0 {\n      n += 10\n    } else {\n      n += 1\n    }\n  } else if i > 9 {\n    n -= 1\n  }\n}\nexhale n\n",
    );

    // `while` with `break` and `continue`, nested.
    add(
        "while-break-continue",
        "roll total = 0\nroll i = 0\nwhile i < 20 {\n  i += 1\n  if i == 3 {\n    continue\n  }\n  roll j = 0\n  while true {\n    j += 1\n    if j > i {\n      break\n    }\n    if j % 2 == 0 {\n      continue\n    }\n    total += j\n  }\n  if i == 15 {\n    break\n  }\n}\nexhale total\n",
    );
    add(
        "while-false",
        "roll n = 0\nwhile n > 0 {\n  n -= 1\n}\nexhale n\n",
    );

    // Pulls, lambdas, `snuff`, recursion (a pull that calls itself
    // through a parameter: one that names itself captures itself).
    add(
        "pulls",
        "pull add(a, b) {\n  snuff a + b\n}\npull none() {\n  roll x = 1\n}\nstick sq = pack(x) => x * x\nexhale str(add(sq(3), 4)) + str(none())\n",
    );
    add(
        "fact",
        "pull fact(self, n) {\n  if n <= 1 {\n    snuff 1\n  }\n  snuff n * self(self, n - 1)\n}\nexhale fact(fact, 20)\n",
    );
    add(
        "fact-overflow",
        "pull fact(self, n) {\n  if n <= 1 {\n    snuff 1\n  }\n  snuff n * self(self, n - 1)\n}\nexhale fact(fact, 25)\n",
    );
    add(
        "fib",
        "pull fib(self, n) {\n  if n < 2 {\n    snuff n\n  }\n  snuff self(self, n - 1) + self(self, n - 2)\n}\nexhale fib(fib, 10)\n",
    );
    add(
        "deep",
        "pull down(self, n) {\n  if n == 0 {\n    snuff 0\n  }\n  snuff 1 + self(self, n - 1)\n}\nexhale down(down, 3000)\n",
    );
    add(
        "too-deep",
        "pull down(self, n) {\n  if n == 0 {\n    snuff 0\n  }\n  snuff 1 + self(self, n - 1)\n}\nexhale down(down, 5000)\n",
    );
    add(
        "callee-type-error",
        "pull f(a) {\n  roll b = a * 2\n  snuff b + \"x\"\n}\nroll i = 0\nwhile i < 3 {\n  i += 1\n}\nexhale f(i)\n",
    );
    add(
        "arity",
        "pull f(a, b) {\n  snuff a\n}\nroll g = f\nexhale g(1)\n",
    );
    add("call-a-number", "roll g = 3\nexhale g(1)\n");
    add(
        "loop-in-callee",
        "pull spin(n) {\n  roll i = 0\n  while i < n {\n    i += 1\n  }\n  snuff i\n}\nroll t = 0\nroll k = 0\nwhile k < 4 {\n  k += 1\n  t += spin(k)\n}\nexhale t\n",
    );
    add(
        "early-snuff-in-loop",
        "pull find(n) {\n  roll i = 0\n  while true {\n    i += 1\n    if i * i > n {\n      snuff i\n    }\n  }\n}\nexhale find(50) + find(1000)\n",
    );
    add(
        "top-level-break-in-loop",
        "roll i = 0\nwhile true {\n  i += 1\n  if i == 7 {\n    exhale i\n    break\n  }\n}\n",
    );
    add(
        "nested-pull",
        "pull outer(a) {\n  pull inner(b) {\n    snuff b + 1\n  }\n  snuff inner(a) * 2\n}\nexhale outer(20)\n",
    );

    // Builtin calls, and their errors.
    add(
        "builtins",
        "roll a = str(12)\nroll b = len(a)\nroll c = int(\"41\")\nroll d = type_of(2.5)\nexhale a + str(b) + str(c + 1) + d\n",
    );
    add("builtin-arity", "roll a = len()\nexhale a\n");
    add("builtin-error", "roll a = int(\"x\")\nexhale a\n");
    add(
        "assert",
        "roll n = 2\nassert(n == 3, \"n is not three\")\nexhale n\n",
    );

    // Nothing to say.
    add("silent", "roll n = 1\nn += 1\n");
    // ----- closures that capture, and named recursion -----
    add("fib-by-name", "pull fib(n) {\n  if n < 2 {\n    snuff n\n  }\n  snuff fib(n - 1) + fib(n - 2)\n}\nexhale fib(15)\n");
    add("mutual", "pull even(n) {\n  if n == 0 {\n    snuff true\n  }\n  snuff odd(n - 1)\n}\npull odd(n) {\n  if n == 0 {\n    snuff false\n  }\n  snuff even(n - 1)\n}\nexhale even(10), odd(7), even(3)\n");
    add("deep-by-name", "pull down(n) {\n  if n == 0 {\n    snuff 0\n  }\n  snuff 1 + down(n - 1)\n}\nexhale down(5000)\n");
    add("counter", "pull make() {\n  roll n = 0\n  snuff pack() {\n    n += 1\n    snuff n\n  }\n}\nstick a = make()\nstick b = make()\nexhale a(), a(), b(), a()\n");
    add("per-turn-for", "roll fs = []\nfor i in 0..3 {\n  fs.push(pack() => i)\n}\nfor f in fs {\n  exhale f()\n}\n");
    add("per-turn-for-list", "roll fs = []\nfor s in [\"a\", \"b\"] {\n  fs.push(pack() => s + s)\n}\nexhale fs[0](), fs[1]()\n");
    add("per-turn-while", "roll fs = []\nroll k = 0\nwhile k < 3 {\n  roll j = k * 10\n  fs.push(pack() => j)\n  k += 1\n}\nexhale fs[0](), fs[1](), fs[2]()\n");
    add("shared-before-loop", "roll fs = []\nroll n = 0\nwhile n < 3 {\n  fs.push(pack() => n)\n  n += 1\n}\nexhale fs[0](), fs[2]()\n");
    add("three-levels", "pull outer(a) {\n  roll b = a * 2\n  pull mid(c) {\n    snuff pack(d) => a + b + c + d\n  }\n  snuff mid(100)\n}\nstick f = outer(1)\nexhale f(1000)\n");
    add("captured-param", "pull adder(x) {\n  snuff pack(y) => x + y\n}\nstick add5 = adder(5)\nexhale add5(1), adder(10)(2)\n");
    add("closure-writes", "roll total = 0\nstick bump = pack(x) {\n  total += x\n  snuff total\n}\nbump(3)\nbump(4)\nexhale total\n");
    add("nested-pulls-call-each-other", "pull run(n) {\n  pull a(x) {\n    if x <= 0 {\n      snuff 0\n    }\n    snuff b(x - 1) + 1\n  }\n  pull b(x) {\n    snuff a(x - 1) + n\n  }\n  snuff a(6)\n}\nexhale run(10)\n");
    add("callback", "roll seen = 0\nroll ys = [1, 2, 3].map(pack(x) {\n  seen += 1\n  snuff x * seen\n})\nexhale ys, seen\n");
    add(
        "callback-error",
        "roll ys = [1, \"a\"].map(pack(x) => x * 2)\nexhale ys\n",
    );
    add(
        "pull-as-value",
        "pull twice(x) {\n  snuff x * 2\n}\nexhale [1, 2].map(twice)\n",
    );
    // ----- lists and maps -----
    add("literals", "roll xs = [1, \"two\", [3, 4], {k: null}]\nroll m = {a: 1, \"b c\": [2], d: {e: true}}\nexhale xs, m\nexhale xs[2][1], m.d.e, m[\"b c\"][0]\n");
    add(
        "negative-index",
        "roll xs = [1, 2, 3]\nroll s = \"héllo\"\nexhale xs[-1], s[1], s[-1]\n",
    );
    add("index-past-end", "roll xs = [1, 2]\nexhale xs[5]\n");
    add("string-index-past-end", "roll s = \"ab\"\nexhale s[9]\n");
    add(
        "index-type",
        "roll xs = [1, 2]\nroll k = \"a\"\nexhale xs[k]\n",
    );
    add("map-key-type", "roll m = {a: 1}\nroll k = 1\nexhale m[k]\n");
    add("index-into-int", "roll n = 5\nexhale n[0]\n");
    add("missing-key-index", "roll m = {a: 1}\nexhale m[\"b\"]\n");
    add("missing-member", "roll m = {a: 1}\nexhale m.b\n");
    add("member-of-null", "roll m = null\nexhale m.b\n");
    add("member-of-int", "roll m = 3\nexhale m.b\n");
    add("safe-member", "roll m = {a: 1, n: null}\nroll z = null\nroll s = \"x\"\nexhale m?.a, m?.b, m?.n, z?.a\nexhale s?.a\n");
    add(
        "safe-member-shadowed",
        "roll m = {keys: 1, contains: 2, a: 3}\nexhale m?.a, m?.keys, m?.zz\n",
    );
    add("stores", "roll xs = [1, 2, 3]\nroll m = {a: 1}\nxs[0] = 9\nxs[-1] += 10\nm[\"b\"] = 2\nm.c = 3\nm.a += 5\nm[\"b\"] *= 7\nexhale xs, m\n");
    add("store-past-end", "roll xs = [1]\nxs[3] = 1\n");
    add(
        "store-index-type",
        "roll xs = [1]\nroll k = \"a\"\nxs[k] = 1\n",
    );
    add("store-map-key-type", "roll m = {}\nroll k = 2\nm[k] = 1\n");
    add("store-into-int", "roll n = 4\nroll k = 0\nn[k] = 1\n");
    add(
        "store-into-string",
        "roll s = \"ab\"\nroll k = 0\ns[k] = \"x\"\n",
    );
    add(
        "compound-into-string",
        "roll s = \"ab\"\nroll k = 0\ns[k] += \"x\"\n",
    );
    add("compound-missing-key", "roll m = {a: 1}\nm[\"b\"] += 1\n");
    add("compound-missing-member", "roll m = {a: 1}\nm.b += 1\n");
    add("compound-past-end", "roll xs = [1]\nxs[2] -= 1\n");
    add("compound-type", "roll xs = [\"a\"]\nxs[0] -= 1\n");
    add("set-member-on-int", "roll n = 1\nn.k = 2\n");
    add("compound-member-on-int", "roll n = 1\nn.k += 2\n");
    add(
        "member-compound-type",
        "roll m = {a: \"s\"}\nm.a *= \"t\"\n",
    );
    add(
        "dynamic-keys",
        "roll k = \"x\"\nroll n = 7\nroll m = {[k]: 1, [n]: 2, plain: 3}\nexhale m\n",
    );
    add(
        "dynamic-key-type",
        "roll f = 1.5\nroll m = {[f]: 1}\nexhale m\n",
    );
    add(
        "aliasing",
        "roll a = [1]\nroll b = a\nb.push(2)\nroll m = {l: a}\nm.l.push(3)\nexhale a, b, m\n",
    );
    // ----- for -----
    add("for-range", "roll t = 0\nfor i in 0..10 {\n  if i == 3 {\n    continue\n  }\n  if i == 8 {\n    break\n  }\n  t += i\n}\nexhale t\n");
    add("for-range-empty-and-back", "roll n = 0\nfor i in 5..5 {\n  n += 1\n}\nfor i in 5..2 {\n  n += 10\n}\nfor i in -3..-1 {\n  n += i\n}\nexhale n\n");
    add(
        "for-range-bounds",
        "roll a = 2\nroll b = a * 3\nfor i in a..b {\n  i = 100\n  exhale i\n}\n",
    );
    add(
        "for-range-float",
        "roll t = 0\nfor i in 1..2.5 {\n  t += i\n}\n",
    );
    add(
        "for-range-too-big",
        "roll t = 0\nfor i in 0..100000000 {\n  t += i\n}\n",
    );
    add("for-nested", "roll t = 0\nfor i in 0..4 {\n  for j in 0..i {\n    if j == 2 {\n      break\n    }\n    t += i * j\n  }\n}\nexhale t\n");
    add("for-list-map-text", "roll out = []\nfor x in [1, [2], null] {\n  out.push(x)\n}\nfor k in {b: 1, a: 2} {\n  out.push(k)\n}\nfor c in \"hé!\" {\n  out.push(c)\n}\nfor e in [] {\n  out.push(e)\n}\nexhale out\n");
    add("for-mutating", "roll xs = [1, 2]\nfor x in xs {\n  xs.push(x * 10)\n}\nroll m = {a: 1}\nfor k in m {\n  m[k + k] = 2\n}\nexhale xs, m\n");
    add(
        "for-not-iterable",
        "roll n = 5\nfor x in n {\n  exhale x\n}\n",
    );
    add("for-continue-list", "roll t = []\nfor x in [1, 2, 3, 4] {\n  if x % 2 == 0 {\n    continue\n  }\n  t.push(x)\n}\nexhale t\n");
    add("for-in-pull", "pull total(xs) {\n  roll t = 0\n  for x in xs {\n    t += x\n  }\n  snuff t\n}\nroll s = 0\nfor i in 0..3 {\n  s += total([i, i])\n}\nexhale s\n");
    // ----- interpolation, exhale, method calls -----
    add("interp", "roll n = 3\nroll xs = [1, \"a\"]\nroll m = {k: 2.5}\nexhale \"n=${n} xs=${xs} m=${m} f=${1.0} ${null} ${true}\"\n");
    add(
        "interp-sees-before",
        "roll xs = [1]\nexhale \"${xs} then ${xs.push(2)} ${xs}\"\n",
    );
    add(
        "exhale-sees-before",
        "roll xs = [1]\nexhale xs, xs.push(2), xs\n",
    );
    add(
        "exhale-many",
        "exhale 1, \"two\", 3.5, null, [true]\nexhale \"\"\n",
    );
    add("methods", "roll s = \"Hello\"\nroll xs = [3, 1, 2]\nexhale s.upper(), s.len(), xs.len(), xs.contains(2)\nroll ys = xs.filter(pack(x) => x > 1)\nexhale ys, xs.map(pack(x) => x * x)\n");
    add("method-unknown", "roll xs = [1]\nexhale xs.lenn()\n");
    add("method-arity", "roll xs = [1]\nxs.push()\n");
    add(
        "safe-method",
        "roll z = null\nroll s = \"a\"\nexhale z?.upper(), s?.upper()\n",
    );
    add(
        "map-holds-function",
        "roll m = {f: pack(x) => x + 1}\nexhale m.f(1)\n",
    );
    add("exit", "exhale 1\nexit(3)\nexhale 2\n");
    add(
        "interp-lines",
        "pull f(x) {\n  roll a = 1\n  snuff \"${a} ${x / 0}\"\n}\nexhale f(1)\n",
    );
    add(
        "interp-index",
        "roll xs = [1]\nroll k = \"a\"\nexhale \"v=${xs[k]}\"\n",
    );
    add(
        "range-values",
        "roll a = 3\nroll b = 0\nexhale 0..4, a..b, -2..1, (0..3).map(pack(x) => x * x)\n",
    );
    add(
        "range-value-float",
        "roll a = 1\nroll b = 2.5\nroll r = a..b\n",
    );
    add("range-value-too-big", "roll a = 0\nroll r = a..100000000\n");
    add("range-value-extremes", "roll a = 9223372036854775807\nroll b = -9223372036854775807 - 1\nexhale a..b\nexhale b..a\n");
    // A computed key is checked before the entry's value runs; a pass
    // that replaces an operand leaves its position where it was.
    add(
        "key-before-failing-value",
        "roll k = true\nexhale {[k]: 1, b: 1 / 0}\n",
    );
    add(
        "key-before-a-call",
        "pull f() {\n  snuff \"v\"\n}\nroll k = \"j\"\nroll m = {[k]: f(), [3]: f()}\nexhale m\n",
    );
    add(
        "key-effect-read-by-value",
        "roll xs = [1]\nroll m = {[str(xs.push(2))]: xs.len(), n: xs}\nexhale m\n",
    );
    add(
        "key-list-after-copy-prop",
        "stick k = [1]\nexhale {[k]: 1}\n",
    );
    add(
        "key-int-after-copy-prop",
        "stick n = 7\nstick m = {[n]: 1}\nexhale m[\"7\"]\n",
    );
    add(
        "index-type-after-copy-prop",
        "stick xs = [1, 2]\nstick k = \"a\"\nexhale xs[k]\n",
    );
    add(
        "cannot-index-after-copy-prop",
        "stick n = 5\nstick k = 0\nexhale n[k]\n",
    );
    add(
        "store-index-type-after-copy-prop",
        "stick xs = [1]\nstick k = \"a\"\nxs[k] = 2\n",
    );
    add("store-into-int-after-copy-prop", "stick n = 5\nn[0] = 1\n");
    add("member-store-after-copy-prop", "stick n = 5\nn.k = 1\n");
    add(
        "member-compound-after-copy-prop",
        "stick n = \"s\"\nn.k += 1\n",
    );
    add(
        "module-call",
        "exhale math.floor(2.5), path.join(\"a\", \"b\"), text.split(\"a,b\", \",\")\n",
    );
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
        (
            "try {\n  roll a = 1\n} ashtray e {\n  roll b = 2\n}\n",
            "`try`",
            1,
            1,
        ),
        ("roll a = 1\ncough a\n", "`cough`", 2, 1),
        (
            "roll f = len\n",
            "a builtin, module or `args` used as a value",
            1,
            10,
        ),
        (
            "roll m = math\n",
            "a builtin, module or `args` used as a value",
            1,
            10,
        ),
        (
            "roll a = args\n",
            "a builtin, module or `args` used as a value",
            1,
            10,
        ),
        ("burn {\n  fs.write_text(\"o\", \"x\")\n}\n", "`burn`", 1, 1),
        (
            "pull w() {\n  fs.write_text(\"o\", \"x\")\n}\nroll n = 1\n",
            "a call that changes the world (a burn op)",
            2,
            3,
        ),
        ("pack { \"./build\" }\nroll n = 1\n", "`pack`", 1, 1),
        (
            "stick a = pack(x) => x\nstick b = pack(x) => x\nchain c { a, b }\n",
            "a chain",
            3,
            1,
        ),
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
            Kind::Key(_) => "key",
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
        Kind::Key(v),
        Kind::Interp(vec![Piece::Lit("x".into())]),
        Kind::Member {
            obj: v,
            name: "k".into(),
            safe: false,
        },
        Kind::Index {
            obj: v,
            idx: v,
            idx_at: Span::default(),
        },
        Kind::SetIndex {
            obj: v,
            idx: v,
            op: AssignOp::Set,
            val: v,
            obj_at: Span::default(),
            idx_at: Span::default(),
        },
        Kind::SetMember {
            obj: v,
            name: "k".into(),
            op: AssignOp::Set,
            val: v,
            obj_at: Span::default(),
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
        "list lowered",
        "map lowered",
        "key lowered",
        "interp lowered",
        "member lowered",
        "index lowered",
        "set.index lowered",
        "set.member lowered",
        "closure lowered",
        "burn.op refused: a call that changes the world (a burn op)",
        "call.builtin lowered",
        "call.user lowered",
        "call.value lowered",
        "call.method lowered",
        "chain refused: a chain",
        "pack refused: `pack`",
        "exhale lowered",
        "burn refused: `burn`",
        "try refused: `try`",
        "if lowered",
        "loop lowered",
        "foreach lowered",
        "yield lowered",
        "ret lowered",
        "break lowered",
        "continue lowered",
        "throw refused: `cough`",
    ];
    assert_eq!(table, want);
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
    src.push_str("exhale s + len(str(v299))\n");
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
    src.push_str("}\nexhale n\n");
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
    let src = "stick a = 2 + 3\nexhale a * 4\n";
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

    std::fs::write(dir.path().join("value.cig"), "roll a = 1\nroll f = len\n").unwrap();
    let o = cc(&["build", "value.cig", "--emit", "bc"]);
    assert_eq!(o.status.code(), Some(3));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(
        err.contains("a builtin, module or `args` used as a value is not lowered to bytecode yet")
            && err.contains("value.cig:2:10"),
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

/// How far stage B is from real programs: every script of the CigScript
/// checkout `CIGSCRIPT_SRC` names (`examples`, `tests/scripts`,
/// `tests/corpus`, as the differential suite reads them) and
/// `orchestrate.cig` of the checkout `CIGO_SRC` names, each lowered at
/// -O1 or named by the construct that first refuses it. A count, not a
/// gate: what lowers verifies, and nothing may fail inside the lowering.
#[test]
fn real_programs_lower_or_name_what_refuses_them() {
    let mut scripts = Vec::new();
    if let Some(src) = std::env::var_os("CIGSCRIPT_SRC").map(std::path::PathBuf::from) {
        for dir in ["examples", "tests/scripts", "tests/corpus"] {
            if let Ok(rd) = std::fs::read_dir(src.join(dir)) {
                let mut found: Vec<_> = rd
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|x| x == "cig"))
                    .collect();
                found.sort();
                scripts.extend(found.into_iter().map(|p| (format!("{dir}/"), p)));
            }
        }
    }
    if let Some(src) = std::env::var_os("CIGO_SRC").map(std::path::PathBuf::from) {
        scripts.push((String::new(), src.join("orchestrate.cig")));
    }
    if scripts.is_empty() {
        eprintln!("note: skipping; set CIGSCRIPT_SRC and CIGO_SRC to checkouts");
        return;
    }
    let (mut lowered, mut refused, mut unchecked) = (0, 0, 0);
    let mut firsts: std::collections::BTreeMap<String, u32> = Default::default();
    for (dir, path) in &scripts {
        let name = format!("{dir}{}", path.file_name().unwrap().to_string_lossy());
        let src = std::fs::read_to_string(path).unwrap();
        let what = match bc::compile(&src, &name, &pass::default_passes()) {
            Ok(m) => {
                verify_module(&m).unwrap_or_else(|e| panic!("{name}: {e}"));
                lowered += 1;
                // The language scripts need no files: run them both ways.
                if dir == "tests/scripts/" {
                    let src = src.clone();
                    let (got, want) =
                        big_stack(move || (vm(&m, 1_000_000), tree_walker(&src, 1_000_000)));
                    assert_eq!(got, want, "{name}");
                    "lowered, and runs like the tree-walker".to_string()
                } else {
                    "lowered".to_string()
                }
            }
            Err(CompileError::Bytecode(Error::Refused { construct, span })) => {
                refused += 1;
                *firsts.entry(construct.clone()).or_default() += 1;
                format!("refused at {}:{}: {construct}", span.line, span.col)
            }
            Err(CompileError::Lower(cigcompile::cir::LowerError::Diagnostic(d))) => {
                unchecked += 1;
                format!("does not check: {} {}", d.code, d.message)
            }
            Err(e) => panic!("{name}: {e:?}"),
        };
        println!("{name}: {what}");
    }
    println!(
        "{} scripts: {lowered} lowered, {refused} refused, {unchecked} do not check; first refusals: {firsts:?}",
        scripts.len()
    );
}
