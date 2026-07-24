use crate::{SqlitePersistenceError, prepare_connection};
use nimora_agent_runtime::{AutoModePauseReason, AutoModeSession, AutoModeStatus};
use rusqlite::{Connection, OptionalExtension, params};
use std::{path::Path, sync::Mutex};
use uuid::Uuid;

#[derive(Debug)]
pub struct SqliteAutoModeRepository {
    connection: Mutex<Connection>,
}

impl SqliteAutoModeRepository {
    /// Lists sessions that require a restart projection.
    ///
    /// # Errors
    ///
    /// Returns an error when the bounded result would be truncated, or storage is corrupt.
    pub fn list_recoverable(
        &self,
        limit: usize,
    ) -> Result<Vec<AutoModeSession>, SqlitePersistenceError> {
        if limit == 0 || limit > 256 {
            return Err(SqlitePersistenceError::InvalidAutoModeSession);
        }
        let connection = self.lock()?;
        let query_limit = i64::try_from(limit.saturating_add(1))
            .map_err(|_| SqlitePersistenceError::InvalidAutoModeSession)?;
        let ids = {
            let mut statement = connection.prepare(
                "SELECT session_id FROM auto_mode_session
                 WHERE (status = 'paused' AND pause_reason = 'restarted') OR EXISTS (
                    SELECT 1 FROM auto_mode_turn_attempt
                    WHERE auto_mode_turn_attempt.session_id = auto_mode_session.session_id
                 )
                 ORDER BY updated_at_ms DESC, session_id DESC LIMIT ?1",
            )?;
            statement
                .query_map([query_limit], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        if ids.len() > limit {
            return Err(SqlitePersistenceError::InvalidAutoModeSession);
        }
        ids.into_iter()
            .map(|id| {
                let id = Uuid::parse_str(&id)
                    .map_err(|_| SqlitePersistenceError::InvalidAutoModeSession)?;
                load(&connection, id)?.ok_or(SqlitePersistenceError::AutoModeSessionNotFound)
            })
            .collect()
    }

    /// Opens or creates a persistent Auto Mode session store.
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

    /// Creates an isolated in-memory store.
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

    /// Inserts one validated session.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid, duplicate, or conflicting running sessions.
    pub fn create(&self, session: &AutoModeSession) -> Result<(), SqlitePersistenceError> {
        validate_session(session)?;
        let payload = serde_json::to_string(session)?;
        let connection = self.lock()?;
        connection.execute(
            "INSERT INTO auto_mode_session (
                session_id, goal_id, plan_revision, status, pause_reason,
                created_at_ms, updated_at_ms, schema_version, payload
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8)",
            params![
                session.id.to_string(),
                session.goal_id.to_string(),
                to_i64(session.plan_revision)?,
                status_name(session.status),
                session.pause_reason.map(pause_reason_name),
                to_i64(session.created_at_ms)?,
                to_i64(session.updated_at_ms)?,
                payload,
            ],
        )?;
        Ok(())
    }

    /// Loads one session while checking indexed metadata against its payload.
    ///
    /// # Errors
    ///
    /// Returns an error for corrupt, unsupported, or unavailable storage.
    pub fn get(&self, id: Uuid) -> Result<Option<AutoModeSession>, SqlitePersistenceError> {
        let connection = self.lock()?;
        load(&connection, id)
    }

    /// Persists one legal domain transition using optimistic concurrency.
    ///
    /// # Errors
    ///
    /// Returns an error when the previous timestamp is stale or metadata is invalid.
    pub fn update(
        &self,
        session: &AutoModeSession,
        previous_updated_at_ms: u64,
    ) -> Result<(), SqlitePersistenceError> {
        validate_session(session)?;
        if session.updated_at_ms < previous_updated_at_ms {
            return Err(SqlitePersistenceError::InvalidAutoModeSession);
        }
        let payload = serde_json::to_string(session)?;
        let connection = self.lock()?;
        let changed = connection.execute(
            "UPDATE auto_mode_session SET status = ?1, pause_reason = ?2,
                updated_at_ms = ?3, payload = ?4
             WHERE session_id = ?5 AND goal_id = ?6 AND plan_revision = ?7
                AND created_at_ms = ?8 AND updated_at_ms = ?9",
            params![
                status_name(session.status),
                session.pause_reason.map(pause_reason_name),
                to_i64(session.updated_at_ms)?,
                payload,
                session.id.to_string(),
                session.goal_id.to_string(),
                to_i64(session.plan_revision)?,
                to_i64(session.created_at_ms)?,
                to_i64(previous_updated_at_ms)?,
            ],
        )?;
        if changed != 1 {
            return Err(SqlitePersistenceError::AutoModeSessionConflict);
        }
        Ok(())
    }

    /// Pauses every persisted running session after a host restart.
    ///
    /// # Errors
    ///
    /// Lists sessions for a Goal, newest first (bounded).
    ///
    /// # Errors
    ///
    /// Returns an error when `limit` is out of range or storage is corrupt.
    pub fn list_for_goal(
        &self,
        goal_id: Uuid,
        limit: usize,
    ) -> Result<Vec<AutoModeSession>, SqlitePersistenceError> {
        if limit == 0 || limit > 256 {
            return Err(SqlitePersistenceError::InvalidAutoModeSession);
        }
        let connection = self.lock()?;
        let query_limit = i64::try_from(limit)
            .map_err(|_| SqlitePersistenceError::InvalidAutoModeSession)?;
        let ids = {
            let mut statement = connection.prepare(
                "SELECT session_id FROM auto_mode_session
                 WHERE goal_id = ?1
                 ORDER BY updated_at_ms DESC, session_id DESC
                 LIMIT ?2",
            )?;
            statement
                .query_map(params![goal_id.to_string(), query_limit], |row| {
                    row.get::<_, String>(0)
                })?
                .collect::<Result<Vec<_>, _>>()?
        };
        let mut sessions = Vec::with_capacity(ids.len());
        for id in ids {
            let id =
                Uuid::parse_str(&id).map_err(|_| SqlitePersistenceError::InvalidAutoModeSession)?;
            if let Some(session) = load(&connection, id)? {
                sessions.push(session);
            }
        }
        Ok(sessions)
    }

    /// Returns an error if any session is corrupt or time would move backwards.
    pub fn pause_running_after_restart(
        &self,
        now_ms: u64,
    ) -> Result<usize, SqlitePersistenceError> {
        let mut connection = self.lock()?;
        let transaction = connection.transaction()?;
        let ids = {
            let mut statement = transaction.prepare(
                "SELECT session_id FROM auto_mode_session WHERE status = 'running'
                 ORDER BY session_id",
            )?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        for id in &ids {
            let id =
                Uuid::parse_str(id).map_err(|_| SqlitePersistenceError::InvalidAutoModeSession)?;
            let mut session =
                load(&transaction, id)?.ok_or(SqlitePersistenceError::AutoModeSessionNotFound)?;
            let previous = session.updated_at_ms;
            session
                .pause_after_restart(now_ms)
                .map_err(|_| SqlitePersistenceError::InvalidAutoModeSession)?;
            let payload = serde_json::to_string(&session)?;
            let changed = transaction.execute(
                "UPDATE auto_mode_session SET status = 'paused', pause_reason = 'restarted',
                    updated_at_ms = ?1, payload = ?2
                 WHERE session_id = ?3 AND status = 'running' AND updated_at_ms = ?4",
                params![to_i64(now_ms)?, payload, id.to_string(), to_i64(previous)?],
            )?;
            if changed != 1 {
                return Err(SqlitePersistenceError::AutoModeSessionConflict);
            }
        }
        transaction.commit()?;
        Ok(ids.len())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, SqlitePersistenceError> {
        self.connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)
    }
}

fn load(
    connection: &Connection,
    id: Uuid,
) -> Result<Option<AutoModeSession>, SqlitePersistenceError> {
    let stored = connection
        .query_row(
            "SELECT schema_version, payload, goal_id, plan_revision, status, pause_reason,
                created_at_ms, updated_at_ms FROM auto_mode_session WHERE session_id = ?1",
            [id.to_string()],
            |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                ))
            },
        )
        .optional()?;
    let Some((version, payload, goal_id, revision, status, reason, created, updated)) = stored
    else {
        return Ok(None);
    };
    if version != 1 {
        return Err(SqlitePersistenceError::UnsupportedAutoModeSessionVersion(
            version,
        ));
    }
    let session = serde_json::from_str::<AutoModeSession>(&payload)?;
    validate_session(&session)?;
    if session.id != id
        || session.goal_id.to_string() != goal_id
        || to_i64(session.plan_revision)? != revision
        || status_name(session.status) != status
        || session.pause_reason.map(pause_reason_name) != reason.as_deref()
        || to_i64(session.created_at_ms)? != created
        || to_i64(session.updated_at_ms)? != updated
    {
        return Err(SqlitePersistenceError::InvalidAutoModeSession);
    }
    Ok(Some(session))
}

fn validate_session(session: &AutoModeSession) -> Result<(), SqlitePersistenceError> {
    session
        .validate()
        .map_err(|_| SqlitePersistenceError::InvalidAutoModeSession)
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
    i64::try_from(value).map_err(|_| SqlitePersistenceError::InvalidAutoModeSession)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteAgentGoalRepository;
    use nimora_agent_runtime::{
        AgentBudget, AgentGoal, AgentPlan, AgentPlanStep, AutoModePolicy, DataClassification,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture(now_ms: u64) -> (AgentGoal, AgentPlan, AutoModeSession) {
        let plan = AgentPlan::new(
            Uuid::now_v7(),
            vec![AgentPlanStep::new("Inspect").expect("step")],
            "initial",
            now_ms,
        )
        .expect("plan");
        let goal = AgentGoal::new("Auto", "Inspect safely", &plan, now_ms).expect("goal");
        let policy = AutoModePolicy::new(
            4,
            1,
            AgentBudget {
                max_steps: 4,
                max_tool_calls: 2,
                max_elapsed_ms: 10_000,
                max_input_tokens: 1_000,
                max_output_tokens: 500,
                max_cost_microunits: 0,
            },
            DataClassification::Personal,
            ["pet.state.read".to_owned()],
            "git:abc",
        )
        .expect("policy");
        let session = AutoModeSession::start(&goal, &plan, policy, now_ms).expect("session");
        (goal, plan, session)
    }

    fn stores(
        now_ms: u64,
    ) -> (
        std::path::PathBuf,
        SqliteAutoModeRepository,
        AutoModeSession,
    ) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("nimora-auto-mode-{nonce}.sqlite3"));
        let (goal, plan, session) = fixture(now_ms);
        SqliteAgentGoalRepository::open(&path)
            .expect("goal store")
            .create(&goal, &plan)
            .expect("create goal");
        let repository = SqliteAutoModeRepository::open(&path).expect("store");
        (path, repository, session)
    }

    #[test]
    fn round_trips_and_rejects_stale_updates() {
        let (path, repository, mut session) = stores(1_000);
        repository.create(&session).expect("create");
        session
            .pause(AutoModePauseReason::UserRequested, 1_001)
            .expect("pause");
        repository.update(&session, 1_000).expect("update");
        assert!(matches!(
            repository.update(&session, 1_000),
            Err(SqlitePersistenceError::AutoModeSessionConflict)
        ));
        assert_eq!(repository.get(session.id).expect("get"), Some(session));
        drop(repository);
        std::fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn restart_never_auto_resumes_running_sessions() {
        let (path, repository, session) = stores(1_000);
        repository.create(&session).expect("create");
        assert_eq!(
            repository
                .pause_running_after_restart(1_001)
                .expect("recover"),
            1
        );
        let restored = repository.get(session.id).expect("get").expect("session");
        assert_eq!(restored.status, AutoModeStatus::Paused);
        assert_eq!(restored.pause_reason, Some(AutoModePauseReason::Restarted));
        drop(repository);
        std::fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn one_goal_cannot_have_two_running_sessions() {
        let (path, repository, session) = stores(1_000);
        repository.create(&session).expect("first session");
        let mut duplicate = session.clone();
        duplicate.id = Uuid::now_v7();
        assert!(repository.create(&duplicate).is_err());
        drop(repository);
        std::fs::remove_file(path).expect("cleanup");
    }
    #[test]
    fn list_for_goal_returns_created_session() {
        let (path, repository, session) = stores(1_000);
        repository.create(&session).expect("create");
        let listed = repository
            .list_for_goal(session.goal_id, 10)
            .expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, session.id);
        assert_eq!(listed[0].goal_id, session.goal_id);
        assert!(repository
            .list_for_goal(Uuid::now_v7(), 10)
            .expect("empty list")
            .is_empty());
        drop(repository);
        std::fs::remove_file(path).expect("cleanup");
    }

}
