use nimora_asset_installer::{InstallError, InstallFile, install_atomically, rollback_latest};
use nimora_persistence_sqlite::{
    SqlitePersistenceError, SqlitePetRepository, SqliteProfileRepository,
};
use nimora_runtime_app::{
    ProfileService, ProfileServiceError, ProfileSnapshot, RuntimeError, RuntimeEventBus,
    RuntimeService, SafetyService, SafetyServiceError,
};
use nimora_runtime_core::{
    Command, CommandRisk, Event, EventSource, Pet, PetAction, PointerButton, Position, ProfileId,
    ProfilePolicy, RuntimeMode, SafeModeReason, SafetySnapshot,
};
use serde::{Deserialize, Serialize};
use std::{
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

const CONTROL_CENTER_LABEL: &str = "control-center";
const PET_WINDOW_LABEL: &str = "pet";
const POSITION_WRITE_DEBOUNCE: Duration = Duration::from_millis(200);
const CLICK_FEEDBACK_DURATION: Duration = Duration::from_millis(600);

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
    fn open(database_path: &Path, asset_store: PathBuf) -> Result<Self, DesktopError> {
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
    Io(#[from] io::Error),
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
            rollback_asset
        ])
        .run(tauri::generate_context!())
        .expect("Nimora desktop runtime failed");
}

#[cfg(test)]
mod tests {
    use super::{
        DesktopError, PetAction, ProfilePolicy, TrayAction, WindowPolicy, screen_coordinate,
        valid_asset_identifier,
    };

    #[test]
    fn accepts_finite_screen_coordinates() {
        assert_eq!(screen_coordinate(42.6).expect("valid coordinate"), 43);
        assert_eq!(screen_coordinate(-12.4).expect("valid coordinate"), -12);
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
