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
    // The program says which script it came from: its name, not the path
    // it was built from (#9).
    let info = text(&s.prog(&a, &["--cig-program", "--json"]).stdout);
    assert!(info.contains("\"file\":\"a.cig\""), "{info}");
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

/// #9: a program records its script's name, never the path it was built
/// from, so no folder of the build machine is in it, and one script gives
/// the same bytes however it was named.
#[test]
fn one_script_gives_the_same_bytes_however_it_is_named() {
    let sb = Sandbox::new();
    sb.write("tools/hello.cig", "exhale \"hello\"\n");
    let abs = sb.path().join("tools").join("hello.cig");
    let (by_path, by_name) = (exe_name("by-path"), exe_name("by-name"));
    ok(&sb.cc(&["build", abs.to_str().unwrap(), "-o", &by_path]));
    ok(&sb.cc(&["build", "tools/hello.cig", "-o", &by_name]));
    let a = fs::read(sb.path().join(&by_path)).unwrap();
    let b = fs::read(sb.path().join(&by_name)).unwrap();
    assert!(a == b, "the same script built to different bytes");
    let folder = sb.path().to_string_lossy().to_string();
    assert!(
        !a.windows(folder.len()).any(|w| w == folder.as_bytes()),
        "the build machine's folder is in the program"
    );
    assert_eq!(ok(&sb.prog(&by_path, &[])).trim(), "hello");
}

/// A script named by a path says `tools/x.cig:1:8` when it fails, its
/// program `x.cig:1:8` (#9): `cigcompile test` hears the two as the same.
#[test]
fn test_hears_a_script_named_by_a_path_as_its_program() {
    let s = Sandbox::new();
    s.write("tools/x.cig", "exhale args[5]\n");
    let o = s.cc(&["test", "tools/x.cig"]);
    assert!(
        o.status.success() && text(&o.stdout).contains("same"),
        "exit {:?}\n{}{}",
        o.status.code(),
        text(&o.stdout),
        text(&o.stderr)
    );
}

/// The same kind of machine with the other CPU, and another kind of machine.
fn other_machines() -> Vec<&'static str> {
    use std::env::consts::{ARCH, OS};
    let other_cpu = match (OS, ARCH) {
        ("linux", "x86_64") => Some("aarch64-unknown-linux-musl"),
        ("linux", "aarch64") => Some("x86_64-unknown-linux-musl"),
        ("macos", "aarch64") => Some("x86_64-apple-darwin"),
        ("macos", "x86_64") => Some("aarch64-apple-darwin"),
        ("windows", "x86_64") => Some("aarch64-pc-windows-msvc"),
        ("windows", "aarch64") => Some("x86_64-pc-windows-msvc"),
        _ => None,
    };
    let other_kind = if cfg!(windows) {
        "x86_64-unknown-linux-musl"
    } else {
        "x86_64-pc-windows-msvc"
    };
    other_cpu.into_iter().chain([other_kind]).collect()
}

/// A runtime is checked as the runtime for the machine asked for, its kind
/// of executable and its CPU, even after the build index has come to know
/// it as this machine's: that once let a Linux cig through as a Windows
/// runtime, and wrote an ELF program named for Windows.
#[test]
fn a_runtime_known_here_is_still_refused_for_another_machine() {
    let s = Sandbox::new();
    s.write("x.cig", "exhale 1\n");
    ok(&s.cc(&["build", "x.cig"]));
    for t in other_machines() {
        let o = s.cc(&["build", "x.cig", "--target", t, "-o", "y"]);
        assert_eq!(o.status.code(), Some(3), "{t}: {}", text(&o.stderr));
        assert!(
            text(&o.stderr).contains("E1002"),
            "{t}: {}",
            text(&o.stderr)
        );
        assert!(!s.path().join("y").exists(), "{t}: a program was written");
    }
}

/// What clean takes out is what a build of this path wrote there (#28). A
/// program of the same name, built in another folder and copied here, is
/// kept and named, unless --force.
#[test]
fn clean_keeps_a_program_built_elsewhere_unless_forced() {
    let s = Sandbox::new();
    s.write("other/tidy.cig", "exhale \"a different tidy\"\n");
    s.write("cl/tidy.cig", "exhale \"this tidy\"\n");
    let run = |dir: &str, args: &[&str]| {
        let mut c = Command::new(env!("CARGO_BIN_EXE_cigcompile"));
        s.env(&mut c);
        c.current_dir(s.path().join(dir))
            .args(args)
            .output()
            .unwrap()
    };
    ok(&run("other", &["build", "tidy.cig"]));
    let from = s.path().join("other").join(exe_name("tidy"));
    let here = s.path().join("cl").join(exe_name("tidy"));
    fs::copy(&from, &here).unwrap();
    let out = ok(&run("cl", &["clean", "tidy.cig"]));
    assert!(out.contains("kept") && out.contains("--force"), "{out}");
    assert!(here.exists(), "clean took a program it did not write");
    let out = ok(&run("cl", &["clean", "--force", "tidy.cig"]));
    assert!(out.contains("removed"), "{out}");
    assert!(!here.exists());
    // What this build wrote goes, and the copy it was made from stays.
    ok(&run("cl", &["build", "tidy.cig"]));
    let out = ok(&run("cl", &["clean", "tidy.cig"]));
    assert!(out.contains("removed"), "{out}");
    assert!(!here.exists());
    assert!(from.exists());
}

/// With the build index gone, clean cannot tell what it wrote: it keeps the
/// program and says --force (#28).
#[test]
fn clean_with_no_build_index_keeps_and_says_force() {
    let s = Sandbox::new();
    s.write("t.cig", "exhale 1\n");
    ok(&s.cc(&["build", "t.cig"]));
    fs::remove_file(s.path().join("home/cache/compile/index.json")).unwrap();
    let out = ok(&s.cc(&["clean", "t.cig"]));
    assert!(out.contains("kept") && out.contains("--force"), "{out}");
    assert!(s.exe("t").exists());
    let v = text(&s.cc(&["clean", "--json", "t.cig"]).stdout);
    assert!(v.contains("\"kept\""), "{v}");
}

/// A benchmark whose output is not its `.expect` file is WRONG and gets no
/// time: a fast wrong answer is a bug, not a result (#30).
#[test]
fn a_wrong_benchmark_output_fails_before_it_is_timed() {
    let s = Sandbox::new();
    s.write("set/x.cig", "exhale 0\n");
    s.write("set/x.expect", "1\n");
    let o = s.cc(&["bench", "--set", "set", "--runs", "2"]);
    let (out, err) = (text(&o.stdout), text(&o.stderr));
    assert_eq!(o.status.code(), Some(1), "{out}{err}");
    let row = out
        .lines()
        .find(|l| l.starts_with("x "))
        .unwrap_or_default();
    assert!(row.contains("WRONG"), "{out}");
    assert!(err.contains("x (interpreted) is WRONG"), "{err}");
}

/// The report for a pull request comment is a Markdown table (#30).
#[test]
fn the_markdown_bench_report_renders() {
    let s = Sandbox::new();
    s.write("set/one.cig", "exhale 1\n");
    s.write("set/one.expect", "1\n");
    let out = ok(&s.cc(&["bench", "--set", "set", "--runs", "2", "--markdown"]));
    assert!(out.contains("| benchmark | stresses |"), "{out}");
    assert!(out.contains("| `one` |"), "{out}");
    assert!(out.contains("| budget (#14) |"), "{out}");
    let v: serde_json::Value =
        serde_json::from_str(ok(&s.cc(&["bench", "--set", "set", "--runs", "2", "--json"])).trim())
            .unwrap();
    assert_eq!(v["programs"][0]["name"], "one");
    assert!(
        v["programs"][0]["interpreted"]["median_ms"].is_number(),
        "{v}"
    );
}

/// The machine a glibc build of these tests is on, as the build says it.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn gnu_triple() -> String {
    format!("{}-unknown-linux-gnu", std::env::consts::ARCH)
}

/// A runtime given by hand that links glibc still builds, and the build
/// says what its programs need (#27). A glibc build's cig-runtime is one.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
#[test]
fn a_dynamic_runtime_given_by_hand_warns_and_says_gnu() {
    let s = Sandbox::new();
    s.write("x.cig", "exhale 1\n");
    let o = s.cc(&["build", "x.cig"]);
    let (out, err) = (text(&o.stdout), text(&o.stderr));
    assert!(o.status.success(), "{err}");
    assert!(
        out.contains(&format!("for {} (glibc ", gnu_triple())),
        "{out}"
    );
    assert!(
        err.contains("warning: ") && err.contains("dynamically linked"),
        "{err}"
    );
    let o = s.cc(&["build", "--json", "x.cig"]);
    let v: serde_json::Value = serde_json::from_str(text(&o.stdout).trim()).unwrap();
    assert_eq!(v["target"], gnu_triple());
    assert!(v["needs"].as_str().unwrap().starts_with("glibc "), "{v}");
    assert!(text(&o.stderr).is_empty(), "json says it in the record");
}

/// A dynamically linked cig found here is passed over for a static Linux
/// target; with nothing else here and no fetching, the build is refused
/// with E1008 and writes nothing (#27).
#[cfg(all(target_os = "linux", target_env = "gnu"))]
#[test]
fn a_dynamic_runtime_found_here_is_passed_over_and_offline_refuses() {
    let s = Sandbox::new();
    s.write("x.cig", "exhale 1\n");
    let mut c = Command::new(env!("CARGO_BIN_EXE_cigcompile"));
    s.env(&mut c);
    let o = c
        .env_remove("CIGCOMPILE_RUNTIME")
        .env("PATH", s.path())
        .args(["build", "x.cig"])
        .output()
        .unwrap();
    let err = text(&o.stderr);
    assert_eq!(o.status.code(), Some(3), "{err}");
    assert!(
        err.contains("E1008") && err.contains("needs glibc"),
        "{err}"
    );
    assert!(err.contains("cigcompile fetch"), "{err}");
    assert!(!s.exe("x").exists(), "a program was written");
}

/// A script whose name holds an invisible character is refused before
/// anything is written (E1009): cig would refuse to run the program
/// (otmof-ops/cig-script#137), and the name shows as another.
#[cfg(unix)]
#[test]
fn a_script_whose_name_is_not_printable_is_refused() {
    let s = Sandbox::new();
    let name = "tidy\u{202e}gpj.cig";
    s.write(name, "exhale 1\n");
    let o = s.cc(&["build", name, "-o", "tidy"]);
    let err = text(&o.stderr);
    assert_eq!(o.status.code(), Some(3), "{err}");
    assert!(err.contains("E1009"), "{err}");
    assert!(
        err.contains("\\u{202e}"),
        "the name is shown escaped: {err}"
    );
    assert!(!s.path().join("tidy").exists(), "a program was written");
}

/// #33, #39: `reduce` cuts a script down to what its test needs, and every
/// line it keeps still passes `cig check`. The test here is a CigScript
/// script (it runs on every machine): interesting while the candidate
/// still says "boom".
#[test]
fn reduction_keeps_the_interest_and_is_small() {
    let s = Sandbox::new();
    let noise: String = (0..12)
        .map(|n| format!("roll v{n} = {n}\nexhale \"line {n}\"\n"))
        .collect();
    s.write(
        "big.cig",
        &format!(
            "{noise}pull twice(x) {{\n  snuff x * 2\n}}\nif v3 < v7 {{\n  exhale \"boom\", twice(v3)\n}}\nexhale twice(v11)\n"
        ),
    );
    s.write(
        "boom.cig",
        "if fs.read_text(args[0]).contains(\"boom\") {\n  exit(0)\n}\nexit(1)\n",
    );
    let before = fs::read_to_string(s.path().join("big.cig")).unwrap();
    let out = ok(&s.cc(&["reduce", "big.cig", "--", RT, "run", "boom.cig", "--", "{}"]));
    assert!(out.contains("reduced big.cig from 31 lines to"), "{out}");
    let reduced = fs::read_to_string(s.path().join("big.reduced.cig")).unwrap();
    assert!(reduced.contains("boom"), "{reduced}");
    assert!(
        reduced.lines().count() <= 8,
        "not reduced far enough:\n{reduced}"
    );
    // What reduce promises: the result still passes cig check, and the
    // test still holds on it (here, that it says "boom" somewhere).
    ok(&s.cig(&["check", "big.reduced.cig"]));
    // The original is untouched; a script that is not interesting to begin
    // with is refused; one that does not check is said as cig check says.
    assert_eq!(
        fs::read_to_string(s.path().join("big.cig")).unwrap(),
        before
    );
    s.write("calm.cig", "exhale \"calm\"\n");
    let no = s.cc(&[
        "reduce", "calm.cig", "--", RT, "run", "boom.cig", "--", "{}",
    ]);
    assert_eq!(no.status.code(), Some(3), "{}", text(&no.stderr));
    assert!(
        text(&no.stderr).contains("not interesting to begin with"),
        "{}",
        text(&no.stderr)
    );
    s.write("broken.cig", "exhale (\n");
    let bad = s.cc(&[
        "reduce",
        "broken.cig",
        "--",
        RT,
        "run",
        "boom.cig",
        "--",
        "{}",
    ]);
    assert_eq!(bad.status.code(), Some(2), "{}", text(&bad.stderr));
    // At stage A a program never differs from its script, so the default
    // test finds nothing to reduce, and says so.
    let same = s.cc(&["reduce", "calm.cig"]);
    assert_eq!(same.status.code(), Some(3), "{}", text(&same.stderr));
    assert!(
        text(&same.stderr).contains("finds no difference"),
        "{}",
        text(&same.stderr)
    );
}

/// #33: `-O0` is lowering only, `-O1` (the default) folds and prunes,
/// `--passes` runs exactly the passes named, in order, and
/// `--passes-diff` says what each one changed, and which changed nothing.
#[test]
fn levels_and_pass_lists_shape_the_emitted_ir() {
    let s = Sandbox::new();
    s.write("f.cig", "stick a = 2 + 3\nstick b = 7\nexhale a, b\n");
    let o0 = ok(&s.cc(&["build", "f.cig", "--emit", "cir", "-O0"]));
    assert!(o0.contains("bin +") && !o0.contains("const 5"), "{o0}");
    let o1 = ok(&s.cc(&["build", "f.cig", "--emit", "cir"]));
    assert!(!o1.contains("bin +") && o1.contains("const 5"), "{o1}");
    assert!(
        !o1.contains("const 2 "),
        "dead-pure kept the folded operands: {o1}"
    );
    assert_eq!(ok(&s.cc(&["build", "f.cig", "--emit", "cir", "-O2"])), o1);
    let fold_only = ok(&s.cc(&["build", "f.cig", "--emit", "cir", "--passes", "fold-consts"]));
    assert!(
        fold_only.contains("const 5") && fold_only.contains("const 2 "),
        "{fold_only}"
    );
    let diffed = ok(&s.cc(&[
        "build",
        "f.cig",
        "--emit",
        "cir",
        "--passes",
        "dead-pure,fold-consts,dead-pure",
        "--passes-diff",
    ]));
    assert!(
        diffed.contains("=== dead-pure: no change in f.cig"),
        "the first dead-pure has nothing to prune: {diffed}"
    );
    assert!(
        diffed.contains("--- f.cig before fold-consts")
            && diffed.contains("-  %2 = bin + %0, %1")
            && diffed.contains("+  %2 = const 5"),
        "{diffed}"
    );
    assert!(
        diffed.contains("--- f.cig before dead-pure"),
        "the second dead-pure prunes the folded operands: {diffed}"
    );
    let bad = s.cc(&["build", "f.cig", "--emit", "cir", "--passes", "inline"]);
    assert_eq!(bad.status.code(), Some(3));
    assert!(
        text(&bad.stderr).contains("there is no pass `inline`")
            && text(&bad.stderr).contains("fold-consts, dead-pure"),
        "{}",
        text(&bad.stderr)
    );
    let both = s.cc(&[
        "build",
        "f.cig",
        "--emit",
        "cir",
        "-O1",
        "--passes",
        "dead-pure",
    ]);
    assert_eq!(both.status.code(), Some(3), "-O and --passes together");
    let no_emit = s.cc(&["build", "f.cig", "--passes-diff"]);
    assert_eq!(
        no_emit.status.code(),
        Some(3),
        "--passes-diff without --emit"
    );
}

/// #40: `verify` reads a program's three parts and names the one that
/// differs: the runtime (against the cig given, or cig's release assets),
/// the payload (against its own sha256, and the script with --script) and
/// the whole file (against what a build of those parts writes).
#[test]
fn verify_names_the_part_that_differs() {
    let s = Sandbox::new();
    s.write("hi.cig", "exhale \"hi\"\n");
    ok(&s.cc(&["build", "hi.cig"]));
    let prog = exe_name("hi");
    let good = ok(&s.cc(&["verify", &prog, "--runtime", RT, "--script", "hi.cig"]));
    for part in ["runtime", "payload", "script", "whole"] {
        assert!(
            good.lines()
                .any(|l| l.trim_start().starts_with(part) && l.contains("ok")),
            "{part}: {good}"
        );
    }
    let json: serde_json::Value =
        serde_json::from_str(ok(&s.cc(&["--json", "verify", &prog, "--runtime", RT])).trim())
            .unwrap();
    assert_eq!(json["ok"], true, "{json}");
    // The runtime this test built is not one of cig's release assets.
    let plain = s.cc(&["verify", &prog]);
    assert_eq!(plain.status.code(), Some(1), "{}", text(&plain.stdout));
    assert!(
        text(&plain.stdout).contains("none of cig"),
        "{}",
        text(&plain.stdout)
    );
    // One byte changed in each part.
    let bytes = fs::read(s.path().join(&prog)).unwrap();
    let flip = |name: &str, at: usize| {
        let mut b = bytes.clone();
        b[at] ^= 0xff;
        fs::write(s.path().join(name), b).unwrap();
    };
    flip("in-runtime", bytes.len() / 2);
    flip("in-payload", bytes.len() - 60);
    flip("in-trailer", bytes.len() - 5);
    let r = s.cc(&["verify", "in-runtime", "--runtime", RT]);
    assert_eq!(r.status.code(), Some(1));
    assert!(
        text(&r.stdout)
            .lines()
            .any(|l| l.contains("runtime") && l.contains("DIFFERS")),
        "{}",
        text(&r.stdout)
    );
    for damaged in ["in-payload", "in-trailer"] {
        let r = s.cc(&["verify", damaged, "--runtime", RT]);
        assert_eq!(r.status.code(), Some(1), "{damaged}");
        assert!(
            text(&r.stdout).contains("damaged"),
            "{damaged}: {}",
            text(&r.stdout)
        );
    }
    // The script changed since the build.
    s.write("hi.cig", "exhale \"bye\"\n");
    let r = s.cc(&["verify", &prog, "--runtime", RT, "--script", "hi.cig"]);
    assert_eq!(r.status.code(), Some(1));
    assert!(
        text(&r.stdout).contains("changed since the build, from line 1"),
        "{}",
        text(&r.stdout)
    );
    // Something that is not a program at all.
    s.write("plain.txt", "not a program\n");
    assert_eq!(s.cc(&["verify", "plain.txt"]).status.code(), Some(3));
}

/// #37, #12, #40: `--timings` tables the phases (and writes a Chrome trace),
/// `--json-progress` says each step in order, one JSON object a line, and
/// `--sbom` writes a CycloneDX file beside each program that a rebuild
/// leaves byte for byte the same.
#[test]
fn timings_progress_and_sboms_say_what_the_build_did() {
    use sha2::Digest;
    let s = Sandbox::new();
    s.write("proj/a.cig", "exhale 1\n");
    s.write("proj/b.cig", "exhale 2\n");
    let out = s.cc(&[
        "build",
        "proj",
        "--timings=trace.json",
        "--json-progress",
        "--sbom",
    ]);
    ok(&out);
    let err = text(&out.stderr);
    let events: Vec<serde_json::Value> = err
        .lines()
        .filter(|l| l.starts_with('{'))
        .map(|l| serde_json::from_str(l).expect("a progress line is JSON"))
        .collect();
    let kinds: Vec<String> = events
        .iter()
        .filter(|e| e["event"] != "phase")
        .map(|e| e["event"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        kinds,
        ["start", "script", "built", "script", "built", "done"],
        "{err}"
    );
    assert_eq!(events[0]["scripts"], 2);
    assert!(events
        .iter()
        .any(|e| e["event"] == "phase" && e["phase"] == "check"));
    assert!(err.contains("  total"), "{err}");
    let trace: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(s.path().join("trace.json")).unwrap()).unwrap();
    let names: Vec<&str> = trace["traceEvents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    for phase in [
        "read", "check", "assemble", "hash", "write", "sbom", "build",
    ] {
        assert!(
            names.contains(&phase),
            "{phase} not in the trace: {names:?}"
        );
    }
    // The SBOM names the program, its runtime and its script by sha256.
    let prog = s.path().join("proj/bin").join(exe_name("a"));
    let sbom_path = s
        .path()
        .join("proj/bin")
        .join(format!("{}.cdx.json", exe_name("a")));
    let first = fs::read_to_string(&sbom_path).unwrap();
    let sbom: serde_json::Value = serde_json::from_str(&first).unwrap();
    assert_eq!(sbom["bomFormat"], "CycloneDX");
    assert_eq!(sbom["specVersion"], "1.5");
    let program_sha = hex::encode(sha2::Sha256::digest(fs::read(&prog).unwrap()));
    assert_eq!(
        sbom["metadata"]["component"]["hashes"][0]["content"],
        program_sha.as_str()
    );
    let runtime_sha = hex::encode(sha2::Sha256::digest(fs::read(RT).unwrap()));
    assert_eq!(
        sbom["components"][0]["hashes"][0]["content"],
        runtime_sha.as_str()
    );
    let script_sha = hex::encode(sha2::Sha256::digest(b"exhale 1\n"));
    assert_eq!(
        sbom["components"][1]["hashes"][0]["content"],
        script_sha.as_str()
    );
    assert!(
        !first.contains(&*s.path().to_string_lossy()),
        "a build path is in the SBOM"
    );
    // A rebuild that changes nothing writes the same SBOM.
    ok(&s.cc(&["build", "proj", "--sbom"]));
    assert_eq!(fs::read_to_string(&sbom_path).unwrap(), first);
}

/// #41: `targets` says, for each machine, the stage its programs are built
/// at, how CI proves them, and what a program there lacks.
#[test]
fn targets_say_the_stage_how_ci_proves_them_and_what_is_missing() {
    let s = Sandbox::new();
    let out = ok(&s.cc(&["targets"]));
    let line = |triple: &str| {
        out.lines()
            .find(|l| l.starts_with(triple))
            .unwrap_or_else(|| panic!("{triple}: {out}"))
            .to_string()
    };
    assert!(line("x86_64-unknown-linux-musl").contains("stage A  run on its own runner"));
    assert!(!line("aarch64-unknown-linux-musl").contains("no box"));
    assert!(line("armv7-unknown-linux-musleabihf").contains("built, not run"));
    assert!(line("aarch64-apple-darwin").contains("no box (E777, E763)"));
    assert!(
        out.lines().all(|l| l == l.trim_end()),
        "trailing spaces: {out:?}"
    );
    let json: Vec<serde_json::Value> = ok(&s.cc(&["--json", "targets"]))
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let freebsd = json
        .iter()
        .find(|t| t["triple"] == "x86_64-unknown-freebsd")
        .unwrap();
    assert_eq!(freebsd["stage"], "A");
    assert_eq!(freebsd["ci"], "built, not run");
    assert_eq!(freebsd["missing"], "no box (E777, E763)");
}

/// #37: `build --watch` builds, then builds again when the script is saved,
/// until it is stopped.
#[test]
fn watch_rebuilds_on_save() {
    let s = Sandbox::new();
    s.write("w.cig", "exhale \"one\"\n");
    let mut c = Command::new(env!("CARGO_BIN_EXE_cigcompile"));
    s.env(&mut c);
    let mut child = c
        .args(["build", "w.cig", "--watch"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("start the watch");
    let name = exe_name("w");
    // The program is run only when its file has changed: Windows will not
    // replace a program that is running, so a test that ran it in a loop
    // would hold the rebuild off.
    let last = std::cell::RefCell::new(None::<Vec<u8>>);
    let says = |want: &str| -> bool {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
        while std::time::Instant::now() < deadline {
            let now = fs::read(s.path().join(&name)).ok();
            if now.is_some() && now != *last.borrow() {
                *last.borrow_mut() = now;
                let o = s.prog(&name, &[]);
                if text(&o.stdout).trim() == want {
                    return true;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        false
    };
    let first = says("one");
    // A later time on a filesystem with coarse clocks, then the save.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    s.write("w.cig", "exhale \"two\"\n");
    let second = first && says("two");
    let _ = child.kill();
    let _ = child.wait();
    assert!(first, "the first build never ran");
    assert!(second, "the save was not rebuilt");
}

/// The files in a tar, by name: enough of ustar for an image's layout.
fn untar(t: &[u8]) -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut out = std::collections::BTreeMap::new();
    let mut at = 0;
    while at + 512 <= t.len() && t[at] != 0 {
        let h = &t[at..at + 512];
        let name = String::from_utf8_lossy(&h[..100])
            .trim_end_matches('\0')
            .to_string();
        let size = usize::from_str_radix(
            std::str::from_utf8(&h[124..135])
                .unwrap()
                .trim_matches(char::from(0)),
            8,
        )
        .unwrap();
        out.insert(name, t[at + 512..at + 512 + size].to_vec());
        at += 512 + size.div_ceil(512) * 512;
    }
    out
}

/// #41: `--emit oci` writes, beside a Linux program, an OCI image layout
/// whose one layer is the program itself: every digest names its blob, the
/// entrypoint is the program, and a rebuild writes the same bytes. Off
/// Linux it is refused, since the image is of a Linux program.
#[test]
fn emit_oci_writes_an_image_of_the_program() {
    use sha2::Digest;
    let s = Sandbox::new();
    s.write("hi.cig", "exhale \"hi\"\n");
    if !cfg!(target_os = "linux") {
        let r = s.cc(&["build", "hi.cig", "--emit", "oci"]);
        assert_eq!(r.status.code(), Some(3));
        assert!(
            text(&r.stderr).contains("an OCI image is of a Linux program"),
            "{}",
            text(&r.stderr)
        );
        return;
    }
    let out = ok(&s.cc(&["build", "hi.cig", "--emit", "oci"]));
    assert!(out.contains("hi.oci.tar"), "{out}");
    let image = fs::read(s.path().join("hi.oci.tar")).unwrap();
    let files = untar(&image);
    assert!(
        files.contains_key("oci-layout") && files.contains_key("index.json"),
        "{:?}",
        files.keys()
    );
    let blob =
        |d: &str| files[&format!("blobs/sha256/{}", d.trim_start_matches("sha256:"))].clone();
    let check = |d: &str| {
        let b = blob(d);
        assert_eq!(
            format!("sha256:{}", hex::encode(sha2::Sha256::digest(&b))),
            d
        );
        b
    };
    let index: serde_json::Value = serde_json::from_slice(&files["index.json"]).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&check(index["manifests"][0]["digest"].as_str().unwrap())).unwrap();
    let config: serde_json::Value =
        serde_json::from_slice(&check(manifest["config"]["digest"].as_str().unwrap())).unwrap();
    assert_eq!(config["os"], "linux");
    assert_eq!(config["config"]["Entrypoint"][0], "/hi");
    let layer = untar(&check(manifest["layers"][0]["digest"].as_str().unwrap()));
    assert_eq!(
        layer["hi"],
        fs::read(s.path().join("hi")).unwrap(),
        "the layer is the program"
    );
    // Docker's manifest names the same config and layer, and the tag.
    let docker: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    assert_eq!(docker[0]["RepoTags"][0], "localhost/hi:latest");
    for path in [
        docker[0]["Config"].as_str().unwrap(),
        docker[0]["Layers"][0].as_str().unwrap(),
    ] {
        assert!(files.contains_key(path), "{path} is not in the archive");
    }
    ok(&s.cc(&["build", "hi.cig", "--emit", "oci"]));
    assert_eq!(
        fs::read(s.path().join("hi.oci.tar")).unwrap(),
        image,
        "a rebuild wrote other bytes"
    );
}

/// #41: `--emit deb` writes, beside a Linux program, a `.deb` whose control
/// file names the package, version, architecture and maintainer and whose
/// `data.tar.gz` holds the program at `/usr/bin/<name>`. A rebuild writes
/// the same bytes. Off Linux it is refused, since the archive is of a Linux
/// program.
#[test]
fn emit_deb_writes_a_debian_package_of_the_program() {
    let s = Sandbox::new();
    s.write("hi.cig", "exhale \"hi\"\n");
    if !cfg!(target_os = "linux") {
        let r = s.cc(&["build", "hi.cig", "--emit", "deb"]);
        assert_eq!(r.status.code(), Some(3));
        assert!(
            text(&r.stderr).contains("a .deb is for a Linux target"),
            "{}",
            text(&r.stderr)
        );
        return;
    }
    let out = ok(&s.cc(&[
        "build",
        "hi.cig",
        "--emit",
        "deb",
        "--pkg-version",
        "1.2.3",
        "--pkg-maintainer",
        "Jay Taylor <jay@example.com>",
        "--pkg-description",
        "A hi program.",
    ]));
    let deb_name = "hi_1.2.3_amd64.deb";
    assert!(out.contains(deb_name), "{out}");
    let deb = fs::read(s.path().join(deb_name)).unwrap();
    assert_eq!(&deb[..8], b"!<arch>\n", "the file is not an ar archive");
    // Three members in order: debian-binary, control.tar.gz, data.tar.gz.
    let members = ar_members(&deb[8..]);
    assert_eq!(
        members.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
        vec!["debian-binary", "control.tar.gz", "data.tar.gz"],
    );
    assert_eq!(members[0].1, b"2.0\n");
    // `dpkg-deb --info` and `--contents` read the archive.
    let dpkg = std::process::Command::new("dpkg-deb")
        .args(["--info", s.path().join(deb_name).to_str().unwrap()])
        .output();
    let dpkg = match dpkg {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => panic!("dpkg-deb --info: {e}"),
    };
    let info = text(&dpkg.stdout);
    assert!(dpkg.status.success(), "dpkg-deb --info failed: {info}");
    for want in [
        "Package: hi",
        "Version: 1.2.3",
        "Architecture: amd64",
        "Maintainer: Jay Taylor <jay@example.com>",
        "Description: A hi program.",
    ] {
        assert!(info.contains(want), "{want:?} not in {info}");
    }
    let contents = std::process::Command::new("dpkg-deb")
        .args(["--contents", s.path().join(deb_name).to_str().unwrap()])
        .output()
        .unwrap();
    let c = text(&contents.stdout);
    assert!(contents.status.success(), "dpkg-deb --contents: {c}");
    assert!(c.contains("./usr/bin/hi"), "{c}");
    // Time is zero and owner root, so the listing shows 0/0 and 1970.
    assert!(c.contains("0/0"), "{c}");
    assert!(c.contains("1970-"), "{c}");
    // A rebuild writes the same bytes.
    ok(&s.cc(&[
        "build",
        "hi.cig",
        "--emit",
        "deb",
        "--pkg-version",
        "1.2.3",
        "--pkg-maintainer",
        "Jay Taylor <jay@example.com>",
        "--pkg-description",
        "A hi program.",
    ]));
    assert_eq!(
        fs::read(s.path().join(deb_name)).unwrap(),
        deb,
        "a rebuild wrote other bytes"
    );
}

/// The ar archive's members, by name: enough of BSD-style ar for a .deb's
/// three members (16-byte space-padded name, 60-byte header, data padded to
/// an even length with `\n`).
fn ar_members(mut b: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    while b.len() >= 60 {
        let name = std::str::from_utf8(&b[..16])
            .unwrap()
            .trim_end()
            .to_string();
        let size: usize = std::str::from_utf8(&b[48..58])
            .unwrap()
            .trim_end()
            .parse()
            .unwrap();
        b = &b[60..];
        out.push((name, b[..size].to_vec()));
        let step = size + size % 2;
        b = &b[step..];
    }
    out
}

/// #41: `cigcompile package --brew` writes a Homebrew formula pointing at
/// per-target release URLs with their sha256, and `--scoop` the same for
/// Windows. The output is stable: the same inputs write the same bytes.
#[test]
fn package_brew_and_scoop_name_each_release_assets_url_and_sha256() {
    let s = Sandbox::new();
    let sha_mac = "a".repeat(64);
    let sha_linux = "b".repeat(64);
    let sha_win = "c".repeat(64);
    let brew = ok(&s.cc(&[
        "package",
        "--brew",
        "--name",
        "tidy",
        "--version",
        "1.0.0",
        "--homepage",
        "https://example.com/tidy",
        "--license",
        "Apache-2.0",
        "--description",
        "Tidy downloads.",
        "--asset",
        &format!("aarch64-apple-darwin=https://example.com/tidy-1.0.0-macos-aarch64,{sha_mac}"),
        "--asset",
        &format!(
            "x86_64-unknown-linux-musl=https://example.com/tidy-1.0.0-linux-x86_64,{sha_linux}"
        ),
    ]));
    assert!(brew.starts_with("class Tidy < Formula\n"), "{brew}");
    assert!(brew.contains("desc \"Tidy downloads.\""));
    assert!(brew.contains("on_macos do"));
    assert!(brew.contains("on_linux do"));
    assert!(brew.contains(&format!("\"{sha_mac}\"")));
    assert!(brew.contains(&format!("\"{sha_linux}\"")));
    assert!(brew.contains("bin.install Dir[\"*\"].first => \"tidy\""));
    assert!(brew.contains("assert_predicate bin/\"tidy\", :exist?"));
    let scoop = ok(&s.cc(&[
        "package",
        "--scoop",
        "--name",
        "tidy",
        "--version",
        "1.0.0",
        "--homepage",
        "https://example.com/tidy",
        "--license",
        "Apache-2.0",
        "--description",
        "Tidy downloads.",
        "--asset",
        &format!(
            "x86_64-pc-windows-msvc=https://example.com/tidy-1.0.0-windows-x86_64.exe,{sha_win}"
        ),
    ]));
    let v: serde_json::Value = serde_json::from_str(&scoop).unwrap();
    assert_eq!(v["version"], "1.0.0");
    assert_eq!(v["license"], "Apache-2.0");
    assert_eq!(v["architecture"]["64bit"]["hash"], sha_win);
    assert_eq!(
        v["architecture"]["64bit"]["url"],
        "https://example.com/tidy-1.0.0-windows-x86_64.exe"
    );
    assert_eq!(v["bin"], "tidy.exe");
}

/// #41: `cigcompile package` reads the release from a JSON manifest, and
/// a `--asset` on the command line overrides one named in it. Golden: the
/// text is the same bytes on every run.
#[test]
fn package_reads_a_manifest_and_overrides_are_applied() {
    let s = Sandbox::new();
    let sha = "a".repeat(64);
    let sha_other = "9".repeat(64);
    let manifest = serde_json::json!({
        "name": "tidy",
        "version": "1.0.0",
        "homepage": "https://example.com/tidy",
        "license": "Apache-2.0",
        "description": "Tidy downloads.",
        "assets": [{
            "target": "aarch64-apple-darwin",
            "url": "https://example.com/tidy-1.0.0-macos-aarch64",
            "sha256": sha,
        }],
    });
    s.write("release.json", &manifest.to_string());
    let out1 = ok(&s.cc(&[
        "package",
        "--brew",
        "--manifest",
        "release.json",
        "--out",
        "tidy.rb",
    ]));
    assert!(out1.contains("wrote"), "{out1}");
    let a = fs::read_to_string(s.path().join("tidy.rb")).unwrap();
    assert!(a.contains("class Tidy < Formula"));
    assert!(a.contains(&format!("\"{sha}\"")));
    // A --asset on the command line replaces the manifest's for that target.
    let out2 = ok(&s.cc(&[
        "package",
        "--brew",
        "--manifest",
        "release.json",
        "--asset",
        &format!("aarch64-apple-darwin=https://example.com/other,{sha_other}"),
        "--out",
        "tidy2.rb",
    ]));
    assert!(out2.contains("wrote"), "{out2}");
    let b = fs::read_to_string(s.path().join("tidy2.rb")).unwrap();
    assert!(b.contains(&format!("\"{sha_other}\"")));
    assert!(!b.contains(&format!("\"{sha}\"")));
    assert!(b.contains("https://example.com/other"));
    // A rerun with the same inputs writes the same bytes.
    ok(&s.cc(&[
        "package",
        "--brew",
        "--manifest",
        "release.json",
        "--out",
        "tidy3.rb",
    ]));
    assert_eq!(
        fs::read_to_string(s.path().join("tidy3.rb")).unwrap(),
        a,
        "the formula was not reproducible"
    );
}

/// A release with no macOS or Linux asset cannot have a Homebrew formula;
/// one with no Windows asset cannot have a Scoop manifest. The command says
/// which.
#[test]
fn package_refuses_a_release_that_has_no_asset_for_the_tool() {
    let s = Sandbox::new();
    let sha = "a".repeat(64);
    let brew = s.cc(&[
        "package",
        "--brew",
        "--name",
        "tidy",
        "--version",
        "1.0.0",
        "--homepage",
        "https://example.com",
        "--license",
        "Apache-2.0",
        "--description",
        "A thing.",
        "--asset",
        &format!("x86_64-pc-windows-msvc=https://example.com/windows,{sha}"),
    ]);
    assert_eq!(brew.status.code(), Some(3));
    assert!(
        text(&brew.stderr).contains("macOS or Linux"),
        "{}",
        text(&brew.stderr)
    );
    let scoop = s.cc(&[
        "package",
        "--scoop",
        "--name",
        "tidy",
        "--version",
        "1.0.0",
        "--homepage",
        "https://example.com",
        "--license",
        "Apache-2.0",
        "--description",
        "A thing.",
        "--asset",
        &format!("aarch64-apple-darwin=https://example.com/macos,{sha}"),
    ]);
    assert_eq!(scoop.status.code(), Some(3));
    assert!(
        text(&scoop.stderr).contains("Windows"),
        "{}",
        text(&scoop.stderr)
    );
    // Without --brew or --scoop, it says to pick one.
    let neither = s.cc(&["package", "--name", "x"]);
    assert_eq!(neither.status.code(), Some(3));
    assert!(
        text(&neither.stderr).contains("--brew or --scoop"),
        "{}",
        text(&neither.stderr)
    );
    // A malformed --asset is refused with a clear message.
    let bad = s.cc(&[
        "package",
        "--brew",
        "--name",
        "x",
        "--version",
        "1.0.0",
        "--homepage",
        "https://example.com",
        "--license",
        "Apache-2.0",
        "--description",
        "x",
        "--asset",
        "aarch64-apple-darwin=https://example.com,short",
    ]);
    assert_eq!(bad.status.code(), Some(3));
    assert!(
        text(&bad.stderr).contains("64 hex"),
        "{}",
        text(&bad.stderr)
    );
}
