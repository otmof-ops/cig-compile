// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! A crash in cigcompile joins the family's one queue (cig-installer#10): cig's ashtray, in cig's
//! own report schema, under the origin `cig-compile`, so `ciginstaller crash list` shows it and
//! `ciginstaller crash send` files it in this repository.
//!
//! There is no second pipeline here. The report is built with cig's own crash code (the
//! `cigscript` library this compiler already links), so the schema, the redaction of the command
//! line and the shapes that stand for paths are cig's. It is handed over the way cigo hands its
//! reports over, `cig crash submit --origin cig-compile`; with no `cig` on the machine it is
//! written into the ashtray in the same schema. Nothing is ever sent from here: sending is
//! `ciginstaller crash send`, under the person's `reports` setting.
//!
//! `reports = never` (set with `ciginstaller config set reports never`, in `CIG_REPORTS` or
//! `CIGI_REPORTS`, in cig's own config, or locked by the machine layer) means nothing is saved:
//! the hook says so in one line and the program stops as it would have.
//!
//! The hook never panics (a second panic while it runs is ignored, a failure to save is said),
//! and it is bounded: every program it starts is given seconds, not forever.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use cigscript::crash::{self, CrashReport, Kind};

/// The tool that made the report; `repo_of_origin` in cig-installer routes it by this name.
pub const ORIGIN: &str = "cig-compile";

/// Where its reports are filed.
pub const REPO: &str = "otmof-ops/cig-compile";

/// The program's name in a report's command line.
const PROGRAM: &str = "cigcompile";

/// The longest the hook waits for a program it starts.
const LIMIT: Duration = Duration::from_secs(10);

static BUSY: AtomicBool = AtomicBool::new(false);

/// Install the hook. `words` are what the command line may say in a report (the parser's
/// subcommands and flags); everything else in it travels as its shape. Call it first in `main`.
pub fn install(words: Vec<String>) {
    let mut known = words;
    known.push(PROGRAM.to_string());
    crash::know_words(known);
    std::panic::set_hook(Box::new(|info| {
        if BUSY.swap(true, Ordering::SeqCst) {
            return;
        }
        let message = match info.payload().downcast_ref::<&str>() {
            Some(s) => (*s).to_string(),
            None => info
                .payload()
                .downcast_ref::<String>()
                .cloned()
                .unwrap_or_else(|| "(no message)".to_string()),
        };
        if message.contains("Broken pipe") {
            // whoever read the output went away (`cigcompile build | head`): not a crash
            std::process::exit(141);
        }
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", crate_relative(l.file()), l.line(), l.column()))
            .unwrap_or_else(|| "?".to_string());
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        let _ = std::panic::catch_unwind(|| record(&message, &location, &backtrace));
    }));
}

/// Why nothing is saved, when `reports` says never; `None` when a report may be kept.
///
/// Every way of saying it counts, and a `never` anywhere is a `never`: the environment, cig's own
/// config file, and the installer's effective setting (its config file, or the machine layer's
/// lock), which `ciginstaller config get reports` answers.
pub fn refused() -> Option<String> {
    for var in ["CIG_REPORTS", "CIGI_REPORTS"] {
        if std::env::var(var).is_ok_and(|v| v.trim() == "never") {
            return Some(format!("{var} is never"));
        }
    }
    let cfg = cigscript::config::Config::load();
    if cfg.is_set("reports") && cfg.get("reports") == "never" {
        return Some("cig's reports setting is never".to_string());
    }
    if let Some((value, source)) = installer_reports() {
        if value == "never" && source != "default" {
            return Some(if source == "locked" {
                "the machine locks reports to never".to_string()
            } else {
                "ciginstaller's reports setting is never".to_string()
            });
        }
    }
    None
}

/// The installer's effective `reports` and where it comes from, when there is an installer.
fn installer_reports() -> Option<(String, String)> {
    let exe = find("ciginstaller")?;
    let mut cmd = Command::new(exe);
    cmd.args(["config", "get", "reports", "--json"]);
    let out = run_bounded(cmd)?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let data = v.get("data")?;
    Some((
        data.get("value")?.as_str()?.to_string(),
        data.get("source")?.as_str()?.to_string(),
    ))
}

fn record(message: &str, location: &str, backtrace: &str) {
    // cig's own constructor: the message is redacted, the command line reduced to the words the
    // parser knows, the working directory a shape
    let mut report = CrashReport::new(Kind::Crash, location, message, None, None);
    report.title = None;
    // the way cig's own hook does it: every path in the message is its shape, so no folder name
    // of the person's travels (the constructor alone keeps a home-relative one)
    let cwd = std::env::current_dir().unwrap_or_default();
    report.panic_message =
        cigscript::repro::Mapping::new(&[], &cwd).anonymise_text(&crash::truncate(message, 500));
    report.origin = ORIGIN.to_string();
    report.repo = Some(REPO.to_string());
    report.version = env!("CARGO_PKG_VERSION").to_string();
    report.backtrace = own_frames(backtrace);
    let headline = format!(
        "cigcompile hit a bug and stopped: {}",
        crash::truncate(&report.panic_message, 200)
    );
    if let Some(why) = refused() {
        eprintln!("error[E901 internal]: {headline}\n  nothing was saved ({why})");
        return;
    }
    match hand_over(&report) {
        Ok(said) => {
            eprintln!(
                "error[E901 internal]: {headline}\n  at {}",
                report.panic_location
            );
            eprintln!("  {said}");
            eprintln!("  = report it: ciginstaller crash list, then ciginstaller crash send <id>");
        }
        Err(e) => {
            eprintln!(
                "error[E901 internal]: {headline}\n  at {}\n  the report could not be saved ({e})",
                report.panic_location
            );
        }
    }
}

/// To cig's queue: `cig crash submit --origin cig-compile`, the way cigo does it; with no cig,
/// into the ashtray itself. Returns the line that says what happened.
fn hand_over(report: &CrashReport) -> Result<String, String> {
    if let Some(cig) = find("cig") {
        let file = temp_file(report)?;
        let mut cmd = Command::new(cig);
        cmd.args([
            "crash", "submit", "--origin", ORIGIN, "--repo", REPO, "--file",
        ])
        .arg(&file);
        let out = run_bounded(cmd);
        let _ = std::fs::remove_file(&file);
        if let Some(out) = out.filter(|o| o.status.success()) {
            let said = String::from_utf8_lossy(&out.stderr).trim().to_string();
            if !said.is_empty() {
                return Ok(said);
            }
            return Ok("the report is in the family's ashtray".to_string());
        }
        // cig would not take it: keep it ourselves rather than lose it
    }
    let mut owned = report.clone();
    crash::write_new(&mut owned).map_err(|e| e.to_string())?;
    Ok(format!(
        "a report is saved in the family's ashtray ({}); nothing was sent",
        owned.id
    ))
}

/// The report as JSON in a file only this person can read, for `cig crash submit --file`.
fn temp_file(report: &CrashReport) -> Result<PathBuf, String> {
    let json = serde_json::to_string_pretty(report).map_err(|e| e.to_string())?;
    let dir = std::env::temp_dir();
    for n in 0..100u32 {
        let path = dir.join(format!(
            ".cig-compile-crash-{}-{n}.json",
            std::process::id()
        ));
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        match opts.open(&path) {
            Ok(mut f) => {
                use std::io::Write;
                f.write_all(json.as_bytes()).map_err(|e| e.to_string())?;
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("no free name for the report".to_string())
}

/// Run a program for at most [`LIMIT`], stdin closed. A program that overruns is left behind:
/// this process is stopping anyway.
fn run_bounded(mut cmd: Command) -> Option<std::process::Output> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = cmd.spawn().ok()?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    rx.recv_timeout(LIMIT).ok()?.ok()
}

#[cfg(windows)]
fn exe(name: &str) -> String {
    format!("{name}.exe")
}
#[cfg(not(windows))]
fn exe(name: &str) -> String {
    name.to_string()
}

/// `PATH`, `$CIGSCRIPT_HOME/bin`, `~/.cigscript/bin`: where the installer looks for the family.
fn find(name: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    if let Some(h) = std::env::var_os("CIGSCRIPT_HOME") {
        dirs.push(PathBuf::from(h).join("bin"));
    }
    if let Some(h) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        dirs.push(PathBuf::from(h).join(".cigscript").join("bin"));
    }
    let file = exe(name);
    dirs.into_iter()
        .filter(|d| !d.as_os_str().is_empty())
        .map(|d| d.join(&file))
        .find(|c| is_program(c))
}

fn is_program(p: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(p) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// A source path as a report carries it: inside this crate from `src/`; a dependency or the
/// toolchain by its tail; never a directory of the person's.
fn crate_relative(file: &str) -> String {
    let p = file.replace('\\', "/");
    if p.starts_with("src/") {
        return p;
    }
    if let Some(i) = p.find("/rustc/") {
        let tail = &p[i + "/rustc/".len()..];
        return format!("rustc/{}", tail.split_once('/').map_or(tail, |(_, t)| t));
    }
    if let Some(i) = p.rfind("/src/") {
        return p[i + 1..].to_string();
    }
    p.rsplit('/').next().unwrap_or(&p).to_string()
}

/// The frames a report keeps: this compiler's own code, by symbol and crate-relative place,
/// never the hook, at most forty.
fn own_frames(bt: &str) -> Vec<String> {
    let lines: Vec<&str> = bt.lines().collect();
    let mut out = Vec::new();
    for (i, raw) in lines.iter().enumerate() {
        let Some((n, symbol)) = raw.trim().split_once(": ") else {
            continue;
        };
        if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let bare = symbol.trim_start_matches('<');
        if !(bare.starts_with("cigcompile::")
            || bare.starts_with("cigarscript::")
            || bare.starts_with("cigscript::"))
            || bare.contains("::crash::")
        {
            continue;
        }
        let symbol = match symbol.rfind("::h") {
            Some(k) if symbol.len() - k == 19 => &symbol[..k],
            _ => symbol,
        };
        let at = lines
            .get(i + 1)
            .map(|l| l.trim())
            .and_then(|l| l.strip_prefix("at "))
            .map(crate_relative);
        out.push(match at {
            Some(a) => format!("{symbol} ({a})"),
            None => symbol.to_string(),
        });
        if out.len() == 40 {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use wait_timeout::ChildExt;

    /// The child half of every test: when the parent says so, install the hook and panic. The
    /// switch exists only in test builds; a release build reads no such variable.
    #[test]
    fn child_panics() {
        if std::env::var_os("CIGCOMPILE_TEST_PANIC_CHILD").is_none() {
            return;
        }
        install(vec!["build".into(), "--json".into()]);
        let secret = std::env::var("CIGCOMPILE_TEST_SECRET_PATH").unwrap_or_default();
        panic!("cannot read {secret}: boom");
    }

    /// What says never: a label, the environment, cig's config file, the installer's source.
    #[cfg(unix)]
    type Case<'a> = (
        &'a str,
        Vec<(&'a str, &'a str)>,
        Option<String>,
        Option<&'a str>,
    );

    struct Scratch {
        dir: tempfile::TempDir,
    }

    impl Scratch {
        fn new() -> Scratch {
            Scratch {
                dir: tempfile::tempdir().unwrap(),
            }
        }
        fn home(&self) -> PathBuf {
            self.dir.path().join("home")
        }
        fn bin(&self) -> PathBuf {
            self.dir.path().join("bin")
        }
        fn ashtray(&self) -> PathBuf {
            self.home().join("ashtray")
        }
        fn reports(&self) -> Vec<serde_json::Value> {
            let Ok(rd) = std::fs::read_dir(self.ashtray()) else {
                return Vec::new();
            };
            rd.flatten()
                .map(|e| serde_json::from_str(&std::fs::read_to_string(e.path()).unwrap()).unwrap())
                .collect()
        }
        #[cfg(unix)]
        fn stub(&self, name: &str, body: &str) {
            use std::os::unix::fs::PermissionsExt;
            std::fs::create_dir_all(self.bin()).unwrap();
            let p = self.bin().join(name);
            std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        /// Panic in a child of this very test binary, in a scratch HOME and nothing else.
        fn panic_child(&self, env: &[(&str, &str)]) {
            std::fs::create_dir_all(self.home()).unwrap();
            std::fs::create_dir_all(self.bin()).unwrap();
            let path = if cfg!(unix) {
                format!("{}:/usr/bin:/bin", self.bin().display())
            } else {
                self.bin().display().to_string()
            };
            let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
            cmd.args(["--exact", "crash::tests::child_panics", "--nocapture"])
                .env_clear()
                .env("CIGCOMPILE_TEST_PANIC_CHILD", "1")
                .env(
                    "CIGCOMPILE_TEST_SECRET_PATH",
                    self.dir.path().join("secret-client/plans.cig"),
                )
                .env("PATH", path)
                .env("HOME", self.dir.path())
                .env("USERPROFILE", self.dir.path())
                .env("CIGSCRIPT_HOME", self.home())
                .env("XDG_CONFIG_HOME", self.dir.path().join("xdg-c"))
                .env("XDG_DATA_HOME", self.dir.path().join("xdg-d"))
                .env("XDG_STATE_HOME", self.dir.path().join("xdg-s"))
                .env("XDG_CACHE_HOME", self.dir.path().join("xdg-k"))
                .env("STUB_OUT", self.dir.path())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped());
            #[cfg(windows)]
            if let Some(root) = std::env::var_os("SystemRoot") {
                cmd.env("SystemRoot", root);
            }
            let vars: BTreeMap<_, _> = env.iter().cloned().collect();
            for (k, v) in vars {
                cmd.env(k, v);
            }
            let mut child = cmd.spawn().unwrap();
            let status = child.wait_timeout(Duration::from_secs(60)).unwrap();
            if status.is_none() {
                let _ = child.kill();
                panic!("the child did not stop in 60 seconds");
            }
        }
    }

    #[test]
    fn a_panic_leaves_one_queued_report_naming_cig_compile() {
        let s = Scratch::new();
        // no cig on the machine: the report goes into the ashtray itself
        s.panic_child(&[]);
        let r = s.reports();
        assert_eq!(r.len(), 1, "exactly one report: {r:?}");
        assert_eq!(r[0]["origin"], ORIGIN);
        assert_eq!(r[0]["repo"], REPO);
        assert_eq!(r[0]["kind"], "crash");
        assert_eq!(r[0]["version"], env!("CARGO_PKG_VERSION"));
        assert!(r[0]["panic_message"].as_str().unwrap().contains("boom"));
        assert!(r[0]["panic_location"]
            .as_str()
            .unwrap()
            .starts_with("src/crash.rs"));
        // nothing personal: no scratch path, no folder name of the person's
        let text = std::fs::read_to_string(
            std::fs::read_dir(s.ashtray())
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path(),
        )
        .unwrap();
        let root = s.dir.path().display().to_string();
        assert!(!text.contains(&root), "a path of the person's: {text}");
        assert!(!text.contains("secret-client"), "{text}");
    }

    #[cfg(unix)]
    #[test]
    fn with_cig_on_the_machine_the_report_is_handed_to_cig_submit() {
        let s = Scratch::new();
        s.stub(
            "cig",
            r#"echo "$@" > "$STUB_OUT/args.txt"
while [ $# -gt 0 ]; do if [ "$1" = "--file" ]; then cp "$2" "$STUB_OUT/got.json"; echo "$2" > "$STUB_OUT/file.txt"; fi; shift; done
echo "ashtray: stub saved it" >&2"#,
        );
        s.panic_child(&[]);
        let args = std::fs::read_to_string(s.dir.path().join("args.txt")).unwrap();
        assert!(
            args.starts_with(
                "crash submit --origin cig-compile --repo otmof-ops/cig-compile --file "
            ),
            "{args}"
        );
        let got: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(s.dir.path().join("got.json")).unwrap())
                .unwrap();
        assert_eq!(got["origin"], ORIGIN);
        assert!(!got.to_string().contains("secret-client"), "{got}");
        // handed over, not also written: one queue, one copy; and the hand-over file is gone
        assert!(s.reports().is_empty());
        let file = std::fs::read_to_string(s.dir.path().join("file.txt")).unwrap();
        assert!(!Path::new(file.trim()).exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_cig_that_will_not_take_it_does_not_lose_it() {
        let s = Scratch::new();
        s.stub("cig", "exit 2");
        s.panic_child(&[]);
        assert_eq!(s.reports().len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn reports_never_saves_nothing_by_any_way_of_saying_it() {
        let never = r#"{"ok":true,"data":{"key":"reports","value":"never","source":"SRC"}}"#;
        // each: the environment variables, cig's config, the installer's file setting (`config set
        // reports never`) and a machine-layer lock (the installer answers `locked`)
        let cases: Vec<Case> = vec![
            ("CIG_REPORTS", vec![("CIG_REPORTS", "never")], None, None),
            ("CIGI_REPORTS", vec![("CIGI_REPORTS", "never")], None, None),
            (
                "cig config",
                vec![],
                Some("reports = never\n".to_string()),
                None,
            ),
            ("installer file", vec![], None, Some("file")),
            (
                "machine lock beats CIG_REPORTS=always",
                vec![("CIG_REPORTS", "always")],
                None,
                Some("locked"),
            ),
        ];
        for (what, env, cig_config, installer) in cases {
            let s = Scratch::new();
            s.stub("cig", r#"echo ran > "$STUB_OUT/cig-ran.txt""#);
            if let Some(src) = installer {
                let json = never.replace("SRC", src);
                s.stub("ciginstaller", &format!("echo '{json}'"));
            }
            if let Some(c) = cig_config {
                std::fs::create_dir_all(s.home()).unwrap();
                std::fs::write(s.home().join("config"), c).unwrap();
            }
            s.panic_child(&env);
            assert!(s.reports().is_empty(), "{what}: nothing may be queued");
            assert!(
                !s.dir.path().join("cig-ran.txt").exists(),
                "{what}: nothing may be handed to cig"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn an_installer_that_says_ask_still_queues_the_report() {
        let s = Scratch::new();
        s.stub(
            "ciginstaller",
            r#"echo '{"ok":true,"data":{"key":"reports","value":"ask","source":"default"}}'"#,
        );
        s.panic_child(&[]);
        assert_eq!(s.reports().len(), 1);
    }

    #[test]
    fn a_source_path_is_its_tail() {
        assert_eq!(crate_relative("src/crash.rs"), "src/crash.rs");
        assert_eq!(
            crate_relative("/home/a/.cargo/registry/src/idx-1/clap-4/src/x.rs"),
            "src/x.rs"
        );
        assert_eq!(crate_relative("/somewhere/else.rs"), "else.rs");
    }

    #[test]
    fn only_this_compilers_frames_stay() {
        let bt = "   0: std::backtrace::Backtrace::force_capture\n   1: cigcompile::crash::install::{{closure}}\n   2: core::panicking::panic_fmt\n   3: cigcompile::build::run::h0123456789abcdef\n             at /home/user/w/src/build.rs:30:9\n   4: main\n";
        assert_eq!(
            own_frames(bt),
            vec!["cigcompile::build::run (src/build.rs:30:9)".to_string()]
        );
    }
}
