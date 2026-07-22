/**
 * Secondary motion for the desktop lifeform's soft parts (tail, ears, hair).
 *
 * When a body moves, its loose appendages lag behind, swing past, and settle —
 * they do not rigidly track the torso. This module models one such follower as
 * a single Verlet-integrated point tethered to an anchor by a spring, with
 * damping and a maximum leash so it trails the anchor's motion and swings back
 * to rest without ever detaching.
 *
 * It is a pure, DOM-free scalar/point integrator: no React, no window, no
 * `requestAnimationFrame`. The caller owns the frame loop and the anchor
 * position (which it already tracks); this module only advances the follower.
 * That keeps it unit-testable in isolation and inside the architecture
 * boundary.
 *
 * Verlet integration is used instead of storing an explicit velocity: the
 * follower remembers its previous position, and velocity is implied by the
 * difference. This is stable, cheap, and reads naturally as "swing and settle".
 */

/** A point in the follower's local coordinate space. */
export interface FollowerPoint {
  x: number;
  y: number;
}

/** Tuning for a single secondary-motion follower. */
export interface FollowerConfig {
  /**
   * Restoring pull toward the anchor per frame, in `0..1`. Higher is stiffer
   * (tracks the anchor more tightly, less lag); lower trails and swings more.
   */
  stiffness: number;
  /**
   * Velocity retained each frame, in `0..1`. `1` swings forever; lower settles
   * faster. This is applied to the implicit Verlet velocity.
   */
  damping: number;
  /**
   * Maximum distance, in local units, the follower may sit from its anchor so a
   * violent anchor jump can never fling the appendage across the character.
   */
  maxLeash: number;
}

/** The mutable state of a follower: current and previous position. */
export interface FollowerState {
  position: FollowerPoint;
  previous: FollowerPoint;
}

function isFinitePoint(point: FollowerPoint): boolean {
  return Number.isFinite(point.x) && Number.isFinite(point.y);
}

function clamp01(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(1, value));
}

/**
 * Creates a follower at rest at `origin` (zero implied velocity).
 */
export function createFollower(origin: FollowerPoint): FollowerState {
  const safe = isFinitePoint(origin) ? origin : { x: 0, y: 0 };
  return { position: { ...safe }, previous: { ...safe } };
}

/**
 * Advances a follower one frame toward `anchor` and returns the new state.
 *
 * The step is pure: it returns a fresh state rather than mutating the input, so
 * callers holding React state stay in control of when it updates.
 *
 * Non-finite anchors or states are treated as the anchor position (the follower
 * snaps to a safe point rather than integrating garbage).
 */
export function stepFollower(
  state: FollowerState,
  anchor: FollowerPoint,
  config: FollowerConfig,
): FollowerState {
  if (!isFinitePoint(anchor)) {
    return state;
  }
  if (!isFinitePoint(state.position) || !isFinitePoint(state.previous)) {
    return createFollower(anchor);
  }

  const stiffness = clamp01(config.stiffness);
  const damping = clamp01(config.damping);
  const maxLeash = Number.isFinite(config.maxLeash) ? Math.max(0, config.maxLeash) : 0;

  // Implicit Verlet velocity: how far the point moved last frame, retained and
  // scaled by damping so motion bleeds off rather than persisting forever.
  const velocityX = (state.position.x - state.previous.x) * damping;
  const velocityY = (state.position.y - state.previous.y) * damping;

  // Integrate: carry the retained velocity, then pull toward the anchor.
  let nextX = state.position.x + velocityX + (anchor.x - state.position.x) * stiffness;
  let nextY = state.position.y + velocityY + (anchor.y - state.position.y) * stiffness;

  // Enforce the leash so the appendage can never detach from the anchor.
  const offsetX = nextX - anchor.x;
  const offsetY = nextY - anchor.y;
  const distance = Math.hypot(offsetX, offsetY);
  if (distance > maxLeash && distance > 0) {
    const scale = maxLeash / distance;
    nextX = anchor.x + offsetX * scale;
    nextY = anchor.y + offsetY * scale;
  }

  return {
    position: { x: nextX, y: nextY },
    previous: { x: state.position.x, y: state.position.y },
  };
}

/**
 * Reports whether a follower has effectively come to rest at its anchor: within
 * `epsilon` of the anchor and moving slower than `epsilon` per frame.
 */
export function followerAtRest(
  state: FollowerState,
  anchor: FollowerPoint,
  epsilon: number,
): boolean {
  const tolerance = Math.abs(epsilon);
  const displacement = Math.hypot(state.position.x - anchor.x, state.position.y - anchor.y);
  const speed = Math.hypot(
    state.position.x - state.previous.x,
    state.position.y - state.previous.y,
  );
  return displacement <= tolerance && speed <= tolerance;
}
