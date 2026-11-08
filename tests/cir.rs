// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! CIR (#3): lowering, the verifier, the passes, the text form.

use cigcompile::cir::ir::{Kind, VarId};
use cigcompile::cir::pass::{self, broken, run_passes, Pass};
use cigcompile::cir::verify::{errors, verify, Severity};
use cigcompile::cir::{emit_with, lower_source, print, LowerError};

const DEMO: &str = "pull greet(name) {
  fs.write_text(\"out.txt\", \"hi \" + name)
}
roll n = 0
burn (\"demo\") {
  greet(\"a\")
  n += 1
}
exhale n
";

#[test]
fn the_demo_lowers_and_verifies_and_reads_as_the_spec_shows() {
    let mut m = lower_source(DEMO, "demo.cig").unwrap();
    let found = verify(&m);
    assert!(errors(&found).is_empty(), "{found:#?}");
    run_passes(&mut m, &pass::default_passes()).unwrap();
    let text = print::print(&m);
    for want in [
        "cir 1 \"demo.cig\"",
        "pull requires_burn",
        "burn.op write fs.write_text(",
        "burn.begin r1 normal label=",
        "call.user @1(",
        "} burn.end",
        "exhale %",
    ] {
        assert!(text.contains(want), "missing {want:?} in:\n{text}");
    }
    // Every line of an instruction carries its span.
    assert!(
        text.lines()
            .filter(|l| l.contains(" = ") || l.contains("burn.op"))
            .all(|l| l.contains(" @")),
        "{text}"
    );
}

/// The text form, byte for byte: tests/cir/demo.cir. After a deliberate
/// change, CIR_UPDATE_GOLDEN=1 writes it again.
#[test]
fn the_demo_matches_its_golden_text() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cir");
    // A checkout that ignored .gitattributes still compares by lines.
    let lf = |t: String| t.replace("\r\n", "\n");
    let src = lf(std::fs::read_to_string(dir.join("demo.cig")).unwrap());
    let (text, _) = cigcompile::cir::emit(&src, "demo.cig").unwrap();
    let golden = dir.join("demo.cir");
    if std::env::var_os("CIR_UPDATE_GOLDEN").is_some() {
        std::fs::write(&golden, &text).unwrap();
    }
    assert_eq!(
        text,
        lf(std::fs::read_to_string(&golden).unwrap_or_default()),
        "CIR_UPDATE_GOLDEN=1 writes it again"
    );
}

#[test]
fn an_effect_hoisted_out_of_its_burn_is_caught_v4() {
    let mut m = lower_source(
        DEMO.replace("greet(\"a\")", "fs.write_text(\"x\", \"y\")")
            .as_str(),
        "demo.cig",
    )
    .unwrap();
    let e = run_passes(&mut m, &[&broken::HoistBurnOp as &dyn Pass]).unwrap_err();
    assert_eq!(e.pass, "hoist-burn-op");
    let v4: Vec<_> = e.errors.iter().filter(|x| x.code == "V4").collect();
    assert_eq!(v4.len(), 1, "{:#?}", e.errors);
    assert!(v4[0].message.contains("fs.write_text"), "{}", v4[0]);
    assert_eq!(
        v4[0].span.line, 6,
        "the error points at the effect's own line"
    );
}

#[test]
fn effects_swapped_in_their_region_are_caught_only_by_the_transition_check_t2() {
    let src = "burn {\n  fs.write_text(\"a\", \"1\")\n  fs.write_text(\"b\", \"2\")\n}\n";
    let mut m = lower_source(src, "s.cig").unwrap();
    let e =
        run_passes(&mut m, &[&broken::SwapBurnOps as &dyn Pass]).expect_err("the swap is caught");
    assert!(e.errors.iter().any(|x| x.code == "T2"), "{:#?}", e.errors);
    assert!(
        !e.errors.iter().any(|x| x.code == "V4"),
        "a swap is structurally fine: {:#?}",
        e.errors
    );
}

#[test]
fn an_effect_dropped_by_a_pass_is_caught_t3() {
    let src = "burn {\n  fs.mkdir(\"d\")\n}\n";
    let mut m = lower_source(src, "s.cig").unwrap();
    let e = run_passes(&mut m, &[&broken::DropBurnOp as &dyn Pass]).unwrap_err();
    assert!(e.errors.iter().any(|x| x.code == "T3"), "{:#?}", e.errors);
}

#[test]
fn constants_fold_where_the_interpreter_cannot_fail_and_nowhere_else() {
    let src =
        "stick a = 2 + 3 * 4\nstick b = 1 / 0\nstick c = 9223372036854775807 + 1\nexhale a, b, c\n";
    let mut m = lower_source(src, "f.cig").unwrap();
    run_passes(&mut m, &pass::default_passes()).unwrap();
    let text = print::print(&m);
    assert!(text.contains("const 14"), "{text}");
    assert!(text.contains("bin / "), "division is never folded: {text}");
    assert!(
        text.contains("bin + "),
        "an overflowing add is left to the runtime (E503): {text}"
    );
}

#[test]
fn closures_capture_and_effects_in_lambdas_are_theirs() {
    let src = "roll total = 0\nstick add = pack(x) => total + x\nburn {\n  [1, 2].each(pack(p) => fs.write_text(\"f${p}\", \"x\"))\n}\nexhale add(1)\n";
    let mut m = lower_source(src, "c.cig").unwrap();
    let found = run_passes(&mut m, &pass::default_passes()).unwrap();
    assert!(errors(&found).is_empty());
    let text = print::print(&m);
    assert!(text.contains("captures($0:total!)"), "{text}");
    // The lambda's effect is legal only when it runs in a burn: the lambda
    // requires one.
    assert!(text.contains("lambda requires_burn"), "{text}");
}

#[test]
fn an_effectful_pull_called_outside_a_burn_is_a_warning_not_an_error() {
    let src = "pull w() {\n  fs.mkdir(\"d\")\n}\nw()\n";
    let mut m = lower_source(src, "w.cig").unwrap();
    let found = run_passes(&mut m, &pass::default_passes()).unwrap();
    assert!(
        found
            .iter()
            .any(|w| w.code == "V6" && w.severity == Severity::Warn),
        "{found:#?}"
    );
}

#[test]
fn a_script_that_does_not_check_is_not_lowered() {
    assert!(matches!(
        lower_source("exhale nope\n", "x.cig"),
        Err(LowerError::Diagnostic(_))
    ));
    assert!(matches!(
        lower_source("fs.rm(\"x\")\n", "x.cig"),
        Err(LowerError::Diagnostic(_))
    ));
}

#[test]
fn an_effect_moved_out_of_its_if_is_caught_only_by_the_places_t1() {
    let src = "roll c = true\nburn {\n  if c {\n    fs.rm(\"a\")\n  }\n}\n";
    let mut m = lower_source(src, "i.cig").unwrap();
    let e =
        run_passes(&mut m, &[&broken::HoistOutOfIf as &dyn Pass]).expect_err("the move is caught");
    assert_eq!(e.pass, "hoist-out-of-if");
    assert!(!e.errors.is_empty());
    // The same order, the same region, every value defined before use:
    // only the effect's place says it is no longer conditional.
    assert!(e.errors.iter().all(|x| x.code == "T1"), "{:#?}", e.errors);
    assert!(
        e.errors.iter().any(|x| x.message.contains("if.then")),
        "{:#?}",
        e.errors
    );
}

#[test]
fn a_compensation_that_reaches_outside_its_state_is_caught_v8() {
    let src = "roll n = 1\nburn (s) {\n  s.n = n\n  fs.mkdir(\"d\")\n} unburn {\n  exhale s.n\n}\n";
    let mut m = lower_source(src, "u.cig").unwrap();
    let found = verify(&m);
    assert!(errors(&found).is_empty(), "{found:#?}");
    // Read `n` in the compensation instead of the state: the checker's
    // E754, which a pass must not bring back.
    let n = VarId(m.vars.iter().position(|v| v.name == "n").unwrap() as u32);
    let comp = m.funcs[0]
        .body
        .instrs
        .iter_mut()
        .find_map(|i| match &mut i.kind {
            Kind::Burn(b) => b.compensate.as_mut(),
            _ => None,
        })
        .unwrap();
    let load = comp
        .instrs
        .iter_mut()
        .find(|i| matches!(i.kind, Kind::Load(_)))
        .unwrap();
    load.kind = Kind::Load(n);
    let found = verify(&m);
    assert!(
        found
            .iter()
            .any(|e| e.code == "V8" && e.message.contains(":n, which")),
        "{found:#?}"
    );
}

#[test]
fn a_capture_the_flags_do_not_record_is_caught_v9() {
    let src = "roll total = 0\nstick add = pack(x) => total + x\nexhale add(1)\n";
    let mut m = lower_source(src, "c.cig").unwrap();
    let total = m.vars.iter().position(|v| v.name == "total").unwrap();
    assert!(m.vars[total].captured);
    m.vars[total].captured = false;
    let found = verify(&m);
    assert!(
        found
            .iter()
            .any(|e| e.code == "V9" && e.message.contains("total")),
        "{found:#?}"
    );
}

#[test]
fn a_safe_member_is_null_for_a_missing_key_too() {
    // eval.rs member(): `?.` gives null for a null subject and for a map
    // without the key. One instruction carries both; a short.safe around
    // it would leave the second to a back end to forget.
    let src = "roll cfg = {a: 1}\nexhale cfg?.missing\n";
    let mut m = lower_source(src, "s.cig").unwrap();
    run_passes(&mut m, &pass::default_passes()).unwrap();
    let text = print::print(&m);
    assert!(text.contains("?.missing"), "{text}");
    assert!(!text.contains("short.safe"), "{text}");
}

#[test]
fn a_bare_name_piped_into_resolves_as_a_call_would() {
    let src = "pull dbl(x) {\n  snuff x * 2\n}\nburn {\n  \"x\" >> fs.rm\n}\nexhale 3 >> dbl\n";
    let mut m = lower_source(src, "p.cig").unwrap();
    run_passes(&mut m, &pass::default_passes()).unwrap();
    let text = print::print(&m);
    assert!(
        text.contains("burn.op rm fs.rm("),
        "the effect stays an effect: {text}"
    );
    assert!(text.contains("call.user @1("), "{text}");
    assert!(!text.contains("call.value"), "{text}");
}

/// Golden: `copy-prop` alone over `stick a = 1; stick b = a; exhale b`
/// turns both loads into direct uses of the first constant.
#[test]
fn copy_prop_matches_its_golden_text() {
    golden_for_pass("copy-prop", "stick-chain", &[&pass::CopyProp as &dyn Pass]);
}

#[test]
fn copy_prop_propagates_through_chains_and_leaves_mutables_alone() {
    // A chain of immutable copies collapses to the first value.
    let src = "stick a = 1\nstick b = a\nexhale b\n";
    let mut m = lower_source(src, "cp.cig").unwrap();
    run_passes(&mut m, &[&pass::CopyProp as &dyn Pass]).unwrap();
    let text = print::print(&m);
    assert!(!text.contains("load "), "copies removed: {text}");
    assert_eq!(
        text.matches("const 1").count(),
        1,
        "and only the one constant is the source: {text}"
    );
    // A mutable slot is never propagated through: between a declaration
    // and a use another store may land, and the load would read it.
    let src = "roll a = 1\na = 2\nexhale a\n";
    let mut m = lower_source(src, "cp.cig").unwrap();
    run_passes(&mut m, &[&pass::CopyProp as &dyn Pass]).unwrap();
    let text = print::print(&m);
    assert!(text.contains("load $0:a!"), "mutable load kept: {text}");
}

/// Negative: a value `copy-prop` propagates must be visible where it
/// lands. A broken twin that lifts a value out of an `if.then` into the
/// enclosing block is caught by V1 (`%r used where it is not defined`).
#[test]
fn a_copy_propagation_out_of_scope_is_caught_v1() {
    let src = "stick a = 1\nif true {\n  stick b = 2\n  exhale b\n}\nexhale a\n";
    let mut m = lower_source(src, "neg.cig").unwrap();
    let found = verify(&m);
    assert!(errors(&found).is_empty(), "the input itself verifies");
    let e = run_passes(&mut m, &[&broken::PropOutOfNestedScope as &dyn Pass])
        .expect_err("the propagation is caught");
    assert_eq!(e.pass, "prop-out-of-nested-scope");
    assert!(e.errors.iter().any(|x| x.code == "V1"), "{:#?}", e.errors);
}

/// Golden: `sccp` alone over `roll n; if true { n = 1 } else { n = 2 }`
/// empties the dead `else` branch; the `if` and its cond stay.
#[test]
fn sccp_matches_its_golden_text() {
    golden_for_pass("sccp", "if-true", &[&pass::Sccp as &dyn Pass]);
}

#[test]
fn sccp_empties_only_the_dead_side_and_only_when_it_carries_no_effect() {
    // `if true` with a dead `else` that has no effect: emptied.
    let src = "roll n = 0\nif true {\n  n = 1\n} else {\n  n = 2\n}\nexhale n\n";
    let mut m = lower_source(src, "s.cig").unwrap();
    let before = print::print(&m);
    run_passes(&mut m, &[&pass::Sccp as &dyn Pass]).unwrap();
    let after = print::print(&m);
    assert_ne!(before, after, "the pass ran");
    assert!(
        after.contains("const true") && after.contains("if %"),
        "the cond and its branch stay: {after}"
    );
    // The `else` is now empty, so no `const 2` from `n = 2` survives.
    assert!(
        !after.contains("const 2"),
        "the dead side is empty: {after}"
    );
    // `if false` with a dead `then` that has a `burn.op` is left alone:
    // emptying it would drop an effect, and T3 would catch that.
    let src = "burn {\n  if false {\n    fs.rm(\"x\")\n  }\n}\n";
    let mut m = lower_source(src, "s.cig").unwrap();
    run_passes(&mut m, &[&pass::Sccp as &dyn Pass]).unwrap();
    let text = print::print(&m);
    assert!(
        text.contains("burn.op rm fs.rm("),
        "the dead effect is kept: {text}"
    );
}

/// Negative: `sccp` never drops a branch whose body has a `burn.op`. A
/// broken twin that clears an `if.then` holding one is caught by T3.
#[test]
fn an_sccp_that_drops_a_branch_with_an_effect_is_caught_t3() {
    let src = "burn {\n  if true {\n    fs.rm(\"gone\")\n  }\n}\n";
    let mut m = lower_source(src, "neg.cig").unwrap();
    let found = verify(&m);
    assert!(errors(&found).is_empty(), "the input itself verifies");
    let e = run_passes(&mut m, &[&broken::DropBranchWithEffect as &dyn Pass])
        .expect_err("the drop is caught");
    assert_eq!(e.pass, "drop-branch-with-effect");
    assert!(e.errors.iter().any(|x| x.code == "T3"), "{:#?}", e.errors);
}

/// `-O1` runs `fold-consts, copy-prop, fold-consts, sccp, dead-pure`, so
/// `stick a = 1; if a == 1 { exhale 1 } else { exhale 2 }` has its
/// `else` emptied and its compare gone.
#[test]
fn default_passes_fold_copy_through_sccp() {
    let src = "stick a = 1\nif a == 1 {\n  exhale 1\n} else {\n  exhale 2\n}\n";
    let mut m = lower_source(src, "o1.cig").unwrap();
    run_passes(&mut m, &pass::default_passes()).unwrap();
    let text = print::print(&m);
    assert!(
        text.contains("const true"),
        "fold-consts after copy-prop sees the compare: {text}"
    );
    assert!(!text.contains("bin == "), "the compare is gone: {text}");
    assert!(!text.contains("const 2"), "the else is empty: {text}");
    assert!(
        text.contains("const 1"),
        "the then's `exhale 1` stays: {text}"
    );
}

/// The CIR text each pass prints matches its golden file; after a
/// deliberate change, `CIR_UPDATE_GOLDEN=1` writes it again.
fn golden_for_pass(sub: &str, name: &str, passes: &[&'static dyn Pass]) {
    let lf = |t: String| t.replace("\r\n", "\n");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/cir")
        .join(sub);
    let cig = dir.join(format!("{name}.cig"));
    let src =
        lf(std::fs::read_to_string(&cig).unwrap_or_else(|e| panic!("{}: {e}", cig.display())));
    let file = format!("{name}.cig");
    let e = emit_with(&src, &file, passes, false, false).unwrap();
    let golden = dir.join(format!("{name}.cir"));
    if std::env::var_os("CIR_UPDATE_GOLDEN").is_some() {
        std::fs::write(&golden, &e.text).unwrap();
    }
    assert_eq!(
        e.text,
        lf(std::fs::read_to_string(&golden).unwrap_or_default()),
        "CIR_UPDATE_GOLDEN=1 writes it again"
    );
}

/// Every script in a CigScript checkout of the pinned version lowers and
/// verifies through the default passes; the corpus scripts the checker
/// refuses are refused here too.
#[test]
fn every_script_cigscript_carries_lowers_and_verifies() {
    let Some(src) = std::env::var_os("CIGSCRIPT_SRC").map(std::path::PathBuf::from) else {
        eprintln!("CIGSCRIPT_SRC is not set; skipped");
        return;
    };
    let mut lowered = 0;
    let mut refused = Vec::new();
    for dir in ["examples", "tests/scripts", "tests/corpus"] {
        let mut files: Vec<_> = std::fs::read_dir(src.join(dir))
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "cig"))
            .collect();
        files.sort();
        for f in files {
            let text = std::fs::read_to_string(&f).unwrap();
            let name = format!("{dir}/{}", f.file_name().unwrap().to_string_lossy());
            match lower_source(&text, &name) {
                Ok(mut m) => {
                    if let Err(e) = run_passes(&mut m, &pass::default_passes()) {
                        panic!("{name}: {} broke a rule:\n{:#?}", e.pass, e.errors);
                    }
                    lowered += 1;
                }
                Err(LowerError::Diagnostic(_)) => refused.push(name),
                Err(LowerError::Internal(e)) => panic!("{name}: {e}"),
            }
        }
    }
    refused.sort();
    assert_eq!(
        refused,
        [
            "tests/corpus/e104-too-big.cig",
            "tests/corpus/e202-unclosed.cig",
            "tests/corpus/e301-typo.cig",
            "tests/corpus/e303-outside-burn.cig",
        ],
        "exactly the corpus scripts the checker refuses"
    );
    assert!(lowered >= 24, "only {lowered} scripts lowered");
}
