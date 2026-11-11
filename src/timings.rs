// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! Where a build's time goes (`build --timings`, #37), and what it is doing
//! as it goes (`build --json-progress`, #12: the data the window reads). Off,
//! each phase costs one load of a flag.
//!
//! The same phases go to the descriptor `CIG_TIMINGS_FD` names, when it is set
//! and open (#68, otmof-ops/cig-script#179): one JSON line a phase, summed over
//! the scripts built, in the order of [`PHASES`] and the build's wall time
//! apart (the reader's `other` is what they leave):
//!
//! ```text
//! {"v":1,"product":"cigcompile","phase":"check","ms":0.412}
//! ```
//!
//! That is what `cig bench --timings` reads. The variable is removed from the
//! environment once read, so a program started from here does not write the
//! same phases again. A descriptor below 3, or one that is not open, is
//! ignored. The descriptor is reached through `/dev/fd/N`, opened for
//! appending; where there is no such path (Windows) nothing is written.

use std::fs::File;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

static TIMINGS: AtomicBool = AtomicBool::new(false);
static PROGRESS: AtomicBool = AtomicBool::new(false);
static START: OnceLock<Instant> = OnceLock::new();
static SPANS: Mutex<Vec<Span>> = Mutex::new(Vec::new());
static SINK: OnceLock<Option<Mutex<File>>> = OnceLock::new();

/// The environment variable that names the descriptor.
pub const FD_ENV: &str = "CIG_TIMINGS_FD";
/// The line format's version.
pub const VERSION: u32 = 1;
/// The name written in every line's `product` key.
pub const PRODUCT: &str = "cigcompile";

#[cfg(unix)]
fn open_fd(value: &str) -> Option<File> {
    let fd: i32 = value.trim().parse().ok()?;
    if fd < 3 {
        return None;
    }
    std::fs::OpenOptions::new()
        .append(true)
        .open(format!("/dev/fd/{fd}"))
        .ok()
}

#[cfg(not(unix))]
fn open_fd(_value: &str) -> Option<File> {
    None
}

/// Is a descriptor open for the phases?
pub fn fd_open() -> bool {
    matches!(SINK.get(), Some(Some(_)))
}

/// One line of the format.
pub fn line(phase: &str, ms: f64) -> String {
    format!("{{\"v\":{VERSION},\"product\":\"{PRODUCT}\",\"phase\":\"{phase}\",\"ms\":{ms:.3}}}\n")
}

/// Write each phase's time, summed over the scripts, to the descriptor, in
/// [`PHASES`] order: the rows `--timings` prints, one line each.
pub fn write_fd() {
    let Some(Some(sink)) = SINK.get() else { return };
    let spans = SPANS
        .lock()
        .map(|s| s.iter().map(|x| (x.phase, x.dur)).collect::<Vec<_>>())
        .unwrap_or_default();
    let Ok(mut file) = sink.lock() else { return };
    for p in PHASES {
        if spans.iter().any(|(q, _)| q == p) {
            let t: u128 = spans.iter().filter(|(q, _)| q == p).map(|(_, d)| d).sum();
            // One write a line: far under PIPE_BUF, so lines never mix.
            let _ = file.write_all(line(p, t as f64 / 1000.0).as_bytes());
        }
    }
}

/// One phase of one script's build.
pub struct Span {
    pub script: String,
    pub phase: &'static str,
    /// Microseconds since the build started.
    pub start: u128,
    pub dur: u128,
}

/// The order phases are reported in.
pub const PHASES: &[&str] = &[
    "read", "index", "check", "pin", "runtime", "assemble", "hash", "write", "sbom",
];

pub fn enable(timings: bool, progress: bool) {
    let _ = START.set(Instant::now());
    let raw = std::env::var(FD_ENV).ok();
    std::env::remove_var(FD_ENV);
    let _ = SINK.set(raw.and_then(|v| open_fd(&v)).map(Mutex::new));
    TIMINGS.store(timings || fd_open(), Ordering::Relaxed);
    PROGRESS.store(progress, Ordering::Relaxed);
}

fn since_start() -> u128 {
    START.get_or_init(Instant::now).elapsed().as_micros()
}

/// A progress event, one JSON object a line on stderr, when asked for.
pub fn progress(event: serde_json::Value) {
    if PROGRESS.load(Ordering::Relaxed) {
        let mut e = event;
        e["ms"] = serde_json::json!((since_start() / 1000) as u64);
        eprintln!("{e}");
    }
}

/// Run `f` as one phase of `script`'s build.
pub fn phase<T>(script: &str, phase: &'static str, f: impl FnOnce() -> T) -> T {
    let timing = TIMINGS.load(Ordering::Relaxed);
    if !timing && !PROGRESS.load(Ordering::Relaxed) {
        return f();
    }
    progress(serde_json::json!({"event": "phase", "script": script, "phase": phase}));
    let start = since_start();
    let t = Instant::now();
    let r = f();
    if timing {
        let dur = t.elapsed().as_micros();
        if let Ok(mut s) = SPANS.lock() {
            s.push(Span {
                script: script.to_string(),
                phase,
                start,
                dur,
            });
        }
    }
    r
}

/// The table, on stderr: each phase's time across the scripts, its share,
/// and the whole build's.
pub fn report(total_us: u128) {
    let spans = SPANS
        .lock()
        .map(|s| s.iter().map(|x| (x.phase, x.dur)).collect::<Vec<_>>())
        .unwrap_or_default();
    eprintln!(
        "timings: {} script{}",
        count_scripts(),
        if count_scripts() == 1 { "" } else { "s" }
    );
    eprintln!("  {:<9} {:>9} {:>6}", "phase", "ms", "share");
    let mut covered = 0u128;
    for p in PHASES {
        let t: u128 = spans.iter().filter(|(q, _)| q == p).map(|(_, d)| d).sum();
        if spans.iter().any(|(q, _)| q == p) {
            covered += t;
            eprintln!(
                "  {:<9} {:>9.3} {:>5.1}%",
                p,
                t as f64 / 1000.0,
                if total_us > 0 {
                    t as f64 * 100.0 / total_us as f64
                } else {
                    0.0
                }
            );
        }
    }
    eprintln!(
        "  {:<9} {:>9.3} {:>5.1}%",
        "other",
        total_us.saturating_sub(covered) as f64 / 1000.0,
        if total_us > 0 {
            total_us.saturating_sub(covered) as f64 * 100.0 / total_us as f64
        } else {
            0.0
        }
    );
    eprintln!("  {:<9} {:>9.3}", "total", total_us as f64 / 1000.0);
}

fn count_scripts() -> usize {
    SPANS
        .lock()
        .map(|s| {
            let mut v: Vec<&str> = s.iter().map(|x| x.script.as_str()).collect();
            v.sort();
            v.dedup();
            v.len()
        })
        .unwrap_or(0)
}

/// The spans as a Chrome trace (chrome://tracing, Perfetto): one complete
/// event a phase, a thread a script.
pub fn trace(total_us: u128) -> serde_json::Value {
    let spans = SPANS.lock();
    let Ok(spans) = spans else {
        return serde_json::json!({"traceEvents": []});
    };
    let mut scripts: Vec<&str> = spans.iter().map(|s| s.script.as_str()).collect();
    scripts.sort();
    scripts.dedup();
    let mut events: Vec<serde_json::Value> = spans
        .iter()
        .map(|s| {
            serde_json::json!({
                "name": s.phase,
                "cat": "build",
                "ph": "X",
                "ts": s.start as u64,
                "dur": s.dur as u64,
                "pid": 1,
                "tid": scripts.iter().position(|x| *x == s.script).unwrap_or(0) + 1,
                "args": {"script": s.script},
            })
        })
        .collect();
    events.push(serde_json::json!({"name": "build", "cat": "build", "ph": "X", "ts": 0, "dur": total_us as u64, "pid": 1, "tid": 0}));
    serde_json::json!({"traceEvents": events, "displayTimeUnit": "ms"})
}
