// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! `cigcompile verify <program>` (#40): is this program what a build makes?
//! It reads the program's three parts and checks each on its own: the
//! runtime against the sha256 of every release asset this compiler pins
//! (or a runtime given with `--runtime`), the payload against the sha256
//! its trailer and its record carry (and against the script, with
//! `--script`), and the whole file against the bytes a build of those parts
//! makes. The first part that differs is named. Nothing runs.

use crate::build::exit;
use cigscript::cli::Ctx;
use cigscript::program;
use sha2::{Digest, Sha256};
use std::path::Path;

pub struct Opts<'a> {
    pub program: &'a Path,
    pub script: Option<&'a Path>,
    pub runtime: Option<&'a Path>,
}

/// One part's verdict: `ok`, `differs` or `unchecked`, and what was found.
struct Row {
    part: &'static str,
    verdict: &'static str,
    said: String,
}

pub fn verify(ctx: &Ctx, o: &Opts) -> i32 {
    let usage = |message: String, hint: &str| {
        let d = cigscript::diagnostics::usage(message)
            .code("E1000")
            .with_hint(hint.to_string());
        cigscript::cli::report(ctx, &d, None, None);
        exit::USAGE
    };
    let shown = o.program.display().to_string();
    let bytes = match std::fs::read(o.program) {
        Ok(b) => b,
        Err(e) => {
            return usage(
                format!("cannot read {shown}: {e}"),
                "name a program cigcompile built",
            )
        }
    };
    let mut rows = Vec::new();
    let mut cursor = std::io::Cursor::new(&bytes);
    let p = match program::read(&mut cursor) {
        Ok(Some(p)) => p,
        Ok(None) => {
            return usage(
                format!("{shown} is not a CigScript program: no program follows its runtime"),
                "cigcompile verify reads programs that cigcompile build wrote",
            )
        }
        Err(why) => {
            rows.push(Row {
                part: "payload",
                verdict: "differs",
                said: format!("damaged: {why}"),
            });
            return say(ctx, &shown, None, &rows);
        }
    };
    let payload = p.payload();
    let end = bytes.len() as u64;
    // Where the runtime ends: its image's end, from its headers; else
    // where the payload the trailer counts begins.
    let mut c2 = std::io::Cursor::new(&bytes);
    let image = program::image_of(&mut c2, end);
    let runtime_end = image
        .map(|i| i.end)
        .unwrap_or_else(|| end.saturating_sub(program::TRAILER_LEN + payload.len() as u64))
        as usize;
    let runtime = &bytes[..runtime_end.min(bytes.len())];
    let runtime_sha = hex::encode(Sha256::digest(runtime));
    // The runtime: a release asset this compiler pins, or the one given.
    match o.runtime {
        Some(given) => match std::fs::read(given) {
            Ok(g) if g == runtime => rows.push(Row {
                part: "runtime",
                verdict: "ok",
                said: format!(
                    "the same bytes as {} (sha256 {})",
                    given.display(),
                    &runtime_sha[..16]
                ),
            }),
            Ok(g) => rows.push(Row {
                part: "runtime",
                verdict: "differs",
                said: format!("not {}: {}", given.display(), first_difference(&g, runtime)),
            }),
            Err(e) => {
                return usage(
                    format!("cannot read {}: {e}", given.display()),
                    "name the cig the program was built on",
                )
            }
        },
        None => {
            let asset = crate::digests::DIGESTS
                .iter()
                .find(|(_, sha)| *sha == runtime_sha)
                .map(|(name, _)| *name);
            match asset {
                Some(name) => rows.push(Row {
                    part: "runtime",
                    verdict: "ok",
                    said: format!("cig's release asset {name} (sha256 {})", &runtime_sha[..16]),
                }),
                None if p.cig != crate::digests::VERSION => rows.push(Row {
                    part: "runtime",
                    verdict: "unchecked",
                    said: format!(
                        "the program runs on cig {}, and this cigcompile pins the assets of cig {}; give that cig with --runtime",
                        p.cig,
                        crate::digests::VERSION
                    ),
                }),
                None => rows.push(Row {
                    part: "runtime",
                    verdict: "differs",
                    said: format!(
                        "sha256 {} is none of cig {}'s release assets (a cig built from source is its own bytes: give it with --runtime)",
                        &runtime_sha[..16],
                        crate::digests::VERSION
                    ),
                }),
            }
        }
    }
    // The payload: read() held it to its trailer's sha256 and its record's;
    // with --script, to the script too.
    rows.push(Row {
        part: "payload",
        verdict: "ok",
        said: format!(
            "{} ({} lines, sha256 {}), built by {} for cig {}",
            p.file,
            p.source.lines().count(),
            &p.sha256[..16],
            p.compiler,
            p.cig
        ),
    });
    if let Some(script) = o.script {
        match std::fs::read_to_string(script) {
            Ok(s) if s == p.source => rows.push(Row {
                part: "script",
                verdict: "ok",
                said: format!("{} is the script it was built from", script.display()),
            }),
            Ok(s) => {
                let line = s
                    .lines()
                    .zip(p.source.lines())
                    .position(|(a, b)| a != b)
                    .unwrap_or_else(|| s.lines().count().min(p.source.lines().count()))
                    + 1;
                rows.push(Row {
                    part: "script",
                    verdict: "differs",
                    said: format!(
                        "{} has changed since the build, from line {line}",
                        script.display()
                    ),
                })
            }
            Err(e) => {
                return usage(
                    format!("cannot read {}: {e}", script.display()),
                    "name the script the program was built from",
                )
            }
        }
    }
    // The whole file, against what a build of these parts writes.
    let rebuilt = program::append(runtime, &p);
    if rebuilt == bytes {
        rows.push(Row {
            part: "whole",
            verdict: "ok",
            said: "byte for byte what a build of these parts writes".to_string(),
        });
    } else if bytes.starts_with(&rebuilt) && image.is_some_and(|i| i.certificates.is_some()) {
        rows.push(Row {
            part: "whole",
            verdict: "ok",
            said: "a build of these parts, signed since (a certificate table follows the trailer)"
                .to_string(),
        });
    } else {
        rows.push(Row {
            part: "whole",
            verdict: "differs",
            said: format!(
                "not what a build of these parts writes: {}",
                first_difference(&rebuilt, &bytes)
            ),
        });
    }
    say(ctx, &shown, Some(&p), &rows)
}

/// Where two byte strings first part, said by offset.
fn first_difference(want: &[u8], got: &[u8]) -> String {
    match want.iter().zip(got).position(|(a, b)| a != b) {
        Some(at) => format!("the first difference is at byte {at}"),
        None => format!("{} bytes against {}", got.len(), want.len()),
    }
}

fn say(ctx: &Ctx, shown: &str, p: Option<&program::Program>, rows: &[Row]) -> i32 {
    let differs = rows.iter().any(|r| r.verdict == "differs");
    if ctx.json {
        println!(
            "{}",
            serde_json::json!({
                "program": shown,
                "name": p.map(|p| p.name.clone()),
                "ok": !differs,
                "parts": rows.iter().map(|r| serde_json::json!({"part": r.part, "verdict": r.verdict, "said": r.said})).collect::<Vec<_>>(),
            })
        );
    } else {
        println!("{shown}");
        for r in rows {
            let v = match r.verdict {
                "ok" => ctx.green("ok       "),
                "differs" => ctx.red("DIFFERS  "),
                _ => ctx.yellow("unchecked"),
            };
            println!("  {:<8} {v} {}", r.part, r.said);
        }
    }
    if differs {
        exit::DIFFERS
    } else {
        exit::OK
    }
}
