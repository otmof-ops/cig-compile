//! The verifier (#3). `verify` is structural and runs after lowering and
//! after every pass; `verify_transition` compares a module with a snapshot
//! taken before a pass, because an output alone cannot show that something
//! was reordered. Every error names its invariant, its function and its
//! span.

use super::effects::kind_of;
use super::ir::*;
use cigscript::syntax::span::Span;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warn,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifyError {
    /// The invariant: `V1`..`V10`, `T1`..`T3`.
    pub code: &'static str,
    pub severity: Severity,
    pub func: String,
    pub span: Span,
    pub message: String,
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} in {} at {}:{}: {}",
            if self.severity == Severity::Warn {
                "warning"
            } else {
                "error"
            },
            self.code,
            self.func,
            self.span.line,
            self.span.col,
            self.message
        )
    }
}

fn fname(f: &Func) -> String {
    match (&f.kind, &f.name) {
        (FuncKind::Main, _) => "main".to_string(),
        (_, Some(n)) => n.clone(),
        (_, None) => format!("lambda@{}", f.id.0),
    }
}

/// Which functions require a burn: the least fixpoint of "has an effect
/// outside its own burns, or calls (outside its burns) one that does".
/// A closure's body is its own function: making it is not calling it.
pub fn requires_burn(m: &Module) -> Vec<bool> {
    let n = m.funcs.len();
    let mut direct = vec![false; n];
    let mut calls: Vec<Vec<usize>> = vec![Vec::new(); n];
    for f in &m.funcs {
        let i = f.id.0 as usize;
        scan(&f.body, 0, &mut |instr, depth| {
            if depth == 0 {
                match &instr.kind {
                    Kind::BurnOp { .. } => direct[i] = true,
                    Kind::CallUser { func, .. } => calls[i].push(func.0 as usize),
                    _ => {}
                }
            }
        });
    }
    let mut flag = direct;
    loop {
        let mut changed = false;
        for i in 0..n {
            if !flag[i]
                && calls[i]
                    .iter()
                    .any(|c| flag.get(*c).copied().unwrap_or(false))
            {
                flag[i] = true;
                changed = true;
            }
        }
        if !changed {
            return flag;
        }
    }
}

/// Visit every instruction with its burn depth.
fn scan(b: &Block, depth: usize, f: &mut dyn FnMut(&Instr, usize)) {
    for i in &b.instrs {
        f(i, depth);
        let inner = if matches!(i.kind, Kind::Burn(_)) {
            depth + 1
        } else {
            depth
        };
        for blk in i.kind.blocks() {
            scan(blk, inner, f);
        }
    }
}

struct Walk<'m> {
    m: &'m Module,
    f: &'m Func,
    errors: Vec<VerifyError>,
    seen_instr: &'m mut HashSet<InstrId>,
    defined: HashSet<ValueId>,
    regions: &'m mut HashSet<RegionId>,
    flags: &'m [bool],
}

impl Walk<'_> {
    fn err(&mut self, code: &'static str, span: Span, message: String) {
        self.errors.push(VerifyError {
            code,
            severity: Severity::Error,
            func: fname(self.f),
            span,
            message,
        });
    }

    fn warn(&mut self, code: &'static str, span: Span, message: String) {
        self.errors.push(VerifyError {
            code,
            severity: Severity::Warn,
            func: fname(self.f),
            span,
            message,
        });
    }

    fn var_ok(&mut self, v: VarId, span: Span) {
        let Some(info) = self.m.vars.get(v.0 as usize) else {
            self.err("V9", span, format!("${} is not a variable", v.0));
            return;
        };
        if info.owner != self.f.id && !self.f.captures.contains(&v) {
            self.err(
                "V9",
                span,
                format!(
                    "${}:{} is neither this function's nor captured by it",
                    v.0, info.name
                ),
            );
        }
    }

    /// `visible`: the values in scope here. `yields`: this block must end
    /// in `yield` (and only such a block may).
    fn block(
        &mut self,
        b: &Block,
        visible: &HashSet<ValueId>,
        burn_depth: usize,
        loop_depth: usize,
        yields: bool,
    ) {
        let mut scope = visible.clone();
        let n = b.instrs.len();
        for (k, i) in b.instrs.iter().enumerate() {
            let last = k + 1 == n;
            // V1: ids.
            if !self.seen_instr.insert(i.id) {
                self.err(
                    "V1",
                    i.span,
                    format!("instruction #{} appears twice", i.id.0),
                );
            }
            for u in i.kind.uses() {
                if !scope.contains(&u) {
                    self.err(
                        "V1",
                        i.span,
                        format!("%{} is used where it is not defined", u.0),
                    );
                }
            }
            // V2: spans.
            if i.span.line < 1 || i.span.start > i.span.end {
                self.err("V2", i.span, "an instruction without a span".to_string());
            }
            // V3: terminators.
            if i.kind.is_terminator() && !last {
                self.err(
                    "V3",
                    i.span,
                    "a terminator before the end of its block".to_string(),
                );
            }
            match &i.kind {
                Kind::Yield(_) if !yields || !last => {
                    self.err("V3", i.span, "a yield outside a value block".to_string())
                }
                Kind::Break | Kind::Continue if loop_depth == 0 => {
                    self.err("V3", i.span, "break or continue outside a loop".to_string())
                }
                Kind::Ret(_) if self.f.kind == FuncKind::Main => {
                    self.err("V3", i.span, "a return in the top level".to_string())
                }
                _ => {}
            }
            // V4, V7: effects only in regions, and as the table says.
            if let Kind::BurnOp {
                kind,
                callee,
                effect,
                ..
            } = &i.kind
            {
                let allowed = burn_depth > 0
                    || (self.f.kind != FuncKind::Main && self.flags[self.f.id.0 as usize]);
                if !allowed {
                    self.err(
                        "V4",
                        i.span,
                        format!("burn.op {} ({callee}) outside every burn", kind.name()),
                    );
                }
                if kind_of(callee) != Some(*kind) || !effect.needs_burn() {
                    self.err(
                        "V7",
                        i.span,
                        format!("burn.op {} does not match {callee}", kind.name()),
                    );
                }
            }
            // V6: a lint, never an error: an effectful pull called outside
            // any burn reaches E701 at run time, as the interpreter's does.
            if let Kind::CallUser { func, .. } = &i.kind {
                if burn_depth == 0
                    && self.f.kind == FuncKind::Main
                    && self.flags.get(func.0 as usize).copied().unwrap_or(false)
                {
                    self.warn(
                        "V6",
                        i.span,
                        format!("calls @{} (which effects) outside any burn", func.0),
                    );
                }
                if func.0 as usize >= self.m.funcs.len() {
                    self.err("V10", i.span, format!("@{} is not a function", func.0));
                }
            }
            // V8, V9.
            match &i.kind {
                Kind::Load(v) | Kind::Store(v, _) | Kind::Decl(v, _) => self.var_ok(*v, i.span),
                Kind::ForEach { var, .. } => self.var_ok(*var, i.span),
                Kind::Burn(b) => {
                    if b.region == RegionId::ROOT || !self.regions.insert(b.region) {
                        self.err(
                            "V8",
                            i.span,
                            format!("region r{} is not unique", b.region.0),
                        );
                    }
                }
                Kind::Closure(f) if f.0 as usize >= self.m.funcs.len() => self.err(
                    "V10",
                    i.span,
                    format!("closure of @{}, which is not a function", f.0),
                ),
                _ => {}
            }
            if let Kind::Store(v, _) = &i.kind {
                if !self.m.vars.get(v.0 as usize).is_some_and(|x| x.mutable) {
                    self.err("V9", i.span, format!("a store to the immutable ${}", v.0));
                }
            }
            // Nested blocks.
            let inner_burn = if matches!(i.kind, Kind::Burn(_)) {
                burn_depth + 1
            } else {
                burn_depth
            };
            match &i.kind {
                Kind::Short { rhs, .. } => self.block(rhs, &scope, inner_burn, loop_depth, true),
                Kind::Loop { cond, body } => {
                    self.block(cond, &scope, inner_burn, loop_depth, true);
                    self.block(body, &scope, inner_burn, loop_depth + 1, false);
                }
                Kind::ForEach { body, .. } => {
                    self.block(body, &scope, inner_burn, loop_depth + 1, false)
                }
                other => {
                    for blk in other.blocks() {
                        self.block(blk, &scope, inner_burn, loop_depth, false);
                    }
                }
            }
            if let Some(r) = i.result {
                if !self.defined.insert(r) {
                    self.err("V1", i.span, format!("%{} is defined twice", r.0));
                }
                if self.f.vals.get(r.0 as usize).is_none_or(|v| v.def != i.id) {
                    self.err(
                        "V1",
                        i.span,
                        format!("%{}'s record does not name this instruction", r.0),
                    );
                }
                scope.insert(r);
            }
        }
        if yields
            && !b
                .instrs
                .last()
                .is_some_and(|i| matches!(i.kind, Kind::Yield(_)))
        {
            let span = b.instrs.last().map(|i| i.span).unwrap_or(self.f.span);
            self.err(
                "V3",
                span,
                "a value block that does not end in yield".to_string(),
            );
        }
    }
}

/// Every structural rule, every error (never only the first).
pub fn verify(m: &Module) -> Vec<VerifyError> {
    let mut errors = Vec::new();
    if m.funcs.first().is_none_or(|f| f.kind != FuncKind::Main) {
        errors.push(VerifyError {
            code: "V10",
            severity: Severity::Error,
            func: "main".to_string(),
            span: Span::new(0, 0, 1, 1),
            message: "the first function is not the top level".to_string(),
        });
        return errors;
    }
    let flags = requires_burn(m);
    let mut seen_instr = HashSet::new();
    let mut regions = HashSet::new();
    for f in &m.funcs {
        // V5: the stored flag is the derived one; the top level never.
        let want = flags[f.id.0 as usize] && f.kind != FuncKind::Main;
        if f.requires_burn != want {
            errors.push(VerifyError {
                code: "V5",
                severity: Severity::Error,
                func: fname(f),
                span: f.span,
                message: format!(
                    "requires_burn is {}, the call graph says {want}",
                    f.requires_burn
                ),
            });
        }
        let mut w = Walk {
            m,
            f,
            errors: Vec::new(),
            seen_instr: &mut seen_instr,
            defined: HashSet::new(),
            regions: &mut regions,
            flags: &flags,
        };
        if f.span.line < 1 {
            w.err("V2", f.span, "a function without a span".to_string());
        }
        w.block(&f.body, &HashSet::new(), 0, 0, false);
        errors.extend(w.errors);
    }
    errors
}

/// The errors only (not the V6 warnings).
pub fn errors(found: &[VerifyError]) -> Vec<&VerifyError> {
    found
        .iter()
        .filter(|e| e.severity == Severity::Error)
        .collect()
}

/// Before a pass: every ordered instruction of every function, in order,
/// with the region it sits in.
pub struct Trace {
    funcs: HashMap<FuncId, Vec<(InstrId, RegionId, bool)>>,
}

impl Trace {
    pub fn of(m: &Module) -> Trace {
        let mut funcs = HashMap::new();
        for f in &m.funcs {
            let mut seq = Vec::new();
            trace(&f.body, RegionId::ROOT, &mut seq);
            funcs.insert(f.id, seq);
        }
        Trace { funcs }
    }
}

fn trace(b: &Block, region: RegionId, out: &mut Vec<(InstrId, RegionId, bool)>) {
    for i in &b.instrs {
        if i.kind.is_ordered() {
            out.push((i.id, region, matches!(i.kind, Kind::BurnOp { .. })));
        }
        let inner = match &i.kind {
            Kind::Burn(n) => n.region,
            _ => region,
        };
        for blk in i.kind.blocks() {
            trace(blk, inner, out);
        }
    }
}

/// What a pass may not do: move an ordered instruction to another region
/// (T1), reorder ordered instructions (T2), lose or clone an effect (T3).
pub fn verify_transition(before: &Trace, m: &Module) -> Vec<VerifyError> {
    let after = Trace::of(m);
    let mut errors = Vec::new();
    for f in &m.funcs {
        let (Some(b), Some(a)) = (before.funcs.get(&f.id), after.funcs.get(&f.id)) else {
            continue;
        };
        let mk = |code: &'static str, message: String| VerifyError {
            code,
            severity: Severity::Error,
            func: fname(f),
            span: f.span,
            message,
        };
        let before_ids: HashMap<InstrId, (RegionId, bool)> =
            b.iter().map(|(i, r, e)| (*i, (*r, *e))).collect();
        let after_ids: HashMap<InstrId, RegionId> = a.iter().map(|(i, r, _)| (*i, *r)).collect();
        // T3.
        for (id, (_, effect)) in &before_ids {
            if *effect && !after_ids.contains_key(id) {
                errors.push(mk("T3", format!("burn.op #{} was removed", id.0)));
            }
        }
        let mut count: HashMap<InstrId, usize> = HashMap::new();
        for (id, _, _) in a {
            *count.entry(*id).or_default() += 1;
        }
        for (id, n) in count {
            if n > 1 && before_ids.get(&id).is_some_and(|x| x.1) {
                errors.push(mk("T3", format!("burn.op #{} appears {n} times", id.0)));
            }
        }
        // T1.
        for (id, r) in &after_ids {
            if let Some((rb, _)) = before_ids.get(id) {
                if rb != r {
                    errors.push(mk(
                        "T1",
                        format!("#{} moved from region r{} to r{}", id.0, rb.0, r.0),
                    ));
                }
            }
        }
        // T2: the common instructions in the same order.
        let common_b: Vec<InstrId> = b
            .iter()
            .map(|x| x.0)
            .filter(|i| after_ids.contains_key(i))
            .collect();
        let common_a: Vec<InstrId> = a
            .iter()
            .map(|x| x.0)
            .filter(|i| before_ids.contains_key(i))
            .collect();
        if common_b != common_a {
            let at = common_b
                .iter()
                .zip(&common_a)
                .position(|(x, y)| x != y)
                .unwrap_or(0);
            errors.push(mk(
                "T2",
                format!(
                    "ordered instructions were reordered (first at #{})",
                    common_b.get(at).map(|i| i.0).unwrap_or(0)
                ),
            ));
        }
    }
    errors
}
