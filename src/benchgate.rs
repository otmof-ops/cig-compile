// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! The benchmark gates (#30): what `cigcompile bench --against` and
//! `bench --trend` decide, apart from running anything.
//!
//! - **Regression** (a pull request against `main`): a benchmark fails when
//!   its median is more than 5% slower and the difference is real, and the
//!   set fails when the geometric mean of the ratios is more than 2% slower
//!   and real. "Real" is a one-sided Mann-Whitney test at 5% over the raw
//!   samples of both sides; the CI job interleaves `main`'s and the pull
//!   request's runs round by round and pools them, so a machine that drifts
//!   drifts under both. A benchmark that is noisy on either side (CV over
//!   5%) is reported and never gated: a gate that trips on noise is
//!   ignored.
//! - **Memory:** peak resident set, and the allocation count where both
//!   sides have one (`bench --alloc`), more than 10% over `main`'s.
//! - **Trend** (nightly): the set's geometric mean slower than the night
//!   before, by more than 0.5%, five nights in a row.
//!
//! Both sides are `bench --json` files. Nothing here runs a program.

use std::path::{Path, PathBuf};

/// A row slower than this fraction of its baseline is a regression.
pub const ROW_LIMIT: f64 = 0.05;
/// The set's geometric mean slower than this is a regression.
pub const GEOMEAN_LIMIT: f64 = 0.02;
/// Peak memory or allocations over this fraction is a regression.
pub const MEMORY_LIMIT: f64 = 0.10;
/// A peak-memory difference under this many bytes is not a leak.
pub const MEMORY_FLOOR_BYTES: u64 = 1 << 20;
/// An allocation-count difference under this is not a leak.
pub const ALLOC_FLOOR: u64 = 1000;
/// The significance of the Mann-Whitney test (one-sided).
pub const ALPHA: f64 = 0.05;
/// Fewer samples than this on a side and the test is not run: the medians
/// decide alone, and the report says so.
pub const MIN_SAMPLES: usize = 5;
/// A coefficient of variation over this is noisy, and not gated.
pub const NOISY_CV: f64 = 0.05;
/// Nights in a row, each slower, that fail the trend.
pub const TREND_NIGHTS: usize = 5;
/// A night is slower than the one before when its geometric mean is over
/// this fraction up: under it is the same night.
pub const TREND_STEP: f64 = 0.005;

/// One side of one benchmark, as a `bench --json` file has it.
#[derive(Clone, Debug, Default)]
pub struct Point {
    pub median: f64,
    pub cv: f64,
    pub samples: Vec<f64>,
    pub rss: Option<u64>,
    pub allocs: Option<u64>,
}

impl Point {
    pub fn noisy(&self) -> bool {
        self.cv > NOISY_CV
    }
}

/// Every timed side in one or more result files, keyed `name (side)`.
/// Files that name the same side are pooled: the samples join, the median
/// and CV are those of the pool.
#[derive(Clone, Debug, Default)]
pub struct Results {
    pub points: Vec<(String, Point)>,
}

impl Results {
    pub fn get(&self, key: &str) -> Option<&Point> {
        self.points.iter().find(|(k, _)| k == key).map(|(_, p)| p)
    }

    /// Read result files and pool them.
    pub fn load(paths: &[PathBuf]) -> Result<Results, String> {
        let mut all = Results::default();
        for p in paths {
            let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
            let v: serde_json::Value = serde_json::from_str(text.trim())
                .map_err(|e| format!("{}: not bench --json output: {e}", p.display()))?;
            all.add(&v).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        if all.points.is_empty() {
            return Err("no timed benchmark in the results".into());
        }
        Ok(all)
    }

    pub fn add(&mut self, v: &serde_json::Value) -> Result<(), String> {
        let programs = v["programs"]
            .as_array()
            .ok_or("no \"programs\" array: not bench --json output")?;
        for prog in programs {
            let name = prog["name"].as_str().unwrap_or_default();
            let allocs = prog["allocs"]["count"].as_u64();
            for side in ["interpreted", "compiled"] {
                let s = &prog[side];
                let Some(median) = s["median_ms"].as_f64() else {
                    continue; // WRONG or absent: no time, never a row
                };
                let samples: Vec<f64> = s["samples_ms"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|x| x.as_f64()).collect())
                    .unwrap_or_default();
                let point = Point {
                    median,
                    cv: s["cv"].as_f64().unwrap_or(0.0),
                    samples,
                    rss: s["rss_bytes"].as_u64(),
                    allocs: if side == "interpreted" { allocs } else { None },
                };
                let key = format!("{name} ({side})");
                match self.points.iter_mut().find(|(k, _)| *k == key) {
                    None => self.points.push((key, point)),
                    Some((_, old)) => pool(old, point),
                }
            }
        }
        Ok(())
    }
}

/// Add a second file's side to a first's.
fn pool(old: &mut Point, new: Point) {
    if old.samples.is_empty() || new.samples.is_empty() {
        // No raw samples to pool: keep the slower median, the safer side.
        if new.median > old.median {
            *old = Point {
                rss: old.rss.max(new.rss),
                ..new
            };
        }
        return;
    }
    old.samples.extend(new.samples);
    let n = old.samples.len() as f64;
    let mean = old.samples.iter().sum::<f64>() / n;
    let var = old.samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
    let mut v = old.samples.clone();
    v.sort_by(|a, b| a.total_cmp(b));
    old.median = v[((v.len() - 1) as f64 * 0.5).round() as usize];
    old.cv = if mean > 0.0 { var.sqrt() / mean } else { 0.0 };
    old.rss = old.rss.max(new.rss);
    old.allocs = old.allocs.or(new.allocs);
}

/// What the gate says of one row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Verdict {
    Ok,
    /// Over the limit, and real: fails the run.
    Slower,
    /// Over the limit but noisy on a side, or not significant: said, not gated.
    NotGated,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub key: String,
    pub base_ms: f64,
    pub cur_ms: f64,
    /// `cur / base - 1`.
    pub change: f64,
    pub noisy: bool,
    /// The Mann-Whitney p-value; `None` when a side has too few samples.
    pub p: Option<f64>,
    pub verdict: Verdict,
    /// Peak resident set, base then current, and its verdict.
    pub rss: Option<(u64, u64)>,
    pub allocs: Option<(u64, u64)>,
    pub memory_slower: bool,
}

#[derive(Debug)]
pub struct Report {
    pub lines: Vec<Line>,
    /// The geometric mean of the ratios over the non-noisy rows, minus one.
    pub geomean_change: Option<f64>,
    pub geomean_rows: usize,
    pub geomean_p: Option<f64>,
    pub geomean_slower: bool,
    /// Rows only one side has: said, never compared.
    pub unmatched: Vec<String>,
}

impl Report {
    /// The reasons the run fails; empty when it passes.
    pub fn failures(&self) -> Vec<String> {
        let mut out = Vec::new();
        for l in &self.lines {
            if l.verdict == Verdict::Slower {
                out.push(format!(
                    "{} is {:.1}% slower than main (limit {:.0}%)",
                    l.key,
                    l.change * 100.0,
                    ROW_LIMIT * 100.0
                ));
            }
            if l.memory_slower {
                out.push(format!(
                    "{} uses more than {:.0}% over main's memory",
                    l.key,
                    MEMORY_LIMIT * 100.0
                ));
            }
        }
        if self.geomean_slower {
            out.push(format!(
                "the set's geometric mean is {:.1}% slower than main (limit {:.0}%)",
                self.geomean_change.unwrap_or(0.0) * 100.0,
                GEOMEAN_LIMIT * 100.0
            ));
        }
        out
    }
}

/// Compare a pull request's results with `main`'s.
pub fn compare(base: &Results, cur: &Results) -> Report {
    let mut lines = Vec::new();
    let mut unmatched = Vec::new();
    let (mut logs, mut norm_base, mut norm_cur) = (Vec::new(), Vec::new(), Vec::new());
    let mut every_row_has_samples = true;
    for (key, b) in &base.points {
        let Some(c) = cur.get(key) else {
            unmatched.push(format!("{key}: not in the pull request's results"));
            continue;
        };
        let change = if b.median > 0.0 {
            c.median / b.median - 1.0
        } else {
            0.0
        };
        let noisy = b.noisy() || c.noisy();
        let p = (b.samples.len() >= MIN_SAMPLES && c.samples.len() >= MIN_SAMPLES)
            .then(|| mann_whitney_greater(&c.samples, &b.samples));
        let real = p.is_none_or(|p| p < ALPHA);
        let verdict = if change <= ROW_LIMIT {
            Verdict::Ok
        } else if noisy || !real {
            Verdict::NotGated
        } else {
            Verdict::Slower
        };
        if !noisy && b.median > 0.0 && c.median > 0.0 {
            logs.push((c.median / b.median).ln());
            if p.is_some() {
                norm_base.extend(b.samples.iter().map(|x| x / b.median));
                norm_cur.extend(c.samples.iter().map(|x| x / b.median));
            } else {
                every_row_has_samples = false;
            }
        }
        let over = |a: u64, c: u64, floor: u64| {
            c > a && (c - a) > floor && (c as f64) > (a as f64) * (1.0 + MEMORY_LIMIT)
        };
        let rss = b.rss.zip(c.rss);
        let allocs = b.allocs.zip(c.allocs);
        let memory_slower = rss.is_some_and(|(a, c)| over(a, c, MEMORY_FLOOR_BYTES))
            || allocs.is_some_and(|(a, c)| over(a, c, ALLOC_FLOOR));
        lines.push(Line {
            key: key.clone(),
            base_ms: b.median,
            cur_ms: c.median,
            change,
            noisy,
            p,
            verdict,
            rss,
            allocs,
            memory_slower,
        });
    }
    for (key, _) in &cur.points {
        if base.get(key).is_none() {
            unmatched.push(format!("{key}: not in main's results"));
        }
    }
    let geomean_change =
        (!logs.is_empty()).then(|| (logs.iter().sum::<f64>() / logs.len() as f64).exp() - 1.0);
    let geomean_p = (every_row_has_samples && !norm_base.is_empty())
        .then(|| mann_whitney_greater(&norm_cur, &norm_base));
    let geomean_slower =
        geomean_change.is_some_and(|g| g > GEOMEAN_LIMIT) && geomean_p.is_none_or(|p| p < ALPHA);
    Report {
        lines,
        geomean_change,
        geomean_rows: logs.len(),
        geomean_p,
        geomean_slower,
        unmatched,
    }
}

/// The p-value of "`a` is stochastically greater than `b`": the
/// Mann-Whitney U test, normal approximation with the tie correction and
/// a continuity correction, one-sided.
pub fn mann_whitney_greater(a: &[f64], b: &[f64]) -> f64 {
    let (n1, n2) = (a.len() as f64, b.len() as f64);
    let mut all: Vec<(f64, bool)> = a
        .iter()
        .map(|&x| (x, true))
        .chain(b.iter().map(|&x| (x, false)))
        .collect();
    all.sort_by(|x, y| x.0.total_cmp(&y.0));
    let n = all.len();
    let (mut rank_sum_a, mut tie_term) = (0.0, 0.0);
    let mut i = 0;
    while i < n {
        let mut j = i;
        while j < n && all[j].0 == all[i].0 {
            j += 1;
        }
        let rank = (i + j + 1) as f64 / 2.0; // mean of ranks i+1 ..= j
        let t = (j - i) as f64;
        tie_term += t * t * t - t;
        rank_sum_a += all[i..j].iter().filter(|x| x.1).count() as f64 * rank;
        i = j;
    }
    let u = rank_sum_a - n1 * (n1 + 1.0) / 2.0;
    let mean = n1 * n2 / 2.0;
    let nn = n1 + n2;
    let var = n1 * n2 / 12.0 * ((nn + 1.0) - tie_term / (nn * (nn - 1.0)));
    if var <= 0.0 {
        return 1.0;
    }
    let z = (u - mean - 0.5) / var.sqrt();
    0.5 * erfc(z / std::f64::consts::SQRT_2)
}

/// The complementary error function (Abramowitz and Stegun 7.1.26, to
/// 1.5e-7: a p-value is not read past its third digit).
fn erfc(x: f64) -> f64 {
    let z = x.abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * z);
    let poly = t
        * (0.254_829_592
            + t * (-0.284_496_736
                + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    let r = poly * (-z * z).exp();
    if x >= 0.0 {
        r
    } else {
        2.0 - r
    }
}

fn verdict_word(l: &Line) -> &'static str {
    match (l.verdict, l.memory_slower) {
        (Verdict::Slower, _) => "SLOWER",
        (_, true) => "MORE MEMORY",
        (Verdict::NotGated, _) if l.noisy => "noisy, not gated",
        (Verdict::NotGated, _) => "not significant",
        _ => "ok",
    }
}

fn pct(x: f64) -> String {
    format!("{:+.1}%", x * 100.0)
}

fn mem_cell(m: Option<(u64, u64)>, mb: bool) -> String {
    match m {
        None => "-".into(),
        Some((a, c)) if mb => format!("{:.1} to {:.1} MB", a as f64 / 1e6, c as f64 / 1e6),
        Some((a, c)) => format!("{a} to {c}"),
    }
}

/// The report as text, or as a Markdown table for a pull request comment.
pub fn render(r: &Report, markdown: bool) -> String {
    let mut out = String::new();
    let p = |p: Option<f64>| p.map_or("-".to_string(), |p| format!("{p:.3}"));
    if markdown {
        out.push_str("**cigcompile bench against main**\n\n");
        out.push_str("| benchmark | main ms | this ms | change | p | peak RSS | allocations | verdict |\n|---|---:|---:|---:|---:|---:|---:|---|\n");
        for l in &r.lines {
            out.push_str(&format!(
                "| `{}` | {:.1} | {:.1} | {} | {} | {} | {} | {} |\n",
                l.key,
                l.base_ms,
                l.cur_ms,
                pct(l.change),
                p(l.p),
                mem_cell(l.rss, true),
                mem_cell(l.allocs, false),
                verdict_word(l)
            ));
        }
    } else {
        out.push_str(&format!(
            "{:<34} {:>9} {:>9} {:>8} {:>6}  {}\n",
            "benchmark", "main ms", "this ms", "change", "p", "verdict"
        ));
        for l in &r.lines {
            out.push_str(&format!(
                "{:<34} {:>9.1} {:>9.1} {:>8} {:>6}  {}\n",
                l.key,
                l.base_ms,
                l.cur_ms,
                pct(l.change),
                p(l.p),
                verdict_word(l)
            ));
        }
    }
    let geo = match r.geomean_change {
        Some(g) => format!(
            "geometric mean over {} non-noisy rows: {} (limit +{:.0}%), p {}: {}",
            r.geomean_rows,
            pct(g),
            GEOMEAN_LIMIT * 100.0,
            p(r.geomean_p),
            if r.geomean_slower { "SLOWER" } else { "ok" }
        ),
        None => "geometric mean: no non-noisy row to measure".to_string(),
    };
    out.push_str(&format!("\n{}{geo}\n", if markdown { "\n" } else { "" }));
    for u in &r.unmatched {
        out.push_str(&format!("not compared: {u}\n"));
    }
    out
}

/// What the nightly trend says.
#[derive(Debug)]
pub struct Trend {
    /// The night's file name and the set's geometric mean change from the
    /// night before (`None` for the first night).
    pub nights: Vec<(String, Option<f64>)>,
    /// How many of the latest nights, in a row, were each slower.
    pub streak: usize,
}

impl Trend {
    pub fn failed(&self) -> bool {
        self.streak >= TREND_NIGHTS
    }
}

/// The geometric mean change of `cur` over `prev`, on the rows neither
/// calls noisy.
fn night_change(prev: &Results, cur: &Results) -> Option<f64> {
    let logs: Vec<f64> = prev
        .points
        .iter()
        .filter(|(_, b)| !b.noisy() && b.median > 0.0)
        .filter_map(|(k, b)| {
            let c = cur.get(k)?;
            (!c.noisy() && c.median > 0.0).then(|| (c.median / b.median).ln())
        })
        .collect();
    (!logs.is_empty()).then(|| (logs.iter().sum::<f64>() / logs.len() as f64).exp() - 1.0)
}

/// Read the nights in a folder (`*.json`, in name order: name them by
/// date) and say how long the set has been getting slower.
pub fn trend(dir: &Path) -> Result<Trend, String> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(format!("{}: no night's results (*.json)", dir.display()));
    }
    let mut nights = Vec::new();
    let mut prev: Option<Results> = None;
    for f in &files {
        let cur = Results::load(std::slice::from_ref(f))?;
        let change = prev.as_ref().and_then(|p| night_change(p, &cur));
        let name = f
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        nights.push((name, change));
        prev = Some(cur);
    }
    let streak = nights
        .iter()
        .rev()
        .take_while(|(_, c)| c.is_some_and(|c| c > TREND_STEP))
        .count();
    Ok(Trend { nights, streak })
}

pub fn render_trend(t: &Trend) -> String {
    let mut out = String::from("night                      change of the geometric mean\n");
    for (name, c) in &t.nights {
        out.push_str(&format!(
            "{name:<26} {}\n",
            c.map_or("first night".to_string(), pct)
        ));
    }
    out.push_str(&format!(
        "\n{} night(s) in a row slower by more than {:.1}% (fails at {TREND_NIGHTS}): {}\n",
        t.streak,
        TREND_STEP * 100.0,
        if t.failed() { "SLOWER" } else { "ok" }
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny deterministic generator: a test with noise must repeat.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((self.0 >> 33) as f64) / ((1u64 << 31) as f64)
        }
    }

    /// `n` samples about `center`, spread by `spread` (a fraction).
    fn samples(g: &mut Lcg, center: f64, spread: f64, n: usize) -> Vec<f64> {
        (0..n)
            .map(|_| center * (1.0 + spread * (g.next() - 0.5)))
            .collect()
    }

    fn point(s: Vec<f64>) -> Point {
        let st = crate::bench::Stats::of(&s, Some(10_000_000));
        Point {
            median: st.median,
            cv: st.cv,
            samples: s,
            rss: st.rss,
            allocs: Some(100_000),
        }
    }

    fn set(rows: Vec<(&str, Point)>) -> Results {
        Results {
            points: rows
                .into_iter()
                .map(|(k, p)| (format!("{k} (interpreted)"), p))
                .collect(),
        }
    }

    /// 19 quiet benchmarks, `slow` times slower in the one named `victim`.
    fn nineteen(g: &mut Lcg, victim: &str, slow: f64) -> Results {
        let names: Vec<String> = (0..19).map(|i| format!("b{i}")).collect();
        Results {
            points: names
                .iter()
                .map(|n| {
                    let f = if n == victim { slow } else { 1.0 };
                    (
                        format!("{n} (interpreted)"),
                        point(samples(g, 100.0 * f, 0.02, 10)),
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn the_regression_gate_trips_at_five_percent() {
        let mut g = Lcg(7);
        let base = nineteen(&mut g, "none", 1.0);
        let slow = nineteen(&mut g, "b3", 1.08);
        let r = compare(&base, &slow);
        let hit = r.lines.iter().find(|l| l.key.starts_with("b3 ")).unwrap();
        assert_eq!(hit.verdict, Verdict::Slower, "{hit:?}");
        assert!(r.failures().iter().any(|f| f.contains("b3 (interpreted)")));
        // Just under the limit is not a failure, and nothing else tripped.
        let near = nineteen(&mut g, "b3", 1.03);
        let r = compare(&base, &near);
        assert!(r.failures().is_empty(), "{:?}", r.failures());
    }

    #[test]
    fn the_geomean_gate_trips_at_two_percent() {
        let mut g = Lcg(11);
        let base = nineteen(&mut g, "none", 1.0);
        // Every benchmark 3% slower: none over 5%, the set over 2%.
        let cur = Results {
            points: base
                .points
                .iter()
                .map(|(k, _)| (k.clone(), point(samples(&mut g, 103.0, 0.02, 10))))
                .collect(),
        };
        let r = compare(&base, &cur);
        assert!(r.lines.iter().all(|l| l.verdict == Verdict::Ok));
        assert!(r.geomean_slower, "{:?}", r.geomean_change);
        // 1% slower everywhere is inside the limit.
        let cur = Results {
            points: base
                .points
                .iter()
                .map(|(k, _)| (k.clone(), point(samples(&mut g, 101.0, 0.02, 10))))
                .collect(),
        };
        assert!(compare(&base, &cur).failures().is_empty());
    }

    #[test]
    fn the_regression_gate_stays_quiet_on_noise() {
        // The same program measured twice, many times, with real jitter:
        // the gate must never trip on it.
        for seed in 1..=200u64 {
            let mut g = Lcg(seed);
            let base = nineteen(&mut g, "none", 1.0);
            let cur = nineteen(&mut g, "none", 1.0);
            let r = compare(&base, &cur);
            assert!(r.failures().is_empty(), "seed {seed}: {:?}", r.failures());
        }
    }

    #[test]
    fn a_noisy_benchmark_is_not_gated() {
        let mut g = Lcg(3);
        let quiet = point(samples(&mut g, 100.0, 0.02, 10));
        // 40% jitter: a CV well over 5%; 25% slower in the median.
        let noisy_base = point(samples(&mut g, 100.0, 0.4, 10));
        let noisy_cur = point(samples(&mut g, 125.0, 0.4, 10));
        assert!(noisy_base.noisy() && noisy_cur.noisy());
        let base = set(vec![("quiet", quiet.clone()), ("noisy", noisy_base)]);
        let cur = set(vec![("quiet", quiet), ("noisy", noisy_cur)]);
        let r = compare(&base, &cur);
        let n = r.lines.iter().find(|l| l.key.starts_with("noisy")).unwrap();
        assert!(n.change > ROW_LIMIT, "the planted slowdown is there: {n:?}");
        assert_eq!(n.verdict, Verdict::NotGated, "{n:?}");
        assert!(r.failures().is_empty(), "{:?}", r.failures());
        assert_eq!(
            r.geomean_rows, 1,
            "a noisy row is not in the geometric mean"
        );
    }

    #[test]
    fn a_slowdown_that_is_not_significant_is_not_gated() {
        // Medians 6% apart, but the samples overlap completely.
        let base = set(vec![(
            "x",
            point(vec![100.0, 104.0, 96.0, 100.0, 102.0, 98.0]),
        )]);
        let mut cur = point(vec![106.0, 102.0, 99.0, 104.0, 100.0, 108.0]);
        cur.median = 106.0;
        cur.cv = 0.03;
        let r = compare(&base, &set(vec![("x", cur)]));
        assert_eq!(r.lines[0].verdict, Verdict::NotGated, "{:?}", r.lines[0]);
    }

    #[test]
    fn the_memory_gate_trips_over_ten_percent() {
        let mut g = Lcg(5);
        let mk = |rss: u64, allocs: u64, g: &mut Lcg| {
            let mut p = point(samples(g, 100.0, 0.02, 10));
            p.rss = Some(rss);
            p.allocs = Some(allocs);
            p
        };
        let base = set(vec![("m", mk(50_000_000, 200_000, &mut g))]);
        let fine = set(vec![("m", mk(54_000_000, 215_000, &mut g))]);
        assert!(compare(&base, &fine).failures().is_empty());
        let fat = set(vec![("m", mk(56_000_000, 200_000, &mut g))]);
        assert!(compare(&base, &fat).lines[0].memory_slower);
        let busy = set(vec![("m", mk(50_000_000, 230_000, &mut g))]);
        assert!(compare(&base, &busy).lines[0].memory_slower);
        // A few KB on a tiny program is not a leak.
        let tiny = set(vec![("m", mk(1_000_000, 1_000, &mut g))]);
        let tinier = set(vec![("m", mk(1_300_000, 1_500, &mut g))]);
        assert!(compare(&tiny, &tinier).failures().is_empty());
    }

    #[test]
    fn mann_whitney_separates_shifted_samples_and_not_equal_ones() {
        let a: Vec<f64> = (0..10).map(|i| 10.0 + i as f64 * 0.1).collect();
        let b: Vec<f64> = (0..10).map(|i| 20.0 + i as f64 * 0.1).collect();
        assert!(mann_whitney_greater(&b, &a) < 0.001);
        assert!(mann_whitney_greater(&a, &b) > 0.999);
        let p = mann_whitney_greater(&a, &a);
        assert!((0.4..0.6).contains(&p), "{p}");
        assert_eq!(mann_whitney_greater(&[1.0; 6], &[1.0; 6]), 1.0);
    }

    fn night(dir: &Path, name: &str, ms: f64) {
        let v = serde_json::json!({"programs": [
            {"name": "a", "interpreted": {"median_ms": ms, "cv": 0.01}},
            {"name": "b", "interpreted": {"median_ms": ms * 2.0, "cv": 0.01}}]});
        std::fs::write(dir.join(name), v.to_string()).unwrap();
    }

    #[test]
    fn the_trend_fails_after_five_slower_nights_in_a_row() {
        let d = tempfile::tempdir().unwrap();
        for (i, ms) in [100.0, 101.0, 102.5, 104.0, 105.5, 107.0]
            .iter()
            .enumerate()
        {
            night(d.path(), &format!("2026-11-0{}.json", i + 1), *ms);
        }
        let t = trend(d.path()).unwrap();
        assert_eq!(t.streak, 5, "{t:?}");
        assert!(t.failed());
        // A night that was faster ends the run.
        night(d.path(), "2026-11-07.json", 100.0);
        let t = trend(d.path()).unwrap();
        assert_eq!(t.streak, 0);
        assert!(!t.failed());
        // Four slower nights are not five.
        let d = tempfile::tempdir().unwrap();
        for (i, ms) in [100.0, 102.0, 104.0, 106.0, 108.0].iter().enumerate() {
            night(d.path(), &format!("2026-11-0{}.json", i + 1), *ms);
        }
        assert_eq!(trend(d.path()).unwrap().streak, 4);
        assert!(!trend(d.path()).unwrap().failed());
    }

    #[test]
    fn nights_that_differ_by_less_than_the_step_are_the_same_night() {
        let d = tempfile::tempdir().unwrap();
        for i in 0..8 {
            night(
                d.path(),
                &format!("2026-11-0{}.json", i + 1),
                100.0 + i as f64 * 0.1,
            );
        }
        assert_eq!(trend(d.path()).unwrap().streak, 0);
    }
}
