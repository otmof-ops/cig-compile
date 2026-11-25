<!-- SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile) -->
<!-- SPDX-License-Identifier: CC-BY-4.0 -->

# cig-codegen

Typed functions of a CigScript module to machine code, on Cranelift, for
cig's stage `c` programs.

## What it does

Two jobs, in one crate: a module's IR and its types in, a `native`
section's **typed entries** out; and a module's bytecode and a rule in,
the section's **bodies** out. The two share the back end, the layout
and the test harness.

`compile` takes the module (`cigscript::lower::cir`), its inference
(`cigscript::lower::cir::infer`), each function's index in the bytecode
module, the module's hash and a machine (`cig_native::Arch`). It picks
the specializations that qualify, compiles each with Cranelift, packs
the code and its tables into a `cig_native::Section`, and reports every
specialization as compiled or left to the bytecode, with the reason.

`bodies` takes the bytecode module, a `Rule` (none, loops, or every)
and a machine, walks the functions pre-order, and gives back a `Bodies`:
a code blob with one body per function that gets one, each body's row
(its offset in the blob and its resume points), and an outcome per
function. A body is native code of signature
`extern "C" fn(ctx: *mut Context, at: u32) -> u64`: it is entered at
one of its resume points on the frame the VM pushed and returns a
stop that says where the bytecode goes on, raised, or a defect.

It knows nothing of programs, files or the command line.
`cigcompile build --stage c` calls both and puts the section in a
program; cig's VM loads the code and calls it. The layout both sides
use is cig-script's `cig-native` crate. The design is decisions
[0005](../../docs/decisions/0005-what-goes-native-first.md),
[0006](../../docs/decisions/0006-native-code-in-a-program.md),
[0007](../../docs/decisions/0007-where-the-pieces-live.md) and
[0008](../../docs/decisions/0008-native-code-on-the-vms-frame.md).

The code does exactly what the tree-walker does. It never makes an
error: where the interpreter would raise one, native code writes the
site, the operands and the steps it has left to the context it shares
with the VM and returns a non-zero status. The VM then has the
interpreter's own function make the error from those operands, so an
error's code, message, hint and position cannot differ.

## What it knows of the bytecode

Two modules read a function of the bytecode module, with no Cranelift
in them. They are what [decision
0008](../../docs/decisions/0008-native-code-on-the-vms-frame.md) builds
a function's native code from.

- **`flow`**: where each instruction can go on to, the targets of
  backward jumps, the registers that can carry a cell (the `A` of every
  `cellnew`), the block leaders, and the pcs a function's native code
  can be entered at (its start, after each call, each loop's start,
  each `catch` and `finally`).
- **`types`**: for each register before each instruction, the type it
  has on every path the bytecode can take: null, a bool, an int, a
  float, or anything. Every rule is read from what the VM's instruction
  does when it goes on (an int plus an int is an int, or the
  instruction raises). An opcode the pass has no rule for makes every
  register "anything"; so does a `try` anywhere in the function, for
  every register, and a `cellnew`, for its register.

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

## The bodies

A **body** is one function's native code, the top level's included,
run on the frame the VM made for the call: the same registers, holding
the values the bytecode would find. It is entered at one of its
**resume points** and returns a **stop**: `At(pc)` (the bytecode goes
on at `pc`), `Raised(pc)` (a helper raised while the frame's pc was
this one) or `Defect(pc)` (a register was not of the type the
compiler proved: a bug). A body never makes an error.

**Any instruction can be the interpreter's**, on the same frame, so a
body is always right and every construct of the language is in every
body. What is machine code is a question of time alone.

### What an instruction becomes (`src/body.rs`)

- **Native:** its inline form. `nop`, `jmp`, `loop`, `steps`; `mov`,
  `loadnull`, `loadbool`, `loadint`, `loadk` of a number or a bool;
  `add`, `sub`, `mul`, `div`, `mod` and their `k` forms on two ints or
  on a number and a float (an int divided by an int is an int when it
  divides exactly and a float otherwise, as the interpreter has it);
  `neg`, `not`; `eq`, `lt`, `le`, their `k` forms and their jump
  forms; `test`, `testn`, `testset`; `forprep` and `forloop` with a
  step of 1; `getindex` and `setindex` of a list by an int, and
  `iternext` over the list a `for` loop walks, for an element that is
  a number, a bool or null; `callm` of a list's `push` with one value,
  through the runtime's own function for it. A form is the
  interpreter's result for the operands it covers. For any others (an
  overflow, a zero divisor, a string among the operands) it calls the
  runtime's `again(ctx, pc)` before it has written anything, and the
  interpreter runs that one instruction.
- **A stretch:** a run of instructions with no inline form is one
  call, `run(ctx, from, to)`, and the body goes on wherever the
  bytecode comes out of that range. A `call` in a stretch is
  **carried**: the runtime makes it and runs the callee to its return
  under the body, in the callee's own body where it has one, and goes
  on in the range. Where it cannot (the stack has no room), it stands
  at the call, and the body returns that stop.
- **A leave:** the body returns `At(pc)`, and the VM's own loop runs
  the bytecode until it reaches a resume point. `tailcall` and `ret`
  are always leaves: the VM's instruction ends the frame. A `call` is
  one where the plan finds that cheaper than a stretch.

### Where a register's value is (stage D on the frame)

In the frame, with its tag, unless the type pass (`src/types.rs`)
proves it an int, a float or a bool at that instruction: then it is
**held** in a machine register and no tag is read or written. Three
rules keep that exact:

1. every held register is written to the frame before anything else
   reads the frame (a helper call, a return);
2. a register is read back where its value comes from the frame (a
   resume point, after the interpreter ran an instruction that wrote
   it), and its tag is compared with the proven type: a mismatch is
   `Defect(pc)`;
3. a held register's place in the frame never holds a value that owns
   memory, so writing it back frees nothing.

### What owns memory is the runtime's

A string, a list, a map or a function is shared by a count and freed
when the count runs out, and only the runtime keeps the count. So
machine code never copies such a value and never stores over one. It
does not have the interpreter run the instruction for that either:
that is a failed guess, and a body whose guesses keep failing is
given up, loop and all. It asks the runtime for the one thing it
cannot do:

- **`copy(ctx, dst, src)`:** `mov` of a value that owns memory, or
  over one that does. The runtime shares the one and frees the other.
- **`clear(ctx, reg)`:** before a number is stored over a register
  that holds a value that owns memory, the runtime frees it and
  leaves null there. It is asked for where the instruction can no
  longer fail (every operand tested and read), and just before the
  store: an instruction the interpreter runs instead must find the
  register as it was.
- **`push(ctx, list, value, dst)`:** `xs.push(v)`. Adding to a list
  may move every element, so no list is in view after it.

None of the three runs an instruction, so the frame's address, the
steps left and (after `copy` and `clear`) every other list in view
stand. Each says when it cannot (a register that carries a cell, a
list something is walking), and then the interpreter runs the
instruction.

### A loop's scratch registers (`src/live.rs`)

The test before a store, and the call of `clear` beside it, would run
at every write of every turn of a loop, and the call keeps every float
the loop holds out of the machine's registers around it, whether it
runs or not. So the emitter finds each loop's **scratch registers**
and frees them once:

- **Dead where the loop starts** (`live::live`, a backward pass over
  the bytecode's flow): nobody reads the register again before
  something writes it, so what it holds is nobody's to see.
- **Written only with numbers in the loop** (`live::plain_writer`):
  every instruction of the loop that writes it can only leave null, a
  bool, an int or a float, whoever runs it. `-`, `/`, `%`, a negation,
  a comparison and `not` always; `+` when one operand is proven to be
  no string and no list; `*` when one is a float or both are proven
  numbers; a number loaded; a copy of a proven number; a counted
  loop's own registers.
- **No cell, no call above it, one way into the loop**, and no handler
  or burn in the function.

The loop's first block is its entry from outside: it has the runtime
free each scratch register that owns memory, and a turn of the loop
jumps to the head past it. In the loop a write to such a register is a
plain store. A body entered inside the loop, and one the interpreter
comes back into from a stretch that began outside it, makes the
registers ready before it goes on: one nobody reads again is freed,
and one that is read again must hold a number (the body returns
`Defect(pc)` if it does not). `Mode::Checked` tests every such write.

### A list's elements, where they lie

A register that holds a list is never held in a machine register: the
list is shared and counted, and only the interpreter copies or frees
one. Machine code reads and writes its **elements**:

- **It asks the runtime once** (`list(ctx, reg)`) where the list's
  elements lie and how many there are, and keeps the answer (a
  **view**) in two machine registers.
- **A view holds until machine code next calls a function of the
  runtime that runs the interpreter** (`run`, `again`), which may grow
  the list or put another value in its register. After such a call no
  list is in view, and the next read asks again. A loop that is native
  whole asks once, however many turns it makes.
- **A read** takes an int index the list has an element at (an index
  below zero counts from the end, which is the interpreter's to work
  out) and an element that owns no memory: a number, a bool, null. It
  copies the tag and the payload to the register.
- **A write** takes the same, a value that owns no memory, and an old
  element that owns none, so nothing is freed and nothing is shared.
- **The next item of a `for` loop** is read the same way: `iterprep`
  makes a list of what the loop walks and counts from zero, and
  `iternext` takes the item at the count, where the list has one.
- **An item that owns memory** (a string, a map in the list a loop
  walks) is put in its register by the runtime (`elem`), and the loop
  goes on in machine code.
- **Anything else is the runtime's first, then the interpreter's**
  (below): a map or a string in the register, an index past the end
  (its error), a string read out of a list or written into one.

### A map's fields, where they lie

A map a script writes as a record has a **shape** in cig's runtime: a
number every map has which got the same keys in the same order. Maps
of one shape have the same keys in the same places, and the runtime
keeps, for each field instruction, the shape of the last map it found
its field in and the field's place there, in a cell machine code can
read (`ctx.sites`, one cell a pc).

- **It asks the runtime once** (`fields(ctx, reg)`) where the map's
  values lie, how far each is from the last, and which shape the map
  has, and keeps that (a view, as of a list) in machine registers.
- **A read** compares the map's shape with the one in the
  instruction's cell. When they are the same, the value is at the
  cell's place: it copies the tag and the payload of a value that owns
  no memory to the register. No hash, no key looked at.
- **A write** takes the same, a value that owns no memory, and a field
  that holds one that owns none, and stores where the field's value
  lies.
- **Anything else is the runtime's:** a map of another shape or of
  none (the runtime finds the field, and the cell learns the shape), a
  field that holds a string, a field the map does not have yet.

### The runtime, for one instruction

What has no machine code, or met operands its machine code does not
cover, used to be the interpreter's alone: `again` for one instruction
(a failed guess, counted against the body) or a stretch of them
(`run`). Most of it the runtime can run where the frame stands,
without its loop: `quick(ctx, pc)`.

- **The fall back of every form that tests its operands** asks `quick`
  first, and the interpreter only when `quick` did nothing. A string
  where a number was guessed, a value that owns memory, a map of
  another shape are no failed guesses any more: each costs a call, and
  the body is not given up for them. A form whose types are proven
  does not ask: what it does not cover (an overflow, a zero divisor)
  the runtime's fast path does not either.
- **An instruction with no inline form** is one `quick` call where
  the runtime has it: an upvalue read, a constant that is no number, a
  string built, a list or a map made or added to, a builtin or a
  method that calls no function of the script. The plan weighs that
  against a stretch.
- **The body writes what it holds to the frame first** (the runtime
  reads the instruction's operands there) and reads back what the
  instruction wrote. `quick` never runs what jumps, counts a step or
  ends the frame: those are the body's own, or the interpreter's.
- **`math.sqrt` of a number not below zero is one machine
  instruction.** It is the one builtin with a form of its own.

### Where machine code pays (`src/plan.rs`)

Machine code beats the interpreter by a couple of nanoseconds an
instruction. Crossing between them costs several times that: a
stretch is a call into the runtime, and leaving a body and entering
it again is two trips through the VM's loop. A loop that reads a
list, calls a function and adds two numbers is faster left to the
interpreter whole; a loop of arithmetic is faster native whole.

So before anything is emitted, each loop is settled, innermost first:
its own instructions are a ring, each one native, part of a stretch
or left to the VM, and the cheapest answers for the whole ring are
found exactly. "All of it is the interpreter's" is always a
candidate and costs what the bytecode costs, so by its own figures a
plan is never slower than stage `b`.

Then a function's own code outside its loops is settled the same way,
as a line from the call to the return with the loops as items: it
starts and ends in the interpreter, pays for entering where its first
native instruction is and for leaving after its last, and spreads
nothing over turns. A function is called any number of times, so its
straight code is worth weighing; the top level runs once, and only
its loops are. A function nothing of which pays gets no body.

What the search weighs, in tenths of a nanosecond, measured on one
x86-64 laptop and rounded:

| | cost |
|---|---:|
| an instruction as machine code | 3 |
| an instruction in the interpreter | 15 |
| a stretch (a call into the runtime and back) | 120 |
| a `call` carried in a stretch, beyond the stretch | 40 |
| leaving a body | 50 |
| entering a body | 250 |
| entering a body at its function's first instruction | 70 |
| a list's element read or written as machine code | 10 |
| the same in the interpreter | 30 |
| a map's field read or written as machine code | 8 |
| the same in the interpreter | 24 |
| asking the runtime where a list's elements or a map's values lie | 100 |
| a list's `push` through the runtime's own function | 40 |
| the same as the interpreter's method call | 80 |
| one instruction run by the runtime where the frame stands, beyond the instruction itself: a kind it has a path of its own for | 5 |
| the same for any other kind | 20 |

A loop is taken to make ten turns each time it is reached: what
entering and leaving it are spread over.

**The figures are of one runtime.** An instruction in the interpreter
stood at 21 until the VM's fast loop read each instruction's own word
and its calls stopped leaving the loop: the interpreter then ran the
compute programs in 0.85 of the time it had taken, and a plan made
with the old figure gave bodies to functions the bytecode now runs
faster (`trees`, whose work is calls, took 0.85 where its bytecode
took 0.77). They were fit again by sweeps over the 21 programs.

Entering at a function's first instruction is the cheap way in: the
frame is new, no register is held in a machine register yet, and
nothing is read back from it. Anywhere else the body reads back, and
checks the type of, every register it holds there.

An inline form whose operands the type pass proved is always taken.
One that tests its operands when it runs is a **guess**: it is
counted as failing a share of the time (3% for a plain write, 10%
for arithmetic on values of unknown type or a list read by a proven
int, 30% for a move), each failure four tenths of a call: a guess
that keeps failing is given up by the runtime and costs nothing
more, so over a run it costs less than its rate. A guess next to
instructions the interpreter runs anyway joins their stretch, where a
failure costs nothing.

**The item of a `for` loop is read from the loop's own instructions.**
An inline `iternext` copies an item that owns no memory, and a loop
over strings or maps would fail that test every turn. So the
instructions from the `iternext` to the jump back are read: an item
that is the object of a field or an index, the receiver of a method,
or called, owns memory for certain, and its `iternext` is the
interpreter's; one that is multiplied, subtracted or ordered is taken
for a number (failing 5% of the time), and any other fails 20%.

**The estimates lean towards a guess holding**, because the runtime
bounds what it costs when it does not: a body whose guesses keep
failing is no longer entered there, and the bytecode has the loop
back within microseconds. So a loop is planned for the values its
instructions usually see (numbers in a list read by an int), and a
loop that sees others (strings in it) runs as it would have with no
body.

**A list in view is part of the search.** An index form is native
only where the list's elements are in view, and bringing them into
view costs one call. In a loop that is native whole the view outlives
the turn, so it costs nothing a turn; after a stretch it is asked for
again, so one read between two instructions the interpreter runs
joins their stretch, and eight reads after one share one view.

**A call is weighed like anything else.** Left to the VM's loop it
costs the body's way out and its way in again; carried in a stretch
it costs the stretch (which the instruction that loads the callee
often shares) and a little more. A loop with enough arithmetic
around a call stays native around it; a loop of calls and nothing
else is the interpreter's whole, and a stretch of the loop around it
can hold it, calls and all.

A body's **resume points** are the native instructions the
interpreter can arrive at: a function's first instruction, the start
of a native loop, the instruction after a stretch or a call.

### The rules

- `Rule::None`: no body.
- `Rule::Loops`: what a build makes. A body for each function with a
  loop or straight code that pays, entered there.
- `Rule::Every`: for the tests. A body for every function, with
  every instruction that has an inline form native, every call
  carried and every resume point the bytecode has, so every form and
  every way in runs on every script. `Mode::Frame` keeps every
  register in the frame there, and `Mode::Checked` also compares each
  type the pass proved with the register's real tag at every
  instruction that uses it (how the pass is held to the VM) and
  leaves every call to the VM's own loop, so the two builds the
  differential suite makes of a script make its calls both ways.

## The same input gives the same bytes

On any host, for either machine:

- specializations are compiled in the order of (module index, row);
  every typed-entry body is laid out before every entry;
- bodies are compiled in the module's pre-order (0 is the top level);
- sites are numbered through the section in the order they are met;
- the crate holds no unordered map, reads no clock and detects no host
  CPU: Cranelift is given the target's baseline and nothing else;
- Cranelift is pinned to one exact version.

That is what lets `cigcompile verify` compile a program's script again
and hold its native code to the result, byte for byte.

## Tests

`cargo test -p cig-codegen`:

- `tests/generate.rs` (the typed entries): the same bytes on two
  compiles and the same entries and sites for both machines; what is
  compiled, and the reason for each thing that is not; the frame bound;
  and, on Linux, the generated code run through cig-native's loader:
  results, every way of leaving with its site and operands, steps and
  depth counted mark by mark.
- `src/live.rs` (which registers are read again, and what an
  instruction can leave in one): a register's life from its write to
  its last read, across a loop, through a closure's captures and past
  a call; every register alive in a function with a handler; each
  operator's rule, with and without a proven operand.
- `src/body.rs` (a loop's scratch registers): the one found, the one
  the loop reads first, the one a sum of two unknowns is written to,
  two loops one inside the other, a call in the loop, a handler.
- `src/plan.rs` (the plan): a loop of proven arithmetic is native
  whole, a loop the interpreter runs anyway is left to it whole, a
  guess joins the stretch beside it, a call among little work is left
  with its loop and among much is carried (with what loads its
  callee, in one stretch), a loop of calls alone is one stretch of
  the loop around it, a native loop draws its outer loop in; a
  function's straight code is native when there is enough of it, is
  entered at its first native instruction, and carries its calls; a
  loop that writes a list and counts is native whole with the list in
  view, an index after a stretch pays for its view each turn, and a
  loop native whole keeps its outer loop's lists in view.
- `tests/bodies.rs` (the bodies): which functions get one and where
  it is entered under each rule; the same bytes on two compiles and a
  fingerprint for each machine; and, on Linux, bodies mapped and
  entered on real frames against a recording host: a loop that runs
  to its end in machine code in each mode, one entered part way, the
  step budget, an overflow, operands an inline form does not cover, a
  stretch, a call carried in one a turn and left in the checked
  build, an int divided by an int over its edges, a stop the body
  hands back, and a register that is not of its proven type; a
  function's straight code entered at its start; a value that owns
  memory moved by the runtime, a register that holds one cleared
  before a number is stored (and not before the instruction can still
  fail), a loop's items read where they lie, a `push` through the
  runtime; a list of numbers read where it lies and asked for
  once, an element machine code may not copy, an index with no
  element, a register that holds no list, and a list written in place
  until a string is in the way; a loop's scratch registers freed once
  where it is entered and never again in its turns, nothing asked
  when none owns memory, a register the runtime cannot free, a body
  entered inside the loop, and a frame no run makes.

Whole programs are held to the tree-walker by the differential suite
at the workspace root, which builds each one at stage `c`, and, with
`DIFF_BODIES`, again with a body for every function.
