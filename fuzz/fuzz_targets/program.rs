#![no_main]
//! Fuzz target `program`: see cigcompile::fuzz.

libfuzzer_sys::fuzz_target!(|data: &[u8]| cigcompile::fuzz::program(data));
