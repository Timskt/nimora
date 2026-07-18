use crate::{SqlitePersistenceError, prepare_connection};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Mutex};
use uuid::Uuid;

const SKILL_APPROVAL_JOURNAL_VERSION: u32 = 1;
const MAX_SKILL_ID_BYTES: usize = 160;
const MAX_PLAN_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillApprovalJournalStatus {
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
pub struct SkillApprovalJournalEntry {
    pub spec: String,
    pub approval_id: Uuid,
    pub execution_id: Uuid,
    pub skill_id: String,
    pub status: SkillApprovalJournalStatus,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub expires_at_ms: u64,
    pub plan: serde_json::Value,
    pub error: Option<String>,
}

impl SkillApprovalJournalEntry {
    /// Creates one immutable pending Skill execution plan.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identity, bounds, timestamps, or plan payload.
    pub fn new(
        approval_id: Uuid,
        execution_id: Uuid,
        skill_id: impl Into<String>,
        created_at_ms: u64,
        expires_at_ms: u64,
        plan: serde_json::Value,
    ) -> Result<Self, SqlitePersistenceError> {
        let entry = Self {
            spec: "nimora.skill-approval-journal/1".to_owned(),
            approval_id,
            execution_id,
            skill_id: skill_id.into(),
            status: SkillApprovalJournalStatus::Pending,
            created_at_ms,
            updated_at_ms: created_at_ms,
            expires_at_ms,
            plan,
            error: None,
        };
        validate_entry(&entry)?;
        Ok(entry)
    }
}

#[derive(Debug)]
pub struct SqliteSkillApprovalJournal {
    connection: Mutex<Connection>,
}

impl SqliteSkillApprovalJournal {
    /// Opens the Skill approval journal in the shared application database.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot initialize the schema.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Creates an isolated journal for tests and Recovery Mode.
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

    /// Inserts one pending plan exactly once.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid data, duplicate IDs, or `SQLite` failure.
    pub fn insert(&self, entry: &SkillApprovalJournalEntry) -> Result<(), SqlitePersistenceError> {
        validate_entry(entry)?;
        if entry.status != SkillApprovalJournalStatus::Pending {
            return Err(SqlitePersistenceError::InvalidSkillApprovalJournal);
        }
        self.connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .execute(
                "INSERT INTO skill_approval_journal
                    (approval_id, execution_id, skill_id, status, created_at_ms, updated_at_ms,
                     expires_at_ms, schema_version, payload)
                 VALUES (?1, ?2, ?3, 'pending', ?4, ?4, ?5, ?6, ?7)",
                params![
                    entry.approval_id.to_string(),
                    entry.execution_id.to_string(),
                    entry.skill_id,
                    to_i64(entry.created_at_ms)?,
                    to_i64(entry.expires_at_ms)?,
                    SKILL_APPROVAL_JOURNAL_VERSION,
                    serde_json::to_string(entry)?,
                ],
            )?;
        Ok(())
    }

    /// Counts non-expired plans that are still awaiting a user decision.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid time bounds or `SQLite` failure.
    pub fn pending_count(&self, now_ms: u64) -> Result<usize, SqlitePersistenceError> {
        let count = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .query_row(
                "SELECT COUNT(*) FROM skill_approval_journal
                 WHERE status = 'pending' AND expires_at_ms > ?1",
                [to_i64(now_ms)?],
                |row| row.get::<_, i64>(0),
            )?;
        usize::try_from(count).map_err(|_| SqlitePersistenceError::InvalidSkillApprovalJournal)
    }

    /// Lists non-expired pending plans in stable creation order.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid time bounds, malformed records, or `SQLite` failure.
    pub fn list_pending(
        &self,
        now_ms: u64,
        limit: usize,
    ) -> Result<Vec<SkillApprovalJournalEntry>, SqlitePersistenceError> {
        if limit == 0 || limit > 200 {
            return Err(SqlitePersistenceError::InvalidSkillApprovalJournal);
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let mut statement = connection.prepare(
            "SELECT schema_version, payload FROM skill_approval_journal
             WHERE status = 'pending' AND expires_at_ms > ?1
             ORDER BY created_at_ms ASC, approval_id ASC LIMIT ?2",
        )?;
        statement
            .query_map(
                params![
                    to_i64(now_ms)?,
                    i64::try_from(limit)
                        .map_err(|_| { SqlitePersistenceError::InvalidSkillApprovalJournal })?
                ],
                |row| Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?)),
            )?
            .map(|row| {
                let (version, payload) = row?;
                decode_entry(version, &payload)
            })
            .collect()
    }

    /// Atomically claims a non-expired pending plan for one executor.
    ///
    /// # Errors
    ///
    /// Returns an error when the plan is missing, expired, already resolved, or malformed.
    pub fn claim(
        &self,
        approval_id: Uuid,
        now_ms: u64,
    ) -> Result<SkillApprovalJournalEntry, SqlitePersistenceError> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let transaction = connection.transaction()?;
        let row = transaction
            .query_row(
                "SELECT schema_version, payload FROM skill_approval_journal
                 WHERE approval_id = ?1",
                [approval_id.to_string()],
                |row| Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
            .ok_or(SqlitePersistenceError::SkillApprovalNotPending)?;
        let mut entry = decode_entry(row.0, &row.1)?;
        if entry.status != SkillApprovalJournalStatus::Pending {
            return Err(SqlitePersistenceError::SkillApprovalNotPending);
        }
        if entry.expires_at_ms <= now_ms {
            entry.status = SkillApprovalJournalStatus::Expired;
            entry.updated_at_ms = now_ms;
            write_entry(&transaction, &entry)?;
            transaction.commit()?;
            return Err(SqlitePersistenceError::SkillApprovalExpired);
        }
        entry.status = SkillApprovalJournalStatus::Executing;
        entry.updated_at_ms = now_ms;
        write_entry(&transaction, &entry)?;
        transaction.commit()?;
        Ok(entry)
    }

    /// Rejects a non-expired pending plan without returning its private payload.
    ///
    /// # Errors
    ///
    /// Returns an error when the plan is missing, expired, or already resolved.
    pub fn reject(
        &self,
        approval_id: Uuid,
        now_ms: u64,
    ) -> Result<SkillApprovalJournalEntry, SqlitePersistenceError> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let transaction = connection.transaction()?;
        let row = transaction
            .query_row(
                "SELECT schema_version, payload FROM skill_approval_journal
                 WHERE approval_id = ?1",
                [approval_id.to_string()],
                |row| Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
            .ok_or(SqlitePersistenceError::SkillApprovalNotPending)?;
        let mut entry = decode_entry(row.0, &row.1)?;
        if entry.status != SkillApprovalJournalStatus::Pending {
            return Err(SqlitePersistenceError::SkillApprovalNotPending);
        }
        if entry.expires_at_ms <= now_ms {
            entry.status = SkillApprovalJournalStatus::Expired;
            entry.updated_at_ms = now_ms;
            write_entry(&transaction, &entry)?;
            transaction.commit()?;
            return Err(SqlitePersistenceError::SkillApprovalExpired);
        }
        entry.status = SkillApprovalJournalStatus::Rejected;
        entry.updated_at_ms = now_ms;
        write_entry(&transaction, &entry)?;
        transaction.commit()?;
        Ok(entry)
    }

    /// Finalizes a claimed plan.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid terminal state, stale claim, bounds, or `SQLite` failure.
    pub fn finish(
        &self,
        approval_id: Uuid,
        status: SkillApprovalJournalStatus,
        now_ms: u64,
        error: Option<String>,
    ) -> Result<(), SqlitePersistenceError> {
        if !matches!(
            status,
            SkillApprovalJournalStatus::Completed
                | SkillApprovalJournalStatus::Rejected
                | SkillApprovalJournalStatus::Failed
        ) || error.as_ref().is_some_and(|value| value.len() > 4 * 1024)
        {
            return Err(SqlitePersistenceError::InvalidSkillApprovalJournal);
        }
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let transaction = connection.transaction()?;
        let row = transaction
            .query_row(
                "SELECT schema_version, payload FROM skill_approval_journal
                 WHERE approval_id = ?1 AND status = 'executing'",
                [approval_id.to_string()],
                |row| Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
            .ok_or(SqlitePersistenceError::SkillApprovalNotPending)?;
        let mut entry = decode_entry(row.0, &row.1)?;
        entry.status = status;
        entry.updated_at_ms = now_ms;
        entry.error = error;
        validate_entry(&entry)?;
        write_entry(&transaction, &entry)?;
        transaction.commit()?;
        Ok(())
    }

    /// Recovers process-owned states and expires stale pending approvals.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed records or `SQLite` failure.
    pub fn recover(&self, now_ms: u64) -> Result<usize, SqlitePersistenceError> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let transaction = connection.transaction()?;
        let mut statement = transaction.prepare(
            "SELECT schema_version, payload FROM skill_approval_journal
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
            let mut entry = decode_entry(version, &payload)?;
            let next = if entry.status == SkillApprovalJournalStatus::Executing {
                Some(SkillApprovalJournalStatus::Interrupted)
            } else if entry.expires_at_ms <= now_ms {
                Some(SkillApprovalJournalStatus::Expired)
            } else {
                None
            };
            if let Some(status) = next {
                entry.status = status;
                entry.updated_at_ms = now_ms;
                write_entry(&transaction, &entry)?;
                changed += 1;
            }
        }
        transaction.commit()?;
        Ok(changed)
    }
}

fn write_entry(
    transaction: &rusqlite::Transaction<'_>,
    entry: &SkillApprovalJournalEntry,
) -> Result<(), SqlitePersistenceError> {
    validate_entry(entry)?;
    let changed = transaction.execute(
        "UPDATE skill_approval_journal SET status = ?2, updated_at_ms = ?3, payload = ?4
         WHERE approval_id = ?1",
        params![
            entry.approval_id.to_string(),
            status_name(entry.status),
            to_i64(entry.updated_at_ms)?,
            serde_json::to_string(entry)?,
        ],
    )?;
    if changed != 1 {
        return Err(SqlitePersistenceError::SkillApprovalNotPending);
    }
    Ok(())
}

fn decode_entry(
    version: u32,
    payload: &str,
) -> Result<SkillApprovalJournalEntry, SqlitePersistenceError> {
    if version != SKILL_APPROVAL_JOURNAL_VERSION {
        return Err(SqlitePersistenceError::UnsupportedSkillApprovalJournalVersion(version));
    }
    let entry: SkillApprovalJournalEntry = serde_json::from_str(payload)?;
    validate_entry(&entry)?;
    Ok(entry)
}

fn validate_entry(entry: &SkillApprovalJournalEntry) -> Result<(), SqlitePersistenceError> {
    let plan_bytes = serde_json::to_vec(&entry.plan)?.len();
    if entry.spec != "nimora.skill-approval-journal/1"
        || entry.approval_id.is_nil()
        || entry.execution_id.is_nil()
        || entry.skill_id.trim().is_empty()
        || entry.skill_id.len() > MAX_SKILL_ID_BYTES
        || entry.skill_id.chars().any(char::is_control)
        || entry.updated_at_ms < entry.created_at_ms
        || entry.expires_at_ms <= entry.created_at_ms
        || plan_bytes == 0
        || plan_bytes > MAX_PLAN_BYTES
        || (entry.status != SkillApprovalJournalStatus::Failed && entry.error.is_some())
    {
        return Err(SqlitePersistenceError::InvalidSkillApprovalJournal);
    }
    Ok(())
}

const fn status_name(status: SkillApprovalJournalStatus) -> &'static str {
    match status {
        SkillApprovalJournalStatus::Pending => "pending",
        SkillApprovalJournalStatus::Executing => "executing",
        SkillApprovalJournalStatus::Completed => "completed",
        SkillApprovalJournalStatus::Rejected => "rejected",
        SkillApprovalJournalStatus::Expired => "expired",
        SkillApprovalJournalStatus::Failed => "failed",
        SkillApprovalJournalStatus::Interrupted => "interrupted",
    }
}

fn to_i64(value: u64) -> Result<i64, SqlitePersistenceError> {
    i64::try_from(value).map_err(|_| SqlitePersistenceError::InvalidSkillApprovalJournal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> SkillApprovalJournalEntry {
        SkillApprovalJournalEntry::new(
            Uuid::now_v7(),
            Uuid::now_v7(),
            "studio.example.focus",
            1_000,
            2_000,
            serde_json::json!({"commands": [{"id": "safe.profile.switch"}]}),
        )
        .expect("valid entry")
    }

    #[test]
    fn claim_is_atomic_and_terminal_state_is_one_way() {
        let journal = SqliteSkillApprovalJournal::in_memory().expect("journal");
        let entry = entry();
        journal.insert(&entry).expect("insert");
        assert_eq!(
            journal.claim(entry.approval_id, 1_100).unwrap().plan,
            entry.plan
        );
        assert!(matches!(
            journal.claim(entry.approval_id, 1_101),
            Err(SqlitePersistenceError::SkillApprovalNotPending)
        ));
        journal
            .finish(
                entry.approval_id,
                SkillApprovalJournalStatus::Completed,
                1_200,
                None,
            )
            .expect("finish");
        assert!(matches!(
            journal.finish(
                entry.approval_id,
                SkillApprovalJournalStatus::Completed,
                1_201,
                None,
            ),
            Err(SqlitePersistenceError::SkillApprovalNotPending)
        ));
    }

    #[test]
    fn recovery_interrupts_claims_and_expires_only_stale_pending() {
        let journal = SqliteSkillApprovalJournal::in_memory().expect("journal");
        let claimed = entry();
        journal.insert(&claimed).expect("insert claimed");
        journal.claim(claimed.approval_id, 1_100).expect("claim");
        let mut expired = entry();
        expired.approval_id = Uuid::now_v7();
        expired.execution_id = Uuid::now_v7();
        journal.insert(&expired).expect("insert expired");
        assert_eq!(journal.recover(2_500).expect("recover"), 2);
        assert!(matches!(
            journal.claim(claimed.approval_id, 2_501),
            Err(SqlitePersistenceError::SkillApprovalNotPending)
        ));
        assert!(matches!(
            journal.claim(expired.approval_id, 2_501),
            Err(SqlitePersistenceError::SkillApprovalNotPending)
        ));
    }

    #[test]
    fn pending_plan_survives_reopen_and_remains_queryable() {
        let path =
            std::env::temp_dir().join(format!("nimora-skill-approval-{}.sqlite3", Uuid::now_v7()));
        let entry = entry();
        {
            let journal = SqliteSkillApprovalJournal::open(&path).expect("journal");
            journal.insert(&entry).expect("insert");
        }
        let reopened = SqliteSkillApprovalJournal::open(&path).expect("reopened journal");
        assert_eq!(reopened.recover(1_500).expect("recover"), 0);
        assert_eq!(
            reopened.list_pending(1_500, 10).expect("pending list"),
            vec![entry]
        );
        std::fs::remove_file(path).expect("fixture cleanup");
    }
}
