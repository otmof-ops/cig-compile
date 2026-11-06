// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! What to compile: one script, or every script at the top of a folder,
//! and where each program goes.

use crate::runtime::{host, Target};
use cigscript::diagnostics::{usage, Diagnostic};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One script to compile.
#[derive(Clone, Debug)]
pub struct Job {
    /// The script on disk.
    pub script: PathBuf,
    /// The script's path as the program records it: as it was given, so a
    /// runtime error points where `cig run` with the same path would.
    pub file: String,
    /// The program's name.
    pub name: String,
    /// Where the program goes.
    pub out: PathBuf,
}

/// The jobs for `path` (a `.cig` file, or a folder of them; the current
/// folder when none is given). `out` is the program's path for one script,
/// the folder the programs go to for a folder (default `<folder>/bin`, and
/// `<folder>/bin/<target>` for another machine).
pub fn jobs(
    path: Option<&Path>,
    out: Option<&Path>,
    target: &Target,
) -> Result<Vec<Job>, Diagnostic> {
    let path = path.unwrap_or(Path::new("."));
    let nothing = |why: String| {
        usage(why).code("E1000").with_hint(
            "name a .cig script, or a folder that holds them: cigcompile build tools/tidy.cig",
        )
    };
    if path.is_dir() {
        let mut scripts: Vec<PathBuf> = std::fs::read_dir(path)
            .map_err(|e| nothing(format!("cannot read {}: {e}", path.display())))?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "cig"))
            .collect();
        scripts.sort();
        if scripts.is_empty() {
            return Err(nothing(format!("{} holds no .cig script", shown(path))));
        }
        let dir = match out {
            Some(o) => o.to_path_buf(),
            None if target.triple == host().triple => path.join("bin"),
            None => path.join("bin").join(target.triple),
        };
        let mut jobs = Vec::new();
        let mut taken: BTreeMap<String, String> = BTreeMap::new();
        for s in scripts {
            let name = name_for(&s);
            let file = shown(&s);
            if let Some(other) = taken.insert(name.to_lowercase(), file.clone()) {
                return Err(usage(format!(
                    "{other} and {file} would both compile to {}",
                    dir.join(format!("{name}{}", target.suffix())).display()
                ))
                .code("E1007")
                .with_hint("rename one of them, or build each with its own -o"));
            }
            jobs.push(Job {
                out: dir.join(format!("{name}{}", target.suffix())),
                script: s,
                file,
                name,
            });
        }
        return Ok(jobs);
    }
    if !path.is_file() {
        return Err(nothing(format!("{} is not there", shown(path))));
    }
    let name = name_for(path);
    let out = match out {
        Some(o) => o.to_path_buf(),
        None => PathBuf::from(format!("{name}{}", target.suffix())),
    };
    Ok(vec![Job {
        script: path.to_path_buf(),
        file: shown(path),
        name,
        out,
    }])
}

/// A path as a person typed it: no leading `./`.
fn shown(p: &Path) -> String {
    let s = p.to_string_lossy().to_string();
    s.strip_prefix("./").map(str::to_string).unwrap_or(s)
}

/// The program's name: the script's stem, as a plain word (letters,
/// digits, `.`, `_` and `-`; anything else becomes `-`).
pub fn name_for(script: &Path) -> String {
    let stem = script
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut name: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect();
    while name.starts_with(|c: char| !c.is_ascii_alphanumeric()) {
        name.remove(0);
    }
    name.truncate(64);
    if name.is_empty() {
        name = "program".to_string();
    }
    debug_assert!(cigscript::program::good_name(&name));
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_plain_words() {
        assert_eq!(
            name_for(Path::new("tools/tidy-downloads.cig")),
            "tidy-downloads"
        );
        assert_eq!(name_for(Path::new("my tool.cig")), "my-tool");
        assert_eq!(name_for(Path::new("_x.cig")), "x");
        assert_eq!(name_for(Path::new("…….cig")), "program");
        assert!(cigscript::program::good_name(&name_for(Path::new(
            &"a".repeat(90)
        ))));
    }
}
