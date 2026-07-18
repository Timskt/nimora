use crate::{SqlitePersistenceError, prepare_connection};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Mutex};
use uuid::Uuid;

const JOURNAL_VERSION: u32 = 1;
const MAX_AUTOMATION_ID_BYTES: usize = 160;
const MAX_PLAN_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationApprovalStatus {
    Pending,
    Executing,
    Completed,
    Rejected,
    Expired,
    Failed,
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutomationApprovalEntry {
    pub spec: String,
    pub approval_id: Uuid,
    pub run_id: Uuid,
    pub automation_id: String,
    pub status: AutomationApprovalStatus,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub expires_at_ms: u64,
    pub plan: serde_json::Value,
    pub error: Option<String>,
}

impl AutomationApprovalEntry {
    /// Creates one immutable approval request.
    ///
    /// # Errors
    ///
    /// Returns an error when identities, timestamps, or the bounded plan are invalid.
    pub fn new(
        approval_id: Uuid,
        run_id: Uuid,
        automation_id: impl Into<String>,
        created_at_ms: u64,
        expires_at_ms: u64,
        plan: serde_json::Value,
    ) -> Result<Self, SqlitePersistenceError> {
        let entry = Self {
            spec: "nimora.automation-approval-journal/1".to_owned(),
            approval_id,
            run_id,
            automation_id: automation_id.into(),
            status: AutomationApprovalStatus::Pending,
            created_at_ms,
            updated_at_ms: created_at_ms,
            expires_at_ms,
            plan,
            error: None,
        };
        validate(&entry)?;
        Ok(entry)
    }
}

#[derive(Debug)]
pub struct SqliteAutomationApprovalJournal {
    connection: Mutex<Connection>,
}

impl SqliteAutomationApprovalJournal {
    /// Opens the journal in the shared application database.
    ///
    /// # Errors
    ///
    /// Returns an error when the database cannot be opened or prepared.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Creates an isolated in-memory journal.
    ///
    /// # Errors
    ///
    /// Returns an error when the schema cannot be prepared.
    pub fn in_memory() -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut connection: Connection) -> Result<Self, SqlitePersistenceError> {
        prepare_connection(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Inserts one pending plan exactly once.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid data, duplicate identities, or database failure.
    pub fn insert(&self, entry: &AutomationApprovalEntry) -> Result<(), SqlitePersistenceError> {
        validate(entry)?;
        if entry.status != AutomationApprovalStatus::Pending {
            return Err(SqlitePersistenceError::InvalidAutomationApprovalJournal);
        }
        self.connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .execute(
                "INSERT INTO automation_approval_journal
                    (approval_id, run_id, automation_id, status, created_at_ms, updated_at_ms,
                     expires_at_ms, schema_version, payload)
                 VALUES (?1, ?2, ?3, 'pending', ?4, ?4, ?5, ?6, ?7)",
                params![
                    entry.approval_id.to_string(),
                    entry.run_id.to_string(),
                    entry.automation_id,
                    to_i64(entry.created_at_ms)?,
                    to_i64(entry.expires_at_ms)?,
                    JOURNAL_VERSION,
                    serde_json::to_string(entry)?,
                ],
            )?;
        Ok(())
    }

    /// Lists non-expired pending approvals in stable order.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid bounds, malformed rows, or database failure.
    pub fn list_pending(
        &self,
        now_ms: u64,
        limit: usize,
    ) -> Result<Vec<AutomationApprovalEntry>, SqlitePersistenceError> {
        if limit == 0 || limit > 256 {
            return Err(SqlitePersistenceError::InvalidAutomationApprovalJournal);
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let mut statement = connection.prepare(
            "SELECT schema_version, payload FROM automation_approval_journal
             WHERE status = 'pending' AND expires_at_ms > ?1
             ORDER BY created_at_ms, approval_id LIMIT ?2",
        )?;
        statement
            .query_map(
                params![
                    to_i64(now_ms)?,
                    i64::try_from(limit)
                        .map_err(|_| SqlitePersistenceError::InvalidAutomationApprovalJournal)?
                ],
                |row| Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?)),
            )?
            .map(|row| {
                let (version, payload) = row?;
                decode(version, &payload)
            })
            .collect()
    }

    /// Counts non-expired approvals awaiting a decision.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid time bounds or database failure.
    pub fn pending_count(&self, now_ms: u64) -> Result<usize, SqlitePersistenceError> {
        let count = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .query_row(
                "SELECT COUNT(*) FROM automation_approval_journal WHERE status = 'pending' AND expires_at_ms > ?1",
                [to_i64(now_ms)?],
                |row| row.get::<_, i64>(0),
            )?;
        usize::try_from(count).map_err(|_| SqlitePersistenceError::InvalidAutomationApprovalJournal)
    }

    /// Atomically claims one non-expired pending approval.
    ///
    /// # Errors
    ///
    /// Returns an error when the approval is missing, expired, already resolved, or malformed.
    pub fn claim(
        &self,
        approval_id: Uuid,
        now_ms: u64,
    ) -> Result<AutomationApprovalEntry, SqlitePersistenceError> {
        self.transition_pending(approval_id, now_ms, AutomationApprovalStatus::Executing)
    }

    /// Atomically rejects one non-expired pending approval.
    ///
    /// # Errors
    ///
    /// Returns an error when the approval is missing, expired, already resolved, or malformed.
    pub fn reject(
        &self,
        approval_id: Uuid,
        now_ms: u64,
    ) -> Result<AutomationApprovalEntry, SqlitePersistenceError> {
        self.transition_pending(approval_id, now_ms, AutomationApprovalStatus::Rejected)
    }

    fn transition_pending(
        &self,
        approval_id: Uuid,
        now_ms: u64,
        status: AutomationApprovalStatus,
    ) -> Result<AutomationApprovalEntry, SqlitePersistenceError> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let transaction = connection.transaction()?;
        let row = transaction
            .query_row(
                "SELECT schema_version, payload FROM automation_approval_journal
                 WHERE approval_id = ?1 AND status = 'pending'",
                [approval_id.to_string()],
                |row| Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
            .ok_or(SqlitePersistenceError::AutomationApprovalNotPending)?;
        let mut entry = decode(row.0, &row.1)?;
        if entry.expires_at_ms <= now_ms {
            entry.status = AutomationApprovalStatus::Expired;
            entry.updated_at_ms = now_ms;
            write(&transaction, &entry)?;
            transaction.commit()?;
            return Err(SqlitePersistenceError::AutomationApprovalExpired);
        }
        entry.status = status;
        entry.updated_at_ms = now_ms;
        write(&transaction, &entry)?;
        transaction.commit()?;
        Ok(entry)
    }

    /// Completes one previously claimed approval.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid terminal state, stale claim, malformed row, or database failure.
    pub fn finish(
        &self,
        approval_id: Uuid,
        status: AutomationApprovalStatus,
        now_ms: u64,
        error: Option<String>,
    ) -> Result<(), SqlitePersistenceError> {
        if !matches!(
            status,
            AutomationApprovalStatus::Completed | AutomationApprovalStatus::Failed
        ) {
            return Err(SqlitePersistenceError::InvalidAutomationApprovalJournal);
        }
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let transaction = connection.transaction()?;
        let row = transaction
            .query_row(
                "SELECT schema_version, payload FROM automation_approval_journal
                 WHERE approval_id = ?1 AND status = 'executing'",
                [approval_id.to_string()],
                |row| Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
            .ok_or(SqlitePersistenceError::AutomationApprovalNotPending)?;
        let mut entry = decode(row.0, &row.1)?;
        entry.status = status;
        entry.updated_at_ms = now_ms;
        entry.error = error;
        write(&transaction, &entry)?;
        transaction.commit()?;
        Ok(())
    }

    /// Interrupts process-owned executions and expires stale pending approvals.
    ///
    /// # Errors
    ///
    /// Returns an error when persisted rows are malformed or the transaction fails.
    pub fn recover(&self, now_ms: u64) -> Result<usize, SqlitePersistenceError> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let transaction = connection.transaction()?;
        let mut statement = transaction.prepare(
            "SELECT schema_version, payload FROM automation_approval_journal
             WHERE status IN ('pending', 'executing')",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let mut changed = 0;
        for (version, payload) in rows {
            let mut entry = decode(version, &payload)?;
            let next = if entry.status == AutomationApprovalStatus::Executing {
                Some(AutomationApprovalStatus::Interrupted)
            } else if entry.expires_at_ms <= now_ms {
                Some(AutomationApprovalStatus::Expired)
            } else {
                None
            };
            if let Some(status) = next {
                entry.status = status;
                entry.updated_at_ms = now_ms;
                write(&transaction, &entry)?;
                changed += 1;
            }
        }
        transaction.commit()?;
        Ok(changed)
    }
}

pub(crate) fn ensure_automation_approval_schema(
    connection: &Connection,
) -> Result<(), SqlitePersistenceError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS automation_approval_journal (
            approval_id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL UNIQUE,
            automation_id TEXT NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('pending', 'executing', 'completed',
                'rejected', 'expired', 'failed', 'interrupted')),
            created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
            updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= created_at_ms),
            expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms > created_at_ms),
            schema_version INTEGER NOT NULL,
            payload TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS automation_approval_journal_status_idx
            ON automation_approval_journal(status, expires_at_ms, approval_id);",
    )?;
    Ok(())
}

fn write(
    transaction: &rusqlite::Transaction<'_>,
    entry: &AutomationApprovalEntry,
) -> Result<(), SqlitePersistenceError> {
    validate(entry)?;
    let changed = transaction.execute(
        "UPDATE automation_approval_journal SET status = ?2, updated_at_ms = ?3, payload = ?4 WHERE approval_id = ?1",
        params![entry.approval_id.to_string(), status_name(entry.status), to_i64(entry.updated_at_ms)?, serde_json::to_string(entry)?],
    )?;
    if changed != 1 {
        return Err(SqlitePersistenceError::AutomationApprovalNotPending);
    }
    Ok(())
}

fn decode(version: u32, payload: &str) -> Result<AutomationApprovalEntry, SqlitePersistenceError> {
    if version != JOURNAL_VERSION {
        return Err(SqlitePersistenceError::UnsupportedAutomationApprovalJournalVersion(version));
    }
    let entry = serde_json::from_str(payload)?;
    validate(&entry)?;
    Ok(entry)
}

fn validate(entry: &AutomationApprovalEntry) -> Result<(), SqlitePersistenceError> {
    let plan_bytes = serde_json::to_vec(&entry.plan)?.len();
    if entry.spec != "nimora.automation-approval-journal/1"
        || entry.approval_id.is_nil()
        || entry.run_id.is_nil()
        || entry.automation_id.trim().is_empty()
        || entry.automation_id.len() > MAX_AUTOMATION_ID_BYTES
        || entry.automation_id.chars().any(char::is_control)
        || entry.updated_at_ms < entry.created_at_ms
        || entry.expires_at_ms <= entry.created_at_ms
        || plan_bytes == 0
        || plan_bytes > MAX_PLAN_BYTES
        || (entry.status != AutomationApprovalStatus::Failed && entry.error.is_some())
    {
        return Err(SqlitePersistenceError::InvalidAutomationApprovalJournal);
    }
    Ok(())
}

const fn status_name(status: AutomationApprovalStatus) -> &'static str {
    match status {
        AutomationApprovalStatus::Pending => "pending",
        AutomationApprovalStatus::Executing => "executing",
        AutomationApprovalStatus::Completed => "completed",
        AutomationApprovalStatus::Rejected => "rejected",
        AutomationApprovalStatus::Expired => "expired",
        AutomationApprovalStatus::Failed => "failed",
        AutomationApprovalStatus::Interrupted => "interrupted",
    }
}

fn to_i64(value: u64) -> Result<i64, SqlitePersistenceError> {
    i64::try_from(value).map_err(|_| SqlitePersistenceError::InvalidAutomationApprovalJournal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry() -> AutomationApprovalEntry {
        AutomationApprovalEntry::new(
            Uuid::now_v7(),
            Uuid::now_v7(),
            "local.test.approval",
            100,
            200,
            json!({"definitionVersion":"1.0.0","actions":[]}),
        )
        .expect("entry")
    }

    #[test]
    fn claim_is_atomic_and_single_use() {
        let journal = SqliteAutomationApprovalJournal::in_memory().expect("journal");
        let entry = entry();
        journal.insert(&entry).expect("insert");
        assert_eq!(
            journal.claim(entry.approval_id, 150).expect("claim").status,
            AutomationApprovalStatus::Executing
        );
        assert!(matches!(
            journal.claim(entry.approval_id, 150),
            Err(SqlitePersistenceError::AutomationApprovalNotPending)
        ));
        journal
            .finish(
                entry.approval_id,
                AutomationApprovalStatus::Completed,
                160,
                None,
            )
            .expect("finish");
    }

    #[test]
    fn recovery_expires_pending_and_interrupts_executing() {
        let journal = SqliteAutomationApprovalJournal::in_memory().expect("journal");
        let expired = entry();
        journal.insert(&expired).expect("expired insert");
        let mut executing = entry();
        executing.created_at_ms = 300;
        executing.updated_at_ms = 300;
        executing.expires_at_ms = 500;
        journal.insert(&executing).expect("executing insert");
        journal.claim(executing.approval_id, 350).expect("claim");
        assert_eq!(journal.recover(400).expect("recover"), 2);
        assert!(journal.list_pending(400, 10).expect("pending").is_empty());
    }
}
