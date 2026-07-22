/**
 * Worker / Skill status → physical pet expression for the desktop lifeform.
 *
 * Milestone 2 ("personality & physiology") asks the pet to embody the state of
 * the background workers that are its "body": a busy worker makes the pet break
 * a sweat, a crash leaves it seeing stars, a timeout has it dozing off, and a
 * success makes it bounce with delight. This is the difference between a status
 * bar and a creature that *feels* the work happening inside it.
 *
 * The existing `agentCompanion` module already maps agent-task status to a
 * coarse action (work / celebrate / idle) for the companion bubble. This module
 * is complementary and narrower: it maps a worker lifecycle state to a
 * content-free *physical expression cue* (sweat / bounce / smoke / snore) the
 * character layer can overlay, without duplicating the bubble's messaging.
 *
 * It is a pure, DOM-free mapping — no React, no window, no IPC — so it
 * unit-tests in isolation and stays inside the architecture boundary. The
 * caller owns which worker states it observes and when it clears the cue.
 */

/**
 * The lifecycle of a background worker or skill execution, as the pet
 * experiences it. These are host-independent labels, not any single module's
 * enum, so the caller maps its own status vocabulary onto them.
 */
export type WorkerVitalityState =
  | "idle"
  | "busy"
  | "succeeded"
  | "failed"
  | "timed_out";

/** A physical expression the pet adopts in response to worker activity. */
export type VitalityExpression = "none" | "sweat" | "bounce" | "smoke" | "snore";

/** The pet's embodied reaction to the workers that are its body. */
export interface WorkerVitalityReaction {
  /** Physical cue the character overlays (sweat drop, dizzy smoke, etc.). */
  expression: VitalityExpression;
  /**
   * Energy drain per busy tick, in `0..1` of a full bar — the pet tires as its
   * workers labor. Zero for non-busy states; the caller applies it to its own
   * energy model (this module does not own energy).
   */
  exertion: number;
  /**
   * Whether the cue is sustained (held while the state persists, e.g. sweating
   * while busy) or transient (a one-shot pulse, e.g. a success bounce).
   */
  sustained: boolean;
  /** A short, content-free status line — reflects the worker fact, not its work. */
  cue: string;
}

/** The resting reaction: no worker activity, no expression. */
export const NEUTRAL_VITALITY: WorkerVitalityReaction = {
  expression: "none",
  exertion: 0,
  sustained: false,
  cue: "",
};

/** Energy drained per busy sample while workers labor. */
const BUSY_EXERTION = 0.04;

/**
 * Maps a worker/skill lifecycle state to the pet's physical reaction.
 *
 * - `busy`: the pet breaks a sweat and slowly tires (sustained sweat cue).
 * - `succeeded`: a transient bounce of delight.
 * - `failed`: a transient puff of dizzy smoke (the "crash" tell).
 * - `timed_out`: the pet dozes off — a snore — since the worker stalled.
 * - `idle` / unknown: no expression.
 *
 * An unknown state falls back to {@link NEUTRAL_VITALITY} rather than throwing,
 * so a future worker state can never break the renderer.
 */
export function workerVitalityReaction(state: string): WorkerVitalityReaction {
  switch (state) {
    case "busy":
      return { expression: "sweat", exertion: BUSY_EXERTION, sustained: true, cue: "忙得满头大汗" };
    case "succeeded":
      return { expression: "bounce", exertion: 0, sustained: false, cue: "成功啦，蹦一个！" };
    case "failed":
      return { expression: "smoke", exertion: 0, sustained: false, cue: "出错了，头上冒烟" };
    case "timed_out":
      return { expression: "snore", exertion: 0, sustained: true, cue: "等太久，打起了呼噜" };
    case "idle":
      return NEUTRAL_VITALITY;
    default:
      return NEUTRAL_VITALITY;
  }
}

/**
 * Reports whether a vitality expression is a distress signal (the worker is in
 * trouble). Useful for callers deciding whether to raise a feedback-priority
 * attention request versus an ambient one.
 */
export function isDistressExpression(expression: VitalityExpression): boolean {
  return expression === "smoke";
}

/**
 * Maps an agent-companion status (the vocabulary the desktop host publishes on
 * its companion signal) onto a {@link WorkerVitalityState}. This lets the
 * overlay drive the physical expression from the signal it already receives,
 * without inventing a second worker-status channel.
 *
 * - `thinking` / `running` → `busy` (the pet labors and sweats)
 * - `completed` → `succeeded` (a happy bounce)
 * - `failed` → `failed` (dizzy smoke)
 * - `cancelled` / `waiting_for_confirmation` → `idle` (no distress cue)
 *
 * Unknown statuses map to `idle` so a future status can never break rendering.
 */
export function vitalityStateFromCompanionStatus(status: string): WorkerVitalityState {
  switch (status) {
    case "thinking":
    case "running":
      return "busy";
    case "completed":
      return "succeeded";
    case "failed":
      return "failed";
    case "waiting_for_confirmation":
    case "cancelled":
      return "idle";
    default:
      return "idle";
  }
}
