//! Deterministic automation admission and execution for `Nimora`.

use nimora_runtime_core::{Command, CommandId, CommandRisk, CommandStatus, Event};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashSet, time::Duration};
use thiserror::Error;
use uuid::Uuid;

const MAX_AUTOMATION_ACTIONS: usize = 64;
const MAX_AUTOMATION_CONDITIONS: usize = 32;
const MAX_AUTOMATION_TIMEOUT_MS: u64 = 300_000;
const MAX_AUTOMATION_CONCURRENT_RUNS: u16 = 16;
const MAX_AUTOMATION_COOLDOWN_MS: u64 = 24 * 60 * 60 * 1000;
const MAX_AUTOMATION_DAILY_COST_MICROUNITS: u64 = 1_000_000_000_000;
const MAX_ACTION_ATTEMPTS: u8 = 3;
const MAX_JSON_POINTER_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutomationDefinition {
    pub spec: String,
    pub id: String,
    pub version: String,
    pub name: String,
    pub enabled: bool,
    pub trigger: EventTrigger,
    #[serde(default)]
    pub conditions: Vec<ValueCondition>,
    pub actions: Vec<AutomationAction>,
    pub policy: AutomationPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventTrigger {
    pub event_type: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValueCondition {
    pub pointer: String,
    pub equals: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutomationAction {
    pub id: String,
    pub command: String,
    pub arguments: Value,
    pub risk: CommandRisk,
    #[serde(default)]
    pub retry_safe: bool,
    pub idempotency_key: Option<String>,
    pub compensation: Option<CompensationAction>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompensationAction {
    pub command: String,
    pub arguments: Value,
    pub risk: CommandRisk,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutomationPolicy {
    pub timeout_ms: u64,
    pub failure: FailurePolicy,
    pub max_concurrent_runs: u16,
    pub cooldown_ms: u64,
    pub daily_cost_budget_microunits: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailurePolicy {
    Stop,
    Compensate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    Live,
    DryRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationRunStatus {
    TriggerNotMatched,
    ConditionNotMatched,
    Planned,
    WaitingForApproval,
    Succeeded,
    Failed,
    CompensationFailed,
    Cancelled,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationStepResult {
    pub action_id: String,
    pub command: String,
    pub status: CommandStatus,
    pub attempts: u8,
    pub compensated: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRun {
    pub spec: String,
    pub run_id: Uuid,
    pub automation_id: String,
    pub trace_id: Uuid,
    pub event_id: String,
    pub mode: String,
    pub status: AutomationRunStatus,
    pub steps: Vec<AutomationStepResult>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionFailure {
    pub message: String,
    pub transient: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationExecutionContext {
    pub run_id: Uuid,
    pub automation_id: String,
    pub action_id: String,
    pub event_id: String,
    pub trace_id: Uuid,
}

pub trait AutomationBackend: std::fmt::Debug + Send + Sync {
    /// Executes an admitted command through a host-owned capability boundary.
    ///
    /// # Errors
    ///
    /// Returns a classified action failure without exposing host internals.
    fn execute(
        &self,
        context: &AutomationExecutionContext,
        command: Command,
    ) -> Result<(), ActionFailure>;
}

pub trait RunControl: std::fmt::Debug + Send + Sync {
    fn cancelled(&self) -> bool;
    fn elapsed(&self) -> Duration;
}

#[derive(Debug, Default)]
pub struct Uncancelled;

impl RunControl for Uncancelled {
    fn cancelled(&self) -> bool {
        false
    }

    fn elapsed(&self) -> Duration {
        Duration::ZERO
    }
}

#[derive(Debug, Default)]
pub struct AutomationEngine;

impl AutomationEngine {
    /// Validates one current automation definition.
    ///
    /// # Errors
    ///
    /// Rejects malformed identifiers, unsafe retry declarations and excessive plans.
    pub fn validate(definition: &AutomationDefinition) -> Result<(), AutomationError> {
        if definition.spec != "nimora.automation/1"
            || !valid_automation_id(&definition.id)
            || !valid_automation_version(&definition.version)
            || definition.name.trim().is_empty()
            || definition.name.len() > 128
            || definition.actions.is_empty()
            || definition.actions.len() > MAX_AUTOMATION_ACTIONS
            || definition.conditions.len() > MAX_AUTOMATION_CONDITIONS
            || definition.policy.timeout_ms == 0
            || definition.policy.timeout_ms > MAX_AUTOMATION_TIMEOUT_MS
            || definition.policy.max_concurrent_runs == 0
            || definition.policy.max_concurrent_runs > MAX_AUTOMATION_CONCURRENT_RUNS
            || definition.policy.cooldown_ms > MAX_AUTOMATION_COOLDOWN_MS
            || definition.policy.daily_cost_budget_microunits > MAX_AUTOMATION_DAILY_COST_MICROUNITS
        {
            return Err(AutomationError::InvalidDefinition);
        }
        Event::new(
            definition.trigger.event_type.clone(),
            nimora_runtime_core::EventSource::Automation(definition.id.clone()),
            Value::Null,
        )
        .map_err(|_| AutomationError::InvalidDefinition)?;
        let mut action_ids = HashSet::new();
        for condition in &definition.conditions {
            if !valid_pointer(&condition.pointer) {
                return Err(AutomationError::InvalidCondition);
            }
        }
        for action in &definition.actions {
            if !valid_action_id(&action.id)
                || !action_ids.insert(action.id.as_str())
                || action.command.parse::<CommandId>().is_err()
                || action.arguments.as_object().is_none()
                || action.idempotency_key.as_ref().is_some_and(|key| {
                    key.is_empty() || key.len() > 128 || key.chars().any(char::is_control)
                })
                || (action.retry_safe && action.idempotency_key.is_none())
            {
                return Err(AutomationError::InvalidAction);
            }
            if let Some(compensation) = &action.compensation
                && (compensation.command.parse::<CommandId>().is_err()
                    || compensation.arguments.as_object().is_none())
            {
                return Err(AutomationError::InvalidAction);
            }
        }
        Ok(())
    }

    /// Evaluates and runs one automation against an immutable event snapshot.
    ///
    /// # Errors
    ///
    /// Returns only admission errors. Runtime failures are represented in the run result.
    pub fn run(
        definition: &AutomationDefinition,
        event: &Event,
        mode: RunMode,
        backend: &dyn AutomationBackend,
        control: &dyn RunControl,
    ) -> Result<AutomationRun, AutomationError> {
        Self::run_with_id(Uuid::now_v7(), definition, event, mode, backend, control)
    }

    /// Evaluates one automation using a host-assigned stable run identity.
    ///
    /// # Errors
    ///
    /// Returns only admission errors. Runtime failures are represented in the run result.
    pub fn run_with_id(
        run_id: Uuid,
        definition: &AutomationDefinition,
        event: &Event,
        mode: RunMode,
        backend: &dyn AutomationBackend,
        control: &dyn RunControl,
    ) -> Result<AutomationRun, AutomationError> {
        Self::validate(definition)?;
        let mut run = AutomationRun {
            spec: "nimora.automation-run/1".to_owned(),
            run_id,
            automation_id: definition.id.clone(),
            trace_id: event.trace_id,
            event_id: event.id.to_string(),
            mode: match mode {
                RunMode::Live => "live",
                RunMode::DryRun => "dry_run",
            }
            .to_owned(),
            status: AutomationRunStatus::TriggerNotMatched,
            steps: Vec::new(),
            reason: None,
        };
        if !definition.enabled || definition.trigger.event_type != event.event_type {
            run.reason = Some("trigger did not match an enabled automation".to_owned());
            return Ok(run);
        }
        if let Some(condition) = definition
            .conditions
            .iter()
            .find(|condition| event.data.pointer(&condition.pointer) != Some(&condition.equals))
        {
            run.status = AutomationRunStatus::ConditionNotMatched;
            run.reason = Some(format!("condition not matched: {}", condition.pointer));
            return Ok(run);
        }
        if mode == RunMode::DryRun {
            run.status = AutomationRunStatus::Planned;
            run.steps = definition
                .actions
                .iter()
                .map(|action| AutomationStepResult {
                    action_id: action.id.clone(),
                    command: action.command.clone(),
                    status: CommandStatus::Pending,
                    attempts: 0,
                    compensated: false,
                    error: None,
                })
                .collect();
            return Ok(run);
        }

        for action in &definition.actions {
            if let Some(status) = interrupted_status(definition, control) {
                run.status = status;
                run.reason = Some("automation execution interrupted".to_owned());
                return Ok(run);
            }
            let context = AutomationExecutionContext {
                run_id: run.run_id,
                automation_id: definition.id.clone(),
                action_id: action.id.clone(),
                event_id: event.id.to_string(),
                trace_id: event.trace_id,
            };
            let (mut step, succeeded) = execute_action(action, event, &context, backend)?;
            if !succeeded {
                step.status = CommandStatus::Failed;
                run.steps.push(step);
                run.status = AutomationRunStatus::Failed;
                run.reason = Some("action execution failed".to_owned());
                if definition.policy.failure == FailurePolicy::Compensate
                    && !compensate(definition, event, run.run_id, backend, &mut run.steps)
                {
                    run.status = AutomationRunStatus::CompensationFailed;
                    run.reason = Some("action and compensation execution failed".to_owned());
                }
                return Ok(run);
            }
            run.steps.push(step);
        }
        run.status = AutomationRunStatus::Succeeded;
        Ok(run)
    }
}

fn valid_automation_version(version: &str) -> bool {
    let segments = version.split('.').collect::<Vec<_>>();
    segments.len() == 3
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && (*segment == "0" || !segment.starts_with('0'))
                && segment.chars().all(|character| character.is_ascii_digit())
        })
}

fn execute_action(
    action: &AutomationAction,
    event: &Event,
    context: &AutomationExecutionContext,
    backend: &dyn AutomationBackend,
) -> Result<(AutomationStepResult, bool), AutomationError> {
    let attempts = if action.retry_safe {
        MAX_ACTION_ATTEMPTS
    } else {
        1
    };
    let mut step = AutomationStepResult {
        action_id: action.id.clone(),
        command: action.command.clone(),
        status: CommandStatus::Running,
        attempts: 0,
        compensated: false,
        error: None,
    };
    for attempt in 1..=attempts {
        step.attempts = attempt;
        let mut command = Command::new(&action.command, action.arguments.clone(), action.risk)
            .map_err(|_| AutomationError::InvalidAction)?;
        command.trace_id = event.trace_id;
        command.status = CommandStatus::Running;
        command.idempotency_key.clone_from(&action.idempotency_key);
        match backend.execute(context, command) {
            Ok(()) => {
                step.status = CommandStatus::Succeeded;
                return Ok((step, true));
            }
            Err(error) => {
                step.error = Some(error.message);
                if !error.transient {
                    break;
                }
            }
        }
    }
    Ok((step, false))
}

fn compensate(
    definition: &AutomationDefinition,
    event: &Event,
    run_id: Uuid,
    backend: &dyn AutomationBackend,
    steps: &mut [AutomationStepResult],
) -> bool {
    let mut complete = true;
    for (action, step) in definition.actions.iter().zip(steps.iter_mut()).rev() {
        if step.status != CommandStatus::Succeeded {
            continue;
        }
        let Some(compensation) = &action.compensation else {
            continue;
        };
        let Ok(mut command) = Command::new(
            &compensation.command,
            compensation.arguments.clone(),
            compensation.risk,
        ) else {
            complete = false;
            continue;
        };
        command.trace_id = event.trace_id;
        command.status = CommandStatus::Running;
        let context = AutomationExecutionContext {
            run_id,
            automation_id: definition.id.clone(),
            action_id: action.id.clone(),
            event_id: event.id.to_string(),
            trace_id: event.trace_id,
        };
        match backend.execute(&context, command) {
            Ok(()) => step.compensated = true,
            Err(error) => {
                complete = false;
                step.error = Some(error.message);
            }
        }
    }
    complete
}

fn interrupted_status(
    definition: &AutomationDefinition,
    control: &dyn RunControl,
) -> Option<AutomationRunStatus> {
    if control.cancelled() {
        Some(AutomationRunStatus::Cancelled)
    } else if control.elapsed() >= Duration::from_millis(definition.policy.timeout_ms) {
        Some(AutomationRunStatus::TimedOut)
    } else {
        None
    }
}

fn valid_automation_id(value: &str) -> bool {
    value.len() <= 128 && value.split('.').count() >= 3 && value.split('.').all(valid_segment)
}

fn valid_action_id(value: &str) -> bool {
    value.len() <= 64 && valid_segment(value)
}

fn valid_segment(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

fn valid_pointer(value: &str) -> bool {
    value.len() <= MAX_JSON_POINTER_BYTES
        && (value.is_empty()
            || (value.starts_with('/')
                && !value.chars().any(char::is_control)
                && !value.split('/').skip(1).any(|part| {
                    part.as_bytes()
                        .windows(2)
                        .any(|pair| pair[0] == b'~' && pair[1] != b'0' && pair[1] != b'1')
                        || part.ends_with('~')
                })))
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AutomationError {
    #[error("invalid automation definition")]
    InvalidDefinition,
    #[error("invalid automation condition")]
    InvalidCondition,
    #[error("invalid automation action")]
    InvalidAction,
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimora_runtime_core::EventSource;
    use serde_json::json;
    use std::sync::Mutex;

    #[derive(Debug, Default)]
    struct Backend {
        commands: Mutex<Vec<String>>,
        contexts: Mutex<Vec<AutomationExecutionContext>>,
        transient_failures: Mutex<usize>,
        fail_command: Option<String>,
    }

    impl AutomationBackend for Backend {
        fn execute(
            &self,
            context: &AutomationExecutionContext,
            command: Command,
        ) -> Result<(), ActionFailure> {
            self.contexts
                .lock()
                .expect("contexts")
                .push(context.clone());
            self.commands
                .lock()
                .expect("commands")
                .push(command.command_id.to_string());
            let mut failures = self.transient_failures.lock().expect("failures");
            if *failures > 0 {
                *failures -= 1;
                return Err(ActionFailure {
                    message: "temporary".to_owned(),
                    transient: true,
                });
            }
            if self
                .fail_command
                .as_ref()
                .is_some_and(|value| value == &command.command_id.to_string())
            {
                return Err(ActionFailure {
                    message: "denied".to_owned(),
                    transient: false,
                });
            }
            Ok(())
        }
    }

    #[derive(Debug)]
    struct Control {
        cancelled: bool,
        elapsed: Duration,
    }

    impl RunControl for Control {
        fn cancelled(&self) -> bool {
            self.cancelled
        }

        fn elapsed(&self) -> Duration {
            self.elapsed
        }
    }

    fn definition() -> AutomationDefinition {
        AutomationDefinition {
            spec: "nimora.automation/1".to_owned(),
            id: "local.focus.on-build".to_owned(),
            version: "1.0.0".to_owned(),
            name: "Build companion".to_owned(),
            enabled: true,
            trigger: EventTrigger {
                event_type: "dev.build.finished".to_owned(),
            },
            conditions: vec![ValueCondition {
                pointer: "/succeeded".to_owned(),
                equals: json!(true),
            }],
            actions: vec![AutomationAction {
                id: "celebrate".to_owned(),
                command: "pet.animation.play".to_owned(),
                arguments: json!({"action": "celebrate"}),
                risk: CommandRisk::Low,
                retry_safe: true,
                idempotency_key: Some("build-42-celebrate".to_owned()),
                compensation: Some(CompensationAction {
                    command: "pet.animation.play".to_owned(),
                    arguments: json!({"action": "idle"}),
                    risk: CommandRisk::Low,
                }),
            }],
            policy: AutomationPolicy {
                timeout_ms: 5_000,
                failure: FailurePolicy::Compensate,
                max_concurrent_runs: 1,
                cooldown_ms: 0,
                daily_cost_budget_microunits: 0,
            },
        }
    }

    fn event(succeeded: bool) -> Event {
        Event::new(
            "dev.build.finished",
            EventSource::System("test".to_owned()),
            json!({"succeeded": succeeded}),
        )
        .expect("event")
    }

    #[test]
    fn dry_run_is_side_effect_free_and_lists_steps() {
        let backend = Backend::default();
        let run = AutomationEngine::run(
            &definition(),
            &event(true),
            RunMode::DryRun,
            &backend,
            &Uncancelled,
        )
        .expect("run");
        assert_eq!(run.status, AutomationRunStatus::Planned);
        assert_eq!(run.steps[0].attempts, 0);
        assert!(backend.commands.lock().expect("commands").is_empty());
    }

    #[test]
    fn version_requires_three_canonical_numeric_segments() {
        let mut candidate = definition();
        candidate.version = "1.0.0".to_owned();
        assert_eq!(AutomationEngine::validate(&candidate), Ok(()));

        for version in ["1.0", "01.0.0", "1.0.0-beta", "1.0.0.0"] {
            candidate.version = version.to_owned();
            assert_eq!(
                AutomationEngine::validate(&candidate),
                Err(AutomationError::InvalidDefinition),
                "version {version} must be rejected"
            );
        }
    }

    #[test]
    fn condition_short_circuits_without_dispatch() {
        let backend = Backend::default();
        let run = AutomationEngine::run(
            &definition(),
            &event(false),
            RunMode::Live,
            &backend,
            &Uncancelled,
        )
        .expect("run");
        assert_eq!(run.status, AutomationRunStatus::ConditionNotMatched);
        assert!(backend.commands.lock().expect("commands").is_empty());
    }

    #[test]
    fn retry_requires_idempotency_and_reuses_the_declared_action() {
        let backend = Backend {
            transient_failures: Mutex::new(2),
            ..Backend::default()
        };
        let run = AutomationEngine::run(
            &definition(),
            &event(true),
            RunMode::Live,
            &backend,
            &Uncancelled,
        )
        .expect("run");
        assert_eq!(run.status, AutomationRunStatus::Succeeded);
        assert_eq!(run.steps[0].attempts, 3);
        let contexts = backend.contexts.lock().expect("contexts");
        assert_eq!(contexts.len(), 3);
        assert!(contexts.iter().all(|context| context.run_id == run.run_id));
        assert!(
            contexts
                .iter()
                .all(|context| context.trace_id == run.trace_id)
        );
        assert!(
            contexts
                .iter()
                .all(|context| context.action_id == "celebrate")
        );

        let mut invalid = definition();
        invalid.actions[0].idempotency_key = None;
        assert_eq!(
            AutomationEngine::validate(&invalid),
            Err(AutomationError::InvalidAction)
        );
    }

    #[test]
    fn failure_compensates_completed_actions_in_reverse_order() {
        let mut definition = definition();
        definition.actions.push(AutomationAction {
            id: "notify".to_owned(),
            command: "notification.desktop.send".to_owned(),
            arguments: json!({"message": "done"}),
            risk: CommandRisk::Low,
            retry_safe: false,
            idempotency_key: None,
            compensation: None,
        });
        let backend = Backend {
            fail_command: Some("notification.desktop.send".to_owned()),
            ..Backend::default()
        };
        let run = AutomationEngine::run(
            &definition,
            &event(true),
            RunMode::Live,
            &backend,
            &Uncancelled,
        )
        .expect("run");
        assert_eq!(run.status, AutomationRunStatus::Failed);
        assert!(run.steps[0].compensated);
        assert_eq!(
            *backend.commands.lock().expect("commands"),
            [
                "pet.animation.play",
                "notification.desktop.send",
                "pet.animation.play"
            ]
        );
    }

    #[test]
    fn cancellation_and_timeout_fail_before_dispatch() {
        let backend = Backend::default();
        let cancelled = AutomationEngine::run(
            &definition(),
            &event(true),
            RunMode::Live,
            &backend,
            &Control {
                cancelled: true,
                elapsed: Duration::ZERO,
            },
        )
        .expect("cancelled run");
        assert_eq!(cancelled.status, AutomationRunStatus::Cancelled);
        let timed_out = AutomationEngine::run(
            &definition(),
            &event(true),
            RunMode::Live,
            &backend,
            &Control {
                cancelled: false,
                elapsed: Duration::from_secs(5),
            },
        )
        .expect("timed out run");
        assert_eq!(timed_out.status, AutomationRunStatus::TimedOut);
        assert!(backend.commands.lock().expect("commands").is_empty());
    }
}
