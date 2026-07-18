use crate::{SqlitePersistenceError, prepare_connection};
use nimora_automation_runtime::AutomationRun;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Mutex};
use uuid::Uuid;

const AUTOMATION_JOURNAL_VERSION: u32 = 1;
const MAX_INTERRUPTION_REASON_BYTES: usize = 4 * 1024;
const MAX_AUTOMATION_HISTORY_PAGE_SIZE: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationRunStart {
    pub run_id: Uuid,
    pub automation_id: String,
    pub trace_id: Uuid,
    pub event_id: String,
    pub started_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationJournalStatus {
    Running,
    Completed,
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutomationJournalEntry {
    pub spec: String,
    pub run_id: Uuid,
    pub automation_id: String,
    pub trace_id: Uuid,
    pub event_id: String,
    pub status: AutomationJournalStatus,
    pub started_at_ms: u64,
    pub updated_at_ms: u64,
    pub result: Option<AutomationRun>,
    pub interruption_reason: Option<String>,
}

#[derive(Debug)]
pub struct SqliteAutomationJournal {
    connection: Mutex<Connection>,
}

impl SqliteAutomationJournal {
    /// Opens the Automation journal in the shared application database.
    ///
    /// # Errors
    ///
    /// Returns an error when the database cannot be opened or initialized.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Creates an isolated Automation journal for tests.
    ///
    /// # Errors
    ///
    /// Returns an error when the database cannot be initialized.
    pub fn in_memory() -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut connection: Connection) -> Result<Self, SqlitePersistenceError> {
        prepare_connection(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Records a run before any live action is dispatched.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid metadata, duplicate IDs, or storage failures.
    pub fn start(&self, start: &AutomationRunStart) -> Result<(), SqlitePersistenceError> {
        validate_start(start)?;
        self.connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .execute(
                "INSERT INTO automation_run_journal
                    (run_id, automation_id, trace_id, event_id, status, started_at_ms,
                     updated_at_ms, schema_version, payload, interruption_reason)
                 VALUES (?1, ?2, ?3, ?4, 'running', ?5, ?5, ?6, NULL, NULL)",
                params![
                    start.run_id.to_string(),
                    start.automation_id,
                    start.trace_id.to_string(),
                    start.event_id,
                    i64::try_from(start.started_at_ms)
                        .map_err(|_| SqlitePersistenceError::InvalidAutomationJournal)?,
                    AUTOMATION_JOURNAL_VERSION,
                ],
            )?;
        Ok(())
    }

    /// Atomically completes a running journal entry with its immutable result.
    ///
    /// # Errors
    ///
    /// Returns an error when identity, timestamps, state, or payload are inconsistent.
    pub fn complete(
        &self,
        run: &AutomationRun,
        completed_at_ms: u64,
    ) -> Result<(), SqlitePersistenceError> {
        let payload = serde_json::to_string(run)?;
        let completed_at_ms = i64::try_from(completed_at_ms)
            .map_err(|_| SqlitePersistenceError::InvalidAutomationJournal)?;
        let changed = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .execute(
                "UPDATE automation_run_journal
                 SET status = 'completed', updated_at_ms = ?2, payload = ?3
                 WHERE run_id = ?1 AND automation_id = ?4 AND trace_id = ?5 AND event_id = ?6
                   AND status = 'running' AND started_at_ms <= ?2",
                params![
                    run.run_id.to_string(),
                    completed_at_ms,
                    payload,
                    run.automation_id,
                    run.trace_id.to_string(),
                    run.event_id,
                ],
            )?;
        if changed != 1 {
            return Err(SqlitePersistenceError::InvalidAutomationJournal);
        }
        Ok(())
    }

    /// Marks all crash-left running entries as interrupted.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid recovery metadata or storage failures.
    pub fn recover_running(
        &self,
        recovered_at_ms: u64,
        reason: &str,
    ) -> Result<usize, SqlitePersistenceError> {
        if reason.trim().is_empty() || reason.len() > MAX_INTERRUPTION_REASON_BYTES {
            return Err(SqlitePersistenceError::InvalidAutomationJournal);
        }
        let recovered_at_ms = i64::try_from(recovered_at_ms)
            .map_err(|_| SqlitePersistenceError::InvalidAutomationJournal)?;
        self.connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .execute(
                "UPDATE automation_run_journal
                 SET status = 'interrupted', updated_at_ms = MAX(updated_at_ms, ?1),
                     interruption_reason = ?2
                 WHERE status = 'running'",
                params![recovered_at_ms, reason],
            )
            .map_err(Into::into)
    }

    /// Loads one run journal entry by stable ID.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed stored data or storage failures.
    pub fn get(
        &self,
        run_id: Uuid,
    ) -> Result<Option<AutomationJournalEntry>, SqlitePersistenceError> {
        let row = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .query_row(
                "SELECT automation_id, trace_id, event_id, status, started_at_ms,
                        updated_at_ms, schema_version, payload, interruption_reason
                 FROM automation_run_journal WHERE run_id = ?1",
                [run_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, u32>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, Option<String>>(8)?,
                    ))
                },
            )
            .optional()?;
        row.map(|row| decode_entry(run_id, row)).transpose()
    }

    /// Lists a bounded newest-first page of Automation runs.
    ///
    /// # Errors
    ///
    /// Rejects zero or oversized limits and malformed stored rows.
    pub fn list(
        &self,
        limit: usize,
    ) -> Result<Vec<AutomationJournalEntry>, SqlitePersistenceError> {
        if limit == 0 || limit > MAX_AUTOMATION_HISTORY_PAGE_SIZE {
            return Err(SqlitePersistenceError::InvalidAutomationJournal);
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let mut statement = connection.prepare(
            "SELECT run_id, automation_id, trace_id, event_id, status, started_at_ms,
                    updated_at_ms, schema_version, payload, interruption_reason
             FROM automation_run_journal
             ORDER BY started_at_ms DESC, run_id DESC LIMIT ?1",
        )?;
        statement
            .query_map(
                [i64::try_from(limit)
                    .map_err(|_| SqlitePersistenceError::InvalidAutomationJournal)?],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, u32>(7)?,
                        row.get::<_, Option<String>>(8)?,
                        row.get::<_, Option<String>>(9)?,
                    ))
                },
            )?
            .map(|row| {
                let (
                    run_id,
                    automation_id,
                    trace_id,
                    event_id,
                    status,
                    started,
                    updated,
                    version,
                    payload,
                    reason,
                ) = row?;
                let run_id = Uuid::parse_str(&run_id)
                    .map_err(|_| SqlitePersistenceError::InvalidAutomationJournal)?;
                decode_entry(
                    run_id,
                    (
                        automation_id,
                        trace_id,
                        event_id,
                        status,
                        started,
                        updated,
                        version,
                        payload,
                        reason,
                    ),
                )
            })
            .collect()
    }

    /// Deletes one terminal run or all terminal Automation history.
    ///
    /// # Errors
    ///
    /// Returns an error for storage failures. Running rows are always retained.
    pub fn delete_terminal(&self, run_id: Option<Uuid>) -> Result<usize, SqlitePersistenceError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        match run_id {
            Some(run_id) => connection.execute(
                "DELETE FROM automation_run_journal WHERE run_id = ?1 AND status != 'running'",
                [run_id.to_string()],
            ),
            None => connection.execute(
                "DELETE FROM automation_run_journal WHERE status != 'running'",
                [],
            ),
        }
        .map_err(Into::into)
    }
}

fn validate_start(start: &AutomationRunStart) -> Result<(), SqlitePersistenceError> {
    if start.run_id.is_nil()
        || start.trace_id.is_nil()
        || start.automation_id.trim().is_empty()
        || start.event_id.trim().is_empty()
    {
        return Err(SqlitePersistenceError::InvalidAutomationJournal);
    }
    Ok(())
}

type StoredJournalRow = (
    String,
    String,
    String,
    String,
    i64,
    i64,
    u32,
    Option<String>,
    Option<String>,
);

fn decode_entry(
    run_id: Uuid,
    row: StoredJournalRow,
) -> Result<AutomationJournalEntry, SqlitePersistenceError> {
    let (automation_id, trace_id, event_id, status, started, updated, version, payload, reason) =
        row;
    if version != AUTOMATION_JOURNAL_VERSION {
        return Err(SqlitePersistenceError::UnsupportedAutomationJournalVersion(
            version,
        ));
    }
    let status = match status.as_str() {
        "running" => AutomationJournalStatus::Running,
        "completed" => AutomationJournalStatus::Completed,
        "interrupted" => AutomationJournalStatus::Interrupted,
        _ => return Err(SqlitePersistenceError::InvalidAutomationJournal),
    };
    let result = payload
        .map(|value| serde_json::from_str::<AutomationRun>(&value))
        .transpose()?;
    let entry = AutomationJournalEntry {
        spec: "nimora.automation-journal-entry/1".to_owned(),
        run_id,
        automation_id,
        trace_id: trace_id
            .parse()
            .map_err(|_| SqlitePersistenceError::InvalidAutomationJournal)?,
        event_id,
        status,
        started_at_ms: u64::try_from(started)
            .map_err(|_| SqlitePersistenceError::InvalidAutomationJournal)?,
        updated_at_ms: u64::try_from(updated)
            .map_err(|_| SqlitePersistenceError::InvalidAutomationJournal)?,
        result,
        interruption_reason: reason,
    };
    validate_entry(&entry)?;
    Ok(entry)
}

fn validate_entry(entry: &AutomationJournalEntry) -> Result<(), SqlitePersistenceError> {
    let valid_payload = match entry.status {
        AutomationJournalStatus::Running => {
            entry.result.is_none() && entry.interruption_reason.is_none()
        }
        AutomationJournalStatus::Completed => {
            entry.result.as_ref().is_some_and(|run| {
                run.run_id == entry.run_id
                    && run.automation_id == entry.automation_id
                    && run.trace_id == entry.trace_id
                    && run.event_id == entry.event_id
            }) && entry.interruption_reason.is_none()
        }
        AutomationJournalStatus::Interrupted => {
            entry.result.is_none()
                && entry
                    .interruption_reason
                    .as_ref()
                    .is_some_and(|reason| !reason.trim().is_empty())
        }
    };
    if !valid_payload || entry.updated_at_ms < entry.started_at_ms {
        return Err(SqlitePersistenceError::InvalidAutomationJournal);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimora_automation_runtime::{AutomationRunStatus, AutomationStepResult};

    fn start() -> AutomationRunStart {
        AutomationRunStart {
            run_id: Uuid::now_v7(),
            automation_id: "local.focus.summary".to_owned(),
            trace_id: Uuid::now_v7(),
            event_id: "event:test".to_owned(),
            started_at_ms: 1_000,
        }
    }

    fn result(start: &AutomationRunStart) -> AutomationRun {
        AutomationRun {
            spec: "nimora.automation-run/1".to_owned(),
            run_id: start.run_id,
            automation_id: start.automation_id.clone(),
            trace_id: start.trace_id,
            event_id: start.event_id.clone(),
            mode: "live".to_owned(),
            status: AutomationRunStatus::Succeeded,
            steps: Vec::<AutomationStepResult>::new(),
            reason: None,
        }
    }

    #[test]
    fn start_and_complete_preserve_run_identity() {
        let journal = SqliteAutomationJournal::in_memory().expect("journal");
        let start = start();
        journal.start(&start).expect("start");
        journal.complete(&result(&start), 1_500).expect("complete");
        let entry = journal.get(start.run_id).expect("get").expect("entry");
        assert_eq!(entry.status, AutomationJournalStatus::Completed);
        assert_eq!(entry.result.expect("result").run_id, start.run_id);
    }

    #[test]
    fn recovery_interrupts_only_unfinished_runs() {
        let journal = SqliteAutomationJournal::in_memory().expect("journal");
        let completed = start();
        journal.start(&completed).expect("start completed");
        journal
            .complete(&result(&completed), 1_500)
            .expect("complete");
        let running = start();
        journal.start(&running).expect("start running");
        assert_eq!(
            journal
                .recover_running(2_000, "desktop restarted")
                .expect("recover"),
            1
        );
        assert_eq!(
            journal
                .get(running.run_id)
                .expect("get")
                .expect("entry")
                .status,
            AutomationJournalStatus::Interrupted
        );
        assert_eq!(
            journal
                .get(completed.run_id)
                .expect("get")
                .expect("entry")
                .status,
            AutomationJournalStatus::Completed
        );
    }

    #[test]
    fn duplicate_or_terminal_completion_is_rejected() {
        let journal = SqliteAutomationJournal::in_memory().expect("journal");
        let start = start();
        journal.start(&start).expect("start");
        assert!(journal.start(&start).is_err());
        let result = result(&start);
        journal.complete(&result, 1_500).expect("complete");
        assert!(matches!(
            journal.complete(&result, 1_600),
            Err(SqlitePersistenceError::InvalidAutomationJournal)
        ));
    }

    #[test]
    fn history_is_bounded_ordered_and_never_deletes_running_rows() {
        let journal = SqliteAutomationJournal::in_memory().expect("journal");
        let older = start();
        journal.start(&older).expect("start older");
        journal
            .complete(&result(&older), 1_500)
            .expect("complete older");
        let mut newer = start();
        newer.started_at_ms = 2_000;
        journal.start(&newer).expect("start newer");

        let entries = journal.list(2).expect("history");
        assert_eq!(entries[0].run_id, newer.run_id);
        assert_eq!(entries[1].run_id, older.run_id);
        assert!(journal.list(0).is_err());
        assert!(journal.list(MAX_AUTOMATION_HISTORY_PAGE_SIZE + 1).is_err());
        assert_eq!(
            journal
                .delete_terminal(Some(newer.run_id))
                .expect("retain running"),
            0
        );
        assert_eq!(
            journal
                .delete_terminal(Some(older.run_id))
                .expect("delete older"),
            1
        );
        assert_eq!(journal.list(10).expect("remaining").len(), 1);
    }
}
