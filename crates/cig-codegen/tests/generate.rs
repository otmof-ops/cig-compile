// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2

//! The code generator on its own: what it compiles and what it leaves,
//! that the same input gives the same bytes for either machine, and, on
//! a machine with a loader, what the code it makes does.
//!
//! Whole programs are held to the tree-walker by the differential suite
//! (`tests/differential.rs` at the workspace root), which builds each at
//! stage `c`. These tests are the generator's own, closer to a defect.

use std::collections::BTreeMap;

use cig_codegen::{compile, Output};
use cig_native::{Arch, Section, Ty};
use cigscript::lower::cir::infer;
use cigscript::lower::cir::ir::{FuncId, Module};
use cigscript::lower::cir::{lower_source, pass};

/// A script as the IR native code is compiled from: lowered, through the
/// passes of level 1.
fn module(src: &str) -> Module {
    let mut m = lower_source(src, "t.cig").expect("the script lowers");
    pass::run_passes(&mut m, &pass::level(1)).expect("the passes hold");
    m
}

/// Each function's index in a module. A build takes it from the
/// lowering to bytecode; here any numbering does.
fn index(m: &Module) -> BTreeMap<FuncId, u32> {
    m.funcs.iter().map(|f| (f.id, f.id.0)).collect()
}

fn generate(src: &str, arch: Arch) -> (Module, Output) {
    let m = module(src);
    let out = compile(&m, &infer::infer(&m), &index(&m), [7; 32], arch).expect("it compiles");
    (m, out)
}

fn func(m: &Module, name: &str) -> FuncId {
    m.funcs
        .iter()
        .find(|f| f.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no function `{name}`"))
        .id
}

/// A script of several functions that compile, of every result type.
const SEVERAL: &str = "\
pull fib(n) {
  if n < 2 { snuff n }
  snuff fib(n - 1) + fib(n - 2)
}
pull scale(x) { snuff x * 1.5 }
pull positive(n) { snuff n > 0 }
pull nothing(n) { snuff }
pull sum_to(n) {
  roll s = 0
  for i in 0..n {
    if i == 2 { continue }
    s = s + i
  }
  snuff s
}
pull rem(a, b) { snuff a % b }
pull both(a, b) { snuff positive(a) and positive(b) }
exhale fib(10), scale(2.0), scale(3), positive(1), nothing(1), sum_to(7), rem(7, 3), both(1, 2)
";

#[test]
fn the_same_input_gives_the_same_bytes_for_each_machine() {
    let mut shapes = Vec::new();
    for arch in [Arch::X86_64, Arch::Aarch64] {
        let (_, first) = generate(SEVERAL, arch);
        let (_, again) = generate(SEVERAL, arch);
        let bytes = first.section.to_bytes().expect("a section");
        assert_eq!(Ok(&bytes), again.section.to_bytes().as_ref(), "{arch:?}");
        // What it wrote is a section, and reads back as it was made.
        assert_eq!(Section::parse(&bytes).as_ref(), Ok(&first.section));
        assert_eq!(first.section.arch, arch);
        assert_eq!(first.section.module, [7; 32]);
        // Code for a machine of fixed-size instructions is whole
        // instructions, and every entry starts one.
        let align = arch.code_align();
        assert_eq!(first.section.code.len() as u32 % align, 0);
        assert!(first.section.entries.iter().all(|e| e.offset % align == 0));
        shapes.push((
            first
                .section
                .entries
                .iter()
                .map(|e| (e.func, e.args.clone(), e.ret))
                .collect::<Vec<_>>(),
            first.section.sites.clone(),
        ));
    }
    // The two machines' code differs; what it is code for does not.
    assert_eq!(shapes[0], shapes[1]);
    assert!(shapes[0].0.len() >= 8, "{:?}", shapes[0].0);
}

#[test]
fn an_entry_has_its_functions_row_and_result_as_the_language_types_them() {
    use Ty::{Bool, Float, Int, Null};
    let (m, out) = generate(SEVERAL, Arch::X86_64);
    let entries = |name: &str| -> Vec<(Vec<Ty>, Ty)> {
        let f = func(&m, name).0;
        out.section
            .entries
            .iter()
            .filter(|e| e.func == f)
            .map(|e| (e.args.clone(), e.ret))
            .collect()
    };
    assert_eq!(entries("fib"), [(vec![Int], Int)]);
    // Two rows: called with a float and with an int.
    assert_eq!(entries("scale"), [(vec![Int], Float), (vec![Float], Float)]);
    assert_eq!(entries("positive"), [(vec![Int], Bool)]);
    assert_eq!(entries("nothing"), [(vec![Int], Null)]);
    assert_eq!(entries("sum_to"), [(vec![Int], Int)]);
    assert_eq!(entries("rem"), [(vec![Int, Int], Int)]);
    assert_eq!(entries("both"), [(vec![Int, Int], Bool)]);
}

#[test]
fn every_specialization_is_reported_compiled_or_with_its_reason() {
    let (m, out) = generate(SEVERAL, Arch::X86_64);
    let specs = infer::infer(&m);
    assert_eq!(out.report.len(), specs.specs().len());
    for o in &out.report {
        assert_eq!(o.compiled, o.why.is_empty(), "{:?}: {:?}", o.func, o.why);
    }
    // The top level is a specialization, and is never compiled.
    let top = out
        .report
        .iter()
        .find(|o| o.func == FuncId(0))
        .expect("the top level");
    assert_eq!(top.why, "the top level is not compiled");
    assert_eq!(out.report.iter().filter(|o| o.compiled).count(), 8);
}

/// The reason function `name` of `src` is left to the bytecode.
fn left(src: &str, name: &str) -> String {
    let (m, out) = generate(src, Arch::X86_64);
    let f = func(&m, name);
    let o = out
        .report
        .iter()
        .find(|o| o.func == f)
        .unwrap_or_else(|| panic!("`{name}` has no specialization"));
    assert!(!o.compiled, "`{name}` was compiled");
    assert!(out.section.entries.iter().all(|e| e.func != f.0));
    o.why.clone()
}

#[test]
fn what_native_code_cannot_hold_yet_is_left_to_the_bytecode_with_its_reason() {
    let mut wrong = Vec::new();
    for (body, call, why) in [
        // An int divided by an int is an int or a float by its values.
        (
            "snuff a / 2",
            "f(7)",
            "line 2: an int divided by an int (an int or a float, by its values)",
        ),
        ("snuff a % 2.5", "f(7)", "line 2: `%` with a float"),
        ("snuff \"n = ${a}\"", "f(7)", "line 2: a string"),
        ("snuff [a, a]", "f(7)", "line 2: a list or a map"),
        ("snuff str(a)", "f(7)", "line 2: calls a builtin"),
        ("snuff a.b", "f({b: 1})", "it is called with a map"),
        ("snuff a", "f(\"s\")", "it is called with a string"),
        ("exhale a\n  snuff a", "f(7)", "line 2: `exhale`"),
        ("snuff -a", "f(true)", "line 2: `-` on a bool"),
        (
            "snuff a + 1",
            "f(true)",
            "line 2: arithmetic on a bool and an int",
        ),
        (
            "roll s = 0\n  for x in [a, a] { s = s + x }\n  snuff s",
            "f(7)",
            "line 2: the variable `s` is not an int, a float, a bool or null on every path",
        ),
        (
            "try { snuff a + 1 } ashtray e { snuff 0 }",
            "f(7)",
            "line 2: a burn or a try",
        ),
        (
            "stick g = pack(x) => x + a\n  snuff g(1)",
            "f(7)",
            "line 2: a closure",
        ),
        // A value of two types on two paths.
        (
            "if a > 0 { snuff 1 }\n  snuff 1.5",
            "f(7)",
            "its result is not of one type on every path",
        ),
        // A `for` over what is not `a..b`.
        (
            "for i in a { }\n  snuff 1",
            "f(7)",
            "line 2: a `for` over something that is not `a..b` of two ints",
        ),
    ] {
        let src = format!("pull f(a) {{\n  {body}\n}}\nexhale {call}\n");
        let said = left(&src, "f");
        if said != why {
            wrong.push(format!("{body:?}: {said:?}, not {why:?}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
    // A variable a closure shares, and a function that calls one that is
    // left.
    let shared = "\
pull counter(n) {
  roll total = n
  pull bump() { total = total + 1 }
  bump()
  snuff total
}
exhale counter(1)
";
    assert!(left(shared, "counter").starts_with("line 3: a closure"));
    let chain = "\
pull text(n) { snuff str(n) }
pull calls(n) { snuff text(n) }
exhale calls(1)
";
    assert_eq!(left(chain, "text"), "line 1: calls a builtin");
    assert_eq!(
        left(chain, "calls"),
        "line 2: calls `text`, whose result is not of one type on every path"
    );
    let numbers = "\
pull halve(n) { snuff n / 2.0 }
pull uses(n) {
  stick h = halve(n)
  snuff str(h)
}
exhale uses(3)
";
    let (m, out) = generate(numbers, Arch::X86_64);
    assert!(out
        .report
        .iter()
        .any(|o| o.func == func(&m, "halve") && o.compiled));
    assert_eq!(left(numbers, "uses"), "line 4: calls a builtin");
}

#[test]
fn a_function_whose_frame_is_over_the_bound_is_left_and_so_is_its_caller() {
    // Six hundred values kept across a call: more than fit in registers
    // and 4,096 bytes of frame.
    let n = 600;
    let mut big = String::from("pull one(a) { snuff a + 1 }\npull big(a) {\n");
    for i in 0..n {
        big += &format!("  stick v{i} = a + {i}\n");
    }
    big += "  roll s = one(a)\n";
    for i in 0..n {
        big += &format!("  s = s + v{i}\n");
    }
    big += "  snuff s\n}\npull calls_big(a) { snuff big(a) + 1 }\nexhale calls_big(1)\n";
    for arch in [Arch::X86_64, Arch::Aarch64] {
        let (m, out) = generate(&big, arch);
        let why = |name: &str| {
            let f = func(&m, name);
            out.report
                .iter()
                .find(|o| o.func == f)
                .map(|o| (o.compiled, o.why.clone()))
        };
        assert_eq!(why("one"), Some((true, String::new())), "{arch:?}");
        assert_eq!(
            why("big"),
            Some((false, "frame larger than 4096 bytes".to_string())),
            "{arch:?}"
        );
        assert_eq!(
            why("calls_big"),
            Some((false, "calls a function not compiled".to_string())),
            "{arch:?}"
        );
        assert_eq!(out.section.entries.len(), 1, "{arch:?}");
    }
}

/// On a machine with a loader: what the generated code does.
#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod runs {
    use super::*;
    use cig_native::{op, Code, Context, Exit, Mapped, Site, Tables};
    use std::io::Write;

    /// A script's code, mapped and callable, with its tables.
    struct Loaded {
        m: Module,
        tables: Tables,
        code: Code,
        _keep: tempfile::NamedTempFile,
    }

    fn load(src: &str) -> Loaded {
        let (m, out) = generate(src, Arch::host().expect("a machine with a loader"));
        let body = out.section.to_bytes().expect("a section");
        let mut file = tempfile::NamedTempFile::new().expect("a temp file");
        // Where a program keeps the section: on a 64 KiB boundary.
        file.write_all(&vec![0; 65_536]).expect("written");
        file.write_all(&body).expect("written");
        file.flush().expect("flushed");
        let read = std::fs::File::open(file.path()).expect("opened");
        let tables = Tables::parse(&body).expect("its tables");
        let code_len = tables.code_len as usize;
        let mapped = Mapped::map(&read, 65_536, code_len).expect("mapped");
        // SAFETY: the file is this test's own and nothing writes it
        // after the flush; the code is what the generator made a moment
        // ago in this process, for this machine, with these tables.
        let code = unsafe { mapped.arm(&body[..code_len], &tables) }.expect("armed");
        Loaded {
            m,
            tables,
            code,
            _keep: file,
        }
    }

    impl Loaded {
        /// Call `name`'s entry for a row of `args.len()` ints or floats
        /// (the first that has that many arguments), with `steps` steps
        /// and `depth` calls: the status, the result slot and the
        /// context as it came back.
        fn call(&self, name: &str, args: &[u64], steps: i64, depth: i64) -> (u32, u64, Context) {
            let f = func(&self.m, name).0;
            let entry = self
                .tables
                .entries
                .iter()
                .position(|e| e.func == f && e.args.len() == args.len())
                .unwrap_or_else(|| panic!("`{name}` has no entry"));
            let mut ctx = Context::new(steps, depth);
            let (status, slot) = self.code.call(entry, &mut ctx, args).expect("called");
            (status, slot, ctx)
        }

        /// `name` on ints: its result, when it returned.
        fn int(&self, name: &str, args: &[i64]) -> Result<i64, (Exit, Site, Context)> {
            let slots: Vec<u64> = args.iter().map(|a| *a as u64).collect();
            let (status, slot, ctx) = self.call(name, &slots, 1_000_000, 100);
            match Exit::from_u8(status as u8) {
                None => Ok(slot as i64),
                Some(exit) => Err((exit, self.tables.sites[ctx.site as usize].clone(), ctx)),
            }
        }
    }

    #[test]
    fn functions_return_what_the_tree_walker_computes() {
        let l = load(SEVERAL);
        assert_eq!(l.int("fib", &[10]), Ok(55));
        assert_eq!(l.int("fib", &[20]), Ok(6765));
        // 0 + 1 + 3 + 4 + 5 + 6: `continue` goes on to the next number.
        assert_eq!(l.int("sum_to", &[7]), Ok(19));
        assert_eq!(l.int("sum_to", &[0]), Ok(0));
        assert_eq!(l.int("sum_to", &[-5]), Ok(0));
        // The remainder has the dividend's sign; by -1 it is 0 for any
        // dividend.
        for (a, b, r) in [(7, 3, 1), (-7, 3, -1), (7, -3, 1), (-7, -3, -1)] {
            assert_eq!(l.int("rem", &[a, b]), Ok(r), "{a} % {b}");
        }
        assert_eq!(l.int("rem", &[i64::MIN, -1]), Ok(0));
        assert_eq!(l.int("rem", &[i64::MAX, -1]), Ok(0));
        // A bool is 0 or 1, null is 0, a float is its bits.
        assert_eq!(l.int("positive", &[5]), Ok(1));
        assert_eq!(l.int("positive", &[0]), Ok(0));
        assert_eq!(l.int("nothing", &[5]), Ok(0));
        assert_eq!(l.int("both", &[1, 2]), Ok(1));
        assert_eq!(l.int("both", &[1, -2]), Ok(0));
        assert_eq!(l.int("both", &[-1, 2]), Ok(0));
        let scaled = |x: f64| {
            let (status, slot, _) = l.call("scale", &[x.to_bits()], 100, 10);
            assert_eq!(status, 0);
            f64::from_bits(slot)
        };
        // The second row of `scale` is the float's (rows sort int first).
        let f = func(&l.m, "scale").0;
        let float_row = l
            .tables
            .entries
            .iter()
            .position(|e| e.func == f && e.args == [Ty::Float])
            .expect("the float row");
        let mut ctx = Context::new(100, 10);
        let (status, slot) = l
            .code
            .call(float_row, &mut ctx, &[2.0f64.to_bits()])
            .expect("called");
        assert_eq!((status, f64::from_bits(slot)), (0, 3.0));
        let _ = scaled;
    }

    #[test]
    fn a_for_gives_its_variable_each_number_afresh() {
        let l = load(
            "\
pull turns(n) {
  roll s = 0
  for i in 0..n {
    i = i + 10
    s = s + 1
  }
  snuff s
}
pull last(a, b) {
  roll seen = -1
  for i in a..b { seen = i }
  snuff seen
}
exhale turns(5), last(1, 2)
",
        );
        // Storing to the variable does not move the loop.
        assert_eq!(l.int("turns", &[5]), Ok(5));
        assert_eq!(l.int("last", &[3, 6]), Ok(5));
        assert_eq!(l.int("last", &[6, 6]), Ok(-1));
        assert_eq!(l.int("last", &[7, 6]), Ok(-1));
        assert_eq!(l.int("last", &[i64::MAX - 1, i64::MAX]), Ok(i64::MAX - 1));
        assert_eq!(l.int("last", &[i64::MIN, i64::MIN + 3]), Ok(i64::MIN + 2));
        // Wider than the limit, by any distance: it leaves at the range.
        for (a, b) in [(0, 10_000_001), (i64::MIN, i64::MAX), (-1, i64::MAX)] {
            let (exit, site, ctx) = l.int("last", &[a, b]).expect_err("too wide");
            assert_eq!((exit, site.kind, site.line), (Exit::Range, Exit::Range, 11));
            assert_eq!((ctx.left as i64, ctx.right as i64), (a, b));
            assert!(!site.in_loop);
        }
        // The widest there is: ten million turns, two steps each.
        let (status, slot, ctx) = l.call("last", &[0, 10_000_000], i64::MAX, 100);
        assert_eq!((status, slot), (0, 9_999_999));
        assert_eq!(i64::MAX - ctx.steps_left, 20_000_003);
    }

    #[test]
    fn what_cannot_give_a_result_leaves_with_its_site_and_its_operands() {
        let l = load(
            "\
pull add(a, b) { snuff a + b }
pull mul(a, b) { snuff a * b }
pull neg(a) { snuff -a }
pull rem(a, b) { snuff a % b }
pull div(a, b) { snuff a / b }
pull less(a, b) { snuff a < b }
exhale add(1, 2), mul(1, 2), neg(1), rem(1, 2), div(1, 2.0), less(1, 2.0)
",
        );
        let (exit, site, ctx) = l.int("add", &[i64::MAX, 1]).expect_err("overflows");
        assert_eq!(
            (exit, site.kind, site.op),
            (Exit::Operator, Exit::Operator, op::ADD)
        );
        assert_eq!((site.left, site.right, site.line), (Ty::Int, Ty::Int, 1));
        assert_eq!((ctx.left as i64, ctx.right), (i64::MAX, 1));
        assert_eq!(l.int("add", &[i64::MAX, 0]), Ok(i64::MAX));
        let (_, site, ctx) = l.int("mul", &[i64::MIN, -1]).expect_err("overflows");
        assert_eq!((site.op, site.line), (op::MUL, 2));
        assert_eq!((ctx.left as i64, ctx.right as i64), (i64::MIN, -1));
        let (exit, site, ctx) = l.int("neg", &[i64::MIN]).expect_err("overflows");
        assert_eq!(
            (exit, site.left, site.right),
            (Exit::Negate, Ty::Int, Ty::Null)
        );
        assert_eq!(ctx.left as i64, i64::MIN);
        assert_eq!(l.int("neg", &[i64::MAX]), Ok(-i64::MAX));
        let (_, site, ctx) = l.int("rem", &[5, 0]).expect_err("by zero");
        assert_eq!((site.op, ctx.left, ctx.right), (op::REM, 5, 0));
        // An int by a float: the operands leave as they were, the int
        // an int.
        let div = |a: i64, b: f64| l.call("div", &[a as u64, b.to_bits()], 100, 10);
        let (status, slot, _) = div(1, 4.0);
        assert_eq!((status, f64::from_bits(slot)), (0, 0.25));
        for zero in [0.0, -0.0] {
            let (status, _, ctx) = div(7, zero);
            let site = &l.tables.sites[ctx.site as usize];
            assert_eq!(status, Exit::Operator as u32);
            assert_eq!(
                (site.op, site.left, site.right),
                (op::DIV, Ty::Int, Ty::Float)
            );
            assert_eq!((ctx.left, ctx.right), (7, zero.to_bits()));
        }
        // By a NaN it does not leave: the quotient is a NaN.
        let (status, slot, _) = div(7, f64::NAN);
        assert!(status == 0 && f64::from_bits(slot).is_nan());
        let less = |a: i64, b: f64| l.call("less", &[a as u64, b.to_bits()], 100, 10);
        assert_eq!(less(1, 1.5).0, 0);
        assert_eq!((less(1, 1.5).1, less(2, 1.5).1), (1, 0));
        let (status, _, ctx) = less(3, f64::NAN);
        let site = &l.tables.sites[ctx.site as usize];
        assert_eq!(status, Exit::Operator as u32);
        assert_eq!(
            (site.op, site.left, site.right),
            (op::LT, Ty::Int, Ty::Float)
        );
        assert_eq!((ctx.left, f64::from_bits(ctx.right).is_nan()), (3, true));
    }

    #[test]
    fn steps_and_depth_are_counted_mark_by_mark_and_call_by_call() {
        let l = load(
            "\
pull down(n) {
  if n <= 0 { snuff 0 }
  snuff down(n - 1) + 1
}
pull spin(n) {
  roll i = 0
  while i < n {
    i = i + 1
  }
  snuff i
}
pull outer(n) {
  roll s = 0
  for i in 0..n {
    s = s + spin(3)
  }
  snuff s
}
exhale down(3), spin(3), outer(2)
",
        );
        // `down(3)`: four calls of two statements each.
        let (status, slot, ctx) = l.call("down", &[3], 1000, 100);
        assert_eq!((status, slot), (0, 3));
        let took = 1000 - ctx.steps_left;
        assert_eq!(took, 8);
        // With exactly that many it returns, with none left; with one
        // fewer it leaves at a mark, one step overspent.
        let (status, _, ctx) = l.call("down", &[3], took, 100);
        assert_eq!((status, ctx.steps_left), (0, 0));
        let (status, _, ctx) = l.call("down", &[3], took - 1, 100);
        assert_eq!((status, ctx.steps_left), (Exit::Budget as u32, -1));
        assert_eq!(l.tables.sites[ctx.site as usize].kind, Exit::Budget);
        // Three calls nest under `down(3)`: three are enough, two are
        // not, and the depth is given back when it returns.
        let (status, _, ctx) = l.call("down", &[3], 1000, 3);
        assert_eq!((status, ctx.depth_left), (0, 3));
        let (status, _, ctx) = l.call("down", &[3], 1000, 2);
        let site = &l.tables.sites[ctx.site as usize];
        assert_eq!(
            (status, site.kind, site.line),
            (Exit::Depth as u32, Exit::Depth, 3)
        );
        // A budget spent in a loop says so at its site; spent in a
        // function called from a loop, the caller's flag says so.
        let (status, _, ctx) = l.call("spin", &[1_000_000], 50, 10);
        let site = &l.tables.sites[ctx.site as usize];
        assert_eq!(
            (status, site.in_loop, ctx.flags),
            (Exit::Budget as u32, true, 0)
        );
        // `outer`: two statements, then a turn's mark and its statement,
        // then `spin(3)` takes its first statement's step and is out.
        let (status, _, ctx) = l.call("outer", &[2], 4, 10);
        let site = &l.tables.sites[ctx.site as usize];
        assert_eq!(status, Exit::Budget as u32);
        assert_eq!((site.line, site.in_loop, ctx.flags & 1), (6, false, 1));
    }

    /// One figure, said and not held: `fib(32)` as native code.
    /// `cargo test --release -p cig-codegen -- --ignored --nocapture`.
    #[test]
    #[ignore = "a timing, for a person to read"]
    fn fib_32_timed() {
        let l = load(SEVERAL);
        let mut times: Vec<std::time::Duration> = (0..20)
            .map(|_| {
                let t = std::time::Instant::now();
                assert_eq!(l.int("fib", &[32]), Ok(2_178_309));
                t.elapsed()
            })
            .collect();
        times.sort();
        eprintln!("fib(32) as native code: {:?} (the median of 20)", times[10]);
    }
}

/// FNV-1a, 64 bits: a fingerprint of bytes, for a test to hold.
fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// A program's native code is held to a fresh compile of its script,
/// which may run on another system than the build did: the bytes must be
/// the same on every host. This holds them to what one host made; every
/// system CI tests on runs it.
///
/// It fails, rightly, when the generator, the IR it reads or Cranelift's
/// version changes what is generated: read the change, then put the
/// numbers the failure prints here.
#[test]
fn the_bytes_are_the_same_on_every_host() {
    let made: Vec<(Arch, usize, u64)> = [Arch::X86_64, Arch::Aarch64]
        .into_iter()
        .map(|arch| {
            let bytes = generate(SEVERAL, arch)
                .1
                .section
                .to_bytes()
                .expect("a section");
            (arch, bytes.len(), fingerprint(&bytes))
        })
        .collect();
    // Updated for interface 2: the footer carries new fields (body
    // count, resume count, rule) and the version and interface bytes
    // moved from 1 to 2. `Rule::None` keeps the code and the entry
    // and site tables the same byte for byte; only the footer differs.
    let held = [
        (Arch::X86_64, 3384, 10_074_670_610_979_580_805),
        (Arch::Aarch64, 3216, 17_860_211_359_695_669_158),
    ];
    assert_eq!(made, held, "what this host generates (length, fingerprint)");
}
