//! Pure pet-body occlusion from desktop window geometry.
//!
//! Computes how real windows cover the pet subject rectangle so the host can
//! apply CSS clip / hole masks. Never inspects window titles (privacy).
//!
//! # z_order convention
//!
//! Larger `z_order` is treated as more front. Occluders with
//! `z_order >= `[`PET_OCCLUSION_Z_PLANE`] occlude the pet; equal z counts as
//! occluding when listed. Platform adapters currently assign front-to-back
//! indices (CGWindowList / EnumWindows) starting at 0 for the frontmost window,
//! so typical samples land at `z_order >= 0` and all listed candidates occlude.
//! Pass a negative `z_order` to mark a window as behind the pet plane.

use crate::snapshot::{DesktopSnapshot, DesktopWindow};
use serde::{Deserialize, Serialize};

/// Implicit pet z-plane for pure occlusion tests.
///
/// Occluders with `z_order >= PET_OCCLUSION_Z_PLANE` are treated as in front of
/// (or coplanar with) the pet and may occlude it.
pub const PET_OCCLUSION_Z_PLANE: i32 = 0;

/// Pet body rectangle in physical screen coordinates (top-left origin).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PetBodyRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Axis-aligned window rectangle that may cover the pet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OccluderRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// Larger values are more front (see module docs).
    pub z_order: i32,
}

/// Normalized axis-aligned strip of the pet body that is occluded.
///
/// Coordinates are in `[0, 1]` relative to the pet body (top-left origin):
/// `y0`/`y1` vertical, `x0`/`x1` horizontal.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OcclusionStrip {
    /// Normalized vertical start (top = 0).
    pub y0: f32,
    /// Normalized vertical end.
    pub y1: f32,
    /// Normalized horizontal start (left = 0).
    pub x0: f32,
    /// Normalized horizontal end.
    pub x1: f32,
}

/// Occlusion result for a pet body against a set of window occluders.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PetOcclusion {
    /// Fraction of pet area covered, clamped to `0.0..=1.0`.
    pub coverage: f32,
    /// Merged strips covering occluded parts (for CSS clip or hole masks).
    pub strips: Vec<OcclusionStrip>,
    /// True when the pet is fully covered.
    pub fully_hidden: bool,
}

impl PetOcclusion {
    /// Empty occlusion (no coverage). Used for fail-closed paths.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            coverage: 0.0,
            strips: Vec::new(),
            fully_hidden: false,
        }
    }
}

impl PetBodyRect {
    #[must_use]
    pub fn area(&self) -> u64 {
        u64::from(self.width).saturating_mul(u64::from(self.height))
    }

    #[must_use]
    fn right(&self) -> i64 {
        i64::from(self.x).saturating_add(i64::from(self.width))
    }

    #[must_use]
    fn bottom(&self) -> i64 {
        i64::from(self.y).saturating_add(i64::from(self.height))
    }
}

impl OccluderRect {
    #[must_use]
    fn right(&self) -> i64 {
        i64::from(self.x).saturating_add(i64::from(self.width))
    }

    #[must_use]
    fn bottom(&self) -> i64 {
        i64::from(self.y).saturating_add(i64::from(self.height))
    }

    /// True when this occluder is in front of (or coplanar with) the pet plane.
    #[must_use]
    pub const fn is_in_front_of_pet(&self) -> bool {
        self.z_order >= PET_OCCLUSION_Z_PLANE
    }
}

/// Local (pet-relative) integer rectangle used while unioning coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LocalRect {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

impl LocalRect {
    fn is_empty(&self) -> bool {
        self.x1 <= self.x0 || self.y1 <= self.y0
    }
}

/// Computes how `occluders` cover `pet`.
///
/// Only occluders that overlap the pet body and have
/// `z_order >= `[`PET_OCCLUSION_Z_PLANE`] (larger z = more front; equal z
/// occludes) contribute. Coverage is the exact area-union of the axis-aligned
/// intersections divided by the pet area.
#[must_use]
pub fn compute_pet_occlusion(pet: PetBodyRect, occluders: &[OccluderRect]) -> PetOcclusion {
    let pet_area = pet.area();
    if pet_area == 0 {
        return PetOcclusion::empty();
    }

    let mut local_rects: Vec<LocalRect> = Vec::new();
    for occluder in occluders {
        if !occluder.is_in_front_of_pet() {
            continue;
        }
        if occluder.width == 0 || occluder.height == 0 {
            continue;
        }
        if let Some(local) = intersect_to_local(pet, *occluder) {
            local_rects.push(local);
        }
    }

    if local_rects.is_empty() {
        return PetOcclusion::empty();
    }

    let (coverage_area, strips) = union_coverage_and_strips(&local_rects, pet.width, pet.height);
    let coverage = (coverage_area as f64 / pet_area as f64).clamp(0.0, 1.0) as f32;
    let fully_hidden = coverage_area >= pet_area;

    PetOcclusion {
        coverage,
        strips,
        fully_hidden,
    }
}

/// Maps snapshot windows to occluder rects for a pet owned by `pet_owner_pid`.
///
/// Skips shell windows, minimized, offscreen, zero-size, and windows owned by
/// the pet process itself. Does not read titles. Coordinates are left as sampled
/// (see [`occluders_from_snapshot_dpi`] when logical→physical conversion is needed).
#[must_use]
pub fn occluders_from_snapshot(
    snapshot: &DesktopSnapshot,
    pet_owner_pid: u32,
) -> Vec<OccluderRect> {
    occluders_from_snapshot_dpi(snapshot, pet_owner_pid, false)
}

/// Like [`occluders_from_snapshot`], optionally scaling window rects by the
/// containing display `scale_factor` (logical points → physical pixels).
///
/// Enable `apply_display_scale` when window geometry is known to be in points
/// while the pet body pose is in physical pixels (mixed DPI hosts). When
/// disabled, rects are used as sampled — correct for CG-points-only or
/// fully-physical Windows pipelines.
#[must_use]
pub fn occluders_from_snapshot_dpi(
    snapshot: &DesktopSnapshot,
    pet_owner_pid: u32,
    apply_display_scale: bool,
) -> Vec<OccluderRect> {
    let mut occluders: Vec<OccluderRect> = snapshot
        .windows
        .iter()
        .filter(|window| is_occluder_candidate(window, pet_owner_pid))
        .map(|window| window_to_occluder(window, &snapshot.displays, apply_display_scale))
        .collect();
    // Cap for stable, cheap occlusion on crowded desktops (front-most first).
    const MAX_OCCLUDERS: usize = 48;
    if occluders.len() > MAX_OCCLUDERS {
        occluders.sort_by_key(|o| o.z_order);
        occluders.truncate(MAX_OCCLUDERS);
    }
    occluders
}

fn is_occluder_candidate(window: &DesktopWindow, pet_owner_pid: u32) -> bool {
    if window.is_shell || window.is_minimized || !window.onscreen {
        return false;
    }
    if window.width == 0 || window.height == 0 {
        return false;
    }
    if window.owner_pid == pet_owner_pid {
        return false;
    }
    true
}

fn window_to_occluder(
    window: &DesktopWindow,
    displays: &[crate::DesktopDisplay],
    apply_display_scale: bool,
) -> OccluderRect {
    let (x, y, width, height) = if apply_display_scale && !displays.is_empty() {
        let cx = f64::from(window.x) + f64::from(window.width) * 0.5;
        let cy = f64::from(window.y) + f64::from(window.height) * 0.5;
        let scale = crate::displays::scale_factor_for_point(displays, cx, cy);
        if let Some(display) = crate::displays::display_containing_point(displays, cx, cy)
            .or_else(|| crate::displays::primary_display(displays))
        {
            // Display-local scale keeps multi-monitor origins stable while
            // expanding logical window size/offset into physical pixels.
            let scale = crate::sensory::sanitize_scale_factor(scale);
            let local_x = f64::from(window.x) - f64::from(display.x);
            let local_y = f64::from(window.y) - f64::from(display.y);
            let px = (f64::from(display.x) + local_x * scale).round();
            let py = (f64::from(display.y) + local_y * scale).round();
            let pw = (f64::from(window.width) * scale).round().max(0.0);
            let ph = (f64::from(window.height) * scale).round().max(0.0);
            (
                clamp_i32(px, window.x),
                clamp_i32(py, window.y),
                clamp_u32(pw, window.width),
                clamp_u32(ph, window.height),
            )
        } else {
            crate::sensory::logical_rect_to_physical(
                window.x,
                window.y,
                window.width,
                window.height,
                scale,
            )
        }
    } else {
        (window.x, window.y, window.width, window.height)
    };
    OccluderRect {
        x,
        y,
        width,
        height,
        z_order: window.z_order,
    }
}

#[allow(clippy::cast_possible_truncation)]
fn clamp_i32(value: f64, fallback: i32) -> i32 {
    if value.is_finite() {
        value.clamp(i32::MIN as f64, i32::MAX as f64) as i32
    } else {
        fallback
    }
}

#[allow(clippy::cast_possible_truncation)]
fn clamp_u32(value: f64, fallback: u32) -> u32 {
    if value.is_finite() {
        value.clamp(0.0, f64::from(u32::MAX)) as u32
    } else {
        fallback
    }
}

fn intersect_to_local(pet: PetBodyRect, occluder: OccluderRect) -> Option<LocalRect> {
    let left = i64::from(pet.x).max(i64::from(occluder.x));
    let top = i64::from(pet.y).max(i64::from(occluder.y));
    let right = pet.right().min(occluder.right());
    let bottom = pet.bottom().min(occluder.bottom());
    if right <= left || bottom <= top {
        return None;
    }
    let x0 = i32::try_from(left - i64::from(pet.x)).ok()?;
    let y0 = i32::try_from(top - i64::from(pet.y)).ok()?;
    let x1 = i32::try_from(right - i64::from(pet.x)).ok()?;
    let y1 = i32::try_from(bottom - i64::from(pet.y)).ok()?;
    let local = LocalRect { x0, y0, x1, y1 };
    if local.is_empty() {
        None
    } else {
        Some(local)
    }
}

/// Exact area union of local rects plus merged non-overlapping strips.
fn union_coverage_and_strips(
    rects: &[LocalRect],
    pet_width: u32,
    pet_height: u32,
) -> (u64, Vec<OcclusionStrip>) {
    if rects.is_empty() || pet_width == 0 || pet_height == 0 {
        return (0, Vec::new());
    }

    let mut ys: Vec<i32> = Vec::with_capacity(rects.len() * 2);
    for rect in rects {
        ys.push(rect.y0);
        ys.push(rect.y1);
    }
    ys.sort_unstable();
    ys.dedup();

    let mut area = 0u64;
    let mut strips: Vec<OcclusionStrip> = Vec::new();
    let width_f = f64::from(pet_width);
    let height_f = f64::from(pet_height);

    for band in ys.windows(2) {
        let y0 = band[0];
        let y1 = band[1];
        if y1 <= y0 {
            continue;
        }
        let mut xs: Vec<(i32, i32)> = Vec::new();
        for rect in rects {
            if rect.y0 <= y0 && rect.y1 >= y1 {
                xs.push((rect.x0, rect.x1));
            }
        }
        if xs.is_empty() {
            continue;
        }
        xs.sort_unstable_by_key(|(a, _)| *a);
        let merged = merge_x_intervals(&xs);
        let band_height = u64::try_from(i64::from(y1) - i64::from(y0)).unwrap_or(0);
        for (x0, x1) in merged {
            let band_width = u64::try_from(i64::from(x1) - i64::from(x0)).unwrap_or(0);
            area = area.saturating_add(band_height.saturating_mul(band_width));
            strips.push(OcclusionStrip {
                y0: (f64::from(y0) / height_f) as f32,
                y1: (f64::from(y1) / height_f) as f32,
                x0: (f64::from(x0) / width_f) as f32,
                x1: (f64::from(x1) / width_f) as f32,
            });
        }
    }

    // Merge vertically adjacent strips that share the same x-range.
    let strips = merge_vertical_strips(strips);
    (area, strips)
}

fn merge_x_intervals(sorted: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let mut out: Vec<(i32, i32)> = Vec::new();
    for &(x0, x1) in sorted {
        if x1 <= x0 {
            continue;
        }
        match out.last_mut() {
            Some(last) if x0 <= last.1 => {
                last.1 = last.1.max(x1);
            }
            _ => out.push((x0, x1)),
        }
    }
    out
}

fn merge_vertical_strips(strips: Vec<OcclusionStrip>) -> Vec<OcclusionStrip> {
    if strips.is_empty() {
        return strips;
    }
    let mut sorted = strips;
    sorted.sort_by(|a, b| {
        a.x0.partial_cmp(&b.x0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.x1.partial_cmp(&b.x1).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| a.y0.partial_cmp(&b.y0).unwrap_or(std::cmp::Ordering::Equal))
    });

    let mut merged: Vec<OcclusionStrip> = Vec::with_capacity(sorted.len());
    for strip in sorted {
        if let Some(last) = merged.last_mut() {
            let same_x = (last.x0 - strip.x0).abs() < f32::EPSILON
                && (last.x1 - strip.x1).abs() < f32::EPSILON;
            let adjacent_y = (last.y1 - strip.y0).abs() < 1e-5 || strip.y0 <= last.y1;
            if same_x && adjacent_y {
                last.y1 = last.y1.max(strip.y1);
                continue;
            }
        }
        merged.push(strip);
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{
        DesktopSnapshot, DesktopSnapshotParts, DesktopWindow, Freshness, MeetingHint, MeetingState,
    };

    fn pet(x: i32, y: i32, w: u32, h: u32) -> PetBodyRect {
        PetBodyRect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    fn occ(x: i32, y: i32, w: u32, h: u32, z: i32) -> OccluderRect {
        OccluderRect {
            x,
            y,
            width: w,
            height: h,
            z_order: z,
        }
    }

    fn window(
        id: &str,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        z: i32,
        owner_pid: u32,
        onscreen: bool,
        is_minimized: bool,
        is_shell: bool,
    ) -> DesktopWindow {
        DesktopWindow {
            id: id.into(),
            title_hash: None,
            title_redacted: true,
            x,
            y,
            width: w,
            height: h,
            z_order: z,
            owner_pid,
            owner_name: "App".into(),
            onscreen,
            is_minimized,
            is_fullscreen_candidate: false,
            is_shell,
        }
    }

    fn snapshot_with(windows: Vec<DesktopWindow>) -> DesktopSnapshot {
        DesktopSnapshot::new(DesktopSnapshotParts {
            windows,
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
        .expect("valid snapshot")
    }

    #[test]
    fn no_occluders_is_empty() {
        let result = compute_pet_occlusion(pet(10, 20, 100, 200), &[]);
        assert_eq!(result.coverage, 0.0);
        assert!(result.strips.is_empty());
        assert!(!result.fully_hidden);
    }

    #[test]
    fn partial_overlap_coverage_and_strip() {
        // Pet 100x100; occluder covers left half.
        let result = compute_pet_occlusion(pet(0, 0, 100, 100), &[occ(0, 0, 50, 100, 1)]);
        assert!((result.coverage - 0.5).abs() < 1e-5, "coverage={}", result.coverage);
        assert!(!result.fully_hidden);
        assert_eq!(result.strips.len(), 1);
        let strip = result.strips[0];
        assert!((strip.x0 - 0.0).abs() < 1e-5);
        assert!((strip.x1 - 0.5).abs() < 1e-5);
        assert!((strip.y0 - 0.0).abs() < 1e-5);
        assert!((strip.y1 - 1.0).abs() < 1e-5);
    }

    #[test]
    fn full_cover_sets_fully_hidden() {
        let result = compute_pet_occlusion(pet(50, 50, 80, 120), &[occ(0, 0, 400, 400, 2)]);
        assert!((result.coverage - 1.0).abs() < 1e-5);
        assert!(result.fully_hidden);
        assert!(!result.strips.is_empty());
    }

    #[test]
    fn shell_minimized_offscreen_own_pid_skipped() {
        let snap = snapshot_with(vec![
            window("shell", 0, 0, 200, 200, 1, 9, true, false, true),
            window("min", 0, 0, 200, 200, 1, 9, true, true, false),
            window("off", 0, 0, 200, 200, 1, 9, false, false, false),
            window("zero", 0, 0, 0, 200, 1, 9, true, false, false),
            window("self", 0, 0, 200, 200, 1, 42, true, false, false),
            window("ok", 0, 0, 200, 200, 3, 9, true, false, false),
        ]);
        let occluders = occluders_from_snapshot(&snap, 42);
        assert_eq!(occluders.len(), 1);
        assert_eq!(occluders[0].z_order, 3);
        assert_eq!(occluders[0].width, 200);
    }

    #[test]
    fn z_order_behind_pet_plane_ignored() {
        // z < PET_OCCLUSION_Z_PLANE is behind the pet and must not occlude.
        let behind = occ(0, 0, 100, 100, -1);
        let result = compute_pet_occlusion(pet(0, 0, 100, 100), &[behind]);
        assert_eq!(result.coverage, 0.0);
        assert!(result.strips.is_empty());
        assert!(!result.fully_hidden);

        // Equal z occludes when listed.
        let coplanar = occ(0, 0, 50, 100, PET_OCCLUSION_Z_PLANE);
        let result = compute_pet_occlusion(pet(0, 0, 100, 100), &[coplanar]);
        assert!((result.coverage - 0.5).abs() < 1e-5);
    }

    #[test]
    fn strip_merge_unions_overlapping_rects() {
        // Two overlapping halves should merge into full coverage without double-counting.
        let left = occ(0, 0, 60, 100, 1);
        let right = occ(40, 0, 60, 100, 2);
        let result = compute_pet_occlusion(pet(0, 0, 100, 100), &[left, right]);
        assert!((result.coverage - 1.0).abs() < 1e-5, "coverage={}", result.coverage);
        assert!(result.fully_hidden);
        // Vertically adjacent / identical x-range bands collapse.
        assert_eq!(result.strips.len(), 1);
        let strip = result.strips[0];
        assert!((strip.x0 - 0.0).abs() < 1e-5);
        assert!((strip.x1 - 1.0).abs() < 1e-5);
        assert!((strip.y0 - 0.0).abs() < 1e-5);
        assert!((strip.y1 - 1.0).abs() < 1e-5);
    }

    #[test]
    fn non_overlapping_strips_remain_separate() {
        let top = occ(0, 0, 100, 20, 1);
        let bottom = occ(0, 80, 100, 20, 1);
        let result = compute_pet_occlusion(pet(0, 0, 100, 100), &[top, bottom]);
        assert!((result.coverage - 0.4).abs() < 1e-5, "coverage={}", result.coverage);
        assert_eq!(result.strips.len(), 2);
    }

    #[test]
    fn dpi_scale_expands_logical_occluder() {
        let mut snap = snapshot_with(vec![window(
            "app", 0, 0, 100, 100, 1, 9, true, false, false,
        )]);
        snap.displays = vec![crate::DesktopDisplay {
            id: "main".into(),
            x: 0,
            y: 0,
            width: 800,
            height: 600,
            work_area: crate::WorkArea {
                x: 0,
                y: 0,
                width: 800,
                height: 600,
            },
            scale_factor: 2.0,
        }];
        let raw = occluders_from_snapshot(&snap, 1);
        assert_eq!(raw[0].width, 100);
        let scaled = occluders_from_snapshot_dpi(&snap, 1, true);
        assert_eq!(scaled[0].width, 200);
        assert_eq!(scaled[0].height, 200);
    }

}
