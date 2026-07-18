use crate::{
    AutoModeTurnAttempt, AutoModeTurnAttemptStatus, SqlitePersistenceError, prepare_connection,
};
use nimora_agent_runtime::{
    AgentTaskStatus, AutoModeCheckpoint, AutoModePauseReason, AutoModeSession, AutoModeStatus,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

const RESOLUTION_VERSION: u32 = 1;
const MAX_ACTOR_BYTES: usize = 128;
const MAX_REASON_BYTES: usize = 2 * 1024;
const MAX_RESOLUTION_PAGE: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoModeAttemptResolutionDecision {
    ConfirmedNotExecuted,
    AcceptExternalEffectAndCancel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolveAutoModeAttemptRequest {
    pub session_id: Uuid,
    pub attempt_id: Uuid,
    pub checkpoint_sequence: u64,
    pub request_fingerprint: String,
    pub decision: AutoModeAttemptResolutionDecision,
    pub actor: String,
    pub reason: Option<String>,
    pub resolved_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutoModeAttemptResolution {
    pub spec: String,
    pub id: Uuid,
    pub session_id: Uuid,
    pub attempt_id: Uuid,
    pub checkpoint_sequence: u64,
    pub request_fingerprint: String,
    pub decision: AutoModeAttemptResolutionDecision,
    pub actor: String,
    pub reason: Option<String>,
    pub resolved_at_ms: u64,
}

#[derive(Debug)]
pub struct SqliteAutoModeAttemptResolutionRepository {
    connection: Connection,
}

impl SqliteAutoModeAttemptResolutionRepository {
    /// Opens the immutable manual-resolution audit store.
    ///
    /// # Errors
    ///
    /// Returns an error when the database cannot be opened or validated.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        let mut connection = Connection::open(path)?;
        prepare_connection(&mut connection)?;
        Ok(Self { connection })
    }

    /// Atomically resolves one indeterminate attempt and converges session and checkpoint state.
    ///
    /// This never retries Provider work and never fabricates a successful Provider result.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid input, stale bindings, replay, or corrupt persisted state.
    pub fn resolve(
        &mut self,
        request: &ResolveAutoModeAttemptRequest,
    ) -> Result<AutoModeAttemptResolution, SqlitePersistenceError> {
        validate_request(request)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let attempt = load_attempt(&transaction, request.session_id)?
            .ok_or(SqlitePersistenceError::AutoModeAttemptResolutionConflict)?;
        if attempt.status != AutoModeTurnAttemptStatus::Indeterminate
            || attempt.id != request.attempt_id
            || attempt.checkpoint_sequence != request.checkpoint_sequence
            || attempt.request_fingerprint != request.request_fingerprint
        {
            return Err(SqlitePersistenceError::AutoModeAttemptResolutionConflict);
        }
        let mut session = load_session(&transaction, request.session_id)?;
        let previous_session_updated_at_ms = session.updated_at_ms;
        let mut checkpoint = load_checkpoint(&transaction, request.session_id)?;
        if checkpoint.sequence != request.checkpoint_sequence
            || request.resolved_at_ms < attempt.updated_at_ms
            || request.resolved_at_ms < session.updated_at_ms
            || request.resolved_at_ms < checkpoint.updated_at_ms
            || !checkpoint.matches_bindings(
                session.id,
                session.goal_id,
                session.plan_revision,
                &session.policy.workspace_revision,
                &session.policy_fingerprint,
            )
        {
            return Err(SqlitePersistenceError::AutoModeAttemptResolutionConflict);
        }

        converge_state(
            &mut session,
            &mut checkpoint,
            request.decision,
            request.resolved_at_ms,
        )?;
        checkpoint.sequence = checkpoint.sequence.saturating_add(1);
        checkpoint.updated_at_ms = request.resolved_at_ms;
        checkpoint
            .validate()
            .map_err(|_| SqlitePersistenceError::InvalidAutoModeAttemptResolution)?;
        session
            .validate()
            .map_err(|_| SqlitePersistenceError::InvalidAutoModeAttemptResolution)?;

        let resolution = AutoModeAttemptResolution {
            spec: "nimora.auto-mode-attempt-resolution/1".to_owned(),
            id: Uuid::now_v7(),
            session_id: request.session_id,
            attempt_id: request.attempt_id,
            checkpoint_sequence: request.checkpoint_sequence,
            request_fingerprint: request.request_fingerprint.clone(),
            decision: request.decision,
            actor: request.actor.clone(),
            reason: request.reason.clone(),
            resolved_at_ms: request.resolved_at_ms,
        };
        persist_resolution(
            &transaction,
            request,
            &resolution,
            &session,
            previous_session_updated_at_ms,
            &checkpoint,
        )?;
        transaction.commit()?;
        Ok(resolution)
    }

    /// Lists immutable resolutions for one session, newest first.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid bounds or corrupt records.
    pub fn list_for_session(
        &self,
        session_id: Uuid,
        limit: usize,
    ) -> Result<Vec<AutoModeAttemptResolution>, SqlitePersistenceError> {
        if limit == 0 || limit > MAX_RESOLUTION_PAGE {
            return Err(SqlitePersistenceError::InvalidAutoModeAttemptResolution);
        }
        let mut statement = self.connection.prepare(
            "SELECT schema_version, payload FROM auto_mode_attempt_resolution
             WHERE session_id = ?1 ORDER BY resolved_at_ms DESC, resolution_id DESC LIMIT ?2",
        )?;
        let rows = statement.query_map(
            params![
                session_id.to_string(),
                i64::try_from(limit)
                    .map_err(|_| SqlitePersistenceError::InvalidAutoModeAttemptResolution)?
            ],
            |row| Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?)),
        )?;
        rows.map(|row| {
            let (version, payload) = row?;
            if version != RESOLUTION_VERSION {
                return Err(SqlitePersistenceError::InvalidAutoModeAttemptResolution);
            }
            let resolution: AutoModeAttemptResolution = serde_json::from_str(&payload)?;
            validate_resolution(&resolution)?;
            Ok(resolution)
        })
        .collect()
    }
}

fn persist_resolution(
    transaction: &Transaction<'_>,
    request: &ResolveAutoModeAttemptRequest,
    resolution: &AutoModeAttemptResolution,
    session: &AutoModeSession,
    previous_session_updated_at_ms: u64,
    checkpoint: &AutoModeCheckpoint,
) -> Result<(), SqlitePersistenceError> {
    let session_payload = serde_json::to_string(session)?;
    let checkpoint_payload = serde_json::to_string(checkpoint)?;
    let resolution_payload = serde_json::to_string(resolution)?;
    let session_changed = transaction.execute(
        "UPDATE auto_mode_session SET status = ?1, pause_reason = ?2, updated_at_ms = ?3,
                payload = ?4 WHERE session_id = ?5 AND updated_at_ms = ?6",
        params![
            status_name(session.status),
            session.pause_reason.map(pause_reason_name),
            to_i64(session.updated_at_ms)?,
            session_payload,
            session.id.to_string(),
            to_i64(previous_session_updated_at_ms)?
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
            to_i64(request.checkpoint_sequence)?
        ],
    )?;
    let resolution_changed = transaction.execute(
        "INSERT OR IGNORE INTO auto_mode_attempt_resolution (
                resolution_id, session_id, attempt_id, checkpoint_sequence, request_fingerprint,
                decision, actor, reason, resolved_at_ms, schema_version, payload
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            resolution.id.to_string(),
            resolution.session_id.to_string(),
            resolution.attempt_id.to_string(),
            to_i64(resolution.checkpoint_sequence)?,
            resolution.request_fingerprint,
            decision_name(resolution.decision),
            resolution.actor,
            resolution.reason,
            to_i64(resolution.resolved_at_ms)?,
            RESOLUTION_VERSION,
            resolution_payload
        ],
    )?;
    let attempt_changed = transaction.execute(
            "DELETE FROM auto_mode_turn_attempt WHERE session_id = ?1 AND attempt_id = ?2
                AND checkpoint_sequence = ?3 AND request_fingerprint = ?4 AND status = 'indeterminate'",
            params![request.session_id.to_string(), request.attempt_id.to_string(),
                to_i64(request.checkpoint_sequence)?, request.request_fingerprint],
        )?;
    if session_changed != 1
        || checkpoint_changed != 1
        || resolution_changed != 1
        || attempt_changed != 1
    {
        return Err(SqlitePersistenceError::AutoModeAttemptResolutionConflict);
    }
    Ok(())
}

fn converge_state(
    session: &mut AutoModeSession,
    checkpoint: &mut AutoModeCheckpoint,
    decision: AutoModeAttemptResolutionDecision,
    now_ms: u64,
) -> Result<(), SqlitePersistenceError> {
    match decision {
        AutoModeAttemptResolutionDecision::ConfirmedNotExecuted => {
            match session.status {
                AutoModeStatus::Running => {
                    session.pause(AutoModePauseReason::UserRequested, now_ms)
                }
                AutoModeStatus::Paused => Ok(()),
                _ => Err(nimora_agent_runtime::AutoModeError::InvalidTransition),
            }
            .map_err(|_| SqlitePersistenceError::InvalidAutoModeAttemptResolution)?;
            match checkpoint.task.status {
                AgentTaskStatus::Running => {
                    checkpoint.task.transition(AgentTaskStatus::Paused, now_ms)
                }
                AgentTaskStatus::Paused => Ok(()),
                _ => Err(nimora_agent_runtime::AgentRuntimeError::InvalidTaskTransition),
            }
            .map_err(|_| SqlitePersistenceError::InvalidAutoModeAttemptResolution)?;
        }
        AutoModeAttemptResolutionDecision::AcceptExternalEffectAndCancel => {
            session
                .cancel(now_ms)
                .map_err(|_| SqlitePersistenceError::InvalidAutoModeAttemptResolution)?;
            checkpoint.task.cancel(now_ms);
        }
    }
    Ok(())
}

fn load_attempt(
    connection: &Connection,
    session_id: Uuid,
) -> Result<Option<AutoModeTurnAttempt>, SqlitePersistenceError> {
    connection
        .query_row(
            "SELECT attempt_id, checkpoint_sequence, expected_session_updated_at_ms,
            request_fingerprint, status, started_at_ms, updated_at_ms
         FROM auto_mode_turn_attempt WHERE session_id = ?1",
            [session_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        )
        .optional()?
        .map(
            |(id, sequence, expected, fingerprint, status, started, updated)| {
                Ok(AutoModeTurnAttempt {
                    id: Uuid::parse_str(&id)
                        .map_err(|_| SqlitePersistenceError::InvalidAutoModeTurnAttempt)?,
                    session_id,
                    checkpoint_sequence: to_u64(sequence)?,
                    expected_session_updated_at_ms: to_u64(expected)?,
                    request_fingerprint: fingerprint,
                    status: match status.as_str() {
                        "active" => AutoModeTurnAttemptStatus::Active,
                        "indeterminate" => AutoModeTurnAttemptStatus::Indeterminate,
                        _ => return Err(SqlitePersistenceError::InvalidAutoModeTurnAttempt),
                    },
                    started_at_ms: to_u64(started)?,
                    updated_at_ms: to_u64(updated)?,
                })
            },
        )
        .transpose()
}

fn load_session(
    connection: &Connection,
    session_id: Uuid,
) -> Result<AutoModeSession, SqlitePersistenceError> {
    let payload = connection
        .query_row(
            "SELECT payload FROM auto_mode_session WHERE session_id = ?1",
            [session_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(SqlitePersistenceError::AutoModeAttemptResolutionConflict)?;
    serde_json::from_str(&payload).map_err(Into::into)
}

fn load_checkpoint(
    connection: &Connection,
    session_id: Uuid,
) -> Result<AutoModeCheckpoint, SqlitePersistenceError> {
    let payload = connection
        .query_row(
            "SELECT payload FROM auto_mode_checkpoint WHERE session_id = ?1",
            [session_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(SqlitePersistenceError::AutoModeAttemptResolutionConflict)?;
    serde_json::from_str(&payload).map_err(Into::into)
}

fn validate_request(request: &ResolveAutoModeAttemptRequest) -> Result<(), SqlitePersistenceError> {
    if request.checkpoint_sequence == 0
        || !valid_text(&request.request_fingerprint, 256)
        || !valid_text(&request.actor, MAX_ACTOR_BYTES)
        || request.reason.as_ref().is_some_and(|value| {
            value.len() > MAX_REASON_BYTES || value.chars().any(char::is_control)
        })
    {
        return Err(SqlitePersistenceError::InvalidAutoModeAttemptResolution);
    }
    Ok(())
}

fn validate_resolution(value: &AutoModeAttemptResolution) -> Result<(), SqlitePersistenceError> {
    if value.spec != "nimora.auto-mode-attempt-resolution/1"
        || value.checkpoint_sequence == 0
        || !valid_text(&value.request_fingerprint, 256)
        || !valid_text(&value.actor, MAX_ACTOR_BYTES)
        || value.reason.as_ref().is_some_and(|reason| {
            reason.len() > MAX_REASON_BYTES || reason.chars().any(char::is_control)
        })
    {
        return Err(SqlitePersistenceError::InvalidAutoModeAttemptResolution);
    }
    Ok(())
}

fn valid_text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}

fn decision_name(value: AutoModeAttemptResolutionDecision) -> &'static str {
    match value {
        AutoModeAttemptResolutionDecision::ConfirmedNotExecuted => "confirmed_not_executed",
        AutoModeAttemptResolutionDecision::AcceptExternalEffectAndCancel => {
            "accept_external_effect_and_cancel"
        }
    }
}

fn status_name(value: AutoModeStatus) -> &'static str {
    match value {
        AutoModeStatus::Running => "running",
        AutoModeStatus::Paused => "paused",
        AutoModeStatus::Completed => "completed",
        AutoModeStatus::Cancelled => "cancelled",
    }
}

fn pause_reason_name(value: AutoModePauseReason) -> &'static str {
    match value {
        AutoModePauseReason::ConfirmationRequired => "confirmation_required",
        AutoModePauseReason::BudgetExhausted => "budget_exhausted",
        AutoModePauseReason::GoalChanged => "goal_changed",
        AutoModePauseReason::UserRequested => "user_requested",
        AutoModePauseReason::Restarted => "restarted",
        AutoModePauseReason::WorkspaceChanged => "workspace_changed",
        AutoModePauseReason::ProviderUnavailable => "provider_unavailable",
        AutoModePauseReason::UnsafeEffect => "unsafe_effect",
    }
}

fn to_i64(value: u64) -> Result<i64, SqlitePersistenceError> {
    i64::try_from(value).map_err(|_| SqlitePersistenceError::InvalidAutoModeAttemptResolution)
}
fn to_u64(value: i64) -> Result<u64, SqlitePersistenceError> {
    u64::try_from(value).map_err(|_| SqlitePersistenceError::InvalidAutoModeAttemptResolution)
}
