use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use thiserror::Error;

const GLB_MAGIC: &[u8; 4] = b"glTF";
const GLB_JSON_CHUNK: u32 = 0x4e4f_534a;
const GLB_BINARY_CHUNK: u32 = 0x004e_4942;
const MAX_MODEL_BYTES: u64 = 80 * 1024 * 1024;
const MAX_JSON_BYTES: usize = 1024 * 1024;
const MAX_NODES: usize = 10_000;
const MAX_MESHES: usize = 2_000;
const MAX_MATERIALS: usize = 1_000;
const MAX_TEXTURES: usize = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelProbeRequest {
    pub spec: String,
    pub source: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProbeReport {
    pub spec: String,
    pub format: String,
    pub format_version: String,
    pub bytes: u64,
    pub json_bytes: usize,
    pub binary_bytes: usize,
    pub nodes: usize,
    pub meshes: usize,
    pub materials: usize,
    pub textures: usize,
    pub animations: usize,
    pub skins: usize,
}

#[derive(Debug, Error)]
pub enum ModelImportError {
    #[error("model probe request violates nimora.model-probe/1")]
    InvalidRequest,
    #[error("model source must be a relative file in the isolated staging directory")]
    UnsafeSource,
    #[error("model source is not a regular file")]
    InvalidSource,
    #[error("model exceeds the 80 MiB import budget")]
    InputBudgetExceeded,
    #[error("GLB container is malformed: {0}")]
    InvalidGlb(&'static str),
    #[error("GLB JSON metadata is invalid: {0}")]
    InvalidJson(String),
    #[error("GLB resource counts exceed the import budget")]
    ResourceBudgetExceeded,
    #[error("model filesystem operation failed: {0}")]
    Io(#[from] io::Error),
}

#[derive(Debug, Error)]
pub enum ModelWorkerError {
    #[error("failed to start model importer worker: {0}")]
    Start(String),
    #[error("model importer worker timed out")]
    TimedOut,
    #[error("model importer worker crashed")]
    Crashed,
    #[error("model importer worker exceeded its output budget")]
    OutputBudgetExceeded,
    #[error("model importer worker protocol failed: {0}")]
    Protocol(String),
    #[error("model was rejected: {0}")]
    Rejected(String),
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum WorkerResponse {
    Result { report: ModelProbeReport },
    Error { code: String, message: String },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GltfDocument {
    asset: GltfAsset,
    #[serde(default)]
    nodes: Vec<serde_json::Value>,
    #[serde(default)]
    meshes: Vec<serde_json::Value>,
    #[serde(default)]
    materials: Vec<serde_json::Value>,
    #[serde(default)]
    textures: Vec<serde_json::Value>,
    #[serde(default)]
    animations: Vec<serde_json::Value>,
    #[serde(default)]
    skins: Vec<serde_json::Value>,
    #[serde(default)]
    buffers: Vec<GltfBuffer>,
    #[serde(default)]
    images: Vec<GltfImage>,
}

#[derive(Debug, Deserialize)]
struct GltfAsset {
    version: String,
}

#[derive(Debug, Deserialize)]
struct GltfBuffer {
    uri: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GltfImage {
    uri: Option<String>,
}

/// Probes one GLB file from an isolated staging directory without producing
/// installable output.
///
/// # Errors
///
/// Returns an error for unsafe paths, malformed containers, external URIs, or
/// any exceeded input or resource budget.
pub fn probe_staged_model(
    staging_root: &Path,
    request: &ModelProbeRequest,
) -> Result<ModelProbeReport, ModelImportError> {
    if request.spec != "nimora.model-probe/1" {
        return Err(ModelImportError::InvalidRequest);
    }
    let source = safe_relative_file(&request.source)?;
    let root = staging_root.canonicalize()?;
    let path = root.join(source);
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.file_type().is_file() {
        return Err(ModelImportError::InvalidSource);
    }
    if metadata.len() > MAX_MODEL_BYTES {
        return Err(ModelImportError::InputBudgetExceeded);
    }
    let canonical = path.canonicalize()?;
    if !canonical.starts_with(&root) {
        return Err(ModelImportError::UnsafeSource);
    }
    let mut bytes = Vec::with_capacity(metadata.len().try_into().unwrap_or(MAX_JSON_BYTES));
    fs::File::open(canonical)?
        .take(MAX_MODEL_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MODEL_BYTES {
        return Err(ModelImportError::InputBudgetExceeded);
    }
    probe_glb(&bytes)
}

/// Runs the model probe in a one-shot child process with a deadline and bounded
/// protocol output.
///
/// # Errors
///
/// Returns a structured error when the worker cannot start, crashes, times out,
/// violates the protocol, exceeds its output budget, or rejects the model.
pub fn probe_model_in_worker(
    executable: &Path,
    staging_root: &Path,
    request: &ModelProbeRequest,
    timeout: Duration,
) -> Result<ModelProbeReport, ModelWorkerError> {
    let payload = serde_json::to_string(request)
        .map_err(|error| ModelWorkerError::Protocol(error.to_string()))?;
    let mut child = Command::new(executable)
        .arg(payload)
        .current_dir(staging_root)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| ModelWorkerError::Start(error.to_string()))?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut output = Vec::new();
                child
                    .stdout
                    .take()
                    .ok_or_else(|| ModelWorkerError::Protocol("stdout unavailable".to_owned()))?
                    .take(64 * 1024 + 1)
                    .read_to_end(&mut output)
                    .map_err(|error| ModelWorkerError::Protocol(error.to_string()))?;
                if output.len() > 64 * 1024 {
                    return Err(ModelWorkerError::OutputBudgetExceeded);
                }
                if !status.success() {
                    return Err(ModelWorkerError::Crashed);
                }
                return match serde_json::from_slice::<WorkerResponse>(&output)
                    .map_err(|error| ModelWorkerError::Protocol(error.to_string()))?
                {
                    WorkerResponse::Result { report } => Ok(report),
                    WorkerResponse::Error { code, message } => {
                        Err(ModelWorkerError::Rejected(format!("{code}: {message}")))
                    }
                };
            }
            Ok(None) if started.elapsed() < timeout => thread::sleep(Duration::from_millis(5)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ModelWorkerError::TimedOut);
            }
            Err(_) => return Err(ModelWorkerError::Crashed),
        }
    }
}

fn probe_glb(bytes: &[u8]) -> Result<ModelProbeReport, ModelImportError> {
    if bytes.len() < 20 || bytes.get(..4) != Some(GLB_MAGIC) {
        return Err(ModelImportError::InvalidGlb("missing GLB header"));
    }
    let version = read_u32(bytes, 4)?;
    if version != 2 {
        return Err(ModelImportError::InvalidGlb("only GLB 2.0 is accepted"));
    }
    if usize::try_from(read_u32(bytes, 8)?).ok() != Some(bytes.len()) {
        return Err(ModelImportError::InvalidGlb(
            "declared length does not match file",
        ));
    }
    let json_length = usize::try_from(read_u32(bytes, 12)?)
        .map_err(|_| ModelImportError::InvalidGlb("JSON chunk length overflows"))?;
    if read_u32(bytes, 16)? != GLB_JSON_CHUNK || json_length == 0 || json_length > MAX_JSON_BYTES {
        return Err(ModelImportError::InvalidGlb(
            "first chunk must be bounded JSON",
        ));
    }
    let json_end = 20_usize
        .checked_add(json_length)
        .filter(|end| *end <= bytes.len())
        .ok_or(ModelImportError::InvalidGlb("JSON chunk exceeds container"))?;
    let document: GltfDocument = serde_json::from_slice(&bytes[20..json_end])
        .map_err(|error| ModelImportError::InvalidJson(error.to_string()))?;
    if document.asset.version != "2.0" {
        return Err(ModelImportError::InvalidGlb("asset.version must be 2.0"));
    }
    if document
        .buffers
        .iter()
        .filter_map(|buffer| buffer.uri.as_deref())
        .chain(
            document
                .images
                .iter()
                .filter_map(|image| image.uri.as_deref()),
        )
        .next()
        .is_some()
    {
        return Err(ModelImportError::InvalidGlb(
            "external and data URIs are not accepted in GLB",
        ));
    }
    if document.nodes.len() > MAX_NODES
        || document.meshes.len() > MAX_MESHES
        || document.materials.len() > MAX_MATERIALS
        || document.textures.len() > MAX_TEXTURES
    {
        return Err(ModelImportError::ResourceBudgetExceeded);
    }
    let binary_bytes = parse_optional_binary_chunk(bytes, json_end)?;
    Ok(ModelProbeReport {
        spec: "nimora.model-probe-report/1".to_owned(),
        format: "glb".to_owned(),
        format_version: document.asset.version,
        bytes: bytes.len() as u64,
        json_bytes: json_length,
        binary_bytes,
        nodes: document.nodes.len(),
        meshes: document.meshes.len(),
        materials: document.materials.len(),
        textures: document.textures.len(),
        animations: document.animations.len(),
        skins: document.skins.len(),
    })
}

fn parse_optional_binary_chunk(bytes: &[u8], json_end: usize) -> Result<usize, ModelImportError> {
    if json_end == bytes.len() {
        return Ok(0);
    }
    if bytes.len().saturating_sub(json_end) < 8 {
        return Err(ModelImportError::InvalidGlb(
            "truncated binary chunk header",
        ));
    }
    let length = usize::try_from(read_u32(bytes, json_end)?)
        .map_err(|_| ModelImportError::InvalidGlb("binary chunk length overflows"))?;
    if read_u32(bytes, json_end + 4)? != GLB_BINARY_CHUNK
        || json_end
            .checked_add(8)
            .and_then(|start| start.checked_add(length))
            != Some(bytes.len())
    {
        return Err(ModelImportError::InvalidGlb(
            "invalid trailing binary chunk",
        ));
    }
    Ok(length)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, ModelImportError> {
    bytes
        .get(offset..offset + 4)
        .and_then(|slice| slice.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or(ModelImportError::InvalidGlb("truncated integer"))
}

fn safe_relative_file(path: &Path) -> Result<&Path, ModelImportError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().count() != 1
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ModelImportError::UnsafeSource);
    }
    Ok(path)
}
