// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
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
        let words = [
            "ash", "ember", "smoke", "leaf", "pipe", "tin", "burn", "puff",
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
                    _ => format!("str({})", self.expr(Ty::List, d)),
                }
            }
            Ty::Bool => {
                if leaf {
                    return if self.rng.chance(50) { "true" } else { "false" }.to_string();
                }
                let d = depth - 1;
                match self.rng.below(6) {
                    0 => format!("{} < {}", self.expr(Ty::Int, d), self.expr(Ty::Int, d)),
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
            Ty::Str,
            Ty::Str,
            Ty::Bool,
            Ty::List,
            Ty::Map,
        ][self.rng.below(7) as usize]
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
        format!(
            "pull {name}(a, b) {{\n{}{}snuff {ret}\n}}",
            body.join("\n"),
            if body.is_empty() { "" } else { "\n" }
        )
    }

    fn stmt(&mut self, depth: u32, ctx: Ctx) -> String {
        let nested = depth < 3;
        let pick = self.rng.below(if nested { 16 } else { 8 });
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
                self.declare(&w, Ty::Int, true);
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
