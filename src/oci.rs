// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! `build --emit oci` (#41): a static Linux program as an OCI image, a
//! scratch image of one file. The image is an OCI image layout in a tar,
//! with Docker's `manifest.json` beside its index as `docker save` writes
//! them (`docker load`, `podman load`, `skopeo copy oci-archive:`): one
//! uncompressed layer holding the program at `/<name>`, its entrypoint,
//! and nothing else, since the program needs nothing else. Every time in it
//! is zero and every owner root, so the same build writes the same bytes.

use crate::tar::{end, entry};
use sha2::{Digest, Sha256};

/// The OCI architecture (and variant) of a Linux target, or `None` for a
/// target that is not Linux.
pub fn platform(triple: &str) -> Option<(&'static str, Option<&'static str>)> {
    match triple {
        "x86_64-unknown-linux-musl" => Some(("amd64", None)),
        "aarch64-unknown-linux-musl" => Some(("arm64", Some("v8"))),
        "armv7-unknown-linux-musleabihf" => Some(("arm", Some("v7"))),
        _ => None,
    }
}

fn digest(b: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(b)))
}

/// The image of program `name` (its bytes `program`, built for `triple`),
/// as an OCI image layout in a tar.
pub fn image(name: &str, program: &[u8], triple: &str) -> Option<Vec<u8>> {
    let (arch, variant) = platform(triple)?;
    let mut layer = Vec::new();
    entry(&mut layer, name, 0o755, b'0', program);
    end(&mut layer);
    let mut config = serde_json::json!({
        "architecture": arch,
        "os": "linux",
        "config": {"Entrypoint": [format!("/{name}")]},
        "rootfs": {"type": "layers", "diff_ids": [digest(&layer)]},
    });
    if let Some(v) = variant {
        config["variant"] = serde_json::json!(v);
    }
    let config = serde_json::to_vec(&config).expect("a config serialises");
    let manifest = serde_json::to_vec(&serde_json::json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.manifest.v1+json",
        "config": {
            "mediaType": "application/vnd.oci.image.config.v1+json",
            "digest": digest(&config),
            "size": config.len(),
        },
        "layers": [{
            "mediaType": "application/vnd.oci.image.layer.v1.tar",
            "digest": digest(&layer),
            "size": layer.len(),
        }],
    }))
    .expect("a manifest serialises");
    let index = serde_json::to_vec(&serde_json::json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.index.v1+json",
        "manifests": [{
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "digest": digest(&manifest),
            "size": manifest.len(),
            "annotations": {
                "io.containerd.image.name": format!("localhost/{name}:latest"),
                "org.opencontainers.image.ref.name": "latest",
            },
        }],
    }))
    .expect("an index serialises");
    // Docker's own manifest beside the OCI index, as `docker save` writes
    // both since Docker 25: its loader reads this one, podman and skopeo
    // the index.
    let blob = |b: &[u8]| format!("blobs/sha256/{}", digest(b).trim_start_matches("sha256:"));
    let docker = serde_json::to_vec(&serde_json::json!([{
        "Config": blob(&config),
        "RepoTags": [format!("localhost/{name}:latest")],
        "Layers": [blob(&layer)],
    }]))
    .expect("a manifest serialises");
    let mut out = Vec::new();
    entry(
        &mut out,
        "oci-layout",
        0o644,
        b'0',
        br#"{"imageLayoutVersion":"1.0.0"}"#,
    );
    entry(&mut out, "index.json", 0o644, b'0', &index);
    entry(&mut out, "manifest.json", 0o644, b'0', &docker);
    entry(&mut out, "blobs/", 0o755, b'5', b"");
    entry(&mut out, "blobs/sha256/", 0o755, b'5', b"");
    for blob in [&config, &manifest, &layer] {
        let d = digest(blob);
        let path = format!("blobs/sha256/{}", d.trim_start_matches("sha256:"));
        entry(&mut out, &path, 0o644, b'0', blob);
    }
    end(&mut out);
    Some(out)
}

/// Where a program's image goes: beside it, `<program>.oci.tar`.
pub fn path_for(program: &std::path::Path) -> std::path::PathBuf {
    let name = program
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "program".to_string());
    program.with_file_name(format!("{name}.oci.tar"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_linux_targets_make_images_and_the_same_program_the_same_bytes() {
        assert!(image("x", b"\x7fELF", "aarch64-apple-darwin").is_none());
        let a = image("x", b"\x7fELF", "x86_64-unknown-linux-musl").unwrap();
        let b = image("x", b"\x7fELF", "x86_64-unknown-linux-musl").unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len() % 512, 0);
    }
}
