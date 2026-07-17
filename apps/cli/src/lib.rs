use nimora_agent_provider_worker::{OllamaEndpoint, WorkerOllamaProvider, verify_provider_sidecar};
use nimora_agent_runtime::{
    AgentAutonomy, AgentBudget, AgentCoordinator, AgentTask, AgentTaskGateway,
    AgentTaskGatewayPolicy, AgentTaskOrigin, AgentTaskRequest, CancellationFlag,
    DataClassification, DeterministicLocalProvider, ProviderExecutionContext, ProviderMessage,
    ProviderMessageRole, ProviderRegistry, ProviderStepInput, ProviderStepOutcome,
};
use nimora_agent_tools::production_tool_registry;
use nimora_persistence_sqlite::{AgentHistoryRecord, SqliteAgentHistoryRepository};
use serde::Deserialize;
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
            "nimora ai history delete --database <path> (--task-id <uuid>|--all)"
        ]
    })
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
