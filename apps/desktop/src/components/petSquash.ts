/**
 * Squash-and-stretch deformation for the desktop lifeform.
 *
 * The oldest trick in character animation: a body that moves fast stretches
 * along its travel axis and thins across it; on impact it squashes — wide and
 * short — before springing back. It reads as weight and life rather than a
 * rigid sprite sliding around.
 *
 * This module is a pure, DOM-free mapping from a scalar "intensity" (how hard
 * the pet is moving or how hard it just hit something, normalized to `0..1`) to
 * a non-uniform scale. The deformation is volume-preserving: the axis that
 * grows is matched by the perpendicular axis shrinking, so the character keeps
 * its apparent mass. The caller turns the result into a CSS/SVG `scale(sx, sy)`.
 */

/** A non-uniform scale to apply to the character stage. */
export interface SquashScale {
  /** Horizontal scale factor. */
  sx: number;
  /** Vertical scale factor. */
  sy: number;
}

/** No deformation — the resting, undeformed character. */
export const NEUTRAL_SQUASH: SquashScale = { sx: 1, sy: 1 };

/** The axis a deformation acts along. */
export type SquashAxis = "vertical" | "horizontal";

export interface SquashInput {
  /**
   * Deformation strength in `0..1`. `0` is undeformed; `1` is the maximum
   * deflection permitted by `maxStrain`. Values outside the range are clamped;
   * non-finite values collapse to `0`.
   */
  intensity: number;
  /**
   * Which way the deformation stretches. `"vertical"` stretches tall and thin
   * (a leap or fall); `"horizontal"` squashes wide and short (a landing or a
   * sideways wall bump).
   */
  axis: SquashAxis;
  /**
   * Largest fractional deflection at full intensity, e.g. `0.25` allows the
   * stretched axis to reach `1.25` and the squashed axis to fall to its
   * volume-preserving complement. Clamped to `[0, 0.9]` so the character can
   * never invert or collapse to a line.
   */
  maxStrain: number;
}

/** Upper bound on strain so the body can never invert or vanish. */
const MAX_STRAIN_CEILING = 0.9;

function sanitize(value: number): number {
  return Number.isFinite(value) ? value : 0;
}

function clamp01(value: number): number {
  return Math.max(0, Math.min(1, value));
}

/**
 * Computes a volume-preserving squash/stretch scale.
 *
 * At intensity `i` and max strain `m`, the primary axis scales by
 * `1 + i*m` and the perpendicular axis takes the reciprocal so `sx * sy === 1`
 * (area, and thus apparent volume in 2D, is conserved). A neutral or invalid
 * input returns {@link NEUTRAL_SQUASH}.
 */
export function computeSquash(input: SquashInput): SquashScale {
  const intensity = clamp01(sanitize(input.intensity));
  // `Infinity` clamps down to the ceiling (a caller asking for "as much as
  // possible" gets the maximum), while `NaN` collapses to zero deflection.
  const rawStrain = Number.isNaN(input.maxStrain) ? 0 : input.maxStrain;
  const maxStrain = Math.max(0, Math.min(MAX_STRAIN_CEILING, rawStrain));
  if (intensity === 0 || maxStrain === 0) {
    return NEUTRAL_SQUASH;
  }
  const primary = 1 + intensity * maxStrain;
  // Reciprocal keeps sx * sy === 1: the growing axis is exactly compensated by
  // the shrinking one.
  const secondary = 1 / primary;
  return input.axis === "vertical"
    ? { sx: secondary, sy: primary }
    : { sx: primary, sy: secondary };
}

/**
 * Formats a {@link SquashScale} as a CSS `scale()` transform value.
 *
 * Factors are rounded to four decimals so identical inputs yield byte-identical
 * output (stable snapshots) without visible precision loss.
 */
export function squashTransform(scale: SquashScale): string {
  const sx = round4(scale.sx);
  const sy = round4(scale.sy);
  return `scale(${sx}, ${sy})`;
}

function round4(value: number): number {
  return Math.round(value * 10_000) / 10_000;
}
