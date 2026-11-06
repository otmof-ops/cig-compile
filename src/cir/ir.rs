// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! CIR's data (#3): structured SSA. `burn`, `try`, `if`, loops and the
//! short-circuit operators own nested blocks, so "inside a burn" is lexical
//! nesting under a `Burn` node, and moving an effect out of its region
//! means editing two block vectors, which the verifier sees.
//! Temporaries (`ValueId`) are defined once; `roll`, `stick`, parameters
//! and loop variables are slots (`VarId`) that closures share, as the
//! interpreter's environments are.
//!
//! What a back end must honour, because the interpreter does:
//! - a slot exists from its block's entry (hoisted pulls capture each
//!   other before their `decl`); `decl` initialises it;
//! - a slot declared in a loop body, and a `foreach` variable, is a fresh
//!   cell each iteration (`Scope::child` per iteration), so a closure made
//!   in one iteration keeps that iteration's value;
//! - `call.user @f` calls the closure most recently made for `f` in scope
//!   (a pull made inside a loop or a recursion has one closure per entry).

use cigscript::syntax::ast::{AssignOp, BinOp, BurnClass, UnOp};
use cigscript::syntax::span::Span;
use cigscript::value::Effect;

macro_rules! id {
    ($name:ident) => {
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
        pub struct $name(pub u32);
    };
}
id!(InstrId);
id!(ValueId);
id!(VarId);
id!(FuncId);
id!(RegionId);

impl RegionId {
    /// Outside every burn.
    pub const ROOT: RegionId = RegionId(0);
}

pub struct Module {
    pub file: String,
    /// `funcs[0]` is the script's top level.
    pub funcs: Vec<Func>,
    /// Indexed by `VarId`.
    pub vars: Vec<VarInfo>,
    pub next_instr: u32,
    pub next_region: u32,
}

impl Module {
    pub fn fresh_instr(&mut self) -> InstrId {
        self.next_instr += 1;
        InstrId(self.next_instr)
    }
}

pub struct VarInfo {
    pub name: String,
    pub mutable: bool,
    pub owner: FuncId,
    /// Some other function reads or writes it: a back end boxes it.
    pub captured: bool,
    pub span: Span,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FuncKind {
    Main,
    Pull,
    Lambda,
}

pub struct Func {
    pub id: FuncId,
    pub kind: FuncKind,
    pub name: Option<String>,
    pub params: Vec<VarId>,
    /// Free variables, owned by an enclosing function.
    pub captures: Vec<VarId>,
    /// Derived (verifier rule V5): it effects outside any burn of its own,
    /// so it is legal only when called inside one.
    pub requires_burn: bool,
    /// Indexed by `ValueId`.
    pub vals: Vec<ValueInfo>,
    pub body: Block,
    pub span: Span,
}

pub struct ValueInfo {
    pub ty: Ty,
    pub def: InstrId,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ty {
    Dynamic,
    Null,
    Bool,
    Int,
    Float,
    Str,
    List,
    Map,
    Func,
}

#[derive(Default)]
pub struct Block {
    pub instrs: Vec<Instr>,
}

pub struct Instr {
    pub id: InstrId,
    pub span: Span,
    pub result: Option<ValueId>,
    pub kind: Kind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Const {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShortOp {
    And,
    Or,
    Coalesce,
    /// `?.`: the rest is not evaluated on a null subject.
    Safe,
}

pub enum MapKey {
    Static(String),
    Dyn(ValueId),
}

pub enum Piece {
    Lit(String),
    Val(ValueId),
}

/// What an effectful stdlib call does to the world.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OpKind {
    Write,
    Append,
    Mkdir,
    Rm,
    Cp,
    Mv,
    Hop,
    EnvSet,
    EnvUnset,
    LayerApply,
    LayerDiscard,
}

impl OpKind {
    pub fn name(self) -> &'static str {
        match self {
            OpKind::Write => "write",
            OpKind::Append => "append",
            OpKind::Mkdir => "mkdir",
            OpKind::Rm => "rm",
            OpKind::Cp => "cp",
            OpKind::Mv => "mv",
            OpKind::Hop => "hop",
            OpKind::EnvSet => "env.set",
            OpKind::EnvUnset => "env.unset",
            OpKind::LayerApply => "layer.apply",
            OpKind::LayerDiscard => "layer.discard",
        }
    }
}

pub struct BurnNode {
    pub region: RegionId,
    pub class: BurnClass,
    pub state: Option<VarId>,
    pub label: Option<ValueId>,
    pub body: Block,
    pub compensate: Option<Block>,
    pub unburn_src: Option<String>,
}

pub enum Kind {
    Const(Const),
    /// A name that resolves to the prelude: a builtin, a module, `args`.
    Prelude(String),
    Load(VarId),
    /// Declare and initialise.
    Decl(VarId, ValueId),
    Store(VarId, ValueId),
    Unary(UnOp, ValueId),
    /// Never `And`, `Or` or `Coalesce`: those are `Short`.
    Binary(BinOp, ValueId, ValueId),
    /// The result is `subject` when the guard does not enter (`And`: falsy,
    /// `Or`: truthy, `Coalesce`: non-null), null for `Safe` on a null
    /// subject, else what `rhs` yields.
    Short {
        op: ShortOp,
        subject: ValueId,
        rhs: Block,
    },
    MakeList(Vec<ValueId>),
    MakeMap(Vec<(MapKey, ValueId)>),
    Interp(Vec<Piece>),
    /// `safe`: `?.`, null for a null subject and for a map without the key.
    Member {
        obj: ValueId,
        name: String,
        safe: bool,
    },
    Index {
        obj: ValueId,
        idx: ValueId,
    },
    /// The combined read-modify-write: the type errors live in it.
    SetIndex {
        obj: ValueId,
        idx: ValueId,
        op: AssignOp,
        val: ValueId,
    },
    SetMember {
        obj: ValueId,
        name: String,
        op: AssignOp,
        val: ValueId,
    },
    Closure(FuncId),
    BurnOp {
        kind: OpKind,
        callee: String,
        effect: Effect,
        args: Vec<ValueId>,
    },
    CallBuiltin {
        callee: String,
        args: Vec<ValueId>,
    },
    CallUser {
        func: FuncId,
        args: Vec<ValueId>,
    },
    CallValue {
        f: ValueId,
        args: Vec<ValueId>,
    },
    CallMethod {
        recv: ValueId,
        name: String,
        args: Vec<ValueId>,
    },
    MakeChain {
        name: VarId,
        steps: Vec<(Option<String>, ValueId, Span)>,
    },
    PackDecl(Vec<ValueId>),
    Exhale(Vec<ValueId>),
    Burn(Box<BurnNode>),
    Try {
        body: Block,
        catch: Option<(Option<VarId>, Block)>,
        finally: Option<Block>,
    },
    If {
        cond: ValueId,
        then: Block,
        otherwise: Option<Block>,
    },
    Loop {
        cond: Block,
        body: Block,
    },
    ForEach {
        iter: ValueId,
        var: VarId,
        body: Block,
    },
    // Terminators.
    Yield(ValueId),
    Ret(ValueId),
    Break,
    Continue,
    /// `cough`.
    Throw(ValueId),
}

impl Kind {
    pub fn is_terminator(&self) -> bool {
        matches!(
            self,
            Kind::Yield(_) | Kind::Ret(_) | Kind::Break | Kind::Continue | Kind::Throw(_)
        )
    }

    /// Ordered: everything a pass may not move across an effect. Only
    /// constants, prelude names and closure creation are free to move.
    pub fn is_ordered(&self) -> bool {
        !matches!(self, Kind::Const(_) | Kind::Prelude(_) | Kind::Closure(_))
    }

    /// The nested blocks, in evaluation order.
    pub fn blocks(&self) -> Vec<&Block> {
        self.slots().into_iter().map(|(_, b)| b).collect()
    }

    /// The nested blocks with where each one is (`if.then`,
    /// `burn.compensate`), in evaluation order. The transition check
    /// compares these, so moving an instruction out of a branch, or from a
    /// burn's body into its compensation, is seen.
    pub fn slots(&self) -> Vec<(&'static str, &Block)> {
        match self {
            Kind::Short { rhs, .. } => vec![("short.rhs", rhs)],
            Kind::Burn(b) => {
                let mut v = vec![("burn.body", &b.body)];
                if let Some(c) = &b.compensate {
                    v.push(("burn.compensate", c));
                }
                v
            }
            Kind::Try {
                body,
                catch,
                finally,
            } => {
                let mut v = vec![("try.body", body)];
                if let Some((_, c)) = catch {
                    v.push(("try.catch", c));
                }
                if let Some(f) = finally {
                    v.push(("try.finally", f));
                }
                v
            }
            Kind::If {
                then, otherwise, ..
            } => {
                let mut v = vec![("if.then", then)];
                if let Some(o) = otherwise {
                    v.push(("if.else", o));
                }
                v
            }
            Kind::Loop { cond, body } => vec![("loop.cond", cond), ("loop.body", body)],
            Kind::ForEach { body, .. } => vec![("foreach.body", body)],
            _ => Vec::new(),
        }
    }

    pub fn blocks_mut(&mut self) -> Vec<&mut Block> {
        match self {
            Kind::Short { rhs, .. } => vec![rhs],
            Kind::Burn(b) => {
                let b = &mut **b;
                let mut v = vec![&mut b.body];
                if let Some(c) = &mut b.compensate {
                    v.push(c);
                }
                v
            }
            Kind::Try {
                body,
                catch,
                finally,
            } => {
                let mut v = vec![body];
                if let Some((_, c)) = catch {
                    v.push(c);
                }
                if let Some(f) = finally {
                    v.push(f);
                }
                v
            }
            Kind::If {
                then, otherwise, ..
            } => {
                let mut v = vec![then];
                if let Some(o) = otherwise {
                    v.push(o);
                }
                v
            }
            Kind::Loop { cond, body } => vec![cond, body],
            Kind::ForEach { body, .. } => vec![body],
            _ => Vec::new(),
        }
    }

    /// Every value it reads, nested blocks not included.
    pub fn uses(&self) -> Vec<ValueId> {
        match self {
            Kind::Const(_)
            | Kind::Prelude(_)
            | Kind::Load(_)
            | Kind::Closure(_)
            | Kind::Break
            | Kind::Continue
            | Kind::Loop { .. }
            | Kind::Try { .. } => Vec::new(),
            Kind::Decl(_, v) | Kind::Store(_, v) | Kind::Unary(_, v) => vec![*v],
            Kind::Binary(_, a, b) => vec![*a, *b],
            Kind::Short { subject, .. } => vec![*subject],
            Kind::MakeList(v) | Kind::PackDecl(v) | Kind::Exhale(v) => v.clone(),
            Kind::MakeMap(kv) => {
                let mut v = Vec::new();
                for (k, x) in kv {
                    if let MapKey::Dyn(k) = k {
                        v.push(*k);
                    }
                    v.push(*x);
                }
                v
            }
            Kind::Interp(p) => p
                .iter()
                .filter_map(|p| match p {
                    Piece::Val(v) => Some(*v),
                    Piece::Lit(_) => None,
                })
                .collect(),
            Kind::Member { obj, .. } => vec![*obj],
            Kind::Index { obj, idx } => vec![*obj, *idx],
            Kind::SetIndex { obj, idx, val, .. } => vec![*obj, *idx, *val],
            Kind::SetMember { obj, val, .. } => vec![*obj, *val],
            Kind::BurnOp { args, .. }
            | Kind::CallBuiltin { args, .. }
            | Kind::CallUser { args, .. } => args.clone(),
            Kind::CallValue { f, args } => {
                let mut v = vec![*f];
                v.extend(args);
                v
            }
            Kind::CallMethod { recv, args, .. } => {
                let mut v = vec![*recv];
                v.extend(args);
                v
            }
            Kind::MakeChain { steps, .. } => steps.iter().map(|s| s.1).collect(),
            Kind::Burn(b) => b.label.into_iter().collect(),
            Kind::If { cond, .. } => vec![*cond],
            Kind::ForEach { iter, .. } => vec![*iter],
            Kind::Yield(v) | Kind::Ret(v) | Kind::Throw(v) => vec![*v],
        }
    }
}
