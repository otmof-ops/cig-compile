// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! The typed corpus: small programs whose arithmetic lives inside
//! functions that take and return only ints, floats, bools and null.
//! These are the functions stage `c` runs as native code
//! (`docs/decisions/0005-what-goes-native-first.md`); the suite holds
//! every stage it runs to the tree-walker on them, so a stage that
//! compiles them is held to the same programs from its first day. Each
//! thing native code does (decision 0005, point 4, and what point 7
//! leaves to the bytecode) has at least one program here.
//!
//! The scripts are under `tests/corpus/typed/`, embedded in the test
//! binary with `include_str!` so the suite needs nothing else on disk.
//!
//! `DIFF_TYPED_RANDOM=<seed>:<count>` adds `count` random typed
//! programs from seed `seed` ([`gen::typed_program`]), shrunk like
//! `DIFF_RANDOM`'s when one differs; the default run compares only the
//! hand-written corpus.

use std::collections::BTreeMap;

use super::{Case, Run, Step};

/// The corpus: one entry a `.cig` file under `tests/corpus/typed/`.
/// The order is the order the suite runs them in: the ones that compare
/// exact output first, then the ones that leave uncaught (so a
/// difference on an easy case is printed before the budget sweep for a
/// trickier one).
const CORPUS: &[(&str, &str)] = &[
    (
        "recursion-fib",
        include_str!("../corpus/typed/recursion-fib.cig"),
    ),
    (
        "recursion-mutual",
        include_str!("../corpus/typed/recursion-mutual.cig"),
    ),
    (
        "fn-eight-params",
        include_str!("../corpus/typed/fn-eight-params.cig"),
    ),
    (
        "fn-returns-null",
        include_str!("../corpus/typed/fn-returns-null.cig"),
    ),
    (
        "fn-returns-bool",
        include_str!("../corpus/typed/fn-returns-bool.cig"),
    ),
    ("loop-while", include_str!("../corpus/typed/loop-while.cig")),
    (
        "loop-for-three-ranges",
        include_str!("../corpus/typed/loop-for-three-ranges.cig"),
    ),
    (
        "loop-nested",
        include_str!("../corpus/typed/loop-nested.cig"),
    ),
    (
        "range-value-empty",
        include_str!("../corpus/typed/range-value-empty.cig"),
    ),
    (
        "list-elements",
        include_str!("../corpus/typed/list-elements.cig"),
    ),
    (
        "loop-break-continue",
        include_str!("../corpus/typed/loop-break-continue.cig"),
    ),
    (
        "loop-body-call",
        include_str!("../corpus/typed/loop-body-call.cig"),
    ),
    (
        "int-plus-not-overflow",
        include_str!("../corpus/typed/int-plus-not-overflow.cig"),
    ),
    (
        "int-overflow-caught",
        include_str!("../corpus/typed/int-overflow-caught.cig"),
    ),
    (
        "int-unary-neg-min",
        include_str!("../corpus/typed/int-unary-neg-min.cig"),
    ),
    (
        "int-mod-edges",
        include_str!("../corpus/typed/int-mod-edges.cig"),
    ),
    (
        "int-comparisons-extremes",
        include_str!("../corpus/typed/int-comparisons-extremes.cig"),
    ),
    (
        "int-div-exact-inexact",
        include_str!("../corpus/typed/int-div-exact-inexact.cig"),
    ),
    ("float-ops", include_str!("../corpus/typed/float-ops.cig")),
    (
        "float-div-zero",
        include_str!("../corpus/typed/float-div-zero.cig"),
    ),
    (
        "float-inf-and-nan-cond",
        include_str!("../corpus/typed/float-inf-and-nan-cond.cig"),
    ),
    (
        "float-nan-order",
        include_str!("../corpus/typed/float-nan-order.cig"),
    ),
    (
        "float-mod-is-skipped",
        include_str!("../corpus/typed/float-mod-is-skipped.cig"),
    ),
    ("mixed-ops", include_str!("../corpus/typed/mixed-ops.cig")),
    ("mixed-eq", include_str!("../corpus/typed/mixed-eq.cig")),
    (
        "truth-all-types",
        include_str!("../corpus/typed/truth-all-types.cig"),
    ),
    ("call-depth", include_str!("../corpus/typed/call-depth.cig")),
    (
        "range-too-wide",
        include_str!("../corpus/typed/range-too-wide.cig"),
    ),
    (
        "fn-polymorphic-rows",
        include_str!("../corpus/typed/fn-polymorphic-rows.cig"),
    ),
    (
        "fn-reassign-type",
        include_str!("../corpus/typed/fn-reassign-type.cig"),
    ),
    (
        "fn-builtin-one-branch",
        include_str!("../corpus/typed/fn-builtin-one-branch.cig"),
    ),
    (
        "step-budget-inside-fn",
        include_str!("../corpus/typed/step-budget-inside-fn.cig"),
    ),
    (
        "step-budget-nested",
        include_str!("../corpus/typed/step-budget-nested.cig"),
    ),
    (
        "step-budget-outside-loop",
        include_str!("../corpus/typed/step-budget-outside-loop.cig"),
    ),
    (
        "int-overflow-uncaught",
        include_str!("../corpus/typed/int-overflow-uncaught.cig"),
    ),
    (
        "float-nan-order-uncaught",
        include_str!("../corpus/typed/float-nan-order-uncaught.cig"),
    ),
    (
        "call-depth-uncaught",
        include_str!("../corpus/typed/call-depth-uncaught.cig"),
    ),
    (
        "range-too-wide-uncaught",
        include_str!("../corpus/typed/range-too-wide-uncaught.cig"),
    ),
];

fn words(w: &[&str]) -> Vec<String> {
    w.iter().map(|s| s.to_string()).collect()
}

/// A corpus `Case`: one real run, compared under `run`, `run --json`
/// and `run --plain`. `CIG_MAX_STEPS` is raised to match the other
/// corpora (the budget sweep lowers it as it goes).
fn corpus_case(name: &str, text: &str) -> Case {
    let mut env = BTreeMap::new();
    env.insert("CIG_MAX_STEPS".to_string(), "1000000".to_string());
    Case {
        script: format!("tests/corpus/typed/{name}.cig"),
        inline: Some(text.to_string()),
        files: BTreeMap::new(),
        setup: Vec::new(),
        needs: Vec::new(),
        env,
        runs: vec![Run {
            steps: vec![Step::default()],
            modes: words(&["run", "run json", "run plain"]),
        }],
        expect_stdout: None,
    }
}

/// Every case in the hand-written typed corpus, ready for `run_case`.
pub fn cases() -> Vec<Case> {
    CORPUS
        .iter()
        .map(|(name, text)| corpus_case(name, text))
        .collect()
}

/// A random typed program as a case: one real run, from the generator's
/// typed mode. Named like `typed-random/<seed>` so the shrinker's loop
/// can spot the row it made.
pub fn random_case(seed: u64, stmts: &[String]) -> Case {
    Case {
        script: format!("typed-random/{seed}"),
        inline: Some(format!("{}\n", stmts.join("\n"))),
        files: BTreeMap::new(),
        setup: Vec::new(),
        needs: Vec::new(),
        env: BTreeMap::new(),
        runs: vec![Run {
            steps: vec![Step::default()],
            modes: words(&["run"]),
        }],
        expect_stdout: None,
    }
}
