use crate::{
    AgentBudget, AgentGoal, AgentGoalStatus, AgentPlan, AgentRuntimeError, AuthorizationDecision,
    AuthorizationError, AuthorizationGrant, AuthorizationRequest, DataClassification, ToolEffect,
    ToolId,
};
use nimora_runtime_core::CommandRisk;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use thiserror::Error;
use uuid::Uuid;

const MAX_AUTO_CYCLES: u32 = 256;
const MAX_AUTO_TOOLS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoModeStatus {
    Running,
    Paused,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoModePauseReason {
    ConfirmationRequired,
    BudgetExhausted,
    GoalChanged,
    WorkspaceChanged,
    ProviderUnavailable,
    Restarted,
    UnsafeEffect,
    UserRequested,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutoModePolicy {
    pub max_cycles: u32,
    pub max_concurrency: u16,
    pub budget: AgentBudget,
    pub maximum_data_classification: DataClassification,
    pub tool_allowlist: BTreeSet<ToolId>,
    pub workspace_revision: String,
}

impl AutoModePolicy {
    /// Creates a bounded policy snapshot for one Auto Mode session.
    ///
    /// # Errors
    ///
    /// Returns an error for empty revisions, invalid limits, budgets, or Tool IDs.
    pub fn new(
        max_cycles: u32,
        max_concurrency: u16,
        budget: AgentBudget,
        maximum_data_classification: DataClassification,
        tools: impl IntoIterator<Item = String>,
        workspace_revision: impl Into<String>,
    ) -> Result<Self, AutoModeError> {
        let tool_allowlist = tools
            .into_iter()
            .map(|tool| tool.parse())
            .collect::<Result<BTreeSet<_>, AgentRuntimeError>>()?;
        let workspace_revision = workspace_revision.into();
        let policy = Self {
            max_cycles,
            max_concurrency,
            budget,
            maximum_data_classification,
            tool_allowlist,
            workspace_revision,
        };
        policy.validate()?;
        Ok(policy)
    }

    /// Validates a policy restored across a persistence boundary.
    ///
    /// # Errors
    ///
    /// Returns an error when any hard Auto Mode limit is violated.
    pub fn validate(&self) -> Result<(), AutoModeError> {
        if self.max_cycles == 0
            || self.max_cycles > MAX_AUTO_CYCLES
            || self.max_concurrency == 0
            || self.max_concurrency > 16
            || self.tool_allowlist.len() > MAX_AUTO_TOOLS
            || self.workspace_revision.trim().is_empty()
            || self.workspace_revision.len() > 256
            || self.budget.max_steps == 0
            || self.budget.max_tool_calls > self.budget.max_steps
            || self.budget.max_elapsed_ms == 0
        {
            return Err(AutoModeError::InvalidPolicy);
        }
        Ok(())
    }

    #[must_use]
    /// Returns the canonical fingerprint for this validated policy.
    ///
    /// # Panics
    ///
    /// Panics only if Serde cannot encode this fixed data-only structure.
    pub fn fingerprint(&self) -> String {
        let encoded = serde_json::to_vec(self).expect("Auto Mode policy is serializable");
        format!("sha256:{:x}", Sha256::digest(encoded))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutoModeUsage {
    pub cycles: u32,
    pub tool_calls: u32,
    pub elapsed_ms: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_microunits: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutoModeSession {
    pub spec: String,
    pub id: Uuid,
    pub goal_id: Uuid,
    pub plan_revision: u64,
    pub policy: AutoModePolicy,
    pub policy_fingerprint: String,
    pub status: AutoModeStatus,
    pub pause_reason: Option<AutoModePauseReason>,
    pub usage: AutoModeUsage,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoModeStepRequest {
    pub goal_id: Uuid,
    pub plan_revision: u64,
    pub workspace_revision: String,
    pub tool_id: Option<ToolId>,
    pub risk: CommandRisk,
    pub effect: ToolEffect,
    pub data_classification: DataClassification,
    pub projected_usage: AutoModeUsage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoModeStepDecision {
    Proceed,
    Pause(AutoModePauseReason),
}

impl AutoModeSession {
    /// Starts an Auto Mode session bound to an active Goal and exact Plan revision.
    ///
    /// # Errors
    ///
    /// Returns an error for inactive Goals, mismatched plans, or invalid policies.
    pub fn start(
        goal: &AgentGoal,
        plan: &AgentPlan,
        policy: AutoModePolicy,
        now_ms: u64,
    ) -> Result<Self, AutoModeError> {
        if goal.status != AgentGoalStatus::Active
            || goal.id != plan.goal_id
            || goal.current_plan_revision != plan.revision
        {
            return Err(AutoModeError::InvalidGoalBinding);
        }
        policy.validate()?;
        Ok(Self {
            spec: "nimora.auto-mode-session/1".to_owned(),
            id: Uuid::now_v7(),
            goal_id: goal.id,
            plan_revision: plan.revision,
            policy_fingerprint: policy.fingerprint(),
            policy,
            status: AutoModeStatus::Running,
            pause_reason: None,
            usage: AutoModeUsage::default(),
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
        })
    }

    /// Evaluates one proposed step without granting any new capability.
    ///
    /// # Errors
    ///
    /// Returns an error when the session or request is structurally invalid.
    pub fn evaluate_step(
        &mut self,
        request: &AutoModeStepRequest,
        now_ms: u64,
    ) -> Result<AutoModeStepDecision, AutoModeError> {
        self.evaluate_step_with_grant(request, None, None, None, now_ms)
    }

    /// Evaluates one proposed step, optionally auto-approving in-scope grant work.
    ///
    /// When `grant` authorizes with never-ask-within-grant, risk/effect confirmation pauses are
    /// skipped. Goal, workspace, policy allowlist, data class, and budget guards still fail closed.
    ///
    /// # Errors
    ///
    /// Returns an error when the session or request is structurally invalid.
    pub fn evaluate_step_with_grant(
        &mut self,
        request: &AutoModeStepRequest,
        grant: Option<&AuthorizationGrant>,
        provider_id: Option<&str>,
        model: Option<&str>,
        now_ms: u64,
    ) -> Result<AutoModeStepDecision, AutoModeError> {
        self.validate()?;
        if self.status != AutoModeStatus::Running || now_ms < self.updated_at_ms {
            return Err(AutoModeError::InvalidTransition);
        }
        let mut reason =
            if request.goal_id != self.goal_id || request.plan_revision != self.plan_revision {
                Some(AutoModePauseReason::GoalChanged)
            } else if request.workspace_revision != self.policy.workspace_revision {
                Some(AutoModePauseReason::WorkspaceChanged)
            } else if request.data_classification > self.policy.maximum_data_classification
                || request
                    .tool_id
                    .as_ref()
                    .is_some_and(|tool| !self.policy.tool_allowlist.contains(tool))
            {
                Some(AutoModePauseReason::UnsafeEffect)
            } else if requires_confirmation(request.risk, request.effect) {
                grant_confirmation_reason(grant, request, provider_id, model, now_ms)
            } else {
                None
            };
        if reason.is_none() && exceeds_budget(&self.policy, self.usage, request.projected_usage) {
            reason = Some(AutoModePauseReason::BudgetExhausted);
        }
        if let Some(reason) = reason {
            self.pause(reason, now_ms)?;
            return Ok(AutoModeStepDecision::Pause(reason));
        }
        self.usage = add_usage(self.usage, request.projected_usage)?;
        self.updated_at_ms = now_ms;
        Ok(AutoModeStepDecision::Proceed)
    }

    /// Pauses a running session with a stable machine-readable reason.
    ///
    /// # Errors
    ///
    /// Returns an error when the session is not running or time moves backwards.
    pub fn pause(&mut self, reason: AutoModePauseReason, now_ms: u64) -> Result<(), AutoModeError> {
        if self.status != AutoModeStatus::Running || now_ms < self.updated_at_ms {
            return Err(AutoModeError::InvalidTransition);
        }
        self.status = AutoModeStatus::Paused;
        self.pause_reason = Some(reason);
        self.updated_at_ms = now_ms;
        Ok(())
    }

    /// Resumes only when Goal, Plan, workspace, and policy fingerprint still match.
    ///
    /// # Errors
    ///
    /// Returns an error when any bound resource changed or the session is not paused.
    pub fn resume(
        &mut self,
        goal: &AgentGoal,
        plan: &AgentPlan,
        workspace_revision: &str,
        policy_fingerprint: &str,
        now_ms: u64,
    ) -> Result<(), AutoModeError> {
        if self.status != AutoModeStatus::Paused
            || goal.status != AgentGoalStatus::Active
            || goal.id != self.goal_id
            || plan.goal_id != self.goal_id
            || plan.revision != self.plan_revision
            || workspace_revision != self.policy.workspace_revision
            || policy_fingerprint != self.policy_fingerprint
            || now_ms < self.updated_at_ms
        {
            return Err(AutoModeError::ResumeBindingChanged);
        }
        self.status = AutoModeStatus::Running;
        self.pause_reason = None;
        self.updated_at_ms = now_ms;
        Ok(())
    }

    /// Converts a persisted running session into a safe restart pause.
    ///
    /// # Errors
    ///
    /// Returns an error unless the session was running and time is monotonic.
    pub fn pause_after_restart(&mut self, now_ms: u64) -> Result<(), AutoModeError> {
        self.pause(AutoModePauseReason::Restarted, now_ms)
    }

    /// Cancels a running or paused session without executing another step.
    ///
    /// # Errors
    ///
    /// Returns an error for terminal sessions or a non-monotonic timestamp.
    pub fn cancel(&mut self, now_ms: u64) -> Result<(), AutoModeError> {
        if matches!(
            self.status,
            AutoModeStatus::Completed | AutoModeStatus::Cancelled
        ) || now_ms < self.updated_at_ms
        {
            return Err(AutoModeError::InvalidTransition);
        }
        self.status = AutoModeStatus::Cancelled;
        self.pause_reason = None;
        self.updated_at_ms = now_ms;
        Ok(())
    }

    /// Completes a running session after its terminal Provider result is durable-ready.
    ///
    /// # Errors
    ///
    /// Returns an error unless the session is running and time is monotonic.
    pub fn complete(&mut self, now_ms: u64) -> Result<(), AutoModeError> {
        if self.status != AutoModeStatus::Running || now_ms < self.updated_at_ms {
            return Err(AutoModeError::InvalidTransition);
        }
        self.status = AutoModeStatus::Completed;
        self.pause_reason = None;
        self.updated_at_ms = now_ms;
        Ok(())
    }

    /// Validates a session restored across a persistence boundary.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid specs, policy fingerprints, lifecycle, or usage.
    pub fn validate(&self) -> Result<(), AutoModeError> {
        self.policy.validate()?;
        if self.spec != "nimora.auto-mode-session/1"
            || self.plan_revision == 0
            || self.policy_fingerprint != self.policy.fingerprint()
            || self.updated_at_ms < self.created_at_ms
            || (self.status == AutoModeStatus::Paused) != self.pause_reason.is_some()
            || exceeds_budget(&self.policy, AutoModeUsage::default(), self.usage)
        {
            return Err(AutoModeError::InvalidSession);
        }
        Ok(())
    }
}

fn requires_confirmation(risk: CommandRisk, effect: ToolEffect) -> bool {
    matches!(
        risk,
        CommandRisk::Medium | CommandRisk::High | CommandRisk::Critical
    ) || !matches!(effect, ToolEffect::ReadOnly)
}

fn grant_confirmation_reason(
    grant: Option<&AuthorizationGrant>,
    request: &AutoModeStepRequest,
    provider_id: Option<&str>,
    model: Option<&str>,
    now_ms: u64,
) -> Option<AutoModePauseReason> {
    let Some(grant) = grant else {
        return Some(AutoModePauseReason::ConfirmationRequired);
    };
    let Some(tool_id) = request.tool_id.as_ref() else {
        return Some(AutoModePauseReason::ConfirmationRequired);
    };
    let provider_id =
        provider_id.or_else(|| grant.provider_allowlist.iter().next().map(String::as_str));
    let model = model.or_else(|| grant.model_allowlist.iter().next().map(String::as_str));
    let (Some(provider_id), Some(model)) = (provider_id, model) else {
        return Some(AutoModePauseReason::ConfirmationRequired);
    };
    let auth_request = AuthorizationRequest {
        goal_id: request.goal_id,
        plan_revision: request.plan_revision,
        workspace_fingerprint: &request.workspace_revision,
        tool_id,
        provider_id,
        model,
        data_classification: request.data_classification,
        requires_network: false,
        now_ms,
    };
    match grant.authorize(&auth_request) {
        Ok(AuthorizationDecision::Authorized) => None,
        Ok(AuthorizationDecision::ApprovalRequired) => {
            Some(AutoModePauseReason::ConfirmationRequired)
        }
        Err(AuthorizationError::OutOfScope) => Some(AutoModePauseReason::UnsafeEffect),
        Err(AuthorizationError::BindingChanged) => {
            if grant.goal_id != request.goal_id || grant.plan_revision != request.plan_revision {
                Some(AutoModePauseReason::GoalChanged)
            } else {
                Some(AutoModePauseReason::WorkspaceChanged)
            }
        }
        Err(
            AuthorizationError::Expired
            | AuthorizationError::Revoked
            | AuthorizationError::InvalidGrant,
        ) => Some(AutoModePauseReason::ConfirmationRequired),
    }
}

fn exceeds_budget(
    policy: &AutoModePolicy,
    current: AutoModeUsage,
    projected: AutoModeUsage,
) -> bool {
    let Ok(total) = add_usage(current, projected) else {
        return true;
    };
    total.cycles > policy.max_cycles
        || total.cycles > policy.budget.max_steps
        || total.tool_calls > policy.budget.max_tool_calls
        || total.elapsed_ms > policy.budget.max_elapsed_ms
        || total.input_tokens > policy.budget.max_input_tokens
        || total.output_tokens > policy.budget.max_output_tokens
        || total.cost_microunits > policy.budget.max_cost_microunits
}

fn add_usage(left: AutoModeUsage, right: AutoModeUsage) -> Result<AutoModeUsage, AutoModeError> {
    Ok(AutoModeUsage {
        cycles: left
            .cycles
            .checked_add(right.cycles)
            .ok_or(AutoModeError::UsageOverflow)?,
        tool_calls: left
            .tool_calls
            .checked_add(right.tool_calls)
            .ok_or(AutoModeError::UsageOverflow)?,
        elapsed_ms: left
            .elapsed_ms
            .checked_add(right.elapsed_ms)
            .ok_or(AutoModeError::UsageOverflow)?,
        input_tokens: left
            .input_tokens
            .checked_add(right.input_tokens)
            .ok_or(AutoModeError::UsageOverflow)?,
        output_tokens: left
            .output_tokens
            .checked_add(right.output_tokens)
            .ok_or(AutoModeError::UsageOverflow)?,
        cost_microunits: left
            .cost_microunits
            .checked_add(right.cost_microunits)
            .ok_or(AutoModeError::UsageOverflow)?,
    })
}

#[derive(Debug, Error)]
pub enum AutoModeError {
    #[error("Auto Mode policy is invalid")]
    InvalidPolicy,
    #[error("Auto Mode Goal binding is invalid")]
    InvalidGoalBinding,
    #[error("Auto Mode session is invalid")]
    InvalidSession,
    #[error("Auto Mode transition is invalid")]
    InvalidTransition,
    #[error("Auto Mode resume binding changed")]
    ResumeBindingChanged,
    #[error("Auto Mode usage overflowed")]
    UsageOverflow,
    #[error(transparent)]
    Runtime(#[from] AgentRuntimeError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentGoal, AgentPlan, AgentPlanStep, ApprovalPolicy};

    fn fixture() -> (AgentGoal, AgentPlan, AutoModePolicy) {
        let plan = AgentPlan::new(
            Uuid::now_v7(),
            vec![AgentPlanStep::new("Read state").expect("step")],
            "Initial",
            1_000,
        )
        .expect("plan");
        let goal = AgentGoal::new("Auto", "Run safely", &plan, 1_000).expect("goal");
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
        (goal, plan, policy)
    }

    #[test]
    fn safe_read_proceeds_but_write_pauses_for_confirmation() {
        let (goal, plan, policy) = fixture();
        let mut session = AutoModeSession::start(&goal, &plan, policy, 1_000).expect("session");
        let mut request = AutoModeStepRequest {
            goal_id: goal.id,
            plan_revision: 1,
            workspace_revision: "git:abc".to_owned(),
            tool_id: Some("pet.state.read".parse().expect("tool")),
            risk: CommandRisk::Safe,
            effect: ToolEffect::ReadOnly,
            data_classification: DataClassification::Personal,
            projected_usage: AutoModeUsage {
                cycles: 1,
                ..AutoModeUsage::default()
            },
        };
        assert_eq!(
            session.evaluate_step(&request, 1_001).expect("evaluate"),
            AutoModeStepDecision::Proceed
        );
        request.effect = ToolEffect::ReversibleWrite;
        assert_eq!(
            session.evaluate_step(&request, 1_002).expect("evaluate"),
            AutoModeStepDecision::Pause(AutoModePauseReason::ConfirmationRequired)
        );
    }

    #[test]
    fn resume_rejects_changed_workspace_plan_or_policy() {
        let (goal, plan, policy) = fixture();
        let mut session = AutoModeSession::start(&goal, &plan, policy, 1_000).expect("session");
        session
            .pause(AutoModePauseReason::UserRequested, 1_001)
            .expect("pause");
        let fingerprint = session.policy_fingerprint.clone();
        assert!(session
            .resume(&goal, &plan, "git:changed", &fingerprint, 1_002)
            .is_err());
    }

    #[test]
    fn unknown_tools_and_budget_overruns_pause_fail_closed() {
        let (goal, plan, policy) = fixture();
        let mut session =
            AutoModeSession::start(&goal, &plan, policy.clone(), 1_000).expect("session");
        let request = AutoModeStepRequest {
            goal_id: goal.id,
            plan_revision: plan.revision,
            workspace_revision: "git:abc".to_owned(),
            tool_id: Some("unknown.state.read".parse().expect("tool")),
            risk: CommandRisk::Safe,
            effect: ToolEffect::ReadOnly,
            data_classification: DataClassification::Public,
            projected_usage: AutoModeUsage {
                cycles: 1,
                tool_calls: 1,
                ..AutoModeUsage::default()
            },
        };
        assert_eq!(
            session.evaluate_step(&request, 1_001).expect("evaluate"),
            AutoModeStepDecision::Pause(AutoModePauseReason::UnsafeEffect)
        );

        let mut session = AutoModeSession::start(&goal, &plan, policy, 1_000).expect("session");
        let request = AutoModeStepRequest {
            tool_id: Some("pet.state.read".parse().expect("tool")),
            projected_usage: AutoModeUsage {
                cycles: 5,
                ..AutoModeUsage::default()
            },
            ..request
        };
        assert_eq!(
            session.evaluate_step(&request, 1_001).expect("evaluate"),
            AutoModeStepDecision::Pause(AutoModePauseReason::BudgetExhausted)
        );
    }

    fn workspace_fingerprint() -> String {
        format!("sha256:{}", "a".repeat(64))
    }

    fn test_grant(
        goal_id: Uuid,
        plan_revision: u64,
        approval: ApprovalPolicy,
        tools: &[&str],
        expires_at_ms: Option<u64>,
        revoked_at_ms: Option<u64>,
    ) -> AuthorizationGrant {
        use crate::{GrantLifetime, NetworkPolicy, SandboxScope};
        AuthorizationGrant {
            spec: "nimora.authorization-grant/1".to_owned(),
            id: Uuid::now_v7(),
            goal_id,
            plan_revision,
            workspace_fingerprint: workspace_fingerprint(),
            sandbox: SandboxScope::WorkspaceWrite,
            approval,
            network: NetworkPolicy::Offline,
            selected_roots: BTreeSet::new(),
            tool_allowlist: tools
                .iter()
                .map(|tool| tool.parse().expect("tool"))
                .collect(),
            provider_allowlist: BTreeSet::from(["provider:local".to_owned()]),
            model_allowlist: BTreeSet::from(["model:local".to_owned()]),
            maximum_data_classification: DataClassification::Personal,
            budget: AgentBudget::default(),
            lifetime: if expires_at_ms.is_some() {
                GrantLifetime::UntilTimestamp
            } else {
                GrantLifetime::Session
            },
            issued_at_ms: 900,
            expires_at_ms,
            revoked_at_ms,
        }
    }

    fn grant_fixture() -> (AgentGoal, AgentPlan, AutoModePolicy) {
        let plan = AgentPlan::new(
            Uuid::now_v7(),
            vec![AgentPlanStep::new("Write state").expect("step")],
            "Initial",
            1_000,
        )
        .expect("plan");
        let goal = AgentGoal::new("Auto", "Run unattended", &plan, 1_000).expect("goal");
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
            ["pet.test.write".to_owned(), "pet.test.read".to_owned()],
            workspace_fingerprint(),
        )
        .expect("policy");
        (goal, plan, policy)
    }

    #[test]
    fn never_ask_grant_allows_write_effect_without_confirmation() {
        let (goal, plan, policy) = grant_fixture();
        let mut session = AutoModeSession::start(&goal, &plan, policy, 1_000).expect("session");
        let grant = test_grant(
            goal.id,
            plan.revision,
            ApprovalPolicy::NeverAskWithinGrant,
            &["pet.test.write"],
            None,
            None,
        );
        let request = AutoModeStepRequest {
            goal_id: goal.id,
            plan_revision: plan.revision,
            workspace_revision: workspace_fingerprint(),
            tool_id: Some("pet.test.write".parse().expect("tool")),
            risk: CommandRisk::Safe,
            effect: ToolEffect::ReversibleWrite,
            data_classification: DataClassification::Personal,
            projected_usage: AutoModeUsage {
                cycles: 1,
                tool_calls: 1,
                ..AutoModeUsage::default()
            },
        };
        assert_eq!(
            session
                .evaluate_step_with_grant(
                    &request,
                    Some(&grant),
                    Some("provider:local"),
                    Some("model:local"),
                    1_001,
                )
                .expect("evaluate"),
            AutoModeStepDecision::Proceed
        );
    }

    #[test]
    fn always_ask_grant_still_pauses_for_write_effect() {
        let (goal, plan, policy) = grant_fixture();
        let mut session = AutoModeSession::start(&goal, &plan, policy, 1_000).expect("session");
        let grant = test_grant(
            goal.id,
            plan.revision,
            ApprovalPolicy::AlwaysAsk,
            &["pet.test.write"],
            None,
            None,
        );
        let request = AutoModeStepRequest {
            goal_id: goal.id,
            plan_revision: plan.revision,
            workspace_revision: workspace_fingerprint(),
            tool_id: Some("pet.test.write".parse().expect("tool")),
            risk: CommandRisk::Safe,
            effect: ToolEffect::ReversibleWrite,
            data_classification: DataClassification::Personal,
            projected_usage: AutoModeUsage {
                cycles: 1,
                tool_calls: 1,
                ..AutoModeUsage::default()
            },
        };
        assert_eq!(
            session
                .evaluate_step_with_grant(
                    &request,
                    Some(&grant),
                    Some("provider:local"),
                    Some("model:local"),
                    1_001,
                )
                .expect("evaluate"),
            AutoModeStepDecision::Pause(AutoModePauseReason::ConfirmationRequired)
        );
    }

    #[test]
    fn expired_or_revoked_grant_still_pauses_for_confirmation() {
        let (goal, plan, policy) = grant_fixture();
        let mut session =
            AutoModeSession::start(&goal, &plan, policy.clone(), 1_000).expect("session");
        let expired = test_grant(
            goal.id,
            plan.revision,
            ApprovalPolicy::NeverAskWithinGrant,
            &["pet.test.write"],
            Some(1_000),
            None,
        );
        let request = AutoModeStepRequest {
            goal_id: goal.id,
            plan_revision: plan.revision,
            workspace_revision: workspace_fingerprint(),
            tool_id: Some("pet.test.write".parse().expect("tool")),
            risk: CommandRisk::Medium,
            effect: ToolEffect::ReadOnly,
            data_classification: DataClassification::Personal,
            projected_usage: AutoModeUsage {
                cycles: 1,
                ..AutoModeUsage::default()
            },
        };
        assert_eq!(
            session
                .evaluate_step_with_grant(
                    &request,
                    Some(&expired),
                    Some("provider:local"),
                    Some("model:local"),
                    1_001,
                )
                .expect("evaluate"),
            AutoModeStepDecision::Pause(AutoModePauseReason::ConfirmationRequired)
        );

        let mut session =
            AutoModeSession::start(&goal, &plan, policy.clone(), 1_000).expect("session");
        let revoked = test_grant(
            goal.id,
            plan.revision,
            ApprovalPolicy::NeverAskWithinGrant,
            &["pet.test.write"],
            None,
            Some(1_000),
        );
        assert_eq!(
            session
                .evaluate_step_with_grant(
                    &request,
                    Some(&revoked),
                    Some("provider:local"),
                    Some("model:local"),
                    1_001,
                )
                .expect("evaluate"),
            AutoModeStepDecision::Pause(AutoModePauseReason::ConfirmationRequired)
        );
    }

    #[test]
    fn grant_tool_outside_allowlist_pauses_as_unsafe_effect() {
        let (goal, plan, policy) = grant_fixture();
        let mut session = AutoModeSession::start(&goal, &plan, policy, 1_000).expect("session");
        let grant = test_grant(
            goal.id,
            plan.revision,
            ApprovalPolicy::NeverAskWithinGrant,
            &["pet.test.read"],
            None,
            None,
        );
        let request = AutoModeStepRequest {
            goal_id: goal.id,
            plan_revision: plan.revision,
            workspace_revision: workspace_fingerprint(),
            tool_id: Some("pet.test.write".parse().expect("tool")),
            risk: CommandRisk::Safe,
            effect: ToolEffect::ReversibleWrite,
            data_classification: DataClassification::Personal,
            projected_usage: AutoModeUsage {
                cycles: 1,
                tool_calls: 1,
                ..AutoModeUsage::default()
            },
        };
        assert_eq!(
            session
                .evaluate_step_with_grant(
                    &request,
                    Some(&grant),
                    Some("provider:local"),
                    Some("model:local"),
                    1_001,
                )
                .expect("evaluate"),
            AutoModeStepDecision::Pause(AutoModePauseReason::UnsafeEffect)
        );
    }

    #[test]
    fn medium_risk_without_grant_still_pauses() {
        let (goal, plan, policy) = grant_fixture();
        let mut session = AutoModeSession::start(&goal, &plan, policy, 1_000).expect("session");
        let request = AutoModeStepRequest {
            goal_id: goal.id,
            plan_revision: plan.revision,
            workspace_revision: workspace_fingerprint(),
            tool_id: Some("pet.test.read".parse().expect("tool")),
            risk: CommandRisk::Medium,
            effect: ToolEffect::ReadOnly,
            data_classification: DataClassification::Personal,
            projected_usage: AutoModeUsage {
                cycles: 1,
                ..AutoModeUsage::default()
            },
        };
        assert_eq!(
            session.evaluate_step(&request, 1_001).expect("evaluate"),
            AutoModeStepDecision::Pause(AutoModePauseReason::ConfirmationRequired)
        );
    }
}
