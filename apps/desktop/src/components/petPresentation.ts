import type { Pet } from "@nimora/schemas";

export type PetFacing = "left" | "right" | "neutral";
export type PetMoodBand = "low" | "steady" | "high";

/** Short public pet actions already used by IPC / agent companion signals. */
const PET_ACTION_TOKENS: Record<string, string> = {
  idle: "pet.idle",
  observe: "pet.observe",
  walk: "pet.walk",
  play: "pet.play",
  perch: "pet.perch",
  climb: "pet.climb",
  peek: "pet.peek",
  stretch: "pet.stretch",
  sleep: "pet.sleep",
  drag: "pet.drag",
  click: "pet.click",
  work: "pet.work",
  celebrate: "pet.celebrate",
  // Micro-performance directive tokens (Subject path).
  yawn: "pet.yawn",
  dig_nose: "pet.dig_nose",
  "dig-nose": "pet.dig_nose",
  count_ants: "pet.count_ants",
  "count-ants": "pet.count_ants",
  wave: "pet.wave",
  look_around: "pet.look_around",
  "look-around": "pet.look_around",
  hop: "pet.hop",
};

/** Canonical pet lifecycle states → public animation tokens. */
const PET_STATE_TOKENS: Record<string, string> = {
  idle: "pet.idle",
  observing: "pet.observe",
  walking: "pet.walk",
  playing: "pet.play",
  perching: "pet.perch",
  climbing: "pet.climb",
  peeking: "pet.peek",
  stretching: "pet.stretch",
  sleeping: "pet.sleep",
  dragged: "pet.drag",
  interacting: "pet.click",
  working: "pet.work",
  recovering: "pet.idle",
  // Micro-performance states (directive / host vocabulary).
  yawn: "pet.yawn",
  dig_nose: "pet.dig_nose",
  count_ants: "pet.count_ants",
  wave: "pet.wave",
  look_around: "pet.look_around",
  hop: "pet.hop",
};

export interface PetFacingHints {
  /** Prefer host/animation token when state alone is ambiguous. */
  animation?: string | null;
  /** Screen-space heading proxy (+right / -left) when available. */
  headingX?: number | null;
  /** Active directive speech keeps an attentive neutral face toward the user. */
  directiveSpeech?: string | null;
}

export interface PetStatusOptions {
  /** Prefer host directive speech over ambient vitals status. */
  directiveSpeech?: string | null;
}

/** Resolve left/right stage facing for walk cycles; speech keeps neutral attention. */
export function petFacing(
  pet: Pick<Pet, "state" | "autonomy">,
  hints?: PetFacingHints,
): PetFacing {
  const speech = hints?.directiveSpeech?.trim();
  if (speech) return "neutral";

  const animation = petAnimationToken(hints?.animation ?? pet.state);
  const walking = pet.state === "walking" || animation === "pet.walk";
  if (!walking) return "neutral";

  if (typeof hints?.headingX === "number" && Number.isFinite(hints.headingX)) {
    if (hints.headingX < -0.02) return "left";
    if (hints.headingX > 0.02) return "right";
  }
  return (pet.autonomy?.sequence ?? 0) % 2 === 0 ? "right" : "left";
}

/** Ambient status line; directive speech wins when the host is talking. */
export function petStatusMessage(
  pet: Pick<Pet, "state" | "energy" | "mood" | "satiety" | "cleanliness">,
  options?: PetStatusOptions,
): string {
  const directive = options?.directiveSpeech?.trim();
  if (directive) return directive;

  switch (pet.state) {
    case "observing": return "正好奇地看看桌面";
    case "sleeping": return "正在安静恢复体力";
    case "walking": return "去桌面上走走看看";
    case "playing": return "正在桌面上自得其乐";
    case "stretching": return "舒舒服服伸个懒腰";
    case "working": return "正在专心陪你工作";
    case "dragged": return "抓稳啦…";
    case "interacting": return "很开心和你互动";
    default:
      if (pet.energy <= 25) return "有点困了，想休息一下";
      if (pet.satiety <= 25) return "肚子有点空，陪我吃点东西吧";
      if (pet.cleanliness <= 25) return "想整理一下，保持清清爽爽";
      if (pet.mood <= 25) return "今天想和你待一会儿";
      return "本地陪伴中";
  }
}

/** Map vitals mood (0–100) into a coarse lifeform band for CSS / data hooks. */
export function petMoodBand(mood: number): PetMoodBand {
  if (mood <= 33) return "low";
  if (mood <= 66) return "steady";
  return "high";
}

/**
 * Normalize animation tokens from pet state or short PetAction values.
 * Prefer existing IPC vocabulary (`pet.idle`, `walk`, `working`, …).
 */
export function petAnimationToken(stateOrAction: string | null | undefined): string {
  if (!stateOrAction) return "pet.idle";
  if (stateOrAction.startsWith("pet.")) return stateOrAction;
  if (stateOrAction in PET_ACTION_TOKENS) return PET_ACTION_TOKENS[stateOrAction]!;
  return PET_STATE_TOKENS[stateOrAction] ?? "pet.idle";
}

/**
 * exactOptionalPropertyTypes-safe speech value.
 * Always returns string | null (never undefined) for optional directive fields.
 */
export function normalizeDirectiveSpeech(speech: string | null | undefined): string | null {
  return typeof speech === "string" ? speech : null;
}

export interface PetSubjectMotionInput {
  /** Host structured directive animation clip token (highest priority). */
  directiveAnimation?: string | null;
  /** Host structured directive action / lifecycle hint. */
  directiveAction?: string | null;
  /** FE companion signal action (agent thinking / running, …). */
  companionAction?: string | null;
  /** Pet lifecycle state from snapshot (lowest priority). */
  lifecycleState?: string | null;
}

/**
 * Subject rule: directive.animation/action beats companion signal / lifecycle.
 * Returns the first non-empty motion source without normalizing vocabulary.
 */
export function resolvePetSubjectMotion(input: PetSubjectMotionInput): string {
  for (const candidate of [
    input.directiveAnimation,
    input.directiveAction,
    input.companionAction,
    input.lifecycleState,
  ]) {
    if (typeof candidate === "string" && candidate.trim()) return candidate.trim();
  }
  return "idle";
}

/**
 * Normalize subject motion into BuiltinPet3D / SVG lifecycle vocabulary
 * (`walking`, `work`, `sleeping`, …) while preserving unknown tokens.
 */
export function resolvePetRenderState(motion: string | null | undefined): string {
  if (!motion || !motion.trim()) return "idle";
  const raw = motion.trim();
  const token = raw.startsWith("pet.") ? raw.slice(4) : raw;
  if (!token) return "idle";
  if (token === "work_busy" || token === "work-busy" || token === "working") return "work";
  if (token === "work_crash" || token === "work-crash") return "crash";
  if (token === "walk") return "walking";
  if (token === "play") return "playing";
  if (token === "sleep" || token === "rest") return "sleeping";
  if (token === "observe") return "observing";
  if (token === "perch") return "perching";
  if (token === "climb") return "climbing";
  if (token === "peek") return "peeking";
  if (token === "stretch") return "stretching";
  if (token === "drag" || token === "dragged") return "dragged";
  if (token === "click" || token === "interacting") return "interacting";
  if (token === "celebrate") return "celebrate";
  if (token === "idle" || token === "recovering") return "idle";
  // First-class micro-performances pass through for BuiltinPet3D.
  if (
    token === "yawn"
    || token === "dig_nose"
    || token === "dig-nose"
    || token === "count_ants"
    || token === "count-ants"
    || token === "wave"
    || token === "look_around"
    || token === "look-around"
    || token === "hop"
  ) {
    if (token === "dig-nose") return "dig_nose";
    if (token === "count-ants") return "count_ants";
    if (token === "look-around") return "look_around";
    return token;
  }
  return token;
}

/**
 * Normalize Subject motion tokens for stage / BuiltinPet3D consumers.
 * Alias of {@link resolvePetRenderState} kept for directive wiring call sites.
 */
export function normalizeMotionToken(motion: string | null | undefined): string {
  return resolvePetRenderState(motion);
}

export interface PetLifeformTokens {
  state: string;
  emotion: string;
  mood: number;
  moodBand: PetMoodBand;
  animation: string;
}

/** Derive lifeform mood/animation tokens from already-shipped pet + action fields. */
export function petLifeformTokens(
  pet: Pick<Pet, "state" | "emotion" | "mood"> | null | undefined,
  companionAction?: string | null,
): PetLifeformTokens {
  const state = pet?.state ?? "idle";
  const emotion = pet?.emotion ?? "neutral";
  const mood = typeof pet?.mood === "number" ? pet.mood : 70;
  return {
    state,
    emotion,
    mood,
    moodBand: petMoodBand(mood),
    animation: petAnimationToken(companionAction ?? state),
  };
}

/** CSS custom properties ready for host-driven squash/stretch scale. */
export function petSquashVars(scaleX = 1, scaleY = 1): Record<string, string> {
  return {
    "--pet-scale-x": String(scaleX),
    "--pet-scale-y": String(scaleY),
  };
}

export const PET_BODY_WIDTH_PX = 260;
export const PET_BODY_HEIGHT_PX = 300;

export interface OverlayStageOrigin {
  originX: number;
  originY: number;
  width?: number;
  height?: number;
}

/** Finite stage number helper (multi-monitor origins may be negative). */
export function sanitizeStageNumber(value: number | null | undefined, fallback = 0): number {
  if (typeof value !== "number" || !Number.isFinite(value)) return fallback;
  return value;
}

/** True when the host published a positive work-area size for the overlay stage. */
export function isStageWorkAreaReady(stage?: OverlayStageOrigin | null): boolean {
  const resolved = resolveOverlayStage(stage);
  return resolved.width > 0 && resolved.height > 0;
}

/** Resolve stage origin for screen→local placement (browser preview falls back to 0,0). */
export function resolveOverlayStage(stage?: OverlayStageOrigin | null): Required<Pick<OverlayStageOrigin, "originX" | "originY">> & { width: number; height: number } {
  return {
    originX: sanitizeStageNumber(stage?.originX, 0),
    originY: sanitizeStageNumber(stage?.originY, 0),
    width: Math.max(0, sanitizeStageNumber(stage?.width, 0)),
    height: Math.max(0, sanitizeStageNumber(stage?.height, 0)),
  };
}

export interface PetLocalPositionOptions {
  /** Keep the 260×300 body inside the stage work area when dimensions are known. */
  clampToStage?: boolean;
  bodyWidth?: number;
  bodyHeight?: number;
}

/** Clamp stage-local top-left so the pet body stays inside the work area. */
export function clampPetLocalToStage(
  local: { localX: number; localY: number },
  stage?: OverlayStageOrigin | null,
  bodyWidth = PET_BODY_WIDTH_PX,
  bodyHeight = PET_BODY_HEIGHT_PX,
): { localX: number; localY: number } {
  const origin = resolveOverlayStage(stage);
  if (origin.width <= 0 || origin.height <= 0) return local;
  const maxX = Math.max(0, origin.width - Math.max(1, bodyWidth));
  const maxY = Math.max(0, origin.height - Math.max(1, bodyHeight));
  return {
    localX: Math.min(Math.max(0, local.localX), maxX),
    localY: Math.min(Math.max(0, local.localY), maxY),
  };
}

/** Convert screen pet position into stage-local CSS coordinates (supports negative multi-monitor origins). */
export function petLocalPosition(
  screen: { x: number; y: number },
  stage?: OverlayStageOrigin | null,
  options?: PetLocalPositionOptions,
): { localX: number; localY: number } {
  const origin = resolveOverlayStage(stage);
  const local = {
    localX: sanitizeStageNumber(screen.x) - origin.originX,
    localY: sanitizeStageNumber(screen.y) - origin.originY,
  };
  if (options?.clampToStage) {
    return clampPetLocalToStage(
      local,
      origin,
      options.bodyWidth ?? PET_BODY_WIDTH_PX,
      options.bodyHeight ?? PET_BODY_HEIGHT_PX,
    );
  }
  return local;
}

/** Inverse of petLocalPosition for host drag / multi-monitor round-trips. */
export function petScreenPosition(
  local: { localX: number; localY: number },
  stage?: OverlayStageOrigin | null,
): { x: number; y: number } {
  const origin = resolveOverlayStage(stage);
  return {
    x: sanitizeStageNumber(local.localX) + origin.originX,
    y: sanitizeStageNumber(local.localY) + origin.originY,
  };
}

/**
 * Clamp a screen-space pet top-left into the stage work area (multi-monitor safe).
 * Used by optimistic drag so the body cannot leave the current stage bounds.
 */
export function clampPetScreenToStage(
  screen: { x: number; y: number },
  stage?: OverlayStageOrigin | null,
  bodyWidth = PET_BODY_WIDTH_PX,
  bodyHeight = PET_BODY_HEIGHT_PX,
): { x: number; y: number } {
  if (!isStageWorkAreaReady(stage)) {
    return {
      x: sanitizeStageNumber(screen.x),
      y: sanitizeStageNumber(screen.y),
    };
  }
  const local = petLocalPosition(screen, stage, {
    clampToStage: true,
    bodyWidth,
    bodyHeight,
  });
  return petScreenPosition(local, stage);
}

export interface OcclusionStrip {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

function clampUnit(value: number): number {
  if (!Number.isFinite(value)) return 0;
  if (value < 0) return 0;
  if (value > 1) return 1;
  return value;
}

/** Normalize free-region strips into unit-space axis-aligned rectangles. */
export function occlusionVisibleRegions(strips: readonly OcclusionStrip[] | null | undefined): OcclusionStrip[] {
  if (!strips?.length) return [];
  const regions: OcclusionStrip[] = [];
  for (const strip of strips) {
    const x0 = clampUnit(Math.min(strip.x0, strip.x1));
    const x1 = clampUnit(Math.max(strip.x0, strip.x1));
    const y0 = clampUnit(Math.min(strip.y0, strip.y1));
    const y1 = clampUnit(Math.max(strip.y0, strip.y1));
    if (x1 - x0 < 0.001 || y1 - y0 < 0.001) continue;
    regions.push({ x0, y0, x1, y1 });
  }
  return regions;
}

/**
 * CSS clip-path that keeps the pet visible only inside free regions.
 * Empty free regions yield a fully clipped path (caller may also set opacity).
 * Unit-space strips map to percentage polygons / fill-box paths.
 */
export function occlusionClipPath(strips: readonly OcclusionStrip[] | null | undefined): string {
  const regions = occlusionVisibleRegions(strips);
  if (regions.length === 0) return "inset(100%)";
  if (regions.length === 1) {
    const region = regions[0]!;
    const x0 = `${(region.x0 * 100).toFixed(2)}%`;
    const y0 = `${(region.y0 * 100).toFixed(2)}%`;
    const x1 = `${(region.x1 * 100).toFixed(2)}%`;
    const y1 = `${(region.y1 * 100).toFixed(2)}%`;
    return `polygon(${x0} ${y0}, ${x1} ${y0}, ${x1} ${y1}, ${x0} ${y1})`;
  }
  const d = regions.map((region) => {
    const x0 = region.x0.toFixed(4);
    const y0 = region.y0.toFixed(4);
    const x1 = region.x1.toFixed(4);
    const y1 = region.y1.toFixed(4);
    return `M ${x0} ${y0} L ${x1} ${y0} L ${x1} ${y1} L ${x0} ${y1} Z`;
  }).join(" ");
  return `path(evenodd fill-box, "${d}")`;
}

export interface OcclusionStyle {
  opacity: number;
  clipPath: string;
  coverage: number;
  fullyHidden: boolean;
}

/** Offline-first presentation style for host occlusion events. */
export function occlusionPresentation(
  occlusion: { coverage: number; fullyHidden: boolean; strips: readonly OcclusionStrip[] } | null | undefined,
): OcclusionStyle {
  if (!occlusion || occlusion.fullyHidden) {
    return {
      opacity: 0,
      clipPath: "inset(100%)",
      coverage: occlusion?.coverage ?? 1,
      fullyHidden: true,
    };
  }
  const coverage = Number.isFinite(occlusion.coverage) ? Math.min(1, Math.max(0, occlusion.coverage)) : 0;
  const regions = occlusionVisibleRegions(occlusion.strips);
  // No free-region geometry yet: keep fully visible while coverage is partial metadata-only.
  if (regions.length === 0) {
    return {
      opacity: 1,
      clipPath: "none",
      coverage,
      fullyHidden: false,
    };
  }
  return {
    opacity: 1,
    clipPath: occlusionClipPath(regions),
    coverage,
    fullyHidden: false,
  };
}

/** Ambient bubbles should mute when the pet is mostly covered. */
export function occlusionMutesAmbient(coverage: number, threshold = 0.85): boolean {
  return Number.isFinite(coverage) && coverage > threshold;
}
