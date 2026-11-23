<!-- SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile) -->
<!-- SPDX-License-Identifier: CC-BY-4.0 -->

# 0005: Stage C and stage D start as one thing: typed specializations, compiled ahead of time, entered from the VM

- Status: accepted for the first step, 2026-11-23. Revisited with its numbers when that step runs.
- Issues: #32 (stage C), #34 (stage D), #35 (values), #98 (the study); otmof-ops/cig-script#256 (cig-cherry)
- Research: [the studies](../research/README.md); the entries each point rests on are named in place.
- Related: [0001](0001-backend.md) (the staged path), [0006](0006-native-code-in-a-program.md) (how a program carries native code), [0007](0007-where-the-pieces-live.md) (crates)

## The question

[0001](0001-backend.md) planned stage C as native code for every function, with dynamic values, the standard library and the kernel called through a runtime library, and stage D as a later step where typed code skips the dynamic value path. Now that both are due: **what is the first native code a program runs, and what has to exist before it?**

## What was measured and read

- **Types in compute code do not move.** In 21 compute programs, 226 of 227 arithmetic and comparison sites saw one pair of operand types for the whole run, every field site saw one shape, 28 of 29 call sites called one function, and every function was only ever called with one row of argument types. In cigo's own script, the largest cig program there is, no function saw more than three rows ([the census](../research/data/census/README.md)).
- **Step marks are a quarter of what the VM executes** in a compute program (9.1% to 50.0%, median 23.8%), and half of the median program's register moves copy a counted value (the same census).
- **A stage `b` program takes 1.97 times Lua 5.4's time, 2.77 times LuaJIT's interpreter's and 5.20 times LuaJIT's with its compiler on** ([BENCHMARKS-2026-11-15](../BENCHMARKS-2026-11-15.md)). What separates it from them is not dispatch alone: every value is 24 bytes behind a tag, every integer add is a call of the shared operator, and every statement charges the budget through memory.
- **Julia gets its speed from inference across calls and one compiled copy per row of argument types,** with fixed limits that make inference end ([study 09](../research/studies/09-types-julia-dotnet-rustc.md), entries 1, 3 and 6).
- **GHC's worker and wrapper** split a function into a wrapper that takes boxed values and a worker that takes raw machine values, so a loop never boxes ([study 10](../research/studies/10-whole-program-mlton-and-unboxing-ghc.md), entry 10).
- **Every production engine keeps its lower tier under its native code** and falls back to it ([study 04](../research/studies/04-spidermonkey-cacheir-and-warp.md), entries 9 and 17; [study 12](../research/studies/12-v8-and-javascriptcore-tiers.md), entries 8 and 11).
- **The generic path needs three things before its first instruction runs:** the value layout decided and built across the runtime (#35), a runtime library with a C interface (#4), and a linker in the compiler ([study 17](../research/studies/17-from-machine-code-to-a-program.md), entries 3 and 5). None exists.

## The decision

1. **A program built at stage `c` is a stage `b` program with native code added.** It carries the whole script as verified bytecode, which the VM runs, and native code for some of its functions. A function with no native code runs as it does at stage `b`. Nothing a program can do is lost by building it at stage `c`, on any target.

2. **The unit of native code is a specialization: one function and one row of argument types.** A function may have several. A function nobody calls with a row the compiler could type has none.

3. **The rows come from inference over CIR, across calls, from the top level down.** A call's argument types are the row its callee is inferred with; the callee's inferred result is the call's type. The lattice is CIR's nine types (`dynamic` on top) to begin with. A function keeps at most three rows; a fourth makes it `dynamic` in every argument, which is always sound. Inference lives with CIR, in cig-script, so the VM can use it too.

4. **What is compiled first is the specialization in which every value is an `int`, a `float`, a `bool` or `null`.** Its values are raw machine values in registers: no tag, no reference count, no call into the runtime. This is stage D's "typed code skips the dynamic value path" and stage C's "native code" in one piece: the worker of worker and wrapper.

5. **The VM's call is the wrapper.** When a call's callee has a native specialization and the arguments' types are its row, the VM calls the native code with the raw values and wraps the result. When they are not, it runs the bytecode. The guard is a tag test per argument.

6. **Behavior is the tree-walker's, exactly, or the function is not compiled.** Integer overflow, division and modulo by zero, a NaN in an ordering, the call depth and the step budget leave native code with the place and the operands, and the error is then made by the interpreter's own operator, so its code, message, hint and position cannot differ ([0006](0006-native-code-in-a-program.md)). The step budget is charged at every step mark, one at a time.

7. **Left to the bytecode in this first step,** each because its native form needs something not yet decided:
   - an integer divided by an integer (the result is an `int` or a `float` depending on the values);
   - the remainder of two floats (needs a call out of native code);
   - strings, lists, maps, closures, captured variables, builtins, methods (need the value layout and calls into the runtime);
   - `try`, `burn`, chains (need exits that carry open tries and burns back to the VM).

8. **The value layout stays open.** This step never holds a dynamic value in native code, so it does not need #35's answer. [Study 11](../research/studies/11-the-value-layout.md) gives the candidates; the decision is made with #35's prototype measurement.

## Why not the generic path first

- **It costs the most and gains the least.** What native code over dynamic values removes is instruction dispatch; the tag test on every operand, the reference counts and the calls into the runtime stay. Raw integers and floats in registers are where native code leaves an interpreter behind, and the census says that is what compute code is made of.
- **It cannot land in steps.** A program that stops carrying an interpreter for its own code must be able to compile every construct before the first one ships. A stage `b` program with native code added is correct at every step, and each new construct moves from the bytecode to native code when it is ready.
- **It is the same work, later.** The generator, the loader, the calling convention, exits, the exact budget and the differential tests built for typed specializations are what the generic path needs too. Dynamic values enter native code when #35 has given them a layout.

## What this is not

- **Not a run-time compiler.** Everything is compiled when the program is built. cig-cherry (otmof-ops/cig-script#256) later drives the same generator from what the VM observed; it adds rows the build could not infer, never a second design.
- **Not a change to what a program is.** The same file, the same runtime, the same kernel, journal and rollback; `--interp` still runs the source on the tree-walker.

## What follows

In the order the census argues for:
1. loops at the top level as units of their own (most benchmark programs keep their hot loop outside any function);
2. lists and maps reached through calls into the runtime, then shapes for records (#35);
3. integer division and float remainder through exits to the VM;
4. dynamic values in native code, once the value layout is decided.

## What would change it

- The first step's own numbers: `fib` at stage `c` against stage `b` and against LuaJIT's interpreter, by `bench/compare`. If typed specializations do not at least halve stage `b`'s time on the programs they cover, the reasoning above is wrong somewhere and this record is reopened.
- A program whose behavior at stage `c` differs from the tree-walker's in any way the differential suite can show. There is no acceptable number of those.
