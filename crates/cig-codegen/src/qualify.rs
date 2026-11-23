// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2

//! The qualifying set: which specializations are compiled.
//!
//! A specialization qualifies when all of these hold:
//!
//! 1. not the top level, in the module's index, at most `MAX_ARGS`
//!    parameters, every row type and the return type scalar
//!    (`Int | Float | Bool | Null`);
//! 2. every instruction of its body is compilable and every value it
//!    defines is scalar;
//! 3. `for i in a..b`: a `ForEach` whose iter is a `Binary(Range, a, b)`
//!    over two ints, that range value has no other use, and the var is
//!    a scalar `Int` slot of the function;
//! 4. its frame is at most `MAX_FRAME` bytes. That is known only of
//!    compiled code, so [`crate::compile`] holds it: it compiles,
//!    measures, drops what is over and compiles the rest again.
//!
//! A call (`CallUser`) is compilable when what it calls is compiled,
//! which is circular through calls, so the
//! algorithm is a largest fixed point: start from every specialization
//! that passes the local rules, remove any whose call chain names a
//! specialization outside the set, repeat until nothing changes. The
//! loop ends because the set only shrinks and each pass removes at
//! least one element, or none.

use std::collections::{BTreeMap, BTreeSet};

use cig_native::{Ty as NTy, MAX_ARGS};
use cigscript::lower::cir::infer::{Inference, Row, Spec};
use cigscript::lower::cir::ir::{Block, FuncId, InstrId, Kind, Module, ShortOp, Ty, ValueId};
use cigscript::syntax::ast::BinOp;

/// A native scalar: the types native code holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scalar {
    /// `null`.
    Null,
    /// A boolean.
    Bool,
    /// A 64-bit signed integer.
    Int,
    /// A 64-bit IEEE float.
    Float,
}

impl Scalar {
    /// The scalar a CIR `Ty` names, or `None` for a non-scalar type.
    pub fn from_ty(t: Ty) -> Option<Scalar> {
        match t {
            Ty::Null => Some(Scalar::Null),
            Ty::Bool => Some(Scalar::Bool),
            Ty::Int => Some(Scalar::Int),
            Ty::Float => Some(Scalar::Float),
            _ => None,
        }
    }

    /// The type as a reason names it.
    fn said(self) -> &'static str {
        match self {
            Scalar::Null => "null",
            Scalar::Bool => "a bool",
            Scalar::Int => "an int",
            Scalar::Float => "a float",
        }
    }

    /// The native-section type code for this scalar.
    pub fn as_native(self) -> NTy {
        match self {
            Scalar::Null => NTy::Null,
            Scalar::Bool => NTy::Bool,
            Scalar::Int => NTy::Int,
            Scalar::Float => NTy::Float,
        }
    }
}

/// What a reason says of a value the inference gave no one type.
const UNTYPED: &str = "a value that is not of one type on every path";

/// A specialization identifier: `(FuncId, Row)` as the inference
/// addresses it.
pub type SpecKey = (FuncId, Row);

/// Why a specialization is not compiled, as a user-readable string.
///
/// `Ok` means it passes every rule but the two only a whole set shows
/// (its calls, [`fixed_point`]; its frame, [`crate::compile`]). The
/// reason is the first thing in the way in source order, as
/// `line L: what` with the file's 1-based line, or a reason without a
/// line where the function as a whole is in the way.
pub fn qualifies(
    m: &Module,
    inf: &Inference,
    index: &BTreeMap<FuncId, u32>,
    spec: &Spec,
) -> Result<(), String> {
    // The function as a whole.
    if spec.func.0 == 0 {
        return Err("the top level is not compiled".into());
    }
    if !index.contains_key(&spec.func) {
        return Err("the function is not in the bytecode module".into());
    }
    let f = &m.funcs[spec.func.0 as usize];
    if f.params.len() > MAX_ARGS {
        return Err(format!("more than {MAX_ARGS} parameters"));
    }
    for t in &spec.row.0 {
        let what = match t {
            Ty::Null | Ty::Bool | Ty::Int | Ty::Float => continue,
            Ty::Dynamic => "it is called with an argument that is not of one type",
            Ty::Str => "it is called with a string",
            Ty::List => "it is called with a list",
            Ty::Map => "it is called with a map",
            Ty::Func => "it is called with a function",
        };
        return Err(what.into());
    }
    // The ranges whose result is used only by a `for` over an int
    // variable: counted loops.
    let ranges = counted_ranges(m, spec);
    // The body, instruction by instruction. Before the result's type,
    // so the reason names the line that is in the way where one is.
    let mut w = Walk {
        m,
        inf,
        spec,
        ranges: &ranges,
    };
    w.walk(&f.body)?;
    match Scalar::from_ty(spec.ret) {
        Some(_) => Ok(()),
        None => Err("its result is not of one type on every path".into()),
    }
}

/// The `ValueId`s whose defining `Binary(Range, a, b)` is used exactly
/// once, and that one use is a `ForEach` of this function with a scalar
/// `Int` slot as its variable.
///
/// Used twice: `qualifies` checks the rule, `lower::body` lowers the
/// `ForEach` as a counted loop and skips the range value.
pub fn counted_ranges(m: &Module, spec: &Spec) -> BTreeSet<InstrId> {
    let f = &m.funcs[spec.func.0 as usize];
    // Which `ForEach`es iterate a range, by the range's `ValueId`.
    let mut by_range: BTreeMap<ValueId, (InstrId, bool)> = BTreeMap::new();
    for_each_instr(&f.body, &mut |ins| {
        if let Kind::ForEach { iter, var, .. } = &ins.kind {
            let slot_ok = matches!(spec.slots.get(var), Some(Ty::Int));
            by_range.insert(*iter, (ins.id, slot_ok));
        }
    });
    // Count uses of each value: a range qualifies only if its one use
    // is the ForEach.
    let mut uses: BTreeMap<ValueId, usize> = BTreeMap::new();
    for_each_instr(&f.body, &mut |ins| {
        for v in ins.kind.uses() {
            *uses.entry(v).or_insert(0) += 1;
        }
    });
    // Which InstrIds are the qualifying range instructions?
    let mut out = BTreeSet::new();
    for_each_instr(&f.body, &mut |ins| {
        let Some(res) = ins.result else {
            return;
        };
        let Kind::Binary(BinOp::Range, a, b) = ins.kind else {
            return;
        };
        if spec.vals.get(a.0 as usize).copied() != Some(Ty::Int)
            || spec.vals.get(b.0 as usize).copied() != Some(Ty::Int)
        {
            return;
        }
        let Some((fe, slot_ok)) = by_range.get(&res) else {
            return;
        };
        if !slot_ok {
            return;
        }
        if uses.get(&res).copied() != Some(1) {
            return;
        }
        // The use is the ForEach: record this Binary(Range) as counted.
        let _ = fe;
        out.insert(ins.id);
    });
    out
}

fn for_each_instr(b: &Block, f: &mut impl FnMut(&cigscript::lower::cir::ir::Instr)) {
    for ins in &b.instrs {
        f(ins);
        for nested in ins.kind.blocks() {
            for_each_instr(nested, f);
        }
    }
}

struct Walk<'a> {
    m: &'a Module,
    inf: &'a Inference,
    spec: &'a Spec,
    ranges: &'a BTreeSet<InstrId>,
}

impl Walk<'_> {
    fn walk(&mut self, b: &Block) -> Result<(), String> {
        for ins in &b.instrs {
            self.check_instr(ins)?;
            for nested in ins.kind.blocks() {
                self.walk(nested)?;
            }
        }
        Ok(())
    }

    fn val_scalar(&self, v: ValueId) -> Option<Scalar> {
        Scalar::from_ty(self.spec.vals.get(v.0 as usize).copied()?)
    }

    fn check_instr(&mut self, ins: &cigscript::lower::cir::ir::Instr) -> Result<(), String> {
        let m = self.m;
        let f = &m.funcs[self.spec.func.0 as usize];
        let line = ins.span.line;
        let say = |s: String| format!("line {line}: {s}");
        // The instruction's own reason first; then that a value it
        // defines must be scalar (but for a counted range's, which
        // nothing but its `for` reads, and that does not read it).
        self.check_kind(ins, f, &say)?;
        match ins.result {
            Some(v) if !self.ranges.contains(&ins.id) && self.val_scalar(v).is_none() => {
                Err(say("a value that is not of one type on every path".into()))
            }
            _ => Ok(()),
        }
    }

    fn check_kind(
        &mut self,
        ins: &cigscript::lower::cir::ir::Instr,
        f: &cigscript::lower::cir::ir::Func,
        say: &dyn Fn(String) -> String,
    ) -> Result<(), String> {
        match &ins.kind {
            Kind::Step(_)
            | Kind::Const(cigscript::lower::cir::ir::Const::Null)
            | Kind::Const(cigscript::lower::cir::ir::Const::Bool(_))
            | Kind::Const(cigscript::lower::cir::ir::Const::Int(_))
            | Kind::Const(cigscript::lower::cir::ir::Const::Float(_)) => Ok(()),
            Kind::Const(cigscript::lower::cir::ir::Const::Str(_)) => Err(say("a string".into())),
            Kind::Load(v) | Kind::Store(v, _) | Kind::Decl(v, _) => {
                let info = &self.m.vars[v.0 as usize];
                if info.owner != f.id {
                    return Err(say(format!(
                        "a variable of another function (`{}`)",
                        info.name
                    )));
                }
                if info.captured {
                    return Err(say(format!(
                        "a variable another function shares (`{}`)",
                        info.name
                    )));
                }
                match self.spec.slots.get(v) {
                    Some(t) if Scalar::from_ty(*t).is_some() => Ok(()),
                    _ => Err(say(format!(
                        "the variable `{}` is not an int, a float, a bool or null on every path",
                        info.name
                    ))),
                }
            }
            Kind::Unary(op, x) => {
                use cigscript::syntax::ast::UnOp;
                let sx = self.val_scalar(*x).ok_or_else(|| say(UNTYPED.into()))?;
                match (op, sx) {
                    (UnOp::Not, _) => Ok(()),
                    (UnOp::Neg, Scalar::Int | Scalar::Float) => Ok(()),
                    (UnOp::Neg, _) => Err(say(format!("`-` on {}", sx.said()))),
                }
            }
            Kind::Binary(op, a, b) => {
                let sa = self.val_scalar(*a);
                let sb = self.val_scalar(*b);
                // The counted-range case: its result is a range, not a
                // scalar. We allow Binary(Range, Int, Int) here because
                // the earlier result check skipped it; so this match
                // only accepts that when it is in `ranges`.
                if matches!(op, BinOp::Range) {
                    return if self.ranges.contains(&ins.id) {
                        Ok(())
                    } else {
                        Err(say("a range that is not a `for`'s".into()))
                    };
                }
                let (sa, sb) = match (sa, sb) {
                    (Some(a), Some(b)) => (a, b),
                    _ => return Err(say(UNTYPED.into())),
                };
                check_binop(*op, sa, sb).map_err(say)
            }
            Kind::Short { op, subject, rhs } => {
                if self.val_scalar(*subject).is_none() {
                    return Err(say(UNTYPED.into()));
                }
                // The result's type is the caller's to check.
                match op {
                    ShortOp::And | ShortOp::Or | ShortOp::Coalesce => self.walk(rhs),
                    ShortOp::Safe => Err(say("`?.`".into())),
                }
            }
            Kind::If { .. } | Kind::Loop { .. } | Kind::Break | Kind::Continue => Ok(()),
            // Only a `for` over a counted range. Anything else
            // it could be given (a list, or a value the tree-walker
            // refuses to iterate) is left to the bytecode.
            Kind::ForEach { iter, .. } => {
                let counted = self
                    .ranges
                    .iter()
                    .any(|range| instr_result(&f.body, *range).is_some_and(|value| value == *iter));
                if counted {
                    Ok(())
                } else {
                    Err(say(
                        "a `for` over something that is not `a..b` of two ints".into()
                    ))
                }
            }
            Kind::Yield(v) | Kind::Ret(v) => {
                if self.val_scalar(*v).is_some() {
                    Ok(())
                } else {
                    Err(say(UNTYPED.into()))
                }
            }
            Kind::CallUser { func, args } => {
                let Some(callee) = f_args_lookup(self.m, *func) else {
                    return Err(say("calls a function the module does not have".into()));
                };
                if args.len() != callee {
                    return Err(say(format!(
                        "a call with {} arguments of a function with {callee} parameters",
                        args.len()
                    )));
                }
                // Each argument must be scalar.
                for a in args {
                    if self.val_scalar(*a).is_none() {
                        return Err(say(UNTYPED.into()));
                    }
                }
                // The `Spec::calls` entry names the specialization
                // this call enters.
                let Some((g, row)) = self.spec.calls.get(&ins.id) else {
                    return Err(say("a call whose function is not known".into()));
                };
                // Does that callee have a spec, with a result native code
                // can hold? (That it is compiled itself is the fixed
                // point's to find.)
                let name = self
                    .m
                    .funcs
                    .get(g.0 as usize)
                    .and_then(|f| f.name.as_deref())
                    .unwrap_or("a function");
                match self.inf.spec(*g, row) {
                    None => Err(say(format!("calls `{name}`, which was not inferred"))),
                    Some(callee) if Scalar::from_ty(callee.ret).is_none() => Err(say(format!(
                        "calls `{name}`, whose result is not of one type on every path"
                    ))),
                    Some(_) => Ok(()),
                }
            }
            Kind::Prelude(_) => Err(say("a builtin used as a value".into())),
            Kind::MakeList(_) | Kind::MakeMap(_) => Err(say("a list or a map".into())),
            Kind::Interp(_) => Err(say("a string".into())),
            Kind::Key(_) | Kind::Member { .. } | Kind::Index { .. } => {
                Err(say("a field or an index".into()))
            }
            Kind::SetIndex { .. } | Kind::SetMember { .. } => {
                Err(say("a write into a value".into()))
            }
            Kind::Closure(_) => Err(say("a closure".into())),
            Kind::BurnOp { .. } | Kind::CallBuiltin { .. } => Err(say("calls a builtin".into())),
            Kind::CallValue { .. } | Kind::CallMethod { .. } => Err(say("an indirect call".into())),
            Kind::Exhale(_) => Err(say("`exhale`".into())),
            Kind::MakeChain { .. } | Kind::PackDecl(_) => Err(say("a chain or a pack".into())),
            Kind::Burn(_) | Kind::Try { .. } => Err(say("a burn or a try".into())),
            Kind::Throw(_) => Err(say("`cough`".into())),
        }
    }
}

/// The value instruction `id` of `b` (or of a block nested in it)
/// defines.
fn instr_result(b: &Block, id: InstrId) -> Option<ValueId> {
    let mut found = None;
    for_each_instr(b, &mut |ins| {
        if ins.id == id {
            found = ins.result;
        }
    });
    found
}

fn f_args_lookup(m: &Module, f: FuncId) -> Option<usize> {
    m.funcs.get(f.0 as usize).map(|f| f.params.len())
}

fn check_binop(op: BinOp, a: Scalar, b: Scalar) -> Result<(), String> {
    use BinOp::*;
    use Scalar::*;
    let on = |what: &str| format!("{what} on {} and {}", a.said(), b.said());
    // Which operand types are compiled.
    match op {
        Add | Sub | Mul => match (a, b) {
            (Int, Int) | (Float, Float) | (Int, Float) | (Float, Int) => Ok(()),
            _ => Err(on("arithmetic")),
        },
        Rem => match (a, b) {
            (Int, Int) => Ok(()),
            (Float, _) | (_, Float) => Err("`%` with a float".into()),
            _ => Err(on("`%`")),
        },
        Div => match (a, b) {
            (Int, Int) => Err("an int divided by an int (an int or a float, by its values)".into()),
            (Float, Float) | (Int, Float) | (Float, Int) => Ok(()),
            _ => Err(on("`/`")),
        },
        Lt | Le | Gt | Ge => match (a, b) {
            (Int, Int) | (Float, Float) | (Int, Float) | (Float, Int) => Ok(()),
            _ => Err(on("an ordering")),
        },
        Eq | Ne => Ok(()),
        Range => Err("a range that is not a `for`'s".into()),
        And | Or | Coalesce => unreachable!("short-circuit ops are `Kind::Short`"),
    }
}

/// The largest set of specializations that pass [`qualifies`] and call
/// only members of the set.
///
/// Start from every specialization that passes [`qualifies`], then
/// remove any whose `Spec::calls` names a specialization outside the
/// set. Repeat until nothing changes. The set only shrinks, so this
/// terminates in at most `|specs|` passes.
pub fn fixed_point<'a>(
    m: &'a Module,
    inf: &'a Inference,
    index: &BTreeMap<FuncId, u32>,
) -> (BTreeSet<SpecKey>, Vec<(SpecKey, String)>) {
    let mut outcomes = Vec::new();
    let mut set: BTreeSet<SpecKey> = BTreeSet::new();
    for s in inf.specs() {
        let key = (s.func, s.row.clone());
        match qualifies(m, inf, index, s) {
            Ok(()) => {
                set.insert(key);
            }
            Err(why) => outcomes.push((key, why)),
        }
    }
    loop {
        let before = set.len();
        let mut removed = Vec::new();
        for key in set.iter() {
            let s = match inf.spec(key.0, &key.1) {
                Some(s) => s,
                None => continue,
            };
            for (g, row) in s.calls.values() {
                let callee = (*g, row.clone());
                if !set.contains(&callee) {
                    removed.push(key.clone());
                    break;
                }
            }
        }
        for key in &removed {
            set.remove(key);
            outcomes.push((key.clone(), "calls a function not compiled".into()));
        }
        if set.len() == before {
            break;
        }
    }
    (set, outcomes)
}
