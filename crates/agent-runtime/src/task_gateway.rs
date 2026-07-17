use super::{
    AgentBudget, AgentRuntimeError, AgentTask, AgentTaskOrigin, DataClassification, ToolId,
    valid_principal,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

const MAX_TASK_TOOLS: usize = 128;
const MAX_CALL_DEPTH: u8 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentAutonomy {
    Suggest,
    Draft,
    ConfirmEach,
    ApprovedPlan,
    PolicyBound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTaskGatewayPolicy {
    pub requester: String,
    pub origins: BTreeSet<AgentTaskOrigin>,
    pub providers: BTreeSet<String>,
    pub tools: BTreeSet<ToolId>,
    pub max_classification: DataClassification,
    pub max_autonomy: AgentAutonomy,
    pub max_budget: AgentBudget,
    pub max_call_depth: u8,
}

impl AgentTaskGatewayPolicy {
    /// Creates a fail-closed task creation policy for one module principal.
    ///
    /// # Errors
    ///
    /// Returns an error when identities, providers, limits, or tool IDs are invalid.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        requester: impl Into<String>,
        origins: impl IntoIterator<Item = AgentTaskOrigin>,
        providers: impl IntoIterator<Item = String>,
        tools: impl IntoIterator<Item = String>,
        max_classification: DataClassification,
        max_autonomy: AgentAutonomy,
        max_budget: AgentBudget,
        max_call_depth: u8,
    ) -> Result<Self, AgentRuntimeError> {
        let requester = requester.into();
        let origins = origins.into_iter().collect::<BTreeSet<_>>();
        let providers = providers.into_iter().collect::<BTreeSet<_>>();
        let tools = parse_tools(tools)?;
        if !valid_principal(&requester)
            || origins.is_empty()
            || providers.is_empty()
            || providers.iter().any(|provider| !valid_principal(provider))
            || max_call_depth == 0
            || max_call_depth > MAX_CALL_DEPTH
        {
            return Err(AgentRuntimeError::TaskRequestNotAuthorized);
        }
        max_budget.validate()?;
        Ok(Self {
            requester,
            origins,
            providers,
            tools,
            max_classification,
            max_autonomy,
            max_budget,
            max_call_depth,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentTaskRequest {
    pub spec: String,
    pub origin: AgentTaskOrigin,
    pub requester: String,
    pub provider_id: String,
    pub tool_allowlist: BTreeSet<String>,
    pub classification: DataClassification,
    pub autonomy: AgentAutonomy,
    pub budget: AgentBudget,
    pub parent: Option<AgentTaskParent>,
}

impl AgentTaskRequest {
    #[must_use]
    pub fn new(
        origin: AgentTaskOrigin,
        requester: impl Into<String>,
        provider_id: impl Into<String>,
        tool_allowlist: impl IntoIterator<Item = String>,
        classification: DataClassification,
        autonomy: AgentAutonomy,
        budget: AgentBudget,
    ) -> Self {
        Self {
            spec: "nimora.agent-task-request/1".to_owned(),
            origin,
            requester: requester.into(),
            provider_id: provider_id.into(),
            tool_allowlist: tool_allowlist.into_iter().collect(),
            classification,
            autonomy,
            budget,
            parent: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentTaskParent {
    pub root_task_id: Uuid,
    pub parent_task_id: Uuid,
    pub trace_id: Uuid,
    pub call_depth: u8,
    pub remaining_budget: AgentBudget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentTaskAdmission {
    pub spec: String,
    pub task: AgentTask,
    pub root_task_id: Uuid,
    pub parent_task_id: Option<Uuid>,
    pub call_depth: u8,
    pub tool_allowlist: BTreeSet<ToolId>,
    pub classification: DataClassification,
    pub autonomy: AgentAutonomy,
}

#[derive(Debug, Clone)]
pub struct AgentTaskGateway {
    policy: AgentTaskGatewayPolicy,
}

impl AgentTaskGateway {
    #[must_use]
    pub const fn new(policy: AgentTaskGatewayPolicy) -> Self {
        Self { policy }
    }

    /// Authorizes a module task and binds child work to the parent's remaining budget.
    ///
    /// # Errors
    ///
    /// Returns an error without creating a task when identity, provider, tools, data,
    /// autonomy, depth, or budget exceed the caller policy.
    pub fn admit(
        &self,
        request: AgentTaskRequest,
        now_ms: u64,
    ) -> Result<AgentTaskAdmission, AgentRuntimeError> {
        if request.spec != "nimora.agent-task-request/1"
            || request.requester != self.policy.requester
            || !self.policy.origins.contains(&request.origin)
            || !self.policy.providers.contains(&request.provider_id)
            || request.classification > self.policy.max_classification
            || request.autonomy > self.policy.max_autonomy
        {
            return Err(AgentRuntimeError::TaskRequestNotAuthorized);
        }
        let tools = parse_tools(request.tool_allowlist)?;
        if !tools.is_subset(&self.policy.tools) {
            return Err(AgentRuntimeError::TaskRequestNotAuthorized);
        }
        request.budget.validate()?;
        let budget = intersect_budget(request.budget, self.policy.max_budget);
        let (root_task_id, parent_task_id, trace_id, call_depth) = match request.parent {
            Some(parent) => {
                let call_depth = parent.call_depth.saturating_add(1);
                if call_depth > self.policy.max_call_depth {
                    return Err(AgentRuntimeError::TaskCallDepthExceeded);
                }
                (
                    Some(parent.root_task_id),
                    Some(parent.parent_task_id),
                    parent.trace_id,
                    call_depth,
                )
            }
            None => (None, None, Uuid::now_v7(), 0),
        };
        let budget = request.parent.map_or(budget, |parent| {
            intersect_budget(budget, parent.remaining_budget)
        });
        budget.validate()?;
        let mut task = AgentTask::new(
            request.origin,
            request.requester,
            request.provider_id,
            budget,
            now_ms,
        )?;
        task.trace_id = trace_id;
        Ok(AgentTaskAdmission {
            spec: "nimora.agent-task-admission/1".to_owned(),
            root_task_id: root_task_id.unwrap_or(task.id),
            parent_task_id,
            call_depth,
            task,
            tool_allowlist: tools,
            classification: request.classification,
            autonomy: request.autonomy,
        })
    }
}

fn parse_tools(
    tools: impl IntoIterator<Item = String>,
) -> Result<BTreeSet<ToolId>, AgentRuntimeError> {
    let values = tools.into_iter().collect::<Vec<_>>();
    if values.len() > MAX_TASK_TOOLS {
        return Err(AgentRuntimeError::InvalidTaskToolAllowlist);
    }
    values
        .into_iter()
        .map(|tool| tool.parse())
        .collect::<Result<BTreeSet<_>, _>>()
}

fn intersect_budget(left: AgentBudget, right: AgentBudget) -> AgentBudget {
    AgentBudget {
        max_steps: left.max_steps.min(right.max_steps),
        max_tool_calls: left.max_tool_calls.min(right.max_tool_calls),
        max_elapsed_ms: left.max_elapsed_ms.min(right.max_elapsed_ms),
        max_input_tokens: left.max_input_tokens.min(right.max_input_tokens),
        max_output_tokens: left.max_output_tokens.min(right.max_output_tokens),
        max_cost_microunits: left.max_cost_microunits.min(right.max_cost_microunits),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> AgentTaskGatewayPolicy {
        AgentTaskGatewayPolicy::new(
            "automation:daily-focus",
            [AgentTaskOrigin::Automation],
            ["provider:local".to_owned()],
            [
                "core.pet.state-read".to_owned(),
                "core.pet.action-play".to_owned(),
            ],
            DataClassification::Personal,
            AgentAutonomy::ConfirmEach,
            AgentBudget {
                max_steps: 8,
                max_tool_calls: 4,
                max_elapsed_ms: 60_000,
                max_input_tokens: 4_000,
                max_output_tokens: 1_000,
                max_cost_microunits: 0,
            },
            3,
        )
        .expect("policy")
    }

    fn request() -> AgentTaskRequest {
        AgentTaskRequest::new(
            AgentTaskOrigin::Automation,
            "automation:daily-focus",
            "provider:local",
            ["core.pet.state-read".to_owned()],
            DataClassification::Personal,
            AgentAutonomy::Draft,
            AgentBudget::default(),
        )
    }

    #[test]
    fn intersects_request_with_policy_budget() {
        let admitted = AgentTaskGateway::new(policy())
            .admit(request(), 1_000)
            .expect("admit");
        assert_eq!(admitted.task.budget.max_steps, 8);
        assert_eq!(admitted.task.budget.max_tool_calls, 4);
        assert_eq!(admitted.task.budget.max_cost_microunits, 0);
        assert_eq!(admitted.root_task_id, admitted.task.id);
        assert_eq!(admitted.call_depth, 0);
    }

    #[test]
    fn child_inherits_trace_and_cannot_reset_remaining_budget() {
        let mut request = request();
        let trace_id = Uuid::now_v7();
        let root_task_id = Uuid::now_v7();
        let parent_task_id = Uuid::now_v7();
        request.parent = Some(AgentTaskParent {
            root_task_id,
            parent_task_id,
            trace_id,
            call_depth: 1,
            remaining_budget: AgentBudget {
                max_steps: 2,
                max_tool_calls: 1,
                max_elapsed_ms: 5_000,
                max_input_tokens: 500,
                max_output_tokens: 200,
                max_cost_microunits: 0,
            },
        });
        let admitted = AgentTaskGateway::new(policy())
            .admit(request, 2_000)
            .expect("child");
        assert_eq!(admitted.root_task_id, root_task_id);
        assert_eq!(admitted.parent_task_id, Some(parent_task_id));
        assert_eq!(admitted.task.trace_id, trace_id);
        assert_eq!(admitted.call_depth, 2);
        assert_eq!(admitted.task.budget.max_steps, 2);
        assert_eq!(admitted.task.budget.max_tool_calls, 1);
    }

    #[test]
    fn rejects_provider_tool_identity_data_and_autonomy_escalation() {
        let gateway = AgentTaskGateway::new(policy());
        let mut cases = Vec::new();
        let mut provider = request();
        provider.provider_id = "provider:remote".to_owned();
        cases.push(provider);
        let mut tool = request();
        tool.tool_allowlist.insert("core.files.write".to_owned());
        cases.push(tool);
        let mut requester = request();
        requester.requester = "automation:other".to_owned();
        cases.push(requester);
        let mut data = request();
        data.classification = DataClassification::Sensitive;
        cases.push(data);
        let mut autonomy = request();
        autonomy.autonomy = AgentAutonomy::PolicyBound;
        cases.push(autonomy);
        for request in cases {
            assert_eq!(
                gateway.admit(request, 3_000),
                Err(AgentRuntimeError::TaskRequestNotAuthorized)
            );
        }
    }

    #[test]
    fn rejects_child_beyond_policy_depth() {
        let mut request = request();
        request.parent = Some(AgentTaskParent {
            root_task_id: Uuid::now_v7(),
            parent_task_id: Uuid::now_v7(),
            trace_id: Uuid::now_v7(),
            call_depth: 3,
            remaining_budget: AgentBudget::default(),
        });
        assert_eq!(
            AgentTaskGateway::new(policy()).admit(request, 4_000),
            Err(AgentRuntimeError::TaskCallDepthExceeded)
        );
    }
}
