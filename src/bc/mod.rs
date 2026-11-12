// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! Stage B's lowering (#31): a verified CIR module to the register
//! bytecode the cig runtime's VM runs (`docs/BYTECODE.md` in cig-script is
//! the contract), for the language the VM runs today: constants, slots
//! and moves; arithmetic, comparison and logic; `if`, `while`, `for`,
//! `break`, `continue`; pulls and lambdas, closures that capture (cells),
//! calls by name, by value, to builtins and to methods, `snuff`; lists,
//! maps, members, indexes and their stores; interpolation and `exhale`;
//! step marks. `try`, `cough`, burns, `pack`, chains and a prelude name
//! used as a value are refused.
//!
//! A program lowers completely or is refused: a construct outside this
//! part stops the lowering with [`Error::Refused`], which names it and
//! its span. Every module that leaves is verified by the format crate's
//! verifier; one that does not verify is [`Error::Internal`], a bug here.
//!
//! How each piece reads:
//! - a slot (`roll`, `stick`, a parameter) is one register for the whole
//!   function; a CIR value is a register from its definition to its last
//!   use, then the register is free again. A value defined before a loop
//!   and read inside it lives to the loop's end;
//! - a call's callee and arguments are moved into a fresh window of
//!   consecutive registers (`call A, B, 0`, `callb A, idx, n`);
//! - `a > b` is `le a, b` then `not`, `a >= b` is `lt a, b` then `not`,
//!   `a != b` is `eq` then `not`: the operands keep their order, so the
//!   interpreter's error wording does too;
//! - a step mark is `steps k, 1` (k the loop nesting where it sits), a
//!   loop starts with `steps k, 0` before its condition and ends with
//!   `steps k-1, 0`, where `break` lands. The iteration's tick is the
//!   body's own first mark, after the condition's test;
//! - a captured slot's register carries a cell, fresh each time its
//!   block runs, in a register nothing else uses;
//! - an index read or store, and a compound member store, check their
//!   operands' types first (`type_of`) so each error is raised at the
//!   position the tree-walker gives it;
//! - every instruction gets the span of the CIR instruction it came from,
//!   and the line table keeps a row wherever the span changes from the
//!   instruction before it.

use crate::cir::ir::{
    self, Block, Class, Const, Func, FuncId, Instr, Kind, Piece, ShortOp, Ty, ValueId, VarId,
};
use cigscript::cig_bytecode::{
    self as bc, verify_module, wide_prefix, Constant, Instr as Word, LineEntry, Op, SpanBase,
    UpvalDesc, UpvalKind, MAX_FUNCTION_DEPTH, MAX_REG_COUNT,
};
use cigscript::syntax::ast::{AssignOp, BinOp, UnOp};
use cigscript::syntax::span::Span;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;

/// Why a module was not lowered.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The program uses a construct this part of the lowering does not
    /// cover yet.
    Refused { construct: String, span: Span },
    /// A function past a limit of the format: its frame, an operand's
    /// widest form, the nesting of functions.
    Limit { what: String, span: Span },
    /// The lowering made a module the verifier refuses: a bug here.
    Internal(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Refused { construct, span } => write!(
                f,
                "{}:{}: {construct} is not lowered to bytecode yet",
                span.line, span.col
            ),
            Error::Limit { what, span } => write!(f, "{}:{}: {what}", span.line, span.col),
            Error::Internal(e) => write!(f, "the bytecode lowering made a bad module: {e}"),
        }
    }
}

/// The construct a CIR instruction kind is, when this part refuses the
/// kind whatever its operands: `None` for a kind it lowers. A kind it
/// lowers can still be refused for what it names (a closure that
/// captures, a `..` range).
pub fn refused_kind(kind: &Kind) -> Option<&'static str> {
    Some(match kind {
        Kind::Step(_)
        | Kind::Const(_)
        | Kind::Load(_)
        | Kind::Decl(..)
        | Kind::Store(..)
        | Kind::Unary(..)
        | Kind::Binary(..)
        | Kind::Short { .. }
        | Kind::MakeList(_)
        | Kind::MakeMap(_)
        | Kind::Key(_)
        | Kind::Interp(_)
        | Kind::Member { .. }
        | Kind::Index { .. }
        | Kind::SetIndex { .. }
        | Kind::SetMember { .. }
        | Kind::Closure(_)
        | Kind::CallBuiltin { .. }
        | Kind::CallUser { .. }
        | Kind::CallValue { .. }
        | Kind::CallMethod { .. }
        | Kind::Exhale(_)
        | Kind::If { .. }
        | Kind::Loop { .. }
        | Kind::ForEach { .. }
        | Kind::Yield(_)
        | Kind::Ret(_)
        | Kind::Break
        | Kind::Continue => return None,
        Kind::Prelude(_) => "a builtin, module or `args` used as a value",
        Kind::BurnOp { .. } => "a call that changes the world (a burn op)",
        Kind::MakeChain { .. } => "a chain",
        Kind::PackDecl(_) => "`pack`",
        Kind::Burn(_) => "`burn`",
        Kind::Try { .. } => "`try`",
        Kind::Throw(_) => "`cough`",
    })
}

/// Lower a verified CIR module to bytecode, and verify what came out.
pub fn lower(m: &ir::Module) -> Result<bc::Module, Error> {
    let catalogue: HashMap<&'static str, u32> = cigscript::stdlib::builtin_catalogue()
        .iter()
        .enumerate()
        .map(|(i, b)| (b.name, i as u32))
        .collect();
    // The slot each pull's closure is declared into, module-wide: a
    // function calls a pull of an enclosing one through that slot's cell.
    let mut pull_slot = HashMap::new();
    for f in &m.funcs {
        pulls(&f.body, &mut HashMap::new(), &mut pull_slot);
    }
    let cx = Cx {
        m,
        catalogue: &catalogue,
        pull_slot,
    };
    let main = m
        .funcs
        .first()
        .ok_or_else(|| Error::Internal("the module has no top level".into()))?;
    let module = bc::Module {
        source_name: m.file.clone(),
        main: cx.function(main, 1)?,
    };
    if let Err(e) = verify_module(&module) {
        return Err(Error::Internal(format!(
            "it does not verify: {e} (rule `{}`)",
            e.rule()
        )));
    }
    Ok(module)
}

/// Parse, check, lower to CIR, run `passes`, lower to bytecode.
pub fn compile(
    source: &str,
    file: &str,
    passes: &[&dyn crate::cir::pass::Pass],
) -> Result<bc::Module, CompileError> {
    let mut m = crate::cir::lower_source(source, file).map_err(CompileError::Lower)?;
    crate::cir::pass::run_passes(&mut m, passes).map_err(CompileError::Verify)?;
    lower(&m).map_err(CompileError::Bytecode)
}

#[derive(Debug)]
pub enum CompileError {
    Lower(crate::cir::LowerError),
    Verify(crate::cir::pass::PassError),
    Bytecode(Error),
}

/// `--emit bc`: the format crate's disassembly, then each function's line
/// table (pre-order, as the disassembly numbers functions): `pc line:col`.
pub fn disassemble(m: &bc::Module) -> String {
    let mut out = bc::disasm::disassemble_module(m);
    let mut fns = Vec::new();
    preorder(&m.main, &mut fns);
    out.push('\n');
    for (i, f) in fns.iter().enumerate() {
        out.push_str(&format!("; fn {i} lines:"));
        for row in &f.lines {
            out.push_str(&format!(
                " {}@{}:{}",
                row.pc,
                f.span_base.line.saturating_add(row.line_off),
                row.col
            ));
        }
        out.push('\n');
    }
    out
}

fn preorder<'f>(f: &'f bc::Function, out: &mut Vec<&'f bc::Function>) {
    out.push(f);
    for c in &f.children {
        preorder(c, out);
    }
}

struct Cx<'m> {
    m: &'m ir::Module,
    catalogue: &'m HashMap<&'static str, u32>,
    pull_slot: HashMap<FuncId, VarId>,
}

impl<'m> Cx<'m> {
    /// Lower one function, narrow jumps first; if a jump's offset does not
    /// fit 16 bits, again with every jump wide.
    fn function(&self, f: &'m Func, depth: u32) -> Result<bc::Function, Error> {
        if depth > MAX_FUNCTION_DEPTH {
            return Err(Error::Limit {
                what: format!(
                    "functions nest more than {MAX_FUNCTION_DEPTH} deep, the bytecode's limit"
                ),
                span: f.span,
            });
        }
        match FnLower::new(self, f, depth, false).run() {
            Err(Retry::Wide) => FnLower::new(self, f, depth, true)
                .run()
                .map_err(|r| match r {
                    Retry::Error(e) => e,
                    Retry::Wide => Error::Internal("a wide jump that does not fit".into()),
                }),
            Err(Retry::Error(e)) => Err(e),
            Ok(x) => Ok(x),
        }
    }
}

enum Retry {
    Wide,
    Error(Error),
}

impl From<Error> for Retry {
    fn from(e: Error) -> Retry {
        Retry::Error(e)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum ConstKey<'a> {
    Int(i64),
    Float(u64),
    Str(&'a str),
}

#[derive(Clone, Copy)]
struct Label(usize);

/// A jump whose offset is written once its target is placed.
struct Patch {
    pc: usize,
    op: Op,
    a: u32,
    wide: bool,
    target: Label,
}

struct LoopCx {
    /// Where `continue` goes: back to the condition (`loop`), or on to
    /// the next item (`jmp`).
    cont: Label,
    cont_op: Op,
    exit: Label,
}

/// Where a slot lives in the function that names it.
#[derive(Clone, Copy)]
enum Slot {
    /// Its own register (a cell when a closure captures it).
    Reg(u32),
    /// An enclosing function's: the upvalue at this index.
    Up(u32),
}

/// Where a `yield` sends its value.
enum YieldTo {
    /// A short-circuit's result register.
    Reg(u32),
    /// A loop's condition: falsy leaves the loop.
    Exit(Label),
}

struct FnLower<'c, 'm> {
    cx: &'c Cx<'m>,
    f: &'m Func,
    depth: u32,
    wide_jumps: bool,
    code: Vec<Word>,
    rows: Vec<(u32, Span)>,
    consts: Vec<Constant>,
    const_index: HashMap<ConstKey<'m>, u32>,
    children: Vec<bc::Function>,
    labels: Vec<Option<usize>>,
    patches: Vec<Patch>,
    /// The frame's length so far, and the registers below it that are
    /// free.
    frame: u32,
    free_regs: BTreeSet<u32>,
    var_reg: HashMap<VarId, u32>,
    val_reg: HashMap<ValueId, u32>,
    /// After the instruction with this position finishes, these values
    /// are dead.
    dies_at: HashMap<usize, Vec<ValueId>>,
    start: HashMap<ir::InstrId, usize>,
    end: HashMap<ir::InstrId, usize>,
    /// The span of each instruction, for an operand's own position.
    def_span: HashMap<ir::InstrId, Span>,
    /// `a..b` values only a `for` reads: lowered as the loop's bounds.
    ranges: HashMap<ValueId, (ValueId, ValueId, Span)>,
    /// `exhale` parts and interpolation pieces to turn to text before or
    /// after an instruction, where the tree-walker evaluates them (keyed
    /// by the instruction, each with the `exhale` or interpolation that
    /// shows it), and the register holding each text.
    show_before: HashMap<ir::InstrId, Vec<(ir::InstrId, ValueId)>>,
    show_after: HashMap<ir::InstrId, Vec<(ir::InstrId, ValueId)>>,
    text_of: HashMap<(ir::InstrId, ValueId), u32>,
    loops: Vec<LoopCx>,
    yields: Vec<YieldTo>,
    span: Span,
}

impl<'c, 'm> FnLower<'c, 'm> {
    fn new(cx: &'c Cx<'m>, f: &'m Func, depth: u32, wide_jumps: bool) -> Self {
        FnLower {
            cx,
            f,
            depth,
            wide_jumps,
            code: Vec::new(),
            rows: Vec::new(),
            consts: Vec::new(),
            const_index: HashMap::new(),
            children: Vec::new(),
            labels: Vec::new(),
            patches: Vec::new(),
            frame: 0,
            free_regs: BTreeSet::new(),
            var_reg: HashMap::new(),
            val_reg: HashMap::new(),
            dies_at: HashMap::new(),
            start: HashMap::new(),
            end: HashMap::new(),
            def_span: HashMap::new(),
            ranges: HashMap::new(),
            show_before: HashMap::new(),
            show_after: HashMap::new(),
            text_of: HashMap::new(),
            loops: Vec::new(),
            yields: Vec::new(),
            span: f.span,
        }
    }

    fn run(mut self) -> Result<bc::Function, Retry> {
        let f = self.f;
        // Parameters are the frame's first registers.
        for p in &f.params {
            let r = self.alloc();
            self.var_reg.insert(*p, r);
        }
        let mut counter = 0;
        number(&f.body, &mut self.start, &mut self.end, &mut counter);
        self.prepass();
        self.liveness();
        // A captured parameter is a cell from the call's start.
        for p in &f.params {
            if self.captured(*p) {
                let r = self.var_reg[p];
                self.abc(Op::CELLNEW, r, 0, 0)?;
            }
        }
        self.block(&f.body)?;
        if !f.body.instrs.last().is_some_and(|i| i.kind.is_terminator()) {
            // The top level ends with no `snuff`: it leaves null.
            let r = self.alloc();
            self.abc(Op::LOADNULL, r, 0, 0)?;
            self.abc(Op::RET, r, 0, 0)?;
        }
        self.patch()?;
        let reg_count = self.frame.max(1);
        if reg_count > MAX_REG_COUNT {
            return Err(Error::Limit {
                what: format!(
                    "the function needs {reg_count} registers, past the frame limit of {MAX_REG_COUNT}"
                ),
                span: f.span,
            }
            .into());
        }
        let (span_base, lines) = self.line_table();
        let kind = match f.kind {
            ir::FuncKind::Main => bc::FuncKind::Main,
            ir::FuncKind::Pull => bc::FuncKind::Pull,
            ir::FuncKind::Lambda => bc::FuncKind::Lambda,
        };
        let arity = u16::try_from(f.params.len()).map_err(|_| Error::Limit {
            what: "a function with more than 65,535 parameters".into(),
            span: f.span,
        })?;
        Ok(bc::Function {
            name: f.name.clone().unwrap_or_default(),
            kind,
            reg_count,
            arity,
            requires_burn: f.requires_burn,
            // The parent fills these in at its `closure`.
            upvalues: Vec::new(),
            constants: self.consts,
            children: self.children,
            regions: Vec::new(),
            handlers: Vec::new(),
            instrs: self.code,
            span_base,
            lines,
            region_rows: Vec::new(),
        })
    }

    // ----- registers -------------------------------------------------------------

    fn alloc(&mut self) -> u32 {
        match self.free_regs.pop_first() {
            Some(r) => r,
            None => {
                self.frame += 1;
                self.frame - 1
            }
        }
    }

    /// `n` consecutive free registers, the lowest run there is: a run of
    /// freed registers, or one that ends where the frame grows.
    fn alloc_window(&mut self, n: u32) -> u32 {
        let mut run: Option<(u32, u32)> = None;
        let mut first = None;
        for &r in &self.free_regs {
            run = match run {
                Some((s, len)) if s + len == r => Some((s, len + 1)),
                _ => Some((r, 1)),
            };
            if let Some((s, len)) = run {
                if len == n {
                    first = Some(s);
                    break;
                }
            }
        }
        let first = first.unwrap_or_else(|| match run {
            // A free run that reaches the frame's end continues past it.
            Some((s, len)) if s + len == self.frame => s,
            _ => self.frame,
        });
        for r in first..first + n {
            if !self.free_regs.remove(&r) && r >= self.frame {
                self.frame = r + 1;
            }
        }
        first
    }

    /// `n` registers past every register used so far, never freed.
    fn fresh(&mut self, n: u32) -> u32 {
        let first = self.frame;
        self.frame += n;
        first
    }

    fn free(&mut self, r: u32) {
        if r < self.frame {
            self.free_regs.insert(r);
        }
    }

    fn var_info(&self, v: VarId) -> Option<&'m ir::VarInfo> {
        self.cx.m.vars.get(v.0 as usize)
    }

    /// A slot this function owns and some other function reads or writes:
    /// its register carries a cell.
    fn captured(&self, v: VarId) -> bool {
        self.var_info(v)
            .is_some_and(|x| x.captured && x.owner == self.f.id)
    }

    /// The register of a slot this function owns.
    fn var(&mut self, v: VarId) -> Result<u32, Error> {
        match self.slot(v)? {
            Slot::Reg(r) => Ok(r),
            Slot::Up(_) => Err(Error::Internal(format!(
                "slot {} is declared by a function that does not own it",
                v.0
            ))),
        }
    }

    fn slot(&mut self, v: VarId) -> Result<Slot, Error> {
        let owner = self.var_info(v).map(|x| x.owner);
        if owner != Some(self.f.id) {
            return match self.f.captures.iter().position(|c| *c == v) {
                Some(u) => Ok(Slot::Up(u as u32)),
                None => Err(Error::Internal(format!(
                    "slot {} of another function is not among this one's captures",
                    v.0
                ))),
            };
        }
        if let Some(r) = self.var_reg.get(&v) {
            return Ok(Slot::Reg(*r));
        }
        // A captured slot's register carries a cell until the frame ends:
        // a register no other value has used or will use, or a write to
        // that value would land in the cell the closures read.
        let r = if self.captured(v) {
            self.fresh(1)
        } else {
            self.alloc()
        };
        self.var_reg.insert(v, r);
        Ok(Slot::Reg(r))
    }

    /// `R(r) = slot`.
    fn read_slot(&mut self, r: u32, v: VarId) -> Result<(), Error> {
        match self.slot(v)? {
            Slot::Reg(s) => self.abc(Op::MOV, r, s, 0),
            Slot::Up(u) => self.abc(Op::GETUP, r, u, 0),
        }
    }

    fn val(&self, v: ValueId) -> Result<u32, Error> {
        self.val_reg
            .get(&v)
            .copied()
            .ok_or_else(|| Error::Internal(format!("%{} is read before it is defined", v.0)))
    }

    fn define(&mut self, i: &Instr) -> Result<u32, Error> {
        let v = i
            .result
            .ok_or_else(|| Error::Internal(format!("instruction {} has no result", i.id.0)))?;
        let r = self.alloc();
        self.val_reg.insert(v, r);
        Ok(r)
    }

    /// What the lowering needs to know before it starts: each
    /// instruction's span, and the `a..b` values only a `for` reads.
    fn prepass(&mut self) {
        let mut range_def: HashMap<ValueId, (ValueId, ValueId, Span)> = HashMap::new();
        let mut for_iters = Vec::new();
        let mut uses: HashMap<ValueId, usize> = HashMap::new();
        let mut spans = HashMap::new();
        walk(&self.f.body, &mut |i| {
            spans.insert(i.id, i.span);
            for u in i.kind.uses() {
                *uses.entry(u).or_default() += 1;
            }
            match &i.kind {
                Kind::Binary(BinOp::Range, a, b) => {
                    if let Some(v) = i.result {
                        range_def.insert(v, (*a, *b, i.span));
                    }
                }
                Kind::ForEach { iter, .. } => for_iters.push(*iter),
                _ => {}
            }
        });
        for it in for_iters {
            if uses.get(&it) == Some(&1) {
                if let Some(r) = range_def.get(&it) {
                    self.ranges.insert(it, *r);
                }
            }
        }
        self.def_span = spans;
    }

    /// The span of the expression that made `v`, when it lies inside
    /// `within` (the instruction that reads it). A pass that replaced the
    /// operand with a value made elsewhere leaves no span for it.
    fn part_span(&self, v: ValueId, within: Span) -> Option<Span> {
        let s = self
            .f
            .vals
            .get(v.0 as usize)
            .and_then(|x| self.def_span.get(&x.def))?;
        (s.start >= within.start && s.end <= within.end).then_some(*s)
    }

    /// Every value's last use, as the position after which its register
    /// is free. A use inside a loop of a value defined before the loop
    /// keeps it to the loop's end: the next turn reads it again.
    fn liveness(&mut self) {
        let mut last: HashMap<ValueId, usize> = HashMap::new();
        let def_pos = |v: ValueId, start: &HashMap<ir::InstrId, usize>| {
            self.f
                .vals
                .get(v.0 as usize)
                .and_then(|x| start.get(&x.def))
                .copied()
                .unwrap_or(0)
        };
        let mut uses: Vec<(ValueId, usize)> = Vec::new();
        collect_uses(
            &self.f.body,
            &self.start,
            &self.end,
            &self.ranges,
            &mut Vec::new(),
            &mut |v, pos, loops: &[LoopRange]| {
                uses.push((v, pos));
                let d = def_pos(v, &self.start);
                // The outermost enclosing loop that started after the
                // definition.
                if let Some(&(_, e)) = loops.iter().find(|(s, _)| *s > d) {
                    uses.push((v, e));
                }
            },
        );
        for (v, p) in uses {
            let e = last.entry(v).or_insert(p);
            *e = (*e).max(p);
        }
        for (i, info) in self.f.vals.iter().enumerate() {
            let v = ValueId(i as u32);
            let at = match last.get(&v) {
                Some(p) => *p,
                None => match self.start.get(&info.def) {
                    Some(s) => *s,
                    None => continue,
                },
            };
            self.dies_at.entry(at).or_default().push(v);
        }
    }

    fn release(&mut self, i: &Instr) {
        for pos in [self.start.get(&i.id), self.end.get(&i.id)]
            .into_iter()
            .flatten()
            .copied()
            .collect::<Vec<_>>()
        {
            if let Some(vs) = self.dies_at.remove(&pos) {
                for v in vs {
                    if let Some(r) = self.val_reg.remove(&v) {
                        self.free(r);
                    }
                }
            }
        }
    }

    // ----- emitting ----------------------------------------------------------------

    fn row(&mut self) {
        self.rows.push((self.code.len() as u32, self.span));
    }

    fn abc(&mut self, op: Op, a: u32, b: u32, c: u32) -> Result<(), Error> {
        if a > 0xFFFF || b > 0xFFFF || c > 0xFFFF {
            return Err(self.too_wide(op));
        }
        self.row();
        if a > 0xFF || b > 0xFF || c > 0xFF {
            self.code.push(wide_prefix(
                (a >> 8) as u8,
                (((c >> 8) << 8) | (b >> 8)) as u16,
            ));
        }
        self.code.push(Word::abc(op, a as u8, b as u8, c as u8));
        Ok(())
    }

    fn abx(&mut self, op: Op, a: u32, bx: u32) -> Result<(), Error> {
        if a > 0xFFFF {
            return Err(self.too_wide(op));
        }
        self.row();
        if a > 0xFF || bx > 0xFFFF {
            self.code
                .push(wide_prefix((a >> 8) as u8, (bx >> 16) as u16));
        }
        self.code.push(Word::abx(op, a as u8, bx as u16));
        Ok(())
    }

    fn too_wide(&self, op: Op) -> Error {
        let what = if self.frame > MAX_REG_COUNT {
            format!("the function needs more than {MAX_REG_COUNT} registers, the frame limit")
        } else {
            format!("`{op}` needs an operand wider than 16 bits")
        };
        Error::Limit {
            what,
            span: self.span,
        }
    }

    fn label(&mut self) -> Label {
        self.labels.push(None);
        Label(self.labels.len() - 1)
    }

    fn place(&mut self, l: Label) {
        self.labels[l.0] = Some(self.code.len());
    }

    /// An AsBx jump (`jmp`, `loop`, `test`, `testn`) to `target`.
    fn jump(&mut self, op: Op, a: u32, target: Label) -> Result<(), Error> {
        if a > 0xFFFF {
            return Err(self.too_wide(op));
        }
        self.row();
        let wide = self.wide_jumps || a > 0xFF;
        let pc = self.code.len();
        if wide {
            self.code.push(wide_prefix(0, 0));
        }
        self.code.push(Word::asbx(op, 0, 0));
        self.patches.push(Patch {
            pc,
            op,
            a,
            wide,
            target,
        });
        Ok(())
    }

    fn patch(&mut self) -> Result<(), Retry> {
        for p in &self.patches {
            let target = self.labels[p.target.0]
                .ok_or_else(|| Error::Internal("a jump to a label never placed".into()))?;
            let off = target as i64 - p.pc as i64 - 1;
            if p.wide {
                let off = i32::try_from(off).map_err(|_| Error::Limit {
                    what: "a jump past 32 bits".into(),
                    span: self.f.span,
                })?;
                let bits = off as u32;
                self.code[p.pc] = wide_prefix((p.a >> 8) as u8, (bits >> 16) as u16);
                self.code[p.pc + 1] = Word::asbx(p.op, p.a as u8, (bits & 0xFFFF) as u16 as i16);
            } else {
                let Ok(off) = i16::try_from(off) else {
                    return Err(Retry::Wide);
                };
                self.code[p.pc] = Word::asbx(p.op, p.a as u8, off);
            }
        }
        Ok(())
    }

    fn konst(&mut self, k: ConstKey<'m>) -> u32 {
        if let Some(i) = self.const_index.get(&k) {
            return *i;
        }
        let i = self.consts.len() as u32;
        self.consts.push(match k {
            ConstKey::Int(x) => Constant::Int(x),
            ConstKey::Float(b) => Constant::Float(f64::from_bits(b)),
            ConstKey::Str(s) => Constant::Str(s.to_string()),
        });
        self.const_index.insert(k, i);
        i
    }

    /// The line table: a row wherever the position changes from the
    /// instruction before, relative to the function's first position.
    fn line_table(&self) -> (SpanBase, Vec<LineEntry>) {
        let first = self
            .rows
            .iter()
            .map(|r| r.1)
            .chain(std::iter::once(self.f.span))
            // By line and column: an interpolated piece's byte offsets
            // count from its string, not from the file.
            .min_by_key(|s| (s.line, s.col))
            .unwrap_or(self.f.span);
        let base = SpanBase {
            byte_offset: first.start as u32,
            line: first.line,
            col: first.col,
        };
        let mut lines: Vec<LineEntry> = Vec::new();
        let mut prev: Option<Span> = None;
        for (pc, s) in &self.rows {
            if prev == Some(*s) || lines.last().is_some_and(|l| l.pc == *pc) {
                continue;
            }
            prev = Some(*s);
            lines.push(LineEntry {
                pc: *pc,
                span_off: (s.start as u32).saturating_sub(base.byte_offset),
                span_len: (s.end.saturating_sub(s.start)) as u32,
                line_off: s.line.saturating_sub(base.line),
                col: s.col,
            });
        }
        (base, lines)
    }

    fn nest(&self) -> u32 {
        self.loops.len() as u32
    }

    // ----- lowering -----------------------------------------------------------------

    fn block(&mut self, b: &'m Block) -> Result<(), Error> {
        // A captured slot declared here gets a fresh cell each time the
        // block runs (each turn of a loop), before anything, a hoisted
        // pull included, can capture it.
        let mut fresh = HashSet::new();
        for i in &b.instrs {
            if let Kind::Decl(v, _) = &i.kind {
                if self.captured(*v) && fresh.insert(*v) {
                    self.span = i.span;
                    // The new cell first: a null written to the
                    // register before it would land in the last turn's
                    // cell, which that turn's closures still read.
                    let r = self.var(*v)?;
                    self.abc(Op::CELLNEW, r, 0, 0)?;
                    self.abc(Op::LOADNULL, r, 0, 0)?;
                }
            }
        }
        for j in 0..b.instrs.len() {
            self.schedule_shows(b, j);
        }
        for (j, i) in b.instrs.iter().enumerate() {
            if let Kind::MakeMap(kv) = &i.kind {
                self.computed_keys(b, j, kv, i.span)?;
            }
            self.instr(i)?;
            self.release(i);
        }
        Ok(())
    }

    /// The tree-walker checks a computed map key before it evaluates the
    /// next value; CIR evaluates every key and value first, and
    /// `insertmap` checks the key then. The two agree only when nothing
    /// between the first computed key and the map can fail, run code or
    /// change a slot.
    fn computed_keys(
        &self,
        b: &Block,
        j: usize,
        kv: &[(ir::MapKey, ValueId)],
        span: Span,
    ) -> Result<(), Error> {
        let Some(k) = kv.iter().find_map(|(k, _)| match k {
            ir::MapKey::Dyn(k, _) => Some(*k),
            ir::MapKey::Static(_) => None,
        }) else {
            return Ok(());
        };
        let def = self.f.vals.get(k.0 as usize).map(|x| x.def);
        let from = b.instrs[..j].iter().position(|i| Some(i.id) == def);
        let quiet = from.is_some_and(|p| {
            b.instrs[p + 1..j].iter().all(|i| {
                matches!(i.kind, Kind::Load(_) | Kind::Key(_))
                    || i.kind.class(self.f, &self.cx.m.vars) == Class::Pure
            })
        });
        if quiet {
            Ok(())
        } else {
            Err(self.refuse(
                "a computed map key followed by a value that can fail or run code",
                span,
            ))
        }
    }

    fn refuse(&self, construct: impl Into<String>, span: Span) -> Error {
        Error::Refused {
            construct: construct.into(),
            span,
        }
    }

    fn instr(&mut self, i: &'m Instr) -> Result<(), Error> {
        if let Some(vs) = self.show_before.remove(&i.id) {
            self.span = i.span;
            self.show_as_text(&vs)?;
        }
        self.instr_kind(i)?;
        if let Some(vs) = self.show_after.remove(&i.id) {
            self.span = i.span;
            self.show_as_text(&vs)?;
        }
        Ok(())
    }

    fn show_as_text(&mut self, vs: &[(ir::InstrId, ValueId)]) -> Result<(), Error> {
        for (owner, v) in vs {
            let x = self.val(*v)?;
            let t = self.alloc();
            self.abc(Op::INTERP, t, x, 0)?;
            self.text_of.insert((*owner, *v), t);
        }
        Ok(())
    }

    /// The tree-walker shows each `exhale` part and interpolation piece as
    /// soon as it is evaluated; the instruction shows them all at the end.
    /// A part that is not a scalar, with a later part after it, is turned
    /// to text where the tree-walker would have shown it: after the code
    /// that makes it, or, for a value made elsewhere (a pass replaced the
    /// read), after the previous part's code, or before the first part's.
    fn schedule_shows(&mut self, b: &Block, j: usize) {
        let i = &b.instrs[j];
        let parts: Vec<ValueId> = match &i.kind {
            Kind::Interp(ps) => ps
                .iter()
                .filter_map(|p| match p {
                    Piece::Val(v) => Some(*v),
                    Piece::Lit(_) => None,
                })
                .collect(),
            Kind::Exhale(vs) => vs.clone(),
            _ => return,
        };
        // The parts' own code: the instructions just before it that start
        // at or after its own start, up to the statement's step mark.
        // Lines and columns, not bytes: an interpolated piece's byte span
        // counts from its string.
        let mut k = j;
        while k > 0 && {
            let p = &b.instrs[k - 1];
            !matches!(p.kind, Kind::Step(_))
                && (p.span.line, p.span.col) >= (i.span.line, i.span.col)
        } {
            k -= 1;
        }
        let code = &b.instrs[k..j];
        let def_of = |v: &ValueId| self.f.vals.get(v.0 as usize).map(|x| x.def);
        let mut anchor: Option<ir::InstrId> = None;
        for (n, v) in parts.iter().enumerate() {
            let def = def_of(v).filter(|d| code.iter().any(|c| c.id == *d));
            let scalar = self.f.vals.get(v.0 as usize).is_some_and(|x| {
                matches!(x.ty, Ty::Str | Ty::Int | Ty::Float | Ty::Bool | Ty::Null)
            });
            if n + 1 < parts.len() && !scalar {
                let at = (i.id, *v);
                match (def, anchor, code.first()) {
                    (Some(d), _, _) => self.show_after.entry(d).or_default().push(at),
                    (None, Some(a), _) => self.show_after.entry(a).or_default().push(at),
                    (None, None, Some(c)) => self.show_before.entry(c.id).or_default().push(at),
                    // No code runs before the instruction: it shows it.
                    (None, None, None) => {}
                }
            }
            if def.is_some() {
                anchor = def;
            }
        }
    }

    fn instr_kind(&mut self, i: &'m Instr) -> Result<(), Error> {
        self.span = i.span;
        if let Some(c) = refused_kind(&i.kind) {
            return Err(self.refuse(c, i.span));
        }
        match &i.kind {
            Kind::Step(_) => {
                let k = self.nest();
                self.abx(Op::STEPS, k, 1)?;
            }
            Kind::Const(c) => {
                let r = self.define(i)?;
                match c {
                    Const::Null => self.abc(Op::LOADNULL, r, 0, 0)?,
                    Const::Bool(b) => self.abc(Op::LOADBOOL, r, u32::from(*b), 0)?,
                    Const::Int(x) => self.load_int(r, *x)?,
                    Const::Float(x) => {
                        let k = self.konst(ConstKey::Float(x.to_bits()));
                        self.abx(Op::LOADK, r, k)?;
                    }
                    Const::Str(s) => {
                        let k = self.konst(ConstKey::Str(s));
                        self.abx(Op::LOADK, r, k)?;
                    }
                }
            }
            Kind::Load(v) => {
                let r = self.define(i)?;
                self.read_slot(r, *v)?;
            }
            Kind::Decl(v, x) | Kind::Store(v, x) => {
                let x = self.val(*x)?;
                match self.slot(*v)? {
                    Slot::Reg(s) => self.abc(Op::MOV, s, x, 0)?,
                    Slot::Up(u) => self.abc(Op::SETUP, u, x, 0)?,
                }
            }
            // `insertmap` checks the key, as before.
            Kind::Key(k) => {
                let k = self.val(*k)?;
                let r = self.define(i)?;
                self.abc(Op::MOV, r, k, 0)?;
            }
            Kind::Unary(op, x) => {
                let x = self.val(*x)?;
                let r = self.define(i)?;
                let op = match op {
                    UnOp::Neg => Op::NEG,
                    UnOp::Not => Op::NOT,
                };
                self.abc(op, r, x, 0)?;
            }
            Kind::Binary(op, a, b) => {
                let (a, b) = (self.val(*a)?, self.val(*b)?);
                let (code, negate) = match op {
                    BinOp::Add => (Op::ADD, false),
                    BinOp::Sub => (Op::SUB, false),
                    BinOp::Mul => (Op::MUL, false),
                    BinOp::Div => (Op::DIV, false),
                    BinOp::Rem => (Op::MOD, false),
                    BinOp::Eq => (Op::EQ, false),
                    BinOp::Ne => (Op::EQ, true),
                    BinOp::Lt => (Op::LT, false),
                    BinOp::Le => (Op::LE, false),
                    BinOp::Gt => (Op::LE, true),
                    BinOp::Ge => (Op::LT, true),
                    // Read by a `for` as its bounds: nothing to make here.
                    BinOp::Range if i.result.is_some_and(|v| self.ranges.contains_key(&v)) => {
                        return Ok(())
                    }
                    BinOp::Range => {
                        let r = self.define(i)?;
                        return self.range_value(r, a, b);
                    }
                    BinOp::And | BinOp::Or | BinOp::Coalesce => {
                        return Err(Error::Internal(format!(
                            "`{}` as a plain binary operator",
                            op.symbol()
                        )))
                    }
                };
                let r = self.define(i)?;
                self.abc(code, r, a, b)?;
                if negate {
                    self.abc(Op::NOT, r, r, 0)?;
                }
            }
            Kind::Short { op, subject, rhs } => {
                let s = self.val(*subject)?;
                let r = self.define(i)?;
                let end = self.label();
                self.abc(Op::MOV, r, s, 0)?;
                match op {
                    // `and`: a falsy subject is the result.
                    ShortOp::And => self.jump(Op::TESTN, r, end)?,
                    // `or`: a truthy subject is the result.
                    ShortOp::Or => self.jump(Op::TEST, r, end)?,
                    // `??`: a subject that is not null is the result;
                    // `?.`: a null subject is the result.
                    ShortOp::Coalesce | ShortOp::Safe => {
                        let t = self.alloc();
                        self.abc(Op::LOADNULL, t, 0, 0)?;
                        self.abc(Op::EQ, t, r, t)?;
                        let op = if *op == ShortOp::Coalesce {
                            Op::TESTN
                        } else {
                            Op::TEST
                        };
                        self.jump(op, t, end)?;
                        self.free(t);
                    }
                }
                self.yields.push(YieldTo::Reg(r));
                self.block(rhs)?;
                self.yields.pop();
                self.place(end);
            }
            Kind::Closure(fid) => {
                let target = self
                    .cx
                    .m
                    .funcs
                    .get(fid.0 as usize)
                    .ok_or_else(|| Error::Internal(format!("no function {}", fid.0)))?;
                let mut child = self.cx.function(target, self.depth + 1)?;
                self.span = i.span;
                // One upvalue per captured slot, in the child's capture
                // order: this function's register (its cell), or this
                // function's own upvalue.
                for c in &target.captures {
                    child.upvalues.push(match self.slot(*c)? {
                        Slot::Reg(r) => UpvalDesc {
                            kind: UpvalKind::ParentLocal,
                            index: u16::try_from(r).map_err(|_| self.too_wide(Op::CLOSURE))?,
                        },
                        Slot::Up(u) => UpvalDesc {
                            kind: UpvalKind::ParentUpvalue,
                            index: u16::try_from(u).map_err(|_| self.too_wide(Op::CLOSURE))?,
                        },
                    });
                }
                let index = self.children.len() as u32;
                self.children.push(child);
                let r = self.define(i)?;
                self.abx(Op::CLOSURE, r, index)?;
            }
            Kind::CallBuiltin { callee, args } => {
                let index = *self.cx.catalogue.get(callee.as_str()).ok_or_else(|| {
                    self.refuse(
                        format!("`{callee}`, a builtin this cig does not carry"),
                        i.span,
                    )
                })?;
                let w = self.window(None, args)?;
                self.abc(Op::CALLB, w, index, args.len() as u32)?;
                self.call_result(i, w, args.len());
            }
            Kind::CallUser { func, args } => {
                let slot = *self.cx.pull_slot.get(func).ok_or_else(|| {
                    Error::Internal(format!("no slot holds the closure of function {}", func.0))
                })?;
                let t = self.alloc();
                self.read_slot(t, slot)?;
                let w = self.window(Some(t), args)?;
                self.free(t);
                self.abc(Op::CALL, w, args.len() as u32, 0)?;
                self.call_result(i, w, args.len());
            }
            Kind::CallValue { f, args } => {
                let callee = self.val(*f)?;
                let w = self.window(Some(callee), args)?;
                self.abc(Op::CALL, w, args.len() as u32, 0)?;
                self.call_result(i, w, args.len());
            }
            Kind::If {
                cond,
                then,
                otherwise,
            } => {
                let c = self.val(*cond)?;
                let other = self.label();
                self.jump(Op::TESTN, c, other)?;
                self.block(then)?;
                match otherwise {
                    Some(o) => {
                        let end = self.label();
                        if !ends(then) {
                            self.span = i.span;
                            self.jump(Op::JMP, 0, end)?;
                        }
                        self.place(other);
                        self.block(o)?;
                        self.place(end);
                    }
                    None => self.place(other),
                }
            }
            Kind::Loop { cond, body } => {
                let outer = self.nest();
                let (head, exit) = (self.label(), self.label());
                self.abx(Op::STEPS, outer + 1, 0)?;
                self.loops.push(LoopCx {
                    cont: head,
                    cont_op: Op::LOOP,
                    exit,
                });
                self.place(head);
                self.yields.push(YieldTo::Exit(exit));
                self.block(cond)?;
                self.yields.pop();
                self.block(body)?;
                if !ends(body) {
                    self.span = i.span;
                    self.jump(Op::LOOP, 0, head)?;
                }
                self.loops.pop();
                self.place(exit);
                self.span = i.span;
                self.abx(Op::STEPS, outer, 0)?;
            }
            Kind::Yield(v) => {
                let x = self.val(*v)?;
                match self.yields.last() {
                    Some(YieldTo::Reg(r)) => {
                        let r = *r;
                        self.abc(Op::MOV, r, x, 0)?;
                    }
                    Some(YieldTo::Exit(l)) => {
                        let l = *l;
                        self.jump(Op::TESTN, x, l)?;
                    }
                    None => return Err(Error::Internal("a `yield` outside a value block".into())),
                }
            }
            Kind::Ret(v) => {
                let x = self.val(*v)?;
                self.abc(Op::RET, x, 0, 0)?;
            }
            Kind::Break | Kind::Continue => {
                let Some(l) = self.loops.last() else {
                    return Err(Error::Internal(
                        "`break` or `continue` outside a loop".into(),
                    ));
                };
                let (op, to) = match i.kind {
                    Kind::Break => (Op::JMP, l.exit),
                    _ => (l.cont_op, l.cont),
                };
                self.jump(op, 0, to)?;
            }
            Kind::ForEach { iter, var, body } => self.for_each(i, *iter, *var, body)?,
            Kind::MakeList(vs) => {
                let regs = self.vals(vs)?;
                let r = self.define(i)?;
                self.abc(Op::NEWLIST, r, (vs.len() as u32).min(0xFFFF), 0)?;
                for x in regs {
                    self.abc(Op::APPENDLIST, r, x, 0)?;
                }
            }
            Kind::MakeMap(kv) => {
                let r = self.define(i)?;
                self.abc(Op::NEWMAP, r, (kv.len() as u32).min(0xFFFF), 0)?;
                for (k, x) in kv {
                    let x = self.val(*x)?;
                    match k {
                        ir::MapKey::Static(name) => {
                            let t = self.alloc();
                            let c = self.konst(ConstKey::Str(name));
                            self.abx(Op::LOADK, t, c)?;
                            self.abc(Op::INSERTMAP, r, t, x)?;
                            self.free(t);
                        }
                        ir::MapKey::Dyn(key, _) => {
                            // A key that is not a string or an int is an
                            // error at the key's own position.
                            let at = self.part_span(*key, i.span).ok_or_else(|| {
                                self.refuse(
                                    "a computed map key a pass replaced (its position is lost)",
                                    i.span,
                                )
                            })?;
                            let key = self.val(*key)?;
                            self.span = at;
                            self.abc(Op::INSERTMAP, r, key, x)?;
                            self.span = i.span;
                        }
                    }
                }
            }
            Kind::Interp(pieces) => {
                let mut regs = Vec::new();
                for p in pieces {
                    regs.push(match p {
                        Piece::Lit(l) => Err(l.as_str()),
                        Piece::Val(v) => Ok(self.shown(i.id, *v)?),
                    });
                }
                let r = self.define(i)?;
                let n = regs.len().max(1) as u32;
                let w = self.alloc_window(n);
                if regs.is_empty() {
                    let c = self.konst(ConstKey::Str(""));
                    self.abx(Op::LOADK, w, c)?;
                }
                let mut texts = Vec::new();
                for (k, x) in regs.into_iter().enumerate() {
                    match x {
                        Ok((x, text)) => {
                            self.abc(Op::MOV, w + k as u32, x, 0)?;
                            if text {
                                texts.push(x);
                            }
                        }
                        Err(l) => {
                            let c = self.konst(ConstKey::Str(l));
                            self.abx(Op::LOADK, w + k as u32, c)?;
                        }
                    }
                }
                self.abc(Op::INTERP, r, w, n - 1)?;
                for k in 0..n {
                    self.free(w + k);
                }
                for t in texts {
                    self.free(t);
                }
            }
            Kind::Exhale(vs) => {
                let mut regs = Vec::new();
                for v in vs {
                    regs.push(self.shown(i.id, *v)?);
                }
                let w = self.alloc_window((regs.len() as u32).max(1));
                for (k, (x, _)) in regs.iter().enumerate() {
                    self.abc(Op::MOV, w + k as u32, *x, 0)?;
                }
                self.abc(Op::EXHALE, w, regs.len() as u32, 0)?;
                for k in 0..(regs.len() as u32).max(1) {
                    self.free(w + k);
                }
                for (x, text) in regs {
                    if text {
                        self.free(x);
                    }
                }
            }
            Kind::Member { obj, name, safe } => {
                let o = self.val(*obj)?;
                let r = self.define(i)?;
                let k = self.konst(ConstKey::Str(name));
                if *safe {
                    self.safe_member(r, o, name, k)?;
                } else {
                    self.abc(Op::GETFIELD, r, o, k)?;
                }
            }
            Kind::Index { obj, idx, .. } => {
                let at = self.index_span(*idx, i.span)?;
                let (o, x) = (self.val(*obj)?, self.val(*idx)?);
                let r = self.define(i)?;
                self.index_read(r, o, x, at)?;
            }
            Kind::SetIndex {
                obj, idx, op, val, ..
            } => {
                let at = self.index_span(*idx, i.span)?;
                let obj_at = self.part_span(*obj, i.span).unwrap_or(i.span);
                let (o, x, v) = (self.val(*obj)?, self.val(*idx)?, self.val(*val)?);
                self.index_store(o, x, v, *op, at, obj_at)?;
            }
            Kind::SetMember { obj, name, op, val } => {
                let obj_at = self.part_span(*obj, i.span).unwrap_or(i.span);
                let (o, v) = (self.val(*obj)?, self.val(*val)?);
                let k = self.konst(ConstKey::Str(name));
                self.member_store(o, name, k, v, *op, obj_at)?;
            }
            Kind::CallMethod { recv, name, args } => {
                let recv = self.val(*recv)?;
                let k = self.konst(ConstKey::Str(name));
                let w = self.window(Some(recv), args)?;
                self.abc(Op::CALLM, w, k, args.len() as u32)?;
                self.call_result(i, w, args.len());
            }
            _ => return Err(Error::Internal("a refused kind got past the table".into())),
        }
        Ok(())
    }

    /// `R(r) = x`: `loadint` where it fits sixteen bits, else a constant.
    fn load_int(&mut self, r: u32, x: i64) -> Result<(), Error> {
        let Ok(small) = i16::try_from(x) else {
            let k = self.konst(ConstKey::Int(x));
            return self.abx(Op::LOADK, r, k);
        };
        if r > 0xFFFF {
            return Err(self.too_wide(Op::LOADINT));
        }
        self.row();
        if r > 0xFF {
            self.code.push(wide_prefix(
                (r >> 8) as u8,
                if small < 0 { 0xFFFF } else { 0 },
            ));
        }
        self.code.push(Word::asbx(Op::LOADINT, r as u8, small));
        Ok(())
    }

    fn vals(&self, vs: &[ValueId]) -> Result<Vec<u32>, Error> {
        vs.iter().map(|v| self.val(*v)).collect()
    }

    /// The register an `exhale` part or interpolation piece shows: its
    /// text, when it was turned to text where it was made.
    fn shown(&mut self, owner: ir::InstrId, v: ValueId) -> Result<(u32, bool), Error> {
        match self.text_of.remove(&(owner, v)) {
            Some(t) => Ok((t, true)),
            None => Ok((self.val(v)?, false)),
        }
    }

    /// The index expression's own position, where the tree-walker puts
    /// its type errors.
    fn index_span(&self, idx: ValueId, within: Span) -> Result<Span, Error> {
        self.part_span(idx, within)
            .ok_or_else(|| self.refuse("an index a pass replaced (its position is lost)", within))
    }

    /// `type_of(R(x))` in a fresh register.
    fn type_name_of(&mut self, x: u32) -> Result<u32, Error> {
        let f = *self
            .cx
            .catalogue
            .get("type_of")
            .ok_or_else(|| Error::Internal("no `type_of` in the catalogue".into()))?;
        let w = self.alloc_window(2);
        self.abc(Op::MOV, w + 1, x, 0)?;
        self.abc(Op::CALLB, w, f, 1)?;
        self.free(w + 1);
        Ok(w)
    }

    /// Jump to `to` when the type name in `R(t)` is (or, with `is`
    /// false, is not) `name`.
    fn jump_if_is(&mut self, t: u32, name: &'static str, to: Label, is: bool) -> Result<(), Error> {
        let k = self.konst(ConstKey::Str(name));
        let c = self.alloc();
        self.abc(Op::EQK, c, t, k)?;
        self.jump(if is { Op::TEST } else { Op::TESTN }, c, to)?;
        self.free(c);
        Ok(())
    }

    /// An instruction at another position than the current one.
    fn at(
        &mut self,
        span: Span,
        f: impl FnOnce(&mut Self) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let keep = self.span;
        self.span = span;
        let r = f(self);
        self.span = keep;
        r
    }

    /// `R(r) = R(o)[R(x)]`. The tree-walker puts a wrong index type for a
    /// list, a string or a map at the index's own position (`at`), and
    /// every other error at the expression's: the type of each operand
    /// picks which of two `getindex` runs.
    fn index_read(&mut self, r: u32, o: u32, x: u32, at: Span) -> Result<(), Error> {
        let to = self.type_name_of(o)?;
        let ti = self.type_name_of(x)?;
        let (seq, map, plain, bad, end) = (
            self.label(),
            self.label(),
            self.label(),
            self.label(),
            self.label(),
        );
        self.jump_if_is(to, "map", map, true)?;
        self.jump_if_is(to, "list", seq, true)?;
        self.jump_if_is(to, "string", plain, false)?;
        self.place(seq);
        self.jump_if_is(ti, "int", plain, true)?;
        self.jump(Op::JMP, 0, bad)?;
        self.place(map);
        self.jump_if_is(ti, "string", plain, true)?;
        self.place(bad);
        self.at(at, |s| s.abc(Op::GETINDEX, r, o, x))?;
        self.jump(Op::JMP, 0, end)?;
        self.place(plain);
        self.abc(Op::GETINDEX, r, o, x)?;
        self.place(end);
        self.free(to);
        self.free(ti);
        Ok(())
    }

    /// `R(o)[R(x)] op= R(v)`. The tree-walker puts a wrong index type for
    /// a list or a map at the index (`at`), a value that cannot be
    /// indexed at the object (`obj_at`), and the rest (`E504`, a missing
    /// key in a compound, the operator's own error) at the statement. A
    /// compound reads with `getindex` (the same `E504` and `E505`, with
    /// no hint) only once the store is known to be one that can happen.
    fn index_store(
        &mut self,
        o: u32,
        x: u32,
        v: u32,
        op: AssignOp,
        at: Span,
        obj_at: Span,
    ) -> Result<(), Error> {
        let to = self.type_name_of(o)?;
        let ti = self.type_name_of(x)?;
        let (seq, map, ok, bad, end) = (
            self.label(),
            self.label(),
            self.label(),
            self.label(),
            self.label(),
        );
        self.jump_if_is(to, "list", seq, true)?;
        self.jump_if_is(to, "map", map, true)?;
        self.at(obj_at, |s| s.abc(Op::SETINDEX, o, x, v))?;
        self.jump(Op::JMP, 0, end)?;
        self.place(seq);
        self.jump_if_is(ti, "int", ok, true)?;
        self.jump(Op::JMP, 0, bad)?;
        self.place(map);
        self.jump_if_is(ti, "string", ok, true)?;
        self.place(bad);
        self.at(at, |s| s.abc(Op::SETINDEX, o, x, v))?;
        self.jump(Op::JMP, 0, end)?;
        self.place(ok);
        match compound(op) {
            None => self.abc(Op::SETINDEX, o, x, v)?,
            Some(code) => {
                let t = self.alloc();
                self.abc(Op::GETINDEX, t, o, x)?;
                self.abc(code, t, t, v)?;
                self.abc(Op::SETINDEX, o, x, t)?;
                self.free(t);
            }
        }
        self.place(end);
        self.free(to);
        self.free(ti);
        Ok(())
    }

    /// `R(o).name op= R(v)`: `setfield`'s type error is at the object. A
    /// compound on a map reads with `getindex`, whose `E505` carries no
    /// hint, as the tree-walker's compound assignment does not.
    fn member_store(
        &mut self,
        o: u32,
        name: &'m str,
        k: u32,
        v: u32,
        op: AssignOp,
        obj_at: Span,
    ) -> Result<(), Error> {
        let Some(code) = compound(op) else {
            return self.at(obj_at, |s| s.abc(Op::SETFIELD, o, k, v));
        };
        let to = self.type_name_of(o)?;
        let (map, end) = (self.label(), self.label());
        self.jump_if_is(to, "map", map, true)?;
        self.at(obj_at, |s| s.abc(Op::SETFIELD, o, k, v))?;
        self.jump(Op::JMP, 0, end)?;
        self.place(map);
        let (t, key) = (self.alloc(), self.alloc());
        let c = self.konst(ConstKey::Str(name));
        self.abx(Op::LOADK, key, c)?;
        self.abc(Op::GETINDEX, t, o, key)?;
        self.abc(code, t, t, v)?;
        self.at(obj_at, |s| s.abc(Op::SETFIELD, o, k, t))?;
        self.place(end);
        self.free(t);
        self.free(key);
        self.free(to);
        Ok(())
    }

    /// `R(r) = R(o)?.name`: null for null and for a map without the key
    /// (`getfield` is the plain `.`), the plain member read otherwise.
    fn safe_member(&mut self, r: u32, o: u32, name: &'m str, k: u32) -> Result<(), Error> {
        let keys = *self
            .cx
            .catalogue
            .get("keys")
            .ok_or_else(|| Error::Internal("no `keys` in the catalogue".into()))?;
        let to = self.type_name_of(o)?;
        let (null, map, end) = (self.label(), self.label(), self.label());
        self.jump_if_is(to, "null", null, true)?;
        self.jump_if_is(to, "map", map, true)?;
        self.abc(Op::GETFIELD, r, o, k)?;
        self.jump(Op::JMP, 0, end)?;
        self.place(map);
        // `keys(o).contains(name)`: a list's method, which a map's own
        // entries cannot stand in for.
        let w = self.alloc_window(2);
        self.abc(Op::MOV, w + 1, o, 0)?;
        self.abc(Op::CALLB, w, keys, 1)?;
        let c = self.konst(ConstKey::Str(name));
        self.abx(Op::LOADK, w + 1, c)?;
        let contains = self.konst(ConstKey::Str("contains"));
        self.abc(Op::CALLM, w, contains, 1)?;
        self.jump(Op::TESTN, w, null)?;
        self.free(w);
        self.free(w + 1);
        self.abc(Op::GETFIELD, r, o, k)?;
        self.jump(Op::JMP, 0, end)?;
        self.place(null);
        self.abc(Op::LOADNULL, r, 0, 0)?;
        self.place(end);
        self.free(to);
        Ok(())
    }

    /// `R(r) = R(a)..R(b)`, the list. `forprep` checks the bounds as
    /// `a..b` does (`E403` for anything but two ints, `E512` past
    /// 10,000,000) and, either way, goes on to the next instruction: no
    /// loop, so no step to charge. Then two ints that fit are `range(a,
    /// b)`, which makes the same list, or an empty one when `b <= a`.
    fn range_value(&mut self, r: u32, a: u32, b: u32) -> Result<(), Error> {
        let range = *self
            .cx
            .catalogue
            .get("range")
            .ok_or_else(|| Error::Internal("no `range` in the catalogue".into()))?;
        let w = self.alloc_window(4);
        self.abc(Op::MOV, w, a, 0)?;
        self.abc(Op::MOV, w + 1, b, 0)?;
        self.load_int(w + 2, 1)?;
        let next = self.label();
        self.jump(Op::FORPREP, w, next)?;
        self.place(next);
        let (empty, end) = (self.label(), self.label());
        let c = self.alloc();
        self.abc(Op::LE, c, b, a)?;
        self.jump(Op::TEST, c, empty)?;
        self.free(c);
        let args = self.alloc_window(3);
        self.abc(Op::MOV, args + 1, a, 0)?;
        self.abc(Op::MOV, args + 2, b, 0)?;
        self.abc(Op::CALLB, args, range, 2)?;
        self.abc(Op::MOV, r, args, 0)?;
        self.jump(Op::JMP, 0, end)?;
        self.place(empty);
        self.abc(Op::NEWLIST, r, 0, 0)?;
        self.place(end);
        for k in 0..4 {
            self.free(w + k);
        }
        for k in 0..3 {
            self.free(args + k);
        }
        Ok(())
    }

    /// `for x in v`: over `a..b` with `forprep`/`forloop`, anything else
    /// with `iterprep`/`iternext`. The item is `R(A+3)`; a captured one
    /// takes a fresh cell at the top of each turn. The body's first
    /// instruction is its `step.iter`, after the exit test, so every turn
    /// is charged.
    fn for_each(
        &mut self,
        i: &'m Instr,
        iter: ValueId,
        var: VarId,
        body: &'m Block,
    ) -> Result<(), Error> {
        let outer = self.nest();
        let (cont, exit) = (self.label(), self.label());
        let captured = self.captured(var);
        // A captured item's window is the item's alone, as a captured
        // slot's register is.
        let a = if captured {
            self.fresh(4)
        } else {
            self.alloc_window(4)
        };
        self.var_reg.insert(var, a + 3);
        if let Some(&(lo, hi, range_span)) = self.ranges.get(&iter) {
            let (lo, hi) = (self.val(lo)?, self.val(hi)?);
            self.abc(Op::MOV, a, lo, 0)?;
            self.abc(Op::MOV, a + 1, hi, 0)?;
            self.load_int(a + 2, 1)?;
            self.abx(Op::STEPS, outer + 1, 0)?;
            // `E403` and `E512` are the range's, at its position.
            self.span = range_span;
            self.jump(Op::FORPREP, a, exit)?;
            self.span = i.span;
            let top = self.label();
            self.place(top);
            self.loops.push(LoopCx {
                cont,
                cont_op: Op::JMP,
                exit,
            });
            if captured {
                self.abc(Op::CELLNEW, a + 3, 0, 0)?;
            }
            self.block(body)?;
            self.loops.pop();
            self.place(cont);
            self.span = i.span;
            self.jump(Op::FORLOOP, a, top)?;
        } else {
            let it = self.val(iter)?;
            self.abc(Op::MOV, a, it, 0)?;
            // `E405` is at the `for`.
            self.abc(Op::ITERPREP, a, 0, 0)?;
            self.abx(Op::STEPS, outer + 1, 0)?;
            let head = self.label();
            self.place(head);
            self.jump(Op::ITERNEXT, a, exit)?;
            self.loops.push(LoopCx {
                cont,
                cont_op: Op::JMP,
                exit,
            });
            if captured {
                self.abc(Op::CELLNEW, a + 3, 0, 0)?;
            }
            self.block(body)?;
            self.loops.pop();
            self.place(cont);
            self.span = i.span;
            self.jump(Op::LOOP, 0, head)?;
        }
        self.place(exit);
        self.span = i.span;
        self.abx(Op::STEPS, outer, 0)?;
        if !captured {
            self.var_reg.remove(&var);
            for k in 0..4 {
                self.free(a + k);
            }
        }
        Ok(())
    }

    /// A call's window: the callee (when there is one) and the arguments
    /// moved into consecutive registers; `callb` leaves the first one for
    /// its result.
    fn window(&mut self, callee: Option<u32>, args: &[ValueId]) -> Result<u32, Error> {
        let regs: Vec<u32> = args
            .iter()
            .map(|a| self.val(*a))
            .collect::<Result<_, _>>()?;
        let w = self.alloc_window(args.len() as u32 + 1);
        if let Some(c) = callee {
            self.abc(Op::MOV, w, c, 0)?;
        }
        for (k, r) in regs.into_iter().enumerate() {
            self.abc(Op::MOV, w + 1 + k as u32, r, 0)?;
        }
        Ok(w)
    }

    /// The call's result is `R(w)`; the argument registers are free.
    fn call_result(&mut self, i: &Instr, w: u32, n: usize) {
        for k in 0..n {
            self.free(w + 1 + k as u32);
        }
        match i.result {
            Some(v) => {
                self.val_reg.insert(v, w);
            }
            None => self.free(w),
        }
    }
}

/// The operator of a compound assignment.
fn compound(op: AssignOp) -> Option<Op> {
    match op {
        AssignOp::Set => None,
        AssignOp::Add => Some(Op::ADD),
        AssignOp::Sub => Some(Op::SUB),
        AssignOp::Mul => Some(Op::MUL),
        AssignOp::Div => Some(Op::DIV),
    }
}

/// Does a block end in a terminator (nothing falls out of it)?
fn ends(b: &Block) -> bool {
    b.instrs.last().is_some_and(|i| i.kind.is_terminator())
}

/// Number the instructions in the order they are lowered: an
/// instruction's start before its nested blocks, its end after them.
fn number(
    b: &Block,
    start: &mut HashMap<ir::InstrId, usize>,
    end: &mut HashMap<ir::InstrId, usize>,
    counter: &mut usize,
) {
    for i in &b.instrs {
        *counter += 1;
        start.insert(i.id, *counter);
        for nested in i.kind.blocks() {
            number(nested, start, end, counter);
        }
        *counter += 1;
        end.insert(i.id, *counter);
    }
}

/// A loop's (start, end) positions.
type LoopRange = (usize, usize);

/// Every use of a value, with the loops (start, end) around it,
/// outermost first.
fn collect_uses(
    b: &Block,
    start: &HashMap<ir::InstrId, usize>,
    end: &HashMap<ir::InstrId, usize>,
    ranges: &HashMap<ValueId, (ValueId, ValueId, Span)>,
    loops: &mut Vec<LoopRange>,
    f: &mut dyn FnMut(ValueId, usize, &[LoopRange]),
) {
    for i in &b.instrs {
        let s = start.get(&i.id).copied().unwrap_or(0);
        for v in i.kind.uses() {
            f(v, s, loops);
            // A `for` over `a..b` reads the bounds itself.
            if let Some((a, b, _)) = ranges.get(&v) {
                f(*a, s, loops);
                f(*b, s, loops);
            }
        }
        let is_loop = matches!(i.kind, Kind::Loop { .. } | Kind::ForEach { .. });
        if is_loop {
            loops.push((s, end.get(&i.id).copied().unwrap_or(s)));
        }
        for nested in i.kind.blocks() {
            collect_uses(nested, start, end, ranges, loops, f);
        }
        if is_loop {
            loops.pop();
        }
    }
}

/// Every instruction, nested blocks included, in lowering order.
fn walk<'a>(b: &'a Block, f: &mut dyn FnMut(&'a Instr)) {
    for i in &b.instrs {
        f(i);
        for nested in i.kind.blocks() {
            walk(nested, f);
        }
    }
}

/// The slot each hoisted pull's closure is declared into.
fn pulls(b: &Block, closures: &mut HashMap<ValueId, FuncId>, out: &mut HashMap<FuncId, VarId>) {
    for i in &b.instrs {
        match (&i.kind, i.result) {
            (Kind::Closure(f), Some(v)) => {
                closures.insert(v, *f);
            }
            (Kind::Decl(var, v), _) => {
                if let Some(f) = closures.get(v) {
                    out.insert(*f, *var);
                }
            }
            _ => {}
        }
        for nested in i.kind.blocks() {
            pulls(nested, closures, out);
        }
    }
}
