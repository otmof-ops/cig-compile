// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2

//! Where a function's machine code runs, and where the interpreter does
//! (decision 0008).
//!
//! Any instruction can be the interpreter's: a body is always right. So
//! what is machine code is a question of time alone, and this module
//! answers it before anything is emitted. Each instruction gets one of
//! three answers ([`How`]):
//!
//! - **native:** its inline form runs;
//! - **a stretch:** machine code calls the interpreter for a run of
//!   instructions and goes on after it;
//! - **a leave:** the body returns, and the VM's own loop runs the
//!   bytecode from there until it reaches a resume point.
//!
//! ## What is weighed
//!
//! Machine code is faster than the interpreter for one instruction by a
//! couple of nanoseconds. Going from one to the other costs several
//! times that: a stretch is a call into the runtime, and leaving a body
//! and entering it again is two trips through the VM's loop. So a loop
//! that reads a list, calls a function and adds two numbers is faster
//! left to the interpreter whole, and a loop of arithmetic is faster
//! native whole. [`COSTS`] has the figures; they are measurements of
//! one machine rounded to what matters, and only their proportions
//! decide anything.
//!
//! An inline form whose operands' types are proven ([`Class::Sure`])
//! is always taken. One that tests its operands when it runs
//! ([`Class::Guess`]) is taken as often as the estimate beside it says,
//! and each time it is not, the interpreter runs the instruction through
//! a call. A guess next to instructions the interpreter runs anyway
//! joins their stretch, where a failure costs nothing.
//!
//! The estimates lean towards the guess holding, because the runtime
//! bounds what it costs when it does not: a body whose guesses keep
//! failing is no longer entered there, and the bytecode has the loop
//! back within microseconds. So a loop is planned for the values its
//! instructions usually see, and a loop that sees others runs as it
//! would have with no body.
//!
//! ## How it is decided
//!
//! Loop by loop, innermost first ([`Rule::Loops`]). A loop's own
//! instructions, in the order they are written, are a ring: each takes
//! one of the three answers, and the answers that cost least a turn are
//! found exactly (three states an instruction, the loop closed on
//! itself). A loop inside it is one item of the ring: native when its
//! own answer starts native, otherwise the interpreter's. "Everything
//! is the interpreter's" is always one of the candidates, and it costs
//! what the bytecode costs: the plan is never worse than that by its
//! own figures.
//!
//! Then the function's own code, outside every loop, in the same way:
//! a line of items from the call to the return, with the loops as
//! items, entered from the interpreter and left to it. A function is
//! called any number of times, so its straight code is weighed as a
//! loop's is; the top level runs once, and only its loops are.
//!
//! ## Calls
//!
//! A `call` is the VM's instruction: it makes the frame. It is either
//! left to the VM's own loop (the body returns at it, and is entered
//! again after it), or **carried**: it is part of a stretch, and the
//! runtime's `run` makes the call and runs the callee to its return
//! under the body, entering the callee's own body where it has one.
//! Carrying costs the stretch and a little more; leaving costs the
//! body's way out and its way in again.
//!
//! With [`Rule::Every`] nothing is weighed: every instruction that has
//! an inline form is native, the rest are stretches, and every resume
//! point the bytecode has is one. It is the rule the tests build with,
//! so that every form and every way in runs.
//!
//! ## Resume points
//!
//! Where the VM enters the body: each native instruction the
//! interpreter can arrive at. That is one that follows an instruction
//! which is not native (the instruction after a call among them), a
//! handler's entry, and pc 0.

use std::collections::{BTreeMap, BTreeSet};

use cig_native::Rule;
use cigscript::cig_bytecode::instr::DecodedInstr;
use cigscript::cig_bytecode::module::RESERVED_U32;
use cigscript::cig_bytecode::opcode::Op;

use crate::flow::{self, Flow};

/// What machine code can do with one instruction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Class {
    /// An inline form the instruction's proven types cover.
    Sure,
    /// An inline form that tests its operands when it runs. The number
    /// is how often it is taken to cover them, in a hundred: an
    /// estimate.
    Guess(u8),
    /// An inline read or write of a list's element. It tests its
    /// operands as a guess does (the number is how often it is taken to
    /// cover them), and it needs the list's elements in view: the
    /// runtime says where they lie, and that holds until machine code
    /// next calls the runtime.
    Index(u8),
    /// `xs.push(v)`: machine code asks the runtime's own function for
    /// it, which costs less than the interpreter's method call. The
    /// list may move, so no list is in view after it.
    Push,
    /// An inline read or write of a map's field. It tests what it finds
    /// as a guess does (the number is how often it is taken to find a
    /// number in a map of the shape the site last saw), and it needs
    /// the map's values in view, as an index form needs a list's.
    Field(u8),
    /// No inline form, but one instruction the runtime runs where the
    /// frame stands, without its loop (`quick`): an upvalue read, a
    /// string loaded or built, a builtin called. `direct` when the
    /// runtime has a path of its own for the kind (an upvalue, a
    /// constant), which costs little more than the instruction;
    /// `moves` when it may add to a list or a map or take from one, so
    /// nothing is in view after it.
    Quick { moves: bool, direct: bool },
    /// No inline form: the interpreter's.
    Interp,
    /// `call`: the VM's own instruction makes the frame, in the VM's
    /// loop with the body left, or under the body in a stretch.
    Call,
    /// `ret` and `tailcall`: the frame ends.
    Exit,
}

/// Who runs an instruction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum How {
    /// Machine code.
    Native,
    /// The interpreter, called from machine code that goes on after it.
    Stretch,
    /// The interpreter, with the body left.
    Leave,
}

/// What the plan weighs, in tenths of a nanosecond.
pub(crate) struct Costs {
    /// One instruction as machine code.
    pub native: u32,
    /// One instruction in the interpreter.
    pub interp: u32,
    /// Machine code calls the interpreter and goes on after it.
    pub stretch: u32,
    /// Machine code leaves its body.
    pub out: u32,
    /// The VM enters a body.
    pub enter: u32,
    /// Machine code asks the runtime where a list's elements lie.
    pub view: u32,
    /// A list's element read or written as machine code, and by the
    /// interpreter: it borrows the list, checks the index and copies
    /// the value, which is more than its plain instructions do.
    pub index_native: u32,
    pub index_interp: u32,
    /// A `push` through the runtime's own function, and as the
    /// interpreter's method call.
    pub push_native: u32,
    pub push_interp: u32,
    /// A map's field read or written as machine code (the shape
    /// compared, the value copied), and by the interpreter.
    pub field_native: u32,
    pub field_interp: u32,
    /// One instruction run by the runtime where the frame stands,
    /// beyond what the interpreter takes to run it: what machine code
    /// holds is written to the frame, and the runtime is called. Less
    /// for a kind the runtime has a path of its own for than for one
    /// that goes through its fast path's function.
    pub quick: u32,
    pub quick_call: u32,
    /// The VM enters a body at its function's first instruction, on
    /// the frame a call has just made: nothing is held in a machine
    /// register there, so nothing is read back from the frame.
    pub begin: u32,
    /// A call carried through in a stretch, beyond the stretch itself:
    /// the runtime's loop is started once more under the body.
    pub carry: u32,
    /// How much of a failed guess's call is counted, in a hundred. A
    /// guess that fails at the rate beside it costs that share of a
    /// stretch each time; one that keeps failing is given up by the
    /// runtime and costs nothing more, so over a run a guess costs less
    /// than its rate says.
    pub guess: u32,
    /// How many turns a loop is taken to make each time it is reached:
    /// what entering and leaving it are spread over.
    pub turns: u32,
}

/// The figures of one x86-64 laptop, rounded.
pub(crate) const COSTS: Costs = Costs {
    native: 3,
    interp: 15,
    stretch: 120,
    out: 50,
    enter: 250,
    view: 100,
    index_native: 10,
    index_interp: 30,
    push_native: 40,
    push_interp: 80,
    field_native: 8,
    field_interp: 24,
    quick: 5,
    quick_call: 20,
    begin: 70,
    carry: 40,
    guess: 40,
    turns: 10,
};

/// What a plan is asked for beyond its rule.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Asked {
    /// The function's own code outside its loops is weighed too: for
    /// every function but the top level, which runs once.
    pub straight: bool,
    /// With [`Rule::Every`]: a call is carried in a stretch. Otherwise
    /// every call is left to the VM's own loop, so that both ways of
    /// making a call run in the tests' builds.
    pub carry: bool,
}

/// A function's plan.
#[derive(Debug, Clone)]
pub(crate) struct Plan {
    how: BTreeMap<u32, How>,
    /// Where the VM enters the body, ascending. Empty when nothing of
    /// the function is worth machine code.
    pub(crate) resumes: Vec<u32>,
    /// Where the body's blocks start, ascending: the bytecode's own
    /// block starts, every resume point, and every pc whose answer is
    /// not the one before it.
    pub(crate) leaders: Vec<u32>,
    end: u32,
}

impl Plan {
    /// Who runs the instruction at `pc`.
    pub(crate) fn how(&self, pc: u32) -> How {
        self.how.get(&pc).copied().unwrap_or(How::Leave)
    }

    /// The end of the stretch `pc` is in: the first pc after it whose
    /// answer is not a stretch, or the function's end.
    pub(crate) fn stretch_end(&self, pc: u32) -> u32 {
        self.how
            .range(pc..)
            .find(|(_, how)| **how != How::Stretch)
            .map_or(self.end, |(at, _)| *at)
    }
}

/// The plan of one function: `class(pc)` is what machine code can do
/// with the instruction at each first-word pc, `code` the decoded
/// instructions by pc and `end` the function's length in words.
pub(crate) fn plan(
    rule: Rule,
    flow: &Flow,
    code: &[Option<DecodedInstr>],
    class: &BTreeMap<u32, Class>,
    end: u32,
    asked: Asked,
) -> Plan {
    let how = match rule {
        Rule::None => flow.pcs.iter().map(|pc| (*pc, How::Leave)).collect(),
        Rule::Every => every(flow, class, asked.carry),
        Rule::Loops => loops(flow, code, class, asked.straight),
    };
    let mut how = how;
    let mut leaders: BTreeSet<u32> = flow.leaders.iter().copied().collect();
    // The interpreter goes on past the `nop`s that follow an index
    // instruction (they carry its parts' positions), so they are run by
    // whoever runs it, and the pc after them starts a block: it is
    // where the bytecode stands when the interpreter ran the
    // instruction for an inline form that did not cover its operands.
    for d in code.iter().flatten() {
        let Some(past) = past_nops(code, d) else {
            continue;
        };
        let answer = how.get(&d.pc).copied().unwrap_or(How::Leave);
        for (_, h) in how.range_mut(d.pc + d.width..past) {
            *h = answer;
        }
        if past < end {
            leaders.insert(past);
        }
    }
    let resumes = match rule {
        Rule::None => Vec::new(),
        // Every way in the bytecode has.
        Rule::Every => flow.resumes.clone(),
        Rule::Loops => resumes(flow, &how),
    };
    leaders.extend(resumes.iter().copied());
    let mut last = None;
    for (pc, h) in &how {
        if last.is_some_and(|l| l != *h) {
            leaders.insert(*pc);
        }
        last = Some(*h);
    }
    Plan {
        how,
        resumes,
        leaders: leaders.into_iter().collect(),
        end,
    }
}

/// For an index instruction (`getindex`, `setindex`): the pc the
/// interpreter goes on at when it has run it, past the `nop`s after it
/// that carry its parts' positions (at most one for a read, two for a
/// write). `None` for any other instruction.
pub(crate) fn past_nops(code: &[Option<DecodedInstr>], d: &DecodedInstr) -> Option<u32> {
    let most = match d.op {
        Op::GETINDEX => 1,
        Op::SETINDEX => 2,
        _ => return None,
    };
    let next = d.pc + d.width;
    let mut pc = next;
    while pc < next + most && matches!(code.get(pc as usize), Some(Some(n)) if n.op == Op::NOP) {
        pc += 1;
    }
    Some(pc)
}

/// [`Rule::Every`]: whatever has an inline form is native, and a call
/// is carried in a stretch when `carry` says so.
fn every(flow: &Flow, class: &BTreeMap<u32, Class>, carry: bool) -> BTreeMap<u32, How> {
    flow.pcs
        .iter()
        .map(|pc| {
            let how = match class.get(pc).copied().unwrap_or(Class::Interp) {
                Class::Sure
                | Class::Guess(_)
                | Class::Index(_)
                | Class::Push
                | Class::Field(_)
                | Class::Quick { .. } => How::Native,
                Class::Interp => How::Stretch,
                Class::Call if carry => How::Stretch,
                Class::Call | Class::Exit => How::Leave,
            };
            (*pc, how)
        })
        .collect()
}

/// The native instructions the interpreter can arrive at.
fn resumes(flow: &Flow, how: &BTreeMap<u32, How>) -> Vec<u32> {
    let native = |pc: u32| how.get(&pc) == Some(&How::Native);
    let mut out = BTreeSet::new();
    if native(0) {
        out.insert(0);
    }
    for (from, to) in &flow.succs {
        if native(*from) {
            continue;
        }
        out.extend(to.iter().copied().filter(|pc| native(*pc)));
    }
    for h in &flow.handler_entries {
        for pc in [h.handler_pc, h.finally_pc] {
            if pc != RESERVED_U32 && native(pc) {
                out.insert(pc);
            }
        }
    }
    out.into_iter().collect()
}

/// One loop: the instructions from the target of a backward jump to the
/// jump, and the loops inside it.
#[derive(Debug)]
struct Loop {
    start: u32,
    end: u32,
    inside: Vec<Loop>,
}

/// The function's loops, outermost first, each with the loops inside
/// it. Two loops that overlap without one holding the other (no
/// structured program has them) are taken as one.
fn loops_of(code: &[Option<DecodedInstr>]) -> Vec<Loop> {
    let mut ranges: BTreeMap<u32, u32> = BTreeMap::new();
    for d in code.iter().flatten() {
        let Some(t) = flow::target_of(d) else {
            continue;
        };
        if t <= d.pc {
            let end = d.pc + d.width;
            let e = ranges.entry(t).or_insert(end);
            *e = (*e).max(end);
        }
    }
    let mut flat: Vec<(u32, u32)> = ranges.into_iter().collect();
    // Widen until no two cross.
    loop {
        let mut changed = false;
        for i in 0..flat.len() {
            for j in i + 1..flat.len() {
                let (a, b) = (flat[i], flat[j]);
                if b.0 < a.1 && b.1 > a.1 {
                    flat[i].1 = b.1;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    // Outer loops first: by start, then the wider first.
    flat.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    flat.dedup();
    fn nest(flat: &[(u32, u32)], at: &mut usize, within: u32) -> Vec<Loop> {
        let mut out = Vec::new();
        while let Some((start, end)) = flat.get(*at).copied() {
            if start >= within {
                break;
            }
            *at += 1;
            let inside = nest(flat, at, end);
            out.push(Loop { start, end, inside });
        }
        out
    }
    let mut at = 0;
    nest(&flat, &mut at, u32::MAX)
}

/// [`Rule::Loops`]: each loop's cheapest answers, innermost first; then,
/// with `straight`, the function's own code around its loops. The rest
/// is the interpreter's.
fn loops(
    flow: &Flow,
    code: &[Option<DecodedInstr>],
    class: &BTreeMap<u32, Class>,
    straight: bool,
) -> BTreeMap<u32, How> {
    let mut how: BTreeMap<u32, How> = flow.pcs.iter().map(|pc| (*pc, How::Leave)).collect();
    let outer = loops_of(code);
    let kinds: Vec<Inner> = outer
        .iter()
        .map(|l| settle(l, flow, class, &mut how))
        .collect();
    if straight {
        settle_line(&outer, &kinds, flow, class, &mut how);
    }
    how
}

/// What a loop inside another is to the outer loop's ring.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Inner {
    /// Its answer starts native: the outer loop's machine code runs
    /// into it. `whole` when all of it is native, so it calls the
    /// runtime for nothing and a list in view before it is in view
    /// after it.
    Native { whole: bool },
    /// The interpreter's whole: a stretch of the outer loop's machine
    /// code can hold it, its calls carried.
    Quiet,
    /// Native in part, and not from its start: its body is entered
    /// inside it, so the outer loop is out of its body around it.
    Out,
}

/// One item of a loop's ring.
#[derive(Clone, Copy, Debug)]
enum Item {
    /// An instruction of the loop's own.
    Own(u32, Class),
    /// A loop inside it, from this pc to that.
    Loop(u32, u32, Inner),
}

/// Native, with no list known to be in view.
const NATIVE: usize = 0;
/// Native, with the lists the loop indexes in view: nothing has called
/// the runtime since each was asked for.
const VIEWED: usize = 1;
const STRETCH: usize = 2;
const LEAVE: usize = 3;
const STATES: [usize; 4] = [NATIVE, VIEWED, STRETCH, LEAVE];
/// More than any plan costs, and far from overflowing when added.
const NEVER: u32 = u32::MAX / 8;

fn is_native(state: usize) -> bool {
    state == NATIVE || state == VIEWED
}

/// The item is an inline index or field form: what brings a list's
/// elements or a map's values into view.
fn takes_view(item: &Item) -> bool {
    matches!(item, Item::Own(_, Class::Index(_) | Class::Field(_)))
}

/// What an item costs in a state: [`NEVER`] when it cannot be in it.
fn item_cost(item: &Item, state: usize, c: &Costs) -> u32 {
    let miss = |p: u8| c.stretch * (100 - u32::from(p.min(100))) * c.guess / 10_000;
    match (item, state) {
        (Item::Own(_, Class::Sure), NATIVE | VIEWED) => c.native,
        (Item::Own(_, Class::Guess(p)), NATIVE | VIEWED) => c.native + miss(*p),
        (Item::Own(_, Class::Index(p)), VIEWED) => c.index_native + miss(*p),
        (Item::Own(_, Class::Index(_)), NATIVE) => NEVER,
        (Item::Own(_, Class::Index(_)), _) => c.index_interp,
        (Item::Own(_, Class::Sure | Class::Guess(_)), _) => c.interp,
        (Item::Own(_, Class::Push), NATIVE) => c.push_native,
        (Item::Own(_, Class::Push), VIEWED) => NEVER,
        (Item::Own(_, Class::Push), _) => c.push_interp,
        (Item::Own(_, Class::Field(p)), VIEWED) => c.field_native + miss(*p),
        (Item::Own(_, Class::Field(_)), NATIVE) => NEVER,
        (Item::Own(_, Class::Field(_)), _) => c.field_interp,
        // The interpreter's work is the instruction's own either way:
        // what machine code pays is the call.
        (Item::Own(_, Class::Quick { moves: true, .. }), VIEWED) => NEVER,
        (Item::Own(_, Class::Quick { direct: true, .. }), NATIVE | VIEWED) => c.quick,
        (Item::Own(_, Class::Quick { direct: false, .. }), NATIVE | VIEWED) => c.quick_call,
        (Item::Own(_, Class::Quick { .. }), _) => 0,
        (Item::Own(_, Class::Interp), NATIVE | VIEWED) => NEVER,
        (Item::Own(_, Class::Interp), _) => 0,
        // The VM's own loop makes it for nothing more; under the body
        // the runtime's loop is started once more.
        (Item::Own(_, Class::Call), LEAVE) => 0,
        (Item::Own(_, Class::Call), STRETCH) => c.carry,
        (Item::Own(_, Class::Call), _) => NEVER,
        (Item::Own(_, Class::Exit), LEAVE) => 0,
        (Item::Own(_, Class::Exit), _) => NEVER,
        (Item::Loop(_, _, Inner::Native { .. }), NATIVE) => 0,
        (Item::Loop(_, _, Inner::Native { whole: true }), VIEWED) => 0,
        (Item::Loop(_, _, Inner::Native { .. }), _) => NEVER,
        (Item::Loop(_, _, Inner::Quiet), NATIVE | VIEWED) => NEVER,
        (Item::Loop(_, _, Inner::Quiet), _) => 0,
        (Item::Loop(_, _, Inner::Out), LEAVE) => 0,
        (Item::Loop(_, _, Inner::Out), _) => NEVER,
    }
}

/// What going from an item in state `from` to the item `next` in state
/// `to` costs. A list comes into view only at an inline index form,
/// for one call of the runtime.
fn step_cost(from: usize, to: usize, next: &Item, c: &Costs, begins: bool) -> u32 {
    let view = match takes_view(next) {
        true => c.view,
        false => NEVER,
    };
    // Into the body: at a function's first instruction, or anywhere
    // else.
    let enter = match begins {
        true => c.begin,
        false => c.enter,
    };
    match (from, to) {
        (NATIVE | VIEWED, STRETCH) => c.stretch,
        (NATIVE | VIEWED, LEAVE) => c.out,
        (LEAVE, NATIVE) => enter,
        (LEAVE, VIEWED) => enter.saturating_add(view),
        // Out of the body, a stretch is no different from the rest.
        (LEAVE, STRETCH) => NEVER,
        (NATIVE | STRETCH, VIEWED) => view,
        _ => 0,
    }
}

/// The cheapest states of a ring of items, and what they cost a turn.
///
/// Exact: the cheapest way to have each item in each state is kept as
/// the ring is walked, once for each state the first item can start
/// in, and the ring is closed on that state. A plan with an
/// instruction of the loop's own native also pays for entering and
/// leaving the loop, spread over the turns it is taken to make. Of two
/// plans that cost the same, the one found first stays: nothing native
/// before something native.
fn cheapest(items: &[Item], c: &Costs) -> (Vec<usize>, u32) {
    let mut best: Option<(Vec<usize>, u32)> = None;
    let Some(first) = items.first() else {
        return (Vec::new(), 0);
    };
    let tax = (c.enter + c.out) / c.turns.max(1);
    // Whether an instruction of the loop's own is native in the plan
    // so far is part of the state: `cost[state][native]`.
    let own_native = |item: &Item, state: usize| matches!(item, Item::Own(..)) && is_native(state);
    for start in [LEAVE, NATIVE, VIEWED, STRETCH] {
        let here = item_cost(first, start, c);
        if here >= NEVER {
            continue;
        }
        let mut cost = [[NEVER; 2]; 4];
        cost[start][usize::from(own_native(first, start))] = here;
        // For each item after the first: where its `[state][native]`
        // came from.
        let mut came: Vec<[[(usize, usize); 2]; 4]> = Vec::with_capacity(items.len());
        // A line starts with its end item: the step out of it is the
        // step into the function.
        let line = matches!(first, Item::Own(ENDS, _));
        for (at, item) in items[1..].iter().enumerate() {
            let mut next = [[NEVER; 2]; 4];
            let mut from = [[(LEAVE, 0); 2]; 4];
            for to in STATES {
                let here = item_cost(item, to, c);
                if here >= NEVER {
                    continue;
                }
                for prev in STATES {
                    let step = step_cost(prev, to, item, c, line && at == 0);
                    if step >= NEVER {
                        continue;
                    }
                    for flag in [0, 1] {
                        if cost[prev][flag] >= NEVER {
                            continue;
                        }
                        let total = cost[prev][flag].saturating_add(step).saturating_add(here);
                        let now = flag | usize::from(own_native(item, to));
                        if total < next[to][now] {
                            next[to][now] = total;
                            from[to][now] = (prev, flag);
                        }
                    }
                }
            }
            cost = next;
            came.push(from);
        }
        // Close the ring.
        for flag in [0, 1] {
            for last in STATES {
                if cost[last][flag] >= NEVER {
                    continue;
                }
                let back = step_cost(last, start, first, c, false);
                if back >= NEVER {
                    continue;
                }
                let total = cost[last][flag]
                    .saturating_add(back)
                    .saturating_add(if flag == 1 { tax } else { 0 });
                if best.as_ref().is_some_and(|(_, least)| total >= *least) {
                    continue;
                }
                let mut states = vec![start; items.len()];
                let (mut state, mut f) = (last, flag);
                for i in (1..items.len()).rev() {
                    states[i] = state;
                    (state, f) = came[i - 1][state][f];
                }
                best = Some((states, total));
            }
        }
    }
    // A ring with nothing it can be (no structured loop makes one): the
    // interpreter's.
    best.unwrap_or_else(|| (vec![LEAVE; items.len()], 0))
}

/// Settle one loop: the loops inside it first, then its own ring. The
/// loop's answers go to `how`. Returns what it is to a loop around it.
fn settle(
    l: &Loop,
    flow: &Flow,
    class: &BTreeMap<u32, Class>,
    how: &mut BTreeMap<u32, How>,
) -> Inner {
    let mut items = Vec::new();
    let mut inner = l.inside.iter().peekable();
    let mut skip_to = l.start;
    for pc in flow
        .pcs
        .iter()
        .copied()
        .filter(|pc| (l.start..l.end).contains(pc))
    {
        if pc < skip_to {
            continue;
        }
        if let Some(i) = inner.peek().filter(|i| i.start == pc) {
            let kind = settle(i, flow, class, how);
            items.push(Item::Loop(i.start, i.end, kind));
            skip_to = i.end;
            inner.next();
            continue;
        }
        match class.get(&pc).copied().unwrap_or(Class::Interp) {
            // A return leaves the loop for good: it weighs nothing a
            // turn and holds its neighbors to nothing.
            Class::Exit => {}
            c => items.push(Item::Own(pc, c)),
        }
    }
    let (states, _) = cheapest(&items, &COSTS);
    let (any_native, whole) = answer(&items, &states, how);
    if how.get(&l.start) == Some(&How::Native) {
        Inner::Native { whole }
    } else if any_native {
        Inner::Out
    } else {
        Inner::Quiet
    }
}

/// Write a ring's or a line's answers to `how`. Returns whether
/// anything of it is native (an instruction of its own, or a loop
/// inside it that is native at all), and whether all of it is.
fn answer(items: &[Item], states: &[usize], how: &mut BTreeMap<u32, How>) -> (bool, bool) {
    let mut any_native = false;
    // Native whole: every instruction of its own, and every loop in it.
    let mut whole = !items.is_empty();
    for (item, state) in items.iter().zip(states) {
        match item {
            Item::Own(pc, _) => {
                let answer = match *state {
                    NATIVE | VIEWED => How::Native,
                    STRETCH => How::Stretch,
                    _ => How::Leave,
                };
                any_native |= answer == How::Native;
                whole &= answer == How::Native;
                if let Some(h) = how.get_mut(pc) {
                    *h = answer;
                }
            }
            Item::Loop(start, end, kind) => {
                match kind {
                    Inner::Native { whole: all } => {
                        any_native = true;
                        whole &= *all;
                    }
                    Inner::Out => {
                        any_native = true;
                        whole = false;
                    }
                    Inner::Quiet => whole = false,
                }
                // A quiet loop held by a stretch is part of that
                // stretch, whole.
                if *kind == Inner::Quiet && *state == STRETCH {
                    for (_, h) in how.range_mut(*start..*end) {
                        *h = How::Stretch;
                    }
                }
            }
        }
    }
    (any_native, whole)
}

/// A pc no instruction has: the line's two ends, where the interpreter
/// has the frame.
const ENDS: u32 = u32::MAX;

/// Settle a function's own code outside its loops (`outer`, settled
/// already, each with what it is to the code around it): a line from
/// the call to the return. It is the search a loop's ring gets, with
/// one item more that only the interpreter can have: so the line
/// starts and ends out of the body, entering costs once where the
/// first native item is and leaving once after the last, and nothing
/// is spread over turns.
fn settle_line(
    outer: &[Loop],
    kinds: &[Inner],
    flow: &Flow,
    class: &BTreeMap<u32, Class>,
    how: &mut BTreeMap<u32, How>,
) {
    let mut items = vec![Item::Own(ENDS, Class::Exit)];
    let mut loops = outer.iter().zip(kinds).peekable();
    let mut skip_to = 0;
    for pc in flow.pcs.iter().copied() {
        if pc < skip_to {
            continue;
        }
        if let Some((l, kind)) = loops.peek().filter(|(l, _)| l.start == pc) {
            items.push(Item::Loop(l.start, l.end, **kind));
            skip_to = l.end;
            loops.next();
            continue;
        }
        match class.get(&pc).copied().unwrap_or(Class::Interp) {
            // A return ends the line wherever it stands: it weighs
            // nothing and holds its neighbors to nothing.
            Class::Exit => {}
            c => items.push(Item::Own(pc, c)),
        }
    }
    let (states, _) = cheapest(&items, &LINE);
    answer(&items, &states, how);
}

/// What a line weighs: a ring's figures, with nothing spread over
/// turns (entering and leaving are in the line itself).
const LINE: Costs = Costs {
    turns: u32::MAX,
    ..COSTS
};

#[cfg(test)]
mod tests {
    use super::*;

    fn own(classes: &[Class]) -> Vec<Item> {
        classes
            .iter()
            .zip(0u32..)
            .map(|(c, pc)| Item::Own(pc, *c))
            .collect()
    }

    #[test]
    fn a_loop_of_proven_arithmetic_is_native_whole() {
        let (states, cost) = cheapest(&own(&[Class::Sure; 6]), &COSTS);
        assert_eq!(states, vec![NATIVE; 6]);
        // Six native instructions, and the loop's entering and leaving
        // spread over its turns.
        assert_eq!(
            cost,
            6 * COSTS.native + (COSTS.enter + COSTS.out) / COSTS.turns
        );
    }

    #[test]
    fn a_loop_the_interpreter_runs_anyway_is_left_to_it_whole() {
        // A list read, an add that guesses, a list write, a call.
        let items = own(&[
            Class::Interp,
            Class::Guess(75),
            Class::Interp,
            Class::Call,
            Class::Sure,
        ]);
        let (states, cost) = cheapest(&items, &COSTS);
        assert_eq!(states, vec![LEAVE; 5]);
        assert_eq!(cost, 2 * COSTS.interp);
    }

    #[test]
    fn a_guess_between_two_interpreter_instructions_joins_their_stretch() {
        // Enough proven work to be native, then: read, guess, write.
        let mut classes = vec![Class::Sure; 20];
        classes.extend([Class::Interp, Class::Guess(75), Class::Interp]);
        let (states, _) = cheapest(&own(&classes), &COSTS);
        assert_eq!(&states[..20], &[NATIVE; 20]);
        assert_eq!(&states[20..], &[STRETCH; 3], "one stretch, not two");
    }

    #[test]
    fn a_guess_among_proven_instructions_is_taken() {
        let mut classes = vec![Class::Sure; 12];
        classes.insert(6, Class::Guess(75));
        let (states, _) = cheapest(&own(&classes), &COSTS);
        assert_eq!(states, vec![NATIVE; 13]);
    }

    #[test]
    fn a_call_among_little_work_is_left_to_the_interpreter_with_its_loop() {
        let mut classes = vec![Class::Sure; 6];
        classes.push(Class::Call);
        let (states, _) = cheapest(&own(&classes), &COSTS);
        assert_eq!(states, vec![LEAVE; 7]);
    }

    #[test]
    fn a_call_among_much_work_is_carried_under_the_body() {
        let mut classes = vec![Class::Sure; 30];
        classes.push(Class::Call);
        let (states, cost) = cheapest(&own(&classes), &COSTS);
        assert_eq!(&states[..30], &[NATIVE; 30]);
        assert_eq!(states[30], STRETCH, "carried, not left");
        assert_eq!(
            cost,
            30 * COSTS.native
                + COSTS.stretch
                + COSTS.carry
                + (COSTS.enter + COSTS.out) / COSTS.turns
        );
        // What loads the callee is the interpreter's too (a variable
        // of another function): the two are one stretch.
        let mut classes = vec![Class::Sure; 30];
        classes.extend([Class::Interp, Class::Call]);
        let (states, with_load) = cheapest(&own(&classes), &COSTS);
        assert_eq!(&states[30..], &[STRETCH; 2]);
        assert_eq!(with_load, cost);
    }

    /// The states of a function's own line of `classes`, call to
    /// return.
    fn line(classes: &[Class]) -> Vec<usize> {
        let mut items = vec![Item::Own(ENDS, Class::Exit)];
        items.extend(own(classes));
        let (states, _) = cheapest(&items, &LINE);
        assert_eq!(
            states[0], LEAVE,
            "the interpreter has the frame at the call"
        );
        states[1..].to_vec()
    }

    #[test]
    fn a_functions_straight_code_is_native_when_there_is_enough_of_it() {
        // Entering and leaving are paid once a call: the least number
        // of proven instructions that pays for them.
        let pays = (1..)
            .find(|n| n * COSTS.native + COSTS.begin + COSTS.out < n * COSTS.interp)
            .unwrap();
        assert_eq!(
            line(&vec![Class::Sure; pays as usize]),
            vec![NATIVE; pays as usize]
        );
        let short = (pays - 1) as usize;
        assert_eq!(line(&vec![Class::Sure; short]), vec![LEAVE; short]);
    }

    #[test]
    fn a_function_is_entered_at_its_first_native_instruction() {
        // It starts with what the interpreter runs anyway (it loads a
        // variable of another function): the VM's loop runs that, and
        // the body is entered after it.
        let mut classes = vec![Class::Interp];
        classes.extend([Class::Sure; 40]);
        let states = line(&classes);
        assert_eq!(states[0], LEAVE);
        assert_eq!(&states[1..], &[NATIVE; 40]);
    }

    #[test]
    fn a_call_in_a_functions_straight_code_is_carried() {
        let mut classes = vec![Class::Sure; 40];
        classes.insert(20, Class::Call);
        let states = line(&classes);
        assert_eq!(states[20], STRETCH);
        assert!(states
            .iter()
            .enumerate()
            .all(|(i, s)| i == 20 || *s == NATIVE));
    }

    #[test]
    fn a_loop_of_calls_alone_is_quiet_and_an_outer_stretch_holds_it() {
        // Nothing of it is native, so nothing is entered inside it.
        let items = own(&[Class::Interp, Class::Call, Class::Sure]);
        let (states, _) = cheapest(&items, &COSTS);
        assert_eq!(states, vec![LEAVE; 3]);
        // Around it, a loop with enough work of its own stays in its
        // body and runs the inner loop as one stretch.
        let mut outer = own(&[Class::Sure; 30]);
        outer.push(Item::Loop(30, 40, Inner::Quiet));
        let (states, _) = cheapest(&outer, &COSTS);
        assert_eq!(&states[..30], &[NATIVE; 30]);
        assert_eq!(states[30], STRETCH);
    }

    #[test]
    fn a_native_loop_inside_draws_its_outer_loop_in() {
        // The outer loop's own work is three proven instructions around
        // a native loop: out of the body it would be entered every turn.
        let items = vec![
            Item::Own(0, Class::Sure),
            Item::Loop(1, 9, Inner::Native { whole: true }),
            Item::Own(9, Class::Sure),
            Item::Own(10, Class::Sure),
        ];
        let (states, _) = cheapest(&items, &COSTS);
        assert_eq!(states, vec![NATIVE; 4]);
    }

    #[test]
    fn a_quiet_loop_inside_a_native_one_is_one_stretch() {
        let mut items = own(&[Class::Sure; 20]);
        items.push(Item::Loop(20, 30, Inner::Quiet));
        let (states, _) = cheapest(&items, &COSTS);
        assert_eq!(states[20], STRETCH);
        assert_eq!(&states[..20], &[NATIVE; 20]);
    }

    #[test]
    fn a_loop_that_writes_a_list_and_counts_is_native_whole_with_the_list_in_view() {
        // `while m <= n { is[m] = false; m += p }`: a compare, a write
        // of an element, an add, a jump. Nothing in it calls the
        // runtime, so the list is asked for once and stays in view.
        let items = own(&[Class::Sure, Class::Index(90), Class::Sure, Class::Sure]);
        let (states, cost) = cheapest(&items, &COSTS);
        assert_eq!(states, vec![VIEWED; 4]);
        let miss = COSTS.stretch * 10 * COSTS.guess / 10_000;
        assert_eq!(
            cost,
            3 * COSTS.native + COSTS.index_native + miss + (COSTS.enter + COSTS.out) / COSTS.turns
        );
    }

    #[test]
    fn an_index_after_a_stretch_pays_for_its_view_each_turn() {
        // One element read among instructions the interpreter runs
        // anyway: asking where the list lies every turn costs more
        // than the read saves, so it joins the stretch.
        let mut classes = vec![Class::Sure; 20];
        classes.extend([Class::Interp, Class::Index(90), Class::Interp]);
        let (states, _) = cheapest(&own(&classes), &COSTS);
        assert_eq!(&states[20..], &[STRETCH; 3]);
        // Several reads after one stretch share the one view.
        let mut classes = vec![Class::Sure; 20];
        classes.push(Class::Interp);
        classes.extend([Class::Index(90); 8]);
        let (states, _) = cheapest(&own(&classes), &COSTS);
        assert_eq!(states[20], STRETCH);
        assert_eq!(&states[21..], &[VIEWED; 8]);
    }

    #[test]
    fn a_loop_native_whole_keeps_its_outer_loops_lists_in_view() {
        let around = |inner: Inner| {
            vec![
                Item::Own(0, Class::Index(90)),
                Item::Own(1, Class::Sure),
                Item::Loop(2, 9, inner),
                Item::Own(9, Class::Sure),
            ]
        };
        // Whole: the outer loop's read is in view every turn.
        let (states, whole) = cheapest(&around(Inner::Native { whole: true }), &COSTS);
        assert_eq!(states, vec![VIEWED; 4]);
        // With a stretch inside, the view is asked for again each turn.
        let (states, part) = cheapest(&around(Inner::Native { whole: false }), &COSTS);
        assert!(is_native(states[2]) && states[0] == VIEWED, "{states:?}");
        assert_eq!(part, whole + COSTS.view);
    }

    #[test]
    fn nothing_native_is_a_candidate_whatever_the_ring_holds() {
        for classes in [
            vec![Class::Interp; 4],
            vec![Class::Call; 2],
            vec![Class::Guess(0); 9],
            Vec::new(),
        ] {
            let (states, _) = cheapest(&own(&classes), &COSTS);
            assert!(states.iter().all(|s| !is_native(*s)), "{classes:?}");
        }
    }
}
