use crate::{SqlitePersistenceError, prepare_connection};
use nimora_agent_runtime::AgentTaskAdmission;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Mutex};
use uuid::Uuid;

const AUTOMATION_AGENT_JOURNAL_VERSION: u32 = 1;
const MAX_MODEL_BYTES: usize = 128;
const MAX_IDEMPOTENCY_KEY_BYTES: usize = 128;
const MAX_ERROR_BYTES: usize = 4 * 1024;
const MAX_TASKS_PER_RUN: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationAgentJournalStatus {
    Submitted,
    WaitingForConfirmation,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutomationAgentJournalEntry {
    pub spec: String,
    pub run_id: Uuid,
    pub idempotency_key: String,
    pub admission: AgentTaskAdmission,
    pub model: String,
    pub status: AutomationAgentJournalStatus,
    pub submitted_at_ms: u64,
    pub updated_at_ms: u64,
    pub error: Option<String>,
}

impl AutomationAgentJournalEntry {
    /// Creates metadata for one admitted Automation child Agent task.
    ///
    /// # Errors
    ///
    /// Returns an error for inconsistent run identity, model, key, or timestamps.
    pub fn new(
        run_id: Uuid,
        idempotency_key: impl Into<String>,
        admission: AgentTaskAdmission,
        model: impl Into<String>,
        submitted_at_ms: u64,
    ) -> Result<Self, SqlitePersistenceError> {
        let idempotency_key = idempotency_key.into();
        let model = model.into();
        if run_id.is_nil()
            || admission.root_task_id != run_id
            || idempotency_key.is_empty()
            || idempotency_key.len() > MAX_IDEMPOTENCY_KEY_BYTES
            || idempotency_key.chars().any(char::is_control)
            || model.trim().is_empty()
            || model.len() > MAX_MODEL_BYTES
        {
            return Err(SqlitePersistenceError::InvalidAutomationAgentJournal);
        }
        Ok(Self {
            spec: "nimora.automation-agent-journal/1".to_owned(),
            run_id,
            idempotency_key,
            admission,
            model,
            status: AutomationAgentJournalStatus::Submitted,
            submitted_at_ms,
            updated_at_ms: submitted_at_ms,
            error: None,
        })
    }
}

#[derive(Debug)]
pub struct SqliteAutomationAgentJournal {
    connection: Mutex<Connection>,
}

impl SqliteAutomationAgentJournal {
    /// Opens the child task journal in the shared application database.
    ///
    /// # Errors
    ///
    /// Returns an error when the database cannot be initialized.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Creates an isolated journal for tests.
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

    /// Inserts one admitted child task exactly once per run and idempotency key.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid metadata, duplicates, or storage failures.
    pub fn submit(
        &self,
        entry: &AutomationAgentJournalEntry,
    ) -> Result<(), SqlitePersistenceError> {
        validate_entry(entry)?;
        let payload = serde_json::to_string(entry)?;
        self.connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .execute(
                "INSERT INTO automation_agent_journal
                    (task_id, run_id, idempotency_key, status, submitted_at_ms, updated_at_ms,
                     schema_version, payload)
                 VALUES (?1, ?2, ?3, 'submitted', ?4, ?4, ?5, ?6)",
                params![
                    entry.admission.task.id.to_string(),
                    entry.run_id.to_string(),
                    entry.idempotency_key,
                    i64::try_from(entry.submitted_at_ms)
                        .map_err(|_| SqlitePersistenceError::InvalidAutomationAgentJournal)?,
                    AUTOMATION_AGENT_JOURNAL_VERSION,
                    payload,
                ],
            )?;
        Ok(())
    }

    /// Finds a prior submission by its stable run-scoped idempotency identity.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed persisted data or storage failures.
    pub fn get_by_key(
        &self,
        run_id: Uuid,
        idempotency_key: &str,
    ) -> Result<Option<AutomationAgentJournalEntry>, SqlitePersistenceError> {
        let run_id = run_id.to_string();
        self.get_where(
            "run_id = ?1 AND idempotency_key = ?2",
            &run_id,
            idempotency_key,
        )
    }

    /// Finds a child task by task ID.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed persisted data or storage failures.
    pub fn get_by_task_id(
        &self,
        task_id: Uuid,
    ) -> Result<Option<AutomationAgentJournalEntry>, SqlitePersistenceError> {
        let task_id = task_id.to_string();
        self.get_where("task_id = ?1 AND ?2 = ?2", &task_id, "")
    }

    /// Lists the bounded child task history for one Automation run.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed persisted data or storage failures.
    pub fn list_by_run(
        &self,
        run_id: Uuid,
    ) -> Result<Vec<AutomationAgentJournalEntry>, SqlitePersistenceError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let mut statement = connection.prepare(
            "SELECT schema_version, payload FROM automation_agent_journal
             WHERE run_id = ?1 ORDER BY submitted_at_ms ASC, task_id ASC LIMIT ?2",
        )?;
        statement
            .query_map(
                params![
                    run_id.to_string(),
                    i64::try_from(MAX_TASKS_PER_RUN)
                        .map_err(|_| SqlitePersistenceError::InvalidAutomationAgentJournal)?
                ],
                |row| Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?)),
            )?
            .map(|row| {
                let (version, payload) = row?;
                decode_entry(version, &payload)
            })
            .collect()
    }

    fn get_where(
        &self,
        predicate: &str,
        first: &str,
        second: &str,
    ) -> Result<Option<AutomationAgentJournalEntry>, SqlitePersistenceError> {
        let sql = format!(
            "SELECT schema_version, payload FROM automation_agent_journal WHERE {predicate}"
        );
        let row = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .query_row(&sql, params![first, second], |row| {
                Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?))
            })
            .optional()?;
        row.map(|(version, payload)| decode_entry(version, &payload))
            .transpose()
    }

    /// Moves an active child task to a durable lifecycle state.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid transitions, timestamps, payloads, or storage failures.
    pub fn transition(
        &self,
        task_id: Uuid,
        status: AutomationAgentJournalStatus,
        updated_at_ms: u64,
        error: Option<&str>,
    ) -> Result<(), SqlitePersistenceError> {
        let mut entry = self
            .get_by_task_id(task_id)?
            .ok_or(SqlitePersistenceError::InvalidAutomationAgentJournal)?;
        if updated_at_ms < entry.updated_at_ms
            || error.is_some_and(|value| value.is_empty() || value.len() > MAX_ERROR_BYTES)
            || !valid_transition(entry.status, status)
        {
            return Err(SqlitePersistenceError::InvalidAutomationAgentJournal);
        }
        entry.status = status;
        entry.updated_at_ms = updated_at_ms;
        entry.error = error.map(ToOwned::to_owned);
        let payload = serde_json::to_string(&entry)?;
        let changed = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .execute(
                "UPDATE automation_agent_journal SET status = ?2, updated_at_ms = ?3, payload = ?4
                 WHERE task_id = ?1 AND status IN ('submitted', 'waiting_for_confirmation')",
                params![
                    task_id.to_string(),
                    status_name(status),
                    i64::try_from(updated_at_ms)
                        .map_err(|_| SqlitePersistenceError::InvalidAutomationAgentJournal)?,
                    payload,
                ],
            )?;
        if changed != 1 {
            return Err(SqlitePersistenceError::InvalidAutomationAgentJournal);
        }
        Ok(())
    }

    /// Marks crash-left active child tasks as interrupted.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid timestamps or storage failures.
    pub fn recover_active(&self, updated_at_ms: u64) -> Result<usize, SqlitePersistenceError> {
        let task_ids = {
            let connection = self
                .connection
                .lock()
                .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
            let mut statement = connection.prepare(
                "SELECT task_id FROM automation_agent_journal
                 WHERE status IN ('submitted', 'waiting_for_confirmation')",
            )?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        for task_id in &task_ids {
            self.transition(
                Uuid::parse_str(task_id)
                    .map_err(|_| SqlitePersistenceError::InvalidAutomationAgentJournal)?,
                AutomationAgentJournalStatus::Interrupted,
                updated_at_ms,
                Some("desktop process restarted"),
            )?;
        }
        Ok(task_ids.len())
    }
}

fn decode_entry(
    version: u32,
    payload: &str,
) -> Result<AutomationAgentJournalEntry, SqlitePersistenceError> {
    if version != AUTOMATION_AGENT_JOURNAL_VERSION {
        return Err(SqlitePersistenceError::UnsupportedAutomationAgentJournalVersion(version));
    }
    let entry = serde_json::from_str(payload)?;
    validate_entry(&entry)?;
    Ok(entry)
}

fn validate_entry(entry: &AutomationAgentJournalEntry) -> Result<(), SqlitePersistenceError> {
    let expected = AutomationAgentJournalEntry::new(
        entry.run_id,
        entry.idempotency_key.clone(),
        entry.admission.clone(),
        entry.model.clone(),
        entry.submitted_at_ms,
    )?;
    if entry.spec != expected.spec
        || entry.updated_at_ms < entry.submitted_at_ms
        || entry
            .error
            .as_ref()
            .is_some_and(|error| error.len() > MAX_ERROR_BYTES)
    {
        return Err(SqlitePersistenceError::InvalidAutomationAgentJournal);
    }
    Ok(())
}

const fn valid_transition(
    from: AutomationAgentJournalStatus,
    to: AutomationAgentJournalStatus,
) -> bool {
    matches!(
        (from, to),
        (
            AutomationAgentJournalStatus::Submitted,
            AutomationAgentJournalStatus::WaitingForConfirmation
                | AutomationAgentJournalStatus::Completed
                | AutomationAgentJournalStatus::Failed
                | AutomationAgentJournalStatus::Cancelled
                | AutomationAgentJournalStatus::Interrupted
        ) | (
            AutomationAgentJournalStatus::WaitingForConfirmation,
            AutomationAgentJournalStatus::Completed
                | AutomationAgentJournalStatus::Failed
                | AutomationAgentJournalStatus::Cancelled
                | AutomationAgentJournalStatus::Interrupted
        )
    )
}

const fn status_name(status: AutomationAgentJournalStatus) -> &'static str {
    match status {
        AutomationAgentJournalStatus::Submitted => "submitted",
        AutomationAgentJournalStatus::WaitingForConfirmation => "waiting_for_confirmation",
        AutomationAgentJournalStatus::Completed => "completed",
        AutomationAgentJournalStatus::Failed => "failed",
        AutomationAgentJournalStatus::Cancelled => "cancelled",
        AutomationAgentJournalStatus::Interrupted => "interrupted",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimora_agent_runtime::{
        AgentAutonomy, AgentBudget, AgentTaskGateway, AgentTaskGatewayPolicy, AgentTaskOrigin,
        AgentTaskParent, AgentTaskRequest, DataClassification,
    };
    use std::collections::BTreeSet;

    fn entry(run_id: Uuid, key: &str) -> AutomationAgentJournalEntry {
        let budget = AgentBudget {
            max_steps: 4,
            max_tool_calls: 1,
            max_elapsed_ms: 30_000,
            max_input_tokens: 1_000,
            max_output_tokens: 500,
            max_cost_microunits: 0,
        };
        let policy = AgentTaskGatewayPolicy::new(
            "automation:test",
            [AgentTaskOrigin::Automation],
            ["provider:test".to_owned()],
            ["runtime.health.read".to_owned()],
            DataClassification::Personal,
            AgentAutonomy::ConfirmEach,
            budget,
            2,
        )
        .expect("policy");
        let admission = AgentTaskGateway::new(policy)
            .admit(
                AgentTaskRequest {
                    spec: "nimora.agent-task-request/1".to_owned(),
                    origin: AgentTaskOrigin::Automation,
                    requester: "automation:test".to_owned(),
                    provider_id: "provider:test".to_owned(),
                    tool_allowlist: BTreeSet::from(["runtime.health.read".to_owned()]),
                    classification: DataClassification::Personal,
                    autonomy: AgentAutonomy::Draft,
                    budget,
                    parent: Some(AgentTaskParent {
                        root_task_id: run_id,
                        parent_task_id: run_id,
                        trace_id: Uuid::now_v7(),
                        call_depth: 0,
                        remaining_budget: budget,
                    }),
                },
                100,
            )
            .expect("admission");
        AutomationAgentJournalEntry::new(run_id, key, admission, "model:test", 100).expect("entry")
    }

    #[test]
    fn persists_run_scoped_idempotency_and_terminal_state() {
        let journal = SqliteAutomationAgentJournal::in_memory().expect("journal");
        let run_id = Uuid::now_v7();
        let entry = entry(run_id, "once");
        journal.submit(&entry).expect("submit");
        assert!(journal.submit(&entry).is_err());
        journal
            .transition(
                entry.admission.task.id,
                AutomationAgentJournalStatus::Completed,
                200,
                None,
            )
            .expect("complete");
        let stored = journal
            .get_by_key(run_id, "once")
            .expect("query")
            .expect("stored");
        assert_eq!(stored.status, AutomationAgentJournalStatus::Completed);
        assert!(
            journal
                .transition(
                    entry.admission.task.id,
                    AutomationAgentJournalStatus::Failed,
                    300,
                    Some("late failure"),
                )
                .is_err()
        );
    }

    #[test]
    fn startup_recovery_interrupts_submitted_and_waiting_tasks() {
        let journal = SqliteAutomationAgentJournal::in_memory().expect("journal");
        let submitted = entry(Uuid::now_v7(), "submitted");
        let waiting = entry(Uuid::now_v7(), "waiting");
        journal.submit(&submitted).expect("submit first");
        journal.submit(&waiting).expect("submit second");
        journal
            .transition(
                waiting.admission.task.id,
                AutomationAgentJournalStatus::WaitingForConfirmation,
                150,
                None,
            )
            .expect("waiting");
        assert_eq!(journal.recover_active(200).expect("recover"), 2);
        for task_id in [submitted.admission.task.id, waiting.admission.task.id] {
            assert_eq!(
                journal
                    .get_by_task_id(task_id)
                    .expect("query")
                    .expect("entry")
                    .status,
                AutomationAgentJournalStatus::Interrupted
            );
        }
    }

    #[test]
    fn lists_only_tasks_from_the_requested_run_in_submission_order() {
        let journal = SqliteAutomationAgentJournal::in_memory().expect("journal");
        let run_id = Uuid::now_v7();
        let first = entry(run_id, "first");
        let mut second = entry(run_id, "second");
        second.submitted_at_ms = 200;
        second.updated_at_ms = 200;
        let unrelated = entry(Uuid::now_v7(), "unrelated");
        journal.submit(&second).expect("submit second");
        journal.submit(&unrelated).expect("submit unrelated");
        journal.submit(&first).expect("submit first");

        let tasks = journal.list_by_run(run_id).expect("list run tasks");
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].idempotency_key, "first");
        assert_eq!(tasks[1].idempotency_key, "second");
    }
}
