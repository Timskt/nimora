use std::{
    fs, io,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallFile {
    pub relative_path: PathBuf,
}

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
    #[error("filesystem operation failed: {0}")]
    Io(#[from] io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallResult {
    pub active_path: PathBuf,
    pub backup_path: Option<PathBuf>,
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
    if !source_root.is_dir() {
        return Err(InstallError::SourceNotDirectory);
    }
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
            let destination = staging.join(relative);
            if let Some(destination_parent) = destination.parent() {
                fs::create_dir_all(destination_parent)?;
            }
            fs::copy(source, destination)?;
        }
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
            }],
        )
        .unwrap_err();
        assert!(matches!(error, InstallError::EscapedSource(_)));
        fs::remove_dir_all(root).unwrap();
    }
}
