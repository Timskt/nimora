use crate::{AutoModeTurnAttempt, AutoModeTurnAttemptStatus, StoredWorkspaceSnapshot};
use crate::{SqlitePersistenceError, prepare_connection};
use nimora_agent_runtime::{
    AgentTaskStatus, AutoModeCheckpoint, AutoModePauseReason, AutoModeSession, AutoModeStatus,
};
use rusqlite::{Connection, TransactionBehavior, params};
use std::path::Path;

#[derive(Debug)]
pub struct SqliteAutoModeCommitRepository {
    connection: Connection,
}

impl SqliteAutoModeCommitRepository {
    /// Opens the shared database used for atomic Auto Mode state commits.
    ///
    /// # Errors
    ///
    /// Returns an error when the database cannot be opened or validated.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        let mut connection = Connection::open(path)?;
        prepare_connection(&mut connection)?;
        Ok(Self { connection })
    }

    /// Atomically resumes a session and advances its checkpoint with dual optimistic concurrency.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid state or when either expected persisted version is stale.
    pub fn commit_resume(
        &mut self,
        session: &AutoModeSession,
        previous_session_updated_at_ms: u64,
        checkpoint: &AutoModeCheckpoint,
        previous_checkpoint_sequence: u64,
    ) -> Result<(), SqlitePersistenceError> {
        session
            .validate()
            .map_err(|_| SqlitePersistenceError::InvalidAutoModeSession)?;
        checkpoint
            .validate()
            .map_err(|_| SqlitePersistenceError::InvalidAutoModeCheckpoint)?;
        if session.status != AutoModeStatus::Running
            || checkpoint.task.status != AgentTaskStatus::Running
            || checkpoint.sequence != previous_checkpoint_sequence.saturating_add(1)
            || !checkpoint.matches_bindings(
                session.id,
                session.goal_id,
                session.plan_revision,
                &session.policy.workspace_revision,
                &session.policy_fingerprint,
            )
        {
            return Err(SqlitePersistenceError::InvalidAutoModeCheckpoint);
        }

        let session_payload = serde_json::to_string(session)?;
        let checkpoint_payload = serde_json::to_string(checkpoint)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let session_changed = transaction.execute(
            "UPDATE auto_mode_session SET status = 'running', pause_reason = NULL,
                updated_at_ms = ?1, payload = ?2
             WHERE session_id = ?3 AND goal_id = ?4 AND plan_revision = ?5
                AND created_at_ms = ?6 AND updated_at_ms = ?7 AND status = 'paused'",
            params![
                to_i64(session.updated_at_ms)?,
                session_payload,
                session.id.to_string(),
                session.goal_id.to_string(),
                to_i64(session.plan_revision)?,
                to_i64(session.created_at_ms)?,
                to_i64(previous_session_updated_at_ms)?,
            ],
        )?;
        let checkpoint_changed = transaction.execute(
            "UPDATE auto_mode_checkpoint SET sequence = ?1, task_id = ?2, updated_at_ms = ?3,
                payload = ?4
             WHERE session_id = ?5 AND goal_id = ?6 AND plan_revision = ?7 AND sequence = ?8",
            params![
                to_i64(checkpoint.sequence)?,
                checkpoint.task.id.to_string(),
                to_i64(checkpoint.updated_at_ms)?,
                checkpoint_payload,
                checkpoint.session_id.to_string(),
                checkpoint.goal_id.to_string(),
                to_i64(checkpoint.plan_revision)?,
                to_i64(previous_checkpoint_sequence)?,
            ],
        )?;
        if session_changed != 1 || checkpoint_changed != 1 {
            return Err(SqlitePersistenceError::AutoModeCommitConflict);
        }
        transaction.commit()?;
        Ok(())
    }

    /// Atomically pauses a drifting turn and appends its observed Workspace successor.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid bindings or when any expected version is stale.
    #[allow(clippy::too_many_arguments)]
    pub fn commit_workspace_drift(
        &mut self,
        session: &AutoModeSession,
        previous_session_updated_at_ms: u64,
        checkpoint: &AutoModeCheckpoint,
        previous_checkpoint_sequence: u64,
        workspace: &StoredWorkspaceSnapshot,
        previous_workspace_revision: u64,
        previous_workspace_fingerprint: &str,
    ) -> Result<(), SqlitePersistenceError> {
        session
            .validate()
            .map_err(|_| SqlitePersistenceError::InvalidAutoModeSession)?;
        checkpoint
            .validate()
            .map_err(|_| SqlitePersistenceError::InvalidAutoModeCheckpoint)?;
        workspace
            .snapshot
            .validate()
            .map_err(|_| SqlitePersistenceError::InvalidWorkspaceSnapshot)?;
        if session.status != AutoModeStatus::Paused
            || session.pause_reason != Some(AutoModePauseReason::WorkspaceChanged)
            || checkpoint.task.status != AgentTaskStatus::Paused
            || checkpoint.sequence != previous_checkpoint_sequence.saturating_add(1)
            || workspace.session_id != session.id
            || workspace.snapshot.revision != previous_workspace_revision.saturating_add(1)
            || workspace.snapshot.parent_fingerprint.as_deref()
                != Some(previous_workspace_fingerprint)
        {
            return Err(SqlitePersistenceError::InvalidWorkspaceSnapshot);
        }
        let session_payload = serde_json::to_string(session)?;
        let checkpoint_payload = serde_json::to_string(checkpoint)?;
        let workspace_payload = serde_json::to_string(workspace)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let session_changed = transaction.execute(
            "UPDATE auto_mode_session SET status = 'paused', pause_reason = 'workspace_changed',
                updated_at_ms = ?1, payload = ?2
             WHERE session_id = ?3 AND updated_at_ms = ?4 AND status = 'running'",
            params![
                to_i64(session.updated_at_ms)?,
                session_payload,
                session.id.to_string(),
                to_i64(previous_session_updated_at_ms)?,
            ],
        )?;
        let checkpoint_changed = transaction.execute(
            "UPDATE auto_mode_checkpoint SET sequence = ?1, task_id = ?2, updated_at_ms = ?3,
                payload = ?4 WHERE session_id = ?5 AND sequence = ?6",
            params![
                to_i64(checkpoint.sequence)?,
                checkpoint.task.id.to_string(),
                to_i64(checkpoint.updated_at_ms)?,
                checkpoint_payload,
                checkpoint.session_id.to_string(),
                to_i64(previous_checkpoint_sequence)?,
            ],
        )?;
        let workspace_changed = transaction.execute(
            "INSERT INTO agent_workspace_snapshot (
                session_id, revision, root_fingerprint, snapshot_fingerprint, created_at_ms,
                schema_version, payload
             ) SELECT ?1, ?2, ?3, ?4, ?5, 1, ?6
             WHERE EXISTS (
                SELECT 1 FROM agent_workspace_snapshot
                WHERE session_id = ?1 AND revision = ?7 AND snapshot_fingerprint = ?8
                    AND root_fingerprint = ?3
             ) AND NOT EXISTS (
                SELECT 1 FROM agent_workspace_snapshot WHERE session_id = ?1 AND revision > ?7
             )",
            params![
                workspace.session_id.to_string(),
                to_i64(workspace.snapshot.revision)?,
                workspace.root_fingerprint,
                workspace.snapshot.fingerprint,
                to_i64(workspace.snapshot.created_at_ms)?,
                workspace_payload,
                to_i64(previous_workspace_revision)?,
                previous_workspace_fingerprint,
            ],
        )?;
        if session_changed != 1 || checkpoint_changed != 1 || workspace_changed != 1 {
            return Err(SqlitePersistenceError::AutoModeCommitConflict);
        }
        transaction.commit()?;
        Ok(())
    }

    /// Atomically commits one completed, continuing, or paused Auto Mode turn.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid lifecycle coupling or stale session/checkpoint versions.
    pub fn commit_turn(
        &mut self,
        attempt: &AutoModeTurnAttempt,
        session: &AutoModeSession,
        previous_session_updated_at_ms: u64,
        checkpoint: &AutoModeCheckpoint,
        previous_checkpoint_sequence: u64,
    ) -> Result<(), SqlitePersistenceError> {
        session
            .validate()
            .map_err(|_| SqlitePersistenceError::InvalidAutoModeSession)?;
        checkpoint
            .validate()
            .map_err(|_| SqlitePersistenceError::InvalidAutoModeCheckpoint)?;
        let coupled = matches!(
            (session.status, checkpoint.task.status),
            (AutoModeStatus::Running, AgentTaskStatus::Running)
                | (AutoModeStatus::Paused, AgentTaskStatus::Paused)
                | (AutoModeStatus::Completed, AgentTaskStatus::Succeeded)
        );
        if attempt.status != AutoModeTurnAttemptStatus::Active
            || attempt.session_id != session.id
            || attempt.checkpoint_sequence != previous_checkpoint_sequence
            || attempt.expected_session_updated_at_ms != previous_session_updated_at_ms
            || !coupled
            || checkpoint.sequence != previous_checkpoint_sequence.saturating_add(1)
            || !checkpoint.matches_bindings(
                session.id,
                session.goal_id,
                session.plan_revision,
                &session.policy.workspace_revision,
                &session.policy_fingerprint,
            )
        {
            return Err(SqlitePersistenceError::InvalidAutoModeCheckpoint);
        }

        let session_payload = serde_json::to_string(session)?;
        let checkpoint_payload = serde_json::to_string(checkpoint)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let session_changed = transaction.execute(
            "UPDATE auto_mode_session SET status = ?1, pause_reason = ?2, updated_at_ms = ?3,
                payload = ?4 WHERE session_id = ?5 AND goal_id = ?6 AND plan_revision = ?7
                AND created_at_ms = ?8 AND updated_at_ms = ?9 AND status = 'running'",
            params![
                status_name(session.status),
                session.pause_reason.map(pause_reason_name),
                to_i64(session.updated_at_ms)?,
                session_payload,
                session.id.to_string(),
                session.goal_id.to_string(),
                to_i64(session.plan_revision)?,
                to_i64(session.created_at_ms)?,
                to_i64(previous_session_updated_at_ms)?,
            ],
        )?;
        let checkpoint_changed = transaction.execute(
            "UPDATE auto_mode_checkpoint SET sequence = ?1, task_id = ?2, updated_at_ms = ?3,
                payload = ?4 WHERE session_id = ?5 AND goal_id = ?6 AND plan_revision = ?7
                AND sequence = ?8",
            params![
                to_i64(checkpoint.sequence)?,
                checkpoint.task.id.to_string(),
                to_i64(checkpoint.updated_at_ms)?,
                checkpoint_payload,
                checkpoint.session_id.to_string(),
                checkpoint.goal_id.to_string(),
                to_i64(checkpoint.plan_revision)?,
                to_i64(previous_checkpoint_sequence)?,
            ],
        )?;
        let attempt_changed = transaction.execute(
            "DELETE FROM auto_mode_turn_attempt WHERE session_id = ?1 AND attempt_id = ?2
                AND checkpoint_sequence = ?3 AND expected_session_updated_at_ms = ?4
                AND request_fingerprint = ?5 AND status = 'active'",
            params![
                attempt.session_id.to_string(),
                attempt.id.to_string(),
                to_i64(attempt.checkpoint_sequence)?,
                to_i64(attempt.expected_session_updated_at_ms)?,
                attempt.request_fingerprint,
            ],
        )?;
        if session_changed != 1 || checkpoint_changed != 1 || attempt_changed != 1 {
            return Err(SqlitePersistenceError::AutoModeCommitConflict);
        }
        transaction.commit()?;
        Ok(())
    }

    /// Atomically applies a host-requested pause or cancellation at a clean batch boundary.
    ///
    /// This transition deliberately has no Turn Attempt because no Provider or Tool work is in
    /// flight at a yielded boundary.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid lifecycle coupling or stale session/checkpoint versions.
    pub fn commit_host_control(
        &mut self,
        session: &AutoModeSession,
        previous_session_updated_at_ms: u64,
        checkpoint: &AutoModeCheckpoint,
        previous_checkpoint_sequence: u64,
    ) -> Result<(), SqlitePersistenceError> {
        session
            .validate()
            .map_err(|_| SqlitePersistenceError::InvalidAutoModeSession)?;
        checkpoint
            .validate()
            .map_err(|_| SqlitePersistenceError::InvalidAutoModeCheckpoint)?;
        let coupled = matches!(
            (session.status, checkpoint.task.status),
            (AutoModeStatus::Paused, AgentTaskStatus::Paused)
                | (AutoModeStatus::Cancelled, AgentTaskStatus::Cancelled)
        );
        if !coupled
            || checkpoint.sequence != previous_checkpoint_sequence.saturating_add(1)
            || !checkpoint.matches_bindings(
                session.id,
                session.goal_id,
                session.plan_revision,
                &session.policy.workspace_revision,
                &session.policy_fingerprint,
            )
        {
            return Err(SqlitePersistenceError::InvalidAutoModeCheckpoint);
        }

        let session_payload = serde_json::to_string(session)?;
        let checkpoint_payload = serde_json::to_string(checkpoint)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let session_changed = transaction.execute(
            "UPDATE auto_mode_session SET status = ?1, pause_reason = ?2, updated_at_ms = ?3,
                payload = ?4 WHERE session_id = ?5 AND goal_id = ?6 AND plan_revision = ?7
                AND created_at_ms = ?8 AND updated_at_ms = ?9 AND status = 'running'",
            params![
                status_name(session.status),
                session.pause_reason.map(pause_reason_name),
                to_i64(session.updated_at_ms)?,
                session_payload,
                session.id.to_string(),
                session.goal_id.to_string(),
                to_i64(session.plan_revision)?,
                to_i64(session.created_at_ms)?,
                to_i64(previous_session_updated_at_ms)?,
            ],
        )?;
        let checkpoint_changed = transaction.execute(
            "UPDATE auto_mode_checkpoint SET sequence = ?1, task_id = ?2, updated_at_ms = ?3,
                payload = ?4 WHERE session_id = ?5 AND goal_id = ?6 AND plan_revision = ?7
                AND sequence = ?8",
            params![
                to_i64(checkpoint.sequence)?,
                checkpoint.task.id.to_string(),
                to_i64(checkpoint.updated_at_ms)?,
                checkpoint_payload,
                checkpoint.session_id.to_string(),
                checkpoint.goal_id.to_string(),
                to_i64(checkpoint.plan_revision)?,
                to_i64(previous_checkpoint_sequence)?,
            ],
        )?;
        if session_changed != 1 || checkpoint_changed != 1 {
            return Err(SqlitePersistenceError::AutoModeCommitConflict);
        }
        transaction.commit()?;
        Ok(())
    }
}

const fn status_name(status: AutoModeStatus) -> &'static str {
    match status {
        AutoModeStatus::Running => "running",
        AutoModeStatus::Paused => "paused",
        AutoModeStatus::Completed => "completed",
        AutoModeStatus::Cancelled => "cancelled",
    }
}

const fn pause_reason_name(reason: AutoModePauseReason) -> &'static str {
    match reason {
        AutoModePauseReason::ConfirmationRequired => "confirmation_required",
        AutoModePauseReason::BudgetExhausted => "budget_exhausted",
        AutoModePauseReason::GoalChanged => "goal_changed",
        AutoModePauseReason::WorkspaceChanged => "workspace_changed",
        AutoModePauseReason::ProviderUnavailable => "provider_unavailable",
        AutoModePauseReason::Restarted => "restarted",
        AutoModePauseReason::UnsafeEffect => "unsafe_effect",
        AutoModePauseReason::UserRequested => "user_requested",
    }
}

fn to_i64(value: u64) -> Result<i64, SqlitePersistenceError> {
    i64::try_from(value).map_err(|_| SqlitePersistenceError::InvalidAutoModeCheckpoint)
}
