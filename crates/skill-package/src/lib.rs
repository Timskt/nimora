use nimora_asset_installer::{
    GeneratedInstallFile, InstallError, InstallFile, InstallResult, RollbackResult,
    install_atomically_with_generated, rollback_latest,
};
use nimora_skill_runtime::{
    SKILL_SPEC, SkillContributions, SkillManifest, ValidatedSkillManifest, validate_manifest,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

const MANIFEST_FILE: &str = "manifest.json";
const INTEGRITY_FILE: &str = ".nimora-skill-integrity.json";
const INTEGRITY_SCHEMA_VERSION: u32 = 1;
const MAX_PACKAGE_FILES: usize = 256;
const MAX_PACKAGE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum SkillPackageError {
    #[error("Skill package must contain manifest.json and its declared entrypoint")]
    MissingRequiredFile,
    #[error("Skill package exceeds the 256-file or 16 MiB budget")]
    BudgetExceeded,
    #[error("Skill package manifest does not match the requested manifest")]
    ManifestMismatch,
    #[error("Skill package source is not a directory")]
    SourceNotDirectory,
    #[error("Skill package contains a duplicate or reserved path: {0}")]
    DuplicatePath(PathBuf),
    #[error("Skill package inventory path is not valid UTF-8: {0}")]
    InvalidInventoryPath(PathBuf),
    #[error("Skill package manifest resolves outside the source root")]
    ManifestEscapedSource,
    #[error("installed Skill is unavailable")]
    InstalledSkillUnavailable,
    #[error("installed Skill entry resolves outside its active directory")]
    InstalledSkillEscaped,
    #[error("installed Skill failed its integrity check")]
    InstalledSkillIntegrity,
    #[error(transparent)]
    Manifest(#[from] nimora_skill_runtime::SkillError),
    #[error(transparent)]
    Install(#[from] InstallError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillInstallResult {
    pub skill_id: String,
    pub version: String,
    pub active_path: PathBuf,
    pub backup_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledSkill {
    pub manifest: ValidatedSkillManifest,
    pub source: String,
    pub active_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SkillIntegrityLock {
    schema_version: u32,
    skill_id: String,
    version: String,
    files: Vec<LockedSkillFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LockedSkillFile {
    path: String,
    bytes: u64,
    sha256: String,
}

/// Validates, inventories, and atomically activates one Skill package.
///
/// # Errors
///
/// Returns an error for invalid manifests, mismatched package identity,
/// unsafe inventory, budget violations, or filesystem failures.
pub fn install_skill_atomically(
    source_root: &Path,
    skill_store: &Path,
    expected_manifest: SkillManifest,
    files: &[InstallFile],
) -> Result<SkillInstallResult, SkillPackageError> {
    if !source_root.is_dir() {
        return Err(SkillPackageError::SourceNotDirectory);
    }
    let validated = validate_manifest(expected_manifest)?;
    validate_inventory_contract(files, validated.manifest())?;
    let canonical_source_root = source_root.canonicalize()?;
    let manifest_path = source_root.join(MANIFEST_FILE).canonicalize()?;
    if !manifest_path.starts_with(&canonical_source_root) {
        return Err(SkillPackageError::ManifestEscapedSource);
    }
    let packaged_manifest = serde_json::from_slice::<SkillManifest>(&fs::read(manifest_path)?)?;
    if &packaged_manifest != validated.manifest() {
        return Err(SkillPackageError::ManifestMismatch);
    }
    let integrity_lock = create_integrity_lock(validated.manifest(), files)?;
    let active_path = skill_store.join(&validated.manifest().id).join("active");
    let InstallResult {
        active_path,
        backup_path,
    } = install_atomically_with_generated(
        source_root,
        &active_path,
        files,
        &[GeneratedInstallFile {
            relative_path: INTEGRITY_FILE.into(),
            contents: serde_json::to_vec_pretty(&integrity_lock)?,
        }],
    )?;
    Ok(SkillInstallResult {
        skill_id: validated.manifest().id.clone(),
        version: validated.manifest().version.clone(),
        active_path,
        backup_path,
    })
}

/// Restores the latest previous active version for one Skill.
///
/// # Errors
///
/// Returns an error for an invalid identifier, missing backup, or filesystem failure.
pub fn rollback_skill(
    skill_store: &Path,
    skill_id: &str,
) -> Result<RollbackResult, SkillPackageError> {
    validate_skill_id(skill_id)?;
    Ok(rollback_latest(&skill_store.join(skill_id).join("active"))?)
}

/// Loads and re-verifies the active Skill Manifest and JavaScript entrypoint.
///
/// # Errors
///
/// Returns an error for invalid identity, missing files, path escape, inventory
/// drift, hash mismatch, invalid Manifest, or non-UTF-8 JavaScript.
pub fn load_installed_skill(
    skill_store: &Path,
    skill_id: &str,
) -> Result<InstalledSkill, SkillPackageError> {
    validate_skill_id(skill_id)?;
    let active_path = skill_store.join(skill_id).join("active");
    if !active_path.is_dir() {
        return Err(SkillPackageError::InstalledSkillUnavailable);
    }
    let canonical_active = active_path.canonicalize()?;
    verify_installed_integrity(&canonical_active, skill_id)?;
    let manifest_path = canonical_installed_file(&canonical_active, Path::new(MANIFEST_FILE))?;
    let manifest = validate_manifest(serde_json::from_slice::<SkillManifest>(&fs::read(
        manifest_path,
    )?)?)?;
    if manifest.manifest().id != skill_id {
        return Err(SkillPackageError::ManifestMismatch);
    }
    let entry_path = canonical_installed_file(
        &canonical_active,
        Path::new(&manifest.manifest().entrypoint),
    )?;
    let source = fs::read_to_string(entry_path)?;
    Ok(InstalledSkill {
        manifest,
        source,
        active_path,
    })
}

fn create_integrity_lock(
    manifest: &SkillManifest,
    files: &[InstallFile],
) -> Result<SkillIntegrityLock, SkillPackageError> {
    let mut locked_files = files
        .iter()
        .map(|file| {
            if file.relative_path == Path::new(INTEGRITY_FILE) {
                return Err(SkillPackageError::DuplicatePath(file.relative_path.clone()));
            }
            let path = file.relative_path.to_str().ok_or_else(|| {
                SkillPackageError::InvalidInventoryPath(file.relative_path.clone())
            })?;
            Ok(LockedSkillFile {
                path: path.to_owned(),
                bytes: file.bytes,
                sha256: file.sha256.to_ascii_lowercase(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    locked_files.sort_unstable_by(|left, right| left.path.cmp(&right.path));
    Ok(SkillIntegrityLock {
        schema_version: INTEGRITY_SCHEMA_VERSION,
        skill_id: manifest.id.clone(),
        version: manifest.version.clone(),
        files: locked_files,
    })
}

fn verify_installed_integrity(
    canonical_active: &Path,
    expected_skill_id: &str,
) -> Result<(), SkillPackageError> {
    let lock_path = canonical_installed_file(canonical_active, Path::new(INTEGRITY_FILE))?;
    let lock = serde_json::from_slice::<SkillIntegrityLock>(&fs::read(lock_path)?)
        .map_err(|_| SkillPackageError::InstalledSkillIntegrity)?;
    if lock.schema_version != INTEGRITY_SCHEMA_VERSION || lock.skill_id != expected_skill_id {
        return Err(SkillPackageError::InstalledSkillIntegrity);
    }
    let manifest_path = canonical_installed_file(canonical_active, Path::new(MANIFEST_FILE))?;
    let manifest = serde_json::from_slice::<SkillManifest>(&fs::read(manifest_path)?)
        .map_err(|_| SkillPackageError::InstalledSkillIntegrity)?;
    if manifest.id != lock.skill_id || manifest.version != lock.version {
        return Err(SkillPackageError::InstalledSkillIntegrity);
    }
    let mut expected = lock
        .files
        .iter()
        .map(|file| (PathBuf::from(&file.path), file))
        .collect::<HashMap<_, _>>();
    if expected.len() != lock.files.len() || expected.contains_key(Path::new(INTEGRITY_FILE)) {
        return Err(SkillPackageError::InstalledSkillIntegrity);
    }
    verify_directory(canonical_active, canonical_active, &mut expected)?;
    if !expected.is_empty() {
        return Err(SkillPackageError::InstalledSkillIntegrity);
    }
    Ok(())
}

fn verify_directory(
    canonical_active: &Path,
    directory: &Path,
    expected: &mut HashMap<PathBuf, &LockedSkillFile>,
) -> Result<(), SkillPackageError> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path
            .strip_prefix(canonical_active)
            .map_err(|_| SkillPackageError::InstalledSkillIntegrity)?;
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(SkillPackageError::InstalledSkillIntegrity);
        }
        if metadata.is_dir() {
            verify_directory(canonical_active, &path, expected)?;
        } else if relative != Path::new(INTEGRITY_FILE) {
            let locked = expected
                .remove(relative)
                .ok_or(SkillPackageError::InstalledSkillIntegrity)?;
            if !metadata.is_file() || metadata.len() != locked.bytes {
                return Err(SkillPackageError::InstalledSkillIntegrity);
            }
            let digest = format!("{:x}", Sha256::digest(fs::read(&path)?));
            if digest != locked.sha256 {
                return Err(SkillPackageError::InstalledSkillIntegrity);
            }
        }
    }
    Ok(())
}

fn canonical_installed_file(
    canonical_active: &Path,
    relative_path: &Path,
) -> Result<PathBuf, SkillPackageError> {
    let path = canonical_active.join(relative_path);
    if !path.is_file() {
        return Err(SkillPackageError::InstalledSkillUnavailable);
    }
    let canonical = path.canonicalize()?;
    if !canonical.starts_with(canonical_active) {
        return Err(SkillPackageError::InstalledSkillEscaped);
    }
    Ok(canonical)
}

fn validate_inventory_contract(
    files: &[InstallFile],
    manifest: &SkillManifest,
) -> Result<(), SkillPackageError> {
    let mut paths = HashSet::with_capacity(files.len());
    for file in files {
        if file.relative_path == Path::new(INTEGRITY_FILE)
            || !paths.insert(file.relative_path.clone())
        {
            return Err(SkillPackageError::DuplicatePath(file.relative_path.clone()));
        }
        file.relative_path
            .to_str()
            .ok_or_else(|| SkillPackageError::InvalidInventoryPath(file.relative_path.clone()))?;
    }
    let total_bytes = files.iter().try_fold(0_u64, |total, file| {
        total
            .checked_add(file.bytes)
            .ok_or(SkillPackageError::BudgetExceeded)
    })?;
    if files.len() > MAX_PACKAGE_FILES || total_bytes > MAX_PACKAGE_BYTES {
        return Err(SkillPackageError::BudgetExceeded);
    }
    let has_manifest = files
        .iter()
        .any(|file| file.relative_path == Path::new(MANIFEST_FILE));
    let has_entry = files
        .iter()
        .any(|file| file.relative_path == Path::new(&manifest.entrypoint));
    if !has_manifest || !has_entry {
        return Err(SkillPackageError::MissingRequiredFile);
    }
    Ok(())
}

fn validate_skill_id(skill_id: &str) -> Result<(), SkillPackageError> {
    validate_manifest(SkillManifest {
        spec: SKILL_SPEC.to_owned(),
        id: skill_id.to_owned(),
        version: "0.0.0".to_owned(),
        publisher: "system.validation".to_owned(),
        entrypoint: "main.js".to_owned(),
        capabilities: BTreeSet::default(),
        activation_events: BTreeSet::default(),
        command_allowlist: BTreeSet::default(),
        contributions: SkillContributions::default(),
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimora_skill_runtime::{SkillCapability, SkillGrant, SkillHost};

    fn manifest(version: &str) -> SkillManifest {
        SkillManifest {
            spec: SKILL_SPEC.to_owned(),
            id: "studio.example.focus".to_owned(),
            version: version.to_owned(),
            publisher: "studio.example".to_owned(),
            entrypoint: "dist/main.js".to_owned(),
            capabilities: BTreeSet::from([SkillCapability::InvokeCommands]),
            activation_events: BTreeSet::from(["onStartup".to_owned()]),
            command_allowlist: BTreeSet::from(["safe.pet.animate".to_owned()]),
            contributions: SkillContributions::default(),
        }
    }

    fn inventory(path: &str, bytes: &[u8]) -> InstallFile {
        InstallFile {
            relative_path: path.into(),
            bytes: u64::try_from(bytes.len()).unwrap(),
            sha256: format!("{:x}", Sha256::digest(bytes)),
        }
    }

    fn write_package(root: &Path, manifest: &SkillManifest, source: &str) -> Vec<InstallFile> {
        fs::create_dir_all(root.join("dist")).unwrap();
        let manifest_bytes = serde_json::to_vec(manifest).unwrap();
        fs::write(root.join(MANIFEST_FILE), &manifest_bytes).unwrap();
        fs::write(root.join("dist/main.js"), source).unwrap();
        vec![
            inventory(MANIFEST_FILE, &manifest_bytes),
            inventory("dist/main.js", source.as_bytes()),
        ]
    }

    fn test_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "nimora-skill-package-{name}-{}",
            std::process::id()
        ))
    }

    #[test]
    fn installs_loads_and_rolls_back_verified_versions() {
        let root = test_root("rollback");
        let _ = fs::remove_dir_all(&root);
        let source = root.join("source");
        let store = root.join("store");
        let v1 = manifest("1.0.0");
        let files = write_package(&source, &v1, "nimora.commands.invoke('pet.wave');");
        install_skill_atomically(&source, &store, v1, &files).unwrap();
        fs::remove_dir_all(&source).unwrap();
        let v2 = manifest("2.0.0");
        let files = write_package(&source, &v2, "nimora.commands.invoke('pet.sleep');");
        let result = install_skill_atomically(&source, &store, v2, &files).unwrap();
        assert!(result.backup_path.is_some());
        assert_eq!(
            load_installed_skill(&store, "studio.example.focus")
                .unwrap()
                .manifest
                .manifest()
                .version,
            "2.0.0"
        );
        rollback_skill(&store, "studio.example.focus").unwrap();
        let restored = load_installed_skill(&store, "studio.example.focus").unwrap();
        assert_eq!(restored.manifest.manifest().version, "1.0.0");
        assert!(restored.source.contains("pet.wave"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_manifest_mismatch_and_missing_declared_entrypoint() {
        let root = test_root("contract");
        let _ = fs::remove_dir_all(&root);
        let packaged = manifest("1.0.0");
        let files = write_package(&root, &packaged, "");
        let mut expected = packaged.clone();
        expected.version = "2.0.0".to_owned();
        assert!(matches!(
            install_skill_atomically(&root, &root.join("store"), expected, &files),
            Err(SkillPackageError::ManifestMismatch)
        ));
        let missing_entry = vec![files[0].clone()];
        assert!(matches!(
            install_skill_atomically(&root, &root.join("store"), packaged, &missing_entry,),
            Err(SkillPackageError::MissingRequiredFile)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_source_tampering_and_untracked_installed_files() {
        let root = test_root("integrity");
        let _ = fs::remove_dir_all(&root);
        let source = root.join("source");
        let store = root.join("store");
        let skill_manifest = manifest("1.0.0");
        let files = write_package(&source, &skill_manifest, "null;");
        install_skill_atomically(&source, &store, skill_manifest, &files).unwrap();
        let active = store.join("studio.example.focus/active");
        fs::write(active.join("dist/main.js"), "tampered;").unwrap();
        assert!(matches!(
            load_installed_skill(&store, "studio.example.focus"),
            Err(SkillPackageError::InstalledSkillIntegrity)
        ));
        fs::write(active.join("dist/main.js"), "null;").unwrap();
        fs::write(active.join("extra.js"), "unknown").unwrap();
        assert!(matches!(
            load_installed_skill(&store, "studio.example.focus"),
            Err(SkillPackageError::InstalledSkillIntegrity)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn verified_manifest_becomes_the_exact_active_runtime_lease() {
        let root = test_root("runtime-lease");
        let _ = fs::remove_dir_all(&root);
        let source = root.join("source");
        let store = root.join("store");
        let skill_manifest = manifest("1.0.0");
        let files = write_package(&source, &skill_manifest, "null;");
        install_skill_atomically(&source, &store, skill_manifest, &files).unwrap();

        let installed = load_installed_skill(&store, "studio.example.focus").unwrap();
        let grant = SkillGrant {
            skill_id: installed.manifest.manifest().id.clone(),
            version: installed.manifest.manifest().version.clone(),
            capabilities: installed.manifest.manifest().capabilities.clone(),
        };
        let mut host = SkillHost::default();
        host.install(installed.manifest.clone()).unwrap();
        host.authorize(grant).unwrap();
        host.activate("studio.example.focus").unwrap();

        assert_eq!(
            host.active_manifest("studio.example.focus").unwrap(),
            installed.manifest.manifest()
        );
        fs::remove_dir_all(root).unwrap();
    }
}
