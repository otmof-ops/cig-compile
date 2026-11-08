// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: Apache-2.0
//! `build --sbom` (#40): a CycloneDX 1.5 file beside each program, naming
//! what it is made of: the runtime (cig's release asset, by version and
//! sha256), the script (by name and sha256) and the compiler. No time and
//! no path of the build machine is in it, so the same build writes the same
//! file. The crates inside the runtime are cig's to list (cig-script).

use crate::project::Job;
use crate::runtime::Runtime;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// Where a program's SBOM goes: beside it, `<program>.cdx.json`.
pub fn path_for(program: &std::path::Path) -> PathBuf {
    let name = program
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "program".to_string());
    program.with_file_name(format!("{name}.cdx.json"))
}

/// The SBOM's text.
pub fn document(
    job: &Job,
    source: &str,
    program_sha: &str,
    runtime: &Runtime,
) -> std::io::Result<String> {
    let runtime_sha = runtime.sha256()?.to_string();
    let asset = crate::digests::DIGESTS
        .iter()
        .find(|(_, sha)| *sha == runtime_sha)
        .map(|(name, _)| *name);
    let mut rt = serde_json::json!({
        "type": "application",
        "bom-ref": "runtime",
        "name": "cig",
        "version": runtime.version,
        "description": "the CigScript runtime the program runs on",
        "hashes": [{"alg": "SHA-256", "content": runtime_sha}],
        "licenses": [{"license": {"id": "Apache-2.0"}}],
        "externalReferences": [{"type": "distribution", "url": format!("https://github.com/otmof-ops/cig-script/releases/tag/v{}", runtime.version)}],
    });
    rt["properties"] = serde_json::json!([{
        "name": "cig:asset",
        "value": asset.unwrap_or("not a release asset: a cig built from source"),
    }]);
    let doc = serde_json::json!({
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "metadata": {
            "tools": {"components": [{"type": "application", "name": "cigcompile", "version": env!("CARGO_PKG_VERSION")}]},
            "component": {
                "type": "application",
                "bom-ref": "program",
                "name": job.name,
                "hashes": [{"alg": "SHA-256", "content": program_sha}],
            },
        },
        "components": [
            rt,
            {
                "type": "file",
                "bom-ref": "script",
                "name": crate::build::recorded_file(job),
                "hashes": [{"alg": "SHA-256", "content": hex::encode(Sha256::digest(source.as_bytes()))}],
            },
        ],
        "dependencies": [{"ref": "program", "dependsOn": ["runtime", "script"]}],
    });
    Ok(serde_json::to_string_pretty(&doc).expect("an SBOM serialises") + "\n")
}

/// Write the SBOM beside the program (unchanged bytes are left alone).
pub fn write(job: &Job, source: &str, program_sha: &str, runtime: &Runtime) -> std::io::Result<()> {
    let text = document(job, source, program_sha, runtime)?;
    crate::build::write_if_changed(&path_for(&job.out), text.as_bytes()).map(|_| ())
}
