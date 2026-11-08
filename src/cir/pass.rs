// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! Passes over CIR. Each runs between two checks: the structural verifier,
//! and the transition check against a snapshot taken before it, so a pass
//! that moves an effect out of its region, reorders effects or drops one
//! is caught where it happens.

use super::ir::*;
use super::verify::{errors, verify, verify_transition, Trace, VerifyError};
use cigscript::syntax::ast::{BinOp, UnOp};
use cigscript::syntax::span::Span;
use std::collections::{HashMap, HashSet};

pub trait Pass {
    fn name(&self) -> &'static str;
    fn run(&self, m: &mut Module);

    /// A variant that records what the pass did or left alone, for
    /// `--remarks`. The default calls `run` and records nothing: a pass
    /// that has remarks overrides this. The cost is paid only by the
    /// caller that asked for remarks; `run` itself is unchanged.
    fn run_remarking(&self, m: &mut Module, _remarks: &mut Vec<Remark>) {
        self.run(m);
    }
}

/// One thing a pass did or left alone: shown by `--remarks` on stderr.
#[derive(Clone, Debug)]
pub struct Remark {
    pub pass: &'static str,
    pub message: String,
    pub span: Span,
}

#[derive(Debug)]
pub struct PassError {
    /// `lowering`, or the pass that broke a rule.
    pub pass: &'static str,
    pub errors: Vec<VerifyError>,
}

/// Verify, then run each pass between the two checks. The warnings
/// (`V6`) are returned; any error stops the pipeline at the pass that
/// caused it.
pub fn run_passes(m: &mut Module, passes: &[&dyn Pass]) -> Result<Vec<VerifyError>, PassError> {
    run_passes_inner(m, passes, None)
}

/// `run_passes`, and alongside it a remark for each pass: one summary and
/// one line per thing that changed (or a `nothing to do` line when the
/// pass left the IR as it found it). The cost is paid only here; the
/// plain `run_passes` keeps its old shape.
pub fn run_passes_with_remarks(
    m: &mut Module,
    passes: &[&dyn Pass],
) -> Result<(Vec<VerifyError>, Vec<Remark>), PassError> {
    let mut remarks = Vec::new();
    let warnings = run_passes_inner(m, passes, Some(&mut remarks))?;
    Ok((warnings, remarks))
}

fn run_passes_inner(
    m: &mut Module,
    passes: &[&dyn Pass],
    mut remarks: Option<&mut Vec<Remark>>,
) -> Result<Vec<VerifyError>, PassError> {
    let found = verify(m);
    if !errors(&found).is_empty() {
        return Err(PassError {
            pass: "lowering",
            errors: found,
        });
    }
    let mut warnings: Vec<VerifyError> = found;
    for p in passes {
        let before = Trace::of(m);
        match &mut remarks {
            Some(sink) => {
                let start = sink.len();
                p.run_remarking(m, sink);
                if sink.len() == start {
                    sink.push(Remark {
                        pass: p.name(),
                        message: format!("{}: nothing to do", p.name()),
                        span: Span::default(),
                    });
                }
            }
            None => p.run(m),
        }
        let mut found = verify(m);
        found.extend(verify_transition(&before, m));
        if !errors(&found).is_empty() {
            return Err(PassError {
                pass: p.name(),
                errors: found,
            });
        }
        warnings = found;
    }
    Ok(warnings)
}

/// The passes `--emit=cir` runs: `-O1`.
pub fn default_passes() -> Vec<&'static dyn Pass> {
    level(1)
}

/// Every pass `--passes` can name, in the order the levels run them.
pub const NAMES: &[&str] = &["fold-consts", "dead-pure"];

/// A pass by the name `--passes` gives it.
pub fn by_name(name: &str) -> Option<&'static dyn Pass> {
    match name {
        "fold-consts" => Some(&FoldConsts),
        "dead-pure" => Some(&DeadPure),
        _ => None,
    }
}

/// The passes an optimisation level runs. `-O0` is lowering only; `-O1`
/// and `-O2` run every pass there is: stage B's passes (sccp, gvn, licm,
/// inlining) are what will set `-O2` apart (#33).
pub fn level(n: u8) -> Vec<&'static dyn Pass> {
    match n {
        0 => vec![],
        _ => vec![&FoldConsts, &DeadPure],
    }
}

/// Folds a unary or binary operator over constants, only where the
/// interpreter cannot fail: integer `+ - *` that do not overflow,
/// comparisons of like constants, string `+`, `not`. Never `/`, `%`, `..`
/// or mixed numbers. The folded instruction keeps its id and its span.
pub struct FoldConsts;

impl Pass for FoldConsts {
    fn name(&self) -> &'static str {
        "fold-consts"
    }

    fn run(&self, m: &mut Module) {
        for f in &mut m.funcs {
            let mut known: HashMap<ValueId, Const> = HashMap::new();
            fold_block(&mut f.body, &mut known, &mut f.vals);
        }
    }

    fn run_remarking(&self, m: &mut Module, remarks: &mut Vec<Remark>) {
        let before = snapshot(m);
        self.run(m);
        let mut folded = 0usize;
        let mut details: Vec<Remark> = Vec::new();
        for (f, after) in walk(m) {
            if let Some(b) = before.get(&after.id) {
                if b.kind_tag != "const" && after.kind_tag == "const" {
                    folded += 1;
                    let what = b.kind_tag;
                    let val = after
                        .const_text
                        .clone()
                        .unwrap_or_else(|| "a constant".to_string());
                    details.push(Remark {
                        pass: self.name(),
                        message: format!("fold-consts: folded {what} in {f} to const {val}"),
                        span: after.span,
                    });
                }
            }
        }
        remarks.push(Remark {
            pass: self.name(),
            message: match folded {
                0 => "fold-consts: nothing to fold".to_string(),
                1 => "fold-consts: folded 1 constant".to_string(),
                n => format!("fold-consts: folded {n} constants"),
            },
            span: Span::default(),
        });
        remarks.extend(details);
    }
}

fn fold_block(
    b: &mut Block,
    known: &mut std::collections::HashMap<ValueId, Const>,
    vals: &mut [ValueInfo],
) {
    for i in &mut b.instrs {
        for blk in i.kind.blocks_mut() {
            fold_block(blk, known, vals);
        }
        let folded = match &i.kind {
            Kind::Const(c) => {
                if let Some(r) = i.result {
                    known.insert(r, c.clone());
                }
                None
            }
            Kind::Unary(UnOp::Not, x) => match known.get(x) {
                Some(Const::Bool(b)) => Some(Const::Bool(!b)),
                _ => None,
            },
            Kind::Binary(op, a, b) => match (known.get(a), known.get(b)) {
                (Some(x), Some(y)) => fold(*op, x, y),
                _ => None,
            },
            _ => None,
        };
        if let (Some(c), Some(r)) = (folded, i.result) {
            vals[r.0 as usize].ty = match &c {
                Const::Null => Ty::Null,
                Const::Bool(_) => Ty::Bool,
                Const::Int(_) => Ty::Int,
                Const::Float(_) => Ty::Float,
                Const::Str(_) => Ty::Str,
            };
            known.insert(r, c.clone());
            i.kind = Kind::Const(c);
        }
    }
}

fn fold(op: BinOp, a: &Const, b: &Const) -> Option<Const> {
    use Const::*;
    Some(match (op, a, b) {
        (BinOp::Add, Int(x), Int(y)) => Int(x.checked_add(*y)?),
        (BinOp::Sub, Int(x), Int(y)) => Int(x.checked_sub(*y)?),
        (BinOp::Mul, Int(x), Int(y)) => Int(x.checked_mul(*y)?),
        (BinOp::Add, Str(x), Str(y)) => Str(format!("{x}{y}")),
        (BinOp::Eq, x, y) if same_kind(x, y) => Bool(x == y),
        (BinOp::Ne, x, y) if same_kind(x, y) => Bool(x != y),
        (BinOp::Lt, Int(x), Int(y)) => Bool(x < y),
        (BinOp::Le, Int(x), Int(y)) => Bool(x <= y),
        (BinOp::Gt, Int(x), Int(y)) => Bool(x > y),
        (BinOp::Ge, Int(x), Int(y)) => Bool(x >= y),
        _ => return None,
    })
}

fn same_kind(a: &Const, b: &Const) -> bool {
    matches!(
        (a, b),
        (Const::Null, Const::Null)
            | (Const::Bool(_), Const::Bool(_))
            | (Const::Int(_), Const::Int(_))
            | (Const::Str(_), Const::Str(_))
    )
}

/// Removes constants, prelude names and closures nothing uses: exactly the
/// kinds that are free to move, so it can never touch an effect.
pub struct DeadPure;

impl Pass for DeadPure {
    fn name(&self) -> &'static str {
        "dead-pure"
    }

    fn run(&self, m: &mut Module) {
        for f in &mut m.funcs {
            loop {
                let mut used = HashSet::new();
                uses(&f.body, &mut used);
                if !prune(&mut f.body, &used) {
                    break;
                }
            }
        }
    }

    fn run_remarking(&self, m: &mut Module, remarks: &mut Vec<Remark>) {
        let before = snapshot(m);
        self.run(m);
        let after: HashMap<InstrId, ()> = walk(m).into_iter().map(|(_, s)| (s.id, ())).collect();
        let mut pruned = 0usize;
        let mut details: Vec<Remark> = Vec::new();
        for (id, b) in &before {
            if !after.contains_key(id) {
                pruned += 1;
                details.push(Remark {
                    pass: self.name(),
                    message: format!("dead-pure: pruned a {} in {}", b.kind_tag, b.func),
                    span: b.span,
                });
            }
        }
        // The snapshot is a map: the lines go in source order, so the same
        // build says the same thing.
        details.sort_by_key(|r| (r.span.line, r.span.col, r.message.clone()));
        remarks.push(Remark {
            pass: self.name(),
            message: match pruned {
                0 => "dead-pure: nothing to prune".to_string(),
                1 => "dead-pure: pruned 1 pure value".to_string(),
                n => format!("dead-pure: pruned {n} pure values"),
            },
            span: Span::default(),
        });
        remarks.extend(details);
    }
}

fn uses(b: &Block, used: &mut HashSet<ValueId>) {
    for i in &b.instrs {
        used.extend(i.kind.uses());
        for blk in i.kind.blocks() {
            uses(blk, used);
        }
    }
}

fn prune(b: &mut Block, used: &HashSet<ValueId>) -> bool {
    let before = b.instrs.len();
    b.instrs.retain(|i| {
        let dead = !i.kind.is_ordered() && i.result.is_some_and(|r| !used.contains(&r));
        !dead
    });
    let mut changed = b.instrs.len() != before;
    for i in &mut b.instrs {
        for blk in i.kind.blocks_mut() {
            changed |= prune(blk, used);
        }
    }
    changed
}

/// One instruction's shape at a moment in time: enough for `--remarks`
/// to say what a pass changed or dropped. `kind_tag` is the mnemonic
/// used in the printed IR (`bin +`, `const`, `load`, `call.user`, ...);
/// `func` identifies the function the instruction sits in, by number or
/// name, so the remark can say where the change was made.
pub(crate) struct Summary {
    pub id: InstrId,
    pub span: Span,
    pub kind_tag: &'static str,
    pub const_text: Option<String>,
    pub func: String,
}

fn walk(m: &Module) -> Vec<(String, Summary)> {
    let mut out = Vec::new();
    for f in &m.funcs {
        let label = match (&f.kind, &f.name) {
            (_, Some(n)) => format!("@{} {n}", f.id.0),
            (FuncKind::Main, _) => "@0 main".to_string(),
            _ => format!("@{}", f.id.0),
        };
        walk_block(&f.body, &label, &mut out);
    }
    out
}

fn walk_block(b: &Block, func: &str, out: &mut Vec<(String, Summary)>) {
    for i in &b.instrs {
        out.push((
            func.to_string(),
            Summary {
                id: i.id,
                span: i.span,
                kind_tag: kind_tag(&i.kind),
                const_text: match &i.kind {
                    Kind::Const(c) => Some(const_text(c)),
                    _ => None,
                },
                func: func.to_string(),
            },
        ));
        for inner in i.kind.blocks() {
            walk_block(inner, func, out);
        }
    }
}

fn snapshot(m: &Module) -> HashMap<InstrId, Summary> {
    walk(m).into_iter().map(|(_, s)| (s.id, s)).collect()
}

fn kind_tag(k: &Kind) -> &'static str {
    match k {
        Kind::Const(_) => "const",
        Kind::Prelude(_) => "prelude",
        Kind::Load(_) => "load",
        Kind::Decl(_, _) => "decl",
        Kind::Store(_, _) => "store",
        Kind::Unary(_, _) => "un",
        Kind::Binary(_, _, _) => "bin",
        Kind::Short { .. } => "short",
        Kind::MakeList(_) => "list",
        Kind::MakeMap(_) => "map",
        Kind::Interp(_) => "interp",
        Kind::Member { .. } => "member",
        Kind::Index { .. } => "index",
        Kind::SetIndex { .. } => "set-index",
        Kind::SetMember { .. } => "set-member",
        Kind::Closure(_) => "closure",
        Kind::BurnOp { .. } => "burn.op",
        Kind::CallBuiltin { .. } => "call.builtin",
        Kind::CallUser { .. } => "call.user",
        Kind::CallValue { .. } => "call.value",
        Kind::CallMethod { .. } => "call.method",
        Kind::MakeChain { .. } => "chain",
        Kind::PackDecl(_) => "pack",
        Kind::Exhale(_) => "exhale",
        Kind::Burn(_) => "burn",
        Kind::Try { .. } => "try",
        Kind::If { .. } => "if",
        Kind::Loop { .. } => "loop",
        Kind::ForEach { .. } => "foreach",
        Kind::Yield(_) => "yield",
        Kind::Ret(_) => "ret",
        Kind::Break => "break",
        Kind::Continue => "continue",
        Kind::Throw(_) => "throw",
    }
}

fn const_text(c: &Const) -> String {
    match c {
        Const::Null => "null".to_string(),
        Const::Bool(b) => b.to_string(),
        Const::Int(i) => i.to_string(),
        Const::Float(f) => format!("{f:?}"),
        Const::Str(s) => format!("{s:?}"),
    }
}

/// Passes that break the rules on purpose, so the tests can show the
/// verifier catches each.
pub mod broken {
    use super::*;

    /// Moves the first `burn.op` in a burn's body to just before the burn.
    pub struct HoistBurnOp;

    impl Pass for HoistBurnOp {
        fn name(&self) -> &'static str {
            "hoist-burn-op"
        }

        fn run(&self, m: &mut Module) {
            for f in &mut m.funcs {
                if hoist(&mut f.body) {
                    return;
                }
            }
        }
    }

    fn hoist(b: &mut Block) -> bool {
        for k in 0..b.instrs.len() {
            if let Kind::Burn(node) = &mut b.instrs[k].kind {
                if let Some(at) = node
                    .body
                    .instrs
                    .iter()
                    .position(|i| matches!(i.kind, Kind::BurnOp { .. }))
                {
                    let op = node.body.instrs.remove(at);
                    b.instrs.insert(k, op);
                    return true;
                }
            }
            for blk in b.instrs[k].kind.blocks_mut() {
                if hoist(blk) {
                    return true;
                }
            }
        }
        false
    }

    /// Swaps two adjacent `burn.op`s in one region: structurally fine, so
    /// only the transition check sees it.
    pub struct SwapBurnOps;

    impl Pass for SwapBurnOps {
        fn name(&self) -> &'static str {
            "swap-burn-ops"
        }

        fn run(&self, m: &mut Module) {
            for f in &mut m.funcs {
                if swap(&mut f.body) {
                    return;
                }
            }
        }
    }

    /// Moves the first `burn.op` of a block past the second one and the
    /// instructions that compute the second's arguments. Every use still
    /// follows its definition (when nothing in between uses the first
    /// op's result), so only the order of effects has changed.
    fn swap(b: &mut Block) -> bool {
        let ops: Vec<usize> = b
            .instrs
            .iter()
            .enumerate()
            .filter(|(_, i)| matches!(i.kind, Kind::BurnOp { .. }))
            .map(|(k, _)| k)
            .collect();
        if ops.len() >= 2 {
            let (a, c) = (ops[0], ops[1]);
            let first = b.instrs[a].result;
            let used_between = b.instrs[a + 1..=c]
                .iter()
                .any(|i| first.is_some_and(|r| i.kind.uses().contains(&r)));
            if !used_between {
                b.instrs[a..=c].rotate_left(1);
                return true;
            }
        }
        for i in &mut b.instrs {
            for blk in i.kind.blocks_mut() {
                if swap(blk) {
                    return true;
                }
            }
        }
        false
    }

    /// Removes the first `burn.op` it finds.
    pub struct DropBurnOp;

    impl Pass for DropBurnOp {
        fn name(&self) -> &'static str {
            "drop-burn-op"
        }

        fn run(&self, m: &mut Module) {
            for f in &mut m.funcs {
                if drop_one(&mut f.body) {
                    return;
                }
            }
        }
    }

    fn drop_one(b: &mut Block) -> bool {
        if let Some(at) = b
            .instrs
            .iter()
            .position(|i| matches!(i.kind, Kind::BurnOp { .. }) && i.result.is_none_or(|_| true))
        {
            b.instrs.remove(at);
            return true;
        }
        for i in &mut b.instrs {
            for blk in i.kind.blocks_mut() {
                if drop_one(blk) {
                    return true;
                }
            }
        }
        false
    }

    /// Moves the then-branch of the first `if` holding a `burn.op` out to
    /// just after the `if`. The order and the region stay the same and
    /// every use still follows its definition, but the effect is no longer
    /// conditional: only T1's places see it.
    pub struct HoistOutOfIf;

    impl Pass for HoistOutOfIf {
        fn name(&self) -> &'static str {
            "hoist-out-of-if"
        }

        fn run(&self, m: &mut Module) {
            for f in &mut m.funcs {
                if unwrap_if(&mut f.body) {
                    return;
                }
            }
        }
    }

    fn unwrap_if(b: &mut Block) -> bool {
        for k in 0..b.instrs.len() {
            if let Kind::If { then, .. } = &mut b.instrs[k].kind {
                let effect = then
                    .instrs
                    .iter()
                    .any(|i| matches!(i.kind, Kind::BurnOp { .. }));
                if effect && !then.instrs.iter().any(|i| i.kind.is_terminator()) {
                    let moved = std::mem::take(&mut then.instrs);
                    b.instrs.splice(k + 1..k + 1, moved);
                    return true;
                }
            }
            for blk in b.instrs[k].kind.blocks_mut() {
                if unwrap_if(blk) {
                    return true;
                }
            }
        }
        false
    }
}
