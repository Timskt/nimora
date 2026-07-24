//! Pure window filtering helpers (platform-independent).

use crate::types::WindowFact;

/// Layers at or above Dock/menu bar chrome on macOS CGWindow levels.
pub const SHELL_LAYER_MIN: i32 = 20;

/// Owner names treated as shell/chrome surfaces (case-insensitive contains).
const SHELL_OWNER_MARKERS: &[&str] = &[
    "dock",
    "window server",
    "systemuiserver",
    "control center",
    "notification center",
    "wallpaper",
    "loginwindow",
    "spotlight",
    "siri",
    "axvisualsupportagent",
    "textinputmenuagent",
    "universalcontrol",
];

/// Returns true when the window has zero width or height.
#[must_use]
pub const fn is_zero_size(width: u32, height: u32) -> bool {
    width == 0 || height == 0
}

/// Returns true when the CGWindow layer is dock/menu/status chrome.
#[must_use]
pub const fn is_shell_layer(layer: i32) -> bool {
    layer >= SHELL_LAYER_MIN
}

/// Returns true when the owner name matches known shell processes.
#[must_use]
pub fn is_shell_owner_name(owner_name: &str) -> bool {
    let lower = owner_name.to_ascii_lowercase();
    SHELL_OWNER_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
}

/// Returns true when a raw candidate should be dropped before emission.
#[must_use]
pub fn should_drop_window(
    owner_pid: u32,
    owner_name: &str,
    layer: i32,
    width: u32,
    height: u32,
    own_pid: Option<u32>,
) -> bool {
    if is_zero_size(width, height) {
        return true;
    }
    if own_pid.is_some_and(|pid| pid == owner_pid) {
        return true;
    }
    if is_shell_layer(layer) || is_shell_owner_name(owner_name) {
        return true;
    }
    false
}

/// Maximum windows retained after filtering (stable occlusion / obstacle samples).
pub const MAX_FILTERED_WINDOWS: usize = 64;

/// Filters a collected window list with the standard privacy/obstacle rules.
///
/// Drops own-process windows, shell chrome, zero-size / minimized surfaces.
/// Sorts by `z_order` (front-first) and caps at [`MAX_FILTERED_WINDOWS`].
/// Titles are forced empty (privacy).
#[must_use]
pub fn filter_windows(windows: Vec<WindowFact>, own_pid: Option<u32>) -> Vec<WindowFact> {
    let mut filtered: Vec<WindowFact> = windows
        .into_iter()
        .filter(|window| {
            !window.is_minimized
                && !should_drop_window(
                    window.owner_pid,
                    &window.owner_name,
                    window.layer,
                    window.width,
                    window.height,
                    own_pid,
                )
        })
        .map(|mut window| {
            window.title.clear();
            window.is_shell = false;
            window
        })
        .collect();
    // Stable front-to-back order for occlusion (lower z = front).
    filtered.sort_by_key(|window| window.z_order);
    if filtered.len() > MAX_FILTERED_WINDOWS {
        filtered.truncate(MAX_FILTERED_WINDOWS);
    }
    filtered
}
