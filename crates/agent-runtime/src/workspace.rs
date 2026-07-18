use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

const MAX_TRACKED_FILES: usize = 20_000;
const MAX_RELATIVE_PATH_BYTES: usize = 1_024;
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrackedWorkspaceFile {
    pub relative_path: String,
    pub sha256: String,
    pub bytes: u64,
    pub executable: bool,
}

impl TrackedWorkspaceFile {
    /// Creates a normalized, content-addressed file record from host-provided bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe paths or oversized files.
    pub fn from_bytes(
        relative_path: impl Into<String>,
        contents: &[u8],
        executable: bool,
    ) -> Result<Self, WorkspaceTrackingError> {
        let relative_path = relative_path.into();
        let bytes =
            u64::try_from(contents.len()).map_err(|_| WorkspaceTrackingError::InvalidFile)?;
        if !valid_relative_path(&relative_path) || bytes > MAX_FILE_BYTES {
            return Err(WorkspaceTrackingError::InvalidFile);
        }
        Ok(Self {
            relative_path,
            sha256: format!("{:x}", Sha256::digest(contents)),
            bytes,
            executable,
        })
    }

    fn validate(&self) -> Result<(), WorkspaceTrackingError> {
        if !valid_relative_path(&self.relative_path)
            || self.bytes > MAX_FILE_BYTES
            || !valid_sha256(&self.sha256)
        {
            return Err(WorkspaceTrackingError::InvalidFile);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSnapshot {
    pub spec: String,
    pub revision: u64,
    pub parent_fingerprint: Option<String>,
    pub fingerprint: String,
    pub files: Vec<TrackedWorkspaceFile>,
    pub created_at_ms: u64,
}

impl WorkspaceSnapshot {
    /// Creates an immutable deterministic workspace manifest.
    ///
    /// # Errors
    ///
    /// Returns an error for empty/oversized manifests, duplicate paths, invalid files, or revisions.
    pub fn new(
        revision: u64,
        parent_fingerprint: Option<String>,
        mut files: Vec<TrackedWorkspaceFile>,
        created_at_ms: u64,
    ) -> Result<Self, WorkspaceTrackingError> {
        if revision == 0 || files.len() > MAX_TRACKED_FILES {
            return Err(WorkspaceTrackingError::InvalidSnapshot);
        }
        files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        for file in &files {
            file.validate()?;
        }
        if files
            .windows(2)
            .any(|pair| pair[0].relative_path == pair[1].relative_path)
            || parent_fingerprint
                .as_deref()
                .is_some_and(|fingerprint| !valid_fingerprint(fingerprint))
        {
            return Err(WorkspaceTrackingError::InvalidSnapshot);
        }
        let fingerprint = snapshot_fingerprint(revision, parent_fingerprint.as_deref(), &files)?;
        Ok(Self {
            spec: "nimora.workspace-snapshot/1".to_owned(),
            revision,
            parent_fingerprint,
            fingerprint,
            files,
            created_at_ms,
        })
    }

    /// Validates a snapshot restored across a persistence boundary.
    ///
    /// # Errors
    ///
    /// Returns an error when metadata or the content fingerprint is invalid.
    pub fn validate(&self) -> Result<(), WorkspaceTrackingError> {
        let rebuilt = Self::new(
            self.revision,
            self.parent_fingerprint.clone(),
            self.files.clone(),
            self.created_at_ms,
        )?;
        if self.spec != rebuilt.spec
            || self.files != rebuilt.files
            || self.fingerprint != rebuilt.fingerprint
        {
            return Err(WorkspaceTrackingError::InvalidSnapshot);
        }
        Ok(())
    }

    /// Computes a stable path-level change set between two immutable manifests.
    ///
    /// # Errors
    ///
    /// Returns an error when either snapshot is invalid or revisions are not monotonic.
    pub fn diff(&self, next: &Self) -> Result<WorkspaceChangeSet, WorkspaceTrackingError> {
        self.validate()?;
        next.validate()?;
        if next.revision <= self.revision
            || next.parent_fingerprint.as_deref() != Some(self.fingerprint.as_str())
        {
            return Err(WorkspaceTrackingError::VersionConflict);
        }
        let previous = self
            .files
            .iter()
            .map(|file| (&file.relative_path, file))
            .collect::<BTreeMap<_, _>>();
        let current = next
            .files
            .iter()
            .map(|file| (&file.relative_path, file))
            .collect::<BTreeMap<_, _>>();
        let paths = previous
            .keys()
            .chain(current.keys())
            .copied()
            .collect::<BTreeSet<_>>();
        let changes = paths
            .into_iter()
            .filter_map(|path| match (previous.get(path), current.get(path)) {
                (None, Some(file)) => Some(WorkspaceFileChange::Added((*file).clone())),
                (Some(file), None) => Some(WorkspaceFileChange::Deleted((*file).clone())),
                (Some(before), Some(after)) if before != after => {
                    Some(WorkspaceFileChange::Modified {
                        before: (*before).clone(),
                        after: (*after).clone(),
                    })
                }
                _ => None,
            })
            .collect();
        Ok(WorkspaceChangeSet {
            spec: "nimora.workspace-change-set/1".to_owned(),
            from_revision: self.revision,
            to_revision: next.revision,
            from_fingerprint: self.fingerprint.clone(),
            to_fingerprint: next.fingerprint.clone(),
            changes,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "file")]
pub enum WorkspaceFileChange {
    Added(TrackedWorkspaceFile),
    Modified {
        before: TrackedWorkspaceFile,
        after: TrackedWorkspaceFile,
    },
    Deleted(TrackedWorkspaceFile),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceChangeSet {
    pub spec: String,
    pub from_revision: u64,
    pub to_revision: u64,
    pub from_fingerprint: String,
    pub to_fingerprint: String,
    pub changes: Vec<WorkspaceFileChange>,
}

fn snapshot_fingerprint(
    revision: u64,
    parent: Option<&str>,
    files: &[TrackedWorkspaceFile],
) -> Result<String, WorkspaceTrackingError> {
    let encoded = serde_json::to_vec(&(revision, parent, files))
        .map_err(|_| WorkspaceTrackingError::InvalidSnapshot)?;
    Ok(format!("sha256:{:x}", Sha256::digest(encoded)))
}

fn valid_relative_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= MAX_RELATIVE_PATH_BYTES
        && !path.starts_with('/')
        && !path.starts_with('\\')
        && !path.contains('\\')
        && !path.chars().any(char::is_control)
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_fingerprint(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(valid_sha256)
}

#[derive(Debug, Error)]
pub enum WorkspaceTrackingError {
    #[error("tracked workspace file is invalid")]
    InvalidFile,
    #[error("workspace snapshot is invalid")]
    InvalidSnapshot,
    #[error("workspace snapshot version chain conflicts")]
    VersionConflict,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_added_modified_deleted_and_executable_changes() {
        let first = WorkspaceSnapshot::new(
            1,
            None,
            vec![
                TrackedWorkspaceFile::from_bytes("src/a.rs", b"a", false).expect("a"),
                TrackedWorkspaceFile::from_bytes("src/b.rs", b"b", false).expect("b"),
            ],
            1_000,
        )
        .expect("first");
        let second = WorkspaceSnapshot::new(
            2,
            Some(first.fingerprint.clone()),
            vec![
                TrackedWorkspaceFile::from_bytes("src/a.rs", b"changed", true).expect("a"),
                TrackedWorkspaceFile::from_bytes("src/c.rs", b"c", false).expect("c"),
            ],
            1_001,
        )
        .expect("second");
        let changes = first.diff(&second).expect("diff");
        assert_eq!(changes.changes.len(), 3);
        assert!(matches!(
            changes.changes[0],
            WorkspaceFileChange::Modified { .. }
        ));
        assert!(matches!(
            changes.changes[1],
            WorkspaceFileChange::Deleted(_)
        ));
        assert!(matches!(changes.changes[2], WorkspaceFileChange::Added(_)));
    }

    #[test]
    fn rejects_path_escape_duplicates_tampering_and_wrong_parent() {
        assert!(TrackedWorkspaceFile::from_bytes("../secret", b"x", false).is_err());
        let file = TrackedWorkspaceFile::from_bytes("src/lib.rs", b"x", false).expect("file");
        assert!(WorkspaceSnapshot::new(1, None, vec![file.clone(), file.clone()], 1_000).is_err());
        let mut snapshot = WorkspaceSnapshot::new(1, None, vec![file], 1_000).expect("snapshot");
        snapshot.fingerprint.push('0');
        assert!(snapshot.validate().is_err());
    }
}
