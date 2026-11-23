<!-- SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile) -->
<!-- SPDX-License-Identifier: CC-BY-4.0 -->

# 0006: Native code in a program: the section, the loader, the calling convention, and leaving it

- Status: accepted for the first step, 2026-11-23; amended the same day to what was built (the loader maps the code alone, who answers for the bytes, the stack, `CIG_NATIVE`, what `--stage c` does where native code cannot be had)
- Issues: #32 (stage C), #36 (start-up and size), #40 (hardened programs); otmof-ops/cig-script#256 (cig-cherry)
- Research: [study 03](../research/studies/03-wasmtime-ahead-of-time-and-winch.md) (Wasmtime ahead of time), [study 02](../research/studies/02-cranelift-back-end-and-regalloc2.md) (Cranelift's back end), [study 17](../research/studies/17-from-machine-code-to-a-program.md) (from machine code to a program)
- Related: [0004](0004-payload-format-2.md) (payload format 2), [0005](0005-what-goes-native-first.md) (what is compiled), [0007](0007-where-the-pieces-live.md) (crates); the layout's own reference is the `cig-native` crate's README in cig-script

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

**An entry** is one specialization: the function's index in the module (u32), the offset of its entry in the code (u32), its argument count (u8, at most 8), its result's type (u8), two zero bytes, its arguments' types (8 x u8), and four zero bytes. A type is `0` null, `1` bool, `2` int, `3` float. Entries are sorted by function index, then by argument types, no two alike; every entry of one function has the same argument count, and a function has at most 16. An entry's offset is inside the code, and where instructions have one size (aarch64: 4 bytes) it and the code's length are multiples of it.

**A site** is a place native code can leave from: the position of the instruction in the source (start, end, line and column, 4 x u32), what happened there (u8), the operator (u8), the two operands' types (2 x u8), flags (u8; bit 0: the place is inside a loop of its function), and three zero bytes. A site says only what its kind uses, and every other field is zero: an operator only where an operator left, and the operand types that operator leaves with.

**A section has one encoding.** The writer and the reader apply the same rules, so what is written is read back, and what is read is written back as the same bytes.

**A reader refuses** (as a damaged section, falling back to the bytecode with a line in `--cig-program`, and as `E1050` under `CIG_VALIDATE=1`): a wrong magic, version, architecture or interface version; lengths that do not add up to the body; whatever breaks the tables' rules above; non-zero padding or reserved bytes; an entry for the top level, for a function the module does not have, or with an argument count that is not the function's; a module hash that is not the loaded module's.

### 2. The code is position independent and complete

- **No relocation is left for the loader.** A call between two native functions is relative, resolved when the section is built, as Wasmtime resolves every call inside an image ([study 03](../research/studies/03-wasmtime-ahead-of-time-and-winch.md), entry 3).
- **Native code calls nothing outside the section** in this step. It needs no table of runtime functions yet; when it does, calls go through a table reached from the context, never through a symbol (the same study, entry 12).
- **It cannot fault.** No instruction in it is reached in a state where it traps: integer division is not compiled in this step, a remainder tests its divisor first and leaves on zero, and every memory access is to the context or to the argument and result slots the caller passed. The generator refuses a function whose code holds any other trap, or a relocation that is not a call inside the section.

### 3. Loading: the program maps its own code, read and execute

- **When.** The first time the run calls a function of the script. A run that calls none never reads the section. `CIG_VALIDATE=1` loads it at the start instead.
- **Read, then mapped.** The runtime reads the section from the program's own file and checks it against the sha256 the table gives, reads its tables, and checks them against the module it loaded: the module's hash, and each entry's function and argument count. Then it maps **the code alone** (the body's first bytes, up to the code's length), read and execute, private. The tables are data and stay data. The pages are never writable, so no page is ever both. This is what the system's loader does for any executable, which is why policies that forbid making memory executable (`execmem`) allow it (#32, "Why Cranelift, and why ahead of time").
- **Only its own image.** The file that is mapped is the image the process is running (`/proc/self/exe`, checked by device and inode against the file the program was read from), and no other. The mapping is then as stable as the process's own code: the system maps that code from the same file in the same way.
- **What is run is what was read.** Before any entry can be called, the mapped bytes are compared with the bytes that were read and checked.
- **When it cannot** (a target without a loader, a file system mounted `noexec`, a policy that refuses, a section that is not this program's), **the program runs its bytecode.** `--cig-program` says what the section holds and whether this machine runs it. Nothing a program does depends on the native section being used. A seccomp filter that kills a process for mapping executable memory kills it here too: nothing can fall back from that.
- **`CIG_NATIVE=0`** makes a program run its bytecode and never load its native code.
- **Targets in this step: Linux on x86-64 and aarch64.** On macOS and Windows a signature does not cover bytes appended to an executable, so executable code there has to live inside the signed image; [study 17](../research/studies/17-from-machine-code-to-a-program.md), entry 8, has the shape (a section reserved in the runtime), and it is a later record. `--stage c` for a target without a loader builds the stage `b` program, which is the same program without native code, and says so in a note; so does `--stage c` on a runtime that does not run stage `c` programs (a cig says which stages it runs in its own bytes, beside its version), and for a script none of whose functions compiles.

#### Who answers for the bytes

No check makes bytes safe to run, and the format cannot show that a section is the generator's code. The sha256s show that a program is whole, not who made it.

- **A stage `c` program's native section is machine code, trusted as the program's runtime is and not as its script is.** The VM verifies bytecode and confines it to the kernel and its journal; native code is run as it is. cigcompile writes native code that computes on numbers and calls nothing, but a builder who put other code there has made a program that does something else, as one who changed the runtime has. `runtime_sha256` names a program's runtime, not its native code.
- **`cigcompile verify` shows it is the compiler's.** It compiles the program's script again, at the recorded level for the recorded target, and holds the `native` section to the result byte for byte: the same input gives the same bytes. That is how machine code that cannot be read is checked against a script that can.
- **Signing** (#40) covers the section with the rest of the payload.

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

- **The VM's call is the guard.** At a `call` of a function with entries, the VM compares the arguments' tags with each of the function's rows; on a match it counts the call's depth as for any call and calls the entry. On status `0` it writes the result as the entry's result type says and clears the call's window as a bytecode call does. On no matching row it pushes a frame as before. A tail call and a builtin's callback reach a function as bytecode in this step.
- **The stack is checked.** One call of any function in a section uses at most 4,096 bytes of stack (the generator compiles no function whose frame is larger), and native code's own calls nest no deeper than the depth limit leaves them. The VM enters native code only where the stack has room for that many calls; otherwise that call runs as bytecode. The depth is not cut to fit: that would move `E506`.

### 5. Leaving: every error is the interpreter's own

Native code never makes an error. Where the tree-walker would raise one it stores the site and the operands in the context and returns a status up through every native frame.

| what happened | what the VM does with it |
|---|---|
| an operator cannot give a result (integer overflow in `+`, `-`, `*` or negation; division or modulo by zero; a NaN in an ordering) | calls the interpreter's own operator on the two operands at the site's position: the error it returns is the error |
| a range wider than 10,000,000 | calls the interpreter's own range check on the two bounds |
| the step budget is spent | sets the counter to the budget and counts one step at the site's position, inside a loop if any frame on the way out was |
| the call depth is reached | makes the interpreter's own depth error at the call's position |

So the code, the message, the hint and the position of an error cannot differ from the tree-walker's: they are produced by the same functions from the same operands. If the interpreter's operator does not return an error for what native code reported, or native code comes back in any other way it cannot rightly come back (a status that is not its site's kind, a budget exit that did not overspend the budget by the one step its mark took, a return with more steps left than it was given), that is a defect in the generator, and the run stops with an internal error naming the site: never a wrong answer.

After any exit, the step counter and the call depth are what they would be had the VM run the same code: native code writes back the steps it has left before it returns, and the VM undoes the depth it counted.

### 6. The budget and the depth are exact

- **One step is charged at every step mark,** where CIR has one: each statement and each turn of a loop. A body keeps the count in a register, writes it to the context before a call and before returning, and reads it back after a call. The test is a decrement and a branch on the sign: at a budget exit the count is -1, the mark that spent the budget counted. Wasmtime's fuel charges a block at a time and is cheaper for it ([study 03](../research/studies/03-wasmtime-ahead-of-time-and-winch.md), entries 7 and 8); cig's rule is that `E515` falls on the same statement on every engine, so the charge is per mark. Charging several marks at once, where nothing between them can be observed, is an optimization for later and has to keep that rule.
- **The depth** is one decrement and test at each call between native functions, undone on return, against the same limit the interpreter uses.

### 7. `unsafe`

Mapping a file as code and calling into it cannot be written in safe Rust. **The machinery is in one crate, `cig-native`** ([0007](0007-where-the-pieces-live.md)): the mapping, the comparison with what was read, the call of an entry, and nothing else. It splits what can be safe from what cannot: mapping is safe and gives nothing to read or call; making a mapping callable is one `unsafe` step, where the caller answers for the bytes and for the file not changing; a call is checked against its entry. The runtime makes that step in one place (`src/native.rs` in cig-script), which states its grounds. The crate's reader of the section is safe code and is fuzzed in CI. The VM, the lowering and the bytecode crate keep `forbid(unsafe_code)`.

## What it costs

- **A program grows** by the native code and up to 65,535 bytes of padding before it.
- **A start pays nothing.** The first call of one of the script's functions pays reading and hashing the section, one mapping and one comparison of the code; a program that calls none pays nothing.
- **The runtime grows** by about 44 KB on x86-64 Linux (the reader, the loader and the VM's side).
- **A call from the VM** pays a tag test per argument, the context's set-up and one indirect call.

## What would change it

- A target where a program cannot map its own file as code and `--stage c` is still wanted: that target gets its own record (macOS and Windows are known to need one).
- Measurements: the cost of the per-mark charge against the per-block charge on the compute programs, once both can be built, decides how far the optimization in 6 goes.
