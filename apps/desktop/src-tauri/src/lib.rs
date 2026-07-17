use nimora_asset_installer::{InstallError, InstallFile, install_atomically, rollback_latest};
use nimora_persistence_sqlite::{
    ProgramPermissionGrant, SqlitePersistenceError, SqlitePetRepository, SqliteProfileRepository,
    SqliteProgramPermissionRepository,
};
use nimora_runtime_app::{
    ProfileService, ProfileServiceError, ProfileSnapshot, RuntimeError, RuntimeEventBus,
    RuntimeService, SafetyService, SafetyServiceError,
};
use nimora_runtime_core::{
    Command, CommandRisk, Event, EventSource, Pet, PetAction, PointerButton, Position, ProfileId,
    ProfilePolicy, RuntimeMode, SafeModeReason, SafetySnapshot,
};
use nimora_user_code_gateway::{
    CapabilityBackend, CapabilityGateway, CapabilityResponse, GatewayEnvelope, GatewayError,
};
use nimora_user_code_host::{WorkerConfig, WorkerMessage, WorkerProcess};
use nimora_user_code_package::{
    ProgramPackageError, install_program_atomically, load_installed_program, rollback_program,
};
use nimora_user_code_policy::{
    Capability, ExecutionController, ExecutionHandle, ExecutionPolicy, PolicyError,
    ProgramManifest, WorkerError, evaluate,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use tauri::{
    AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent,
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
};
use thiserror::Error;
use uuid::Uuid;

const CONTROL_CENTER_LABEL: &str = "control-center";
const PET_WINDOW_LABEL: &str = "pet";
const POSITION_WRITE_DEBOUNCE: Duration = Duration::from_millis(200);
const CLICK_FEEDBACK_DURATION: Duration = Duration::from_millis(600);
const MAX_USER_PROGRAM_COMMANDS: usize = 32;

#[derive(Debug)]
struct DesktopState {
    runtime: RuntimeService<SqlitePetRepository>,
    profiles: ProfileService<SqliteProfileRepository>,
    safety: SafetyService,
    events: RuntimeEventBus,
    window_policy: Mutex<WindowPolicy>,
    policy_before_safe_mode: Mutex<Option<WindowPolicy>>,
    position_revision: AtomicU64,
    dragging: AtomicBool,
    asset_store: PathBuf,
    program_store: PathBuf,
    program_permissions: SqliteProgramPermissionRepository,
    user_programs: Mutex<HashMap<Uuid, UserProgramSession>>,
    execution_controller: ExecutionController,
}

#[derive(Debug)]
struct UserProgramSession {
    policy: ExecutionPolicy,
    execution: ExecutionHandle,
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
        database_path: &Path,
        asset_store: PathBuf,
        program_store: PathBuf,
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
        Ok(Self {
            runtime,
            profiles,
            safety: SafetyService::new(events.clone()),
            events,
            window_policy: Mutex::new(window_policy),
            policy_before_safe_mode: Mutex::new(None),
            position_revision: AtomicU64::new(0),
            dragging: AtomicBool::new(false),
            asset_store,
            program_store,
            program_permissions: SqliteProgramPermissionRepository::open(database_path)?,
            user_programs: Mutex::new(HashMap::new()),
            execution_controller: ExecutionController::default(),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopSnapshot {
    pet: Pet,
    window_policy: WindowPolicy,
    safety: SafetySnapshot,
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
#[serde(rename_all = "camelCase")]
struct InstallAssetRequest {
    asset_id: String,
    source_path: PathBuf,
    files: Vec<InstallAssetFile>,
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
    active_path: PathBuf,
    replaced_previous: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AssetRollbackReceipt {
    asset_id: String,
    active_path: PathBuf,
    quarantined_failed_version: bool,
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
    active_path: PathBuf,
    replaced_previous: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UserProgramRollbackReceipt {
    program_id: String,
    active_path: PathBuf,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UserProgramPlan {
    #[serde(default)]
    commands: Vec<UserProgramPlanCommand>,
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
    #[error("desktop window is unavailable: {0}")]
    WindowUnavailable(String),
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
    #[error(transparent)]
    Persistence(#[from] SqlitePersistenceError),
    #[error(transparent)]
    AssetInstall(#[from] InstallError),
    #[error("asset identifier must be a lowercase namespaced identifier")]
    InvalidAssetIdentifier,
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
    #[error("user program execution was not found")]
    UserProgramNotFound,
    #[error("user program permissions must be granted for this exact installed version")]
    UserProgramPermissionRequired,
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
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
    })
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn drain_runtime_events(state: State<'_, DesktopState>) -> Result<Vec<Event>, DesktopError> {
    Ok(state.events.drain()?)
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
    Ok(state.profiles.create_profile(name, policy)?)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn switch_profile(
    app: AppHandle,
    state: State<'_, DesktopState>,
    profile_id: ProfileId,
) -> Result<Command, DesktopError> {
    ensure_normal_mode(&state)?;
    let snapshot = state.profiles.snapshot()?;
    let target = snapshot
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or(ProfileServiceError::ProfileNotFound)?;
    let next_policy = WindowPolicy::from_profile(&target.policy);
    let previous_policy = current_window_policy(&state)?;
    apply_window_policy(&app, previous_policy, next_policy)?;
    match state.profiles.switch_active(profile_id) {
        Ok(command) => {
            set_current_window_policy(&state, next_policy)?;
            Ok(command)
        }
        Err(primary) => match apply_window_policy(&app, next_policy, previous_policy) {
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
            *state
                .policy_before_safe_mode
                .lock()
                .map_err(|_| DesktopError::StatePoisoned)? = Some(previous_policy);
            set_current_window_policy(&state, WindowPolicy::SAFE)?;
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
    Ok(state.runtime.play_action(action)?)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn click_pet(
    app: AppHandle,
    state: State<'_, DesktopState>,
    request: ClickPetRequest,
) -> Result<Command, DesktopError> {
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
    if !valid_asset_identifier(&request.asset_id) {
        return Err(DesktopError::InvalidAssetIdentifier);
    }
    let active_path = state.asset_store.join(&request.asset_id);
    let files = request
        .files
        .into_iter()
        .map(|file| InstallFile {
            relative_path: file.relative_path,
            bytes: file.bytes,
            sha256: file.sha256,
        })
        .collect::<Vec<_>>();
    let result = install_atomically(&request.source_path, &active_path, &files)?;
    Ok(AssetInstallReceipt {
        asset_id: request.asset_id,
        active_path: result.active_path,
        replaced_previous: result.backup_path.is_some(),
    })
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
        active_path: result.active_path,
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
    Ok(UserProgramInstallReceipt {
        program_id: result.program_id,
        version: result.version,
        active_path: result.active_path,
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
    let result = rollback_program(&state.program_store, &program_id)?;
    Ok(UserProgramRollbackReceipt {
        program_id,
        active_path: result.active_path,
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
    execute_user_program_source(&app, &state, manifest, source)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn execute_installed_user_program(
    app: AppHandle,
    state: State<'_, DesktopState>,
    program_id: String,
) -> Result<UserProgramExecutionReceipt, DesktopError> {
    ensure_normal_mode(&state)?;
    let installed = load_installed_program(&state.program_store, &program_id)?;
    ensure_program_permissions(&state.program_permissions, &installed.manifest)?;
    execute_user_program_source(&app, &state, installed.manifest, installed.source)
}

fn execute_user_program_source(
    app: &AppHandle,
    state: &DesktopState,
    manifest: ProgramManifest,
    source: String,
) -> Result<UserProgramExecutionReceipt, DesktopError> {
    let policy = evaluate(manifest.clone())?;
    let execution = state.execution_controller.admit(&policy)?;
    let execution_id = execution.execution_id();
    let pet = if policy.can_read_pet_state {
        Some(serde_json::to_value(state.runtime.snapshot()?)?)
    } else {
        None
    };
    let input = user_program_input(&policy, pet);
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
    let mut responses = Vec::with_capacity(plan.commands.len());
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
    if plan.commands.len() > MAX_USER_PROGRAM_COMMANDS {
        return Err(DesktopError::UserCodeHost(format!(
            "capability plan exceeds the {MAX_USER_PROGRAM_COMMANDS}-command limit"
        )));
    }
    Ok(plan)
}

fn user_program_input(
    policy: &ExecutionPolicy,
    pet: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut input =
        serde_json::Map::from_iter([("schemaVersion".to_owned(), serde_json::Value::from(1))]);
    if policy.can_read_pet_state
        && let Some(pet) = pet
    {
        input.insert("pet".to_owned(), pet);
    }
    serde_json::Value::Object(input)
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
    let session = state
        .user_programs
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?
        .remove(&execution_id)
        .ok_or(DesktopError::UserProgramNotFound)?;
    session.execution.cancel();
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
                self.state.runtime.play_action(action)
            }
            "safe.pet.move" => {
                let position = serde_json::from_value::<Position>(arguments)
                    .map_err(|error| error.to_string())?;
                self.state.runtime.move_pet(position)
            }
            _ => return Err("command has no registered desktop backend".to_owned()),
        }
        .map_err(|error| error.to_string())?;
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
        .setup(|app| {
            let data_directory = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_directory)?;
            app.manage(DesktopState::open(
                &data_directory.join("runtime.sqlite3"),
                data_directory.join("assets"),
                data_directory.join("programs"),
            )?);
            create_pet_window(app.handle())?;
            create_tray(app.handle())?;
            Ok(())
        })
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
            drain_runtime_events,
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
            install_asset,
            rollback_asset,
            install_user_program,
            rollback_user_program,
            user_program_permission_status,
            grant_user_program_permissions,
            revoke_user_program_permissions,
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

#[cfg(test)]
mod tests {
    use super::{
        DesktopError, PetAction, ProfilePolicy, TrayAction, WindowPolicy,
        ensure_program_permissions, parse_user_program_plan, permission_grant, screen_coordinate,
        user_program_input, valid_asset_identifier,
    };
    use nimora_persistence_sqlite::SqliteProgramPermissionRepository;
    use nimora_user_code_policy::{Capability, ProgramManifest, evaluate};
    use serde_json::json;

    #[test]
    fn accepts_finite_screen_coordinates() {
        assert_eq!(screen_coordinate(42.6).expect("valid coordinate"), 43);
        assert_eq!(screen_coordinate(-12.4).expect("valid coordinate"), -12);
    }

    #[test]
    fn parses_a_bounded_user_program_capability_plan() {
        let plan = parse_user_program_plan(json!({
            "commands": [{
                "command": "safe.pet.animate",
                "arguments": {"action": "work"},
                "idempotencyKey": "action-1"
            }]
        }))
        .expect("valid plan");
        assert_eq!(plan.commands.len(), 1);
        assert_eq!(plan.commands[0].command, "safe.pet.animate");
        assert_eq!(
            plan.commands[0].idempotency_key.as_deref(),
            Some("action-1")
        );
    }

    #[test]
    fn rejects_oversized_user_program_capability_plans() {
        let commands = (0..33)
            .map(|_| json!({"command": "safe.pet.animate"}))
            .collect::<Vec<_>>();
        assert!(matches!(
            parse_user_program_plan(json!({"commands": commands})),
            Err(DesktopError::UserCodeHost(message)) if message.contains("32-command")
        ));
    }

    #[test]
    fn omits_pet_state_without_explicit_read_capability() {
        let policy = evaluate(ProgramManifest {
            id: "studio.example.no-read".to_owned(),
            version: "1.0.0".to_owned(),
            capabilities: vec![],
            subscriptions: vec![],
            commands: vec![],
            timeout_ms: 1_000,
            memory_bytes: 1024 * 1024,
        })
        .expect("valid policy");
        assert_eq!(
            user_program_input(&policy, Some(json!({"name": "private"}))),
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
            commands: vec![],
            timeout_ms: 1_000,
            memory_bytes: 1024 * 1024,
        })
        .expect("valid policy");
        assert_eq!(
            user_program_input(&policy, Some(json!({"name": "Aster"}))),
            json!({"schemaVersion": 1, "pet": {"name": "Aster"}})
        );
    }

    #[test]
    fn permission_grants_use_stable_exhaustive_capability_names() {
        let grant = permission_grant(&ProgramManifest {
            id: "studio.example.permissions".to_owned(),
            version: "1.0.0".to_owned(),
            capabilities: vec![
                Capability::ReadPetState,
                Capability::SubscribeEvents,
                Capability::InvokeSafeCommands,
                Capability::StoreLocalData,
            ],
            subscriptions: vec![],
            commands: vec![],
            timeout_ms: 1_000,
            memory_bytes: 1024 * 1024,
        });
        assert_eq!(
            grant.capabilities,
            [
                "read-pet-state",
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
