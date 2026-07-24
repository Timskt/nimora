//! Platform-agnostic desktop lifeform context and pure motion planning.
//!
//! This crate owns versioned desktop observations, freshness policy, spring-damper
//! physics, multi-monitor helpers, occlusion, and obstacle-aware wander planning.
//! Hosts sample the OS, feed a [`DesktopSnapshot`], and drive animation with the
//! physics integrator.

mod displays;
mod freshness;
mod motion;
mod obstacles;
mod occlusion;
mod physics;
mod sensory;
mod snapshot;

pub use displays::{
    display_containing_origin, display_containing_point, display_contains_point,
    is_fullscreen_over_any_display, plan_cross_display_target, primary_display,
    scale_factor_for_point, union_display_bounds, union_work_areas, work_area_contains_point,
    work_area_for_point, work_area_stages, work_areas,
};
pub use freshness::{
    is_expired, is_usable, obstacles_usable, refresh_freshness, MAX_SNAPSHOT_LIFETIME_MS,
};
pub use motion::{
    plan_wander, plan_wander_from_snapshot, MotionGoal, MotionMode, Size2, Vec2i, WanderRequest,
};
pub use obstacles::{
    avoid_obstacles, clamp_pet_origin, free_wander_target, rects_overlap, safe_origin_bounds,
    AvoidRequest, Rect, Size,
};
pub use occlusion::{
    compute_pet_occlusion, occluders_from_snapshot, occluders_from_snapshot_dpi, OccluderRect,
    OcclusionStrip, PetBodyRect, PetOcclusion, PET_OCCLUSION_Z_PLANE,
};
pub use sensory::{
    battery_sensory_band, battery_should_emit, idle_sensory_band, idle_sensory_band_from_ms,
    idle_should_emit, logical_rect_to_physical, meeting_hint_label, meeting_should_emit,
    notification_should_emit, notification_unread_from_counts, sanitize_idle_ms, sanitize_scale_factor,
    BooleanSensorGate, BATTERY_BAND_CHARGING,
    BATTERY_BAND_CRITICAL, BATTERY_BAND_LOW, BATTERY_BAND_OK, BATTERY_BAND_UNKNOWN,
    BATTERY_SAME_BAND_THROTTLE_MS, BOOLEAN_SENSOR_HOLD_MS, IDLE_BAND_ACTIVE, IDLE_BAND_APPROACH,
    IDLE_BAND_NOTICE, IDLE_BAND_REST, MAX_ENUMERATED_WINDOWS,
};
pub use physics::{
    bounce_on_bounds, integrate, integrate_to_target, jump_parabola, sample_spring_trajectory,
    squash_stretch_scale, CollisionOutcome, JumpSample, SpringParams, SpringState, DEFAULT_DAMPING,
    DEFAULT_MASS, DEFAULT_STIFFNESS,
};
pub use snapshot::{
    CursorPosition, DegradationReason, DesktopDisplay, DesktopSnapshot, DesktopSnapshotParts,
    DesktopWindow, ForegroundApp, Freshness, MeetingHint, MeetingState, PowerState, WorkArea,
    DESKTOP_CONTEXT_SPEC,
};

use thiserror::Error;

/// Errors produced while constructing or validating desktop context values.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum DesktopContextError {
    #[error("desktop snapshot lifetime is invalid")]
    InvalidLifetime,
    #[error("desktop snapshot timestamps moved backwards")]
    ClockSkew,
    #[error("desktop snapshot is missing the required spec")]
    InvalidSpec,
}
