//! High-level motion goals for the desktop host.

use crate::obstacles::{
    avoid_obstacles, clamp_pet_origin, free_wander_target, AvoidRequest, Rect, Size,
};
use crate::snapshot::{CursorPosition, DesktopSnapshot, WorkArea};
use crate::{is_usable, obstacles_usable};
use serde::{Deserialize, Serialize};

/// Integer size of the pet surface.
pub type Size2 = Size;

/// Integer 2D point (origin of the pet window).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Vec2i {
    pub x: i32,
    pub y: i32,
}

/// How the host should interpret the motion goal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MotionMode {
    Walk,
    Jump,
    Perch,
}

/// Target the host feeds into the spring integrator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MotionGoal {
    pub target_x: i32,
    pub target_y: i32,
    pub mode: MotionMode,
}

/// Inputs for [`plan_wander`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WanderRequest {
    pub sequence: u64,
    pub current: Vec2i,
    pub pet_size: Size2,
    pub work_area: WorkArea,
    pub cursor: Option<CursorPosition>,
}

fn work_area_to_rect(area: WorkArea) -> Rect {
    Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: area.height,
    }
}

fn select_mode(sequence: u64, target_y: i32, work_area: Rect, pet: Size) -> MotionMode {
    let (_, min_y, _, max_y) = crate::obstacles::safe_origin_bounds(work_area, pet);
    if sequence.is_multiple_of(7) {
        return MotionMode::Jump;
    }
    if i64::from(target_y) >= max_y.saturating_sub(2) || i64::from(target_y) <= min_y + 2 {
        return MotionMode::Perch;
    }
    MotionMode::Walk
}

/// Plans a wander goal inside `work_area`, optionally avoiding obstacles.
///
/// When `cursor` is present and inside the work area, the planner takes a small
/// step toward the cursor while keeping clearance. For snapshot-driven planning
/// prefer [`plan_wander_from_snapshot`], which fail-closes on unusable samples.
#[must_use]
pub fn plan_wander(request: WanderRequest, obstacles: &[Rect]) -> MotionGoal {
    let area = work_area_to_rect(request.work_area);
    let (current_x, current_y) =
        clamp_pet_origin(request.current.x, request.current.y, request.pet_size, area);

    let mut desired = if let Some(cursor) = request
        .cursor
        .filter(|position| position.x.is_finite() && position.y.is_finite())
    {
        cursor_approach_target(current_x, current_y, request.pet_size, area, cursor)
            .unwrap_or_else(|| {
                free_wander_target(
                    current_x,
                    current_y,
                    request.pet_size,
                    area,
                    request.sequence,
                )
            })
    } else {
        free_wander_target(
            current_x,
            current_y,
            request.pet_size,
            area,
            request.sequence,
        )
    };

    desired = avoid_obstacles(
        AvoidRequest {
            current_x,
            current_y,
            target_x: desired.0,
            target_y: desired.1,
            pet: request.pet_size,
            work_area: area,
            sequence: request.sequence,
        },
        obstacles,
    );

    let mode = select_mode(request.sequence, desired.1, area, request.pet_size);
    MotionGoal {
        target_x: desired.0,
        target_y: desired.1,
        mode,
    }
}

/// Snapshot-aware planner: fail-closed to free wander when sample is not usable.
#[must_use]
pub fn plan_wander_from_snapshot(
    request: WanderRequest,
    snapshot: &DesktopSnapshot,
    now_ms: u64,
) -> MotionGoal {
    let obstacles = if obstacles_usable(snapshot, now_ms) {
        snapshot
            .obstacle_windows()
            .into_iter()
            .map(work_area_to_rect)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let cursor = if is_usable(snapshot, now_ms) {
        snapshot.cursor
    } else {
        None
    };
    plan_wander(
        WanderRequest {
            cursor,
            ..request
        },
        &obstacles,
    )
}

fn cursor_approach_target(
    current_x: i32,
    current_y: i32,
    pet: Size,
    work_area: Rect,
    cursor: CursorPosition,
) -> Option<(i32, i32)> {
    const HORIZONTAL_STEP: i32 = 140;
    const VERTICAL_STEP: i32 = 96;
    const CURSOR_CLEARANCE: f64 = 96.0;
    const ARRIVAL_TOLERANCE: f64 = 24.0;

    let monitor_right = f64::from(work_area.x) + f64::from(work_area.width);
    let monitor_bottom = f64::from(work_area.y) + f64::from(work_area.height);
    if cursor.x < f64::from(work_area.x)
        || cursor.x >= monitor_right
        || cursor.y < f64::from(work_area.y)
        || cursor.y >= monitor_bottom
    {
        return None;
    }

    let half_width = f64::from(pet.width) / 2.0;
    let half_height = f64::from(pet.height) / 2.0;
    let current_center_x = f64::from(current_x) + half_width;
    let current_center_y = f64::from(current_y) + half_height;
    let delta_x = cursor.x - current_center_x;
    let delta_y = cursor.y - current_center_y;
    let distance = delta_x.hypot(delta_y);
    let safe_center_distance = half_width.hypot(half_height) + CURSOR_CLEARANCE;
    if !distance.is_finite() || distance <= safe_center_distance + ARRIVAL_TOLERANCE {
        return None;
    }

    let remaining_distance = distance - safe_center_distance;
    let movement_ratio = (remaining_distance / distance).clamp(0.0, 1.0);
    let movement_x =
        (delta_x * movement_ratio).clamp(-f64::from(HORIZONTAL_STEP), f64::from(HORIZONTAL_STEP));
    let movement_y =
        (delta_y * movement_ratio).clamp(-f64::from(VERTICAL_STEP), f64::from(VERTICAL_STEP));
    let (min_x, min_y, max_x, max_y) = crate::obstacles::safe_origin_bounds(work_area, pet);
    let step_x = rounded_bounded_step(movement_x, HORIZONTAL_STEP);
    let step_y = rounded_bounded_step(movement_y, VERTICAL_STEP);
    let target_x = i64::from(current_x)
        .saturating_add(i64::from(step_x))
        .clamp(min_x, max_x);
    let target_y = i64::from(current_y)
        .saturating_add(i64::from(step_y))
        .clamp(min_y, max_y);
    let next_x = i32::try_from(target_x).ok()?;
    let next_y = i32::try_from(target_y).ok()?;
    if next_x == current_x && next_y == current_y {
        return None;
    }
    Some((next_x, next_y))
}

#[allow(clippy::cast_possible_truncation)]
fn rounded_bounded_step(value: f64, maximum_magnitude: i32) -> i32 {
    value
        .round()
        .clamp(-f64::from(maximum_magnitude), f64::from(maximum_magnitude)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{
        DegradationReason, DesktopSnapshotParts, DesktopWindow, Freshness, MeetingHint,
        MeetingState,
    };

    fn work() -> WorkArea {
        WorkArea {
            x: 0,
            y: 0,
            width: 1400,
            height: 900,
        }
    }

    #[test]
    fn plan_wander_stays_in_bounds() {
        let pet = Size {
            width: 100,
            height: 100,
        };
        let goal = plan_wander(
            WanderRequest {
                sequence: 3,
                current: Vec2i { x: 200, y: 200 },
                pet_size: pet,
                work_area: work(),
                cursor: None,
            },
            &[],
        );
        let (cx, cy) =
            clamp_pet_origin(goal.target_x, goal.target_y, pet, work_area_to_rect(work()));
        assert_eq!((goal.target_x, goal.target_y), (cx, cy));
        assert!(matches!(
            goal.mode,
            MotionMode::Walk | MotionMode::Jump | MotionMode::Perch
        ));
    }

    #[test]
    fn stale_snapshot_does_not_use_obstacles() {
        let pet = Size {
            width: 80,
            height: 80,
        };
        let snapshot = DesktopSnapshot::new(DesktopSnapshotParts {
            windows: vec![DesktopWindow {
                id: "block".into(),
                title_hash: None,
                title_redacted: true,
                x: 0,
                y: 0,
                width: 1400,
                height: 900,
                z_order: 1,
                owner_pid: 1,
                owner_name: "Blocker".into(),
                onscreen: true,
                is_minimized: false,
                is_fullscreen_candidate: false,
                is_shell: false,
            }],
            foreground: None,
            displays: Vec::new(),
            power: None,
            idle_ms: 0,
            meeting: MeetingState {
                active: false,
                hint: MeetingHint::None,
            },
            cursor: None,
            observed_at_ms: 1_000,
            expires_at_ms: 6_000,
            freshness: Freshness::Stale,
            degradation_reason: None,
        })
        .expect("snapshot");
        assert!(!is_usable(&snapshot, 1_500));

        let request = WanderRequest {
            sequence: 0,
            current: Vec2i { x: 40, y: 40 },
            pet_size: pet,
            work_area: work(),
            cursor: None,
        };
        let free = plan_wander(request, &[]);
        let from_stale = plan_wander_from_snapshot(request, &snapshot, 1_500);
        assert_eq!(
            (from_stale.target_x, from_stale.target_y),
            (free.target_x, free.target_y),
            "stale must fail-closed to free wander"
        );

        let mut degraded = snapshot;
        degraded.freshness = Freshness::Degraded;
        degraded.degradation_reason = Some(DegradationReason::PermissionDenied);
        let from_degraded = plan_wander_from_snapshot(request, &degraded, 1_500);
        assert_eq!(
            (from_degraded.target_x, from_degraded.target_y),
            (free.target_x, free.target_y)
        );
    }

    #[test]
    fn cursor_approach_stays_in_bounds() {
        let pet = Size {
            width: 100,
            height: 100,
        };
        let goal = plan_wander(
            WanderRequest {
                sequence: 1,
                current: Vec2i { x: 100, y: 100 },
                pet_size: pet,
                work_area: work(),
                cursor: Some(CursorPosition {
                    x: 900.0,
                    y: 500.0,
                }),
            },
            &[],
        );
        let (cx, cy) =
            clamp_pet_origin(goal.target_x, goal.target_y, pet, work_area_to_rect(work()));
        assert_eq!((goal.target_x, goal.target_y), (cx, cy));
    }

    #[test]
    fn jump_mode_every_seventh_sequence() {
        let pet = Size {
            width: 80,
            height: 80,
        };
        let goal = plan_wander(
            WanderRequest {
                sequence: 7,
                current: Vec2i { x: 200, y: 300 },
                pet_size: pet,
                work_area: work(),
                cursor: None,
            },
            &[],
        );
        assert_eq!(goal.mode, MotionMode::Jump);
    }

    #[test]
    fn fresh_snapshot_uses_obstacles() {
        let pet = Size {
            width: 80,
            height: 80,
        };
        let work_area = work();
        let obstacle = Rect {
            x: 0,
            y: 0,
            width: 1400,
            height: 900,
        };
        let snapshot = DesktopSnapshot::new(DesktopSnapshotParts {
            windows: vec![DesktopWindow {
                id: "block".into(),
                title_hash: None,
                title_redacted: true,
                x: obstacle.x,
                y: obstacle.y,
                width: obstacle.width,
                height: obstacle.height,
                z_order: 1,
                owner_pid: 1,
                owner_name: "Blocker".into(),
                onscreen: true,
                is_minimized: false,
                is_fullscreen_candidate: true,
                is_shell: false,
            }],
            foreground: None,
            displays: Vec::new(),
            power: None,
            idle_ms: 0,
            meeting: MeetingState {
                active: false,
                hint: MeetingHint::None,
            },
            cursor: None,
            observed_at_ms: 1_000,
            expires_at_ms: 6_000,
            freshness: Freshness::Fresh,
            degradation_reason: None,
        })
        .expect("snapshot");
        let request = WanderRequest {
            sequence: 0,
            current: Vec2i { x: 40, y: 40 },
            pet_size: pet,
            work_area,
            cursor: None,
        };
        let free = plan_wander(request, &[]);
        let with_obs = plan_wander_from_snapshot(request, &snapshot, 1_500);
        // Large fullscreen obstacle should rewrite the free target when usable.
        assert_ne!(
            (with_obs.target_x, with_obs.target_y),
            (free.target_x, free.target_y)
        );
    }
}
