// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! The real-world corpus (#39): the cig-orchestrator scripts, as cases
//! the differential runner can run safely. Every compile holds the whole
//! program to parity between the interpreter's check and the compiler's,
//! and the cases below hold the live program against `cig run` through
//! entry points that write no files and read no network: orchestrate.cig's
//! `--version`, `help` and `toolchains --markdown`, run.cig and mark.cig
//! in `--dry-run` so their burns stay as plans.
//!
//! The scripts are checked in under tests/corpus/cig-orchestrator/. Each
//! is the same owner's Apache-2.0, SPDX header kept, and embedded in the
//! test binary with `include_str!` so the suite needs nothing else on disk.

use std::collections::BTreeMap;

use super::{Case, Run, Step};

const ORCHESTRATE: &str = include_str!("../corpus/cig-orchestrator/orchestrate.cig");
const RUN_TESTS: &str = include_str!("../corpus/cig-orchestrator/run.cig");
const MARK: &str = include_str!("../corpus/cig-orchestrator/mark.cig");

fn words(w: &[&str]) -> Vec<String> {
    w.iter().map(|s| s.to_string()).collect()
}

/// cig-orchestrator cases, ready for `run_case`. Each `inline` carries the
/// script's text, so `src.join(&case.script)` is never read; `script` is
/// a logical name the runner prints and uses for the compiled file.
pub fn cases() -> Vec<Case> {
    vec![
        // orchestrate.cig: three side-effect-free entry points, each a
        // live run. One compile; three runs per side.
        Case {
            script: "tests/corpus/cig-orchestrator/orchestrate.cig".to_string(),
            inline: Some(ORCHESTRATE.to_string()),
            files: BTreeMap::new(),
            setup: Vec::new(),
            needs: Vec::new(),
            env: BTreeMap::new(),
            runs: vec![
                Run {
                    steps: vec![Step {
                        args: vec!["--version".to_string()],
                        light: None,
                    }],
                    modes: words(&["run"]),
                },
                Run {
                    steps: vec![Step {
                        args: vec!["help".to_string()],
                        light: None,
                    }],
                    modes: words(&["run"]),
                },
                Run {
                    steps: vec![Step {
                        args: vec!["toolchains".to_string(), "--markdown".to_string()],
                        light: None,
                    }],
                    modes: words(&["run"]),
                },
            ],
            expect_stdout: None,
        },
        // tests/run.cig (cig-orchestrator's test suite, 4,592 lines): the
        // whole program compiles, and a dry run's plan is compared. No
        // real run: it would launch child cigs.
        Case {
            script: "tests/corpus/cig-orchestrator/run.cig".to_string(),
            inline: Some(RUN_TESTS.to_string()),
            files: BTreeMap::new(),
            setup: Vec::new(),
            needs: Vec::new(),
            env: BTreeMap::new(),
            runs: vec![Run {
                steps: vec![Step::default()],
                modes: words(&["dry-run"]),
            }],
            expect_stdout: None,
        },
        // mark.cig: ten lines, a burn with a compensation. Dry-run
        // compares the plan: the burn body is not executed on either
        // side.
        Case {
            script: "tests/corpus/cig-orchestrator/mark.cig".to_string(),
            inline: Some(MARK.to_string()),
            files: BTreeMap::new(),
            setup: Vec::new(),
            needs: Vec::new(),
            env: BTreeMap::new(),
            runs: vec![Run {
                steps: vec![Step::default()],
                modes: words(&["dry-run"]),
            }],
            expect_stdout: None,
        },
    ]
}
