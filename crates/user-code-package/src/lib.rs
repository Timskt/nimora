use nimora_asset_installer::{
    InstallError, InstallFile, InstallResult, RollbackResult, install_atomically, rollback_latest,
};
use nimora_user_code_policy::{PolicyError, ProgramManifest, evaluate};
use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
};
use thiserror::Error;

const MANIFEST_FILE: &str = "manifest.json";
const ENTRY_FILE: &str = "main.js";
const MAX_PACKAGE_FILES: usize = 64;
const MAX_PACKAGE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum ProgramPackageError {
    #[error("program package must contain manifest.json and main.js")]
    MissingRequiredFile,
    #[error("program package exceeds the 64-file or 2 MiB budget")]
    BudgetExceeded,
    #[error("program package manifest does not match the requested manifest")]
    ManifestMismatch,
    #[error("program package source is not a directory")]
    SourceNotDirectory,
    #[error("program package contains a duplicate path: {0}")]
    DuplicatePath(PathBuf),
    #[error("program package manifest resolves outside the package root")]
    ManifestEscapedSource,
    #[error(transparent)]
    Policy(#[from] PolicyError),
    #[error(transparent)]
    Install(#[from] InstallError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramInstallResult {
    pub program_id: String,
    pub version: String,
    pub active_path: PathBuf,
    pub backup_path: Option<PathBuf>,
}

/// Validates and atomically activates one user program package.
///
/// # Errors
///
/// Returns an error for invalid policy, mismatched manifests, missing entry
/// files, package budget violations, unsafe inventory, or filesystem failures.
pub fn install_program_atomically(
    source_root: &Path,
    program_store: &Path,
    expected_manifest: ProgramManifest,
    files: &[InstallFile],
) -> Result<ProgramInstallResult, ProgramPackageError> {
    if !source_root.is_dir() {
        return Err(ProgramPackageError::SourceNotDirectory);
    }
    evaluate(expected_manifest.clone())?;
    validate_inventory_contract(files)?;
    let canonical_source_root = source_root.canonicalize()?;
    let manifest_path = source_root.join(MANIFEST_FILE).canonicalize()?;
    if !manifest_path.starts_with(canonical_source_root) {
        return Err(ProgramPackageError::ManifestEscapedSource);
    }
    let packaged_manifest = serde_json::from_slice::<ProgramManifest>(&fs::read(manifest_path)?)?;
    if packaged_manifest != expected_manifest {
        return Err(ProgramPackageError::ManifestMismatch);
    }
    let active_path = program_store.join(&expected_manifest.id).join("active");
    let InstallResult {
        active_path,
        backup_path,
    } = install_atomically(source_root, &active_path, files)?;
    Ok(ProgramInstallResult {
        program_id: expected_manifest.id,
        version: expected_manifest.version,
        active_path,
        backup_path,
    })
}

/// Restores the latest previously active version of a user program.
///
/// # Errors
///
/// Returns an error when the program identifier is unsafe, no backup exists,
/// or the filesystem rollback fails.
pub fn rollback_program(
    program_store: &Path,
    program_id: &str,
) -> Result<RollbackResult, ProgramPackageError> {
    let manifest = ProgramManifest {
        id: program_id.to_owned(),
        version: "0.0.0".to_owned(),
        capabilities: vec![],
        subscriptions: vec![],
        commands: vec![],
        timeout_ms: 1,
        memory_bytes: 1,
    };
    evaluate(manifest)?;
    Ok(rollback_latest(
        &program_store.join(program_id).join("active"),
    )?)
}

fn validate_inventory_contract(files: &[InstallFile]) -> Result<(), ProgramPackageError> {
    let mut paths = HashSet::with_capacity(files.len());
    for file in files {
        if !paths.insert(file.relative_path.clone()) {
            return Err(ProgramPackageError::DuplicatePath(
                file.relative_path.clone(),
            ));
        }
    }
    let total_bytes = files.iter().try_fold(0_u64, |total, file| {
        total
            .checked_add(file.bytes)
            .ok_or(ProgramPackageError::BudgetExceeded)
    })?;
    if files.len() > MAX_PACKAGE_FILES || total_bytes > MAX_PACKAGE_BYTES {
        return Err(ProgramPackageError::BudgetExceeded);
    }
    let has_manifest = files
        .iter()
        .any(|file| file.relative_path == Path::new(MANIFEST_FILE));
    let has_entry = files
        .iter()
        .any(|file| file.relative_path == Path::new(ENTRY_FILE));
    if !has_manifest || !has_entry {
        return Err(ProgramPackageError::MissingRequiredFile);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn manifest(version: &str) -> ProgramManifest {
        ProgramManifest {
            id: "studio.example.focus".to_owned(),
            version: version.to_owned(),
            capabilities: vec![],
            subscriptions: vec![],
            commands: vec![],
            timeout_ms: 1_000,
            memory_bytes: 1024 * 1024,
        }
    }

    fn write_package(root: &Path, manifest: &ProgramManifest, source: &str) -> Vec<InstallFile> {
        fs::create_dir_all(root).unwrap();
        let manifest_bytes = serde_json::to_vec(manifest).unwrap();
        fs::write(root.join(MANIFEST_FILE), &manifest_bytes).unwrap();
        fs::write(root.join(ENTRY_FILE), source).unwrap();
        vec![
            inventory(MANIFEST_FILE, &manifest_bytes),
            inventory(ENTRY_FILE, source.as_bytes()),
        ]
    }

    fn inventory(path: &str, bytes: &[u8]) -> InstallFile {
        InstallFile {
            relative_path: path.into(),
            bytes: u64::try_from(bytes.len()).unwrap(),
            sha256: format!("{:x}", Sha256::digest(bytes)),
        }
    }

    #[test]
    fn installs_and_rolls_back_versioned_programs() {
        let root =
            std::env::temp_dir().join(format!("nimora-program-package-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let source = root.join("source");
        let store = root.join("store");
        let v1 = manifest("1.0.0");
        let files = write_package(&source, &v1, "({ commands: [] })");
        install_program_atomically(&source, &store, v1, &files).unwrap();
        fs::remove_dir_all(&source).unwrap();
        let v2 = manifest("2.0.0");
        let files = write_package(
            &source,
            &v2,
            "({ commands: [{ command: 'safe.pet.animate' }] })",
        );
        let result = install_program_atomically(&source, &store, v2, &files).unwrap();
        assert!(result.backup_path.is_some());
        rollback_program(&store, "studio.example.focus").unwrap();
        let restored = fs::read_to_string(result.active_path.join(ENTRY_FILE)).unwrap();
        assert_eq!(restored, "({ commands: [] })");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_a_manifest_that_differs_from_the_request() {
        let root =
            std::env::temp_dir().join(format!("nimora-program-mismatch-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let packaged = manifest("1.0.0");
        let files = write_package(&root, &packaged, "null");
        assert!(matches!(
            install_program_atomically(&root, &root.join("store"), manifest("2.0.0"), &files),
            Err(ProgramPackageError::ManifestMismatch)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_duplicate_inventory_paths() {
        let files = vec![
            inventory(MANIFEST_FILE, b"{}"),
            inventory(MANIFEST_FILE, b"{}"),
            inventory(ENTRY_FILE, b"null"),
        ];
        assert!(matches!(
            validate_inventory_contract(&files),
            Err(ProgramPackageError::DuplicatePath(path)) if path == Path::new(MANIFEST_FILE)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_manifest_symlink_escape_before_parsing() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "nimora-program-manifest-escape-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let source = root.join("source");
        fs::create_dir_all(&source).unwrap();
        let outside = root.join("outside.json");
        fs::write(&outside, serde_json::to_vec(&manifest("1.0.0")).unwrap()).unwrap();
        symlink(&outside, source.join(MANIFEST_FILE)).unwrap();
        fs::write(source.join(ENTRY_FILE), "null").unwrap();
        let files = vec![
            inventory(MANIFEST_FILE, b"ignored"),
            inventory(ENTRY_FILE, b"null"),
        ];
        assert!(matches!(
            install_program_atomically(&source, &root.join("store"), manifest("1.0.0"), &files),
            Err(ProgramPackageError::ManifestEscapedSource)
        ));
        fs::remove_dir_all(root).unwrap();
    }
}
