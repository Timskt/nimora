use nimora_agent_provider_worker::{OllamaEndpoint, WorkerOllamaProvider, verify_provider_worker};
use nimora_agent_runtime::{
    AgentAutonomy, AgentBudget, AgentCoordinator, AgentGoal, AgentGoalStatus, AgentPlan,
    AgentPlanStep, AgentPlanStepStatus, AgentTask, AgentTaskGateway, AgentTaskGatewayPolicy,
    AgentTaskOrigin, AgentTaskRequest, ApprovalPolicy, AuthorizationGrant, AutoModeCheckpoint,
    AutoModePauseReason, AutoModePolicy, AutoModeSession, CancellationFlag, DataClassification,
    DeterministicLocalProvider, GrantLifetime, ModelReasoningPolicy, NetworkPolicy,
    ProviderExecutionContext, ProviderMessage, ProviderMessageRole, ProviderRegistry,
    ProviderStepInput, ProviderStepOutcome, SandboxScope, ToolId,
};
use nimora_agent_tools::production_tool_registry;
use nimora_agent_workspace_host::{
    GitWorkspaceAdapter, WorkspaceScanPolicy, WorkspaceScanner, unix_time_ms,
};
use nimora_persistence_sqlite::{
    AgentHistoryRecord, SqliteAgentGoalRepository, SqliteAgentHistoryRepository,
    SqliteAuthorizationGrantRepository, SqliteAutoModeCheckpointRepository,
    SqliteAutoModeRepository, SqliteWorkspaceSnapshotRepository, StoredWorkspaceSnapshot,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    collections::{BTreeSet, HashSet},
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
        ["ai", "goal", "auto", "away-summary", rest @ ..] => goal_auto_away_summary(rest)?,
        ["ai", "goal", "grant", "issue", rest @ ..] => goal_grant_issue(rest)?,
        ["ai", "goal", "grant", "list", rest @ ..] => goal_grant_list(rest)?,
        ["ai", "goal", "grant", "show", rest @ ..] => goal_grant_show(rest)?,
        ["ai", "goal", "grant", "revoke", rest @ ..] => goal_grant_revoke(rest)?,
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
            "nimora ai goal auto cancel --database <path> --session-id <uuid>",
            "nimora ai goal auto away-summary --database <path> --goal-id <uuid>",
            "nimora ai goal grant issue --database <path> --goal-id <uuid> --tier <observe|workspace|trusted_workspace|unattended|full_device> --workspace-root <path> [--reason <text>] [--offline|--online] [--tool <id>]... [--provider-id <id>] [--model <id>] [--reasoning-policy <path|->]",
            "nimora ai goal grant list --database <path> [--goal-id <uuid>] [--limit <1..200>]",
            "nimora ai goal grant show --database <path> --id <uuid>",
            "nimora ai goal grant revoke --database <path> --id <uuid>"
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
        let verified = verify_provider_worker(
            Path::new(config.root),
            "agent-provider-worker.json",
            config.manifest_sha256,
        )
        .map_err(|_| {
            CliError::new(
                "sidecar-integrity",
                "Agent provider worker integrity verification failed",
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
                reasoning: None,
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

const GRANT_SPEC: &str = "nimora.authorization-grant/1";
const GRANT_SUMMARY_SPEC: &str = "nimora.authorization-grant-summary/1";
const EIGHT_HOURS_MS: u64 = 8 * 60 * 60 * 1_000;
const FOUR_HOURS_MS: u64 = 4 * 60 * 60 * 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AuthorizationTier {
    Observe,
    Workspace,
    TrustedWorkspace,
    Unattended,
    FullDevice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum AuthorizationGrantStatus {
    Active,
    Revoked,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TierPolicy {
    sandbox: SandboxScope,
    approval: ApprovalPolicy,
    network: NetworkPolicy,
    selected_roots: BTreeSet<String>,
    lifetime: GrantLifetime,
    expires_at_ms: Option<u64>,
}

/// Maps an authorization tier onto sandbox / approval / network / lifetime constraints.
fn tier_policy(
    tier: AuthorizationTier,
    offline: bool,
    workspace_roots: &[String],
    now_ms: u64,
) -> TierPolicy {
    let restricted_network = if offline {
        NetworkPolicy::Offline
    } else {
        NetworkPolicy::LoopbackOnly
    };
    let selected_roots = workspace_roots.iter().cloned().collect::<BTreeSet<_>>();
    match tier {
        AuthorizationTier::Observe => TierPolicy {
            sandbox: SandboxScope::ReadOnly,
            approval: ApprovalPolicy::AskRisky,
            network: restricted_network,
            selected_roots: BTreeSet::new(),
            lifetime: GrantLifetime::Session,
            expires_at_ms: None,
        },
        AuthorizationTier::Workspace => TierPolicy {
            sandbox: SandboxScope::WorkspaceWrite,
            approval: ApprovalPolicy::AskRisky,
            network: restricted_network,
            selected_roots: BTreeSet::new(),
            lifetime: GrantLifetime::Session,
            expires_at_ms: None,
        },
        AuthorizationTier::TrustedWorkspace => TierPolicy {
            sandbox: SandboxScope::WorkspaceWrite,
            approval: ApprovalPolicy::NeverAskWithinGrant,
            network: restricted_network,
            selected_roots: BTreeSet::new(),
            lifetime: GrantLifetime::Session,
            expires_at_ms: None,
        },
        AuthorizationTier::Unattended => TierPolicy {
            sandbox: SandboxScope::SelectedRoots,
            approval: ApprovalPolicy::NeverAskWithinGrant,
            network: restricted_network,
            selected_roots,
            lifetime: GrantLifetime::UntilTimestamp,
            expires_at_ms: Some(now_ms.saturating_add(EIGHT_HOURS_MS)),
        },
        AuthorizationTier::FullDevice => TierPolicy {
            sandbox: SandboxScope::FullDevice,
            approval: ApprovalPolicy::NeverAskWithinGrant,
            network: if offline {
                NetworkPolicy::Offline
            } else {
                NetworkPolicy::Unrestricted
            },
            selected_roots: BTreeSet::new(),
            lifetime: GrantLifetime::UntilTimestamp,
            expires_at_ms: Some(now_ms.saturating_add(FOUR_HOURS_MS)),
        },
    }
}

/// Derives authorization tier from persisted grant sandbox + approval.
fn infer_tier(grant: &AuthorizationGrant) -> AuthorizationTier {
    match (grant.sandbox, grant.approval) {
        (SandboxScope::ReadOnly, ApprovalPolicy::AlwaysAsk | ApprovalPolicy::AskRisky) => {
            AuthorizationTier::Observe
        }
        (SandboxScope::WorkspaceWrite, ApprovalPolicy::AskRisky | ApprovalPolicy::AlwaysAsk) => {
            AuthorizationTier::Workspace
        }
        (SandboxScope::WorkspaceWrite, ApprovalPolicy::NeverAskWithinGrant) => {
            AuthorizationTier::TrustedWorkspace
        }
        (SandboxScope::SelectedRoots, ApprovalPolicy::NeverAskWithinGrant) => {
            AuthorizationTier::Unattended
        }
        (SandboxScope::FullDevice, ApprovalPolicy::NeverAskWithinGrant) => {
            AuthorizationTier::FullDevice
        }
        (SandboxScope::ReadOnly, _) => AuthorizationTier::Observe,
        (SandboxScope::WorkspaceWrite, _) => AuthorizationTier::Workspace,
        (SandboxScope::SelectedRoots, _) => AuthorizationTier::Unattended,
        (SandboxScope::FullDevice, _) => AuthorizationTier::FullDevice,
    }
}

fn grant_status(grant: &AuthorizationGrant, now_ms: u64) -> AuthorizationGrantStatus {
    if grant.revoked_at_ms.is_some() {
        AuthorizationGrantStatus::Revoked
    } else if grant
        .expires_at_ms
        .is_some_and(|expires_at_ms| now_ms >= expires_at_ms)
    {
        AuthorizationGrantStatus::Expired
    } else {
        AuthorizationGrantStatus::Active
    }
}

fn grant_summary_value(
    grant: &AuthorizationGrant,
    workspace_root: Option<String>,
    now_ms: u64,
) -> Value {
    let workspace_root = workspace_root.or_else(|| {
        grant
            .selected_roots
            .iter()
            .next()
            .cloned()
            .filter(|_| grant.sandbox == SandboxScope::SelectedRoots)
    });
    json!({
        "spec": GRANT_SUMMARY_SPEC,
        "grantId": grant.id,
        "goalId": grant.goal_id,
        "tier": infer_tier(grant),
        "status": grant_status(grant, now_ms),
        "workspaceRoot": workspace_root,
        "issuedAtMs": grant.issued_at_ms,
        "expiresAtMs": grant.expires_at_ms,
        "revokedAtMs": grant.revoked_at_ms,
    })
}

fn generous_grant_budget() -> AgentBudget {
    AgentBudget {
        max_steps: 64,
        max_tool_calls: 64,
        max_elapsed_ms: EIGHT_HOURS_MS,
        max_input_tokens: 500_000,
        max_output_tokens: 128_000,
        max_cost_microunits: 50_000_000,
    }
}

fn grant_repository(path: &str) -> Result<SqliteAuthorizationGrantRepository, CliError> {
    if path.is_empty() {
        return Err(CliError::new("usage", "数据库路径不能为空", 2));
    }
    SqliteAuthorizationGrantRepository::open(Path::new(path)).map_err(grant_storage_error)
}

fn default_grant_tools() -> Result<BTreeSet<ToolId>, CliError> {
    let tools = production_tool_registry()
        .map_err(runtime_error)?
        .descriptors()
        .into_iter()
        .map(|descriptor| descriptor.id.clone())
        .collect::<BTreeSet<_>>();
    if tools.is_empty() {
        return Err(CliError::new(
            "grant-input",
            "生产工具目录为空，无法签发授权",
            3,
        ));
    }
    Ok(tools)
}

fn parse_authorization_tier(value: &str) -> Result<AuthorizationTier, CliError> {
    match value {
        "observe" => Ok(AuthorizationTier::Observe),
        "workspace" => Ok(AuthorizationTier::Workspace),
        "trusted_workspace" => Ok(AuthorizationTier::TrustedWorkspace),
        "unattended" => Ok(AuthorizationTier::Unattended),
        "full_device" => Ok(AuthorizationTier::FullDevice),
        _ => Err(CliError::new(
            "usage",
            "授权档位无效；可用 observe|workspace|trusted_workspace|unattended|full_device",
            2,
        )),
    }
}

fn parse_grant_id(value: &str) -> Result<uuid::Uuid, CliError> {
    uuid::Uuid::parse_str(value).map_err(|_| CliError::new("usage", "授权 ID 无效", 2))
}

fn absolute_workspace_root(path: &str) -> String {
    fs::canonicalize(Path::new(path))
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_owned())
}

fn build_authorization_grant(
    tier: AuthorizationTier,
    offline: bool,
    goal_id: uuid::Uuid,
    plan_revision: u64,
    workspace_fingerprint: &str,
    workspace_roots: &[String],
    tools: BTreeSet<ToolId>,
    provider_id: &str,
    model: &str,
    now_ms: u64,
) -> Result<AuthorizationGrant, CliError> {
    if tools.is_empty() {
        return Err(CliError::new(
            "grant-input",
            "授权需要非空工具白名单",
            3,
        ));
    }
    let policy = tier_policy(tier, offline, workspace_roots, now_ms);
    let grant = AuthorizationGrant {
        spec: GRANT_SPEC.to_owned(),
        id: uuid::Uuid::now_v7(),
        goal_id,
        plan_revision,
        workspace_fingerprint: workspace_fingerprint.to_owned(),
        sandbox: policy.sandbox,
        approval: policy.approval,
        network: policy.network,
        selected_roots: policy.selected_roots,
        tool_allowlist: tools,
        provider_allowlist: BTreeSet::from([provider_id.to_owned()]),
        model_allowlist: BTreeSet::from([model.to_owned()]),
        maximum_data_classification: DataClassification::Personal,
        budget: generous_grant_budget(),
        lifetime: policy.lifetime,
        issued_at_ms: now_ms,
        expires_at_ms: policy.expires_at_ms,
        revoked_at_ms: None,
    };
    grant.validate().map_err(grant_input_error)?;
    Ok(grant)
}

fn goal_grant_issue(arguments: &[&str]) -> Result<Value, CliError> {
    let mut database = None;
    let mut goal_id = None;
    let mut tier = None;
    let mut workspace_roots = Vec::new();
    let mut reason = None;
    let mut offline = true;
    let mut provider_id = None;
    let mut model = None;
    let mut tools = Vec::new();
    let mut reasoning_policy_path = None;
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
            "--tier" if index + 1 < arguments.len() => {
                tier = Some(parse_authorization_tier(arguments[index + 1])?);
                index += 2;
            }
            "--workspace-root" if index + 1 < arguments.len() => {
                workspace_roots.push(arguments[index + 1].to_owned());
                index += 2;
            }
            "--reason" if index + 1 < arguments.len() => {
                reason = Some(arguments[index + 1].to_owned());
                index += 2;
            }
            "--offline" => {
                offline = true;
                index += 1;
            }
            "--online" => {
                offline = false;
                index += 1;
            }
            "--provider-id" if index + 1 < arguments.len() => {
                provider_id = Some(arguments[index + 1].to_owned());
                index += 2;
            }
            "--model" if index + 1 < arguments.len() => {
                model = Some(arguments[index + 1].to_owned());
                index += 2;
            }
            "--tool" if index + 1 < arguments.len() => {
                tools.push(arguments[index + 1].to_owned());
                index += 2;
            }
            "--reasoning-policy" if index + 1 < arguments.len() => {
                reasoning_policy_path = Some(arguments[index + 1]);
                index += 2;
            }
            _ => return Err(grant_usage_error()),
        }
    }
    let database = database.ok_or_else(grant_usage_error)?;
    let goal_id = goal_id.ok_or_else(grant_usage_error)?;
    let tier = tier.ok_or_else(grant_usage_error)?;
    if workspace_roots.is_empty() {
        return Err(CliError::new(
            "usage",
            "签发授权需要至少一个 --workspace-root",
            2,
        ));
    }
    let reason = reason.unwrap_or_else(|| "cli-authorization-grant".to_owned());
    if reason.trim().is_empty() || reason.len() > 500 {
        return Err(CliError::new(
            "grant-input",
            "授权原因不能为空且不超过 500 字节",
            3,
        ));
    }
    let reasoning_policy = reasoning_policy_path
        .map(|path| read_bounded_json::<ModelReasoningPolicy>(path, "推理策略"))
        .transpose()?;
    let snapshot = goal_repository(database)?
        .get(goal_id)
        .map_err(goal_storage_error)?
        .ok_or_else(|| CliError::new("goal-not-found", "未找到对应的 Agent Goal", 5))?;
    let primary_root = absolute_workspace_root(&workspace_roots[0]);
    let normalized_roots = workspace_roots
        .iter()
        .map(|root| absolute_workspace_root(root))
        .collect::<Vec<_>>();
    let scanner = WorkspaceScanner::open(Path::new(&primary_root), WorkspaceScanPolicy::default())
        .map_err(|_| CliError::new("workspace", "无法安全打开工作区以签发授权", 4))?;
    let now_ms = current_time_ms()?.max(snapshot.goal.updated_at_ms);
    let workspace = scanner
        .scan(1, None, now_ms)
        .map_err(|_| CliError::new("workspace", "工作区扫描失败，无法签发授权", 4))?;
    let tool_allowlist = if tools.is_empty() {
        default_grant_tools()?
    } else {
        tools
            .into_iter()
            .map(|tool| {
                tool.parse::<ToolId>().map_err(|_| {
                    CliError::new("grant-input", format!("工具 ID 无效: {tool}"), 3)
                })
            })
            .collect::<Result<BTreeSet<_>, _>>()?
    };
    let provider_id = provider_id.unwrap_or_else(|| PROVIDER_ID.to_owned());
    let model = model.unwrap_or_else(default_model);
    let grant = build_authorization_grant(
        tier,
        offline,
        snapshot.goal.id,
        snapshot.current_plan.revision,
        &workspace.fingerprint,
        &normalized_roots,
        tool_allowlist,
        &provider_id,
        &model,
        now_ms,
    )?;
    grant_repository(database)?
        .issue(&grant)
        .map_err(grant_storage_error)?;
    Ok(json!({
        "spec": "nimora.ai-authorization-grant/1",
        "grant": grant,
        "summary": grant_summary_value(&grant, Some(primary_root), now_ms),
        "reason": reason,
        "reasoningPolicy": reasoning_policy,
        "workspace": {
            "fingerprint": workspace.fingerprint,
            "revision": workspace.revision,
            "root": absolute_workspace_root(&workspace_roots[0]),
        }
    }))
}

fn goal_grant_list(arguments: &[&str]) -> Result<Value, CliError> {
    let mut database = None;
    let mut goal_id = None;
    let mut limit = 50_usize;
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
            "--limit" if index + 1 < arguments.len() => {
                limit = arguments[index + 1].parse().map_err(|_| {
                    CliError::new("usage", "授权列表 limit 无效", 2)
                })?;
                index += 2;
            }
            _ => return Err(grant_usage_error()),
        }
    }
    let database = database.ok_or_else(grant_usage_error)?;
    if !(1..=200).contains(&limit) {
        return Err(CliError::new("usage", "授权列表 limit 必须在 1..200", 2));
    }
    let now_ms = current_time_ms()?;
    let repository = grant_repository(database)?;
    let mut grants = Vec::new();
    if let Some(goal_id) = goal_id {
        grants.extend(
            repository
                .list_for_goal(goal_id, limit)
                .map_err(grant_storage_error)?,
        );
    } else {
        let mut seen = HashSet::new();
        for goal in goal_repository(database)?
            .list(100)
            .map_err(goal_storage_error)?
        {
            for grant in repository
                .list_for_goal(goal.id, 32)
                .map_err(grant_storage_error)?
            {
                if seen.insert(grant.id) {
                    grants.push(grant);
                }
            }
        }
        grants.sort_by(|left, right| {
            right
                .issued_at_ms
                .cmp(&left.issued_at_ms)
                .then_with(|| right.id.cmp(&left.id))
        });
        grants.truncate(limit);
    }
    let summaries = grants
        .iter()
        .map(|grant| grant_summary_value(grant, None, now_ms))
        .collect::<Vec<_>>();
    Ok(json!({
        "spec": "nimora.ai-authorization-grant-list/1",
        "grants": summaries
    }))
}

fn goal_grant_show(arguments: &[&str]) -> Result<Value, CliError> {
    let (database, grant_id) = parse_grant_identity(arguments)?;
    let now_ms = current_time_ms()?;
    let grant = grant_repository(database)?
        .get(grant_id)
        .map_err(grant_storage_error)?
        .ok_or_else(|| CliError::new("grant-not-found", "未找到授权凭证", 5))?;
    Ok(json!({
        "spec": "nimora.ai-authorization-grant/1",
        "grant": grant,
        "summary": grant_summary_value(&grant, None, now_ms)
    }))
}

fn goal_grant_revoke(arguments: &[&str]) -> Result<Value, CliError> {
    let (database, grant_id) = parse_grant_identity(arguments)?;
    let now_ms = current_time_ms()?;
    let repository = grant_repository(database)?;
    let grant = match repository.revoke(grant_id, now_ms) {
        Ok(grant) => grant,
        Err(nimora_persistence_sqlite::SqlitePersistenceError::AuthorizationGrantAlreadyRevoked) => {
            repository
                .get(grant_id)
                .map_err(grant_storage_error)?
                .ok_or_else(|| CliError::new("grant-not-found", "未找到授权凭证", 5))?
        }
        Err(nimora_persistence_sqlite::SqlitePersistenceError::AuthorizationGrantNotFound) => {
            return Err(CliError::new("grant-not-found", "未找到授权凭证", 5));
        }
        Err(_) => return Err(grant_storage_error("revoke failed")),
    };
    Ok(json!({
        "spec": "nimora.ai-authorization-grant-revoke/1",
        "grantId": grant.id,
        "revoked": true,
        "grant": grant,
        "summary": grant_summary_value(&grant, None, now_ms)
    }))
}

fn parse_grant_identity<'a>(arguments: &'a [&'a str]) -> Result<(&'a str, uuid::Uuid), CliError> {
    let mut database = None;
    let mut grant_id = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index] {
            "--database" if index + 1 < arguments.len() => {
                database = Some(arguments[index + 1]);
                index += 2;
            }
            "--id" | "--grant-id" if index + 1 < arguments.len() => {
                grant_id = Some(parse_grant_id(arguments[index + 1])?);
                index += 2;
            }
            _ => return Err(grant_usage_error()),
        }
    }
    Ok((
        database.ok_or_else(grant_usage_error)?,
        grant_id.ok_or_else(grant_usage_error)?,
    ))
}

fn grant_usage_error() -> CliError {
    CliError::new("usage", "授权命令参数无效；请使用 --help 查看用法", 2)
}

fn grant_input_error(_: impl std::fmt::Display) -> CliError {
    CliError::new("grant-input", "授权请求不满足边界契约", 3)
}

fn grant_storage_error(_: impl std::fmt::Display) -> CliError {
    CliError::new("grant-storage", "授权存储操作失败", 10)
}

fn list_auto_session_ids_for_goal(
    database: &str,
    goal_id: uuid::Uuid,
    limit: usize,
) -> Result<Vec<uuid::Uuid>, CliError> {
    let connection = match rusqlite::Connection::open(Path::new(database)) {
        Ok(connection) => connection,
        Err(_) => return Ok(Vec::new()),
    };
    let mut statement = match connection.prepare(
        "SELECT session_id FROM auto_mode_session
         WHERE goal_id = ?1
         ORDER BY updated_at_ms DESC, session_id DESC
         LIMIT ?2",
    ) {
        Ok(statement) => statement,
        Err(_) => return Ok(Vec::new()),
    };
    let rows = match statement.query_map(
        rusqlite::params![goal_id.to_string(), i64::try_from(limit).unwrap_or(64)],
        |row| row.get::<_, String>(0),
    ) {
        Ok(rows) => rows,
        Err(_) => return Ok(Vec::new()),
    };
    let mut ids = Vec::new();
    for row in rows {
        let Ok(value) = row else {
            continue;
        };
        if let Ok(id) = uuid::Uuid::parse_str(&value) {
            ids.push(id);
        }
    }
    Ok(ids)
}

fn extract_last_assistant_text(checkpoint: &AutoModeCheckpoint) -> Option<String> {
    checkpoint
        .messages
        .iter()
        .rev()
        .find(|message| message.role == ProviderMessageRole::Assistant)
        .map(|message| message.content.chars().take(200).collect::<String>())
        .filter(|content| !content.is_empty())
}

fn goal_auto_away_summary(arguments: &[&str]) -> Result<Value, CliError> {
    let (database, goal_id) = parse_goal_identity(arguments)?;
    let now_ms = current_time_ms()?;
    let snapshot = goal_repository(database)?
        .get(goal_id)
        .map_err(goal_storage_error)?
        .ok_or_else(|| CliError::new("goal-not-found", "未找到对应的 Agent Goal", 5))?;
    let grants = grant_repository(database)?
        .list_for_goal(goal_id, 64)
        .map_err(grant_storage_error)?;
    let session_ids = list_auto_session_ids_for_goal(database, goal_id, 64)?;
    let auto_repo = auto_repository(database)?;
    let checkpoint_repo = SqliteAutoModeCheckpointRepository::open(Path::new(database))
        .map_err(auto_storage_error)?;
    let workspace_repo = workspace_repository(database)?;
    let mut sessions = Vec::new();
    let mut pauses = Vec::new();
    let mut total_input_tokens = 0_u64;
    let mut total_output_tokens = 0_u64;
    let mut total_cost_microunits = 0_u64;
    let mut total_elapsed_ms = 0_u64;
    let mut total_cycles = 0_u32;
    let mut total_tool_calls = 0_u32;
    let mut last_speech = None;
    let mut last_directives = Value::Null;
    let mut workspace_changes = Value::Null;
    for session_id in session_ids {
        let Some(session) = auto_repo.get(session_id).map_err(auto_storage_error)? else {
            continue;
        };
        total_input_tokens = total_input_tokens.saturating_add(session.usage.input_tokens);
        total_output_tokens = total_output_tokens.saturating_add(session.usage.output_tokens);
        total_cost_microunits = total_cost_microunits.saturating_add(session.usage.cost_microunits);
        total_elapsed_ms = total_elapsed_ms.saturating_add(session.usage.elapsed_ms);
        total_cycles = total_cycles.saturating_add(session.usage.cycles);
        total_tool_calls = total_tool_calls.saturating_add(session.usage.tool_calls);
        if let Some(reason) = session.pause_reason {
            pauses.push(json!({
                "sessionId": session.id,
                "reason": reason,
                "updatedAtMs": session.updated_at_ms
            }));
        }
        let checkpoint = checkpoint_repo
            .get(session.id)
            .map_err(auto_storage_error)?;
        if let Some(checkpoint) = checkpoint.as_ref() {
            if last_speech.is_none() {
                last_speech = extract_last_assistant_text(checkpoint);
            }
            if last_directives.is_null() {
                // CLI host does not persist pet directives; surface checkpoint model/sequence only.
                last_directives = json!({
                    "source": "auto-mode-checkpoint",
                    "sessionId": checkpoint.session_id,
                    "sequence": checkpoint.sequence,
                    "model": checkpoint.model,
                    "messageCount": checkpoint.messages.len()
                });
            }
        }
        let workspace = workspace_repo
            .latest(session.id)
            .map_err(auto_storage_error)?;
        if workspace_changes.is_null() {
            if let Some(stored) = workspace.as_ref() {
                workspace_changes = json!({
                    "sessionId": session.id,
                    "revision": stored.snapshot.revision,
                    "fingerprint": stored.snapshot.fingerprint,
                    "fileCount": stored.snapshot.files.len(),
                    "files": stored
                        .snapshot
                        .files
                        .iter()
                        .take(64)
                        .map(|file| file.relative_path.clone())
                        .collect::<Vec<_>>(),
                });
            }
        }
        sessions.push(json!({
            "sessionId": session.id,
            "status": session.status,
            "pauseReason": session.pause_reason,
            "planRevision": session.plan_revision,
            "usage": session.usage,
            "createdAtMs": session.created_at_ms,
            "updatedAtMs": session.updated_at_ms,
            "checkpointSequence": checkpoint.as_ref().map(|item| item.sequence),
            "workspaceRevision": workspace
                .as_ref()
                .map(|item| item.snapshot.revision),
            "workspaceFingerprint": workspace
                .as_ref()
                .map(|item| item.snapshot.fingerprint.clone()),
        }));
    }
    let completed_steps = snapshot
        .current_plan
        .steps
        .iter()
        .filter(|step| step.status == AgentPlanStepStatus::Completed)
        .count();
    let next_step = snapshot
        .current_plan
        .steps
        .iter()
        .find(|step| {
            matches!(
                step.status,
                AgentPlanStepStatus::Pending | AgentPlanStepStatus::InProgress
            )
        })
        .map(|step| {
            json!({
                "id": step.id,
                "text": step.text,
                "status": step.status
            })
        });
    let current_pause_reason = sessions
        .iter()
        .find_map(|session| session.get("pauseReason").cloned().filter(|value| !value.is_null()));
    let revoke_grant_ids = grants
        .iter()
        .filter(|grant| matches!(grant_status(grant, now_ms), AuthorizationGrantStatus::Active))
        .map(|grant| grant.id)
        .collect::<Vec<_>>();
    let active_grants = grants
        .iter()
        .filter(|grant| matches!(grant_status(grant, now_ms), AuthorizationGrantStatus::Active))
        .count();
    let mut highlights = Vec::new();
    let goal_status = serde_json::to_value(snapshot.goal.status)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_owned());
    highlights.push(format!(
        "目标「{}」状态为 {}，计划进度 {}/{}",
        snapshot.goal.title,
        goal_status,
        completed_steps,
        snapshot.current_plan.steps.len()
    ));
    highlights.push(format!(
        "关联 Auto Mode 会话 {} 个，活跃授权 {} 个",
        sessions.len(),
        active_grants
    ));
    if let Some(reason) = current_pause_reason.as_ref() {
        let reason_text = reason
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| reason.to_string());
        highlights.push(format!("当前暂停原因：{reason_text}"));
    }
    if let Some(step) = next_step.as_ref() {
        if let Some(text_value) = step.get("text").and_then(Value::as_str) {
            highlights.push(format!("下一步：{text_value}"));
        }
    }
    highlights.push(format!(
        "累计用量：cycles={total_cycles} tools={total_tool_calls} in={total_input_tokens} out={total_output_tokens} costμ={total_cost_microunits}"
    ));
    if !workspace_changes.is_null() {
        if let Some(count) = workspace_changes.get("fileCount").and_then(Value::as_u64) {
            highlights.push(format!("工作区快照文件数：{count}"));
        }
    }
    let mut risk_notes = Vec::new();
    for grant in &grants {
        if !matches!(grant_status(grant, now_ms), AuthorizationGrantStatus::Active) {
            continue;
        }
        match infer_tier(grant) {
            AuthorizationTier::Unattended => risk_notes.push(format!(
                "授权 {} 为 unattended（SelectedRoots + NeverAskWithinGrant），离开期间可能自动推进可逆写入",
                grant.id
            )),
            AuthorizationTier::FullDevice => risk_notes.push(format!(
                "授权 {} 为 full_device，可影响本机文件、命令与联网；请确认后一键撤销",
                grant.id
            )),
            AuthorizationTier::TrustedWorkspace => risk_notes.push(format!(
                "授权 {} 为 trusted_workspace，工作区内写入在 Grant 有效期内可不询问",
                grant.id
            )),
            _ => {}
        }
        if matches!(grant.network, NetworkPolicy::Unrestricted) {
            risk_notes.push(format!(
                "授权 {} 允许 unrestricted 网络，数据可能离开设备",
                grant.id
            ));
        }
    }
    Ok(json!({
        "spec": "nimora.ai-away-summary/1",
        "goalId": goal_id,
        "generatedAtMs": now_ms,
        "highlights": highlights,
        "riskNotes": risk_notes,
        "goal": {
            "id": snapshot.goal.id,
            "title": snapshot.goal.title,
            "objective": snapshot.goal.objective,
            "status": snapshot.goal.status,
            "currentPlanRevision": snapshot.goal.current_plan_revision,
            "updatedAtMs": snapshot.goal.updated_at_ms
        },
        "plan": {
            "revision": snapshot.current_plan.revision,
            "completedSteps": completed_steps,
            "totalSteps": snapshot.current_plan.steps.len(),
            "steps": snapshot.current_plan.steps.iter().map(|step| json!({
                "id": step.id,
                "text": step.text,
                "status": step.status,
                "evidenceCount": step.evidence.len()
            })).collect::<Vec<_>>(),
            "nextStep": next_step
        },
        "autoSessions": sessions,
        "pauses": pauses,
        "currentPauseReason": current_pause_reason,
        "grants": grants.iter().map(|grant| grant_summary_value(grant, None, now_ms)).collect::<Vec<_>>(),
        "tokenUsage": {
            "cycles": total_cycles,
            "toolCalls": total_tool_calls,
            "elapsedMs": total_elapsed_ms,
            "inputTokens": total_input_tokens,
            "outputTokens": total_output_tokens,
            "costMicrounits": total_cost_microunits
        },
        "workspaceChanges": workspace_changes,
        "lastSpeech": last_speech,
        "lastDirectives": if last_directives.is_null() { Value::Null } else { last_directives },
        "revokeGrantIds": revoke_grant_ids
    }))
}

fn runtime_error(error: impl std::fmt::Display) -> CliError {
    CliError::new("agent-runtime", error.to_string(), 10)
}


#[cfg(test)]
mod tests {
    use super::{
        infer_tier, tier_policy, AuthorizationTier, EIGHT_HOURS_MS, FOUR_HOURS_MS,
    };
    use nimora_agent_runtime::{ApprovalPolicy, GrantLifetime, NetworkPolicy, SandboxScope};
    use std::collections::BTreeSet;

    #[test]
    fn tier_policy_maps_desktop_semantics() {
        let roots = vec!["/tmp/ws".to_owned()];
        let observe = tier_policy(AuthorizationTier::Observe, true, &roots, 1_000);
        assert_eq!(observe.sandbox, SandboxScope::ReadOnly);
        assert_eq!(observe.approval, ApprovalPolicy::AskRisky);
        assert_eq!(observe.network, NetworkPolicy::Offline);

        let trusted = tier_policy(AuthorizationTier::TrustedWorkspace, false, &roots, 1_000);
        assert_eq!(trusted.sandbox, SandboxScope::WorkspaceWrite);
        assert_eq!(trusted.approval, ApprovalPolicy::NeverAskWithinGrant);

        let unattended = tier_policy(AuthorizationTier::Unattended, true, &roots, 1_000);
        assert_eq!(unattended.sandbox, SandboxScope::SelectedRoots);
        assert_eq!(unattended.approval, ApprovalPolicy::NeverAskWithinGrant);
        assert_eq!(unattended.selected_roots, BTreeSet::from(["/tmp/ws".to_owned()]));
        assert_eq!(unattended.lifetime, GrantLifetime::UntilTimestamp);
        assert_eq!(unattended.expires_at_ms, Some(1_000 + EIGHT_HOURS_MS));

        let full = tier_policy(AuthorizationTier::FullDevice, false, &roots, 2_000);
        assert_eq!(full.sandbox, SandboxScope::FullDevice);
        assert_eq!(full.approval, ApprovalPolicy::NeverAskWithinGrant);
        assert_eq!(full.network, NetworkPolicy::Unrestricted);
        assert_eq!(full.expires_at_ms, Some(2_000 + FOUR_HOURS_MS));
    }

    #[test]
    fn infer_tier_matches_sandbox_and_approval() {
        let roots = vec!["/tmp/ws".to_owned()];
        for tier in [
            AuthorizationTier::Observe,
            AuthorizationTier::Workspace,
            AuthorizationTier::TrustedWorkspace,
            AuthorizationTier::Unattended,
            AuthorizationTier::FullDevice,
        ] {
            let policy = tier_policy(tier, true, &roots, 1_000);
            let grant = nimora_agent_runtime::AuthorizationGrant {
                spec: "nimora.authorization-grant/1".to_owned(),
                id: uuid::Uuid::now_v7(),
                goal_id: uuid::Uuid::now_v7(),
                plan_revision: 1,
                workspace_fingerprint: format!("sha256:{}", "a".repeat(64)),
                sandbox: policy.sandbox,
                approval: policy.approval,
                network: policy.network,
                selected_roots: policy.selected_roots,
                tool_allowlist: BTreeSet::from([
                    "pet.state.read".parse().expect("tool"),
                ]),
                provider_allowlist: BTreeSet::from(["provider:deterministic-local".to_owned()]),
                model_allowlist: BTreeSet::from(["model:echo-v1".to_owned()]),
                maximum_data_classification: nimora_agent_runtime::DataClassification::Personal,
                budget: nimora_agent_runtime::AgentBudget::default(),
                lifetime: policy.lifetime,
                issued_at_ms: 1_000,
                expires_at_ms: policy.expires_at_ms,
                revoked_at_ms: None,
            };
            assert_eq!(infer_tier(&grant), tier);
        }
    }
}
