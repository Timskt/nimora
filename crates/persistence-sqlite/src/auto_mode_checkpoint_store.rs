use crate::{SqlitePersistenceError, prepare_connection};
use nimora_agent_runtime::AutoModeCheckpoint;
use rusqlite::{Connection, OptionalExtension, params};
use std::{path::Path, sync::Mutex};
use uuid::Uuid;

#[derive(Debug)]
pub struct SqliteAutoModeCheckpointRepository {
    connection: Mutex<Connection>,
}

impl SqliteAutoModeCheckpointRepository {
    /// Opens or creates the persistent Auto Mode checkpoint store.
    ///
    /// # Errors
    ///
    /// Returns an error when the database cannot be opened or validated.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        let mut connection = Connection::open(path)?;
        prepare_connection(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Creates an isolated in-memory checkpoint store.
    ///
    /// # Errors
    ///
    /// Returns an error when the schema cannot be initialized.
    pub fn in_memory() -> Result<Self, SqlitePersistenceError> {
        let mut connection = Connection::open_in_memory()?;
        prepare_connection(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Inserts the first checkpoint for a session.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid checkpoints, non-initial sequences, or duplicate sessions.
    pub fn create(&self, checkpoint: &AutoModeCheckpoint) -> Result<(), SqlitePersistenceError> {
        validate(checkpoint)?;
        if checkpoint.sequence != 1 {
            return Err(SqlitePersistenceError::InvalidAutoModeCheckpoint);
        }
        let payload = serde_json::to_string(checkpoint)?;
        self.lock()?.execute(
            "INSERT INTO auto_mode_checkpoint (
                session_id, goal_id, plan_revision, sequence, task_id, updated_at_ms,
                schema_version, payload
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7)",
            params![
                checkpoint.session_id.to_string(),
                checkpoint.goal_id.to_string(),
                to_i64(checkpoint.plan_revision)?,
                to_i64(checkpoint.sequence)?,
                checkpoint.task.id.to_string(),
                to_i64(checkpoint.updated_at_ms)?,
                payload,
            ],
        )?;
        Ok(())
    }

    /// Replaces a checkpoint only when the previous sequence still matches.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid checkpoints or concurrent/stale writers.
    pub fn replace(
        &self,
        checkpoint: &AutoModeCheckpoint,
        previous_sequence: u64,
    ) -> Result<(), SqlitePersistenceError> {
        validate(checkpoint)?;
        if checkpoint.sequence != previous_sequence.saturating_add(1) {
            return Err(SqlitePersistenceError::InvalidAutoModeCheckpoint);
        }
        let payload = serde_json::to_string(checkpoint)?;
        let changed = self.lock()?.execute(
            "UPDATE auto_mode_checkpoint SET sequence = ?1, task_id = ?2, updated_at_ms = ?3,
                payload = ?4
             WHERE session_id = ?5 AND goal_id = ?6 AND plan_revision = ?7 AND sequence = ?8",
            params![
                to_i64(checkpoint.sequence)?,
                checkpoint.task.id.to_string(),
                to_i64(checkpoint.updated_at_ms)?,
                payload,
                checkpoint.session_id.to_string(),
                checkpoint.goal_id.to_string(),
                to_i64(checkpoint.plan_revision)?,
                to_i64(previous_sequence)?,
            ],
        )?;
        if changed != 1 {
            return Err(SqlitePersistenceError::AutoModeCheckpointConflict);
        }
        Ok(())
    }

    /// Loads the latest checkpoint while verifying indexed metadata against its payload.
    ///
    /// # Errors
    ///
    /// Returns an error when persisted data is corrupt or unsupported.
    pub fn get(
        &self,
        session_id: Uuid,
    ) -> Result<Option<AutoModeCheckpoint>, SqlitePersistenceError> {
        let stored = self
            .lock()?
            .query_row(
                "SELECT schema_version, payload, goal_id, plan_revision, sequence, task_id,
                    updated_at_ms FROM auto_mode_checkpoint WHERE session_id = ?1",
                [session_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, u32>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, i64>(6)?,
                    ))
                },
            )
            .optional()?;
        let Some((version, payload, goal_id, revision, sequence, task_id, updated_at_ms)) = stored
        else {
            return Ok(None);
        };
        if version != 1 {
            return Err(SqlitePersistenceError::UnsupportedAutoModeCheckpointVersion(version));
        }
        let checkpoint = serde_json::from_str::<AutoModeCheckpoint>(&payload)?;
        validate(&checkpoint)?;
        if checkpoint.session_id != session_id
            || checkpoint.goal_id.to_string() != goal_id
            || to_i64(checkpoint.plan_revision)? != revision
            || to_i64(checkpoint.sequence)? != sequence
            || checkpoint.task.id.to_string() != task_id
            || to_i64(checkpoint.updated_at_ms)? != updated_at_ms
        {
            return Err(SqlitePersistenceError::InvalidAutoModeCheckpoint);
        }
        Ok(Some(checkpoint))
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, SqlitePersistenceError> {
        self.connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)
    }
}

fn validate(checkpoint: &AutoModeCheckpoint) -> Result<(), SqlitePersistenceError> {
    checkpoint
        .validate()
        .map_err(|_| SqlitePersistenceError::InvalidAutoModeCheckpoint)
}

fn to_i64(value: u64) -> Result<i64, SqlitePersistenceError> {
    i64::try_from(value).map_err(|_| SqlitePersistenceError::InvalidAutoModeCheckpoint)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimora_agent_runtime::{
        AgentBudget, AgentTask, AgentTaskOrigin, AgentTaskStatus, DataClassification,
        ProviderMessage, ProviderMessageRole,
    };

    fn checkpoint(session_id: Uuid, sequence: u64, updated_at_ms: u64) -> AutoModeCheckpoint {
        let mut task = AgentTask::new(
            AgentTaskOrigin::Desktop,
            "desktop:auto-mode",
            "provider:local",
            AgentBudget::default(),
            1_000,
        )
        .expect("task");
        task.transition(AgentTaskStatus::Planning, 1_000)
            .expect("planning");
        task.transition(AgentTaskStatus::Running, 1_001)
            .expect("running");
        AutoModeCheckpoint::new(
            session_id,
            Uuid::from_u128(42),
            1,
            sequence,
            task,
            "model:local",
            vec![ProviderMessage::text(
                ProviderMessageRole::User,
                "continue",
                DataClassification::Personal,
                true,
            )],
            "git:abc",
            "sha256:policy",
            1_000,
            updated_at_ms,
        )
        .expect("checkpoint")
    }

    #[test]
    fn round_trips_and_rejects_stale_replacement() {
        let repository = SqliteAutoModeCheckpointRepository::in_memory().expect("store");
        let session_id = Uuid::now_v7();
        let first = checkpoint(session_id, 1, 1_001);
        repository.create(&first).expect("create");
        let second = checkpoint(session_id, 2, 1_002);
        repository.replace(&second, 1).expect("replace");
        assert_eq!(
            repository.get(session_id).expect("get"),
            Some(second.clone())
        );
        assert!(matches!(
            repository.replace(&second, 1),
            Err(SqlitePersistenceError::AutoModeCheckpointConflict)
        ));
    }

    #[test]
    fn rejects_non_initial_create_and_skipped_sequence() {
        let repository = SqliteAutoModeCheckpointRepository::in_memory().expect("store");
        let session_id = Uuid::now_v7();
        assert!(matches!(
            repository.create(&checkpoint(session_id, 2, 1_002)),
            Err(SqlitePersistenceError::InvalidAutoModeCheckpoint)
        ));
        repository
            .create(&checkpoint(session_id, 1, 1_001))
            .expect("create");
        assert!(matches!(
            repository.replace(&checkpoint(session_id, 3, 1_003), 1),
            Err(SqlitePersistenceError::InvalidAutoModeCheckpoint)
        ));
    }
}
