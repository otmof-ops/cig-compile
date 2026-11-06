// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! cigcompile end to end, with the `cig-runtime` this build made as the
//! runtime (and, run as `cig-runtime run`, as the interpreter the programs
//! are compared with).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const RT: &str = env!("CARGO_BIN_EXE_cig-runtime");

struct Sandbox {
    dir: tempfile::TempDir,
}

impl Sandbox {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("tempdir"),
        }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn write(&self, name: &str, content: &str) {
        let p = self.path().join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, content).unwrap();
    }

    fn env(&self, c: &mut Command) {
        c.current_dir(self.path())
            .env("CIGSCRIPT_HOME", self.path().join("home"))
            .env("NO_COLOR", "1")
            .env("CIG_SEAL_KEY", "file")
            .env("CIG_DOCTOR", "0")
            .env("CIG_REPORTS", "never")
            .env("CIG_INTERNAL_TRACKER_DIR", self.path().join("tracker"))
            .env("CIG_UPDATE_MANIFEST", self.path().join("no-releases.json"))
            .env("CIGCOMPILE_RUNTIME", RT)
            // Never the network from a test: a runtime not here is an error.
            .env("CIGCOMPILE_OFFLINE", "1")
            .env_remove("CI");
    }

    fn cc(&self, args: &[&str]) -> Output {
        let mut c = Command::new(env!("CARGO_BIN_EXE_cigcompile"));
        self.env(&mut c);
        c.args(args).output().expect("run cigcompile")
    }

    fn cig(&self, args: &[&str]) -> Output {
        let mut c = Command::new(RT);
        self.env(&mut c);
        c.args(args).output().expect("run cig")
    }

    fn prog(&self, exe: &str, args: &[&str]) -> Output {
        let mut c = Command::new(self.path().join(exe));
        self.env(&mut c);
        c.args(args);
        // ETXTBSY: another test's fork can still hold a program just
        // written open for writing, between its fork and its exec.
        for _ in 0..80 {
            match c.output() {
                Err(e) if cfg!(unix) && e.raw_os_error() == Some(26) => {
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                other => return other.expect("run the program"),
            }
        }
        c.output().expect("run the program")
    }

    fn exe(&self, name: &str) -> PathBuf {
        self.path()
            .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
    }
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).to_string()
}

fn ok(o: &Output) -> String {
    assert!(
        o.status.success(),
        "exit {:?}\nstdout:\n{}\nstderr:\n{}",
        o.status.code(),
        text(&o.stdout),
        text(&o.stderr)
    );
    text(&o.stdout)
}

fn exe_name(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

#[test]
fn a_script_compiles_and_the_program_says_what_cig_run_says() {
    let s = Sandbox::new();
    s.write(
        "hello.cig",
        "stick name = args.get(0, \"world\")\nexhale \"hello, ${name}\"\n",
    );
    let out = ok(&s.cc(&["build", "hello.cig"]));
    assert!(out.contains("compiled hello.cig -> hello"), "{out}");
    assert!(s.exe("hello").is_file());
    let p = s.prog(&exe_name("hello"), &["you"]);
    let c = s.cig(&["run", "hello.cig", "--", "you"]);
    assert_eq!(text(&p.stdout), "hello, you\n");
    assert_eq!(text(&p.stdout), text(&c.stdout));
    assert_eq!(p.status.code(), c.status.code());
}

#[test]
fn a_rebuild_that_changes_nothing_touches_nothing_and_builds_are_reproducible() {
    let s = Sandbox::new();
    s.write("a.cig", "exhale \"a\"\n");
    ok(&s.cc(&["build", "a.cig"]));
    let first = fs::read(s.exe("a")).unwrap();
    let mtime = fs::metadata(s.exe("a")).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(30));
    let again = ok(&s.cc(&["build", "a.cig"]));
    assert!(again.contains("up to date"), "{again}");
    assert_eq!(fs::metadata(s.exe("a")).unwrap().modified().unwrap(), mtime);
    // Another output path, the same bytes.
    ok(&s.cc(&["build", "a.cig", "-o", &exe_name("other")]));
    assert_eq!(fs::read(s.exe("other")).unwrap(), first);
    // A changed script is a new program.
    s.write("a.cig", "exhale \"b\"\n");
    assert!(ok(&s.cc(&["build", "a.cig"])).contains("compiled"));
    assert_eq!(text(&s.prog(&exe_name("a"), &[]).stdout), "b\n");
}

#[test]
fn a_script_that_does_not_check_builds_nothing_and_exits_like_cig_check() {
    let s = Sandbox::new();
    s.write("bad.cig", "exhale nope_not_declared\n");
    let o = s.cc(&["build", "bad.cig"]);
    assert_eq!(o.status.code(), Some(2), "{}", text(&o.stderr));
    let err = text(&o.stderr);
    assert!(err.contains("bad.cig:1:"), "{err}");
    assert!(
        !s.exe("bad").exists(),
        "a program was written from a script that does not check"
    );
    let c = s.cig(&["check", "bad.cig"]);
    assert_eq!(c.status.code(), Some(2));
}

#[test]
fn what_cannot_be_built_says_its_code() {
    let s = Sandbox::new();
    s.write("x.cig", "exhale 1\n");
    let cases: Vec<(Vec<&str>, &str)> = vec![
        (vec!["build", "missing.cig"], "E1000"),
        (
            vec!["build", "x.cig", "--target", "sparc-sun-solaris"],
            "E1006",
        ),
        (vec!["build", "x.cig", "--runtime", "x.cig"], "E1002"),
    ];
    for (args, code) in cases {
        let o = s.cc(&args);
        assert_eq!(o.status.code(), Some(3), "{args:?}: {}", text(&o.stderr));
        assert!(
            text(&o.stderr).contains(code),
            "{args:?}: {}",
            text(&o.stderr)
        );
    }
    // A program is not a runtime.
    ok(&s.cc(&["build", "x.cig"]));
    let o = s.cc(&["build", "x.cig", "-o", "y", "--runtime", &exe_name("x")]);
    assert!(text(&o.stderr).contains("E1005"), "{}", text(&o.stderr));
    // Another machine's runtime is not found here unless given.
    let other = if cfg!(windows) {
        "x86_64-unknown-linux-musl"
    } else {
        "x86_64-pc-windows-msvc"
    };
    let mut c = Command::new(env!("CARGO_BIN_EXE_cigcompile"));
    s.env(&mut c);
    let o = c
        .env_remove("CIGCOMPILE_RUNTIME")
        .args(["build", "x.cig", "--target", other])
        .output()
        .unwrap();
    assert!(text(&o.stderr).contains("E1001"), "{}", text(&o.stderr));
    assert!(text(&o.stderr).contains("--runtime"), "{}", text(&o.stderr));
}

#[test]
fn a_pin_the_runtime_does_not_meet_is_refused() {
    let s = Sandbox::new();
    s.write("p.cig", "# cig 2.0\nexhale 1\n");
    let o = s.cc(&["build", "p.cig"]);
    assert_eq!(o.status.code(), Some(3));
    assert!(text(&o.stderr).contains("E1003"), "{}", text(&o.stderr));
    assert!(!s.exe("p").exists());
}

#[test]
fn a_folder_builds_every_script_into_bin_and_clean_takes_only_those_out() {
    let s = Sandbox::new();
    s.write("proj/a.cig", "exhale \"a\"\n");
    s.write("proj/b.cig", "exhale \"b\"\n");
    s.write("proj/bin/keep.txt", "not ours");
    let out = ok(&s.cc(&["build", "proj"]));
    // The path as the platform writes it: proj\a.cig on Windows.
    let sep = std::path::MAIN_SEPARATOR;
    assert!(
        out.contains(&format!("proj{sep}a.cig")) && out.contains(&format!("proj{sep}b.cig")),
        "{out}"
    );
    let a = format!("proj/bin/{}", exe_name("a"));
    assert_eq!(text(&s.prog(&a, &[]).stdout), "a\n");
    // The program says where it came from, as compiled.
    let info = text(&s.prog(&a, &["--cig-program", "--json"]).stdout);
    assert!(
        info.contains(&format!(
            "\"file\":{}",
            serde_json::to_string(&format!("proj{sep}a.cig")).unwrap()
        )),
        "{info}"
    );
    ok(&s.cc(&["clean", "proj"]));
    assert!(!s.path().join(&a).exists());
    assert!(
        s.path().join("proj/bin/keep.txt").exists(),
        "clean took something that was not a program"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn two_scripts_that_would_share_a_program_are_refused() {
    let s = Sandbox::new();
    s.write("proj/tool.cig", "exhale 1\n");
    s.write("proj/Tool.cig", "exhale 2\n");
    let o = s.cc(&["build", "proj"]);
    assert_eq!(o.status.code(), Some(3));
    assert!(text(&o.stderr).contains("E1007"), "{}", text(&o.stderr));
}

#[test]
fn test_compares_each_program_with_its_script() {
    let s = Sandbox::new();
    s.write(
        "w.cig",
        "pull f() {\n  stick unused = 1\n  snuff 2\n}\nburn {\n  fs.write_text(\"w.txt\", \"x\")\n}\nexhale f()\n",
    );
    let out = ok(&s.cc(&["test", "w.cig"]));
    assert!(out.contains("same     w.cig"), "{out}");
    assert!(!s.path().join("w.txt").exists(), "a dry run burned");
}

#[test]
fn check_needs_no_runtime_and_writes_nothing() {
    let s = Sandbox::new();
    s.write("c.cig", "exhale 1\n");
    let mut c = Command::new(env!("CARGO_BIN_EXE_cigcompile"));
    s.env(&mut c);
    let o = c
        .env_remove("CIGCOMPILE_RUNTIME")
        .args(["build", "--check", "c.cig"])
        .output()
        .unwrap();
    assert_eq!(text(&o.stdout), "checked c.cig\n", "{}", text(&o.stderr));
    assert!(!s.exe("c").exists());
}

#[test]
fn json_says_what_was_built() {
    let s = Sandbox::new();
    s.write("j.cig", "exhale 1\n");
    let out = ok(&s.cc(&["build", "--json", "j.cig"]));
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(v["name"], "j");
    assert_eq!(v["script"], "j.cig");
    assert_eq!(v["written"], true);
    assert_eq!(v["runtime"]["cig"], cigscript::VERSION);
    assert_eq!(v["sha256"].as_str().unwrap().len(), 64);
}

#[test]
fn a_program_changed_by_hand_is_built_again() {
    let s = Sandbox::new();
    s.write("h.cig", "exhale \"h\"\n");
    ok(&s.cc(&["build", "h.cig"]));
    let good = fs::read(s.exe("h")).unwrap();
    fs::write(s.exe("h"), b"not a program").unwrap();
    let again = ok(&s.cc(&["build", "h.cig"]));
    assert!(again.contains("compiled"), "{again}");
    assert_eq!(fs::read(s.exe("h")).unwrap(), good);
}

#[test]
fn a_build_that_changes_nothing_says_the_warnings_again() {
    let s = Sandbox::new();
    // An effect in a function, outside any burn: a warning, never an error.
    s.write(
        "w.cig",
        "pull f() {\n  fs.write_text(\"x.txt\", \"y\")\n}\nexhale \"ok\"\n",
    );
    let first = s.cc(&["build", "w.cig"]);
    assert!(first.status.success(), "{}", text(&first.stderr));
    assert!(
        text(&first.stderr).contains("warning["),
        "{}",
        text(&first.stderr)
    );
    let again = s.cc(&["build", "w.cig"]);
    assert!(
        text(&again.stdout).contains("up to date"),
        "{}",
        text(&again.stdout)
    );
    assert_eq!(text(&again.stderr), text(&first.stderr));
}

/// A script nested deeper than the parser allows is refused (E208), as
/// `cig check` refuses it, never a stack overflow.
#[test]
fn a_deeply_nested_script_is_refused_not_a_crash() {
    let s = Sandbox::new();
    s.write(
        "deep.cig",
        &format!("exhale {}{}\n", "[".repeat(100_000), "]".repeat(100_000)),
    );
    let o = s.cc(&["build", "--check", "deep.cig"]);
    assert_eq!(o.status.code(), Some(2), "{}", text(&o.stderr));
    assert!(text(&o.stderr).contains("E208"), "{}", text(&o.stderr));
}
