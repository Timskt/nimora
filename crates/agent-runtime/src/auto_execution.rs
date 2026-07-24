use crate::{
    AgentCoordinator, AgentTask, AuthorizationDecision, AuthorizationError, AuthorizationGrant,
    AuthorizationRequest, AutoModeError, AutoModePauseReason, AutoModeSession,
    AutoModeStepDecision, AutoModeStepRequest, AutoModeUsage, CoordinatorError, DataClassification,
    PlannedToolCall, ProviderMessage, ProviderResponse, ProviderStepInput, ProviderStepOutcome,
    ProviderToolTurn, ToolAdmission, ToolApproval, ToolBackend, ToolEffect, ToolId, ToolRegistry,
    ToolRiskEvaluator, ToolStepOutcome,
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
    authorization_grant: Option<AuthorizationGrant>,
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
            authorization_grant: None,
        }
    }

    /// Attaches an immutable pre-authorization used for unattended in-scope tool work.
    #[must_use]
    pub fn with_authorization_grant(mut self, grant: AuthorizationGrant) -> Self {
        self.authorization_grant = Some(grant);
        self
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
        let model = input.model.clone();
        let decision = session.evaluate_step_with_grant(
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
            self.authorization_grant.as_ref(),
            Some(task.provider_id.as_str()),
            Some(model.as_str()),
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
                self.advance_tool_batch(session, task, response, calls, now_ms, &model)
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
        model: &str,
    ) -> Result<AutoModeTurnOutcome, AutoModeTurnError> {
        let provider_id = task.provider_id.clone();
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
            let decision = preview.evaluate_step_with_grant(
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
                self.authorization_grant.as_ref(),
                Some(provider_id.as_str()),
                Some(model),
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
            let descriptor = self
                .tools
                .descriptor(&call.invocation.tool_id)
                .ok_or(AutoModeTurnError::MissingToolDescriptor)?;
            let classification = descriptor
                .data_classifications
                .iter()
                .copied()
                .max()
                .unwrap_or(DataClassification::Public);
            let risk = effective_risk(&call.admission);
            let decision = session.evaluate_step_with_grant(
                &AutoModeStepRequest {
                    goal_id: session.goal_id,
                    plan_revision: session.plan_revision,
                    workspace_revision: session.policy.workspace_revision.clone(),
                    tool_id: Some(call.invocation.tool_id.clone()),
                    risk,
                    effect: ToolEffect::ReadOnly,
                    data_classification: DataClassification::Public,
                    projected_usage: AutoModeUsage {
                        tool_calls: 1,
                        ..AutoModeUsage::default()
                    },
                },
                self.authorization_grant.as_ref(),
                Some(provider_id.as_str()),
                Some(model),
                now_ms,
            )?;
            if let AutoModeStepDecision::Pause(reason) = decision {
                return Ok(AutoModeTurnOutcome::Paused {
                    reason,
                    pending_calls: vec![call],
                });
            }
            let approval = self.auto_tool_approval(
                session,
                &call,
                risk,
                classification,
                provider_id.as_str(),
                model,
                now_ms,
            );
            let tool_id = call.invocation.tool_id.to_string();
            let provider_call_id = call.provider_call_id.clone();
            let ToolStepOutcome::Completed { output, .. } = self.coordinator.tool_step(
                task,
                self.backend,
                call.invocation,
                approval.as_ref(),
                now_ms,
            )?
            else {
                return Err(AutoModeTurnError::UnexpectedConfirmation);
            };
            turn.record_result(&provider_call_id, &tool_id, output)?;
        }
        Ok(AutoModeTurnOutcome::Continue {
            messages: turn.continuation_messages()?,
        })
    }

    fn auto_tool_approval(
        &self,
        session: &AutoModeSession,
        call: &PlannedToolCall,
        effective_risk: CommandRisk,
        data_classification: DataClassification,
        provider_id: &str,
        model: &str,
        now_ms: u64,
    ) -> Option<ToolApproval> {
        let ToolAdmission::ConfirmationRequired { .. } = call.admission else {
            return None;
        };
        let grant = self.authorization_grant.as_ref()?;
        match authorize_tool(
            grant,
            session,
            &call.invocation.tool_id,
            provider_id,
            model,
            data_classification,
            now_ms,
        ) {
            Ok(AuthorizationDecision::Authorized) => {
                Some(ToolApproval::bind(&call.invocation, effective_risk))
            }
            Ok(AuthorizationDecision::ApprovalRequired) | Err(_) => None,
        }
    }
}

fn authorize_tool(
    grant: &AuthorizationGrant,
    session: &AutoModeSession,
    tool_id: &ToolId,
    provider_id: &str,
    model: &str,
    data_classification: DataClassification,
    now_ms: u64,
) -> Result<AuthorizationDecision, AuthorizationError> {
    grant.authorize(&AuthorizationRequest {
        goal_id: session.goal_id,
        plan_revision: session.plan_revision,
        workspace_fingerprint: &session.policy.workspace_revision,
        tool_id,
        provider_id,
        model,
        data_classification,
        requires_network: false,
        now_ms,
    })
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
    use serde_json::{json, Value};
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
                reasoning: None,
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
            reasoning: None,
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

    fn workspace_fingerprint() -> String {
        format!("sha256:{}", "b".repeat(64))
    }

    fn fixture_with_workspace(
        effect: ToolEffect,
        workspace: &str,
    ) -> (
        ProviderRegistry,
        ToolRegistry,
        AgentTask,
        AutoModeSession,
        Uuid,
        u64,
    ) {
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
                reasoning: None,
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
            workspace,
        )
        .expect("policy");
        let session = AutoModeSession::start(&goal, &plan, policy, 1_000).expect("session");
        (providers, tools, task, session, goal.id, plan.revision)
    }

    fn test_grant(
        goal_id: Uuid,
        plan_revision: u64,
        approval: crate::ApprovalPolicy,
        tools: &[&str],
        workspace: &str,
    ) -> AuthorizationGrant {
        use crate::{GrantLifetime, NetworkPolicy, SandboxScope};
        let grant = AuthorizationGrant {
            spec: "nimora.authorization-grant/1".to_owned(),
            id: Uuid::now_v7(),
            goal_id,
            plan_revision,
            workspace_fingerprint: workspace.to_owned(),
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
            lifetime: GrantLifetime::Session,
            issued_at_ms: 1_000,
            expires_at_ms: None,
            revoked_at_ms: None,
        };
        grant.validate().expect("grant valid");
        grant
    }

    #[test]
    fn never_ask_grant_dispatches_write_without_confirmation_pause() {
        let workspace = workspace_fingerprint();
        let (providers, tools, mut task, mut session, goal_id, plan_revision) =
            fixture_with_workspace(ToolEffect::ReversibleWrite, &workspace);
        let grant = test_grant(
            goal_id,
            plan_revision,
            crate::ApprovalPolicy::NeverAskWithinGrant,
            &["pet.state.read"],
            &workspace,
        );
        let backend = CountingBackend::default();
        let supervisor = AutoModeTurnSupervisor::new(
            AgentCoordinator::new(&providers, &tools),
            &tools,
            &backend,
        )
        .with_authorization_grant(grant);
        let outcome = supervisor
            .advance(&mut session, &mut task, input())
            .expect("advance");
        let AutoModeTurnOutcome::Continue { messages } = outcome else {
            panic!("expected continuation, got {outcome:?}");
        };
        assert_eq!(backend.0.load(Ordering::SeqCst), 1);
        assert_eq!(messages.len(), 2);
        assert_eq!(session.usage.tool_calls, 1);
    }

    #[test]
    fn always_ask_grant_still_pauses_write_turn() {
        let workspace = workspace_fingerprint();
        let (providers, tools, mut task, mut session, goal_id, plan_revision) =
            fixture_with_workspace(ToolEffect::ReversibleWrite, &workspace);
        let grant = test_grant(
            goal_id,
            plan_revision,
            crate::ApprovalPolicy::AlwaysAsk,
            &["pet.state.read"],
            &workspace,
        );
        let backend = CountingBackend::default();
        let supervisor = AutoModeTurnSupervisor::new(
            AgentCoordinator::new(&providers, &tools),
            &tools,
            &backend,
        )
        .with_authorization_grant(grant);
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
}
