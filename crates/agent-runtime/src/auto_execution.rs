use crate::{
    AgentCoordinator, AgentTask, AutoModeError, AutoModePauseReason, AutoModeSession,
    AutoModeStepDecision, AutoModeStepRequest, AutoModeUsage, CoordinatorError, DataClassification,
    PlannedToolCall, ProviderMessage, ProviderResponse, ProviderStepInput, ProviderStepOutcome,
    ProviderToolTurn, ToolAdmission, ToolBackend, ToolEffect, ToolRegistry, ToolRiskEvaluator,
    ToolStepOutcome,
};
use nimora_runtime_core::CommandRisk;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum AutoModeTurnOutcome {
    Completed {
        response: ProviderResponse,
    },
    Continue {
        messages: Vec<ProviderMessage>,
    },
    Paused {
        reason: AutoModePauseReason,
        pending_calls: Vec<PlannedToolCall>,
    },
}

#[derive(Debug, Error)]
pub enum AutoModeTurnError {
    #[error(transparent)]
    AutoMode(#[from] AutoModeError),
    #[error(transparent)]
    Coordinator(#[from] CoordinatorError),
    #[error("Auto Mode tool descriptor disappeared after admission")]
    MissingToolDescriptor,
    #[error("Auto Mode safe tool unexpectedly requested confirmation")]
    UnexpectedConfirmation,
}

#[derive(Debug)]
pub struct AutoModeTurnSupervisor<'a, R, B> {
    coordinator: AgentCoordinator<'a, R>,
    tools: &'a ToolRegistry<R>,
    backend: &'a B,
}

impl<'a, R: ToolRiskEvaluator, B: ToolBackend> AutoModeTurnSupervisor<'a, R, B> {
    #[must_use]
    pub const fn new(
        coordinator: AgentCoordinator<'a, R>,
        tools: &'a ToolRegistry<R>,
        backend: &'a B,
    ) -> Self {
        Self {
            coordinator,
            tools,
            backend,
        }
    }

    /// Advances one Provider turn and executes only an entirely safe read-only tool batch.
    ///
    /// The whole tool batch is preflighted before the first dispatch. A write, external effect,
    /// disallowed tool, excessive classification, or confirmation risk pauses with zero tool
    /// dispatches for that turn.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid session state, Provider failure, corrupt tool registration,
    /// task correlation failure, or backend failure.
    pub fn advance(
        &self,
        session: &mut AutoModeSession,
        task: &mut AgentTask,
        input: ProviderStepInput,
    ) -> Result<AutoModeTurnOutcome, AutoModeTurnError> {
        let now_ms = input.now_ms;
        let classification = input
            .messages
            .iter()
            .map(|message| message.classification)
            .max()
            .unwrap_or(DataClassification::Public);
        let decision = session.evaluate_step(
            &AutoModeStepRequest {
                goal_id: session.goal_id,
                plan_revision: session.plan_revision,
                workspace_revision: session.policy.workspace_revision.clone(),
                tool_id: None,
                risk: CommandRisk::Safe,
                effect: ToolEffect::ReadOnly,
                data_classification: classification,
                projected_usage: AutoModeUsage {
                    cycles: 1,
                    ..AutoModeUsage::default()
                },
            },
            now_ms,
        )?;
        if let AutoModeStepDecision::Pause(reason) = decision {
            return Ok(AutoModeTurnOutcome::Paused {
                reason,
                pending_calls: Vec::new(),
            });
        }

        match self.coordinator.provider_step(task, input)? {
            ProviderStepOutcome::Completed { response } => {
                Ok(AutoModeTurnOutcome::Completed { response })
            }
            ProviderStepOutcome::ToolCalls { response, calls } => {
                self.advance_tool_batch(session, task, response, calls, now_ms)
            }
        }
    }

    fn advance_tool_batch(
        &self,
        session: &mut AutoModeSession,
        task: &mut AgentTask,
        response: ProviderResponse,
        calls: Vec<PlannedToolCall>,
        now_ms: u64,
    ) -> Result<AutoModeTurnOutcome, AutoModeTurnError> {
        for call in &calls {
            let descriptor = self
                .tools
                .descriptor(&call.invocation.tool_id)
                .ok_or(AutoModeTurnError::MissingToolDescriptor)?;
            let risk = effective_risk(&call.admission);
            let classification = descriptor
                .data_classifications
                .iter()
                .copied()
                .max()
                .unwrap_or(DataClassification::Public);
            let mut preview = session.clone();
            let decision = preview.evaluate_step(
                &AutoModeStepRequest {
                    goal_id: session.goal_id,
                    plan_revision: session.plan_revision,
                    workspace_revision: session.policy.workspace_revision.clone(),
                    tool_id: Some(call.invocation.tool_id.clone()),
                    risk,
                    effect: descriptor.effect,
                    data_classification: classification,
                    projected_usage: AutoModeUsage::default(),
                },
                now_ms,
            )?;
            if let AutoModeStepDecision::Pause(reason) = decision {
                session.pause(reason, now_ms)?;
                return Ok(AutoModeTurnOutcome::Paused {
                    reason,
                    pending_calls: calls,
                });
            }
        }

        let mut turn = ProviderToolTurn::new(response)?;
        for call in calls {
            let decision = session.evaluate_step(
                &AutoModeStepRequest {
                    goal_id: session.goal_id,
                    plan_revision: session.plan_revision,
                    workspace_revision: session.policy.workspace_revision.clone(),
                    tool_id: Some(call.invocation.tool_id.clone()),
                    risk: effective_risk(&call.admission),
                    effect: ToolEffect::ReadOnly,
                    data_classification: DataClassification::Public,
                    projected_usage: AutoModeUsage {
                        tool_calls: 1,
                        ..AutoModeUsage::default()
                    },
                },
                now_ms,
            )?;
            if let AutoModeStepDecision::Pause(reason) = decision {
                return Ok(AutoModeTurnOutcome::Paused {
                    reason,
                    pending_calls: vec![call],
                });
            }
            let tool_id = call.invocation.tool_id.to_string();
            let provider_call_id = call.provider_call_id.clone();
            let ToolStepOutcome::Completed { output, .. } =
                self.coordinator
                    .tool_step(task, self.backend, call.invocation, None, now_ms)?
            else {
                return Err(AutoModeTurnError::UnexpectedConfirmation);
            };
            turn.record_result(&provider_call_id, &tool_id, output)?;
        }
        Ok(AutoModeTurnOutcome::Continue {
            messages: turn.continuation_messages()?,
        })
    }
}

const fn effective_risk(admission: &ToolAdmission) -> CommandRisk {
    match admission {
        ToolAdmission::Ready { effective_risk }
        | ToolAdmission::ConfirmationRequired { effective_risk, .. } => *effective_risk,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AgentBudget, AgentGoal, AgentPlan, AgentPlanStep, AgentTaskOrigin, AutoModePolicy,
        CancellationFlag, ProviderAdapter, ProviderCapabilities, ProviderCapability,
        ProviderDescriptor, ProviderError, ProviderExecutionContext, ProviderFinishReason,
        ProviderLocality, ProviderRegistry, ProviderRequest, ProviderToolCall, ProviderUsage,
        ToolDescriptor, ToolInvocation,
    };
    use serde_json::{Value, json};
    use std::{
        collections::BTreeSet,
        sync::atomic::{AtomicUsize, Ordering},
        time::Duration,
    };
    use uuid::Uuid;

    #[derive(Debug)]
    struct CallingProvider {
        descriptor: ProviderDescriptor,
        tool_id: crate::ToolId,
    }

    impl ProviderAdapter for CallingProvider {
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
                    arguments: json!({}),
                }],
                finish_reason: ProviderFinishReason::ToolCalls,
                usage: ProviderUsage {
                    input_tokens: 4,
                    output_tokens: 2,
                    cost_microunits: 0,
                },
            })
        }
    }

    #[derive(Debug, Default)]
    struct CountingBackend(AtomicUsize);

    impl ToolBackend for CountingBackend {
        fn invoke(
            &self,
            _invocation: &ToolInvocation,
            _descriptor: &ToolDescriptor,
            _timeout: Duration,
        ) -> Result<Value, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(json!({"state": "idle"}))
        }
    }

    fn fixture(effect: ToolEffect) -> (ProviderRegistry, ToolRegistry, AgentTask, AutoModeSession) {
        let tool = ToolDescriptor::new(
            "pet.state.read",
            "Read pet",
            "Reads bounded pet state.",
            json!({"type": "object"}),
            json!({"type": "object"}),
            CommandRisk::Safe,
            effect,
        )
        .expect("tool");
        let mut tools = ToolRegistry::default();
        tools.register(tool.clone()).expect("register");
        let descriptor = ProviderDescriptor::new(
            "provider:local",
            "Local",
            ProviderLocality::Local,
            4_096,
            1_024,
            ProviderCapabilities {
                supported: BTreeSet::from([ProviderCapability::StructuredToolCalls]),
            },
        )
        .expect("provider");
        let mut providers = ProviderRegistry::default();
        providers
            .register(CallingProvider {
                descriptor,
                tool_id: tool.id,
            })
            .expect("register provider");
        let task = AgentTask::new(
            AgentTaskOrigin::Cli,
            "cli:user",
            "provider:local",
            AgentBudget::default(),
            1_000,
        )
        .expect("task");
        let plan = AgentPlan::new(
            Uuid::now_v7(),
            vec![AgentPlanStep::new("Inspect").expect("step")],
            "initial",
            1_000,
        )
        .expect("plan");
        let goal = AgentGoal::new("Inspect", "Inspect safely", &plan, 1_000).expect("goal");
        let policy = AutoModePolicy::new(
            4,
            1,
            AgentBudget::default(),
            DataClassification::Personal,
            ["pet.state.read".to_owned()],
            "git:abc",
        )
        .expect("policy");
        let session = AutoModeSession::start(&goal, &plan, policy, 1_000).expect("session");
        (providers, tools, task, session)
    }

    fn input() -> ProviderStepInput {
        ProviderStepInput {
            model: "model:local".to_owned(),
            messages: vec![ProviderMessage::text(
                crate::ProviderMessageRole::User,
                "Inspect",
                DataClassification::Personal,
                true,
            )],
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
    fn write_turn_pauses_before_any_backend_dispatch() {
        let (providers, tools, mut task, mut session) = fixture(ToolEffect::ReversibleWrite);
        let backend = CountingBackend::default();
        let supervisor = AutoModeTurnSupervisor::new(
            AgentCoordinator::new(&providers, &tools),
            &tools,
            &backend,
        );
        let outcome = supervisor
            .advance(&mut session, &mut task, input())
            .expect("advance");
        assert!(matches!(
            outcome,
            AutoModeTurnOutcome::Paused {
                reason: AutoModePauseReason::ConfirmationRequired,
                ..
            }
        ));
        assert_eq!(backend.0.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn safe_read_turn_dispatches_and_returns_correlated_continuation() {
        let (providers, tools, mut task, mut session) = fixture(ToolEffect::ReadOnly);
        let backend = CountingBackend::default();
        let supervisor = AutoModeTurnSupervisor::new(
            AgentCoordinator::new(&providers, &tools),
            &tools,
            &backend,
        );
        let outcome = supervisor
            .advance(&mut session, &mut task, input())
            .expect("advance");
        let AutoModeTurnOutcome::Continue { messages } = outcome else {
            panic!("expected continuation");
        };
        assert_eq!(backend.0.load(Ordering::SeqCst), 1);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].tool_call_id.as_deref(), Some("call:1"));
        assert_eq!(session.usage.cycles, 1);
        assert_eq!(session.usage.tool_calls, 1);
    }
}
