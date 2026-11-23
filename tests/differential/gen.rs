// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! A seeded generator of well-formed CigScript programs, for the
//! differential suite's random cases (#7): the interpreter and the program
//! must agree on every one. A program is a list of top-level statements, so
//! a difference can be shrunk by dropping them. Programs are small and end:
//! every loop is bounded, nothing recurses, and nothing reads the clock, the
//! environment or a random source. Burns write under `out/` only.

/// xorshift64*: small, fast, and the same everywhere.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }

    fn small(&mut self) -> i64 {
        self.below(41) as i64 - 20
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ty {
    Int,
    Float,
    Str,
    Bool,
    List,
    Map,
}

struct Var {
    name: String,
    ty: Ty,
    mutable: bool,
}

pub struct Gen {
    rng: Rng,
    scopes: Vec<Vec<Var>>,
    funcs: Vec<String>,
    names: u32,
}

/// The statements of program `seed`.
pub fn program(seed: u64) -> Vec<String> {
    let mut g = Gen {
        rng: Rng::new(seed),
        scopes: vec![Vec::new()],
        funcs: Vec::new(),
        names: 0,
    };
    let mut out = Vec::new();
    // Functions first (they are hoisted anyway): two ints in, an int out.
    for _ in 0..g.rng.below(3) {
        out.push(g.function());
    }
    let n = 6 + g.rng.below(14);
    for _ in 0..n {
        out.push(g.stmt(0, Ctx::default()));
    }
    out
}

#[derive(Clone, Copy, Default)]
struct Ctx {
    in_loop: bool,
    in_try: bool,
    in_burn: bool,
}

impl Gen {
    fn fresh(&mut self, prefix: &str) -> String {
        self.names += 1;
        format!("{prefix}{}", self.names)
    }

    fn var_of(&mut self, ty: Ty, mutable_only: bool) -> Option<String> {
        let found: Vec<String> = self
            .scopes
            .iter()
            .flatten()
            .filter(|v| v.ty == ty && (!mutable_only || v.mutable))
            .map(|v| v.name.clone())
            .collect();
        if found.is_empty() {
            return None;
        }
        let k = self.rng.below(found.len() as u64) as usize;
        Some(found[k].clone())
    }

    fn declare(&mut self, name: &str, ty: Ty, mutable: bool) {
        self.scopes.last_mut().expect("a scope").push(Var {
            name: name.to_string(),
            ty,
            mutable,
        });
    }

    fn word(&mut self) -> String {
        // Two of them past ASCII: upper-casing, lengths and slices of
        // multi-byte text must agree too.
        let words = [
            "ash", "ember", "smoke", "leaf", "pipe", "tin", "burn", "puff", "ásh", "煙",
        ];
        words[self.rng.below(words.len() as u64) as usize].to_string()
    }

    fn expr(&mut self, ty: Ty, depth: u32) -> String {
        let leaf = depth == 0 || self.rng.chance(35);
        match ty {
            Ty::Int => {
                if leaf {
                    if self.rng.chance(55) {
                        if let Some(v) = self.var_of(Ty::Int, false) {
                            return v;
                        }
                    }
                    // Now and then an edge of i64, so `+`, `-` and `*` meet
                    // E503 on both sides.
                    if self.rng.chance(2) {
                        return if self.rng.chance(50) {
                            "9223372036854775807".to_string()
                        } else {
                            "(-9223372036854775807 - 1)".to_string()
                        };
                    }
                    return self.rng.small().to_string();
                }
                let d = depth - 1;
                match self.rng.below(7) {
                    0 => format!("({} + {})", self.expr(Ty::Int, d), self.expr(Ty::Int, d)),
                    1 => format!("({} - {})", self.expr(Ty::Int, d), self.expr(Ty::Int, d)),
                    2 => format!("({} * {})", self.expr(Ty::Int, d), self.rng.below(5)),
                    3 => format!("({} % {})", self.expr(Ty::Int, d), 1 + self.rng.below(9)),
                    4 => format!("len({})", self.expr(Ty::List, d)),
                    5 => format!("{}.len()", self.expr(Ty::Str, d)),
                    _ => {
                        if self.funcs.is_empty() {
                            self.expr(Ty::Int, d)
                        } else {
                            let k = self.rng.below(self.funcs.len() as u64) as usize;
                            let f = self.funcs[k].clone();
                            format!("{f}({}, {})", self.expr(Ty::Int, d), self.expr(Ty::Int, d))
                        }
                    }
                }
            }
            Ty::Float => {
                if leaf {
                    if self.rng.chance(45) {
                        if let Some(v) = self.var_of(Ty::Float, false) {
                            return v;
                        }
                    }
                    let lits = ["0.1", "0.2", "1.5", "-2.75", "1e3", "2.5e-3", "-0.0", "3.0"];
                    return lits[self.rng.below(lits.len() as u64) as usize].to_string();
                }
                let d = depth - 1;
                match self.rng.below(6) {
                    0 => format!(
                        "({} + {})",
                        self.expr(Ty::Float, d),
                        self.expr(Ty::Float, d)
                    ),
                    1 => format!("({} - {})", self.expr(Ty::Float, d), self.expr(Ty::Int, d)),
                    2 => format!(
                        "({} * {})",
                        self.expr(Ty::Float, d),
                        self.expr(Ty::Float, d)
                    ),
                    // Division by a constant that is never zero.
                    3 => format!("({} / 2.5)", self.expr(Ty::Float, d)),
                    4 => format!("({} / {})", self.expr(Ty::Int, d), 1 + self.rng.below(7)),
                    _ => format!("({} + 0.5)", self.expr(Ty::Int, d)),
                }
            }
            Ty::Str => {
                if leaf {
                    if self.rng.chance(50) {
                        if let Some(v) = self.var_of(Ty::Str, false) {
                            return v;
                        }
                    }
                    let w = self.word();
                    return if self.rng.chance(20) {
                        format!("'{w}'")
                    } else {
                        format!("\"{w}\"")
                    };
                }
                let d = depth - 1;
                match self.rng.below(7) {
                    0 => format!("({} + {})", self.expr(Ty::Str, d), self.expr(Ty::Str, d)),
                    1 => format!("\"{}-${{{}}}\"", self.word(), self.expr(Ty::Int, d)),
                    2 => format!("str({})", self.expr(Ty::Int, d)),
                    3 => format!("{}.upper()", self.expr(Ty::Str, d)),
                    4 => format!("{}.repeat({})", self.expr(Ty::Str, d), self.rng.below(3)),
                    5 => {
                        let w = self.word();
                        format!("{}.replace(\"{w}\", \"-\")", self.expr(Ty::Str, d))
                    }
                    _ if self.rng.chance(50) => format!("str({})", self.expr(Ty::Float, d)),
                    _ => format!("str({})", self.expr(Ty::List, d)),
                }
            }
            Ty::Bool => {
                if leaf {
                    return if self.rng.chance(50) { "true" } else { "false" }.to_string();
                }
                let d = depth - 1;
                match self.rng.below(7) {
                    0 => format!("{} < {}", self.expr(Ty::Int, d), self.expr(Ty::Int, d)),
                    6 => format!("{} < {}", self.expr(Ty::Float, d), self.expr(Ty::Float, d)),
                    1 => format!("{} == {}", self.expr(Ty::Int, d), self.expr(Ty::Int, d)),
                    2 => {
                        let w = self.word();
                        format!("{}.contains(\"{w}\")", self.expr(Ty::Str, d))
                    }
                    3 => format!("not ({})", self.expr(Ty::Bool, d)),
                    4 => format!(
                        "({}) and ({})",
                        self.expr(Ty::Bool, d),
                        self.expr(Ty::Bool, d)
                    ),
                    _ => format!(
                        "({}) or ({})",
                        self.expr(Ty::Bool, d),
                        self.expr(Ty::Bool, d)
                    ),
                }
            }
            Ty::List => {
                if leaf {
                    if self.rng.chance(50) {
                        if let Some(v) = self.var_of(Ty::List, false) {
                            return v;
                        }
                    }
                    // In parentheses: `..` binds looser than `+`.
                    return format!("(0..{})", self.rng.below(5));
                }
                let d = depth - 1;
                match self.rng.below(5) {
                    0 => {
                        let n = self.rng.below(4);
                        let items: Vec<String> = (0..n).map(|_| self.expr(Ty::Int, d)).collect();
                        format!("[{}]", items.join(", "))
                    }
                    1 => format!("({} + {})", self.expr(Ty::List, d), self.expr(Ty::List, d)),
                    2 => format!("{}.sort()", self.expr(Ty::List, d)),
                    3 => format!("{}.reverse()", self.expr(Ty::List, d)),
                    _ => format!(
                        "{}.filter(pack(x) => x > {})",
                        self.expr(Ty::List, d),
                        self.rng.small()
                    ),
                }
            }
            Ty::Map => {
                if leaf {
                    if let Some(v) = self.var_of(Ty::Map, false) {
                        return v;
                    }
                }
                let d = depth.saturating_sub(1);
                let a = self.expr(Ty::Int, d);
                let b = self.expr(Ty::Str, d);
                format!("{{n: {a}, s: {b}}}")
            }
        }
    }

    fn any_ty(&mut self) -> Ty {
        [
            Ty::Int,
            Ty::Int,
            Ty::Float,
            Ty::Str,
            Ty::Str,
            Ty::Bool,
            Ty::List,
            Ty::Map,
        ][self.rng.below(8) as usize]
    }

    fn block(&mut self, depth: u32, ctx: Ctx, extra: &[(String, Ty)]) -> String {
        self.scopes.push(Vec::new());
        for (name, ty) in extra {
            self.declare(name, *ty, false);
        }
        let n = 1 + self.rng.below(3);
        let body: Vec<String> = (0..n).map(|_| self.stmt(depth + 1, ctx)).collect();
        self.scopes.pop();
        body.join("\n")
    }

    fn function(&mut self) -> String {
        let name = self.fresh("f");
        self.scopes.push(Vec::new());
        self.declare("a", Ty::Int, false);
        self.declare("b", Ty::Int, false);
        let n = self.rng.below(3);
        let body: Vec<String> = (0..n)
            .map(|_| {
                let v = self.fresh("t");
                let e = self.expr(Ty::Int, 2);
                self.declare(&v, Ty::Int, true);
                format!("roll {v} = {e}")
            })
            .collect();
        let ret = self.expr(Ty::Int, 3);
        self.scopes.pop();
        self.funcs.push(name.clone());
        // Now and then the value leaves through a `finally`.
        let tail = if self.rng.chance(25) {
            format!("try {{\nsnuff {ret}\n}} finally {{\nexhale \"{name} done\"\n}}")
        } else {
            format!("snuff {ret}")
        };
        format!(
            "pull {name}(a, b) {{\n{}{}{tail}\n}}",
            body.join("\n"),
            if body.is_empty() { "" } else { "\n" }
        )
    }

    fn stmt(&mut self, depth: u32, ctx: Ctx) -> String {
        let nested = depth < 3;
        let pick = self.rng.below(if nested { 18 } else { 8 });
        match pick {
            0 | 1 => {
                let ty = self.any_ty();
                let mutable = self.rng.chance(70);
                let name = self.fresh("v");
                let e = self.expr(ty, 3);
                self.declare(&name, ty, mutable);
                format!("{} {name} = {e}", if mutable { "roll" } else { "stick" })
            }
            2 => {
                let ty = self.any_ty();
                match self.var_of(ty, true) {
                    Some(v) if ty == Ty::Int && self.rng.chance(50) => {
                        format!("{v} += {}", self.expr(Ty::Int, 2))
                    }
                    Some(v) if ty == Ty::Str && self.rng.chance(50) => {
                        format!("{v} += {}", self.expr(Ty::Str, 2))
                    }
                    Some(v) if ty == Ty::List && self.rng.chance(50) => {
                        format!("{v}.push({})", self.expr(Ty::Int, 2))
                    }
                    Some(v) => format!("{v} = {}", self.expr(ty, 3)),
                    None => format!("exhale {}", self.expr(Ty::Str, 2)),
                }
            }
            3..=5 => {
                let ty = self.any_ty();
                let ty2 = self.any_ty();
                format!("exhale {}, {}", self.expr(ty, 3), self.expr(ty2, 2))
            }
            6 if ctx.in_loop => if self.rng.chance(50) {
                "break"
            } else {
                "continue"
            }
            .to_string(),
            7 if ctx.in_try => format!("cough \"{}\"", self.word()),
            8 => {
                let c = self.expr(Ty::Bool, 2);
                let then = self.block(depth, ctx, &[]);
                if self.rng.chance(50) {
                    let other = self.block(depth, ctx, &[]);
                    format!("if {c} {{\n{then}\n}} else {{\n{other}\n}}")
                } else {
                    format!("if {c} {{\n{then}\n}}")
                }
            }
            9 => {
                let i = self.fresh("i");
                let k = self.rng.below(5);
                let body = self.block(
                    depth,
                    Ctx {
                        in_loop: true,
                        ..ctx
                    },
                    &[(i.clone(), Ty::Int)],
                );
                format!("for {i} in 0..{k} {{\n{body}\n}}")
            }
            10 => {
                let w = self.fresh("w");
                let k = 1 + self.rng.below(4);
                // Read in the body, never assigned there: a body that set
                // its own counter back would never end.
                self.declare(&w, Ty::Int, false);
                let body = self.block(
                    depth,
                    Ctx {
                        in_loop: true,
                        ..ctx
                    },
                    &[],
                );
                format!("roll {w} = 0\nwhile {w} < {k} {{\n{w} += 1\n{body}\n}}")
            }
            11 => {
                let e = self.fresh("e");
                let body = self.block(
                    depth,
                    Ctx {
                        in_try: true,
                        ..ctx
                    },
                    &[],
                );
                let caught = format!("exhale \"caught\", {e}.message");
                let fin = if self.rng.chance(40) {
                    "\n} finally {\nexhale \"finally\""
                } else {
                    ""
                };
                format!("try {{\n{body}\n}} ashtray {e} {{\n{caught}{fin}\n}}")
            }
            12 | 13 if !ctx.in_burn => {
                let body = self.burn_body();
                let named = if self.rng.chance(30) {
                    format!(" (\"{}\")", self.word())
                } else {
                    String::new()
                };
                format!("burn{named} {{\n{body}\n}}")
            }
            14 => {
                let g = self.fresh("g");
                let k = self.rng.small();
                format!(
                    "stick {g} = pack(x) => x + {k}\nexhale {} >> {g}",
                    self.expr(Ty::Int, 2)
                )
            }
            // Closures made in a loop: each keeps its own turn's value.
            16 => {
                let cl = self.fresh("cl");
                let i = self.fresh("i");
                let k = 1 + self.rng.below(4);
                let m = self.rng.small();
                format!(
                    "roll {cl} = []\nfor {i} in 0..{k} {{\n{cl}.push(pack() => {i} * {m})\n}}\nexhale {cl}.map(pack(f) => f())"
                )
            }
            // A loop whose body leaves through a `finally`: break and
            // continue run it on the way out.
            17 => {
                let i = self.fresh("i");
                let k = 1 + self.rng.below(4);
                let body = self.block(
                    depth,
                    Ctx {
                        in_loop: true,
                        in_try: true,
                        ..ctx
                    },
                    &[(i.clone(), Ty::Int)],
                );
                format!("for {i} in 0..{k} {{\ntry {{\n{body}\n}} ashtray {{\nexhale \"caught\"\n}} finally {{\nexhale \"turn\", {i}\n}}\n}}")
            }
            _ => format!("exhale {}", self.expr(Ty::Str, 3)),
        }
    }

    fn burn_body(&mut self) -> String {
        let n = 1 + self.rng.below(4);
        let mut lines = Vec::new();
        for _ in 0..n {
            let f = self.fresh("f");
            let text = self.expr(Ty::Str, 2);
            lines.push(match self.rng.below(4) {
                0 => format!("fs.mkdir(\"out/{f}\")"),
                1 => format!("fs.append_text(\"out/{f}.txt\", {text})"),
                _ => format!("fs.write_text(\"out/{f}.txt\", {text})"),
            });
        }
        // Now and then the burn fails, and both sides roll it back.
        if self.rng.chance(15) {
            lines.push(format!("cough \"{}\"", self.word()));
        }
        lines.join("\n")
    }
}

/// Programs written by hand for the edges a seed rarely reaches: float
/// bits as they print (`0.1 + 0.2`, `-0.0`, infinities, NaN, subnormals),
/// i64's edges and the overflows each operator meets (E503), and text past
/// ASCII. Both sides run each one, as the random programs are run.
pub fn edges() -> Vec<(&'static str, String)> {
    vec![
        (
            "floats",
            "exhale 0.1 + 0.2, 1.0 / 3.0, 2.5 * 4\nexhale -0.0, 0.0 == -0.0, 1e308 * 10, -(1e308 * 10)\nexhale (1e308 * 10) - (1e308 * 10), 1e-320, 2.5e-5\nexhale 7 / 2, 7.0 / 2, 7 % 3, -7 % 3, -7 / 2\nexhale str(1.0), str(0.1 + 0.2), [0.5, -0.0, 3.0]\n".to_string(),
        ),
        (
            "int-edges",
            [
                "stick big = 9223372036854775807",
                "stick small = -big - 1",
                "exhale big, small",
                "for case in [\"add\", \"sub\", \"mul\", \"div\", \"mod\", \"neg\", \"zero\", \"zmod\"] {",
                "  try {",
                "    if case == \"add\" { exhale big + 1 }",
                "    if case == \"sub\" { exhale small - 1 }",
                "    if case == \"mul\" { exhale big * 2 }",
                "    if case == \"div\" { exhale small / -1 }",
                "    if case == \"mod\" { exhale small % -1 }",
                "    if case == \"neg\" { exhale -small }",
                "    if case == \"zero\" { exhale 1 / 0 }",
                "    if case == \"zmod\" { exhale 5 % 0 }",
                "  } ashtray e {",
                "    exhale case, e.message",
                "  }",
                "}",
            ]
            .join("\n")
                + "\n",
        ),
        (
            "text",
            "stick s = \"ásh 煙 🚬 Straße\"\nexhale s.len(), s.upper(), s.lower(), s.chars().len()\nexhale s.replace(\"煙\", \"smoke\"), s.contains(\"🚬\"), s.split(\" \")\n".to_string(),
        ),
    ]
}

/// The typed generator (#39): a program of functions whose every value
/// is an int, a float, a bool or null, and a top level that calls each
/// with several rows of argument types and prints the result, or the
/// caught error's kind, message and line. These are the functions stage
/// `c` runs as native code (`docs/decisions/0005`); the suite holds
/// every stage it runs to the tree-walker on each program made here.
///
/// A program ends: there are no `while true`, every `for` is bounded by
/// a small count, and every call carries a decreasing `fuel` so
/// recursion stops. Constants are drawn from the edges of i64 and from
/// the edges of `f64` (`0`, `1`, `-1`, `i64::MAX`, `i64::MIN`, `0.0`,
/// `-0.0`, `1e308`, `1e-300`) as often as from the middle. Deterministic
/// from `seed`.
pub fn typed_program(seed: u64) -> Vec<String> {
    let mut g = TypedGen {
        rng: Rng::new(seed ^ 0xA5A5_5A5A_5A5A_A5A5),
        funcs: Vec::new(),
        names: 0,
    };
    let n = 1 + g.rng.below(4);
    let mut out = Vec::new();
    for _ in 0..n {
        out.push(g.function());
    }
    // Every function is called from the top level with several rows,
    // each inside a try so an overflow or an ordered NaN is caught and
    // printed instead of ending the program.
    for i in 0..g.funcs.len() {
        let name = g.funcs[i].name.clone();
        let arity = g.funcs[i].arity;
        let rows = 2 + g.rng.below(3);
        for _ in 0..rows {
            let args: Vec<String> = (0..arity).map(|_| g.edge_arg()).collect();
            let call = format!("{name}({})", args.join(", "));
            out.push(format!(
                "try {{\n  exhale \"{name}\", {call}\n}} ashtray e {{\n  exhale \"{name}\", \"err\", e.kind, e.message, e.line\n}}"
            ));
        }
    }
    out
}

/// The hand-written edge programs for the typed mode, which a seed
/// rarely reaches: a NaN as a condition, an int beside the float it
/// rounds to at 2^53, and `-0.0` beside `0.0`. Each program's values
/// stay int, float, bool or null.
pub fn typed_edges() -> Vec<(&'static str, String)> {
    vec![
        (
            "nan-as-condition",
            [
                "pull sub_f(a, b) { snuff a - b }",
                "pull big_mul(a, b) { snuff a * b }",
                "stick inf = big_mul(1e308, 10.0)",
                "stick nan = sub_f(inf, inf)",
                "if nan { exhale \"nan is truthy\" } else { exhale \"nan is falsy\" }",
                "exhale not nan, nan == nan, nan != nan, nan ?? 7",
            ]
            .join("\n")
                + "\n",
        ),
        (
            "two-pow-53",
            [
                "pull eq(a, b) { snuff a == b }",
                "stick big_int = 9007199254740993",
                "stick big_flt = 9007199254740992.0",
                "exhale eq(big_int, big_flt), eq(big_int - 1, big_flt)",
            ]
            .join("\n")
                + "\n",
        ),
        (
            "neg-zero",
            [
                "pull add(a, b) { snuff a + b }",
                "exhale add(0.0, -0.0), add(-0.0, -0.0), add(-0.0, 0.0)",
                "exhale 0.0 == -0.0, -0.0 < 0.0",
            ]
            .join("\n")
                + "\n",
        ),
    ]
}

/// Per-function state for the typed generator: the name and how many
/// int-or-float parameters it takes.
struct TypedFn {
    name: String,
    arity: u64,
}

struct TypedGen {
    rng: Rng,
    funcs: Vec<TypedFn>,
    names: u32,
}

impl TypedGen {
    fn fresh(&mut self, prefix: &str) -> String {
        self.names += 1;
        format!("{prefix}{}", self.names)
    }

    /// One constant from i64's and f64's edges as often as from the
    /// middle. An integer is unparenthesized when it is non-negative
    /// and parenthesized `(-N)` when it is negative, so a call site
    /// like `f(-7, 1)` always parses.
    fn edge_arg(&mut self) -> String {
        match self.rng.below(16) {
            // Integers from the edges.
            0 => "9223372036854775807".to_string(),
            1 => "(-9223372036854775807 - 1)".to_string(),
            2 => "0".to_string(),
            3 => "1".to_string(),
            4 => "(-1)".to_string(),
            // Floats from the edges.
            5 => "0.0".to_string(),
            6 => "(-0.0)".to_string(),
            7 => "1.0".to_string(),
            8 => "(-1.0)".to_string(),
            9 => "1e308".to_string(),
            10 => "1e-300".to_string(),
            11 => "(-1e308)".to_string(),
            // Smaller mid values: integers.
            12 | 13 => {
                let n = self.rng.small();
                if n < 0 {
                    format!("({n})")
                } else {
                    n.to_string()
                }
            }
            // Smaller mid values: floats.
            _ => {
                let lits = ["0.5", "2.5", "(-3.25)", "0.1", "(-0.5)"];
                lits[self.rng.below(lits.len() as u64) as usize].to_string()
            }
        }
    }

    /// A function of two kinds: a non-recursive body of a few int
    /// statements and a typed `snuff`, or a bounded recursion of the
    /// form `if fuel <= 0 { snuff 0 } snuff self(a..., fuel-1) + k`.
    fn function(&mut self) -> String {
        let name = self.fresh("tf");
        let arity = 1 + self.rng.below(4);
        let params: Vec<String> = (0..arity).map(|i| format!("p{i}")).collect();
        let recursive = !self.funcs.is_empty() && self.rng.chance(35);
        let body = if recursive {
            self.recursive_body(&name, &params)
        } else {
            self.straight_body(&params)
        };
        self.funcs.push(TypedFn {
            name: name.clone(),
            arity,
        });
        format!("pull {name}({}) {{\n{body}\n}}", params.join(", "))
    }

    /// `fuel` is the last param: when zero or less, the function
    /// returns an int from its own name. Otherwise it calls itself or
    /// another typed function, with `fuel - 1`. Both sides end.
    fn recursive_body(&mut self, name: &str, params: &[String]) -> String {
        let fuel = params.last().cloned().unwrap_or_else(|| "p0".to_string());
        let inner = if self.funcs.len() >= 2 && self.rng.chance(50) {
            // Call another generated function.
            let k = self.rng.below(self.funcs.len() as u64) as usize;
            let other = self.funcs[k].name.clone();
            let other_arity = self.funcs[k].arity;
            let args: Vec<String> = (0..other_arity).map(|_| self.leaf_int(params)).collect();
            format!("{other}({})", args.join(", "))
        } else {
            // Self call with `fuel - 1` in the fuel slot.
            let args: Vec<String> = params
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    if i + 1 == params.len() {
                        format!("{p} - 1")
                    } else {
                        p.clone()
                    }
                })
                .collect();
            format!("{name}({})", args.join(", "))
        };
        let step = self.leaf_int(params);
        format!(
            "  if {fuel} <= 0 {{\n    snuff 0\n  }}\n  snuff {inner} + {step}",
            fuel = fuel,
            inner = inner,
            step = step,
        )
    }

    /// A straight-line body: a few locals, a conditional, an optional
    /// bounded loop, and a `snuff` of an int or a float.
    fn straight_body(&mut self, params: &[String]) -> String {
        let mut lines = Vec::new();
        let locals = self.rng.below(3);
        let mut locs: Vec<String> = Vec::new();
        for _ in 0..locals {
            let v = self.fresh("l");
            let e = self.int_expr(params, &locs, 2);
            locs.push(v.clone());
            lines.push(format!("  roll {v} = {e}"));
        }
        if self.rng.chance(45) {
            let bound = 1 + self.rng.below(6);
            let i = self.fresh("i");
            let inner = self.int_expr(params, &locs, 1);
            let acc = self.fresh("acc");
            lines.push(format!("  roll {acc} = 0"));
            lines.push(format!("  for {i} in 0..{bound} {{"));
            lines.push(format!("    {acc} += {inner} + {i}"));
            lines.push("  }".to_string());
            locs.push(acc);
        }
        // The returned expression, by the function's declared type.
        let ret = match self.rng.below(5) {
            0 => format!("snuff {}", self.bool_expr(params, &locs, 2)),
            1 | 2 => format!("snuff {}", self.float_expr(params, &locs, 2)),
            _ => format!("snuff {}", self.int_expr(params, &locs, 2)),
        };
        if self.rng.chance(40) {
            let cond = self.bool_expr(params, &locs, 1);
            lines.push(format!("  if {cond} {{"));
            lines.push(format!("    {}", ret));
            lines.push("  }".to_string());
            lines.push(format!("  {}", ret));
        } else {
            lines.push(format!("  {}", ret));
        }
        lines.join("\n")
    }

    /// A leaf int expression: a parameter, a local, or an edge literal.
    fn leaf_int(&mut self, params: &[String]) -> String {
        if !params.is_empty() && self.rng.chance(60) {
            let k = self.rng.below(params.len() as u64) as usize;
            return params[k].clone();
        }
        if self.rng.chance(20) {
            return self.edge_arg();
        }
        let n = self.rng.small();
        if n < 0 {
            format!("({n})")
        } else {
            n.to_string()
        }
    }

    fn pick_local(&mut self, locs: &[String]) -> Option<String> {
        if locs.is_empty() {
            return None;
        }
        let k = self.rng.below(locs.len() as u64) as usize;
        Some(locs[k].clone())
    }

    fn int_expr(&mut self, params: &[String], locs: &[String], depth: u32) -> String {
        if depth == 0 || self.rng.chance(45) {
            if let Some(l) = self.pick_local(locs) {
                if self.rng.chance(40) {
                    return l;
                }
            }
            return self.leaf_int(params);
        }
        let d = depth - 1;
        match self.rng.below(7) {
            0 => format!(
                "({} + {})",
                self.int_expr(params, locs, d),
                self.int_expr(params, locs, d)
            ),
            1 => format!(
                "({} - {})",
                self.int_expr(params, locs, d),
                self.int_expr(params, locs, d)
            ),
            2 => format!(
                "({} * {})",
                self.int_expr(params, locs, d),
                self.int_expr(params, locs, d)
            ),
            3 => format!(
                "({} % {})",
                self.int_expr(params, locs, d),
                1 + self.rng.below(7)
            ),
            4 => format!("(-{})", self.int_expr(params, locs, d)),
            _ => self.leaf_int(params),
        }
    }

    fn float_expr(&mut self, params: &[String], locs: &[String], depth: u32) -> String {
        if depth == 0 || self.rng.chance(50) {
            let lits = ["0.1", "0.5", "1.0", "(-0.0)", "0.0", "1e308", "1e-300"];
            return lits[self.rng.below(lits.len() as u64) as usize].to_string();
        }
        let d = depth - 1;
        match self.rng.below(5) {
            0 => format!(
                "({} + {})",
                self.float_expr(params, locs, d),
                self.float_expr(params, locs, d)
            ),
            1 => format!(
                "({} - {})",
                self.float_expr(params, locs, d),
                self.float_expr(params, locs, d)
            ),
            2 => format!(
                "({} * {})",
                self.float_expr(params, locs, d),
                self.float_expr(params, locs, d)
            ),
            3 => format!(
                "({} / {})",
                self.float_expr(params, locs, d),
                1 + self.rng.below(5)
            ),
            // An int converted to a float through a mixed op.
            _ => format!(
                "({} + {})",
                self.int_expr(params, locs, d),
                self.float_expr(params, locs, d)
            ),
        }
    }

    fn bool_expr(&mut self, params: &[String], locs: &[String], depth: u32) -> String {
        if depth == 0 {
            return if self.rng.chance(50) { "true" } else { "false" }.to_string();
        }
        let d = depth - 1;
        match self.rng.below(6) {
            0 => format!(
                "{} < {}",
                self.int_expr(params, locs, d),
                self.int_expr(params, locs, d)
            ),
            1 => format!(
                "{} == {}",
                self.int_expr(params, locs, d),
                self.int_expr(params, locs, d)
            ),
            2 => format!(
                "{} >= {}",
                self.int_expr(params, locs, d),
                self.int_expr(params, locs, d)
            ),
            3 => format!("not ({})", self.bool_expr(params, locs, d)),
            4 => format!(
                "({}) and ({})",
                self.bool_expr(params, locs, d),
                self.bool_expr(params, locs, d)
            ),
            _ => format!(
                "({}) or ({})",
                self.bool_expr(params, locs, d),
                self.bool_expr(params, locs, d)
            ),
        }
    }
}
