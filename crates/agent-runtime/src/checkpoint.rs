use crate::{AgentTask, AgentTaskStatus, ProviderMessage, provider::validate_checkpoint_messages};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

const MAX_BINDING_BYTES: usize = 256;
const MAX_MODEL_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutoModeCheckpoint {
    pub spec: String,
    pub session_id: Uuid,
    pub goal_id: Uuid,
    pub plan_revision: u64,
    pub sequence: u64,
    pub task: AgentTask,
    pub model: String,
    pub messages: Vec<ProviderMessage>,
    pub workspace_revision: String,
    pub policy_fingerprint: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

impl AutoModeCheckpoint {
    /// Creates a bounded continuation checkpoint without persisting approvals or native handles.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identities, bindings, messages, task state, or timestamps.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_id: Uuid,
        goal_id: Uuid,
        plan_revision: u64,
        sequence: u64,
        task: AgentTask,
        model: impl Into<String>,
        messages: Vec<ProviderMessage>,
        workspace_revision: impl Into<String>,
        policy_fingerprint: impl Into<String>,
        created_at_ms: u64,
        updated_at_ms: u64,
    ) -> Result<Self, AutoModeCheckpointError> {
        let checkpoint = Self {
            spec: "nimora.auto-mode-checkpoint/1".to_owned(),
            session_id,
            goal_id,
            plan_revision,
            sequence,
            task,
            model: model.into(),
            messages,
            workspace_revision: workspace_revision.into(),
            policy_fingerprint: policy_fingerprint.into(),
            created_at_ms,
            updated_at_ms,
        };
        checkpoint.validate()?;
        Ok(checkpoint)
    }

    /// Validates a checkpoint restored across a process or persistence boundary.
    ///
    /// # Errors
    ///
    /// Returns an error when metadata, continuation messages, or resource bindings are invalid.
    pub fn validate(&self) -> Result<(), AutoModeCheckpointError> {
        self.task
            .validate()
            .map_err(|_| AutoModeCheckpointError::InvalidCheckpoint)?;
        if self.spec != "nimora.auto-mode-checkpoint/1"
            || self.plan_revision == 0
            || self.sequence == 0
            || self.updated_at_ms < self.created_at_ms
            || !matches!(
                self.task.status,
                AgentTaskStatus::Planning
                    | AgentTaskStatus::Running
                    | AgentTaskStatus::Paused
                    | AgentTaskStatus::Succeeded
                    | AgentTaskStatus::Cancelled
            )
            || self.model.trim().is_empty()
            || self.model.len() > MAX_MODEL_BYTES
            || !valid_binding(&self.workspace_revision)
            || !valid_binding(&self.policy_fingerprint)
        {
            return Err(AutoModeCheckpointError::InvalidCheckpoint);
        }
        validate_checkpoint_messages(&self.messages)
            .map_err(|_| AutoModeCheckpointError::InvalidCheckpoint)?;
        Ok(())
    }

    #[must_use]
    pub fn matches_bindings(
        &self,
        session_id: Uuid,
        goal_id: Uuid,
        plan_revision: u64,
        workspace_revision: &str,
        policy_fingerprint: &str,
    ) -> bool {
        self.session_id == session_id
            && self.goal_id == goal_id
            && self.plan_revision == plan_revision
            && self.workspace_revision == workspace_revision
            && self.policy_fingerprint == policy_fingerprint
    }
}

fn valid_binding(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_BINDING_BYTES
        && !value.chars().any(char::is_control)
}

#[derive(Debug, Error)]
pub enum AutoModeCheckpointError {
    #[error("Auto Mode checkpoint is invalid")]
    InvalidCheckpoint,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentBudget, AgentTaskOrigin, DataClassification, ProviderMessageRole};

    fn checkpoint() -> AutoModeCheckpoint {
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
        AutoModeCheckpoint::new(
            Uuid::now_v7(),
            Uuid::now_v7(),
            1,
            1,
            task,
            "model:local",
            vec![ProviderMessage::text(
                ProviderMessageRole::User,
                "continue",
                DataClassification::Personal,
                true,
            )],
            "git:abc",
            "sha256:policy",
            1_000,
            1_001,
        )
        .expect("checkpoint")
    }

    #[test]
    fn validates_exact_resume_bindings() {
        let checkpoint = checkpoint();
        assert!(checkpoint.matches_bindings(
            checkpoint.session_id,
            checkpoint.goal_id,
            1,
            "git:abc",
            "sha256:policy"
        ));
        assert!(!checkpoint.matches_bindings(
            checkpoint.session_id,
            checkpoint.goal_id,
            2,
            "git:abc",
            "sha256:policy"
        ));
    }

    #[test]
    fn accepts_cancelled_task_but_rejects_unbounded_bindings() {
        let mut checkpoint = checkpoint();
        checkpoint.task.cancel(1_002);
        assert!(checkpoint.validate().is_ok());
        checkpoint.task.status = AgentTaskStatus::Running;
        checkpoint.workspace_revision = "x".repeat(MAX_BINDING_BYTES + 1);
        assert!(checkpoint.validate().is_err());
    }
}
