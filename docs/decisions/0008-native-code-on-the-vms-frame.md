<!-- SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile) -->
<!-- SPDX-License-Identifier: CC-BY-4.0 -->

# 0008: Native code for every function: it runs on the VM's frame, and the bytecode goes on wherever it stops

- Status: accepted, 2026-11-24. Revisited with its numbers when each step below lands; the first bodies changed three things, which [the last section](#what-the-first-bodies-showed-and-what-changed) records.
- Issues: #32 (stage C), #34 (stage D), #35 (values), #33 (the optimizer), #40 (hardened programs); otmof-ops/cig-script#256 (cig-cherry)
- Research: [study 04](../research/studies/04-spidermonkey-cacheir-and-warp.md) (entries 9 and 17), [study 12](../research/studies/12-v8-and-javascriptcore-tiers.md) (entries 8 and 11), [study 03](../research/studies/03-wasmtime-ahead-of-time-and-winch.md) (entries 7, 8 and 12), [study 14](../research/studies/14-compiling-fast.md); otmof-ops/cig-script `docs/research/luajit.md`, entry B7
- Related: [0005](0005-what-goes-native-first.md) (typed specializations), [0006](0006-native-code-in-a-program.md) (the section, the loader, leaving), [0007](0007-where-the-pieces-live.md) (crates)

## The question

[0005](0005-what-goes-native-first.md) compiled the functions whose every value is a number, and left the rest for later steps: loops at the top level, lists and maps through calls into the runtime, integer division through exits to the VM, and dynamic values once the value layout is decided. Its first step ran, and its numbers hold: a function it covers takes 0.045 to 0.128 of its stage `b` time ([BENCHMARKS-2026-11-23-stage-c](../BENCHMARKS-2026-11-23-stage-c.md)). **What does the rest of a program run as, and in what order does it get there?**

## What was measured and read

- **Numbers alone are rare.** Of the 21 programs of the shared benchmark set that have a hot loop, three compute on numbers alone (`fib`, `loop`, `mandel`). The other 18 index a list, read a field, call a method, build a string or call a closure inside the loop. Of cigo's 1,038 functions, the first step compiles none.
- **Most hot loops are at the top level,** outside any function, so nothing calls them: the unit of 0005 (a function and a row of argument types, entered at a call) does not reach them.
- **A unit that cannot hold a list cannot hold the loop around it.** One `xs[i]` in a loop of arithmetic keeps the whole loop on the bytecode. Each of 0005's later steps would have needed its own way in and out of native code for the same loop.
- **Every production engine keeps its lower tier under its native code and goes back to it at an exact instruction,** with the lower tier's state rebuilt ([study 04](../research/studies/04-spidermonkey-cacheir-and-warp.md), entries 9 and 17; [study 12](../research/studies/12-v8-and-javascriptcore-tiers.md), entries 8 and 11; LuaJIT's snapshots, entry B7). cig-cherry needs the same exit (#32's second comment).
- **cig's VM keeps a frame's whole state in one place:** its registers, in one stack of values, with the open tries and burns beside it. Nothing of a running function lives anywhere else.

## The decision

### 1. A function's native code is a body, and it runs on the frame the VM made

- **A body** is the native code of one function of the module, the top level included. It is entered with the frame the VM pushed for the call: the same registers, in the same place, holding the same values the bytecode would find.
- **Native code keeps no state of its own that the bytecode needs.** Between two instructions, whatever the bytecode would read is either in the frame or is written there before anything else can read it (section 5).
- So a stage `c` program is still a stage `b` program with native code added ([0005](0005-what-goes-native-first.md), point 1), and a function's body and its bytecode are two ways to run the same frame.

### 2. A body can stop at any instruction, and the bytecode goes on from there

- **A body returns to the VM with a pc:** "go on here". The VM's loop runs the bytecode from that instruction, as it would have had it run the function from the start.
- **That is the only way a body leaves,** and it is used for everything native code does not do:
  - **an error.** Native code never makes one ([0006](0006-native-code-in-a-program.md), section 5). Where an operation cannot give a result, the body leaves at that instruction and the interpreter runs it and raises: the code, the message, the hint and the position are the interpreter's because the interpreter made them. No table of sites and no operands are handed over.
  - **the step budget.** A step mark that would overspend the budget is not counted by native code: the body leaves at the mark, and the interpreter counts it and raises `E515`.
  - **a call, a tail call, a return.** The VM's own instruction makes the frame, checks the arity and the depth (`E401`, `E506`, `E514`), and ends the frame. A body goes on after a call when the callee has returned.
  - **anything a body has no code for.**
- **A body can be entered where it can be gone on from:** at the function's start, after each call, at the start of each loop, and at each `catch` and `finally`. These are its **resume points**. When the bytecode reaches one in a function that has a body, the VM enters the body there.
- **Nothing depends on a body being used.** A run that enters no body is a stage `b` run. `CIG_NATIVE=0` and every case where the section cannot be loaded ([0006](0006-native-code-in-a-program.md), section 3) stay what they are.

### 3. What is not inline yet, the interpreter runs

- **The context carries a table of functions of the runtime** ([0006](0006-native-code-in-a-program.md), section 2: "calls go through a table reached from the context, never through a symbol"). It starts with one: **run** the bytecode from this pc while it stays inside this range of instructions, and say where it stopped.
- **A stretch of instructions with no native form is one such call,** and what runs is the interpreter's own code for them, at the interpreter's own speed. An instruction inside the stretch that raises is offered to the frame's tries as it is at stage `b`; the body is told, and returns.
- **So every construct of the language is in every body from the first one built:** burns and their compensations, hops, chains, `try`, closures and captured variables, builtins and methods are the VM's instructions, run by the VM, in order, through the one kernel. Nothing is moved across a burn because nothing of a burn is compiled.
- **An instruction moves from the interpreter to native code when its inline form is ready,** one kind at a time, and each move is held to the tree-walker by the differential suite before it merges.

### 4. An inline form is the interpreter's result for the operands it tests for

- **Native code reads a register's type from the value itself** (a tag) and takes its fast path only for the types the path was written for: two ints, two floats, an int and a float, a bool. For anything else it runs that one instruction through the interpreter (section 3).
- **A fast path gives exactly what the interpreter gives** for those operands, or it is not taken: an int sum that overflows, a zero divisor, a NaN in an ordering, an index past a list's end all go to the interpreter, which raises.
- **Writing a register never loses a value that has to be freed.** A fast path writes a number over a number; a register holding anything else is written by the interpreter.
- **A captured variable's register is never touched inline.** It carries a cell the closures share; the interpreter's instructions read and write it.

### 5. Stage D on the frame: a register of proven type lives in a machine register

- **A type pass over the bytecode** gives, for each register at each instruction, the type it has on every path the bytecode can take: `int`, `float`, `bool`, `null`, or "anything". It is the lattice of [0005](0005-what-goes-native-first.md) and #34, read from the instructions' own rules (an `int` plus an `int` is an `int` or an error; a counted loop's variable is an `int`).
- **A register proven to be a number or a bool is held in a machine register** while native code runs: no tag is tested, nothing is read from or written to the frame, and a loop of arithmetic is a loop of machine arithmetic.
- **It is written back to the frame before anything else can read it:** before the interpreter runs a stretch, before the body leaves, and where two paths meet that do not agree on it.
- **Proven, never guessed.** #34 keeps speculation (types that a profile or the VM saw) apart from inference, and so does this: a body tests no guess. What is not proven is read with its tag (section 4).
- **A proven type is still checked where it comes from the frame** (at a resume point, after a stretch the interpreter ran): one compare a register. A value of another type there is a defect in the type pass, and the run stops with an internal error naming the place: never a wrong answer ([0006](0006-native-code-in-a-program.md), section 5).
- **A function with a `try` keeps every register in the frame** for now. A `catch` or a `finally` can be entered from any instruction of its range, and the type pass does not yet follow that.

### 6. What native code knows of a value

- **Four things, and they are part of the interface version:** a value's size in a frame, where its tag is, where its number is, and the tags of null, bool, int and float. `cig-native` states them; the runtime checks them against its value type when it is built.
- **The value type gets a stated representation** (a tag byte, then the payload), which Rust otherwise leaves to the compiler. Its size does not change.
- **This decides nothing of #35.** A new layout changes those constants and the interface number, and every program is rebuilt, as for any change of interface. Strings, lists, maps and functions stay behind the runtime's own functions: native code never reads inside one, with one exception stated when it is built (a list's elements, through a view the runtime gives and takes back).

### 7. Calls

- **A call from a body goes through the VM,** which does what it does at stage `b` and then enters the callee's body, its typed entry ([0005](0005-what-goes-native-first.md)), or its bytecode. The caller's body resumes at the pc after the call.
- **Typed entries stay.** A function on numbers alone keeps the entry of 0005: its arguments in registers, no frame, a direct call from one entry to another. It is the fast call, and the VM's guard at a call still chooses it first.
- **A direct call from a body to a typed entry,** where the type pass proves the arguments' types and the lowering names the callee, is a later step of this record. So is a call from one body to another without the VM.

### 8. What stays exact, and by what

| | how |
|---|---|
| errors: code, message, hint, position | the interpreter raises every one, at the instruction, from the frame |
| the step budget | native code counts a mark's steps only when the budget holds all of them; otherwise the interpreter counts them one by one |
| what a loop says when the budget ends inside it | the frame's loop nesting is written with every mark, as the bytecode's `steps` writes it |
| the call depth, the stack's ceiling, the arity | the VM's call |
| burns, the journal, rollback | the VM's instructions, in order, through the one kernel |
| `try`, `catch`, `finally` | the VM's handler tables, entered at the instruction that raised |
| the order of output | `exhale` is the VM's instruction |
| floats | the same operations in the same order, no contraction, no reassociation ([0005](0005-what-goes-native-first.md), point 6) |

### 9. Which functions get a body

- **The build chooses, by a rule that reads only the program,** so the same input gives the same bytes: a function gets a body when native code has something to add to it. The first rule is "it has a loop"; it is tuned by measurement and each change of rule is a change of the generator's version.
- **The rule a section was built with is written in the section,** so `cigcompile verify` compiles with the same one. The differential suite builds every case with a body for every function as well.

### 10. The section and the context ([0006](0006-native-code-in-a-program.md)) grow

- **The section's version is 2.** After the entry and site tables come a **body table** (the function, where its code starts, its resume points) and a **resume table** (the pc of each). A reader refuses a body for a function the module does not have, a resume point that is not the start of an instruction, and tables out of order, as it refuses a bad entry.
- **The interface version is 2.** The context gains the frame's address, the table of runtime functions, the frame's loop nesting, and a word the runtime's functions are handed back. A body's signature is `(context, resume point) -> where it stopped`.
- **Loading.** A program whose top level has a body reads and maps its section when the run starts, not at the first call. A program without one is unchanged.
- **Who answers for the bytes is unchanged** ([0006](0006-native-code-in-a-program.md), section 3). Native code now writes the VM's registers, which is no more than it could do before: it is machine code in the program's own process, trusted as the runtime is, and `cigcompile verify` holds it to the compiler byte for byte.

## Why this, and not 0005's later steps one by one

- **One way in and one way out.** Loops at the top level, lists and maps, integer division and dynamic values each needed a path between native code and the VM. Leaving at a pc with the frame whole is that path for all of them, and it is the exit cig-cherry needs.
- **It lands in steps, and every step is correct.** 0005 held the generic path back because a program had to compile every construct before the first one shipped. With the interpreter under every body, the first body built already runs every construct; what changes from step to step is only how much of it is machine code.
- **The three things the generic path waited for are not needed for it:**
  - *the value layout* (#35): native code knows a number's tag and place, and nothing else of a value (section 6);
  - *a runtime library with a C interface* (#4): the table in the context, which starts with one function;
  - *a linker* in the compiler: the code stays in the `native` section, mapped as it is ([0006](0006-native-code-in-a-program.md)).
- **It is where the numbers are.** Field reads, list indexing and calls through the runtime gain little over a good interpreter; what gains is arithmetic and control on values whose type is known, and that is now reachable inside any loop, not only in functions made of nothing else.

## What it costs

- **A round trip.** Entering a body and leaving it costs more than the interpreter's own dispatch of one instruction. A function made of calls and nothing else gains nothing from a body, which is what the rule of section 9 is for.
- **Code size.** A body is larger than its bytecode. The build reports it.
- **Start.** A program with a body for its top level reads, checks and maps its native section before its first statement.
- **Two generators' worth of rules to keep exact:** the typed entries of 0005 and the bodies of this record. They share the back end, the section, the loader and the suite.

## What is not decided here

- Speculation: types from a profile or from what the VM saw, with exits that give up a guess (#33, #34, otmof-ops/cig-script#256). The exit this record builds is the one they need.
- Shapes for maps and typed storage for lists (#35). Until then a field read and a method call are the interpreter's.
- Inlining a small function into its caller (#33).
- Native code on macOS and Windows: [0006](0006-native-code-in-a-program.md), section 3, still holds.

## What would change it

- **A body that makes its function slower.** Each step is measured on the shared set against stage `b`; a program that loses is fixed by the rule of section 9 or by the step being held back, and the figure is published either way.
- **A difference from the tree-walker** that the differential suite can show, in any program, at any step. There is no acceptable number of those.
- **The round trip's cost,** if measurement shows calls dominate what is left: then a call from body to body without the VM moves up the order.

## What the first bodies showed, and what changed

The first build made a body for every function with a loop, with every instruction that has an inline form native and every resume point of section 2. It was right on all 25 programs of the shared set at its first full run, in every mode, and **14 of the 25 were slower than their stage `b` programs**, by up to 2.04 times. Three causes, and what each changed:

- **Between two resume points the VM ran a function with a body one instruction at a time,** on its general path. It now runs it on its fast loop, as a function without a body: a second copy of that loop, on the machine that has bodies, so a program without them runs the code it ran before (stage `b`'s instruction counts moved by under 0.1%). That alone took `fannkuch` from 1.85 to 1.08 and `richards` from 2.04 to 1.19.
- **Crossing between native code and the interpreter costs several instructions' worth, and a loop of mixed work crosses at every other instruction.** A stretch is about 12 ns here, leaving a body and entering it again about 30 ns, and an instruction saves about 2 ns by being native. So "it has a loop" (section 9) is not when native code has something to add. **Section 9's rule is now a plan:** each loop, innermost first, is settled by what its own instructions cost a turn as native code, as stretches or left to the VM, the cheapest answers found exactly, with "all of it is the interpreter's" always among them. A function none of whose loops pays gets no body. The plan and its figures are in the generator's [README](../../crates/cig-codegen/README.md); the differential suite still builds every case with a body for every function, now as two more programs a script (one with every proven type checked).
- **Section 2's resume points are the compiler's to choose,** and a body lists only the ones its native code starts at: the start of a native loop, the instruction after a call inside one. The VM stops its fast loop before each and enters there. A body need not be entered at its function's start, and the section no longer asks for it.

**A test of an operand's tag is a guess, and it is counted.** Section 5 stands for a held register: proven, never guessed. An inline form on a register of unknown type (section 4) tests the tag and has the interpreter run the instruction when the test fails, and that failure is a call, where the interpreter alone would have spent 2 ns. So:

- **the plan counts such a form as failing a share of the time,** and lets it join a stretch beside it, where failing costs nothing;
- **the runtime counts the failures** (a third function of its table, `again`, runs the one instruction) and **gives a body up** at the resume point it was entered at when more than one in 16 steps fails over 64 failures. The bytecode then has the loop. What a wrong guess can cost is bounded at a few microseconds a loop.

**The checked build earned its place on its first full run.** The suite's second test build compares every type the pass proved with the register's real tag (section 5's check, at every instruction instead of where a value comes from the frame). Over 3,032 rows it differed on three of the hundred random programs, and the pass was wrong: a range written as a value is checked by a `forprep` whose jump goes to the very next instruction, the pass kept the rule of the path that writes the loop's variable, and so took that register for an int after an empty range that leaves it alone. Where an instruction's two ways on reach the same pc, a register now has a type only when both give it. No program built by the rule a build uses differed, and none would have computed wrongly (a held register is checked where it is read from the frame, and the run stops); the run would have stopped with an internal error on a script that was right.

This is the exit that speculation needs ("What is not decided here"), built small: a guess that fails goes back to the interpreter at an exact instruction, and one that keeps failing is given up.

**With the plan, no program of the 25 is slower than its stage `b` program** (0.91 to 1.05 of its time, whole processes, start-up included), and `fib`, `loop` and `mandel` take 0.13, 0.28 and 0.14 of it. The other programs' loops are left to the bytecode: what they need is in "What is not decided here" (lists, fields) and section 7 (calls), and the plan will take each loop native the day its instructions have inline forms that pay.

**The first such forms: a list's elements, by an int.** Decided here because it did not need #35: native code does not learn what a list is. It asks the runtime where a list's elements lie and how many there are (the function `list` of the runtime's table, which 0006's interface 2 already had), keeps the answer in two machine registers, and reads and writes elements that own no memory (numbers, bools, null) there. The answer holds until native code next calls a function of the runtime that runs the interpreter, after which it asks again; a loop that is native whole asks once. The plan carries that as a fourth state of the ring (the loop's lists are in view). With it `fannkuch` takes 0.27 of its stage `b` time, `sieve` 0.75, `deltablue` 0.90 and `richards` 0.93.
