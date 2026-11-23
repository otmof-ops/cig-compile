<!-- SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile) -->
<!-- SPDX-License-Identifier: CC-BY-4.0 -->

# 0006: Native code in a program: the section, the loader, the calling convention, and leaving it

- Status: accepted for the first step, 2026-11-23
- Issues: #32 (stage C), #36 (start-up and size), #40 (hardened programs); otmof-ops/cig-script#256 (cig-cherry)
- Research: [study 03](../research/studies/03-wasmtime-ahead-of-time-and-winch.md) (Wasmtime ahead of time), [study 02](../research/studies/02-cranelift-back-end-and-regalloc2.md) (Cranelift's back end), [study 17](../research/studies/17-from-machine-code-to-a-program.md) (from machine code to a program)
- Related: [0004](0004-payload-format-2.md) (payload format 2), [0005](0005-what-goes-native-first.md) (what is compiled), [0007](0007-where-the-pieces-live.md) (crates)

## The question

[0005](0005-what-goes-native-first.md) adds native code for typed specializations to a stage `b` program. **Where do those bytes live, how do they become code a process may run, how is that code called, and how does it leave when it cannot go on?** Every answer has to keep the rules: no memory both writable and executable, the same bytes from the same input, the tree-walker's behavior exactly, and `unsafe` in one small crate.

## The decision

### 1. The bytes are a section of the program: `native`

Payload format 2 ([0004](0004-payload-format-2.md)) gains one section id, `native`, and one stage, `c`.

- **A stage `c` program is a stage `b` program** (its `bytecode` section is required and complete) **with a `native` section.** The record's `stage` is `"c"`. The section is not required: a runtime that cannot use it runs the bytecode.
- **It is stored as it is** (encoding `0`), with its sha256 in the section table like any other, and it sorts after the five known ids, by the rule format 2 already has for other ids.
- **Its body starts at a file offset that is a multiple of 65,536,** and the zero padding before it is shorter than that. Every other body keeps format 2's 16. 65,536 is the largest unit any target maps files in (4 KiB pages on x86-64 Linux, up to 64 KiB on aarch64 Linux, 16 KiB on Apple's, 64 KiB mapping granularity on Windows), so the code can be mapped where it lies.
- **The same input gives the same bytes:** the generator is deterministic and the section holds no time, path or host name.

**The body** (integers little-endian):

```text
code           the machine code, from the body's first byte
zero padding   to a multiple of 8
entry table    count x 24 bytes
site table     count x 24 bytes
footer         80 bytes, at the body's end
```

**The footer:**

| offset | size | field |
|---:|---:|---|
| 0 | 8 | magic, ASCII `CIGNATV1` |
| 8 | 4 | version: `1` |
| 12 | 4 | architecture: `1` x86-64, `2` aarch64 |
| 16 | 4 | interface version: `1` (the context's layout and the entry convention below) |
| 20 | 4 | code length |
| 24 | 4 | entry count |
| 28 | 4 | site count |
| 32 | 32 | the BLAKE3 in the header of the bytecode module this code was compiled from |
| 64 | 16 | zero |

**An entry** is one specialization: the function's index in the module (u32), the offset of its entry in the code (u32), its argument count (u8, at most 8), its result's type (u8), two zero bytes, its arguments' types (8 x u8), and four zero bytes. A type is `0` null, `1` bool, `2` int, `3` float. Entries are sorted by function index, then by argument types.

**A site** is a place native code can leave from: the position of the instruction in the source (start, end, line and column, 4 x u32), what happened there (u8), the operator (u8), the two operands' types (2 x u8), flags (u8; bit 0: the place is inside a loop of its function), and three zero bytes.

**A reader refuses** (as a damaged section, falling back to the bytecode with a note in `--cig-program`, and as `E1050` under `CIG_VALIDATE=1`): a wrong magic, version, architecture or interface version; lengths that do not add up to the body; an entry that names a function the module does not have, an argument count that is not the function's, a type code out of range, an offset outside the code; a site with an unknown kind or operator; non-zero padding; a module hash that is not the loaded module's.

### 2. The code is position independent and complete

- **No relocation is left for the loader.** A call between two native functions is relative, resolved when the section is built, as Wasmtime resolves every call inside an image ([study 03](../research/studies/03-wasmtime-ahead-of-time-and-winch.md), entry 3).
- **Native code calls nothing outside the section** in this step. It needs no table of runtime functions yet; when it does, calls go through a table reached from the context, never through a symbol (the same study, entry 12).
- **It cannot fault.** No instruction in it can trap: integer division is not compiled in this step, a remainder tests its divisor first, and every memory access is to the context or to the argument and result slots the caller passed.

### 3. Loading: the program maps its own file, read and execute

- **The runtime maps the code from the program's own file,** read and execute, private, at the section's offset. The pages are never writable, so no page is ever both. This is what the system's loader does for any executable, which is why policies that forbid making memory executable (`execmem`) allow it (#32, "Why Cranelift, and why ahead of time").
- **Before the first call** it checks the section's sha256 against the table over the mapped bytes, then the footer and the tables.
- **The section is as trusted as the program it is in, no more:** on Linux a running executable cannot be opened for writing, a program replaced on disk is a new file while the mapping keeps the old one, and whoever can change the file can change all of the program. The hash catches damage, as the other sections' hashes do; signing (#40) covers the section with the rest of the payload.
- **When it cannot** (a target without a loader, a file system mounted `noexec`, a policy that refuses, a damaged section), **the program runs its bytecode.** `--cig-program` says which it did and why. Nothing a program does depends on the native section being used.
- **Targets in this step: Linux on x86-64 and aarch64.** On macOS and Windows a signature does not cover bytes appended to an executable, so executable code there has to live inside the signed image; [study 17](../research/studies/17-from-machine-code-to-a-program.md), entry 8, has the shape (a section reserved in the runtime), and it is a later record. Until then `--stage c` is refused for those targets and `--stage b` is what they build.

### 4. Calling it

- **The entry** of a specialization has one signature on every target, in the target's C convention: `(context, arguments, result) -> status`, where `arguments` points to the raw values (one 64-bit slot each: an int, a float's bits, a bool as 0 or 1), `result` to one slot, and `status` is `0` or the kind of exit. It is a short piece of code that loads the arguments into registers and calls the function's body.
- **Inside the section** a body takes the context and its arguments in registers and returns a status and a value in two registers, so a caller tests one register after each call: the result convention #32 chose over unwinding.
- **The context** is 64 bytes the VM owns for the length of one call from it:

  | offset | size | field |
  |---:|---:|---|
  | 0 | 8 | steps left before the budget is spent (the largest value when the budget is off) |
  | 8 | 8 | calls left before the depth limit |
  | 16 | 4 | on an exit: the site |
  | 20 | 4 | on an exit: flags (bit 0: a native frame on the way out was inside a loop) |
  | 24 | 8 | on an exit: the left operand's raw value |
  | 32 | 8 | on an exit: the right operand's raw value |
  | 40 | 24 | zero |

- **The VM's call is the guard.** At a `call` of a function with entries, the VM counts the call's depth as for any call, compares the arguments' tags with an entry's row, and calls it. On status `0` it writes the result as the entry's result type says and clears the call's window as a bytecode call does. On no matching row it pushes a frame as before.

### 5. Leaving: every error is the interpreter's own

Native code never makes an error. Where the tree-walker would raise one it stores the site and the operands in the context and returns a status up through every native frame.

| what happened | what the VM does with it |
|---|---|
| an operator cannot give a result (integer overflow in `+`, `-`, `*` or negation; division or modulo by zero; a NaN in an ordering) | calls the interpreter's own operator on the two operands at the site's position: the error it returns is the error |
| a range wider than 10,000,000 | calls the interpreter's own range check on the two bounds |
| the step budget is spent | sets the counter to the budget and counts one step at the site's position, inside a loop if any frame on the way out was |
| the call depth is reached | makes the interpreter's own depth error at the call's position |

So the code, the message, the hint and the position of an error cannot differ from the tree-walker's: they are produced by the same functions from the same operands. If the interpreter's operator does not return an error for what native code reported, that is a defect in the generator, and the run stops with an internal error naming the site.

After any exit, the step counter and the call depth are what they would be had the VM run the same code: native code writes back the steps it has left before it returns, and the VM undoes the depth it counted.

### 6. The budget and the depth are exact

- **One step is charged at every step mark,** where CIR has one: each statement and each turn of a loop. A body keeps the count in a register, writes it to the context before a call and before returning, and reads it back after a call. The test is a decrement and a branch on the sign. Wasmtime's fuel charges a block at a time and is cheaper for it ([study 03](../research/studies/03-wasmtime-ahead-of-time-and-winch.md), entries 7 and 8); cig's rule is that `E515` falls on the same statement on every engine, so the charge is per mark. Charging several marks at once, where nothing between them can be observed, is an optimization for later and has to keep that rule.
- **The depth** is one decrement and test at each call between native functions, undone on return, against the same limit the interpreter uses.

### 7. `unsafe`

Mapping a file as code and calling into it cannot be written in safe Rust. **All of it is in one crate, `cig-native`** ([0007](0007-where-the-pieces-live.md)): the mapping, the call of an entry, and nothing else. Its reader of the section is safe code and is fuzzed. The VM, the lowering and the bytecode crate keep `forbid(unsafe_code)`; the VM uses the crate's safe interface, which is given only a section whose hash the program's table vouches for.

## What it costs

- **A program grows** by the native code and up to 65,535 bytes of padding before it.
- **A start pays** one mapping and a hash of the section when the first native call happens, not before: a program that never reaches one pays nothing.
- **A call from the VM** pays a tag test per argument, the context's set-up and one indirect call.

## What would change it

- A target where a program cannot map its own file as code and `--stage c` is still wanted: that target gets its own record (macOS and Windows are known to need one).
- Measurements: the cost of the per-mark charge against the per-block charge on the compute programs, once both can be built, decides how far the optimization in 6 goes.
