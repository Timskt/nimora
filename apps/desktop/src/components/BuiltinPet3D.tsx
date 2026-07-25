import { useEffect, useRef } from "react";
import {
  AmbientLight,
  CapsuleGeometry,
  CircleGeometry,
  Color,
  DirectionalLight,
  Group,
  MathUtils,
  Mesh,
  MeshBasicMaterial,
  MeshPhysicalMaterial,
  OrthographicCamera,
  Scene,
  SphereGeometry,
  Timer,
  TorusGeometry,
  WebGLRenderer,
} from "three";
import { createContactShadowTexture } from "./petSceneHelpers";
import type { LifeformAttention, LifeformMicroAct } from "./lifeformLiving";
import {
  createLifeformPerfTracker,
  createPerfEmitGate,
  LIFEFORM_PERF_EVENT,
  recordFrame,
  summarize,
  type LifeformPerfSummary,
} from "./lifeformPerf";

interface BuiltinPet3DProps {
  state: string;
  emotion: string;
  /** Living-presence attention target for ambient gaze when the pointer is stale. */
  attention?: LifeformAttention | undefined;
  /** Living-presence contextual micro-act hint to bias idle choreography. */
  microAct?: LifeformMicroAct | undefined;
  onFailure(): void;
  /** Optional throttled (~500ms) render budget summary for Control Center. */
  onPerfSummary?: (summary: LifeformPerfSummary) => void;
}

export interface BuiltinPetPose {
  bounce: number;
  bodyTilt: number;
  eyeScale: number;
  tailSpeed: number;
  breath: number;
  armRest: number;
  hop: number;
  squash: number;
  stretch: number;
  lookAround: number;
  shadowPulse: number;
  /** Forward lean / slump amount. */
  slump: number;
  /** Work stress intensity used for sweat VFX. */
  sweat: number;
}

export interface BuiltinPetMotionSample {
  rootY: number;
  scaleX: number;
  scaleY: number;
  bodyTilt: number;
  bodyYaw: number;
  headYaw: number;
  headPitch: number;
  /** Shared blink/open scale (mean of dual goggles). */
  eyeScaleY: number;
  /** Left goggle vertical scale for asymmetric blink. */
  eyeScaleYL: number;
  /** Right goggle vertical scale for asymmetric blink. */
  eyeScaleYR: number;
  irisX: number;
  irisY: number;
  armL: number;
  armR: number;
  footLY: number;
  footRY: number;
  footLZ: number;
  footRZ: number;
  shadowScale: number;
  shadowOpacity: number;
  mouthOpen: number;
  /** 0..1 work sweat / glisten strength. */
  sweat: number;
  /** 0..1 celebrate fireworks-lite strength. */
  sparkle: number;
  /** 0..1 crash / wounded daze strength. */
  dazed: number;
  /** 0..1 subtle dig-nose micro-act. */
  digNose: number;
  /** 0..1 count-ants idle micro-performance. */
  countAnts: number;
  /** Secondary hair lean target for spring-driven crown tufts. */
  hairSway: number;
  /** 0..1 cheek flush / blush intensity. */
  cheekFlush: number;
  /** Mouth arc: +1 smile, -1 frown (connector offline / sad). */
  mouthSmile: number;
}

/** After the pointer is idle this long, ambient attention takes over gaze. */
export const POINTER_ATTENTION_STALE_MS = 2600;

export function clampGaze(value: number): number {
  return MathUtils.clamp(value, -1, 1);
}

/**
 * Ambient gaze bias for a living-moment attention target (screen space, -1..1).
 *
 * When the pointer is stale the pet should look at whatever the living-presence
 * engine decided is interesting (taskbar, notification tray, another display,
 * the sky…) instead of freezing on the last cursor spot. Directions are gentle
 * so the head still reads as glancing, not snapping to a wall.
 */
export function attentionGazeBias(attention?: LifeformAttention): { x: number; y: number } {
  switch (attention) {
    case "user":
    case "cursor":
      // Toward the person / pointer: near-center, slight upward warmth.
      return { x: 0, y: -0.12 };
    case "taskbar":
      // Down toward the dock / taskbar strip.
      return { x: 0.12, y: 0.72 };
    case "notification":
      // Toward the notification corner (top-right on both platforms).
      return { x: 0.7, y: -0.62 };
    case "window-edge":
      return { x: 0.6, y: -0.1 };
    case "battery":
      // Menu-bar / tray battery indicator (top-right-ish).
      return { x: 0.66, y: -0.5 };
    case "other-display":
      // Look off toward a neighbouring monitor.
      return { x: -0.85, y: -0.05 };
    case "ants":
      // Peer down at the "ants" on the desktop floor.
      return { x: -0.22, y: 0.66 };
    case "sky":
      return { x: 0.08, y: -0.78 };
    case "work":
      // Focused slightly down at the work surface.
      return { x: -0.1, y: 0.24 };
    case "self":
    default:
      return { x: 0, y: 0.08 };
  }
}

/** Idle micro-act channel gains for a living-presence micro-act hint. */
export interface MicroActBiasGain {
  yawn: number;
  dig: number;
  ants: number;
  wave: number;
  hop: number;
}

/**
 * Idle micro-performance gains for the living-presence engine's contextual
 * micro-act choice.
 *
 * The renderer already cycles yawn / dig-nose / count-ants on internal timers,
 * but the engine decides which act *fits the moment* (yawn when energy is low at
 * night, count-ants when bored). This gently boosts the matching idle channel
 * and softens the rest so the contextual act reads as the dominant one — without
 * hard-snapping to a directed pose. `"none"` returns all-1 gains, so an absent
 * hint keeps idle choreography byte-identical to the untinted baseline.
 */
export function microActBiasGain(hint?: LifeformMicroAct): MicroActBiasGain {
  const base: MicroActBiasGain = { yawn: 1, dig: 1, ants: 1, wave: 1, hop: 1 };
  switch (hint) {
    case "yawn":
      return { ...base, yawn: 1.55, dig: 0.7, ants: 0.7 };
    case "dig_nose":
      return { ...base, dig: 1.55, yawn: 0.7, ants: 0.7 };
    case "count_ants":
      return { ...base, ants: 1.55, yawn: 0.7, dig: 0.7 };
    case "wave":
      return { ...base, wave: 1.6, hop: 0.85 };
    case "hop":
      return { ...base, hop: 1.6, wave: 0.85 };
    case "none":
    default:
      return base;
  }
}

/** Warm classic Q-minion palette (no chrome plate / no square frame). */
export const Q_MINION_COLORS = {
  /** Classic film minion yellow — saturated, warm, candy-like. */
  yellow: "#FFE01A",
  yellowLight: "#FFF38A",
  yellowEmissive: "#FFC400",
  denim: "#2A78B0",
  denimSoft: "#4A99CC",
  denimDeep: "#1F5F8A",
  goggleBand: "#0E1016",
  /** Soft brushed metal — buckle, never chrome plate. */
  metal: "#DDE3EA",
  sclera: "#FFFEF9",
  /** Warm cocoa iris — soft Q-minion, not tech cyan. */
  iris: "#6A3A1C",
  irisEmissive: "#3A2010",
  glove: "#14151A",
  boot: "#6B4224",
  bootSole: "#352212",
  hair: "#12131A",
  blush: "#FF8A78",
  graphite: "#151820",
} as const;

/** Tall warm pill + dual goggle layout (classic Q-minion proportions). */
export const Q_MINION_LAYOUT = {
  /** Cylinder radius of the yellow capsule — chubbier Q silhouette. */
  bodyRadius: 1.0,
  /** Cylinder length between hemispheres (total height ≈ length + 2r). */
  bodyLength: 1.06,
  bodyY: -0.26,
  headY: 0.5,
  /** Oversized dual goggles = cuter Q face / Bob vibe. */
  goggleRadius: 0.56,
  goggleSpacing: 0.49,
  goggleZ: 1.02,
  overallY: -0.76,
  bootY: -1.26,
  armX: 1.04,
  hairY: 0.9,
  shadowY: -1.4,
} as const;

export type QMinionVec3 = readonly [number, number, number];

/** Numeric silhouette fields used by classic Q-minion recognition (not the full const layout). */
export type QMinionSilhouetteLayout = {
  bodyRadius: number;
  bodyLength: number;
  goggleRadius: number;
  goggleSpacing: number;
  bootY: number;
};

/**
 * Sparse black crown tufts: [x, y, z, tiltZ].
 * Classic minion has only a few thin hairs — keep sparse.
 */
export function qMinionHairTuftSpecs(): ReadonlyArray<readonly [number, number, number, number]> {
  return [
    [-0.14, 0.04, 0.03, -0.34],
    [0.01, 0.12, -0.04, 0.06],
    [0.15, 0.03, 0.05, 0.38],
    [-0.05, 0.07, -0.08, -0.12],
    [0.09, 0.02, 0.08, 0.22],
  ] as const;
}

/** Dual goggle group position for side (-1 left, +1 right). */
export function qMinionGogglePose(side: -1 | 1): QMinionVec3 {
  const { goggleSpacing, goggleZ } = Q_MINION_LAYOUT;
  return [side * goggleSpacing, 0.02, goggleZ] as const;
}

/** Denim strap + metal button placement for overall suspenders. */
export function qMinionOverallStrapPose(side: -1 | 1): {
  strap: QMinionVec3;
  rotationZ: number;
  button: QMinionVec3;
} {
  return {
    strap: [side * 0.42, -0.08, 0.78] as const,
    rotationZ: side * -0.38,
    button: [side * 0.32, -0.36, 1.02] as const,
  };
}

/**
 * True when layout reads as a tall Q-pill with dual large goggles
 * (not a square plate, not a single tiny eye).
 */
export function isClassicQMinionSilhouette(layout: QMinionSilhouetteLayout = Q_MINION_LAYOUT): boolean {
  const diameter = layout.bodyRadius * 2;
  const totalHeight = layout.bodyLength + diameter;
  return (
    layout.bodyLength > layout.bodyRadius * 1.05
    && layout.bodyLength < layout.bodyRadius * 1.35
    && totalHeight > diameter * 1.45
    && layout.goggleRadius >= 0.36
    && layout.goggleSpacing > layout.goggleRadius * 0.75
    && layout.goggleSpacing < layout.goggleRadius * 1.15
    && layout.bootY < -1.2
  );
}


/** Position + velocity sample for secondary spring-damper channels. */
export interface SpringState {
  value: number;
  velocity: number;
}

export function createSpringState(value = 0): SpringState {
  return { value, velocity: 0 };
}

/**
 * Semi-implicit Euler spring-damper (not linear lerp).
 * omega = natural frequency (rad/s), zeta = damping ratio (1 ≈ critical).
 */
export function stepSpringDamper(
  state: SpringState,
  target: number,
  dtSeconds: number,
  omega = 14,
  zeta = 0.82,
): SpringState {
  const dt = MathUtils.clamp(dtSeconds, 0, 1 / 20);
  // x'' + 2ζω x' + ω² (x - target) = 0
  const accel = (-2 * zeta * omega * state.velocity) - (omega * omega * (state.value - target));
  const velocity = state.velocity + accel * dt;
  const value = state.value + velocity * dt;
  return { value, velocity };
}

/** Convenience: advance spring and return the new position. */
export function springToward(
  state: SpringState,
  target: number,
  dtSeconds: number,
  omega = 14,
  zeta = 0.82,
): number {
  const next = stepSpringDamper(state, target, dtSeconds, omega, zeta);
  state.value = next.value;
  state.velocity = next.velocity;
  return next.value;
}

/** Normalize host directive / lifecycle tokens into BuiltinPet3D motion keys. */
export function normalizeMotionState(state: string): string {
  const raw = state.trim().toLowerCase();
  const token = raw.startsWith("pet.") ? raw.slice(4) : raw;
  // Structured directive actions → motion vocabulary.
  if (token === "work_busy" || token === "work-busy" || token === "working") return "work";
  if (token === "work_crash" || token === "work-crash" || token === "crash") return "crash";
  if (token === "playing" || token === "play") return "play";
  if (token === "walking" || token === "walk") return "walk";
  if (token === "sleeping" || token === "sleep" || token === "rest") return "sleep";
  if (token === "observing" || token === "observe") return "observe";
  if (token === "celebrate" || token === "celebration") return "celebrate";
  if (token === "perch" || token === "perching") return "perch";
  // First-class micro-performance directive tokens.
  if (token === "yawn") return "yawn";
  if (token === "dig_nose" || token === "dig-nose" || token === "dignose") return "dig_nose";
  if (token === "count_ants" || token === "count-ants" || token === "countants") return "count_ants";
  if (token === "wave" || token === "waving") return "wave";
  if (token === "look_around" || token === "look-around" || token === "lookaround") return "look_around";
  if (token === "hop" || token === "hopping") return "hop";
  return token;
}

function isWorking(state: string): boolean {
  const s = normalizeMotionState(state);
  return s === "work" || s === "working";
}

function isPlaying(state: string): boolean {
  const s = normalizeMotionState(state);
  return s === "playing" || s === "play" || s === "celebrate";
}

function isWalking(state: string): boolean {
  const s = normalizeMotionState(state);
  return s === "walking" || s === "walk";
}

function isSleeping(state: string, emotion: string): boolean {
  const s = normalizeMotionState(state);
  return s === "sleeping" || s === "sleep" || s === "rest" || emotion === "sleepy";
}

function isObserving(state: string, emotion: string): boolean {
  const s = normalizeMotionState(state);
  return (
    s === "observing"
    || s === "observe"
    || s === "perch"
    || s === "look_around"
    || emotion === "surprised"
  );
}

function isDirectedYawn(state: string): boolean {
  return normalizeMotionState(state) === "yawn";
}

function isDirectedDigNose(state: string): boolean {
  return normalizeMotionState(state) === "dig_nose";
}

function isDirectedCountAnts(state: string): boolean {
  return normalizeMotionState(state) === "count_ants";
}

function isDirectedWave(state: string): boolean {
  return normalizeMotionState(state) === "wave";
}

function isDirectedHop(state: string): boolean {
  return normalizeMotionState(state) === "hop";
}

/** Work crash / wounded strings drive a soft dazed pose. */
export function isDazedState(state: string, emotion: string): boolean {
  const s = normalizeMotionState(state);
  const e = emotion.toLowerCase();
  return (
    s.includes("crash")
    || s.includes("error")
    || s === "work_crash"
    || e === "wounded"
    || e === "hurt"
    || e === "dazed"
    || e === "dizzy"
  );
}

/** Connector offline / low mood → soft sad silhouette (not crash daze). */
export function isSadEmotion(emotion: string): boolean {
  const e = emotion.toLowerCase();
  return e === "sad" || e === "low" || e === "lonely" || e === "upset";
}

export function builtinPetPose(state: string, emotion: string): BuiltinPetPose {
  if (isDazedState(state, emotion)) {
    return {
      bounce: 0.016,
      bodyTilt: 0.2,
      eyeScale: 0.52,
      tailSpeed: 0.28,
      breath: 0.018,
      armRest: 0.18,
      hop: 0,
      squash: 1.12,
      stretch: 0.86,
      lookAround: 0.1,
      shadowPulse: 0.035,
      slump: 0.24,
      sweat: 0.55,
    };
  }
  if (isSadEmotion(emotion) && !isWorking(state) && !isPlaying(state)) {
    return {
      bounce: 0.022,
      bodyTilt: 0.09,
      eyeScale: 0.78,
      tailSpeed: 0.48,
      breath: 0.024,
      armRest: 0.22,
      hop: 0,
      squash: 1.06,
      stretch: 0.93,
      lookAround: 0.16,
      shadowPulse: 0.028,
      slump: 0.18,
      sweat: 0.05,
    };
  }
  if (isSleeping(state, emotion)) {
    return {
      bounce: 0.008,
      bodyTilt: 0.08,
      eyeScale: 0.04,
      tailSpeed: 0.16,
      breath: 0.042,
      armRest: 0.2,
      hop: 0,
      squash: 1.12,
      stretch: 0.88,
      lookAround: 0.01,
      shadowPulse: 0.02,
      slump: 0.16,
      sweat: 0,
    };
  }
  if (isWorking(state)) {
    return {
      bounce: 0.018,
      bodyTilt: 0.06,
      eyeScale: 0.88,
      tailSpeed: 0.55,
      breath: 0.014,
      armRest: 0.58,
      hop: 0,
      squash: 1.07,
      stretch: 0.93,
      lookAround: 0.1,
      shadowPulse: 0.028,
      slump: 0.2,
      sweat: 0.98,
    };
  }
  if (isWalking(state)) {
    return {
      bounce: 0.11,
      bodyTilt: 0.07,
      eyeScale: 1.02,
      tailSpeed: 2.2,
      breath: 0.016,
      armRest: 0.1,
      hop: 0,
      squash: 0.98,
      stretch: 1.04,
      lookAround: 0.22,
      shadowPulse: 0.06,
      slump: 0,
      sweat: 0.1,
    };
  }
  if (isPlaying(state) || emotion === "happy") {
    return {
      bounce: 0.24,
      bodyTilt: 0.12,
      eyeScale: 1.14,
      tailSpeed: 3.2,
      breath: 0.028,
      armRest: 0.92,
      hop: 0.3,
      squash: 0.82,
      stretch: 1.26,
      lookAround: 0.48,
      shadowPulse: 0.14,
      slump: 0,
      sweat: 0,
    };
  }
  if (isObserving(state, emotion)) {
    return {
      bounce: 0.024,
      bodyTilt: 0.04,
      eyeScale: 1.28,
      tailSpeed: 1.4,
      breath: 0.014,
      armRest: 0.12,
      hop: 0,
      squash: 0.98,
      stretch: 1.05,
      lookAround: 0.82,
      shadowPulse: 0.035,
      slump: -0.03,
      sweat: 0,
    };
  }
  const motion = normalizeMotionState(state);
  if (motion === "yawn") {
    return {
      bounce: 0.024,
      bodyTilt: 0.07,
      eyeScale: 0.42,
      tailSpeed: 0.42,
      breath: 0.048,
      armRest: 0.55,
      hop: 0,
      squash: 1.1,
      stretch: 0.9,
      lookAround: 0.06,
      shadowPulse: 0.03,
      slump: 0.1,
      sweat: 0,
    };
  }
  if (motion === "dig_nose") {
    return {
      bounce: 0.034,
      bodyTilt: 0.08,
      eyeScale: 0.9,
      tailSpeed: 0.9,
      breath: 0.02,
      armRest: 0.64,
      hop: 0,
      squash: 1.05,
      stretch: 0.96,
      lookAround: 0.2,
      shadowPulse: 0.032,
      slump: 0.07,
      sweat: 0,
    };
  }
  if (motion === "count_ants") {
    return {
      bounce: 0.028,
      bodyTilt: 0.12,
      eyeScale: 1.24,
      tailSpeed: 1.0,
      breath: 0.016,
      armRest: 0.42,
      hop: 0,
      squash: 1.08,
      stretch: 0.94,
      lookAround: 0.38,
      shadowPulse: 0.034,
      slump: 0.16,
      sweat: 0,
    };
  }
  if (motion === "wave") {
    return {
      bounce: 0.1,
      bodyTilt: 0.07,
      eyeScale: 1.12,
      tailSpeed: 1.85,
      breath: 0.024,
      armRest: 0.95,
      hop: 0.06,
      squash: 0.94,
      stretch: 1.1,
      lookAround: 0.4,
      shadowPulse: 0.07,
      slump: 0,
      sweat: 0,
    };
  }
  if (motion === "look_around") {
    return {
      bounce: 0.026,
      bodyTilt: 0.04,
      eyeScale: 1.28,
      tailSpeed: 1.45,
      breath: 0.014,
      armRest: 0.12,
      hop: 0,
      squash: 0.98,
      stretch: 1.05,
      lookAround: 0.95,
      shadowPulse: 0.035,
      slump: -0.03,
      sweat: 0,
    };
  }
  if (motion === "hop") {
    return {
      bounce: 0.16,
      bodyTilt: 0.1,
      eyeScale: 1.1,
      tailSpeed: 2.5,
      breath: 0.026,
      armRest: 0.62,
      hop: 0.3,
      squash: 0.84,
      stretch: 1.24,
      lookAround: 0.32,
      shadowPulse: 0.12,
      slump: 0,
      sweat: 0,
    };
  }
  if (state === "stretch" || state === "stretching" || motion === "stretch") {
    return {
      bounce: 0.03,
      bodyTilt: 0.04,
      eyeScale: 1.05,
      tailSpeed: 1.1,
      breath: 0.03,
      armRest: 0.55,
      hop: 0.02,
      squash: 0.94,
      stretch: 1.1,
      lookAround: 0.2,
      shadowPulse: 0.04,
      slump: -0.04,
      sweat: 0,
    };
  }
  return {
    bounce: 0.05,
    bodyTilt: 0.065,
    eyeScale: 1.04,
    tailSpeed: 1.4,
    breath: 0.034,
    armRest: 0.22,
    hop: 0,
    squash: 0.98,
    stretch: 1.04,
    lookAround: 0.78,
    shadowPulse: 0.07,
    slump: 0,
    sweat: 0,
  };
}

export function builtinPetBodyYaw(state: string, elapsed: number, gazeX: number): number {
  if (isPlaying(state)) return elapsed * 1.35;
  if (isWalking(state)) return Math.sin(elapsed * 0.72) >= 0 ? Math.PI * 0.28 : -Math.PI * 0.28;
  if (isObserving(state, "")) {
    return Math.sin(elapsed * 0.62) * Math.PI * 0.42 + gazeX * 0.18;
  }
  if (isDirectedHop(state) || isDirectedWave(state)) {
    // Celebrate-lite: soft sway only — no full-body spin / linear travel.
    return Math.sin(elapsed * 1.6) * 0.18 + gazeX * 0.12;
  }
  if (isWorking(state)) return Math.sin(elapsed * 0.35) * 0.14 + gazeX * 0.16;
  if (isDazedState(state, "")) return Math.sin(elapsed * 0.9) * 0.42 + gazeX * 0.08;
  // Idle: clear body turn toward pointer + slow ambient sweep for 360° feel.
  return gazeX * 0.42 + Math.sin(elapsed * 0.28) * 0.14 + Math.sin(elapsed * 0.11) * 0.06;
}

/** Soft sin envelope over [start, end) of a modular cycle (deterministic). */
export function cycleEnvelope(elapsed: number, period: number, start: number, end: number): number {
  if (!(period > 0) || !(end > start)) return 0;
  const c = ((elapsed % period) + period) % period;
  if (c < start || c >= end) return 0;
  return Math.sin(((c - start) / (end - start)) * Math.PI);
}

/** Deterministic micro-performance gates for idle liveliness. */
export function idlePerformancePhase(elapsed: number): {
  blink: boolean;
  /** Left-eye blink gate (slightly leads for asymmetry). */
  blinkL: boolean;
  /** Right-eye blink gate (slight lag / solo wink). */
  blinkR: boolean;
  yawn: number;
  weightShift: number;
  lookBurst: number;
  digNose: number;
  /** 0..1 lean-and-peer "counting ants" act (idle only). */
  countAnts: number;
  /** 0..1 soft bounce / settle hop so idle is never flat. */
  microBounce: number;
  /** -1..1 look-around sweep for idle head/gaze liveliness. */
  lookSweep: number;
  /** 0..1 soft cheek flush pulse (emotion / micro-act driven in sample). */
  cheekPulse: number;
  /** 0..1 discrete fidget peak (arm/head micro-act; ≤3s gaps). */
  fidget: number;
  /** 0..1 settle-hop peak for grounded rubber bounce. */
  settleHop: number;
  /** 0..1 soft wave-lite micro-act while idle. */
  waveLite: number;
  /** 0..1 soft hop-lite micro-act while idle. */
  hopLite: number;
} {
  const blinkCycle = ((elapsed * 0.58) % 3.55);
  // Primary blink plus a rarer double-blink tail so eyes stay alive.
  const blinkBoth = (blinkCycle > 3.18 && blinkCycle < 3.36)
    || (blinkCycle > 3.4 && blinkCycle < 3.48);
  // Asymmetric extras: left leads the close; rare right-only wink.
  const winkCycle = ((elapsed * 0.38) % 5.9);
  const winkR = winkCycle > 5.35 && winkCycle < 5.58;
  const blinkL = blinkBoth || (blinkCycle > 3.14 && blinkCycle < 3.26);
  const blinkR = blinkBoth || winkR || (blinkCycle > 3.24 && blinkCycle < 3.4);
  const blink = blinkL || blinkR;
  // Yawn ~every 7.2s with a readable open window.
  const yawn = cycleEnvelope(elapsed, 7.2, 5.55, 6.95);
  // Continuous weight rock — never a flat zero.
  const weightShift = MathUtils.clamp(
    0.5 + Math.sin(elapsed * 0.78) * 0.4 + Math.sin(elapsed * 1.55) * 0.14,
    0,
    1,
  );
  // Continuous look product + discrete glance bursts (~every 2.55s).
  const lookBurst = Math.sin(elapsed * 0.28) * Math.sin(elapsed * 0.72);
  const glance = cycleEnvelope(elapsed, 2.55, 2.05, 2.48);
  // Offset look-around sweep peak so it interleaves with settle hop.
  const lookAroundPeak = cycleEnvelope(elapsed + 1.15, 2.7, 2.15, 2.62);
  const lookSweep = MathUtils.clamp(
    Math.sin(elapsed * 0.5) * 0.9
      + Math.sin(elapsed * 1.25) * 0.42
      + Math.sin(elapsed * 0.2) * 0.22
      + glance * 1.05
      + lookAroundPeak * 0.82,
    -1,
    1,
  );
  // Soft continuous bounce + settle hop (~every 2.4s) so idle never freezes.
  const settleHop = cycleEnvelope(elapsed, 2.4, 1.98, 2.34);
  const microBounce = MathUtils.clamp(
    0.58
      + 0.5 * Math.sin(elapsed * 2.7)
      + settleHop * 1.05
      + Math.sin(elapsed * 0.95) * 0.18
      + Math.sin(elapsed * 5.4) * 0.09,
    0,
    1,
  );
  // Arm/head fidget peaks every ~2.35s, phase-offset from settle hop.
  const fidget = MathUtils.clamp(
    cycleEnvelope(elapsed + 1.05, 2.35, 1.9, 2.28)
      + cycleEnvelope(elapsed + 0.4, 4.7, 3.9, 4.45) * 0.55,
    0,
    1,
  );
  // Soft wave-lite / hop-lite so classic QQ micro-acts keep cycling.
  const waveLite = cycleEnvelope(elapsed + 0.6, 6.2, 5.15, 5.95);
  const hopLite = cycleEnvelope(elapsed + 2.1, 5.5, 4.7, 5.25);
  // Dig-nose pulse every ~8.8s.
  const digNose = cycleEnvelope(elapsed, 8.8, 7.2, 8.35);
  // Count ants: lean/peer window ~every 11.8s, offset from yawn/dig.
  const antsCycle = ((elapsed * 0.11) % 11.8);
  const antsEnvelope = antsCycle > 8.5 && antsCycle < 11.1
    ? Math.sin(((antsCycle - 8.5) / 2.6) * Math.PI)
    : 0;
  const antsPoint = antsEnvelope > 0.3
    ? Math.max(0, Math.sin((antsCycle - 8.85) * 2.8) * 0.9)
    : 0;
  const countAnts = MathUtils.clamp(antsEnvelope * 0.88 + antsPoint * 0.34, 0, 1);
  // Soft cheek pulse: peaks during micro-acts and a slow ambient wave.
  const cheekPulse = MathUtils.clamp(
    0.2
      + yawn * 0.45
      + digNose * 0.3
      + countAnts * 0.24
      + fidget * 0.18
      + waveLite * 0.22
      + hopLite * 0.16
      + Math.max(0, Math.sin(elapsed * 0.62)) * 0.14
      + (blink ? 0.1 : 0),
    0,
    1,
  );
  return {
    blink,
    blinkL,
    blinkR,
    yawn,
    weightShift,
    lookBurst,
    digNose,
    countAnts,
    microBounce,
    lookSweep,
    cheekPulse,
    fidget,
    settleHop,
    waveLite,
    hopLite,
  };
}

/**
 * Composite 0..1 intensity of idle micro-performances.
 * Guarantees a readable peak at least every 3s (settle hop / glance / fidget).
 */
export function idleMicroActIntensity(elapsed: number): number {
  const p = idlePerformancePhase(elapsed);
  const glancePeak = Math.max(0, Math.abs(p.lookSweep) - 0.42) * 1.35;
  const bouncePeak = p.microBounce > 0.78 ? (p.microBounce - 0.55) * 1.6 : 0;
  return MathUtils.clamp(
    Math.max(
      p.yawn,
      p.digNose,
      p.countAnts,
      p.fidget,
      p.settleHop,
      p.waveLite,
      p.hopLite,
      glancePeak,
      bouncePeak,
      p.blink ? 0.45 : 0,
    ),
    0,
    1,
  );
}

export function sampleBuiltinPetMotion(
  state: string,
  emotion: string,
  elapsed: number,
  gazeX: number,
  gazeY: number,
  motion = 1,
  microActHint: LifeformMicroAct = "none",
): BuiltinPetMotionSample {
  const pose = builtinPetPose(state, emotion);
  const walking = isWalking(state);
  const playing = isPlaying(state);
  const sleeping = isSleeping(state, emotion);
  const working = isWorking(state);
  const observing = isObserving(state, emotion);
  const dazedAmt = isDazedState(state, emotion) ? 1 : 0;
  const sadAmt = (!dazedAmt && isSadEmotion(emotion)) ? 1 : 0;
  const directedYawn = isDirectedYawn(state);
  const directedDig = isDirectedDigNose(state);
  const directedAnts = isDirectedCountAnts(state);
  const waving = isDirectedWave(state);
  const hopping = isDirectedHop(state);
  const micro = idlePerformancePhase(elapsed);
  const microGain = microActBiasGain(microActHint);
  const m = MathUtils.clamp(motion, 0, 1);
  // Directed micro-performances stay local (no linear body travel) and do not
  // steal observe/play locomotion channels unless they map there themselves.
  const idleish = (
    !walking && !playing && !working && !sleeping && !observing
    && !waving && !hopping && dazedAmt < 0.5
  ) || directedYawn || directedDig || directedAnts;

  const yawnAmt = MathUtils.clamp(
    (idleish && !directedYawn && !directedDig && !directedAnts ? micro.yawn * microGain.yawn : 0)
      + (directedYawn ? 0.95 : 0),
    0,
    1,
  );
  const digAmt = MathUtils.clamp(
    (idleish && !directedYawn && !directedAnts ? micro.digNose * microGain.dig : 0)
      + (directedDig ? 0.95 : 0),
    0,
    1,
  );
  const ants = MathUtils.clamp(
    (idleish && !directedYawn && !directedDig ? micro.countAnts * microGain.ants : 0)
      + (directedAnts ? 0.92 : 0),
    0,
    1,
  );

  const pureIdle = idleish && !directedYawn && !directedDig && !directedAnts;
  const idleBounce = pureIdle ? micro.microBounce : micro.microBounce * (idleish ? 0.45 : 0.2);
  const settleBoost = pureIdle ? micro.settleHop : 0;
  // Hop channel is spring-friendly vertical squash only — never linear body travel.
  const hopLiteAmt = pureIdle ? micro.hopLite * microGain.hop : 0;
  const waveLiteAmt = pureIdle ? micro.waveLite * microGain.wave : 0;
  const fidgetAmt = pureIdle ? micro.fidget : micro.fidget * 0.2;
  // Hop is vertical squash/stretch only — land has a rubbery settle beat.
  const hopPhase = hopping
    ? Math.abs(Math.sin(elapsed * 5.2))
    : playing
      ? Math.abs(Math.sin(elapsed * 5.6))
      : 0;
  const hopWave = playing
    ? hopPhase * pose.hop
    : hopping
      ? hopPhase * Math.max(pose.hop, 0.24)
      : walking
        ? Math.abs(Math.sin(elapsed * 5.4)) * pose.bounce
        : Math.abs(Math.sin(elapsed * (pose.tailSpeed + 0.75))) * pose.bounce
          + idleBounce * pose.bounce * 1.05
          + hopLiteAmt * 0.2
          + settleBoost * 0.08
          + yawnAmt * 0.02;

  const breathRate = sleeping ? 0.78 : working ? 2.5 : 1.75;
  const breath = Math.sin(elapsed * breathRate) * pose.breath;
  const slumpDrop = pose.slump * 0.26 + dazedAmt * 0.05 + sadAmt * 0.04 + ants * 0.045;
  const rootY = (
    -0.01
    + hopWave
    + breath * (sleeping ? 0.62 : 0.4)
    + idleBounce * 0.032
    + settleBoost * 0.055
    - slumpDrop
    + (sleeping ? -0.05 : 0)
  ) * m + (-0.01) * (1 - m);

  // Land squash: stronger when hop phase is falling / settle hop peaks.
  const hopLand = (playing || hopping)
    ? Math.max(0, Math.sin(elapsed * (playing ? 5.6 : 5.2) + Math.PI) * 0.14)
    : 0;
  const settleLand = idleish
    ? Math.max(0, Math.sin(elapsed * 2.25) * 0.014)
      + (micro.microBounce > 0.78 ? (micro.microBounce - 0.78) * 0.2 : 0)
      + settleBoost * 0.1
    : 0;
  const squashPulse = playing
    ? 1 + Math.sin(elapsed * 5.6) * 0.16 + hopLand
    : hopping
      ? 1 + Math.sin(elapsed * 5.2) * 0.14 + hopLand
      : walking
        ? 1 + Math.sin(elapsed * 8.2) * 0.05
        : 1
          + Math.sin(elapsed * breathRate) * 0.024
          + yawnAmt * 0.06
          + idleBounce * 0.06
          + dazedAmt * 0.04
          + ants * 0.03
          + sadAmt * 0.02
          + settleLand;
  const scaleX = (pose.squash * squashPulse + (1 - pose.squash)) * m + (1 - m);
  const scaleY = (pose.stretch / Math.max(squashPulse, 0.001) + (1 - pose.stretch)) * m + (1 - m);

  const bodyTilt = (
    Math.sin(elapsed * (walking ? 4.6 : playing ? 3.4 : dazedAmt ? 1.9 : 1.55)) * pose.bodyTilt
    + (micro.weightShift - 0.5) * 0.14 * (walking || playing ? 0.25 : 1)
    + yawnAmt * 0.07
    + pose.slump * 0.2
    + dazedAmt * Math.sin(elapsed * 2.2) * 0.1
    + digAmt * 0.07 * (idleish ? 1 : 0.15)
    + ants * 0.12
    + idleBounce * 0.035 * (idleish ? 1 : 0.25)
    + fidgetAmt * 0.06
    + waveLiteAmt * 0.04
    + hopLiteAmt * 0.03
    + gazeX * 0.05 * (idleish || observing ? 1 : 0.4)
  ) * m;

  const bodyYaw = builtinPetBodyYaw(state, elapsed, gazeX) * m;
  const look = pose.lookAround * (
    observing
      ? micro.lookBurst * 1.55 + Math.sin(elapsed * 0.58) * 0.65 + micro.lookSweep * 0.4
      : micro.lookBurst * 1.05 + micro.lookSweep * (idleish ? 1.0 : 0.38)
  );
  const headYaw = (
    gazeX * (observing ? 0.68 : working ? 0.42 : idleish ? 0.62 : 0.48)
    + look * (observing ? 0.78 : idleish ? 0.68 : 0.55)
    - bodyYaw * 0.18
    + digAmt * 0.16 * (idleish ? 1 : 0)
    + ants * Math.sin(elapsed * 1.15) * 0.08
    + dazedAmt * Math.sin(elapsed * 1.75) * 0.28
    + yawnAmt * Math.sin(elapsed * 2.2) * 0.05
    + fidgetAmt * Math.sin(elapsed * 3.2) * 0.1
    + waveLiteAmt * 0.08
  ) * m;
  const headPitch = (
    gazeY * (observing ? 0.36 : idleish ? 0.28 : 0.18)
    + (sleeping ? 0.24 : 0)
    + pose.slump * 0.72
    + yawnAmt * -0.18
    + Math.sin(elapsed * 0.98) * 0.05 * pose.lookAround
    + digAmt * 0.2 * (idleish ? 1 : 0)
    + ants * 0.32
    + dazedAmt * 0.18
    + sadAmt * 0.14
    + idleBounce * -0.03 * (idleish ? 1 : 0)
    + fidgetAmt * 0.05
    + (working ? 0.08 : 0)
  ) * m;

  const blinkGateL = micro.blinkL || Math.sin(elapsed * 1.08) > 0.986;
  const blinkGateR = micro.blinkR || Math.sin(elapsed * 1.08 + 0.18) > 0.988;
  const openEye = pose.eyeScale * (1 - yawnAmt * 0.4 - digAmt * 0.15 * (idleish ? 1 : 0) - sadAmt * 0.08);
  const eyeScaleYLRaw = (
    sleeping
      ? 0.07 + Math.sin(elapsed * 0.85) * 0.012
      : dazedAmt
        ? 0.52 + Math.sin(elapsed * 2.4) * 0.12
        : blinkGateL
          ? 0.08
          : openEye * (1 - (blinkGateR ? 0.04 : 0))
  );
  const eyeScaleYRRaw = (
    sleeping
      ? 0.07 + Math.sin(elapsed * 0.85 + 0.2) * 0.012
      : dazedAmt
        ? 0.58 + Math.sin(elapsed * 2.4 + 0.4) * 0.1
        : blinkGateR
          ? 0.08
          : openEye * (1 - (blinkGateL ? 0.04 : 0))
  );
  // Reduced motion freezes both goggles fully open and symmetric.
  const eyeScaleYL = eyeScaleYLRaw * m + 1 * (1 - m);
  const eyeScaleYR = eyeScaleYRRaw * m + 1 * (1 - m);
  const eyeScaleY = (eyeScaleYL + eyeScaleYR) * 0.5;

  // Eye IK: snappy gaze follow inside large goggles (Bob "big eyes" read).
  const irisX = (
    gazeX * (observing ? 0.13 : idleish ? 0.095 : 0.07)
    + look * (observing ? 0.08 : idleish ? 0.055 : 0.045)
    + micro.lookSweep * (observing ? 0.028 : idleish ? 0.03 : 0.016) * pose.lookAround
    + ants * Math.sin(elapsed * 1.4) * 0.016
    + dazedAmt * Math.sin(elapsed * 3.2) * 0.04
    + fidgetAmt * Math.sin(elapsed * 2.7) * 0.012
    + digAmt * 0.01 * (idleish ? 1 : 0)
  ) * m;
  const irisY = (
    gazeY * (observing ? 0.08 : idleish ? 0.055 : 0.04)
    + (sleeping ? 0.03 : 0)
    + ants * 0.055
    + idleBounce * -0.014 * (idleish ? 1 : 0)
    + dazedAmt * Math.cos(elapsed * 2.7) * 0.028
    + fidgetAmt * 0.01
    + digAmt * 0.02 * (idleish ? 1 : 0)
    + sadAmt * 0.015
  ) * m;

  const digArm = digAmt * 0.62 * (idleish || directedDig ? 1 : 0);
  // Occasional downward point while counting ants (right arm leads).
  const antsPointPulse = ants * (0.35 + 0.65 * Math.max(0, Math.sin(elapsed * 2.8)));
  const weightLean = (micro.weightShift - 0.5) * (idleish ? 1 : 0.2);
  const armSwing = waving
    ? 0.72 + Math.sin(elapsed * 6.8) * 0.28
    : walking
      ? Math.sin(elapsed * 7) * 0.38
      : playing
        ? 0.78 + Math.sin(elapsed * 5) * 0.28
        : hopping
          ? 0.48 + Math.sin(elapsed * 5.0) * 0.18
          : working
            ? 0.52 + Math.sin(elapsed * 3.2) * 0.12
            : dazedAmt
              ? 0.14 + Math.sin(elapsed * 1.4) * 0.08
              : pose.armRest
                + Math.sin(elapsed * 1.25) * 0.08
                + Math.sin(elapsed * 2.6) * 0.03
                + yawnAmt * 0.28
                + digArm
                + ants * 0.1
                + weightLean * 0.06
                + fidgetAmt * 0.16
                + waveLiteAmt * 0.42
                + hopLiteAmt * 0.12;
  // Wave is celebrate-lite: lead arm high, opposite arm soft counter-swing (no body travel).
  const armL = waving
    ? (0.88 + Math.sin(elapsed * 6.8) * 0.22) * m
    : (armSwing + digArm * 0.35 - antsPointPulse * 0.12 + weightLean * 0.08 + waveLiteAmt * 0.38) * m;
  const armR = waving
    ? (0.12 + Math.sin(elapsed * 6.8 + 1.1) * 0.1) * m
    : (walking || playing ? -armSwing : armSwing * 0.92 - digArm * 0.85 - antsPointPulse * 0.6 - weightLean * 0.08 + fidgetAmt * 0.1) * m;

  const footWave = walking ? Math.sin(elapsed * 8) * 0.11 : playing ? Math.sin(elapsed * 5.4) * 0.06 : idleish ? Math.sin(elapsed * 1.4) * 0.012 : 0;
  const bootBaseY = Q_MINION_LAYOUT.bootY;
  const footLY = bootBaseY + footWave * m + weightLean * 0.02 * m;
  const footRY = bootBaseY + (walking || playing ? -footWave : footWave * 0.35) * m - weightLean * 0.02 * m;
  const footLZ = walking
    ? Math.sin(elapsed * 8) * 0.15 * m
    : playing
      ? Math.sin(elapsed * 5.4 + 0.4) * 0.1 * m
      : idleish
        ? weightLean * 0.04 * m
        : 0;
  const footRZ = walking
    ? Math.sin(elapsed * 8 + Math.PI) * 0.15 * m
    : playing
      ? Math.sin(elapsed * 5.4 + 1.2) * 0.1 * m
      : idleish
        ? -weightLean * 0.04 * m
        : 0;

  // Grounded contact shadow: denser base, widens with squash (scaleX), shrinks on hop.
  const shadowBase = sleeping ? 1.12 : working || dazedAmt ? 1.08 : ants > 0.2 ? 1.06 : 1.03;
  const squashShadow = MathUtils.clamp(scaleX, 0.88, 1.18);
  const stretchShadow = MathUtils.clamp(scaleY, 0.88, 1.2);
  const shadowScale = (
    shadowBase * (0.86 + squashShadow * 0.2)
    + Math.sin(elapsed * 1.6) * pose.shadowPulse
    - hopWave * 0.58
    - idleBounce * 0.04 * (idleish ? 1 : 0.25)
    + pose.slump * 0.09
    + ants * 0.05
    + (1.05 - stretchShadow) * 0.08
  ) * m + shadowBase * (1 - m);
  const shadowOpacity = (
    (sleeping ? 0.95 : 0.93)
    - Math.sin(elapsed * 1.6) * pose.shadowPulse
    - hopWave * 0.45
    - idleBounce * 0.03 * (idleish ? 1 : 0.2)
    + pose.slump * 0.04
    + ants * 0.03
    + (squashShadow - 1) * 0.08
  ) * m + 0.93 * (1 - m);

  const mouthOpen = (
    sleeping
      ? 0.08 + Math.max(0, Math.sin(elapsed * 0.78)) * 0.05
      : yawnAmt * 0.82
        + (playing ? 0.18 + Math.sin(elapsed * 4.2) * 0.08 : 0.025)
        + dazedAmt * 0.14
        + sadAmt * 0.05
        + (idleish ? digAmt * 0.14 : 0)
        + ants * 0.05 * Math.max(0, Math.sin(elapsed * 3.1))
        + hopLiteAmt * 0.04
  ) * m;
  // +1 smile (classic) · -1 frown for sad / connector offline mood.
  const mouthSmile = MathUtils.clamp(
    (playing || emotion === "happy" || emotion === "excited" ? 1 : 0.86)
      - sadAmt * 1.85
      - dazedAmt * 0.7
      - yawnAmt * 0.18
      - (working ? 0.12 : 0)
      + (idleish ? micro.cheekPulse * 0.1 : 0)
      + hopLiteAmt * 0.05,
    -1,
    1,
  ) * m + 0.86 * (1 - m);

  const sweatPulse = working || dazedAmt
    ? pose.sweat * (0.55 + 0.45 * (0.5 + 0.5 * Math.sin(elapsed * 4.2)))
    : pose.sweat * 0.15;
  const sweat = MathUtils.clamp(sweatPulse, 0, 1) * m;

  const sparkle = playing
    ? MathUtils.clamp(0.45 + 0.55 * Math.abs(Math.sin(elapsed * 6.2)), 0, 1) * m
    : waving || hopping
      ? MathUtils.clamp(0.22 + 0.35 * Math.abs(Math.sin(elapsed * 5.4)), 0, 1) * m
      : waveLiteAmt * 0.18 * m;

  const digNose = (idleish || directedDig) ? digAmt * m : 0;
  const countAnts = ants * m;

  // Hair lag target: opposite head pitch + root hop + idle sway (bounce stays pose-only).
  const hairSway = (
    -headPitch * 0.78
    + rootY * 0.58
    + Math.sin(elapsed * 2.5) * 0.09 * (idleish ? 1 : 0.4)
    + bodyTilt * 0.28
    + ants * 0.1
    + digNose * 0.08
    + fidgetAmt * 0.07
    + hopLiteAmt * 0.06
    + hopWave * 0.12
  ) * m;

  // Cheek flush: celebrate, work stress, yawns, dig-nose, and ambient pulse.
  const cheekFlush = MathUtils.clamp(
    (
      micro.cheekPulse * (idleish ? 0.82 : 0.38)
      + (playing ? 0.45 + Math.abs(Math.sin(elapsed * 5.2)) * 0.3 : 0)
      + (working ? 0.24 + sweat * 0.38 : 0)
      + dazedAmt * 0.18
      + digNose * 0.28
      + ants * 0.14
      + fidgetAmt * 0.12
      + waveLiteAmt * 0.18
      + hopLiteAmt * 0.1
      + (emotion === "happy" || emotion === "excited" ? 0.22 : 0)
      - sadAmt * 0.18
    ) * m,
    0,
    1,
  );

  return {
    rootY,
    scaleX,
    scaleY,
    bodyTilt,
    bodyYaw,
    headYaw,
    headPitch,
    eyeScaleY,
    eyeScaleYL,
    eyeScaleYR,
    irisX,
    irisY,
    armL,
    armR,
    footLY,
    footRY,
    footLZ,
    footRZ,
    shadowScale,
    shadowOpacity: MathUtils.clamp(shadowOpacity, 0.38, 0.96),
    mouthOpen: MathUtils.clamp(mouthOpen, 0, 1),
    sweat,
    sparkle,
    dazed: dazedAmt * m,
    digNose,
    countAnts,
    hairSway,
    cheekFlush,
    mouthSmile,
  };
}

export function BuiltinPet3D({ state, emotion, attention, microAct, onFailure, onPerfSummary }: BuiltinPet3DProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const stateRef = useRef({ state, emotion });
  stateRef.current = { state, emotion };
  const attentionRef = useRef<LifeformAttention | undefined>(attention);
  attentionRef.current = attention;
  const microActRef = useRef<LifeformMicroAct | undefined>(microAct);
  microActRef.current = microAct;
  const onPerfSummaryRef = useRef(onPerfSummary);
  onPerfSummaryRef.current = onPerfSummary;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    let renderer: WebGLRenderer;
    try {
      renderer = new WebGLRenderer({
        canvas,
        alpha: true,
        antialias: true,
        premultipliedAlpha: false,
        powerPreference: "high-performance",
      });
    } catch {
      onFailure();
      return;
    }
    renderer.setClearColor(0x000000, 0);
    renderer.setClearAlpha(0);
    renderer.autoClear = true;
    canvas.style.background = "transparent";
    canvas.style.backgroundColor = "transparent";
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));

    const scene = new Scene();
    scene.background = null;
    // Frustum frames chubbier dual-goggle pill + boots + dual contact shadow.
    // Slight bottom bias so feet/shadow stand solid in 260×320 canvas without clipping crown.
    const camera = new OrthographicCamera(-2.42, 2.42, 2.68, -2.58, 0.1, 100);
    camera.position.set(0, -0.06, 8);
    scene.add(new AmbientLight(0xfff6e0, 3.05));
    const keyLight = new DirectionalLight(0xfffaf2, 4.35);
    keyLight.position.set(-2.4, 5.0, 6.4);
    scene.add(keyLight);
    const fillLight = new DirectionalLight(0xeaf4ff, 1.85);
    fillLight.position.set(3.1, 1.8, 2.8);
    scene.add(fillLight);
    const rimLight = new DirectionalLight(0xb8e6ff, 2.2);
    rimLight.position.set(3.4, 2.0, -2.0);
    scene.add(rimLight);
    // Soft front bounce so candy yellow stays soft on dark wallpapers.
    const bounce = new DirectionalLight(0xfff2b0, 1.35);
    bounce.position.set(0.15, -2.2, 4.4);
    scene.add(bounce);

    const root = new Group();
    // Full Q-Bob (crown → boots → dual shadow) slightly low for grounded read.
    root.scale.setScalar(0.98);
    root.position.y = -0.06;
    scene.add(root);

    const layout = Q_MINION_LAYOUT;
    const colors = Q_MINION_COLORS;

    // Dual contact shadow: soft wide pool + denser core glued under boots (no plate).
    const shadowTexture = createContactShadowTexture(192);
    const contactShadow = new Mesh(
      new CircleGeometry(1.42, 64),
      new MeshBasicMaterial({ map: shadowTexture, transparent: true, opacity: 1, depthWrite: false }),
    );
    contactShadow.rotation.x = -Math.PI / 2;
    contactShadow.position.set(0, layout.shadowY, 0.04);
    contactShadow.scale.set(1.08, 1, 0.48);
    root.add(contactShadow);

    const shadowCore = new Mesh(
      new CircleGeometry(0.62, 48),
      new MeshBasicMaterial({ map: shadowTexture, transparent: true, opacity: 0.84, depthWrite: false }),
    );
    shadowCore.rotation.x = -Math.PI / 2;
    shadowCore.position.set(0, layout.shadowY + 0.005, 0.07);
    shadowCore.scale.set(0.88, 1, 0.5);
    root.add(shadowCore);

    const body = new Group();
    root.add(body);
    const head = new Group();
    head.position.set(0, layout.headY, 0.05);
    body.add(head);

    // Soft candy Q-Bob materials: rubbery clearcoat, brushed metal buckles (no chrome plate).
    const yellow = material(colors.yellow, 0.42, 0.72, colors.yellowEmissive, 0.22);
    const yellowLight = material(colors.yellowLight, 0.38, 0.78, colors.yellowEmissive, 0.18);
    const graphite = material(colors.graphite, 0.4, 0.42);
    const goggleBand = material(colors.goggleBand, 0.48, 0.28);
    const metal = metallicMaterial(colors.metal, 0.42, 0.48);
    const glass = material(colors.iris, 0.2, 0.9, colors.irisEmissive, 0.28);
    const white = material(colors.sclera, 0.48, 0.4);
    const coral = material(colors.blush, 0.62, 0.28, "#d83d2c", 0.06);
    const denim = material(colors.denim, 0.8, 0.1);
    const denimSoft = material(colors.denimSoft, 0.76, 0.12);
    const denimDeep = material(colors.denimDeep, 0.82, 0.08);
    const gloveMat = material(colors.glove, 0.56, 0.14);
    const bootMat = material(colors.boot, 0.62, 0.14);
    const bootSoleMat = material(colors.bootSole, 0.74, 0.08);
    const hairMat = material(colors.hair, 0.48, 0.16);

    // Chubby warm yellow capsule (Q-pill silhouette, never a box/plate).
    const torso = makeMesh(
      new CapsuleGeometry(layout.bodyRadius, layout.bodyLength, 20, 48),
      yellow,
      [0, layout.bodyY, 0],
      [1.05, 0.98, 0.96],
    );
    body.add(torso);
    // Soft front belly highlight — rounded lifeform, not a flat plate.
    const belly = makeMesh(new SphereGeometry(0.62, 28, 20), yellowLight, [0, -0.08, 0.66], [1.12, 1.08, 0.42]);
    body.add(belly);
    // Soft crown dome blends into the capsule (keeps Q-round top).
    const crown = makeMesh(new SphereGeometry(0.92, 40, 32), yellowLight, [0, 0.18, 0.02], [1.04, 0.76, 0.92]);
    head.add(crown);
    // Tiny coral antenna: soft secondary spring accent (Q personalization).
    const antennaStem = makeMesh(new CapsuleGeometry(0.022, 0.16, 6, 10), metal, [0.14, 0.86, 0.02], [1, 1, 1]);
    const antennaTip = makeMesh(new SphereGeometry(0.062, 14, 10), coral, [0.14, 0.98, 0.02], [1, 1, 1]);
    head.add(antennaStem, antennaTip);

    // Sparse black hair tufts (spring-damper secondary only).
    const hairGroup = new Group();
    hairGroup.position.set(0, layout.hairY, -0.02);
    head.add(hairGroup);
    const hairTufts: Mesh[] = [];
    const hairSpecs = qMinionHairTuftSpecs().map((spec) => [...spec] as [number, number, number, number]);
    for (const [hx, hy, hz, tilt] of hairSpecs) {
      const tuft = makeMesh(new CapsuleGeometry(0.02, 0.26, 6, 10), hairMat, [hx, hy, hz], [0.82, 1.35, 0.7]);
      tuft.rotation.z = tilt;
      tuft.rotation.x = -0.48;
      hairGroup.add(tuft);
      hairTufts.push(tuft);
    }

    // Black goggle strap wrapping the head (classic band — no square chrome frame).
    const strap = makeMesh(new TorusGeometry(0.94, 0.072, 12, 56), goggleBand, [0, 0.02, 0], [1.05, 0.7, 1.03]);
    strap.rotation.x = Math.PI / 2;
    head.add(strap);
    const strapBridge = makeMesh(new CapsuleGeometry(0.05, 0.2, 8, 16), goggleBand, [0, 0.02, 0.94], [1.08, 0.45, 0.45]);
    strapBridge.rotation.z = Math.PI / 2;
    head.add(strapBridge);
    for (const side of [-1, 1] as const) {
      const sidePad = makeMesh(new CapsuleGeometry(0.068, 0.2, 8, 14), goggleBand, [side * 0.82, 0.02, 0.4], [1, 0.58, 0.48]);
      sidePad.rotation.y = side * 0.55;
      head.add(sidePad);
    }

    // Dual oversized goggles: soft metal rim + white sclera + warm brown iris + dual catchlights.
    const eyeGroups: Group[] = [];
    const irises: Mesh[] = [];
    const pupils: Mesh[] = [];
    const gR = layout.goggleRadius;
    for (const side of [-1, 1] as const) {
      const eyeGroup = new Group();
      const [ex, ey, ez] = qMinionGogglePose(side);
      eyeGroup.position.set(ex, ey, ez);
      // Soft silver rim (round), not a chrome plate.
      const rim = makeMesh(new TorusGeometry(gR, 0.068, 14, 42), metal, [0, 0, 0.02], [1, 1.02, 0.5]);
      const rimInner = makeMesh(new TorusGeometry(gR * 0.9, 0.02, 10, 32), goggleBand, [0, 0, 0.04], [1, 1.02, 0.4]);
      const eye = makeMesh(new SphereGeometry(gR * 0.9, 32, 24), white, [0, 0, 0.05], [1, 1.04, 0.34]);
      const iris = makeMesh(new SphereGeometry(gR * 0.5, 24, 18), glass, [0, -0.006, 0.16], [1, 1, 0.4]);
      const pupil = makeMesh(new SphereGeometry(gR * 0.23, 18, 14), graphite, [0, -0.006, 0.23], [1, 1, 0.4]);
      // Dual catchlights for glossy "alive" eyes.
      const shine = makeMesh(new SphereGeometry(0.055, 12, 10), white, [0.09, 0.08, 0.28], [1, 1, 0.48]);
      const shineSoft = makeMesh(new SphereGeometry(0.028, 10, 8), white, [-0.06, -0.05, 0.26], [1, 1, 0.4]);
      eyeGroup.add(rim, rimInner, eye, iris, pupil, shine, shineSoft);
      head.add(eyeGroup);
      eyeGroups.push(eyeGroup);
      irises.push(iris);
      pupils.push(pupil);
    }

    // Soft smile arc (flips to frown via rotation spring).
    const mouth = makeMesh(new TorusGeometry(0.18, 0.03, 10, 28, Math.PI), graphite, [0, -0.42, 0.9], [1.08, 0.72, 0.46]);
    mouth.rotation.z = Math.PI;
    head.add(mouth);
    const cheekMatL = coral.clone();
    cheekMatL.transparent = true;
    cheekMatL.opacity = 0.48;
    const cheekMatR = coral.clone();
    cheekMatR.transparent = true;
    cheekMatR.opacity = 0.48;
    const cheekLeft = makeMesh(new SphereGeometry(0.09, 16, 12), cheekMatL, [-0.7, -0.34, 0.8], [1.42, 0.56, 0.28]);
    const cheekRight = makeMesh(new SphereGeometry(0.09, 16, 12), cheekMatR, [0.7, -0.34, 0.8], [1.42, 0.56, 0.28]);
    head.add(cheekLeft, cheekRight);

    // Blue denim overalls: pants, bib, springy straps, pocket + metal buckles.
    const overalls = makeMesh(new CapsuleGeometry(0.9, 0.52, 12, 36), denim, [0, layout.overallY, 0.02], [1.06, 0.94, 0.96]);
    const overallBib = makeMesh(new SphereGeometry(0.74, 36, 26), denimDeep, [0, -0.36, 0.46], [0.98, 0.74, 0.56]);
    // Front pocket with flap — classic overalls tell.
    const overallPocket = makeMesh(new CapsuleGeometry(0.15, 0.24, 8, 20), denimSoft, [0, -0.52, 0.96], [1.45, 0.86, 0.2]);
    overallPocket.rotation.z = Math.PI / 2;
    const pocketFlap = makeMesh(new CapsuleGeometry(0.055, 0.34, 8, 14), denim, [0, -0.4, 1.0], [1.22, 0.38, 0.16]);
    pocketFlap.rotation.z = Math.PI / 2;
    const pocketStitch = makeMesh(new CapsuleGeometry(0.018, 0.3, 6, 10), denimDeep, [0, -0.52, 1.02], [1.05, 0.32, 0.12]);
    pocketStitch.rotation.z = Math.PI / 2;
    body.add(overalls, overallBib, overallPocket, pocketFlap, pocketStitch);
    const overallStrapMeshes: Mesh[] = [];
    const strapBaseRotZ: number[] = [];
    for (const side of [-1, 1] as const) {
      const strapPose = qMinionOverallStrapPose(side);
      const overallStrap = makeMesh(
        new CapsuleGeometry(0.07, 0.88, 8, 18),
        denimSoft,
        [strapPose.strap[0], strapPose.strap[1], strapPose.strap[2]],
        [1, 1, 0.36],
      );
      overallStrap.rotation.z = strapPose.rotationZ;
      overallStrapMeshes.push(overallStrap);
      strapBaseRotZ.push(strapPose.rotationZ);
      // Shoulder pad where strap meets yellow body.
      const strapPad = makeMesh(
        new SphereGeometry(0.1, 14, 10),
        denim,
        [side * 0.5, 0.2, 0.66],
        [1.08, 0.52, 0.52],
      );
      // Metal buckle: disc + dark center dimple (readable hardware, not a plate).
      const button = makeMesh(
        new SphereGeometry(0.082, 16, 12),
        metal,
        [strapPose.button[0], strapPose.button[1], strapPose.button[2]],
        [1, 1, 0.28],
      );
      const buttonDimple = makeMesh(
        new SphereGeometry(0.032, 12, 10),
        graphite,
        [strapPose.button[0], strapPose.button[1], strapPose.button[2] + 0.03],
        [1, 1, 0.2],
      );
      body.add(overallStrap, strapPad, button, buttonDimple);
    }

    const arms: Group[] = [];
    const feet: Group[] = [];
    for (const side of [-1, 1] as const) {
      const arm = new Group();
      arm.position.set(side * layout.armX, -0.18, 0.02);
      const upperArm = makeMesh(new CapsuleGeometry(0.14, 0.44, 8, 20), yellow, [side * 0.08, -0.22, 0], [1, 1, 1]);
      upperArm.rotation.z = side * -0.2;
      // Black mitten gloves (classic minion).
      const mitten = makeMesh(new SphereGeometry(0.2, 20, 16), gloveMat, [side * 0.15, -0.54, 0.04], [1.1, 0.88, 0.9]);
      const thumb = makeMesh(new SphereGeometry(0.078, 14, 10), gloveMat, [side * 0.32, -0.48, 0.1], [1, 0.7, 0.76]);
      arm.add(upperArm, mitten, thumb);
      body.add(arm);
      arms.push(arm);

      const foot = new Group();
      foot.position.set(side * 0.4, layout.bootY, 0.12);
      // Brown rounded boots with darker soles, slightly stubby for Q read.
      const boot = makeMesh(new CapsuleGeometry(0.23, 0.26, 8, 18), bootMat, [0, 0.02, 0.04], [1.22, 0.7, 1.26]);
      boot.rotation.x = Math.PI / 2;
      const bootToe = makeMesh(new SphereGeometry(0.17, 16, 12), bootMat, [0, -0.01, 0.26], [1.22, 0.68, 0.96]);
      const bootCuff = makeMesh(new CapsuleGeometry(0.17, 0.07, 6, 12), bootMat, [0, 0.11, 0.02], [1.12, 0.68, 1.02]);
      const sole = makeMesh(new CapsuleGeometry(0.21, 0.08, 6, 14), bootSoleMat, [0, -0.1, 0.05], [1.26, 0.4, 1.36]);
      sole.rotation.x = Math.PI / 2;
      foot.add(boot, bootToe, bootCuff, sole);
      body.add(foot);
      feet.push(foot);
    }

    // Work sweat / glisten dots (procedural particles parented to head).
    const sweatGroup = new Group();
    sweatGroup.position.set(0.72, 0.28, 0.72);
    head.add(sweatGroup);
    const sweatDots: Mesh[] = [];
    const sweatMat = new MeshBasicMaterial({ color: new Color("#9edfff"), transparent: true, opacity: 0, depthWrite: false });
    for (let i = 0; i < 4; i += 1) {
      const drop = new Mesh(new SphereGeometry(0.045, 10, 8), sweatMat.clone());
      drop.position.set((i % 2) * 0.12 - 0.04, -i * 0.08, 0.04 + i * 0.01);
      drop.scale.setScalar(0.001);
      sweatGroup.add(drop);
      sweatDots.push(drop);
    }

    // Celebrate fireworks-lite sparkles around the body.
    const sparkleGroup = new Group();
    body.add(sparkleGroup);
    const sparkles: Mesh[] = [];
    const sparkleColors = ["#ff7d69", "#ffdf4f", "#7bdfff", "#c7a6ff", "#fffdf2"];
    for (let i = 0; i < 8; i += 1) {
      const sparkMat = new MeshBasicMaterial({
        color: new Color(sparkleColors[i % sparkleColors.length]!),
        transparent: true,
        opacity: 0,
        depthWrite: false,
      });
      const spark = new Mesh(new SphereGeometry(0.06, 10, 8), sparkMat);
      spark.scale.setScalar(0.001);
      sparkleGroup.add(spark);
      sparkles.push(spark);
    }

    // Soft daze stars when wounded / crash.
    const dazeGroup = new Group();
    dazeGroup.position.set(0, 0.95, 0.2);
    head.add(dazeGroup);
    const dazeStars: Mesh[] = [];
    const dazeMat = new MeshBasicMaterial({ color: new Color("#ffd86b"), transparent: true, opacity: 0, depthWrite: false });
    for (let i = 0; i < 3; i += 1) {
      const star = new Mesh(new SphereGeometry(0.07, 10, 8), dazeMat.clone());
      star.scale.setScalar(0.001);
      dazeGroup.add(star);
      dazeStars.push(star);
    }

    let gazeX = 0;
    let gazeY = 0;
    // Pointer activity clock: after this goes stale, ambient attention drives gaze.
    let lastPointerMoveMs = -Infinity;
    // Soft gaze lag so eyes/head follow the pointer with life, not stiff snaps.
    const springGazeX = createSpringState(0);
    const springGazeY = createSpringState(0);
    const trackPointer = (event: PointerEvent) => {
      // Prefer canvas-local look (pet feels aware when parked in a corner).
      const rect = canvas.getBoundingClientRect();
      const cx = rect.left + rect.width * 0.5;
      const cy = rect.top + rect.height * 0.4;
      const spanX = Math.max(rect.width * 0.55, window.innerWidth * 0.18, 1);
      const spanY = Math.max(rect.height * 0.55, window.innerHeight * 0.18, 1);
      gazeX = clampGaze((event.clientX - cx) / spanX);
      gazeY = clampGaze((event.clientY - cy) / spanY);
      lastPointerMoveMs = performance.now();
    };
    window.addEventListener("pointermove", trackPointer, { passive: true });

    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
    const timer = new Timer();
    const lifeformPerf = createLifeformPerfTracker();
    const perfEmitGate = createPerfEmitGate();
    let frame = 0;
    let disposed = false;
    // Spring-damper body + secondary (no linear body lerp). High omega tracks hop liveliness.
    const springRootY = createSpringState(-0.01);
    const springScaleX = createSpringState(1);
    const springScaleY = createSpringState(1);
    const springBodyTilt = createSpringState(0);
    const springBodyYaw = createSpringState(0);
    const springHeadYaw = createSpringState(0);
    const springHeadPitch = createSpringState(0);
    const springArmL = createSpringState(0);
    const springArmR = createSpringState(0);
    const springEyeL = createSpringState(1);
    const springEyeR = createSpringState(1);
    const springIrisX = createSpringState(0);
    const springIrisY = createSpringState(0);
    const springSweat = createSpringState(0);
    const springMouthSmile = createSpringState(0.86);
    const springHair = createSpringState(0);
    const springAntenna = createSpringState(0);
    const springStrapL = createSpringState(0);
    const springStrapR = createSpringState(0);

    const render = () => {
      if (disposed) return;
      frame = window.requestAnimationFrame(render);
      timer.update();
      const dtMs = timer.getDelta() * 1000;
      const dt = MathUtils.clamp(dtMs / 1000, 0, 1 / 20);
      const nowMs = performance.now();
      recordFrame(lifeformPerf, nowMs, dtMs);
      if (perfEmitGate.shouldEmit(nowMs)) {
        const summary = summarize(lifeformPerf);
        if (summary.sampleCount > 0) {
          onPerfSummaryRef.current?.(summary);
          try {
            window.dispatchEvent(new CustomEvent(LIFEFORM_PERF_EVENT, { detail: summary }));
          } catch {
            // CustomEvent may be unavailable in exotic hosts; callback path still works.
          }
        }
      }
      const elapsed = timer.getElapsed();
      const current = stateRef.current;
      const motion = reducedMotion.matches ? 0 : 1;
      // Pointer stale → drift gaze toward the living-presence attention target so
      // the pet looks at meaningful desktop things (tray, dock, other display)
      // instead of freezing on the last cursor spot.
      const pointerStale = nowMs - lastPointerMoveMs > POINTER_ATTENTION_STALE_MS;
      let targetGazeX = gazeX;
      let targetGazeY = gazeY;
      if (pointerStale) {
        const bias = attentionGazeBias(attentionRef.current);
        // Gentle wander around the biased target so glances read as alive.
        const wanderX = Math.sin(elapsed * 0.31) * 0.12 + Math.sin(elapsed * 0.13) * 0.06;
        const wanderY = Math.sin(elapsed * 0.24 + 1.1) * 0.08;
        targetGazeX = clampGaze(bias.x + wanderX);
        targetGazeY = clampGaze(bias.y + wanderY);
      }
      // VISUAL: snappier eye/head IK lag (secondary spring only — no body travel lerp).
      // Ambient gaze eases slower than pointer tracking so it feels like wandering, not snapping.
      const gazeOmega = pointerStale ? 6.5 : 15.5;
      const softGazeX = springToward(springGazeX, targetGazeX, dt, gazeOmega, 0.85) * motion;
      const softGazeY = springToward(springGazeY, targetGazeY, dt, gazeOmega, 0.85) * motion;
      const sample = sampleBuiltinPetMotion(current.state, current.emotion, elapsed, softGazeX, softGazeY, motion, microActRef.current);
      const playing = isPlaying(current.state);

      // Body channels: spring-damper only (never linear lerp). Play spin stays snappy via higher omega.
      const bodyOmega = playing ? 24 : 17.5;
      const softRootY = springToward(springRootY, sample.rootY, dt, bodyOmega, 0.86);
      const softScaleX = springToward(springScaleX, sample.scaleX, dt, bodyOmega, 0.84);
      const softScaleY = springToward(springScaleY, sample.scaleY, dt, bodyOmega, 0.84);
      const softBodyTilt = springToward(springBodyTilt, sample.bodyTilt, dt, 15.5, 0.86);
      const softBodyYaw = springToward(springBodyYaw, sample.bodyYaw, dt, playing ? 20 : 13.5, 0.88);
      const softHeadYaw = springToward(springHeadYaw, sample.headYaw, dt, 14.5, 0.86);
      const softHeadPitch = springToward(springHeadPitch, sample.headPitch, dt, 14.5, 0.86);
      const softArmL = springToward(springArmL, sample.armL, dt, 13.5, 0.88);
      const softArmR = springToward(springArmR, sample.armR, dt, 13.5, 0.88);
      const softEyeL = springToward(springEyeL, sample.eyeScaleYL, dt, 22, 0.78);
      const softEyeR = springToward(springEyeR, sample.eyeScaleYR, dt, 22, 0.78);
      const softSweat = springToward(springSweat, sample.sweat, dt, 9.5, 0.9);
      const softMouthSmile = springToward(springMouthSmile, sample.mouthSmile, dt, 11, 0.88);

      // Keep grounded framing offset; softRootY only adds motion (never linear travel).
      root.position.y = -0.06 + softRootY;
      body.scale.set(softScaleX, softScaleY, softScaleX * 0.98 + 0.02);
      body.rotation.z = softBodyTilt;
      body.rotation.y = softBodyYaw;

      head.rotation.y = softHeadYaw;
      head.rotation.x = softHeadPitch;
      // Hair lag: lower-omega spring secondary (Verlet-like delay on crown tufts).
      const hairLean = springToward(springHair, sample.hairSway, dt, 6.8, 0.86);
      hairGroup.rotation.x = hairLean;
      hairGroup.rotation.z = Math.sin(elapsed * 1.85) * 0.07 * motion + sample.bodyTilt * 0.16;
      for (let i = 0; i < hairTufts.length; i += 1) {
        hairTufts[i]!.rotation.z = hairSpecs[i]![3]
          + Math.sin(elapsed * 2.6 + i) * 0.12 * motion
          + hairLean * 0.22
          + sample.bodyTilt * 0.08;
        hairTufts[i]!.rotation.x = -0.48
          + hairLean * 0.42
          + Math.sin(elapsed * 2.1 + i) * 0.06 * motion
          + sample.rootY * 0.15;
      }
      // Overall straps lag body tilt / hop (secondary spring, not body travel).
      const strapTarget = sample.bodyTilt * 0.55 + sample.rootY * 0.35 + sample.hairSway * 0.12;
      const strapLeanL = springToward(springStrapL, strapTarget + sample.armL * 0.04, dt, 7.5, 0.9);
      const strapLeanR = springToward(springStrapR, -strapTarget + sample.armR * 0.04, dt, 7.5, 0.9);
      if (overallStrapMeshes[0]) {
        overallStrapMeshes[0].rotation.z = strapBaseRotZ[0]! + strapLeanL * 0.12;
        overallStrapMeshes[0].rotation.x = strapLeanL * 0.08 + Math.sin(elapsed * 1.6) * 0.02 * motion;
      }
      if (overallStrapMeshes[1]) {
        overallStrapMeshes[1].rotation.z = strapBaseRotZ[1]! + strapLeanR * 0.12;
        overallStrapMeshes[1].rotation.x = strapLeanR * 0.08 + Math.sin(elapsed * 1.6 + 0.4) * 0.02 * motion;
      }
      // Antenna soft lag secondary (follows head pitch with spring delay — never lerp body hop).
      const antennaLean = springToward(
        springAntenna,
        -sample.headPitch * 0.58 + sample.hairSway * 0.36 + sample.rootY * 0.22,
        dt,
        9.2,
        0.86,
      );
      antennaStem.rotation.x = antennaLean * 0.7;
      antennaStem.rotation.z = Math.sin(elapsed * 2.2) * 0.05 * motion;
      antennaTip.position.y = 0.98 + antennaLean * 0.09;
      antennaTip.position.x = 0.14 + Math.sin(elapsed * 2.9) * 0.016 * motion + antennaLean * 0.025;
      antennaTip.scale.setScalar(1 + Math.sin(elapsed * 3.5) * 0.07 * motion);

      // Dual goggles: livelier asymmetric blink + spring-smoothed iris IK.
      eyeGroups[0]!.scale.y = softEyeL;
      eyeGroups[1]!.scale.y = softEyeR;
      const softIrisX = springToward(springIrisX, sample.irisX, dt, 18, 0.78);
      const softIrisY = springToward(springIrisY, sample.irisY, dt, 18, 0.78);
      for (let i = 0; i < eyeGroups.length; i += 1) {
        const sideSign = i === 0 ? -1 : 1;
        const eyeIrisX = softIrisX + sideSign * 0.005 * motion;
        irises[i]!.position.x = eyeIrisX;
        irises[i]!.position.y = -0.006 + softIrisY;
        pupils[i]!.position.x = eyeIrisX * 1.12;
        pupils[i]!.position.y = -0.006 + softIrisY * 1.12;
      }

      // Smile (π) ↔ frown (0) spring; open for yawn / play.
      const smileT = MathUtils.clamp((softMouthSmile + 1) * 0.5, 0, 1);
      mouth.rotation.z = Math.PI * smileT;
      mouth.scale.y = 0.72 + sample.mouthOpen * 1.05;
      mouth.position.y = -0.42 - sample.mouthOpen * 0.05 + (1 - smileT) * 0.035;
      mouth.scale.x = 1.08 + sample.digNose * 0.1 + sample.countAnts * 0.06 + (1 - smileT) * 0.08;

      arms[0]!.rotation.z = softArmL;
      arms[1]!.rotation.z = softArmR;
      const armXIdleL = sample.digNose * 0.35 + sample.countAnts * 0.28;
      const armXIdleR = sample.digNose * -0.2 + sample.countAnts * 0.72;
      arms[0]!.rotation.x = playing ? -0.55 : armXIdleL;
      arms[1]!.rotation.x = playing ? -0.55 : armXIdleR;

      feet[0]!.position.y = sample.footLY;
      feet[1]!.position.y = sample.footRY;
      feet[0]!.rotation.z = sample.footLZ;
      feet[1]!.rotation.z = sample.footRZ;

      // Dual contact shadow: denser when grounded, shrinks on hop — glued under boots.
      contactShadow.scale.x = sample.shadowScale * 1.02;
      contactShadow.scale.z = 0.48 * sample.shadowScale;
      (contactShadow.material as MeshBasicMaterial).opacity = sample.shadowOpacity;
      shadowCore.scale.x = sample.shadowScale * 0.86;
      shadowCore.scale.z = 0.48 * sample.shadowScale;
      (shadowCore.material as MeshBasicMaterial).opacity = MathUtils.clamp(sample.shadowOpacity * 0.9, 0.34, 0.92);

      const lagSweat = softSweat;
      const lagSparkle = sample.sparkle;
      const lagDazed = sample.dazed;

      for (let i = 0; i < sweatDots.length; i += 1) {
        const drop = sweatDots[i]!;
        const phase = (elapsed * 1.85 + i * 0.48) % 1;
        const active = lagSweat > 0.035;
        const size = active ? (0.62 + lagSweat * 1.05) * (1 - phase * 0.4) : 0.001;
        drop.scale.setScalar(Math.max(size, 0.001));
        drop.position.y = -i * 0.05 - phase * 0.34 * lagSweat;
        drop.position.x = ((i % 2) * 0.15 - 0.05) + Math.sin(elapsed * 2.2 + i) * 0.025 * lagSweat;
        (drop.material as MeshBasicMaterial).opacity = active ? lagSweat * (0.82 - phase * 0.55) : 0;
      }

      for (let i = 0; i < sparkles.length; i += 1) {
        const spark = sparkles[i]!;
        const angle = (i / sparkles.length) * Math.PI * 2 + elapsed * 2.4;
        const radius = 1.05 + Math.sin(elapsed * 5 + i) * 0.18 + lagSparkle * 0.35;
        const rise = Math.sin(elapsed * 4.2 + i * 0.9) * 0.35 + 0.35;
        spark.position.set(Math.cos(angle) * radius * 0.85, rise + (i % 3) * 0.12, Math.sin(angle) * radius * 0.35);
        const pulse = lagSparkle * (0.4 + 0.6 * Math.abs(Math.sin(elapsed * 7 + i)));
        spark.scale.setScalar(Math.max(0.001, pulse * 0.85));
        (spark.material as MeshBasicMaterial).opacity = pulse * 0.9;
      }

      for (let i = 0; i < dazeStars.length; i += 1) {
        const star = dazeStars[i]!;
        const angle = elapsed * 1.8 + (i / dazeStars.length) * Math.PI * 2;
        star.position.set(Math.cos(angle) * 0.42, 0.12 + Math.sin(elapsed * 3 + i) * 0.06, Math.sin(angle) * 0.2);
        const size = lagDazed * (0.55 + 0.45 * Math.abs(Math.sin(elapsed * 5 + i)));
        star.scale.setScalar(Math.max(0.001, size));
        (star.material as MeshBasicMaterial).opacity = lagDazed * 0.85;
      }

      // Cheek flush follows sample (celebrate / work stress / idle micro-acts).
      const cheekY = 0.48 + sample.cheekFlush * 0.72 + lagSparkle * 0.18 + lagSweat * 0.12;
      const cheekOp = MathUtils.clamp(0.28 + sample.cheekFlush * 0.62 + lagSparkle * 0.15, 0.2, 0.95);
      cheekLeft.scale.y = cheekY;
      cheekRight.scale.y = cheekY;
      cheekLeft.scale.x = 1.38 + sample.cheekFlush * 0.22;
      cheekRight.scale.x = 1.38 + sample.cheekFlush * 0.22;
      (cheekLeft.material as MeshPhysicalMaterial).opacity = cheekOp;
      (cheekRight.material as MeshPhysicalMaterial).opacity = cheekOp;
      (cheekLeft.material as MeshPhysicalMaterial).emissiveIntensity = 0.05 + sample.cheekFlush * 0.18;
      (cheekRight.material as MeshPhysicalMaterial).emissiveIntensity = 0.05 + sample.cheekFlush * 0.18;

      renderer.render(scene, camera);
    };

    const resize = () => {
      const parent = canvas.parentElement;
      const width = Math.max(
        canvas.clientWidth,
        parent?.clientWidth ?? 0,
        260,
      );
      const height = Math.max(
        canvas.clientHeight,
        parent?.clientHeight ?? 0,
        320,
      );
      renderer.setSize(width, height, false);
      canvas.style.width = "100%";
      canvas.style.height = "100%";
    };
    const observer = new ResizeObserver(resize);
    observer.observe(canvas);
    resize();
    frame = window.requestAnimationFrame(render);
    return () => {
      disposed = true;
      window.cancelAnimationFrame(frame);
      window.removeEventListener("pointermove", trackPointer);
      observer.disconnect();
      scene.traverse((object) => {
        if (!(object instanceof Mesh)) return;
        object.geometry.dispose();
        const materials = Array.isArray(object.material) ? object.material : [object.material];
        for (const value of materials) value.dispose();
      });
      shadowTexture.dispose();
      renderer.dispose();
    };
  }, [onFailure]);

  return <canvas ref={canvasRef} className="builtin-pet-3d" aria-label="灵灵 · Nimora Q版小黄人伙伴" />;
}

function material(color: string, roughness: number, clearcoat: number, emissive = "#000000", emissiveIntensity = 0): MeshPhysicalMaterial {
  return new MeshPhysicalMaterial({ color: new Color(color), roughness, clearcoat, clearcoatRoughness: 0.42, emissive: new Color(emissive), emissiveIntensity });
}

function metallicMaterial(color: string, roughness: number, clearcoat: number): MeshPhysicalMaterial {
  return new MeshPhysicalMaterial({ color: new Color(color), roughness, metalness: 0.62, clearcoat, clearcoatRoughness: 0.28 });
}

function makeMesh(
  geometry: SphereGeometry | CapsuleGeometry | TorusGeometry,
  meshMaterial: MeshPhysicalMaterial,
  position: [number, number, number],
  scale: [number, number, number],
): Mesh {
  const value = new Mesh(geometry, meshMaterial);
  value.position.set(...position);
  value.scale.set(...scale);
  return value;
}
