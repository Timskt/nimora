use crate::{SqlitePersistenceError, prepare_connection};
use nimora_agent_runtime::WorkspaceSnapshot;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredWorkspaceSnapshot {
    pub spec: String,
    pub session_id: Uuid,
    pub root_fingerprint: String,
    pub snapshot: WorkspaceSnapshot,
}

impl StoredWorkspaceSnapshot {
    /// Binds one domain snapshot to an Auto Mode session and opaque host root identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the snapshot or root fingerprint is invalid.
    pub fn new(
        session_id: Uuid,
        root_fingerprint: impl Into<String>,
        snapshot: WorkspaceSnapshot,
    ) -> Result<Self, SqlitePersistenceError> {
        let record = Self {
            spec: "nimora.stored-workspace-snapshot/1".to_owned(),
            session_id,
            root_fingerprint: root_fingerprint.into(),
            snapshot,
        };
        validate(&record)?;
        Ok(record)
    }
}

#[derive(Debug)]
pub struct SqliteWorkspaceSnapshotRepository {
    connection: Mutex<Connection>,
}

impl SqliteWorkspaceSnapshotRepository {
    /// Opens the shared application database.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot be opened or initialized.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Creates an isolated repository for tests.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot initialize the schema.
    pub fn in_memory() -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut connection: Connection) -> Result<Self, SqlitePersistenceError> {
        prepare_connection(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Inserts the first snapshot for a session exactly once.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid records or an existing version chain.
    pub fn create(&self, record: &StoredWorkspaceSnapshot) -> Result<(), SqlitePersistenceError> {
        validate(record)?;
        if record.snapshot.revision != 1 || record.snapshot.parent_fingerprint.is_some() {
            return Err(SqlitePersistenceError::InvalidWorkspaceSnapshot);
        }
        let payload = serde_json::to_string(record)?;
        self.lock()?
            .execute(
                "INSERT INTO agent_workspace_snapshot (
                    session_id, revision, root_fingerprint, snapshot_fingerprint, created_at_ms,
                    schema_version, payload
                 ) VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)",
                params![
                    record.session_id.to_string(),
                    to_i64(record.snapshot.revision)?,
                    record.root_fingerprint,
                    record.snapshot.fingerprint,
                    to_i64(record.snapshot.created_at_ms)?,
                    payload,
                ],
            )
            .map_err(|error| match error {
                rusqlite::Error::SqliteFailure(_, _) => {
                    SqlitePersistenceError::WorkspaceSnapshotConflict
                }
                other => SqlitePersistenceError::Sqlite(other),
            })?;
        Ok(())
    }

    /// Appends exactly one successor using a monotonic compare-and-swap chain.
    ///
    /// # Errors
    ///
    /// Returns an error when the previous revision/fingerprint is stale or metadata is invalid.
    pub fn append(
        &self,
        record: &StoredWorkspaceSnapshot,
        previous_revision: u64,
        previous_fingerprint: &str,
    ) -> Result<(), SqlitePersistenceError> {
        validate(record)?;
        if record.snapshot.revision != previous_revision.saturating_add(1)
            || record.snapshot.parent_fingerprint.as_deref() != Some(previous_fingerprint)
        {
            return Err(SqlitePersistenceError::InvalidWorkspaceSnapshot);
        }
        let payload = serde_json::to_string(record)?;
        let mut connection = self.lock()?;
        let transaction = connection.transaction()?;
        let latest = latest_metadata(&transaction, record.session_id)?
            .ok_or(SqlitePersistenceError::WorkspaceSnapshotConflict)?;
        if latest.0 != previous_revision
            || latest.1 != previous_fingerprint
            || latest.2 != record.root_fingerprint
        {
            return Err(SqlitePersistenceError::WorkspaceSnapshotConflict);
        }
        transaction.execute(
            "INSERT INTO agent_workspace_snapshot (
                session_id, revision, root_fingerprint, snapshot_fingerprint, created_at_ms,
                schema_version, payload
             ) VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)",
            params![
                record.session_id.to_string(),
                to_i64(record.snapshot.revision)?,
                record.root_fingerprint,
                record.snapshot.fingerprint,
                to_i64(record.snapshot.created_at_ms)?,
                payload,
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Loads the latest snapshot and revalidates all indexed metadata.
    ///
    /// # Errors
    ///
    /// Returns an error for corrupt or unsupported persisted records.
    pub fn latest(
        &self,
        session_id: Uuid,
    ) -> Result<Option<StoredWorkspaceSnapshot>, SqlitePersistenceError> {
        let stored = self
            .lock()?
            .query_row(
                "SELECT schema_version, payload, revision, root_fingerprint,
                    snapshot_fingerprint, created_at_ms
                 FROM agent_workspace_snapshot WHERE session_id = ?1
                 ORDER BY revision DESC LIMIT 1",
                [session_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, u32>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                },
            )
            .optional()?;
        let Some((version, payload, revision, root, fingerprint, created_at)) = stored else {
            return Ok(None);
        };
        if version != 1 {
            return Err(SqlitePersistenceError::UnsupportedWorkspaceSnapshotVersion(
                version,
            ));
        }
        let record = serde_json::from_str::<StoredWorkspaceSnapshot>(&payload)?;
        validate(&record)?;
        if record.session_id != session_id
            || to_i64(record.snapshot.revision)? != revision
            || record.root_fingerprint != root
            || record.snapshot.fingerprint != fingerprint
            || to_i64(record.snapshot.created_at_ms)? != created_at
        {
            return Err(SqlitePersistenceError::InvalidWorkspaceSnapshot);
        }
        Ok(Some(record))
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, SqlitePersistenceError> {
        self.connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)
    }
}

fn latest_metadata(
    connection: &Connection,
    session_id: Uuid,
) -> Result<Option<(u64, String, String)>, SqlitePersistenceError> {
    let stored = connection
        .query_row(
            "SELECT revision, snapshot_fingerprint, root_fingerprint
             FROM agent_workspace_snapshot WHERE session_id = ?1
             ORDER BY revision DESC LIMIT 1",
            [session_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?;
    stored
        .map(|(revision, fingerprint, root)| {
            Ok((
                u64::try_from(revision)
                    .map_err(|_| SqlitePersistenceError::InvalidWorkspaceSnapshot)?,
                fingerprint,
                root,
            ))
        })
        .transpose()
}

fn validate(record: &StoredWorkspaceSnapshot) -> Result<(), SqlitePersistenceError> {
    record
        .snapshot
        .validate()
        .map_err(|_| SqlitePersistenceError::InvalidWorkspaceSnapshot)?;
    if record.spec != "nimora.stored-workspace-snapshot/1"
        || record.root_fingerprint.len() != 64
        || !record
            .root_fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(SqlitePersistenceError::InvalidWorkspaceSnapshot);
    }
    Ok(())
}

fn to_i64(value: u64) -> Result<i64, SqlitePersistenceError> {
    i64::try_from(value).map_err(|_| SqlitePersistenceError::InvalidWorkspaceSnapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimora_agent_runtime::TrackedWorkspaceFile;

    fn snapshot(revision: u64, parent: Option<String>, contents: &[u8]) -> WorkspaceSnapshot {
        WorkspaceSnapshot::new(
            revision,
            parent,
            vec![TrackedWorkspaceFile::from_bytes("src/lib.rs", contents, false).expect("file")],
            revision,
        )
        .expect("snapshot")
    }

    #[test]
    fn persists_monotonic_snapshot_chain_and_rejects_stale_append() {
        let repository = SqliteWorkspaceSnapshotRepository::in_memory().expect("repository");
        let session_id = Uuid::now_v7();
        let root = "a".repeat(64);
        let first = StoredWorkspaceSnapshot::new(session_id, &root, snapshot(1, None, b"one"))
            .expect("first");
        repository.create(&first).expect("create");
        let second = StoredWorkspaceSnapshot::new(
            session_id,
            &root,
            snapshot(2, Some(first.snapshot.fingerprint.clone()), b"two"),
        )
        .expect("second");
        repository
            .append(&second, 1, &first.snapshot.fingerprint)
            .expect("append");
        assert_eq!(
            repository.latest(session_id).expect("latest"),
            Some(second.clone())
        );
        assert!(matches!(
            repository.append(&second, 1, &first.snapshot.fingerprint),
            Err(SqlitePersistenceError::WorkspaceSnapshotConflict
                | SqlitePersistenceError::Sqlite(_))
        ));
    }
}
