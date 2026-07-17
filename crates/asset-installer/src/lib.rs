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
