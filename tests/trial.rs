// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! `cigcompile trial <new>` (#15, ADR 0003): the new cigcompile is found through the installer,
//! both build the project and run its tests, and what differs is said first. The installer here
//! is a script that answers `which cigcompile --path`; the "new" compiler is a script that wraps
//! this build and changes one line, so the difference is known. Every child runs under `timeout`
//! (or `gtimeout`) where one is on PATH, with a scratch HOME, CIGSCRIPT_HOME and XDG folders.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const RT: &str = env!("CARGO_BIN_EXE_cig-runtime");
const CC: &str = env!("CARGO_BIN_EXE_cigcompile");

struct Sandbox {
    dir: tempfile::TempDir,
}

/// `program` under a hard limit of `secs` seconds, through `timeout` (GNU coreutils, the BSDs) or
/// `gtimeout` (coreutils on a Mac) where one is on PATH; on a Mac with neither it runs as it is,
/// under the CI job's own limit.
fn guarded(secs: u32, program: &str) -> Command {
    let limiter = std::env::var_os("PATH").and_then(|path| {
        ["timeout", "gtimeout"].iter().find_map(|name| {
            std::env::split_paths(&path)
                .map(|dir| dir.join(name))
                .find(|p| p.is_file())
        })
    });
    match limiter {
        Some(t) => {
            let mut c = Command::new(t);
            c.arg(secs.to_string()).arg(program);
            c
        }
        None => Command::new(program),
    }
}

impl Sandbox {
    fn new() -> Self {
        let s = Self {
            dir: tempfile::tempdir().expect("tempdir"),
        };
        s.write("proj/a.cig", "exhale 1\n");
        s.write("proj/b.cig", "stick x = 2\nexhale x\n");
        s
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let p = self.path().join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, content).unwrap();
        p
    }

    fn script(&self, name: &str, body: &str) -> PathBuf {
        let p = self.write(name, &format!("#!/bin/sh\n{body}\n"));
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    /// A `ciginstaller` that knows one carton, `1.1`, and says where its cigcompile is.
    fn installer(&self, new_cc: &Path) {
        self.script(
            "tools/ciginstaller",
            &format!(
                "if [ \"$1\" = which ] && [ \"$2\" = cigcompile ] && [ \"$3\" = --path ] && [ \"$CIGI_CARTON\" = 1.1 ]; then echo '{}'; exit 0; fi\n\
                 echo 'E1840: carton '\"$CIGI_CARTON\"' is not installed' >&2; exit 2",
                new_cc.display()
            ),
        );
    }

    /// A cigcompile that is this build with `sed` applied to what `test` and `build --help` say.
    fn fake_cc(&self, test_sed: &str, help_sed: &str) -> PathBuf {
        self.script(
            "new/cigcompile",
            &format!(
                "case \"$1\" in\n\
                 --version) \"{CC}\" \"$@\" | sed 's/^cigcompile [^ ]*/cigcompile 1.1.0/';;\n\
                 test) \"{CC}\" \"$@\" | sed '{test_sed}';;\n\
                 build) if [ \"$2\" = --help ]; then \"{CC}\" \"$@\" | sed '{help_sed}'; else exec \"{CC}\" \"$@\"; fi;;\n\
                 *) exec \"{CC}\" \"$@\";;\n\
                 esac"
            ),
        )
    }

    fn trial(&self, path_dirs: &str, args: &[&str]) -> Output {
        let home = self.path().join("home");
        fs::create_dir_all(&home).unwrap();
        let mut c = guarded(300, CC);
        c.arg("trial")
            .args(args)
            .current_dir(self.path().join("proj"))
            .env_clear()
            .env("PATH", path_dirs)
            .env("HOME", &home)
            .env("CIGSCRIPT_HOME", home.join(".cigscript"))
            .env("XDG_CONFIG_HOME", home.join("config"))
            .env("XDG_DATA_HOME", home.join("data"))
            .env("XDG_CACHE_HOME", home.join("cache"))
            .env("XDG_STATE_HOME", home.join("state"))
            .env("TMPDIR", self.path())
            .env("NO_COLOR", "1")
            .env("CIG_SEAL_KEY", "file")
            .env("CIG_DOCTOR", "0")
            .env("CIG_REPORTS", "never")
            .env("CIG_INTERNAL_TRACKER_DIR", self.path().join("tracker"))
            .env("CIG_UPDATE_MANIFEST", self.path().join("no-releases.json"))
            .env("CIGCOMPILE_RUNTIME", RT)
            .env("CIGCOMPILE_OFFLINE", "1");
        c.output().expect("run cigcompile trial")
    }

    fn with_tools(&self) -> String {
        format!("{}:/usr/bin:/bin", self.path().join("tools").display())
    }
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

#[test]
fn a_new_compiler_that_changes_one_output_is_shown_first() {
    let s = Sandbox::new();
    let cc = s.fake_cc("s/^same     a.cig.*/same     a.cig (changed by 1.1)/", "");
    s.installer(&cc);
    let o = s.trial(&s.with_tools(), &["1.1"]);
    let (out, err) = (text(&o.stdout), text(&o.stderr));
    assert_eq!(o.status.code(), Some(1), "{out}\n{err}");
    let diff = out.find("DIFFERENCES").expect(&out);
    let line = out.find("changed by 1.1").expect(&out);
    let summary = out.find("nothing was pinned").expect(&out);
    assert!(
        diff < line && line < summary,
        "differences come first: {out}"
    );
    assert!(out.contains("--- test cigcompile test"), "{out}");
    assert!(
        out.contains("to trust it anyway: ciginstaller pin carton 1.1"),
        "{out}"
    );
    // the plans are the same: only the test output differs
    assert!(!out.contains("--- plan"), "{out}");
    // the user's folder was not built in
    assert!(!s.path().join("proj/bin").exists());
}

#[test]
fn a_same_answering_compiler_exits_zero() {
    let s = Sandbox::new();
    let cc = s.fake_cc("", "");
    s.installer(&cc);
    let o = s.trial(&s.with_tools(), &["1.1"]);
    let (out, err) = (text(&o.stdout), text(&o.stderr));
    assert_eq!(o.status.code(), Some(0), "{out}\n{err}");
    assert!(out.contains("no differences in 3 checks"), "{out}");
    assert!(out.contains("this cigcompile"), "{out}");
    assert!(
        out.contains("to trust it: ciginstaller pin carton 1.1"),
        "{out}"
    );
    let j = s.trial(&s.with_tools(), &["1.1", "--project", "../proj", "--json"]);
    assert_eq!(j.status.code(), Some(0), "{}", text(&j.stderr));
    let v: serde_json::Value = serde_json::from_slice(&j.stdout).expect("json");
    assert_eq!(v["same"], true, "{v}");
    assert_eq!(v["checks"], 3, "{v}");
}

#[test]
fn a_changed_codegen_flag_skips_the_differential_and_says_why() {
    let s = Sandbox::new();
    let cc = s.fake_cc("", "s/optimisation level/optimization level/");
    s.installer(&cc);
    s.write("suite/c.cig", "exhale 3\n");
    let o = s.trial(&s.with_tools(), &["1.1", "--differential", "../suite"]);
    let (out, err) = (text(&o.stdout), text(&o.stderr));
    assert_eq!(o.status.code(), Some(0), "{out}\n{err}");
    assert!(out.contains("differential skipped: -O differs"), "{out}");
    // with the flags the same, the corpus runs for real and counts as checks
    let same = s.fake_cc("", "");
    s.installer(&same);
    let o = s.trial(&s.with_tools(), &["1.1", "--differential", "../suite"]);
    let out = text(&o.stdout);
    assert_eq!(o.status.code(), Some(0), "{out}\n{}", text(&o.stderr));
    assert!(out.contains("no differences in 4 checks"), "{out}");
}

#[test]
fn a_differential_difference_is_found() {
    let s = Sandbox::new();
    // the new compiler's programs are the real ones, but its `build` of the corpus is made to
    // fail for c.cig, so the program is missing and the run differs
    let cc = s.script(
        "new/cigcompile",
        &format!(
            "if [ \"$1\" = build ] && [ -f c.cig ] && [ ! -f a.cig ]; then echo nope >&2; exit 2; fi\nexec \"{CC}\" \"$@\""
        ),
    );
    s.installer(&cc);
    s.write("suite/c.cig", "exhale 3\n");
    let o = s.trial(&s.with_tools(), &["1.1", "--differential", "../suite"]);
    let out = text(&o.stdout);
    assert_eq!(o.status.code(), Some(1), "{out}\n{}", text(&o.stderr));
    assert!(out.contains("--- differential c.cig"), "{out}");
}

#[test]
fn a_version_the_installer_does_not_have_is_a_clear_error() {
    let s = Sandbox::new();
    let cc = s.fake_cc("", "");
    s.installer(&cc);
    let o = s.trial(&s.with_tools(), &["9.9"]);
    let err = text(&o.stderr);
    assert_eq!(o.status.code(), Some(3), "{err}");
    assert!(err.contains("no cigcompile for carton 9.9"), "{err}");
    assert!(
        err.contains("ciginstaller install carton --carton 9.9"),
        "{err}"
    );
    assert!(!err.contains("panicked"), "{err}");
}

#[test]
fn no_installer_is_exit_127_with_the_way_to_get_one() {
    let s = Sandbox::new();
    let o = s.trial("/usr/bin:/bin", &["1.1"]);
    let err = text(&o.stderr);
    assert_eq!(o.status.code(), Some(127), "{err}");
    assert!(err.contains("get ciginstaller: "), "{err}");
    assert!(!err.contains("panicked"), "{err}");
}
