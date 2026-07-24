//! Multi-monitor display helpers for host integration.
//!
//! Pure geometry over [`DesktopDisplay`] / [`WorkArea`] — no OS calls.

use crate::snapshot::{DesktopDisplay, WorkArea};

/// Returns the primary display when present (first entry; adapters put primary first).
#[must_use]
pub fn primary_display(displays: &[DesktopDisplay]) -> Option<&DesktopDisplay> {
    displays.first()
}

/// Work areas from every known display, in adapter order.
#[must_use]
pub fn work_areas(displays: &[DesktopDisplay]) -> Vec<WorkArea> {
    displays.iter().map(|display| display.work_area).collect()
}

/// True when the point lies inside the display's full bounds (not just work area).
#[must_use]
pub fn display_contains_point(display: &DesktopDisplay, x: f64, y: f64) -> bool {
    if !x.is_finite() || !y.is_finite() {
        return false;
    }
    let right = f64::from(display.x) + f64::from(display.width);
    let bottom = f64::from(display.y) + f64::from(display.height);
    x >= f64::from(display.x) && x < right && y >= f64::from(display.y) && y < bottom
}

/// True when the point lies inside a work area.
#[must_use]
pub fn work_area_contains_point(area: WorkArea, x: f64, y: f64) -> bool {
    if !x.is_finite() || !y.is_finite() || area.width == 0 || area.height == 0 {
        return false;
    }
    let right = f64::from(area.x) + f64::from(area.width);
    let bottom = f64::from(area.y) + f64::from(area.height);
    x >= f64::from(area.x) && x < right && y >= f64::from(area.y) && y < bottom
}

/// Display whose full bounds contain `(x, y)`, preferring the smallest area on ties.
#[must_use]
pub fn display_containing_point(
    displays: &[DesktopDisplay],
    x: f64,
    y: f64,
) -> Option<&DesktopDisplay> {
    displays
        .iter()
        .filter(|display| display_contains_point(display, x, y))
        .min_by_key(|display| {
            u64::from(display.width).saturating_mul(u64::from(display.height))
        })
}

/// Display whose full bounds contain integer origin `(x, y)`.
#[must_use]
pub fn display_containing_origin(
    displays: &[DesktopDisplay],
    x: i32,
    y: i32,
) -> Option<&DesktopDisplay> {
    display_containing_point(displays, f64::from(x), f64::from(y))
}

/// Work area for the display containing `(x, y)`, falling back to the primary work area.
#[must_use]
pub fn work_area_for_point(
    displays: &[DesktopDisplay],
    x: f64,
    y: f64,
) -> Option<WorkArea> {
    display_containing_point(displays, x, y)
        .map(|display| display.work_area)
        .or_else(|| primary_display(displays).map(|display| display.work_area))
}

/// Axis-aligned union of full display bounds, or `None` when empty / invalid.
#[must_use]
pub fn union_display_bounds(displays: &[DesktopDisplay]) -> Option<WorkArea> {
    let mut iter = displays.iter().filter(|d| d.width > 0 && d.height > 0);
    let first = iter.next()?;
    let mut left = i64::from(first.x);
    let mut top = i64::from(first.y);
    let mut right = left.saturating_add(i64::from(first.width));
    let mut bottom = top.saturating_add(i64::from(first.height));
    for display in iter {
        let d_left = i64::from(display.x);
        let d_top = i64::from(display.y);
        let d_right = d_left.saturating_add(i64::from(display.width));
        let d_bottom = d_top.saturating_add(i64::from(display.height));
        left = left.min(d_left);
        top = top.min(d_top);
        right = right.max(d_right);
        bottom = bottom.max(d_bottom);
    }
    let width = u32::try_from(right.saturating_sub(left)).ok().filter(|&w| w > 0)?;
    let height = u32::try_from(bottom.saturating_sub(top)).ok().filter(|&h| h > 0)?;
    let x = i32::try_from(left).ok()?;
    let y = i32::try_from(top).ok()?;
    Some(WorkArea {
        x,
        y,
        width,
        height,
    })
}

/// Marks whether `window` covers any display work area within `tolerance` pixels.
#[must_use]
pub fn is_fullscreen_over_any_display(
    window: WorkArea,
    displays: &[DesktopDisplay],
    tolerance: i32,
) -> bool {
    let tol = tolerance.max(0);
    displays.iter().any(|display| {
        rect_covers(window, display.work_area, tol) || rect_covers(window, display.bounds(), tol)
    })
}

fn rect_covers(window: WorkArea, target: WorkArea, tolerance: i32) -> bool {
    if target.width == 0 || target.height == 0 {
        return false;
    }
    let w_right = i64::from(window.x).saturating_add(i64::from(window.width));
    let w_bottom = i64::from(window.y).saturating_add(i64::from(window.height));
    let t_left = i64::from(target.x);
    let t_top = i64::from(target.y);
    let t_right = t_left.saturating_add(i64::from(target.width));
    let t_bottom = t_top.saturating_add(i64::from(target.height));
    let tol = i64::from(tolerance);
    i64::from(window.x) <= t_left.saturating_add(tol)
        && i64::from(window.y) <= t_top.saturating_add(tol)
        && w_right >= t_right.saturating_sub(tol)
        && w_bottom >= t_bottom.saturating_sub(tol)
}

impl DesktopDisplay {
    /// Full display bounds as a [`WorkArea`].
    #[must_use]
    pub const fn bounds(&self) -> WorkArea {
        WorkArea {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }
}

impl WorkArea {
    /// Area in square pixels.
    #[must_use]
    pub const fn area(&self) -> u64 {
        (self.width as u64).saturating_mul(self.height as u64)
    }

    /// Right edge (exclusive) as `i64`.
    #[must_use]
    pub fn right(&self) -> i64 {
        i64::from(self.x).saturating_add(i64::from(self.width))
    }

    /// Bottom edge (exclusive) as `i64`.
    #[must_use]
    pub fn bottom(&self) -> i64 {
        i64::from(self.y).saturating_add(i64::from(self.height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(id: &str, x: i32, y: i32, w: u32, h: u32, work_inset: i32) -> DesktopDisplay {
        DesktopDisplay {
            id: id.into(),
            x,
            y,
            width: w,
            height: h,
            work_area: WorkArea {
                x: x + work_inset,
                y: y + work_inset,
                width: w.saturating_sub((work_inset * 2) as u32),
                height: h.saturating_sub((work_inset * 2) as u32),
            },
            scale_factor: 1.0,
        }
    }

    #[test]
    fn primary_and_containing_prefer_smallest() {
        let displays = vec![
            display("main", 0, 0, 1920, 1080, 0),
            display("side", 1920, 0, 1280, 800, 0),
        ];
        assert_eq!(primary_display(&displays).map(|d| d.id.as_str()), Some("main"));
        let hit = display_containing_point(&displays, 2000.0, 10.0).expect("side");
        assert_eq!(hit.id, "side");
        assert!(work_area_for_point(&displays, -10.0, 0.0).is_some());
    }

    #[test]
    fn union_spans_negative_origins() {
        let displays = vec![
            display("left", -1920, 0, 1920, 1080, 0),
            display("right", 0, 0, 1920, 1080, 0),
        ];
        let union = union_display_bounds(&displays).expect("union");
        assert_eq!(union.x, -1920);
        assert_eq!(union.width, 3840);
        assert_eq!(union.height, 1080);
    }

    #[test]
    fn fullscreen_over_work_area() {
        let displays = vec![display("main", 0, 0, 1000, 800, 0)];
        let full = WorkArea {
            x: 0,
            y: 0,
            width: 1000,
            height: 800,
        };
        assert!(is_fullscreen_over_any_display(full, &displays, 2));
        let partial = WorkArea {
            x: 100,
            y: 100,
            width: 200,
            height: 200,
        };
        assert!(!is_fullscreen_over_any_display(partial, &displays, 2));
    }
}

/// Work-area stage rects for every display (multi-monitor stage candidates).
#[must_use]
pub fn work_area_stages(displays: &[DesktopDisplay]) -> Vec<WorkArea> {
    work_areas(displays)
        .into_iter()
        .filter(|area| area.width > 0 && area.height > 0)
        .collect()
}

/// Scale factor of the display containing `(x, y)`, else primary, else `1.0`.
#[must_use]
pub fn scale_factor_for_point(displays: &[DesktopDisplay], x: f64, y: f64) -> f64 {
    display_containing_point(displays, x, y)
        .or_else(|| primary_display(displays))
        .map(|display| {
            if display.scale_factor.is_finite() && display.scale_factor >= 0.5 {
                display.scale_factor.clamp(0.5, 4.0)
            } else {
                1.0
            }
        })
        .unwrap_or(1.0)
}

/// Union of all work areas (not full bounds), when valid.
#[must_use]
pub fn union_work_areas(displays: &[DesktopDisplay]) -> Option<WorkArea> {
    let mut iter = displays
        .iter()
        .map(|d| d.work_area)
        .filter(|a| a.width > 0 && a.height > 0);
    let first = iter.next()?;
    let mut left = i64::from(first.x);
    let mut top = i64::from(first.y);
    let mut right = left.saturating_add(i64::from(first.width));
    let mut bottom = top.saturating_add(i64::from(first.height));
    for area in iter {
        let a_left = i64::from(area.x);
        let a_top = i64::from(area.y);
        let a_right = a_left.saturating_add(i64::from(area.width));
        let a_bottom = a_top.saturating_add(i64::from(area.height));
        left = left.min(a_left);
        top = top.min(a_top);
        right = right.max(a_right);
        bottom = bottom.max(a_bottom);
    }
    let width = u32::try_from(right.saturating_sub(left))
        .ok()
        .filter(|&w| w > 0)?;
    let height = u32::try_from(bottom.saturating_sub(top))
        .ok()
        .filter(|&h| h > 0)?;
    let x = i32::try_from(left).ok()?;
    let y = i32::try_from(top).ok()?;
    Some(WorkArea {
        x,
        y,
        width,
        height,
    })
}

/// Picks a grounded cross-display walk target on a different monitor.
///
/// Sequence-stable destination index. Returns `None` when fewer than two
/// valid work areas exist or the pet center cannot be located.
#[must_use]
pub fn plan_cross_display_target(
    sequence: u64,
    pet_x: i32,
    pet_y: i32,
    pet_w: u32,
    pet_h: u32,
    displays: &[DesktopDisplay],
) -> Option<(i32, i32)> {
    if displays.len() < 2 {
        return None;
    }
    let cx = i64::from(pet_x).saturating_add(i64::from(pet_w) / 2);
    let cy = i64::from(pet_y).saturating_add(i64::from(pet_h) / 2);
    let cx_i = i32::try_from(cx).unwrap_or(pet_x);
    let cy_i = i32::try_from(cy).unwrap_or(pet_y);
    let current_idx = displays.iter().position(|d| {
        work_area_contains_point(d.work_area, f64::from(cx_i), f64::from(cy_i))
    })?;
    let dest_idx =
        (current_idx + 1 + (sequence as usize % (displays.len() - 1))) % displays.len();
    if dest_idx == current_idx {
        return None;
    }
    let dest = &displays[dest_idx];
    let area = dest.work_area;
    if area.width == 0 || area.height == 0 {
        return None;
    }
    // Grounded near bottom with sequence-stable horizontal variety so multi-monitor
    // walks do not always park at the exact same bottom-center landmark.
    const MARGIN: i64 = 16;
    let usable_w = i64::from(area.width).saturating_sub(i64::from(pet_w));
    let usable_h = i64::from(area.height).saturating_sub(i64::from(pet_h));
    let slot = (sequence % 5) as i64; // 0..4
    // slots: leftish, mid-left, center, mid-right, rightish
    let local_x = if usable_w <= 0 {
        0
    } else {
        let numerator = usable_w.saturating_mul(slot + 1);
        (numerator / 6).clamp(0, usable_w)
    };
    let local_y = usable_h.saturating_sub(MARGIN).max(0);
    let x = i32::try_from(i64::from(area.x).saturating_add(local_x)).ok()?;
    let y = i32::try_from(i64::from(area.y).saturating_add(local_y)).ok()?;
    Some((x, y))
}

#[cfg(test)]
mod cross_display_tests {
    use super::*;

    fn dual() -> Vec<DesktopDisplay> {
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
                scale_factor: 2.0,
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
    fn stages_and_cross_target() {
        let displays = dual();
        assert_eq!(work_area_stages(&displays).len(), 2);
        assert!((scale_factor_for_point(&displays, 10.0, 10.0) - 2.0).abs() < f64::EPSILON);
        assert!((scale_factor_for_point(&displays, 1400.0, 10.0) - 1.0).abs() < f64::EPSILON);
        let target = plan_cross_display_target(0, 100, 400, 260, 300, &displays).expect("target");
        assert!(target.0 >= 1280);
        let union = union_work_areas(&displays).expect("union");
        assert_eq!(union.width, 2560);
    }

    #[test]
    fn cross_target_single_display_is_none() {
        let one = vec![dual()[0].clone()];
        assert!(plan_cross_display_target(0, 100, 400, 260, 300, &one).is_none());
        assert!(plan_cross_display_target(0, 100, 400, 260, 300, &[]).is_none());
    }

    #[test]
    fn cross_target_stays_inside_destination_work_area() {
        let displays = dual();
        let pet_w = 260u32;
        let pet_h = 300u32;
        let target = plan_cross_display_target(0, 100, 400, pet_w, pet_h, &displays).expect("target");
        let dest = displays[1].work_area;
        assert!(target.0 >= dest.x);
        assert!(target.1 >= dest.y);
        assert!(i64::from(target.0) + i64::from(pet_w) <= i64::from(dest.x) + i64::from(dest.width));
        assert!(i64::from(target.1) + i64::from(pet_h) <= i64::from(dest.y) + i64::from(dest.height));
    }

    #[test]
    fn cross_target_sequence_cycles_among_other_displays() {
        let mut displays = dual();
        displays.push(DesktopDisplay {
            id: "c".into(),
            x: 2560,
            y: 0,
            width: 1280,
            height: 800,
            work_area: WorkArea {
                x: 2560,
                y: 0,
                width: 1280,
                height: 800,
            },
            scale_factor: 1.0,
        });
        // Pet on display 0; sequence 0 → display 1, sequence 1 → display 2.
        let t0 = plan_cross_display_target(0, 100, 400, 260, 300, &displays).expect("t0");
        let t1 = plan_cross_display_target(1, 100, 400, 260, 300, &displays).expect("t1");
        assert!(t0.0 >= 1280 && t0.0 < 2560, "t0={t0:?}");
        assert!(t1.0 >= 2560, "t1={t1:?}");
        // Same sequence is deterministic.
        assert_eq!(
            plan_cross_display_target(0, 100, 400, 260, 300, &displays),
            Some(t0)
        );
    }

    #[test]
    fn union_work_areas_spans_dual_monitors() {
        let union = union_work_areas(&dual()).expect("union");
        assert_eq!(union.x, 0);
        assert_eq!(union.y, 0);
        assert_eq!(union.width, 2560);
        assert_eq!(union.height, 800);
    }

    #[test]
    fn park_slot_variety_sequence_mod_five() {
        let displays = dual();
        let pet_w = 260u32;
        let pet_h = 300u32;
        let mut xs = Vec::new();
        for sequence in 0..5u64 {
            let (x, y) =
                plan_cross_display_target(sequence, 100, 400, pet_w, pet_h, &displays)
                    .expect("target");
            let dest = displays[1].work_area;
            assert!(x >= dest.x);
            assert!(y >= dest.y);
            assert!(i64::from(x) + i64::from(pet_w) <= i64::from(dest.x) + i64::from(dest.width));
            assert!(i64::from(y) + i64::from(pet_h) <= i64::from(dest.y) + i64::from(dest.height));
            xs.push(x);
        }
        let unique: std::collections::BTreeSet<_> = xs.iter().copied().collect();
        assert_eq!(unique.len(), 5, "slots={xs:?}");
        // sequence % 5 wraps: 5 → same park x as 0.
        let (x5, _) =
            plan_cross_display_target(5, 100, 400, pet_w, pet_h, &displays).expect("x5");
        assert_eq!(x5, xs[0]);
    }
}
