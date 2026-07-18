use nimora_agent_provider_worker::{
    ProviderWorkerManifest, SidecarVerificationError, verify_provider_worker,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn temporary_directory(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "nimora-provider-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create temporary directory");
    path
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn write_fixture(root: &std::path::Path) -> (String, PathBuf) {
    let executable_path = root.join("nimora-agent-provider-worker");
    let executable = b"verified worker executable";
    fs::write(&executable_path, executable).expect("write executable");
    let manifest = ProviderWorkerManifest {
        spec: "nimora.provider-worker-manifest/1".to_owned(),
        worker_protocol_version: 1,
        capabilities: vec![
            "provider:ollama-loopback/1".to_owned(),
            "provider:openai-compatible/1".to_owned(),
        ],
        executable: "nimora-agent-provider-worker".to_owned(),
        executable_bytes: executable.len() as u64,
        executable_sha256: digest(executable),
    };
    let manifest_bytes = serde_json::to_vec(&manifest).expect("manifest");
    fs::write(root.join("agent-provider-worker.json"), &manifest_bytes).expect("write manifest");
    (digest(&manifest_bytes), executable_path)
}

#[test]
fn resolves_only_when_trusted_manifest_and_executable_match() {
    let root = temporary_directory("valid");
    let (manifest_digest, executable_path) = write_fixture(&root);
    let verified = verify_provider_worker(&root, "agent-provider-worker.json", &manifest_digest)
        .expect("verified sidecar");
    assert_eq!(
        verified.executable_path,
        executable_path.canonicalize().expect("canonical path")
    );
    assert_eq!(verified.manifest.worker_protocol_version, 1);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn rejects_manifest_and_executable_tampering_independently() {
    let root = temporary_directory("tampered");
    let (manifest_digest, executable_path) = write_fixture(&root);
    fs::write(&executable_path, b"tampered worker executable").expect("tamper executable");
    assert_eq!(
        verify_provider_worker(&root, "agent-provider-worker.json", &manifest_digest),
        Err(SidecarVerificationError::ExecutableDigestMismatch)
    );

    let (manifest_digest, _) = write_fixture(&root);
    fs::write(root.join("agent-provider-worker.json"), b"{}").expect("tamper manifest");
    assert_eq!(
        verify_provider_worker(&root, "agent-provider-worker.json", &manifest_digest),
        Err(SidecarVerificationError::ManifestDigestMismatch)
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn rejects_manifest_path_traversal_before_filesystem_access() {
    let root = temporary_directory("traversal");
    assert_eq!(
        verify_provider_worker(&root, "../provider.json", &"0".repeat(64)),
        Err(SidecarVerificationError::InvalidManifestPath)
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn rejects_missing_or_noncanonical_worker_capabilities() {
    let root = temporary_directory("capabilities");
    let (_, _) = write_fixture(&root);
    let executable = b"verified worker executable";
    for capabilities in [
        vec!["provider:ollama-loopback/1".to_owned()],
        vec![
            "provider:openai-compatible/1".to_owned(),
            "provider:ollama-loopback/1".to_owned(),
        ],
    ] {
        let manifest = ProviderWorkerManifest {
            spec: "nimora.provider-worker-manifest/1".to_owned(),
            worker_protocol_version: 1,
            capabilities,
            executable: "nimora-agent-provider-worker".to_owned(),
            executable_bytes: executable.len() as u64,
            executable_sha256: digest(executable),
        };
        let bytes = serde_json::to_vec(&manifest).expect("manifest");
        fs::write(root.join("agent-provider-worker.json"), &bytes).expect("write manifest");
        assert_eq!(
            verify_provider_worker(&root, "agent-provider-worker.json", &digest(&bytes)),
            Err(SidecarVerificationError::InvalidManifest)
        );
    }
    fs::remove_dir_all(root).expect("cleanup");
}

#[cfg(unix)]
#[test]
fn rejects_symbolic_link_executables() {
    use std::os::unix::fs::symlink;

    let root = temporary_directory("symlink");
    let outside = root.with_extension("outside");
    fs::write(&outside, b"outside executable").expect("write outside file");
    let executable_path = root.join("nimora-agent-provider-worker");
    symlink(&outside, &executable_path).expect("create symlink");
    let manifest = ProviderWorkerManifest {
        spec: "nimora.provider-worker-manifest/1".to_owned(),
        worker_protocol_version: 1,
        capabilities: vec![
            "provider:ollama-loopback/1".to_owned(),
            "provider:openai-compatible/1".to_owned(),
        ],
        executable: "nimora-agent-provider-worker".to_owned(),
        executable_bytes: 18,
        executable_sha256: digest(b"outside executable"),
    };
    let manifest_bytes = serde_json::to_vec(&manifest).expect("manifest");
    fs::write(root.join("agent-provider-worker.json"), &manifest_bytes).expect("write manifest");
    assert_eq!(
        verify_provider_worker(
            &root,
            "agent-provider-worker.json",
            &digest(&manifest_bytes)
        ),
        Err(SidecarVerificationError::ExecutableUnavailable)
    );
    fs::remove_dir_all(root).expect("cleanup root");
    fs::remove_file(outside).expect("cleanup outside");
}
