//! The fuzz targets on stable, over a fixed sample made here, so a crash
//! the nightly fuzzer would find on these inputs fails a pull request.
//! The seeds are CigScript's own scripts (when `CIGSCRIPT_SRC` names a
//! checkout) and a few written here; each is cut, flipped and spliced with
//! the language's own words, deterministically. Not a fuzzer: a sample.

use cigcompile::fuzz;

const WORDS: &[&str] = &[
    "burn",
    "unburn",
    "unlit",
    "pull",
    "pack",
    "stick",
    "roll",
    "exhale",
    "cough",
    "snuff",
    "try",
    "ashtray",
    "finally",
    "chain",
    "light",
    "if",
    "else",
    "while",
    "for",
    "in",
    "break",
    "continue",
    "{",
    "}",
    "(",
    ")",
    "[",
    "]",
    "\"",
    "'",
    "${",
    "?.",
    "??",
    "..",
    ">>",
    "=>",
    ",",
    ":",
    "\n",
    "#",
    "+=",
    "==",
    "-9223372036854775808",
    "9223372036854775807",
    "1e999",
    "0.0",
    "null",
    "fs.rm(\"x\")",
    "proc.run(\"sh\", [])",
    "args[0]",
    "pack(x) => x",
    "\u{feff}",
    "\u{202e}",
];

const OWN: &[&str] = &[
    "pull f(n) {\n  if n < 2 {\n    snuff n\n  }\n  snuff f(n - 1) + f(n - 2)\n}\nexhale f(10)\n",
    "burn (s, \"l\") {\n  fs.write_text(\"a\", \"${s}\")\n} unburn {\n  exhale s\n}\n",
    "stick m = {a: 1, [\"b\"]: [1, 2, {c: null}]}\nexhale m?.a ?? 0, m.b[2]?.c\n",
    "try {\n  cough \"x\"\n} ashtray e {\n  exhale e\n} finally {\n  exhale 1\n}\n",
    "chain c { a, b }\npull a() {\n  snuff 1\n}\npull b() {\n  snuff 2\n}\n",
];

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn seeds() -> Vec<Vec<u8>> {
    let mut v: Vec<Vec<u8>> = OWN.iter().map(|s| s.as_bytes().to_vec()).collect();
    if let Some(src) = std::env::var_os("CIGSCRIPT_SRC").map(std::path::PathBuf::from) {
        for dir in ["examples", "tests/scripts", "tests/corpus"] {
            if let Ok(rd) = std::fs::read_dir(src.join(dir)) {
                for e in rd.flatten() {
                    if e.path().extension().is_some_and(|x| x == "cig") {
                        v.push(std::fs::read(e.path()).unwrap());
                    }
                }
            }
        }
    }
    v
}

fn mutate(seed: &[u8], r: &mut Rng) -> Vec<u8> {
    let mut b = seed.to_vec();
    for _ in 0..1 + r.below(4) {
        match r.below(5) {
            0 if !b.is_empty() => b.truncate(r.below(b.len())),
            1 if !b.is_empty() => {
                let at = r.below(b.len());
                b[at] ^= 1 << r.below(8);
            }
            2 => {
                let at = r.below(b.len() + 1);
                let w = WORDS[r.below(WORDS.len())].as_bytes();
                b.splice(at..at, w.iter().copied());
            }
            3 if b.len() > 1 => {
                let (x, y) = (r.below(b.len()), r.below(b.len()));
                let (lo, hi) = (x.min(y), x.max(y));
                let piece = b[lo..hi].to_vec();
                let at = r.below(b.len() + 1);
                b.splice(at..at, piece);
            }
            _ => {
                let at = r.below(b.len() + 1);
                b.insert(at, r.below(256) as u8);
            }
        }
    }
    b
}

fn hold(name: &str, input: &[u8], target: fn(&[u8])) {
    if std::panic::catch_unwind(|| target(input)).is_err() {
        panic!(
            "fuzz target `{name}` failed on this input ({} bytes):\n{}",
            input.len(),
            String::from_utf8_lossy(input)
        );
    }
}

#[test]
fn the_targets_hold_on_a_fixed_sample() {
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    let mut n = 0;
    for seed in seeds() {
        for t in [
            ("parse", fuzz::parse as fn(&[u8])),
            ("check", fuzz::check),
            ("lower", fuzz::lower),
        ] {
            hold(t.0, &seed, t.1);
        }
        for _ in 0..60 {
            let input = mutate(&seed, &mut r);
            for t in [
                ("parse", fuzz::parse as fn(&[u8])),
                ("check", fuzz::check),
                ("lower", fuzz::lower),
            ] {
                hold(t.0, &input, t.1);
            }
            n += 1;
        }
    }
    assert!(n >= 300, "only {n} inputs");
}

#[test]
fn the_nesting_ceiling_is_a_refusal_not_a_crash() {
    for (open, close) in [("[", "]"), ("(", ")"), ("{a: ", "}")] {
        let s = format!("exhale {}1{}\n", open.repeat(6_000), close.repeat(6_000));
        hold("parse", s.as_bytes(), fuzz::parse);
        hold("lower", s.as_bytes(), fuzz::lower);
    }
    let chain = format!("exhale 1{}\n", " + 1".repeat(12_000));
    hold("lower", chain.as_bytes(), fuzz::lower);
}

#[test]
fn a_programs_trailer_holds_on_any_bytes() {
    let mut r = Rng(42);
    let good = cigscript::program::append(
        b"runtime",
        &cigscript::program::Program::new("p", "p.cig", "exhale 1\n", "t"),
    );
    for _ in 0..2_000 {
        let mut b = good.clone();
        for _ in 0..1 + r.below(3) {
            let at = r.below(b.len());
            b[at] = r.next() as u8;
        }
        if r.below(4) == 0 {
            b.truncate(r.below(b.len()));
        }
        hold("program", &b, fuzz::program);
    }
}
