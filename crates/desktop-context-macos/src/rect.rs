//! Pure rectangle helpers for fullscreen / coverage checks.

/// Integer axis-aligned bounds in physical desktop coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RectBounds {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl RectBounds {
    /// Builds bounds from origin + size, saturating on overflow.
    #[must_use]
    pub fn from_origin_size(x: i32, y: i32, width: u32, height: u32) -> Self {
        let width_i = i32::try_from(width).unwrap_or(i32::MAX);
        let height_i = i32::try_from(height).unwrap_or(i32::MAX);
        Self {
            left: x,
            top: y,
            right: x.saturating_add(width_i),
            bottom: y.saturating_add(height_i),
        }
    }

    #[must_use]
    pub const fn width(&self) -> i32 {
        self.right.saturating_sub(self.left)
    }

    #[must_use]
    pub const fn height(&self) -> i32 {
        self.bottom.saturating_sub(self.top)
    }
}

/// Returns true when `window` covers `monitor` within `tolerance` pixels.
#[must_use]
pub fn rect_covers_monitor(window: RectBounds, monitor: RectBounds, tolerance: i32) -> bool {
    window.left <= monitor.left.saturating_add(tolerance)
        && window.top <= monitor.top.saturating_add(tolerance)
        && window.right >= monitor.right.saturating_sub(tolerance)
        && window.bottom >= monitor.bottom.saturating_sub(tolerance)
}
