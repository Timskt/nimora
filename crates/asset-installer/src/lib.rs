use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallFile {
    pub relative_path: PathBuf,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedInstallFile {
    pub relative_path: PathBuf,
    pub contents: Vec<u8>,
}

const MAX_FILES: usize = 10_000;
const MAX_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
const MAX_METADATA_BYTES: u64 = 1024 * 1024;
const MANIFEST_FILE: &str = "manifest.json";

#[derive(Debug, Error)]
pub enum InstallError {
    #[error("asset source is not a directory")]
    SourceNotDirectory,
    #[error("asset path escapes package root: {0}")]
    UnsafePath(PathBuf),
    #[error("asset file is missing: {0}")]
    MissingFile(PathBuf),
    #[error("asset file resolves outside package root: {0}")]
    EscapedSource(PathBuf),
    #[error("asset inventory exceeds installation budget")]
    BudgetExceeded,
    #[error("asset metadata is invalid: {0}")]
    InvalidMetadata(String),
    #[error("asset SHA-256 is malformed: {0}")]
    InvalidHash(PathBuf),
    #[error("asset size does not match inventory: {0}")]
    SizeMismatch(PathBuf),
    #[error("asset SHA-256 does not match inventory: {0}")]
    HashMismatch(PathBuf),
    #[error("no previous asset version is available")]
    BackupUnavailable,
    #[error("filesystem operation failed: {0}")]
    Io(#[from] io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallResult {
    pub active_path: PathBuf,
    pub backup_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackResult {
    pub active_path: PathBuf,
    pub quarantined_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetPackageInstallResult {
    pub asset_id: String,
    pub version: String,
    pub install: InstallResult,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssetManifestHeader {
    spec: String,
    id: String,
    #[serde(rename = "type")]
    asset_type: String,
    version: String,
    name: serde_json::Value,
    publisher: String,
    license: String,
    engines: serde_json::Value,
    #[serde(default)]
    render: Option<serde_json::Value>,
    #[serde(default)]
    entrypoints: Option<serde_json::Value>,
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default = "empty_json_object")]
    fallbacks: serde_json::Value,
    #[serde(default)]
    locales: Vec<String>,
    integrity: AssetIntegrityReference,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetIntegrityReference {
    algorithm: String,
    files: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssetIntegrityDocument {
    files: Vec<AssetIntegrityFile>,
    total_bytes: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssetIntegrityFile {
    path: PathBuf,
    sha256: String,
    bytes: u64,
    media_type: String,
}

/// Loads package-owned metadata, verifies the declared inventory, and atomically
/// activates the package under the manifest's own asset identifier.
///
/// # Errors
///
/// Returns an error when metadata is missing, malformed, unsafe, inconsistent,
/// or when an inventory file fails validation.
pub fn install_asset_package(
    source_root: &Path,
    asset_store: &Path,
) -> Result<AssetPackageInstallResult, InstallError> {
    let manifest_bytes = read_metadata(source_root, Path::new(MANIFEST_FILE))?;
    let manifest: AssetManifestHeader = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| InstallError::InvalidMetadata(error.to_string()))?;
    validate_manifest_header(&manifest)?;
    let integrity_path = safe_relative_path(&manifest.integrity.files)?;
    if integrity_path == Path::new(MANIFEST_FILE) {
        return Err(InstallError::InvalidMetadata(
            "integrity inventory cannot replace manifest.json".to_owned(),
        ));
    }
    let integrity_bytes = read_metadata(source_root, integrity_path)?;
    let integrity: AssetIntegrityDocument = serde_json::from_slice(&integrity_bytes)
        .map_err(|error| InstallError::InvalidMetadata(error.to_string()))?;
    let files = integrity
        .files
        .into_iter()
        .map(|file| {
            if file.media_type.trim().is_empty() {
                return Err(InstallError::InvalidMetadata(
                    "inventory mediaType cannot be empty".to_owned(),
                ));
            }
            Ok(InstallFile {
                relative_path: file.path,
                bytes: file.bytes,
                sha256: file.sha256,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_integrity_document(&files, integrity.total_bytes, integrity_path)?;
    let active_path = asset_store.join(&manifest.id);
    let install = install_atomically_with_generated(
        source_root,
        &active_path,
        &files,
        &[GeneratedInstallFile {
            relative_path: integrity_path.to_path_buf(),
            contents: integrity_bytes,
        }],
    )?;
    Ok(AssetPackageInstallResult {
        asset_id: manifest.id,
        version: manifest.version,
        install,
    })
}

fn read_metadata(source_root: &Path, relative_path: &Path) -> Result<Vec<u8>, InstallError> {
    if !source_root.is_dir() {
        return Err(InstallError::SourceNotDirectory);
    }
    let relative_path = safe_relative_path(relative_path)?;
    let source_root = source_root.canonicalize()?;
    let path = source_root.join(relative_path);
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_METADATA_BYTES {
        return Err(InstallError::InvalidMetadata(
            relative_path.display().to_string(),
        ));
    }
    let canonical_path = path.canonicalize()?;
    if !canonical_path.starts_with(&source_root) {
        return Err(InstallError::EscapedSource(relative_path.to_path_buf()));
    }
    fs::read(canonical_path).map_err(InstallError::from)
}

fn validate_manifest_header(manifest: &AssetManifestHeader) -> Result<(), InstallError> {
    let supported_types = [
        "character",
        "skin",
        "theme",
        "behavior",
        "voice",
        "interaction",
        "bundle",
    ];
    if manifest.spec != "nimora.asset/1"
        || !valid_asset_identifier(&manifest.id)
        || !valid_asset_identifier(&manifest.publisher)
        || !supported_types.contains(&manifest.asset_type.as_str())
        || !valid_semver(&manifest.version)
        || manifest.license.trim().is_empty()
        || !valid_localized_text(&manifest.name)
        || manifest
            .engines
            .get("nimora")
            .and_then(serde_json::Value::as_str)
            .is_none_or(str::is_empty)
        || manifest.integrity.algorithm != "sha256"
        || manifest.capabilities.len() > 64
        || !manifest
            .capabilities
            .iter()
            .all(|capability| valid_asset_identifier(capability))
        || manifest.locales.len() > 32
        || !manifest.locales.iter().all(|locale| valid_locale(locale))
        || !manifest.fallbacks.is_object()
    {
        return Err(InstallError::InvalidMetadata(
            "manifest header violates nimora.asset/1".to_owned(),
        ));
    }
    let _ = (&manifest.render, &manifest.entrypoints);
    Ok(())
}

fn empty_json_object() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

fn valid_localized_text(value: &serde_json::Value) -> bool {
    value.as_object().is_some_and(|entries| {
        !entries.is_empty()
            && entries.iter().all(|(locale, text)| {
                valid_locale(locale) && text.as_str().is_some_and(|text| !text.trim().is_empty())
            })
    })
}

fn valid_locale(value: &str) -> bool {
    let mut parts = value.split('-');
    let language = parts.next().unwrap_or_default();
    let region = parts.next();
    parts.next().is_none()
        && language.len() == 2
        && language.bytes().all(|byte| byte.is_ascii_lowercase())
        && region.is_none_or(|region| {
            region.len() == 2 && region.bytes().all(|byte| byte.is_ascii_uppercase())
        })
}

fn validate_integrity_document(
    files: &[InstallFile],
    declared_total: u64,
    integrity_path: &Path,
) -> Result<(), InstallError> {
    validate_budget(files)?;
    let mut paths = std::collections::HashSet::with_capacity(files.len());
    let mut total = 0_u64;
    for file in files {
        let path = safe_relative_path(&file.relative_path)?;
        if !paths.insert(path.to_path_buf()) {
            return Err(InstallError::InvalidMetadata(format!(
                "duplicate inventory path: {}",
                path.display()
            )));
        }
        if path == integrity_path {
            return Err(InstallError::InvalidMetadata(
                "integrity inventory cannot hash itself".to_owned(),
            ));
        }
        total = total
            .checked_add(file.bytes)
            .ok_or(InstallError::BudgetExceeded)?;
    }
    if !paths.contains(Path::new(MANIFEST_FILE)) || total != declared_total {
        return Err(InstallError::InvalidMetadata(
            "inventory must include manifest.json and match totalBytes".to_owned(),
        ));
    }
    Ok(())
}

fn valid_asset_identifier(value: &str) -> bool {
    value.len() <= 128
        && value.split('.').count() >= 2
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

fn valid_semver(value: &str) -> bool {
    let core = value.split_once('-').map_or(value, |(core, _)| core);
    let mut segments = core.split('.');
    segments.clone().count() == 3
        && segments
            .all(|segment| !segment.is_empty() && segment.bytes().all(|b| b.is_ascii_digit()))
}

/// Copies a validated inventory into a staging directory and activates it atomically.
///
/// # Errors
///
/// Returns an error when the source is invalid, an inventory path escapes the
/// package root, a listed file is missing, or a filesystem operation fails.
pub fn install_atomically(
    source_root: &Path,
    active_path: &Path,
    files: &[InstallFile],
) -> Result<InstallResult, InstallError> {
    install_atomically_with_generated(source_root, active_path, files, &[])
}

/// Copies a validated inventory plus trusted generated files and activates it atomically.
///
/// # Errors
///
/// Returns an error when either inventory is unsafe, budgets are exceeded, a
/// generated path overlaps a source path, or a filesystem operation fails.
pub fn install_atomically_with_generated(
    source_root: &Path,
    active_path: &Path,
    files: &[InstallFile],
    generated_files: &[GeneratedInstallFile],
) -> Result<InstallResult, InstallError> {
    if !source_root.is_dir() {
        return Err(InstallError::SourceNotDirectory);
    }
    validate_budget(files)?;
    validate_generated_files(files, generated_files)?;
    let canonical_source_root = source_root.canonicalize()?;
    let parent = active_path
        .parent()
        .ok_or_else(|| InstallError::UnsafePath(active_path.to_path_buf()))?;
    fs::create_dir_all(parent)?;
    let staging = unique_sibling(active_path, "staging");
    fs::create_dir(&staging)?;
    let result = (|| {
        for file in files {
            let relative = safe_relative_path(&file.relative_path)?;
            let source = source_root.join(relative);
            if !source.is_file() {
                return Err(InstallError::MissingFile(file.relative_path.clone()));
            }
            let canonical_source = source.canonicalize()?;
            if !canonical_source.starts_with(&canonical_source_root) {
                return Err(InstallError::EscapedSource(file.relative_path.clone()));
            }
            validate_file(&canonical_source, file)?;
            let destination = staging.join(relative);
            if let Some(destination_parent) = destination.parent() {
                fs::create_dir_all(destination_parent)?;
            }
            fs::copy(source, destination)?;
        }
        for file in generated_files {
            let relative = safe_relative_path(&file.relative_path)?;
            let destination = staging.join(relative);
            if let Some(destination_parent) = destination.parent() {
                fs::create_dir_all(destination_parent)?;
            }
            fs::write(destination, &file.contents)?;
        }
        validate_inventory(&staging, files)?;
        let backup = if active_path.exists() {
            let backup = unique_sibling(active_path, "backup");
            fs::rename(active_path, &backup)?;
            Some(backup)
        } else {
            None
        };
        if let Err(error) = fs::rename(&staging, active_path) {
            if let Some(backup_path) = &backup {
                fs::rename(backup_path, active_path)?;
            }
            return Err(InstallError::Io(error));
        }
        Ok(InstallResult {
            active_path: active_path.to_path_buf(),
            backup_path: backup,
        })
    })();
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    result
}

fn validate_generated_files(
    files: &[InstallFile],
    generated_files: &[GeneratedInstallFile],
) -> Result<(), InstallError> {
    let mut paths = std::collections::HashSet::with_capacity(files.len() + generated_files.len());
    for file in files {
        paths.insert(safe_relative_path(&file.relative_path)?.to_path_buf());
    }
    let mut total_bytes = files.iter().try_fold(0_u64, |total, file| {
        total
            .checked_add(file.bytes)
            .ok_or(InstallError::BudgetExceeded)
    })?;
    for file in generated_files {
        let relative = safe_relative_path(&file.relative_path)?.to_path_buf();
        if !paths.insert(relative.clone()) {
            return Err(InstallError::UnsafePath(relative));
        }
        total_bytes = total_bytes
            .checked_add(
                u64::try_from(file.contents.len()).map_err(|_| InstallError::BudgetExceeded)?,
            )
            .ok_or(InstallError::BudgetExceeded)?;
    }
    if paths.len() > MAX_FILES || total_bytes > MAX_TOTAL_BYTES {
        return Err(InstallError::BudgetExceeded);
    }
    Ok(())
}

/// Restores the newest backup next to an active asset directory.
///
/// # Errors
///
/// Returns an error when no backup exists or a filesystem operation fails.
pub fn rollback_latest(active_path: &Path) -> Result<RollbackResult, InstallError> {
    let backup = latest_backup(active_path)?.ok_or(InstallError::BackupUnavailable)?;
    let quarantine = active_path
        .exists()
        .then(|| unique_sibling(active_path, "failed"));
    if let Some(quarantine_path) = &quarantine {
        fs::rename(active_path, quarantine_path)?;
    }
    if let Err(error) = fs::rename(&backup, active_path) {
        if let Some(quarantine_path) = &quarantine {
            fs::rename(quarantine_path, active_path)?;
        }
        return Err(InstallError::Io(error));
    }
    Ok(RollbackResult {
        active_path: active_path.to_path_buf(),
        quarantined_path: quarantine,
    })
}

fn validate_inventory(root: &Path, files: &[InstallFile]) -> Result<(), InstallError> {
    for file in files {
        validate_file(&root.join(safe_relative_path(&file.relative_path)?), file)?;
    }
    Ok(())
}

fn latest_backup(active_path: &Path) -> Result<Option<PathBuf>, InstallError> {
    let parent = active_path
        .parent()
        .ok_or_else(|| InstallError::UnsafePath(active_path.to_path_buf()))?;
    let prefix = format!(
        "{}.backup.",
        active_path
            .file_name()
            .ok_or_else(|| InstallError::UnsafePath(active_path.to_path_buf()))?
            .to_string_lossy()
    );
    let mut backups = fs::read_dir(parent)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    backups.sort_unstable();
    Ok(backups.pop())
}

fn validate_budget(files: &[InstallFile]) -> Result<(), InstallError> {
    if files.is_empty() || files.len() > MAX_FILES {
        return Err(InstallError::BudgetExceeded);
    }
    let total = files
        .iter()
        .try_fold(0_u64, |total, file| total.checked_add(file.bytes));
    if total.is_none_or(|total| total > MAX_TOTAL_BYTES) {
        return Err(InstallError::BudgetExceeded);
    }
    Ok(())
}

fn validate_file(path: &Path, expected: &InstallFile) -> Result<(), InstallError> {
    if expected.sha256.len() != 64
        || !expected
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(InstallError::InvalidHash(expected.relative_path.clone()));
    }
    if fs::metadata(path)?.len() != expected.bytes {
        return Err(InstallError::SizeMismatch(expected.relative_path.clone()));
    }
    let mut source = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
    loop {
        let read = source.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    if format!("{:x}", digest.finalize()) != expected.sha256 {
        return Err(InstallError::HashMismatch(expected.relative_path.clone()));
    }
    Ok(())
}

fn safe_relative_path(path: &Path) -> Result<&Path, InstallError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(InstallError::UnsafePath(path.to_path_buf()));
    }
    Ok(path)
}

fn unique_sibling(active_path: &Path, suffix: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    active_path.with_file_name(format!(
        "{}.{}.{}",
        active_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy(),
        suffix,
        stamp
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sha256(contents: &[u8]) -> String {
        format!("{:x}", Sha256::digest(contents))
    }

    fn write_asset_package(root: &Path, asset_id: &str) {
        fs::create_dir_all(root).unwrap();
        let manifest = serde_json::to_vec(&serde_json::json!({
            "spec": "nimora.asset/1",
            "id": asset_id,
            "type": "character",
            "version": "1.0.0",
            "name": { "en": "Mochi" },
            "publisher": "publisher.example",
            "license": "MIT",
            "engines": { "nimora": ">=0.1.0" },
            "capabilities": [],
            "fallbacks": {},
            "locales": ["en"],
            "integrity": { "algorithm": "sha256", "files": "integrity.json" }
        }))
        .unwrap();
        fs::write(root.join(MANIFEST_FILE), &manifest).unwrap();
        let integrity = serde_json::to_vec(&serde_json::json!({
            "files": [{
                "path": MANIFEST_FILE,
                "sha256": sha256(&manifest),
                "bytes": manifest.len(),
                "mediaType": "application/json"
            }],
            "totalBytes": manifest.len()
        }))
        .unwrap();
        fs::write(root.join("integrity.json"), integrity).unwrap();
    }

    #[test]
    fn installs_package_using_manifest_owned_identity_and_inventory() {
        let root = std::env::temp_dir().join("nimora-package-authority");
        let source = root.join("source");
        let store = root.join("store");
        let _ = fs::remove_dir_all(&root);
        write_asset_package(&source, "character.example.mochi");
        let result = install_asset_package(&source, &store).unwrap();
        assert_eq!(result.asset_id, "character.example.mochi");
        assert_eq!(result.version, "1.0.0");
        assert!(
            store
                .join("character.example.mochi/manifest.json")
                .is_file()
        );
        assert!(
            store
                .join("character.example.mochi/integrity.json")
                .is_file()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_self_referential_integrity_inventory() {
        let root = std::env::temp_dir().join("nimora-package-self-integrity");
        let source = root.join("source");
        let store = root.join("store");
        let _ = fs::remove_dir_all(&root);
        write_asset_package(&source, "character.example.mochi");
        let integrity = fs::read(source.join("integrity.json")).unwrap();
        fs::write(
            source.join("integrity.json"),
            serde_json::to_vec(&serde_json::json!({
                "files": [{
                    "path": "integrity.json",
                    "sha256": sha256(&integrity),
                    "bytes": integrity.len(),
                    "mediaType": "application/json"
                }],
                "totalBytes": integrity.len()
            }))
            .unwrap(),
        )
        .unwrap();
        let error = install_asset_package(&source, &store).unwrap_err();
        assert!(matches!(error, InstallError::InvalidMetadata(_)));
        assert!(!store.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn installs_files_and_preserves_previous_directory() {
        let root = std::env::temp_dir().join(format!(
            "nimora-installer-{}",
            unique_sibling(Path::new("x"), "test").display()
        ));
        let source = root.join("source");
        let active = root.join("active");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("manifest.json"), b"new").unwrap();
        fs::create_dir_all(&active).unwrap();
        fs::write(active.join("old.txt"), b"old").unwrap();
        let result = install_atomically(
            &source,
            &active,
            &[InstallFile {
                relative_path: "manifest.json".into(),
                bytes: 3,
                sha256: "11507a0e2f5e69d5dfa40a62a1bd7b6ee57e6bcd85c67c9b8431b36fff21c437".into(),
            }],
        )
        .unwrap();
        assert_eq!(fs::read(active.join("manifest.json")).unwrap(), b"new");
        assert_eq!(
            fs::read(result.backup_path.unwrap().join("old.txt")).unwrap(),
            b"old"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_path_escape_before_copying() {
        let root = std::env::temp_dir().join("nimora-installer-escape");
        fs::create_dir_all(&root).unwrap();
        let error = install_atomically(
            &root,
            &root.join("active"),
            &[InstallFile {
                relative_path: "../secret".into(),
                bytes: 0,
                sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into(),
            }],
        )
        .unwrap_err();
        assert!(matches!(error, InstallError::UnsafePath(_)));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape_from_source_root() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join("nimora-installer-symlink");
        let source = root.join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(root.join("secret"), b"secret").unwrap();
        symlink(root.join("secret"), source.join("linked")).unwrap();
        let error = install_atomically(
            &source,
            &root.join("active"),
            &[InstallFile {
                relative_path: "linked".into(),
                bytes: 6,
                sha256: "2bb80d537b1da3e38bd30361aa855686bde0eacd7162fef6a25fe97bf527a25b".into(),
            }],
        )
        .unwrap_err();
        assert!(matches!(error, InstallError::EscapedSource(_)));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_inventory_hash_mismatch_without_replacing_active() {
        let root = std::env::temp_dir().join("nimora-installer-hash");
        let source = root.join("source");
        let active = root.join("active");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&active).unwrap();
        fs::write(source.join("manifest.json"), b"new").unwrap();
        fs::write(active.join("old.txt"), b"old").unwrap();
        let error = install_atomically(
            &source,
            &active,
            &[InstallFile {
                relative_path: "manifest.json".into(),
                bytes: 3,
                sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into(),
            }],
        )
        .unwrap_err();
        assert!(matches!(error, InstallError::HashMismatch(_)));
        assert_eq!(fs::read(active.join("old.txt")).unwrap(), b"old");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn installs_trusted_generated_files_in_the_atomic_activation() {
        let root = std::env::temp_dir().join("nimora-installer-generated");
        let source = root.join("source");
        let active = root.join("active");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("manifest.json"), b"new").unwrap();
        install_atomically_with_generated(
            &source,
            &active,
            &[InstallFile {
                relative_path: "manifest.json".into(),
                bytes: 3,
                sha256: "11507a0e2f5e69d5dfa40a62a1bd7b6ee57e6bcd85c67c9b8431b36fff21c437".into(),
            }],
            &[GeneratedInstallFile {
                relative_path: ".integrity.json".into(),
                contents: b"trusted".to_vec(),
            }],
        )
        .unwrap();
        assert_eq!(
            fs::read(active.join(".integrity.json")).unwrap(),
            b"trusted"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_generated_files_that_overlap_source_inventory() {
        let root = std::env::temp_dir().join("nimora-installer-generated-overlap");
        let source = root.join("source");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&source).unwrap();
        let error = install_atomically_with_generated(
            &source,
            &root.join("active"),
            &[InstallFile {
                relative_path: "manifest.json".into(),
                bytes: 0,
                sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into(),
            }],
            &[GeneratedInstallFile {
                relative_path: "manifest.json".into(),
                contents: Vec::new(),
            }],
        )
        .unwrap_err();
        assert!(
            matches!(error, InstallError::UnsafePath(path) if path == Path::new("manifest.json"))
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn restores_latest_backup_and_quarantines_failed_version() {
        let root = std::env::temp_dir().join("nimora-installer-rollback");
        let active = root.join("character.example.mochi");
        let older = root.join("character.example.mochi.backup.1");
        let latest = root.join("character.example.mochi.backup.2");
        fs::create_dir_all(&active).unwrap();
        fs::create_dir_all(&older).unwrap();
        fs::create_dir_all(&latest).unwrap();
        fs::write(active.join("version"), b"broken").unwrap();
        fs::write(older.join("version"), b"one").unwrap();
        fs::write(latest.join("version"), b"two").unwrap();
        let result = rollback_latest(&active).unwrap();
        assert_eq!(fs::read(active.join("version")).unwrap(), b"two");
        assert_eq!(
            fs::read(result.quarantined_path.unwrap().join("version")).unwrap(),
            b"broken"
        );
        assert!(older.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
