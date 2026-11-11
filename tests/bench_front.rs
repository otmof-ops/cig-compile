// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! `cigcompile bench` on the family's harness (#68, otmof-ops/cig-script#179):
//! its old flags still work, the program set is the pinned cig's, both sides of
//! a compiled-against-interpreted case run one runtime, and a build's phases
//! go to `CIG_TIMINGS_FD`.
//!
//! Every child process runs in a scratch home under a hard timeout.

use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;

const RT: &str = env!("CARGO_BIN_EXE_cig-runtime");
const CC: &str = env!("CARGO_BIN_EXE_cigcompile");

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run_for(mut cmd: Command, secs: u64) -> Output {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("cannot start the command");
    let mut out = child.stdout.take().unwrap();
    let mut err = child.stderr.take().unwrap();
    let t1 = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = std::io::Read::read_to_end(&mut out, &mut v);
        v
    });
    let t2 = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = std::io::Read::read_to_end(&mut err, &mut v);
        v
    });
    let status = match child.wait_timeout(Duration::from_secs(secs)).unwrap() {
        Some(s) => s,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            panic!("the command ran over {secs} s and was killed");
        }
    };
    Output {
        status,
        stdout: t1.join().unwrap(),
        stderr: t2.join().unwrap(),
    }
}

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

    /// The scratch environment: nothing of the developer's home, the pinned
    /// runtime of this build, never the network.
    fn env(&self, c: &mut Command) {
        c.current_dir(self.path())
            .env("HOME", self.path().join("home"))
            .env("CIGSCRIPT_HOME", self.path().join("home/cigscript"))
            .env("XDG_CONFIG_HOME", self.path().join("home/xdg-config"))
            .env("XDG_DATA_HOME", self.path().join("home/xdg-data"))
            .env("XDG_CACHE_HOME", self.path().join("home/xdg-cache"))
            .env("XDG_STATE_HOME", self.path().join("home/xdg-state"))
            .env("NO_COLOR", "1")
            .env("CIG_SEAL_KEY", "file")
            .env("CIG_DOCTOR", "0")
            .env("CIG_REPORTS", "never")
            .env("CIG_INTERNAL_TRACKER_DIR", self.path().join("tracker"))
            .env("CIG_UPDATE_MANIFEST", self.path().join("no-releases.json"))
            .env("CIGCOMPILE_RUNTIME", RT)
            .env("CIGCOMPILE_OFFLINE", "1")
            .env_remove("CIG_TIMINGS_FD")
            .env_remove("CIG_STATS_FD")
            .env_remove("CI");
    }

    fn cc(&self, args: &[&str]) -> Output {
        let mut c = Command::new(CC);
        self.env(&mut c);
        c.args(args);
        run_for(c, 300)
    }
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

// ----- cigcompile_bench_flags_still_work ---------------------------------------

/// The flags `cigcompile bench --help` listed before the front end (#68).
const OLD_FLAGS: &[&str] = &[
    "--runs",
    "--ref",
    "--markdown",
    "--set",
    "--runtime",
    "--budgets",
    "--orchestrate",
    "--alloc",
    "--only",
    "--against",
    "--current",
    "--trend",
];

/// The flags it gains.
const NEW_FLAGS: &[&str] = &[
    "--watch",
    "--timings",
    "--stats",
    "--syscalls",
    "--manifest",
    "--group",
    "--cig",
];

#[test]
fn cigcompile_bench_flags_still_work() {
    let s = Sandbox::new();
    let help = text(&s.cc(&["bench", "--help"]).stdout);
    for f in OLD_FLAGS.iter().chain(NEW_FLAGS) {
        assert!(help.contains(f), "`bench --help` lost {f}:\n{help}");
    }

    s.write("set/a.cig", "exhale 1\n");
    s.write("set/a.expect", "1\n");
    s.write("set/b.cig", "exhale 2\n");
    s.write("set/b.expect", "2\n");
    s.write("orch.cig", "exhale 3\n");
    // A cig that would fail if any of these reached it: the old flags are the
    // set harness's, they never go to `cig bench`.
    let nowhere = s.path().join("no-such-cig");
    let nowhere = nowhere.to_str().unwrap();

    // --runs, --set: the table, both programs, nothing WRONG.
    let o = s.cc(&["bench", "--set", "set", "--runs", "2", "--cig", nowhere]);
    let out = text(&o.stdout);
    assert_eq!(o.status.code(), Some(0), "{out}{}", text(&o.stderr));
    assert!(out.lines().any(|l| l.starts_with("a ")), "{out}");
    assert!(out.lines().any(|l| l.starts_with("b ")), "{out}");

    // --markdown, and --json (cig's global flag): the same data.
    let out = text(
        &s.cc(&["bench", "--set", "set", "--runs", "2", "--markdown"])
            .stdout,
    );
    assert!(
        out.contains("| benchmark | stresses |") && out.contains("| `a` |"),
        "{out}"
    );
    let o = s.cc(&["--json", "bench", "--set", "set", "--runs", "2"]);
    let v: serde_json::Value = serde_json::from_str(text(&o.stdout).trim()).unwrap();
    assert_eq!(v["programs"].as_array().unwrap().len(), 2, "{v}");
    assert!(
        v["programs"][0]["interpreted"]["median_ms"].is_number(),
        "{v}"
    );

    // --only: the named one alone; a name that matches nothing is a usage error.
    let out = text(
        &s.cc(&["bench", "--set", "set", "--runs", "2", "--only", "b"])
            .stdout,
    );
    assert!(
        out.lines().any(|l| l.starts_with("b ")) && !out.lines().any(|l| l.starts_with("a ")),
        "{out}"
    );
    assert_eq!(
        s.cc(&["bench", "--set", "set", "--only", "nope"])
            .status
            .code(),
        Some(3)
    );

    // --runtime, --alloc, --orchestrate, --budgets, --ref.
    let o = s.cc(&["bench", "--set", "set", "--runs", "2", "--runtime", RT]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
    let o = s.cc(&[
        "--json", "bench", "--set", "set", "--runs", "2", "--alloc", RT,
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
    let o = s.cc(&[
        "bench",
        "--set",
        "set",
        "--runs",
        "2",
        "--orchestrate",
        "orch.cig",
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
    assert!(
        text(&o.stdout).contains("orchestrate"),
        "{}",
        text(&o.stdout)
    );
    let o = s.cc(&["bench", "--set", "set", "--runs", "2", "--budgets"]);
    assert!(
        matches!(o.status.code(), Some(0) | Some(1)),
        "{}",
        text(&o.stderr)
    );
    assert!(text(&o.stdout).contains("budget"), "{}", text(&o.stdout));
    let o = s.cc(&["bench", "--set", "set", "--runs", "1", "--ref"]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));

    // --against with --current (and a saved result of its own harness): the
    // regression gate on two saved runs of the same programs passes.
    let saved = s
        .cc(&["--json", "bench", "--set", "set", "--runs", "3"])
        .stdout;
    fs::write(s.path().join("main.json"), &saved).unwrap();
    fs::write(s.path().join("pr.json"), &saved).unwrap();
    let o = s.cc(&["bench", "--against", "main.json", "--current", "pr.json"]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}{}",
        text(&o.stdout),
        text(&o.stderr)
    );
}

/// The new flags go to `cig bench`; the old ones never do. A stand-in `cig`
/// records how it was called, and what the PATH looked like to its cases.
#[cfg(unix)]
#[test]
fn the_new_flags_are_handed_to_cig_bench() {
    use std::os::unix::fs::PermissionsExt;
    let s = Sandbox::new();
    let record = s.path().join("called.txt");
    let stand_in = s.path().join("stand-in-cig");
    fs::write(
        &stand_in,
        format!(
            "#!/bin/sh\nif [ \"$1\" = bench ] && [ \"$2\" = --help ]; then exit 0; fi\n{{ echo \"$@\"; echo \"$PATH\"; readlink -f \"$(command -v cig)\"; }} > '{}'\nexit 0\n",
            record.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&stand_in, fs::Permissions::from_mode(0o755)).unwrap();
    let cig = stand_in.to_str().unwrap();

    let o = s.cc(&[
        "bench",
        "--cig",
        cig,
        "--timings",
        "--runs",
        "3",
        "--group",
        "programs",
        "--markdown",
        "--against",
        "origin/main",
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
    let called = fs::read_to_string(&record).unwrap();
    let mut lines = called.lines();
    assert_eq!(
        lines.next().unwrap(),
        "bench bench/bench.toml --samples 3 --group programs --markdown --against origin/main --timings"
    );
    // While it runs, `cig` on the PATH is the runtime given, and cigcompile's
    // own folder is on the PATH too.
    let path = lines.next().unwrap();
    assert_eq!(
        Path::new(lines.next().unwrap()),
        fs::canonicalize(&stand_in).unwrap()
    );
    let ours = Path::new(CC).parent().unwrap().to_str().unwrap();
    assert!(path.split(':').any(|d| d == ours), "{path}");

    // `--against` of a saved result is the old gate and never reaches cig.
    fs::remove_file(&record).unwrap();
    s.write("main.json", "{}");
    let _ = s.cc(&[
        "bench",
        "--cig",
        cig,
        "--against",
        "main.json",
        "--current",
        "main.json",
    ]);
    assert!(!record.exists());

    // A cig without `bench` is said, with what to do instead, and exits 3.
    let old = s.path().join("old-cig");
    fs::write(&old, "#!/bin/sh\nexit 2\n").unwrap();
    fs::set_permissions(&old, fs::Permissions::from_mode(0o755)).unwrap();
    let o = s.cc(&["bench", "--cig", old.to_str().unwrap(), "--timings"]);
    assert_eq!(o.status.code(), Some(3));
    assert!(
        text(&o.stderr).contains("has no `bench` command"),
        "{}",
        text(&o.stderr)
    );
}

// ----- set_comes_from_the_pinned_cig -------------------------------------------

/// `bench/set.sha256`: (sha256, file name) of the pinned cig's compute set.
fn set_sums() -> Vec<(String, String)> {
    fs::read_to_string(root().join("bench/set.sha256"))
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let (h, n) = l.split_once(' ').expect("sha256 then a name");
            (h.to_string(), n.trim().to_string())
        })
        .collect()
}

fn set_sh(s: &Sandbox, set_dir: &Path, sums: Option<&Path>) -> Output {
    let mut c = Command::new("sh");
    s.env(&mut c);
    c.arg(root().join("bench/set.sh")).arg("out");
    c.env("CIG_BENCH_SET", set_dir).env_remove("CIGSCRIPT_SRC");
    if let Some(p) = sums {
        c.env("CIG_BENCH_SUMS", p);
    }
    run_for(c, 120)
}

#[test]
fn set_comes_from_the_pinned_cig() {
    // The manifest and set.sha256 name the same programs: every one the
    // `programs` group runs is in the sums, as source and as expected output.
    let sums = set_sums();
    let names: Vec<&str> = sums.iter().map(|(_, n)| n.as_str()).collect();
    let manifest = fs::read_to_string(root().join("bench/bench.toml")).unwrap();
    let mut programs = 0;
    for line in manifest.lines() {
        if let Some(n) = line
            .strip_prefix("name  = \"compiled: ")
            .and_then(|r| r.strip_suffix('"'))
            .filter(|n| !n.contains(','))
        {
            assert!(
                names.contains(&format!("{n}.cig").as_str()),
                "{n}.cig is not in set.sha256"
            );
            assert!(
                names.contains(&format!("{n}.expect").as_str()),
                "{n}.expect is not in set.sha256"
            );
            programs += 1;
        }
    }
    assert_eq!(programs, 21);

    let s = Sandbox::new();
    // Against the pinned cig's own checkout, when there is one: every file
    // matches, and set.sh copies exactly the set.
    let src = std::env::var_os("CIGSCRIPT_SRC").map(PathBuf::from);
    match src.map(|p| p.join("bench/compute")).filter(|p| p.is_dir()) {
        Some(dir) => {
            for (want, name) in &sums {
                let have = sha(&fs::read(dir.join(name)).unwrap_or_default());
                assert_eq!(&have, want, "{name} is not the pinned cig's file");
            }
            let o = set_sh(&s, &dir, None);
            assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
            for (want, name) in &sums {
                assert_eq!(&sha(&fs::read(s.path().join("out").join(name)).unwrap()), want);
            }
        }
        None => eprintln!(
            "note: the pinned-cig half is skipped; CIGSCRIPT_SRC does not point at a cig checkout with bench/compute"
        ),
    }

    // A set that is not the sums' (one byte different, one file missing) is
    // refused by name, and a file that is not right is never copied.
    // A backslash in the set's path: sha256sum escapes such a name (every
    // Windows path), so the set must be hashed from stdin on every machine.
    let fake = s.path().join("fa\\ke");
    fs::create_dir_all(&fake).unwrap();
    fs::write(fake.join("a.cig"), "exhale 1\n").unwrap();
    fs::write(fake.join("a.expect"), "1\n").unwrap();
    fs::write(fake.join("b.cig"), "exhale 2\n").unwrap();
    let mine = s.path().join("mine.sha256");
    fs::write(
        &mine,
        format!(
            "{} a.cig\n{} a.expect\n{} b.cig\n{} b.expect\n",
            sha(b"exhale 1\n"),
            sha(b"1\n"),
            sha(b"exhale 2\n"),
            sha(b"2\n")
        ),
    )
    .unwrap();
    let o = set_sh(&s, &fake, Some(&mine));
    assert_eq!(o.status.code(), Some(3), "a missing file is a refusal");
    assert!(
        text(&o.stderr).contains("b.expect is not the pinned cig's"),
        "{}",
        text(&o.stderr)
    );
    fs::write(fake.join("b.expect"), "2\n").unwrap();
    fs::write(fake.join("a.cig"), "exhale 9\n").unwrap();
    fs::remove_dir_all(s.path().join("out")).unwrap();
    let o = set_sh(&s, &fake, Some(&mine));
    assert_eq!(o.status.code(), Some(3), "a changed byte is a refusal");
    assert!(
        text(&o.stderr).contains("a.cig is not the pinned cig's"),
        "{}",
        text(&o.stderr)
    );
    assert!(!s.path().join("out/a.cig").exists());
    assert!(s.path().join("out/b.cig").exists());
}

// ----- same_runtime_for_both_sides ---------------------------------------------

#[cfg(unix)]
/// A runtime that is not the one a program carries: the real one with a byte
/// more at the end.
fn other_runtime(s: &Sandbox) -> PathBuf {
    let mut bytes = fs::read(RT).unwrap();
    bytes.push(0);
    let p = s.path().join("other/cig");
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(&p, bytes).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    }
    p
}

#[cfg(unix)]
#[test]
fn same_runtime_for_both_sides() {
    let s = Sandbox::new();
    s.write("generated/set/t.cig", "exhale 7\n");
    // `cig` on the PATH is the runtime; cigcompile is beside it.
    let bin = s.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    std::os::unix::fs::symlink(RT, bin.join("cig")).unwrap();
    std::os::unix::fs::symlink(CC, bin.join("cigcompile")).unwrap();
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let prep = |s: &Sandbox, path: &str| {
        let mut c = Command::new("sh");
        s.env(&mut c);
        c.env("PATH", path).env_remove("CIGCOMPILE_RUNTIME").args([
            root().join("bench/prep-program.sh").to_str().unwrap(),
            "t",
            "generated/prog",
        ]);
        run_for(c, 300)
    };
    let o = prep(&s, &path);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));

    // The program carries, byte for byte at its front, the runtime that
    // `cig run` is: the same sha256 on both sides.
    let rt = fs::read(RT).unwrap();
    let prog = fs::read(s.path().join("generated/prog/t")).unwrap();
    assert!(prog.len() > rt.len());
    assert_eq!(
        sha(&prog[..rt.len()]),
        sha(&rt),
        "the program carries another runtime"
    );
    let on_path = fs::read(bin.join("cig")).unwrap();
    assert_eq!(sha(&on_path), sha(&rt));

    // A program built on one runtime is not accepted as the other's: verify
    // names the difference, which is what prep-program.sh turns into a refusal.
    let other = other_runtime(&s);
    let mut c = Command::new(CC);
    s.env(&mut c);
    c.args([
        "verify",
        "generated/prog/t",
        "--script",
        "generated/set/t.cig",
        "--runtime",
        other.to_str().unwrap(),
    ]);
    let o = run_for(c, 120);
    assert_ne!(o.status.code(), Some(0), "{}", text(&o.stdout));

    // The same script, run interpreted and compiled, prints the same.
    let mut c = Command::new(bin.join("cig"));
    s.env(&mut c);
    c.args(["run", "generated/set/t.cig"]);
    let interpreted = run_for(c, 120);
    let mut c = Command::new(s.path().join("generated/prog/t"));
    s.env(&mut c);
    let compiled = run_for(c, 120);
    assert_eq!(interpreted.stdout, compiled.stdout);
    assert_eq!(text(&compiled.stdout).trim(), "7");
}

// ----- build_phases_on_the_fd --------------------------------------------------

/// `cigcompile build t.cig`, with descriptor 9 open on a file as the harness
/// hands it; returns the stderr and what the descriptor was given.
#[cfg(unix)]
fn build_with_fd(s: &Sandbox, var: Option<&str>, timings_flag: bool) -> (Output, String) {
    let fd_out = s.path().join("fd.out");
    fs::write(&fd_out, "").unwrap();
    let mut c = Command::new("sh");
    s.env(&mut c);
    let flag = if timings_flag { "--timings" } else { "" };
    c.arg("-c")
        .arg(format!(
            "exec '{CC}' build t.cig -o t.out {flag} 9>\"$FD_OUT\""
        ))
        .env("FD_OUT", &fd_out);
    if let Some(v) = var {
        c.env("CIG_TIMINGS_FD", v);
    }
    let o = run_for(c, 300);
    (o, fs::read_to_string(&fd_out).unwrap())
}

#[cfg(unix)]
#[test]
fn build_phases_on_the_fd() {
    let s = Sandbox::new();
    s.write("t.cig", "exhale 1\n");
    let (o, written) = build_with_fd(&s, Some("9"), true);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));

    // Every line is the v1 format, from cigcompile, one phase each.
    let mut fd_rows: Vec<(String, String)> = Vec::new();
    for l in written.lines() {
        let v: serde_json::Value = serde_json::from_str(l).unwrap_or_else(|e| panic!("{e}: {l}"));
        assert_eq!(v["v"], 1, "{l}");
        assert_eq!(v["product"], "cigcompile", "{l}");
        let ms = v["ms"].as_f64().expect("ms");
        assert!(ms.is_finite() && ms >= 0.0, "{l}");
        fd_rows.push((v["phase"].as_str().unwrap().to_string(), format!("{ms:.3}")));
    }
    let phases: Vec<&str> = fd_rows.iter().map(|(p, _)| p.as_str()).collect();
    for want in ["read", "check", "assemble", "write"] {
        assert!(phases.contains(&want), "no `{want}` in {phases:?}");
    }
    let mut seen = phases.clone();
    seen.sort();
    seen.dedup();
    assert_eq!(
        seen.len(),
        phases.len(),
        "a phase written twice: {phases:?}"
    );

    // They are the phases `--timings` prints, with the same times.
    let table: Vec<(String, String)> = text(&o.stderr)
        .lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let (p, ms) = (w.next()?, w.next()?);
            [
                "read", "index", "check", "pin", "runtime", "assemble", "hash", "write", "sbom",
            ]
            .contains(&p)
            .then(|| (p.to_string(), ms.to_string()))
        })
        .collect();
    assert_eq!(
        fd_rows,
        table,
        "the fd and `--timings` disagree:\n{}",
        text(&o.stderr)
    );

    // The descriptor alone is enough (that is how the harness asks): the
    // phases are written and no table is printed.
    let s2 = Sandbox::new();
    s2.write("t.cig", "exhale 1\n");
    let (o, written) = build_with_fd(&s2, Some("9"), false);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
    assert!(written.lines().count() >= 4, "{written}");
    assert!(!text(&o.stderr).contains("phase"), "{}", text(&o.stderr));

    // Not asked for, or stale: not a byte, and the build is as it was.
    for var in [None, Some("77"), Some("1"), Some("x")] {
        let s3 = Sandbox::new();
        s3.write("t.cig", "exhale 1\n");
        let (o, written) = build_with_fd(&s3, var, false);
        assert_eq!(o.status.code(), Some(0), "{var:?}: {}", text(&o.stderr));
        assert_eq!(written, "", "{var:?}");
        assert!(!text(&o.stderr).contains("\"v\":1") && !text(&o.stdout).contains("\"v\":1"));
    }
}
