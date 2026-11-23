# What the cig 1.25.0 runtime is made of

A program cigcompile makes is the `cig` runtime with a payload appended, 17.0 MB for hello on Linux. cig-compile#36 wants hello under 2 MB, and #4 and #1 plan a runtime that is only what a program uses. This is what there is to cut from: the code of the released binary, by crate and by module.

## What was measured

- **The binary:** `cig-v1.25.0-linux-x86_64`, the release asset, 17,016,616 bytes, sha256 `423d648340fd0db8fd541319f5dec3f1adf0329a387ebdf5d98c064232a90e46` (the published checksum). It is static and keeps its symbol names; only the line tables are stripped.
- **How:** [size-by-crate.py](size-by-crate.py) reads the symbol table with `nm` and the section sizes with `size`, and adds up the size of every code symbol by the crate its name starts with. The symbol table names 99.3% of `.text`.
- **The limit of the method:** a symbol is counted where it is named. Generic code from `core`, `alloc` or `hashbrown` that was made for a cig type counts for `core`, `alloc` or `hashbrown`, so `cigscript`'s own share is a floor and `core`'s is a ceiling.

The tables are in [report.md](report.md).

## What it shows

1. **Code is 12.3 MB of the 17.0 MB.** `.text` 12,321,862 bytes; then read-only data 980,556, unwinding tables 724,576 and 439,852, relocations 316,632. The symbol table and its strings are 1,859,692 bytes, nearly all the rest: the names a crash report prints cost 1.9 MB of every program.
2. **The engines are small.** The VM is 114,394 bytes of code and the tree-walker 109,454; the parser 96,979, the lowering 295,935, the whole standard library 522,360. **Both engines, the parser, the lowering and the standard library together are 1.14 MB.**
3. **The command line is the largest part of cig's own code:** `cigscript::cli` is 2,003,764 bytes, 40.1% of `cigscript`'s 4,997,274. The kernel (`burn`) is 1,282,285.
4. **The keyring costs about 1.4 MB.** `zbus` 719,032, `zvariant` 407,684, `secret_service` 205,338 and `keyring` 61,043 are there to ask the desktop's keyring for one key. A run with `CIG_SEAL_KEY=file` never calls them.
5. **zstd's C code is 821,181 bytes,** the largest dependency after the Rust core library.
6. **Regular expressions are about 617,000 bytes** (`regex_automata`, `regex_syntax`, `aho_corasick`), carried by every program whether or not its script uses `text`.

## What it says to the issues

- **cig-compile#36, #4, #1:** a compiled program needs the engine it runs on, the kernel, and the builtins its script names. It does not need `cli` (2.0 MB), the lowering (0.3 MB; a stage `b` program carries its module), or, when its seal key is a file, the keyring (1.4 MB). Those three are 3.7 MB of 12.3 MB of code before any builtin is dropped.
- **What a script can be proven not to use** decides the rest: the standard library is 0.5 MB in all, so dropping unused builtins saves less than dropping the regular-expression engine (0.6 MB) or zstd (0.8 MB) when the script cannot reach them.

## Running it again

```sh
python3 size-by-crate.py <a cig binary that keeps its symbol names> > report.md
```
