#![no_main]
//! Fuzz target `lower`: see cigcompile::fuzz.

libfuzzer_sys::fuzz_target!(|data: &[u8]| cigcompile::fuzz::lower(data));
