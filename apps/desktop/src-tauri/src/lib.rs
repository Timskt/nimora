use asterpet_persistence_sqlite::{SqlitePersistenceError, SqlitePetRepository};
use asterpet_runtime_app::{RuntimeError, RuntimeService};
use asterpet_runtime_core::{Command, Pet, PetAction, Position};
use serde::{Deserialize, Serialize};
use std::{io, path::Path, sync::Mutex};
use tauri::{
    AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent,
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
};
use thiserror::Error;

const CONTROL_CENTER_LABEL: &str = "control-center";
const PET_WINDOW_LABEL: &str = "pet";

#[derive(Debug)]
struct DesktopState {
    runtime: RuntimeService<SqlitePetRepository>,
    click_through: Mutex<bool>,
}

impl DesktopState {
    fn open(database_path: &Path) -> Result<Self, DesktopError> {
        Ok(Self {
            runtime: RuntimeService::initialize(
                SqlitePetRepository::open(database_path)?,
                "Aster",
            )?,
            click_through: Mutex::new(false),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopSnapshot {
    pet: Pet,
    click_through: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MovePetRequest {
    x: f64,
    y: f64,
}

#[derive(Debug, Error)]
enum DesktopError {
    #[error("pet state is unavailable")]
    StatePoisoned,
    #[error("desktop window is unavailable: {0}")]
    WindowUnavailable(String),
    #[error("pet position must be a finite 32-bit screen coordinate")]
    InvalidPosition,
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error(transparent)]
    Persistence(#[from] SqlitePersistenceError),
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
    let click_through = *state
        .click_through
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?;
    Ok(DesktopSnapshot { pet, click_through })
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
fn set_click_through(
    app: AppHandle,
    state: State<'_, DesktopState>,
    enabled: bool,
) -> Result<(), DesktopError> {
    app.get_webview_window(PET_WINDOW_LABEL)
        .ok_or_else(|| DesktopError::WindowUnavailable(PET_WINDOW_LABEL.to_owned()))?
        .set_ignore_cursor_events(enabled)?;
    *state
        .click_through
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)? = enabled;
    Ok(())
}

fn show_control_center(app: &AppHandle) -> Result<(), DesktopError> {
    let window = app
        .get_webview_window(CONTROL_CENTER_LABEL)
        .ok_or_else(|| DesktopError::WindowUnavailable(CONTROL_CENTER_LABEL.to_owned()))?;
    window.show()?;
    window.unminimize()?;
    window.set_focus()?;
    Ok(())
}

fn create_pet_window(app: &AppHandle) -> Result<(), DesktopError> {
    let window =
        WebviewWindowBuilder::new(app, PET_WINDOW_LABEL, WebviewUrl::App("/?view=pet".into()))
            .title("Aster")
            .inner_size(260.0, 300.0)
            .min_inner_size(180.0, 210.0)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .shadow(false)
            .build()?;
    let position = app.state::<DesktopState>().runtime.snapshot()?.position;
    window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
        screen_coordinate(position.x)?,
        screen_coordinate(position.y)?,
    )))?;
    Ok(())
}

fn create_tray(app: &AppHandle) -> Result<(), DesktopError> {
    let open = MenuItem::with_id(app, "open", "打开控制中心", true, None::<&str>)?;
    let interactive = MenuItem::with_id(app, "interactive", "恢复宠物交互", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出 AsterPet", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &interactive, &quit])?;

    TrayIconBuilder::with_id("asterpet-tray")
        .tooltip("AsterPet · 本地运行")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => {
                let _ = show_control_center(app);
            }
            "interactive" => {
                if let Some(window) = app.get_webview_window(PET_WINDOW_LABEL) {
                    let _ = window.set_ignore_cursor_events(false);
                    let _ = window.show();
                    let _ = window.set_focus();
                }
                if let Some(state) = app.try_state::<DesktopState>()
                    && let Ok(mut enabled) = state.click_through.lock()
                {
                    *enabled = false;
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(event, TrayIconEvent::DoubleClick { .. }) {
                let _ = show_control_center(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Starts the `AsterPet` desktop application.
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
            app.manage(DesktopState::open(&data_directory.join("runtime.sqlite3"))?);
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
            if let WindowEvent::Moved(position) = event
                && window.label() == PET_WINDOW_LABEL
            {
                let state = window.state::<DesktopState>();
                let _ = state.runtime.move_pet(Position {
                    x: f64::from(position.x),
                    y: f64::from(position.y),
                });
            }
        })
        .invoke_handler(tauri::generate_handler![
            desktop_snapshot,
            move_pet,
            play_pet_action,
            set_click_through
        ])
        .run(tauri::generate_context!())
        .expect("AsterPet desktop runtime failed");
}

#[cfg(test)]
mod tests {
    use super::{DesktopError, PetAction, screen_coordinate};

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
    fn action_contract_uses_snake_case_values() {
        let value = serde_json::to_value(PetAction::Celebrate).expect("serializable action");
        assert_eq!(value, "celebrate");
    }
}
