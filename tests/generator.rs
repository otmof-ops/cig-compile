// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
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
