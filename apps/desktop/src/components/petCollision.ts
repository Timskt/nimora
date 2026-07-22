/**
 * Collision feedback for the desktop lifeform.
 *
 * When the pet runs into a screen edge it should not stop dead like a sprite
 * clipping a bound: it squashes against the wall, recoils back off it, and — if
 * it hit hard enough — reels dizzily for a moment before recovering. That arc
 * (impact → squash → recoil → dizzy → settle) is what sells the wall as a
 * physical thing the creature collided with.
 *
 * This module is a pure, DOM-free mapping from an impact (which wall, how fast)
 * to a time-parameterized response: at any elapsed time it reports the current
 * squash, a recoil offset in local units, and whether the pet is seeing stars.
 * The caller owns the clock and applies the transform; nothing here touches
 * React, the window, or a timer, so it unit-tests in isolation and stays inside
 * the architecture boundary.
 */

import { computeSquash, NEUTRAL_SQUASH, type SquashScale } from "./petSquash";

/** Which screen edge the pet struck. Drives the recoil and squash direction. */
export type CollisionWall = "left" | "right" | "top" | "bottom";

/** A single wall impact to respond to. */
export interface CollisionImpact {
  wall: CollisionWall;
  /**
   * Impact speed normalized to `0..1`, where `1` is the fastest bump the pet
   * can plausibly make. Values outside the range are clamped; non-finite values
   * are treated as no impact.
   */
  speed: number;
}

/** The pet's visible response at a given instant after the impact. */
export interface CollisionResponse {
  /** Non-uniform scale to apply (squash against the wall, easing back to rest). */
  squash: SquashScale;
  /** Recoil displacement away from the wall, in local units. */
  recoilX: number;
  recoilY: number;
  /** Whether the pet is reeling — the caller shows dizzy stars / crossed eyes. */
  dizzy: boolean;
  /** True once the whole response has elapsed and the pet is back at rest. */
  done: boolean;
}

/** Tuning shared by every collision response. */
export interface CollisionConfig {
  /** Duration of the squash-against-the-wall phase, in ms. */
  squashDurationMs: number;
  /** Duration of the recoil-and-settle phase, in ms, following the squash. */
  recoilDurationMs: number;
  /** Peak squash strain at full-speed impact (passed to {@link computeSquash}). */
  maxStrain: number;
  /** Peak recoil distance at full-speed impact, in local units. */
  maxRecoil: number;
  /**
   * Impact speed at or above which the pet gets dizzy. Below it the pet just
   * bounces without reeling.
   */
  dizzyThreshold: number;
  /** How long the dizzy state lasts from the moment of impact, in ms. */
  dizzyDurationMs: number;
}

/** A calm, readable default response arc. */
export const DEFAULT_COLLISION_CONFIG: CollisionConfig = {
  squashDurationMs: 110,
  recoilDurationMs: 320,
  maxStrain: 0.28,
  maxRecoil: 14,
  dizzyThreshold: 0.6,
  dizzyDurationMs: 900,
};

/** The response of a pet that has not hit anything. */
export const NEUTRAL_COLLISION: CollisionResponse = {
  squash: NEUTRAL_SQUASH,
  recoilX: 0,
  recoilY: 0,
  dizzy: false,
  done: true,
};

function clamp01(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(1, value));
}

/**
 * A wall's inward normal — the direction the pet recoils, away from the wall.
 * Hitting the left wall pushes the pet right (+x); hitting the top pushes it
 * down (+y); and so on.
 */
function recoilDirection(wall: CollisionWall): { x: number; y: number } {
  switch (wall) {
    case "left":
      return { x: 1, y: 0 };
    case "right":
      return { x: -1, y: 0 };
    case "top":
      return { x: 0, y: 1 };
    case "bottom":
      return { x: 0, y: -1 };
  }
}

/**
 * A wall hit along the horizontal axis squashes the body wide-and-short
 * ("horizontal" grows sx); a vertical hit (floor/ceiling) squashes it
 * tall-and-thin along the impact — modeled as the horizontal axis compressing,
 * i.e. a "vertical" stretch. This keeps the deformation reading as "flattened
 * against the surface it hit".
 */
function squashAxis(wall: CollisionWall): "horizontal" | "vertical" {
  return wall === "left" || wall === "right" ? "horizontal" : "vertical";
}

/**
 * Computes the pet's collision response at `elapsedMs` after `impact`.
 *
 * Phase 1 (`0..squashDurationMs`): squash ramps from zero to its peak — the
 * body compresses against the wall while barely recoiling.
 * Phase 2 (`squashDurationMs..+recoilDurationMs`): squash eases back to rest as
 * the recoil offset springs out and decays, pushing the pet off the wall.
 * After both phases the response is neutral and `done`.
 *
 * `dizzy` is true from impact until `dizzyDurationMs` elapses, but only when the
 * impact speed met `dizzyThreshold`. A non-finite or zero-speed impact yields a
 * neutral, already-done response.
 */
export function collisionResponse(
  impact: CollisionImpact,
  elapsedMs: number,
  config: CollisionConfig = DEFAULT_COLLISION_CONFIG,
): CollisionResponse {
  const speed = clamp01(impact.speed);
  const time = Number.isFinite(elapsedMs) ? Math.max(0, elapsedMs) : 0;
  if (speed === 0) {
    return NEUTRAL_COLLISION;
  }

  const totalMs = config.squashDurationMs + config.recoilDurationMs;
  const dizzy = speed >= config.dizzyThreshold && time < config.dizzyDurationMs;

  if (time >= totalMs) {
    // Motion has settled; the pet may still be reeling if dizzy outlasts it.
    return { squash: NEUTRAL_SQUASH, recoilX: 0, recoilY: 0, dizzy, done: !dizzy };
  }

  const direction = recoilDirection(impact.wall);
  const axis = squashAxis(impact.wall);

  let strainIntensity: number;
  let recoilMagnitude: number;
  if (time <= config.squashDurationMs) {
    // Phase 1: squash ramps in linearly; recoil barely begins.
    const t = config.squashDurationMs === 0 ? 1 : time / config.squashDurationMs;
    strainIntensity = speed * t;
    recoilMagnitude = 0;
  } else {
    // Phase 2: squash releases and recoil springs out, both decaying to rest.
    const t =
      config.recoilDurationMs === 0
        ? 1
        : (time - config.squashDurationMs) / config.recoilDurationMs;
    const decay = 1 - t; // linear ease-out over the recoil phase
    strainIntensity = speed * decay;
    // A single decaying half-sine gives a smooth push-out-and-back.
    recoilMagnitude = speed * config.maxRecoil * Math.sin(Math.PI * t) * decay;
  }

  const squash = computeSquash({
    intensity: strainIntensity,
    axis,
    maxStrain: config.maxStrain,
  });

  return {
    squash,
    recoilX: direction.x * recoilMagnitude,
    recoilY: direction.y * recoilMagnitude,
    dizzy,
    done: false,
  };
}

/**
 * Maps a desktop-surface classification to the wall the pet just contacted, or
 * `null` for free space (no wall). Corner surfaces resolve to their vertical
 * edge (top/bottom) since the pet arrives at a corner by traveling along the
 * horizontal edge into it. The caller uses this to translate a
 * `pet-surface-changed` event into a collision impact without duplicating the
 * host's surface vocabulary.
 */
export function surfaceToWall(surface: string): CollisionWall | null {
  switch (surface) {
    case "left":
      return "left";
    case "right":
      return "right";
    case "top":
    case "top_left":
    case "top_right":
      return "top";
    case "bottom":
    case "bottom_left":
    case "bottom_right":
      return "bottom";
    default:
      return null;
  }
}
