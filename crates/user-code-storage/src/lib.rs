use serde_json::Value;
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};
use thiserror::Error;
use uuid::Uuid;

const MAX_KEY_BYTES: usize = 128;
const MAX_VALUE_BYTES: usize = 256 * 1024;
pub const DEFAULT_PROGRAM_QUOTA_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ProgramDataStore {
    root: PathBuf,
    quota_bytes: u64,
}

#[derive(Debug, Error)]
pub enum ProgramDataError {
    #[error("program ID is invalid")]
    InvalidProgramId,
    #[error("storage key is invalid")]
    InvalidKey,
    #[error("stored value exceeds the 256 KiB item limit")]
    ItemTooLarge,
    #[error("program storage exceeds its quota")]
    QuotaExceeded,
    #[error("program storage contains an unsafe filesystem entry")]
    UnsafeEntry,
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl ProgramDataStore {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self::with_quota(root, DEFAULT_PROGRAM_QUOTA_BYTES)
    }

    #[must_use]
    pub fn with_quota(root: impl Into<PathBuf>, quota_bytes: u64) -> Self {
        Self {
            root: root.into(),
            quota_bytes,
        }
    }

    /// Reads one JSON value from a program-isolated namespace.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identifiers, unsafe entries, I/O failures,
    /// or malformed stored JSON.
    pub fn read(&self, program_id: &str, key: &str) -> Result<Option<Value>, ProgramDataError> {
        let path = self.value_path(program_id, key)?;
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_file() => {
                let bytes = fs::read(path)?;
                Ok(Some(serde_json::from_slice(&bytes)?))
            }
            Ok(_) => Err(ProgramDataError::UnsafeEntry),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Atomically writes one JSON value after enforcing item and program quotas.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identifiers, unsafe entries, quota
    /// exhaustion, serialization failures, or I/O failures.
    pub fn write(
        &self,
        program_id: &str,
        key: &str,
        value: &Value,
    ) -> Result<(), ProgramDataError> {
        let directory = self.program_directory(program_id)?;
        fs::create_dir_all(&directory)?;
        ensure_directory(&directory)?;
        let path = self.value_path(program_id, key)?;
        let bytes = serde_json::to_vec(value)?;
        if bytes.len() > MAX_VALUE_BYTES {
            return Err(ProgramDataError::ItemTooLarge);
        }
        let existing = file_size_if_safe(&path)?;
        let used = directory_size(&directory)?;
        let next = used
            .saturating_sub(existing)
            .saturating_add(bytes.len() as u64);
        if next > self.quota_bytes {
            return Err(ProgramDataError::QuotaExceeded);
        }
        let temporary = directory.join(format!(".tmp-{}", Uuid::now_v7()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
            let _ = fs::remove_file(&temporary);
            return Err(error.into());
        }
        if let Err(error) = fs::rename(&temporary, &path) {
            let _ = fs::remove_file(&temporary);
            return Err(error.into());
        }
        Ok(())
    }

    /// Deletes one value without following links.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identifiers, unsafe entries, or I/O failures.
    pub fn delete(&self, program_id: &str, key: &str) -> Result<bool, ProgramDataError> {
        let path = self.value_path(program_id, key)?;
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_file() => {
                fs::remove_file(path)?;
                Ok(true)
            }
            Ok(_) => Err(ProgramDataError::UnsafeEntry),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    fn program_directory(&self, program_id: &str) -> Result<PathBuf, ProgramDataError> {
        if !valid_program_id(program_id) {
            return Err(ProgramDataError::InvalidProgramId);
        }
        Ok(self.root.join(program_id))
    }

    fn value_path(&self, program_id: &str, key: &str) -> Result<PathBuf, ProgramDataError> {
        if !valid_key(key) {
            return Err(ProgramDataError::InvalidKey);
        }
        Ok(self
            .program_directory(program_id)?
            .join(format!("{key}.json")))
    }
}

fn valid_program_id(value: &str) -> bool {
    let segments = value.split('.').collect::<Vec<_>>();
    segments.len() >= 3
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

fn valid_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_KEY_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn ensure_directory(path: &Path) -> Result<(), ProgramDataError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_dir() {
        Ok(())
    } else {
        Err(ProgramDataError::UnsafeEntry)
    }
}

fn file_size_if_safe(path: &Path) -> Result<u64, ProgramDataError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(metadata.len()),
        Ok(_) => Err(ProgramDataError::UnsafeEntry),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(error.into()),
    }
}

fn directory_size(path: &Path) -> Result<u64, ProgramDataError> {
    let mut total = 0_u64;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if !metadata.file_type().is_file() {
            return Err(ProgramDataError::UnsafeEntry);
        }
        total = total.saturating_add(metadata.len());
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_store(quota: u64) -> (PathBuf, ProgramDataStore) {
        let root = std::env::temp_dir().join(format!("nimora-store-{}", Uuid::now_v7()));
        (root.clone(), ProgramDataStore::with_quota(root, quota))
    }

    #[test]
    fn isolates_programs_and_round_trips_json() {
        let (root, store) = temporary_store(1024);
        store
            .write(
                "studio.one.program",
                "state",
                &serde_json::json!({"value": 1}),
            )
            .unwrap();
        store
            .write(
                "studio.two.program",
                "state",
                &serde_json::json!({"value": 2}),
            )
            .unwrap();
        assert_eq!(
            store.read("studio.one.program", "state").unwrap().unwrap()["value"],
            1
        );
        assert_eq!(
            store.read("studio.two.program", "state").unwrap().unwrap()["value"],
            2
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_path_escape_and_quota_overflow() {
        let (root, store) = temporary_store(16);
        assert!(matches!(
            store.write("studio.one.program", "../escape", &Value::Null),
            Err(ProgramDataError::InvalidKey)
        ));
        assert!(matches!(
            store.write(
                "studio.one.program",
                "large",
                &serde_json::json!("a".repeat(32))
            ),
            Err(ProgramDataError::QuotaExceeded)
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symbolic_link_entries() {
        use std::os::unix::fs::symlink;
        let (root, store) = temporary_store(1024);
        let directory = root.join("studio.one.program");
        fs::create_dir_all(&directory).unwrap();
        symlink(root.join("outside"), directory.join("state.json")).unwrap();
        assert!(matches!(
            store.read("studio.one.program", "state"),
            Err(ProgramDataError::UnsafeEntry)
        ));
        fs::remove_dir_all(root).unwrap();
    }
}
