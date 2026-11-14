// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! `cigcompile verify <program>` (#40): is this program what a build makes?
//! Nothing runs.
//!
//! A format 2 program is read part by part, as its layout puts them
//! (`docs/how-it-works.md`): the `runtime` (against cig's release assets or
//! `--runtime`, and against the sha256 its record gives), the `record`,
//! the `section table`, each `section <id>` (its bytes against the table's
//! sha256, decoded; `source` against the record and, with `--script`, the
//! script; `bytecode` parsed, verified and held to a fresh lowering at the
//! recorded level), the `padding` and the `trailer`, and then the whole
//! file against what a build of those parts writes, the first difference
//! named by its part. The trailer's head sha256 covers the record and the
//! table, so a change there is told apart from a change in a body.
//!
//! A format 1 program keeps its three parts: the `runtime`, the `payload`
//! against the sha256 its trailer and its record carry (and the `script`,
//! with `--script`), and the `whole` file.

use crate::build::exit;
use cigscript::cli::Ctx;
use cigscript::program;
use sha2::{Digest, Sha256};
use std::path::Path;

pub struct Opts<'a> {
    pub program: &'a Path,
    pub script: Option<&'a Path>,
    pub runtime: Option<&'a Path>,
    /// A detached SSH signature of the program to check, as `build --sign`
    /// writes it (`<program>.sig`).
    pub signature: Option<&'a Path>,
    /// The allowed-signers file naming who may have signed the program;
    /// without one, only the signature's integrity is checked.
    pub allowed_signers: Option<&'a Path>,
    /// The signer's identity to require in the allowed-signers file
    /// (default `*`, any identity there).
    pub identity: Option<&'a str>,
    /// A program pass to check (`<program>.pass`, as `build --pass` writes
    /// it). `allowed_signers`, when given, is who is trusted for it.
    pub pass: Option<&'a Path>,
}

/// One part's verdict: `ok`, `differs` or `unchecked`, and what was found.
pub(crate) struct Row {
    part: String,
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
    if format2::is(&bytes) {
        return format2::verify(ctx, o, &shown, &bytes);
    }
    let mut rows = Vec::new();
    let mut cursor = std::io::Cursor::new(&bytes);
    let p = match program::read(&mut cursor) {
        Ok(Some(l)) => match l.format1() {
            Some(p) => p.clone(),
            None => return format2::verify(ctx, o, &shown, &bytes),
        },
        Ok(None) => {
            return usage(
                format!("{shown} is not a CigScript program: no program follows its runtime"),
                "cigcompile verify reads programs that cigcompile build wrote",
            )
        }
        Err(why) => {
            rows.push(Row {
                part: "payload".into(),
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
    // The runtime: a release asset this compiler pins, or the one given.
    match runtime_row(o, runtime, &p.cig) {
        Ok(r) => rows.push(r),
        Err(e) => return usage(e, "name the cig the program was built on"),
    }
    // The payload: read() held it to its trailer's sha256 and its record's;
    // with --script, to the script too.
    rows.push(Row {
        part: "payload".into(),
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
                part: "script".into(),
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
                    part: "script".into(),
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
    let signed_since =
        bytes.starts_with(&rebuilt) && image.is_some_and(|i| i.certificates.is_some());
    if rebuilt == bytes {
        rows.push(Row {
            part: "whole".into(),
            verdict: "ok",
            said: "byte for byte what a build of these parts writes".to_string(),
        });
    } else if signed_since {
        rows.push(Row {
            part: "whole".into(),
            verdict: "ok",
            said: "a build of these parts, signed since (a certificate table follows the trailer)"
                .to_string(),
        });
    } else {
        rows.push(Row {
            part: "whole".into(),
            verdict: "differs",
            said: format!(
                "not what a build of these parts writes: {}",
                first_difference(&rebuilt, &bytes)
            ),
        });
    }
    // The signature: an SSH signature made by `build --sign` goes beside
    // the program, and the verify uses `ssh-keygen -Y verify` (with
    // --allowed-signers) or `-Y check-novalidate` (without it, which
    // reports the signer is unchecked).
    trust_rows(o, &bytes, rebuilt != bytes && signed_since, &mut rows);
    say(ctx, &shown, Some(&p.name), &rows)
}

/// The runtime's row: the cig given with `--runtime`, byte for byte, or
/// one of the release assets this compiler pins, by sha256. `Err`: the
/// runtime given does not read.
pub(crate) fn runtime_row(o: &Opts, runtime: &[u8], cig: &str) -> Result<Row, String> {
    let runtime_sha = hex::encode(Sha256::digest(runtime));
    Ok(match o.runtime {
        Some(given) => match std::fs::read(given) {
            Ok(g) if g == runtime => Row {
                part: "runtime".into(),
                verdict: "ok",
                said: format!(
                    "the same bytes as {} (sha256 {})",
                    given.display(),
                    &runtime_sha[..16]
                ),
            },
            Ok(g) => Row {
                part: "runtime".into(),
                verdict: "differs",
                said: format!("not {}: {}", given.display(), first_difference(&g, runtime)),
            },
            Err(e) => return Err(format!("cannot read {}: {e}", given.display())),
        },
        None => {
            let asset = crate::digests::DIGESTS
                .iter()
                .find(|(_, sha)| *sha == runtime_sha)
                .map(|(name, _)| *name);
            match asset {
                Some(name) => Row {
                    part: "runtime".into(),
                    verdict: "ok",
                    said: format!("cig's release asset {name} (sha256 {})", &runtime_sha[..16]),
                },
                None if cig != crate::digests::VERSION => Row {
                    part: "runtime".into(),
                    verdict: "unchecked",
                    said: format!(
                        "the program runs on cig {cig}, and this cigcompile pins the assets of cig {}; give that cig with --runtime",
                        crate::digests::VERSION
                    ),
                },
                None => Row {
                    part: "runtime".into(),
                    verdict: "differs",
                    said: format!(
                        "sha256 {} is none of cig {}'s release assets (a cig built from source is its own bytes: give it with --runtime)",
                        &runtime_sha[..16],
                        crate::digests::VERSION
                    ),
                },
            }
        }
    })
}

/// The signature and the pass, with either format.
fn trust_rows(o: &Opts, bytes: &[u8], signed_since: bool, rows: &mut Vec<Row>) {
    if let Some(sig) = o.signature {
        rows.push(signature_row(o.program, sig, o.allowed_signers, o.identity));
    }
    // The program pass names the program's bytes as they were built. Code
    // signing rewrites them (a PE's headers, a Mach-O's image), so a pass
    // is checked before signing, and a signed program says so.
    if let Some(at) = o.pass {
        let sha = hex::encode(Sha256::digest(bytes));
        rows.push(match crate::pass::check(at, &sha, o.allowed_signers) {
            Ok(said) => Row {
                part: "pass".into(),
                verdict: "ok",
                said,
            },
            Err(why) if signed_since => Row {
                part: "pass".into(),
                verdict: "differs",
                said: format!("{why}; this program was code-signed since its build, and a pass names the bytes before signing: check it then"),
            },
            Err(why) => Row {
                part: "pass".into(),
                verdict: "differs",
                said: why,
            },
        });
    }
}

/// Check the signature at `sig` of `program`, and shape the `signature`
/// row the table shows: `ok`, `DIFFERS` or `unchecked`.
pub(crate) fn signature_row(
    program: &Path,
    sig: &Path,
    allowed: Option<&Path>,
    identity: Option<&str>,
) -> Row {
    match crate::sign::check(program, sig, allowed, identity) {
        Ok(crate::sign::Check::Ok(said)) => Row {
            part: "signature".into(),
            verdict: "ok",
            said,
        },
        Ok(crate::sign::Check::Unchecked(said)) => Row {
            part: "signature".into(),
            verdict: "unchecked",
            said,
        },
        Ok(crate::sign::Check::Differs(said)) => Row {
            part: "signature".into(),
            verdict: "differs",
            said,
        },
        Err(e) => Row {
            part: "signature".into(),
            verdict: "differs",
            said: e,
        },
    }
}

/// Where two byte strings first part, said by offset.
pub(crate) fn first_difference(want: &[u8], got: &[u8]) -> String {
    match want.iter().zip(got).position(|(a, b)| a != b) {
        Some(at) => format!("the first difference is at byte {at}"),
        None => format!("{} bytes against {}", got.len(), want.len()),
    }
}

fn say(ctx: &Ctx, shown: &str, name: Option<&str>, rows: &[Row]) -> i32 {
    let differs = rows.iter().any(|r| r.verdict == "differs");
    if ctx.json {
        println!(
            "{}",
            serde_json::json!({
                "program": shown,
                "name": name,
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
            println!("  {:<16} {v} {}", r.part, r.said);
        }
    }
    if differs {
        exit::DIFFERS
    } else {
        exit::OK
    }
}

/// A format 2 program, part by part (CigScript's `docs/PROGRAMS.md`).
mod format2 {
    use super::{first_difference, runtime_row, say, trust_rows, Opts, Row};
    use crate::build::exit;
    use cigscript::cli::Ctx;
    use cigscript::program::{
        self, Encoding, Entry, Record, ALIGN, ENTRY_LEN, KNOWN_SECTIONS, MAGIC2, MAX_PAYLOAD2,
        MAX_RECORD, MAX_SECTIONS, TRAILER2_LEN,
    };
    use sha2::{Digest, Sha256};
    use std::io::Read as _;

    const RECORD_START: &[u8] = b"{\"format\":";

    /// Is `bytes` a format 2 program, whole or damaged: a `CIGPROG2`
    /// trailer at its end, or a record that starts `{"format":2` where its
    /// runtime's image ends.
    pub fn is(bytes: &[u8]) -> bool {
        trailer(bytes).is_some()
            || image(bytes).is_some_and(|i| {
                bytes
                    .get(i.end as usize..)
                    .is_some_and(|r| r.starts_with(b"{\"format\":2"))
            })
    }

    fn image(bytes: &[u8]) -> Option<program::Image> {
        program::image_of(&mut std::io::Cursor::new(bytes), bytes.len() as u64)
    }

    /// Where the trailer starts, and whether a certificate table follows
    /// it: the last `CIGPROG2` with only zeros after it, up to the end or
    /// to a PE certificate table that ends the file.
    fn trailer(bytes: &[u8]) -> Option<(usize, bool)> {
        let (end, signed) = match image(bytes).and_then(|i| i.certificates) {
            Some((at, size)) if at.checked_add(size) == Some(bytes.len() as u64) => {
                (at as usize, true)
            }
            _ => (bytes.len(), false),
        };
        let t = TRAILER2_LEN as usize;
        let mut at = end.checked_sub(t)?;
        loop {
            if bytes[at..at + 8] == MAGIC2 {
                return Some((at, signed));
            }
            if at == 0 || bytes[at + t - 1] != 0 {
                return None;
            }
            at -= 1;
        }
    }

    fn le(b: &[u8]) -> u64 {
        b.iter()
            .rev()
            .fold(0u64, |n, byte| (n << 8) | u64::from(*byte))
    }

    fn entry(b: &[u8]) -> Entry {
        let mut raw_id = [0u8; 8];
        raw_id.copy_from_slice(&b[..8]);
        let mut sha256 = [0u8; 32];
        sha256.copy_from_slice(&b[32..64]);
        Entry {
            raw_id,
            version: le(&b[8..10]) as u16,
            encoding: b[10],
            flags: b[11],
            decoded_len: le(&b[12..16]) as u32,
            offset: le(&b[16..24]),
            length: le(&b[24..32]),
            sha256,
        }
    }

    /// The parts, in the order the file holds them, each with its verdict.
    /// The first `differs` said of a part is the one kept.
    #[derive(Default)]
    struct Parts {
        rows: Vec<Row>,
    }

    impl Parts {
        fn mark(&mut self, part: &str, verdict: &'static str, said: String) {
            match self.rows.iter_mut().find(|r| r.part == part) {
                Some(r) if r.verdict != "differs" && verdict == "differs" => {
                    r.verdict = verdict;
                    r.said = said;
                }
                Some(_) => {}
                None => self.rows.push(Row {
                    part: part.to_string(),
                    verdict,
                    said,
                }),
            }
        }

        fn differs(&self, part: &str) -> bool {
            self.rows
                .iter()
                .any(|r| r.part == part && r.verdict == "differs")
        }

        fn any_differs(&self) -> bool {
            self.rows.iter().any(|r| r.verdict == "differs")
        }

        /// The rows in the file's order: runtime, record, the sections in
        /// table order, the table, the padding, the trailer.
        fn ordered(mut self, ids: &[String]) -> Vec<Row> {
            let mut order: Vec<String> = vec!["runtime".into(), "record".into()];
            order.extend(ids.iter().map(|i| format!("section {i}")));
            order.extend(["section table", "padding", "trailer"].map(String::from));
            let at = |p: &str| order.iter().position(|o| o == p).unwrap_or(order.len());
            self.rows.sort_by_key(|r| at(&r.part));
            self.rows
        }
    }

    /// The table's own rules, as a reader checks them, and what a build
    /// always writes: `source` and `bytecode` required, at their versions.
    fn table_rules(entries: &[Entry], p: u64, record_len: u64, table_at: u64) -> Option<String> {
        let mut prev_end = record_len;
        let mut prev_key: Option<(usize, [u8; 8])> = None;
        for (n, e) in entries.iter().enumerate() {
            let id = e.id();
            if id.is_empty() {
                return Some(format!(
                    "entry {} has an id that is not lower-case ASCII, zero-padded",
                    n + 1
                ));
            }
            let key = (
                KNOWN_SECTIONS
                    .iter()
                    .position(|k| *k == id)
                    .unwrap_or(KNOWN_SECTIONS.len()),
                e.raw_id,
            );
            if prev_key.is_some_and(|k| key <= k) {
                return Some(format!("section `{id}` is repeated or out of order"));
            }
            prev_key = Some(key);
            if e.encoding().is_none() {
                return Some(format!("section `{id}` has encoding {}", e.encoding));
            }
            if e.flags & !1 != 0 {
                return Some(format!(
                    "section `{id}` sets flag bits {:#04x}, and only bit 0 is one",
                    e.flags
                ));
            }
            if KNOWN_SECTIONS.contains(&id) && !e.required() && id != "sbom" {
                return Some(format!(
                    "section `{id}` is not marked required, and a build marks it"
                ));
            }
            let version = match id {
                "source" => Some(1),
                "bytecode" => Some(cigscript::cig_bytecode::FORMAT_VERSION),
                _ => None,
            };
            if version.is_some_and(|v| v != e.version) {
                return Some(format!("section `{id}` says version {}", e.version));
            }
            if u64::from(e.decoded_len) > MAX_PAYLOAD2 {
                return Some(format!("section `{id}` decodes past 64 MiB"));
            }
            if e.encoding() == Some(Encoding::Stored) && u64::from(e.decoded_len) != e.length {
                return Some(format!(
                    "section `{id}` is stored as is, and its decoded length is not its length"
                ));
            }
            if e.offset < prev_end {
                return Some(format!(
                    "section `{id}` starts at {}, inside what comes before it",
                    e.offset
                ));
            }
            if e.offset - prev_end >= ALIGN {
                return Some(format!(
                    "section `{id}` starts {} bytes after what comes before it, more than its alignment needs",
                    e.offset - prev_end
                ));
            }
            match e.offset.checked_add(e.length) {
                Some(end) if end <= table_at => prev_end = end,
                _ => return Some(format!("section `{id}` runs into the section table")),
            }
            if (p + e.offset) % ALIGN != 0 {
                return Some(format!(
                    "section `{id}` does not start at a file offset that is a multiple of 16"
                ));
            }
        }
        if table_at - prev_end >= ALIGN {
            return Some(
                "the table starts further from the last section than its alignment needs".into(),
            );
        }
        if entries.first().map(Entry::id) != Some("source") {
            return Some("its first section is not `source`, which a build always writes".into());
        }
        None
    }

    /// The record's own rules.
    fn record_rules(bytes: &[u8]) -> Result<Record, String> {
        if !bytes.starts_with(RECORD_START) {
            return Err("it does not start with {\"format\":".into());
        }
        let r: Record = serde_json::from_slice(bytes)
            .map_err(|e| format!("it does not read: {}", e.to_string().escape_debug()))?;
        let hex64 = |s: &str| {
            s.len() == 64
                && s.bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        };
        if r.format != program::FORMAT2 {
            return Err(format!("it says format {}", r.format));
        }
        if !program::good_name(&r.name) {
            return Err(format!(
                "its name `{}` is not a program's name",
                r.name.escape_debug()
            ));
        }
        for (k, v) in [
            ("file", &r.file),
            ("compiler", &r.compiler),
            ("cig", &r.cig),
            ("opt", &r.opt),
            ("target", &r.target),
        ] {
            if v.is_empty() || !program::printable(v) {
                return Err(format!("its `{k}` is not printable text"));
            }
        }
        for (k, v) in [("sha256", &r.sha256), ("runtime_sha256", &r.runtime_sha256)] {
            if !hex64(v) {
                return Err(format!("its `{k}` is not a sha256"));
            }
        }
        if serde_json::to_vec(&r).ok().as_deref() != Some(bytes) {
            return Err(
                "it is not written as a build writes it (its keys, their order, no whitespace)"
                    .into(),
            );
        }
        Ok(r)
    }

    fn decode(e: &Entry, stored: &[u8]) -> Result<Vec<u8>, String> {
        match e.encoding() {
            Some(Encoding::Deflate) => {
                let len = u64::from(e.decoded_len);
                let mut out = Vec::new();
                flate2::read::DeflateDecoder::new(stored)
                    .take(len + 1)
                    .read_to_end(&mut out)
                    .map_err(|x| format!("it does not inflate: {x}"))?;
                if out.len() as u64 != len {
                    return Err(format!(
                        "it inflates to {} bytes, not the {len} its entry gives",
                        out.len()
                    ));
                }
                Ok(out)
            }
            _ => Ok(stored.to_vec()),
        }
    }

    pub fn verify(ctx: &Ctx, o: &Opts, shown: &str, bytes: &[u8]) -> i32 {
        let usage = |message: String, hint: &str| {
            let d = cigscript::diagnostics::usage(message)
                .code("E1000")
                .with_hint(hint.to_string());
            cigscript::cli::report(ctx, &d, None, None);
            exit::USAGE
        };
        let mut parts = Parts::default();
        let img = image(bytes);
        let found = trailer(bytes);
        let finish = |parts: Parts, ids: &[String], name: Option<&str>, signed: bool| {
            let mut rows = parts.ordered(ids);
            trust_rows(o, bytes, signed, &mut rows);
            say(ctx, shown, name, &rows)
        };
        let runtime_of = |parts: &mut Parts, p: Option<u64>, cig: &str| -> Result<(), i32> {
            match p {
                Some(p) => match runtime_row(o, &bytes[..p as usize], cig) {
                    Ok(r) => parts.mark("runtime", r.verdict, r.said),
                    Err(e) => return Err(usage(e, "name the cig the program was built on")),
                },
                None => parts.mark(
                    "runtime",
                    "unchecked",
                    "where it ends is not known: its headers do not say, and its trailer does not read".into(),
                ),
            }
            Ok(())
        };
        let Some((t_at, signed)) = found else {
            parts.mark(
                "trailer",
                "differs",
                "damaged: no CIGPROG2 trailer at its end (the file was cut short, or something was written after it)".into(),
            );
            if let Err(c) = runtime_of(&mut parts, img.map(|i| i.end), crate::digests::VERSION) {
                return c;
            }
            return finish(parts, &[], None, false);
        };
        let t = &bytes[t_at..t_at + TRAILER2_LEN as usize];
        let len = le(&t[8..16]);
        let record_len = le(&t[16..20]);
        let count = le(&t[20..22]);
        let flags = le(&t[22..24]);
        let p_trailer = (t_at as u64).checked_sub(len);
        let trailer_problem = if flags != 0 {
            Some(format!("its flags are {flags:#06x}, and a build writes 0"))
        } else if len > MAX_PAYLOAD2 || p_trailer.is_none() {
            Some(format!(
                "its payload length ({len}) is more than the file holds"
            ))
        } else if img.is_some_and(|i| Some(i.end) != p_trailer) {
            Some(format!(
                "its payload length puts the payload at byte {}, and the runtime's image ends at byte {}",
                p_trailer.unwrap_or(0),
                img.map_or(0, |i| i.end)
            ))
        } else if record_len == 0 || record_len > MAX_RECORD {
            Some(format!(
                "its record length ({record_len}) is not 1 to 65,536"
            ))
        } else if count == 0 || count > MAX_SECTIONS {
            Some(format!(
                "its section count ({count}) is not 1 to {MAX_SECTIONS}"
            ))
        } else if record_len + count * ENTRY_LEN > len {
            Some("its record and table do not fit in its payload".into())
        } else {
            None
        };
        if let Some(why) = trailer_problem {
            parts.mark("trailer", "differs", format!("damaged: {why}"));
            if let Err(c) = runtime_of(&mut parts, img.map(|i| i.end), crate::digests::VERSION) {
                return c;
            }
            return finish(parts, &[], None, signed);
        }
        let p = p_trailer.expect("checked");
        let pu = p as usize;
        let table_at = len - count * ENTRY_LEN;
        let record_bytes = &bytes[pu..pu + record_len as usize];
        let table_bytes = &bytes[pu + table_at as usize..t_at];
        let head_ok = {
            let mut h = Sha256::new();
            h.update(record_bytes);
            h.update(table_bytes);
            h.finalize()[..] == t[24..56]
        };
        let entries: Vec<Entry> = table_bytes
            .chunks_exact(ENTRY_LEN as usize)
            .map(entry)
            .collect();
        let ids: Vec<String> = entries.iter().map(|e| e.id().to_string()).collect();
        let record = record_rules(record_bytes);
        let mut table_problem = table_rules(&entries, p, record_len, table_at);
        if let (None, Ok(r)) = (&table_problem, &record) {
            if !ids.iter().any(|i| i == r.stage.needs()) {
                table_problem = Some(format!(
                    "a stage {} program needs section `{}`, and it has none",
                    r.stage.as_str(),
                    r.stage.needs()
                ));
            }
        }
        // Each body against its entry, and decoded.
        let mut bodies: Vec<Result<Vec<u8>, String>> = Vec::new();
        let mut padding_problem = None;
        if table_problem.is_none() {
            let mut prev_end = record_len;
            for e in &entries {
                let at = pu + e.offset as usize;
                if padding_problem.is_none()
                    && bytes[pu + prev_end as usize..at].iter().any(|b| *b != 0)
                {
                    padding_problem = Some(format!(
                        "the padding before section `{}` is not zero",
                        e.id()
                    ));
                }
                prev_end = e.offset + e.length;
                let stored = &bytes[at..at + e.length as usize];
                bodies.push(if Sha256::digest(stored)[..] != e.sha256 {
                    Err("its bytes do not match the sha256 in the section table".into())
                } else {
                    decode(e, stored)
                });
            }
            if padding_problem.is_none()
                && bytes[pu + prev_end as usize..pu + table_at as usize]
                    .iter()
                    .any(|b| *b != 0)
            {
                padding_problem = Some("the padding before the section table is not zero".into());
            }
        }
        let body_problem = bodies.iter().any(Result::is_err);
        // Which part changed. The head sha256 covers the record and the
        // table: when it holds, they are as they were written, and a
        // change is in a body, the padding or the runtime.
        if !head_ok {
            let why =
                "the record and the section table do not match the head sha256 in the trailer";
            if let Some(t) = &table_problem {
                parts.mark("section table", "differs", format!("damaged: {why}: {t}"));
            } else if body_problem {
                parts.mark(
                    "section table",
                    "differs",
                    format!("damaged: {why}, and an entry does not match its section"),
                );
            } else if let Err(r) = &record {
                parts.mark("record", "differs", format!("damaged: {why}: {r}"));
            } else {
                parts.mark("record", "differs", format!("damaged: {why}, and the table matches every section: the record changed (or the head sha256 itself)"));
            }
        } else {
            if let Some(t) = &table_problem {
                parts.mark(
                    "section table",
                    "differs",
                    format!("not what a build writes: {t}"),
                );
            }
            if let Err(r) = &record {
                parts.mark("record", "differs", format!("not what a build writes: {r}"));
            }
            for (e, b) in entries.iter().zip(&bodies) {
                if let Err(why) = b {
                    parts.mark(
                        &format!("section {}", e.id()),
                        "differs",
                        format!("damaged: {why}"),
                    );
                }
            }
            if let Some(why) = padding_problem.clone() {
                parts.mark("padding", "differs", format!("damaged: {why}"));
            }
        }
        let name = record.as_ref().ok().map(|r| r.name.clone());
        let cig = record
            .as_ref()
            .map_or(crate::digests::VERSION, |r| r.cig.as_str())
            .to_string();
        if let Err(c) = runtime_of(&mut parts, Some(p), &cig) {
            return c;
        }
        let runtime = &bytes[..pu];
        let Ok(r) = record else {
            return finish(parts, &ids, name.as_deref(), signed);
        };
        // The runtime against what the record says of it.
        let runtime_sha = hex::encode(Sha256::digest(runtime));
        if runtime_sha != r.runtime_sha256 {
            parts.mark(
                "runtime",
                "differs",
                format!(
                    "sha256 {}, and the record says it was {} when the program was built",
                    &runtime_sha[..16],
                    &r.runtime_sha256[..16]
                ),
            );
        } else if let Some(v) = program::runtime_version(runtime).filter(|v| *v != r.cig) {
            parts.mark(
                "runtime",
                "differs",
                format!("cig {v}, and the record says cig {}", r.cig),
            );
        }
        if table_problem.is_some() || body_problem || !head_ok {
            return finish(parts, &ids, name.as_deref(), signed);
        }
        let body = |id: &str| {
            entries
                .iter()
                .position(|e| e.id() == id)
                .and_then(|i| bodies[i].as_ref().ok())
        };
        // The source: its text against the record, and the script.
        let script = match o.script {
            Some(s) => match std::fs::read_to_string(s) {
                Ok(t) => Some((s, t)),
                Err(e) => {
                    return usage(
                        format!("cannot read {}: {e}", s.display()),
                        "name the script the program was built from",
                    )
                }
            },
            None => None,
        };
        let text = body("source").and_then(|b| String::from_utf8(b.clone()).ok());
        match &text {
            None => parts.mark(
                "section source",
                "differs",
                "damaged: it is not UTF-8 text".into(),
            ),
            Some(t) if hex::encode(Sha256::digest(t.as_bytes())) != r.sha256 => parts.mark(
                "section source",
                "differs",
                "damaged: its text does not match the sha256 the record gives".into(),
            ),
            Some(t) => match &script {
                Some((path, s)) if s != t => {
                    let line = s
                        .lines()
                        .zip(t.lines())
                        .position(|(a, b)| a != b)
                        .unwrap_or_else(|| s.lines().count().min(t.lines().count()))
                        + 1;
                    parts.mark(
                        "section source",
                        "differs",
                        format!(
                            "{} has changed since the build, from line {line}",
                            path.display()
                        ),
                    )
                }
                Some((path, _)) => parts.mark(
                    "section source",
                    "ok",
                    format!(
                        "{} is the script it was built from ({} lines, sha256 {})",
                        path.display(),
                        t.lines().count(),
                        &r.sha256[..16]
                    ),
                ),
                None => parts.mark(
                    "section source",
                    "ok",
                    format!(
                        "{} ({} lines, sha256 {})",
                        r.file,
                        t.lines().count(),
                        &r.sha256[..16]
                    ),
                ),
            },
        }
        // The bytecode: parsed, verified and loaded; then against a fresh
        // lowering at the recorded level, of the script or of the source.
        if let Some(code) = body("bytecode") {
            let checked = cigscript::cig_bytecode::parse_module(code)
                .map_err(|e| format!("it does not read: {e}"))
                .and_then(|m| {
                    if m.source_name != r.file {
                        return Err(format!(
                            "it was lowered from `{}`, and the record says `{}`",
                            m.source_name.escape_debug(),
                            r.file
                        ));
                    }
                    cigscript::cig_bytecode::verify_module(&m)
                        .map(|_| ())
                        .map_err(|e| format!("it does not verify: {e}"))
                });
            let ours = format!("cigcompile {}", env!("CARGO_PKG_VERSION"));
            let level = match r.opt.as_str() {
                "0" => Some(0u8),
                "1" => Some(1),
                "2" => Some(2),
                _ => None,
            };
            let from = script
                .as_ref()
                .map(|(p, s)| (p.display().to_string(), s.clone()))
                .or_else(|| text.clone().map(|t| ("its source".to_string(), t)));
            match (checked, from, level) {
                (Err(why), _, _) => parts.mark("section bytecode", "differs", format!("damaged: {why}")),
                (Ok(()), _, _) if r.compiler != ours || r.cig != cigscript::VERSION => parts.mark(
                    "section bytecode",
                    "unchecked",
                    format!("it reads and verifies; built by {} for cig {}, so this {ours} does not lower it again", r.compiler, r.cig),
                ),
                (Ok(()), Some((what, s)), Some(level)) => {
                    match crate::build::bytecode_of(&s, &r.file, level) {
                        Ok(fresh) if fresh == *code => parts.mark(
                            "section bytecode",
                            "ok",
                            format!("{} bytes, verifies, and is what {what} lowers to at -O{level}", code.len()),
                        ),
                        Ok(fresh) => parts.mark(
                            "section bytecode",
                            "differs",
                            format!("not what {what} lowers to at -O{level}: {}", first_difference(&fresh, code)),
                        ),
                        Err(n) => parts.mark(
                            "section bytecode",
                            "differs",
                            format!("{what} does not lower at -O{level}: {}", n.said(&r.file)),
                        ),
                    }
                }
                (Ok(()), _, _) => parts.mark(
                    "section bytecode",
                    "unchecked",
                    format!("it reads and verifies; its level `{}` is not one this cigcompile lowers at", r.opt),
                ),
            }
        }
        for (e, b) in entries.iter().zip(&bodies) {
            if let Ok(b) = b {
                parts.mark(
                    &format!("section {}", e.id()),
                    "ok",
                    format!(
                        "{} bytes ({}, {} decoded), its sha256 holds",
                        e.length,
                        e.encoding().map_or("?", Encoding::name),
                        b.len()
                    ),
                );
            }
        }
        // The whole file, against what a build of these parts writes: the
        // first difference is named by its part.
        if !parts.any_differs() {
            let mut b = program::Build::new(
                &r.name,
                &r.file,
                text.as_deref().unwrap_or(""),
                &r.compiler,
                &r.target,
            );
            b.cig = r.cig.clone();
            b.stage = r.stage;
            b.opt = r.opt.clone();
            b.own_flags = r.own_flags;
            for (e, body) in entries.iter().zip(&bodies).skip(1) {
                b.sections.push(program::Section {
                    id: e.id().to_string(),
                    version: e.version,
                    encoding: e.encoding().unwrap_or(Encoding::Stored),
                    required: e.required(),
                    bytes: body.clone().unwrap_or_default(),
                });
            }
            let whole = &bytes[..t_at + TRAILER2_LEN as usize];
            match program::append2(runtime, &b) {
                Ok(rebuilt) if rebuilt == whole => {}
                Ok(rebuilt) => {
                    let at = rebuilt
                        .iter()
                        .zip(whole)
                        .position(|(a, b)| a != b)
                        .unwrap_or(rebuilt.len().min(whole.len()))
                        as u64;
                    let part = if at < p {
                        "runtime".to_string()
                    } else if at < p + record_len {
                        "record".to_string()
                    } else if at >= p + len {
                        "trailer".to_string()
                    } else if at >= p + table_at {
                        "section table".to_string()
                    } else {
                        entries
                            .iter()
                            .find(|e| at >= p + e.offset && at < p + e.offset + e.length)
                            .map_or("padding".to_string(), |e| format!("section {}", e.id()))
                    };
                    parts.mark(
                        &part,
                        "differs",
                        format!(
                            "not what a build of these parts writes: {}",
                            first_difference(&rebuilt, whole)
                        ),
                    );
                }
                Err(why) => parts.mark(
                    "record",
                    "differs",
                    format!("a build would refuse it: {why}"),
                ),
            }
        }
        let said = format!(
            "{}: {} for cig {}, built by {}, stage {}, -O{}, {}{}",
            r.name,
            r.file,
            r.cig,
            r.compiler,
            r.stage.as_str(),
            r.opt,
            r.target,
            if r.own_flags { ", own flags" } else { "" }
        );
        parts.mark("record", "ok", said);
        parts.mark(
            "section table",
            "ok",
            format!("{} sections: {}", entries.len(), ids.join(", ")),
        );
        if !parts.differs("padding") {
            parts.mark("padding", "ok", "zeros, as the alignment needs".into());
        }
        parts.mark(
            "trailer",
            "ok",
            if signed {
                "CIGPROG2, the head sha256 holds; signed since (a certificate table follows it)"
                    .into()
            } else {
                "CIGPROG2, the head sha256 holds".to_string()
            },
        );
        finish(parts, &ids, name.as_deref(), signed)
    }
}
