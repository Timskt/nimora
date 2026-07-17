use sha2::{Digest, Sha256};
use std::{fs, path::Path};

fn main() {
    let manifest = Path::new("binaries/ollama-provider.json");
    println!("cargo:rerun-if-changed={}", manifest.display());
    if let Ok(bytes) = fs::read(manifest) {
        println!(
            "cargo:rustc-env=NIMORA_OLLAMA_MANIFEST_SHA256={:x}",
            Sha256::digest(bytes)
        );
    }
    tauri_build::build();
}
