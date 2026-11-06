#![no_main]
//! Fuzz target `check`: see cigcompile::fuzz.

libfuzzer_sys::fuzz_target!(|data: &[u8]| cigcompile::fuzz::check(data));
