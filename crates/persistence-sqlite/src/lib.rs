//! `SQLite` persistence adapters for the `Nimora` runtime.

mod agent_goal_store;
mod auto_mode_checkpoint_store;
mod auto_mode_commit_store;
mod auto_mode_store;
mod automation_agent_journal;
mod automation_journal;
mod backup;
mod context_cache_store;
mod skill_approval_journal;
mod skill_execution_history;
mod workspace_snapshot_store;

pub use agent_goal_store::{AgentGoalSnapshot, SqliteAgentGoalRepository};
pub use auto_mode_checkpoint_store::SqliteAutoModeCheckpointRepository;
pub use auto_mode_commit_store::SqliteAutoModeCommitRepository;
pub use auto_mode_store::SqliteAutoModeRepository;
pub use automation_agent_journal::{
    AutomationAgentJournalEntry, AutomationAgentJournalStatus, SqliteAutomationAgentJournal,
};
pub use automation_journal::{
    AutomationJournalEntry, AutomationJournalStatus, AutomationRunStart, SqliteAutomationJournal,
};
pub use backup::{
    BackupCoordinator, BackupHealth, BackupPolicy, BackupRecord, PendingRestore,
    apply_pending_restore,
};
pub use context_cache_store::{
    ContextCachePolicy, SqliteContextCacheRepository, StoredContextCacheEntry,
};
pub use skill_approval_journal::{
    SkillApprovalJournalEntry, SkillApprovalJournalStatus, SqliteSkillApprovalJournal,
};
pub use skill_execution_history::{
    SkillExecutionHistoryRecord, SkillExecutionHistoryStatus, SqliteSkillExecutionHistory,
};
pub use workspace_snapshot_store::{SqliteWorkspaceSnapshotRepository, StoredWorkspaceSnapshot};

use nimora_agent_runtime::{AgentTask, ProviderFinishReason, ProviderUsage};
use nimora_runtime_app::{
    PetRepository, ProfileRepository, ProfileServiceError, ProfileSnapshot, RepositoryError,
};
use nimora_runtime_core::{Event, Pet};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, backup::Backup, params};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Mutex, time::Duration};
use thiserror::Error;

pub const DATABASE_VERSION: i64 = 1;
const PET_SNAPSHOT_VERSION: u32 = 1;
const PROFILE_SNAPSHOT_VERSION: u32 = 1;
const MAX_OUTBOX_BATCH: usize = 256;
const MAX_OUTBOX_ERROR_BYTES: usize = 4 * 1024;
const AGENT_HISTORY_VERSION: u32 = 1;
const MAX_AGENT_HISTORY_CONTENT_BYTES: usize = 256 * 1024;
const MAX_AGENT_HISTORY_PAGE: usize = 200;

#[derive(Debug)]
pub struct SqlitePetRepository {
    connection: Mutex<Connection>,
}

#[derive(Debug)]
pub struct SqliteProgramPermissionRepository {
    connection: Mutex<Connection>,
}

#[derive(Debug)]
pub struct SqliteSkillStateRepository {
    connection: Mutex<Connection>,
}

#[derive(Debug)]
pub struct SqliteOutboxRepository {
    connection: Mutex<Connection>,
}

#[derive(Debug)]
pub struct SqliteAgentHistoryRepository {
    connection: Mutex<Connection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentHistoryRecord {
    pub spec: String,
    pub task: AgentTask,
    pub model: String,
    pub prompt: String,
    pub response: String,
    pub finish_reason: ProviderFinishReason,
    pub usage: ProviderUsage,
    pub completed_at_ms: u64,
}

impl AgentHistoryRecord {
    /// Creates one bounded, completed Agent history record.
    ///
    /// # Errors
    ///
    /// Returns an error for inconsistent task state, invalid model/content, or timestamps.
    pub fn new(
        task: AgentTask,
        model: impl Into<String>,
        prompt: impl Into<String>,
        response: impl Into<String>,
        finish_reason: ProviderFinishReason,
        usage: ProviderUsage,
        completed_at_ms: u64,
    ) -> Result<Self, SqlitePersistenceError> {
        let model = model.into();
        let prompt = prompt.into();
        let response = response.into();
        if task.status != nimora_agent_runtime::AgentTaskStatus::Succeeded
            || model.trim().is_empty()
            || model.len() > 128
            || prompt.is_empty()
            || prompt.len() > MAX_AGENT_HISTORY_CONTENT_BYTES
            || response.len() > MAX_AGENT_HISTORY_CONTENT_BYTES
            || completed_at_ms < task.created_at_ms
        {
            return Err(SqlitePersistenceError::InvalidAgentHistory);
        }
        Ok(Self {
            spec: "nimora.agent-history/1".to_owned(),
            task,
            model,
            prompt,
            response,
            finish_reason,
            usage,
            completed_at_ms,
        })
    }
}

impl SqliteAgentHistoryRepository {
    /// Opens Agent history in the shared application database.
    ///
    /// # Errors
    ///
    /// Returns an error when the database cannot be opened or initialized.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Creates isolated Agent history for tests.
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

    /// Inserts one completed record exactly once.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid records, duplicate tasks, or storage failures.
    pub fn insert(&self, record: &AgentHistoryRecord) -> Result<(), SqlitePersistenceError> {
        validate_agent_history(record)?;
        let payload = serde_json::to_string(record)?;
        let created_at_ms = i64::try_from(record.task.created_at_ms)
            .map_err(|_| SqlitePersistenceError::InvalidAgentHistory)?;
        let completed_at_ms = i64::try_from(record.completed_at_ms)
            .map_err(|_| SqlitePersistenceError::InvalidAgentHistory)?;
        self.connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .execute(
                "INSERT INTO agent_history
                    (task_id, trace_id, provider_id, created_at_ms, completed_at_ms, schema_version, payload)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    record.task.id.to_string(),
                    record.task.trace_id.to_string(),
                    record.task.provider_id,
                    created_at_ms,
                    completed_at_ms,
                    AGENT_HISTORY_VERSION,
                    payload
                ],
            )?;
        Ok(())
    }

    /// Lists newest records using a stable `(created_at_ms, task_id)` cursor.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid bounds or malformed persisted records.
    pub fn list(
        &self,
        before: Option<(u64, uuid::Uuid)>,
        limit: usize,
    ) -> Result<Vec<AgentHistoryRecord>, SqlitePersistenceError> {
        if limit == 0 || limit > MAX_AGENT_HISTORY_PAGE {
            return Err(SqlitePersistenceError::InvalidAgentHistory);
        }
        let limit =
            i64::try_from(limit).map_err(|_| SqlitePersistenceError::InvalidAgentHistory)?;
        let (before_ms, before_id) = before
            .map(|(created_at_ms, task_id)| {
                i64::try_from(created_at_ms)
                    .map(|value| (value, task_id.to_string()))
                    .map_err(|_| SqlitePersistenceError::InvalidAgentHistory)
            })
            .transpose()?
            .unwrap_or((i64::MAX, "~".to_owned()));
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let mut statement = connection.prepare(
            "SELECT schema_version, payload FROM agent_history
             WHERE created_at_ms < ?1 OR (created_at_ms = ?1 AND task_id < ?2)
             ORDER BY created_at_ms DESC, task_id DESC LIMIT ?3",
        )?;
        let rows = statement.query_map(params![before_ms, before_id, limit], |row| {
            Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.map(|row| {
            let (version, payload) = row?;
            if version != AGENT_HISTORY_VERSION {
                return Err(SqlitePersistenceError::UnsupportedAgentHistoryVersion(
                    version,
                ));
            }
            let record = serde_json::from_str(&payload)?;
            validate_agent_history(&record)?;
            Ok(record)
        })
        .collect()
    }

    /// Deletes one task history record without affecting runtime state.
    ///
    /// # Errors
    ///
    /// Returns an error when storage cannot be updated.
    pub fn delete(&self, task_id: uuid::Uuid) -> Result<bool, SqlitePersistenceError> {
        Ok(self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .execute(
                "DELETE FROM agent_history WHERE task_id = ?1",
                params![task_id.to_string()],
            )?
            == 1)
    }

    /// Deletes all Agent history and returns the number removed.
    ///
    /// # Errors
    ///
    /// Returns an error when storage cannot be updated.
    pub fn delete_all(&self) -> Result<u64, SqlitePersistenceError> {
        self.connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .execute("DELETE FROM agent_history", [])?
            .try_into()
            .map_err(|_| SqlitePersistenceError::InvalidAgentHistory)
    }
}

fn validate_agent_history(record: &AgentHistoryRecord) -> Result<(), SqlitePersistenceError> {
    if record.spec != "nimora.agent-history/1" {
        return Err(SqlitePersistenceError::InvalidAgentHistory);
    }
    AgentHistoryRecord::new(
        record.task.clone(),
        record.model.clone(),
        record.prompt.clone(),
        record.response.clone(),
        record.finish_reason,
        record.usage,
        record.completed_at_ms,
    )?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboxDelivery {
    pub event: Event,
    pub attempt: u32,
    pub lease_until_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboxSnapshot {
    pub pending: u64,
    pub leased: u64,
    pub delivered: u64,
    pub dead_letter: u64,
}

impl SqliteOutboxRepository {
    /// Opens the durable event outbox in the shared application database.
    ///
    /// # Errors
    ///
    /// Returns an error when the database cannot be opened or initialized.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Creates an isolated outbox for tests and ephemeral tools.
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

    /// Atomically leases an ordered batch. Expired leases become claimable again.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid bounds, malformed persisted events, or storage failures.
    pub fn claim(
        &self,
        consumer: &str,
        now_ms: i64,
        lease_ms: u64,
        limit: usize,
    ) -> Result<Vec<OutboxDelivery>, SqlitePersistenceError> {
        validate_consumer(consumer)?;
        if now_ms < 0
            || lease_ms == 0
            || lease_ms > i64::MAX as u64
            || limit == 0
            || limit > MAX_OUTBOX_BATCH
        {
            return Err(SqlitePersistenceError::InvalidOutboxRequest);
        }
        let lease_ms =
            i64::try_from(lease_ms).map_err(|_| SqlitePersistenceError::InvalidOutboxRequest)?;
        let limit =
            i64::try_from(limit).map_err(|_| SqlitePersistenceError::InvalidOutboxRequest)?;
        let lease_until_ms = now_ms
            .checked_add(lease_ms)
            .ok_or(SqlitePersistenceError::InvalidOutboxRequest)?;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let rows = {
            let mut statement = transaction.prepare(
                "SELECT event_id, payload, delivery_attempts
                 FROM event_outbox
                 WHERE (status = 'pending' AND available_at_ms <= ?1)
                    OR (status = 'leased' AND lease_until_ms <= ?1)
                 ORDER BY created_at, event_id
                 LIMIT ?2",
            )?;
            statement
                .query_map(params![now_ms, limit], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, u32>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?
        };
        let mut deliveries = Vec::with_capacity(rows.len());
        for (event_id, payload, attempts) in rows {
            let attempt = attempts
                .checked_add(1)
                .ok_or(SqlitePersistenceError::InvalidOutboxRequest)?;
            transaction.execute(
                "UPDATE event_outbox
                 SET status = 'leased', delivery_attempts = ?1, lease_owner = ?2,
                     lease_until_ms = ?3, last_error = NULL
                 WHERE event_id = ?4",
                params![attempt, consumer, lease_until_ms, event_id],
            )?;
            deliveries.push(OutboxDelivery {
                event: serde_json::from_str(&payload)?,
                attempt,
                lease_until_ms,
            });
        }
        transaction.commit()?;
        Ok(deliveries)
    }

    /// Acknowledges one delivery owned by the consumer.
    ///
    /// # Errors
    ///
    /// Returns an error when the lease is missing, owned by another consumer, or storage fails.
    pub fn acknowledge(
        &self,
        consumer: &str,
        event_id: &str,
        delivered_at_ms: i64,
    ) -> Result<(), SqlitePersistenceError> {
        validate_consumer(consumer)?;
        if event_id.trim().is_empty() || delivered_at_ms < 0 {
            return Err(SqlitePersistenceError::InvalidOutboxRequest);
        }
        let changed = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .execute(
                "UPDATE event_outbox SET status = 'delivered', delivered_at_ms = ?1,
                    lease_owner = NULL, lease_until_ms = NULL, last_error = NULL
                 WHERE event_id = ?2 AND status = 'leased' AND lease_owner = ?3
                   AND lease_until_ms > ?1",
                params![delivered_at_ms, event_id, consumer],
            )?;
        if changed != 1 {
            return Err(SqlitePersistenceError::OutboxLeaseNotOwned);
        }
        Ok(())
    }

    /// Releases a failed delivery for retry or moves it to the dead-letter state.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid policy, missing lease ownership, or storage failures.
    pub fn fail(
        &self,
        consumer: &str,
        event_id: &str,
        now_ms: i64,
        retry_after_ms: u64,
        max_attempts: u32,
        error: &str,
    ) -> Result<bool, SqlitePersistenceError> {
        validate_consumer(consumer)?;
        if event_id.trim().is_empty()
            || now_ms < 0
            || max_attempts == 0
            || error.is_empty()
            || error.len() > MAX_OUTBOX_ERROR_BYTES
            || retry_after_ms > i64::MAX as u64
        {
            return Err(SqlitePersistenceError::InvalidOutboxRequest);
        }
        let retry_after_ms = i64::try_from(retry_after_ms)
            .map_err(|_| SqlitePersistenceError::InvalidOutboxRequest)?;
        let available_at_ms = now_ms
            .checked_add(retry_after_ms)
            .ok_or(SqlitePersistenceError::InvalidOutboxRequest)?;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let attempts = transaction.query_row(
            "SELECT delivery_attempts FROM event_outbox WHERE event_id = ?1 AND status = 'leased' AND lease_owner = ?2 AND lease_until_ms > ?3",
            params![event_id, consumer, now_ms],
            |row| row.get::<_, u32>(0),
        ).optional()?.ok_or(SqlitePersistenceError::OutboxLeaseNotOwned)?;
        let dead_letter = attempts >= max_attempts;
        transaction.execute(
            "UPDATE event_outbox SET status = ?1, available_at_ms = ?2,
                lease_owner = NULL, lease_until_ms = NULL, last_error = ?3
             WHERE event_id = ?4",
            params![
                if dead_letter {
                    "dead-letter"
                } else {
                    "pending"
                },
                available_at_ms,
                error,
                event_id
            ],
        )?;
        transaction.commit()?;
        Ok(dead_letter)
    }

    /// Returns durable queue counts without exposing event payloads.
    ///
    /// # Errors
    ///
    /// Returns an error when the database cannot be queried.
    pub fn snapshot(&self) -> Result<OutboxSnapshot, SqlitePersistenceError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let count = |status: &str| {
            connection.query_row(
                "SELECT COUNT(*) FROM event_outbox WHERE status = ?1",
                params![status],
                |row| row.get::<_, i64>(0),
            )
        };
        Ok(OutboxSnapshot {
            pending: count("pending")?
                .try_into()
                .map_err(|_| SqlitePersistenceError::InvalidOutboxRequest)?,
            leased: count("leased")?
                .try_into()
                .map_err(|_| SqlitePersistenceError::InvalidOutboxRequest)?,
            delivered: count("delivered")?
                .try_into()
                .map_err(|_| SqlitePersistenceError::InvalidOutboxRequest)?,
            dead_letter: count("dead-letter")?
                .try_into()
                .map_err(|_| SqlitePersistenceError::InvalidOutboxRequest)?,
        })
    }

    /// Deletes an ordered bounded batch of acknowledged records older than the cutoff.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid bounds or storage failures.
    pub fn purge_delivered(
        &self,
        before_ms: i64,
        limit: usize,
    ) -> Result<usize, SqlitePersistenceError> {
        if before_ms < 0 || limit == 0 || limit > MAX_OUTBOX_BATCH {
            return Err(SqlitePersistenceError::InvalidOutboxRequest);
        }
        let limit =
            i64::try_from(limit).map_err(|_| SqlitePersistenceError::InvalidOutboxRequest)?;
        Ok(self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .execute(
                "DELETE FROM event_outbox WHERE event_id IN (
                SELECT event_id FROM event_outbox
                WHERE status = 'delivered' AND delivered_at_ms < ?1
                ORDER BY delivered_at_ms, event_id LIMIT ?2
             )",
                params![before_ms, limit],
            )?)
    }
}

fn validate_consumer(consumer: &str) -> Result<(), SqlitePersistenceError> {
    if consumer.is_empty()
        || consumer.len() > 128
        || !consumer
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(SqlitePersistenceError::InvalidOutboxRequest);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramPermissionGrant {
    pub program_id: String,
    pub version: String,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillStateRecord {
    pub skill_id: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub authorized: bool,
    pub enabled: bool,
}

impl SqliteSkillStateRepository {
    /// Opens the shared application database and initializes its schema.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot open, configure, or initialize the database.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Creates an isolated Skill state store for tests and ephemeral tools.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot configure or initialize the database.
    pub fn in_memory() -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut connection: Connection) -> Result<Self, SqlitePersistenceError> {
        prepare_connection(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Replaces the exact-version Skill grant and desired activation state.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid records or persistence failures.
    pub fn save(&self, record: &SkillStateRecord) -> Result<(), SqlitePersistenceError> {
        let capabilities = canonical_skill_capabilities(record)?;
        let payload = serde_json::to_string(&capabilities)?;
        self.connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .execute(
                "INSERT INTO skill_state
                    (skill_id, skill_version, capabilities, authorized, enabled)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(skill_id) DO UPDATE SET
                    skill_version = excluded.skill_version,
                    capabilities = excluded.capabilities,
                    authorized = excluded.authorized,
                    enabled = excluded.enabled,
                    updated_at = CURRENT_TIMESTAMP",
                params![
                    record.skill_id,
                    record.version,
                    payload,
                    record.authorized,
                    record.enabled
                ],
            )?;
        Ok(())
    }

    /// Loads the persisted state for one Skill identity.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed stored data or persistence failures.
    pub fn load(&self, skill_id: &str) -> Result<Option<SkillStateRecord>, SqlitePersistenceError> {
        validate_skill_identity(skill_id)?;
        let row = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .query_row(
                "SELECT skill_version, capabilities, authorized, enabled
                 FROM skill_state WHERE skill_id = ?1",
                params![skill_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, bool>(2)?,
                        row.get::<_, bool>(3)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(version, payload, authorized, enabled)| {
            let capabilities = serde_json::from_str::<Vec<String>>(&payload)?;
            let record = SkillStateRecord {
                skill_id: skill_id.to_owned(),
                version,
                capabilities,
                authorized,
                enabled,
            };
            canonical_skill_capabilities(&record)?;
            Ok(record)
        })
        .transpose()
    }

    /// Lists all persisted Skill states in stable identity order.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed stored data or persistence failures.
    pub fn list(&self) -> Result<Vec<SkillStateRecord>, SqlitePersistenceError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let mut statement = connection.prepare(
            "SELECT skill_id, skill_version, capabilities, authorized, enabled
             FROM skill_state ORDER BY skill_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, bool>(3)?,
                row.get::<_, bool>(4)?,
            ))
        })?;
        rows.map(|row| {
            let (skill_id, version, payload, authorized, enabled) = row?;
            let capabilities = serde_json::from_str::<Vec<String>>(&payload)?;
            let record = SkillStateRecord {
                skill_id,
                version,
                capabilities,
                authorized,
                enabled,
            };
            canonical_skill_capabilities(&record)?;
            Ok(record)
        })
        .collect()
    }

    /// Removes all persisted authorization and desired state for one Skill.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identities or persistence failures.
    pub fn remove(&self, skill_id: &str) -> Result<(), SqlitePersistenceError> {
        validate_skill_identity(skill_id)?;
        self.connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .execute(
                "DELETE FROM skill_state WHERE skill_id = ?1",
                params![skill_id],
            )?;
        Ok(())
    }
}

fn canonical_skill_capabilities(
    record: &SkillStateRecord,
) -> Result<Vec<String>, SqlitePersistenceError> {
    validate_skill_identity(&record.skill_id)?;
    if record.version.trim().is_empty()
        || record.version.len() > 128
        || (record.enabled && !record.authorized)
    {
        return Err(SqlitePersistenceError::InvalidSkillState);
    }
    let mut capabilities = record.capabilities.clone();
    if capabilities
        .iter()
        .any(|capability| capability.trim().is_empty() || capability.len() > 128)
    {
        return Err(SqlitePersistenceError::InvalidSkillState);
    }
    capabilities.sort_unstable();
    if capabilities.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(SqlitePersistenceError::InvalidSkillState);
    }
    Ok(capabilities)
}

fn validate_skill_identity(skill_id: &str) -> Result<(), SqlitePersistenceError> {
    if skill_id.is_empty()
        || skill_id.len() > 128
        || !skill_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(SqlitePersistenceError::InvalidSkillState);
    }
    Ok(())
}

impl SqliteProgramPermissionRepository {
    /// Opens the shared application database and initializes its schema.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot open, configure, or migrate the database.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Creates an isolated permission store for tests and ephemeral tools.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot configure or initialize the database.
    pub fn in_memory() -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut connection: Connection) -> Result<Self, SqlitePersistenceError> {
        prepare_connection(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Replaces the exact capability grant for one installed program version.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identities, duplicate capabilities, or persistence failures.
    pub fn grant(&self, grant: &ProgramPermissionGrant) -> Result<(), SqlitePersistenceError> {
        let capabilities = canonical_capabilities(grant)?;
        let payload = serde_json::to_string(&capabilities)?;
        self.connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .execute(
                "INSERT INTO user_program_permission_grant
                    (program_id, program_version, capabilities)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(program_id, program_version) DO UPDATE SET
                    capabilities = excluded.capabilities,
                    granted_at = CURRENT_TIMESTAMP",
                params![grant.program_id, grant.version, payload],
            )?;
        Ok(())
    }

    /// Returns whether one version has an exact grant for its full capability set.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid requests, malformed stored data, or persistence failures.
    pub fn is_granted(
        &self,
        grant: &ProgramPermissionGrant,
    ) -> Result<bool, SqlitePersistenceError> {
        let requested = canonical_capabilities(grant)?;
        if requested.is_empty() {
            return Ok(true);
        }
        let payload = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .query_row(
                "SELECT capabilities FROM user_program_permission_grant
                 WHERE program_id = ?1 AND program_version = ?2",
                params![grant.program_id, grant.version],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(payload) = payload else {
            return Ok(false);
        };
        let mut stored = serde_json::from_str::<Vec<String>>(&payload)?;
        stored.sort_unstable();
        stored.dedup();
        Ok(stored == requested)
    }

    /// Revokes every persisted grant for a program identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the identity is invalid or persistence fails.
    pub fn revoke_program(&self, program_id: &str) -> Result<(), SqlitePersistenceError> {
        if program_id.trim().is_empty() {
            return Err(SqlitePersistenceError::InvalidPermissionGrant);
        }
        self.connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .execute(
                "DELETE FROM user_program_permission_grant WHERE program_id = ?1",
                params![program_id],
            )?;
        Ok(())
    }
}

fn canonical_capabilities(
    grant: &ProgramPermissionGrant,
) -> Result<Vec<String>, SqlitePersistenceError> {
    if grant.program_id.trim().is_empty() || grant.version.trim().is_empty() {
        return Err(SqlitePersistenceError::InvalidPermissionGrant);
    }
    let mut capabilities = grant.capabilities.clone();
    if capabilities
        .iter()
        .any(|capability| capability.trim().is_empty())
    {
        return Err(SqlitePersistenceError::InvalidPermissionGrant);
    }
    capabilities.sort_unstable();
    let original_len = capabilities.len();
    capabilities.dedup();
    if capabilities.len() != original_len {
        return Err(SqlitePersistenceError::InvalidPermissionGrant);
    }
    Ok(capabilities)
}

impl SqlitePetRepository {
    /// Opens or creates an `Nimora` database and initializes its schema.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot open, configure, or initialize the
    /// database. A database from a newer application version is rejected.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Creates an isolated in-memory database for tests and ephemeral tools.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot configure or initialize the database.
    pub fn in_memory() -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    /// Creates a consistent online backup, including WAL-backed pages.
    ///
    /// # Errors
    ///
    /// Returns an error when the destination cannot be created or `SQLite`
    /// cannot complete the online backup.
    pub fn backup_to(&self, destination: impl AsRef<Path>) -> Result<(), SqlitePersistenceError> {
        backup_connection(&self.connection, destination)
    }

    fn from_connection(mut connection: Connection) -> Result<Self, SqlitePersistenceError> {
        prepare_connection(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn load_snapshot(&self) -> Result<Option<Pet>, SqlitePersistenceError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let row = connection
            .query_row(
                "SELECT schema_version, payload FROM pet_snapshot WHERE singleton = 1",
                [],
                |row| Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let Some((schema_version, payload)) = row else {
            return Ok(None);
        };
        if schema_version != PET_SNAPSHOT_VERSION {
            return Err(SqlitePersistenceError::UnsupportedPetSnapshotVersion(
                schema_version,
            ));
        }
        let snapshot: StoredPetSnapshot = serde_json::from_str(&payload)?;
        if snapshot.schema_version != schema_version {
            return Err(SqlitePersistenceError::SnapshotVersionMismatch);
        }
        snapshot.pet.validate()?;
        Ok(Some(snapshot.pet))
    }

    fn save_snapshot(&self, pet: &Pet) -> Result<(), SqlitePersistenceError> {
        pet.validate()?;
        let payload = serde_json::to_string(&StoredPetSnapshot {
            schema_version: PET_SNAPSHOT_VERSION,
            pet: pet.clone(),
        })?;
        save_singleton_snapshot(
            &self.connection,
            "pet_snapshot",
            PET_SNAPSHOT_VERSION,
            &payload,
            None,
        )
    }

    fn save_snapshot_with_event(
        &self,
        pet: &Pet,
        event: &Event,
    ) -> Result<(), SqlitePersistenceError> {
        pet.validate()?;
        let payload = serde_json::to_string(&StoredPetSnapshot {
            schema_version: PET_SNAPSHOT_VERSION,
            pet: pet.clone(),
        })?;
        save_singleton_snapshot(
            &self.connection,
            "pet_snapshot",
            PET_SNAPSHOT_VERSION,
            &payload,
            Some(event),
        )
    }
}

fn prepare_connection(connection: &mut Connection) -> Result<(), SqlitePersistenceError> {
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    let version = connection.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))?;
    if version != 0 && version != DATABASE_VERSION {
        return Err(SqlitePersistenceError::UnsupportedDatabaseVersion(version));
    }
    if version == 0 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "CREATE TABLE pet_snapshot (
                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                    schema_version INTEGER NOT NULL,
                    payload TEXT NOT NULL,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE TABLE profile_snapshot (
                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                    schema_version INTEGER NOT NULL,
                    payload TEXT NOT NULL,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE TABLE event_outbox (
                    event_id TEXT PRIMARY KEY,
                    event_type TEXT NOT NULL,
                    trace_id TEXT NOT NULL,
                    payload TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'pending'
                        CHECK (status IN ('pending', 'leased', 'delivered', 'dead-letter')),
                    delivery_attempts INTEGER NOT NULL DEFAULT 0 CHECK (delivery_attempts >= 0),
                    available_at_ms INTEGER NOT NULL DEFAULT 0 CHECK (available_at_ms >= 0),
                    lease_owner TEXT,
                    lease_until_ms INTEGER,
                    last_error TEXT,
                    delivered_at_ms INTEGER,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE INDEX event_outbox_created_at_idx
                    ON event_outbox(created_at, event_id);
                CREATE INDEX event_outbox_delivery_idx
                    ON event_outbox(status, available_at_ms, lease_until_ms, created_at, event_id);
                CREATE TABLE user_program_permission_grant (
                    program_id TEXT NOT NULL,
                    program_version TEXT NOT NULL,
                    capabilities TEXT NOT NULL,
                    granted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    PRIMARY KEY (program_id, program_version)
                );
                CREATE TABLE agent_history (
                    task_id TEXT PRIMARY KEY,
                    trace_id TEXT NOT NULL,
                    provider_id TEXT NOT NULL,
                    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
                    completed_at_ms INTEGER NOT NULL CHECK (completed_at_ms >= created_at_ms),
                    schema_version INTEGER NOT NULL,
                    payload TEXT NOT NULL
                );
                CREATE INDEX agent_history_created_idx
                    ON agent_history(created_at_ms DESC, task_id DESC);
                CREATE TABLE automation_run_journal (
                    run_id TEXT PRIMARY KEY,
                    automation_id TEXT NOT NULL,
                    trace_id TEXT NOT NULL,
                    event_id TEXT NOT NULL,
                    status TEXT NOT NULL CHECK (status IN ('running', 'completed', 'interrupted')),
                    started_at_ms INTEGER NOT NULL CHECK (started_at_ms >= 0),
                    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= started_at_ms),
                    schema_version INTEGER NOT NULL,
                    payload TEXT,
                    interruption_reason TEXT
                );
                CREATE INDEX automation_run_journal_updated_idx
                    ON automation_run_journal(updated_at_ms DESC, run_id DESC);
                CREATE TABLE automation_agent_journal (
                    task_id TEXT PRIMARY KEY,
                    run_id TEXT NOT NULL,
                    idempotency_key TEXT NOT NULL,
                    status TEXT NOT NULL CHECK (status IN ('submitted', 'waiting_for_confirmation',
                        'completed', 'failed', 'cancelled', 'interrupted')),
                    submitted_at_ms INTEGER NOT NULL CHECK (submitted_at_ms >= 0),
                    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= submitted_at_ms),
                    schema_version INTEGER NOT NULL,
                    payload TEXT NOT NULL,
                    UNIQUE (run_id, idempotency_key)
                );
                CREATE INDEX automation_agent_journal_updated_idx
                    ON automation_agent_journal(updated_at_ms DESC, task_id DESC);
                PRAGMA user_version = 1;",
        )?;
        transaction.commit()?;
    }
    ensure_current_schema_extensions(connection)?;
    Ok(())
}

fn ensure_current_schema_extensions(connection: &Connection) -> Result<(), SqlitePersistenceError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS automation_agent_journal (
            task_id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            idempotency_key TEXT NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('submitted', 'waiting_for_confirmation',
                'completed', 'failed', 'cancelled', 'interrupted')),
            submitted_at_ms INTEGER NOT NULL CHECK (submitted_at_ms >= 0),
            updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= submitted_at_ms),
            schema_version INTEGER NOT NULL,
            payload TEXT NOT NULL,
            UNIQUE (run_id, idempotency_key)
        );
        CREATE INDEX IF NOT EXISTS automation_agent_journal_updated_idx
            ON automation_agent_journal(updated_at_ms DESC, task_id DESC);
        CREATE TABLE IF NOT EXISTS skill_approval_journal (
            approval_id TEXT PRIMARY KEY,
            execution_id TEXT NOT NULL UNIQUE,
            skill_id TEXT NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('pending', 'executing', 'completed',
                'rejected', 'expired', 'failed', 'interrupted')),
            created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
            updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= created_at_ms),
            expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms > created_at_ms),
            schema_version INTEGER NOT NULL,
            payload TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS skill_approval_journal_status_idx
            ON skill_approval_journal(status, expires_at_ms, approval_id);
        CREATE TABLE IF NOT EXISTS skill_execution_history (
            execution_id TEXT PRIMARY KEY,
            skill_id TEXT NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('waiting-for-approval', 'completed', 'rejected', 'cancelled', 'failed')),
            created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
            updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= created_at_ms),
            schema_version INTEGER NOT NULL,
            payload TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS skill_execution_history_created_idx
            ON skill_execution_history(created_at_ms DESC, execution_id DESC);
        CREATE TABLE IF NOT EXISTS skill_state (
            skill_id TEXT PRIMARY KEY,
            skill_version TEXT NOT NULL,
            capabilities TEXT NOT NULL,
            authorized INTEGER NOT NULL CHECK (authorized IN (0, 1)),
            enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS agent_goal (
            goal_id TEXT PRIMARY KEY,
            status TEXT NOT NULL CHECK (status IN ('active', 'paused', 'completed', 'cancelled')),
            current_plan_revision INTEGER NOT NULL CHECK (current_plan_revision > 0),
            created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
            updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= created_at_ms),
            schema_version INTEGER NOT NULL,
            payload TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS agent_goal_updated_idx
            ON agent_goal(updated_at_ms DESC, goal_id DESC);
        CREATE TABLE IF NOT EXISTS agent_goal_plan (
            goal_id TEXT NOT NULL REFERENCES agent_goal(goal_id) ON DELETE CASCADE,
            revision INTEGER NOT NULL CHECK (revision > 0),
            created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
            schema_version INTEGER NOT NULL,
            payload TEXT NOT NULL,
            PRIMARY KEY (goal_id, revision)
        );
        CREATE TABLE IF NOT EXISTS auto_mode_session (
            session_id TEXT PRIMARY KEY,
            goal_id TEXT NOT NULL REFERENCES agent_goal(goal_id) ON DELETE CASCADE,
            plan_revision INTEGER NOT NULL CHECK (plan_revision > 0),
            status TEXT NOT NULL CHECK (status IN ('running', 'paused', 'completed', 'cancelled')),
            pause_reason TEXT,
            created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
            updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= created_at_ms),
            schema_version INTEGER NOT NULL,
            payload TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS auto_mode_session_goal_idx
            ON auto_mode_session(goal_id, updated_at_ms DESC, session_id DESC);
        CREATE UNIQUE INDEX IF NOT EXISTS auto_mode_one_running_per_goal_idx
            ON auto_mode_session(goal_id) WHERE status = 'running';
        CREATE TABLE IF NOT EXISTS auto_mode_checkpoint (
            session_id TEXT PRIMARY KEY,
            goal_id TEXT NOT NULL,
            plan_revision INTEGER NOT NULL CHECK (plan_revision > 0),
            sequence INTEGER NOT NULL CHECK (sequence > 0),
            task_id TEXT NOT NULL,
            updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= 0),
            schema_version INTEGER NOT NULL,
            payload TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS auto_mode_checkpoint_goal_idx
            ON auto_mode_checkpoint(goal_id, updated_at_ms DESC, session_id DESC);",
    )?;
    ensure_workspace_snapshot_schema(connection)?;
    ensure_context_cache_schema(connection)?;
    Ok(())
}

fn ensure_workspace_snapshot_schema(connection: &Connection) -> Result<(), SqlitePersistenceError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS agent_workspace_snapshot (
            session_id TEXT NOT NULL,
            revision INTEGER NOT NULL CHECK (revision > 0),
            root_fingerprint TEXT NOT NULL,
            snapshot_fingerprint TEXT NOT NULL UNIQUE,
            created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
            schema_version INTEGER NOT NULL,
            payload TEXT NOT NULL,
            PRIMARY KEY (session_id, revision)
        );
        CREATE INDEX IF NOT EXISTS agent_workspace_snapshot_latest_idx
            ON agent_workspace_snapshot(session_id, revision DESC);",
    )?;
    Ok(())
}

fn ensure_context_cache_schema(connection: &Connection) -> Result<(), SqlitePersistenceError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS agent_context_cache (
            cache_key TEXT PRIMARY KEY,
            provider_id TEXT NOT NULL,
            model TEXT NOT NULL,
            workspace_fingerprint TEXT NOT NULL,
            plan_revision INTEGER NOT NULL CHECK (plan_revision > 0),
            data_classification TEXT NOT NULL,
            created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
            expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms > created_at_ms),
            last_accessed_at_ms INTEGER NOT NULL CHECK (last_accessed_at_ms >= created_at_ms),
            payload_bytes INTEGER NOT NULL CHECK (payload_bytes > 0),
            schema_version INTEGER NOT NULL,
            payload TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS agent_context_cache_lru_idx
            ON agent_context_cache(last_accessed_at_ms, cache_key);
        CREATE INDEX IF NOT EXISTS agent_context_cache_workspace_idx
            ON agent_context_cache(workspace_fingerprint, cache_key);
        CREATE INDEX IF NOT EXISTS agent_context_cache_expiry_idx
            ON agent_context_cache(expires_at_ms, cache_key);",
    )?;
    Ok(())
}

fn save_singleton_snapshot(
    connection: &Mutex<Connection>,
    table: &str,
    schema_version: u32,
    payload: &str,
    event: Option<&Event>,
) -> Result<(), SqlitePersistenceError> {
    let mut connection = connection
        .lock()
        .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
    let transaction = connection.transaction()?;
    let statement = format!(
        "INSERT INTO {table} (singleton, schema_version, payload)
         VALUES (1, ?1, ?2)
         ON CONFLICT(singleton) DO UPDATE SET
           schema_version = excluded.schema_version,
           payload = excluded.payload,
           updated_at = CURRENT_TIMESTAMP"
    );
    transaction.execute(&statement, params![schema_version, payload])?;
    if let Some(event) = event {
        insert_outbox_event(&transaction, event)?;
    }
    transaction.commit()?;
    Ok(())
}

fn insert_outbox_event(
    transaction: &rusqlite::Transaction<'_>,
    event: &Event,
) -> Result<(), SqlitePersistenceError> {
    transaction.execute(
        "INSERT INTO event_outbox (event_id, event_type, trace_id, payload)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            event.id.to_string(),
            event.event_type,
            event.trace_id.to_string(),
            serde_json::to_string(event)?,
        ],
    )?;
    Ok(())
}

fn backup_connection(
    source: &Mutex<Connection>,
    destination: impl AsRef<Path>,
) -> Result<(), SqlitePersistenceError> {
    let source = source
        .lock()
        .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
    let mut destination = Connection::open(destination)?;
    let backup = Backup::new(&source, &mut destination)?;
    backup.run_to_completion(128, Duration::from_millis(10), None)?;
    Ok(())
}

#[derive(Debug)]
pub struct SqliteProfileRepository {
    connection: Mutex<Connection>,
}

impl SqliteProfileRepository {
    /// Opens or creates the shared runtime database for profile storage.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` configuration or initialization fails.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        let mut connection = Connection::open(path)?;
        prepare_connection(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Creates an isolated profile store for recovery mode and tests.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot configure the in-memory database.
    pub fn in_memory() -> Result<Self, SqlitePersistenceError> {
        let mut connection = Connection::open_in_memory()?;
        prepare_connection(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Creates a consistent online backup, including WAL-backed pages.
    ///
    /// # Errors
    ///
    /// Returns an error when the destination cannot be created or `SQLite`
    /// cannot complete the online backup.
    pub fn backup_to(&self, destination: impl AsRef<Path>) -> Result<(), SqlitePersistenceError> {
        backup_connection(&self.connection, destination)
    }

    fn load_snapshot(&self) -> Result<Option<ProfileSnapshot>, SqlitePersistenceError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let row = connection
            .query_row(
                "SELECT schema_version, payload FROM profile_snapshot WHERE singleton = 1",
                [],
                |row| Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let Some((schema_version, payload)) = row else {
            return Ok(None);
        };
        if schema_version != PROFILE_SNAPSHOT_VERSION {
            return Err(SqlitePersistenceError::UnsupportedProfileSnapshotVersion(
                schema_version,
            ));
        }
        let snapshot: ProfileSnapshot = serde_json::from_str(&payload)?;
        if snapshot.schema_version != schema_version {
            return Err(SqlitePersistenceError::SnapshotVersionMismatch);
        }
        snapshot.validate()?;
        Ok(Some(snapshot))
    }

    fn save_snapshot(&self, snapshot: &ProfileSnapshot) -> Result<(), SqlitePersistenceError> {
        snapshot.validate()?;
        let payload = serde_json::to_string(snapshot)?;
        save_singleton_snapshot(
            &self.connection,
            "profile_snapshot",
            PROFILE_SNAPSHOT_VERSION,
            &payload,
            None,
        )
    }

    fn save_snapshot_with_event(
        &self,
        snapshot: &ProfileSnapshot,
        event: &Event,
    ) -> Result<(), SqlitePersistenceError> {
        snapshot.validate()?;
        let payload = serde_json::to_string(snapshot)?;
        save_singleton_snapshot(
            &self.connection,
            "profile_snapshot",
            PROFILE_SNAPSHOT_VERSION,
            &payload,
            Some(event),
        )
    }
}

impl PetRepository for SqlitePetRepository {
    fn load(&self) -> Result<Option<Pet>, RepositoryError> {
        self.load_snapshot()
            .map_err(|error| RepositoryError::new(error.to_string()))
    }

    fn save(&self, pet: &Pet) -> Result<(), RepositoryError> {
        self.save_snapshot(pet)
            .map_err(|error| RepositoryError::new(error.to_string()))
    }

    fn save_with_event(&self, pet: &Pet, event: &Event) -> Result<(), RepositoryError> {
        self.save_snapshot_with_event(pet, event)
            .map_err(|error| RepositoryError::new(error.to_string()))
    }
}

impl ProfileRepository for SqliteProfileRepository {
    fn load(&self) -> Result<Option<ProfileSnapshot>, RepositoryError> {
        self.load_snapshot()
            .map_err(|error| RepositoryError::new(error.to_string()))
    }

    fn save(&self, snapshot: &ProfileSnapshot) -> Result<(), RepositoryError> {
        self.save_snapshot(snapshot)
            .map_err(|error| RepositoryError::new(error.to_string()))
    }

    fn save_with_event(
        &self,
        snapshot: &ProfileSnapshot,
        event: &Event,
    ) -> Result<(), RepositoryError> {
        self.save_snapshot_with_event(snapshot, event)
            .map_err(|error| RepositoryError::new(error.to_string()))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredPetSnapshot {
    schema_version: u32,
    pet: Pet,
}

#[derive(Debug, Error)]
pub enum SqlitePersistenceError {
    #[error("SQLite state lock is unavailable")]
    StatePoisoned,
    #[error("database version {0} is newer than this application supports")]
    UnsupportedDatabaseVersion(i64),
    #[error("pet snapshot version {0} is unsupported")]
    UnsupportedPetSnapshotVersion(u32),
    #[error("profile snapshot version {0} is unsupported")]
    UnsupportedProfileSnapshotVersion(u32),
    #[error("user program permission grant is invalid")]
    InvalidPermissionGrant,
    #[error("Skill persisted state is invalid")]
    InvalidSkillState,
    #[error("outbox request is invalid")]
    InvalidOutboxRequest,
    #[error("Agent history record or request is invalid")]
    InvalidAgentHistory,
    #[error("Agent Goal record or request is invalid")]
    InvalidAgentGoal,
    #[error("Agent Goal was not found")]
    AgentGoalNotFound,
    #[error("Agent Goal was changed by another writer")]
    AgentGoalConflict,
    #[error("Auto Mode session record or request is invalid")]
    InvalidAutoModeSession,
    #[error("Auto Mode session was not found")]
    AutoModeSessionNotFound,
    #[error("Auto Mode session was changed by another writer")]
    AutoModeSessionConflict,
    #[error("Auto Mode checkpoint record or request is invalid")]
    InvalidAutoModeCheckpoint,
    #[error("Auto Mode checkpoint was changed by another writer")]
    AutoModeCheckpointConflict,
    #[error("Auto Mode session or checkpoint was changed by another writer")]
    AutoModeCommitConflict,
    #[error("Workspace snapshot record or request is invalid")]
    InvalidWorkspaceSnapshot,
    #[error("Workspace snapshot was changed by another writer")]
    WorkspaceSnapshotConflict,
    #[error("Context cache record or request is invalid")]
    InvalidContextCache,
    #[error("Context cache schema version {0} is unsupported")]
    UnsupportedContextCacheVersion(u32),
    #[error("Automation journal record or state transition is invalid")]
    InvalidAutomationJournal,
    #[error("Automation Agent journal record or state transition is invalid")]
    InvalidAutomationAgentJournal,
    #[error("Skill approval journal record or state transition is invalid")]
    InvalidSkillApprovalJournal,
    #[error("Skill execution history record or request is invalid")]
    InvalidSkillExecutionHistory,
    #[error("Skill approval is missing, claimed, expired, or already resolved")]
    SkillApprovalNotPending,
    #[error("Skill approval expired")]
    SkillApprovalExpired,
    #[error("Automation journal version {0} is unsupported")]
    UnsupportedAutomationJournalVersion(u32),
    #[error("Automation Agent journal version {0} is unsupported")]
    UnsupportedAutomationAgentJournalVersion(u32),
    #[error("Skill approval journal version {0} is unsupported")]
    UnsupportedSkillApprovalJournalVersion(u32),
    #[error("Skill execution history version {0} is unsupported")]
    UnsupportedSkillExecutionHistoryVersion(u32),
    #[error("Agent history version {0} is unsupported")]
    UnsupportedAgentHistoryVersion(u32),
    #[error("Agent Goal version {0} is unsupported")]
    UnsupportedAgentGoalVersion(u32),
    #[error("Auto Mode session version {0} is unsupported")]
    UnsupportedAutoModeSessionVersion(u32),
    #[error("Auto Mode checkpoint version {0} is unsupported")]
    UnsupportedAutoModeCheckpointVersion(u32),
    #[error("Workspace snapshot version {0} is unsupported")]
    UnsupportedWorkspaceSnapshotVersion(u32),
    #[error("outbox lease is not owned by this consumer")]
    OutboxLeaseNotOwned,
    #[error("backup or restore request is invalid")]
    InvalidBackupRequest,
    #[error("pet snapshot metadata and payload versions do not match")]
    SnapshotVersionMismatch,
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Pet(#[from] nimora_runtime_core::PetError),
    #[error(transparent)]
    Profile(#[from] ProfileServiceError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    SystemClock(#[from] std::time::SystemTimeError),
}

/// Verifies an existing database against the current schema and `SQLite` integrity checks.
///
/// # Errors
///
/// Returns an error when the database cannot be opened, is unsupported, or is corrupt.
pub fn verify_database_file(path: &Path) -> Result<(), SqlitePersistenceError> {
    let mut connection = Connection::open(path)?;
    prepare_connection(&mut connection)?;
    let integrity: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(SqlitePersistenceError::InvalidBackupRequest);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimora_runtime_app::{ProfileService, RuntimeEventBus, RuntimeService};
    use nimora_runtime_core::{Event, EventSource, PetAction, PetState};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn round_trips_a_versioned_pet_snapshot() {
        let repository = SqlitePetRepository::in_memory().expect("database");
        let mut pet = Pet::new("Aster").expect("pet");
        pet.apply_action(PetAction::Sleep);
        repository.save_snapshot(&pet).expect("save");
        let restored = repository.load_snapshot().expect("load").expect("snapshot");
        assert_eq!(restored.id, pet.id);
        assert_eq!(restored.state, PetState::Sleeping);
    }

    #[test]
    fn rejects_future_snapshot_versions() {
        let repository = SqlitePetRepository::in_memory().expect("database");
        repository.connection.lock().expect("lock").execute(
            "INSERT INTO pet_snapshot (singleton, schema_version, payload) VALUES (1, 99, '{}')",
            [],
        ).expect("fixture");
        assert!(matches!(
            repository.load_snapshot(),
            Err(SqlitePersistenceError::UnsupportedPetSnapshotVersion(99))
        ));
    }

    #[test]
    fn initializes_schema_once() {
        let repository = SqlitePetRepository::in_memory().expect("database");
        let version = repository
            .connection
            .lock()
            .expect("lock")
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .expect("version");
        assert_eq!(version, DATABASE_VERSION);
    }

    #[test]
    fn rejects_a_corrupt_database_without_rewriting_it() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("nimora-corrupt-database-{nonce}"));
        let database = root.join("runtime.sqlite3");
        std::fs::create_dir_all(&root).expect("fixture directory");
        let corrupt_bytes = b"not a sqlite database";
        std::fs::write(&database, corrupt_bytes).expect("corrupt fixture");

        assert!(verify_database_file(&database).is_err());
        assert_eq!(
            std::fs::read(&database).expect("preserved fixture"),
            corrupt_bytes
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn permission_grants_are_bound_to_program_version_and_exact_capabilities() {
        let repository = SqliteProgramPermissionRepository::in_memory().expect("database");
        let grant = ProgramPermissionGrant {
            program_id: "studio.example.focus".to_owned(),
            version: "1.0.0".to_owned(),
            capabilities: vec![
                "invoke-safe-commands".to_owned(),
                "read-pet-state".to_owned(),
            ],
        };
        assert!(!repository.is_granted(&grant).expect("check"));
        repository.grant(&grant).expect("grant");
        assert!(repository.is_granted(&grant).expect("check"));
        assert!(
            !repository
                .is_granted(&ProgramPermissionGrant {
                    version: "2.0.0".to_owned(),
                    ..grant.clone()
                })
                .expect("version check")
        );
        assert!(
            !repository
                .is_granted(&ProgramPermissionGrant {
                    capabilities: vec!["read-pet-state".to_owned()],
                    ..grant
                })
                .expect("capability check")
        );
    }

    #[test]
    fn revoking_a_program_removes_all_version_grants() {
        let repository = SqliteProgramPermissionRepository::in_memory().expect("database");
        for version in ["1.0.0", "2.0.0"] {
            repository
                .grant(&ProgramPermissionGrant {
                    program_id: "studio.example.focus".to_owned(),
                    version: version.to_owned(),
                    capabilities: vec!["read-pet-state".to_owned()],
                })
                .expect("grant");
        }
        repository
            .revoke_program("studio.example.focus")
            .expect("revoke");
        assert!(
            !repository
                .is_granted(&ProgramPermissionGrant {
                    program_id: "studio.example.focus".to_owned(),
                    version: "1.0.0".to_owned(),
                    capabilities: vec!["read-pet-state".to_owned()],
                })
                .expect("check")
        );
    }

    #[test]
    fn skill_state_is_exact_versioned_and_stably_listed() {
        let repository = SqliteSkillStateRepository::in_memory().expect("database");
        let first = SkillStateRecord {
            skill_id: "studio.example.focus".to_owned(),
            version: "1.0.0".to_owned(),
            capabilities: vec!["invoke-commands".to_owned()],
            authorized: true,
            enabled: true,
        };
        repository.save(&first).expect("save");
        repository
            .save(&SkillStateRecord {
                skill_id: "studio.example.clock".to_owned(),
                version: "2.0.0".to_owned(),
                capabilities: Vec::new(),
                authorized: false,
                enabled: false,
            })
            .expect("save");
        assert_eq!(repository.load(&first.skill_id).expect("load"), Some(first));
        let records = repository.list().expect("list");
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].skill_id, "studio.example.clock");
        assert_eq!(records[1].skill_id, "studio.example.focus");
    }

    #[test]
    fn skill_upgrade_replaces_grant_and_remove_revokes_state() {
        let repository = SqliteSkillStateRepository::in_memory().expect("database");
        for version in ["1.0.0", "2.0.0"] {
            repository
                .save(&SkillStateRecord {
                    skill_id: "studio.example.focus".to_owned(),
                    version: version.to_owned(),
                    capabilities: vec!["invoke-commands".to_owned()],
                    authorized: true,
                    enabled: version == "2.0.0",
                })
                .expect("save");
        }
        let current = repository
            .load("studio.example.focus")
            .expect("load")
            .expect("record");
        assert_eq!(current.version, "2.0.0");
        assert!(current.enabled);
        repository.remove("studio.example.focus").expect("remove");
        assert!(
            repository
                .load("studio.example.focus")
                .expect("load")
                .is_none()
        );
    }

    #[test]
    fn skill_state_rejects_duplicate_capabilities() {
        let repository = SqliteSkillStateRepository::in_memory().expect("database");
        assert!(matches!(
            repository.save(&SkillStateRecord {
                skill_id: "studio.example.focus".to_owned(),
                version: "1.0.0".to_owned(),
                capabilities: vec!["invoke-commands".to_owned(), "invoke-commands".to_owned()],
                authorized: true,
                enabled: true,
            }),
            Err(SqlitePersistenceError::InvalidSkillState)
        ));
    }

    #[test]
    fn atomically_persists_pet_state_and_event() {
        let repository = SqlitePetRepository::in_memory().expect("database");
        let mut pet = Pet::new("Aster").expect("pet");
        pet.apply_action(PetAction::Sleep);
        let event = Event::new(
            "pet.state.changed",
            EventSource::Core,
            serde_json::json!({ "state": "sleeping" }),
        )
        .expect("event");

        repository.save_with_event(&pet, &event).expect("save");

        let connection = repository.connection.lock().expect("lock");
        let (event_type, trace_id, payload): (String, String, String) = connection
            .query_row(
                "SELECT event_type, trace_id, payload FROM event_outbox WHERE event_id = ?1",
                params![event.id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("outbox row");
        let stored_event: Event = serde_json::from_str(&payload).expect("event payload");
        assert_eq!(event_type, event.event_type);
        assert_eq!(trace_id, event.trace_id.to_string());
        assert_eq!(stored_event, event);
    }

    #[test]
    fn duplicate_event_rolls_back_snapshot_update() {
        let repository = SqlitePetRepository::in_memory().expect("database");
        let original = Pet::new("Aster").expect("pet");
        let event = Event::new(
            "pet.state.changed",
            EventSource::Core,
            serde_json::json!({ "state": "idle" }),
        )
        .expect("event");
        repository
            .save_with_event(&original, &event)
            .expect("initial save");
        let mut candidate = original.clone();
        candidate.apply_action(PetAction::Sleep);

        assert!(repository.save_with_event(&candidate, &event).is_err());
        assert_eq!(
            repository
                .load_snapshot()
                .expect("load")
                .expect("snapshot")
                .state,
            PetState::Idle
        );
    }

    #[test]
    fn restores_state_after_runtime_restart() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "nimora-persistence-{}-{unique}.sqlite3",
            std::process::id()
        ));
        {
            let repository = SqlitePetRepository::open(&path).expect("database");
            let service = RuntimeService::initialize(repository, "Aster").expect("runtime");
            service
                .play_action(PetAction::Sleep)
                .expect("persisted action");
        }
        {
            let repository = SqlitePetRepository::open(&path).expect("database");
            let service = RuntimeService::initialize(repository, "Ignored").expect("runtime");
            assert_eq!(
                service.snapshot().expect("snapshot").state,
                PetState::Sleeping
            );
        }
        std::fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn online_backup_restores_wal_backed_runtime_state() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let source = std::env::temp_dir().join(format!(
            "nimora-backup-source-{}-{unique}.sqlite3",
            std::process::id()
        ));
        let destination = std::env::temp_dir().join(format!(
            "nimora-backup-destination-{}-{unique}.sqlite3",
            std::process::id()
        ));
        let mut pet = Pet::new("Aster").expect("pet");
        pet.apply_action(PetAction::Sleep);
        {
            let repository = SqlitePetRepository::open(&source).expect("source");
            repository.save(&pet).expect("save");
            repository.backup_to(&destination).expect("backup");
        }
        let restored = SqlitePetRepository::open(&destination)
            .expect("destination")
            .load_snapshot()
            .expect("load")
            .expect("snapshot");
        assert_eq!(restored.state, PetState::Sleeping);
        std::fs::remove_file(source).expect("remove source");
        std::fs::remove_file(destination).expect("remove destination");
    }

    #[test]
    fn restores_profiles_after_runtime_restart() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "nimora-profiles-{}-{unique}.sqlite3",
            std::process::id()
        ));
        let focus_id = {
            let repository = SqliteProfileRepository::open(&path).expect("database");
            let service = ProfileService::initialize(repository, RuntimeEventBus::default())
                .expect("profiles");
            service
                .create_profile("Focus", nimora_runtime_core::ProfilePolicy::standard())
                .expect("create");
            let snapshot = service.snapshot().expect("snapshot");
            let focus_id = snapshot
                .profiles
                .iter()
                .find(|profile| profile.name == "Focus")
                .expect("focus profile")
                .id;
            service.switch_active(focus_id).expect("activate");
            focus_id
        };
        {
            let repository = SqliteProfileRepository::open(&path).expect("database");
            let service = ProfileService::initialize(repository, RuntimeEventBus::default())
                .expect("profiles");
            let snapshot = service.snapshot().expect("snapshot");
            assert_eq!(snapshot.profiles.len(), 2);
            assert_eq!(snapshot.active_profile_id, focus_id);
        }
        std::fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn profile_changes_append_deserializable_outbox_events() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "nimora-profile-outbox-{}-{unique}.sqlite3",
            std::process::id()
        ));
        let repository = SqliteProfileRepository::open(&path).expect("database");
        let service =
            ProfileService::initialize(repository, RuntimeEventBus::default()).expect("profiles");
        service
            .create_profile("Focus", nimora_runtime_core::ProfilePolicy::standard())
            .expect("create");
        let snapshot = service.snapshot().expect("snapshot");
        let focus_id = snapshot
            .profiles
            .iter()
            .find(|profile| profile.name == "Focus")
            .expect("focus profile")
            .id;
        service.switch_active(focus_id).expect("activate");
        drop(service);

        let connection = Connection::open(&path).expect("database");
        let mut statement = connection
            .prepare("SELECT payload FROM event_outbox ORDER BY created_at, event_id")
            .expect("statement");
        let events = statement
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query")
            .map(|payload| {
                serde_json::from_str::<Event>(&payload.expect("payload")).expect("event")
            })
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 2);
        assert!(
            events
                .iter()
                .any(|event| event.event_type == "profile.collection.created")
        );
        assert!(
            events
                .iter()
                .any(|event| event.event_type == "profile.active.changed")
        );
        drop(statement);
        drop(connection);
        std::fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn rejects_unpublished_database_versions() {
        let connection = Connection::open_in_memory().expect("database");
        connection
            .pragma_update(None, "user_version", 2)
            .expect("fixture version");
        assert!(matches!(
            SqlitePetRepository::from_connection(connection),
            Err(SqlitePersistenceError::UnsupportedDatabaseVersion(2))
        ));
    }

    fn enqueue_outbox(repository: &SqliteOutboxRepository, event: &Event) {
        repository
            .connection
            .lock()
            .expect("lock")
            .execute(
                "INSERT INTO event_outbox (event_id, event_type, trace_id, payload)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    event.id.to_string(),
                    event.event_type,
                    event.trace_id.to_string(),
                    serde_json::to_string(event).expect("payload"),
                ],
            )
            .expect("enqueue");
    }

    fn outbox_event(state: &str) -> Event {
        Event::new(
            "pet.state.changed",
            EventSource::Core,
            serde_json::json!({ "state": state }),
        )
        .expect("event")
    }

    #[test]
    fn outbox_claim_ack_and_purge_are_bounded_and_owned() {
        let repository = SqliteOutboxRepository::in_memory().expect("database");
        let first = outbox_event("idle");
        let second = outbox_event("sleeping");
        enqueue_outbox(&repository, &first);
        enqueue_outbox(&repository, &second);

        let deliveries = repository
            .claim("connector.audit", 1_000, 500, 1)
            .expect("claim");
        assert_eq!(deliveries.len(), 1);
        assert_eq!(deliveries[0].attempt, 1);
        assert!(matches!(
            repository.acknowledge(
                "connector.other",
                &deliveries[0].event.id.to_string(),
                1_100
            ),
            Err(SqlitePersistenceError::OutboxLeaseNotOwned)
        ));
        repository
            .acknowledge(
                "connector.audit",
                &deliveries[0].event.id.to_string(),
                1_100,
            )
            .expect("acknowledge");
        assert_eq!(
            repository.snapshot().expect("snapshot"),
            OutboxSnapshot {
                pending: 1,
                leased: 0,
                delivered: 1,
                dead_letter: 0,
            }
        );
        assert_eq!(repository.purge_delivered(1_101, 1).expect("purge"), 1);
        assert_eq!(repository.snapshot().expect("snapshot").delivered, 0);
    }

    #[test]
    fn outbox_expired_lease_is_reclaimed_without_stale_ack() {
        let repository = SqliteOutboxRepository::in_memory().expect("database");
        let event = outbox_event("working");
        enqueue_outbox(&repository, &event);

        repository
            .claim("connector.first", 2_000, 100, 1)
            .expect("first claim");
        assert!(matches!(
            repository.acknowledge("connector.first", &event.id.to_string(), 2_100),
            Err(SqlitePersistenceError::OutboxLeaseNotOwned)
        ));
        let reclaimed = repository
            .claim("connector.second", 2_100, 100, 1)
            .expect("reclaim");
        assert_eq!(reclaimed[0].attempt, 2);
        assert!(matches!(
            repository.acknowledge("connector.first", &event.id.to_string(), 2_150),
            Err(SqlitePersistenceError::OutboxLeaseNotOwned)
        ));
        repository
            .acknowledge("connector.second", &event.id.to_string(), 2_150)
            .expect("ack");
    }

    #[test]
    fn outbox_failure_retries_after_delay_then_dead_letters() {
        let repository = SqliteOutboxRepository::in_memory().expect("database");
        let event = outbox_event("sleeping");
        enqueue_outbox(&repository, &event);

        repository
            .claim("connector.audit", 3_000, 500, 1)
            .expect("claim");
        assert!(
            !repository
                .fail(
                    "connector.audit",
                    &event.id.to_string(),
                    3_100,
                    250,
                    2,
                    "temporary"
                )
                .expect("retry")
        );
        assert!(
            repository
                .claim("connector.audit", 3_349, 500, 1)
                .expect("early claim")
                .is_empty()
        );
        let retry = repository
            .claim("connector.audit", 3_350, 500, 1)
            .expect("retry claim");
        assert_eq!(retry[0].attempt, 2);
        assert!(
            repository
                .fail(
                    "connector.audit",
                    &event.id.to_string(),
                    3_400,
                    250,
                    2,
                    "permanent"
                )
                .expect("dead letter")
        );
        assert_eq!(
            repository.snapshot().expect("snapshot"),
            OutboxSnapshot {
                pending: 0,
                leased: 0,
                delivered: 0,
                dead_letter: 1,
            }
        );
        assert!(
            repository
                .claim("connector.audit", 4_000, 500, 1)
                .expect("claim")
                .is_empty()
        );
    }

    #[test]
    fn outbox_rejects_unbounded_or_ambiguous_requests() {
        let repository = SqliteOutboxRepository::in_memory().expect("database");
        assert!(matches!(
            repository.claim("", 0, 1, 1),
            Err(SqlitePersistenceError::InvalidOutboxRequest)
        ));
        assert!(matches!(
            repository.claim("connector/audit", 0, 1, 1),
            Err(SqlitePersistenceError::InvalidOutboxRequest)
        ));
        assert!(matches!(
            repository.claim("connector.audit", 0, 1, MAX_OUTBOX_BATCH + 1),
            Err(SqlitePersistenceError::InvalidOutboxRequest)
        ));
        assert!(matches!(
            repository.purge_delivered(0, 0),
            Err(SqlitePersistenceError::InvalidOutboxRequest)
        ));
    }

    fn agent_history_record(created_at_ms: u64, prompt: &str) -> AgentHistoryRecord {
        let mut task = AgentTask::new(
            nimora_agent_runtime::AgentTaskOrigin::Desktop,
            "desktop:test-user",
            "provider:deterministic-local",
            nimora_agent_runtime::AgentBudget::default(),
            created_at_ms,
        )
        .expect("task");
        task.transition(
            nimora_agent_runtime::AgentTaskStatus::Planning,
            created_at_ms,
        )
        .expect("planning");
        task.transition(
            nimora_agent_runtime::AgentTaskStatus::Succeeded,
            created_at_ms + 1,
        )
        .expect("succeeded");
        AgentHistoryRecord::new(
            task,
            "model:echo-v1",
            prompt,
            format!("response:{prompt}"),
            ProviderFinishReason::Completed,
            ProviderUsage {
                input_tokens: 3,
                output_tokens: 2,
                cost_microunits: 0,
            },
            created_at_ms + 1,
        )
        .expect("history record")
    }

    #[test]
    fn agent_history_round_trips_with_stable_cursor_pagination() {
        let repository = SqliteAgentHistoryRepository::in_memory().expect("database");
        let oldest = agent_history_record(100, "oldest");
        let middle = agent_history_record(200, "middle");
        let newest = agent_history_record(300, "newest");
        for record in [&oldest, &middle, &newest] {
            repository.insert(record).expect("insert history");
        }

        let first_page = repository.list(None, 2).expect("first page");
        assert_eq!(first_page, [newest.clone(), middle.clone()]);
        let cursor = (first_page[1].task.created_at_ms, first_page[1].task.id);
        assert_eq!(
            repository.list(Some(cursor), 2).expect("next page"),
            [oldest]
        );
    }

    #[test]
    fn agent_history_is_insert_once_and_privacy_deletable() {
        let repository = SqliteAgentHistoryRepository::in_memory().expect("database");
        let first = agent_history_record(100, "private prompt");
        let second = agent_history_record(200, "another prompt");
        repository.insert(&first).expect("insert first");
        repository.insert(&second).expect("insert second");
        assert!(repository.insert(&first).is_err());
        assert!(repository.delete(first.task.id).expect("delete one"));
        assert!(!repository.delete(first.task.id).expect("delete missing"));
        assert_eq!(repository.delete_all().expect("delete all"), 1);
        assert!(repository.list(None, 10).expect("empty history").is_empty());
    }

    #[test]
    fn agent_history_rejects_unbounded_content_and_queries() {
        let repository = SqliteAgentHistoryRepository::in_memory().expect("database");
        assert!(matches!(
            repository.list(None, MAX_AGENT_HISTORY_PAGE + 1),
            Err(SqlitePersistenceError::InvalidAgentHistory)
        ));
        let valid = agent_history_record(100, "prompt");
        assert!(matches!(
            AgentHistoryRecord::new(
                valid.task,
                "model:echo-v1",
                "x".repeat(MAX_AGENT_HISTORY_CONTENT_BYTES + 1),
                "response",
                ProviderFinishReason::Completed,
                valid.usage,
                valid.completed_at_ms,
            ),
            Err(SqlitePersistenceError::InvalidAgentHistory)
        ));
    }
}
