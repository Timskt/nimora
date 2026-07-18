use sha2::{Digest, Sha256};
use std::{fs, path::Path};

fn main() {
    let manifest = Path::new("binaries/agent-provider-worker.json");
    println!("cargo:rerun-if-changed={}", manifest.display());
    if let Ok(bytes) = fs::read(manifest) {
        println!(
            "cargo:rustc-env=NIMORA_PROVIDER_WORKER_MANIFEST_SHA256={:x}",
            Sha256::digest(bytes)
        );
    }
    tauri_build::build();
}
