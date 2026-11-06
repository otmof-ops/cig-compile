//! Passes over CIR. Each runs between two checks: the structural verifier,
//! and the transition check against a snapshot taken before it, so a pass
//! that moves an effect out of its region, reorders effects or drops one
//! is caught where it happens.

use super::ir::*;
use super::verify::{errors, verify, verify_transition, Trace, VerifyError};
use cigscript::syntax::ast::{BinOp, UnOp};
use std::collections::HashSet;

pub trait Pass {
    fn name(&self) -> &'static str;
    fn run(&self, m: &mut Module);
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
        p.run(m);
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

/// The passes `--emit=cir` runs.
pub fn default_passes() -> Vec<&'static dyn Pass> {
    vec![&FoldConsts, &DeadPure]
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
            let mut known: std::collections::HashMap<ValueId, Const> =
                std::collections::HashMap::new();
            fold_block(&mut f.body, &mut known, &mut f.vals);
        }
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
}
