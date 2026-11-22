// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! cigcompile as a library: `--emit` with its diffs and remarks, and the
//! fuzz targets' bodies (#16). CIR (cig-compile#3) and its lowering to
//! bytecode (cig-compile#31) live in cig-script, as `cigscript::lower`.

pub mod diff;
pub mod emit;
pub mod fuzz;
