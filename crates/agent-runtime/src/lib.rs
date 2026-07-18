use nimora_runtime_core::CommandRisk;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fmt, str::FromStr, time::Duration};
use thiserror::Error;
use uuid::Uuid;

mod auto_execution;
mod auto_mode;
mod checkpoint;
mod context_management;
mod coordinator;
mod deterministic;
mod goal;
mod provider;
mod task_gateway;
mod workspace;

pub use auto_execution::{AutoModeTurnError, AutoModeTurnOutcome, AutoModeTurnSupervisor};
pub use auto_mode::{
    AutoModeError, AutoModePauseReason, AutoModePolicy, AutoModeSession, AutoModeStatus,
    AutoModeStepDecision, AutoModeStepRequest, AutoModeUsage,
};
pub use checkpoint::{AutoModeCheckpoint, AutoModeCheckpointError};
pub use context_management::{
    CompactedContext, ContextAnchor, ContextCache, ContextCompactionPolicy, ContextCompactor,
    ContextManagementError,
};
pub use coordinator::{
    AgentCoordinator, CoordinatorError, PlannedToolCall, ProviderStepInput, ProviderStepOutcome,
    ProviderToolTurn, ToolStepOutcome,
};
pub use deterministic::DeterministicLocalProvider;
pub use goal::{
    AgentGoal, AgentGoalError, AgentGoalStatus, AgentPlan, AgentPlanStep, AgentPlanStepStatus,
};
pub use provider::{
    CancellationFlag, ProviderAdapter, ProviderCapabilities, ProviderCapability,
    ProviderDataPreview, ProviderDescriptor, ProviderError, ProviderErrorKind,
    ProviderExecutionContext, ProviderFinishReason, ProviderLocality, ProviderMessage,
    ProviderMessageRole, ProviderRegistry, ProviderRequest, ProviderResponse, ProviderToolCall,
    ProviderUsage,
};
pub use task_gateway::{
    AgentAutonomy, AgentTaskAdmission, AgentTaskGateway, AgentTaskGatewayPolicy, AgentTaskParent,
    AgentTaskRequest,
};
pub use workspace::{
    TrackedWorkspaceFile, WorkspaceChangeSet, WorkspaceFileChange, WorkspaceSnapshot,
    WorkspaceTrackingError,
};

const MAX_TOOLS: usize = 512;
const MAX_TOOL_ID_BYTES: usize = 128;
const MAX_SCHEMA_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTaskOrigin {
    Desktop,
    Cli,
    Automation,
    Module,
    Event,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTaskStatus {
    Pending,
    Planning,
    WaitingForConfirmation,
    Running,
    Paused,
    Succeeded,
    Failed,
    Cancelled,
    BudgetExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentBudget {
    pub max_steps: u32,
    pub max_tool_calls: u32,
    pub max_elapsed_ms: u64,
    pub max_input_tokens: u64,
    pub max_output_tokens: u64,
    pub max_cost_microunits: u64,
}

impl Default for AgentBudget {
    fn default() -> Self {
        Self {
            max_steps: 24,
            max_tool_calls: 16,
            max_elapsed_ms: 10 * 60 * 1000,
            max_input_tokens: 64_000,
            max_output_tokens: 16_000,
            max_cost_microunits: 5_000_000,
        }
    }
}

impl AgentBudget {
    fn validate(self) -> Result<Self, AgentRuntimeError> {
        if self.max_steps == 0
            || self.max_steps > 256
            || self.max_tool_calls > self.max_steps
            || self.max_elapsed_ms == 0
            || self.max_elapsed_ms > 24 * 60 * 60 * 1000
            || self.max_input_tokens == 0
            || self.max_output_tokens == 0
        {
            return Err(AgentRuntimeError::InvalidTaskBudget);
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentUsage {
    pub steps: u32,
    pub tool_calls: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_microunits: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentTask {
    pub spec: String,
    pub id: Uuid,
    pub trace_id: Uuid,
    pub origin: AgentTaskOrigin,
    pub requester: String,
    pub provider_id: String,
    pub status: AgentTaskStatus,
    pub budget: AgentBudget,
    pub usage: AgentUsage,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

impl AgentTask {
    /// Creates a bounded Agent task without persisting prompt content in task metadata.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid requester/provider identifiers or unsafe budgets.
    pub fn new(
        origin: AgentTaskOrigin,
        requester: impl Into<String>,
        provider_id: impl Into<String>,
        budget: AgentBudget,
        now_ms: u64,
    ) -> Result<Self, AgentRuntimeError> {
        let requester = requester.into();
        let provider_id = provider_id.into();
        if !valid_principal(&requester) || !valid_principal(&provider_id) {
            return Err(AgentRuntimeError::InvalidTaskIdentity);
        }
        Ok(Self {
            spec: "nimora.agent-task/1".to_owned(),
            id: Uuid::now_v7(),
            trace_id: Uuid::now_v7(),
            origin,
            requester,
            provider_id,
            status: AgentTaskStatus::Pending,
            budget: budget.validate()?,
            usage: AgentUsage::default(),
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
        })
    }

    /// Validates task metadata restored across a persistence or process boundary.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identities, budgets, usage, specs, or timestamps.
    pub fn validate(&self) -> Result<(), AgentRuntimeError> {
        if self.spec != "nimora.agent-task/1"
            || !valid_principal(&self.requester)
            || !valid_principal(&self.provider_id)
            || self.updated_at_ms < self.created_at_ms
        {
            return Err(AgentRuntimeError::InvalidTaskIdentity);
        }
        self.budget.validate()?;
        if self.usage.steps > self.budget.max_steps
            || self.usage.tool_calls > self.budget.max_tool_calls
            || self.usage.input_tokens > self.budget.max_input_tokens
            || self.usage.output_tokens > self.budget.max_output_tokens
            || self.usage.cost_microunits > self.budget.max_cost_microunits
        {
            return Err(AgentRuntimeError::TaskBudgetExhausted);
        }
        Ok(())
    }

    /// Transitions the task into an active lifecycle state.
    ///
    /// # Errors
    ///
    /// Returns an error when a terminal task is resumed or the transition is invalid.
    pub fn transition(
        &mut self,
        next: AgentTaskStatus,
        now_ms: u64,
    ) -> Result<(), AgentRuntimeError> {
        if !valid_transition(self.status, next) {
            return Err(AgentRuntimeError::InvalidTaskTransition);
        }
        self.ensure_elapsed(now_ms)?;
        self.status = next;
        self.updated_at_ms = now_ms;
        Ok(())
    }

    /// Accounts one Provider response and enforces token, cost, step, and elapsed budgets.
    ///
    /// # Errors
    ///
    /// Returns an exhausted-budget error and permanently stops the task when any limit is crossed.
    pub fn account_provider_step(
        &mut self,
        input_tokens: u64,
        output_tokens: u64,
        cost_microunits: u64,
        now_ms: u64,
    ) -> Result<(), AgentRuntimeError> {
        self.ensure_active(now_ms)?;
        self.usage.steps = self.usage.steps.saturating_add(1);
        self.usage.input_tokens = self.usage.input_tokens.saturating_add(input_tokens);
        self.usage.output_tokens = self.usage.output_tokens.saturating_add(output_tokens);
        self.usage.cost_microunits = self.usage.cost_microunits.saturating_add(cost_microunits);
        self.updated_at_ms = now_ms;
        self.enforce_usage()
    }

    /// Reserves one tool call before dispatching it through the Tool Registry.
    ///
    /// # Errors
    ///
    /// Returns an exhausted-budget error without invoking the tool when the limit is crossed.
    pub fn reserve_tool_call(&mut self, now_ms: u64) -> Result<(), AgentRuntimeError> {
        self.ensure_active(now_ms)?;
        self.usage.tool_calls = self.usage.tool_calls.saturating_add(1);
        self.updated_at_ms = now_ms;
        self.enforce_usage()
    }

    pub fn cancel(&mut self, now_ms: u64) {
        if !is_terminal(self.status) {
            self.status = AgentTaskStatus::Cancelled;
            self.updated_at_ms = now_ms;
        }
    }

    fn ensure_active(&mut self, now_ms: u64) -> Result<(), AgentRuntimeError> {
        self.ensure_elapsed(now_ms)?;
        if !matches!(
            self.status,
            AgentTaskStatus::Planning | AgentTaskStatus::Running
        ) {
            return Err(AgentRuntimeError::TaskNotActive);
        }
        Ok(())
    }

    fn ensure_elapsed(&mut self, now_ms: u64) -> Result<(), AgentRuntimeError> {
        if now_ms.saturating_sub(self.created_at_ms) > self.budget.max_elapsed_ms {
            self.status = AgentTaskStatus::BudgetExhausted;
            self.updated_at_ms = now_ms;
            return Err(AgentRuntimeError::TaskBudgetExhausted);
        }
        Ok(())
    }

    fn enforce_usage(&mut self) -> Result<(), AgentRuntimeError> {
        if self.usage.steps > self.budget.max_steps
            || self.usage.tool_calls > self.budget.max_tool_calls
            || self.usage.input_tokens > self.budget.max_input_tokens
            || self.usage.output_tokens > self.budget.max_output_tokens
            || self.usage.cost_microunits > self.budget.max_cost_microunits
        {
            self.status = AgentTaskStatus::BudgetExhausted;
            return Err(AgentRuntimeError::TaskBudgetExhausted);
        }
        Ok(())
    }
}

fn valid_principal(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | ':' | '_' | '-' | '/')
        })
}

const fn is_terminal(status: AgentTaskStatus) -> bool {
    matches!(
        status,
        AgentTaskStatus::Succeeded
            | AgentTaskStatus::Failed
            | AgentTaskStatus::Cancelled
            | AgentTaskStatus::BudgetExhausted
    )
}

const fn valid_transition(current: AgentTaskStatus, next: AgentTaskStatus) -> bool {
    if is_terminal(current) {
        return false;
    }
    matches!(
        (current, next),
        (AgentTaskStatus::Pending, AgentTaskStatus::Planning)
            | (
                AgentTaskStatus::Planning | AgentTaskStatus::Running,
                AgentTaskStatus::WaitingForConfirmation
                    | AgentTaskStatus::Succeeded
                    | AgentTaskStatus::Failed
            )
            | (
                AgentTaskStatus::Planning
                    | AgentTaskStatus::WaitingForConfirmation
                    | AgentTaskStatus::Paused,
                AgentTaskStatus::Running
            )
            | (
                AgentTaskStatus::Planning
                    | AgentTaskStatus::WaitingForConfirmation
                    | AgentTaskStatus::Running,
                AgentTaskStatus::Cancelled
            )
            | (
                AgentTaskStatus::Running | AgentTaskStatus::WaitingForConfirmation,
                AgentTaskStatus::Paused
            )
    )
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ToolId(String);

impl ToolId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for ToolId {
    type Err = AgentRuntimeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let valid = !value.is_empty()
            && value.len() <= MAX_TOOL_ID_BYTES
            && value.split('.').count() >= 3
            && value.split('.').all(|segment| {
                !segment.is_empty()
                    && segment.chars().all(|character| {
                        character.is_ascii_lowercase()
                            || character.is_ascii_digit()
                            || character == '-'
                    })
            });
        valid
            .then(|| Self(value.to_owned()))
            .ok_or_else(|| AgentRuntimeError::InvalidToolId(value.to_owned()))
    }
}

impl fmt::Display for ToolId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataClassification {
    Public,
    Internal,
    Personal,
    Sensitive,
    Restricted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolEffect {
    ReadOnly,
    ReversibleWrite,
    IrreversibleWrite,
    ExternalSideEffect,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolDescriptor {
    pub spec: String,
    pub id: ToolId,
    pub title: String,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub base_risk: CommandRisk,
    pub effect: ToolEffect,
    pub data_classifications: Vec<DataClassification>,
    pub timeout_ms: u64,
    pub supports_cancellation: bool,
    pub idempotent: bool,
}

impl ToolDescriptor {
    /// Creates a validated tool descriptor owned by one module contribution.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid identifier, empty user-facing metadata,
    /// excessive schemas, or an invalid timeout.
    pub fn new(
        id: impl AsRef<str>,
        title: impl Into<String>,
        description: impl Into<String>,
        input_schema: Value,
        output_schema: Value,
        base_risk: CommandRisk,
        effect: ToolEffect,
    ) -> Result<Self, AgentRuntimeError> {
        let title = title.into();
        let description = description.into();
        if title.trim().is_empty() || description.trim().is_empty() {
            return Err(AgentRuntimeError::InvalidToolMetadata);
        }
        validate_schema(&input_schema)?;
        validate_schema(&output_schema)?;
        Ok(Self {
            spec: "nimora.agent-tool/1".to_owned(),
            id: id.as_ref().parse()?,
            title,
            description,
            input_schema,
            output_schema,
            base_risk,
            effect,
            data_classifications: Vec::new(),
            timeout_ms: 30_000,
            supports_cancellation: true,
            idempotent: false,
        })
    }
}

fn validate_schema(schema: &Value) -> Result<(), AgentRuntimeError> {
    if !schema.is_object()
        || serde_json::to_vec(schema)
            .map_err(|_| AgentRuntimeError::InvalidToolSchema)?
            .len()
            > MAX_SCHEMA_BYTES
    {
        return Err(AgentRuntimeError::InvalidToolSchema);
    }
    Ok(())
}

pub trait ToolRiskEvaluator: fmt::Debug + Send + Sync {
    fn evaluate(&self, descriptor: &ToolDescriptor, arguments: &Value) -> CommandRisk;
}

#[derive(Debug, Default)]
pub struct BaseRiskEvaluator;

impl ToolRiskEvaluator for BaseRiskEvaluator {
    fn evaluate(&self, descriptor: &ToolDescriptor, _arguments: &Value) -> CommandRisk {
        descriptor.base_risk
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolInvocation {
    pub spec: String,
    pub invocation_id: Uuid,
    pub task_id: Uuid,
    pub trace_id: Uuid,
    pub tool_id: ToolId,
    pub arguments: Value,
}

impl ToolInvocation {
    /// Creates a tool request correlated to one Agent task and trace.
    ///
    /// # Errors
    ///
    /// Returns an error when the tool identifier is invalid.
    pub fn new(
        task_id: Uuid,
        trace_id: Uuid,
        tool_id: impl AsRef<str>,
        arguments: Value,
    ) -> Result<Self, AgentRuntimeError> {
        Ok(Self {
            spec: "nimora.agent-tool-invocation/1".to_owned(),
            invocation_id: Uuid::now_v7(),
            task_id,
            trace_id,
            tool_id: tool_id.as_ref().parse()?,
            arguments,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolApproval {
    pub spec: String,
    pub invocation_id: Uuid,
    pub fingerprint: String,
}

impl ToolApproval {
    #[must_use]
    pub fn bind(invocation: &ToolInvocation, effective_risk: CommandRisk) -> Self {
        Self {
            spec: "nimora.agent-tool-approval/1".to_owned(),
            invocation_id: invocation.invocation_id,
            fingerprint: invocation_fingerprint(invocation, effective_risk),
        }
    }
}

pub trait ToolBackend: fmt::Debug + Send + Sync {
    /// Invokes a module capability without exposing the module implementation.
    ///
    /// # Errors
    ///
    /// Returns a stable backend error when the module rejects or cannot complete the call.
    fn invoke(
        &self,
        invocation: &ToolInvocation,
        descriptor: &ToolDescriptor,
        timeout: Duration,
    ) -> Result<Value, String>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToolAdmission {
    Ready {
        effective_risk: CommandRisk,
    },
    ConfirmationRequired {
        effective_risk: CommandRisk,
        fingerprint: String,
    },
}

#[derive(Debug)]
pub struct ToolRegistry<R = BaseRiskEvaluator> {
    descriptors: BTreeMap<ToolId, ToolDescriptor>,
    risk_evaluator: R,
}

impl Default for ToolRegistry<BaseRiskEvaluator> {
    fn default() -> Self {
        Self::new(BaseRiskEvaluator)
    }
}

impl<R: ToolRiskEvaluator> ToolRegistry<R> {
    #[must_use]
    pub const fn new(risk_evaluator: R) -> Self {
        Self {
            descriptors: BTreeMap::new(),
            risk_evaluator,
        }
    }

    /// Registers one module-owned Agent tool.
    ///
    /// # Errors
    ///
    /// Returns an error for duplicate tools or when the global tool budget is exhausted.
    pub fn register(&mut self, descriptor: ToolDescriptor) -> Result<(), AgentRuntimeError> {
        if self.descriptors.len() >= MAX_TOOLS {
            return Err(AgentRuntimeError::ToolBudgetExceeded);
        }
        if self.descriptors.contains_key(&descriptor.id) {
            return Err(AgentRuntimeError::DuplicateTool(descriptor.id.to_string()));
        }
        self.descriptors.insert(descriptor.id.clone(), descriptor);
        Ok(())
    }

    #[must_use]
    pub fn descriptor(&self, tool_id: &ToolId) -> Option<&ToolDescriptor> {
        self.descriptors.get(tool_id)
    }

    /// Evaluates a concrete invocation before any side effect occurs.
    ///
    /// # Errors
    ///
    /// Returns an error when the requested tool is not registered.
    pub fn admit(&self, invocation: &ToolInvocation) -> Result<ToolAdmission, AgentRuntimeError> {
        let descriptor = self
            .descriptors
            .get(&invocation.tool_id)
            .ok_or_else(|| AgentRuntimeError::UnknownTool(invocation.tool_id.to_string()))?;
        let effective_risk = max_risk(
            descriptor.base_risk,
            self.risk_evaluator
                .evaluate(descriptor, &invocation.arguments),
        );
        if requires_confirmation(descriptor.effect, effective_risk) {
            Ok(ToolAdmission::ConfirmationRequired {
                effective_risk,
                fingerprint: invocation_fingerprint(invocation, effective_risk),
            })
        } else {
            Ok(ToolAdmission::Ready { effective_risk })
        }
    }

    /// Dispatches an admitted invocation through the module capability backend.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown tools, missing or stale approval, invalid timeout,
    /// or a stable module backend failure.
    pub fn dispatch<B: ToolBackend>(
        &self,
        backend: &B,
        invocation: &ToolInvocation,
        approval: Option<&ToolApproval>,
    ) -> Result<Value, AgentRuntimeError> {
        let descriptor = self
            .descriptors
            .get(&invocation.tool_id)
            .ok_or_else(|| AgentRuntimeError::UnknownTool(invocation.tool_id.to_string()))?;
        let admission = self.admit(invocation)?;
        if let ToolAdmission::ConfirmationRequired {
            effective_risk,
            fingerprint,
        } = admission
        {
            let approval = approval.ok_or(AgentRuntimeError::ConfirmationRequired)?;
            if approval.invocation_id != invocation.invocation_id
                || approval.fingerprint != fingerprint
                || approval.fingerprint != invocation_fingerprint(invocation, effective_risk)
            {
                return Err(AgentRuntimeError::StaleApproval);
            }
        }
        if descriptor.timeout_ms == 0 || descriptor.timeout_ms > 300_000 {
            return Err(AgentRuntimeError::InvalidToolTimeout);
        }
        backend
            .invoke(
                invocation,
                descriptor,
                Duration::from_millis(descriptor.timeout_ms),
            )
            .map_err(AgentRuntimeError::Backend)
    }

    #[must_use]
    pub fn descriptors(&self) -> Vec<&ToolDescriptor> {
        self.descriptors.values().collect()
    }
}

fn requires_confirmation(effect: ToolEffect, risk: CommandRisk) -> bool {
    !matches!(effect, ToolEffect::ReadOnly)
        || matches!(
            risk,
            CommandRisk::Medium | CommandRisk::High | CommandRisk::Critical
        )
}

fn max_risk(left: CommandRisk, right: CommandRisk) -> CommandRisk {
    if risk_rank(left) >= risk_rank(right) {
        left
    } else {
        right
    }
}

const fn risk_rank(risk: CommandRisk) -> u8 {
    match risk {
        CommandRisk::Safe => 0,
        CommandRisk::Low => 1,
        CommandRisk::Medium => 2,
        CommandRisk::High => 3,
        CommandRisk::Critical => 4,
    }
}

fn invocation_fingerprint(invocation: &ToolInvocation, risk: CommandRisk) -> String {
    let mut digest = Sha256::new();
    digest.update(invocation.invocation_id.as_bytes());
    digest.update(invocation.task_id.as_bytes());
    digest.update(invocation.trace_id.as_bytes());
    digest.update(invocation.tool_id.to_string().as_bytes());
    digest.update([risk_rank(risk)]);
    digest.update(serde_json::to_vec(&invocation.arguments).unwrap_or_default());
    format!("sha256:{:x}", digest.finalize())
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AgentRuntimeError {
    #[error("task requester or provider identifier is invalid")]
    InvalidTaskIdentity,
    #[error("task budget is invalid")]
    InvalidTaskBudget,
    #[error("agent task request is not authorized by the caller policy")]
    TaskRequestNotAuthorized,
    #[error("agent task call depth is exhausted")]
    TaskCallDepthExceeded,
    #[error("agent task tool allowlist is invalid")]
    InvalidTaskToolAllowlist,
    #[error("task lifecycle transition is invalid")]
    InvalidTaskTransition,
    #[error("task is not active")]
    TaskNotActive,
    #[error("task budget is exhausted")]
    TaskBudgetExhausted,
    #[error("invalid tool id: {0}")]
    InvalidToolId(String),
    #[error("tool title and description are required")]
    InvalidToolMetadata,
    #[error("tool schema must be a bounded JSON object")]
    InvalidToolSchema,
    #[error("tool timeout is outside the allowed range")]
    InvalidToolTimeout,
    #[error("tool registry capacity is exhausted")]
    ToolBudgetExceeded,
    #[error("tool is already registered: {0}")]
    DuplicateTool(String),
    #[error("tool is not registered: {0}")]
    UnknownTool(String),
    #[error("tool invocation requires confirmation")]
    ConfirmationRequired,
    #[error("tool approval does not match the current invocation")]
    StaleApproval,
    #[error("tool backend failed: {0}")]
    Backend(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[derive(Debug)]
    struct Backend;

    impl ToolBackend for Backend {
        fn invoke(
            &self,
            invocation: &ToolInvocation,
            _descriptor: &ToolDescriptor,
            _timeout: Duration,
        ) -> Result<Value, String> {
            Ok(json!({"accepted": invocation.arguments}))
        }
    }

    #[derive(Debug)]
    struct ArgumentRisk;

    impl ToolRiskEvaluator for ArgumentRisk {
        fn evaluate(&self, descriptor: &ToolDescriptor, arguments: &Value) -> CommandRisk {
            if arguments.get("recursive") == Some(&Value::Bool(true)) {
                CommandRisk::Critical
            } else {
                descriptor.base_risk
            }
        }
    }

    fn descriptor(effect: ToolEffect) -> ToolDescriptor {
        ToolDescriptor::new(
            "core.files.inspect",
            "Inspect file",
            "Reads metadata through the file capability adapter.",
            json!({"type": "object"}),
            json!({"type": "object"}),
            CommandRisk::Safe,
            effect,
        )
        .expect("descriptor")
    }

    #[test]
    fn safe_read_only_tool_dispatches_without_confirmation() {
        let mut registry = ToolRegistry::default();
        registry
            .register(descriptor(ToolEffect::ReadOnly))
            .expect("register");
        let invocation = ToolInvocation::new(
            Uuid::now_v7(),
            Uuid::now_v7(),
            "core.files.inspect",
            json!({"pathRef": "selection:1"}),
        )
        .expect("invocation");

        assert!(matches!(
            registry.admit(&invocation),
            Ok(ToolAdmission::Ready {
                effective_risk: CommandRisk::Safe
            })
        ));
        assert_eq!(
            registry.dispatch(&Backend, &invocation, None),
            Ok(json!({"accepted": {"pathRef": "selection:1"}}))
        );
    }

    #[test]
    fn write_tool_requires_approval_bound_to_exact_arguments() {
        let mut registry = ToolRegistry::default();
        registry
            .register(descriptor(ToolEffect::ReversibleWrite))
            .expect("register");
        let invocation = ToolInvocation::new(
            Uuid::now_v7(),
            Uuid::now_v7(),
            "core.files.inspect",
            json!({"target": "profile:active"}),
        )
        .expect("invocation");
        let ToolAdmission::ConfirmationRequired { effective_risk, .. } =
            registry.admit(&invocation).expect("admission")
        else {
            panic!("write must require confirmation");
        };
        assert_eq!(
            registry.dispatch(&Backend, &invocation, None),
            Err(AgentRuntimeError::ConfirmationRequired)
        );

        let approval = ToolApproval::bind(&invocation, effective_risk);
        let mut changed = invocation.clone();
        changed.arguments = json!({"target": "profile:other"});
        assert_eq!(
            registry.dispatch(&Backend, &changed, Some(&approval)),
            Err(AgentRuntimeError::StaleApproval)
        );
        assert!(
            registry
                .dispatch(&Backend, &invocation, Some(&approval))
                .is_ok()
        );
    }

    #[test]
    fn argument_risk_can_raise_but_never_lower_manifest_risk() {
        let mut registry = ToolRegistry::new(ArgumentRisk);
        registry
            .register(descriptor(ToolEffect::ReadOnly))
            .expect("register");
        let invocation = ToolInvocation::new(
            Uuid::now_v7(),
            Uuid::now_v7(),
            "core.files.inspect",
            json!({"recursive": true}),
        )
        .expect("invocation");
        assert!(matches!(
            registry.admit(&invocation),
            Ok(ToolAdmission::ConfirmationRequired {
                effective_risk: CommandRisk::Critical,
                ..
            })
        ));
    }

    #[test]
    fn registry_rejects_duplicates_and_ambiguous_ids() {
        assert!(
            ToolDescriptor::new(
                "inspect",
                "Inspect",
                "Inspect",
                json!({}),
                json!({}),
                CommandRisk::Safe,
                ToolEffect::ReadOnly,
            )
            .is_err()
        );
        let mut registry = ToolRegistry::default();
        registry
            .register(descriptor(ToolEffect::ReadOnly))
            .expect("register");
        assert_eq!(
            registry.register(descriptor(ToolEffect::ReadOnly)),
            Err(AgentRuntimeError::DuplicateTool(
                "core.files.inspect".to_owned()
            ))
        );
    }

    #[test]
    fn task_budget_stops_provider_loop_before_unbounded_work() {
        let mut task = AgentTask::new(
            AgentTaskOrigin::Cli,
            "cli:local",
            "provider:local",
            AgentBudget {
                max_steps: 2,
                max_tool_calls: 1,
                max_elapsed_ms: 10_000,
                max_input_tokens: 100,
                max_output_tokens: 100,
                max_cost_microunits: 10,
            },
            1_000,
        )
        .expect("task");
        task.transition(AgentTaskStatus::Planning, 1_001)
            .expect("planning");
        task.account_provider_step(20, 10, 3, 1_002)
            .expect("first step");
        task.account_provider_step(20, 10, 3, 1_003)
            .expect("second step");
        assert_eq!(
            task.account_provider_step(20, 10, 3, 1_004),
            Err(AgentRuntimeError::TaskBudgetExhausted)
        );
        assert_eq!(task.status, AgentTaskStatus::BudgetExhausted);
        assert_eq!(
            task.reserve_tool_call(1_005),
            Err(AgentRuntimeError::TaskNotActive)
        );
    }

    #[test]
    fn tool_budget_is_reserved_before_dispatch_and_cancel_is_terminal() {
        let mut task = AgentTask::new(
            AgentTaskOrigin::Module,
            "module:calendar",
            "provider:remote",
            AgentBudget {
                max_tool_calls: 1,
                ..AgentBudget::default()
            },
            2_000,
        )
        .expect("task");
        task.transition(AgentTaskStatus::Planning, 2_001)
            .expect("planning");
        task.transition(AgentTaskStatus::Running, 2_002)
            .expect("running");
        task.reserve_tool_call(2_003).expect("first call");
        assert_eq!(
            task.reserve_tool_call(2_004),
            Err(AgentRuntimeError::TaskBudgetExhausted)
        );
        task.cancel(2_005);
        assert_eq!(task.status, AgentTaskStatus::BudgetExhausted);
        assert_eq!(
            task.transition(AgentTaskStatus::Running, 2_006),
            Err(AgentRuntimeError::InvalidTaskTransition)
        );
    }

    #[test]
    fn elapsed_budget_uses_saturating_clock_math() {
        let mut task = AgentTask::new(
            AgentTaskOrigin::Event,
            "event:pet-idle",
            "provider:local",
            AgentBudget {
                max_elapsed_ms: 100,
                ..AgentBudget::default()
            },
            5_000,
        )
        .expect("task");
        task.transition(AgentTaskStatus::Planning, 4_900)
            .expect("clock rollback does not exhaust task");
        assert_eq!(
            task.account_provider_step(1, 1, 0, 5_101),
            Err(AgentRuntimeError::TaskBudgetExhausted)
        );
        assert_eq!(task.status, AgentTaskStatus::BudgetExhausted);
    }
}
