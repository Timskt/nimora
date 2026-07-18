//! Host-side recovery and supervision services for persistent Auto Mode sessions.

use nimora_agent_runtime::{
    AgentGoal, AgentPlan, AgentTask, AgentTaskStatus, AutoModeCheckpoint, AutoModePauseReason,
    AutoModeSession, AutoModeStatus, AutoModeTurnOutcome, CompactedContext, ContextAnchor,
    ContextCompactionPolicy, ContextCompactor, ContextManagementError, DataClassification,
    ProviderMessage, ProviderMessageRole, ToolDescriptor, WorkspaceSnapshot,
};
use nimora_agent_workspace_host::{WorkspaceHostError, WorkspaceScanPolicy, WorkspaceScanner};
use nimora_persistence_sqlite::{
    ContextCachePolicy, SqliteAgentGoalRepository, SqliteAutoModeCheckpointRepository,
    SqliteAutoModeCommitRepository, SqliteAutoModeRepository, SqliteContextCacheRepository,
    SqlitePersistenceError, SqliteWorkspaceSnapshotRepository, StoredContextCacheEntry,
    StoredWorkspaceSnapshot,
};
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedAutoModeContext {
    pub context: CompactedContext,
    pub cache_hit: bool,
}

#[derive(Debug, Clone)]
pub struct AutoModeContextService {
    database_path: PathBuf,
    cache_policy: ContextCachePolicy,
    compaction_policy: ContextCompactionPolicy,
    ttl_ms: u64,
}

impl AutoModeContextService {
    /// Creates a bounded persistent context preparation service.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero TTL.
    pub fn new(
        database_path: impl Into<PathBuf>,
        cache_policy: ContextCachePolicy,
        compaction_policy: ContextCompactionPolicy,
        ttl_ms: u64,
    ) -> Result<Self, AutoModeContextError> {
        if ttl_ms == 0 {
            return Err(AutoModeContextError::InvalidTtl);
        }
        Ok(Self {
            database_path: database_path.into(),
            cache_policy,
            compaction_policy,
            ttl_ms,
        })
    }

    /// Compacts a continuation and reuses only its exact persistent cache identity.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid protocol, compaction bounds, expiry overflow, or storage.
    #[allow(clippy::too_many_arguments)]
    pub fn compact_or_load(
        &self,
        task: &AgentTask,
        model: &str,
        source: &[ProviderMessage],
        tools: &[ToolDescriptor],
        anchor: &ContextAnchor,
        data_classification: DataClassification,
        maximum_data_classification: DataClassification,
        now_ms: u64,
    ) -> Result<PreparedAutoModeContext, AutoModeContextError> {
        let candidate = ContextCompactor.compact(
            task.id,
            task.trace_id,
            &task.provider_id,
            model,
            source,
            tools,
            anchor,
            self.compaction_policy,
            now_ms,
        )?;
        let repository =
            SqliteContextCacheRepository::open(&self.database_path, self.cache_policy)?;
        if let Some(context) = repository.get(
            &candidate.cache_key,
            &anchor.workspace_fingerprint,
            maximum_data_classification,
            now_ms,
        )? {
            return Ok(PreparedAutoModeContext {
                context,
                cache_hit: true,
            });
        }
        let expires_at_ms = now_ms
            .checked_add(self.ttl_ms)
            .ok_or(AutoModeContextError::InvalidTtl)?;
        repository.put(
            &StoredContextCacheEntry::new(candidate.clone(), data_classification, expires_at_ms)?,
            now_ms,
        )?;
        Ok(PreparedAutoModeContext {
            context: candidate,
            cache_hit: false,
        })
    }
}

#[derive(Debug, Error)]
pub enum AutoModeContextError {
    #[error("Auto Mode context cache TTL is invalid")]
    InvalidTtl,
    #[error(transparent)]
    Context(#[from] ContextManagementError),
    #[error(transparent)]
    Persistence(#[from] SqlitePersistenceError),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredAutoModeTurn {
    pub session: AutoModeSession,
    pub goal: AgentGoal,
    pub plan: AgentPlan,
    pub task: AgentTask,
    pub checkpoint_sequence: u64,
    pub checkpoint_created_at_ms: u64,
    pub model: String,
    pub messages: Vec<ProviderMessage>,
    pub workspace: WorkspaceSnapshot,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WorkspaceTurnPreflight {
    Ready(Box<RecoveredAutoModeTurn>),
    PausedForDrift {
        session: Box<AutoModeSession>,
        checkpoint_sequence: u64,
        workspace: Box<WorkspaceSnapshot>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommittedAutoModeTurn {
    Continue(Box<RecoveredAutoModeTurn>),
    Paused(Box<RecoveredAutoModeTurn>),
    Completed(Box<RecoveredAutoModeTurn>),
}

#[derive(Debug, Clone)]
pub struct AutoModeTurnCommitService {
    database_path: PathBuf,
}

impl AutoModeTurnCommitService {
    #[must_use]
    pub fn new(database_path: impl Into<PathBuf>) -> Self {
        Self {
            database_path: database_path.into(),
        }
    }

    /// Durably couples a post-Provider turn to its session and checkpoint versions.
    ///
    /// A persistence error after a Provider or Tool call is deliberately classified as
    /// indeterminate. Callers must recover or pause for inspection and must never replay the
    /// remote call automatically.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid outcome state or an indeterminate atomic commit.
    pub fn commit(
        &self,
        mut turn: RecoveredAutoModeTurn,
        outcome: AutoModeTurnOutcome,
        expected_session_updated_at_ms: u64,
        now_ms: u64,
    ) -> Result<CommittedAutoModeTurn, AutoModeTurnCommitError> {
        if now_ms < turn.session.updated_at_ms
            || expected_session_updated_at_ms > turn.session.updated_at_ms
        {
            return Err(AutoModeTurnCommitError::InvalidOutcomeState);
        }
        let previous_checkpoint_sequence = turn.checkpoint_sequence;
        let committed = match outcome {
            AutoModeTurnOutcome::Continue { messages } => {
                if turn.task.status != AgentTaskStatus::Running {
                    return Err(AutoModeTurnCommitError::InvalidOutcomeState);
                }
                turn.messages.extend(messages);
                CommittedAutoModeTurnKind::Continue
            }
            AutoModeTurnOutcome::Paused { reason, .. } => {
                if turn.session.status != AutoModeStatus::Paused
                    || turn.session.pause_reason != Some(reason)
                {
                    return Err(AutoModeTurnCommitError::InvalidOutcomeState);
                }
                if turn.task.status != AgentTaskStatus::Paused {
                    turn.task
                        .transition(AgentTaskStatus::Paused, now_ms)
                        .map_err(|_| AutoModeTurnCommitError::InvalidOutcomeState)?;
                }
                CommittedAutoModeTurnKind::Paused
            }
            AutoModeTurnOutcome::Completed { response } => {
                if turn.task.status != AgentTaskStatus::Succeeded {
                    return Err(AutoModeTurnCommitError::InvalidOutcomeState);
                }
                turn.messages.push(ProviderMessage::text(
                    ProviderMessageRole::Assistant,
                    response.content,
                    DataClassification::Personal,
                    false,
                ));
                turn.session
                    .complete(now_ms)
                    .map_err(|_| AutoModeTurnCommitError::InvalidOutcomeState)?;
                CommittedAutoModeTurnKind::Completed
            }
        };
        turn.checkpoint_sequence = turn
            .checkpoint_sequence
            .checked_add(1)
            .ok_or(AutoModeTurnCommitError::InvalidOutcomeState)?;
        let checkpoint = AutoModeCheckpoint::new(
            turn.session.id,
            turn.goal.id,
            turn.plan.revision,
            turn.checkpoint_sequence,
            turn.task.clone(),
            turn.model.clone(),
            turn.messages.clone(),
            turn.workspace.fingerprint.clone(),
            turn.session.policy_fingerprint.clone(),
            turn.checkpoint_created_at_ms,
            now_ms,
        )
        .map_err(|_| AutoModeTurnCommitError::InvalidOutcomeState)?;
        SqliteAutoModeCommitRepository::open(&self.database_path)
            .and_then(|mut repository| {
                repository.commit_turn(
                    &turn.session,
                    expected_session_updated_at_ms,
                    &checkpoint,
                    previous_checkpoint_sequence,
                )
            })
            .map_err(AutoModeTurnCommitError::CommitIndeterminate)?;
        Ok(match committed {
            CommittedAutoModeTurnKind::Continue => CommittedAutoModeTurn::Continue(Box::new(turn)),
            CommittedAutoModeTurnKind::Paused => CommittedAutoModeTurn::Paused(Box::new(turn)),
            CommittedAutoModeTurnKind::Completed => {
                CommittedAutoModeTurn::Completed(Box::new(turn))
            }
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum CommittedAutoModeTurnKind {
    Continue,
    Paused,
    Completed,
}

#[derive(Debug, Error)]
pub enum AutoModeTurnCommitError {
    #[error("Auto Mode turn outcome is inconsistent with its session or task")]
    InvalidOutcomeState,
    #[error("Auto Mode turn commit is indeterminate and must not be replayed automatically")]
    CommitIndeterminate(#[source] SqlitePersistenceError),
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
            created_at_ms,
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
            checkpoint_created_at_ms: created_at_ms,
            model,
            messages,
            workspace,
        })
    }

    /// Explicitly resumes a previously recovered candidate using an atomic dual-CAS commit.
    ///
    /// This method performs no Provider or Tool call and does not restore approval state.
    ///
    /// # Errors
    ///
    /// Returns an error when bindings changed, time moved backwards, or another writer won.
    pub fn commit_resume(
        &self,
        recovered: RecoveredAutoModeTurn,
        now_ms: u64,
    ) -> Result<RecoveredAutoModeTurn, AutoModeRecoveryError> {
        let previous_session_updated_at_ms = recovered.session.updated_at_ms;
        let previous_checkpoint_sequence = recovered.checkpoint_sequence;
        let mut resumed = recovered;
        resumed
            .session
            .resume(
                &resumed.goal,
                &resumed.plan,
                &resumed.workspace.fingerprint,
                &resumed.session.policy_fingerprint.clone(),
                now_ms,
            )
            .map_err(|_| AutoModeRecoveryError::BindingChanged)?;
        resumed
            .task
            .transition(AgentTaskStatus::Running, now_ms)
            .map_err(|_| AutoModeRecoveryError::UnsafeTaskState)?;
        resumed.checkpoint_sequence = previous_checkpoint_sequence
            .checked_add(1)
            .ok_or(AutoModeRecoveryError::UnsafeTaskState)?;
        let checkpoint = AutoModeCheckpoint::new(
            resumed.session.id,
            resumed.goal.id,
            resumed.plan.revision,
            resumed.checkpoint_sequence,
            resumed.task.clone(),
            resumed.model.clone(),
            resumed.messages.clone(),
            resumed.workspace.fingerprint.clone(),
            resumed.session.policy_fingerprint.clone(),
            resumed.checkpoint_created_at_ms,
            now_ms,
        )
        .map_err(|_| AutoModeRecoveryError::UnsafeTaskState)?;
        SqliteAutoModeCommitRepository::open(&self.database_path)?.commit_resume(
            &resumed.session,
            previous_session_updated_at_ms,
            &checkpoint,
            previous_checkpoint_sequence,
        )?;
        Ok(resumed)
    }

    /// Rechecks the Workspace immediately before a Provider turn and atomically pauses drift.
    ///
    /// # Errors
    ///
    /// Returns an error for root changes, scan failures, invalid state, or concurrent writes.
    pub fn preflight_workspace(
        &self,
        mut turn: RecoveredAutoModeTurn,
        workspace_root: &Path,
        now_ms: u64,
    ) -> Result<WorkspaceTurnPreflight, AutoModeRecoveryError> {
        if turn.session.status != AutoModeStatus::Running
            || turn.task.status != AgentTaskStatus::Running
        {
            return Err(AutoModeRecoveryError::UnsafeTaskState);
        }
        let stored = SqliteWorkspaceSnapshotRepository::open(&self.database_path)?
            .latest(turn.session.id)?
            .ok_or(AutoModeRecoveryError::WorkspaceNotFound)?;
        let scanner = WorkspaceScanner::open(workspace_root, self.workspace_policy.clone())?;
        if scanner.root_fingerprint() != stored.root_fingerprint {
            return Err(AutoModeRecoveryError::WorkspaceChanged);
        }
        let observed = scanner.scan(
            stored.snapshot.revision,
            stored.snapshot.parent_fingerprint.clone(),
            now_ms,
        )?;
        if observed.fingerprint == stored.snapshot.fingerprint {
            turn.workspace = observed;
            return Ok(WorkspaceTurnPreflight::Ready(Box::new(turn)));
        }

        let successor_revision = stored
            .snapshot
            .revision
            .checked_add(1)
            .ok_or(AutoModeRecoveryError::WorkspaceChanged)?;
        let successor = scanner.scan(
            successor_revision,
            Some(stored.snapshot.fingerprint.clone()),
            now_ms,
        )?;
        let previous_session_updated_at_ms = turn.session.updated_at_ms;
        let previous_checkpoint_sequence = turn.checkpoint_sequence;
        turn.session
            .pause(AutoModePauseReason::WorkspaceChanged, now_ms)
            .map_err(|_| AutoModeRecoveryError::UnsafeTaskState)?;
        turn.task
            .transition(AgentTaskStatus::Paused, now_ms)
            .map_err(|_| AutoModeRecoveryError::UnsafeTaskState)?;
        turn.checkpoint_sequence = turn
            .checkpoint_sequence
            .checked_add(1)
            .ok_or(AutoModeRecoveryError::UnsafeTaskState)?;
        let checkpoint = AutoModeCheckpoint::new(
            turn.session.id,
            turn.goal.id,
            turn.plan.revision,
            turn.checkpoint_sequence,
            turn.task,
            turn.model,
            turn.messages,
            stored.snapshot.fingerprint.clone(),
            turn.session.policy_fingerprint.clone(),
            turn.checkpoint_created_at_ms,
            now_ms,
        )
        .map_err(|_| AutoModeRecoveryError::UnsafeTaskState)?;
        let successor_record = StoredWorkspaceSnapshot::new(
            turn.session.id,
            stored.root_fingerprint,
            successor.clone(),
        )?;
        SqliteAutoModeCommitRepository::open(&self.database_path)?.commit_workspace_drift(
            &turn.session,
            previous_session_updated_at_ms,
            &checkpoint,
            previous_checkpoint_sequence,
            &successor_record,
            stored.snapshot.revision,
            &stored.snapshot.fingerprint,
        )?;
        Ok(WorkspaceTurnPreflight::PausedForDrift {
            session: Box::new(turn.session),
            checkpoint_sequence: turn.checkpoint_sequence,
            workspace: Box::new(successor),
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
        AgentBudget, AgentPlanStep, AgentTaskOrigin, AutoModePolicy, AutoModeStepRequest,
        AutoModeUsage, DataClassification, ProviderFinishReason, ProviderMessageRole,
        ProviderResponse, ProviderUsage, ToolEffect,
    };
    use nimora_persistence_sqlite::StoredWorkspaceSnapshot;
    use nimora_runtime_core::CommandRisk;
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

    #[test]
    fn explicitly_resumes_session_and_checkpoint_atomically() {
        let (database, workspace, session_id) = fixture();
        let service = AutoModeRecoveryService::new(&database, WorkspaceScanPolicy::default());
        let recovered = service
            .recover(session_id, &workspace, 1_100)
            .expect("recover");
        let resumed = service.commit_resume(recovered, 1_101).expect("resume");
        assert_eq!(resumed.session.status, AutoModeStatus::Running);
        assert_eq!(resumed.task.status, AgentTaskStatus::Running);
        assert_eq!(resumed.checkpoint_sequence, 2);
        assert_eq!(
            SqliteAutoModeRepository::open(&database)
                .expect("sessions")
                .get(session_id)
                .expect("session")
                .expect("stored session")
                .status,
            AutoModeStatus::Running
        );
        let checkpoint = SqliteAutoModeCheckpointRepository::open(&database)
            .expect("checkpoints")
            .get(session_id)
            .expect("checkpoint")
            .expect("stored checkpoint");
        assert_eq!(checkpoint.sequence, 2);
        assert_eq!(checkpoint.task.status, AgentTaskStatus::Running);
        fs::remove_file(database).expect("database cleanup");
        fs::remove_dir_all(workspace).expect("workspace cleanup");
    }

    #[test]
    fn stale_checkpoint_rolls_back_session_resume() {
        let (database, workspace, session_id) = fixture();
        let service = AutoModeRecoveryService::new(&database, WorkspaceScanPolicy::default());
        let recovered = service
            .recover(session_id, &workspace, 1_100)
            .expect("recover");
        let competing = AutoModeCheckpoint::new(
            recovered.session.id,
            recovered.goal.id,
            recovered.plan.revision,
            2,
            recovered.task.clone(),
            recovered.model.clone(),
            recovered.messages.clone(),
            recovered.workspace.fingerprint.clone(),
            recovered.session.policy_fingerprint.clone(),
            1_000,
            1_101,
        )
        .expect("competing checkpoint");
        SqliteAutoModeCheckpointRepository::open(&database)
            .expect("checkpoints")
            .replace(&competing, 1)
            .expect("competing write");

        let error = service
            .commit_resume(recovered, 1_102)
            .expect_err("stale resume must fail");
        assert!(matches!(
            error,
            AutoModeRecoveryError::Persistence(SqlitePersistenceError::AutoModeCommitConflict)
        ));
        assert_eq!(
            SqliteAutoModeRepository::open(&database)
                .expect("sessions")
                .get(session_id)
                .expect("session")
                .expect("stored session")
                .status,
            AutoModeStatus::Paused
        );
        fs::remove_file(database).expect("database cleanup");
        fs::remove_dir_all(workspace).expect("workspace cleanup");
    }

    #[test]
    fn persistent_context_cache_hits_only_exact_turn_identity() {
        let (database, workspace, session_id) = fixture();
        let recovered = AutoModeRecoveryService::new(&database, WorkspaceScanPolicy::default())
            .recover(session_id, &workspace, 1_100)
            .expect("recover");
        let service = AutoModeContextService::new(
            &database,
            ContextCachePolicy::new(16, 1024 * 1024).expect("cache policy"),
            ContextCompactionPolicy {
                max_messages: 32,
                max_content_bytes: 32 * 1024,
                retain_recent_units: 16,
            },
            60_000,
        )
        .expect("context service");
        let anchor = ContextAnchor {
            goal: recovered.goal.objective.clone(),
            constraints: vec!["Do not replay approvals".to_owned()],
            pending_steps: recovered
                .plan
                .steps
                .iter()
                .map(|step| step.text.clone())
                .collect(),
            evidence: Vec::new(),
            workspace_fingerprint: recovered.workspace.fingerprint.clone(),
            plan_revision: recovered.plan.revision,
        };
        let first = service
            .compact_or_load(
                &recovered.task,
                &recovered.model,
                &recovered.messages,
                &[],
                &anchor,
                DataClassification::Personal,
                DataClassification::Personal,
                1_101,
            )
            .expect("first context");
        assert!(!first.cache_hit);
        let second = service
            .compact_or_load(
                &recovered.task,
                &recovered.model,
                &recovered.messages,
                &[],
                &anchor,
                DataClassification::Personal,
                DataClassification::Personal,
                1_102,
            )
            .expect("cached context");
        assert!(second.cache_hit);
        assert_eq!(first.context, second.context);
        let other_model = service
            .compact_or_load(
                &recovered.task,
                "model:other",
                &recovered.messages,
                &[],
                &anchor,
                DataClassification::Personal,
                DataClassification::Personal,
                1_103,
            )
            .expect("isolated model");
        assert!(!other_model.cache_hit);
        fs::remove_file(database).expect("database cleanup");
        fs::remove_dir_all(workspace).expect("workspace cleanup");
    }

    #[test]
    fn per_turn_workspace_drift_pauses_all_state_atomically() {
        let (database, workspace, session_id) = fixture();
        let service = AutoModeRecoveryService::new(&database, WorkspaceScanPolicy::default());
        let recovered = service
            .recover(session_id, &workspace, 1_100)
            .expect("recover");
        let running = service.commit_resume(recovered, 1_101).expect("resume");
        let ready = service
            .preflight_workspace(running, &workspace, 1_102)
            .expect("unchanged preflight");
        let WorkspaceTurnPreflight::Ready(running) = ready else {
            panic!("unchanged workspace must remain ready");
        };
        fs::write(workspace.join("task.txt"), b"drifted").expect("drift");
        let paused = service
            .preflight_workspace(*running, &workspace, 1_103)
            .expect("drift preflight");
        let WorkspaceTurnPreflight::PausedForDrift {
            session,
            checkpoint_sequence,
            workspace: successor,
        } = paused
        else {
            panic!("drift must pause before Provider execution");
        };
        assert_eq!(session.status, AutoModeStatus::Paused);
        assert_eq!(
            session.pause_reason,
            Some(AutoModePauseReason::WorkspaceChanged)
        );
        assert_eq!(checkpoint_sequence, 3);
        assert_eq!(successor.revision, 2);
        let stored_checkpoint = SqliteAutoModeCheckpointRepository::open(&database)
            .expect("checkpoints")
            .get(session_id)
            .expect("checkpoint")
            .expect("stored checkpoint");
        assert_eq!(stored_checkpoint.task.status, AgentTaskStatus::Paused);
        assert_eq!(stored_checkpoint.sequence, 3);
        assert_eq!(
            SqliteWorkspaceSnapshotRepository::open(&database)
                .expect("workspaces")
                .latest(session_id)
                .expect("workspace")
                .expect("stored workspace")
                .snapshot,
            *successor
        );
        fs::remove_file(database).expect("database cleanup");
        fs::remove_dir_all(workspace).expect("workspace cleanup");
    }

    #[test]
    fn commits_continuation_once_and_rejects_stale_replay() {
        let (database, workspace, session_id) = fixture();
        let recovery = AutoModeRecoveryService::new(&database, WorkspaceScanPolicy::default());
        let recovered = recovery
            .recover(session_id, &workspace, 1_100)
            .expect("recover");
        let mut running = recovery.commit_resume(recovered, 1_101).expect("resume");
        let expected_session_updated_at_ms = running.session.updated_at_ms;
        running
            .session
            .evaluate_step(
                &AutoModeStepRequest {
                    goal_id: running.goal.id,
                    plan_revision: running.plan.revision,
                    workspace_revision: running.workspace.fingerprint.clone(),
                    tool_id: None,
                    risk: CommandRisk::Safe,
                    effect: ToolEffect::ReadOnly,
                    data_classification: DataClassification::Personal,
                    projected_usage: AutoModeUsage {
                        cycles: 1,
                        ..AutoModeUsage::default()
                    },
                },
                1_102,
            )
            .expect("account turn");
        let replay = running.clone();
        let outcome = AutoModeTurnOutcome::Continue {
            messages: vec![ProviderMessage::text(
                ProviderMessageRole::Assistant,
                "continue safely",
                DataClassification::Personal,
                false,
            )],
        };
        let committed = AutoModeTurnCommitService::new(&database)
            .commit(
                running,
                outcome.clone(),
                expected_session_updated_at_ms,
                1_102,
            )
            .expect("commit turn");
        let CommittedAutoModeTurn::Continue(committed) = committed else {
            panic!("turn must continue");
        };
        assert_eq!(committed.checkpoint_sequence, 3);
        assert_eq!(committed.messages.len(), 2);
        let error = AutoModeTurnCommitService::new(&database)
            .commit(replay, outcome, expected_session_updated_at_ms, 1_102)
            .expect_err("stale Provider result must not replay");
        assert!(matches!(
            error,
            AutoModeTurnCommitError::CommitIndeterminate(
                SqlitePersistenceError::AutoModeCommitConflict
            )
        ));
        let checkpoint = SqliteAutoModeCheckpointRepository::open(&database)
            .expect("checkpoints")
            .get(session_id)
            .expect("checkpoint")
            .expect("stored checkpoint");
        assert_eq!(checkpoint.sequence, 3);
        assert_eq!(checkpoint.messages.len(), 2);
        fs::remove_file(database).expect("database cleanup");
        fs::remove_dir_all(workspace).expect("workspace cleanup");
    }

    #[test]
    fn commits_terminal_provider_result_with_session_completion() {
        let (database, workspace, session_id) = fixture();
        let recovery = AutoModeRecoveryService::new(&database, WorkspaceScanPolicy::default());
        let recovered = recovery
            .recover(session_id, &workspace, 1_100)
            .expect("recover");
        let mut running = recovery.commit_resume(recovered, 1_101).expect("resume");
        let expected_session_updated_at_ms = running.session.updated_at_ms;
        running
            .task
            .transition(AgentTaskStatus::Succeeded, 1_102)
            .expect("succeed task");
        let response = ProviderResponse {
            spec: "nimora.agent-provider-response/1".to_owned(),
            request_id: Uuid::now_v7(),
            content: "finished".to_owned(),
            tool_calls: Vec::new(),
            finish_reason: ProviderFinishReason::Completed,
            usage: ProviderUsage {
                input_tokens: 1,
                output_tokens: 1,
                cost_microunits: 0,
            },
        };
        let committed = AutoModeTurnCommitService::new(&database)
            .commit(
                running,
                AutoModeTurnOutcome::Completed { response },
                expected_session_updated_at_ms,
                1_102,
            )
            .expect("commit completion");
        let CommittedAutoModeTurn::Completed(committed) = committed else {
            panic!("turn must complete");
        };
        assert_eq!(committed.session.status, AutoModeStatus::Completed);
        assert_eq!(committed.task.status, AgentTaskStatus::Succeeded);
        assert_eq!(
            committed.messages.last().expect("answer").content,
            "finished"
        );
        assert_eq!(
            SqliteAutoModeRepository::open(&database)
                .expect("sessions")
                .get(session_id)
                .expect("session")
                .expect("stored session")
                .status,
            AutoModeStatus::Completed
        );
        fs::remove_file(database).expect("database cleanup");
        fs::remove_dir_all(workspace).expect("workspace cleanup");
    }
}
