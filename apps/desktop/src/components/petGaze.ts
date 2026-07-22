/**
 * IK-style eye gaze for the built-in vector companion.
 *
 * The soul of a living creature is in its eyes: they track what moves. This
 * module turns a pointer position (in the same client-pixel space as the pet's
 * bounding box) into a small, clamped pupil offset the character can apply so
 * the pet appears to *look at* the cursor rather than staring blankly ahead.
 *
 * It is intentionally pure and DOM-free — no React, no window, no
 * `getBoundingClientRect` — so it unit-tests in isolation and cannot leak the
 * platform boundary. The caller supplies the geometry it already has.
 */

/** A point in client-pixel space. */
export interface GazePoint {
  x: number;
  y: number;
}

/** The pupil displacement to apply, in the SVG's own coordinate units. */
export interface GazeOffset {
  dx: number;
  dy: number;
}

/** Inputs describing where the pet's eyes are and where the pointer is. */
export interface GazeInput {
  /** Pointer position in client-pixel space. */
  pointer: GazePoint;
  /** Center of the eye region in the same client-pixel space. */
  eyeCenter: GazePoint;
  /**
   * Largest pupil displacement, in SVG units, allowed on each axis. The eye
   * socket is an ellipse, so horizontal and vertical limits differ.
   */
  maxOffset: GazeOffset;
  /**
   * Client-pixel distance from the eye center at which the gaze reaches its
   * full `maxOffset`. Closer pointers produce a proportionally smaller offset
   * so the eyes ease toward the cursor instead of snapping to the rim.
   */
  saturationDistance: number;
}

/** A neutral, forward gaze — pupils centered. */
export const NEUTRAL_GAZE: GazeOffset = { dx: 0, dy: 0 };

function isFiniteNumber(value: number): boolean {
  return typeof value === "number" && Number.isFinite(value);
}

/**
 * Computes the clamped pupil offset that points the eyes toward `pointer`.
 *
 * The direction to the pointer is scaled by how far away it is (linearly up to
 * `saturationDistance`, then held at full deflection) and clamped to the
 * elliptical socket described by `maxOffset`. Any non-finite input, or a
 * non-positive saturation distance, yields {@link NEUTRAL_GAZE} so a bad value
 * can never throw the eyes to infinity.
 */
export function computeGaze(input: GazeInput): GazeOffset {
  const { pointer, eyeCenter, maxOffset, saturationDistance } = input;
  if (
    !isFiniteNumber(pointer.x) ||
    !isFiniteNumber(pointer.y) ||
    !isFiniteNumber(eyeCenter.x) ||
    !isFiniteNumber(eyeCenter.y) ||
    !isFiniteNumber(maxOffset.dx) ||
    !isFiniteNumber(maxOffset.dy) ||
    !isFiniteNumber(saturationDistance) ||
    saturationDistance <= 0
  ) {
    return NEUTRAL_GAZE;
  }

  const deltaX = pointer.x - eyeCenter.x;
  const deltaY = pointer.y - eyeCenter.y;
  const distance = Math.hypot(deltaX, deltaY);
  if (distance === 0) {
    return NEUTRAL_GAZE;
  }

  // Fraction of full deflection: linear ramp to 1.0 at the saturation distance.
  const intensity = Math.min(distance / saturationDistance, 1);
  // Unit direction toward the pointer, scaled by intensity, then mapped onto
  // the elliptical socket. Dividing by `distance` normalizes the direction.
  const dx = clamp((deltaX / distance) * intensity * Math.abs(maxOffset.dx), maxOffset.dx);
  const dy = clamp((deltaY / distance) * intensity * Math.abs(maxOffset.dy), maxOffset.dy);
  return { dx, dy };
}

/** Clamps `value` to `[-|limit|, +|limit|]`. */
function clamp(value: number, limit: number): number {
  const magnitude = Math.abs(limit);
  return Math.max(-magnitude, Math.min(magnitude, value));
}
