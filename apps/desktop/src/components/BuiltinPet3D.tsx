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

export function clampGaze(value: number): number {
  return MathUtils.clamp(value, -1, 1);
}

/** Warm classic Q-minion palette (no chrome plate / no square frame). */
export const Q_MINION_COLORS = {
  /** Classic film minion yellow — saturated, warm, candy-like. */
  yellow: "#F7D117",
  yellowLight: "#FFE94A",
  yellowEmissive: "#F0C010",
  denim: "#2E7DB5",
  denimSoft: "#4C97C8",
  denimDeep: "#246594",
  goggleBand: "#101218",
  metal: "#E8ECF1",
  sclera: "#FFFEF8",
  /** Warm cocoa iris — soft Q-minion, not tech cyan. */
  iris: "#6B3F22",
  irisEmissive: "#3D2412",
  glove: "#15161A",
  boot: "#6A4326",
  bootSole: "#3A2514",
  hair: "#14151A",
  blush: "#FF8E7A",
  graphite: "#171A20",
} as const;

/** Tall warm pill + dual goggle layout (classic Q-minion proportions). */
export const Q_MINION_LAYOUT = {
  /** Cylinder radius of the yellow capsule. */
  bodyRadius: 0.9,
  /** Cylinder length between hemispheres (total height ≈ length + 2r). */
  bodyLength: 1.08,
  bodyY: -0.32,
  headY: 0.58,
  /** Larger dual goggles = cuter Q face. */
  goggleRadius: 0.48,
  goggleSpacing: 0.45,
  goggleZ: 0.98,
  overallY: -0.82,
  bootY: -1.34,
  armX: 0.98,
  hairY: 0.98,
  shadowY: -1.48,
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
    [-0.12, 0.03, 0.02, -0.3],
    [0.02, 0.1, -0.03, 0.08],
    [0.14, 0.02, 0.04, 0.36],
    [-0.04, 0.06, -0.07, -0.1],
    [0.08, 0.01, 0.07, 0.2],
  ] as const;
}

/** Dual goggle group position for side (-1 left, +1 right). */
export function qMinionGogglePose(side: -1 | 1): QMinionVec3 {
  const { goggleSpacing, goggleZ } = Q_MINION_LAYOUT;
  return [side * goggleSpacing, 0.05, goggleZ] as const;
}

/** Denim strap + metal button placement for overall suspenders. */
export function qMinionOverallStrapPose(side: -1 | 1): {
  strap: QMinionVec3;
  rotationZ: number;
  button: QMinionVec3;
} {
  return {
    strap: [side * 0.4, -0.12, 0.74] as const,
    rotationZ: side * -0.4,
    button: [side * 0.3, -0.4, 0.98] as const,
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
      bounce: 0.012,
      bodyTilt: 0.16,
      eyeScale: 0.58,
      tailSpeed: 0.28,
      breath: 0.016,
      armRest: 0.22,
      hop: 0,
      squash: 1.08,
      stretch: 0.9,
      lookAround: 0.12,
      shadowPulse: 0.03,
      slump: 0.18,
      sweat: 0.48,
    };
  }
  if (isSadEmotion(emotion) && !isWorking(state) && !isPlaying(state)) {
    return {
      bounce: 0.028,
      bodyTilt: 0.07,
      eyeScale: 0.84,
      tailSpeed: 0.55,
      breath: 0.022,
      armRest: 0.28,
      hop: 0,
      squash: 1.04,
      stretch: 0.96,
      lookAround: 0.22,
      shadowPulse: 0.03,
      slump: 0.14,
      sweat: 0.06,
    };
  }
  if (isSleeping(state, emotion)) {
    return {
      bounce: 0.01,
      bodyTilt: 0.06,
      eyeScale: 0.05,
      tailSpeed: 0.18,
      breath: 0.034,
      armRest: 0.24,
      hop: 0,
      squash: 1.08,
      stretch: 0.9,
      lookAround: 0.02,
      shadowPulse: 0.02,
      slump: 0.12,
      sweat: 0,
    };
  }
  if (isWorking(state)) {
    return {
      bounce: 0.014,
      bodyTilt: 0.05,
      eyeScale: 0.9,
      tailSpeed: 0.55,
      breath: 0.012,
      armRest: 0.52,
      hop: 0,
      squash: 1.05,
      stretch: 0.95,
      lookAround: 0.08,
      shadowPulse: 0.024,
      slump: 0.16,
      sweat: 0.96,
    };
  }
  if (isWalking(state)) {
    return {
      bounce: 0.09,
      bodyTilt: 0.05,
      eyeScale: 1,
      tailSpeed: 2.1,
      breath: 0.014,
      armRest: 0.08,
      hop: 0,
      squash: 1,
      stretch: 1,
      lookAround: 0.18,
      shadowPulse: 0.05,
      slump: 0,
      sweat: 0.1,
    };
  }
  if (isPlaying(state) || emotion === "happy") {
    return {
      bounce: 0.2,
      bodyTilt: 0.1,
      eyeScale: 1.1,
      tailSpeed: 3.1,
      breath: 0.024,
      armRest: 0.88,
      hop: 0.24,
      squash: 0.88,
      stretch: 1.18,
      lookAround: 0.4,
      shadowPulse: 0.12,
      slump: 0,
      sweat: 0,
    };
  }
  if (isObserving(state, emotion)) {
    return {
      bounce: 0.018,
      bodyTilt: 0.03,
      eyeScale: 1.22,
      tailSpeed: 1.35,
      breath: 0.012,
      armRest: 0.1,
      hop: 0,
      squash: 0.99,
      stretch: 1.03,
      lookAround: 0.72,
      shadowPulse: 0.03,
      slump: -0.02,
      sweat: 0,
    };
  }
  const motion = normalizeMotionState(state);
  if (motion === "yawn") {
    return {
      bounce: 0.02,
      bodyTilt: 0.055,
      eyeScale: 0.52,
      tailSpeed: 0.42,
      breath: 0.04,
      armRest: 0.48,
      hop: 0,
      squash: 1.07,
      stretch: 0.93,
      lookAround: 0.08,
      shadowPulse: 0.028,
      slump: 0.07,
      sweat: 0,
    };
  }
  if (motion === "dig_nose") {
    return {
      bounce: 0.028,
      bodyTilt: 0.06,
      eyeScale: 0.92,
      tailSpeed: 0.85,
      breath: 0.018,
      armRest: 0.58,
      hop: 0,
      squash: 1.03,
      stretch: 0.98,
      lookAround: 0.18,
      shadowPulse: 0.03,
      slump: 0.05,
      sweat: 0,
    };
  }
  if (motion === "count_ants") {
    return {
      bounce: 0.02,
      bodyTilt: 0.09,
      eyeScale: 1.18,
      tailSpeed: 0.95,
      breath: 0.014,
      armRest: 0.38,
      hop: 0,
      squash: 1.05,
      stretch: 0.96,
      lookAround: 0.32,
      shadowPulse: 0.03,
      slump: 0.12,
      sweat: 0,
    };
  }
  if (motion === "wave") {
    return {
      bounce: 0.08,
      bodyTilt: 0.05,
      eyeScale: 1.08,
      tailSpeed: 1.7,
      breath: 0.02,
      armRest: 0.92,
      hop: 0.04,
      squash: 0.96,
      stretch: 1.06,
      lookAround: 0.34,
      shadowPulse: 0.06,
      slump: 0,
      sweat: 0,
    };
  }
  if (motion === "look_around") {
    return {
      bounce: 0.018,
      bodyTilt: 0.03,
      eyeScale: 1.22,
      tailSpeed: 1.35,
      breath: 0.012,
      armRest: 0.1,
      hop: 0,
      squash: 0.99,
      stretch: 1.03,
      lookAround: 0.88,
      shadowPulse: 0.03,
      slump: -0.02,
      sweat: 0,
    };
  }
  if (motion === "hop") {
    return {
      bounce: 0.14,
      bodyTilt: 0.08,
      eyeScale: 1.06,
      tailSpeed: 2.35,
      breath: 0.022,
      armRest: 0.55,
      hop: 0.22,
      squash: 0.9,
      stretch: 1.14,
      lookAround: 0.28,
      shadowPulse: 0.1,
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
    bounce: 0.082,
    bodyTilt: 0.052,
    eyeScale: 1.02,
    tailSpeed: 1.28,
    breath: 0.03,
    armRest: 0.18,
    hop: 0,
    squash: 1,
    stretch: 1.01,
    lookAround: 0.68,
    shadowPulse: 0.058,
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
  // Yawn ~every 7.4s with a readable open window.
  const yawn = cycleEnvelope(elapsed, 7.4, 5.85, 7.05);
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
    Math.sin(elapsed * 0.46) * 0.82
      + Math.sin(elapsed * 1.18) * 0.36
      + Math.sin(elapsed * 0.19) * 0.2
      + glance * 0.95
      + lookAroundPeak * 0.72,
    -1,
    1,
  );
  // Soft continuous bounce + settle hop (~every 2.4s) so idle never freezes.
  const settleHop = cycleEnvelope(elapsed, 2.4, 1.98, 2.34);
  const microBounce = MathUtils.clamp(
    0.55
      + 0.44 * Math.sin(elapsed * 2.55)
      + settleHop * 0.95
      + Math.sin(elapsed * 0.9) * 0.16
      + Math.sin(elapsed * 5.1) * 0.07,
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
  // Dig-nose pulse every ~9.2s.
  const digNose = cycleEnvelope(elapsed, 9.2, 7.7, 8.7);
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
  const m = MathUtils.clamp(motion, 0, 1);
  // Directed micro-performances stay local (no linear body travel) and do not
  // steal observe/play locomotion channels unless they map there themselves.
  const idleish = (
    !walking && !playing && !working && !sleeping && !observing
    && !waving && !hopping && dazedAmt < 0.5
  ) || directedYawn || directedDig || directedAnts;

  const yawnAmt = MathUtils.clamp(
    (idleish && !directedYawn && !directedDig && !directedAnts ? micro.yawn : 0)
      + (directedYawn ? 0.95 : 0),
    0,
    1,
  );
  const digAmt = MathUtils.clamp(
    (idleish && !directedYawn && !directedAnts ? micro.digNose : 0)
      + (directedDig ? 0.95 : 0),
    0,
    1,
  );
  const ants = MathUtils.clamp(
    (idleish && !directedYawn && !directedDig ? micro.countAnts : 0)
      + (directedAnts ? 0.92 : 0),
    0,
    1,
  );

  const pureIdle = idleish && !directedYawn && !directedDig && !directedAnts;
  const idleBounce = pureIdle ? micro.microBounce : micro.microBounce * (idleish ? 0.45 : 0.2);
  const settleBoost = pureIdle ? micro.settleHop : 0;
  // Hop channel is spring-friendly vertical squash only — never linear body travel.
  const hopLiteAmt = pureIdle ? micro.hopLite : 0;
  const waveLiteAmt = pureIdle ? micro.waveLite : 0;
  const fidgetAmt = pureIdle ? micro.fidget : micro.fidget * 0.2;
  const hopWave = playing
    ? Math.abs(Math.sin(elapsed * 5.4)) * pose.hop
    : hopping
      ? Math.abs(Math.sin(elapsed * 5.0)) * Math.max(pose.hop, 0.2)
      : walking
        ? Math.abs(Math.sin(elapsed * 5.2)) * pose.bounce
        : Math.abs(Math.sin(elapsed * (pose.tailSpeed + 0.7))) * pose.bounce
          + idleBounce * pose.bounce * 0.88
          + hopLiteAmt * 0.14
          + settleBoost * 0.05
          + yawnAmt * 0.014;

  const breathRate = sleeping ? 0.85 : working ? 2.4 : 1.7;
  const breath = Math.sin(elapsed * breathRate) * pose.breath;
  const slumpDrop = pose.slump * 0.22 + dazedAmt * 0.04 + sadAmt * 0.03 + ants * 0.035;
  const rootY = (
    -0.02
    + hopWave
    + breath * (sleeping ? 0.55 : 0.35)
    + idleBounce * 0.022
    + settleBoost * 0.035
    - slumpDrop
    + (sleeping ? -0.04 : 0)
  ) * m + (-0.02) * (1 - m);

  // Settle hop lands with a soft squash beat for rubbery Q-body feel.
  const settleLand = idleish
    ? Math.max(0, Math.sin(elapsed * 2.15) * 0.01) + (micro.microBounce > 0.82 ? (micro.microBounce - 0.82) * 0.12 : 0)
    : 0;
  const squashPulse = playing
    ? 1 + Math.sin(elapsed * 5.4) * 0.11
    : walking
      ? 1 + Math.sin(elapsed * 8) * 0.036
      : 1
        + Math.sin(elapsed * breathRate) * 0.018
        + yawnAmt * 0.045
        + idleBounce * 0.042
        + dazedAmt * 0.028
        + ants * 0.022
        + settleLand;
  const scaleX = (pose.squash * squashPulse + (1 - pose.squash)) * m + (1 - m);
  const scaleY = (pose.stretch / Math.max(squashPulse, 0.001) + (1 - pose.stretch)) * m + (1 - m);

  const bodyTilt = (
    Math.sin(elapsed * (walking ? 4.4 : playing ? 3.2 : dazedAmt ? 1.8 : 1.42)) * pose.bodyTilt
    + (micro.weightShift - 0.5) * 0.1 * (walking || playing ? 0.2 : 1)
    + yawnAmt * 0.055
    + pose.slump * 0.16
    + dazedAmt * Math.sin(elapsed * 2.1) * 0.07
    + digAmt * 0.05 * (idleish ? 1 : 0.15)
    + ants * 0.09
    + idleBounce * 0.024 * (idleish ? 1 : 0.22)
    + fidgetAmt * 0.045
    + waveLiteAmt * 0.03
    + gazeX * 0.035 * (idleish || observing ? 1 : 0.35)
  ) * m;

  const bodyYaw = builtinPetBodyYaw(state, elapsed, gazeX) * m;
  const look = pose.lookAround * (
    observing
      ? micro.lookBurst * 1.4 + Math.sin(elapsed * 0.55) * 0.58 + micro.lookSweep * 0.32
      : micro.lookBurst * 0.95 + micro.lookSweep * (idleish ? 0.88 : 0.32)
  );
  const headYaw = (
    gazeX * (observing ? 0.58 : working ? 0.36 : idleish ? 0.52 : 0.42)
    + look * (observing ? 0.68 : idleish ? 0.58 : 0.5)
    - bodyYaw * 0.2
    + digAmt * 0.14 * (idleish ? 1 : 0)
    + ants * Math.sin(elapsed * 1.1) * 0.06
    + dazedAmt * Math.sin(elapsed * 1.7) * 0.22
    + yawnAmt * Math.sin(elapsed * 2.2) * 0.04
    + fidgetAmt * Math.sin(elapsed * 3.1) * 0.08
    + waveLiteAmt * 0.06
  ) * m;
  const headPitch = (
    gazeY * (observing ? 0.3 : idleish ? 0.22 : 0.16)
    + (sleeping ? 0.2 : 0)
    + pose.slump * 0.62
    + yawnAmt * -0.14
    + Math.sin(elapsed * 0.95) * 0.04 * pose.lookAround
    + digAmt * 0.16 * (idleish ? 1 : 0)
    + ants * 0.26
    + dazedAmt * 0.14
    + sadAmt * 0.1
    + idleBounce * -0.022 * (idleish ? 1 : 0)
    + fidgetAmt * 0.04
    + (working ? 0.06 : 0)
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

  const irisX = (
    gazeX * (observing ? 0.1 : idleish ? 0.068 : 0.055)
    + look * (observing ? 0.065 : idleish ? 0.042 : 0.038)
    + micro.lookSweep * (observing ? 0.02 : idleish ? 0.022 : 0.012) * pose.lookAround
    + ants * Math.sin(elapsed * 1.35) * 0.012
    + dazedAmt * Math.sin(elapsed * 3.1) * 0.03
    + fidgetAmt * Math.sin(elapsed * 2.6) * 0.008
  ) * m;
  const irisY = (
    gazeY * (observing ? 0.06 : idleish ? 0.038 : 0.032)
    + (sleeping ? 0.025 : 0)
    + ants * 0.045
    + idleBounce * -0.01 * (idleish ? 1 : 0)
    + dazedAmt * Math.cos(elapsed * 2.6) * 0.02
    + fidgetAmt * 0.006
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
      ? 0.06 + Math.max(0, Math.sin(elapsed * 0.85)) * 0.04
      : yawnAmt * 0.62
        + (playing ? 0.14 + Math.sin(elapsed * 4) * 0.06 : 0.02)
        + dazedAmt * 0.1
        + sadAmt * 0.04
        + (idleish ? digAmt * 0.12 : 0)
        + ants * 0.04 * Math.max(0, Math.sin(elapsed * 3.1))
  ) * m;
  // +1 smile (classic) · -1 frown for sad / connector offline mood.
  const mouthSmile = MathUtils.clamp(
    (playing || emotion === "happy" || emotion === "excited" ? 1 : 0.82)
      - sadAmt * 1.7
      - dazedAmt * 0.55
      - yawnAmt * 0.12
      + (idleish ? micro.cheekPulse * 0.08 : 0),
    -1,
    1,
  ) * m + 0.82 * (1 - m);

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
    -headPitch * 0.68
    + rootY * 0.48
    + Math.sin(elapsed * 2.35) * 0.065 * (idleish ? 1 : 0.35)
    + bodyTilt * 0.22
    + ants * 0.08
    + digNose * 0.06
    + fidgetAmt * 0.05
    + hopLiteAmt * 0.04
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

export function BuiltinPet3D({ state, emotion, onFailure, onPerfSummary }: BuiltinPet3DProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const stateRef = useRef({ state, emotion });
  stateRef.current = { state, emotion };
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
    // Frustum frames tall dual-goggle pill + boots (transparent, no plate).
    // Extra vertical room so crown tufts + boots + dual contact shadow never clip.
    const camera = new OrthographicCamera(-2.65, 2.65, 3.2, -2.7, 0.1, 100);
    camera.position.set(0, -0.08, 8);
    scene.add(new AmbientLight(0xfff6e0, 2.45));
    const keyLight = new DirectionalLight(0xfff8ee, 4.0);
    keyLight.position.set(-2.8, 5.2, 6.8);
    scene.add(keyLight);
    const fillLight = new DirectionalLight(0xd8e8ff, 1.45);
    fillLight.position.set(3.0, 1.6, 2.6);
    scene.add(fillLight);
    const rimLight = new DirectionalLight(0x9ad8ff, 2.2);
    rimLight.position.set(3.6, 1.8, -2.2);
    scene.add(rimLight);

    const root = new Group();
    // Fit full Q-minion (crown tufts → boots → dual shadow) inside ortho frustum.
    root.scale.setScalar(0.88);
    root.position.y = 0.04;
    scene.add(root);

    const layout = Q_MINION_LAYOUT;
    const colors = Q_MINION_COLORS;

    // Dual contact shadow: soft wide pool + denser core under boots (grounded, no plate).
    const shadowTexture = createContactShadowTexture(192);
    const contactShadow = new Mesh(
      new CircleGeometry(1.48, 64),
      new MeshBasicMaterial({ map: shadowTexture, transparent: true, opacity: 1, depthWrite: false }),
    );
    contactShadow.rotation.x = -Math.PI / 2;
    contactShadow.position.set(0, layout.shadowY - 0.02, 0.05);
    contactShadow.scale.set(1.14, 1, 0.52);
    root.add(contactShadow);

    const shadowCore = new Mesh(
      new CircleGeometry(0.68, 48),
      new MeshBasicMaterial({ map: shadowTexture, transparent: true, opacity: 0.78, depthWrite: false }),
    );
    shadowCore.rotation.x = -Math.PI / 2;
    shadowCore.position.set(0, layout.shadowY - 0.01, 0.08);
    shadowCore.scale.set(0.92, 1, 0.55);
    root.add(shadowCore);

    const body = new Group();
    root.add(body);
    const head = new Group();
    head.position.set(0, layout.headY, 0.05);
    body.add(head);

    // Soft warm Q-minion materials: gentle clearcoat, muted metal (no chrome plate).
    const yellow = material(colors.yellow, 0.38, 0.78, colors.yellowEmissive, 0.09);
    const yellowLight = material(colors.yellowLight, 0.34, 0.84, colors.yellowEmissive, 0.11);
    const graphite = material(colors.graphite, 0.34, 0.48);
    const goggleBand = material(colors.goggleBand, 0.42, 0.36);
    const metal = metallicMaterial(colors.metal, 0.36, 0.58);
    const glass = material(colors.iris, 0.16, 0.86, colors.irisEmissive, 0.24);
    const white = material(colors.sclera, 0.55, 0.34);
    const coral = material(colors.blush, 0.58, 0.32, "#d83d2c", 0.05);
    const denim = material(colors.denim, 0.76, 0.14);
    const denimSoft = material(colors.denimSoft, 0.72, 0.18);
    const denimDeep = material(colors.denimDeep, 0.78, 0.12);
    const gloveMat = material(colors.glove, 0.52, 0.18);
    const bootMat = material(colors.boot, 0.62, 0.14);
    const bootSoleMat = material(colors.bootSole, 0.74, 0.08);
    const hairMat = material(colors.hair, 0.48, 0.16);

    // Tall warm yellow capsule body (classic pill silhouette, not a box/plate).
    const torso = makeMesh(
      new CapsuleGeometry(layout.bodyRadius, layout.bodyLength, 22, 56),
      yellow,
      [0, layout.bodyY, 0],
      [1.02, 1.0, 0.94],
    );
    body.add(torso);
    // Soft front belly highlight — rounded lifeform, not a flat plate.
    const belly = makeMesh(new SphereGeometry(0.58, 32, 24), yellowLight, [0, -0.12, 0.62], [1.08, 1.12, 0.4]);
    body.add(belly);
    // Soft crown dome blends into the capsule (keeps Q-round top).
    const crown = makeMesh(new SphereGeometry(0.9, 48, 36), yellowLight, [0, 0.22, 0.02], [1.02, 0.78, 0.9]);
    head.add(crown);
    // Tiny coral antenna: soft secondary spring accent (Q personalization).
    const antennaStem = makeMesh(new CapsuleGeometry(0.024, 0.18, 6, 12), metal, [0.16, 0.94, 0.02], [1, 1, 1]);
    const antennaTip = makeMesh(new SphereGeometry(0.068, 16, 12), coral, [0.16, 1.08, 0.02], [1, 1, 1]);
    head.add(antennaStem, antennaTip);

    // Sparse black hair tufts (spring-damper secondary only).
    const hairGroup = new Group();
    hairGroup.position.set(0, layout.hairY, -0.02);
    head.add(hairGroup);
    const hairTufts: Mesh[] = [];
    const hairSpecs = qMinionHairTuftSpecs().map((spec) => [...spec] as [number, number, number, number]);
    for (const [hx, hy, hz, tilt] of hairSpecs) {
      const tuft = makeMesh(new CapsuleGeometry(0.022, 0.22, 6, 12), hairMat, [hx, hy, hz], [0.85, 1.25, 0.72]);
      tuft.rotation.z = tilt;
      tuft.rotation.x = -0.42;
      hairGroup.add(tuft);
      hairTufts.push(tuft);
    }

    // Black goggle strap wrapping the head (classic band — no square chrome frame).
    const strap = makeMesh(new TorusGeometry(0.9, 0.078, 14, 64), goggleBand, [0, 0.05, 0], [1.04, 0.72, 1.02]);
    strap.rotation.x = Math.PI / 2;
    head.add(strap);
    const strapBridge = makeMesh(new CapsuleGeometry(0.055, 0.22, 8, 18), goggleBand, [0, 0.05, 0.9], [1.1, 0.48, 0.48]);
    strapBridge.rotation.z = Math.PI / 2;
    head.add(strapBridge);
    for (const side of [-1, 1] as const) {
      const sidePad = makeMesh(new CapsuleGeometry(0.07, 0.22, 8, 16), goggleBand, [side * 0.78, 0.05, 0.38], [1, 0.62, 0.5]);
      sidePad.rotation.y = side * 0.55;
      head.add(sidePad);
    }

    // Dual large goggles: soft metal rim + white sclera + brown iris (classic, not square frame).
    const eyeGroups: Group[] = [];
    const irises: Mesh[] = [];
    const pupils: Mesh[] = [];
    const gR = layout.goggleRadius;
    for (const side of [-1, 1] as const) {
      const eyeGroup = new Group();
      const [ex, ey, ez] = qMinionGogglePose(side);
      eyeGroup.position.set(ex, ey, ez);
      // Soft silver rim (round), not a chrome plate.
      const rim = makeMesh(new TorusGeometry(gR, 0.062, 16, 48), metal, [0, 0, 0.02], [1, 1.02, 0.52]);
      const rimInner = makeMesh(new TorusGeometry(gR * 0.9, 0.022, 12, 36), goggleBand, [0, 0, 0.04], [1, 1.02, 0.42]);
      const eye = makeMesh(new SphereGeometry(gR * 0.9, 36, 28), white, [0, 0, 0.05], [1, 1.04, 0.36]);
      const iris = makeMesh(new SphereGeometry(gR * 0.48, 28, 22), glass, [0, -0.004, 0.15], [1, 1, 0.42]);
      const pupil = makeMesh(new SphereGeometry(gR * 0.22, 22, 18), graphite, [0, -0.004, 0.22], [1, 1, 0.42]);
      const shine = makeMesh(new SphereGeometry(0.045, 14, 12), white, [0.07, 0.06, 0.26], [1, 1, 0.5]);
      const shineSoft = makeMesh(new SphereGeometry(0.022, 12, 10), white, [-0.05, -0.04, 0.24], [1, 1, 0.42]);
      eyeGroup.add(rim, rimInner, eye, iris, pupil, shine, shineSoft);
      head.add(eyeGroup);
      eyeGroups.push(eyeGroup);
      irises.push(iris);
      pupils.push(pupil);
    }

    // Soft smile arc.
    const mouth = makeMesh(new TorusGeometry(0.17, 0.028, 10, 28, Math.PI), graphite, [0, -0.4, 0.86], [1.05, 0.7, 0.48]);
    mouth.rotation.z = Math.PI;
    head.add(mouth);
    const cheekMatL = coral.clone();
    cheekMatL.transparent = true;
    cheekMatL.opacity = 0.48;
    const cheekMatR = coral.clone();
    cheekMatR.transparent = true;
    cheekMatR.opacity = 0.48;
    const cheekLeft = makeMesh(new SphereGeometry(0.085, 18, 12), cheekMatL, [-0.66, -0.32, 0.76], [1.38, 0.54, 0.28]);
    const cheekRight = makeMesh(new SphereGeometry(0.085, 18, 12), cheekMatR, [0.66, -0.32, 0.76], [1.38, 0.54, 0.28]);
    head.add(cheekLeft, cheekRight);

    // Blue denim overalls: pants, bib, clear straps, front pocket + metal buttons.
    const overalls = makeMesh(new CapsuleGeometry(0.86, 0.58, 14, 40), denim, [0, layout.overallY, 0.02], [1.04, 0.96, 0.94]);
    const overallBib = makeMesh(new SphereGeometry(0.72, 42, 30), denimDeep, [0, -0.42, 0.42], [0.96, 0.78, 0.55]);
    // Front pocket with flap — classic overalls tell.
    const overallPocket = makeMesh(new CapsuleGeometry(0.16, 0.22, 10, 24), denimSoft, [0, -0.58, 0.92], [1.42, 0.88, 0.22]);
    overallPocket.rotation.z = Math.PI / 2;
    const pocketFlap = makeMesh(new CapsuleGeometry(0.06, 0.32, 8, 16), denim, [0, -0.46, 0.96], [1.2, 0.4, 0.18]);
    pocketFlap.rotation.z = Math.PI / 2;
    const pocketStitch = makeMesh(new CapsuleGeometry(0.02, 0.28, 6, 12), denimDeep, [0, -0.58, 0.98], [1.05, 0.35, 0.14]);
    pocketStitch.rotation.z = Math.PI / 2;
    body.add(overalls, overallBib, overallPocket, pocketFlap, pocketStitch);
    for (const side of [-1, 1] as const) {
      const strapPose = qMinionOverallStrapPose(side);
      const overallStrap = makeMesh(
        new CapsuleGeometry(0.068, 0.92, 8, 20),
        denimSoft,
        [strapPose.strap[0], strapPose.strap[1], strapPose.strap[2]],
        [1, 1, 0.38],
      );
      overallStrap.rotation.z = strapPose.rotationZ;
      // Shoulder pad where strap meets yellow body.
      const strapPad = makeMesh(
        new SphereGeometry(0.1, 16, 12),
        denim,
        [side * 0.48, 0.18, 0.62],
        [1.05, 0.55, 0.55],
      );
      const button = makeMesh(
        new SphereGeometry(0.075, 18, 12),
        metal,
        [strapPose.button[0], strapPose.button[1], strapPose.button[2]],
        [1, 1, 0.32],
      );
      body.add(overallStrap, strapPad, button);
    }

    const arms: Group[] = [];
    const feet: Group[] = [];
    for (const side of [-1, 1] as const) {
      const arm = new Group();
      arm.position.set(side * layout.armX, -0.22, 0.02);
      const upperArm = makeMesh(new CapsuleGeometry(0.13, 0.48, 10, 24), yellow, [side * 0.08, -0.24, 0], [1, 1, 1]);
      upperArm.rotation.z = side * -0.22;
      // Black mitten gloves (classic minion).
      const mitten = makeMesh(new SphereGeometry(0.19, 24, 18), gloveMat, [side * 0.16, -0.58, 0.04], [1.08, 0.9, 0.88]);
      const thumb = makeMesh(new SphereGeometry(0.075, 16, 12), gloveMat, [side * 0.32, -0.52, 0.1], [1, 0.72, 0.78]);
      arm.add(upperArm, mitten, thumb);
      body.add(arm);
      arms.push(arm);

      const foot = new Group();
      foot.position.set(side * 0.38, layout.bootY, 0.14);
      // Brown rounded boots with darker soles.
      const boot = makeMesh(new CapsuleGeometry(0.22, 0.28, 8, 20), bootMat, [0, 0.03, 0.05], [1.2, 0.72, 1.28]);
      boot.rotation.x = Math.PI / 2;
      const bootToe = makeMesh(new SphereGeometry(0.16, 18, 14), bootMat, [0, -0.01, 0.26], [1.24, 0.7, 0.98]);
      const bootCuff = makeMesh(new CapsuleGeometry(0.16, 0.08, 6, 14), bootMat, [0, 0.12, 0.02], [1.15, 0.7, 1.05]);
      const sole = makeMesh(new CapsuleGeometry(0.2, 0.09, 6, 16), bootSoleMat, [0, -0.1, 0.06], [1.28, 0.42, 1.4]);
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
    };
    window.addEventListener("pointermove", trackPointer, { passive: true });

    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
    const timer = new Timer();
    const lifeformPerf = createLifeformPerfTracker();
    const perfEmitGate = createPerfEmitGate();
    let frame = 0;
    let disposed = false;
    // Spring-damper body + secondary (no linear body lerp). High omega tracks hop liveliness.
    const springRootY = createSpringState(-0.02);
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
    const springSweat = createSpringState(0);
    const springMouthSmile = createSpringState(0.82);
    const springHair = createSpringState(0);
    const springAntenna = createSpringState(0);

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
      // VISUAL: soft eye/head IK lag (secondary spring only — no body travel lerp).
      const softGazeX = springToward(springGazeX, gazeX, dt, 11.5, 0.86) * motion;
      const softGazeY = springToward(springGazeY, gazeY, dt, 11.5, 0.86) * motion;
      const sample = sampleBuiltinPetMotion(current.state, current.emotion, elapsed, softGazeX, softGazeY, motion);
      const playing = isPlaying(current.state);

      // Body channels: spring-damper only (never linear lerp). Play spin stays snappy via higher omega.
      const bodyOmega = playing ? 22 : 16.5;
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

      root.position.y = softRootY;
      body.scale.set(softScaleX, softScaleY, 1);
      body.rotation.z = softBodyTilt;
      body.rotation.y = softBodyYaw;

      head.rotation.y = softHeadYaw;
      head.rotation.x = softHeadPitch;
      // Hair lag: spring-damper secondary (follows soft head pitch targets).
      const hairLean = springToward(springHair, sample.hairSway, dt, 8.2, 0.9);
      hairGroup.rotation.x = hairLean;
      hairGroup.rotation.z = Math.sin(elapsed * 1.7) * 0.05 * motion + sample.bodyTilt * 0.12;
      for (let i = 0; i < hairTufts.length; i += 1) {
        hairTufts[i]!.rotation.z = hairSpecs[i]![3] + Math.sin(elapsed * 2.4 + i) * 0.08 * motion + hairLean * 0.15;
        hairTufts[i]!.rotation.x = -0.42 + hairLean * 0.35 + Math.sin(elapsed * 1.9 + i) * 0.04 * motion;
      }
      // Antenna soft lag secondary (follows head pitch with spring delay — never lerp body hop).
      const antennaLean = springToward(
        springAntenna,
        -sample.headPitch * 0.52 + sample.hairSway * 0.32 + sample.rootY * 0.18,
        dt,
        10.5,
        0.88,
      );
      antennaStem.rotation.x = antennaLean * 0.62;
      antennaStem.rotation.z = Math.sin(elapsed * 2.1) * 0.04 * motion;
      antennaTip.position.y = 1.08 + antennaLean * 0.08;
      antennaTip.position.x = 0.16 + Math.sin(elapsed * 2.8) * 0.014 * motion + antennaLean * 0.02;
      antennaTip.scale.setScalar(1 + Math.sin(elapsed * 3.4) * 0.06 * motion);

      // Dual goggles: livelier asymmetric blink via spring + shared gaze IK.
      eyeGroups[0]!.scale.y = softEyeL;
      eyeGroups[1]!.scale.y = softEyeR;
      for (let i = 0; i < eyeGroups.length; i += 1) {
        const sideSign = i === 0 ? -1 : 1;
        const eyeIrisX = sample.irisX + sideSign * 0.004 * motion;
        irises[i]!.position.x = eyeIrisX;
        irises[i]!.position.y = -0.004 + sample.irisY;
        pupils[i]!.position.x = eyeIrisX * 1.1;
        pupils[i]!.position.y = -0.004 + sample.irisY * 1.1;
      }

      // Smile (π) ↔ frown (0) spring; open for yawn / play.
      const smileT = MathUtils.clamp((softMouthSmile + 1) * 0.5, 0, 1);
      mouth.rotation.z = Math.PI * smileT;
      mouth.scale.y = 0.7 + sample.mouthOpen * 0.9;
      mouth.position.y = -0.4 - sample.mouthOpen * 0.04 + (1 - smileT) * 0.03;
      mouth.scale.x = 1.05 + sample.digNose * 0.08 + sample.countAnts * 0.05 + (1 - smileT) * 0.06;

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

      // Dual contact shadow: denser when grounded, shrinks on hop.
      contactShadow.scale.x = sample.shadowScale * 1.06;
      contactShadow.scale.z = 0.54 * sample.shadowScale;
      (contactShadow.material as MeshBasicMaterial).opacity = sample.shadowOpacity;
      shadowCore.scale.x = sample.shadowScale * 0.9;
      shadowCore.scale.z = 0.54 * sample.shadowScale;
      (shadowCore.material as MeshBasicMaterial).opacity = MathUtils.clamp(sample.shadowOpacity * 0.82, 0.3, 0.88);

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

    const resize = () => renderer.setSize(Math.max(canvas.clientWidth, 1), Math.max(canvas.clientHeight, 1), false);
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
