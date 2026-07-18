use crate::{SqlitePersistenceError, prepare_connection};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Mutex};
use uuid::Uuid;

const VERSION: u32 = 1;
const MAX_PAGE: usize = 200;
const MAX_SKILL_ID_BYTES: usize = 256;
const MAX_ERROR_BYTES: usize = 4 * 1024;

#[derive(Debug)]
pub struct SqliteSkillExecutionHistory {
    connection: Mutex<Connection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillExecutionHistoryStatus {
    WaitingForApproval,
    Completed,
    Rejected,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillExecutionHistoryRecord {
    pub spec: String,
    pub execution_id: Uuid,
    pub skill_id: String,
    pub status: SkillExecutionHistoryStatus,
    pub command_count: u32,
    pub agent_task_count: u32,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub error: Option<String>,
}

impl SkillExecutionHistoryRecord {
    /// Creates one bounded, metadata-only Skill execution record.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identity, counts, timestamps, status, or error text.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        execution_id: Uuid,
        skill_id: impl Into<String>,
        status: SkillExecutionHistoryStatus,
        command_count: usize,
        agent_task_count: usize,
        created_at_ms: u64,
        updated_at_ms: u64,
        error: Option<String>,
    ) -> Result<Self, SqlitePersistenceError> {
        let record = Self {
            spec: "nimora.skill-execution-history/1".to_owned(),
            execution_id,
            skill_id: skill_id.into(),
            status,
            command_count: u32::try_from(command_count)
                .map_err(|_| SqlitePersistenceError::InvalidSkillExecutionHistory)?,
            agent_task_count: u32::try_from(agent_task_count)
                .map_err(|_| SqlitePersistenceError::InvalidSkillExecutionHistory)?,
            created_at_ms,
            updated_at_ms,
            error,
        };
        validate(&record)?;
        Ok(record)
    }
}

impl SqliteSkillExecutionHistory {
    /// Opens Skill execution history in the shared application database.
    ///
    /// # Errors
    ///
    /// Returns an error when the database cannot be opened or initialized.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Creates isolated Skill execution history for tests or recovery mode.
    ///
    /// # Errors
    ///
    /// Returns an error when the in-memory database cannot be initialized.
    pub fn in_memory() -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut connection: Connection) -> Result<Self, SqlitePersistenceError> {
        prepare_connection(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Inserts an execution or advances its status without changing creation order.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid records or storage failures.
    pub fn save(&self, record: &SkillExecutionHistoryRecord) -> Result<(), SqlitePersistenceError> {
        validate(record)?;
        self.connection.lock().map_err(|_| SqlitePersistenceError::StatePoisoned)?.execute(
            "INSERT INTO skill_execution_history
                (execution_id, skill_id, status, created_at_ms, updated_at_ms, schema_version, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(execution_id) DO UPDATE SET
                status = excluded.status,
                updated_at_ms = excluded.updated_at_ms,
                schema_version = excluded.schema_version,
                payload = excluded.payload
             WHERE skill_execution_history.skill_id = excluded.skill_id
               AND skill_execution_history.created_at_ms = excluded.created_at_ms
               AND skill_execution_history.status = 'waiting-for-approval'",
            params![
                record.execution_id.to_string(), record.skill_id, status_name(record.status),
                to_i64(record.created_at_ms)?, to_i64(record.updated_at_ms)?, VERSION,
                serde_json::to_string(record)?
            ],
        )?;
        Ok(())
    }

    /// Lists newest executions using a stable `(created_at_ms, execution_id)` cursor.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid bounds, unsupported payloads, or storage failures.
    pub fn list(
        &self,
        before: Option<(u64, Uuid)>,
        limit: usize,
    ) -> Result<Vec<SkillExecutionHistoryRecord>, SqlitePersistenceError> {
        if limit == 0 || limit > MAX_PAGE {
            return Err(SqlitePersistenceError::InvalidSkillExecutionHistory);
        }
        let (before_ms, before_id) = before
            .map(|(ms, id)| Ok::<_, SqlitePersistenceError>((to_i64(ms)?, id.to_string())))
            .transpose()?
            .unwrap_or((i64::MAX, "~".to_owned()));
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let mut statement = connection.prepare(
            "SELECT schema_version, payload FROM skill_execution_history
             WHERE created_at_ms < ?1 OR (created_at_ms = ?1 AND execution_id < ?2)
             ORDER BY created_at_ms DESC, execution_id DESC LIMIT ?3",
        )?;
        let rows = statement.query_map(
            params![
                before_ms,
                before_id,
                i64::try_from(limit)
                    .map_err(|_| SqlitePersistenceError::InvalidSkillExecutionHistory)?
            ],
            |row| Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?)),
        )?;
        rows.map(|row| {
            let (version, payload) = row?;
            if version != VERSION {
                return Err(
                    SqlitePersistenceError::UnsupportedSkillExecutionHistoryVersion(version),
                );
            }
            let record = serde_json::from_str(&payload)?;
            validate(&record)?;
            Ok(record)
        })
        .collect()
    }

    /// Deletes one historical record without affecting a running execution.
    ///
    /// # Errors
    ///
    /// Returns an error when storage cannot be updated.
    pub fn delete(&self, execution_id: Uuid) -> Result<bool, SqlitePersistenceError> {
        Ok(self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .execute(
                "DELETE FROM skill_execution_history WHERE execution_id = ?1",
                params![execution_id.to_string()],
            )?
            == 1)
    }

    /// Deletes all historical records without changing Skill runtime state.
    ///
    /// # Errors
    ///
    /// Returns an error when storage cannot be updated.
    pub fn delete_all(&self) -> Result<u64, SqlitePersistenceError> {
        Ok(self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .execute("DELETE FROM skill_execution_history", [])? as u64)
    }
}

fn validate(record: &SkillExecutionHistoryRecord) -> Result<(), SqlitePersistenceError> {
    if record.spec != "nimora.skill-execution-history/1"
        || record.execution_id.is_nil()
        || record.skill_id.trim().is_empty()
        || record.skill_id.len() > MAX_SKILL_ID_BYTES
        || record.skill_id.chars().any(char::is_control)
        || record.updated_at_ms < record.created_at_ms
        || record.error.as_ref().is_some_and(|error| {
            error.is_empty() || error.len() > MAX_ERROR_BYTES || error.chars().any(char::is_control)
        })
        || (record.status != SkillExecutionHistoryStatus::Failed && record.error.is_some())
    {
        return Err(SqlitePersistenceError::InvalidSkillExecutionHistory);
    }
    Ok(())
}

const fn status_name(status: SkillExecutionHistoryStatus) -> &'static str {
    match status {
        SkillExecutionHistoryStatus::WaitingForApproval => "waiting-for-approval",
        SkillExecutionHistoryStatus::Completed => "completed",
        SkillExecutionHistoryStatus::Rejected => "rejected",
        SkillExecutionHistoryStatus::Cancelled => "cancelled",
        SkillExecutionHistoryStatus::Failed => "failed",
    }
}

fn to_i64(value: u64) -> Result<i64, SqlitePersistenceError> {
    i64::try_from(value).map_err(|_| SqlitePersistenceError::InvalidSkillExecutionHistory)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: Uuid, at: u64) -> SkillExecutionHistoryRecord {
        SkillExecutionHistoryRecord::new(
            id,
            "studio.example.focus",
            SkillExecutionHistoryStatus::WaitingForApproval,
            2,
            1,
            at,
            at,
            None,
        )
        .unwrap()
    }

    #[test]
    fn status_converges_once_and_history_is_privacy_deletable() {
        let history = SqliteSkillExecutionHistory::in_memory().unwrap();
        let id = Uuid::now_v7();
        history.save(&record(id, 100)).unwrap();
        let completed = SkillExecutionHistoryRecord::new(
            id,
            "studio.example.focus",
            SkillExecutionHistoryStatus::Completed,
            2,
            1,
            100,
            200,
            None,
        )
        .unwrap();
        history.save(&completed).unwrap();
        history
            .save(&SkillExecutionHistoryRecord {
                status: SkillExecutionHistoryStatus::Rejected,
                updated_at_ms: 300,
                ..completed.clone()
            })
            .unwrap();
        assert_eq!(history.list(None, 10).unwrap(), [completed]);
        assert!(history.delete(id).unwrap());
        assert!(history.list(None, 10).unwrap().is_empty());
    }

    #[test]
    fn pagination_uses_stable_timestamp_and_execution_cursor() {
        let history = SqliteSkillExecutionHistory::in_memory().unwrap();
        let oldest = Uuid::now_v7();
        let newest = Uuid::now_v7();
        history.save(&record(oldest, 100)).unwrap();
        history.save(&record(newest, 200)).unwrap();
        assert_eq!(history.list(None, 1).unwrap()[0].execution_id, newest);
        assert_eq!(
            history.list(Some((200, newest)), 1).unwrap()[0].execution_id,
            oldest
        );
    }

    #[test]
    fn cancelled_status_cannot_be_overwritten_by_completion() {
        let history = SqliteSkillExecutionHistory::in_memory().unwrap();
        let id = Uuid::now_v7();
        let cancelled = SkillExecutionHistoryRecord::new(
            id,
            "studio.example.focus",
            SkillExecutionHistoryStatus::Cancelled,
            1,
            1,
            100,
            200,
            None,
        )
        .unwrap();
        history.save(&cancelled).unwrap();
        history
            .save(
                &SkillExecutionHistoryRecord::new(
                    id,
                    "studio.example.focus",
                    SkillExecutionHistoryStatus::Completed,
                    1,
                    1,
                    100,
                    300,
                    None,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(history.list(None, 10).unwrap(), [cancelled]);
    }
}
