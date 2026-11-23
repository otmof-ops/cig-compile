<!-- SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile) -->
<!-- SPDX-License-Identifier: CC-BY-4.0 -->

# 0007: Where the pieces of stage C and stage D live

- Status: accepted, 2026-11-23; amended the same day (Cranelift's patch release, the one `unsafe` call outside the crate)
- Issues: #32 (the code generator as a crate of its own), #34, #1 (the crate split); otmof-ops/cig-script#256 (cig-cherry)
- Research: [study 01](../research/studies/01-cranelift-ir-and-mid-end.md) and [study 02](../research/studies/02-cranelift-back-end-and-regalloc2.md) (Cranelift)
- Related: [0005](0005-what-goes-native-first.md), [0006](0006-native-code-in-a-program.md)

## The question

Three new pieces exist after [0005](0005-what-goes-native-first.md) and [0006](0006-native-code-in-a-program.md): type inference over CIR, a code generator, and a loader. Two programs will use them: cigcompile now, and cig-cherry later, which compiles while a script runs. `cig` itself must stay small and must never carry a compiler (otmof-ops/cig-script#256: "cig carries only a loader"). **Which repository and which crate holds each piece?**

## The decision

| piece | where | why there |
|---|---|---|
| **Type inference and specializations** | cig-script, `cigscript::lower::cir::infer` | It is an analysis of CIR and CIR lives there. The lowering to bytecode can use the same types later (typed instructions), so the VM, stage C and cig-cherry read one answer. |
| **The native section's format, the interface between native code and the VM, and the loader** | cig-script, a crate of its own: `crates/cig-native` | The VM reads the section and calls the code, so the runtime needs it; the generator writes the section, so the compiler needs it too. One crate holds the layout both sides agree on. It holds every `unsafe` block of this work but one: the runtime's single call that makes a mapping callable, where the program answers for its own bytes ([0006](0006-native-code-in-a-program.md), section 7). It depends on nothing but the system's C library. |
| **The code generator** | cig-compile, a crate of its own: `crates/cig-codegen` | It is the compiler. It takes a CIR specialization and gives machine code and the tables that go in the section. It depends on Cranelift; nothing in cig-script depends on it, so `cig` never links a compiler. |
| **cig-cherry**, when it is built | its own executable, using `cig-codegen` and `cig-native` | #32's comment asks for one generator and one interface for both; this is that. |

**`cig-codegen` is a library with one job:** specialization in, code out. It knows nothing of programs, payloads, files or the command line; cigcompile's `build` calls it and writes the section. That is what lets a second caller drive it per function.

**Cranelift is pinned exactly:** `cranelift-codegen` and `cranelift-frontend` `=0.137.3` (Wasmtime 50.0.3), the latest patch release of the minor the study read (0.137.1, Wasmtime 50.0.1), Apache-2.0 WITH LLVM-exception. Both back ends are always built in, and no feature of the machine cigcompile runs on is ever used: a program is compiled for its target's baseline. A code generator's output is part of a program's bytes, and a program's bytes are reproducible: a new Cranelift is a deliberate change that rebuilds the expected outputs, like a new cig. Its minimum Rust is 1.97, so cigcompile's rises from 1.82 to 1.97; the toolchain this repository pins is 1.99.1 already.

**What cigcompile's size is allowed to do.** The compiler grows by Cranelift. The runtime does not grow by it at all: `cig-native` is a few hundred lines. Programs grow only by their own native code.

## What is not decided here

- Whether CIR itself becomes a chain of smaller languages ([study 16](../research/studies/16-ir-shapes-nanopass-zig-sea-of-nodes.md), entry 16). The generator's input is a typed specialization of CIR as it is; a lower IR, when there is one, sits between them without moving a crate.
- A second back end. [Study 12](../research/studies/12-v8-and-javascriptcore-tiers.md), entry 16, argues none is needed.
