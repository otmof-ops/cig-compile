// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! `--emit bc` on the command line. The lowering's own tests are in
//! cig-script (`tests/lower_bc.rs`); `tests/emit_bc/calls.bc` is the same
//! golden text as `tests/lower/bc/calls.bc` there.

/// `cigcompile build --emit bc` prints the golden text; a script outside
/// this part is refused with its construct, its position and exit 3. Run
/// with a scratch home: no runtime is needed or looked for.
#[test]
fn emit_bc_prints_the_disassembly_and_refuses_what_it_does_not_cover() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let cc = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_cigcompile"))
            .args(args)
            .current_dir(dir.path())
            .env("HOME", &home)
            .env("CIGSCRIPT_HOME", home.join(".cigscript"))
            .env("XDG_CONFIG_HOME", home.join(".config"))
            .env("XDG_DATA_HOME", home.join(".local/share"))
            .env("XDG_CACHE_HOME", home.join(".cache"))
            .env("XDG_STATE_HOME", home.join(".local/state"))
            .env("NO_COLOR", "1")
            .env("CIG_DOCTOR", "0")
            .env("CIG_REPORTS", "never")
            .env("CIGCOMPILE_OFFLINE", "1")
            .output()
            .unwrap()
    };
    let golden = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/emit_bc");
    let src = std::fs::read_to_string(golden.join("calls.cig"))
        .unwrap()
        .replace("\r\n", "\n");
    std::fs::write(dir.path().join("calls.cig"), src).unwrap();
    let o = cc(&["build", "calls.cig", "--emit", "bc"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let want = std::fs::read_to_string(golden.join("calls.bc"))
        .unwrap()
        .replace("\r\n", "\n");
    assert_eq!(String::from_utf8_lossy(&o.stdout), want);

    std::fs::write(
        dir.path().join("root.cig"),
        "pack { root() }\npull root() {\n  snuff \"./build\"\n}\n",
    )
    .unwrap();
    let o = cc(&["build", "root.cig", "--emit", "bc"]);
    assert_eq!(o.status.code(), Some(3));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(
        err.contains("a `pack` root that runs a pull or a function is not lowered to bytecode yet")
            && err.contains("root.cig:1:1"),
        "{err}"
    );
}
