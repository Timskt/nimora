//! Desktop lifeform environment mapping and spring-damper pet motion helpers.
//!
//! Pure planning/mapping lives here so the Tauri host can unit-test without OS I/O.
//! Platform adapters are sampled in `lib.rs` and converted via [`LifeformSampleInput`].

use nimora_desktop_context::{
    bounce_on_bounds, compute_pet_occlusion, integrate, is_fullscreen_over_any_display, is_usable,
    jump_parabola, occluders_from_snapshot, occluders_from_snapshot_dpi, plan_cross_display_target,
    plan_wander, plan_wander_from_snapshot, safe_origin_bounds, sanitize_idle_ms, union_work_areas,
    work_area_stages, CursorPosition, DegradationReason, DesktopDisplay, DesktopSnapshot,
    DesktopSnapshotParts, DesktopWindow, ForegroundApp, Freshness, MeetingHint, MeetingState,
    MotionGoal, MotionMode, PowerState, Rect, Size, SpringParams, SpringState, Vec2i,
    WanderRequest, WorkArea, MAX_ENUMERATED_WINDOWS, MAX_SNAPSHOT_LIFETIME_MS,
};

// Re-export sensory throttle helpers for host / tests.
#[allow(unused_imports)]
pub use nimora_desktop_context::{
    battery_sensory_band as lifeform_battery_sensory_band,
    battery_should_emit as lifeform_battery_should_emit,
    idle_sensory_band as lifeform_idle_sensory_band,
    idle_sensory_band_from_ms as lifeform_idle_sensory_band_from_ms,
    idle_should_emit as lifeform_idle_should_emit, meeting_should_emit as lifeform_meeting_should_emit,
    notification_should_emit as lifeform_notification_should_emit,
    notification_unread_from_counts as lifeform_notification_unread_from_counts,
    BooleanSensorGate as LifeformBooleanSensorGate, BATTERY_SAME_BAND_THROTTLE_MS,
    BOOLEAN_SENSOR_HOLD_MS,
};

// Re-export occlusion types for host/callers.
#[allow(unused_imports)]
pub use nimora_desktop_context::{OccluderRect, OcclusionStrip, PetBodyRect, PetOcclusion};

use nimora_runtime_core::{CrowdingLevel, DesktopBehaviorHints, PetVitalsSnapshot};
use serde::{Deserialize, Serialize};

/// Alias for the environment snapshot (host IPC already owns a different `DesktopSnapshot`).
pub type LifeformDesktopContext = DesktopSnapshot;

/// Soft lease applied to successful samples (must stay within [`MAX_SNAPSHOT_LIFETIME_MS`]).
pub const LIFEFORM_LEASE_MS: u64 = 15_000;
/// Fixed integrator timestep (~60 Hz).
pub const SPRING_DT: f64 = 1.0 / 60.0;
/// Maximum spring settle window (seconds).
pub const SPRING_MAX_SECONDS: f64 = 0.6;
/// Frame sleep matching [`SPRING_DT`].
pub const SPRING_FRAME_DURATION_MS: u64 = 16;
/// Extra rise (px) for jump arcs.
pub const JUMP_PEAK_HEIGHT: f64 = 48.0;

/// Default pet body width in physical pixels (overlay content size).
pub const PET_BODY_WIDTH: u32 = 260;
/// Default pet body height in physical pixels (overlay content size).
pub const PET_BODY_HEIGHT: u32 = 300;
/// Small inset used when parking the pet on the stage ground line.
const GROUND_MARGIN_PX: i32 = 16;

/// Full-screen (or work-area) overlay stage origin and size in physical coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayStage {
    pub origin_x: i32,
    pub origin_y: i32,
    pub width: u32,
    pub height: u32,
}

/// Pet origin on screen plus local offset inside the overlay stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PetScreenPose {
    pub x: i32,
    pub y: i32,
    pub local_x: i32,
    pub local_y: i32,
}

/// Integer work-area geometry from the host monitor helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostWorkArea {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Privacy-preserving window fact used while mapping platform samples.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifeformWindowInput {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub z_order: i32,
    pub owner_pid: u32,
    pub owner_name: String,
    pub onscreen: bool,
    pub is_minimized: bool,
    pub is_shell: bool,
}

/// Foreground app fact (no titles).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifeformForegroundInput {
    pub app_name: String,
    pub pid: u32,
}

/// Host-local intermediate before building a validated lifeform snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct LifeformSampleInput {
    pub windows: Vec<LifeformWindowInput>,
    pub foreground: Option<LifeformForegroundInput>,
    pub idle_ms: u64,
    pub power: Option<PowerState>,
    pub meeting_active: bool,
    pub meeting_hint: MeetingHint,
    pub observed_at_ms: u64,
    pub displays: Vec<DesktopDisplay>,
    pub cursor: Option<CursorPosition>,
}

/// One physical pet-window origin for a single animation frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionFrame {
    pub x: i32,
    pub y: i32,
}

/// Maps host monitor geometry into desktop-context [`WorkArea`].
#[must_use]
pub const fn host_work_area_to_context(area: HostWorkArea) -> WorkArea {
    WorkArea {
        x: area.x,
        y: area.y,
        width: area.width,
        height: area.height,
    }
}

/// Maps a lifeform [`WorkArea`] into host physical area fields.
#[must_use]
pub const fn context_work_area_to_host(area: WorkArea) -> HostWorkArea {
    HostWorkArea {
        x: area.x,
        y: area.y,
        width: area.width,
        height: area.height,
    }
}

/// Default pet body [`Size`] matching [`PET_BODY_WIDTH`] × [`PET_BODY_HEIGHT`].
#[must_use]
pub const fn pet_body_size() -> Size {
    Size {
        width: PET_BODY_WIDTH,
        height: PET_BODY_HEIGHT,
    }
}

/// Maps host work-area geometry into an overlay stage (origin + size).
#[must_use]
pub const fn overlay_stage_from_work_area(area: HostWorkArea) -> OverlayStage {
    OverlayStage {
        origin_x: area.x,
        origin_y: area.y,
        width: area.width,
        height: area.height,
    }
}

fn point_in_work_area(x: i32, y: i32, area: WorkArea) -> bool {
    let px = i64::from(x);
    let py = i64::from(y);
    let left = i64::from(area.x);
    let top = i64::from(area.y);
    let right = left.saturating_add(i64::from(area.width));
    let bottom = top.saturating_add(i64::from(area.height));
    px >= left && px < right && py >= top && py < bottom
}

fn pet_center(x: i32, y: i32, w: u32, h: u32) -> (i32, i32) {
    let cx = i64::from(x).saturating_add(i64::from(w) / 2);
    let cy = i64::from(y).saturating_add(i64::from(h) / 2);
    (
        i32::try_from(cx).unwrap_or(x),
        i32::try_from(cy).unwrap_or(y),
    )
}

/// Finds the display whose work area contains the origin point.
///
/// Prefers work-area containment of the point; if none match, returns the first
/// (primary) display when present, else `None`.
#[must_use]
pub fn display_containing_origin<'a>(
    origin_x: i32,
    origin_y: i32,
    displays: &'a [DesktopDisplay],
) -> Option<&'a DesktopDisplay> {
    displays
        .iter()
        .find(|display| point_in_work_area(origin_x, origin_y, display.work_area))
        .or_else(|| displays.first())
}

/// Selects the overlay stage for the pet based on multi-monitor work areas.
///
/// If the pet body center lies inside a display work area, that work area becomes
/// the stage; otherwise falls back to the host work area via
/// [`overlay_stage_from_work_area`].
#[must_use]
pub fn overlay_stage_for_pet(
    pet_x: i32,
    pet_y: i32,
    pet_w: u32,
    pet_h: u32,
    displays: &[DesktopDisplay],
    fallback: HostWorkArea,
) -> OverlayStage {
    let (cx, cy) = pet_center(pet_x, pet_y, pet_w, pet_h);
    if let Some(display) = displays
        .iter()
        .find(|display| point_in_work_area(cx, cy, display.work_area))
    {
        return overlay_stage_from_work_area(HostWorkArea {
            x: display.work_area.x,
            y: display.work_area.y,
            width: display.work_area.width,
            height: display.work_area.height,
        });
    }
    overlay_stage_from_work_area(fallback)
}

/// Returns true when the pet body center moves from one display work area to another.
///
/// Missing display membership (empty list / off-desktop) is not treated as a cross.
#[must_use]
pub fn pet_crosses_display_boundary(
    from_x: i32,
    from_y: i32,
    to_x: i32,
    to_y: i32,
    pet_w: u32,
    pet_h: u32,
    displays: &[DesktopDisplay],
) -> bool {
    if displays.is_empty() {
        return false;
    }
    let (from_cx, from_cy) = pet_center(from_x, from_y, pet_w, pet_h);
    let (to_cx, to_cy) = pet_center(to_x, to_y, pet_w, pet_h);
    let from_id = displays
        .iter()
        .find(|d| point_in_work_area(from_cx, from_cy, d.work_area))
        .map(|d| d.id.as_str());
    let to_id = displays
        .iter()
        .find(|d| point_in_work_area(to_cx, to_cy, d.work_area))
        .map(|d| d.id.as_str());
    match (from_id, to_id) {
        (Some(a), Some(b)) => a != b,
        _ => false,
    }
}

/// Computes pet occlusion for a pose using a usable lifeform snapshot.
///
/// Fail-closed: missing or unusable snapshots yield empty occlusion (`coverage = 0`).
#[must_use]
pub fn pet_occlusion_for_pose(
    pose_x: i32,
    pose_y: i32,
    snapshot: Option<&LifeformDesktopContext>,
    now_ms: u64,
    pet_owner_pid: u32,
) -> PetOcclusion {
    pet_occlusion_for_pose_dpi(pose_x, pose_y, snapshot, now_ms, pet_owner_pid, false)
}

/// Occlusion with optional logical→physical window scaling via display DPI.
#[must_use]
pub fn pet_occlusion_for_pose_dpi(
    pose_x: i32,
    pose_y: i32,
    snapshot: Option<&LifeformDesktopContext>,
    now_ms: u64,
    pet_owner_pid: u32,
    apply_display_scale: bool,
) -> PetOcclusion {
    let Some(snapshot) = snapshot else {
        return PetOcclusion::empty();
    };
    if !is_usable(snapshot, now_ms) {
        return PetOcclusion::empty();
    }
    let pet = PetBodyRect {
        x: pose_x,
        y: pose_y,
        width: PET_BODY_WIDTH,
        height: PET_BODY_HEIGHT,
    };
    let occluders = if apply_display_scale {
        occluders_from_snapshot_dpi(snapshot, pet_owner_pid, true)
    } else {
        occluders_from_snapshot(snapshot, pet_owner_pid)
    };
    compute_pet_occlusion(pet, &occluders)
}

/// Converts a screen-space point into stage-local coordinates (`screen - origin`).
#[must_use]
pub const fn pet_local_offset(
    screen_x: i32,
    screen_y: i32,
    stage: OverlayStage,
) -> (i32, i32) {
    (
        screen_x.saturating_sub(stage.origin_x),
        screen_y.saturating_sub(stage.origin_y),
    )
}

/// Converts a stage-local origin into screen coordinates (`local + origin`).
#[must_use]
pub const fn pet_screen_from_local(
    local_x: i32,
    local_y: i32,
    stage: OverlayStage,
) -> (i32, i32) {
    (
        local_x.saturating_add(stage.origin_x),
        local_y.saturating_add(stage.origin_y),
    )
}

/// Clamps a pet top-left origin so the full body stays inside the stage (0 margin).
#[must_use]
pub fn clamp_pet_origin_to_stage(
    x: i32,
    y: i32,
    stage: OverlayStage,
    body: Size,
) -> (i32, i32) {
    let min_x = i64::from(stage.origin_x);
    let min_y = i64::from(stage.origin_y);
    let max_x = i64::from(stage.origin_x)
        .saturating_add(i64::from(stage.width))
        .saturating_sub(i64::from(body.width))
        .max(min_x);
    let max_y = i64::from(stage.origin_y)
        .saturating_add(i64::from(stage.height))
        .saturating_sub(i64::from(body.height))
        .max(min_y);
    let clamped_x = i64::from(x).clamp(min_x, max_x);
    let clamped_y = i64::from(y).clamp(min_y, max_y);
    (
        i32::try_from(clamped_x).unwrap_or(x),
        i32::try_from(clamped_y).unwrap_or(y),
    )
}

/// Builds a screen + local pose for a requested screen origin, clamped to the stage.
#[must_use]
pub fn pose_for_screen(screen_x: i32, screen_y: i32, stage: OverlayStage) -> PetScreenPose {
    let body = pet_body_size();
    let (x, y) = clamp_pet_origin_to_stage(screen_x, screen_y, stage, body);
    let (local_x, local_y) = pet_local_offset(x, y, stage);
    // Local ↔ screen conversion must round-trip for overlay CSS placement.
    let (round_trip_x, round_trip_y) = pet_screen_from_local(local_x, local_y, stage);
    debug_assert_eq!((round_trip_x, round_trip_y), (x, y));
    PetScreenPose {
        x: round_trip_x,
        y: round_trip_y,
        local_x,
        local_y,
    }
}

/// Builds a pose from stage-local coordinates (FE CSS `--pet-local-*` space).
#[must_use]
pub fn pose_for_local(local_x: i32, local_y: i32, stage: OverlayStage) -> PetScreenPose {
    let (screen_x, screen_y) = pet_screen_from_local(local_x, local_y, stage);
    pose_for_screen(screen_x, screen_y, stage)
}

/// Hit-tests a cursor against the pet body rectangle expanded by `padding` on each side.
///
/// Used for ignore-cursor / click-through decisions on the transparent overlay.
#[must_use]
pub fn cursor_hits_pet(
    cursor_x: i32,
    cursor_y: i32,
    pet_x: i32,
    pet_y: i32,
    body: Size,
    padding: i32,
) -> bool {
    let pad = i64::from(padding.max(0));
    let left = i64::from(pet_x).saturating_sub(pad);
    let top = i64::from(pet_y).saturating_sub(pad);
    let right = i64::from(pet_x)
        .saturating_add(i64::from(body.width))
        .saturating_add(pad);
    let bottom = i64::from(pet_y)
        .saturating_add(i64::from(body.height))
        .saturating_add(pad);
    let cx = i64::from(cursor_x);
    let cy = i64::from(cursor_y);
    cx >= left && cx < right && cy >= top && cy < bottom
}

/// Default ground pose: bottom-center of the stage with a small margin (body fully visible).
#[must_use]
pub fn default_ground_pose(stage: OverlayStage) -> (i32, i32) {
    let body = pet_body_size();
    let usable_width = i64::from(stage.width).saturating_sub(i64::from(body.width));
    let usable_height = i64::from(stage.height).saturating_sub(i64::from(body.height));
    let local_x = (usable_width / 2).max(0);
    let local_y = usable_height
        .saturating_sub(i64::from(GROUND_MARGIN_PX))
        .max(0);
    let screen_x = i64::from(stage.origin_x).saturating_add(local_x);
    let screen_y = i64::from(stage.origin_y).saturating_add(local_y);
    clamp_pet_origin_to_stage(
        i32::try_from(screen_x).unwrap_or(stage.origin_x),
        i32::try_from(screen_y).unwrap_or(stage.origin_y),
        stage,
        body,
    )
}

/// Builds a validated lifeform snapshot from a mapped platform sample.
///
/// # Errors
///
/// Returns a construction error when the lease lifetime is invalid.
pub fn lifeform_snapshot_from_sample(
    sample: LifeformSampleInput,
    expires_at_ms: u64,
) -> Result<LifeformDesktopContext, nimora_desktop_context::DesktopContextError> {
    let displays = sample.displays;
    let windows = sanitize_lifeform_windows(sample.windows, &displays);
    let foreground = sample.foreground.map(|app| ForegroundApp {
        app_name: app.app_name,
        pid: app.pid,
        window_id: None,
    });
    DesktopSnapshot::new(DesktopSnapshotParts {
        windows,
        foreground,
        displays,
        power: sample.power,
        idle_ms: sanitize_idle_ms(sample.idle_ms),
        meeting: MeetingState {
            active: sample.meeting_active,
            hint: sample.meeting_hint,
        },
        cursor: sample.cursor,
        observed_at_ms: sample.observed_at_ms,
        expires_at_ms,
        freshness: Freshness::Fresh,
        degradation_reason: None,
    })
}

/// Privacy-preserving window sanitization: drop zero-size, cap count, mark fullscreen.
fn sanitize_lifeform_windows(
    windows: Vec<LifeformWindowInput>,
    displays: &[DesktopDisplay],
) -> Vec<DesktopWindow> {
    let mut mapped: Vec<DesktopWindow> = windows
        .into_iter()
        .filter(|window| window.width > 0 && window.height > 0)
        .map(|window| {
            let bounds = WorkArea {
                x: window.x,
                y: window.y,
                width: window.width,
                height: window.height,
            };
            let is_fullscreen_candidate =
                !window.is_shell && is_fullscreen_over_any_display(bounds, displays, 8);
            DesktopWindow {
                id: window.id,
                title_hash: None,
                title_redacted: true,
                x: window.x,
                y: window.y,
                width: window.width,
                height: window.height,
                z_order: window.z_order,
                owner_pid: window.owner_pid,
                owner_name: window.owner_name,
                onscreen: window.onscreen,
                is_minimized: window.is_minimized,
                is_fullscreen_candidate,
                is_shell: window.is_shell,
            }
        })
        .collect();
    // Stable order: front-most first (lower z_order from CG/EnumWindows).
    mapped.sort_by_key(|window| window.z_order);
    if mapped.len() > MAX_ENUMERATED_WINDOWS {
        mapped.truncate(MAX_ENUMERATED_WINDOWS);
    }
    mapped
}

/// Work-area stage rects for multi-monitor overlay rebinding.
#[must_use]
pub fn lifeform_work_area_stages(displays: &[DesktopDisplay]) -> Vec<WorkArea> {
    work_area_stages(displays)
}

/// Union work-area bounds for cross-display spring walks (None when empty).
#[must_use]
pub fn lifeform_union_work_area(displays: &[DesktopDisplay]) -> Option<WorkArea> {
    union_work_areas(displays)
}

/// Fail-closed degraded snapshot used when sampling fails.
#[must_use]
pub fn degraded_lifeform_snapshot(
    now_ms: u64,
    reason: DegradationReason,
) -> LifeformDesktopContext {
    let expires_at_ms = now_ms.saturating_add(LIFEFORM_LEASE_MS.min(MAX_SNAPSHOT_LIFETIME_MS));
    DesktopSnapshot::new(DesktopSnapshotParts {
        windows: Vec::new(),
        foreground: None,
        displays: Vec::new(),
        power: None,
        idle_ms: 0,
        meeting: MeetingState {
            active: false,
            hint: MeetingHint::None,
        },
        cursor: None,
        observed_at_ms: now_ms,
        expires_at_ms,
        freshness: Freshness::Degraded,
        degradation_reason: Some(reason),
    })
    .unwrap_or_else(|_| LifeformDesktopContext {
        spec: nimora_desktop_context::DESKTOP_CONTEXT_SPEC.to_owned(),
        windows: Vec::new(),
        foreground: None,
        displays: Vec::new(),
        power: None,
        idle_ms: 0,
        meeting: MeetingState {
            active: false,
            hint: MeetingHint::None,
        },
        cursor: None,
        observed_at_ms: now_ms,
        expires_at_ms: now_ms.saturating_add(1),
        freshness: Freshness::Degraded,
        degradation_reason: Some(reason),
    })
}

/// Lease expiry timestamp for a successful sample.
#[must_use]
pub fn lifeform_expires_at(observed_at_ms: u64) -> u64 {
    observed_at_ms.saturating_add(LIFEFORM_LEASE_MS.min(MAX_SNAPSHOT_LIFETIME_MS))
}

/// Crowding heuristic from onscreen non-shell window count.
#[must_use]
pub const fn crowding_from_window_count(count: usize) -> CrowdingLevel {
    if count >= 12 {
        CrowdingLevel::High
    } else if count >= 6 {
        CrowdingLevel::Medium
    } else {
        CrowdingLevel::Low
    }
}

/// Builds autonomy hints from the latest lifeform sample (fail-closed when missing).
#[must_use]
pub fn behavior_hints_from_lifeform(
    snapshot: Option<&LifeformDesktopContext>,
    now_ms: u64,
    suppress_autonomy: bool,
) -> DesktopBehaviorHints {
    let Some(snapshot) = snapshot else {
        return DesktopBehaviorHints {
            crowding: CrowdingLevel::Low,
            idle_ms: 0,
            on_battery: false,
            meeting_active: false,
            suppress_autonomy,
        };
    };
    let usable = is_usable(snapshot, now_ms);
    let window_count = snapshot
        .windows
        .iter()
        .filter(|window| window.onscreen && !window.is_minimized && !window.is_shell)
        .count();
    DesktopBehaviorHints {
        crowding: if usable {
            crowding_from_window_count(window_count)
        } else {
            CrowdingLevel::Low
        },
        idle_ms: if usable { snapshot.idle_ms } else { 0 },
        on_battery: usable
            && snapshot
                .power
                .is_some_and(|power| power.on_battery && !power.charging),
        meeting_active: snapshot.meeting.active,
        suppress_autonomy: suppress_autonomy || snapshot.meeting.active,
    }
}

/// Vitals projection for [`nimora_runtime_core::select_autonomous_intent`].
#[must_use]
#[allow(dead_code)]
pub const fn vitals_snapshot(
    energy: u8,
    mood: u8,
    satiety: u8,
    cleanliness: u8,
    affinity: u8,
) -> PetVitalsSnapshot {
    PetVitalsSnapshot {
        energy,
        mood,
        satiety,
        cleanliness,
        affinity,
    }
}


/// Picks a wander target on a *different* display when multiple work areas exist.
///
/// Sequence-stable: uses autonomy sequence to pick the destination display and
/// park near its bottom-center (grounded presence after a cross-monitor walk).
#[must_use]
pub fn plan_cross_display_wander_target(
    sequence: u64,
    current_x: i32,
    current_y: i32,
    pet_w: u32,
    pet_h: u32,
    displays: &[DesktopDisplay],
) -> Option<(i32, i32)> {
    // Shared pure planner (desktop-context) — sequence-stable destination display.
    plan_cross_display_target(sequence, current_x, current_y, pet_w, pet_h, displays)
}

/// Plans a wander goal using the lifeform snapshot when it is Fresh and unexpired.
#[must_use]
pub fn plan_lifeform_wander_goal(
    sequence: u64,
    current_x: i32,
    current_y: i32,
    pet_width: u32,
    pet_height: u32,
    work_area: WorkArea,
    cursor: Option<CursorPosition>,
    snapshot: Option<&LifeformDesktopContext>,
    now_ms: u64,
) -> Option<MotionGoal> {
    let request = WanderRequest {
        sequence,
        current: Vec2i {
            x: current_x,
            y: current_y,
        },
        pet_size: Size {
            width: pet_width,
            height: pet_height,
        },
        work_area,
        cursor,
    };
    match snapshot {
        Some(snapshot) if is_usable(snapshot, now_ms) => {
            // Every other wander prefers a multi-monitor "walk across" when available.
            // Occasional hop mode for short inter-display gaps keeps motion lively
            // without linear interpolation (spring/jump planners only).
            if sequence % 2 == 0 {
                if let Some((x, y)) = plan_cross_display_wander_target(
                    sequence,
                    current_x,
                    current_y,
                    pet_width,
                    pet_height,
                    &snapshot.displays,
                ) {
                    let dx = (i64::from(x) - i64::from(current_x)).unsigned_abs();
                    let dy = (i64::from(y) - i64::from(current_y)).unsigned_abs();
                    let short_hop = dx.saturating_add(dy) < 900 && sequence % 4 == 0;
                    return Some(MotionGoal {
                        target_x: x,
                        target_y: y,
                        mode: if short_hop {
                            MotionMode::Jump
                        } else {
                            MotionMode::Walk
                        },
                    });
                }
            }
            Some(plan_wander_from_snapshot(request, snapshot, now_ms))
        }
        Some(snapshot) if matches!(snapshot.freshness, Freshness::Fresh) => {
            // Freshness says Fresh but lease may have lapsed — free wander only.
            Some(plan_wander(request, &[]))
        }
        _ => None,
    }
}

/// Deterministic spring (or jump) frame sequence toward a target origin.
///
/// Positions are produced by spring-damper integration or parabolic jump samples —
/// never by linear interpolation of the remaining distance.
#[must_use]
pub fn spring_position_frames(
    start_x: i32,
    start_y: i32,
    target_x: i32,
    target_y: i32,
    mode: MotionMode,
    work_area: WorkArea,
    pet: Size,
) -> Vec<MotionFrame> {
    match mode {
        MotionMode::Jump => jump_position_frames(start_x, start_y, target_x, target_y),
        MotionMode::Walk | MotionMode::Perch => {
            spring_walk_frames(start_x, start_y, target_x, target_y, work_area, pet)
        }
    }
}

/// Spring frames that may cross displays: bounds use the union of work areas when
/// multi-monitor topology is known, so bounce does not trap the pet on one screen.
#[must_use]
pub fn spring_position_frames_multi_display(
    start_x: i32,
    start_y: i32,
    target_x: i32,
    target_y: i32,
    mode: MotionMode,
    displays: &[DesktopDisplay],
    fallback: WorkArea,
    pet: Size,
) -> Vec<MotionFrame> {
    let work_area = union_work_areas(displays).unwrap_or(fallback);
    spring_position_frames(start_x, start_y, target_x, target_y, mode, work_area, pet)
}

/// Plans a cross-display walk target and returns spring frames toward it.
///
/// Returns `None` when a multi-monitor destination cannot be chosen.
#[must_use]
pub fn plan_cross_display_walk_frames(
    sequence: u64,
    start_x: i32,
    start_y: i32,
    pet_w: u32,
    pet_h: u32,
    displays: &[DesktopDisplay],
    fallback: WorkArea,
) -> Option<Vec<MotionFrame>> {
    let (target_x, target_y) =
        plan_cross_display_wander_target(sequence, start_x, start_y, pet_w, pet_h, displays)?;
    let pet = Size {
        width: pet_w,
        height: pet_h,
    };
    Some(spring_position_frames_multi_display(
        start_x,
        start_y,
        target_x,
        target_y,
        MotionMode::Walk,
        displays,
        fallback,
        pet,
    ))
}

fn jump_position_frames(
    start_x: i32,
    start_y: i32,
    target_x: i32,
    target_y: i32,
) -> Vec<MotionFrame> {
    let max_steps = ((SPRING_MAX_SECONDS / SPRING_DT).ceil() as usize).clamp(8, 48);
    jump_parabola(
        f64::from(start_x),
        f64::from(start_y),
        f64::from(target_x),
        f64::from(target_y),
        JUMP_PEAK_HEIGHT,
        max_steps,
    )
    .into_iter()
    .map(|sample| MotionFrame {
        x: round_coord(sample.x),
        y: round_coord(sample.y),
    })
    .collect()
}

fn spring_walk_frames(
    start_x: i32,
    start_y: i32,
    target_x: i32,
    target_y: i32,
    work_area: WorkArea,
    pet: Size,
) -> Vec<MotionFrame> {
    let max_steps = ((SPRING_MAX_SECONDS / SPRING_DT).ceil() as u32).clamp(8, 48);
    let params = SpringParams::default();
    let area = Rect {
        x: work_area.x,
        y: work_area.y,
        width: work_area.width,
        height: work_area.height,
    };
    let (min_x, min_y, max_x, max_y) = safe_origin_bounds(area, pet);
    let mut state = SpringState::at_rest(f64::from(start_x), f64::from(start_y));
    let target_x_f = f64::from(target_x);
    let target_y_f = f64::from(target_y);
    let mut frames = Vec::with_capacity(max_steps as usize);
    for _ in 0..max_steps {
        state = integrate(state, target_x_f, target_y_f, params, SPRING_DT);
        let collision = bounce_on_bounds(
            state,
            min_x as f64,
            min_y as f64,
            max_x as f64,
            max_y as f64,
            0.35,
            220.0,
        );
        state = collision.state;
        frames.push(MotionFrame {
            x: round_coord(state.x),
            y: round_coord(state.y),
        });
        let position_error = (state.x - target_x_f).hypot(state.y - target_y_f);
        if position_error <= 0.75 && state.speed() <= 1.0 {
            break;
        }
    }
    if frames.is_empty() {
        frames.push(MotionFrame {
            x: target_x,
            y: target_y,
        });
    }
    frames
}

#[allow(clippy::cast_possible_truncation)]
fn round_coord(value: f64) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    value.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32
}

/// Maps platform meeting hints that include Webex into context hints.
#[must_use]
pub fn map_meeting_hint_label(label: &str) -> MeetingHint {
    match label {
        "zoom" => MeetingHint::Zoom,
        "teams" => MeetingHint::Teams,
        "meet" => MeetingHint::Meet,
        "webex" => MeetingHint::Webex,
        "unknown" => MeetingHint::Unknown,
        _ => MeetingHint::None,
    }
}


/// Returns true when a runtime event type is a connector sensory stimulus.
///
/// Accepts namespaced `connector.*` types (three+ segments) so automation and
/// skill event sessions can drive [`ConnectorSenseKind::EventReceived`] without
/// treating every bus event as a pet alert.
#[must_use]
pub fn is_connector_event_type(event_type: &str) -> bool {
    let mut parts = event_type.split('.');
    matches!(parts.next(), Some("connector"))
        && parts.next().is_some_and(|segment| !segment.is_empty())
        && parts.next().is_some_and(|segment| !segment.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimora_desktop_context::MotionMode;

    fn stage() -> OverlayStage {
        OverlayStage {
            origin_x: 0,
            origin_y: 0,
            width: 1280,
            height: 800,
        }
    }

    fn dual_displays() -> Vec<DesktopDisplay> {
        vec![
            DesktopDisplay {
                id: "a".into(),
                x: 0,
                y: 0,
                width: 1280,
                height: 800,
                work_area: WorkArea {
                    x: 0,
                    y: 0,
                    width: 1280,
                    height: 800,
                },
                scale_factor: 1.0,
            },
            DesktopDisplay {
                id: "b".into(),
                x: 1280,
                y: 0,
                width: 1280,
                height: 800,
                work_area: WorkArea {
                    x: 1280,
                    y: 0,
                    width: 1280,
                    height: 800,
                },
                scale_factor: 1.0,
            },
        ]
    }

    #[test]
    fn overlay_stage_from_work_area_maps_origin() {
        let stage = overlay_stage_from_work_area(HostWorkArea {
            x: 100,
            y: 40,
            width: 1600,
            height: 900,
        });
        assert_eq!(stage.origin_x, 100);
        assert_eq!(stage.origin_y, 40);
        assert_eq!(stage.width, 1600);
        assert_eq!(stage.height, 900);
    }

    #[test]
    fn pose_for_local_round_trips() {
        let stage = stage();
        let pose = pose_for_local(40, 80, stage);
        assert_eq!(pose.local_x, 40);
        assert_eq!(pose.local_y, 80);
        assert_eq!(pose.x, stage.origin_x + 40);
        assert_eq!(pose.y, stage.origin_y + 80);
    }

    #[test]
    fn pose_for_screen_clamps_and_localizes() {
        let stage = stage();
        let pose = pose_for_screen(-40, 10_000, stage);
        assert!(pose.x >= stage.origin_x);
        assert!(pose.y + PET_BODY_HEIGHT as i32 <= stage.origin_y + stage.height as i32);
        assert_eq!(pose.local_x, pose.x - stage.origin_x);
        assert_eq!(pose.local_y, pose.y - stage.origin_y);
    }

    #[test]
    fn hit_test_detects_pet_body() {
        let stage = stage();
        let pose = pose_for_screen(200, 400, stage);
        let body = Size {
            width: PET_BODY_WIDTH,
            height: PET_BODY_HEIGHT,
        };
        assert!(cursor_hits_pet(
            pose.x + 10,
            pose.y + 10,
            pose.x,
            pose.y,
            body,
            0,
        ));
        assert!(!cursor_hits_pet(
            pose.x - 20,
            pose.y - 20,
            pose.x,
            pose.y,
            body,
            0,
        ));
    }

    #[test]
    fn multi_monitor_stage_follows_pet_center() {
        let displays = dual_displays();
        let fallback = HostWorkArea {
            x: 0,
            y: 0,
            width: 1280,
            height: 800,
        };
        let left = overlay_stage_for_pet(100, 300, PET_BODY_WIDTH, PET_BODY_HEIGHT, &displays, fallback);
        let right = overlay_stage_for_pet(1500, 300, PET_BODY_WIDTH, PET_BODY_HEIGHT, &displays, fallback);
        assert_eq!(left.origin_x, 0);
        assert_eq!(right.origin_x, 1280);
        assert!(pet_crosses_display_boundary(
            100,
            300,
            1500,
            300,
            PET_BODY_WIDTH,
            PET_BODY_HEIGHT,
            &displays
        ));
    }

    #[test]
    fn cross_display_target_picks_other_monitor() {
        let displays = dual_displays();
        let target = plan_cross_display_wander_target(0, 100, 400, 260, 300, &displays)
            .expect("cross target");
        assert!(
            target.0 >= 1280,
            "should land on second display, got {target:?}"
        );
        assert!(pet_crosses_display_boundary(
            100, 400, target.0, target.1, 260, 300, &displays
        ));
    }

    #[test]
    fn spring_frames_are_deterministic_and_non_linear() {
        let work = WorkArea {
            x: 0,
            y: 0,
            width: 1280,
            height: 800,
        };
        let pet = Size {
            width: PET_BODY_WIDTH,
            height: PET_BODY_HEIGHT,
        };
        let first = spring_position_frames(10, 20, 200, 80, MotionMode::Walk, work, pet);
        let second = spring_position_frames(10, 20, 200, 80, MotionMode::Walk, work, pet);
        assert_eq!(first, second);
        assert!(!first.is_empty());
        if first.len() >= 3 {
            let dx01 = first[1].x - first[0].x;
            let dx12 = first[2].x - first[1].x;
            // Spring steps are not forced equal linear increments.
            assert!(dx01 != 0 || dx12 != 0);
        }
    }

    #[test]
    fn jump_frames_rise_above_endpoints() {
        let work = WorkArea {
            x: 0,
            y: 0,
            width: 1280,
            height: 800,
        };
        let pet = Size {
            width: 260,
            height: 300,
        };
        let frames = spring_position_frames(0, 100, 200, 100, MotionMode::Jump, work, pet);
        let min_y = frames.iter().map(|f| f.y).min().unwrap();
        assert!(min_y < 100, "jump should arc upward (smaller y), min_y={min_y}");
    }

    #[test]
    fn is_connector_event_type_accepts_namespaced() {
        assert!(is_connector_event_type("connector.message.received"));
        assert!(is_connector_event_type("connector.mail.arrived"));
        assert!(!is_connector_event_type("connector.message"));
        assert!(!is_connector_event_type("pet.state.changed"));
        assert!(!is_connector_event_type(""));
        assert!(!is_connector_event_type("connector..bad"));
    }


    #[test]
    fn crowding_and_behavior_hints_fail_closed() {
        assert_eq!(crowding_from_window_count(2), CrowdingLevel::Low);
        assert_eq!(crowding_from_window_count(8), CrowdingLevel::Medium);
        assert_eq!(crowding_from_window_count(20), CrowdingLevel::High);
        let hints = behavior_hints_from_lifeform(None, 1_000, true);
        assert!(hints.suppress_autonomy);
        assert_eq!(hints.idle_ms, 0);
    }

    #[test]
    fn map_meeting_labels_include_webex() {
        assert_eq!(map_meeting_hint_label("webex"), MeetingHint::Webex);
        assert_eq!(map_meeting_hint_label("zoom"), MeetingHint::Zoom);
        assert_eq!(map_meeting_hint_label("other"), MeetingHint::None);
    }

    #[test]
    fn default_ground_pose_keeps_body_visible() {
        let stage = stage();
        let (x, y) = default_ground_pose(stage);
        let pose = pose_for_screen(x, y, stage);
        assert!(pose.local_x >= 0);
        assert!(pose.local_y >= 0);
        assert!(pose.local_x as u32 + PET_BODY_WIDTH <= stage.width);
        assert!(pose.local_y as u32 + PET_BODY_HEIGHT <= stage.height);
    }

    #[test]
    fn sample_marks_fullscreen_and_sanitizes_idle() {
        let displays = dual_displays();
        let sample = LifeformSampleInput {
            windows: vec![
                LifeformWindowInput {
                    id: "full".into(),
                    x: 0,
                    y: 0,
                    width: 1280,
                    height: 800,
                    z_order: 0,
                    owner_pid: 9,
                    owner_name: "Video".into(),
                    onscreen: true,
                    is_minimized: false,
                    is_shell: false,
                },
                LifeformWindowInput {
                    id: "zero".into(),
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 10,
                    z_order: 1,
                    owner_pid: 10,
                    owner_name: "Ghost".into(),
                    onscreen: true,
                    is_minimized: false,
                    is_shell: false,
                },
            ],
            foreground: Some(LifeformForegroundInput {
                app_name: "Video".into(),
                pid: 9,
            }),
            idle_ms: u64::MAX,
            power: Some(PowerState {
                on_battery: true,
                battery_percent: Some(9),
                charging: false,
            }),
            meeting_active: false,
            meeting_hint: MeetingHint::None,
            observed_at_ms: 1_000,
            displays: displays.clone(),
            cursor: None,
        };
        let snap = lifeform_snapshot_from_sample(sample, 16_000).expect("snap");
        assert_eq!(snap.windows.len(), 1);
        assert!(snap.windows[0].is_fullscreen_candidate);
        assert!(snap.idle_ms < u64::MAX);
        assert_eq!(lifeform_work_area_stages(&displays).len(), 2);
        let band = lifeform_battery_sensory_band(snap.power.as_ref());
        assert_eq!(band, 1);
        assert!(lifeform_battery_should_emit(0, band, 0, 1_000));
    }

    #[test]
    fn cross_display_walk_frames_reach_other_monitor() {
        let displays = dual_displays();
        let fallback = WorkArea {
            x: 0,
            y: 0,
            width: 1280,
            height: 800,
        };
        let frames = plan_cross_display_walk_frames(0, 100, 400, 260, 300, &displays, fallback)
            .expect("frames");
        assert!(!frames.is_empty());
        let last = frames.last().copied().expect("last");
        // Final spring pose should be nearer the second display than the start.
        assert!(last.x > 100, "expected eastward walk, last={last:?}");
    }

    #[test]
    fn sensory_idle_and_meeting_throttle_helpers() {
        assert_eq!(lifeform_idle_sensory_band(0), 0);
        assert_eq!(lifeform_idle_sensory_band_from_ms(90_000), 1);
        assert!(lifeform_idle_should_emit(0, 1));
        assert!(!lifeform_idle_should_emit(2, 1));
        assert!(lifeform_meeting_should_emit(false, true));
        let gate = LifeformBooleanSensorGate::new(10);
        let (gate, emit) = gate.observe(true, 0);
        assert_eq!(emit, None);
        let (_gate, emit) = gate.observe(true, 10);
        assert_eq!(emit, Some(true));
    }

    #[test]
    fn pet_occlusion_for_pose_emits_strips_and_fail_closed() {
        // Unusable / missing snapshot → empty.
        assert_eq!(
            pet_occlusion_for_pose(0, 0, None, 1_000, 1).coverage,
            0.0
        );

        let displays = dual_displays();
        let sample = LifeformSampleInput {
            windows: vec![LifeformWindowInput {
                id: "cover".into(),
                x: 0,
                y: 0,
                width: 200,
                height: 150,
                z_order: 2,
                owner_pid: 77,
                owner_name: "Notes".into(),
                onscreen: true,
                is_minimized: false,
                is_shell: false,
            }],
            foreground: None,
            idle_ms: 0,
            power: None,
            meeting_active: false,
            meeting_hint: MeetingHint::None,
            observed_at_ms: 1_000,
            displays,
            cursor: None,
        };
        let snap = lifeform_snapshot_from_sample(sample, 16_000).expect("snap");
        let occ = pet_occlusion_for_pose(0, 0, Some(&snap), 1_000, 1);
        assert!(occ.coverage > 0.0, "coverage={}", occ.coverage);
        assert!(!occ.strips.is_empty(), "expected occlusion strips");
        // Expired lease fail-closed.
        let expired = pet_occlusion_for_pose(0, 0, Some(&snap), 100_000, 1);
        assert_eq!(expired.coverage, 0.0);
        assert!(expired.strips.is_empty());
    }

    #[test]
    fn multi_display_spring_uses_union_bounds() {
        let displays = dual_displays();
        let fallback = WorkArea {
            x: 0,
            y: 0,
            width: 1280,
            height: 800,
        };
        let pet = Size {
            width: 260,
            height: 300,
        };
        let frames = spring_position_frames_multi_display(
            100,
            400,
            1600,
            420,
            MotionMode::Walk,
            &displays,
            fallback,
            pet,
        );
        assert!(!frames.is_empty());
        // Cross-display walk should progress toward the second monitor.
        let last = frames.last().copied().expect("last");
        assert!(last.x > 100, "last={last:?}");
        let union = lifeform_union_work_area(&displays).expect("union");
        assert_eq!(union.width, 2560);
    }

    #[test]
    fn connector_event_type_requires_three_segments() {
        assert!(is_connector_event_type("connector.github.push"));
        assert!(!is_connector_event_type("connector"));
        assert!(!is_connector_event_type("connector.github"));
        assert!(!is_connector_event_type("skill.worker.busy"));
    }
}
