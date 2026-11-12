// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! Stage B's lowering (#31): a verified CIR module to the register
//! bytecode the cig runtime's VM runs (`docs/BYTECODE.md` in cig-script is
//! the contract), for the part of the language the VM core runs today:
//! constants, slots and moves; arithmetic, comparison and logic;
//! `if`, `while`, `break`, `continue`; `pull` functions and lambdas that
//! capture nothing, calls to them and to builtins, `snuff`; step marks.
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
//! - every instruction gets the span of the CIR instruction it came from,
//!   and the line table keeps a row wherever the span changes from the
//!   instruction before it.

use crate::cir::ir::{self, Block, Const, Func, FuncId, Instr, Kind, ShortOp, ValueId, VarId};
use cigscript::cig_bytecode::{
    self as bc, verify_module, wide_prefix, Constant, Instr as Word, LineEntry, Op, SpanBase,
    MAX_FUNCTION_DEPTH, MAX_REG_COUNT,
};
use cigscript::syntax::ast::{BinOp, UnOp};
use cigscript::syntax::span::Span;
use std::collections::{BTreeSet, HashMap};
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
        | Kind::Closure(_)
        | Kind::CallBuiltin { .. }
        | Kind::CallUser { .. }
        | Kind::CallValue { .. }
        | Kind::If { .. }
        | Kind::Loop { .. }
        | Kind::Yield(_)
        | Kind::Ret(_)
        | Kind::Break
        | Kind::Continue => return None,
        Kind::Prelude(_) => "a builtin, module or `args` used as a value",
        Kind::MakeList(_) => "a list",
        Kind::MakeMap(_) => "a map",
        Kind::Interp(_) => "string interpolation",
        Kind::Member { .. } => "a member read",
        Kind::Index { .. } => "an index read",
        Kind::SetIndex { .. } => "an index assignment",
        Kind::SetMember { .. } => "a member assignment",
        Kind::BurnOp { .. } => "a call that changes the world (a burn op)",
        Kind::CallMethod { .. } => "a method call",
        Kind::MakeChain { .. } => "a chain",
        Kind::PackDecl(_) => "`pack`",
        Kind::Exhale(_) => "`exhale`",
        Kind::Burn(_) => "`burn`",
        Kind::Try { .. } => "`try`",
        Kind::ForEach { .. } => "`for`",
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
    let cx = Cx {
        m,
        catalogue: &catalogue,
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
        if let Some(v) = f.captures.first() {
            let name = self
                .m
                .vars
                .get(v.0 as usize)
                .map_or("?", |x| x.name.as_str());
            return Err(Error::Refused {
                construct: format!("a function that captures `{name}` (a closure)"),
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
    head: Label,
    exit: Label,
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
    /// The slot holding the closure made for each pull, in this function.
    pull_slot: HashMap<FuncId, VarId>,
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
            pull_slot: HashMap::new(),
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
        self.liveness();
        let mut closures = HashMap::new();
        pulls(&f.body, &mut closures, &mut self.pull_slot);
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

    fn free(&mut self, r: u32) {
        if r < self.frame {
            self.free_regs.insert(r);
        }
    }

    fn var(&mut self, v: VarId, span: Span) -> Result<u32, Error> {
        let owner = self.cx.m.vars.get(v.0 as usize).map(|x| x.owner);
        if owner != Some(self.f.id) {
            let name = self
                .cx
                .m
                .vars
                .get(v.0 as usize)
                .map_or("?", |x| x.name.as_str());
            return Err(Error::Refused {
                construct: format!("`{name}` of an enclosing function (a closure)"),
                span,
            });
        }
        if let Some(r) = self.var_reg.get(&v) {
            return Ok(*r);
        }
        let r = self.alloc();
        self.var_reg.insert(v, r);
        Ok(r)
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
            .min_by_key(|s| (s.start, s.line, s.col))
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
        for i in &b.instrs {
            self.instr(i)?;
            self.release(i);
        }
        Ok(())
    }

    fn refuse(&self, construct: impl Into<String>, span: Span) -> Error {
        Error::Refused {
            construct: construct.into(),
            span,
        }
    }

    fn instr(&mut self, i: &'m Instr) -> Result<(), Error> {
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
                    Const::Int(x) if i16::try_from(*x).is_ok() => {
                        self.row();
                        if r > 0xFF {
                            self.code
                                .push(wide_prefix((r >> 8) as u8, if *x < 0 { 0xFFFF } else { 0 }));
                        }
                        self.code.push(Word::asbx(Op::LOADINT, r as u8, *x as i16));
                    }
                    Const::Int(x) => {
                        let k = self.konst(ConstKey::Int(*x));
                        self.abx(Op::LOADK, r, k)?;
                    }
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
                let s = self.var(*v, i.span)?;
                let r = self.define(i)?;
                self.abc(Op::MOV, r, s, 0)?;
            }
            Kind::Decl(v, x) | Kind::Store(v, x) => {
                let s = self.var(*v, i.span)?;
                let x = self.val(*x)?;
                self.abc(Op::MOV, s, x, 0)?;
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
                    BinOp::Range => return Err(self.refuse("a `..` range", i.span)),
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
                let child = self.cx.function(target, self.depth + 1)?;
                let index = self.children.len() as u32;
                self.children.push(child);
                self.span = i.span;
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
                let slot = *self.pull_slot.get(func).ok_or_else(|| {
                    let name = self
                        .cx
                        .m
                        .funcs
                        .get(func.0 as usize)
                        .and_then(|f| f.name.clone())
                        .unwrap_or_default();
                    self.refuse(
                        format!("a call of `{name}` from outside the function that makes it (a closure)"),
                        i.span,
                    )
                })?;
                let callee = self.var(slot, i.span)?;
                let w = self.window(Some(callee), args)?;
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
                self.loops.push(LoopCx { head, exit });
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
                    _ => (Op::LOOP, l.head),
                };
                self.jump(op, 0, to)?;
            }
            _ => return Err(Error::Internal("a refused kind got past the table".into())),
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
    loops: &mut Vec<LoopRange>,
    f: &mut dyn FnMut(ValueId, usize, &[LoopRange]),
) {
    for i in &b.instrs {
        let s = start.get(&i.id).copied().unwrap_or(0);
        for v in i.kind.uses() {
            f(v, s, loops);
        }
        let is_loop = matches!(i.kind, Kind::Loop { .. } | Kind::ForEach { .. });
        if is_loop {
            loops.push((s, end.get(&i.id).copied().unwrap_or(s)));
        }
        for nested in i.kind.blocks() {
            collect_uses(nested, start, end, loops, f);
        }
        if is_loop {
            loops.pop();
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
