use nimora_agent_provider_worker::{
    OllamaEndpoint, OllamaModel, WorkerOllamaProvider, probe_ollama_worker, verify_provider_sidecar,
};
use nimora_agent_runtime::{
    AgentAutonomy, AgentBudget, AgentCoordinator, AgentTask, AgentTaskGateway,
    AgentTaskGatewayPolicy, AgentTaskOrigin, AgentTaskRequest, AgentTaskStatus, BaseRiskEvaluator,
    CancellationFlag, DataClassification, DeterministicLocalProvider, PlannedToolCall,
    ProviderExecutionContext, ProviderMessage, ProviderMessageRole, ProviderRegistry,
    ProviderResponse, ProviderStepInput, ProviderStepOutcome, ProviderToolTurn, ToolAdmission,
    ToolApproval, ToolInvocation, ToolStepOutcome,
};
use nimora_agent_tools::{GatewayToolBackend, production_tool_registry};
use nimora_asset_installer::{
    AssetPackageSummary, AssetPreviewReport, AssetRendererDescriptor, GltfCharacterMetadata,
    InstallError, InstallFile, ModelAnimationBinding, RenderAnchor, RenderCanvas, SpriteClips,
    export_asset_package, inspect_asset_package, inspect_asset_renderer,
    inspect_asset_source_preview, install_asset_source, install_gltf_character,
    read_verified_asset_image, read_verified_asset_model, rollback_latest,
};
use nimora_automation_runtime::{
    ActionFailure, AutomationBackend, AutomationDefinition, AutomationEngine, AutomationError,
    AutomationExecutionContext, AutomationRun, RunMode, Uncancelled,
};
use nimora_diagnostics_bundle::{
    ApplicationSummary, DataProtectionSummary, DiagnosticBundleError, DiagnosticBundleReceipt,
    DiagnosticBundleSelection, DiagnosticComponent, DiagnosticEvent, DiagnosticEventCode,
    DiagnosticJournalPolicy, DiagnosticReport, DiagnosticSeverity, DiagnosticSourcesSummary,
    PersistentDiagnosticJournal, PrivacySummary, RuntimeSummary, SystemSummary,
    export_diagnostic_bundle,
};
use nimora_model_importer::{
    ModelProbeReport, ModelProbeRequest, ModelWorkerError, probe_model_in_worker,
};
use nimora_persistence_sqlite::{
    AgentHistoryRecord, AutomationJournalEntry, BackupCoordinator, BackupHealth, BackupPolicy,
    BackupRecord, DATABASE_VERSION, OutboxSnapshot, ProgramPermissionGrant,
    SqliteAgentHistoryRepository, SqliteAutomationJournal, SqliteOutboxRepository,
    SqlitePersistenceError, SqlitePetRepository, SqliteProfileRepository,
    SqliteProgramPermissionRepository, apply_pending_restore, verify_database_file,
};
use nimora_runtime_app::{
    ProfileService, ProfileServiceError, ProfileSnapshot, RuntimeError, RuntimeEventBatch,
    RuntimeEventBus, RuntimeEventSubscription, RuntimeService, SafetyService, SafetyServiceError,
};
use nimora_runtime_core::{
    Command, CommandRisk, CommandStatus, Event, EventSource, Pet, PetAction, PointerButton,
    Position, ProfileId, ProfilePolicy, RuntimeMode, SafeModeReason, SafetySnapshot,
};
use nimora_user_code_gateway::{
    CapabilityBackend, CapabilityGateway, CapabilityResponse, GatewayEnvelope, GatewayError,
};
use nimora_user_code_host::{WorkerConfig, WorkerMessage, WorkerProcess};
use nimora_user_code_package::{
    ProgramPackageError, install_program_atomically, load_installed_program, rollback_program,
};
use nimora_user_code_policy::{
    Capability, EventAdmission, EventConcurrencyPolicy, EventTriggerScheduler,
    ExecutionCancellation, ExecutionController, ExecutionHandle, ExecutionPolicy, PolicyError,
    ProgramManifest, ScheduledEvent, WorkerError, evaluate,
};
use nimora_user_code_storage::ProgramDataStore;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fs, io,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{
    AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
};
use thiserror::Error;
use uuid::Uuid;

const CONTROL_CENTER_LABEL: &str = "control-center";
const BUILTIN_CHARACTER_ID: &str = "builtin.aster";
const ACTIVE_CHARACTER_SPEC: &str = "nimora.active-character/1";
const ACTIVE_CHARACTER_FILE: &str = ".active-character.json";
const PET_WINDOW_LABEL: &str = "pet";
const CHARACTER_RENDERER_CHANGED_EVENT: &str = "nimora://character-renderer-changed";
const ASSET_PROTOCOL: &str = "nimora-asset";
const DETERMINISTIC_PROVIDER_ID: &str = "provider:deterministic-local";
const DEFAULT_AGENT_MODEL: &str = "model:echo-v1";
const POSITION_WRITE_DEBOUNCE: Duration = Duration::from_millis(200);
const CLICK_FEEDBACK_DURATION: Duration = Duration::from_millis(600);
const MAX_USER_PROGRAM_OPERATIONS: usize = 32;
const MAX_USER_PROGRAM_EVENT_SESSIONS: usize = 32;
const MAX_MODEL_BYTES: u64 = 80 * 1024 * 1024;
const MODEL_PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_PENDING_AGENT_TOOLS: usize = 32;
const AGENT_TOOL_APPROVAL_TTL_MS: u64 = 5 * 60 * 1_000;

#[derive(Debug)]
struct DesktopState {
    native_app: Option<AppHandle>,
    runtime: RuntimeService<SqlitePetRepository>,
    profiles: ProfileService<SqliteProfileRepository>,
    safety: SafetyService,
    events: RuntimeEventBus,
    window_policy: Mutex<WindowPolicy>,
    policy_before_safe_mode: Mutex<Option<WindowPolicy>>,
    position_revision: AtomicU64,
    dragging: AtomicBool,
    asset_store: PathBuf,
    active_character_write: Mutex<()>,
    program_store: PathBuf,
    program_data_store: ProgramDataStore,
    program_permissions: SqliteProgramPermissionRepository,
    outbox: SqliteOutboxRepository,
    agent_history: SqliteAgentHistoryRepository,
    automation_journal: SqliteAutomationJournal,
    agent_history_last_error: Mutex<bool>,
    backups: BackupCoordinator,
    backup_last_error: Mutex<Option<String>>,
    diagnostic_journal: Mutex<PersistentDiagnosticJournal>,
    user_program_event_sessions: Mutex<HashMap<Uuid, UserProgramEventSession>>,
    active_user_program_workers: Mutex<HashMap<Uuid, ActiveUserProgramWorker>>,
    user_programs: Mutex<HashMap<Uuid, UserProgramSession>>,
    pending_agent_tools: Mutex<HashMap<Uuid, PendingAgentTool>>,
    execution_controller: ExecutionController,
    ollama_worker: Option<PathBuf>,
    startup: StartupStatus,
}

#[derive(Debug)]
struct PendingAgentTool {
    invocation: ToolInvocation,
    approval: ToolApproval,
    effective_risk: CommandRisk,
    expires_at_ms: u64,
    context: PendingAgentToolContext,
}

#[derive(Debug, Clone)]
enum PendingAgentToolContext {
    Standalone {
        task: AgentTask,
    },
    ProviderTurn {
        approval_index: usize,
        provider_call_id: String,
        session: Arc<Mutex<PendingProviderAgent>>,
    },
}

#[derive(Debug)]
struct PendingProviderAgent {
    task: AgentTask,
    model: String,
    messages: Vec<ProviderMessage>,
    max_output_tokens: u64,
    offline: bool,
    turn: ProviderToolTurn,
    approvals: Vec<Option<ApprovedProviderTool>>,
    remaining_confirmations: usize,
}

#[derive(Debug)]
struct ApprovedProviderTool {
    provider_call_id: String,
    invocation: ToolInvocation,
    approval: ToolApproval,
}

#[derive(Debug)]
enum ProviderAgentOutcome {
    Completed {
        task: AgentTask,
        response: ProviderResponse,
    },
    Waiting {
        task: AgentTask,
        pending: Vec<AgentToolResult>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum StartupMode {
    Normal,
    Recovery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct StartupStatus {
    mode: StartupMode,
    reason: Option<&'static str>,
}

#[derive(Debug)]
struct UserProgramSession {
    policy: ExecutionPolicy,
    execution: ExecutionHandle,
}

#[derive(Debug)]
struct UserProgramEventSession {
    program_id: String,
    subscription: RuntimeEventSubscription,
    automatic: bool,
    executed: u64,
    dropped: u64,
    last_error: Option<String>,
}

#[derive(Debug)]
struct ActiveUserProgramWorker {
    program_id: String,
    cancellation: ExecutionCancellation,
}

struct UserProgramEventCompletion {
    scheduled_execution_id: Uuid,
    result: Result<UserProgramExecutionReceipt, String>,
}

#[derive(Debug)]
struct ActiveUserProgramWorkerGuard<'a> {
    workers: &'a Mutex<HashMap<Uuid, ActiveUserProgramWorker>>,
    execution_id: Uuid,
}

impl Drop for ActiveUserProgramWorkerGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut workers) = self.workers.lock() {
            workers.remove(&self.execution_id);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct WindowPolicy {
    always_on_top: bool,
    click_through: bool,
}

impl WindowPolicy {
    const SAFE: Self = Self {
        always_on_top: true,
        click_through: false,
    };

    fn from_profile(policy: &ProfilePolicy) -> Self {
        let resolved = ProfilePolicy::merge(&ProfilePolicy::standard(), policy);
        Self {
            always_on_top: resolved.always_on_top.unwrap_or(true),
            click_through: resolved.click_through.unwrap_or(false),
        }
    }
}

impl DesktopState {
    fn open(
        native_app: Option<AppHandle>,
        database_path: &Path,
        asset_store: PathBuf,
        program_store: PathBuf,
        backups: BackupCoordinator,
        mut diagnostic_journal: PersistentDiagnosticJournal,
        ollama_worker: Option<PathBuf>,
    ) -> Result<Self, DesktopError> {
        let events = RuntimeEventBus::default();
        let runtime = RuntimeService::initialize_with_event_bus(
            SqlitePetRepository::open(database_path)?,
            "Aster",
            events.clone(),
        )?;
        let profiles = ProfileService::initialize(
            SqliteProfileRepository::open(database_path)?,
            events.clone(),
        )?;
        let window_policy = active_window_policy(&profiles.snapshot()?)?;
        let program_data_store =
            ProgramDataStore::new(program_store.with_file_name("program-data"));
        let _ = diagnostic_journal.record(diagnostic_event(
            DiagnosticSeverity::Info,
            DiagnosticComponent::Application,
            DiagnosticEventCode::ApplicationStarted,
        )?);
        let automation_journal = SqliteAutomationJournal::open(database_path)?;
        automation_journal.recover_running(current_time_ms()?, "desktop process restarted")?;
        Ok(Self {
            native_app,
            runtime,
            profiles,
            safety: SafetyService::new(events.clone()),
            events,
            window_policy: Mutex::new(window_policy),
            policy_before_safe_mode: Mutex::new(None),
            position_revision: AtomicU64::new(0),
            dragging: AtomicBool::new(false),
            asset_store,
            active_character_write: Mutex::new(()),
            program_store,
            program_data_store,
            program_permissions: SqliteProgramPermissionRepository::open(database_path)?,
            outbox: SqliteOutboxRepository::open(database_path)?,
            agent_history: SqliteAgentHistoryRepository::open(database_path)?,
            automation_journal,
            agent_history_last_error: Mutex::new(false),
            backups,
            backup_last_error: Mutex::new(None),
            diagnostic_journal: Mutex::new(diagnostic_journal),
            user_program_event_sessions: Mutex::new(HashMap::new()),
            active_user_program_workers: Mutex::new(HashMap::new()),
            user_programs: Mutex::new(HashMap::new()),
            pending_agent_tools: Mutex::new(HashMap::new()),
            execution_controller: ExecutionController::default(),
            ollama_worker,
            startup: StartupStatus {
                mode: StartupMode::Normal,
                reason: None,
            },
        })
    }

    fn open_recovery(
        native_app: Option<AppHandle>,
        asset_store: PathBuf,
        program_store: PathBuf,
        backups: BackupCoordinator,
        mut diagnostic_journal: PersistentDiagnosticJournal,
        reason: &'static str,
        ollama_worker: Option<PathBuf>,
    ) -> Result<Self, DesktopError> {
        let events = RuntimeEventBus::default();
        let runtime = RuntimeService::initialize_with_event_bus(
            SqlitePetRepository::in_memory()?,
            "Aster",
            events.clone(),
        )?;
        let profiles =
            ProfileService::initialize(SqliteProfileRepository::in_memory()?, events.clone())?;
        let window_policy = active_window_policy(&profiles.snapshot()?)?;
        let program_data_store =
            ProgramDataStore::new(program_store.with_file_name("program-data-recovery"));
        let _ = diagnostic_journal.record(diagnostic_event(
            DiagnosticSeverity::Error,
            DiagnosticComponent::Persistence,
            DiagnosticEventCode::RecoveryModeStarted,
        )?);
        Ok(Self {
            native_app,
            runtime,
            profiles,
            safety: SafetyService::new(events.clone()),
            events,
            window_policy: Mutex::new(window_policy),
            policy_before_safe_mode: Mutex::new(None),
            position_revision: AtomicU64::new(0),
            dragging: AtomicBool::new(false),
            asset_store,
            active_character_write: Mutex::new(()),
            program_store,
            program_data_store,
            program_permissions: SqliteProgramPermissionRepository::in_memory()?,
            outbox: SqliteOutboxRepository::in_memory()?,
            agent_history: SqliteAgentHistoryRepository::in_memory()?,
            automation_journal: SqliteAutomationJournal::in_memory()?,
            agent_history_last_error: Mutex::new(false),
            backups,
            backup_last_error: Mutex::new(None),
            diagnostic_journal: Mutex::new(diagnostic_journal),
            user_program_event_sessions: Mutex::new(HashMap::new()),
            active_user_program_workers: Mutex::new(HashMap::new()),
            user_programs: Mutex::new(HashMap::new()),
            pending_agent_tools: Mutex::new(HashMap::new()),
            execution_controller: ExecutionController::default(),
            ollama_worker,
            startup: StartupStatus {
                mode: StartupMode::Recovery,
                reason: Some(reason),
            },
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopSnapshot {
    pet: Pet,
    window_policy: WindowPolicy,
    safety: SafetySnapshot,
    startup: StartupStatus,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MovePetRequest {
    x: f64,
    y: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClickPetRequest {
    x: f64,
    y: f64,
    button: PointerButton,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstallAssetRequest {
    source_path: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportAssetRequest {
    source_path: PathBuf,
    destination_path: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportDiagnosticRequest {
    destination_path: PathBuf,
    include_events: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InspectModelRequest {
    source_path: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImportModelRequest {
    source_path: PathBuf,
    asset_id: String,
    name: String,
    license: String,
    animation_map: BTreeMap<String, ModelAnimationBinding>,
}

#[derive(Debug)]
struct ModelStagingDirectory {
    root: PathBuf,
}

impl Drop for ModelStagingDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallAssetFile {
    relative_path: PathBuf,
    bytes: u64,
    sha256: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AssetInstallReceipt {
    asset_id: String,
    replaced_previous: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AssetRollbackReceipt {
    asset_id: String,
    quarantined_failed_version: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AssetCatalogSnapshot {
    assets: Vec<AssetPackageSummary>,
    rejected: Vec<RejectedAssetPackage>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ActiveCharacterSnapshot {
    asset_id: String,
    source: ActiveCharacterSource,
    fallback_reason: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CharacterRendererSnapshot {
    spec: &'static str,
    asset_id: String,
    asset_base_url: Option<String>,
    backend: String,
    canvas: RenderCanvas,
    anchor: RenderAnchor,
    default_scale: f64,
    pixel_art: bool,
    fallbacks: std::collections::BTreeMap<String, String>,
    clips: Option<SpriteClips>,
    model: Option<PathBuf>,
    fallback_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ActiveCharacterSource {
    BuiltIn,
    Installed,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredActiveCharacter {
    spec: String,
    asset_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RejectedAssetPackage {
    directory: String,
    reason: String,
}

#[derive(Debug)]
struct AssetProtocolResponse {
    status: tauri::http::StatusCode,
    media_type: &'static str,
    body: Vec<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallUserProgramRequest {
    source_path: PathBuf,
    manifest: ProgramManifest,
    files: Vec<InstallAssetFile>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UserProgramInstallReceipt {
    program_id: String,
    version: String,
    replaced_previous: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UserProgramRollbackReceipt {
    program_id: String,
    quarantined_failed_version: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UserProgramPermissionStatus {
    program_id: String,
    version: String,
    capabilities: Vec<Capability>,
    granted: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UserProgramEventSessionReceipt {
    subscription_id: Uuid,
    program_id: String,
    version: String,
    event_types: Vec<String>,
    queue_capacity: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgramPolicyReport {
    program_id: String,
    granted_capabilities: Vec<Capability>,
    timeout_ms: u64,
    memory_bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UserProgramSessionReceipt {
    execution_id: Uuid,
    program_id: String,
    timeout_ms: u64,
    memory_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UserProgramExecutionReceipt {
    execution_id: Uuid,
    responses: Vec<CapabilityResponse>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UserProgramEventExecutionReceipt {
    execution: Option<UserProgramExecutionReceipt>,
    dropped: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UserProgramEventSessionStatus {
    subscription_id: Uuid,
    program_id: String,
    automatic: bool,
    executed: u64,
    dropped: u64,
    last_error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UserProgramPlan {
    #[serde(default)]
    storage: Vec<UserProgramStorageOperation>,
    #[serde(default)]
    commands: Vec<UserProgramPlanCommand>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
enum UserProgramStorageOperation {
    Read {
        key: String,
    },
    Write {
        key: String,
        value: serde_json::Value,
    },
    Delete {
        key: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UserProgramPlanCommand {
    command: String,
    #[serde(default)]
    arguments: serde_json::Value,
    idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayAction {
    OpenControlCenter,
    RestoreInteraction,
    EnterSafeMode,
    ExitSafeMode,
    Quit,
    Unknown,
}

impl From<&str> for TrayAction {
    fn from(value: &str) -> Self {
        match value {
            "open" => Self::OpenControlCenter,
            "interactive" => Self::RestoreInteraction,
            "safe-mode" => Self::EnterSafeMode,
            "normal-mode" => Self::ExitSafeMode,
            "quit" => Self::Quit,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Error)]
enum DesktopError {
    #[error("pet state is unavailable")]
    StatePoisoned,
    #[error("operation is unavailable while safe mode is active")]
    SafeModeActive,
    #[error("operation is unavailable while database recovery mode is active")]
    RecoveryModeActive,
    #[error("desktop window is unavailable: {0}")]
    WindowUnavailable(String),
    #[error("Agent runtime failed: {0}")]
    Agent(String),
    #[error("operation is unavailable from this window")]
    WindowForbidden,
    #[error("pet position must be a finite 32-bit screen coordinate")]
    InvalidPosition,
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error(transparent)]
    Profile(#[from] ProfileServiceError),
    #[error(transparent)]
    Safety(#[from] SafetyServiceError),
    #[error("operation failed ({primary}); native window policy rollback also failed ({rollback})")]
    NativePolicyRollback { primary: String, rollback: String },
    #[error("character activation failed ({primary}); selection rollback also failed ({rollback})")]
    CharacterActivationRollback { primary: String, rollback: String },
    #[error(transparent)]
    Persistence(#[from] SqlitePersistenceError),
    #[error(transparent)]
    DiagnosticBundle(#[from] DiagnosticBundleError),
    #[error(transparent)]
    AssetInstall(#[from] InstallError),
    #[error("asset identifier must be a lowercase namespaced identifier")]
    InvalidAssetIdentifier,
    #[error("only installed character assets can be activated")]
    AssetIsNotCharacter,
    #[error("package source must be an absolute existing directory or file")]
    InvalidPackageSource,
    #[error("model source must be an absolute regular .glb file and must not be a symbolic link")]
    InvalidModelSource,
    #[error("model exceeds the 80 MiB inspection budget")]
    ModelInputBudgetExceeded,
    #[error("model importer worker failed: {0}")]
    ModelWorker(#[from] ModelWorkerError),
    #[error(transparent)]
    UserCodePolicy(#[from] PolicyError),
    #[error(transparent)]
    UserCodeWorker(#[from] WorkerError),
    #[error("user code worker failed: {0}")]
    UserCodeHost(String),
    #[error(transparent)]
    UserCodeGateway(#[from] GatewayError),
    #[error(transparent)]
    UserCodePackage(#[from] ProgramPackageError),
    #[error(transparent)]
    Automation(#[from] AutomationError),
    #[error("user program execution was not found")]
    UserProgramNotFound,
    #[error("user program permissions must be granted for this exact installed version")]
    UserProgramPermissionRequired,
    #[error("installed user program version changed before execution")]
    UserProgramVersionChanged,
    #[error("user program does not declare event subscriptions")]
    UserProgramSubscriptionsMissing,
    #[error("maximum user program event subscriptions reached")]
    UserProgramEventSessionLimit,
    #[error("user program event subscription was not found")]
    UserProgramEventSessionNotFound,
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LocalAgentRequest {
    prompt: String,
    #[serde(default = "default_agent_provider_id")]
    provider_id: String,
    #[serde(default = "default_agent_model")]
    model: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AutomationTestRequest {
    definition: AutomationDefinition,
    event_type: String,
    event_data: serde_json::Value,
}

#[derive(Debug)]
struct DryRunAutomationBackend;

impl AutomationBackend for DryRunAutomationBackend {
    fn execute(
        &self,
        _context: &AutomationExecutionContext,
        _command: Command,
    ) -> Result<(), ActionFailure> {
        Err(ActionFailure {
            message: "dry-run backend cannot execute commands".to_owned(),
            transient: false,
        })
    }
}

#[tauri::command]
fn test_automation(request: AutomationTestRequest) -> Result<AutomationRun, DesktopError> {
    dry_run_automation(&request.definition, request.event_type, request.event_data)
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri managed state command arguments are owned extractors"
)]
fn automation_run_status(
    run_id: &str,
    state: State<'_, DesktopState>,
) -> Result<Option<AutomationJournalEntry>, DesktopError> {
    let run_id =
        Uuid::parse_str(run_id).map_err(|_| SqlitePersistenceError::InvalidAutomationJournal)?;
    state.automation_journal.get(run_id).map_err(Into::into)
}

fn dry_run_automation(
    definition: &AutomationDefinition,
    event_type: String,
    event_data: serde_json::Value,
) -> Result<AutomationRun, DesktopError> {
    let event = Event::new(
        event_type,
        EventSource::System("automation-test".to_owned()),
        event_data,
    )
    .map_err(RuntimeError::from)?;
    Ok(AutomationEngine::run(
        definition,
        &event,
        RunMode::DryRun,
        &DryRunAutomationBackend,
        &Uncancelled,
    )?)
}

fn default_agent_provider_id() -> String {
    DETERMINISTIC_PROVIDER_ID.to_owned()
}

fn default_agent_model() -> String {
    DEFAULT_AGENT_MODEL.to_owned()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentCatalog {
    spec: &'static str,
    providers: Vec<nimora_agent_runtime::ProviderDescriptor>,
    tools: Vec<nimora_agent_runtime::ToolDescriptor>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AgentProviderStatusRequest {
    provider_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentProviderStatus {
    spec: &'static str,
    provider_id: String,
    state: &'static str,
    worker_verified: bool,
    service_reachable: bool,
    models: Vec<OllamaModel>,
    message: &'static str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AgentHistoryListRequest {
    before_created_at_ms: Option<u64>,
    before_task_id: Option<Uuid>,
    limit: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentHistoryPage {
    spec: &'static str,
    records: Vec<AgentHistoryRecord>,
    history_degraded: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeleteAgentHistoryRequest {
    task_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeleteAgentHistoryResult {
    spec: &'static str,
    deleted: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopAgentRunResult {
    spec: &'static str,
    status: DesktopAgentRunStatus,
    task: AgentTask,
    content: Option<String>,
    finish_reason: Option<nimora_agent_runtime::ProviderFinishReason>,
    usage: Option<nimora_agent_runtime::ProviderUsage>,
    pending_tools: Vec<AgentToolResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum DesktopAgentRunStatus {
    Completed,
    WaitingForConfirmation,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PrepareAgentToolRequest {
    tool_id: String,
    arguments: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResolveAgentToolRequest {
    invocation_id: Uuid,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentToolResult {
    spec: &'static str,
    task: AgentTask,
    invocation: ToolInvocation,
    effective_risk: CommandRisk,
    requires_confirmation: bool,
    expires_at_ms: Option<u64>,
    output: Option<serde_json::Value>,
}

impl Serialize for DesktopError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn desktop_snapshot(state: State<'_, DesktopState>) -> Result<DesktopSnapshot, DesktopError> {
    let pet = state.runtime.snapshot()?;
    let window_policy = *state
        .window_policy
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?;
    let safety = state.safety.snapshot()?;
    Ok(DesktopSnapshot {
        pet,
        window_policy,
        safety,
        startup: state.startup.clone(),
    })
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn agent_catalog(state: State<'_, DesktopState>) -> Result<AgentCatalog, DesktopError> {
    agent_catalog_inner(&state)
}

fn agent_catalog_inner(state: &DesktopState) -> Result<AgentCatalog, DesktopError> {
    let providers = desktop_provider_registry(state)?;
    let tools =
        production_tool_registry().map_err(|error| DesktopError::Agent(error.to_string()))?;
    Ok(AgentCatalog {
        spec: "nimora.desktop-agent-catalog/1",
        providers: providers.descriptors().into_iter().cloned().collect(),
        tools: tools.descriptors().into_iter().cloned().collect(),
    })
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn agent_provider_status(
    request: AgentProviderStatusRequest,
    state: State<'_, DesktopState>,
) -> Result<AgentProviderStatus, DesktopError> {
    agent_provider_status_inner(request, &state)
}

fn agent_provider_status_inner(
    request: AgentProviderStatusRequest,
    state: &DesktopState,
) -> Result<AgentProviderStatus, DesktopError> {
    if request.provider_id == DETERMINISTIC_PROVIDER_ID {
        return Ok(AgentProviderStatus {
            spec: "nimora.desktop-agent-provider-status/1",
            provider_id: request.provider_id,
            state: "ready",
            worker_verified: true,
            service_reachable: true,
            models: vec![OllamaModel {
                name: DEFAULT_AGENT_MODEL.to_owned(),
                size: 0,
                modified_at: None,
            }],
            message: "内置离线 Provider 可用",
        });
    }
    if request.provider_id != "provider:ollama-loopback" {
        return Err(DesktopError::Agent("Provider is not registered".to_owned()));
    }
    let Some(executable) = &state.ollama_worker else {
        return Err(DesktopError::Agent("Provider is not registered".to_owned()));
    };
    if state.startup.mode == StartupMode::Recovery
        || state.safety.snapshot()?.mode == RuntimeMode::Safe
    {
        return Ok(AgentProviderStatus {
            spec: "nimora.desktop-agent-provider-status/1",
            provider_id: request.provider_id,
            state: "unavailable",
            worker_verified: true,
            service_reachable: false,
            models: Vec::new(),
            message: "当前安全模式禁止启动 Provider Worker",
        });
    }
    let endpoint = OllamaEndpoint::default_ipv4();
    match probe_ollama_worker(executable, endpoint, Duration::from_secs(2)) {
        Ok(probe) => Ok(AgentProviderStatus {
            spec: "nimora.desktop-agent-provider-status/1",
            provider_id: request.provider_id,
            state: if probe.models.is_empty() {
                "unavailable"
            } else {
                "ready"
            },
            worker_verified: true,
            service_reachable: true,
            models: probe.models,
            message: "Ollama 服务已响应",
        }),
        Err(_) => Ok(AgentProviderStatus {
            spec: "nimora.desktop-agent-provider-status/1",
            provider_id: request.provider_id,
            state: "unavailable",
            worker_verified: true,
            service_reachable: false,
            models: Vec::new(),
            message: "Ollama 服务不可用",
        }),
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn agent_history_list(
    request: AgentHistoryListRequest,
    state: State<'_, DesktopState>,
) -> Result<AgentHistoryPage, DesktopError> {
    let cursor = match (request.before_created_at_ms, request.before_task_id) {
        (Some(created_at_ms), Some(task_id)) => Some((created_at_ms, task_id)),
        (None, None) => None,
        _ => {
            return Err(DesktopError::Agent(
                "history cursor must include timestamp and task ID".to_owned(),
            ));
        }
    };
    Ok(AgentHistoryPage {
        spec: "nimora.desktop-agent-history/1",
        records: state.agent_history.list(cursor, request.limit)?,
        history_degraded: *state
            .agent_history_last_error
            .lock()
            .map_err(|_| DesktopError::StatePoisoned)?,
    })
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn delete_agent_history(
    request: DeleteAgentHistoryRequest,
    state: State<'_, DesktopState>,
) -> Result<DeleteAgentHistoryResult, DesktopError> {
    let deleted = match request.task_id {
        Some(task_id) => u64::from(state.agent_history.delete(task_id)?),
        None => state.agent_history.delete_all()?,
    };
    Ok(DeleteAgentHistoryResult {
        spec: "nimora.desktop-agent-history-delete/1",
        deleted,
    })
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn run_local_agent(
    request: LocalAgentRequest,
    state: State<'_, DesktopState>,
) -> Result<DesktopAgentRunResult, DesktopError> {
    run_local_agent_inner(request, &state)
}

fn run_local_agent_inner(
    request: LocalAgentRequest,
    state: &DesktopState,
) -> Result<DesktopAgentRunResult, DesktopError> {
    if request.prompt.trim().is_empty() || request.prompt.len() > 32 * 1024 {
        return Err(DesktopError::Agent(
            "prompt must contain 1 to 32768 bytes".to_owned(),
        ));
    }
    if request.model.trim().is_empty() || request.model.len() > 128 {
        return Err(DesktopError::Agent(
            "model must contain 1 to 128 bytes".to_owned(),
        ));
    }
    ensure_normal_mode(state)?;
    if state.safety.snapshot()?.mode == RuntimeMode::Safe {
        return Err(DesktopError::SafeModeActive);
    }
    let providers = desktop_provider_registry(state)?;
    let now_ms = current_time_ms()?;
    let task = admit_desktop_agent_task(request.provider_id, now_ms)?;
    let outcome = advance_provider_agent(
        &providers,
        state,
        task,
        request.model,
        vec![ProviderMessage::text(
            ProviderMessageRole::User,
            request.prompt,
            DataClassification::Personal,
            true,
        )],
        512,
        true,
    )?;
    Ok(desktop_agent_run_result(outcome))
}

fn desktop_agent_run_result(outcome: ProviderAgentOutcome) -> DesktopAgentRunResult {
    match outcome {
        ProviderAgentOutcome::Completed { task, response } => DesktopAgentRunResult {
            spec: "nimora.desktop-agent-result/1",
            status: DesktopAgentRunStatus::Completed,
            task,
            content: Some(response.content),
            finish_reason: Some(response.finish_reason),
            usage: Some(response.usage),
            pending_tools: Vec::new(),
        },
        ProviderAgentOutcome::Waiting { task, pending } => DesktopAgentRunResult {
            spec: "nimora.desktop-agent-result/1",
            status: DesktopAgentRunStatus::WaitingForConfirmation,
            task,
            content: None,
            finish_reason: None,
            usage: None,
            pending_tools: pending,
        },
    }
}

fn admit_desktop_agent_task(provider_id: String, now_ms: u64) -> Result<AgentTask, DesktopError> {
    let tool_ids = production_tool_registry()
        .map_err(agent_error)?
        .descriptors()
        .into_iter()
        .map(|descriptor| descriptor.id.to_string())
        .collect::<Vec<_>>();
    let policy = AgentTaskGatewayPolicy::new(
        "desktop:local-user",
        [AgentTaskOrigin::Desktop],
        [
            DETERMINISTIC_PROVIDER_ID.to_owned(),
            "provider:ollama-loopback".to_owned(),
        ],
        tool_ids.clone(),
        DataClassification::Personal,
        AgentAutonomy::ConfirmEach,
        AgentBudget::default(),
        1,
    )
    .map_err(agent_error)?;
    AgentTaskGateway::new(policy)
        .admit(
            AgentTaskRequest::new(
                AgentTaskOrigin::Desktop,
                "desktop:local-user",
                provider_id,
                tool_ids,
                DataClassification::Personal,
                AgentAutonomy::ConfirmEach,
                AgentBudget::default(),
            ),
            now_ms,
        )
        .map(|admission| admission.task)
        .map_err(agent_error)
}

fn advance_provider_agent(
    providers: &ProviderRegistry,
    state: &DesktopState,
    mut task: AgentTask,
    model: String,
    mut messages: Vec<ProviderMessage>,
    max_output_tokens: u64,
    offline: bool,
) -> Result<ProviderAgentOutcome, DesktopError> {
    let tools = production_tool_registry().map_err(agent_error)?;
    let coordinator = AgentCoordinator::new(providers, &tools);
    let now_ms = current_time_ms()?;
    let outcome = coordinator
        .provider_step(
            &mut task,
            ProviderStepInput {
                model: model.clone(),
                messages: messages.clone(),
                max_output_tokens,
                context: ProviderExecutionContext {
                    timeout: Duration::from_secs(30),
                    cancellation: CancellationFlag::default(),
                    credential_reference: None,
                },
                offline,
                now_ms,
            },
        )
        .map_err(agent_error)?;
    let ProviderStepOutcome::ToolCalls { response, calls } = outcome else {
        let ProviderStepOutcome::Completed { response } = outcome else {
            unreachable!();
        };
        record_agent_history(state, &task, &model, &messages, &response);
        return Ok(ProviderAgentOutcome::Completed { task, response });
    };
    let mut turn = ProviderToolTurn::new(response).map_err(agent_error)?;
    let confirmations =
        execute_ready_provider_tools(providers, state, &mut task, &mut turn, calls, now_ms)?;
    if confirmations.is_empty() {
        messages.extend(turn.continuation_messages().map_err(agent_error)?);
        return advance_provider_agent(
            providers,
            state,
            task,
            model,
            messages,
            max_output_tokens,
            offline,
        );
    }
    register_provider_confirmations(
        state,
        task,
        model,
        messages,
        max_output_tokens,
        offline,
        turn,
        confirmations,
        now_ms,
    )
}

fn record_agent_history(
    state: &DesktopState,
    task: &AgentTask,
    model: &str,
    messages: &[ProviderMessage],
    response: &ProviderResponse,
) {
    let prompt = messages
        .iter()
        .find(|message| message.role == ProviderMessageRole::User)
        .map(|message| message.content.clone());
    let result = prompt
        .ok_or(SqlitePersistenceError::InvalidAgentHistory)
        .and_then(|prompt| {
            AgentHistoryRecord::new(
                task.clone(),
                model,
                prompt,
                response.content.clone(),
                response.finish_reason,
                response.usage,
                task.updated_at_ms,
            )
        })
        .and_then(|record| state.agent_history.insert(&record));
    if let Ok(mut degraded) = state.agent_history_last_error.lock() {
        *degraded = result.is_err();
    }
}

fn execute_ready_provider_tools(
    providers: &ProviderRegistry,
    state: &DesktopState,
    task: &mut AgentTask,
    turn: &mut ProviderToolTurn,
    calls: Vec<PlannedToolCall>,
    now_ms: u64,
) -> Result<Vec<(PlannedToolCall, CommandRisk)>, DesktopError> {
    let tools = production_tool_registry().map_err(agent_error)?;
    let coordinator = AgentCoordinator::new(providers, &tools);
    let backend = GatewayToolBackend::new(
        DesktopCapabilityBackend { state },
        GatewayToolBackend::<DesktopCapabilityBackend<'_>>::standard_policy(task.id, task.trace_id),
    );
    let mut confirmations = Vec::new();
    for call in calls {
        let effective_risk = match call.admission {
            ToolAdmission::Ready { effective_risk }
            | ToolAdmission::ConfirmationRequired { effective_risk, .. } => effective_risk,
        };
        if matches!(call.admission, ToolAdmission::ConfirmationRequired { .. }) {
            confirmations.push((call, effective_risk));
            continue;
        }
        let ToolStepOutcome::Completed { output, .. } = coordinator
            .tool_step(task, &backend, call.invocation.clone(), None, now_ms)
            .map_err(agent_error)?
        else {
            return Err(DesktopError::Agent(
                "read-only Provider tool unexpectedly requested confirmation".to_owned(),
            ));
        };
        turn.record_result(
            &call.provider_call_id,
            call.invocation.tool_id.as_str(),
            output,
        )
        .map_err(agent_error)?;
    }
    Ok(confirmations)
}

#[allow(clippy::too_many_arguments)]
fn register_provider_confirmations(
    state: &DesktopState,
    mut task: AgentTask,
    model: String,
    messages: Vec<ProviderMessage>,
    max_output_tokens: u64,
    offline: bool,
    turn: ProviderToolTurn,
    confirmations: Vec<(PlannedToolCall, CommandRisk)>,
    now_ms: u64,
) -> Result<ProviderAgentOutcome, DesktopError> {
    if task.status != AgentTaskStatus::WaitingForConfirmation {
        task.transition(AgentTaskStatus::WaitingForConfirmation, now_ms)
            .map_err(agent_error)?;
    }
    let expires_at_ms = now_ms.saturating_add(AGENT_TOOL_APPROVAL_TTL_MS);
    let session = Arc::new(Mutex::new(PendingProviderAgent {
        task: task.clone(),
        model,
        messages,
        max_output_tokens,
        offline,
        turn,
        approvals: (0..confirmations.len()).map(|_| None).collect(),
        remaining_confirmations: confirmations.len(),
    }));
    let mut pending_store = state
        .pending_agent_tools
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?;
    pending_store.retain(|_, item| item.expires_at_ms > now_ms);
    if pending_store.len().saturating_add(confirmations.len()) > MAX_PENDING_AGENT_TOOLS {
        return Err(DesktopError::Agent(
            "maximum pending Agent tool confirmations reached".to_owned(),
        ));
    }
    let mut pending_results = Vec::with_capacity(confirmations.len());
    for (approval_index, (call, effective_risk)) in confirmations.into_iter().enumerate() {
        let approval = ToolApproval::bind(&call.invocation, effective_risk);
        pending_store.insert(
            call.invocation.invocation_id,
            PendingAgentTool {
                invocation: call.invocation.clone(),
                approval,
                effective_risk,
                expires_at_ms,
                context: PendingAgentToolContext::ProviderTurn {
                    approval_index,
                    provider_call_id: call.provider_call_id,
                    session: Arc::clone(&session),
                },
            },
        );
        pending_results.push(AgentToolResult {
            spec: "nimora.desktop-agent-tool-result/1",
            task: task.clone(),
            invocation: call.invocation,
            effective_risk,
            requires_confirmation: true,
            expires_at_ms: Some(expires_at_ms),
            output: None,
        });
    }
    Ok(ProviderAgentOutcome::Waiting {
        task,
        pending: pending_results,
    })
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn prepare_agent_tool(
    request: PrepareAgentToolRequest,
    state: State<'_, DesktopState>,
) -> Result<AgentToolResult, DesktopError> {
    prepare_agent_tool_inner(request, &state)
}

fn prepare_agent_tool_inner(
    request: PrepareAgentToolRequest,
    state: &DesktopState,
) -> Result<AgentToolResult, DesktopError> {
    ensure_normal_mode(state)?;
    if state.safety.snapshot()?.mode == RuntimeMode::Safe {
        return Err(DesktopError::SafeModeActive);
    }
    let tools = production_tool_registry().map_err(agent_error)?;
    let providers = ProviderRegistry::default();
    let coordinator = AgentCoordinator::new(&providers, &tools);
    let now_ms = current_time_ms()?;
    let mut task = admit_desktop_agent_task(DETERMINISTIC_PROVIDER_ID.to_owned(), now_ms)?;
    task.transition(AgentTaskStatus::Planning, now_ms)
        .map_err(agent_error)?;
    let invocation =
        ToolInvocation::new(task.id, task.trace_id, request.tool_id, request.arguments)
            .map_err(agent_error)?;
    let admission = tools.admit(&invocation).map_err(agent_error)?;
    let effective_risk = match admission {
        ToolAdmission::Ready { effective_risk }
        | ToolAdmission::ConfirmationRequired { effective_risk, .. } => effective_risk,
    };
    if matches!(admission, ToolAdmission::Ready { .. }) {
        let backend = GatewayToolBackend::new(
            DesktopCapabilityBackend { state },
            GatewayToolBackend::<DesktopCapabilityBackend<'_>>::standard_policy(
                task.id,
                task.trace_id,
            ),
        );
        let ToolStepOutcome::Completed { output, .. } = coordinator
            .tool_step(&mut task, &backend, invocation.clone(), None, now_ms)
            .map_err(agent_error)?
        else {
            return Err(DesktopError::Agent(
                "read-only tool unexpectedly requested confirmation".to_owned(),
            ));
        };
        task.transition(AgentTaskStatus::Succeeded, now_ms)
            .map_err(agent_error)?;
        return Ok(AgentToolResult {
            spec: "nimora.desktop-agent-tool-result/1",
            task,
            invocation,
            effective_risk,
            requires_confirmation: false,
            expires_at_ms: None,
            output: Some(output),
        });
    }
    task.transition(AgentTaskStatus::WaitingForConfirmation, now_ms)
        .map_err(agent_error)?;
    let approval = ToolApproval::bind(&invocation, effective_risk);
    let expires_at_ms = now_ms.saturating_add(AGENT_TOOL_APPROVAL_TTL_MS);
    let mut pending = state
        .pending_agent_tools
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?;
    pending.retain(|_, item| item.expires_at_ms > now_ms);
    if pending.len() >= MAX_PENDING_AGENT_TOOLS {
        return Err(DesktopError::Agent(
            "maximum pending Agent tool confirmations reached".to_owned(),
        ));
    }
    pending.insert(
        invocation.invocation_id,
        PendingAgentTool {
            invocation: invocation.clone(),
            approval,
            effective_risk,
            expires_at_ms,
            context: PendingAgentToolContext::Standalone { task: task.clone() },
        },
    );
    Ok(AgentToolResult {
        spec: "nimora.desktop-agent-tool-result/1",
        task,
        invocation,
        effective_risk,
        requires_confirmation: true,
        expires_at_ms: Some(expires_at_ms),
        output: None,
    })
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn confirm_agent_tool(
    request: ResolveAgentToolRequest,
    state: State<'_, DesktopState>,
) -> Result<AgentToolResult, DesktopError> {
    let providers = desktop_provider_registry(&state)?;
    confirm_agent_tool_with_registry(&request, &state, &providers).map(|(result, _)| result)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn confirm_agent_run_tool(
    request: ResolveAgentToolRequest,
    state: State<'_, DesktopState>,
) -> Result<DesktopAgentRunResult, DesktopError> {
    let providers = desktop_provider_registry(&state)?;
    let (resolved, continuation) = confirm_agent_tool_with_registry(&request, &state, &providers)?;
    desktop_agent_confirmation_result(&state, resolved, continuation)
}

fn desktop_agent_confirmation_result(
    state: &DesktopState,
    resolved: AgentToolResult,
    continuation: Option<ProviderAgentOutcome>,
) -> Result<DesktopAgentRunResult, DesktopError> {
    if let Some(outcome) = continuation {
        return Ok(desktop_agent_run_result(outcome));
    }
    let pending_tools = pending_agent_tools_for_task(state, &resolved.task)?;
    let status = if pending_tools.is_empty() {
        DesktopAgentRunStatus::Completed
    } else {
        DesktopAgentRunStatus::WaitingForConfirmation
    };
    Ok(DesktopAgentRunResult {
        spec: "nimora.desktop-agent-result/1",
        status,
        task: resolved.task,
        content: None,
        finish_reason: None,
        usage: None,
        pending_tools,
    })
}

fn pending_agent_tools_for_task(
    state: &DesktopState,
    task: &AgentTask,
) -> Result<Vec<AgentToolResult>, DesktopError> {
    let pending = state
        .pending_agent_tools
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?;
    let mut tools = pending
        .values()
        .filter(|item| item.invocation.task_id == task.id)
        .map(|item| {
            let order = match item.context {
                PendingAgentToolContext::Standalone { .. } => usize::MAX,
                PendingAgentToolContext::ProviderTurn { approval_index, .. } => approval_index,
            };
            (
                order,
                AgentToolResult {
                    spec: "nimora.desktop-agent-tool-result/1",
                    task: task.clone(),
                    invocation: item.invocation.clone(),
                    effective_risk: item.effective_risk,
                    requires_confirmation: true,
                    expires_at_ms: Some(item.expires_at_ms),
                    output: None,
                },
            )
        })
        .collect::<Vec<_>>();
    tools.sort_by_key(|(order, _)| *order);
    Ok(tools.into_iter().map(|(_, result)| result).collect())
}

#[cfg(test)]
fn confirm_agent_tool_inner(
    request: &ResolveAgentToolRequest,
    state: &DesktopState,
) -> Result<AgentToolResult, DesktopError> {
    let providers = desktop_provider_registry(state)?;
    confirm_agent_tool_with_registry(request, state, &providers).map(|(result, _)| result)
}

fn confirm_agent_tool_with_registry(
    request: &ResolveAgentToolRequest,
    state: &DesktopState,
    providers: &ProviderRegistry,
) -> Result<(AgentToolResult, Option<ProviderAgentOutcome>), DesktopError> {
    ensure_normal_mode(state)?;
    if state.safety.snapshot()?.mode == RuntimeMode::Safe {
        return Err(DesktopError::SafeModeActive);
    }
    let pending = state
        .pending_agent_tools
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?
        .remove(&request.invocation_id)
        .ok_or_else(|| DesktopError::Agent("pending Agent tool was not found".to_owned()))?;
    let now_ms = current_time_ms()?;
    if pending.expires_at_ms <= now_ms {
        cancel_pending_provider_siblings(state, &pending.context)?;
        return Err(DesktopError::Agent(
            "pending Agent tool confirmation expired".to_owned(),
        ));
    }
    let tools = production_tool_registry().map_err(agent_error)?;
    let coordinator = AgentCoordinator::new(providers, &tools);
    match pending.context.clone() {
        PendingAgentToolContext::Standalone { task } => {
            confirm_standalone_agent_tool(state, &coordinator, pending, task, now_ms)
        }
        PendingAgentToolContext::ProviderTurn {
            approval_index,
            provider_call_id,
            session,
        } => confirm_provider_agent_tool(
            providers,
            state,
            &coordinator,
            pending,
            approval_index,
            provider_call_id,
            &session,
            now_ms,
        ),
    }
}

fn confirm_standalone_agent_tool(
    state: &DesktopState,
    coordinator: &AgentCoordinator<'_, BaseRiskEvaluator>,
    pending: PendingAgentTool,
    mut task: AgentTask,
    now_ms: u64,
) -> Result<(AgentToolResult, Option<ProviderAgentOutcome>), DesktopError> {
    let backend = GatewayToolBackend::new(
        DesktopCapabilityBackend { state },
        GatewayToolBackend::<DesktopCapabilityBackend<'_>>::standard_policy(task.id, task.trace_id),
    );
    let ToolStepOutcome::Completed { output, .. } = coordinator
        .tool_step(
            &mut task,
            &backend,
            pending.invocation.clone(),
            Some(&pending.approval),
            now_ms,
        )
        .map_err(agent_error)?
    else {
        return Err(DesktopError::Agent(
            "approved Agent tool remained pending".to_owned(),
        ));
    };
    task.transition(AgentTaskStatus::Succeeded, now_ms)
        .map_err(agent_error)?;
    Ok((completed_agent_tool_result(pending, task, output), None))
}

#[allow(clippy::too_many_arguments)]
fn confirm_provider_agent_tool(
    providers: &ProviderRegistry,
    state: &DesktopState,
    coordinator: &AgentCoordinator<'_, BaseRiskEvaluator>,
    pending: PendingAgentTool,
    approval_index: usize,
    provider_call_id: String,
    session: &Arc<Mutex<PendingProviderAgent>>,
    now_ms: u64,
) -> Result<(AgentToolResult, Option<ProviderAgentOutcome>), DesktopError> {
    let mut session_guard = session.lock().map_err(|_| DesktopError::StatePoisoned)?;
    let approved = ApprovedProviderTool {
        provider_call_id,
        invocation: pending.invocation.clone(),
        approval: pending.approval.clone(),
    };
    let approval_slot = session_guard
        .approvals
        .get_mut(approval_index)
        .ok_or_else(|| DesktopError::Agent("Provider approval index was invalid".to_owned()))?;
    if approval_slot.replace(approved).is_some() {
        return Err(DesktopError::Agent(
            "Provider tool was already approved".to_owned(),
        ));
    }
    session_guard.remaining_confirmations = session_guard.remaining_confirmations.saturating_sub(1);
    if session_guard.remaining_confirmations > 0 {
        return Ok((
            AgentToolResult {
                spec: "nimora.desktop-agent-tool-result/1",
                task: session_guard.task.clone(),
                invocation: pending.invocation,
                effective_risk: pending.effective_risk,
                requires_confirmation: false,
                expires_at_ms: None,
                output: None,
            },
            None,
        ));
    }
    let backend = GatewayToolBackend::new(
        DesktopCapabilityBackend { state },
        GatewayToolBackend::<DesktopCapabilityBackend<'_>>::standard_policy(
            session_guard.task.id,
            session_guard.task.trace_id,
        ),
    );
    let approvals = std::mem::take(&mut session_guard.approvals);
    let mut confirmed_output = None;
    for approved in approvals.into_iter().flatten() {
        let ToolStepOutcome::Completed { output, .. } = coordinator
            .tool_step(
                &mut session_guard.task,
                &backend,
                approved.invocation.clone(),
                Some(&approved.approval),
                now_ms,
            )
            .map_err(agent_error)?
        else {
            return Err(DesktopError::Agent(
                "approved Provider tool remained pending".to_owned(),
            ));
        };
        session_guard
            .turn
            .record_result(
                &approved.provider_call_id,
                approved.invocation.tool_id.as_str(),
                output.clone(),
            )
            .map_err(agent_error)?;
        if approved.invocation.invocation_id == pending.invocation.invocation_id {
            confirmed_output = Some(output);
        }
    }
    let continuation_messages = session_guard
        .turn
        .continuation_messages()
        .map_err(agent_error)?;
    session_guard.messages.extend(continuation_messages);
    let task = session_guard.task.clone();
    let model = session_guard.model.clone();
    let messages = session_guard.messages.clone();
    let max_output_tokens = session_guard.max_output_tokens;
    let offline = session_guard.offline;
    drop(session_guard);
    let continuation = advance_provider_agent(
        providers,
        state,
        task,
        model,
        messages,
        max_output_tokens,
        offline,
    )?;
    let final_task = match &continuation {
        ProviderAgentOutcome::Completed { task, .. }
        | ProviderAgentOutcome::Waiting { task, .. } => task.clone(),
    };
    Ok((
        completed_agent_tool_result(
            pending,
            final_task,
            confirmed_output.ok_or_else(|| {
                DesktopError::Agent("confirmed Provider tool produced no output".to_owned())
            })?,
        ),
        Some(continuation),
    ))
}

fn completed_agent_tool_result(
    pending: PendingAgentTool,
    task: AgentTask,
    output: serde_json::Value,
) -> AgentToolResult {
    AgentToolResult {
        spec: "nimora.desktop-agent-tool-result/1",
        task,
        invocation: pending.invocation,
        effective_risk: pending.effective_risk,
        requires_confirmation: false,
        expires_at_ms: None,
        output: Some(output),
    }
}

fn desktop_provider_registry(state: &DesktopState) -> Result<ProviderRegistry, DesktopError> {
    let mut providers = ProviderRegistry::default();
    providers
        .register(DeterministicLocalProvider::new().map_err(agent_error)?)
        .map_err(agent_error)?;
    if let Some(executable) = &state.ollama_worker {
        let endpoint = OllamaEndpoint::new(
            "127.0.0.1".parse().expect("constant loopback address"),
            11_434,
        )
        .map_err(agent_error)?;
        providers
            .register(WorkerOllamaProvider::new(executable, endpoint).map_err(agent_error)?)
            .map_err(agent_error)?;
    }
    Ok(providers)
}

fn cancel_pending_provider_siblings(
    state: &DesktopState,
    context: &PendingAgentToolContext,
) -> Result<(), DesktopError> {
    let PendingAgentToolContext::ProviderTurn { session, .. } = context else {
        return Ok(());
    };
    state
        .pending_agent_tools
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?
        .retain(|_, pending| match &pending.context {
            PendingAgentToolContext::Standalone { .. } => true,
            PendingAgentToolContext::ProviderTurn {
                session: candidate, ..
            } => !Arc::ptr_eq(candidate, session),
        });
    Ok(())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn reject_agent_tool(
    request: ResolveAgentToolRequest,
    state: State<'_, DesktopState>,
) -> Result<(), DesktopError> {
    reject_agent_tool_inner(&request, &state)
}

fn reject_agent_tool_inner(
    request: &ResolveAgentToolRequest,
    state: &DesktopState,
) -> Result<(), DesktopError> {
    let pending = state
        .pending_agent_tools
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?
        .remove(&request.invocation_id)
        .ok_or_else(|| DesktopError::Agent("pending Agent tool was not found".to_owned()))?;
    cancel_pending_provider_siblings(state, &pending.context)?;
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn agent_error(error: impl ToString) -> DesktopError {
    DesktopError::Agent(error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn drain_runtime_events(state: State<'_, DesktopState>) -> Result<Vec<Event>, DesktopError> {
    Ok(state.events.drain()?)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn outbox_snapshot(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
) -> Result<OutboxSnapshot, DesktopError> {
    if window.label() != CONTROL_CENTER_LABEL {
        return Err(DesktopError::WindowForbidden);
    }
    Ok(state.outbox.snapshot()?)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn backup_health(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
) -> Result<BackupHealth, DesktopError> {
    if window.label() != CONTROL_CENTER_LABEL {
        return Err(DesktopError::WindowForbidden);
    }
    let mut health = state.backups.health()?;
    let last_error = state
        .backup_last_error
        .lock()
        .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
    health.last_error.clone_from(&last_error);
    Ok(health)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn create_backup(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
) -> Result<BackupRecord, DesktopError> {
    if window.label() != CONTROL_CENTER_LABEL {
        return Err(DesktopError::WindowForbidden);
    }
    ensure_normal_mode(&state)?;
    match state.backups.create_now() {
        Ok(record) => {
            *state
                .backup_last_error
                .lock()
                .map_err(|_| SqlitePersistenceError::StatePoisoned)? = None;
            Ok(record)
        }
        Err(error) => {
            if let Ok(mut last_error) = state.backup_last_error.lock() {
                *last_error = Some(error.to_string());
            }
            Err(error.into())
        }
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn request_database_restore(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    backup_id: String,
) -> Result<(), DesktopError> {
    if window.label() != CONTROL_CENTER_LABEL {
        return Err(DesktopError::WindowForbidden);
    }
    ensure_safe_mode_inactive(&state)?;
    state.backups.request_restore(&backup_id)?;
    Ok(())
}

fn current_time_ms() -> Result<u64, DesktopError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(SqlitePersistenceError::from)?
        .as_millis()
        .try_into()
        .map_err(|_| SqlitePersistenceError::InvalidBackupRequest.into())
}

fn diagnostic_event(
    severity: DiagnosticSeverity,
    component: DiagnosticComponent,
    code: DiagnosticEventCode,
) -> Result<DiagnosticEvent, DesktopError> {
    Ok(DiagnosticEvent {
        occurred_at_ms: current_time_ms()?,
        severity,
        component,
        code,
    })
}

fn record_diagnostic_event(
    state: &DesktopState,
    severity: DiagnosticSeverity,
    component: DiagnosticComponent,
    code: DiagnosticEventCode,
) -> Result<(), DesktopError> {
    state
        .diagnostic_journal
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?
        .record(diagnostic_event(severity, component, code)?)?;
    Ok(())
}

fn diagnostic_report(state: &DesktopState) -> Result<DiagnosticReport, DesktopError> {
    let safety = state.safety.snapshot()?;
    let outbox = state.outbox.snapshot()?;
    let mut backup_health = state.backups.health()?;
    let last_error = state
        .backup_last_error
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?;
    backup_health.last_error.clone_from(&last_error);
    let generated_at_ms = current_time_ms()?;
    let event_count = state
        .diagnostic_journal
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?
        .len()
        .try_into()
        .map_err(|_| SqlitePersistenceError::InvalidBackupRequest)?;
    Ok(DiagnosticReport {
        spec: "nimora.diagnostic-report/1".to_owned(),
        generated_at_ms,
        application: ApplicationSummary {
            name: "Nimora".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
        },
        system: SystemSummary {
            os: std::env::consts::OS.to_owned(),
            architecture: std::env::consts::ARCH.to_owned(),
        },
        runtime: RuntimeSummary {
            startup_mode: match state.startup.mode {
                StartupMode::Normal => "normal",
                StartupMode::Recovery => "recovery",
            }
            .to_owned(),
            startup_reason: state.startup.reason.map(str::to_owned),
            safety_mode: match safety.mode {
                RuntimeMode::Normal => "normal",
                RuntimeMode::Safe => "safe",
            }
            .to_owned(),
            outbox_pending: outbox.pending,
            outbox_dead_letter: outbox.dead_letter,
        },
        data_protection: DataProtectionSummary {
            database_schema: u32::try_from(DATABASE_VERSION)
                .map_err(|_| SqlitePersistenceError::InvalidBackupRequest)?,
            backup_count: backup_health.available.len() as u64,
            latest_backup_at_ms: backup_health.latest.map(|record| record.created_at_ms),
            pending_restore: backup_health.pending_restore.is_some(),
            last_backup_error: backup_health.last_error.is_some(),
        },
        sources: DiagnosticSourcesSummary {
            event_count,
            event_retention_days: DiagnosticJournalPolicy::default().retention_days,
        },
        privacy: PrivacySummary {
            includes_logs: false,
            includes_user_content: false,
            includes_secrets: false,
            includes_file_paths: false,
            automatically_uploaded: false,
        },
    })
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn preview_diagnostic_report(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
) -> Result<DiagnosticReport, DesktopError> {
    if window.label() != CONTROL_CENTER_LABEL {
        return Err(DesktopError::WindowForbidden);
    }
    diagnostic_report(&state)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn export_diagnostics(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    request: ExportDiagnosticRequest,
) -> Result<DiagnosticBundleReceipt, DesktopError> {
    if window.label() != CONTROL_CENTER_LABEL {
        return Err(DesktopError::WindowForbidden);
    }
    let events = state
        .diagnostic_journal
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?
        .snapshot();
    Ok(export_diagnostic_bundle(
        &diagnostic_report(&state)?,
        &events,
        DiagnosticBundleSelection {
            include_events: request.include_events,
        },
        &request.destination_path,
    )?)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn profile_snapshot(state: State<'_, DesktopState>) -> Result<ProfileSnapshot, DesktopError> {
    Ok(state.profiles.snapshot()?)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn create_profile(
    state: State<'_, DesktopState>,
    name: String,
    policy: ProfilePolicy,
) -> Result<Command, DesktopError> {
    ensure_normal_mode(&state)?;
    Ok(state.profiles.create_profile(name, policy)?)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn switch_profile(
    app: AppHandle,
    state: State<'_, DesktopState>,
    profile_id: ProfileId,
) -> Result<Command, DesktopError> {
    switch_profile_inner(&app, &state, profile_id)
}

fn switch_profile_inner(
    app: &AppHandle,
    state: &DesktopState,
    profile_id: ProfileId,
) -> Result<Command, DesktopError> {
    ensure_normal_mode(state)?;
    let snapshot = state.profiles.snapshot()?;
    let target = snapshot
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or(ProfileServiceError::ProfileNotFound)?;
    let next_policy = WindowPolicy::from_profile(&target.policy);
    let previous_policy = current_window_policy(state)?;
    apply_window_policy(app, previous_policy, next_policy)?;
    match state.profiles.switch_active(profile_id) {
        Ok(command) => {
            set_current_window_policy(state, next_policy)?;
            Ok(command)
        }
        Err(primary) => match apply_window_policy(app, next_policy, previous_policy) {
            Ok(()) => Err(primary.into()),
            Err(rollback) => Err(DesktopError::NativePolicyRollback {
                primary: primary.to_string(),
                rollback: rollback.to_string(),
            }),
        },
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn enter_safe_mode(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<Command, DesktopError> {
    let previous_policy = current_window_policy(&state)?;
    apply_window_policy(&app, previous_policy, WindowPolicy::SAFE)?;
    match state.safety.enter(SafeModeReason::Manual) {
        Ok(command) => {
            cancel_all_user_programs(&state)?;
            cancel_all_user_program_event_sessions(&state)?;
            cancel_all_pending_agent_tools(&state)?;
            *state
                .policy_before_safe_mode
                .lock()
                .map_err(|_| DesktopError::StatePoisoned)? = Some(previous_policy);
            set_current_window_policy(&state, WindowPolicy::SAFE)?;
            app.emit_to(PET_WINDOW_LABEL, CHARACTER_RENDERER_CHANGED_EVENT, ())?;
            Ok(command)
        }
        Err(primary) => match apply_window_policy(&app, WindowPolicy::SAFE, previous_policy) {
            Ok(()) => Err(primary.into()),
            Err(rollback) => Err(DesktopError::NativePolicyRollback {
                primary: primary.to_string(),
                rollback: rollback.to_string(),
            }),
        },
    }
}

fn cancel_all_pending_agent_tools(state: &DesktopState) -> Result<(), DesktopError> {
    state
        .pending_agent_tools
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?
        .clear();
    Ok(())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn exit_safe_mode(app: AppHandle, state: State<'_, DesktopState>) -> Result<Command, DesktopError> {
    let previous_policy = current_window_policy(&state)?;
    let target_policy = state
        .policy_before_safe_mode
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?
        .unwrap_or(active_window_policy(&state.profiles.snapshot()?)?);
    apply_window_policy(&app, previous_policy, target_policy)?;
    match state.safety.exit() {
        Ok(command) => {
            *state
                .policy_before_safe_mode
                .lock()
                .map_err(|_| DesktopError::StatePoisoned)? = None;
            set_current_window_policy(&state, target_policy)?;
            app.emit_to(PET_WINDOW_LABEL, CHARACTER_RENDERER_CHANGED_EVENT, ())?;
            Ok(command)
        }
        Err(primary) => match apply_window_policy(&app, target_policy, previous_policy) {
            Ok(()) => Err(primary.into()),
            Err(rollback) => Err(DesktopError::NativePolicyRollback {
                primary: primary.to_string(),
                rollback: rollback.to_string(),
            }),
        },
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn move_pet(
    app: AppHandle,
    state: State<'_, DesktopState>,
    request: MovePetRequest,
) -> Result<Command, DesktopError> {
    ensure_normal_mode(&state)?;
    let screen_x = screen_coordinate(request.x)?;
    let screen_y = screen_coordinate(request.y)?;
    let position = Position {
        x: request.x,
        y: request.y,
    };
    let window = app
        .get_webview_window(PET_WINDOW_LABEL)
        .ok_or_else(|| DesktopError::WindowUnavailable(PET_WINDOW_LABEL.to_owned()))?;
    let previous = state.runtime.snapshot()?.position;
    window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
        screen_x, screen_y,
    )))?;
    match state.runtime.move_pet(position) {
        Ok(command) => Ok(command),
        Err(error) => {
            if let (Ok(previous_x), Ok(previous_y)) =
                (screen_coordinate(previous.x), screen_coordinate(previous.y))
            {
                let _ = window.set_position(tauri::Position::Physical(
                    tauri::PhysicalPosition::new(previous_x, previous_y),
                ));
            }
            Err(error.into())
        }
    }
}

fn screen_coordinate(value: f64) -> Result<i32, DesktopError> {
    if !value.is_finite() || value < f64::from(i32::MIN) || value > f64::from(i32::MAX) {
        return Err(DesktopError::InvalidPosition);
    }
    #[allow(clippy::cast_possible_truncation)]
    Ok(value.round() as i32)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn play_pet_action(
    state: State<'_, DesktopState>,
    action: PetAction,
) -> Result<Command, DesktopError> {
    ensure_normal_mode(&state)?;
    Ok(state.runtime.play_action(action)?)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn click_pet(
    app: AppHandle,
    state: State<'_, DesktopState>,
    request: ClickPetRequest,
) -> Result<Command, DesktopError> {
    ensure_normal_mode(&state)?;
    let command = state.runtime.click_pet(
        Position {
            x: request.x,
            y: request.y,
        },
        request.button,
    )?;
    tauri::async_runtime::spawn_blocking(move || {
        std::thread::sleep(CLICK_FEEDBACK_DURATION);
        let _ = app.state::<DesktopState>().runtime.finish_interaction();
    });
    Ok(command)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn begin_pet_drag(state: State<'_, DesktopState>) -> Result<Command, DesktopError> {
    ensure_normal_mode(&state)?;
    let command = state.runtime.begin_drag()?;
    state.dragging.store(true, Ordering::Release);
    Ok(command)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn finish_pet_drag(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<Command, DesktopError> {
    ensure_normal_mode(&state)?;
    let window = app
        .get_webview_window(PET_WINDOW_LABEL)
        .ok_or_else(|| DesktopError::WindowUnavailable(PET_WINDOW_LABEL.to_owned()))?;
    let position = window.outer_position()?;
    let command = state.runtime.drop_pet(Position {
        x: f64::from(position.x),
        y: f64::from(position.y),
    })?;
    state.dragging.store(false, Ordering::Release);
    Ok(command)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn set_click_through(
    app: AppHandle,
    state: State<'_, DesktopState>,
    enabled: bool,
) -> Result<(), DesktopError> {
    if enabled {
        ensure_normal_mode(&state)?;
    }
    app.get_webview_window(PET_WINDOW_LABEL)
        .ok_or_else(|| DesktopError::WindowUnavailable(PET_WINDOW_LABEL.to_owned()))?
        .set_ignore_cursor_events(enabled)?;
    let mut policy = state
        .window_policy
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?;
    policy.click_through = enabled;
    Ok(())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn install_asset(
    state: State<'_, DesktopState>,
    request: InstallAssetRequest,
) -> Result<AssetInstallReceipt, DesktopError> {
    ensure_normal_mode(&state)?;
    validate_package_source(&request.source_path)?;
    let result = install_asset_source(&request.source_path, &state.asset_store)?;
    Ok(AssetInstallReceipt {
        asset_id: result.asset_id,
        replaced_previous: result.install.backup_path.is_some(),
    })
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn preview_asset(
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    request: InstallAssetRequest,
) -> Result<AssetPreviewReport, DesktopError> {
    if window.label() != CONTROL_CENTER_LABEL {
        return Err(DesktopError::WindowForbidden);
    }
    ensure_normal_mode(&state)?;
    validate_package_source(&request.source_path)?;
    Ok(inspect_asset_source_preview(&request.source_path)?)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn inspect_model(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    request: InspectModelRequest,
) -> Result<ModelProbeReport, DesktopError> {
    if window.label() != CONTROL_CENTER_LABEL {
        return Err(DesktopError::WindowForbidden);
    }
    ensure_normal_mode(&state)?;
    validate_model_source(&request.source_path)?;

    let staging = stage_model(&app, &request.source_path)?;
    let request = ModelProbeRequest {
        spec: "nimora.model-probe/1".to_owned(),
        source: PathBuf::from("character.glb"),
    };
    Ok(probe_model_in_worker(
        &model_importer_worker_path(&app),
        &staging.root,
        &request,
        MODEL_PROBE_TIMEOUT,
    )?)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn import_model(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, DesktopState>,
    request: ImportModelRequest,
) -> Result<AssetInstallReceipt, DesktopError> {
    if window.label() != CONTROL_CENTER_LABEL {
        return Err(DesktopError::WindowForbidden);
    }
    ensure_normal_mode(&state)?;
    validate_model_source(&request.source_path)?;
    let staging = stage_model(&app, &request.source_path)?;
    let report = probe_model_in_worker(
        &model_importer_worker_path(&app),
        &staging.root,
        &ModelProbeRequest {
            spec: "nimora.model-probe/1".to_owned(),
            source: PathBuf::from("character.glb"),
        },
        MODEL_PROBE_TIMEOUT,
    )?;
    validate_requested_animation_map(&request.animation_map, &report.animation_names)?;
    let result = install_gltf_character(
        &staging.root.join("character.glb"),
        &state.asset_store,
        &GltfCharacterMetadata {
            id: request.asset_id,
            version: "1.0.0".to_owned(),
            name: request.name,
            publisher: "publisher.local".to_owned(),
            license: request.license,
            animation_map: request.animation_map,
        },
    )?;
    Ok(AssetInstallReceipt {
        asset_id: result.asset_id,
        replaced_previous: result.install.backup_path.is_some(),
    })
}

fn validate_requested_animation_map(
    animation_map: &BTreeMap<String, ModelAnimationBinding>,
    animation_names: &[String],
) -> Result<(), DesktopError> {
    nimora_asset_installer::validate_model_animation_bindings(animation_map)?;
    if !animation_names.is_empty() && animation_map.is_empty() {
        return Err(nimora_asset_installer::InstallError::InvalidMetadata(
            "models with named animations must map pet.idle".to_owned(),
        )
        .into());
    }
    if animation_map
        .values()
        .any(|binding| !animation_names.contains(&binding.animation))
    {
        return Err(nimora_asset_installer::InstallError::InvalidMetadata(
            "model animation map references an animation absent from the latest probe".to_owned(),
        )
        .into());
    }
    Ok(())
}

fn validate_model_source(source_path: &Path) -> Result<(), DesktopError> {
    if !source_path.is_absolute()
        || source_path.extension().and_then(|value| value.to_str()) != Some("glb")
    {
        return Err(DesktopError::InvalidModelSource);
    }
    let metadata =
        fs::symlink_metadata(source_path).map_err(|_| DesktopError::InvalidModelSource)?;
    if !metadata.file_type().is_file() {
        return Err(DesktopError::InvalidModelSource);
    }
    if metadata.len() > MAX_MODEL_BYTES {
        return Err(DesktopError::ModelInputBudgetExceeded);
    }
    Ok(())
}

fn stage_model(app: &AppHandle, source_path: &Path) -> Result<ModelStagingDirectory, DesktopError> {
    let root = app
        .path()
        .app_cache_dir()?
        .join("model-probes")
        .join(Uuid::now_v7().to_string());
    fs::create_dir_all(&root)?;
    let staging = ModelStagingDirectory { root };
    let destination = staging.root.join("character.glb");
    let copied = fs::copy(source_path, &destination)?;
    if copied > MAX_MODEL_BYTES || copied != fs::metadata(source_path)?.len() {
        return Err(DesktopError::ModelInputBudgetExceeded);
    }
    fs::File::open(destination)?.sync_all()?;
    Ok(staging)
}

fn model_importer_worker_path(app: &AppHandle) -> PathBuf {
    std::env::var_os("NIMORA_MODEL_IMPORTER_WORKER_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            let executable_candidates = app
                .path()
                .executable_dir()
                .ok()
                .into_iter()
                .map(|directory| directory.join("nimora-model-importer-worker"));
            let resource_candidates =
                app.path()
                    .resource_dir()
                    .ok()
                    .into_iter()
                    .flat_map(|directory| {
                        [
                            directory.join("binaries/nimora-model-importer-worker"),
                            directory.join("nimora-model-importer-worker"),
                        ]
                    });
            executable_candidates
                .chain(resource_candidates)
                .find(|path| path.is_file())
        })
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|path| path.parent().map(Path::to_path_buf))
                .map(|directory| directory.join("nimora-model-importer-worker"))
        })
        .unwrap_or_else(|| PathBuf::from("nimora-model-importer-worker"))
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn export_asset(
    state: State<'_, DesktopState>,
    request: ExportAssetRequest,
) -> Result<AssetPackageSummary, DesktopError> {
    ensure_normal_mode(&state)?;
    if !request.source_path.is_absolute() || !request.source_path.is_dir() {
        return Err(DesktopError::InvalidPackageSource);
    }
    Ok(export_asset_package(
        &request.source_path,
        &request.destination_path,
    )?)
}

fn validate_package_source(source_path: &Path) -> Result<(), DesktopError> {
    if !source_path.is_absolute() || (!source_path.is_dir() && !source_path.is_file()) {
        return Err(DesktopError::InvalidPackageSource);
    }
    Ok(())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn asset_catalog(state: State<'_, DesktopState>) -> Result<AssetCatalogSnapshot, DesktopError> {
    inspect_asset_catalog(&state.asset_store)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn active_character(
    state: State<'_, DesktopState>,
) -> Result<ActiveCharacterSnapshot, DesktopError> {
    resolve_active_character(&state.asset_store, state.safety.snapshot()?.mode)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn active_character_renderer(
    state: State<'_, DesktopState>,
) -> Result<CharacterRendererSnapshot, DesktopError> {
    resolve_character_renderer(&state.asset_store, state.safety.snapshot()?.mode)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn activate_character(
    app: AppHandle,
    state: State<'_, DesktopState>,
    asset_id: String,
) -> Result<ActiveCharacterSnapshot, DesktopError> {
    activate_character_inner(&app, &state, &asset_id)
}

fn activate_character_inner(
    app: &AppHandle,
    state: &DesktopState,
    asset_id: &str,
) -> Result<ActiveCharacterSnapshot, DesktopError> {
    ensure_normal_mode(state)?;
    let _write_guard = state
        .active_character_write
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?;
    if asset_id != BUILTIN_CHARACTER_ID && !valid_asset_identifier(asset_id) {
        return Err(DesktopError::InvalidAssetIdentifier);
    }
    if asset_id != BUILTIN_CHARACTER_ID {
        let asset = inspect_asset_package(&state.asset_store.join(asset_id))?;
        if asset.id != asset_id || asset.asset_type != "character" {
            return Err(DesktopError::AssetIsNotCharacter);
        }
    }
    let previous = resolve_active_character(&state.asset_store, RuntimeMode::Normal)?;
    persist_active_character(&state.asset_store, asset_id)?;
    let activation = (|| {
        let snapshot = resolve_active_character(&state.asset_store, RuntimeMode::Normal)?;
        app.emit_to(PET_WINDOW_LABEL, CHARACTER_RENDERER_CHANGED_EVENT, ())?;
        Ok(snapshot)
    })();
    match activation {
        Ok(snapshot) => Ok(snapshot),
        Err(primary) => match persist_active_character(&state.asset_store, &previous.asset_id) {
            Ok(()) => Err(primary),
            Err(rollback) => Err(DesktopError::CharacterActivationRollback {
                primary: primary.to_string(),
                rollback: rollback.to_string(),
            }),
        },
    }
}

fn resolve_active_character(
    asset_store: &Path,
    runtime_mode: RuntimeMode,
) -> Result<ActiveCharacterSnapshot, DesktopError> {
    if runtime_mode == RuntimeMode::Safe {
        return Ok(builtin_character(Some(
            "safe mode uses the built-in character".to_owned(),
        )));
    }
    let selection_path = asset_store.join(ACTIVE_CHARACTER_FILE);
    let stored = match fs::read(&selection_path) {
        Ok(bytes) => match serde_json::from_slice::<StoredActiveCharacter>(&bytes) {
            Ok(stored) if stored.spec == ACTIVE_CHARACTER_SPEC => stored,
            Ok(_) => {
                return Ok(builtin_character(Some(
                    "unknown selection contract".to_owned(),
                )));
            }
            Err(_) => {
                return Ok(builtin_character(Some(
                    "selection record is corrupt".to_owned(),
                )));
            }
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(builtin_character(None));
        }
        Err(error) => return Err(error.into()),
    };
    if stored.asset_id == BUILTIN_CHARACTER_ID {
        return Ok(builtin_character(None));
    }
    if !valid_asset_identifier(&stored.asset_id) {
        return Ok(builtin_character(Some(
            "selection identifier is invalid".to_owned(),
        )));
    }
    match inspect_asset_package(&asset_store.join(&stored.asset_id)) {
        Ok(asset) if asset.id == stored.asset_id && asset.asset_type == "character" => {
            Ok(ActiveCharacterSnapshot {
                asset_id: stored.asset_id,
                source: ActiveCharacterSource::Installed,
                fallback_reason: None,
            })
        }
        Ok(_) => Ok(builtin_character(Some(
            "selected asset is not a valid character".to_owned(),
        ))),
        Err(error) => Ok(builtin_character(Some(format!(
            "selected character is unavailable: {error}"
        )))),
    }
}

fn builtin_character(fallback_reason: Option<String>) -> ActiveCharacterSnapshot {
    ActiveCharacterSnapshot {
        asset_id: BUILTIN_CHARACTER_ID.to_owned(),
        source: ActiveCharacterSource::BuiltIn,
        fallback_reason,
    }
}

fn resolve_character_renderer(
    asset_store: &Path,
    runtime_mode: RuntimeMode,
) -> Result<CharacterRendererSnapshot, DesktopError> {
    let active = resolve_active_character(asset_store, runtime_mode)?;
    if matches!(active.source, ActiveCharacterSource::BuiltIn) {
        return Ok(builtin_renderer(active.fallback_reason));
    }
    match inspect_asset_renderer(&asset_store.join(&active.asset_id)) {
        Ok(renderer) => Ok(installed_renderer(active.asset_id, renderer)),
        Err(error) => Ok(builtin_renderer(Some(format!(
            "selected character renderer is unavailable: {error}"
        )))),
    }
}

fn installed_renderer(
    asset_id: String,
    renderer: AssetRendererDescriptor,
) -> CharacterRendererSnapshot {
    CharacterRendererSnapshot {
        spec: "nimora.renderer/1",
        asset_base_url: Some(asset_base_url(&asset_id)),
        asset_id,
        backend: renderer.backend,
        canvas: renderer.canvas,
        anchor: renderer.anchor,
        default_scale: renderer.default_scale,
        pixel_art: renderer.pixel_art,
        fallbacks: renderer.fallbacks,
        clips: renderer.clips,
        model: renderer.model,
        fallback_reason: None,
    }
}

fn builtin_renderer(fallback_reason: Option<String>) -> CharacterRendererSnapshot {
    CharacterRendererSnapshot {
        spec: "nimora.renderer/1",
        asset_id: BUILTIN_CHARACTER_ID.to_owned(),
        asset_base_url: None,
        backend: "built-in".to_owned(),
        canvas: RenderCanvas {
            width: 320,
            height: 360,
        },
        anchor: RenderAnchor { x: 0.5, y: 1.0 },
        default_scale: 1.0,
        pixel_art: false,
        fallbacks: std::collections::BTreeMap::new(),
        clips: None,
        model: None,
        fallback_reason,
    }
}

fn asset_base_url(asset_id: &str) -> String {
    if cfg!(any(target_os = "windows", target_os = "android")) {
        format!("http://{ASSET_PROTOCOL}.localhost/{asset_id}/")
    } else {
        format!("{ASSET_PROTOCOL}://localhost/{asset_id}/")
    }
}

fn persist_active_character(asset_store: &Path, asset_id: &str) -> Result<(), DesktopError> {
    fs::create_dir_all(asset_store)?;
    let destination = asset_store.join(ACTIVE_CHARACTER_FILE);
    let temporary = asset_store.join(format!("{ACTIVE_CHARACTER_FILE}.{}.tmp", Uuid::now_v7()));
    let payload = serde_json::to_vec(&StoredActiveCharacter {
        spec: ACTIVE_CHARACTER_SPEC.to_owned(),
        asset_id: asset_id.to_owned(),
    })?;
    fs::write(&temporary, payload)?;
    if let Err(error) = fs::rename(&temporary, &destination) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    Ok(())
}

fn inspect_asset_catalog(asset_store: &Path) -> Result<AssetCatalogSnapshot, DesktopError> {
    std::fs::create_dir_all(asset_store)?;
    let mut assets = Vec::new();
    let mut rejected = Vec::new();
    for entry in std::fs::read_dir(asset_store)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let directory = entry.file_name().to_string_lossy().into_owned();
        if directory.contains(".backup.")
            || directory.contains(".failed.")
            || directory.contains(".staging.")
        {
            continue;
        }
        match inspect_asset_package(&entry.path()) {
            Ok(asset) if asset.id == directory => assets.push(asset),
            Ok(asset) => rejected.push(RejectedAssetPackage {
                directory,
                reason: format!(
                    "manifest id {} does not match installed directory",
                    asset.id
                ),
            }),
            Err(error) => rejected.push(RejectedAssetPackage {
                directory,
                reason: error.to_string(),
            }),
        }
    }
    assets.sort_by(|left, right| left.id.cmp(&right.id));
    rejected.sort_by(|left, right| left.directory.cmp(&right.directory));
    Ok(AssetCatalogSnapshot { assets, rejected })
}

fn serve_asset_protocol(
    asset_store: &Path,
    runtime_mode: RuntimeMode,
    webview_label: &str,
    method: &tauri::http::Method,
    uri: &tauri::http::Uri,
) -> AssetProtocolResponse {
    if webview_label != PET_WINDOW_LABEL {
        return asset_protocol_error(tauri::http::StatusCode::FORBIDDEN);
    }
    if method != tauri::http::Method::GET || uri.query().is_some() {
        return asset_protocol_error(tauri::http::StatusCode::BAD_REQUEST);
    }
    if !matches!(uri.host(), Some("localhost" | "nimora-asset.localhost")) {
        return asset_protocol_error(tauri::http::StatusCode::BAD_REQUEST);
    }
    let Some((asset_id, relative_path)) = parse_asset_protocol_path(uri.path()) else {
        return asset_protocol_error(tauri::http::StatusCode::BAD_REQUEST);
    };
    let Ok(active) = resolve_active_character(asset_store, runtime_mode) else {
        return asset_protocol_error(tauri::http::StatusCode::SERVICE_UNAVAILABLE);
    };
    if !matches!(active.source, ActiveCharacterSource::Installed) || active.asset_id != asset_id {
        return asset_protocol_error(tauri::http::StatusCode::FORBIDDEN);
    }
    match read_verified_asset_image(&asset_store.join(&asset_id), &relative_path) {
        Ok((body, media_type)) => AssetProtocolResponse {
            status: tauri::http::StatusCode::OK,
            media_type: match media_type.as_str() {
                "image/png" => "image/png",
                "image/webp" => "image/webp",
                "image/jpeg" => "image/jpeg",
                "image/gif" => "image/gif",
                _ => return asset_protocol_error(tauri::http::StatusCode::UNSUPPORTED_MEDIA_TYPE),
            },
            body,
        },
        Err(_) => match read_verified_asset_model(&asset_store.join(&asset_id), &relative_path) {
            Ok(body) => AssetProtocolResponse {
                status: tauri::http::StatusCode::OK,
                media_type: "model/gltf-binary",
                body,
            },
            Err(_) => asset_protocol_error(tauri::http::StatusCode::NOT_FOUND),
        },
    }
}

fn parse_asset_protocol_path(path: &str) -> Option<(String, PathBuf)> {
    let decoded = percent_decode_path(path.as_bytes())?;
    let decoded = std::str::from_utf8(&decoded).ok()?;
    let mut segments = decoded.strip_prefix('/')?.split('/');
    let asset_id = segments.next()?;
    if !valid_asset_identifier(asset_id) {
        return None;
    }
    let remaining = segments.collect::<Vec<_>>();
    if remaining.is_empty()
        || remaining.iter().any(|segment| {
            segment.is_empty() || *segment == "." || *segment == ".." || segment.contains('\\')
        })
    {
        return None;
    }
    let relative_path = PathBuf::from(remaining.join("/"));
    Some((asset_id.to_owned(), relative_path))
}

fn percent_decode_path(input: &[u8]) -> Option<Vec<u8>> {
    let mut decoded = Vec::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        if input[index] == b'%' {
            let high = *input.get(index + 1)?;
            let low = *input.get(index + 2)?;
            let byte = hex_value(high)?
                .checked_mul(16)?
                .checked_add(hex_value(low)?)?;
            if byte == 0 {
                return None;
            }
            decoded.push(byte);
            index += 3;
        } else {
            decoded.push(input[index]);
            index += 1;
        }
    }
    Some(decoded)
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn asset_protocol_error(status: tauri::http::StatusCode) -> AssetProtocolResponse {
    AssetProtocolResponse {
        status,
        media_type: "text/plain; charset=utf-8",
        body: status
            .canonical_reason()
            .unwrap_or("asset request failed")
            .as_bytes()
            .to_vec(),
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn rollback_asset(
    state: State<'_, DesktopState>,
    asset_id: String,
) -> Result<AssetRollbackReceipt, DesktopError> {
    ensure_normal_mode(&state)?;
    if !valid_asset_identifier(&asset_id) {
        return Err(DesktopError::InvalidAssetIdentifier);
    }
    let result = rollback_latest(&state.asset_store.join(&asset_id))?;
    Ok(AssetRollbackReceipt {
        asset_id,
        quarantined_failed_version: result.quarantined_path.is_some(),
    })
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn install_user_program(
    state: State<'_, DesktopState>,
    request: InstallUserProgramRequest,
) -> Result<UserProgramInstallReceipt, DesktopError> {
    ensure_normal_mode(&state)?;
    let files = request
        .files
        .into_iter()
        .map(|file| InstallFile {
            relative_path: file.relative_path,
            bytes: file.bytes,
            sha256: file.sha256,
        })
        .collect::<Vec<_>>();
    let result = install_program_atomically(
        &request.source_path,
        &state.program_store,
        request.manifest,
        &files,
    )?;
    cancel_user_program_workers(&state, &result.program_id)?;
    cancel_user_program_event_sessions(&state, &result.program_id)?;
    Ok(UserProgramInstallReceipt {
        program_id: result.program_id,
        version: result.version,
        replaced_previous: result.backup_path.is_some(),
    })
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn rollback_user_program(
    state: State<'_, DesktopState>,
    program_id: String,
) -> Result<UserProgramRollbackReceipt, DesktopError> {
    ensure_normal_mode(&state)?;
    cancel_user_program_workers(&state, &program_id)?;
    cancel_user_program_event_sessions(&state, &program_id)?;
    let result = rollback_program(&state.program_store, &program_id)?;
    Ok(UserProgramRollbackReceipt {
        program_id,
        quarantined_failed_version: result.quarantined_path.is_some(),
    })
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn user_program_permission_status(
    state: State<'_, DesktopState>,
    program_id: String,
) -> Result<UserProgramPermissionStatus, DesktopError> {
    ensure_normal_mode(&state)?;
    let installed = load_installed_program(&state.program_store, &program_id)?;
    permission_status(&state.program_permissions, installed.manifest)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn grant_user_program_permissions(
    state: State<'_, DesktopState>,
    program_id: String,
) -> Result<UserProgramPermissionStatus, DesktopError> {
    ensure_normal_mode(&state)?;
    let installed = load_installed_program(&state.program_store, &program_id)?;
    let grant = permission_grant(&installed.manifest);
    state.program_permissions.grant(&grant)?;
    permission_status(&state.program_permissions, installed.manifest)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn revoke_user_program_permissions(
    state: State<'_, DesktopState>,
    program_id: String,
) -> Result<(), DesktopError> {
    ensure_normal_mode(&state)?;
    state.program_permissions.revoke_program(&program_id)?;
    cancel_user_program_workers(&state, &program_id)?;
    cancel_user_program_event_sessions(&state, &program_id)?;
    Ok(())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn open_user_program_event_session(
    state: State<'_, DesktopState>,
    program_id: String,
) -> Result<UserProgramEventSessionReceipt, DesktopError> {
    ensure_normal_mode(&state)?;
    let installed = load_installed_program(&state.program_store, &program_id)?;
    let policy = evaluate(installed.manifest.clone())?;
    ensure_program_permissions(&state.program_permissions, &installed.manifest)?;
    if !policy.can_subscribe_events || policy.manifest.subscriptions.is_empty() {
        return Err(DesktopError::UserProgramSubscriptionsMissing);
    }
    let mut sessions = state
        .user_program_event_sessions
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?;
    if sessions.len() >= MAX_USER_PROGRAM_EVENT_SESSIONS {
        return Err(DesktopError::UserProgramEventSessionLimit);
    }
    let queue_capacity = policy.manifest.event_queue_capacity;
    let subscription = state
        .events
        .subscribe(policy.manifest.subscriptions.clone(), queue_capacity)?;
    let subscription_id = Uuid::now_v7();
    sessions.insert(
        subscription_id,
        UserProgramEventSession {
            program_id: policy.manifest.id.clone(),
            subscription,
            automatic: false,
            executed: 0,
            dropped: 0,
            last_error: None,
        },
    );
    Ok(UserProgramEventSessionReceipt {
        subscription_id,
        program_id: policy.manifest.id,
        version: policy.manifest.version,
        event_types: policy.manifest.subscriptions,
        queue_capacity,
    })
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn drain_user_program_events(
    state: State<'_, DesktopState>,
    subscription_id: Uuid,
) -> Result<RuntimeEventBatch, DesktopError> {
    ensure_normal_mode(&state)?;
    let sessions = state
        .user_program_event_sessions
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?;
    let session = sessions
        .get(&subscription_id)
        .ok_or(DesktopError::UserProgramEventSessionNotFound)?;
    Ok(session.subscription.drain()?)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn execute_next_user_program_event(
    app: AppHandle,
    state: State<'_, DesktopState>,
    subscription_id: Uuid,
) -> Result<UserProgramEventExecutionReceipt, DesktopError> {
    ensure_normal_mode(&state)?;
    execute_next_user_program_event_inner(&app, &state, subscription_id)
}

fn execute_next_user_program_event_inner(
    app: &AppHandle,
    state: &DesktopState,
    subscription_id: Uuid,
) -> Result<UserProgramEventExecutionReceipt, DesktopError> {
    let (program_id, batch) = {
        let sessions = state
            .user_program_event_sessions
            .lock()
            .map_err(|_| DesktopError::StatePoisoned)?;
        let session = sessions
            .get(&subscription_id)
            .ok_or(DesktopError::UserProgramEventSessionNotFound)?;
        (session.program_id.clone(), session.subscription.pop()?)
    };
    let Some(event) = batch.events.into_iter().next() else {
        return Ok(UserProgramEventExecutionReceipt {
            execution: None,
            dropped: batch.dropped,
        });
    };
    let installed = load_installed_program(&state.program_store, &program_id)?;
    ensure_program_permissions(&state.program_permissions, &installed.manifest)?;
    let execution = execute_user_program_source(
        app,
        state,
        installed.manifest,
        installed.source,
        Some(event),
    )?;
    Ok(UserProgramEventExecutionReceipt {
        execution: Some(execution),
        dropped: batch.dropped,
    })
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn start_user_program_event_loop(
    app: AppHandle,
    state: State<'_, DesktopState>,
    subscription_id: Uuid,
) -> Result<(), DesktopError> {
    ensure_normal_mode(&state)?;
    let program_id = {
        let sessions = state
            .user_program_event_sessions
            .lock()
            .map_err(|_| DesktopError::StatePoisoned)?;
        sessions
            .get(&subscription_id)
            .ok_or(DesktopError::UserProgramEventSessionNotFound)?
            .program_id
            .clone()
    };
    let installed = load_installed_program(&state.program_store, &program_id)?;
    ensure_program_permissions(&state.program_permissions, &installed.manifest)?;
    let concurrency = installed.manifest.event_concurrency;
    let queue_capacity = installed.manifest.event_queue_capacity;
    {
        let mut sessions = state
            .user_program_event_sessions
            .lock()
            .map_err(|_| DesktopError::StatePoisoned)?;
        let session = sessions
            .get_mut(&subscription_id)
            .ok_or(DesktopError::UserProgramEventSessionNotFound)?;
        if session.automatic {
            return Ok(());
        }
        session.automatic = true;
        session.last_error = None;
    }
    std::thread::Builder::new()
        .name(format!("nimora-event-{subscription_id}"))
        .spawn(move || {
            run_user_program_event_loop(&app, subscription_id, concurrency, queue_capacity);
        })?;
    Ok(())
}

fn run_user_program_event_loop(
    app: &AppHandle,
    subscription_id: Uuid,
    concurrency: EventConcurrencyPolicy,
    queue_capacity: usize,
) {
    let mut scheduler = EventTriggerScheduler::new(concurrency, queue_capacity);
    let mut cancellations = HashMap::<Uuid, ExecutionCancellation>::new();
    let (completion_sender, completion_receiver) = mpsc::channel::<UserProgramEventCompletion>();
    let mut reported_scheduler_drops = 0_u64;
    loop {
        let state = app.state::<DesktopState>();
        if !event_session_is_active(&state, subscription_id) || ensure_normal_mode(&state).is_err()
        {
            break;
        }
        let mut progressed = match process_event_completions(
            app,
            &state,
            subscription_id,
            &completion_sender,
            &completion_receiver,
            &mut scheduler,
            &mut cancellations,
        ) {
            Ok(progressed) => progressed,
            Err(error) => {
                stop_event_session_with_error(&state, subscription_id, error);
                cancel_scheduled_event_executions(&mut scheduler, &mut cancellations);
                return;
            }
        };
        let batch = {
            let Ok(sessions) = state.user_program_event_sessions.lock() else {
                break;
            };
            let Some(session) = sessions.get(&subscription_id) else {
                break;
            };
            match session.subscription.pop() {
                Ok(batch) => batch,
                Err(error) => {
                    drop(sessions);
                    stop_event_session_with_error(&state, subscription_id, error.to_string());
                    break;
                }
            }
        };
        if batch.dropped > 0 {
            update_event_session_drops(&state, subscription_id, batch.dropped);
        }
        if let Some(event) = batch.events.into_iter().next() {
            progressed = true;
            match scheduler.admit(event) {
                EventAdmission::Start(next) => {
                    if let Err(error) = spawn_scheduled_event_execution(
                        app,
                        subscription_id,
                        next,
                        &completion_sender,
                        &mut cancellations,
                    ) {
                        stop_event_session_with_error(&state, subscription_id, error.to_string());
                        break;
                    }
                }
                EventAdmission::CancelAndStart {
                    cancelled_execution_id,
                    next,
                } => {
                    if let Some(cancellation) = cancellations.get(&cancelled_execution_id) {
                        cancellation.cancel();
                    }
                    if let Err(error) = spawn_scheduled_event_execution(
                        app,
                        subscription_id,
                        next,
                        &completion_sender,
                        &mut cancellations,
                    ) {
                        stop_event_session_with_error(&state, subscription_id, error.to_string());
                        break;
                    }
                }
                EventAdmission::Queued | EventAdmission::Dropped => {}
            }
        }
        let scheduler_drops = scheduler.dropped();
        if scheduler_drops > reported_scheduler_drops {
            update_event_session_drops(
                &state,
                subscription_id,
                scheduler_drops - reported_scheduler_drops,
            );
            reported_scheduler_drops = scheduler_drops;
        }
        if !progressed {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    cancel_scheduled_event_executions(&mut scheduler, &mut cancellations);
}

fn event_session_is_active(state: &DesktopState, subscription_id: Uuid) -> bool {
    state
        .user_program_event_sessions
        .lock()
        .ok()
        .and_then(|sessions| {
            sessions
                .get(&subscription_id)
                .map(|session| session.automatic)
        })
        .unwrap_or(false)
}

fn process_event_completions(
    app: &AppHandle,
    state: &DesktopState,
    subscription_id: Uuid,
    completion_sender: &mpsc::Sender<UserProgramEventCompletion>,
    completion_receiver: &mpsc::Receiver<UserProgramEventCompletion>,
    scheduler: &mut EventTriggerScheduler<Event>,
    cancellations: &mut HashMap<Uuid, ExecutionCancellation>,
) -> Result<bool, String> {
    let mut progressed = false;
    while let Ok(completion) = completion_receiver.try_recv() {
        progressed = true;
        cancellations.remove(&completion.scheduled_execution_id);
        if !scheduler.is_active(completion.scheduled_execution_id) {
            continue;
        }
        let next = scheduler.finish(completion.scheduled_execution_id);
        match completion.result {
            Ok(_) => update_event_session_success(state, subscription_id),
            Err(error) => return Err(error),
        }
        if let Some(next) = next {
            spawn_scheduled_event_execution(
                app,
                subscription_id,
                next,
                completion_sender,
                cancellations,
            )
            .map_err(|error| error.to_string())?;
        }
    }
    Ok(progressed)
}

fn spawn_scheduled_event_execution(
    app: &AppHandle,
    subscription_id: Uuid,
    scheduled: ScheduledEvent<Event>,
    completion_sender: &mpsc::Sender<UserProgramEventCompletion>,
    cancellations: &mut HashMap<Uuid, ExecutionCancellation>,
) -> Result<(), DesktopError> {
    let cancellation = ExecutionCancellation::default();
    cancellations.insert(scheduled.execution_id, cancellation.clone());
    let app = app.clone();
    let sender = completion_sender.clone();
    std::thread::Builder::new()
        .name(format!("nimora-event-worker-{}", scheduled.execution_id))
        .spawn(move || {
            let state = app.state::<DesktopState>();
            let result = (|| {
                ensure_normal_mode(&state)?;
                let program_id = state
                    .user_program_event_sessions
                    .lock()
                    .map_err(|_| DesktopError::StatePoisoned)?
                    .get(&subscription_id)
                    .ok_or(DesktopError::UserProgramEventSessionNotFound)?
                    .program_id
                    .clone();
                let installed = load_installed_program(&state.program_store, &program_id)?;
                ensure_program_permissions(&state.program_permissions, &installed.manifest)?;
                execute_user_program_source_with_cancellation(
                    &app,
                    &state,
                    installed.manifest,
                    installed.source,
                    Some(scheduled.event),
                    cancellation,
                )
            })()
            .map_err(|error: DesktopError| error.to_string());
            let _ = sender.send(UserProgramEventCompletion {
                scheduled_execution_id: scheduled.execution_id,
                result,
            });
        })?;
    Ok(())
}

fn cancel_scheduled_event_executions(
    scheduler: &mut EventTriggerScheduler<Event>,
    cancellations: &mut HashMap<Uuid, ExecutionCancellation>,
) {
    if let Some(execution_id) = scheduler.cancel_all()
        && let Some(cancellation) = cancellations.get(&execution_id)
    {
        cancellation.cancel();
    }
    for cancellation in cancellations.values() {
        cancellation.cancel();
    }
    cancellations.clear();
}

fn update_event_session_success(state: &DesktopState, subscription_id: Uuid) {
    if let Ok(mut sessions) = state.user_program_event_sessions.lock()
        && let Some(session) = sessions.get_mut(&subscription_id)
    {
        session.executed = session.executed.saturating_add(1);
        session.last_error = None;
    }
}

fn update_event_session_drops(state: &DesktopState, subscription_id: Uuid, dropped: u64) {
    if let Ok(mut sessions) = state.user_program_event_sessions.lock()
        && let Some(session) = sessions.get_mut(&subscription_id)
    {
        session.dropped = session.dropped.saturating_add(dropped);
    }
}

fn stop_event_session_with_error(state: &DesktopState, subscription_id: Uuid, error: String) {
    if let Ok(mut sessions) = state.user_program_event_sessions.lock()
        && let Some(session) = sessions.get_mut(&subscription_id)
    {
        session.last_error = Some(error);
        session.automatic = false;
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn user_program_event_session_status(
    state: State<'_, DesktopState>,
    subscription_id: Uuid,
) -> Result<UserProgramEventSessionStatus, DesktopError> {
    let sessions = state
        .user_program_event_sessions
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?;
    let session = sessions
        .get(&subscription_id)
        .ok_or(DesktopError::UserProgramEventSessionNotFound)?;
    Ok(UserProgramEventSessionStatus {
        subscription_id,
        program_id: session.program_id.clone(),
        automatic: session.automatic,
        executed: session.executed,
        dropped: session.dropped,
        last_error: session.last_error.clone(),
    })
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn close_user_program_event_session(
    state: State<'_, DesktopState>,
    subscription_id: Uuid,
) -> Result<(), DesktopError> {
    let session = state
        .user_program_event_sessions
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?
        .remove(&subscription_id)
        .ok_or(DesktopError::UserProgramEventSessionNotFound)?;
    session.subscription.cancel()?;
    Ok(())
}

fn permission_status(
    repository: &SqliteProgramPermissionRepository,
    manifest: ProgramManifest,
) -> Result<UserProgramPermissionStatus, DesktopError> {
    let grant = permission_grant(&manifest);
    let granted = repository.is_granted(&grant)?;
    Ok(UserProgramPermissionStatus {
        program_id: manifest.id,
        version: manifest.version,
        capabilities: manifest.capabilities,
        granted,
    })
}

fn permission_grant(manifest: &ProgramManifest) -> ProgramPermissionGrant {
    let capabilities = manifest
        .capabilities
        .iter()
        .map(|capability| match capability {
            Capability::ReadPetState => "read-pet-state",
            Capability::ReadProfileState => "read-profile-state",
            Capability::SubscribeEvents => "subscribe-events",
            Capability::InvokeSafeCommands => "invoke-safe-commands",
            Capability::StoreLocalData => "store-local-data",
        })
        .map(ToOwned::to_owned)
        .collect();
    ProgramPermissionGrant {
        program_id: manifest.id.clone(),
        version: manifest.version.clone(),
        capabilities,
    }
}

fn ensure_program_permissions(
    repository: &SqliteProgramPermissionRepository,
    manifest: &ProgramManifest,
) -> Result<(), DesktopError> {
    if repository.is_granted(&permission_grant(manifest))? {
        Ok(())
    } else {
        Err(DesktopError::UserProgramPermissionRequired)
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn validate_user_program(
    state: State<'_, DesktopState>,
    manifest: ProgramManifest,
) -> Result<ProgramPolicyReport, DesktopError> {
    ensure_normal_mode(&state)?;
    let policy = evaluate(manifest)?;
    let granted_capabilities = policy.manifest.capabilities.clone();
    Ok(ProgramPolicyReport {
        program_id: policy.manifest.id,
        granted_capabilities,
        timeout_ms: policy.manifest.timeout_ms,
        memory_bytes: policy.manifest.memory_bytes,
    })
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn start_user_program(
    state: State<'_, DesktopState>,
    manifest: ProgramManifest,
) -> Result<UserProgramSessionReceipt, DesktopError> {
    ensure_normal_mode(&state)?;
    let policy = evaluate(manifest)?;
    let execution = state.execution_controller.admit(&policy)?;
    let execution_id = execution.execution_id();
    let receipt = UserProgramSessionReceipt {
        execution_id,
        program_id: policy.manifest.id.clone(),
        timeout_ms: policy.manifest.timeout_ms,
        memory_bytes: policy.manifest.memory_bytes,
    };
    state
        .user_programs
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?
        .insert(execution_id, UserProgramSession { policy, execution });
    Ok(receipt)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn execute_user_program(
    app: AppHandle,
    state: State<'_, DesktopState>,
    manifest: ProgramManifest,
    source: String,
) -> Result<UserProgramExecutionReceipt, DesktopError> {
    ensure_normal_mode(&state)?;
    execute_user_program_source(&app, &state, manifest, source, None)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn execute_installed_user_program(
    app: AppHandle,
    state: State<'_, DesktopState>,
    program_id: String,
) -> Result<UserProgramExecutionReceipt, DesktopError> {
    execute_installed_user_program_inner(&app, &state, &program_id, None)
}

fn execute_installed_user_program_inner(
    app: &AppHandle,
    state: &DesktopState,
    program_id: &str,
    expected_version: Option<&str>,
) -> Result<UserProgramExecutionReceipt, DesktopError> {
    ensure_normal_mode(state)?;
    let installed = load_installed_program(&state.program_store, program_id)?;
    if expected_version.is_some_and(|version| version != installed.manifest.version) {
        return Err(DesktopError::UserProgramVersionChanged);
    }
    ensure_program_permissions(&state.program_permissions, &installed.manifest)?;
    execute_user_program_source(app, state, installed.manifest, installed.source, None)
}

fn execute_user_program_source(
    app: &AppHandle,
    state: &DesktopState,
    manifest: ProgramManifest,
    source: String,
    event: Option<Event>,
) -> Result<UserProgramExecutionReceipt, DesktopError> {
    execute_user_program_source_with_cancellation(
        app,
        state,
        manifest,
        source,
        event,
        ExecutionCancellation::default(),
    )
}

fn execute_user_program_source_with_cancellation(
    app: &AppHandle,
    state: &DesktopState,
    manifest: ProgramManifest,
    source: String,
    event: Option<Event>,
    cancellation: ExecutionCancellation,
) -> Result<UserProgramExecutionReceipt, DesktopError> {
    let policy = evaluate(manifest.clone())?;
    let execution = state
        .execution_controller
        .admit_with_cancellation(&policy, cancellation)?;
    let execution_id = execution.execution_id();
    state
        .active_user_program_workers
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?
        .insert(
            execution_id,
            ActiveUserProgramWorker {
                program_id: manifest.id.clone(),
                cancellation: execution.cancellation(),
            },
        );
    let _worker_guard = ActiveUserProgramWorkerGuard {
        workers: &state.active_user_program_workers,
        execution_id,
    };
    let input = authorized_user_program_input(state, &policy, event)?;
    let request = WorkerMessage::Run {
        manifest: serde_json::to_value(manifest)?,
        source,
        input,
    };
    let mut worker = WorkerProcess::spawn(worker_config(app, &execution), &request)
        .map_err(|error| DesktopError::UserCodeHost(error.to_string()))?;
    let response = worker
        .wait()
        .map_err(|error| DesktopError::UserCodeHost(error.to_string()))?;
    let value = match response {
        WorkerMessage::Result { value } => value,
        WorkerMessage::Error { code, message } => {
            return Err(DesktopError::UserCodeHost(format!("{code}: {message}")));
        }
        _ => {
            return Err(DesktopError::UserCodeHost(
                "worker returned a non-terminal response".to_owned(),
            ));
        }
    };
    let plan = parse_user_program_plan(value)?;
    let gateway = CapabilityGateway::new(DesktopCapabilityBackend { state });
    let mut responses = Vec::with_capacity(plan.storage.len() + plan.commands.len());
    for operation in plan.storage {
        execution.checkpoint()?;
        let request = match operation {
            UserProgramStorageOperation::Read { key } => {
                nimora_user_code_gateway::CapabilityRequest::ReadLocalData { key }
            }
            UserProgramStorageOperation::Write { key, value } => {
                nimora_user_code_gateway::CapabilityRequest::WriteLocalData { key, value }
            }
            UserProgramStorageOperation::Delete { key } => {
                nimora_user_code_gateway::CapabilityRequest::DeleteLocalData { key }
            }
        };
        responses.push(gateway.dispatch(
            &policy,
            &execution,
            GatewayEnvelope {
                execution_id: execution_id.to_string(),
                trace_id: Uuid::now_v7().to_string(),
                idempotency_key: None,
                request,
            },
        )?);
    }
    for (index, command) in plan.commands.into_iter().enumerate() {
        execution.checkpoint()?;
        responses.push(
            gateway.dispatch(
                &policy,
                &execution,
                GatewayEnvelope {
                    execution_id: execution_id.to_string(),
                    trace_id: Uuid::now_v7().to_string(),
                    idempotency_key: command
                        .idempotency_key
                        .or_else(|| Some(format!("{execution_id}-{index}"))),
                    request: nimora_user_code_gateway::CapabilityRequest::InvokeCommand {
                        command: command.command,
                        arguments: command.arguments,
                    },
                },
            )?,
        );
    }
    Ok(UserProgramExecutionReceipt {
        execution_id,
        responses,
    })
}

fn parse_user_program_plan(value: serde_json::Value) -> Result<UserProgramPlan, DesktopError> {
    let plan = serde_json::from_value::<UserProgramPlan>(value)
        .map_err(|error| DesktopError::UserCodeHost(format!("invalid capability plan: {error}")))?;
    if plan.storage.len() + plan.commands.len() > MAX_USER_PROGRAM_OPERATIONS {
        return Err(DesktopError::UserCodeHost(format!(
            "capability plan exceeds the {MAX_USER_PROGRAM_OPERATIONS}-operation limit"
        )));
    }
    Ok(plan)
}

fn user_program_input(
    policy: &ExecutionPolicy,
    pet: Option<serde_json::Value>,
    profile: Option<serde_json::Value>,
    event: Option<Event>,
) -> serde_json::Value {
    let mut input =
        serde_json::Map::from_iter([("schemaVersion".to_owned(), serde_json::Value::from(1))]);
    if policy.can_read_pet_state
        && let Some(pet) = pet
    {
        input.insert("pet".to_owned(), pet);
    }
    if policy.can_read_profile_state
        && let Some(profile) = profile
    {
        input.insert("profile".to_owned(), profile);
    }
    if let Some(event) = event {
        input.insert(
            "trigger".to_owned(),
            serde_json::json!({ "type": "event", "event": event }),
        );
    }
    serde_json::Value::Object(input)
}

fn authorized_user_program_input(
    state: &DesktopState,
    policy: &ExecutionPolicy,
    event: Option<Event>,
) -> Result<serde_json::Value, DesktopError> {
    let pet = if policy.can_read_pet_state {
        Some(serde_json::to_value(state.runtime.snapshot()?)?)
    } else {
        None
    };
    let profile = if policy.can_read_profile_state {
        Some(serde_json::to_value(state.profiles.snapshot()?)?)
    } else {
        None
    };
    Ok(user_program_input(policy, pet, profile, event))
}

fn worker_config(app: &AppHandle, execution: &ExecutionHandle) -> WorkerConfig {
    let executable = option_env!("NIMORA_USER_CODE_WORKER_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let executable_candidates = app
                .path()
                .executable_dir()
                .ok()
                .into_iter()
                .map(|directory| directory.join("nimora-user-code-worker"));
            let resource_candidates =
                app.path()
                    .resource_dir()
                    .ok()
                    .into_iter()
                    .flat_map(|directory| {
                        [
                            directory.join("binaries/nimora-user-code-worker"),
                            directory.join("nimora-user-code-worker"),
                        ]
                    });
            executable_candidates
                .chain(resource_candidates)
                .into_iter()
                .find(|path| path.is_file())
                .or_else(|| {
                    std::env::current_exe()
                        .ok()
                        .and_then(|path| path.parent().map(Path::to_path_buf))
                        .map(|directory| directory.join("nimora-user-code-worker"))
                })
                .unwrap_or_else(|| PathBuf::from("nimora-user-code-worker"))
        });
    WorkerConfig {
        executable: executable.to_string_lossy().into_owned(),
        args: Vec::new(),
        execution_id: execution.execution_id().to_string(),
        timeout: execution.limits.timeout,
        output_bytes: execution.limits.output_bytes,
        cancellation: Some(execution.cancellation()),
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn invoke_user_program_capability(
    state: State<'_, DesktopState>,
    envelope: GatewayEnvelope,
) -> Result<CapabilityResponse, DesktopError> {
    ensure_normal_mode(&state)?;
    let execution_id = envelope
        .execution_id
        .parse::<Uuid>()
        .map_err(|_| DesktopError::UserProgramNotFound)?;
    let sessions = state
        .user_programs
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?;
    let session = sessions
        .get(&execution_id)
        .ok_or(DesktopError::UserProgramNotFound)?;
    Ok(
        CapabilityGateway::new(DesktopCapabilityBackend { state: &state }).dispatch(
            &session.policy,
            &session.execution,
            envelope,
        )?,
    )
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn stop_user_program(
    state: State<'_, DesktopState>,
    execution_id: Uuid,
) -> Result<(), DesktopError> {
    if let Some(session) = state
        .user_programs
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?
        .remove(&execution_id)
    {
        session.execution.cancel();
        return Ok(());
    }
    let workers = state
        .active_user_program_workers
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?;
    let worker = workers
        .get(&execution_id)
        .ok_or(DesktopError::UserProgramNotFound)?;
    worker.cancellation.cancel();
    Ok(())
}

fn cancel_all_user_programs(state: &DesktopState) -> Result<(), DesktopError> {
    let mut sessions = state
        .user_programs
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?;
    for session in sessions.values() {
        session.execution.cancel();
    }
    sessions.clear();
    let mut workers = state
        .active_user_program_workers
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?;
    for worker in workers.values() {
        worker.cancellation.cancel();
    }
    workers.clear();
    Ok(())
}

fn cancel_user_program_workers(state: &DesktopState, program_id: &str) -> Result<(), DesktopError> {
    let mut workers = state
        .active_user_program_workers
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?;
    workers.retain(|_, worker| {
        if worker.program_id == program_id {
            worker.cancellation.cancel();
            false
        } else {
            true
        }
    });
    Ok(())
}

fn cancel_all_user_program_event_sessions(state: &DesktopState) -> Result<(), DesktopError> {
    state
        .user_program_event_sessions
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?
        .clear();
    Ok(())
}

fn cancel_user_program_event_sessions(
    state: &DesktopState,
    program_id: &str,
) -> Result<(), DesktopError> {
    state
        .user_program_event_sessions
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?
        .retain(|_, session| session.program_id != program_id);
    Ok(())
}

#[derive(Debug)]
struct DesktopCapabilityBackend<'a> {
    state: &'a DesktopState,
}

impl CapabilityBackend for DesktopCapabilityBackend<'_> {
    fn read_pet_state(&self) -> Result<serde_json::Value, String> {
        serde_json::to_value(
            self.state
                .runtime
                .snapshot()
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())
    }

    fn read_pet_action_catalog(&self) -> Result<serde_json::Value, String> {
        let actions = PetAction::ALL
            .into_iter()
            .map(|action| serde_json::to_value(action).map_err(|error| error.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(serde_json::json!({
            "spec": "nimora.pet-action-catalog/1",
            "actions": actions,
            "commandTool": "pet.animation.play",
            "argument": "action"
        }))
    }

    fn read_profile_state(&self) -> Result<serde_json::Value, String> {
        serde_json::to_value(
            self.state
                .profiles
                .snapshot()
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())
    }

    fn read_character_state(&self) -> Result<serde_json::Value, String> {
        let mode = self
            .state
            .safety
            .snapshot()
            .map_err(|error| error.to_string())?
            .mode;
        let active = resolve_active_character(&self.state.asset_store, mode)
            .map_err(|error| error.to_string())?;
        let renderer = resolve_character_renderer(&self.state.asset_store, mode)
            .map_err(|error| error.to_string())?;
        Ok(serde_json::json!({
            "spec": "nimora.character-state/1",
            "active": active,
            "renderer": {
                "assetId": renderer.asset_id,
                "backend": renderer.backend,
                "canvas": renderer.canvas,
                "anchor": renderer.anchor,
                "defaultScale": renderer.default_scale,
                "pixelArt": renderer.pixel_art,
                "fallbacks": renderer.fallbacks,
                "hasSpriteClips": renderer.clips.is_some(),
                "hasModel": renderer.model.is_some()
            }
        }))
    }

    fn read_asset_catalog(&self) -> Result<serde_json::Value, String> {
        serde_json::to_value(
            inspect_asset_catalog(&self.state.asset_store).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())
    }

    fn read_program_catalog(&self) -> Result<serde_json::Value, String> {
        let entries = fs::read_dir(&self.state.program_store).map_err(|error| error.to_string())?;
        let mut programs = Vec::new();
        let mut rejected = 0_u64;
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            let Some(program_id) = entry.file_name().to_str().map(ToOwned::to_owned) else {
                rejected = rejected.saturating_add(1);
                continue;
            };
            let Ok(installed) = load_installed_program(&self.state.program_store, &program_id)
            else {
                rejected = rejected.saturating_add(1);
                continue;
            };
            let permission_granted = self
                .state
                .program_permissions
                .is_granted(&permission_grant(&installed.manifest))
                .map_err(|error| error.to_string())?;
            programs.push(serde_json::json!({
                "programId": installed.manifest.id,
                "version": installed.manifest.version,
                "capabilities": installed.manifest.capabilities,
                "commands": installed.manifest.commands,
                "subscriptions": installed.manifest.subscriptions,
                "timeoutMs": installed.manifest.timeout_ms,
                "memoryBytes": installed.manifest.memory_bytes,
                "permissionGranted": permission_granted
            }));
        }
        programs
            .sort_by(|left, right| left["programId"].as_str().cmp(&right["programId"].as_str()));
        Ok(serde_json::json!({
            "spec": "nimora.program-catalog/1",
            "programs": programs,
            "rejected": rejected,
            "commandTool": "program.installed.execute",
            "arguments": ["programId", "version"]
        }))
    }

    fn read_runtime_health(&self) -> Result<serde_json::Value, String> {
        let outbox = self
            .state
            .outbox
            .snapshot()
            .map_err(|error| error.to_string())?;
        let backup = self
            .state
            .backups
            .health()
            .map_err(|error| error.to_string())?;
        let safety = self
            .state
            .safety
            .snapshot()
            .map_err(|error| error.to_string())?;
        Ok(serde_json::json!({
            "startup": self.state.startup,
            "safety": safety,
            "outbox": outbox,
            "backup": {
                "due": backup.due,
                "latest": backup.latest,
                "pendingRestore": backup.pending_restore,
                "lastError": backup.last_error.is_some()
            }
        }))
    }

    fn validate_automation(
        &self,
        definition: &serde_json::Value,
        event_type: &str,
        event_data: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let definition = serde_json::from_value::<AutomationDefinition>(definition.clone())
            .map_err(|error| error.to_string())?;
        let run = dry_run_automation(&definition, event_type.to_owned(), event_data.clone())
            .map_err(|error| error.to_string())?;
        serde_json::to_value(run).map_err(|error| error.to_string())
    }

    fn read_local_data(
        &self,
        program_id: &str,
        key: &str,
    ) -> Result<Option<serde_json::Value>, String> {
        self.state
            .program_data_store
            .read(program_id, key)
            .map_err(|error| error.to_string())
    }

    fn write_local_data(
        &self,
        program_id: &str,
        key: &str,
        value: &serde_json::Value,
    ) -> Result<(), String> {
        self.state
            .program_data_store
            .write(program_id, key, value)
            .map_err(|error| error.to_string())
    }

    fn delete_local_data(&self, program_id: &str, key: &str) -> Result<bool, String> {
        self.state
            .program_data_store
            .delete(program_id, key)
            .map_err(|error| error.to_string())
    }

    fn invoke_command(
        &self,
        command: &str,
        arguments: serde_json::Value,
        trace_id: &str,
        idempotency_key: Option<&str>,
    ) -> Result<serde_json::Value, String> {
        let mut result = match command {
            "safe.pet.animate" => {
                let action = serde_json::from_value::<PetAction>(
                    arguments
                        .get("action")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                )
                .map_err(|error| error.to_string())?;
                self.state
                    .runtime
                    .play_action(action)
                    .map_err(|error| error.to_string())
            }
            "safe.pet.move" => {
                let position = serde_json::from_value::<Position>(arguments)
                    .map_err(|error| error.to_string())?;
                self.state
                    .runtime
                    .move_pet(position)
                    .map_err(|error| error.to_string())
            }
            "safe.profile.switch" => {
                let profile_id = serde_json::from_value::<ProfileId>(
                    arguments
                        .get("profileId")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                )
                .map_err(|error| error.to_string())?;
                let app = self
                    .state
                    .native_app
                    .as_ref()
                    .ok_or_else(|| "native desktop context is unavailable".to_owned())?;
                switch_profile_inner(app, self.state, profile_id).map_err(|error| error.to_string())
            }
            "safe.character.switch" => {
                let asset_id = arguments
                    .get("assetId")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| "assetId must be a string".to_owned())?;
                let app = self
                    .state
                    .native_app
                    .as_ref()
                    .ok_or_else(|| "native desktop context is unavailable".to_owned())?;
                let snapshot = activate_character_inner(app, self.state, asset_id)
                    .map_err(|error| error.to_string())?;
                let mut command = Command::new(
                    "safe.character.switch",
                    serde_json::to_value(snapshot).map_err(|error| error.to_string())?,
                    CommandRisk::Low,
                )
                .map_err(|error| error.to_string())?;
                command.status = CommandStatus::Succeeded;
                Ok(command)
            }
            "safe.program.execute" => {
                let program_id = arguments
                    .get("programId")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| "programId must be a string".to_owned())?;
                let version = arguments
                    .get("version")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| "version must be a string".to_owned())?;
                let app = self
                    .state
                    .native_app
                    .as_ref()
                    .ok_or_else(|| "native desktop context is unavailable".to_owned())?;
                let receipt = execute_installed_user_program_inner(
                    app,
                    self.state,
                    program_id,
                    Some(version),
                )
                .map_err(|error| error.to_string())?;
                let mut command = Command::new(
                    "safe.program.execute",
                    serde_json::to_value(receipt).map_err(|error| error.to_string())?,
                    CommandRisk::Medium,
                )
                .map_err(|error| error.to_string())?;
                command.status = CommandStatus::Succeeded;
                Ok(command)
            }
            _ => return Err("command has no registered desktop backend".to_owned()),
        }?;
        result.trace_id = trace_id
            .parse::<Uuid>()
            .map_err(|error| error.to_string())?;
        result.idempotency_key = idempotency_key.map(ToOwned::to_owned);
        serde_json::to_value(result).map_err(|error| error.to_string())
    }
}

fn valid_asset_identifier(value: &str) -> bool {
    let segments = value.split('.').collect::<Vec<_>>();
    segments.len() >= 3
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

fn ensure_normal_mode(state: &DesktopState) -> Result<(), DesktopError> {
    if state.startup.mode == StartupMode::Recovery {
        return Err(DesktopError::RecoveryModeActive);
    }
    ensure_safe_mode_inactive(state)
}

fn ensure_safe_mode_inactive(state: &DesktopState) -> Result<(), DesktopError> {
    if state.safety.snapshot()?.mode == RuntimeMode::Safe {
        return Err(DesktopError::SafeModeActive);
    }
    Ok(())
}

fn active_window_policy(snapshot: &ProfileSnapshot) -> Result<WindowPolicy, DesktopError> {
    snapshot
        .profiles
        .iter()
        .find(|profile| profile.id == snapshot.active_profile_id)
        .map(|profile| WindowPolicy::from_profile(&profile.policy))
        .ok_or(ProfileServiceError::ActiveProfileMissing.into())
}

fn current_window_policy(state: &DesktopState) -> Result<WindowPolicy, DesktopError> {
    state
        .window_policy
        .lock()
        .map(|policy| *policy)
        .map_err(|_| DesktopError::StatePoisoned)
}

fn set_current_window_policy(
    state: &DesktopState,
    policy: WindowPolicy,
) -> Result<(), DesktopError> {
    *state
        .window_policy
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)? = policy;
    Ok(())
}

fn apply_window_policy(
    app: &AppHandle,
    previous: WindowPolicy,
    next: WindowPolicy,
) -> Result<(), DesktopError> {
    let window = app
        .get_webview_window(PET_WINDOW_LABEL)
        .ok_or_else(|| DesktopError::WindowUnavailable(PET_WINDOW_LABEL.to_owned()))?;
    window.set_always_on_top(next.always_on_top)?;
    if let Err(error) = window.set_ignore_cursor_events(next.click_through) {
        let _ = window.set_always_on_top(previous.always_on_top);
        let _ = window.set_ignore_cursor_events(previous.click_through);
        return Err(error.into());
    }
    Ok(())
}

fn publish_desktop_action(
    state: &DesktopState,
    command_id: &'static str,
    event_type: &'static str,
    data: serde_json::Value,
) -> Result<Command, DesktopError> {
    let command =
        Command::new(command_id, data.clone(), CommandRisk::Safe).map_err(RuntimeError::from)?;
    state.events.publish(
        Event::with_trace_id(event_type, EventSource::Core, command.trace_id, data)
            .map_err(RuntimeError::from)?,
    )?;
    Ok(command)
}

fn publish_tray_failure(app: &AppHandle, action: TrayAction, error: &DesktopError) {
    let Some(state) = app.try_state::<DesktopState>() else {
        return;
    };
    let _ = state.events.publish(
        Event::new(
            "desktop.tray.action-failed",
            EventSource::System("desktop".to_owned()),
            serde_json::json!({
                "action": format!("{action:?}"),
                "error": error.to_string(),
            }),
        )
        .unwrap_or_else(|event_error| {
            unreachable!("static tray failure event contract is invalid: {event_error}")
        }),
    );
}

fn show_control_center(app: &AppHandle) -> Result<Command, DesktopError> {
    let window = app
        .get_webview_window(CONTROL_CENTER_LABEL)
        .ok_or_else(|| DesktopError::WindowUnavailable(CONTROL_CENTER_LABEL.to_owned()))?;
    window.show()?;
    window.unminimize()?;
    window.set_focus()?;
    publish_desktop_action(
        &app.state::<DesktopState>(),
        "desktop.window.control-center.open",
        "desktop.window.control-center-opened",
        serde_json::json!({ "source": "tray" }),
    )
}

fn restore_pet_interaction(app: &AppHandle) -> Result<Command, DesktopError> {
    let state = app.state::<DesktopState>();
    let previous = current_window_policy(&state)?;
    let window = app
        .get_webview_window(PET_WINDOW_LABEL)
        .ok_or_else(|| DesktopError::WindowUnavailable(PET_WINDOW_LABEL.to_owned()))?;
    window.show()?;
    window.unminimize()?;
    window.set_ignore_cursor_events(false)?;
    let next = WindowPolicy {
        click_through: false,
        ..previous
    };
    set_current_window_policy(&state, next)?;
    publish_desktop_action(
        &state,
        "pet.window.interaction.restore",
        "pet.window.interaction-restored",
        serde_json::json!({
            "previousClickThrough": previous.click_through,
            "clickThrough": false,
            "source": "tray",
        }),
    )
}

fn persist_pet_window_position(app: &AppHandle) -> Result<(), DesktopError> {
    let window = app
        .get_webview_window(PET_WINDOW_LABEL)
        .ok_or_else(|| DesktopError::WindowUnavailable(PET_WINDOW_LABEL.to_owned()))?;
    let position = window.outer_position()?;
    let next = Position {
        x: f64::from(position.x),
        y: f64::from(position.y),
    };
    let state = app.state::<DesktopState>();
    if state.dragging.load(Ordering::Acquire) {
        return Ok(());
    }
    if state.runtime.snapshot()?.position != next {
        state.runtime.move_pet(next)?;
    }
    Ok(())
}

fn schedule_position_persistence(app: AppHandle) {
    let revision = app
        .state::<DesktopState>()
        .position_revision
        .fetch_add(1, Ordering::Relaxed)
        + 1;
    tauri::async_runtime::spawn_blocking(move || {
        std::thread::sleep(POSITION_WRITE_DEBOUNCE);
        if app
            .state::<DesktopState>()
            .position_revision
            .load(Ordering::Relaxed)
            == revision
        {
            let _ = persist_pet_window_position(&app);
        }
    });
}

fn create_pet_window(app: &AppHandle) -> Result<(), DesktopError> {
    let policy = current_window_policy(&app.state::<DesktopState>())?;
    let window =
        WebviewWindowBuilder::new(app, PET_WINDOW_LABEL, WebviewUrl::App("/?view=pet".into()))
            .title("Aster")
            .inner_size(260.0, 300.0)
            .min_inner_size(180.0, 210.0)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .always_on_top(policy.always_on_top)
            .skip_taskbar(true)
            .shadow(false)
            .build()?;
    let position = app.state::<DesktopState>().runtime.snapshot()?.position;
    window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
        screen_coordinate(position.x)?,
        screen_coordinate(position.y)?,
    )))?;
    window.set_ignore_cursor_events(policy.click_through)?;
    Ok(())
}

fn create_tray(app: &AppHandle) -> Result<(), DesktopError> {
    let open = MenuItem::with_id(app, "open", "打开控制中心", true, None::<&str>)?;
    let interactive = MenuItem::with_id(app, "interactive", "恢复宠物交互", true, None::<&str>)?;
    let safe = MenuItem::with_id(app, "safe-mode", "进入安全模式", true, None::<&str>)?;
    let normal = MenuItem::with_id(app, "normal-mode", "退出安全模式", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出 Nimora", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &interactive, &safe, &normal, &quit])?;

    TrayIconBuilder::with_id("nimora-tray")
        .tooltip("Nimora · 本地运行")
        .menu(&menu)
        .on_menu_event(|app, event| {
            let action = TrayAction::from(event.id.as_ref());
            let result = match action {
                TrayAction::OpenControlCenter => show_control_center(app).map(|_| ()),
                TrayAction::RestoreInteraction => restore_pet_interaction(app).map(|_| ()),
                TrayAction::EnterSafeMode => {
                    if let Some(state) = app.try_state::<DesktopState>() {
                        enter_safe_mode(app.clone(), state).map(|_| ())
                    } else {
                        Err(DesktopError::StatePoisoned)
                    }
                }
                TrayAction::ExitSafeMode => {
                    if let Some(state) = app.try_state::<DesktopState>() {
                        exit_safe_mode(app.clone(), state).map(|_| ())
                    } else {
                        Err(DesktopError::StatePoisoned)
                    }
                }
                TrayAction::Quit => persist_pet_window_position(app),
                TrayAction::Unknown => return,
            };
            if let Err(error) = result {
                publish_tray_failure(app, action, &error);
            }
            if action == TrayAction::Quit {
                app.exit(0);
            }
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(event, TrayIconEvent::DoubleClick { .. })
                && let Err(error) = show_control_center(tray.app_handle())
            {
                publish_tray_failure(tray.app_handle(), TrayAction::OpenControlCenter, &error);
            }
        })
        .build(app)?;
    Ok(())
}

/// Starts the `Nimora` desktop application.
///
/// # Panics
///
/// Panics when the Tauri runtime cannot initialize. This is unrecoverable
/// before application state and diagnostics are available.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_uri_scheme_protocol(ASSET_PROTOCOL, |context, request| {
            let state = context.app_handle().state::<DesktopState>();
            let runtime_mode = state
                .safety
                .snapshot()
                .map_or(RuntimeMode::Safe, |snapshot| snapshot.mode);
            let response = serve_asset_protocol(
                &state.asset_store,
                runtime_mode,
                context.webview_label(),
                request.method(),
                request.uri(),
            );
            tauri::http::Response::builder()
                .status(response.status)
                .header(tauri::http::header::CONTENT_TYPE, response.media_type)
                .header("X-Content-Type-Options", "nosniff")
                .header(tauri::http::header::CACHE_CONTROL, "no-store")
                .body(response.body)
                .expect("static asset protocol response is valid")
        })
        .setup(setup_application)
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event
                && window.label() == CONTROL_CENTER_LABEL
            {
                api.prevent_close();
                let _ = window.hide();
            }
            if let WindowEvent::Moved(_) = event
                && window.label() == PET_WINDOW_LABEL
            {
                schedule_position_persistence(window.app_handle().clone());
            }
        })
        .invoke_handler(tauri::generate_handler![
            desktop_snapshot,
            agent_catalog,
            test_automation,
            automation_run_status,
            agent_provider_status,
            agent_history_list,
            delete_agent_history,
            run_local_agent,
            prepare_agent_tool,
            confirm_agent_tool,
            confirm_agent_run_tool,
            reject_agent_tool,
            drain_runtime_events,
            outbox_snapshot,
            backup_health,
            create_backup,
            request_database_restore,
            preview_diagnostic_report,
            export_diagnostics,
            profile_snapshot,
            create_profile,
            switch_profile,
            enter_safe_mode,
            exit_safe_mode,
            move_pet,
            play_pet_action,
            click_pet,
            begin_pet_drag,
            finish_pet_drag,
            set_click_through,
            asset_catalog,
            active_character,
            active_character_renderer,
            activate_character,
            preview_asset,
            inspect_model,
            import_model,
            export_asset,
            install_asset,
            rollback_asset,
            install_user_program,
            rollback_user_program,
            user_program_permission_status,
            grant_user_program_permissions,
            revoke_user_program_permissions,
            open_user_program_event_session,
            drain_user_program_events,
            execute_next_user_program_event,
            start_user_program_event_loop,
            user_program_event_session_status,
            close_user_program_event_session,
            validate_user_program,
            start_user_program,
            execute_user_program,
            execute_installed_user_program,
            invoke_user_program_capability,
            stop_user_program
        ])
        .run(tauri::generate_context!())
        .expect("Nimora desktop runtime failed");
}

fn open_diagnostic_journal(directory: &Path, now_ms: u64) -> PersistentDiagnosticJournal {
    PersistentDiagnosticJournal::open(directory, DiagnosticJournalPolicy::default(), now_ms)
        .unwrap_or_else(|_| PersistentDiagnosticJournal::in_memory())
}

fn setup_application(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let data_directory = app.path().app_data_dir()?;
    std::fs::create_dir_all(&data_directory)?;
    let database_path = data_directory.join("runtime.sqlite3");
    let backup_directory = data_directory.join("backups");
    let backups =
        BackupCoordinator::new(&database_path, &backup_directory, BackupPolicy::default());
    let asset_store = data_directory.join("assets");
    let program_store = data_directory.join("programs");
    let diagnostic_journal = open_diagnostic_journal(
        &data_directory.join("diagnostics/events"),
        current_time_ms()?,
    );
    let ollama_worker = discover_ollama_worker(app.handle());
    let startup_result = apply_pending_restore(&database_path, &backup_directory).and_then(|_| {
        if database_path.exists() {
            verify_database_file(&database_path)?;
        }
        Ok(())
    });
    let state = match startup_result {
        Ok(()) => DesktopState::open(
            Some(app.handle().clone()),
            &database_path,
            asset_store,
            program_store,
            backups,
            diagnostic_journal,
            ollama_worker,
        )?,
        Err(_) => DesktopState::open_recovery(
            Some(app.handle().clone()),
            asset_store,
            program_store,
            backups,
            diagnostic_journal,
            "database-unavailable",
            ollama_worker,
        )?,
    };
    let schedule_backups = state.startup.mode == StartupMode::Normal;
    app.manage(state);
    if schedule_backups {
        let backup_app = app.handle().clone();
        std::thread::spawn(move || {
            loop {
                let state = backup_app.state::<DesktopState>();
                match state.backups.create_if_due() {
                    Ok(created) => {
                        if let Ok(mut last_error) = state.backup_last_error.lock() {
                            *last_error = None;
                        }
                        if created.is_some() {
                            let _ = record_diagnostic_event(
                                &state,
                                DiagnosticSeverity::Info,
                                DiagnosticComponent::Backup,
                                DiagnosticEventCode::ScheduledBackupCompleted,
                            );
                        }
                    }
                    Err(error) => {
                        if let Ok(mut last_error) = state.backup_last_error.lock() {
                            *last_error = Some(error.to_string());
                        }
                        let _ = record_diagnostic_event(
                            &state,
                            DiagnosticSeverity::Warning,
                            DiagnosticComponent::Backup,
                            DiagnosticEventCode::ScheduledBackupFailed,
                        );
                    }
                }
                std::thread::sleep(Duration::from_mins(15));
            }
        });
    }
    create_pet_window(app.handle())?;
    create_tray(app.handle())?;
    Ok(())
}

fn discover_ollama_worker(app: &AppHandle) -> Option<PathBuf> {
    let trusted_digest = option_env!("NIMORA_OLLAMA_MANIFEST_SHA256")?;
    let configured_roots = std::env::var_os("NIMORA_OLLAMA_SIDECAR_ROOT")
        .map(PathBuf::from)
        .into_iter();
    let resource_roots = app
        .path()
        .resource_dir()
        .ok()
        .into_iter()
        .flat_map(|root| [root.join("binaries"), root]);
    let executable_roots = app.path().executable_dir().ok().into_iter();
    let development_roots = cfg!(debug_assertions)
        .then(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries"))
        .into_iter();
    configured_roots
        .chain(resource_roots)
        .chain(executable_roots)
        .chain(development_roots)
        .find_map(|root| {
            verify_provider_sidecar(&root, "ollama-provider.json", trusted_digest)
                .ok()
                .map(|verified| verified.executable_path)
        })
}

#[cfg(test)]
mod tests {
    use super::{
        ACTIVE_CHARACTER_FILE, AssetInstallReceipt, AutomationTestRequest, BUILTIN_CHARACTER_ID,
        CapabilityBackend, DesktopCapabilityBackend, DesktopError, DesktopState, LocalAgentRequest,
        PetAction, PrepareAgentToolRequest, ProfilePolicy, ResolveAgentToolRequest, StartupMode,
        TrayAction, UserProgramRollbackReceipt, WindowPolicy, agent_catalog_inner,
        cancel_all_pending_agent_tools, confirm_agent_tool_inner, confirm_agent_tool_with_registry,
        default_agent_model, default_agent_provider_id, diagnostic_report, ensure_normal_mode,
        ensure_program_permissions, inspect_asset_catalog, install_gltf_character,
        open_diagnostic_journal, parse_asset_protocol_path, parse_user_program_plan,
        permission_grant, persist_active_character, prepare_agent_tool_inner,
        reject_agent_tool_inner, resolve_active_character, resolve_character_renderer,
        run_local_agent_inner, screen_coordinate, serve_asset_protocol, test_automation,
        user_program_input, valid_asset_identifier, validate_model_source, validate_package_source,
        validate_requested_animation_map,
    };
    use nimora_agent_runtime::{
        AgentBudget, AgentTask, AgentTaskOrigin, AgentTaskStatus, DataClassification,
        ProviderAdapter, ProviderCapabilities, ProviderCapability, ProviderDescriptor,
        ProviderError, ProviderExecutionContext, ProviderFinishReason, ProviderLocality,
        ProviderMessage, ProviderMessageRole, ProviderRegistry, ProviderRequest, ProviderResponse,
        ProviderToolCall, ProviderUsage,
    };
    use nimora_asset_installer::{GltfCharacterMetadata, ModelAnimationBinding};
    use nimora_diagnostics_bundle::{
        DiagnosticComponent, DiagnosticEventCode, DiagnosticSeverity, PersistentDiagnosticJournal,
    };
    use nimora_persistence_sqlite::{
        AutomationJournalStatus, AutomationRunStart, BackupCoordinator, BackupPolicy,
        SqliteAutomationJournal, SqliteProgramPermissionRepository,
    };
    use nimora_runtime_core::{Event, EventSource, Position, RuntimeMode};
    use nimora_user_code_policy::{Capability, ProgramManifest, evaluate};
    use serde_json::json;
    use std::{collections::BTreeSet, path::Path};
    use uuid::Uuid;

    fn normal_desktop_state() -> (std::path::PathBuf, DesktopState) {
        let root = std::env::temp_dir().join(format!("nimora-agent-state-{}", Uuid::now_v7()));
        std::fs::create_dir_all(&root).expect("fixture directory");
        let database = root.join("runtime.sqlite3");
        let state = DesktopState::open(
            None,
            &database,
            root.join("assets"),
            root.join("programs"),
            BackupCoordinator::new(&database, root.join("backups"), BackupPolicy::default()),
            PersistentDiagnosticJournal::in_memory(),
            None,
        )
        .expect("normal desktop state");
        (root, state)
    }

    #[test]
    fn desktop_startup_interrupts_crash_left_automation_runs() {
        let root = std::env::temp_dir().join(format!("nimora-automation-state-{}", Uuid::now_v7()));
        std::fs::create_dir_all(&root).expect("fixture directory");
        let database = root.join("runtime.sqlite3");
        let journal = SqliteAutomationJournal::open(&database).expect("journal");
        let run = AutomationRunStart {
            run_id: Uuid::now_v7(),
            automation_id: "local.focus.summary".to_owned(),
            trace_id: Uuid::now_v7(),
            event_id: "event:before-restart".to_owned(),
            started_at_ms: 1,
        };
        journal.start(&run).expect("running entry");
        drop(journal);

        let state = DesktopState::open(
            None,
            &database,
            root.join("assets"),
            root.join("programs"),
            BackupCoordinator::new(&database, root.join("backups"), BackupPolicy::default()),
            PersistentDiagnosticJournal::in_memory(),
            None,
        )
        .expect("desktop state");
        let recovered = state
            .automation_journal
            .get(run.run_id)
            .expect("query")
            .expect("entry");
        assert_eq!(recovered.status, AutomationJournalStatus::Interrupted);
        assert_eq!(
            recovered.interruption_reason.as_deref(),
            Some("desktop process restarted")
        );
        std::fs::remove_dir_all(root).expect("fixture cleanup");
    }

    #[derive(Debug)]
    struct TwoStepDesktopProvider {
        descriptor: ProviderDescriptor,
    }

    #[derive(Debug)]
    struct MultiWriteDesktopProvider {
        descriptor: ProviderDescriptor,
    }

    impl TwoStepDesktopProvider {
        fn new() -> Self {
            Self {
                descriptor: ProviderDescriptor::new(
                    "provider:desktop-test",
                    "Desktop Test Provider",
                    ProviderLocality::Local,
                    4_096,
                    512,
                    ProviderCapabilities {
                        supported: BTreeSet::from([
                            ProviderCapability::StructuredToolCalls,
                            ProviderCapability::UsageReporting,
                        ]),
                    },
                )
                .expect("provider descriptor"),
            }
        }
    }

    impl MultiWriteDesktopProvider {
        fn new() -> Self {
            Self {
                descriptor: ProviderDescriptor::new(
                    "provider:desktop-multi-write-test",
                    "Desktop Multi Write Test Provider",
                    ProviderLocality::Local,
                    4_096,
                    512,
                    ProviderCapabilities {
                        supported: BTreeSet::from([
                            ProviderCapability::StructuredToolCalls,
                            ProviderCapability::UsageReporting,
                        ]),
                    },
                )
                .expect("provider descriptor"),
            }
        }
    }

    impl ProviderAdapter for TwoStepDesktopProvider {
        fn descriptor(&self) -> &ProviderDescriptor {
            &self.descriptor
        }

        fn complete(
            &self,
            request: &ProviderRequest,
            _context: &ProviderExecutionContext,
        ) -> Result<ProviderResponse, ProviderError> {
            let continued = request
                .messages
                .iter()
                .any(|message| message.role == ProviderMessageRole::Tool);
            Ok(ProviderResponse {
                spec: "nimora.agent-provider-response/1".to_owned(),
                request_id: request.request_id,
                content: if continued {
                    "桌宠位置已经更新".to_owned()
                } else {
                    String::new()
                },
                tool_calls: if continued {
                    Vec::new()
                } else {
                    vec![ProviderToolCall {
                        id: "desktop-call:1".to_owned(),
                        tool_id: "pet.position.move".parse().expect("tool id"),
                        arguments: json!({"x": 44, "y": 66}),
                    }]
                },
                finish_reason: if continued {
                    ProviderFinishReason::Completed
                } else {
                    ProviderFinishReason::ToolCalls
                },
                usage: ProviderUsage {
                    input_tokens: 8,
                    output_tokens: 4,
                    cost_microunits: 0,
                },
            })
        }
    }

    impl ProviderAdapter for MultiWriteDesktopProvider {
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
                tool_calls: vec![
                    ProviderToolCall {
                        id: "desktop-multi-call:1".to_owned(),
                        tool_id: "pet.position.move".parse().expect("tool id"),
                        arguments: json!({"x": 11, "y": 22}),
                    },
                    ProviderToolCall {
                        id: "desktop-multi-call:2".to_owned(),
                        tool_id: "pet.position.move".parse().expect("tool id"),
                        arguments: json!({"x": 33, "y": 44}),
                    },
                ],
                finish_reason: ProviderFinishReason::ToolCalls,
                usage: ProviderUsage {
                    input_tokens: 8,
                    output_tokens: 4,
                    cost_microunits: 0,
                },
            })
        }
    }

    #[test]
    fn desktop_agent_catalog_exposes_only_production_capabilities() {
        let (root, state) = normal_desktop_state();
        let catalog = agent_catalog_inner(&state).expect("agent catalog");
        assert_eq!(catalog.spec, "nimora.desktop-agent-catalog/1");
        assert_eq!(catalog.providers.len(), 1);
        assert_eq!(catalog.providers[0].id, "provider:deterministic-local");
        let tool_ids = catalog
            .tools
            .iter()
            .map(|tool| tool.id.to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            tool_ids,
            [
                "asset.catalog.read".to_owned(),
                "automation.definition.validate".to_owned(),
                "character.active.switch".to_owned(),
                "character.state.read".to_owned(),
                "pet.action.catalog.read".to_owned(),
                "pet.animation.play".to_owned(),
                "pet.position.move".to_owned(),
                "pet.state.read".to_owned(),
                "profile.active.switch".to_owned(),
                "profile.state.read".to_owned(),
                "program.catalog.read".to_owned(),
                "program.installed.execute".to_owned(),
                "runtime.health.read".to_owned(),
            ]
        );
        std::fs::remove_dir_all(root).expect("fixture cleanup");
    }

    #[test]
    fn automation_test_run_returns_a_side_effect_free_plan() {
        let request = serde_json::from_value::<AutomationTestRequest>(json!({
            "definition": {
                "spec": "nimora.automation/1",
                "id": "local.focus.on-build",
                "name": "Build companion",
                "enabled": true,
                "trigger": { "eventType": "dev.build.finished" },
                "conditions": [{ "pointer": "/succeeded", "equals": true }],
                "actions": [{
                    "id": "celebrate",
                    "command": "pet.animation.play",
                    "arguments": { "action": "celebrate" },
                    "risk": "low",
                    "retrySafe": true,
                    "idempotencyKey": "preview-build-celebrate",
                    "compensation": null
                }],
                "policy": { "timeoutMs": 5000, "failure": "stop" }
            },
            "eventType": "dev.build.finished",
            "eventData": { "succeeded": true }
        }))
        .expect("automation request");
        let run = test_automation(request).expect("automation test run");
        assert_eq!(
            run.status,
            nimora_automation_runtime::AutomationRunStatus::Planned
        );
        assert_eq!(run.steps.len(), 1);
        assert_eq!(run.steps[0].command, "pet.animation.play");
        assert_eq!(run.steps[0].attempts, 0);
    }

    #[test]
    fn desktop_agent_validates_automation_without_confirmation_or_side_effects() {
        let (root, state) = normal_desktop_state();
        let prepared = prepare_agent_tool_inner(
            PrepareAgentToolRequest {
                tool_id: "automation.definition.validate".to_owned(),
                arguments: json!({
                    "definition": {
                        "spec": "nimora.automation/1",
                        "id": "local.focus.on-build",
                        "name": "Build companion",
                        "enabled": true,
                        "trigger": { "eventType": "dev.build.finished" },
                        "conditions": [{ "pointer": "/succeeded", "equals": true }],
                        "actions": [{
                            "id": "celebrate",
                            "command": "pet.animation.play",
                            "arguments": { "action": "celebrate" },
                            "risk": "low",
                            "retrySafe": true,
                            "idempotencyKey": "agent-build-celebrate",
                            "compensation": null
                        }],
                        "policy": { "timeoutMs": 5000, "failure": "stop" }
                    },
                    "eventType": "dev.build.finished",
                    "eventData": { "succeeded": true }
                }),
            },
            &state,
        )
        .expect("automation validation");
        assert!(!prepared.requires_confirmation);
        assert_eq!(
            prepared.output.as_ref().expect("output")["status"],
            "planned"
        );
        assert_eq!(
            state.runtime.snapshot().expect("snapshot").state,
            nimora_runtime_core::PetState::Idle
        );
        assert!(
            state
                .pending_agent_tools
                .lock()
                .expect("pending tools")
                .is_empty()
        );
        std::fs::remove_dir_all(root).expect("fixture cleanup");
    }

    #[test]
    fn character_state_capability_is_path_free() {
        let (root, state) = normal_desktop_state();
        let value = DesktopCapabilityBackend { state: &state }
            .read_character_state()
            .expect("character state");
        assert_eq!(value["spec"], "nimora.character-state/1");
        assert_eq!(value["active"]["assetId"], BUILTIN_CHARACTER_ID);
        assert_eq!(value["renderer"]["backend"], "built-in");
        let serialized = value.to_string();
        assert!(!serialized.contains(root.to_string_lossy().as_ref()));
        assert!(!serialized.contains("assetBaseUrl"));
        assert!(!serialized.contains("model"));
        std::fs::remove_dir_all(root).expect("fixture cleanup");
    }

    #[test]
    fn pet_action_catalog_matches_runtime_vocabulary() {
        let (_root, state) = normal_desktop_state();
        let value = DesktopCapabilityBackend { state: &state }
            .read_pet_action_catalog()
            .expect("pet action catalog");
        assert_eq!(value["spec"], "nimora.pet-action-catalog/1");
        assert_eq!(
            value["actions"],
            json!(["idle", "walk", "sleep", "work", "celebrate"])
        );
        assert_eq!(value["commandTool"], "pet.animation.play");
    }

    #[test]
    fn program_catalog_rejects_corrupt_entries_without_exposing_paths() {
        let (root, state) = normal_desktop_state();
        std::fs::create_dir_all(state.program_store.join("corrupt-entry"))
            .expect("corrupt program fixture");
        let value = DesktopCapabilityBackend { state: &state }
            .read_program_catalog()
            .expect("program catalog");
        assert_eq!(value["spec"], "nimora.program-catalog/1");
        assert_eq!(value["programs"], json!([]));
        assert_eq!(value["rejected"], 1);
        assert_eq!(value["commandTool"], "program.installed.execute");
        assert_eq!(value["arguments"], json!(["programId", "version"]));
        let serialized = value.to_string();
        assert!(!serialized.contains(root.to_string_lossy().as_ref()));
        assert!(!serialized.contains("main.js"));
        assert!(!serialized.contains("activePath"));
        assert!(!serialized.contains("source"));
        std::fs::remove_dir_all(root).expect("fixture cleanup");
    }

    #[test]
    fn program_execute_backend_fails_closed_without_native_context() {
        let (root, state) = normal_desktop_state();
        let error = DesktopCapabilityBackend { state: &state }
            .invoke_command(
                "safe.program.execute",
                json!({"programId": "studio.example.focus", "version": "1.0.0"}),
                &Uuid::now_v7().to_string(),
                Some("program-execute-1"),
            )
            .expect_err("native context must be required");
        assert_eq!(error, "native desktop context is unavailable");
        assert!(
            !state.program_store.exists()
                || std::fs::read_dir(&state.program_store)
                    .expect("program store")
                    .next()
                    .is_none()
        );
        std::fs::remove_dir_all(root).expect("fixture cleanup");
    }

    #[test]
    fn profile_switch_backend_fails_closed_without_native_context() {
        let (root, state) = normal_desktop_state();
        state
            .profiles
            .create_profile("Focus", ProfilePolicy::standard())
            .expect("create profile");
        let before = state.profiles.snapshot().expect("before snapshot");
        let target = before.profiles[1].id;
        let error = DesktopCapabilityBackend { state: &state }
            .invoke_command(
                "safe.profile.switch",
                json!({"profileId": target}),
                &Uuid::now_v7().to_string(),
                Some("profile-switch-1"),
            )
            .expect_err("native context must be required");
        assert_eq!(error, "native desktop context is unavailable");
        assert_eq!(
            state
                .profiles
                .snapshot()
                .expect("after snapshot")
                .active_profile_id,
            before.active_profile_id
        );
        std::fs::remove_dir_all(root).expect("fixture cleanup");
    }

    #[test]
    fn character_switch_backend_fails_closed_without_native_context() {
        let (root, state) = normal_desktop_state();
        let before = resolve_active_character(&state.asset_store, RuntimeMode::Normal)
            .expect("before character");
        let error = DesktopCapabilityBackend { state: &state }
            .invoke_command(
                "safe.character.switch",
                json!({"assetId": "character.local.aurora"}),
                &Uuid::now_v7().to_string(),
                Some("character-switch-1"),
            )
            .expect_err("native context must be required");
        assert_eq!(error, "native desktop context is unavailable");
        let after = resolve_active_character(&state.asset_store, RuntimeMode::Normal)
            .expect("after character");
        assert_eq!(after.asset_id, before.asset_id);
        assert_eq!(after.fallback_reason, before.fallback_reason);
        assert!(!state.asset_store.join(ACTIVE_CHARACTER_FILE).exists());
        std::fs::remove_dir_all(root).expect("fixture cleanup");
    }

    #[test]
    fn desktop_agent_catalog_adds_configured_ollama_worker() {
        let (root, mut state) = normal_desktop_state();
        state.ollama_worker = Some(std::env::current_exe().expect("test executable"));
        let catalog = agent_catalog_inner(&state).expect("agent catalog");
        assert_eq!(catalog.providers.len(), 2);
        assert_eq!(catalog.providers[1].id, "provider:ollama-loopback");
        std::fs::remove_dir_all(root).expect("fixture cleanup");
    }

    #[test]
    fn desktop_local_agent_runs_offline_without_cost_or_side_effects() {
        let (root, state) = normal_desktop_state();
        let result = run_local_agent_inner(
            LocalAgentRequest {
                prompt: "检查本地能力".to_owned(),
                provider_id: default_agent_provider_id(),
                model: default_agent_model(),
            },
            &state,
        )
        .expect("local agent result");
        assert_eq!(result.status, super::DesktopAgentRunStatus::Completed);
        assert_eq!(result.content.as_deref(), Some("检查本地能力"));
        assert_eq!(result.task.origin, AgentTaskOrigin::Desktop);
        assert_eq!(result.task.status, AgentTaskStatus::Succeeded);
        assert_eq!(result.usage.expect("completed usage").cost_microunits, 0);
        assert!(result.pending_tools.is_empty());
        let history = state.agent_history.list(None, 10).expect("agent history");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].task.id, result.task.id);
        assert_eq!(history[0].prompt, "检查本地能力");
        assert_eq!(history[0].response, "检查本地能力");
        assert!(
            !*state
                .agent_history_last_error
                .lock()
                .expect("history state")
        );
        std::fs::remove_dir_all(root).expect("fixture cleanup");
    }

    #[test]
    fn desktop_local_agent_rejects_empty_and_oversized_prompts() {
        let (root, state) = normal_desktop_state();
        assert!(
            run_local_agent_inner(
                LocalAgentRequest {
                    prompt: "  ".to_owned(),
                    provider_id: default_agent_provider_id(),
                    model: default_agent_model(),
                },
                &state
            )
            .is_err()
        );
        assert!(
            run_local_agent_inner(
                LocalAgentRequest {
                    prompt: "有效任务".to_owned(),
                    provider_id: "provider:not-registered".to_owned(),
                    model: default_agent_model(),
                },
                &state
            )
            .is_err()
        );
        assert!(
            run_local_agent_inner(
                LocalAgentRequest {
                    prompt: "有效任务".to_owned(),
                    provider_id: default_agent_provider_id(),
                    model: " ".to_owned(),
                },
                &state
            )
            .is_err()
        );
        assert!(
            run_local_agent_inner(
                LocalAgentRequest {
                    prompt: "a".repeat(32 * 1024 + 1),
                    provider_id: default_agent_provider_id(),
                    model: default_agent_model(),
                },
                &state
            )
            .is_err()
        );
        std::fs::remove_dir_all(root).expect("fixture cleanup");
    }

    #[test]
    fn desktop_provider_tool_confirmation_resumes_provider_with_gateway_result() {
        let (root, state) = normal_desktop_state();
        let mut providers = ProviderRegistry::default();
        providers
            .register(TwoStepDesktopProvider::new())
            .expect("register provider");
        let task = AgentTask::new(
            AgentTaskOrigin::Desktop,
            "desktop:test-user",
            "provider:desktop-test",
            AgentBudget::default(),
            super::current_time_ms().expect("clock"),
        )
        .expect("task");
        let outcome = super::advance_provider_agent(
            &providers,
            &state,
            task,
            "model:desktop-test".to_owned(),
            vec![ProviderMessage::text(
                ProviderMessageRole::User,
                "把桌宠移动到右下角",
                DataClassification::Personal,
                true,
            )],
            128,
            true,
        )
        .expect("first provider step");
        let super::ProviderAgentOutcome::Waiting { pending, .. } = outcome else {
            panic!("expected confirmation");
        };
        assert_eq!(pending.len(), 1);
        assert_eq!(
            state.runtime.snapshot().expect("snapshot").position,
            Position { x: 0.0, y: 0.0 }
        );
        let request = ResolveAgentToolRequest {
            invocation_id: pending[0].invocation.invocation_id,
        };
        let (_, continuation) =
            confirm_agent_tool_with_registry(&request, &state, &providers).expect("confirm tool");
        let Some(super::ProviderAgentOutcome::Completed { task, response }) = continuation else {
            panic!("expected completed continuation");
        };
        assert_eq!(response.content, "桌宠位置已经更新");
        assert_eq!(task.status, AgentTaskStatus::Succeeded);
        assert_eq!(task.usage.steps, 2);
        assert_eq!(task.usage.tool_calls, 1);
        assert_eq!(
            state.runtime.snapshot().expect("snapshot").position,
            Position { x: 44.0, y: 66.0 }
        );
        std::fs::remove_dir_all(root).expect("fixture cleanup");
    }

    #[test]
    fn desktop_provider_rejection_cancels_approved_sibling_without_side_effects() {
        let (root, state) = normal_desktop_state();
        let mut providers = ProviderRegistry::default();
        providers
            .register(MultiWriteDesktopProvider::new())
            .expect("register provider");
        let task = AgentTask::new(
            AgentTaskOrigin::Desktop,
            "desktop:test-user",
            "provider:desktop-multi-write-test",
            AgentBudget::default(),
            super::current_time_ms().expect("clock"),
        )
        .expect("task");
        let outcome = super::advance_provider_agent(
            &providers,
            &state,
            task,
            "model:desktop-test".to_owned(),
            vec![ProviderMessage::text(
                ProviderMessageRole::User,
                "连续移动桌宠",
                DataClassification::Personal,
                true,
            )],
            128,
            true,
        )
        .expect("first provider step");
        let run_result = super::desktop_agent_run_result(outcome);
        assert_eq!(
            run_result.status,
            super::DesktopAgentRunStatus::WaitingForConfirmation
        );
        let pending = run_result.pending_tools;
        assert_eq!(pending.len(), 2);
        let first = ResolveAgentToolRequest {
            invocation_id: pending[0].invocation.invocation_id,
        };
        let second = ResolveAgentToolRequest {
            invocation_id: pending[1].invocation.invocation_id,
        };
        let (approved, continuation) =
            confirm_agent_tool_with_registry(&first, &state, &providers).expect("approve first");
        assert!(approved.output.is_none());
        assert!(continuation.is_none());
        let waiting = super::desktop_agent_confirmation_result(&state, approved, continuation)
            .expect("waiting result");
        assert_eq!(
            waiting.status,
            super::DesktopAgentRunStatus::WaitingForConfirmation
        );
        assert_eq!(waiting.pending_tools.len(), 1);
        assert_eq!(
            waiting.pending_tools[0].invocation.invocation_id,
            second.invocation_id
        );
        assert_eq!(
            state.runtime.snapshot().expect("snapshot").position,
            Position { x: 0.0, y: 0.0 }
        );
        reject_agent_tool_inner(&second, &state).expect("reject sibling");
        assert!(confirm_agent_tool_with_registry(&second, &state, &providers).is_err());
        assert!(confirm_agent_tool_with_registry(&first, &state, &providers).is_err());
        assert_eq!(
            state.runtime.snapshot().expect("snapshot").position,
            Position { x: 0.0, y: 0.0 }
        );
        std::fs::remove_dir_all(root).expect("fixture cleanup");
    }

    #[test]
    fn desktop_agent_write_requires_one_time_confirmation() {
        let (root, state) = normal_desktop_state();
        let prepared = prepare_agent_tool_inner(
            PrepareAgentToolRequest {
                tool_id: "pet.animation.play".to_owned(),
                arguments: json!({"action": "celebrate"}),
            },
            &state,
        )
        .expect("pending tool");
        assert!(prepared.requires_confirmation);
        assert!(prepared.output.is_none());
        assert_eq!(
            state.runtime.snapshot().expect("snapshot").state,
            nimora_runtime_core::PetState::Idle
        );

        let invocation_id = prepared.invocation.invocation_id;
        let request = ResolveAgentToolRequest { invocation_id };
        let completed = confirm_agent_tool_inner(&request, &state).expect("confirmed tool");
        assert!(!completed.requires_confirmation);
        assert!(completed.output.is_some());
        assert_eq!(
            state.runtime.snapshot().expect("snapshot").state,
            nimora_runtime_core::PetState::Interacting
        );
        assert!(confirm_agent_tool_inner(&request, &state).is_err());
        std::fs::remove_dir_all(root).expect("fixture cleanup");
    }

    #[test]
    fn desktop_agent_rejection_removes_pending_side_effect() {
        let (root, state) = normal_desktop_state();
        let prepared = prepare_agent_tool_inner(
            PrepareAgentToolRequest {
                tool_id: "pet.position.move".to_owned(),
                arguments: json!({"x": 18, "y": 27}),
            },
            &state,
        )
        .expect("pending tool");
        let invocation_id = prepared.invocation.invocation_id;
        let request = ResolveAgentToolRequest { invocation_id };
        reject_agent_tool_inner(&request, &state).expect("reject tool");
        assert!(confirm_agent_tool_inner(&request, &state).is_err());
        assert_eq!(
            state.runtime.snapshot().expect("snapshot").position,
            Position { x: 0.0, y: 0.0 }
        );
        std::fs::remove_dir_all(root).expect("fixture cleanup");
    }

    #[test]
    fn safe_mode_revokes_pending_agent_confirmations() {
        let (root, state) = normal_desktop_state();
        let prepared = prepare_agent_tool_inner(
            PrepareAgentToolRequest {
                tool_id: "pet.animation.play".to_owned(),
                arguments: json!({"action": "celebrate"}),
            },
            &state,
        )
        .expect("pending tool");
        state
            .safety
            .enter(nimora_runtime_core::SafeModeReason::Manual)
            .expect("safe mode");
        cancel_all_pending_agent_tools(&state).expect("cancel pending tools");
        state.safety.exit().expect("exit safe mode");
        assert!(
            confirm_agent_tool_inner(
                &ResolveAgentToolRequest {
                    invocation_id: prepared.invocation.invocation_id
                },
                &state
            )
            .is_err()
        );
        std::fs::remove_dir_all(root).expect("fixture cleanup");
    }

    #[test]
    fn recovery_state_is_isolated_and_rejects_normal_writes() {
        let root =
            std::env::temp_dir().join(format!("nimora-recovery-state-{}", uuid::Uuid::now_v7()));
        let database = root.join("runtime.sqlite3");
        let corrupt_bytes = b"preserve this unavailable database";
        std::fs::create_dir_all(&root).expect("fixture directory");
        std::fs::write(&database, corrupt_bytes).expect("corrupt fixture");
        let state = DesktopState::open_recovery(
            None,
            root.join("assets"),
            root.join("programs"),
            BackupCoordinator::new(&database, root.join("backups"), BackupPolicy::default()),
            PersistentDiagnosticJournal::in_memory(),
            "database-unavailable",
            None,
        )
        .expect("recovery state");

        assert_eq!(state.startup.mode, StartupMode::Recovery);
        assert_eq!(state.startup.reason, Some("database-unavailable"));
        assert!(matches!(
            ensure_normal_mode(&state),
            Err(DesktopError::RecoveryModeActive)
        ));
        let diagnostic = diagnostic_report(&state).expect("diagnostic preview");
        assert_eq!(diagnostic.runtime.startup_mode, "recovery");
        assert_eq!(
            diagnostic.runtime.startup_reason.as_deref(),
            Some("database-unavailable")
        );
        assert!(!diagnostic.privacy.includes_secrets);
        assert!(!diagnostic.privacy.includes_user_content);
        assert!(!diagnostic.privacy.includes_file_paths);
        assert!(!diagnostic.privacy.automatically_uploaded);
        assert_eq!(diagnostic.sources.event_count, 1);
        assert_eq!(diagnostic.sources.event_retention_days, 14);
        let events = state
            .diagnostic_journal
            .lock()
            .expect("diagnostic journal")
            .snapshot();
        assert_eq!(events.entries.len(), 1);
        assert_eq!(events.entries[0].severity, DiagnosticSeverity::Error);
        assert_eq!(
            events.entries[0].component,
            DiagnosticComponent::Persistence
        );
        assert_eq!(
            events.entries[0].code,
            DiagnosticEventCode::RecoveryModeStarted
        );
        assert_eq!(
            std::fs::read(&database).expect("preserved database"),
            corrupt_bytes
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn unavailable_diagnostic_storage_degrades_to_memory() {
        let root = std::env::temp_dir().join(format!(
            "nimora-diagnostic-fallback-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&root).expect("fixture directory");
        let blocked_path = root.join("events");
        std::fs::write(&blocked_path, b"not a directory").expect("blocked fixture");

        let mut journal = open_diagnostic_journal(&blocked_path, 1_784_294_125_392);
        assert!(journal.is_empty());
        journal
            .record(
                super::diagnostic_event(
                    DiagnosticSeverity::Info,
                    DiagnosticComponent::Application,
                    DiagnosticEventCode::ApplicationStarted,
                )
                .expect("diagnostic event"),
            )
            .expect("memory journal remains available");
        assert_eq!(journal.len(), 1);
        assert_eq!(
            std::fs::read(&blocked_path).expect("preserved fixture"),
            b"not a directory"
        );

        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn accepts_finite_screen_coordinates() {
        assert_eq!(screen_coordinate(42.6).expect("valid coordinate"), 43);
        assert_eq!(screen_coordinate(-12.4).expect("valid coordinate"), -12);
    }

    #[test]
    fn validates_animation_map_against_latest_probe() {
        let idle = ModelAnimationBinding {
            animation: "Idle".to_owned(),
            looped: true,
        };
        let animation_map = std::collections::BTreeMap::from([("pet.idle".to_owned(), idle)]);
        validate_requested_animation_map(&animation_map, &["Idle".to_owned()]).unwrap();
        assert!(validate_requested_animation_map(&animation_map, &["Walk".to_owned()]).is_err());
        assert!(
            validate_requested_animation_map(
                &std::collections::BTreeMap::new(),
                &["Idle".to_owned()]
            )
            .is_err()
        );
        validate_requested_animation_map(&std::collections::BTreeMap::new(), &[]).unwrap();
    }

    #[test]
    fn asset_protocol_paths_reject_encoded_escape_and_ambiguity() {
        assert_eq!(
            parse_asset_protocol_path("/character.example.mochi/sprites/idle.webp"),
            Some((
                "character.example.mochi".to_owned(),
                std::path::PathBuf::from("sprites/idle.webp")
            ))
        );
        for path in [
            "/character.example.mochi/../secret",
            "/character.example.mochi/%2e%2e/secret",
            "/character.example.mochi/sprites%5catlas.webp",
            "/character.example.mochi//atlas.webp",
            "/character.example.mochi/%00atlas.webp",
            "/invalid/atlas.webp",
        ] {
            assert!(parse_asset_protocol_path(path).is_none(), "accepted {path}");
        }
    }

    #[test]
    fn asset_protocol_restricts_window_host_method_and_active_asset() {
        let root = std::env::temp_dir().join("nimora-asset-protocol-policy");
        let _ = std::fs::remove_dir_all(&root);
        let uri: tauri::http::Uri =
            "nimora-asset://localhost/character.example.mochi/sprites/idle.webp"
                .parse()
                .unwrap();
        let request =
            |label, mode, method, uri| serve_asset_protocol(&root, mode, label, method, uri).status;
        assert_eq!(
            request(
                "control-center",
                RuntimeMode::Normal,
                &tauri::http::Method::GET,
                &uri
            ),
            tauri::http::StatusCode::FORBIDDEN
        );
        assert_eq!(
            request(
                super::PET_WINDOW_LABEL,
                RuntimeMode::Safe,
                &tauri::http::Method::GET,
                &uri
            ),
            tauri::http::StatusCode::FORBIDDEN
        );
        let foreign_host: tauri::http::Uri =
            "nimora-asset://evil.invalid/character.example.mochi/sprites/idle.webp"
                .parse()
                .unwrap();
        assert_eq!(
            request(
                super::PET_WINDOW_LABEL,
                RuntimeMode::Normal,
                &tauri::http::Method::GET,
                &foreign_host
            ),
            tauri::http::StatusCode::BAD_REQUEST
        );
        assert_eq!(
            request(
                super::PET_WINDOW_LABEL,
                RuntimeMode::Normal,
                &tauri::http::Method::POST,
                &uri
            ),
            tauri::http::StatusCode::BAD_REQUEST
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn asset_protocol_serves_only_the_active_verified_glb_entrypoint() {
        let root = std::env::temp_dir().join("nimora-asset-protocol-gltf");
        let staged = root.join("staged/character.glb");
        let store = root.join("assets");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(staged.parent().unwrap()).unwrap();
        std::fs::write(&staged, b"verified-glb").unwrap();
        install_gltf_character(
            &staged,
            &store,
            &GltfCharacterMetadata {
                id: "character.local.aurora".to_owned(),
                version: "1.0.0".to_owned(),
                name: "Aurora".to_owned(),
                publisher: "publisher.local".to_owned(),
                license: "LicenseRef-Proprietary".to_owned(),
                animation_map: std::collections::BTreeMap::new(),
            },
        )
        .unwrap();
        persist_active_character(&store, "character.local.aurora").unwrap();

        let model: tauri::http::Uri =
            "nimora-asset://localhost/character.local.aurora/models/character.glb"
                .parse()
                .unwrap();
        let response = serve_asset_protocol(
            &store,
            RuntimeMode::Normal,
            super::PET_WINDOW_LABEL,
            &tauri::http::Method::GET,
            &model,
        );
        assert_eq!(response.status, tauri::http::StatusCode::OK);
        assert_eq!(response.media_type, "model/gltf-binary");
        assert_eq!(response.body, b"verified-glb");

        for forbidden in [
            "nimora-asset://localhost/character.local.aurora/manifest.json",
            "nimora-asset://localhost/character.local.aurora/.integrity.json",
            "nimora-asset://localhost/character.local.aurora/models/other.glb",
            "nimora-asset://localhost/character.local.aurora/models/character.glb?raw=1",
            "nimora-asset://localhost/character.local.other/models/character.glb",
        ] {
            let uri = forbidden.parse().unwrap();
            assert_ne!(
                serve_asset_protocol(
                    &store,
                    RuntimeMode::Normal,
                    super::PET_WINDOW_LABEL,
                    &tauri::http::Method::GET,
                    &uri,
                )
                .status,
                tauri::http::StatusCode::OK,
                "served {forbidden}"
            );
        }
        assert_ne!(
            serve_asset_protocol(
                &store,
                RuntimeMode::Normal,
                "control-center",
                &tauri::http::Method::GET,
                &model,
            )
            .status,
            tauri::http::StatusCode::OK
        );
        assert_ne!(
            serve_asset_protocol(
                &store,
                RuntimeMode::Normal,
                super::PET_WINDOW_LABEL,
                &tauri::http::Method::POST,
                &model,
            )
            .status,
            tauri::http::StatusCode::OK
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn catalog_quarantines_corrupt_packages_without_failing_the_snapshot() {
        let root = std::env::temp_dir().join("nimora-corrupt-asset-catalog");
        let package = root.join("character.example.broken");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(package.join("manifest.json"), b"not-json").unwrap();
        let snapshot = inspect_asset_catalog(&root).unwrap();
        assert!(snapshot.assets.is_empty());
        assert_eq!(snapshot.rejected.len(), 1);
        assert_eq!(snapshot.rejected[0].directory, "character.example.broken");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn active_character_defaults_and_persists_builtin_selection() {
        let root = std::env::temp_dir().join("nimora-active-character-default");
        let _ = std::fs::remove_dir_all(&root);
        let initial = resolve_active_character(&root, RuntimeMode::Normal).unwrap();
        assert_eq!(initial.asset_id, BUILTIN_CHARACTER_ID);
        assert!(initial.fallback_reason.is_none());
        persist_active_character(&root, BUILTIN_CHARACTER_ID).unwrap();
        let restored = resolve_active_character(&root, RuntimeMode::Normal).unwrap();
        assert_eq!(restored.asset_id, BUILTIN_CHARACTER_ID);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn active_character_falls_back_for_corrupt_selection_and_safe_mode() {
        let root = std::env::temp_dir().join("nimora-active-character-fallback");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(ACTIVE_CHARACTER_FILE), b"not-json").unwrap();
        let corrupt = resolve_active_character(&root, RuntimeMode::Normal).unwrap();
        assert_eq!(corrupt.asset_id, BUILTIN_CHARACTER_ID);
        assert!(corrupt.fallback_reason.is_some());
        let safe = resolve_active_character(&root, RuntimeMode::Safe).unwrap();
        assert_eq!(safe.asset_id, BUILTIN_CHARACTER_ID);
        assert!(safe.fallback_reason.is_some());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn renderer_descriptor_uses_builtin_for_safe_mode_and_corrupt_selection() {
        let root = std::env::temp_dir().join("nimora-character-renderer-fallback");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(ACTIVE_CHARACTER_FILE), b"not-json").unwrap();
        let corrupt = resolve_character_renderer(&root, RuntimeMode::Normal).unwrap();
        assert_eq!(corrupt.spec, "nimora.renderer/1");
        assert_eq!(corrupt.asset_id, BUILTIN_CHARACTER_ID);
        assert_eq!(corrupt.backend, "built-in");
        assert!(corrupt.clips.is_none());
        assert!(corrupt.fallback_reason.is_some());
        let safe = resolve_character_renderer(&root, RuntimeMode::Safe).unwrap();
        assert_eq!(safe.backend, "built-in");
        assert!(safe.fallback_reason.is_some());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn package_source_requires_an_absolute_existing_path() {
        assert!(matches!(
            validate_package_source(Path::new("relative/package")),
            Err(DesktopError::InvalidPackageSource)
        ));
        let missing = std::env::temp_dir().join("nimora-missing-package-source");
        assert!(matches!(
            validate_package_source(&missing),
            Err(DesktopError::InvalidPackageSource)
        ));
        let root = std::env::temp_dir().join("nimora-valid-package-source");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        validate_package_source(&root).unwrap();
        let archive = root.join("package.nimora");
        std::fs::write(&archive, b"archive").unwrap();
        validate_package_source(&archive).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn model_source_requires_an_absolute_regular_glb_within_budget() {
        assert!(matches!(
            validate_model_source(Path::new("relative.glb")),
            Err(DesktopError::InvalidModelSource)
        ));
        let root =
            std::env::temp_dir().join(format!("nimora-model-source-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&root).unwrap();
        assert!(matches!(
            validate_model_source(&root),
            Err(DesktopError::InvalidModelSource)
        ));
        let wrong_extension = root.join("character.gltf");
        std::fs::write(&wrong_extension, b"glTF").unwrap();
        assert!(matches!(
            validate_model_source(&wrong_extension),
            Err(DesktopError::InvalidModelSource)
        ));
        let model = root.join("character.glb");
        std::fs::write(&model, b"glTF").unwrap();
        validate_model_source(&model).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn model_source_rejects_symbolic_links() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!("nimora-model-link-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&root).unwrap();
        let target = root.join("target.glb");
        let link = root.join("linked.glb");
        std::fs::write(&target, b"glTF").unwrap();
        symlink(&target, &link).unwrap();
        assert!(matches!(
            validate_model_source(&link),
            Err(DesktopError::InvalidModelSource)
        ));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn package_operation_receipts_do_not_expose_host_paths() {
        let install = serde_json::to_value(AssetInstallReceipt {
            asset_id: "character.example.mochi".to_owned(),
            replaced_previous: false,
        })
        .unwrap();
        let rollback = serde_json::to_value(UserProgramRollbackReceipt {
            program_id: "program.example.focus".to_owned(),
            quarantined_failed_version: true,
        })
        .unwrap();
        assert_eq!(
            install.get("assetId").and_then(serde_json::Value::as_str),
            Some("character.example.mochi")
        );
        assert!(install.get("activePath").is_none());
        assert!(rollback.get("activePath").is_none());
    }

    #[test]
    fn parses_a_bounded_user_program_capability_plan() {
        let plan = parse_user_program_plan(json!({
            "storage": [{"type": "write", "key": "settings", "value": {"volume": 0.8}}],
            "commands": [{
                "command": "safe.pet.animate",
                "arguments": {"action": "work"},
                "idempotencyKey": "action-1"
            }]
        }))
        .expect("valid plan");
        assert_eq!(plan.storage.len(), 1);
        assert_eq!(plan.commands.len(), 1);
        assert_eq!(plan.commands[0].command, "safe.pet.animate");
        assert_eq!(
            plan.commands[0].idempotency_key.as_deref(),
            Some("action-1")
        );
    }

    #[test]
    fn rejects_oversized_user_program_capability_plans() {
        let storage = (0..32)
            .map(|index| json!({"type": "read", "key": format!("key-{index}")}))
            .collect::<Vec<_>>();
        assert!(matches!(
            parse_user_program_plan(json!({
                "storage": storage,
                "commands": [{"command": "safe.pet.animate"}]
            })),
            Err(DesktopError::UserCodeHost(message)) if message.contains("32-operation")
        ));
    }

    #[test]
    fn omits_pet_state_without_explicit_read_capability() {
        let policy = evaluate(ProgramManifest {
            id: "studio.example.no-read".to_owned(),
            version: "1.0.0".to_owned(),
            capabilities: vec![],
            subscriptions: vec![],
            event_concurrency: nimora_user_code_policy::EventConcurrencyPolicy::Serial,
            event_queue_capacity: 16,
            commands: vec![],
            timeout_ms: 1_000,
            memory_bytes: 1024 * 1024,
        })
        .expect("valid policy");
        assert_eq!(
            user_program_input(&policy, Some(json!({"name": "private"})), None, None),
            json!({"schemaVersion": 1})
        );
    }

    #[test]
    fn includes_pet_state_after_explicit_read_capability() {
        let policy = evaluate(ProgramManifest {
            id: "studio.example.read".to_owned(),
            version: "1.0.0".to_owned(),
            capabilities: vec![Capability::ReadPetState],
            subscriptions: vec![],
            event_concurrency: nimora_user_code_policy::EventConcurrencyPolicy::Serial,
            event_queue_capacity: 16,
            commands: vec![],
            timeout_ms: 1_000,
            memory_bytes: 1024 * 1024,
        })
        .expect("valid policy");
        assert_eq!(
            user_program_input(&policy, Some(json!({"name": "Aster"})), None, None),
            json!({"schemaVersion": 1, "pet": {"name": "Aster"}})
        );
    }

    #[test]
    fn includes_profile_state_after_explicit_read_capability() {
        let policy = evaluate(ProgramManifest {
            id: "studio.example.profile-read".to_owned(),
            version: "1.0.0".to_owned(),
            capabilities: vec![Capability::ReadProfileState],
            subscriptions: vec![],
            event_concurrency: nimora_user_code_policy::EventConcurrencyPolicy::Serial,
            event_queue_capacity: 16,
            commands: vec![],
            timeout_ms: 1_000,
            memory_bytes: 1024 * 1024,
        })
        .expect("valid policy");
        assert_eq!(
            user_program_input(
                &policy,
                None,
                Some(json!({"activeProfileId": "profile-1"})),
                None,
            ),
            json!({
                "schemaVersion": 1,
                "profile": {"activeProfileId": "profile-1"}
            })
        );
    }

    #[test]
    fn includes_a_trusted_event_trigger_without_renderer_fields() {
        let policy = evaluate(ProgramManifest {
            id: "studio.example.events".to_owned(),
            version: "1.0.0".to_owned(),
            capabilities: vec![Capability::SubscribeEvents],
            subscriptions: vec!["pet.example.clicked".to_owned()],
            event_concurrency: nimora_user_code_policy::EventConcurrencyPolicy::Serial,
            event_queue_capacity: 16,
            commands: vec![],
            timeout_ms: 1_000,
            memory_bytes: 1024 * 1024,
        })
        .expect("valid policy");
        let event = Event::new(
            "pet.example.clicked",
            EventSource::Core,
            json!({"button": "left"}),
        )
        .expect("valid event");
        let input = user_program_input(&policy, None, None, Some(event));
        assert_eq!(input["schemaVersion"], 1);
        assert_eq!(input["trigger"]["type"], "event");
        assert_eq!(
            input["trigger"]["event"]["eventType"],
            "pet.example.clicked"
        );
        assert_eq!(input["trigger"]["event"]["source"], "core");
    }

    #[test]
    fn permission_grants_use_stable_exhaustive_capability_names() {
        let grant = permission_grant(&ProgramManifest {
            id: "studio.example.permissions".to_owned(),
            version: "1.0.0".to_owned(),
            capabilities: vec![
                Capability::ReadPetState,
                Capability::ReadProfileState,
                Capability::SubscribeEvents,
                Capability::InvokeSafeCommands,
                Capability::StoreLocalData,
            ],
            subscriptions: vec![],
            event_concurrency: nimora_user_code_policy::EventConcurrencyPolicy::Serial,
            event_queue_capacity: 16,
            commands: vec![],
            timeout_ms: 1_000,
            memory_bytes: 1024 * 1024,
        });
        assert_eq!(
            grant.capabilities,
            [
                "read-pet-state",
                "read-profile-state",
                "subscribe-events",
                "invoke-safe-commands",
                "store-local-data",
            ]
        );
    }

    #[test]
    fn installed_program_admission_requires_an_exact_persisted_grant() {
        let repository = SqliteProgramPermissionRepository::in_memory().expect("database");
        let mut manifest = ProgramManifest {
            id: "studio.example.permissions".to_owned(),
            version: "1.0.0".to_owned(),
            capabilities: vec![],
            subscriptions: vec![],
            event_concurrency: nimora_user_code_policy::EventConcurrencyPolicy::Serial,
            event_queue_capacity: 16,
            commands: vec![],
            timeout_ms: 1_000,
            memory_bytes: 1024 * 1024,
        };
        ensure_program_permissions(&repository, &manifest).expect("capability-free program");
        manifest.capabilities.push(Capability::ReadPetState);
        assert!(matches!(
            ensure_program_permissions(&repository, &manifest),
            Err(DesktopError::UserProgramPermissionRequired)
        ));
        repository
            .grant(&permission_grant(&manifest))
            .expect("grant");
        ensure_program_permissions(&repository, &manifest).expect("granted program");
        manifest.version = "2.0.0".to_owned();
        assert!(matches!(
            ensure_program_permissions(&repository, &manifest),
            Err(DesktopError::UserProgramPermissionRequired)
        ));
    }

    #[test]
    fn rejects_unsafe_screen_coordinates() {
        assert!(matches!(
            screen_coordinate(f64::NAN),
            Err(DesktopError::InvalidPosition)
        ));
        assert!(matches!(
            screen_coordinate(f64::INFINITY),
            Err(DesktopError::InvalidPosition)
        ));
        assert!(matches!(
            screen_coordinate(f64::from(i32::MAX) + 1.0),
            Err(DesktopError::InvalidPosition)
        ));
    }

    #[test]
    fn tray_menu_ids_map_to_explicit_actions() {
        assert_eq!(TrayAction::from("open"), TrayAction::OpenControlCenter);
        assert_eq!(
            TrayAction::from("interactive"),
            TrayAction::RestoreInteraction
        );
        assert_eq!(TrayAction::from("safe-mode"), TrayAction::EnterSafeMode);
        assert_eq!(TrayAction::from("normal-mode"), TrayAction::ExitSafeMode);
        assert_eq!(TrayAction::from("quit"), TrayAction::Quit);
        assert_eq!(TrayAction::from("future-action"), TrayAction::Unknown);
    }

    #[test]
    fn action_contract_uses_snake_case_values() {
        let value = serde_json::to_value(PetAction::Celebrate).expect("serializable action");
        assert_eq!(value, "celebrate");
    }

    #[test]
    fn window_policy_resolves_partial_profile_overrides() {
        let policy = ProfilePolicy {
            mode: nimora_runtime_core::ProfileMode::Companion,
            always_on_top: Some(false),
            click_through: None,
            sound_enabled: None,
            proactive_frequency: None,
        };
        assert_eq!(
            WindowPolicy::from_profile(&policy),
            WindowPolicy {
                always_on_top: false,
                click_through: false,
            }
        );
        assert_eq!(
            WindowPolicy::SAFE,
            WindowPolicy {
                always_on_top: true,
                click_through: false,
            }
        );
    }

    #[test]
    fn asset_identifiers_require_safe_namespaced_segments() {
        assert!(valid_asset_identifier("character.example.mochi"));
        assert!(!valid_asset_identifier("character.example"));
        assert!(!valid_asset_identifier("character.example../escape"));
        assert!(!valid_asset_identifier("Character.example.mochi"));
    }
}
