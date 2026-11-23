# How a dynamic value is laid out, across twelve implementations

- **Serves.** cig-compile#35 (values and memory), #32 (stage C and the value ABI); decisions (a), (g), (h) where a header or a shape pointer is part of the layout; cig-script#243 (VM study list) and cig-script#256 (cig-cherry, whose native code uses the same value ABI stage C fixes).
- **Pins.** Lua `v5.4.9` at `312b9ef` (MIT): `lobject.h`, `lvm.c`.
  LuaJIT `v2.1` at `63c8f1a` (MIT): `src/lj_obj.h`.
  V8 at `15.8.62.31` commit `b601a23` (BSD-3-Clause): `include/v8-internal.h`, `src/objects/`.
  SpiderMonkey from `firefox FIREFOX_160_0_1_RELEASE` at `58fed86` (MPL-2.0): `js/public/Value.h`.
  WebKit `wpewebkit-2.54.3` at `c01fbd9` (BSD-2-Clause for the `runtime/` files quoted): `Source/JavaScriptCore/runtime/JSCJSValue.h`.
  QuickJS `quickjs-ng-v0.18.0` at `d7215b0` (MIT): `quickjs.h`.
  Wren `0.4.0` at `4a18fc4` (MIT): `src/vm/wren_value.h`.
  Ruby `v3_4_12` at `366f5fd` (Ruby or BSD-2-Clause): `include/ruby/internal/`.
  CPython `v3.15.2` at `5e58eaa` (PSF-2.0): `Include/object.h`, `Include/refcount.h`, `Include/internal/pycore_stackref.h`, `Include/cpython/longintrepr.h`.
  Lean 4 `v4.35.0` at `eb6cafb` (Apache-2.0): `src/include/lean/lean.h`.
  Koka `v3.2.9` at `facb793` (Apache-2.0): `kklib/include/kklib.h`, `kklib/include/kklib/box.h`.
  Go `go1.27.4` at `7b28810` (BSD-3-Clause): `src/runtime/runtime2.go`, `src/runtime/iface.go`.
  OCaml `5.5.2` at `c7b6787` (LGPL-2.1 with linking exception): `runtime/caml/mlvalues.h`.
- **License rule.** Ideas freely for Lua, LuaJIT, V8, JavaScriptCore (the BSD-2 `runtime/` headers quoted here), QuickJS, Wren, Ruby, CPython, Lean, Koka and Go.
  SpiderMonkey (MPL-2.0): ideas freely, reimplement, never adapt a file.
  OCaml (LGPL): ideas only; file names for pointing, no code structure to follow.
- **Read with.** The cig tree's own notes: `cig-script/docs/research/lua.md` entry 5 (a 16-byte `TValue`, and why `Rc<str>` leaves cig at 24), `cig-script/docs/research/luajit.md` A5 (NaN-tagging rejected: needs `unsafe`), `cig-compile/docs/decisions/0001-backend.md` (stage A to D, and the ahead-of-time choice).
  The census of what cig's VM executes beside this study: `data/census/README.md` and `data/census/report.md` (operand types at arithmetic sites, moves of counted values, shapes at field sites).

## What cig should carry away

- **A tagged 16 is a safe-Rust value; a NaN-boxed 8 is not.** Every pointer-tagging and NaN-boxing scheme in the library (V8 Smi, Ruby fixnum/flonum, OCaml, Lean, Koka, LuaJIT, SpiderMonkey, JavaScriptCore, Wren, QuickJS on 32-bit) is built on `union`s and masked pointer arithmetic that Rust does not permit under `#![forbid(unsafe_code)]`.
  A two-word enum whose payloads are each one word compiles in safe Rust and is the shape Lua's `TValue` has in C (entry 1).
  cig's rule 9 confines `unsafe` to one audited crate; a NaN-boxed 8 fits only if it is built there (entries 5, 14, 15).
- **A full `i64` fits only in the tagged-16 layout.** V8's Smi is 32 bits on 64-bit builds (entry 9); JavaScriptCore's int is 32 bits (entry 2); SpiderMonkey's `JSVAL_INT_BITS` is 32; Ruby's fixnum is 63 bits because one bit is a tag; OCaml's is 63 bits for the same reason.
  An `i64` past the small-int range becomes a heap number or a bignum in every one of them (entry 10).
  cig's `Int` is a full `i64` with overflow raising `E503` (`cig-script/src/vm/mod.rs: num_add` and the `checked_add` beside it): a slow path for large ints would carry correctness risk in every arithmetic op.
  This leans the decision to tagged-16, as issue #35 itself states (entry 15).
- **Shapes live beside the value, not in it.** V8, SpiderMonkey and JSC each carry the shape pointer in the object header (`heap-number.h` and its neighbors; JSC's `Structure`); the value word is for the payload.
  For cig, that means (h)'s shapes are a word on the map object, not a slot in the value itself (entry 11).
  The census found all 74 `getfield`/`setfield` sites in the compute programs saw one shape (`data/census/report.md`), so one shape per site at the inline-cache level is the design point; two shapes handle the cigo case.
- **Where the count lives decides what a move touches.** Lua's `TValue` is 16 bytes and a copy is two stores (`setobj`); the collector, not the count, keeps objects alive.
  cig counts references instead (rule 6), so a move of a counted value is two stores plus an atomic-free `inc` on the header (entry 4; the census median of 50% of moves on counted values gives the floor); a tagged-16 does not change that cost, a NaN-boxed 8 slightly reduces the move.
  The count sits in the object header in every implementation that uses it (CPython, Lean, Koka, cig; entry 11); `#35`'s "cycle collector" has to see values in native frames of stage C and cig-cherry (`issues3/cig-compile-35.newcomments.md`), which is a stack-map question, not a value-layout one.
- **Fit NaN-boxing to one case, not the whole value.** QuickJS ships both layouts side by side in `quickjs.h`, choosing at build time behind one set of macros (entry 15).
  Wren does the same with `WREN_NAN_TAGGING`.
  That pattern is cig's way to measure a NaN-boxed 8: one unsafe crate exposes the accessors; the safe-Rust fallback is the tagged-16; the prototype #35 names compares the two against today's 24, as the `Measurements owed` section lays out.
- **The order of a type test is the same in every fast implementation.** LuaJIT tests the integer tag first for arithmetic (`cig-script/docs/research/luajit.md` A5) and the container type first for a field or index.
  Lua 5.4's `op_arith_aux` tests `ttisinteger(v1) && ttisinteger(v2)` and falls to float; QuickJS's `JS_VALUE_IS_BOTH_INT` is one word compare (`(tag1 | tag2) == 0`).
  cig's `num_add`, `num_sub`, `num_mul` match on `(Value::Int, Value::Int)` first already (`cig-script/src/vm/mod.rs`); the shape of a 16-byte tagged value does not change that (entry 5).

## Design points

### 1. A tag word beside a payload word: Lua's 16-byte `TValue`, QuickJS on 64-bit, Go's `eface`, and cig's planned 16

- **What it does, and why.** Lua 5.4's `TValue` is a `union Value` of 8 bytes (`struct GCObject *gc`, `void *p`, `lua_Integer i`, `lua_Number n`) followed by one `lu_byte tt_` (`lua-v5.4.9/lobject.h`, lines 49 to 69).
  The union holds a double, a 64-bit integer, or a pointer to a collectable object; the tag byte says which, with bits 0 to 3 for the type and bits 4 to 5 for a variant (`makevariant`, `novariant`, `ttypetag`).
  The whole thing is 16 bytes after C's alignment.
  A copy is two stores (`setobj`, lines 118 to 128).
  QuickJS without NaN-boxing uses the same shape: `JSValue` is `{ JSValueUnion u; int64_t tag; }` with `u` a union of `uint64_t`, `double`, `void *`, `int32_t` (`quickjs-ng-v0.18.0/quickjs.h`, lines 316 to 329).
  Go's interface word is a two-word struct too: `eface` is `{ *_type; unsafe.Pointer }`, `iface` is `{ *itab; unsafe.Pointer }` (`go-go1.27.4/src/runtime/runtime2.go`, lines 184 to 192).
  cig today is 24 bytes because `Rc<str>` is a fat pointer; a `Str(Rc<StrObj>)` variant brings the enum to 16 (cig-script/docs/research/lua.md, entry 5).
- **For cig: adapt.** Decided by rule 9 (`forbid(unsafe_code)` outside one crate): a two-word enum whose payloads are each one word is the shape safe Rust compiles to 16 bytes, and cig's current `Value` is that shape once `Str` holds an `Rc<StrObj>`.
  cig's form: the enum in `cig-script/src/value.rs`, narrowed as `cig-script/docs/research/lua.md` entry 5 names; the VM's `num_add` and its neighbors (`cig-script/src/vm/mod.rs`) read the same tag word.
- **What would decide it.** Decided by rule 9 (cig's rules above: no `unsafe` outside one audited crate).
- **Feeds.** cig-compile#35, #32; decisions (a), (e).
- **Read.** `lua-v5.4.9/lobject.h` (`Value`, `TValue`, `TValuefields`, `setobj`, `rawtt`); `quickjs-ng-v0.18.0/quickjs.h` (`JSValue`, `JSValueUnion`, `JS_MKVAL`, `JS_MKPTR`, `JS_VALUE_GET_TAG`); `go-go1.27.4/src/runtime/runtime2.go` (`eface`, `iface`, `funcval`); `cig-script/src/value.rs` (`Value`); `cig-script/docs/research/lua.md`.

### 2. One word, with tag bits in a pointer: V8, Ruby, OCaml, Lean, Koka, CPython's stack references

- **What it does, and why.** Every one of these keeps the whole value in one machine word by stealing low bits of a pointer.
  V8's `kHeapObjectTag = 1` and `kSmiTag = 0` sit in the lowest bits, with `kHeapObjectTagSize = 2` (`v8-15.8.62.31/include/v8-internal.h`, lines 55 to 75); on a 64-bit build with pointer compression `kApiTaggedSize = 4` and `SmiTagging<4>` sets `kSmiShiftSize = 0`, `kSmiValueSize = 31`; on a 64-bit build without pointer compression `SmiTagging<8>` sets `kSmiShiftSize = 31`, `kSmiValueSize = 32`.
  Ruby's `VALUE` is `uintptr_t` (`ruby-v3_4_12/include/ruby/internal/value.h`), and `special_consts.h` sets `RUBY_IMMEDIATE_MASK = 0x07`, `RUBY_FIXNUM_FLAG = 0x01` (odd pointer = 63-bit int), `RUBY_FLONUM_MASK = 0x03`, `RUBY_FLONUM_FLAG = 0x02` (a double held in the pointer word after a rotation; doubles near zero or infinity do not fit and become heap `RFloat`), `RUBY_SYMBOL_FLAG = 0x0c`, `Qnil = 0x04`, `Qtrue = 0x14`, `Qfalse = 0x00`.
  OCaml's `value` is `intnat` with `Is_long(x) = (x & 1)` and `Val_long(x) = (x << 1) + 1`, giving `Max_long = (1 << 62) - 1` (`ocaml-5.5.2/runtime/caml/mlvalues.h`, lines 59 to 90).
  Lean 4 says `lean_is_scalar(o) = ((size_t)o & 1) == 1`, with `lean_box(n) = (n << 1) | 1` and `lean_unbox(o) = (size_t)o >> 1` (`lean4-v4.35.0/src/include/lean/lean.h`, lines 348 to 350).
  Koka's `kk_box_t`'s low bit is clear for pointers, set for values (`koka-v3.2.9/kklib/include/kklib.h`, lines 170 to 187; `kk_box_is_ptr`, `kk_box_is_value` in `kklib/include/kklib/box.h`, lines 80 to 95).
  CPython 3.15's interpreter stack carries `_PyStackRef`, a `union` whose normal build is `uintptr_t bits` and whose debug build (`Py_STACKREF_DEBUG` set and `Py_GIL_DISABLED` unset) is `uint64_t index` (`cpython-v3.15.2/Include/internal/pycore_structs.h` lines 66 to 72); the low two bits (mask `Py_TAG_BITS = 3`, shift `Py_TAGGED_SHIFT = 2`) are the tag, with three tag values defined: `Py_TAG_REFCNT = 1`, `Py_TAG_INVALID = 2`, `Py_INT_TAG = 3` (`cpython-v3.15.2/Include/internal/pycore_stackref.h` lines 51 to 58).
  What does not fit: a full 64-bit integer (every scheme loses one or more bits to the tag), a double in most of these (Ruby's flonum is the exception, and some doubles still do not fit), and a pointer whose alignment is less than the tag space.
- **For cig: reject.** Decided by rule 9: every one of these encodings depends on `unsafe` in Rust: casting a `usize` back to `*T` is not safe, and masking the low bit of a pointer breaks `#[repr(Rust)]` guarantees.
  cig's form: no stolen-tag-bit layout exists; the equivalent machinery is the enum discriminant, read by the `match` in `src/vm/mod.rs: num_add`.
- **What would decide it.** Decided by rule 9.
- **Feeds.** cig-compile#35, cig-script#243; decision (a).
- **Read.** `v8-15.8.62.31/include/v8-internal.h` (`kHeapObjectTag`, `kSmiTag`, `kSmiTagSize`, `SmiTagging`, `kApiTaggedSize`); `ruby-v3_4_12/include/ruby/internal/special_consts.h` (`RUBY_IMMEDIATE_MASK`, `RUBY_FIXNUM_FLAG`, `RUBY_FLONUM_MASK`, `RUBY_FLONUM_FLAG`, `RUBY_SYMBOL_FLAG`, `RB_FIXNUM_P`, `RB_FLONUM_P`); `ocaml-5.5.2/runtime/caml/mlvalues.h` (`value`, `Is_long`, `Val_long`, `Max_long`); `lean4-v4.35.0/src/include/lean/lean.h` (`lean_is_scalar`, `lean_box`, `lean_unbox`); `koka-v3.2.9/kklib/include/kklib/box.h` (`kk_box_is_ptr`, `kk_box_is_value`, `kk_intf_box`, `kk_ptr_box`); `cpython-v3.15.2/Include/internal/pycore_structs.h` (`_PyStackRef`, `uintptr_t`, `bits`, `index`); `cpython-v3.15.2/Include/internal/pycore_stackref.h` (`Py_INT_TAG`, `Py_TAG_REFCNT`, `Py_TAG_INVALID`, `Py_TAG_BITS`, `Py_TAGGED_SHIFT`, `PyStackRef_IsTaggedInt`).

### 3. One word that is a double unless it is a NaN: LuaJIT, SpiderMonkey, JavaScriptCore, Wren, QuickJS on 32-bit

- **What it does, and why.** A 64-bit IEEE 754 double has 2^52 bit patterns that mean NaN; only one is needed, so the others carry tag and payload.
  LuaJIT's `TValue` is 8 bytes; `cig-script/docs/research/luajit.md` A5 gives the details at the `v2.1` pin.
  SpiderMonkey has two: `JS_NUNBOX32` keeps the tag in the high word (`JSVAL_TAG_SHIFT = 32`), `JS_PUNBOX64` packs 47 bits of payload beside a 5-bit tag in the top 17 bits of the fraction field (`JSVAL_TAG_SHIFT = 47`, `JSVAL_TAG_MAX_DOUBLE = 0x1FFF0`, `ValueIsDouble(bits) = bits <= JSVAL_SHIFTED_TAG_MAX_DOUBLE`, `ValueTagMask = 0xFFFF'8000'0000'0000`, `ValueGCThingPayloadMask = 0x0000'7FFF'FFFF'FFFF`; `js/public/Value.h` lines 29 to 300).
  The pointers come from an allocator that uses only 47 bits (`js::gc::MapAlignedPagesRandom`, named in the header comment).
  JavaScriptCore inverts the scheme: doubles are offset by `DoubleEncodeOffset = 2^49` before storage, so after the offset every valid double has one of the top 15 hex patterns `0x0002` to `0xFFFC`; pointers sit at `0x0000:PPPP:...`, integers at `0xFFFE:0000:IIII:IIII` (`webkit-wpewebkit-2.54.3/Source/JavaScriptCore/runtime/JSCJSValue.h`, lines 400 to 500; `NumberTag = 0xfffe0000...`, `OtherTag = 0x2`, `BoolTag = 0x4`, `UndefinedTag = 0x8`, `NotCellMask = NumberTag | OtherTag`).
  Wren encodes a singleton with `QNAN | TAG_*` (`wren-0.4.0/src/vm/wren_value.h` lines 588 to 591: `NULL_VAL`, `FALSE_VAL`, `TRUE_VAL`, `UNDEFINED_VAL`), and an object pointer with `SIGN_BIT | QNAN | PTR` (line 562: `IS_OBJ(value) = (value & (QNAN | SIGN_BIT)) == (QNAN | SIGN_BIT)`); the sign bit is what tells a singleton from an object (`QNAN = 0x7ffc000000000000`, `MASK_TAG = 7`, `IS_NUM(value) = (value & QNAN) != QNAN`).
  QuickJS enables NaN-boxing on 32-bit with `JS_NAN_BOXING = 1`: `JSValue` is a `uint64_t`, tags are above `JS_FLOAT64_TAG_ADDEND = 0x7ff80000 - JS_TAG_FIRST + 1` (`quickjs-ng-v0.18.0/quickjs.h` lines 251 to 300).
  What does not fit: a full 64-bit integer in any of them, a double beside a tag bit (LuaJIT keeps its integer in the lower 32 bits of the NaN space, dual-number mode), and a pointer wider than 47 or 48 bits.
- **For cig: reject.** Decided by rule 9: the shape is a bare `u64` with 48 bits meaning "a pointer" in one branch and "a double's lower bits" in another, read through pointer arithmetic with no Rust type for it.
  It fits only in one `unsafe` crate.
  If the measurement against 24 bytes and against tagged-16 wins by 15% or more (the threshold in cig-compile#35), it is built there; today, no crate does that.
  cig's form: nothing exists; `cig-script/docs/research/luajit.md` A5 records the rejection as the reason, and takes the tag-test order instead.
- **What would decide it.** Not measured yet: a prototype NaN-boxed value in one audited crate, against today's 24 and against the tagged-16, on `nbody`, `mandel`, `fannkuch`, `trees` and `loop`.
  The threshold is #35's 15%.
- **Feeds.** cig-compile#35; decisions (a), (e).
- **Read.** `cig-script/docs/research/luajit.md` (A5); `luajit-v2.1/src/lj_obj.h`; `spidermonkey-FIREFOX_160_0_1_RELEASE/js/public/Value.h` (`JSValueType`, `JSVAL_TAG_SHIFT`, `JSVAL_TAG_MAX_DOUBLE`, `ValueTagMask`, `ValueGCThingPayloadMask`, `ValueIsDouble`); `webkit-wpewebkit-2.54.3/Source/JavaScriptCore/runtime/JSCJSValue.h` (`NumberTag`, `DoubleEncodeOffset`, `OtherTag`, `BoolTag`, `UndefinedTag`, `NotCellMask`, `ValueFalse`, `ValueNull`); `wren-0.4.0/src/vm/wren_value.h` (`WREN_NAN_TAGGING`, `SIGN_BIT`, `QNAN`, `MASK_TAG`, `IS_NUM`, `IS_OBJ`, `NULL_VAL`, `FALSE_VAL`, `TRUE_VAL`, `UNDEFINED_VAL`); `quickjs-ng-v0.18.0/quickjs.h` (`JS_NAN_BOXING`, `JS_FLOAT64_TAG_ADDEND`, `JS_TAG_IS_FLOAT64`, `JS_VALUE_IS_NAN`).

### 4. Testing "is it an integer": one byte compare, one mask-and-compare, one pair of mask-compares

- **What it does, and why.** Across the three families, an "is it an integer" test is one of three shapes.
  In the tag-beside-payload family: a load of the tag byte and a compare to `LUA_VNUMINT` (Lua, `lobject.h: ttisinteger(o) = checktag((o), LUA_VNUMINT)`), a compare to `0` (QuickJS 64-bit, `JS_VALUE_GET_TAG(v) == JS_TAG_INT`, where `JS_TAG_INT = 0`), or a `match` discriminant in cig's enum (one byte compare the compiler lowers to).
  In the pointer-tag family: a mask and a compare, two or three instructions (Ruby's `RB_FIXNUM_P(obj) = obj & RUBY_FIXNUM_FLAG`, OCaml's `Is_long(x) = ((x) & 1) != 0`, Lean's `lean_is_scalar(o) = ((size_t)o & 1) == 1`, V8's `(value & kSmiTagMask) == kSmiTag`).
  In the NaN-box family: a bound compare on the top bits (SpiderMonkey PUNBOX64: `ValueIsDouble(bits) = bits <= JSVAL_SHIFTED_TAG_MAX_DOUBLE`, then another compare for `JSVAL_TAG_INT32`; JSC: `(v & NumberTag) == NumberTag` then integer tag test).
  What they share: a single word is loaded, the test is one or two integer instructions, no branch is needed before the test itself.
  cig today runs a `match` on the enum (`cig-script/src/vm/mod.rs: num_add`), which the compiler lowers to a byte load, one compare and a conditional jump.
- **For cig: take.** Decided by rule 9 (the only shape it allows is the tag-and-payload one).
  cig's form: the `Value` enum's discriminant, read by the `match` arms in `cig-script/src/vm/mod.rs: num_add`, `num_sub`, `num_mul`, `num_div`.
  At a 16-byte tagged value the test is a byte load and a compare; the variant byte is the same byte Lua's `TValue` has.
- **What would decide it.** Decided by rule 9.
- **Feeds.** cig-compile#35, #32; decisions (a), (i).
- **Read.** `lua-v5.4.9/lobject.h` (`ttisinteger`, `checktag`, `LUA_VNUMINT`); `quickjs-ng-v0.18.0/quickjs.h` (`JS_VALUE_GET_TAG`, `JS_TAG_INT`); `ruby-v3_4_12/include/ruby/internal/special_consts.h` (`RB_FIXNUM_P`); `ocaml-5.5.2/runtime/caml/mlvalues.h` (`Is_long`); `lean4-v4.35.0/src/include/lean/lean.h` (`lean_is_scalar`); `v8-15.8.62.31/include/v8-internal.h` (`kSmiTag`, `kSmiTagMask`); `spidermonkey-FIREFOX_160_0_1_RELEASE/js/public/Value.h` (`ValueIsDouble`, `JSVAL_SHIFTED_TAG_MAX_DOUBLE`); `webkit-wpewebkit-2.54.3/Source/JavaScriptCore/runtime/JSCJSValue.h` (`NumberTag`, `NotCellMask`); `cig-script/src/vm/mod.rs` (`num_add`).

### 5. Testing "both integers" before an add

- **What it does, and why.** Every fast implementation fuses the two tests into one word operation.
  Lua 5.4's `op_arith_aux` reads `if (ttisinteger(v1) && ttisinteger(v2))`, two byte loads and two compares (`lua-v5.4.9/lvm.c` lines 962 to 968).
  QuickJS collapses them to one compare: `JS_VALUE_IS_BOTH_INT(v1, v2) = ((JS_VALUE_GET_TAG(v1) | JS_VALUE_GET_TAG(v2)) == 0)` (`quickjs-ng-v0.18.0/quickjs.h`, line 393), because `JS_TAG_INT = 0`.
  JavaScriptCore does the same trick on the one-word value with `NumberTag` masks.
  LuaJIT's handler for `BC_ADDVV` (`cig-script/docs/research/luajit.md` A5) checks the integer tag of each operand and native-integer-adds with overflow.
  cig's `num_add` matches on `(Value::Int, Value::Int)` first, a byte compare and a branch (`cig-script/src/vm/mod.rs`, around lines 3420 to 3430); `checked_add` carries the overflow check, raising `E503` on overflow as rule for cig's semantics.
  QuickJS's one-compare form is the tag-and-payload family's trick: cig can take it when the integer tag byte is `0`.
- **For cig: adapt.** Decided by rule 9 (safe Rust; the trick is a discriminant arithmetic fact, not pointer arithmetic).
  cig's form: order the enum so `Value::Int` is the first variant (discriminant 0), then the compiler's `match` over a pair of enums can test both tag bytes in one `or` and compare.
  The fast path is cig's existing `num_add` match (`cig-script/src/vm/mod.rs`): the change is the enum ordering.
- **What would decide it.** Not measured yet: the cost of the paired tag test on cig's `num_add`, before and after a fixed `Value::Int` discriminant, counted on `nbody`, `loop` and `fib` with the test machine's callgrind.
- **Feeds.** cig-compile#35, #32; decisions (a), (i).
- **Read.** `lua-v5.4.9/lvm.c` (`op_arith_aux`, `op_arith`, `op_arithK`); `quickjs-ng-v0.18.0/quickjs.h` (`JS_VALUE_IS_BOTH_INT`, `JS_VALUE_IS_BOTH_FLOAT`); `webkit-wpewebkit-2.54.3/Source/JavaScriptCore/runtime/JSCJSValue.h` (`NumberTag`); `cig-script/src/vm/mod.rs` (`num_add`, `num_sub`, `num_mul`); `cig-script/docs/research/luajit.md` (A5); `data/census/report.md`.

### 6. Getting a double out and putting one back

- **What it does, and why.** Tag-and-payload (Lua, QuickJS 64, cig): the double sits in the payload word, read as a load.
  SpiderMonkey PUNBOX64 (`js/public/Value.h` the whole SMDOC) and JSC (`JSCJSValue.h` `DoubleEncodeOffset = 2^49`) store the double offset by an integer so that no valid double collides with a tag; reading is a subtract of `DoubleEncodeOffset` then a bit-cast to `double`; writing is the reverse.
  SpiderMonkey canonicalizes NaNs on store (`JS::CanonicalizeNaN`, named in the SMDOC).
  LuaJIT (`cig-script/docs/research/luajit.md` A5) stores the double directly: the NaN space holds tags, every non-NaN double is itself.
  Wren tests `IS_NUM(value) = (value & QNAN) != QNAN`, three instructions (`wren-0.4.0/src/vm/wren_value.h` lines 558 to 560).
  CPython and Lean never put a double in the value word: a double is a heap object (CPython's `PyFloatObject`; Lean's scalar is unboxed only for integers, `lean4-v4.35.0/src/include/lean/lean.h`).
  What they cost: NaN-boxing saves the payload word but adds a subtract, an add, and a canonicalization on store; the tag-and-payload family does a load and a store.
- **For cig: adapt.** Decided by rule 9: in safe Rust, a `Value::Float(f64)` holds the double in the payload word, read by a `match` arm, with no bit tricks.
  cig's form: `Value::Float` in `src/value.rs`, read by `num_add`'s second match arm `(Value::Float(a), Value::Float(b)) => Some(Value::Float(a + b))`.
- **What would decide it.** Not measured yet: whether NaN-boxing's lower peak RSS on `sieve` and `nbody` outweighs the per-op offset arithmetic.
  #35 names the measurement at the 15% threshold.
- **Feeds.** cig-compile#35; decisions (a).
- **Read.** `webkit-wpewebkit-2.54.3/Source/JavaScriptCore/runtime/JSCJSValue.h` (`DoubleEncodeOffset`, `NumberTag`, `LowestOfHighBits`); `spidermonkey-FIREFOX_160_0_1_RELEASE/js/public/Value.h` (SMDOC, `ValueIsDouble`); `wren-0.4.0/src/vm/wren_value.h` (`IS_NUM`); `quickjs-ng-v0.18.0/quickjs.h` (`JS_FLOAT64_TAG_ADDEND`, `__JS_NewFloat64`); `cig-script/src/value.rs`; `cig-script/src/vm/mod.rs` (`num_add`).

### 7. Getting a pointer out

- **What it does, and why.** Lua's payload holds a `GCObject *` directly: `gcvalue(o) = val_(o).gc` with no mask (`lua-v5.4.9/lobject.h` line 305).
  QuickJS 64 does the same: `JS_VALUE_GET_PTR(v) = (v).u.ptr`.
  Go's `eface.data` and `iface.data` are already `unsafe.Pointer` fields.
  In the pointer-tag family: Ruby reads the pointer by `obj & ~RUBY_IMMEDIATE_MASK`, Lean by `(size_t)o` with the low bit cleared when it is a pointer; OCaml by the pointer itself since blocks are aligned and the integer tag lives in the low bit.
  V8 reads a heap object by subtracting `kHeapObjectTag = 1` (`v8-15.8.62.31/include/v8-internal.h`); pointer compression adds a base load first (`PointerCompressionIsEnabled`).
  JSC reads a cell by testing `NotCellMask` and using the whole value as the pointer (`JSCJSValue.h` lines 480 to 500; `ValueEmpty = 0x0`).
  SpiderMonkey PUNBOX64 reads a GC thing by `v & ValueGCThingPayloadMask = 0x0000'7FFF'FFFF'FFFF` (`js/public/Value.h`).
  What they share: in the tag-and-payload family the pointer is in a field and the load is free; in the pointer-tag and NaN-box families there is one mask or offset before the pointer is usable.
- **For cig: take.** Decided by rule 9: cig's `Rc<Function>`, `Rc<Builtin>` and the list and map handles are in the enum's payload word, read as a `match` arm.
  cig's form: `Value::Str(Rc<StrObj>)`, `Value::Func(Rc<Function>)`, `Value::List(Rc<RefCell<Vec<Value>>>)`, each read by its `match` arm in `src/vm/mod.rs`.
- **What would decide it.** Decided by rule 9.
- **Feeds.** cig-compile#35, #32; decision (a).
- **Read.** `lua-v5.4.9/lobject.h` (`gcvalue`, `val_`); `quickjs-ng-v0.18.0/quickjs.h` (`JS_VALUE_GET_PTR`); `go-go1.27.4/src/runtime/runtime2.go` (`eface`, `iface`); `v8-15.8.62.31/include/v8-internal.h` (`kHeapObjectTag`, `PointerCompressionIsEnabled`); `webkit-wpewebkit-2.54.3/Source/JavaScriptCore/runtime/JSCJSValue.h` (`NotCellMask`, `ValueEmpty`); `spidermonkey-FIREFOX_160_0_1_RELEASE/js/public/Value.h` (`ValueGCThingPayloadMask`); `cig-script/src/value.rs`.

### 8. Copying a value between registers, or to memory, and passing one to a function

- **What it does, and why.** A tag-and-payload value is two stores to memory, or two moves between registers.
  Lua's `setobj` is exactly two stores (`lua-v5.4.9/lobject.h` lines 118 to 128).
  QuickJS 64-bit copies a `JSValue` struct (two 8-byte fields).
  cig's `Value` today is 24 bytes and so copies three machine words (the fat pointer in `Rc<str>` is why; `cig-script/docs/research/lua.md` entry 5).
  On a 64-bit System V ABI a two-word struct is passed in two general registers (RDI/RSI on x86-64, X0/X1 on aarch64), and a three-word struct goes through memory.
  A NaN-boxed 8 passes in one register.
  A pointer-tag word passes in one register.
  Go passes an `eface` as two words (`unsafe.Pointer` plus `*_type`) through its own ABI.
  On copy, a counted value (`Rc`) needs a count increment on the pointer's target: an atomic-free add on the strong count's header word for cig's `Rc`.
  The census of cig's VM (`data/census/report.md`, finding 5) says the median compute program sees half of its `mov` instructions copying a counted value.
- **For cig: adapt.** Decided by rule 9 and rule 6 (reference counting is kept): the shape of the copy is "two stores plus an inc on the counted payload", and tagged-16 is the smallest safe-Rust form of it.
  cig's form: a 16-byte `Value` passed in two registers to `num_add` and its neighbors in `cig-script/src/vm/mod.rs`.
  Stage C's #32 ABI already says "tag + payload, two registers" and sits on this entry.
- **What would decide it.** Not measured yet: a prototype 16-byte `Value` against a 24-byte `Value` on `fib` and `loop`, counting callgrind machine instructions per call and per loop turn, and the compiler's pass-by-register behavior on the test machine.
- **Feeds.** cig-compile#35, #32; cig-script#243, #256; decisions (a), (e), (g).
- **Read.** `lua-v5.4.9/lobject.h` (`setobj`); `quickjs-ng-v0.18.0/quickjs.h` (`JSValue` struct); `go-go1.27.4/src/runtime/runtime2.go` (`eface`, `iface`); `cig-script/docs/research/lua.md` (entry 5); `cig-script/src/value.rs`; `cig-script/src/vm/mod.rs` (`num_add`); `data/census/report.md`.

### 9. Which layouts hold an `i64` unboxed, and which force a narrower integer or a heap number

- **What it does, and why.** A 64-bit tag-and-payload value has room for a full `i64` in the payload: Lua, QuickJS 64-bit and Go `eface` all do.
  cig's current enum holds `i64` in `Value::Int(i64)` (`cig-script/src/value.rs`).
  In the pointer-tag family one bit is lost to the tag, so the integer is 63 bits at best: OCaml's `Max_long = 2^62 - 1` (`mlvalues.h`), Ruby's fixnum (63 bits with one tag bit; integers past that become `RBignum`, `include/ruby/internal/core/rbignum.h`'s neighbors), Lean's `lean_box(n)` takes a `size_t` and shifts (`lean.h: lean_box`).
  V8 narrows further: on a 64-bit build without pointer compression the Smi is 32 bits (`SmiTagging<8>` sets `kSmiValueSize = 32`), and with pointer compression the Smi is 31 bits (`SmiTagging<4>` sets `kSmiValueSize = 31`) (`v8-15.8.62.31/include/v8-internal.h` lines 82 to 162).
  An integer past that goes to a `HeapNumber` (`v8-15.8.62.31/src/objects/heap-number.h`, `UnalignedDoubleMember value_`).
  JavaScriptCore keeps a 32-bit integer in the integer tag `0xFFFE:0000:IIII:IIII`; a larger integer is stored as a double, or as a `BigInt32` when `USE(BIGINT32)` holds (`JSCJSValue.h` lines 427 to 480; `BigInt32Tag = 0x12`).
  SpiderMonkey's `JSVAL_INT_BITS = 32` (`js/public/Value.h` line 155).
  In the NaN-box family: LuaJIT in dual-number mode stores a 32-bit integer (`cig-script/docs/research/luajit.md` A5, A7).
  cig's `Int` is a full `i64` with overflow raising `E503`: only the tag-and-payload family (and the current 24-byte enum) gives it unboxed.
- **For cig: adapt.** Decided by rule 9 and rule 2 (errors happen at the same place): the shape is a tagged-16 with a 64-bit integer payload; a narrower integer would change where `E503` is raised, or force a heap-bignum path that adds a correctness risk in every arithmetic operation, as cig-compile#35's own text says.
  cig's form: `Value::Int(i64)` in `src/value.rs`, carried into the 16-byte value as the `i64` payload beside the tag.
- **What would decide it.** Decided by rule 2 and the 15% threshold in #35: a NaN-boxed 8 would need a separate boxed-integer path for values past 48 bits, which is a correctness surface cig does not want.
  The threshold is not about correctness, but the correctness risk is what decides against it.
- **Feeds.** cig-compile#35, #32; decision (a).
- **Read.** `v8-15.8.62.31/include/v8-internal.h` (`SmiTagging`, `kSmiValueSize`); `v8-15.8.62.31/src/objects/smi.h` (`Smi`, `FromInt`); `v8-15.8.62.31/src/objects/heap-number.h` (`HeapNumber`); `webkit-wpewebkit-2.54.3/Source/JavaScriptCore/runtime/JSCJSValue.h` (integer tag at `0xFFFE`, `BigInt32Tag`); `spidermonkey-FIREFOX_160_0_1_RELEASE/js/public/Value.h` (`JSVAL_INT_BITS`); `ocaml-5.5.2/runtime/caml/mlvalues.h` (`Max_long`); `ruby-v3_4_12/include/ruby/internal/core/rbasic.h`; `lean4-v4.35.0/src/include/lean/lean.h` (`lean_box`); `cig-script/src/value.rs` (`Value::Int`); `cig-compile/docs/decisions/0001-backend.md`.

### 10. What each engine does when an integer does not fit

- **What it does, and why.** V8 allocates a `HeapNumber` with its own object header and an unaligned double field (`src/objects/heap-number.h`); every arithmetic op that would overflow a Smi allocates (or hits a cached `HeapNumber` pool, in the hot path).
  Ruby allocates a `RBignum`.
  SpiderMonkey writes a double to the value word (its integer is 32 bits, so `2^31` and larger live as doubles).
  JavaScriptCore does the same below `USE(BIGINT32)`, or encodes a `BigInt32` in the value word (`JSCJSValue.h`).
  Lua 5.4 holds a 64-bit integer unboxed beside its double variant (`LUA_VNUMINT` and `LUA_VNUMFLT`).
  cig does what Lua does: the integer stays an `i64` in the payload; `cig-script/src/vm/mod.rs: num_add` uses `checked_add` and the overflow falls out as `E503` at the instruction's span.
  On the native tier the same semantic belongs in Cranelift's `iadd_overflow` with a conditional raise, as cig-compile#32's own "Fast paths and guards" section names.
- **For cig: take.** Decided by rule 2 (errors happen at the same place and span as the tree-walker's) and rule 9.
  cig's form: `checked_add`/`checked_sub`/`checked_mul` in `cig-script/src/vm/mod.rs: num_add`, `num_sub`, `num_mul`, with the overflow turned into `E503` at the span the instruction holds.
  Stage C's inlined `iadd_overflow` (cig-compile#32, "Fast paths and guards") is the native form.
- **What would decide it.** Decided by rule 2.
- **Feeds.** cig-compile#35, #32; decisions (a), (i).
- **Read.** `v8-15.8.62.31/src/objects/heap-number.h` (`HeapNumber`, `value_`); `webkit-wpewebkit-2.54.3/Source/JavaScriptCore/runtime/JSCJSValue.h` (`BigInt32Tag`); `ruby-v3_4_12/include/ruby/internal/core/rbasic.h`; `lua-v5.4.9/lobject.h` (`LUA_VNUMINT`, `LUA_VNUMFLT`); `cig-script/src/vm/mod.rs` (`num_add`); `cig-compile/docs/decisions/0001-backend.md`.

### 11. Where the reference count and the type of an object live

- **What it does, and why.** In every system that counts references, the count is a word in the object's header, not in the value.
  CPython: `struct _object` opens with `ob_refcnt` (a `Py_ssize_t`, or on free-threaded builds a split 32-bit local count and a 32-bit thread id) and `ob_type` (a `PyTypeObject *`); `cpython-v3.15.2/Include/object.h` lines 127 to 170.
  Lean: `lean_object` is `{ int m_rc; unsigned m_cs_sz:16; unsigned m_other:8; unsigned m_tag:8 }`, 8 bytes; the count is first (`lean4-v4.35.0/src/include/lean/lean.h` lines 160 to 166).
  Koka: `kk_header_t` is `{ uint8_t scan_fsize; uint8_t _field_idx; uint16_t tag; _Atomic(kk_refcount_t) refcount }`, 8 bytes; the refcount is last (`koka-v3.2.9/kklib/include/kklib.h` lines 134 to 141).
  Ruby's `RBasic` is two `VALUE` words: `flags` (which includes a type class and a frozen bit) and a const `klass` pointer (`ruby-v3_4_12/include/ruby/internal/core/rbasic.h` lines 59 to 108); the count sits elsewhere in the object.
  cig: `Rc<T>` carries strong and weak counts in a heap control block preceding `T`, per `std::rc::Rc`'s own layout; `cig-script/src/value.rs` holds `Rc<Function>`, `Rc<Builtin>`, `Rc<Module>`, `Rc<Chain>` and `Rc<str>`, `Rc<RefCell<Vec<Value>>>`, `Rc<RefCell<IndexMap<String, Value>>>`.
  Shapes in V8 and SpiderMonkey are a `Map` or `Shape` pointer in the object's header (the issue cig-compile#35 names this: "a shape compare and a load").
  What they share: the value holds a pointer; the object holds the count, the type, and (for V8 and SpiderMonkey) the shape.
- **For cig: take.** Decided by rule 6 (deterministic cleanup: reference counting, no tracing collector).
  cig's form: `Rc<T>` in `cig-script/src/value.rs` provides the control block; the shape work (h) adds a `Shape` pointer to the map object, not to the value; (g)'s elision is a CIR pass that cancels `Rc::clone` and `drop` pairs.
  The object's type is already its enum variant for primitives and its Rust type for the pointee.
- **What would decide it.** Decided by rule 6.
- **Feeds.** cig-compile#35; decisions (g), (h).
- **Read.** `cpython-v3.15.2/Include/object.h` (`struct _object`, `ob_refcnt`, `ob_type`); `lean4-v4.35.0/src/include/lean/lean.h` (`lean_object`, `m_rc`, `m_tag`); `koka-v3.2.9/kklib/include/kklib.h` (`kk_header_t`, `refcount`); `ruby-v3_4_12/include/ruby/internal/core/rbasic.h` (`RBasic`, `flags`, `klass`); `cig-script/src/value.rs`.

### 12. Immortal and static objects that are never counted

- **What it does, and why.** CPython 3.15 marks "immortal" objects with a reference count at or above `_Py_IMMORTAL_INITIAL_REFCNT = 3 << 30` and never adjusts the count below that; a count at or above `_Py_IMMORTAL_MINIMUM_REFCNT = 1 << 31` tells `_Py_IsImmortal` to skip the inc and dec (`cpython-v3.15.2/Include/refcount.h` lines 44 to 74); the stack reference carries a `Py_TAG_REFCNT = 1` bit for a borrowed stackref so the pop does not touch the count either (`Include/internal/pycore_stackref.h: PyStackRef_CLOSE`, lines 210 to 230).
  Koka marks static objects with a reference count initialized to `INT32_MIN`: `KK_HEADER_STATIC(scan_fsize, tag) = { scan_fsize, 0, tag, KK_ATOMIC_VAR_INIT(INT32_MIN) }` (`koka-v3.2.9/kklib/include/kklib.h` line 147), and `kk_refcount_is_thread_shared(rc) = (rc < 0)` at line 109 reads any count in the negative range as "do not touch".
  Lean 4's own equivalent is a pair of "sticky" thresholds deep in the negative range: `LEAN_RC_STICKY = INT_MIN + 0x10000000` and `LEAN_RC_STICKY_DROP = INT_MIN + 0x20000000` (`lean4-v4.35.0/src/include/lean/lean.h` lines 651 to 652; the comment at lines 639 to 649 reads "once in the sticky range the object is frozen: it is never freed and its count is no longer adjusted").
  Both keep the test cheap: a count above a threshold means "do not touch", and the fast path's inc and dec branch off that.
  Go's `convT64` uses a statically allocated `staticuint64s[256]` so small integer values carried through `interface{}` do not allocate, and the subsequent decref path has nothing to free (`go-go1.27.4/src/runtime/iface.go` lines 409 to 430).
  Ruby's `Qtrue`, `Qfalse`, `Qnil` are immediate values and never counted.
  The census in `data/census/report.md` says cigo builds 2,323 lists, 804 maps and 662 closures at every command's top level that it then drops; a per-process immortal class for the module-constants table is the shape of a win there.
- **For cig: adapt.** Decided by rule 6 (deterministic cleanup stays: an immortal value is one whose count is never adjusted and whose drop is a no-op).
  cig's form: no immortal path exists today; a build-time constant table under `cig-script/src/vm/mod.rs: load_const` (around the `Op::LOADK` arm) that uses `Rc::increment_strong_count`'s equivalent to make a `Rc<str>` whose count never falls below one is a candidate.
  The more measured path is Go's: a static arena for constants loaded from the module, so a `drop` is a cheap no-op.
- **What would decide it.** Not measured yet: the share of a cigo call's `mov` and `loadk` that touches a constant whose lifetime is the whole process; the census's finding 6 says 94.3% of `cigo --version` is the module top level.
  The threshold is a 5% reduction in start-up on cigo's cases.
- **Feeds.** cig-compile#35, #36 (start-up); decisions (g).
- **Read.** `cpython-v3.15.2/Include/refcount.h` (`_Py_IMMORTAL_INITIAL_REFCNT`, `_Py_IMMORTAL_MINIMUM_REFCNT`); `cpython-v3.15.2/Include/internal/pycore_stackref.h` (`Py_TAG_REFCNT`); `koka-v3.2.9/kklib/include/kklib.h` (`KK_HEADER_STATIC`, `kk_refcount_is_thread_shared`); `lean4-v4.35.0/src/include/lean/lean.h` (`LEAN_RC_STICKY`, `LEAN_RC_STICKY_DROP`); `go-go1.27.4/src/runtime/iface.go` (`convT64`, `staticuint64s`); `ruby-v3_4_12/include/ruby/internal/special_consts.h` (`RUBY_Qnil`, `RUBY_Qtrue`); `data/census/report.md`.

### 13. What a clone and a drop touch in memory

- **What it does, and why.** A `Value::clone()` in cig is a `match` on the variant: for `Int`, `Float`, `Bool`, `Null` it copies bits; for `Str(Rc<str>)` it increments the `Rc`'s strong count (a non-atomic add on a word in the heap control block preceding the string); for `List`, `Map`, `Func`, `Builtin`, `Module`, `Chain` it increments the control-block count of the respective `Rc`.
  A `drop` decrements, and at zero it frees.
  The target for the cycle-collector is on top (Bacon-Rajan, cig-compile#35's "Memory management"); nothing in the value changes for that.
  Lean's `lean_dec` is the same shape: a decrement of the object header's `m_rc`, then a free; Koka's `kk_refcount_decrement` is atomic on shared objects and non-atomic on unique ones (`kklib/include/kklib.h` the `kk_refcount_*` family).
  CPython's `Py_DECREF` is a decrement of `ob_refcnt`; immortal objects skip it (`cpython-v3.15.2/Include/refcount.h`).
  What a clone always costs: one load, one add, one store on the counted object's header, plus the value-sized copy (two stores for tagged-16, three for cig's 24 today).
  A drop adds a branch on zero.
  The census (`data/census/report.md` finding 5) says the median compute program sees half of its `mov` instructions move a counted value; a tagged-16 does not shrink the per-move count traffic.
- **For cig: take.** Decided by rule 6 and rule 9 (the count sits in the object header, the value copies safely).
  cig's form: `Value::clone` in `cig-script/src/value.rs` already does this; the CIR pass for (g) cancels `Rc::clone`/`drop` pairs the lowering emits (`cig-script/src/lower/cir/pass/mod.rs: run_passes` names the four passes; none is the elision pass yet).
- **What would decide it.** Decided by rule 6.
- **Feeds.** cig-compile#35; decisions (g).
- **Read.** `cig-script/src/value.rs`; `cpython-v3.15.2/Include/refcount.h`; `lean4-v4.35.0/src/include/lean/lean.h` (`m_rc`, `lean_internal_get_rc`); `koka-v3.2.9/kklib/include/kklib.h` (`kk_refcount_inc`); `data/census/report.md`.

### 14. Which layouts can be written in safe Rust, and which cannot

- **What it does, and why.** A two-word enum whose payloads are each one word compiles in safe Rust at 16 bytes: cig's `Value` today is 24 bytes because `Rc<str>` is a fat pointer (`cig-script/src/value.rs`), and the `Str(Rc<StrObj>)` variant `cig-script/docs/research/lua.md` entry 5 names is what brings it to 16.
  Nothing in it requires `unsafe`.
  Every other layout in this study needs `unsafe` for Rust: a pointer with low tag bits (V8, Ruby, OCaml, Lean, Koka, CPython's stackref) casts a `usize` to a pointer and back; a `union` of a double and a tagged `u64` (LuaJIT, SpiderMonkey, JavaScriptCore, Wren, QuickJS 32-bit) loads the same bytes as two Rust types; and each of these keeps aliasing rules the Rust compiler does not know about.
  The fastest in-production value in the library, LuaJIT's 8-byte NaN-tagged `TValue`, is the clearest example: a Rust port would be `unsafe` throughout its accessors.
  cig's rule 9 forbids that outside one audited crate; today, no crate holds such a value (`cig-script/crates/` has `cig-bytecode`, `cig-copy`, `cig-pass`, `cig-vault`, none of which is a value crate).
  The VM and the bytecode crate carry `#![forbid(unsafe_code)]`.
- **For cig: adapt.** The safe shape is taken; every unsafe shape is rejected outside one audited crate.
  Decided by rule 9.
  cig's form: the enum `Value` in `cig-script/src/value.rs` is the safe shape; a NaN-boxed 8 can only live behind a crate like `cig-value` not yet started, with one entry point and one audit.
- **What would decide it.** Decided by rule 9.
- **Feeds.** cig-compile#35; decisions (a), (e).
- **Read.** `cig-script/src/value.rs`; `cig-script/src/vm/mod.rs` (`#![forbid(unsafe_code)]`); `cig-script/crates/`; `cig-script/docs/research/lua.md` (entry 5); `cig-script/docs/research/luajit.md` (A5).

### 15. QuickJS and Wren: both layouts behind one set of accessors

- **What it does, and why.** QuickJS ships both layouts in the same header, chosen at build time by `JS_NAN_BOXING`.
  The accessors (`JS_VALUE_GET_TAG`, `JS_VALUE_GET_INT`, `JS_VALUE_GET_BOOL`, `JS_VALUE_GET_FLOAT64`, `JS_VALUE_GET_PTR`, `JS_MKVAL`, `JS_MKPTR`, `JS_NAN`, `__JS_NewFloat64`, `JS_VALUE_IS_NAN`, `JS_TAG_IS_FLOAT64`, `JS_VALUE_IS_BOTH_INT`, `JS_VALUE_IS_BOTH_FLOAT`, `JS_VALUE_HAS_REF_COUNT`) are one API; the underlying type is a `uint64_t` in the NaN-box branch and a `{JSValueUnion u; int64_t tag;}` struct in the union branch (`quickjs-ng-v0.18.0/quickjs.h` lines 173 to 393).
  Wren does the same with `WREN_NAN_TAGGING`: `IS_NUM`, `IS_OBJ`, `IS_NULL`, `AS_BOOL`, `AS_OBJ`, `NULL_VAL`, `FALSE_VAL`, `TRUE_VAL` are the one set; the underlying type is a `uint64_t` or a `typedef struct { ValueType type; union { double num; Obj *obj; } as; } Value` (`wren-0.4.0/src/vm/wren_value.h` lines 121 to 619).
  This is the pattern cig can follow to measure the two candidates against each other and against today's 24: the VM calls one set of functions; the value is defined in one crate; the crate has the audited `unsafe` or none.
  The decision in cig-compile#35 already names the 15% threshold.
- **For cig: adapt.** Decided by rule 9: the only way to measure a NaN-boxed 8 without coloring the rest of the tree with `unsafe` is to confine it to one crate, as QuickJS confines its tag-and-payload-or-NaN-box split to one header.
  cig's form: a `cig-value` crate (not yet made) that holds the `Value` type and its accessors, matching `src/value.rs`'s API; the VM and the bytecode crate depend on it and nothing else.
  #35's "Order of work" step 6 names the prototype against today's 24 bytes.
- **What would decide it.** Not measured yet: the prototype crate, the differential suite's zero differences on both shapes, and the 15% threshold on `nbody`, `fannkuch`, `mandel`, `trees`, `sieve`.
- **Feeds.** cig-compile#35; decisions (a), (e).
- **Read.** `quickjs-ng-v0.18.0/quickjs.h` (`JS_NAN_BOXING`, `JS_VALUE_GET_TAG`, `JS_MKVAL`, `JS_MKPTR`, `JS_VALUE_HAS_REF_COUNT`, `JSValueUnion`); `wren-0.4.0/src/vm/wren_value.h` (`WREN_NAN_TAGGING`, `IS_NUM`, `IS_OBJ`, `AS_OBJ`, `NULL_VAL`); `cig-script/src/value.rs`; `cig-script/crates/`; `cig-script/docs/research/luajit.md` (A5).

## What cig should carry away (the decision's inputs)

| candidate | bytes | `i64` unboxed | doubles unboxed | safe Rust | registers per value, native code | type test | where the count lives | VM, tree-walker, native code agree on |
|---|---|---|---|---|---|---|---|---|
| today, 24 bytes | 24 | yes | yes | yes | 3 words through memory | byte load, compare, branch (one `match` arm) | `Rc<T>` header, outside the value | enum variant |
| tagged-16 | 16 | yes | yes | yes | 2 words in two registers | one byte load, compare | `Rc<T>` header, outside the value | enum variant, with `Value::Int` at discriminant 0 |
| NaN-boxed 8 | 8 | no past 48 bits | yes | no (one audited crate) | 1 word in one register | high-bits compare | object header, outside the value | tag-word layout exported by one crate |
| pointer-tagged 8 | 8 | no past 63 bits | some only (Ruby's flonum) | no (one audited crate) | 1 word in one register | mask and compare | object header, outside the value | masked-pointer form exported by one crate |

Of the four, three fail rule 9 outside one audited crate; two fail cig's `i64` semantics without a boxed-bignum path (NaN-boxed 8, pointer-tagged 8); one carries three words through memory under the current ABI (today's 24).
**The only candidate that meets every rule in safe Rust is tagged-16.** The 15% threshold on NaN-boxed 8 is the measurement #35 names, and this study does not measure.

## For the issues

- **cig-compile#35.** The "today, 24 bytes" row above is the start; `Str(Rc<StrObj>)` brings the enum to 16 under `cig-script/docs/research/lua.md` entry 5 (entry 1, 14).
  The NaN-boxed 8 can be measured only behind one audited crate (entry 15); every other pointer-tag layout fails rule 9 (entry 2).
  Full `i64` only survives in tagged-16 or in today's 24, which is why #35's own default bet is tagged-16 (entry 9).
  A clone and a drop touch the object's header, not the value (entries 11, 13); immortals are a start-up and a cigo win, not a value-layout change (entry 12).
- **cig-compile#32.** Stage C's two-register value ABI is the tag-and-payload family (entry 1); the "integer first for arithmetic" test is one word compare on a `|` of the tags, which cig can take by ordering `Value::Int` at discriminant 0 (entry 5); the "inline fast path, runtime slow path" structure for `a + b` sits on top of the paired tag test and `checked_add` for overflow (entry 10).
- **cig-script#243.** The VM's study list asks "how many go to the match on Value" (`cig-script/docs/research/luajit.md` A5's measurement, not done): entries 4, 5 and 7 give the layout cost; the shrink from 24 to 16 is the first budget.
- **cig-script#256.** cig-cherry takes the same ABI stage C fixes (`issues3/cig-compile-35.newcomments.md`): tagged-16, the paired tag test, one Shape pointer in the map object for field guards (entry 11).
  The cycle collector's view of native frames is a stack-map question #35 raises and this study leaves to #35.

## Measurements owed

- **NaN-boxed 8 against tagged-16 against today's 24,** on `nbody`, `fannkuch`, `mandel`, `trees`, `sieve`, `loop`, counting callgrind machine instructions and peak RSS; decides the 15% threshold in #35 (entries 3, 15).
- **Paired-tag test on `num_add`,** before and after ordering `Value::Int` at discriminant 0, on `nbody`, `loop`, `fib`; decides entry 5's adaptation.
- **A copy of a 16-byte tagged `Value` through two registers,** against three words through memory for the 24-byte `Value`, on `fib` and on `loop`; decides entry 8.
- **The immortal-constants path on cigo,** against the baseline cigo start-up (`data/census/report.md` finding 6's 2,323 lists, 804 maps, 662 closures built every call); decides entry 12.

## Not settled

- Which Rust toolchain produces the best codegen for a `#[repr(Rust)]` two-word `Value`: the compiler can reorder the discriminant beside the payload, and the layout is not stable; stage C's ABI depends on the layout being what we think.
  A `#[repr(C)]` or a hand-chosen layout may be needed.
  Not read: the toolchain's layout rules at the pin.
- How a NaN-boxed 8 handles `Value::Chain` and `Value::Module`: these are tagged pointers to Rust objects.
  The four or five bits of tag space in a NaN-box fit the current ten variants with room; a new variant would compete.
- What a Bacon-Rajan cycle collector sees in native frames at stage C and cig-cherry: #35's own open question (`issues3/cig-compile-35.newcomments.md`), not decided here.
