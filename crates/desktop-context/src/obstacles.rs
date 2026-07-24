//! Pure obstacle avoidance for desktop pet wander targets.

use serde::{Deserialize, Serialize};

/// Integer size of the pet window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

/// Axis-aligned rectangle in physical coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    #[must_use]
    pub fn right(&self) -> i64 {
        i64::from(self.x).saturating_add(i64::from(self.width))
    }

    #[must_use]
    pub fn bottom(&self) -> i64 {
        i64::from(self.y).saturating_add(i64::from(self.height))
    }

    #[must_use]
    pub fn contains_origin(&self, origin_x: i32, origin_y: i32, pet: Size) -> bool {
        let pet_rect = Rect {
            x: origin_x,
            y: origin_y,
            width: pet.width,
            height: pet.height,
        };
        rects_overlap(self, &pet_rect)
    }

    #[must_use]
    pub fn area(&self) -> u64 {
        u64::from(self.width).saturating_mul(u64::from(self.height))
    }
}

/// Inputs for obstacle-aware target adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AvoidRequest {
    pub current_x: i32,
    pub current_y: i32,
    pub target_x: i32,
    pub target_y: i32,
    pub pet: Size,
    pub work_area: Rect,
    pub sequence: u64,
}

/// Returns true when two axis-aligned rects have positive area overlap.
#[must_use]
pub fn rects_overlap(a: &Rect, b: &Rect) -> bool {
    let left = i64::from(a.x).max(i64::from(b.x));
    let top = i64::from(a.y).max(i64::from(b.y));
    let right = a.right().min(b.right());
    let bottom = a.bottom().min(b.bottom());
    right > left && bottom > top
}

/// Safe pet origin bounds inside a work area (top-left of pet).
#[must_use]
pub fn safe_origin_bounds(work_area: Rect, pet: Size) -> (i64, i64, i64, i64) {
    const HORIZONTAL_MARGIN: i64 = 16;
    const TOP_MARGIN: i64 = 24;
    const BOTTOM_MARGIN: i64 = 48;
    let minimum_x = i64::from(work_area.x).saturating_add(HORIZONTAL_MARGIN);
    let minimum_y = i64::from(work_area.y).saturating_add(TOP_MARGIN);
    let maximum_x = i64::from(work_area.x)
        .saturating_add(i64::from(work_area.width))
        .saturating_sub(i64::from(pet.width))
        .saturating_sub(HORIZONTAL_MARGIN)
        .max(minimum_x);
    let maximum_y = i64::from(work_area.y)
        .saturating_add(i64::from(work_area.height))
        .saturating_sub(i64::from(pet.height))
        .saturating_sub(BOTTOM_MARGIN)
        .max(minimum_y);
    (minimum_x, minimum_y, maximum_x, maximum_y)
}

/// Clamps a pet origin into the safe work-area range.
#[must_use]
pub fn clamp_pet_origin(origin_x: i32, origin_y: i32, pet: Size, work_area: Rect) -> (i32, i32) {
    let (min_x, min_y, max_x, max_y) = safe_origin_bounds(work_area, pet);
    let x = i64::from(origin_x).clamp(min_x, max_x);
    let y = i64::from(origin_y).clamp(min_y, max_y);
    (
        i32::try_from(x).unwrap_or(origin_x),
        i32::try_from(y).unwrap_or(origin_y),
    )
}

/// Deterministic free wander step inside the work area (no obstacles).
#[must_use]
pub fn free_wander_target(
    current_x: i32,
    current_y: i32,
    pet: Size,
    work_area: Rect,
    sequence: u64,
) -> (i32, i32) {
    const HORIZONTAL_STEP: i64 = 140;
    const VERTICAL_STEP: i64 = 32;
    let (min_x, min_y, max_x, max_y) = safe_origin_bounds(work_area, pet);
    let direction = if sequence.is_multiple_of(2) { 1 } else { -1 };
    let vertical_direction = if (sequence / 2).is_multiple_of(2) {
        1
    } else {
        -1
    };
    let x = i64::from(current_x)
        .saturating_add(HORIZONTAL_STEP * direction)
        .clamp(min_x, max_x);
    let y = i64::from(current_y)
        .saturating_add(VERTICAL_STEP * vertical_direction)
        .clamp(min_y, max_y);
    (
        i32::try_from(x).unwrap_or(current_x),
        i32::try_from(y).unwrap_or(current_y),
    )
}

fn pet_rect_at(origin_x: i32, origin_y: i32, pet: Size) -> Rect {
    Rect {
        x: origin_x,
        y: origin_y,
        width: pet.width,
        height: pet.height,
    }
}

fn blocked_by_any(origin_x: i32, origin_y: i32, pet: Size, obstacles: &[Rect]) -> bool {
    let pet_rect = pet_rect_at(origin_x, origin_y, pet);
    obstacles
        .iter()
        .filter(|obstacle| obstacle.area() >= 20_000)
        .any(|obstacle| rects_overlap(obstacle, &pet_rect))
}

/// Segment from current pet origin to target origin intersects a large obstacle.
fn path_blocked(
    current_x: i32,
    current_y: i32,
    target_x: i32,
    target_y: i32,
    pet: Size,
    obstacles: &[Rect],
) -> bool {
    let samples = 8_u32;
    for step in 0..=samples {
        let t = f64::from(step) / f64::from(samples);
        let sample_x = f64::from(current_x) + (f64::from(target_x) - f64::from(current_x)) * t;
        let sample_y = f64::from(current_y) + (f64::from(target_y) - f64::from(current_y)) * t;
        let x = f64_to_i32(sample_x.round());
        let y = f64_to_i32(sample_y.round());
        if blocked_by_any(x, y, pet, obstacles) {
            return true;
        }
    }
    false
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn f64_to_i32(value: f64) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    value.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

fn candidate_perches(obstacle: &Rect, pet: Size, work_area: Rect) -> Vec<(i32, i32)> {
    let (min_x, min_y, max_x, max_y) = safe_origin_bounds(work_area, pet);
    let gap: i64 = 8;
    let left = i64::from(obstacle.x)
        .saturating_sub(i64::from(pet.width))
        .saturating_sub(gap);
    let right = obstacle.right().saturating_add(gap);
    let top = i64::from(obstacle.y)
        .saturating_sub(i64::from(pet.height))
        .saturating_sub(gap);
    let bottom = obstacle.bottom().saturating_add(gap);
    let center_x = i64::midpoint(
        i64::from(obstacle.x),
        obstacle.right().saturating_sub(i64::from(pet.width)),
    );
    let center_y = i64::midpoint(
        i64::from(obstacle.y),
        obstacle.bottom().saturating_sub(i64::from(pet.height)),
    );
    let raw = [
        (left, center_y),
        (right, center_y),
        (center_x, top),
        (center_x, bottom),
        (left, top),
        (right, top),
        (left, bottom),
        (right, bottom),
    ];
    raw.into_iter()
        .map(|(x, y)| (x.clamp(min_x, max_x), y.clamp(min_y, max_y)))
        .filter_map(|(x, y)| {
            let origin_x = i32::try_from(x).ok()?;
            let origin_y = i32::try_from(y).ok()?;
            Some((origin_x, origin_y))
        })
        .collect()
}

fn edge_slide_target(request: AvoidRequest) -> (i32, i32) {
    let (min_x, min_y, max_x, max_y) = safe_origin_bounds(request.work_area, request.pet);
    let prefer_horizontal = request.sequence.is_multiple_of(2);
    if prefer_horizontal {
        let edge_y = if i64::from(request.current_y) - min_y
            <= max_y - i64::from(request.current_y)
        {
            min_y
        } else {
            max_y
        };
        let x = i64::from(request.target_x).clamp(min_x, max_x);
        (
            i32::try_from(x).unwrap_or(request.target_x),
            i32::try_from(edge_y).unwrap_or(request.current_y),
        )
    } else {
        let edge_x = if i64::from(request.current_x) - min_x
            <= max_x - i64::from(request.current_x)
        {
            min_x
        } else {
            max_x
        };
        let y = i64::from(request.target_y).clamp(min_y, max_y);
        (
            i32::try_from(edge_x).unwrap_or(request.current_x),
            i32::try_from(y).unwrap_or(request.target_y),
        )
    }
}

/// Adjusts a desired target so the pet avoids overlapping large onscreen windows.
///
/// Fail-closed: empty obstacles leaves the (clamped) target unchanged.
/// When the direct path or target is blocked, prefers a free perch near a window
/// edge; otherwise slides along the work-area boundary.
#[must_use]
pub fn avoid_obstacles(request: AvoidRequest, obstacles: &[Rect]) -> (i32, i32) {
    let (target_x, target_y) = clamp_pet_origin(
        request.target_x,
        request.target_y,
        request.pet,
        request.work_area,
    );
    if obstacles.is_empty() {
        return (target_x, target_y);
    }

    let large: Vec<Rect> = obstacles
        .iter()
        .copied()
        .filter(|obstacle| obstacle.area() >= 20_000)
        .collect();
    if large.is_empty() {
        return (target_x, target_y);
    }

    let target_clear = !blocked_by_any(target_x, target_y, request.pet, &large);
    let route_clear = !path_blocked(
        request.current_x,
        request.current_y,
        target_x,
        target_y,
        request.pet,
        &large,
    );
    if target_clear && route_clear {
        return (target_x, target_y);
    }

    let mut ranked = large;
    ranked.sort_by_key(|obstacle| std::cmp::Reverse(obstacle.area()));
    let mut best: Option<(i32, i32, i64)> = None;
    for obstacle in &ranked {
        for (perch_x, perch_y) in candidate_perches(obstacle, request.pet, request.work_area) {
            if blocked_by_any(perch_x, perch_y, request.pet, &ranked) {
                continue;
            }
            if path_blocked(
                request.current_x,
                request.current_y,
                perch_x,
                perch_y,
                request.pet,
                &ranked,
            ) {
                continue;
            }
            let dist = (i64::from(perch_x) - i64::from(request.current_x)).abs()
                + (i64::from(perch_y) - i64::from(request.current_y)).abs();
            match best {
                Some((_, _, best_dist)) if dist >= best_dist => {}
                _ => best = Some((perch_x, perch_y, dist)),
            }
        }
        if best.is_some() {
            break;
        }
    }
    if let Some((perch_x, perch_y, _)) = best {
        return (perch_x, perch_y);
    }

    let slid = edge_slide_target(AvoidRequest {
        target_x,
        target_y,
        ..request
    });
    if !blocked_by_any(slid.0, slid.1, request.pet, &ranked) {
        return slid;
    }

    clamp_pet_origin(
        request.current_x,
        request.current_y,
        request.pet,
        request.work_area,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_clamping_keeps_pet_inside_work_area() {
        let work = Rect {
            x: 0,
            y: 0,
            width: 1000,
            height: 800,
        };
        let pet = Size {
            width: 100,
            height: 100,
        };
        let (x, y) = clamp_pet_origin(-50, 900, pet, work);
        let (min_x, min_y, max_x, max_y) = safe_origin_bounds(work, pet);
        assert!((min_x..=max_x).contains(&i64::from(x)));
        assert!((min_y..=max_y).contains(&i64::from(y)));
    }

    #[test]
    fn empty_obstacles_fail_closed_to_free_target() {
        let work = Rect {
            x: 0,
            y: 0,
            width: 1200,
            height: 900,
        };
        let pet = Size {
            width: 96,
            height: 96,
        };
        let (x, y) = avoid_obstacles(
            AvoidRequest {
                current_x: 100,
                current_y: 100,
                target_x: 500,
                target_y: 400,
                pet,
                work_area: work,
                sequence: 0,
            },
            &[],
        );
        assert_eq!((x, y), clamp_pet_origin(500, 400, pet, work));
    }

    #[test]
    fn obstacle_avoid_changes_target_when_blocked() {
        let work = Rect {
            x: 0,
            y: 0,
            width: 1200,
            height: 900,
        };
        let pet = Size {
            width: 80,
            height: 80,
        };
        let obstacle = Rect {
            x: 200,
            y: 100,
            width: 800,
            height: 600,
        };
        let desired_x = 500;
        let desired_y = 400;
        let adjusted = avoid_obstacles(
            AvoidRequest {
                current_x: 40,
                current_y: 40,
                target_x: desired_x,
                target_y: desired_y,
                pet,
                work_area: work,
                sequence: 0,
            },
            &[obstacle],
        );
        assert_ne!(
            adjusted,
            clamp_pet_origin(desired_x, desired_y, pet, work),
            "blocked target should be rewritten"
        );
        assert!(
            !blocked_by_any(adjusted.0, adjusted.1, pet, &[obstacle]),
            "adjusted target must not overlap large obstacle: {adjusted:?}"
        );
    }
}
