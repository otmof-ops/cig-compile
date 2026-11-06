#![no_main]
//! Fuzz target `parse`: see cigcompile::fuzz.

libfuzzer_sys::fuzz_target!(|data: &[u8]| cigcompile::fuzz::parse(data));
