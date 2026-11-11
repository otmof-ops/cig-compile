// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
#![no_main]
//! Fuzz target `parse`: see cigcompile::fuzz.

libfuzzer_sys::fuzz_target!(|data: &[u8]| cigcompile::fuzz::parse(data));
