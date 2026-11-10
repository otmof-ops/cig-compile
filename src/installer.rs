// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! The seam to `ciginstaller`: `cigcompile report` is a short way in to
//! `ciginstaller bug cigcompile`, which is the one way to report anything.
//!
//! It looks for `ciginstaller` on `PATH`, then in `$CIGSCRIPT_HOME/bin`,
//! then in `~/.cigscript/bin`; runs it with the arguments, stdio inherited;
//! and passes its exit code through. With none it says whose job this is and
//! how to get the installer, and exits 127. Nothing here downloads or
//! installs, and nothing here calls back into cigcompile.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Exit code when the installer cannot be found or started (the shell's own
/// "command not found").
pub const MISSING: i32 = 127;

/// How to get the installer on a machine that has none.
pub const BOOTSTRAP: &str = "https://github.com/otmof-ops/cig-installer#install";

/// Where a bug goes until the installer has a release to carry it.
pub const ISSUES: &str = "https://github.com/otmof-ops/cig-compile/issues";

#[cfg(windows)]
const EXE: &str = "ciginstaller.exe";
#[cfg(not(windows))]
const EXE: &str = "ciginstaller";

/// The sentence a message that asks for a report ends with.
pub fn please_report() -> String {
    format!("please report it: `ciginstaller bug cigcompile` (or `cigcompile report`), or {ISSUES}")
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

/// `PATH`, `$CIGSCRIPT_HOME/bin`, `~/.cigscript/bin`, in that order.
fn search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    if let Some(h) = std::env::var_os("CIGSCRIPT_HOME") {
        dirs.push(PathBuf::from(h).join("bin"));
    }
    if let Some(h) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        dirs.push(PathBuf::from(h).join(".cigscript").join("bin"));
    }
    dirs
}

/// Where `ciginstaller` is, if it is anywhere.
pub fn find() -> Option<PathBuf> {
    search_dirs()
        .into_iter()
        .filter(|d| !d.as_os_str().is_empty())
        .map(|d| d.join(EXE))
        .find(|c| is_program(c))
}

/// The two lines printed when there is no installer.
pub fn missing_message(what: &str, command: &str) -> String {
    format!("{what} is ciginstaller's job now: {command}\nget ciginstaller: {BOOTSTRAP}")
}

/// Hand `what` over to `ciginstaller args...` and return its exit code.
pub fn run(what: &str, args: &[String]) -> i32 {
    let command = format!("ciginstaller {}", args.join(" "));
    let Some(exe) = find() else {
        eprintln!("{}", missing_message(what, command.trim_end()));
        return MISSING;
    };
    match Command::new(&exe).args(args).status() {
        Ok(status) => match status.code() {
            Some(code) => code,
            None => {
                #[cfg(unix)]
                {
                    use std::os::unix::process::ExitStatusExt;
                    status.signal().map_or(1, |s| 128 + s)
                }
                #[cfg(not(unix))]
                {
                    1
                }
            }
        },
        Err(e) => {
            eprintln!("cannot start {}: {e}", exe.display());
            eprintln!("{}", missing_message(what, command.trim_end()));
            MISSING
        }
    }
}

/// `cigcompile report [args]`: `ciginstaller bug cigcompile [args]`.
pub fn report(args: &[String]) -> i32 {
    let mut full = vec!["bug".to_string(), "cigcompile".to_string()];
    full.extend(args.iter().cloned());
    run("cigcompile report", &full)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_missing_message_is_the_contracts_two_lines() {
        let m = missing_message("cigcompile report", "ciginstaller bug cigcompile");
        assert_eq!(
            m.lines().next(),
            Some("cigcompile report is ciginstaller's job now: ciginstaller bug cigcompile")
        );
        assert!(m.lines().nth(1).unwrap().starts_with("get ciginstaller: "));
    }

    #[test]
    fn please_report_names_the_command_and_the_issues_link() {
        let m = please_report();
        assert!(m.contains("ciginstaller bug cigcompile"));
        assert!(m.contains("cigcompile report"));
        assert!(m.contains(ISSUES));
    }
}
