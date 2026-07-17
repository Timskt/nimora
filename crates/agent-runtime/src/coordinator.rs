use super::{
    AgentRuntimeError, AgentTask, AgentTaskStatus, ProviderError, ProviderExecutionContext,
    ProviderFinishReason, ProviderMessage, ProviderRegistry, ProviderRequest, ProviderResponse,
    ToolAdmission, ToolApproval, ToolBackend, ToolInvocation, ToolRegistry, ToolRiskEvaluator,
};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct PlannedToolCall {
    pub provider_call_id: String,
    pub invocation: ToolInvocation,
    pub admission: ToolAdmission,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProviderStepOutcome {
    Completed {
        response: ProviderResponse,
    },
    ToolCalls {
        response: ProviderResponse,
        calls: Vec<PlannedToolCall>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToolStepOutcome {
    ConfirmationRequired {
        invocation: ToolInvocation,
        admission: ToolAdmission,
    },
    Completed {
        invocation: ToolInvocation,
        output: Value,
    },
}

#[derive(Debug, Clone)]
pub struct ProviderStepInput {
    pub model: String,
    pub messages: Vec<ProviderMessage>,
    pub max_output_tokens: u64,
    pub context: ProviderExecutionContext,
    pub offline: bool,
    pub now_ms: u64,
}

#[derive(Debug, Error)]
pub enum CoordinatorError {
    #[error("agent runtime rejected the step: {0}")]
    Runtime(#[from] AgentRuntimeError),
    #[error("provider rejected the step: {0}")]
    Provider(#[from] ProviderError),
    #[error("provider request does not belong to the task")]
    CorrelationMismatch,
}

#[derive(Debug)]
pub struct AgentCoordinator<'a, R> {
    providers: &'a ProviderRegistry,
    tools: &'a ToolRegistry<R>,
}

impl<'a, R: ToolRiskEvaluator> AgentCoordinator<'a, R> {
    #[must_use]
    pub const fn new(providers: &'a ProviderRegistry, tools: &'a ToolRegistry<R>) -> Self {
        Self { providers, tools }
    }

    /// Advances exactly one Provider step and turns requested tools into gated invocations.
    ///
    /// # Errors
    ///
    /// Returns an error for task/request correlation failures, Provider failures, invalid tools,
    /// lifecycle violations, or exhausted budgets.
    pub fn provider_step(
        &self,
        task: &mut AgentTask,
        input: ProviderStepInput,
    ) -> Result<ProviderStepOutcome, CoordinatorError> {
        if task.status == AgentTaskStatus::Pending {
            task.transition(AgentTaskStatus::Planning, input.now_ms)?;
        }
        let request = ProviderRequest::new(
            task.id,
            task.trace_id,
            task.provider_id.clone(),
            input.model,
            input.messages,
            self.tools.descriptors().into_iter().cloned().collect(),
            input.max_output_tokens,
        )?;
        let response = self
            .providers
            .complete(&request, input.context, input.offline)?;
        task.account_provider_step(
            response.usage.input_tokens,
            response.usage.output_tokens,
            response.usage.cost_microunits,
            input.now_ms,
        )?;

        if response.finish_reason != ProviderFinishReason::ToolCalls {
            task.transition(AgentTaskStatus::Succeeded, input.now_ms)?;
            return Ok(ProviderStepOutcome::Completed { response });
        }

        let mut calls = Vec::with_capacity(response.tool_calls.len());
        for call in &response.tool_calls {
            let invocation = ToolInvocation::new(
                task.id,
                task.trace_id,
                call.tool_id.to_string(),
                call.arguments.clone(),
            )?;
            let admission = self.tools.admit(&invocation)?;
            calls.push(PlannedToolCall {
                provider_call_id: call.id.clone(),
                invocation,
                admission,
            });
        }
        let confirmation_required = calls
            .iter()
            .any(|call| matches!(call.admission, ToolAdmission::ConfirmationRequired { .. }));
        let next = if confirmation_required {
            AgentTaskStatus::WaitingForConfirmation
        } else {
            AgentTaskStatus::Running
        };
        if task.status != next {
            task.transition(next, input.now_ms)?;
        }
        Ok(ProviderStepOutcome::ToolCalls { response, calls })
    }

    /// Executes exactly one admitted module tool through the supplied Capability Gateway backend.
    ///
    /// # Errors
    ///
    /// Returns an error for stale approval, unavailable tools, backend failure, lifecycle
    /// violations, or exhausted budgets.
    pub fn tool_step<B: ToolBackend>(
        &self,
        task: &mut AgentTask,
        backend: &B,
        invocation: ToolInvocation,
        approval: Option<&ToolApproval>,
        now_ms: u64,
    ) -> Result<ToolStepOutcome, CoordinatorError> {
        if invocation.task_id != task.id || invocation.trace_id != task.trace_id {
            return Err(CoordinatorError::CorrelationMismatch);
        }
        let admission = self.tools.admit(&invocation)?;
        if matches!(admission, ToolAdmission::ConfirmationRequired { .. }) && approval.is_none() {
            if task.status != AgentTaskStatus::WaitingForConfirmation {
                task.transition(AgentTaskStatus::WaitingForConfirmation, now_ms)?;
            }
            return Ok(ToolStepOutcome::ConfirmationRequired {
                invocation,
                admission,
            });
        }
        if task.status != AgentTaskStatus::Running {
            task.transition(AgentTaskStatus::Running, now_ms)?;
        }
        task.reserve_tool_call(now_ms)?;
        let output = self.tools.dispatch(backend, &invocation, approval)?;
        Ok(ToolStepOutcome::Completed { invocation, output })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AgentBudget, AgentTaskOrigin, CancellationFlag, DataClassification, ProviderAdapter,
        ProviderCapabilities, ProviderCapability, ProviderDescriptor, ProviderLocality,
        ProviderMessageRole, ProviderToolCall, ProviderUsage, ToolDescriptor, ToolEffect,
    };
    use nimora_runtime_core::CommandRisk;
    use serde_json::json;
    use std::{collections::BTreeSet, time::Duration};

    #[derive(Debug)]
    struct ToolCallingProvider {
        descriptor: ProviderDescriptor,
        tool_id: crate::ToolId,
    }

    impl ProviderAdapter for ToolCallingProvider {
        fn descriptor(&self) -> &ProviderDescriptor {
            &self.descriptor
        }

        fn complete(
            &self,
            request: &ProviderRequest,
            _context: &ProviderExecutionContext,
        ) -> Result<ProviderResponse, ProviderError> {
            Ok(ProviderResponse {
                spec: "nimora.agent-provider-response/1".to_owned(),
                request_id: request.request_id,
                content: String::new(),
                tool_calls: vec![ProviderToolCall {
                    id: "call:1".to_owned(),
                    tool_id: self.tool_id.clone(),
                    arguments: json!({"profileRef": "profile:active"}),
                }],
                finish_reason: ProviderFinishReason::ToolCalls,
                usage: ProviderUsage {
                    input_tokens: 8,
                    output_tokens: 4,
                    cost_microunits: 2,
                },
            })
        }
    }

    #[derive(Debug)]
    struct Backend;

    impl ToolBackend for Backend {
        fn invoke(
            &self,
            invocation: &ToolInvocation,
            _descriptor: &ToolDescriptor,
            _timeout: Duration,
        ) -> Result<Value, String> {
            Ok(json!({"moduleAccepted": invocation.arguments}))
        }
    }

    fn fixture() -> (ProviderRegistry, ToolRegistry, AgentTask) {
        let tool = ToolDescriptor::new(
            "profile.appearance.update",
            "Update appearance",
            "Updates appearance through the profile capability gateway.",
            json!({"type": "object"}),
            json!({"type": "object"}),
            CommandRisk::Low,
            ToolEffect::ReversibleWrite,
        )
        .expect("tool");
        let mut tools = ToolRegistry::default();
        tools.register(tool.clone()).expect("register tool");
        let descriptor = ProviderDescriptor::new(
            "provider:local",
            "Local deterministic Provider",
            ProviderLocality::Local,
            4_096,
            1_024,
            ProviderCapabilities {
                supported: BTreeSet::from([
                    ProviderCapability::StructuredToolCalls,
                    ProviderCapability::UsageReporting,
                ]),
            },
        )
        .expect("provider descriptor");
        let mut providers = ProviderRegistry::default();
        providers
            .register(ToolCallingProvider {
                descriptor,
                tool_id: tool.id,
            })
            .expect("register provider");
        let task = AgentTask::new(
            AgentTaskOrigin::Desktop,
            "desktop:user",
            "provider:local",
            AgentBudget::default(),
            1_000,
        )
        .expect("task");
        (providers, tools, task)
    }

    fn input() -> ProviderStepInput {
        ProviderStepInput {
            model: "model:local".to_owned(),
            messages: vec![ProviderMessage {
                role: ProviderMessageRole::User,
                content: "Apply my selected appearance".to_owned(),
                classification: DataClassification::Personal,
                trusted: true,
            }],
            max_output_tokens: 128,
            context: ProviderExecutionContext {
                timeout: Duration::from_secs(5),
                cancellation: CancellationFlag::default(),
                credential_reference: None,
            },
            offline: true,
            now_ms: 1_010,
        }
    }

    #[test]
    fn provider_tool_call_becomes_confirmation_bound_module_invocation() {
        let (providers, tools, mut task) = fixture();
        let coordinator = AgentCoordinator::new(&providers, &tools);
        let outcome = coordinator
            .provider_step(&mut task, input())
            .expect("provider step");
        let ProviderStepOutcome::ToolCalls { calls, .. } = outcome else {
            panic!("expected tool calls");
        };
        assert_eq!(task.status, AgentTaskStatus::WaitingForConfirmation);
        assert_eq!(task.usage.steps, 1);
        assert_eq!(calls[0].invocation.task_id, task.id);
        assert!(matches!(
            calls[0].admission,
            ToolAdmission::ConfirmationRequired { .. }
        ));
        assert_eq!(task.usage.tool_calls, 0);
    }

    #[test]
    fn confirmed_tool_executes_only_through_backend_and_reserves_budget() {
        let (providers, tools, mut task) = fixture();
        let coordinator = AgentCoordinator::new(&providers, &tools);
        let ProviderStepOutcome::ToolCalls { calls, .. } = coordinator
            .provider_step(&mut task, input())
            .expect("provider step")
        else {
            panic!("expected tool calls");
        };
        let call = calls.into_iter().next().expect("call");
        let ToolAdmission::ConfirmationRequired { effective_risk, .. } = call.admission else {
            panic!("expected confirmation");
        };
        let approval = ToolApproval::bind(&call.invocation, effective_risk);
        let outcome = coordinator
            .tool_step(&mut task, &Backend, call.invocation, Some(&approval), 1_020)
            .expect("tool step");
        assert!(matches!(outcome, ToolStepOutcome::Completed { .. }));
        assert_eq!(task.status, AgentTaskStatus::Running);
        assert_eq!(task.usage.tool_calls, 1);
    }

    #[test]
    fn cross_task_invocation_is_rejected_before_backend_dispatch() {
        let (providers, tools, mut task) = fixture();
        task.transition(AgentTaskStatus::Planning, 1_001)
            .expect("planning");
        let coordinator = AgentCoordinator::new(&providers, &tools);
        let invocation = ToolInvocation::new(
            uuid::Uuid::now_v7(),
            task.trace_id,
            "profile.appearance.update",
            json!({}),
        )
        .expect("invocation");
        assert!(matches!(
            coordinator.tool_step(&mut task, &Backend, invocation, None, 1_002),
            Err(CoordinatorError::CorrelationMismatch)
        ));
        assert_eq!(task.usage.tool_calls, 0);
    }
}
