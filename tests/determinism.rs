// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! Determinism checks (#39): floats print what IEEE-754 says, and maps keep
//! insertion order through every operation. Each case has an expected output
//! written down from the arithmetic (not from running cig), and both the
//! interpreter and the program made from the same source are compared byte
//! for byte to it. A mismatch on cig's side is left as a finding, not
//! silently papered over by matching cig's actual output.
//!
//! Each #[test] compiles one combined script (so the 142 MB runtime is
//! copied once, not per case) and splits its output by a sentinel line, one
//! case per block. A case listed in `KNOWN_FLOAT_FINDINGS` or
//! `KNOWN_MAP_FINDINGS` is still run, but its mismatch is reported as a
//! finding rather than failing the test: the ignore granularity the issue
//! asks for, without one test per case and the 142 MB build each would
//! take.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const CIG: &str = env!("CARGO_BIN_EXE_cig-runtime");
const CC: &str = env!("CARGO_BIN_EXE_cigcompile");

/// A case: a name, the lines its program runs, and what every line of its
/// output must be. The name identifies the case in a `known findings` list.
struct Case {
    name: &'static str,
    source: &'static str,
    expect: &'static str,
}

/// Float cases, each with the output IEEE-754 says every side must print.
/// These are what cig does today; a case cig stops matching becomes a
/// finding (`KNOWN_FLOAT_FINDINGS` below) and the mismatch is left in
/// place, not rewritten to match cig.
fn float_cases() -> Vec<Case> {
    vec![
        Case {
            name: "0.1+0.2 is 0.30000000000000004",
            source: "exhale 0.1 + 0.2",
            expect: "0.30000000000000004",
        },
        Case {
            name: "0.1+0.2 is not 0.3",
            source: "exhale 0.1 + 0.2 == 0.3",
            expect: "false",
        },
        Case {
            name: "negative zero prints with its sign",
            source: "exhale -0.0",
            expect: "-0.0",
        },
        Case {
            name: "negative zero equals positive zero",
            source: "exhale 0.0 == -0.0",
            expect: "true",
        },
        Case {
            name: "str(-0.0) keeps the sign",
            source: "exhale str(-0.0)",
            expect: "-0.0",
        },
        Case {
            name: "str(0.0) has no sign",
            source: "exhale str(0.0)",
            expect: "0.0",
        },
        Case {
            name: "1.0 * 0.0 is +0.0",
            source: "exhale 1.0 * 0.0",
            expect: "0.0",
        },
        Case {
            name: "-1.0 * 0.0 is -0.0 (IEEE sign of zero)",
            source: "exhale -1.0 * 0.0",
            expect: "-0.0",
        },
        Case {
            name: "overflow of a double is inf",
            source: "exhale 1e308 * 10",
            expect: "inf",
        },
        Case {
            name: "overflow of a negative double is -inf",
            source: "exhale -(1e308 * 10)",
            expect: "-inf",
        },
        Case {
            name: "inf - inf is nan",
            source: "stick inf = 1e308 * 10\nexhale inf - inf",
            expect: "nan",
        },
        Case {
            name: "inf + 1.0 stays inf",
            source: "stick inf = 1e308 * 10\nexhale inf + 1.0",
            expect: "inf",
        },
        Case {
            name: "inf * 0.0 is nan",
            source: "stick inf = 1e308 * 10\nexhale inf * 0.0",
            expect: "nan",
        },
        Case {
            name: "1.0 / inf is +0.0",
            source: "stick inf = 1e308 * 10\nexhale 1.0 / inf",
            expect: "0.0",
        },
        Case {
            name: "-1.0 / inf is -0.0",
            source: "stick inf = 1e308 * 10\nexhale -1.0 / inf",
            expect: "-0.0",
        },
        Case {
            name: "str(inf) is inf",
            source: "exhale str(1e308 * 10)",
            expect: "inf",
        },
        Case {
            name: "str(-inf) is -inf",
            source: "exhale str(-(1e308 * 10))",
            expect: "-inf",
        },
        Case {
            name: "str(nan) is nan",
            source: "stick inf = 1e308 * 10\nexhale str(inf - inf)",
            expect: "nan",
        },
        Case {
            name: "int / int is a float",
            source: "exhale 7 / 2",
            expect: "3.5",
        },
        Case {
            name: "-7 / 2 is -3.5 (not floored)",
            source: "exhale -7 / 2",
            expect: "-3.5",
        },
        Case {
            name: "-7 % 3 is -1 (sign of dividend)",
            source: "exhale -7 % 3",
            expect: "-1",
        },
        Case {
            name: "7 % -3 is 1 (sign of dividend)",
            source: "exhale 7 % -3",
            expect: "1",
        },
        Case {
            name: "-7 % -3 is -1 (sign of dividend)",
            source: "exhale -7 % -3",
            expect: "-1",
        },
        Case {
            name: "int(3.7) truncates toward zero",
            source: "exhale int(3.7)",
            expect: "3",
        },
        Case {
            name: "int(-3.7) truncates toward zero, not floors",
            source: "exhale int(-3.7)",
            expect: "-3",
        },
        Case {
            name: "int(2.5) truncates toward zero",
            source: "exhale int(2.5)",
            expect: "2",
        },
        Case {
            name: "int(-2.5) truncates toward zero",
            source: "exhale int(-2.5)",
            expect: "-2",
        },
        Case {
            name: "float(2^53) is exact",
            source: "exhale float(9007199254740992)",
            expect: "9007199254740992.0",
        },
        Case {
            name: "float(2^53 + 1) rounds down (precision loss)",
            source: "exhale float(9007199254740993)",
            expect: "9007199254740992.0",
        },
        Case {
            name: "the integer 2^53 + 1 is exact",
            source: "exhale 9007199254740993",
            expect: "9007199254740993",
        },
        Case {
            name: "the greatest finite double prints its bits",
            source: "exhale 1.7976931348623157e308",
            expect: "1.7976931348623157e308",
        },
        Case {
            name: "the greatest finite double, doubled, overflows",
            source: "exhale 1.7976931348623157e308 * 2",
            expect: "inf",
        },
        Case {
            name: "str(1.0) keeps the decimal point",
            source: "exhale str(1.0)",
            expect: "1.0",
        },
        Case {
            name: "str(0.1 + 0.2) and the raw value agree",
            source: "exhale str(0.1 + 0.2)",
            expect: "0.30000000000000004",
        },
    ]
}

/// A case cig does not match today, with the finding. The case is still
/// run, but its mismatch is reported as a finding rather than failing the
/// test: the issue asks a finding to be left in place and marked, not
/// rewritten to cig's actual output.
const KNOWN_FLOAT_FINDINGS: &[(&str, &str)] = &[
    // nothing today
];

/// Map insertion-order cases: the key order maps hand back must be the
/// order they were inserted in, through overwrites, removals and
/// re-insertions. (The language reference: "string keys, insertion order
/// kept".) Each case is a line of source; its output is the expected key
/// list.
fn map_cases() -> Vec<Case> {
    vec![
        Case {
            name: "a literal keeps its written order",
            source: "roll m = {b: 1, a: 2, c: 3}\nexhale m.keys()",
            expect: "[\"b\", \"a\", \"c\"]",
        },
        Case {
            name: "a new key goes to the end",
            source: "roll m = {b: 1, a: 2, c: 3}\nm.d = 4\nexhale m.keys()",
            expect: "[\"b\", \"a\", \"c\", \"d\"]",
        },
        Case {
            name: "overwriting a key keeps its position",
            source: "roll m = {b: 1, a: 2, c: 3}\nm.a = 99\nexhale m.keys()",
            expect: "[\"b\", \"a\", \"c\"]",
        },
        Case {
            name: "removing a key leaves the rest in order",
            source: "roll m = {b: 1, a: 2, c: 3}\nm.remove(\"a\")\nexhale m.keys()",
            expect: "[\"b\", \"c\"]",
        },
        Case {
            name: "a removed key re-added goes to the end",
            source: "roll m = {b: 1, a: 2, c: 3}\nm.remove(\"a\")\nm.a = 5\nexhale m.keys()",
            expect: "[\"b\", \"c\", \"a\"]",
        },
        Case {
            name: "keys inserted through a loop keep their turn",
            source: "roll m = {}\nfor k in [\"z\", \"y\", \"x\", \"w\", \"v\"] { m[k] = 1 }\nexhale m.keys()",
            expect: "[\"z\", \"y\", \"x\", \"w\", \"v\"]",
        },
        Case {
            name: "a mid-insert remove and re-add moves the key to the end",
            source: "roll m = {}\nfor k in [\"z\", \"y\", \"x\", \"w\", \"v\"] { m[k] = 1 }\nm.remove(\"x\")\nm[\"x\"] = 2\nexhale m.keys()",
            expect: "[\"z\", \"y\", \"w\", \"v\", \"x\"]",
        },
        Case {
            name: "writing the same key twice in a loop keeps its first turn",
            source: "roll m = {}\nfor k in [\"alpha\", \"beta\", \"gamma\"] {\n  m[k] = 1\n  m[k] = 2\n}\nexhale m.keys()",
            expect: "[\"alpha\", \"beta\", \"gamma\"]",
        },
        Case {
            name: "iterating a map hands its values back in the same order",
            source: "roll m = {}\nfor k in [\"p\", \"q\", \"r\"] { m[k] = k }\nroll vs = []\nfor k in m { vs.push(m[k]) }\nexhale vs",
            expect: "[\"p\", \"q\", \"r\"]",
        },
        Case {
            name: "printing the map prints pairs in insertion order",
            source: "roll m = {b: 1, a: 2, c: 3}\nexhale m",
            expect: "{b: 1, a: 2, c: 3}",
        },
    ]
}

const KNOWN_MAP_FINDINGS: &[(&str, &str)] = &[
    // nothing today
];

/// The marker between cases in the combined program. Written in a form the
/// parser cannot confuse with code: a raw string, exhaled once between
/// cases. Chosen so split() on it is unambiguous.
const SEP: &str = "<<cig-compile determinism case end>>";

/// Compile `source` to `out`, and run it under `cig` and as a program; the
/// outputs, byte for byte.
fn build_and_run(source: &str) -> (String, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let script = dir.path().join("p.cig");
    std::fs::write(&script, source).expect("write script");
    let prog = dir
        .path()
        .join(format!("p{}", std::env::consts::EXE_SUFFIX));
    let home = dir.path().join("home");
    let quiet = |c: &mut Command| {
        c.current_dir(dir.path())
            .env("CIGSCRIPT_HOME", &home)
            .env("CIG_SEAL_KEY", "file")
            .env("CIG_DOCTOR", "0")
            .env("CIG_REPORTS", "never")
            .env("NO_COLOR", "1")
            .env("TZ", "UTC")
            .env("CIGCOMPILE_RUNTIME", CIG)
            .env("CIGCOMPILE_OFFLINE", "1")
            .env_remove("CI")
            .stdin(Stdio::null());
    };
    let mut build = Command::new(CC);
    quiet(&mut build);
    let built = build
        .args(["build"])
        .arg(&script)
        .arg("-o")
        .arg(&prog)
        .output()
        .expect("spawn cigcompile");
    assert!(
        built.status.success(),
        "cigcompile build failed:\n{}\n{}",
        String::from_utf8_lossy(&built.stdout),
        String::from_utf8_lossy(&built.stderr)
    );
    let interp_stdout = run_capture(Command::new(CIG).arg("run").arg(&script), &quiet);
    let mut prog_cmd = Command::new(&prog);
    let prog_stdout = run_capture(&mut prog_cmd, &quiet);
    (interp_stdout, prog_stdout)
}

fn run_capture(cmd: &mut Command, quiet: &dyn Fn(&mut Command)) -> String {
    quiet(cmd);
    let o = cmd.output().expect("spawn");
    assert!(
        o.status.success(),
        "{:?} failed (exit {:?}): stderr={}",
        cmd,
        o.status.code(),
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// A program of every case, exhaling `SEP` between them. One build, one
/// run of each side; each case's output is one block between separators.
/// Each case's statements run in their own `if true { }` block: the
/// language opens a scope on a block, so cases can share names (`m`,
/// `inf`, `nan`) without colliding.
fn combined(cases: &[Case]) -> String {
    let mut s = String::new();
    for (i, c) in cases.iter().enumerate() {
        if i > 0 {
            s.push('\n');
        }
        s.push_str("if true {\n");
        s.push_str(c.source);
        s.push_str(&format!("\nexhale {SEP:?}\n}}\n"));
    }
    s
}

/// The outputs each case produced, in order. A block that is one case's
/// output ends with a newline from `exhale`; the newline is stripped so
/// `expect` lines without a trailing newline compare cleanly.
fn blocks(stdout: &str, n: usize) -> Vec<String> {
    let sep_line = format!("{SEP}\n");
    let parts: Vec<&str> = stdout.split(&sep_line).collect();
    assert!(
        parts.len() >= n,
        "the combined program produced {} blocks for {n} cases:\n---stdout---\n{stdout}\n---",
        parts.len() - 1
    );
    parts
        .into_iter()
        .take(n)
        .map(|p| p.strip_suffix('\n').unwrap_or(p).to_string())
        .collect()
}

#[derive(Debug)]
struct Mismatch {
    case: String,
    side: &'static str,
    expect: String,
    got: String,
}

fn check(cases: &[Case], known_findings: &[(&str, &str)]) -> (Vec<Mismatch>, Vec<String>) {
    let source = combined(cases);
    let (interp, prog) = build_and_run(&source);
    let interp_blocks = blocks(&interp, cases.len());
    let prog_blocks = blocks(&prog, cases.len());
    let known: std::collections::BTreeMap<&str, &str> = known_findings.iter().copied().collect();
    let mut failures: Vec<Mismatch> = Vec::new();
    let mut findings: Vec<String> = Vec::new();
    for (i, c) in cases.iter().enumerate() {
        let i_got = &interp_blocks[i];
        let p_got = &prog_blocks[i];
        let want = c.expect;
        // The program against cig: if these two disagree, the compiler
        // dropped a bit. That is always a failure.
        if i_got != p_got {
            failures.push(Mismatch {
                case: c.name.to_string(),
                side: "program vs cig run",
                expect: i_got.clone(),
                got: p_got.clone(),
            });
            continue;
        }
        if i_got != want {
            // cig and the program agree, but neither matches IEEE: a
            // finding. It is allowed when it is on the list, with the
            // reason the list carries.
            match known.get(c.name) {
                Some(note) => findings.push(format!(
                    "{} (finding: {note})\n  expected: {want}\n  cig said: {i_got}",
                    c.name
                )),
                None => failures.push(Mismatch {
                    case: c.name.to_string(),
                    side: "both sides vs IEEE",
                    expect: want.to_string(),
                    got: i_got.clone(),
                }),
            }
        }
    }
    (failures, findings)
}

fn report(kind: &str, cases: &[Case], failures: &[Mismatch], findings: &[String]) {
    for f in findings {
        println!("{kind} finding: {f}");
    }
    if failures.is_empty() {
        println!(
            "{kind}: {} same, {} finding(s)",
            cases.len(),
            findings.len()
        );
        return;
    }
    let mut s = format!(
        "{kind}: {} mismatch(es), {} finding(s)\n",
        failures.len(),
        findings.len()
    );
    for f in failures {
        s.push_str(&format!(
            "  {} ({})\n    expected: {}\n    got:      {}\n",
            f.case, f.side, f.expect, f.got
        ));
    }
    panic!("{s}");
}

#[test]
fn floats_match_ieee() {
    let cases = float_cases();
    let (failures, findings) = check(&cases, KNOWN_FLOAT_FINDINGS);
    report("floats", &cases, &failures, &findings);
}

#[test]
fn maps_keep_insertion_order() {
    let cases = map_cases();
    let (failures, findings) = check(&cases, KNOWN_MAP_FINDINGS);
    report("maps", &cases, &failures, &findings);
}

/// Mechanical: the combined-program plumbing splits into exactly one block
/// per case, and a case's block is its exhales' output.
#[test]
fn the_block_splitter_lines_up_with_the_cases() {
    let cases = vec![
        Case {
            name: "one",
            source: "exhale \"1a\"\nexhale \"1b\"",
            expect: "1a\n1b",
        },
        Case {
            name: "two",
            source: "exhale \"2\"",
            expect: "2",
        },
    ];
    let source = combined(&cases);
    // One build, one run through the interpreter is enough here.
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("p.cig");
    std::fs::write(&script, &source).unwrap();
    let out = Command::new(CIG)
        .arg("run")
        .arg(&script)
        .current_dir(dir.path())
        .env("CIGSCRIPT_HOME", dir.path().join("home"))
        .env("CIG_SEAL_KEY", "file")
        .env("CIG_DOCTOR", "0")
        .env("CIG_REPORTS", "never")
        .env("NO_COLOR", "1")
        .output()
        .expect("spawn cig");
    assert!(out.status.success(), "{:?}", out);
    let got = blocks(&String::from_utf8_lossy(&out.stdout), cases.len());
    assert_eq!(got, vec!["1a\n1b".to_string(), "2".to_string()]);
}

// Silences the unused-import check when the file is edited to use only
// one path type at a time.
#[allow(dead_code)]
fn _paths_touched(_p: &Path) -> PathBuf {
    PathBuf::new()
}
