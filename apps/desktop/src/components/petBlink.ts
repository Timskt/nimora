/**
 * Dynamic blinking for the desktop lifeform's eyes.
 *
 * A living creature blinks — and a tired one blinks differently: less often,
 * and with heavier, longer lid closures, the "dry-eye" tell the goal calls for.
 * This module derives the timing of the next blink from the pet's energy so the
 * eyes read as awake-and-alert when rested and drowsy when depleted.
 *
 * It is a pure, DOM-free scheduler: given the current energy and a random roll
 * it returns how long until the next blink and how long that blink lasts. The
 * caller owns the clock and the random source, so the schedule is deterministic
 * under test and stays inside the architecture boundary.
 */

/** A scheduled blink: when it happens and how long the lids stay shut. */
export interface BlinkSchedule {
  /** Delay from now until the blink begins, in ms. */
  delayMs: number;
  /** How long the blink (lid close + reopen) lasts, in ms. */
  durationMs: number;
}

export interface BlinkConfig {
  /** Shortest gap between blinks (a fully rested, alert pet), in ms. */
  minIntervalMs: number;
  /** Longest gap between blinks (an exhausted, drowsy pet), in ms. */
  maxIntervalMs: number;
  /** Random jitter added to the interval, in ms, to avoid a metronomic blink. */
  jitterMs: number;
  /** Blink duration for a rested pet (a crisp flick), in ms. */
  minDurationMs: number;
  /** Blink duration for an exhausted pet (a heavy, lingering close), in ms. */
  maxDurationMs: number;
}

/** A calm, natural-feeling default cadence. */
export const DEFAULT_BLINK_CONFIG: BlinkConfig = {
  minIntervalMs: 2_600,
  maxIntervalMs: 6_400,
  jitterMs: 1_200,
  minDurationMs: 110,
  maxDurationMs: 320,
};

function clamp01(value: number): number {
  if (!Number.isFinite(value)) return 1;
  return Math.max(0, Math.min(1, value));
}

/**
 * Computes the schedule for the next blink given the pet's `energy` (0..100)
 * and a `roll` in `0..1` (from the caller's random source; clamped, and treated
 * as `0` if non-finite for a deterministic fallback).
 *
 * Fatigue is `1 - energy/100`. As fatigue rises the base interval slides from
 * `minIntervalMs` toward `maxIntervalMs` (blinks get rarer — the drowsy stare)
 * and the duration slides from `minDurationMs` toward `maxDurationMs` (each
 * blink gets heavier). Jitter is scaled by `roll` so successive blinks are not
 * perfectly periodic.
 */
export function nextBlink(
  energy: number,
  roll: number,
  config: BlinkConfig = DEFAULT_BLINK_CONFIG,
): BlinkSchedule {
  const normalizedEnergy = clamp01(energy / 100);
  const fatigue = 1 - normalizedEnergy;
  const safeRoll = Number.isFinite(roll) ? Math.max(0, Math.min(1, roll)) : 0;

  const baseInterval =
    config.minIntervalMs + (config.maxIntervalMs - config.minIntervalMs) * fatigue;
  const delayMs = baseInterval + config.jitterMs * safeRoll;
  const durationMs =
    config.minDurationMs + (config.maxDurationMs - config.minDurationMs) * fatigue;

  return { delayMs, durationMs };
}
