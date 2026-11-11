// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! `.cigar` files and `--strength` (cigar-script#7): `cigcompile build
//! x.cigar` checks the file with the `cigarscript` crate and compiles what it
//! lowers to; a `.cig` script goes through as before.
//!
//! Every test runs in a scratch HOME with its own CIGSCRIPT_HOME and XDG
//! folders, under a hard timeout.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

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

    /// The person's folders: nothing of theirs is touched.
    fn home(&self) -> PathBuf {
        self.path().join("person")
    }

    fn write(&self, name: &str, content: &str) {
        let p = self.path().join("work").join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, content).unwrap();
    }

    fn work(&self) -> PathBuf {
        let w = self.path().join("work");
        fs::create_dir_all(&w).unwrap();
        w
    }

    fn env(&self, c: &mut Command) {
        let h = self.home();
        c.current_dir(self.work())
            .env("HOME", &h)
            .env("USERPROFILE", &h)
            .env("XDG_STATE_HOME", h.join("state"))
            .env("XDG_CONFIG_HOME", h.join("config"))
            .env("XDG_CACHE_HOME", h.join("cache"))
            .env("XDG_DATA_HOME", h.join("data"))
            .env("CIGSCRIPT_HOME", self.path().join("cig-home"))
            .env("NO_COLOR", "1")
            .env("CIG_SEAL_KEY", "file")
            .env("CIG_DOCTOR", "0")
            .env("CIG_REPORTS", "never")
            .env("CIG_INTERNAL_TRACKER_DIR", self.path().join("tracker"))
            .env("CIG_UPDATE_MANIFEST", self.path().join("no-releases.json"))
            .env("CIGCOMPILE_RUNTIME", RT)
            .env("CIGCOMPILE_OFFLINE", "1")
            .env_remove("CI")
            .stdin(Stdio::null());
    }

    /// `cigcompile args`, under a hard timeout.
    fn cc(&self, args: &[&str]) -> Output {
        let mut c = guarded(120, env!("CARGO_BIN_EXE_cigcompile"));
        self.env(&mut c);
        c.args(args).output().expect("run cigcompile")
    }

    /// `cig args` (this build's runtime as the interpreter).
    fn cig(&self, args: &[&str]) -> Output {
        let mut c = guarded(120, RT);
        self.env(&mut c);
        c.args(args).output().expect("run cig")
    }

    fn prog(&self, name: &str) -> Output {
        let mut c = guarded(
            120,
            self.work()
                .join(format!("{name}{}", std::env::consts::EXE_SUFFIX)),
        );
        self.env(&mut c);
        // ETXTBSY: another test's fork can hold a program just written.
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
}

/// `program` under a hard limit of `secs` seconds, through `timeout` (GNU coreutils, the BSDs) or
/// `gtimeout` (coreutils on a Mac) where one is on PATH; on Windows, and on a Mac with neither, it
/// runs as it is, under the CI job's own limit.
fn guarded(secs: u32, program: impl AsRef<std::ffi::OsStr>) -> Command {
    let limiter = if cfg!(windows) {
        None
    } else {
        std::env::var_os("PATH").and_then(|path| {
            ["timeout", "gtimeout"].iter().find_map(|name| {
                std::env::split_paths(&path)
                    .map(|dir| dir.join(name))
                    .find(|p| p.is_file())
            })
        })
    };
    match limiter {
        Some(t) => {
            let mut c = Command::new(t);
            c.arg(secs.to_string()).arg(program);
            c
        }
        None => Command::new(program),
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

const MODULE: &str = "strength medium\n\npub pull double(n: Int) -> Int {\n  snuff n * 2\n}\n";

#[test]
fn a_mild_cigar_file_builds_and_runs_as_cig_runs_it() {
    let s = Sandbox::new();
    let script = "stick name = args.get(0, \"world\")\nexhale \"hello, ${name}\"\n";
    s.write("hello.cigar", script);
    s.write("hello.cig", script);
    ok(&s.cc(&["build", "hello.cigar"]));
    let program = s.prog("hello");
    let reference = s.cig(&["run", "hello.cig"]);
    assert_eq!(ok(&program), ok(&reference));
    assert_eq!(text(&program.stdout), "hello, world\n");
}

#[test]
fn a_cig_script_builds_as_it_always_did() {
    let s = Sandbox::new();
    s.write("a.cig", "exhale 1\n");
    // `--strength` is for `.cigar`: a `.cig` script is always mild.
    ok(&s.cc(&["build", "a.cig", "--strength", "full"]));
    assert_eq!(text(&s.prog("a").stdout), "1\n");
}

#[test]
fn a_folder_builds_its_cigar_files_too() {
    let s = Sandbox::new();
    s.write("one.cig", "exhale 1\n");
    s.write("two.cigar", "exhale 2\n");
    ok(&s.cc(&["build", "."]));
    assert_eq!(text(&s.prog("bin/one").stdout), "1\n");
    assert_eq!(text(&s.prog("bin/two").stdout), "2\n");
}

#[test]
fn a_medium_file_builds_at_its_strength_and_runs() {
    let s = Sandbox::new();
    s.write(
        "m.cigar",
        "strength medium\n\nrecord P { n: Int }\nstick p = P { n: 3 }\nexhale p.n + 1\n",
    );
    ok(&s.cc(&["build", "m.cigar"]));
    assert_eq!(text(&s.prog("m").stdout), "4\n");
}

#[test]
fn turning_down_is_checked_and_turning_up_is_not() {
    let s = Sandbox::new();
    s.write(
        "m.cigar",
        "strength medium\n\nrecord P { n: Int }\nexhale 1\n",
    );
    let down = s.cc(&["build", "m.cigar", "--strength", "mild"]);
    assert_eq!(down.status.code(), Some(2), "{}", text(&down.stderr));
    let err = text(&down.stderr);
    assert!(err.contains("E1101") && err.contains("records"), "{err}");
    assert!(err.contains("m.cigar:3"), "with its place: {err}");
    // A mild file built at medium or full is the same program.
    s.write("up.cigar", "exhale 7\n");
    for at in ["mild", "medium"] {
        ok(&s.cc(&["build", "up.cigar", "--strength", at]));
        assert_eq!(text(&s.prog("up").stdout), "7\n", "at {at}");
    }
    let bad = s.cc(&["build", "up.cigar", "--strength", "strong"]);
    assert_eq!(bad.status.code(), Some(3));
    assert!(text(&bad.stderr).contains("--strength is `strong`"));
}

#[test]
fn a_check_only_build_of_a_cigar_file_writes_nothing() {
    let s = Sandbox::new();
    s.write("c.cigar", "exhale 1\n");
    let out = ok(&s.cc(&["build", "c.cigar", "--check"]));
    assert!(out.contains("checked c.cigar"), "{out}");
    assert!(!s.work().join("c").exists());
}

#[test]
fn a_mild_file_calls_a_medium_library_and_the_program_runs() {
    let s = Sandbox::new();
    s.write("tools/plan.cigar", MODULE);
    s.write("main.cigar", "use tools.plan\n\nexhale plan.double(21)\n");
    ok(&s.cc(&["build", "main.cigar"]));
    assert_eq!(text(&s.prog("main").stdout), "42\n");
}

#[test]
fn a_wrong_typed_value_at_the_boundary_stops_the_program_with_e1210() {
    let s = Sandbox::new();
    s.write("tools/plan.cigar", MODULE);
    s.write(
        "main.cigar",
        "use tools.plan\n\nstick vals = [\"twelve\", 5]\nexhale plan.double(vals[0])\n",
    );
    // It checks and builds: only the run can know the value.
    ok(&s.cc(&["build", "main.cigar"]));
    let ran = s.prog("main");
    assert!(!ran.status.success());
    let err = text(&ran.stderr);
    assert!(err.contains("E1210"), "{err}");
    assert!(
        err.contains("n should be Int"),
        "names the parameter: {err}"
    );
    assert!(text(&ran.stdout).is_empty());
}

fn full_file(s: &Sandbox) {
    fs::create_dir_all(s.work().join(".git")).unwrap();
    s.write("f.cigar", "strength full\n\nexhale 1\n");
}

#[test]
fn full_asks_nobody_in_a_run_nobody_can_answer_and_says_so_once() {
    let s = Sandbox::new();
    full_file(&s);
    let out = s.cc(&["build", "f.cigar"]);
    ok(&out);
    let err = text(&out.stderr);
    assert_eq!(err.matches("not interactive").count(), 1, "{err}");
    assert_eq!(text(&s.prog("f").stdout), "1\n");
    // Nothing is remembered anywhere: not in the person's state, not in the
    // project.
    assert!(!s.home().join("state/cigar/full-warning").exists());
    let mut inside: Vec<String> = fs::read_dir(s.work())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    inside.sort();
    assert!(inside.iter().all(|n| !n.contains("warning")), "{inside:?}");
}

#[test]
fn full_says_nothing_when_the_person_turned_the_notice_off() {
    let s = Sandbox::new();
    full_file(&s);
    fs::create_dir_all(s.home().join("config/cigar")).unwrap();
    fs::write(
        s.home().join("config/cigar/config"),
        "strength_full_warning = off\n",
    )
    .unwrap();
    let out = s.cc(&["build", "f.cigar"]);
    ok(&out);
    assert!(!text(&out.stderr).contains("not interactive"));
}

#[test]
fn full_says_nothing_for_a_project_already_answered_and_a_check_never_asks() {
    let s = Sandbox::new();
    full_file(&s);
    // A check is not a commitment: no question, no note.
    let out = s.cc(&["build", "f.cigar", "--check"]);
    ok(&out);
    assert!(!text(&out.stderr).contains("not interactive"));
    // A project answered before (the person's state, keyed by the root).
    let root = fs::canonicalize(s.work()).unwrap();
    fs::create_dir_all(s.home().join("state/cigar")).unwrap();
    fs::write(
        s.home().join("state/cigar/full-warning"),
        format!("{}\n", root.display()),
    )
    .unwrap();
    let out = s.cc(&["build", "f.cigar"]);
    ok(&out);
    assert!(!text(&out.stderr).contains("not interactive"));
}

/// Every CigScript script in a checkout of the CigScript that cigcompile
/// pins, named `.cigar` and built as it is: it checks and builds exactly when
/// `cig check` passes it (the programs' output is the differential suite's,
/// `DIFF_AS=cigar`). Skips, with a note, without `CIGSCRIPT_SRC`.
#[test]
fn every_cigscript_script_builds_as_mild_unchanged() {
    let Some(src) = std::env::var_os("CIGSCRIPT_SRC").map(PathBuf::from) else {
        eprintln!("note: skipping; CIGSCRIPT_SRC does not point at a CigScript checkout");
        return;
    };
    let mut scripts = Vec::new();
    for sub in ["examples", "tests/scripts"] {
        let Ok(rd) = fs::read_dir(src.join(sub)) else {
            continue;
        };
        scripts.extend(
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "cig")),
        );
    }
    scripts.sort();
    if scripts.is_empty() {
        eprintln!("note: skipping; the checkout has no scripts at examples/ or tests/scripts/");
        return;
    }
    let s = Sandbox::new();
    let mut wrong = Vec::new();
    for (i, f) in scripts.iter().enumerate() {
        let body = fs::read_to_string(f).unwrap();
        let (as_cig, as_cigar) = (format!("s{i}.cig"), format!("s{i}.cigar"));
        s.write(&as_cig, &body);
        s.write(&as_cigar, &body);
        let want = s.cig(&["check", &as_cig]).status.success();
        let got = s.cc(&["build", &as_cigar, "--check"]).status.success();
        if want != got {
            wrong.push(format!(
                "{}: cig check {want}, cigcompile build --check {got}",
                f.display()
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
