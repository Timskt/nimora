use crate::{SqlitePersistenceError, prepare_connection};
use nimora_agent_runtime::{AgentGoal, AgentGoalStatus, AgentPlan};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use std::{path::Path, sync::Mutex};
use uuid::Uuid;

const MAX_GOAL_PAGE: usize = 200;

#[derive(Debug)]
pub struct SqliteAgentGoalRepository {
    connection: Mutex<Connection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentGoalSnapshot {
    pub goal: AgentGoal,
    pub current_plan: AgentPlan,
}

impl SqliteAgentGoalRepository {
    /// Opens or creates a persistent Goal store.
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

    /// Creates an isolated in-memory Goal store.
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

    /// Atomically creates a Goal and its first immutable plan revision.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid bindings, duplicates, or unavailable storage.
    pub fn create(&self, goal: &AgentGoal, plan: &AgentPlan) -> Result<(), SqlitePersistenceError> {
        validate_snapshot(goal, plan)?;
        if plan.revision != 1 {
            return Err(SqlitePersistenceError::InvalidAgentGoal);
        }
        let goal_payload = serde_json::to_string(goal)?;
        let plan_payload = serde_json::to_string(plan)?;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO agent_goal (
                goal_id, status, current_plan_revision, created_at_ms, updated_at_ms,
                schema_version, payload
             ) VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)",
            params![
                goal.id.to_string(),
                status_name(goal.status),
                to_i64(goal.current_plan_revision)?,
                to_i64(goal.created_at_ms)?,
                to_i64(goal.updated_at_ms)?,
                goal_payload,
            ],
        )?;
        insert_plan(&transaction, plan, &plan_payload)?;
        transaction.commit()?;
        Ok(())
    }

    /// Loads a Goal with the plan revision it currently adopts.
    ///
    /// # Errors
    ///
    /// Returns an error for corrupt, unsupported, or unavailable storage.
    pub fn get(&self, goal_id: Uuid) -> Result<Option<AgentGoalSnapshot>, SqlitePersistenceError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        load_snapshot(&connection, goal_id)
    }

    /// Loads one immutable historical plan revision for audit and continuation bindings.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identifiers, corrupt payloads, or unavailable storage.
    pub fn get_plan(
        &self,
        goal_id: Uuid,
        revision: u64,
    ) -> Result<Option<AgentPlan>, SqlitePersistenceError> {
        if revision == 0 {
            return Err(SqlitePersistenceError::InvalidAgentGoal);
        }
        let stored = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .query_row(
                "SELECT schema_version, payload, created_at_ms FROM agent_goal_plan
                 WHERE goal_id = ?1 AND revision = ?2",
                params![goal_id.to_string(), to_i64(revision)?],
                |row| {
                    Ok((
                        row.get::<_, u32>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?;
        let Some((version, payload, created_at_ms)) = stored else {
            return Ok(None);
        };
        ensure_version(version)?;
        let plan = serde_json::from_str::<AgentPlan>(&payload)?;
        plan.validate()
            .map_err(|_| SqlitePersistenceError::InvalidAgentGoal)?;
        if plan.goal_id != goal_id
            || plan.revision != revision
            || to_i64(plan.created_at_ms)? != created_at_ms
        {
            return Err(SqlitePersistenceError::InvalidAgentGoal);
        }
        Ok(Some(plan))
    }

    /// Lists Goals in stable newest-first order without returning historical plans.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid limits or corrupt storage.
    pub fn list(&self, limit: usize) -> Result<Vec<AgentGoal>, SqlitePersistenceError> {
        if limit == 0 || limit > MAX_GOAL_PAGE {
            return Err(SqlitePersistenceError::InvalidAgentGoal);
        }
        let limit = i64::try_from(limit).map_err(|_| SqlitePersistenceError::InvalidAgentGoal)?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let mut statement = connection.prepare(
            "SELECT schema_version, payload, status, current_plan_revision, updated_at_ms
             FROM agent_goal
             ORDER BY updated_at_ms DESC, goal_id DESC LIMIT ?1",
        )?;
        let rows = statement.query_map([limit], |row| {
            Ok((
                row.get::<_, u32>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })?;
        let mut goals = Vec::new();
        for row in rows {
            let (version, payload, status, revision, updated_at_ms) = row?;
            ensure_version(version)?;
            let goal = serde_json::from_str::<AgentGoal>(&payload)?;
            goal.validate()
                .map_err(|_| SqlitePersistenceError::InvalidAgentGoal)?;
            validate_goal_metadata(&goal, &status, revision, updated_at_ms)?;
            goals.push(goal);
        }
        Ok(goals)
    }

    /// Atomically appends a plan revision and advances the Goal binding.
    ///
    /// # Errors
    ///
    /// Returns an error for stale writers, invalid revisions, or unavailable storage.
    pub fn revise(&self, goal: &AgentGoal, plan: &AgentPlan) -> Result<(), SqlitePersistenceError> {
        validate_snapshot(goal, plan)?;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let transaction = connection.transaction()?;
        let current = load_snapshot(&transaction, goal.id)?
            .ok_or(SqlitePersistenceError::AgentGoalNotFound)?;
        if goal.current_plan_revision != current.goal.current_plan_revision.saturating_add(1)
            || goal.created_at_ms != current.goal.created_at_ms
            || goal.title != current.goal.title
            || goal.objective != current.goal.objective
            || goal.status != current.goal.status
        {
            return Err(SqlitePersistenceError::AgentGoalConflict);
        }
        let goal_payload = serde_json::to_string(goal)?;
        let plan_payload = serde_json::to_string(plan)?;
        insert_plan(&transaction, plan, &plan_payload)?;
        let changed = transaction.execute(
            "UPDATE agent_goal
             SET current_plan_revision = ?1, updated_at_ms = ?2, payload = ?3
             WHERE goal_id = ?4 AND current_plan_revision = ?5",
            params![
                to_i64(goal.current_plan_revision)?,
                to_i64(goal.updated_at_ms)?,
                goal_payload,
                goal.id.to_string(),
                to_i64(current.goal.current_plan_revision)?,
            ],
        )?;
        if changed != 1 {
            return Err(SqlitePersistenceError::AgentGoalConflict);
        }
        transaction.commit()?;
        Ok(())
    }

    /// Persists a lifecycle transition after revalidating it against current plan evidence.
    ///
    /// # Errors
    ///
    /// Returns an error for stale writers, invalid transitions, or unavailable storage.
    pub fn transition(&self, goal: &AgentGoal) -> Result<(), SqlitePersistenceError> {
        goal.validate()
            .map_err(|_| SqlitePersistenceError::InvalidAgentGoal)?;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let transaction = connection.transaction()?;
        let current = load_snapshot(&transaction, goal.id)?
            .ok_or(SqlitePersistenceError::AgentGoalNotFound)?;
        if goal.current_plan_revision != current.goal.current_plan_revision
            || goal.created_at_ms != current.goal.created_at_ms
            || goal.title != current.goal.title
            || goal.objective != current.goal.objective
        {
            return Err(SqlitePersistenceError::AgentGoalConflict);
        }
        let previous_updated_at_ms = current.goal.updated_at_ms;
        let mut expected = current.goal;
        expected
            .transition(goal.status, &current.current_plan, goal.updated_at_ms)
            .map_err(|_| SqlitePersistenceError::InvalidAgentGoal)?;
        if &expected != goal {
            return Err(SqlitePersistenceError::InvalidAgentGoal);
        }
        let payload = serde_json::to_string(goal)?;
        let changed = transaction.execute(
            "UPDATE agent_goal SET status = ?1, updated_at_ms = ?2, payload = ?3
             WHERE goal_id = ?4 AND updated_at_ms = ?5",
            params![
                status_name(goal.status),
                to_i64(goal.updated_at_ms)?,
                payload,
                goal.id.to_string(),
                to_i64(previous_updated_at_ms)?,
            ],
        )?;
        if changed != 1 {
            return Err(SqlitePersistenceError::AgentGoalConflict);
        }
        transaction.commit()?;
        Ok(())
    }
}

fn insert_plan(
    transaction: &Transaction<'_>,
    plan: &AgentPlan,
    payload: &str,
) -> Result<(), SqlitePersistenceError> {
    transaction.execute(
        "INSERT INTO agent_goal_plan (
            goal_id, revision, created_at_ms, schema_version, payload
         ) VALUES (?1, ?2, ?3, 1, ?4)",
        params![
            plan.goal_id.to_string(),
            to_i64(plan.revision)?,
            to_i64(plan.created_at_ms)?,
            payload,
        ],
    )?;
    Ok(())
}

fn load_snapshot(
    connection: &Connection,
    goal_id: Uuid,
) -> Result<Option<AgentGoalSnapshot>, SqlitePersistenceError> {
    let stored = connection
        .query_row(
            "SELECT schema_version, payload, status, current_plan_revision, updated_at_ms
             FROM agent_goal WHERE goal_id = ?1",
            [goal_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()?;
    let Some((goal_version, goal_payload, status, revision, updated_at_ms)) = stored else {
        return Ok(None);
    };
    ensure_version(goal_version)?;
    let goal = serde_json::from_str::<AgentGoal>(&goal_payload)?;
    validate_goal_metadata(&goal, &status, revision, updated_at_ms)?;
    let revision = u64::try_from(revision).map_err(|_| SqlitePersistenceError::InvalidAgentGoal)?;
    let plan_stored = connection
        .query_row(
            "SELECT schema_version, payload FROM agent_goal_plan
             WHERE goal_id = ?1 AND revision = ?2",
            params![goal_id.to_string(), to_i64(revision)?],
            |row| Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or(SqlitePersistenceError::InvalidAgentGoal)?;
    ensure_version(plan_stored.0)?;
    let plan = serde_json::from_str::<AgentPlan>(&plan_stored.1)?;
    validate_snapshot(&goal, &plan)?;
    Ok(Some(AgentGoalSnapshot {
        goal,
        current_plan: plan,
    }))
}

fn validate_snapshot(goal: &AgentGoal, plan: &AgentPlan) -> Result<(), SqlitePersistenceError> {
    goal.validate()
        .map_err(|_| SqlitePersistenceError::InvalidAgentGoal)?;
    plan.validate()
        .map_err(|_| SqlitePersistenceError::InvalidAgentGoal)?;
    if goal.id != plan.goal_id || goal.current_plan_revision != plan.revision {
        return Err(SqlitePersistenceError::InvalidAgentGoal);
    }
    Ok(())
}

fn validate_goal_metadata(
    goal: &AgentGoal,
    status: &str,
    revision: i64,
    updated_at_ms: i64,
) -> Result<(), SqlitePersistenceError> {
    if status != status_name(goal.status)
        || revision != to_i64(goal.current_plan_revision)?
        || updated_at_ms != to_i64(goal.updated_at_ms)?
    {
        return Err(SqlitePersistenceError::InvalidAgentGoal);
    }
    Ok(())
}

fn ensure_version(version: u32) -> Result<(), SqlitePersistenceError> {
    if version != 1 {
        return Err(SqlitePersistenceError::UnsupportedAgentGoalVersion(version));
    }
    Ok(())
}

const fn status_name(status: AgentGoalStatus) -> &'static str {
    match status {
        AgentGoalStatus::Active => "active",
        AgentGoalStatus::Paused => "paused",
        AgentGoalStatus::Completed => "completed",
        AgentGoalStatus::Cancelled => "cancelled",
    }
}

fn to_i64(value: u64) -> Result<i64, SqlitePersistenceError> {
    i64::try_from(value).map_err(|_| SqlitePersistenceError::InvalidAgentGoal)
}

#[cfg(test)]
mod tests {
    use super::SqliteAgentGoalRepository;
    use crate::SqlitePersistenceError;
    use nimora_agent_runtime::{
        AgentGoal, AgentGoalStatus, AgentPlan, AgentPlanStep, AgentPlanStepStatus,
    };
    use uuid::Uuid;

    fn fixture(now_ms: u64) -> (AgentGoal, AgentPlan) {
        let plan = AgentPlan::new(
            Uuid::now_v7(),
            vec![AgentPlanStep::new("Implement store").expect("step")],
            "Initial plan",
            now_ms,
        )
        .expect("plan");
        let goal = AgentGoal::new("Goal store", "Persist Goal state", &plan, now_ms).expect("Goal");
        (goal, plan)
    }

    #[test]
    fn creates_revises_and_restores_goal_with_current_plan() {
        let repository = SqliteAgentGoalRepository::in_memory().expect("repository");
        let (mut goal, plan) = fixture(1_000);
        repository.create(&goal, &plan).expect("create");
        let revised = plan
            .revise(
                vec![AgentPlanStep::new("Expose CLI").expect("step")],
                "Storage implemented",
                1_100,
            )
            .expect("revise");
        goal.adopt_plan(&revised, 1_100).expect("adopt");
        repository
            .revise(&goal, &revised)
            .expect("persist revision");
        let restored = repository.get(goal.id).expect("get").expect("Goal");
        assert_eq!(restored.goal, goal);
        assert_eq!(restored.current_plan, revised);
        assert_eq!(
            repository
                .get_plan(goal.id, 1)
                .expect("historical plan")
                .expect("revision one"),
            plan
        );
        assert!(
            repository
                .get_plan(goal.id, 3)
                .expect("missing plan")
                .is_none()
        );
    }

    #[test]
    fn completion_and_stale_revision_fail_closed() {
        let repository = SqliteAgentGoalRepository::in_memory().expect("repository");
        let (mut goal, mut plan) = fixture(1_000);
        repository.create(&goal, &plan).expect("create");
        assert!(
            goal.transition(AgentGoalStatus::Completed, &plan, 1_100)
                .is_err()
        );
        plan.steps[0]
            .update(
                AgentPlanStepStatus::Completed,
                vec!["test passed".to_owned()],
            )
            .expect("evidence");
        let revised = plan
            .revise(plan.steps.clone(), "Evidence added", 1_100)
            .expect("revise");
        goal.adopt_plan(&revised, 1_100).expect("adopt");
        repository
            .revise(&goal, &revised)
            .expect("persist revision");
        let mut stale = goal.clone();
        stale.current_plan_revision = 1;
        assert!(matches!(
            repository.revise(&stale, &plan),
            Err(SqlitePersistenceError::AgentGoalConflict
                | SqlitePersistenceError::InvalidAgentGoal)
        ));
        goal.transition(AgentGoalStatus::Completed, &revised, 1_200)
            .expect("complete");
        repository.transition(&goal).expect("persist completion");
        assert_eq!(
            repository
                .get(goal.id)
                .expect("get")
                .expect("Goal")
                .goal
                .status,
            AgentGoalStatus::Completed
        );
    }
}
