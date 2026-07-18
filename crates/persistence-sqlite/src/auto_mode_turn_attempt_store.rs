use crate::{SqlitePersistenceError, prepare_connection};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::Serialize;
use std::path::Path;
use uuid::Uuid;

const MAX_FINGERPRINT_BYTES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoModeTurnAttemptStatus {
    Active,
    Indeterminate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoModeTurnAttempt {
    pub id: Uuid,
    pub session_id: Uuid,
    pub checkpoint_sequence: u64,
    pub expected_session_updated_at_ms: u64,
    pub request_fingerprint: String,
    pub status: AutoModeTurnAttemptStatus,
    pub started_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug)]
pub struct SqliteAutoModeTurnAttemptRepository {
    connection: Connection,
}

impl SqliteAutoModeTurnAttemptRepository {
    /// Opens the durable pre-execution attempt store.
    ///
    /// # Errors
    ///
    /// Returns an error when the database cannot be opened or validated.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        let mut connection = Connection::open(path)?;
        prepare_connection(&mut connection)?;
        Ok(Self { connection })
    }

    /// Creates one non-reclaimable execution attempt against exact persisted versions.
    ///
    /// # Errors
    ///
    /// Returns an error when bindings are stale, another attempt exists, or input is invalid.
    pub fn begin(
        &mut self,
        session_id: Uuid,
        checkpoint_sequence: u64,
        expected_session_updated_at_ms: u64,
        request_fingerprint: impl Into<String>,
        now_ms: u64,
    ) -> Result<AutoModeTurnAttempt, SqlitePersistenceError> {
        let request_fingerprint = request_fingerprint.into();
        if checkpoint_sequence == 0
            || request_fingerprint.trim().is_empty()
            || request_fingerprint.len() > MAX_FINGERPRINT_BYTES
            || request_fingerprint.chars().any(char::is_control)
        {
            return Err(SqlitePersistenceError::InvalidAutoModeTurnAttempt);
        }
        let attempt = AutoModeTurnAttempt {
            id: Uuid::now_v7(),
            session_id,
            checkpoint_sequence,
            expected_session_updated_at_ms,
            request_fingerprint,
            status: AutoModeTurnAttemptStatus::Active,
            started_at_ms: now_ms,
            updated_at_ms: now_ms,
        };
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = transaction.execute(
            "INSERT OR IGNORE INTO auto_mode_turn_attempt (
                session_id, attempt_id, checkpoint_sequence, expected_session_updated_at_ms,
                request_fingerprint, status, started_at_ms, updated_at_ms
             ) SELECT ?1, ?2, ?3, ?4, ?5, 'active', ?6, ?6
             WHERE EXISTS (
                SELECT 1 FROM auto_mode_session WHERE session_id = ?1 AND status = 'running'
                    AND updated_at_ms = ?4
             ) AND EXISTS (
                SELECT 1 FROM auto_mode_checkpoint WHERE session_id = ?1 AND sequence = ?3
             )",
            params![
                session_id.to_string(),
                attempt.id.to_string(),
                to_i64(checkpoint_sequence)?,
                to_i64(expected_session_updated_at_ms)?,
                attempt.request_fingerprint,
                to_i64(now_ms)?,
            ],
        )?;
        if changed != 1 {
            return Err(SqlitePersistenceError::AutoModeTurnAttemptConflict);
        }
        transaction.commit()?;
        Ok(attempt)
    }

    /// Loads and validates the outstanding attempt for a session.
    ///
    /// # Errors
    ///
    /// Returns an error for corrupt metadata or storage failure.
    pub fn get(
        &self,
        session_id: Uuid,
    ) -> Result<Option<AutoModeTurnAttempt>, SqlitePersistenceError> {
        self.connection
            .query_row(
                "SELECT attempt_id, checkpoint_sequence, expected_session_updated_at_ms,
                    request_fingerprint, status, started_at_ms, updated_at_ms
                 FROM auto_mode_turn_attempt WHERE session_id = ?1",
                [session_id.to_string()],
                |row| {
                    let attempt_id = row.get::<_, String>(0)?;
                    let sequence = row.get::<_, i64>(1)?;
                    let expected = row.get::<_, i64>(2)?;
                    let fingerprint = row.get::<_, String>(3)?;
                    let status = row.get::<_, String>(4)?;
                    let started = row.get::<_, i64>(5)?;
                    let updated = row.get::<_, i64>(6)?;
                    Ok((
                        attempt_id,
                        sequence,
                        expected,
                        fingerprint,
                        status,
                        started,
                        updated,
                    ))
                },
            )
            .optional()?
            .map(
                |(id, sequence, expected, fingerprint, status, started, updated)| {
                    let attempt = AutoModeTurnAttempt {
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
                    };
                    validate(&attempt)?;
                    Ok(attempt)
                },
            )
            .transpose()
    }

    /// Permanently quarantines an attempt whose external result cannot be proven.
    ///
    /// # Errors
    ///
    /// Returns an error for stale ownership, invalid time, or storage failure.
    pub fn mark_indeterminate(
        &self,
        attempt: &AutoModeTurnAttempt,
        now_ms: u64,
    ) -> Result<(), SqlitePersistenceError> {
        if attempt.status != AutoModeTurnAttemptStatus::Active || now_ms < attempt.updated_at_ms {
            return Err(SqlitePersistenceError::InvalidAutoModeTurnAttempt);
        }
        let changed = self.connection.execute(
            "UPDATE auto_mode_turn_attempt SET status = 'indeterminate', updated_at_ms = ?1
             WHERE session_id = ?2 AND attempt_id = ?3 AND status = 'active'",
            params![
                to_i64(now_ms)?,
                attempt.session_id.to_string(),
                attempt.id.to_string(),
            ],
        )?;
        if changed != 1 {
            return Err(SqlitePersistenceError::AutoModeTurnAttemptConflict);
        }
        Ok(())
    }
}

fn validate(attempt: &AutoModeTurnAttempt) -> Result<(), SqlitePersistenceError> {
    if attempt.checkpoint_sequence == 0
        || attempt.updated_at_ms < attempt.started_at_ms
        || attempt.request_fingerprint.trim().is_empty()
        || attempt.request_fingerprint.len() > MAX_FINGERPRINT_BYTES
        || attempt.request_fingerprint.chars().any(char::is_control)
    {
        return Err(SqlitePersistenceError::InvalidAutoModeTurnAttempt);
    }
    Ok(())
}

fn to_i64(value: u64) -> Result<i64, SqlitePersistenceError> {
    i64::try_from(value).map_err(|_| SqlitePersistenceError::InvalidAutoModeTurnAttempt)
}

fn to_u64(value: i64) -> Result<u64, SqlitePersistenceError> {
    u64::try_from(value).map_err(|_| SqlitePersistenceError::InvalidAutoModeTurnAttempt)
}
