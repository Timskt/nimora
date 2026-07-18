//! Host-side recovery and supervision services for persistent Auto Mode sessions.

use nimora_agent_runtime::{
    AgentGoal, AgentPlan, AgentTask, AgentTaskStatus, AutoModeCheckpoint, AutoModeSession,
    AutoModeStatus, ProviderMessage, WorkspaceSnapshot,
};
use nimora_agent_workspace_host::{WorkspaceHostError, WorkspaceScanPolicy, WorkspaceScanner};
use nimora_persistence_sqlite::{
    SqliteAgentGoalRepository, SqliteAutoModeCheckpointRepository, SqliteAutoModeRepository,
    SqlitePersistenceError, SqliteWorkspaceSnapshotRepository,
};
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredAutoModeTurn {
    pub session: AutoModeSession,
    pub goal: AgentGoal,
    pub plan: AgentPlan,
    pub task: AgentTask,
    pub checkpoint_sequence: u64,
    pub model: String,
    pub messages: Vec<ProviderMessage>,
    pub workspace: WorkspaceSnapshot,
}

#[derive(Debug, Clone)]
pub struct AutoModeRecoveryService {
    database_path: PathBuf,
    workspace_policy: WorkspaceScanPolicy,
}

impl AutoModeRecoveryService {
    #[must_use]
    pub fn new(database_path: impl Into<PathBuf>, workspace_policy: WorkspaceScanPolicy) -> Self {
        Self {
            database_path: database_path.into(),
            workspace_policy,
        }
    }

    /// Restores and revalidates a paused continuation without invoking a Provider or Tool.
    ///
    /// The returned task is always paused. Approval state is intentionally absent from the
    /// checkpoint contract and therefore cannot be replayed by this service.
    ///
    /// # Errors
    ///
    /// Returns an error for missing state, changed bindings, workspace drift, corrupt storage,
    /// or an unsafe task lifecycle.
    pub fn recover(
        &self,
        session_id: Uuid,
        workspace_root: &Path,
        now_ms: u64,
    ) -> Result<RecoveredAutoModeTurn, AutoModeRecoveryError> {
        let session = SqliteAutoModeRepository::open(&self.database_path)?
            .get(session_id)?
            .ok_or(AutoModeRecoveryError::SessionNotFound)?;
        if session.status != AutoModeStatus::Paused {
            return Err(AutoModeRecoveryError::SessionNotPaused);
        }
        let goal = SqliteAgentGoalRepository::open(&self.database_path)?
            .get(session.goal_id)?
            .ok_or(AutoModeRecoveryError::GoalNotFound)?;
        let stored_workspace = SqliteWorkspaceSnapshotRepository::open(&self.database_path)?
            .latest(session_id)?
            .ok_or(AutoModeRecoveryError::WorkspaceNotFound)?;
        let checkpoint = SqliteAutoModeCheckpointRepository::open(&self.database_path)?
            .get(session_id)?
            .ok_or(AutoModeRecoveryError::CheckpointNotFound)?;

        let scanner = WorkspaceScanner::open(workspace_root, self.workspace_policy.clone())?;
        if scanner.root_fingerprint() != stored_workspace.root_fingerprint {
            return Err(AutoModeRecoveryError::WorkspaceChanged);
        }
        let workspace = scanner.scan(
            stored_workspace.snapshot.revision,
            stored_workspace.snapshot.parent_fingerprint.clone(),
            now_ms,
        )?;
        if workspace.fingerprint != stored_workspace.snapshot.fingerprint {
            return Err(AutoModeRecoveryError::WorkspaceChanged);
        }
        if goal.goal.id != session.goal_id
            || goal.current_plan.goal_id != session.goal_id
            || goal.current_plan.revision != session.plan_revision
            || session.policy.workspace_revision != workspace.fingerprint
            || !checkpoint.matches_bindings(
                session.id,
                goal.goal.id,
                goal.current_plan.revision,
                &workspace.fingerprint,
                &session.policy_fingerprint,
            )
        {
            return Err(AutoModeRecoveryError::BindingChanged);
        }

        let AutoModeCheckpoint {
            sequence,
            mut task,
            model,
            messages,
            ..
        } = checkpoint;
        match task.status {
            AgentTaskStatus::Planning | AgentTaskStatus::Running => task
                .transition(AgentTaskStatus::Paused, now_ms.max(task.updated_at_ms))
                .map_err(|_| AutoModeRecoveryError::UnsafeTaskState)?,
            AgentTaskStatus::Paused => {}
            _ => return Err(AutoModeRecoveryError::UnsafeTaskState),
        }

        Ok(RecoveredAutoModeTurn {
            session,
            goal: goal.goal,
            plan: goal.current_plan,
            task,
            checkpoint_sequence: sequence,
            model,
            messages,
            workspace,
        })
    }
}

#[derive(Debug, Error)]
pub enum AutoModeRecoveryError {
    #[error("Auto Mode session was not found")]
    SessionNotFound,
    #[error("Auto Mode session is not paused")]
    SessionNotPaused,
    #[error("Auto Mode Goal was not found")]
    GoalNotFound,
    #[error("Auto Mode workspace snapshot was not found")]
    WorkspaceNotFound,
    #[error("Auto Mode checkpoint was not found")]
    CheckpointNotFound,
    #[error("Auto Mode recovery binding changed")]
    BindingChanged,
    #[error("Auto Mode workspace changed")]
    WorkspaceChanged,
    #[error("Auto Mode checkpoint task cannot be safely paused")]
    UnsafeTaskState,
    #[error(transparent)]
    Persistence(#[from] SqlitePersistenceError),
    #[error(transparent)]
    Workspace(#[from] WorkspaceHostError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimora_agent_runtime::{
        AgentBudget, AgentPlanStep, AgentTaskOrigin, AutoModePolicy, DataClassification,
        ProviderMessageRole,
    };
    use nimora_persistence_sqlite::StoredWorkspaceSnapshot;
    use std::fs;

    fn temporary_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("nimora-auto-host-{label}-{}", Uuid::now_v7()))
    }

    fn fixture() -> (PathBuf, PathBuf, Uuid) {
        let database = temporary_path("database").with_extension("sqlite3");
        let workspace_root = temporary_path("workspace");
        fs::create_dir(&workspace_root).expect("workspace");
        fs::write(workspace_root.join("task.txt"), b"stable").expect("file");
        let scanner = WorkspaceScanner::open(&workspace_root, WorkspaceScanPolicy::default())
            .expect("scanner");
        let workspace = scanner.scan(1, None, 1_000).expect("snapshot");

        let plan = AgentPlan::new(
            Uuid::now_v7(),
            vec![AgentPlanStep::new("Inspect workspace").expect("step")],
            "initial",
            1_000,
        )
        .expect("plan");
        let goal = AgentGoal::new("Recover", "Resume safely", &plan, 1_000).expect("goal");
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
            workspace.fingerprint.clone(),
        )
        .expect("policy");
        let session = AutoModeSession::start(&goal, &plan, policy, 1_000).expect("session");
        let mut task = AgentTask::new(
            AgentTaskOrigin::Desktop,
            "desktop:auto-mode",
            "provider:local",
            AgentBudget::default(),
            1_000,
        )
        .expect("task");
        task.transition(AgentTaskStatus::Planning, 1_001)
            .expect("planning");
        task.transition(AgentTaskStatus::Running, 1_002)
            .expect("running");
        let checkpoint = AutoModeCheckpoint::new(
            session.id,
            goal.id,
            plan.revision,
            1,
            task,
            "model:local",
            vec![ProviderMessage::text(
                ProviderMessageRole::User,
                "continue",
                DataClassification::Personal,
                true,
            )],
            workspace.fingerprint.clone(),
            session.policy_fingerprint.clone(),
            1_000,
            1_002,
        )
        .expect("checkpoint");

        SqliteAgentGoalRepository::open(&database)
            .expect("goals")
            .create(&goal, &plan)
            .expect("goal");
        let sessions = SqliteAutoModeRepository::open(&database).expect("sessions");
        sessions.create(&session).expect("session");
        sessions
            .pause_running_after_restart(1_003)
            .expect("restart pause");
        SqliteWorkspaceSnapshotRepository::open(&database)
            .expect("workspaces")
            .create(
                &StoredWorkspaceSnapshot::new(session.id, scanner.root_fingerprint(), workspace)
                    .expect("stored workspace"),
            )
            .expect("workspace");
        SqliteAutoModeCheckpointRepository::open(&database)
            .expect("checkpoints")
            .create(&checkpoint)
            .expect("checkpoint");
        (database, workspace_root, session.id)
    }

    #[test]
    fn recovers_exact_bindings_as_paused_without_execution() {
        let (database, workspace, session_id) = fixture();
        let recovered = AutoModeRecoveryService::new(&database, WorkspaceScanPolicy::default())
            .recover(session_id, &workspace, 1_100)
            .expect("recover");
        assert_eq!(recovered.session.status, AutoModeStatus::Paused);
        assert_eq!(recovered.task.status, AgentTaskStatus::Paused);
        assert_eq!(recovered.checkpoint_sequence, 1);
        assert_eq!(recovered.messages.len(), 1);
        fs::remove_file(database).expect("database cleanup");
        fs::remove_dir_all(workspace).expect("workspace cleanup");
    }

    #[test]
    fn rejects_workspace_drift_before_releasing_continuation() {
        let (database, workspace, session_id) = fixture();
        fs::write(workspace.join("task.txt"), b"changed").expect("change");
        let error = AutoModeRecoveryService::new(&database, WorkspaceScanPolicy::default())
            .recover(session_id, &workspace, 1_100)
            .expect_err("drift must fail");
        assert!(matches!(error, AutoModeRecoveryError::WorkspaceChanged));
        fs::remove_file(database).expect("database cleanup");
        fs::remove_dir_all(workspace).expect("workspace cleanup");
    }
}
