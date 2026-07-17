use nimora_model_importer::{ModelProbeRequest, ModelWorkerError, probe_model_in_worker};
use serde_json::json;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{fs, path::Path, time::Duration};

fn glb(document: &serde_json::Value, binary: &[u8]) -> Vec<u8> {
    let mut json = serde_json::to_vec(&document).unwrap();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let binary_chunk = if binary.is_empty() {
        0
    } else {
        8 + binary.len()
    };
    let total = 12 + 8 + json.len() + binary_chunk;
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(b"glTF");
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&u32::try_from(total).unwrap().to_le_bytes());
    bytes.extend_from_slice(&u32::try_from(json.len()).unwrap().to_le_bytes());
    bytes.extend_from_slice(&0x4e4f_534a_u32.to_le_bytes());
    bytes.extend_from_slice(&json);
    if !binary.is_empty() {
        bytes.extend_from_slice(&u32::try_from(binary.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&0x004e_4942_u32.to_le_bytes());
        bytes.extend_from_slice(binary);
    }
    bytes
}

fn request() -> ModelProbeRequest {
    ModelProbeRequest {
        spec: "nimora.model-probe/1".to_owned(),
        source: "character.glb".into(),
    }
}

#[test]
fn real_worker_probes_a_staged_glb() {
    let root = std::env::temp_dir().join("nimora-model-worker-valid");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("character.glb"),
        glb(
            &json!({
                "asset": { "version": "2.0" },
                "nodes": [{ "mesh": 0 }],
                "meshes": [{ "primitives": [] }],
                "materials": [{}],
                "animations": [{}],
                "skins": [{}],
                "buffers": [{ "byteLength": 4 }]
            }),
            &[0, 1, 2, 3],
        ),
    )
    .unwrap();
    let report = probe_model_in_worker(
        Path::new(env!("CARGO_BIN_EXE_nimora-model-importer-worker")),
        &root,
        &request(),
        Duration::from_secs(2),
    )
    .unwrap();
    assert_eq!(report.format, "glb");
    assert_eq!(report.nodes, 1);
    assert_eq!(report.meshes, 1);
    assert_eq!(report.binary_bytes, 4);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn real_worker_rejects_external_model_uris() {
    let root = std::env::temp_dir().join("nimora-model-worker-uri");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("character.glb"),
        glb(
            &json!({
                "asset": { "version": "2.0" },
                "images": [{ "uri": "https://example.invalid/texture.png" }]
            }),
            &[],
        ),
    )
    .unwrap();
    let error = probe_model_in_worker(
        Path::new(env!("CARGO_BIN_EXE_nimora-model-importer-worker")),
        &root,
        &request(),
        Duration::from_secs(2),
    )
    .unwrap_err();
    assert!(matches!(error, ModelWorkerError::Rejected(message) if message.contains("URI")));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn worker_request_cannot_escape_the_staging_directory() {
    let root = std::env::temp_dir().join("nimora-model-worker-escape");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let error = probe_model_in_worker(
        Path::new(env!("CARGO_BIN_EXE_nimora-model-importer-worker")),
        &root,
        &ModelProbeRequest {
            spec: "nimora.model-probe/1".to_owned(),
            source: "../secret.glb".into(),
        },
        Duration::from_secs(2),
    )
    .unwrap_err();
    assert!(
        matches!(error, ModelWorkerError::Rejected(message) if message.contains("relative file"))
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn supervisor_terminates_a_worker_after_its_deadline() {
    let root = std::env::temp_dir().join("nimora-model-worker-timeout");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let worker = root.join("slow-worker");
    fs::write(&worker, "#!/bin/sh\nsleep 2\n").unwrap();
    fs::set_permissions(&worker, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(matches!(
        probe_model_in_worker(&worker, &root, &request(), Duration::from_millis(50)),
        Err(ModelWorkerError::TimedOut)
    ));
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn supervisor_reports_a_worker_crash_without_affecting_the_host() {
    let root = std::env::temp_dir().join("nimora-model-worker-crash");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let worker = root.join("crashing-worker");
    fs::write(&worker, "#!/bin/sh\nexit 17\n").unwrap();
    fs::set_permissions(&worker, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(matches!(
        probe_model_in_worker(&worker, &root, &request(), Duration::from_secs(1)),
        Err(ModelWorkerError::Crashed)
    ));
    fs::remove_dir_all(root).unwrap();
}
