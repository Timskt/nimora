use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
};
use thiserror::Error;

const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const MAX_EXECUTABLE_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderWorkerManifest {
    pub spec: String,
    pub worker_protocol_version: u16,
    pub capabilities: Vec<String>,
    pub executable: String,
    pub executable_bytes: u64,
    pub executable_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedProviderWorker {
    pub manifest: ProviderWorkerManifest,
    pub executable_path: PathBuf,
}

#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum SidecarVerificationError {
    #[error("sidecar manifest path is invalid")]
    InvalidManifestPath,
    #[error("sidecar manifest is unavailable")]
    ManifestUnavailable,
    #[error("sidecar manifest exceeds its size budget")]
    ManifestTooLarge,
    #[error("sidecar manifest digest is invalid")]
    ManifestDigestMismatch,
    #[error("sidecar manifest schema is invalid")]
    InvalidManifest,
    #[error("sidecar executable path is invalid")]
    InvalidExecutablePath,
    #[error("sidecar executable is unavailable")]
    ExecutableUnavailable,
    #[error("sidecar executable size is invalid")]
    ExecutableSizeMismatch,
    #[error("sidecar executable digest is invalid")]
    ExecutableDigestMismatch,
}

/// Resolves a sidecar only after checking a host-trusted Manifest digest and executable digest.
///
/// # Errors
///
/// Returns a stable verification error for path traversal, symbolic links, malformed manifests,
/// size violations, missing files, or digest mismatches.
pub fn verify_provider_worker(
    root: &Path,
    manifest_name: &str,
    trusted_manifest_sha256: &str,
) -> Result<VerifiedProviderWorker, SidecarVerificationError> {
    if !single_component(manifest_name) || !valid_digest(trusted_manifest_sha256) {
        return Err(SidecarVerificationError::InvalidManifestPath);
    }
    let canonical_root = root
        .canonicalize()
        .map_err(|_| SidecarVerificationError::ManifestUnavailable)?;
    let manifest_path = canonical_root.join(manifest_name);
    reject_symlink(
        &manifest_path,
        SidecarVerificationError::ManifestUnavailable,
    )?;
    let manifest_bytes = read_bounded(&manifest_path, MAX_MANIFEST_BYTES).map_err(|error| {
        if error == ReadBoundedError::TooLarge {
            SidecarVerificationError::ManifestTooLarge
        } else {
            SidecarVerificationError::ManifestUnavailable
        }
    })?;
    if sha256(&manifest_bytes) != trusted_manifest_sha256 {
        return Err(SidecarVerificationError::ManifestDigestMismatch);
    }
    let manifest: ProviderWorkerManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| SidecarVerificationError::InvalidManifest)?;
    validate_manifest(&manifest)?;
    let executable_path = canonical_root.join(&manifest.executable);
    reject_symlink(
        &executable_path,
        SidecarVerificationError::ExecutableUnavailable,
    )?;
    let canonical_executable = executable_path
        .canonicalize()
        .map_err(|_| SidecarVerificationError::ExecutableUnavailable)?;
    if !canonical_executable.starts_with(&canonical_root) {
        return Err(SidecarVerificationError::InvalidExecutablePath);
    }
    let executable_bytes =
        read_bounded(&canonical_executable, MAX_EXECUTABLE_BYTES).map_err(|error| {
            if error == ReadBoundedError::TooLarge {
                SidecarVerificationError::ExecutableSizeMismatch
            } else {
                SidecarVerificationError::ExecutableUnavailable
            }
        })?;
    if executable_bytes.len() as u64 != manifest.executable_bytes {
        return Err(SidecarVerificationError::ExecutableSizeMismatch);
    }
    if sha256(&executable_bytes) != manifest.executable_sha256 {
        return Err(SidecarVerificationError::ExecutableDigestMismatch);
    }
    Ok(VerifiedProviderWorker {
        manifest,
        executable_path: canonical_executable,
    })
}

fn validate_manifest(manifest: &ProviderWorkerManifest) -> Result<(), SidecarVerificationError> {
    const REQUIRED_CAPABILITIES: [&str; 2] =
        ["provider:ollama-loopback/1", "provider:openai-compatible/1"];
    if manifest.spec != "nimora.provider-worker-manifest/1"
        || manifest.worker_protocol_version != 1
        || manifest
            .capabilities
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            != REQUIRED_CAPABILITIES
        || !single_component(&manifest.executable)
        || manifest.executable_bytes == 0
        || manifest.executable_bytes > MAX_EXECUTABLE_BYTES
        || !valid_digest(&manifest.executable_sha256)
    {
        return Err(SidecarVerificationError::InvalidManifest);
    }
    Ok(())
}

fn single_component(value: &str) -> bool {
    !value.is_empty()
        && Path::new(value).components().count() == 1
        && matches!(
            Path::new(value).components().next(),
            Some(Component::Normal(_))
        )
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn reject_symlink(
    path: &Path,
    error: SidecarVerificationError,
) -> Result<(), SidecarVerificationError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| error)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(error);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadBoundedError {
    Unavailable,
    TooLarge,
}

fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, ReadBoundedError> {
    let metadata = fs::metadata(path).map_err(|_| ReadBoundedError::Unavailable)?;
    if metadata.len() > maximum {
        return Err(ReadBoundedError::TooLarge);
    }
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
    File::open(path)
        .map_err(|_| ReadBoundedError::Unavailable)?
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ReadBoundedError::Unavailable)?;
    if bytes.len() as u64 > maximum {
        return Err(ReadBoundedError::TooLarge);
    }
    Ok(bytes)
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
