// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! Metamorphic testing (#39): a generated program and a variant of it that
//! must mean the same (its variables renamed, its integer constants split
//! into sums, dead declarations between its statements) say the same and
//! exit the same. At stage A both run on the interpreter, so this holds the
//! interpreter to it; a later stage's programs will be held to it too.
//! stdout and the exit code are compared: stderr shows source lines and
//! their numbers, which a variant changes on purpose.

#[path = "differential/gen.rs"]
#[allow(dead_code)]
mod gen;

use std::path::Path;
use std::process::Command;

const RT: &str = env!("CARGO_BIN_EXE_cig-runtime");

/// The generator's names (`v12`, `i3`, `w9`, `t4`, `e13`, `cl4`, `g7`)
/// renamed. `f` names are left: they also name files in strings.
fn renamed(text: &str) -> String {
    let re = regex::Regex::new(r"\b(v|i|w|t|e|cl|g)(\d+)\b").unwrap();
    re.replace_all(text, "${1}_renamed_${2}").into_owned()
}

/// Each integer constant outside a string written as a sum that makes it:
/// `7` becomes `(3 + 4)`. A float, a number inside an identifier and one
/// inside quotes are left as they are.
fn split_constants(text: &str) -> String {
    let c: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut quote: Option<char> = None;
    while i < c.len() {
        let ch = c[i];
        if let Some(q) = quote {
            out.push(ch);
            if ch == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            out.push(ch);
            i += 1;
            continue;
        }
        // A digit after a decimal point (`2.5`) is not a constant's start; one
        // after a range's `..` is.
        let after_point = i >= 2 && c[i - 1] == '.' && c[i - 2].is_ascii_digit();
        // Nor is a float's exponent (`2.5e-3`).
        let exponent = i >= 2 && matches!(c[i - 1], '-' | '+') && matches!(c[i - 2], 'e' | 'E');
        let starts = ch.is_ascii_digit()
            && (i == 0
                || !(c[i - 1].is_alphanumeric() || c[i - 1] == '_' || after_point || exponent));
        if starts {
            let mut j = i;
            while j < c.len() && c[j].is_ascii_digit() {
                j += 1;
            }
            let float = j < c.len()
                && ((c[j] == '.' && c.get(j + 1).is_some_and(|d| d.is_ascii_digit()))
                    || c[j] == 'e'
                    || c[j] == 'E');
            let word: String = c[i..j].iter().collect();
            match word.parse::<i64>() {
                Ok(n) if !float && n >= 2 => {
                    let k = n / 3;
                    out.push_str(&format!("({k} + {})", n - k));
                }
                _ => out.push_str(&word),
            }
            i = j;
            continue;
        }
        out.push(ch);
        i += 1;
    }
    out
}

/// A declaration nothing reads between each pair of top-level statements.
fn with_dead_code(stmts: &[String]) -> String {
    let mut out = Vec::new();
    for (k, s) in stmts.iter().enumerate() {
        out.push(format!("stick dead_{k} = {k} + 1"));
        out.push(s.clone());
    }
    out.join("\n")
}

fn run(dir: &Path, name: &str, text: &str) -> (String, Option<i32>) {
    let d = dir.join(name);
    std::fs::create_dir_all(d.join("out")).unwrap();
    std::fs::write(d.join("p.cig"), format!("{text}\n")).unwrap();
    let o = Command::new(RT)
        .args(["run", "p.cig"])
        .current_dir(&d)
        .env("CIGSCRIPT_HOME", d.join("home"))
        .env("CIG_SEAL_KEY", "file")
        .env("CIG_DOCTOR", "0")
        .env("CIG_REPORTS", "never")
        .env("NO_COLOR", "1")
        .env_remove("CI")
        .output()
        .expect("run cig");
    (
        String::from_utf8_lossy(&o.stdout).into_owned(),
        o.status.code(),
    )
}

#[test]
fn metamorphic_variants_agree() {
    let dir = tempfile::tempdir().unwrap();
    for seed in 0..60u64 {
        let stmts = gen::program(seed);
        let base = stmts.join("\n");
        let want = run(dir.path(), &format!("{seed}-base"), &base);
        for (kind, variant) in [
            ("renamed", renamed(&base)),
            ("split", split_constants(&base)),
            ("dead code", with_dead_code(&stmts)),
        ] {
            let got = run(
                dir.path(),
                &format!("{seed}-{}", kind.replace(' ', "-")),
                &variant,
            );
            assert_eq!(
                got, want,
                "seed {seed}, {kind}: the variant says otherwise\n--- program\n{base}\n--- variant\n{variant}"
            );
        }
    }
}

#[test]
fn the_variants_change_what_they_say_they_change() {
    assert_eq!(
        renamed("roll v12 = i3 + f1(w2)"),
        "roll v_renamed_12 = i_renamed_3 + f1(w_renamed_2)"
    );
    assert_eq!(
        split_constants("exhale 7, \"a 7\", 2.5, 2.5e-3, x9, 1, 0..10, \"煙\".len()"),
        "exhale (2 + 5), \"a 7\", 2.5, 2.5e-3, x9, 1, 0..(3 + 7), \"煙\".len()"
    );
    assert_eq!(
        with_dead_code(&["a".into(), "b".into()]),
        "stick dead_0 = 0 + 1\na\nstick dead_1 = 1 + 1\nb"
    );
}
