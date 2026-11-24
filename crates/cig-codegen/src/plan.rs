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
//! Code outside every loop runs once a call, and is left to the
//! interpreter.
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
    /// No inline form: the interpreter's.
    Interp,
    /// `call`: the VM's own instruction makes the frame.
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
    /// How many turns a loop is taken to make each time it is reached:
    /// what entering and leaving it are spread over.
    pub turns: u32,
}

/// The figures of one x86-64 laptop, rounded.
pub(crate) const COSTS: Costs = Costs {
    native: 3,
    interp: 21,
    stretch: 120,
    out: 50,
    enter: 250,
    turns: 8,
};

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
) -> Plan {
    let how = match rule {
        Rule::None => flow.pcs.iter().map(|pc| (*pc, How::Leave)).collect(),
        Rule::Every => every(flow, class),
        Rule::Loops => loops(flow, code, class),
    };
    let resumes = match rule {
        Rule::None => Vec::new(),
        // Every way in the bytecode has.
        Rule::Every => flow.resumes.clone(),
        Rule::Loops => resumes(flow, &how),
    };
    let mut leaders: BTreeSet<u32> = flow.leaders.iter().copied().collect();
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

/// [`Rule::Every`]: whatever has an inline form is native.
fn every(flow: &Flow, class: &BTreeMap<u32, Class>) -> BTreeMap<u32, How> {
    flow.pcs
        .iter()
        .map(|pc| {
            let how = match class.get(pc).copied().unwrap_or(Class::Interp) {
                Class::Sure | Class::Guess(_) => How::Native,
                Class::Interp => How::Stretch,
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

/// [`Rule::Loops`]: each loop's cheapest answers, innermost first; the
/// rest is the interpreter's.
fn loops(
    flow: &Flow,
    code: &[Option<DecodedInstr>],
    class: &BTreeMap<u32, Class>,
) -> BTreeMap<u32, How> {
    let mut how: BTreeMap<u32, How> = flow.pcs.iter().map(|pc| (*pc, How::Leave)).collect();
    for l in loops_of(code) {
        settle(&l, flow, class, &mut how);
    }
    how
}

/// What a loop inside another is to the outer loop's ring.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Inner {
    /// Its answer starts native: the outer loop's machine code runs
    /// into it.
    Native,
    /// The interpreter's whole, with no call in it: a stretch of the
    /// outer loop's machine code can hold it.
    Quiet,
    /// The interpreter's, and it leaves the body (a call), or it is
    /// native in part: the outer loop is out of its body around it.
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

const NATIVE: usize = 0;
const STRETCH: usize = 1;
const LEAVE: usize = 2;
const STATES: [usize; 3] = [NATIVE, STRETCH, LEAVE];
/// More than any plan costs, and far from overflowing when added.
const NEVER: u32 = u32::MAX / 8;

/// What an item costs in a state: [`NEVER`] when it cannot be in it.
fn item_cost(item: &Item, state: usize, c: &Costs) -> u32 {
    match (item, state) {
        (Item::Own(_, Class::Sure), NATIVE) => c.native,
        (Item::Own(_, Class::Guess(p)), NATIVE) => {
            c.native + c.stretch * (100 - u32::from((*p).min(100))) / 100
        }
        (Item::Own(_, Class::Sure | Class::Guess(_)), _) => c.interp,
        (Item::Own(_, Class::Interp), NATIVE) => NEVER,
        (Item::Own(_, Class::Interp), _) => 0,
        (Item::Own(_, Class::Call | Class::Exit), LEAVE) => 0,
        (Item::Own(_, Class::Call | Class::Exit), _) => NEVER,
        (Item::Loop(_, _, Inner::Native), NATIVE) => 0,
        (Item::Loop(_, _, Inner::Native), _) => NEVER,
        (Item::Loop(_, _, Inner::Quiet), NATIVE) => NEVER,
        (Item::Loop(_, _, Inner::Quiet), _) => 0,
        (Item::Loop(_, _, Inner::Out), LEAVE) => 0,
        (Item::Loop(_, _, Inner::Out), _) => NEVER,
    }
}

/// What going from an item in state `from` to the next in state `to`
/// costs.
fn step_cost(from: usize, to: usize, c: &Costs) -> u32 {
    match (from, to) {
        (NATIVE, STRETCH) => c.stretch,
        (NATIVE, LEAVE) => c.out,
        (LEAVE, NATIVE) => c.enter,
        // Out of the body, a stretch is no different from the rest.
        (LEAVE, STRETCH) => NEVER,
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
    let own_native = |item: &Item, state: usize| matches!(item, Item::Own(..)) && state == NATIVE;
    for start in [LEAVE, NATIVE, STRETCH] {
        let here = item_cost(first, start, c);
        if here >= NEVER {
            continue;
        }
        let mut cost = [[NEVER; 2]; 3];
        cost[start][usize::from(own_native(first, start))] = here;
        // For each item after the first: where its `[state][native]`
        // came from.
        let mut came: Vec<[[(usize, usize); 2]; 3]> = Vec::with_capacity(items.len());
        for item in &items[1..] {
            let mut next = [[NEVER; 2]; 3];
            let mut from = [[(LEAVE, 0); 2]; 3];
            for to in STATES {
                let here = item_cost(item, to, c);
                if here >= NEVER {
                    continue;
                }
                for prev in STATES {
                    for flag in [0, 1] {
                        if cost[prev][flag] >= NEVER {
                            continue;
                        }
                        let total = cost[prev][flag]
                            .saturating_add(step_cost(prev, to, c))
                            .saturating_add(here);
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
                let back = step_cost(last, start, c);
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
    let mut any_native = false;
    let mut leaves = false;
    for (item, state) in items.iter().zip(&states) {
        match item {
            Item::Own(pc, c) => {
                let answer = match *state {
                    NATIVE => How::Native,
                    STRETCH => How::Stretch,
                    _ => How::Leave,
                };
                any_native |= answer == How::Native;
                leaves |= *c == Class::Call;
                how.insert(*pc, answer);
            }
            Item::Loop(start, end, kind) => {
                match kind {
                    Inner::Native => any_native = true,
                    Inner::Out => leaves = true,
                    Inner::Quiet => {}
                }
                // A quiet loop held by a stretch of this one is part of
                // that stretch, whole.
                if *kind == Inner::Quiet && *state == STRETCH {
                    for (_, h) in how.range_mut(*start..*end) {
                        *h = How::Stretch;
                    }
                }
            }
        }
    }
    if how.get(&l.start) == Some(&How::Native) {
        Inner::Native
    } else if any_native || leaves {
        Inner::Out
    } else {
        Inner::Quiet
    }
}

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
    fn a_call_among_little_work_is_not_worth_leaving_and_entering_for() {
        let mut classes = vec![Class::Sure; 6];
        classes.push(Class::Call);
        let (states, _) = cheapest(&own(&classes), &COSTS);
        assert_eq!(states, vec![LEAVE; 7]);
        // With enough work around it, it is.
        let mut classes = vec![Class::Sure; 30];
        classes.push(Class::Call);
        let (states, _) = cheapest(&own(&classes), &COSTS);
        assert_eq!(&states[..30], &[NATIVE; 30]);
        assert_eq!(states[30], LEAVE);
    }

    #[test]
    fn a_native_loop_inside_draws_its_outer_loop_in() {
        // The outer loop's own work is three proven instructions around
        // a native loop: out of the body it would be entered every turn.
        let items = vec![
            Item::Own(0, Class::Sure),
            Item::Loop(1, 9, Inner::Native),
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
    fn nothing_native_is_a_candidate_whatever_the_ring_holds() {
        for classes in [
            vec![Class::Interp; 4],
            vec![Class::Call; 2],
            vec![Class::Guess(0); 9],
            Vec::new(),
        ] {
            let (states, _) = cheapest(&own(&classes), &COSTS);
            assert!(states.iter().all(|s| *s != NATIVE), "{classes:?}");
        }
    }
}
