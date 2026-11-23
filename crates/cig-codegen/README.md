<!-- SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile) -->
<!-- SPDX-License-Identifier: CC-BY-4.0 -->

# cig-codegen

Typed functions of a CigScript module to machine code, on Cranelift, for
cig's stage `c` programs.

## What it does

One job: a module's IR and its types in, a `native` section out.

`compile` takes the module (`cigscript::lower::cir`), its inference
(`cigscript::lower::cir::infer`), each function's index in the bytecode
module, the module's hash and a machine (`cig_native::Arch`). It picks
the specializations that qualify, compiles each with Cranelift, packs
the code and its tables into a `cig_native::Section`, and reports every
specialization as compiled or left to the bytecode, with the reason.

It knows nothing of programs, files or the command line.
`cigcompile build --stage c` calls it and puts the section in a program;
cig's VM loads the code and calls it. The layout both sides use is
cig-script's `cig-native` crate. The design is decisions
[0005](../../docs/decisions/0005-what-goes-native-first.md),
[0006](../../docs/decisions/0006-native-code-in-a-program.md) and
[0007](../../docs/decisions/0007-where-the-pieces-live.md).

The code does exactly what the tree-walker does. It never makes an
error: where the interpreter would raise one, native code writes the
site, the operands and the steps it has left to the context it shares
with the VM and returns a non-zero status. The VM then has the
interpreter's own function make the error from those operands, so an
error's code, message, hint and position cannot differ.

## What is compiled

A specialization is a function and one row of argument types. It is
compiled when:

1. it is not the top level, it is one function of the bytecode module,
   it has at most 8 parameters, and every type of its row and its result
   is an int, a float, a bool or null;
2. every value its body defines has one of those types on every path,
   and every instruction of its body is one of these:
   - a number, a bool or null; a read or a write of a variable of its
     own that no other function shares;
   - `+ - *` on numbers, `%` on ints, `/` where an operand is a float,
     `< <= > >=` on numbers, `==` and `!=`, unary `-` and `not`,
     `and`, `or` and `??`;
   - `if`, `while`, `break`, `continue`, `snuff`;
   - a call of a function by name, where the specialization it enters
     is compiled too;
3. each `for` of it is `for i in a..b` over two ints, which becomes a
   counted loop;
4. its frame is at most 4,096 bytes (`cig_native::MAX_FRAME`).

An int divided by an int is not compiled: its result is an int or a
float by the operands' values.

What is left is never an error. It is an `Outcome` with its reason
(`line 4: a list or a map`), and the function runs as bytecode. A
function that calls one that is left is left too. An `Error` is a defect
of the generator: Cranelift refused what it was given, a relocation that
is not a call into the section, a trap in the code, or tables the
section's writer refuses.

## The code

Each body is one Cranelift function in the target's C convention:
`(ctx, one parameter an argument) -> (status, value)`. A value is 64
bits: an int, a float, a bool as 0 or 1, zero for null. Each body has an
entry of the one signature every target shares, which is what the VM
calls:

```rust
extern "C" fn(ctx: *mut Context, args: *const u64, ret: *mut u64) -> u32
```

The entry loads the arguments, calls the body, stores the value when
the status is 0 and returns the status.

- **Steps.** The code counts a step where the tree-walker does (each
  statement, each turn of a loop) and leaves when the budget is spent,
  at the same place.
- **Depth.** A call takes one from the depth the context gives and
  leaves when none is left; a return gives it back.
- **Arithmetic.** `+ - *` on ints leave on overflow; `%` leaves on a
  zero divisor and is 0 for `-1`; unary `-` leaves on the least int; a
  float divided by zero leaves; an ordering with a NaN leaves; a range
  wider than the limit leaves. Each leaves with the operands as they
  were.
- **No traps, no calls out.** The code has no instruction that can
  trap and calls nothing but other bodies of its section.

## The same input gives the same bytes

On any host, for either machine:

- specializations are compiled in the order of (module index, row);
  every body is laid out before every entry;
- sites are numbered through the section in that order;
- the crate holds no unordered map, reads no clock and detects no host
  CPU: Cranelift is given the target's baseline and nothing else;
- Cranelift is pinned to one exact version.

That is what lets `cigcompile verify` compile a program's script again
and hold its native code to the result, byte for byte.

## Tests

`cargo test -p cig-codegen` (`tests/generate.rs`): the same bytes on two
compiles and the same entries and sites for both machines; what is
compiled, and the reason for each thing that is not; the frame bound;
and, on Linux, the generated code run through cig-native's loader:
results, every way of leaving with its site and operands, steps and
depth counted mark by mark. Whole programs are held to the tree-walker
by the differential suite at the workspace root, which builds each one
at stage `c`.
