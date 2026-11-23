// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! The differential suite's generator makes only programs `cig check`
//! accepts (#39): a random case that fails the check would test the
//! checker, not the compiler, and hide the run behind it. Thousands of
//! seeds, checked in-process: no runs, quick.

#[path = "differential/gen.rs"]
#[allow(dead_code)]
mod gen;

fn first_error(text: &str) -> Option<String> {
    match cigscript::syntax::parse(text) {
        Err(d) => Some(d.message),
        Ok(p) => cigscript::check::check(&p, &[])
            .errors
            .into_iter()
            .next()
            .map(|d| d.message),
    }
}

#[test]
fn smith_programs_all_check() {
    for seed in 0..3000u64 {
        let text = gen::program(seed).join("\n") + "\n";
        if let Some(e) = first_error(&text) {
            panic!("seed {seed} does not check: {e}\n{text}");
        }
    }
}

#[test]
fn the_edge_programs_check() {
    for (name, text) in gen::edges() {
        assert!(
            first_error(&text).is_none(),
            "{name}: {:?}\n{text}",
            first_error(&text)
        );
    }
}

/// No loop's body assigns the loop's own counter but its own step: a
/// `while` whose body set its counter back would never end (seed 75 did,
/// and ran each side to the step budget).
#[test]
fn no_while_body_resets_its_own_counter() {
    let assign = regex::Regex::new(r"(?m)^(w\d+) (=|\+=|-=) (.*)$").unwrap();
    for seed in 0..3000u64 {
        let text = gen::program(seed).join("\n");
        for c in assign.captures_iter(&text) {
            assert!(
                &c[2] == "+=" && &c[3] == "1",
                "seed {seed}: {}\n{text}",
                c.get(0).unwrap().as_str()
            );
        }
    }
}

/// The widened generator reaches what it was widened for, across the seeds.
#[test]
fn the_generator_reaches_floats_closures_finally_and_edges() {
    let all: String = (0..3000u64)
        .map(|seed| gen::program(seed).join("\n"))
        .collect::<Vec<_>>()
        .join("\n");
    for (what, needle) in [
        ("a float", "1.5"),
        ("a closure made in a loop", "pack() =>"),
        (
            "a loop leaving through finally",
            "} finally {\nexhale \"turn\"",
        ),
        ("a value leaving through finally", "try {\nsnuff"),
        ("an edge of i64", "9223372036854775807"),
        ("text past ASCII", "煙"),
    ] {
        assert!(all.contains(needle), "no seed reaches {what}");
    }
}

/// The typed generator (#39): every seed makes a program the checker
/// accepts. One that did not would compare two refusals, not two runs.
#[test]
fn typed_programs_all_check() {
    for seed in 0..3000u64 {
        let text = gen::typed_program(seed).join("\n") + "\n";
        if let Some(e) = first_error(&text) {
            panic!("typed seed {seed} does not check: {e}\n{text}");
        }
    }
}

/// The hand-written typed edges check too.
#[test]
fn the_typed_edge_programs_check() {
    for (name, text) in gen::typed_edges() {
        assert!(
            first_error(&text).is_none(),
            "{name}: {:?}\n{text}",
            first_error(&text)
        );
    }
}

/// Every script under `tests/corpus/typed/` parses and checks.
#[test]
fn the_typed_corpus_files_all_check() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("corpus")
        .join("typed");
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "cig"))
        .collect();
    entries.sort();
    assert!(
        entries.len() >= 20,
        "the typed corpus is too thin: {} files",
        entries.len()
    );
    for path in entries {
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        if let Some(e) = first_error(&text) {
            panic!(
                "{} does not check: {e}\n{text}",
                path.file_name().unwrap().to_string_lossy()
            );
        }
    }
}

/// The typed generator reaches what it is for: the edges of i64 and of
/// f64, calls between its functions, loops, and a `try` around each
/// call.
#[test]
fn the_typed_generator_reaches_edges_and_rows() {
    let all: String = (0..3000u64)
        .map(|seed| gen::typed_program(seed).join("\n"))
        .collect::<Vec<_>>()
        .join("\n");
    for (what, needle) in [
        ("an edge of i64", "9223372036854775807"),
        ("negative one, literal", "(-1)"),
        ("the smallest i64", "-9223372036854775807 - 1"),
        ("-0.0", "(-0.0)"),
        ("1e308", "1e308"),
        ("1e-300", "1e-300"),
        ("try around every row", "ashtray e {"),
        ("typed function declaration", "pull tf"),
        ("a bounded for loop", "for i"),
    ] {
        assert!(all.contains(needle), "no typed seed reaches {what}");
    }
}
