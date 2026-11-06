// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! The runtime: `cig`, exactly as `src/main.rs` in CigScript starts it.
//! A program is this binary with its script appended; with none appended,
//! this is `cig` of the version cigcompile pins.

fn main() {
    std::process::exit(cigscript::cli::entry());
}
