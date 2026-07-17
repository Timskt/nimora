use nimora_agent_runtime::{
    AgentAutonomy, AgentBudget, AgentTaskAdmission, AgentTaskGateway, AgentTaskGatewayPolicy,
    AgentTaskOrigin, AgentTaskParent, AgentTaskRequest, DataClassification,
};
use nimora_automation_runtime::{ActionFailure, AutomationBackend, AutomationExecutionContext};
use nimora_runtime_core::{Command, CommandRisk};
use serde::Deserialize;
use std::collections::BTreeSet;

pub const AGENT_TASK_RUN_COMMAND: &str = "agent.task.run";
const MAX_INSTRUCTION_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationAgentTask {
    pub admission: AgentTaskAdmission,
    pub instruction: String,
    pub idempotency_key: String,
}

pub trait AgentTaskSubmitter: std::fmt::Debug + Send + Sync {
    /// Submits one already-admitted task to the host Agent service.
    ///
    /// # Errors
    ///
    /// Returns a stable error without exposing Provider or host internals.
    fn submit(&self, task: AutomationAgentTask) -> Result<(), String>;
}

pub trait AutomationAgentContext: std::fmt::Debug + Send + Sync {
    /// Returns host-controlled admission time for this Automation command.
    ///
    /// # Errors
    ///
    /// Returns a stable error when the host cannot provide trusted run context.
    fn now_ms(&self, command: &Command) -> Result<u64, String>;

    /// Returns the trusted root task budget remaining for this Automation run.
    ///
    /// # Errors
    ///
    /// Returns a stable error when the host cannot resolve the run budget.
    fn remaining_budget(&self, command: &Command) -> Result<AgentBudget, String>;
}

#[derive(Debug)]
pub struct AutomationAgentBridge<B, S, C> {
    fallback: B,
    submitter: S,
    context: C,
    policy: AgentTaskGatewayPolicy,
}

impl<B, S, C> AutomationAgentBridge<B, S, C> {
    #[must_use]
    pub const fn new(
        fallback: B,
        submitter: S,
        context: C,
        policy: AgentTaskGatewayPolicy,
    ) -> Self {
        Self {
            fallback,
            submitter,
            context,
            policy,
        }
    }
}

impl<B, S, C> AutomationBackend for AutomationAgentBridge<B, S, C>
where
    B: AutomationBackend,
    S: AgentTaskSubmitter,
    C: AutomationAgentContext,
{
    fn execute(
        &self,
        context: &AutomationExecutionContext,
        command: Command,
    ) -> Result<(), ActionFailure> {
        if command.command_id.to_string() != AGENT_TASK_RUN_COMMAND {
            return self.fallback.execute(context, command);
        }
        self.execute_agent(context, &command)
    }
}

impl<B, S, C> AutomationAgentBridge<B, S, C>
where
    S: AgentTaskSubmitter,
    C: AutomationAgentContext,
{
    fn execute_agent(
        &self,
        context: &AutomationExecutionContext,
        command: &Command,
    ) -> Result<(), ActionFailure> {
        if matches!(command.risk, CommandRisk::Safe | CommandRisk::Low) {
            return Err(permanent("agent task action risk must be medium or higher"));
        }
        let arguments = serde_json::from_value::<AgentActionArguments>(command.arguments.clone())
            .map_err(|_| permanent("agent task action arguments are invalid"))?;
        if arguments.instruction.trim().is_empty()
            || arguments.instruction.len() > MAX_INSTRUCTION_BYTES
            || arguments.context_trust != ContextTrust::Trusted
        {
            return Err(permanent(
                "agent task instruction is invalid or contains untrusted context",
            ));
        }
        let idempotency_key = command
            .idempotency_key
            .clone()
            .ok_or_else(|| permanent("agent task action requires an idempotency key"))?;
        let now_ms = self.context.now_ms(command).map_err(host_context_failure)?;
        let remaining_budget = self
            .context
            .remaining_budget(command)
            .map_err(host_context_failure)?;
        let request = AgentTaskRequest {
            spec: "nimora.agent-task-request/1".to_owned(),
            origin: AgentTaskOrigin::Automation,
            requester: arguments.requester,
            provider_id: arguments.provider_id,
            tool_allowlist: arguments.tool_allowlist,
            classification: arguments.classification,
            autonomy: arguments.autonomy,
            budget: arguments.budget,
            parent: Some(AgentTaskParent {
                root_task_id: context.run_id,
                parent_task_id: context.run_id,
                trace_id: context.trace_id,
                call_depth: 0,
                remaining_budget,
            }),
        };
        let admission = AgentTaskGateway::new(self.policy.clone())
            .admit(request, now_ms)
            .map_err(|error| permanent(error.to_string()))?;
        self.submitter
            .submit(AutomationAgentTask {
                admission,
                instruction: arguments.instruction,
                idempotency_key,
            })
            .map_err(|error| ActionFailure {
                message: error,
                transient: true,
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ContextTrust {
    Trusted,
    Untrusted,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AgentActionArguments {
    requester: String,
    provider_id: String,
    instruction: String,
    #[serde(default)]
    tool_allowlist: BTreeSet<String>,
    classification: DataClassification,
    autonomy: AgentAutonomy,
    budget: AgentBudget,
    context_trust: ContextTrust,
}

fn permanent(message: impl Into<String>) -> ActionFailure {
    ActionFailure {
        message: message.into(),
        transient: false,
    }
}

fn host_context_failure(message: String) -> ActionFailure {
    ActionFailure {
        message,
        transient: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimora_automation_runtime::{
        AutomationAction, AutomationDefinition, AutomationEngine, AutomationPolicy,
        AutomationRunStatus, EventTrigger, FailurePolicy, RunMode, Uncancelled,
    };
    use nimora_runtime_core::{Event, EventSource};
    use serde_json::json;
    use std::sync::Mutex;
    use uuid::Uuid;

    #[derive(Debug, Default)]
    struct Fallback {
        commands: Mutex<Vec<String>>,
    }

    impl AutomationBackend for Fallback {
        fn execute(
            &self,
            _context: &AutomationExecutionContext,
            command: Command,
        ) -> Result<(), ActionFailure> {
            self.commands
                .lock()
                .expect("commands")
                .push(command.command_id.to_string());
            Ok(())
        }
    }

    #[derive(Debug, Default)]
    struct Submitter {
        tasks: Mutex<Vec<AutomationAgentTask>>,
    }

    impl AgentTaskSubmitter for Submitter {
        fn submit(&self, task: AutomationAgentTask) -> Result<(), String> {
            self.tasks.lock().expect("tasks").push(task);
            Ok(())
        }
    }

    #[derive(Debug)]
    struct TrustedContext {
        now_ms: u64,
        remaining_budget: AgentBudget,
    }

    impl AutomationAgentContext for TrustedContext {
        fn now_ms(&self, _command: &Command) -> Result<u64, String> {
            Ok(self.now_ms)
        }

        fn remaining_budget(&self, _command: &Command) -> Result<AgentBudget, String> {
            Ok(self.remaining_budget)
        }
    }

    fn budget(max_steps: u32, max_tool_calls: u32) -> AgentBudget {
        AgentBudget {
            max_steps,
            max_tool_calls,
            max_elapsed_ms: 30_000,
            max_input_tokens: 2_000,
            max_output_tokens: 500,
            max_cost_microunits: 0,
        }
    }

    fn policy() -> AgentTaskGatewayPolicy {
        AgentTaskGatewayPolicy::new(
            "automation:local.focus.ai-summary",
            [AgentTaskOrigin::Automation],
            ["provider:deterministic-local".to_owned()],
            ["runtime.health.read".to_owned()],
            DataClassification::Personal,
            AgentAutonomy::ConfirmEach,
            budget(6, 2),
            2,
        )
        .expect("policy")
    }

    fn definition(context_trust: &str) -> AutomationDefinition {
        AutomationDefinition {
            spec: "nimora.automation/1".to_owned(),
            id: "local.focus.ai-summary".to_owned(),
            name: "AI summary".to_owned(),
            enabled: true,
            trigger: EventTrigger {
                event_type: "focus.session.finished".to_owned(),
            },
            conditions: Vec::new(),
            actions: vec![AutomationAction {
                id: "summarize".to_owned(),
                command: AGENT_TASK_RUN_COMMAND.to_owned(),
                arguments: json!({
                    "requester": "automation:local.focus.ai-summary",
                    "providerId": "provider:deterministic-local",
                    "instruction": "Summarize the completed focus session.",
                    "toolAllowlist": ["runtime.health.read"],
                    "classification": "personal",
                    "autonomy": "draft",
                    "budget": budget(8, 4),
                    "contextTrust": context_trust
                }),
                risk: CommandRisk::Medium,
                retry_safe: true,
                idempotency_key: Some("focus-session-42-summary".to_owned()),
                compensation: None,
            }],
            policy: AutomationPolicy {
                timeout_ms: 30_000,
                failure: FailurePolicy::Stop,
            },
        }
    }

    fn event() -> Event {
        Event::new(
            "focus.session.finished",
            EventSource::Automation("local.focus.timer".to_owned()),
            json!({"durationMinutes": 25}),
        )
        .expect("event")
    }

    fn bridge() -> AutomationAgentBridge<Fallback, Submitter, TrustedContext> {
        AutomationAgentBridge::new(
            Fallback::default(),
            Submitter::default(),
            TrustedContext {
                now_ms: 1_000,
                remaining_budget: budget(3, 1),
            },
            policy(),
        )
    }

    #[test]
    fn automation_submits_admitted_child_with_shared_trace_and_budget() {
        let bridge = bridge();
        let event = event();
        let run = AutomationEngine::run(
            &definition("trusted"),
            &event,
            RunMode::Live,
            &bridge,
            &Uncancelled,
        )
        .expect("run");
        assert_eq!(run.status, AutomationRunStatus::Succeeded);
        let tasks = bridge.submitter.tasks.lock().expect("tasks");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].admission.task.trace_id, event.trace_id);
        assert_eq!(tasks[0].admission.root_task_id, run.run_id);
        assert_eq!(tasks[0].admission.parent_task_id, Some(run.run_id));
        assert_eq!(tasks[0].admission.task.budget.max_steps, 3);
        assert_eq!(tasks[0].admission.task.budget.max_tool_calls, 1);
        assert_eq!(tasks[0].admission.call_depth, 1);
        assert_eq!(tasks[0].idempotency_key, "focus-session-42-summary");
    }

    #[test]
    fn untrusted_context_fails_before_submitter_and_is_not_retried() {
        let bridge = bridge();
        let run = AutomationEngine::run(
            &definition("untrusted"),
            &event(),
            RunMode::Live,
            &bridge,
            &Uncancelled,
        )
        .expect("run");
        assert_eq!(run.steps[0].attempts, 1);
        assert!(bridge.submitter.tasks.lock().expect("tasks").is_empty());
    }

    #[test]
    fn non_agent_commands_continue_to_fallback_backend() {
        let bridge = bridge();
        let command =
            Command::new("pet.animation.play", json!({}), CommandRisk::Low).expect("command");
        let context = AutomationExecutionContext {
            run_id: Uuid::from_u128(1),
            automation_id: "local.focus.ai-summary".to_owned(),
            action_id: "summarize".to_owned(),
            event_id: "event:test".to_owned(),
            trace_id: Uuid::from_u128(2),
        };
        bridge.execute(&context, command).expect("fallback");
        assert_eq!(
            *bridge.fallback.commands.lock().expect("commands"),
            ["pet.animation.play"]
        );
    }

    #[test]
    fn action_cannot_forge_host_time_or_remaining_budget() {
        let bridge = bridge();
        let mut definition = definition("trusted");
        definition.actions[0].arguments["nowMs"] = json!(u64::MAX);
        definition.actions[0].arguments["rootRemainingBudget"] = json!(budget(100, 100));
        let run =
            AutomationEngine::run(&definition, &event(), RunMode::Live, &bridge, &Uncancelled)
                .expect("run");
        assert_eq!(run.steps[0].attempts, 1);
        assert!(bridge.submitter.tasks.lock().expect("tasks").is_empty());
    }
}
