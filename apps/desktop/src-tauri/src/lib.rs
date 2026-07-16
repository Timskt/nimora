use asterpet_runtime_core::{Command, CommandRisk, Emotion, Pet, PetState, Position};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
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
    pet: Mutex<Pet>,
    click_through: Mutex<bool>,
}

impl DesktopState {
    fn new() -> Result<Self, DesktopError> {
        Ok(Self {
            pet: Mutex::new(Pet::new("Aster")?),
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum PetAction {
    Idle,
    Walk,
    Sleep,
    Work,
    Celebrate,
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
    Pet(#[from] asterpet_runtime_core::PetError),
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
    let pet = state
        .pet
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?
        .clone();
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
    state
        .pet
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?
        .move_to(position)?;
    app.get_webview_window(PET_WINDOW_LABEL)
        .ok_or_else(|| DesktopError::WindowUnavailable(PET_WINDOW_LABEL.to_owned()))?
        .set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
            screen_x, screen_y,
        )))?;
    Command::new(
        "pet.window.move",
        serde_json::json!({ "x": request.x, "y": request.y }),
        CommandRisk::Safe,
    )
    .map_err(|error| DesktopError::WindowUnavailable(error.to_string()))
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
    let (pet_state, emotion) = match action {
        PetAction::Idle => (PetState::Idle, Emotion::Neutral),
        PetAction::Walk => (PetState::Walking, Emotion::Happy),
        PetAction::Sleep => (PetState::Sleeping, Emotion::Sleepy),
        PetAction::Work => (PetState::Working, Emotion::Focused),
        PetAction::Celebrate => (PetState::Interacting, Emotion::Happy),
    };
    let mut pet = state.pet.lock().map_err(|_| DesktopError::StatePoisoned)?;
    pet.state = pet_state;
    pet.emotion = emotion;
    Command::new(
        "pet.animation.play",
        serde_json::json!({ "action": action }),
        CommandRisk::Safe,
    )
    .map_err(|error| DesktopError::WindowUnavailable(error.to_string()))
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
            app.manage(DesktopState::new()?);
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
