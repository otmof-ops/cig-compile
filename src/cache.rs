// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! The build index (#9): what each program was built from, so a rebuild
//! that changes nothing reads neither the runtime nor the program, runs no
//! check, and touches nothing. Kept at `<cig home>/cache/compile/index.json`
//! (`CIGSCRIPT_HOME`, else `~/.cigscript`). It is only a shortcut: lost or
//! damaged, the next build does the work and writes it again.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// What a file is, without reading it: its length, modified time and (on
/// Unix) inode. A file with the same three is taken as unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileId {
    pub len: u64,
    pub mtime_ns: u128,
    pub ino: u64,
}

impl FileId {
    pub fn of(p: &Path) -> Option<FileId> {
        let m = std::fs::metadata(p).ok()?;
        let mtime_ns = m
            .modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_nanos();
        #[cfg(unix)]
        let ino = {
            use std::os::unix::fs::MetadataExt;
            m.ino()
        };
        #[cfg(not(unix))]
        let ino = 0;
        Some(FileId {
            len: m.len(),
            mtime_ns,
            ino,
        })
    }

    fn to_json(&self) -> Value {
        json!({"len": self.len, "mtime_ns": self.mtime_ns.to_string(), "ino": self.ino})
    }

    fn from_json(v: &Value) -> Option<FileId> {
        Some(FileId {
            len: v["len"].as_u64()?,
            mtime_ns: v["mtime_ns"].as_str()?.parse().ok()?,
            ino: v["ino"].as_u64()?,
        })
    }
}

pub struct Index {
    path: PathBuf,
    doc: Value,
    changed: bool,
}

impl Index {
    pub fn load() -> Index {
        let path = cigscript::burn::runs::home_dir()
            .join("cache")
            .join("compile")
            .join("index.json");
        let doc = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .filter(|v| v["format"] == 1)
            .unwrap_or_else(|| json!({"format": 1, "runtimes": {}, "programs": {}}));
        Index {
            path,
            doc,
            changed: false,
        }
    }

    /// The version a runtime was found to be, when it is unchanged since.
    /// The version of the runtime at `path`, when it is unchanged and was
    /// checked as the runtime for `triple`: a check for one machine says
    /// nothing about another's.
    pub fn runtime_version(&self, path: &Path, id: &FileId, triple: &str) -> Option<String> {
        let e = &self.doc["runtimes"][path.to_string_lossy().as_ref()];
        (FileId::from_json(&e["id"]).as_ref() == Some(id) && e["for"].as_str() == Some(triple))
            .then(|| e["version"].as_str().map(str::to_string))
            .flatten()
    }

    pub fn note_runtime(&mut self, path: &Path, id: &FileId, version: &str, triple: &str) {
        self.doc["runtimes"][path.to_string_lossy().as_ref()] =
            json!({"id": id.to_json(), "version": version, "for": triple});
        self.changed = true;
    }

    /// The warnings to say again, when the program at `out` was built from
    /// `key` and is as it was written.
    pub fn fresh(&self, out: &Path, key: &str) -> Option<(String, Vec<String>)> {
        let e = &self.doc["programs"][absolute(out).to_string_lossy().as_ref()];
        if e["key"].as_str() != Some(key) {
            return None;
        }
        if FileId::from_json(&e["id"]) != FileId::of(out) {
            return None;
        }
        let warnings = e["warnings"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|w| w.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        Some((e["sha256"].as_str()?.to_string(), warnings))
    }

    pub fn note_program(&mut self, out: &Path, key: &str, sha256: &str, warnings: &[String]) {
        let Some(id) = FileId::of(out) else {
            return;
        };
        self.doc["programs"][absolute(out).to_string_lossy().as_ref()] =
            json!({"key": key, "id": id.to_json(), "sha256": sha256, "warnings": warnings});
        self.changed = true;
    }

    /// Written whole, beside itself, then renamed: never half an index.
    pub fn save(&self) {
        if !self.changed {
            return;
        }
        let Some(dir) = self.path.parent() else {
            return;
        };
        if std::fs::create_dir_all(dir).is_err() {
            return;
        }
        let tmp = dir.join(format!(".index.{}.tmp", std::process::id()));
        let body = serde_json::to_vec_pretty(&self.doc).unwrap_or_default();
        if std::fs::write(&tmp, body).is_ok() && std::fs::rename(&tmp, &self.path).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    }
}

fn absolute(p: &Path) -> PathBuf {
    std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf())
}

/// What a program is built from, as one hash: the compiler, the cig it is
/// for, the target, the runtime file, the path it records, its name and
/// its script.
pub fn key(parts: &[&str]) -> String {
    let mut h = Sha256::new();
    for p in parts {
        h.update((p.len() as u64).to_le_bytes());
        h.update(p.as_bytes());
    }
    hex::encode(h.finalize())
}
