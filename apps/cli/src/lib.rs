use nimora_agent_provider_worker::{OllamaEndpoint, WorkerOllamaProvider, verify_provider_sidecar};
use nimora_agent_runtime::{
    AgentAutonomy, AgentBudget, AgentCoordinator, AgentGoal, AgentGoalStatus, AgentPlan,
    AgentPlanStep, AgentPlanStepStatus, AgentTask, AgentTaskGateway, AgentTaskGatewayPolicy,
    AgentTaskOrigin, AgentTaskRequest, AutoModePauseReason, AutoModePolicy, AutoModeSession,
    CancellationFlag, DataClassification, DeterministicLocalProvider, ProviderExecutionContext,
    ProviderMessage, ProviderMessageRole, ProviderRegistry, ProviderStepInput, ProviderStepOutcome,
};
use nimora_agent_tools::production_tool_registry;
use nimora_agent_workspace_host::{
    GitWorkspaceAdapter, WorkspaceScanPolicy, WorkspaceScanner, unix_time_ms,
};
use nimora_persistence_sqlite::{
    AgentHistoryRecord, SqliteAgentGoalRepository, SqliteAgentHistoryRepository,
    SqliteAutoModeRepository, SqliteWorkspaceSnapshotRepository, StoredWorkspaceSnapshot,
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    fs,
    io::{self, Read},
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const PROVIDER_ID: &str = "provider:deterministic-local";
const OLLAMA_PROVIDER_ID: &str = "provider:ollama-loopback";

#[derive(Debug, Clone, Copy)]
struct SidecarConfig<'a> {
    root: &'a str,
    manifest_sha256: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    kind: &'static str,
    message: String,
    code: u8,
}

impl CliError {
    fn new(kind: &'static str, message: impl Into<String>, code: u8) -> Self {
        Self {
            kind,
            message: message.into(),
            code,
        }
    }

    #[must_use]
    pub const fn code(&self) -> u8 {
        self.code
    }

    #[must_use]
    pub fn json(&self) -> String {
        json!({"spec": "nimora.cli-error/1", "error": self.kind, "message": self.message})
            .to_string()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RunInput {
    prompt: String,
    #[serde(default = "default_model")]
    model: String,
    #[serde(default = "default_provider")]
    provider_id: String,
    #[serde(default = "default_output_tokens")]
    max_output_tokens: u64,
    #[serde(default = "default_ollama_port")]
    ollama_port: u16,
}

fn default_model() -> String {
    "model:echo-v1".to_owned()
}
fn default_provider() -> String {
    PROVIDER_ID.to_owned()
}
const fn default_output_tokens() -> u64 {
    512
}
const fn default_ollama_port() -> u16 {
    11_434
}

/// Runs one CLI invocation and returns exactly one machine-readable JSON document.
///
/// # Errors
///
/// Returns a stable categorized error for invalid syntax, input, unavailable resources, or
/// runtime failures.
pub fn run(arguments: &[String]) -> Result<String, CliError> {
    let values = arguments.iter().map(String::as_str).collect::<Vec<_>>();
    let output = match values.as_slice() {
        ["--help" | "help"] | [] => help(),
        ["--version"] => {
            json!({"spec": "nimora.cli-version/1", "version": env!("CARGO_PKG_VERSION")})
        }
        ["ai", "provider", "list"] => provider_list()?,
        ["ai", "provider", "probe"] => provider_probe()?,
        ["ai", "tool", "list"] => tool_list()?,
        ["ai", "tool", "describe", tool_id] => tool_describe(tool_id)?,
        ["ai", "history", "export", rest @ ..] => history_export(rest)?,
        ["ai", "history", "delete", rest @ ..] => history_delete(rest)?,
        ["ai", "workspace", "inspect", rest @ ..] => workspace_inspect(rest)?,
        ["ai", "goal", "create", rest @ ..] => goal_create(rest)?,
        ["ai", "goal", "list", rest @ ..] => goal_list(rest)?,
        ["ai", "goal", "show", rest @ ..] => goal_show(rest)?,
        ["ai", "goal", "plan", "replace", rest @ ..] => goal_plan_replace(rest)?,
        ["ai", "goal", "status", "set", rest @ ..] => goal_status_set(rest)?,
        ["ai", "goal", "auto", "start", rest @ ..] => goal_auto_start(rest)?,
        ["ai", "goal", "auto", "status", rest @ ..] => goal_auto_status(rest)?,
        ["ai", "goal", "auto", "pause", rest @ ..] => goal_auto_pause(rest)?,
        ["ai", "goal", "auto", "resume", rest @ ..] => goal_auto_resume(rest)?,
        ["ai", "goal", "auto", "cancel", rest @ ..] => goal_auto_cancel(rest)?,
        ["ai", "run", rest @ ..] => run_task(rest)?,
        _ => return Err(CliError::new("usage", "unsupported command; use --help", 2)),
    };
    serde_json::to_string(&output)
        .map_err(|_| CliError::new("serialization", "failed to serialize command result", 10))
}

fn help() -> Value {
    json!({
        "spec": "nimora.cli-help/1",
        "commands": [
            "nimora ai provider list",
            "nimora ai provider probe",
            "nimora ai tool list",
            "nimora ai tool describe <tool-id>",
            "nimora ai run --input <path|-> --output json [--offline] [--history-database <path>] [--sidecar-root <path> --sidecar-manifest-sha256 <digest>]",
            "nimora ai history export --database <path> [--limit <1..200>] [--before-created-at-ms <timestamp> --before-task-id <uuid>]",
            "nimora ai history delete --database <path> (--task-id <uuid>|--all)",
            "nimora ai workspace inspect --root <path> [--revision <number> --parent-fingerprint <digest>] [--git]",
            "nimora ai goal create --database <path> --input <path|->",
            "nimora ai goal list --database <path> [--limit <1..200>]",
            "nimora ai goal show --database <path> --goal-id <uuid>",
            "nimora ai goal plan replace --database <path> --goal-id <uuid> --input <path|->",
            "nimora ai goal status set --database <path> --goal-id <uuid> --status <active|paused|completed|cancelled>",
            "nimora ai goal auto start --database <path> --goal-id <uuid> --input <path|->",
            "nimora ai goal auto status --database <path> --session-id <uuid>",
            "nimora ai goal auto pause --database <path> --session-id <uuid>",
            "nimora ai goal auto resume --database <path> --session-id <uuid> --workspace-root <path>",
            "nimora ai goal auto cancel --database <path> --session-id <uuid>"
        ]
    })
}

fn workspace_inspect(arguments: &[&str]) -> Result<Value, CliError> {
    let mut root = None;
    let mut revision = 1_u64;
    let mut parent_fingerprint = None;
    let mut inspect_git = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index] {
            "--root" if index + 1 < arguments.len() => {
                root = Some(arguments[index + 1]);
                index += 2;
            }
            "--revision" if index + 1 < arguments.len() => {
                revision = arguments[index + 1]
                    .parse()
                    .map_err(|_| CliError::new("usage", "workspace revision is invalid", 2))?;
                index += 2;
            }
            "--parent-fingerprint" if index + 1 < arguments.len() => {
                parent_fingerprint = Some(arguments[index + 1].to_owned());
                index += 2;
            }
            "--git" => {
                inspect_git = true;
                index += 1;
            }
            _ => {
                return Err(CliError::new(
                    "usage",
                    "workspace inspect requires --root and optional revision, parent fingerprint, and --git",
                    2,
                ));
            }
        }
    }
    let root = root.ok_or_else(|| CliError::new("usage", "missing --root", 2))?;
    if revision == 1 && parent_fingerprint.is_some() || revision > 1 && parent_fingerprint.is_none()
    {
        return Err(CliError::new(
            "usage",
            "workspace parent fingerprint is required exactly when revision is greater than one",
            2,
        ));
    }
    let scanner = WorkspaceScanner::open(Path::new(root), WorkspaceScanPolicy::default())
        .map_err(|_| CliError::new("workspace", "cannot open workspace safely", 4))?;
    let snapshot = scanner
        .scan(revision, parent_fingerprint, unix_time_ms())
        .map_err(|_| CliError::new("workspace", "workspace scan failed closed", 4))?;
    let git = if inspect_git {
        Some(
            GitWorkspaceAdapter::open(Path::new(root), Duration::from_secs(5))
                .and_then(|adapter| adapter.inspect())
                .map_err(|_| CliError::new("git-workspace", "cannot inspect Git workspace", 4))?,
        )
    } else {
        None
    };
    Ok(json!({
        "spec": "nimora.ai-workspace-inspection/1",
        "rootFingerprint": scanner.root_fingerprint(),
        "snapshot": snapshot,
        "git": git
    }))
}

fn history_repository(path: &str) -> Result<SqliteAgentHistoryRepository, CliError> {
    if path.is_empty() {
        return Err(CliError::new("usage", "database path cannot be empty", 2));
    }
    SqliteAgentHistoryRepository::open(Path::new(path)).map_err(|_| {
        CliError::new(
            "history-storage",
            "cannot open or validate Agent history database",
            4,
        )
    })
}

fn history_export(arguments: &[&str]) -> Result<Value, CliError> {
    let mut database = None;
    let mut limit = 50_usize;
    let mut before_created_at_ms = None;
    let mut before_task_id = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index] {
            "--database" if index + 1 < arguments.len() => {
                database = Some(arguments[index + 1]);
                index += 2;
            }
            "--limit" if index + 1 < arguments.len() => {
                limit = arguments[index + 1]
                    .parse()
                    .map_err(|_| CliError::new("usage", "history limit must be 1..200", 2))?;
                index += 2;
            }
            "--before-created-at-ms" if index + 1 < arguments.len() => {
                before_created_at_ms = Some(arguments[index + 1].parse().map_err(|_| {
                    CliError::new("usage", "history cursor timestamp is invalid", 2)
                })?);
                index += 2;
            }
            "--before-task-id" if index + 1 < arguments.len() => {
                before_task_id =
                    Some(uuid::Uuid::parse_str(arguments[index + 1]).map_err(|_| {
                        CliError::new("usage", "history cursor task ID is invalid", 2)
                    })?);
                index += 2;
            }
            _ => {
                return Err(CliError::new(
                    "usage",
                    "history export requires --database and an optional paired cursor",
                    2,
                ));
            }
        }
    }
    let database = database.ok_or_else(|| CliError::new("usage", "missing --database", 2))?;
    if !(1..=200).contains(&limit) {
        return Err(CliError::new("usage", "history limit must be 1..200", 2));
    }
    let before = match (before_created_at_ms, before_task_id) {
        (Some(created_at_ms), Some(task_id)) => Some((created_at_ms, task_id)),
        (None, None) => None,
        _ => {
            return Err(CliError::new(
                "usage",
                "history cursor fields must be provided together",
                2,
            ));
        }
    };
    let records = history_repository(database)?
        .list(before, limit)
        .map_err(history_storage_error)?;
    Ok(json!({
        "spec": "nimora.ai-history-export/1",
        "records": records
    }))
}

fn history_delete(arguments: &[&str]) -> Result<Value, CliError> {
    let mut database = None;
    let mut task_id = None;
    let mut delete_all = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index] {
            "--database" if index + 1 < arguments.len() => {
                database = Some(arguments[index + 1]);
                index += 2;
            }
            "--task-id" if index + 1 < arguments.len() => {
                task_id = Some(
                    uuid::Uuid::parse_str(arguments[index + 1])
                        .map_err(|_| CliError::new("usage", "history task ID is invalid", 2))?,
                );
                index += 2;
            }
            "--all" => {
                delete_all = true;
                index += 1;
            }
            _ => {
                return Err(CliError::new(
                    "usage",
                    "history delete requires --database and exactly one deletion target",
                    2,
                ));
            }
        }
    }
    let database = database.ok_or_else(|| CliError::new("usage", "missing --database", 2))?;
    let repository = history_repository(database)?;
    let deleted = match (task_id, delete_all) {
        (Some(task_id), false) => {
            u64::from(repository.delete(task_id).map_err(history_storage_error)?)
        }
        (None, true) => repository.delete_all().map_err(history_storage_error)?,
        _ => {
            return Err(CliError::new(
                "usage",
                "provide exactly one of --task-id or --all",
                2,
            ));
        }
    };
    Ok(json!({
        "spec": "nimora.ai-history-delete/1",
        "deleted": deleted
    }))
}

fn history_storage_error(_: impl std::fmt::Display) -> CliError {
    CliError::new("history-storage", "Agent history operation failed", 10)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GoalCreateInput {
    title: String,
    objective: String,
    steps: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GoalPlanInput {
    rationale: String,
    steps: Vec<GoalPlanStepInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GoalPlanStepInput {
    id: Option<uuid::Uuid>,
    text: String,
    status: AgentPlanStepStatus,
    #[serde(default)]
    evidence: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AutoModeStartInput {
    max_cycles: u32,
    max_concurrency: u16,
    budget: AgentBudget,
    maximum_data_classification: DataClassification,
    tool_allowlist: Vec<String>,
    workspace_root: String,
}

fn goal_repository(path: &str) -> Result<SqliteAgentGoalRepository, CliError> {
    if path.is_empty() {
        return Err(CliError::new("usage", "database path cannot be empty", 2));
    }
    SqliteAgentGoalRepository::open(Path::new(path)).map_err(goal_storage_error)
}

fn goal_create(arguments: &[&str]) -> Result<Value, CliError> {
    let (database, input_path) = parse_goal_database_input(arguments)?;
    let input = read_bounded_json::<GoalCreateInput>(input_path, "Goal create input")?;
    let now_ms = current_time_ms()?;
    let goal_id = uuid::Uuid::now_v7();
    let steps = input
        .steps
        .into_iter()
        .map(AgentPlanStep::new)
        .collect::<Result<Vec<_>, _>>()
        .map_err(goal_input_error)?;
    let plan = AgentPlan::new(goal_id, steps, "Initial plan", now_ms).map_err(goal_input_error)?;
    let goal =
        AgentGoal::new(input.title, input.objective, &plan, now_ms).map_err(goal_input_error)?;
    goal_repository(database)?
        .create(&goal, &plan)
        .map_err(goal_storage_error)?;
    Ok(json!({"spec": "nimora.ai-goal-snapshot/1", "goal": goal, "currentPlan": plan}))
}

fn goal_list(arguments: &[&str]) -> Result<Value, CliError> {
    let mut database = None;
    let mut limit = 50_usize;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index] {
            "--database" if index + 1 < arguments.len() => {
                database = Some(arguments[index + 1]);
                index += 2;
            }
            "--limit" if index + 1 < arguments.len() => {
                limit = arguments[index + 1]
                    .parse()
                    .map_err(|_| CliError::new("usage", "Goal limit is invalid", 2))?;
                index += 2;
            }
            _ => return Err(goal_usage_error()),
        }
    }
    let database = database.ok_or_else(goal_usage_error)?;
    let goals = goal_repository(database)?
        .list(limit)
        .map_err(goal_storage_error)?;
    Ok(json!({"spec": "nimora.ai-goal-list/1", "goals": goals}))
}

fn goal_show(arguments: &[&str]) -> Result<Value, CliError> {
    let (database, goal_id) = parse_goal_identity(arguments)?;
    let snapshot = goal_repository(database)?
        .get(goal_id)
        .map_err(goal_storage_error)?
        .ok_or_else(|| CliError::new("goal-not-found", "Agent Goal was not found", 5))?;
    Ok(json!({
        "spec": "nimora.ai-goal-snapshot/1",
        "goal": snapshot.goal,
        "currentPlan": snapshot.current_plan
    }))
}

fn goal_plan_replace(arguments: &[&str]) -> Result<Value, CliError> {
    let (database, goal_id, input_path) = parse_goal_identity_input(arguments)?;
    let input = read_bounded_json::<GoalPlanInput>(input_path, "Goal plan input")?;
    let repository = goal_repository(database)?;
    let mut snapshot = repository
        .get(goal_id)
        .map_err(goal_storage_error)?
        .ok_or_else(|| CliError::new("goal-not-found", "Agent Goal was not found", 5))?;
    let steps = input
        .steps
        .into_iter()
        .map(|input| {
            let mut step = AgentPlanStep::new(input.text)?;
            if let Some(id) = input.id {
                step.id = id;
            }
            step.update(input.status, input.evidence)?;
            Ok(step)
        })
        .collect::<Result<Vec<_>, nimora_agent_runtime::AgentGoalError>>()
        .map_err(goal_input_error)?;
    let now_ms = current_time_ms()?.max(snapshot.goal.updated_at_ms);
    let plan = snapshot
        .current_plan
        .revise(steps, input.rationale, now_ms)
        .map_err(goal_input_error)?;
    snapshot
        .goal
        .adopt_plan(&plan, now_ms)
        .map_err(goal_input_error)?;
    repository
        .revise(&snapshot.goal, &plan)
        .map_err(goal_storage_error)?;
    Ok(json!({"spec": "nimora.ai-goal-snapshot/1", "goal": snapshot.goal, "currentPlan": plan}))
}

fn goal_status_set(arguments: &[&str]) -> Result<Value, CliError> {
    let mut database = None;
    let mut goal_id = None;
    let mut status = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index] {
            "--database" if index + 1 < arguments.len() => {
                database = Some(arguments[index + 1]);
                index += 2;
            }
            "--goal-id" if index + 1 < arguments.len() => {
                goal_id = Some(parse_goal_id(arguments[index + 1])?);
                index += 2;
            }
            "--status" if index + 1 < arguments.len() => {
                status = Some(parse_goal_status(arguments[index + 1])?);
                index += 2;
            }
            _ => return Err(goal_usage_error()),
        }
    }
    let database = database.ok_or_else(goal_usage_error)?;
    let goal_id = goal_id.ok_or_else(goal_usage_error)?;
    let status = status.ok_or_else(goal_usage_error)?;
    let repository = goal_repository(database)?;
    let mut snapshot = repository
        .get(goal_id)
        .map_err(goal_storage_error)?
        .ok_or_else(|| CliError::new("goal-not-found", "Agent Goal was not found", 5))?;
    let now_ms = current_time_ms()?.max(snapshot.goal.updated_at_ms);
    snapshot
        .goal
        .transition(status, &snapshot.current_plan, now_ms)
        .map_err(goal_input_error)?;
    repository
        .transition(&snapshot.goal)
        .map_err(goal_storage_error)?;
    Ok(
        json!({"spec": "nimora.ai-goal-snapshot/1", "goal": snapshot.goal, "currentPlan": snapshot.current_plan}),
    )
}

fn auto_repository(path: &str) -> Result<SqliteAutoModeRepository, CliError> {
    if path.is_empty() {
        return Err(CliError::new("usage", "database path cannot be empty", 2));
    }
    SqliteAutoModeRepository::open(Path::new(path)).map_err(auto_storage_error)
}

fn goal_auto_start(arguments: &[&str]) -> Result<Value, CliError> {
    let (database, goal_id, input_path) = parse_goal_identity_input(arguments)?;
    let input = read_bounded_json::<AutoModeStartInput>(input_path, "Auto Mode policy")?;
    let snapshot = goal_repository(database)?
        .get(goal_id)
        .map_err(goal_storage_error)?
        .ok_or_else(|| CliError::new("goal-not-found", "Agent Goal was not found", 5))?;
    let scanner = WorkspaceScanner::open(
        Path::new(&input.workspace_root),
        WorkspaceScanPolicy::default(),
    )
    .map_err(workspace_auto_error)?;
    let workspace = scanner
        .scan(1, None, current_time_ms()?)
        .map_err(workspace_auto_error)?;
    let policy = AutoModePolicy::new(
        input.max_cycles,
        input.max_concurrency,
        input.budget,
        input.maximum_data_classification,
        input.tool_allowlist,
        workspace.fingerprint.clone(),
    )
    .map_err(auto_input_error)?;
    let session = AutoModeSession::start(
        &snapshot.goal,
        &snapshot.current_plan,
        policy,
        current_time_ms()?.max(snapshot.goal.updated_at_ms),
    )
    .map_err(auto_input_error)?;
    let stored =
        StoredWorkspaceSnapshot::new(session.id, scanner.root_fingerprint(), workspace.clone())
            .map_err(auto_storage_error)?;
    workspace_repository(database)?
        .create(&stored)
        .map_err(auto_storage_error)?;
    auto_repository(database)?
        .create(&session)
        .map_err(auto_storage_error)?;
    Ok(json!({
        "spec": "nimora.ai-auto-mode-session/1",
        "session": session,
        "workspace": workspace
    }))
}

fn goal_auto_status(arguments: &[&str]) -> Result<Value, CliError> {
    let (database, session_id) = parse_auto_identity(arguments)?;
    let session = load_auto_session(database, session_id)?;
    Ok(json!({"spec": "nimora.ai-auto-mode-session/1", "session": session}))
}

fn goal_auto_pause(arguments: &[&str]) -> Result<Value, CliError> {
    let (database, session_id) = parse_auto_identity(arguments)?;
    let repository = auto_repository(database)?;
    let mut session = load_auto_session_from(&repository, session_id)?;
    let previous = session.updated_at_ms;
    session
        .pause(
            AutoModePauseReason::UserRequested,
            current_time_ms()?.max(previous),
        )
        .map_err(auto_input_error)?;
    repository
        .update(&session, previous)
        .map_err(auto_storage_error)?;
    Ok(json!({"spec": "nimora.ai-auto-mode-session/1", "session": session}))
}

fn goal_auto_resume(arguments: &[&str]) -> Result<Value, CliError> {
    let (database, session_id, workspace_root) = parse_auto_resume(arguments)?;
    let repository = auto_repository(database)?;
    let mut session = load_auto_session_from(&repository, session_id)?;
    let snapshot = goal_repository(database)?
        .get(session.goal_id)
        .map_err(goal_storage_error)?
        .ok_or_else(|| CliError::new("goal-not-found", "Agent Goal was not found", 5))?;
    let previous = session.updated_at_ms;
    let fingerprint = session.policy_fingerprint.clone();
    let workspace_repository = workspace_repository(database)?;
    let stored = workspace_repository
        .latest(session_id)
        .map_err(auto_storage_error)?
        .ok_or_else(|| CliError::new("workspace", "Auto Mode workspace snapshot is missing", 4))?;
    let scanner = WorkspaceScanner::open(Path::new(workspace_root), WorkspaceScanPolicy::default())
        .map_err(workspace_auto_error)?;
    if scanner.root_fingerprint() != stored.root_fingerprint {
        return Err(CliError::new(
            "workspace-changed",
            "Auto Mode workspace root changed",
            3,
        ));
    }
    let candidate = scanner
        .scan(
            stored.snapshot.revision,
            stored.snapshot.parent_fingerprint.clone(),
            current_time_ms()?,
        )
        .map_err(workspace_auto_error)?;
    if candidate.fingerprint != stored.snapshot.fingerprint {
        let successor = scanner
            .scan(
                stored.snapshot.revision.saturating_add(1),
                Some(stored.snapshot.fingerprint.clone()),
                current_time_ms()?,
            )
            .map_err(workspace_auto_error)?;
        let successor =
            StoredWorkspaceSnapshot::new(session_id, stored.root_fingerprint, successor)
                .map_err(auto_storage_error)?;
        workspace_repository
            .append(
                &successor,
                stored.snapshot.revision,
                &stored.snapshot.fingerprint,
            )
            .map_err(auto_storage_error)?;
        return Err(CliError::new(
            "workspace-changed",
            "Auto Mode workspace contents changed",
            3,
        ));
    }
    session
        .resume(
            &snapshot.goal,
            &snapshot.current_plan,
            &candidate.fingerprint,
            &fingerprint,
            current_time_ms()?.max(previous),
        )
        .map_err(auto_input_error)?;
    repository
        .update(&session, previous)
        .map_err(auto_storage_error)?;
    Ok(json!({"spec": "nimora.ai-auto-mode-session/1", "session": session}))
}

fn goal_auto_cancel(arguments: &[&str]) -> Result<Value, CliError> {
    let (database, session_id) = parse_auto_identity(arguments)?;
    let repository = auto_repository(database)?;
    let mut session = load_auto_session_from(&repository, session_id)?;
    let previous = session.updated_at_ms;
    session
        .cancel(current_time_ms()?.max(previous))
        .map_err(auto_input_error)?;
    repository
        .update(&session, previous)
        .map_err(auto_storage_error)?;
    Ok(json!({"spec": "nimora.ai-auto-mode-session/1", "session": session}))
}

fn load_auto_session(database: &str, id: uuid::Uuid) -> Result<AutoModeSession, CliError> {
    load_auto_session_from(&auto_repository(database)?, id)
}

fn load_auto_session_from(
    repository: &SqliteAutoModeRepository,
    id: uuid::Uuid,
) -> Result<AutoModeSession, CliError> {
    repository
        .get(id)
        .map_err(auto_storage_error)?
        .ok_or_else(|| {
            CliError::new(
                "auto-session-not-found",
                "Auto Mode session was not found",
                5,
            )
        })
}

fn parse_auto_identity<'a>(arguments: &'a [&'a str]) -> Result<(&'a str, uuid::Uuid), CliError> {
    let mut database = None;
    let mut session_id = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index] {
            "--database" if index + 1 < arguments.len() => {
                database = Some(arguments[index + 1]);
                index += 2;
            }
            "--session-id" if index + 1 < arguments.len() => {
                session_id = Some(parse_session_id(arguments[index + 1])?);
                index += 2;
            }
            _ => return Err(auto_usage_error()),
        }
    }
    Ok((
        database.ok_or_else(auto_usage_error)?,
        session_id.ok_or_else(auto_usage_error)?,
    ))
}

fn parse_auto_resume<'a>(
    arguments: &'a [&'a str],
) -> Result<(&'a str, uuid::Uuid, &'a str), CliError> {
    let mut database = None;
    let mut session_id = None;
    let mut workspace_root = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index] {
            "--database" if index + 1 < arguments.len() => {
                database = Some(arguments[index + 1]);
                index += 2;
            }
            "--session-id" if index + 1 < arguments.len() => {
                session_id = Some(parse_session_id(arguments[index + 1])?);
                index += 2;
            }
            "--workspace-root" if index + 1 < arguments.len() => {
                workspace_root = Some(arguments[index + 1]);
                index += 2;
            }
            _ => return Err(auto_usage_error()),
        }
    }
    Ok((
        database.ok_or_else(auto_usage_error)?,
        session_id.ok_or_else(auto_usage_error)?,
        workspace_root.ok_or_else(auto_usage_error)?,
    ))
}

fn parse_session_id(value: &str) -> Result<uuid::Uuid, CliError> {
    uuid::Uuid::parse_str(value)
        .map_err(|_| CliError::new("usage", "Auto Mode session ID is invalid", 2))
}

fn auto_usage_error() -> CliError {
    CliError::new("usage", "Auto Mode command arguments are invalid", 2)
}

fn auto_input_error(_: impl std::fmt::Display) -> CliError {
    CliError::new("auto-mode-input", "Auto Mode request is invalid", 3)
}

fn auto_storage_error(_: impl std::fmt::Display) -> CliError {
    CliError::new(
        "auto-mode-storage",
        "Auto Mode storage operation failed",
        10,
    )
}

fn workspace_repository(path: &str) -> Result<SqliteWorkspaceSnapshotRepository, CliError> {
    SqliteWorkspaceSnapshotRepository::open(Path::new(path)).map_err(auto_storage_error)
}

fn workspace_auto_error(_: impl std::fmt::Display) -> CliError {
    CliError::new("workspace", "Auto Mode workspace scan failed closed", 4)
}

fn parse_goal_database_input<'a>(arguments: &'a [&'a str]) -> Result<(&'a str, &'a str), CliError> {
    let mut database = None;
    let mut input = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index] {
            "--database" if index + 1 < arguments.len() => {
                database = Some(arguments[index + 1]);
                index += 2;
            }
            "--input" if index + 1 < arguments.len() => {
                input = Some(arguments[index + 1]);
                index += 2;
            }
            _ => return Err(goal_usage_error()),
        }
    }
    Ok((
        database.ok_or_else(goal_usage_error)?,
        input.ok_or_else(goal_usage_error)?,
    ))
}

fn parse_goal_identity<'a>(arguments: &'a [&'a str]) -> Result<(&'a str, uuid::Uuid), CliError> {
    let mut database = None;
    let mut goal_id = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index] {
            "--database" if index + 1 < arguments.len() => {
                database = Some(arguments[index + 1]);
                index += 2;
            }
            "--goal-id" if index + 1 < arguments.len() => {
                goal_id = Some(parse_goal_id(arguments[index + 1])?);
                index += 2;
            }
            _ => return Err(goal_usage_error()),
        }
    }
    Ok((
        database.ok_or_else(goal_usage_error)?,
        goal_id.ok_or_else(goal_usage_error)?,
    ))
}

fn parse_goal_identity_input<'a>(
    arguments: &'a [&'a str],
) -> Result<(&'a str, uuid::Uuid, &'a str), CliError> {
    let mut database = None;
    let mut goal_id = None;
    let mut input = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index] {
            "--database" if index + 1 < arguments.len() => {
                database = Some(arguments[index + 1]);
                index += 2;
            }
            "--goal-id" if index + 1 < arguments.len() => {
                goal_id = Some(parse_goal_id(arguments[index + 1])?);
                index += 2;
            }
            "--input" if index + 1 < arguments.len() => {
                input = Some(arguments[index + 1]);
                index += 2;
            }
            _ => return Err(goal_usage_error()),
        }
    }
    Ok((
        database.ok_or_else(goal_usage_error)?,
        goal_id.ok_or_else(goal_usage_error)?,
        input.ok_or_else(goal_usage_error)?,
    ))
}

fn parse_goal_id(value: &str) -> Result<uuid::Uuid, CliError> {
    uuid::Uuid::parse_str(value).map_err(|_| CliError::new("usage", "Goal ID is invalid", 2))
}

fn parse_goal_status(value: &str) -> Result<AgentGoalStatus, CliError> {
    match value {
        "active" => Ok(AgentGoalStatus::Active),
        "paused" => Ok(AgentGoalStatus::Paused),
        "completed" => Ok(AgentGoalStatus::Completed),
        "cancelled" => Ok(AgentGoalStatus::Cancelled),
        _ => Err(CliError::new("usage", "Goal status is invalid", 2)),
    }
}

fn read_bounded_json<T: DeserializeOwned>(path: &str, label: &str) -> Result<T, CliError> {
    let bytes = if path == "-" {
        let mut bytes = Vec::new();
        io::stdin()
            .take(256 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| CliError::new("input", "cannot read standard input", 3))?;
        bytes
    } else {
        fs::read(Path::new(path))
            .map_err(|_| CliError::new("input", "cannot read input file", 3))?
    };
    if bytes.len() > 256 * 1024 {
        return Err(CliError::new("input", "input file exceeds 256 KiB", 3));
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| CliError::new("input", format!("{label} is invalid"), 3))
}

fn current_time_ms() -> Result<u64, CliError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| CliError::new("clock", "system clock is before Unix epoch", 10))?
        .as_millis()
        .try_into()
        .map_err(|_| CliError::new("clock", "system clock is outside supported range", 10))
}

fn goal_input_error(_: impl std::fmt::Display) -> CliError {
    CliError::new(
        "goal-input",
        "Agent Goal input violates its bounded contract",
        3,
    )
}

fn goal_storage_error(_: impl std::fmt::Display) -> CliError {
    CliError::new("goal-storage", "Agent Goal storage operation failed", 10)
}

fn goal_usage_error() -> CliError {
    CliError::new("usage", "Agent Goal command arguments are invalid", 2)
}

fn registry(
    sidecar: Option<SidecarConfig<'_>>,
    ollama_port: u16,
) -> Result<ProviderRegistry, CliError> {
    let mut providers = ProviderRegistry::default();
    providers
        .register(DeterministicLocalProvider::new().map_err(runtime_error)?)
        .map_err(runtime_error)?;
    if let Some(config) = sidecar {
        let verified = verify_provider_sidecar(
            Path::new(config.root),
            "ollama-provider.json",
            config.manifest_sha256,
        )
        .map_err(|_| {
            CliError::new(
                "sidecar-integrity",
                "Ollama provider sidecar integrity verification failed",
                4,
            )
        })?;
        let endpoint = OllamaEndpoint::new(
            "127.0.0.1".parse().expect("constant loopback address"),
            ollama_port,
        )
        .map_err(runtime_error)?;
        providers
            .register(
                WorkerOllamaProvider::new(verified.executable_path, endpoint)
                    .map_err(runtime_error)?,
            )
            .map_err(runtime_error)?;
    }
    Ok(providers)
}

fn provider_list() -> Result<Value, CliError> {
    let providers = registry(None, default_ollama_port())?;
    Ok(json!({"spec": "nimora.ai-provider-list/1", "providers": providers.descriptors()}))
}

fn provider_probe() -> Result<Value, CliError> {
    let output = execute(
        RunInput {
            prompt: "nimora-provider-probe".to_owned(),
            model: default_model(),
            provider_id: default_provider(),
            max_output_tokens: 32,
            ollama_port: default_ollama_port(),
        },
        true,
    )?;
    Ok(
        json!({"spec": "nimora.ai-provider-probe/1", "providerId": PROVIDER_ID, "healthy": true, "usage": output["usage"]}),
    )
}

fn tool_list() -> Result<Value, CliError> {
    let tools = production_tool_registry().map_err(runtime_error)?;
    Ok(json!({"spec": "nimora.ai-tool-list/1", "tools": tools.descriptors()}))
}

fn tool_describe(tool_id: &str) -> Result<Value, CliError> {
    let tools = production_tool_registry().map_err(runtime_error)?;
    tools
        .descriptors()
        .into_iter()
        .find(|descriptor| descriptor.id.to_string() == tool_id)
        .map(|descriptor| json!({"spec": "nimora.ai-tool-description/1", "tool": descriptor}))
        .ok_or_else(|| {
            CliError::new(
                "tool-not-found",
                format!("tool is not registered: {tool_id}"),
                4,
            )
        })
}

fn run_task(arguments: &[&str]) -> Result<Value, CliError> {
    let mut input_path = None;
    let mut offline = false;
    let mut json_output = false;
    let mut sidecar_root = None;
    let mut sidecar_manifest_sha256 = None;
    let mut history_database = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index] {
            "--input" if index + 1 < arguments.len() => {
                input_path = Some(arguments[index + 1]);
                index += 2;
            }
            "--output" if index + 1 < arguments.len() && arguments[index + 1] == "json" => {
                json_output = true;
                index += 2;
            }
            "--offline" => {
                offline = true;
                index += 1;
            }
            "--sidecar-root" if index + 1 < arguments.len() => {
                sidecar_root = Some(arguments[index + 1]);
                index += 2;
            }
            "--sidecar-manifest-sha256" if index + 1 < arguments.len() => {
                sidecar_manifest_sha256 = Some(arguments[index + 1]);
                index += 2;
            }
            "--history-database" if index + 1 < arguments.len() => {
                history_database = Some(arguments[index + 1]);
                index += 2;
            }
            _ => {
                return Err(CliError::new(
                    "usage",
                    "run requires --input <path|-> and --output json",
                    2,
                ));
            }
        }
    }
    let input_path = input_path.ok_or_else(|| CliError::new("usage", "missing --input", 2))?;
    if !json_output {
        return Err(CliError::new("usage", "missing --output json", 2));
    }
    let sidecar = match (sidecar_root, sidecar_manifest_sha256) {
        (Some(root), Some(manifest_sha256)) => Some(SidecarConfig {
            root,
            manifest_sha256,
        }),
        (None, None) => None,
        _ => {
            return Err(CliError::new(
                "usage",
                "--sidecar-root and --sidecar-manifest-sha256 must be provided together",
                2,
            ));
        }
    };
    let bytes = if input_path == "-" {
        let mut bytes = Vec::new();
        io::stdin()
            .take(256 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| CliError::new("input", "cannot read standard input", 3))?;
        bytes
    } else {
        fs::read(Path::new(input_path))
            .map_err(|_| CliError::new("input", "cannot read input file", 3))?
    };
    if bytes.len() > 256 * 1024 {
        return Err(CliError::new("input", "input file exceeds 256 KiB", 3));
    }
    let input: RunInput = serde_json::from_slice(&bytes)
        .map_err(|_| CliError::new("input", "input must match the bounded task schema", 3))?;
    execute_with_sidecar(input, offline, sidecar, history_database)
}

fn execute(input: RunInput, offline: bool) -> Result<Value, CliError> {
    execute_with_sidecar(input, offline, None, None)
}

fn execute_with_sidecar(
    input: RunInput,
    offline: bool,
    sidecar: Option<SidecarConfig<'_>>,
    history_database: Option<&str>,
) -> Result<Value, CliError> {
    if input.prompt.is_empty()
        || input.prompt.len() > 256 * 1024
        || !matches!(input.provider_id.as_str(), PROVIDER_ID | OLLAMA_PROVIDER_ID)
        || input.ollama_port == 0
    {
        return Err(CliError::new(
            "input",
            "task prompt or provider is invalid",
            3,
        ));
    }
    if input.provider_id == OLLAMA_PROVIDER_ID && sidecar.is_none() {
        return Err(CliError::new(
            "sidecar-required",
            "Ollama provider requires a verified provider sidecar",
            4,
        ));
    }
    let providers = registry(sidecar, input.ollama_port)?;
    let tools = production_tool_registry().map_err(runtime_error)?;
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| CliError::new("clock", "system clock is before Unix epoch", 10))?
        .as_millis()
        .try_into()
        .map_err(|_| CliError::new("clock", "system clock is outside supported range", 10))?;
    let tool_ids = tools
        .descriptors()
        .into_iter()
        .map(|descriptor| descriptor.id.to_string())
        .collect::<Vec<_>>();
    let mut task = admit_cli_agent_task(input.provider_id, tool_ids, now_ms)?;
    let coordinator = AgentCoordinator::new(&providers, &tools);
    let history_model = input.model.clone();
    let history_prompt = input.prompt.clone();
    let outcome = coordinator
        .provider_step(
            &mut task,
            ProviderStepInput {
                model: input.model,
                messages: vec![ProviderMessage::text(
                    ProviderMessageRole::User,
                    input.prompt,
                    DataClassification::Personal,
                    true,
                )],
                max_output_tokens: input.max_output_tokens,
                context: ProviderExecutionContext {
                    timeout: Duration::from_secs(30),
                    cancellation: CancellationFlag::default(),
                    credential_reference: None,
                },
                offline,
                now_ms,
            },
        )
        .map_err(|error| CliError::new("agent-runtime", error.to_string(), 10))?;
    match outcome {
        ProviderStepOutcome::Completed { response } => {
            let history_requested = history_database.is_some();
            let history_persisted = history_database.is_some_and(|path| {
                let Ok(repository) = SqliteAgentHistoryRepository::open(Path::new(path)) else {
                    return false;
                };
                let Ok(record) = AgentHistoryRecord::new(
                    task.clone(),
                    history_model,
                    history_prompt,
                    response.content.clone(),
                    response.finish_reason,
                    response.usage,
                    now_ms,
                ) else {
                    return false;
                };
                repository.insert(&record).is_ok()
            });
            Ok(json!({
                "spec": "nimora.ai-run-result/1",
                "task": task,
                "content": response.content,
                "finishReason": response.finish_reason,
                "usage": response.usage,
                "history": {
                    "requested": history_requested,
                    "persisted": history_persisted,
                    "degraded": history_requested && !history_persisted
                }
            }))
        }
        ProviderStepOutcome::ToolCalls { .. } => Err(CliError::new(
            "confirmation-required",
            "non-interactive run cannot execute requested tools",
            5,
        )),
    }
}

fn admit_cli_agent_task(
    provider_id: String,
    tool_ids: Vec<String>,
    now_ms: u64,
) -> Result<AgentTask, CliError> {
    let policy = AgentTaskGatewayPolicy::new(
        "cli:local-user",
        [AgentTaskOrigin::Cli],
        [PROVIDER_ID.to_owned(), OLLAMA_PROVIDER_ID.to_owned()],
        tool_ids.clone(),
        DataClassification::Personal,
        AgentAutonomy::Draft,
        AgentBudget::default(),
        1,
    )
    .map_err(runtime_error)?;
    AgentTaskGateway::new(policy)
        .admit(
            AgentTaskRequest::new(
                AgentTaskOrigin::Cli,
                "cli:local-user",
                provider_id,
                tool_ids,
                DataClassification::Personal,
                AgentAutonomy::Draft,
                AgentBudget::default(),
            ),
            now_ms,
        )
        .map(|admission| admission.task)
        .map_err(runtime_error)
}

fn runtime_error(error: impl std::fmt::Display) -> CliError {
    CliError::new("agent-runtime", error.to_string(), 10)
}
