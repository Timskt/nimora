import { describe, expect, it } from "vitest";
import {
  composeLivingMoment,
  DIRTY_LINES,
  HAPPY_IDLE_LINES,
  LOW_BATTERY_LINES,
  LOW_ENERGY_LINES,
  LOW_MOOD_LINES,
  MEETING_LINES,
  MULTI_DISPLAY_LINES,
  NOTIFY_LINES,
  STATE_LINES,
} from "./lifeformLiving";
import {
  clampPetLocalToStage,
  clampPetScreenToStage,
  isStageWorkAreaReady,
  normalizeDirectiveSpeech,
  occlusionClipPath,
  occlusionMutesAmbient,
  occlusionPresentation,
  occlusionVisibleRegions,
  normalizeMotionToken,
  petAmbientMoment,
  petAnimationToken,
  petFacing,
  petLifeformTokens,
  petLocalPosition,
  petMoodBand,
  petScreenPosition,
  petSquashVars,
  petStatusMessage,
  resolveOverlayStage,
  resolvePetRenderState,
  resolvePetSubjectMotion,
  sanitizeStageNumber,
} from "./petPresentation";
describe("petStatusMessage / living presence", () => {
  const HEALTHY = { energy: 80, mood: 80, satiety: 80, cleanliness: 80, emotion: "neutral" as const };
  // Pin the clock to mid-afternoon so circadian morning/night branches never fire.
  const NOON = { hourOfDay: 14 as const, nowMs: 0 };

  it("host directive speech always wins verbatim", () => {
    expect(petStatusMessage(
      { state: "working", ...HEALTHY },
      { directiveSpeech: "正在帮你看任务进度", ...NOON },
    )).toBe("正在帮你看任务进度");
  });

  it("blank directive falls back to a living working line, never robotic silence", () => {
    const moment = composeLivingMoment({ pet: { state: "working", ...HEALTHY }, directiveSpeech: "   ", sequence: 0, ...NOON });
    expect(moment.reason).toBe("state:working");
    expect(STATE_LINES.working).toContain(moment.speech);
  });

  it("named behavior states speak from their living pools, not vitals", () => {
    for (const state of ["observing", "sleeping", "walking", "playing"] as const) {
      const moment = composeLivingMoment({ pet: { state, ...HEALTHY }, sequence: 1, ...NOON });
      expect(moment.reason).toBe(`state:${state}`);
      expect(STATE_LINES[state]).toContain(moment.speech);
    }
  });

  it("expresses low vitals as soft moods without alarming error codes", () => {
    const lowMood = composeLivingMoment({ pet: { state: "idle", ...HEALTHY, mood: 20 }, sequence: 1, ...NOON });
    expect(lowMood.reason).toBe("low-mood");
    expect(LOW_MOOD_LINES).toContain(lowMood.speech);

    const lowEnergy = composeLivingMoment({ pet: { state: "idle", ...HEALTHY, energy: 20 }, sequence: 1, ...NOON });
    expect(lowEnergy.reason).toBe("low-energy");
    expect(LOW_ENERGY_LINES).toContain(lowEnergy.speech);

    const dirty = composeLivingMoment({ pet: { state: "idle", ...HEALTHY, cleanliness: 20 }, sequence: 1, ...NOON });
    expect(dirty.reason).toBe("need-clean");
    expect(DIRTY_LINES).toContain(dirty.speech);
  });

  it("reacts to desktop lifeform sense without leaking window titles", () => {
    const meeting = composeLivingMoment({ pet: { state: "idle", ...HEALTHY }, desktop: { meetingActive: true, meetingHint: "zoom" }, sequence: 1, ...NOON });
    expect(meeting.reason).toBe("meeting");
    expect(MEETING_LINES).toContain(meeting.speech);

    const battery = composeLivingMoment({ pet: { state: "idle", ...HEALTHY }, desktop: { onBattery: true, batteryPercent: 12, charging: false }, sequence: 1, ...NOON });
    expect(battery.reason).toBe("low-battery");
    expect(LOW_BATTERY_LINES).toContain(battery.speech);

    const notify = composeLivingMoment({ pet: { state: "idle", ...HEALTHY }, desktop: { notificationUnread: true }, sequence: 1, ...NOON });
    expect(notify.reason).toBe("notification");
    expect(NOTIFY_LINES).toContain(notify.speech);

    const multi = composeLivingMoment({ pet: { state: "walking", ...HEALTHY }, desktop: { displayCount: 2 }, sequence: 1, ...NOON });
    expect(multi.reason).toBe("multi-display");
    expect(MULTI_DISPLAY_LINES).toContain(multi.speech);
  });

  it("healthy idle draws from a large living pool (not a 3-line loop)", () => {
    const moment = composeLivingMoment({ pet: { state: "idle", ...HEALTHY }, sequence: 7, ...NOON });
    expect(moment.reason).toBe("living-idle");
    expect(HAPPY_IDLE_LINES).toContain(moment.speech);
    expect(HAPPY_IDLE_LINES.length).toBeGreaterThanOrEqual(12);
  });

  it("avoids immediately repeating recent lines when the pool allows", () => {
    const recent = [HAPPY_IDLE_LINES[0]!];
    const moment = composeLivingMoment({ pet: { state: "idle", ...HEALTHY }, sequence: 0, recentLines: recent, ...NOON });
    expect(moment.speech).not.toBe(HAPPY_IDLE_LINES[0]);
  });

  it("petStatusMessage returns a non-empty living string end-to-end", () => {
    const line = petStatusMessage({ state: "idle", ...HEALTHY }, { sequence: 3, hourOfDay: 14, nowMs: 0 });
    expect(typeof line).toBe("string");
    expect(line.length).toBeGreaterThan(0);
  });
});

describe("petFacing", () => {
  it("matches the deterministic native wander direction", () => {
    expect(petFacing({ state: "walking", autonomy: { sequence: 1, nextDueMs: 0, activeUntilMs: 1, activeIntent: "explore" } })).toBe("left");
    expect(petFacing({ state: "walking", autonomy: { sequence: 2, nextDueMs: 0, activeUntilMs: 1, activeIntent: "explore" } })).toBe("right");
  });

  it("does not preserve a movement direction outside walking", () => {
    expect(petFacing({ state: "idle", autonomy: { sequence: 1, nextDueMs: 0, activeUntilMs: null, activeIntent: null } })).toBe("neutral");
    expect(petFacing({ state: "walking", autonomy: undefined })).toBe("right");
  });

  it("faces from animation tokens and heading hints", () => {
    expect(petFacing(
      { state: "idle", autonomy: { sequence: 1, nextDueMs: 0, activeUntilMs: null, activeIntent: null } },
      { animation: "pet.walk" },
    )).toBe("left");
    expect(petFacing(
      { state: "walking", autonomy: { sequence: 2, nextDueMs: 0, activeUntilMs: 1, activeIntent: "explore" } },
      { headingX: -0.4 },
    )).toBe("left");
    expect(petFacing(
      { state: "walking", autonomy: { sequence: 2, nextDueMs: 0, activeUntilMs: 1, activeIntent: "explore" } },
      { headingX: 0.4 },
    )).toBe("right");
  });

  it("keeps neutral facing while directive speech is active", () => {
    expect(petFacing(
      { state: "walking", autonomy: { sequence: 1, nextDueMs: 0, activeUntilMs: 1, activeIntent: "explore" } },
      { directiveSpeech: "我在这儿陪你～" },
    )).toBe("neutral");
  });
});

describe("lifeform mood and animation tokens", () => {
  it("bands vitals mood without inventing new IPC fields", () => {
    expect(petMoodBand(0)).toBe("low");
    expect(petMoodBand(33)).toBe("low");
    expect(petMoodBand(34)).toBe("steady");
    expect(petMoodBand(66)).toBe("steady");
    expect(petMoodBand(67)).toBe("high");
    expect(petMoodBand(100)).toBe("high");
  });

  it("normalizes short PetAction values and pet.* tokens to one vocabulary", () => {
    expect(petAnimationToken("walk")).toBe("pet.walk");
    expect(petAnimationToken("work")).toBe("pet.work");
    expect(petAnimationToken("celebrate")).toBe("pet.celebrate");
    expect(petAnimationToken("pet.sleep")).toBe("pet.sleep");
    expect(petAnimationToken("walking")).toBe("pet.walk");
    expect(petAnimationToken("observing")).toBe("pet.observe");
    expect(petAnimationToken(null)).toBe("pet.idle");
  });

  it("maps micro-performance tokens for Subject path", () => {
    expect(petAnimationToken("yawn")).toBe("pet.yawn");
    expect(petAnimationToken("dig_nose")).toBe("pet.dig_nose");
    expect(petAnimationToken("count_ants")).toBe("pet.count_ants");
    expect(petAnimationToken("wave")).toBe("pet.wave");
    expect(petAnimationToken("look_around")).toBe("pet.look_around");
    expect(petAnimationToken("hop")).toBe("pet.hop");
    expect(petAnimationToken("pet.yawn")).toBe("pet.yawn");
    expect(normalizeMotionToken("pet.look_around")).toBe("look_around");
    expect(normalizeMotionToken("pet.dig_nose")).toBe("dig_nose");
    expect(normalizeMotionToken("pet.hop")).toBe("hop");
    expect(normalizeMotionToken("wave")).toBe("wave");
  });

  it("derives stage data hooks from existing pet + companion action fields", () => {
    const tokens = petLifeformTokens(
      { state: "idle", emotion: "happy", mood: 88 },
      "work",
    );
    expect(tokens).toEqual({
      state: "idle",
      emotion: "happy",
      mood: 88,
      moodBand: "high",
      animation: "pet.work",
    });
    expect(petLifeformTokens({ state: "walking", emotion: "neutral", mood: 20 })).toEqual({
      state: "walking",
      emotion: "neutral",
      mood: 20,
      moodBand: "low",
      animation: "pet.walk",
    });
  });

  it("exposes squash/stretch CSS vars for host scale emission later", () => {
    expect(petSquashVars(1.08, 0.92)).toEqual({
      "--pet-scale-x": "1.08",
      "--pet-scale-y": "0.92",
    });
    expect(petSquashVars()).toEqual({
      "--pet-scale-x": "1",
      "--pet-scale-y": "1",
    });
  });
});

describe("overlay stage local placement", () => {
  it("falls back to a zero origin when the host has not published a stage", () => {
    expect(resolveOverlayStage(undefined)).toEqual({ originX: 0, originY: 0, width: 0, height: 0 });
    expect(petLocalPosition({ x: 120, y: 80 })).toEqual({ localX: 120, localY: 80 });
    expect(isStageWorkAreaReady(undefined)).toBe(false);
  });

  it("subtracts stage origin from screen pet position", () => {
    const stage = { originX: 100, originY: 200, width: 1920, height: 1080 };
    expect(petLocalPosition({ x: 340, y: 560 }, stage)).toEqual({ localX: 240, localY: 360 });
    expect(isStageWorkAreaReady(stage)).toBe(true);
  });

  it("supports multi-monitor negative origins and screen round-trips", () => {
    const stage = { originX: -1920, originY: 0, width: 1920, height: 1080 };
    expect(petLocalPosition({ x: -400, y: 200 }, stage)).toEqual({ localX: 1520, localY: 200 });
    expect(petScreenPosition({ localX: 1520, localY: 200 }, stage)).toEqual({ x: -400, y: 200 });
  });

  it("sanitizes non-finite stage numbers and clamps body into the work area", () => {
    expect(sanitizeStageNumber(Number.NaN)).toBe(0);
    expect(sanitizeStageNumber(Number.POSITIVE_INFINITY, 12)).toBe(12);
    expect(resolveOverlayStage({ originX: Number.NaN, originY: 10, width: -5, height: 800 })).toEqual({
      originX: 0,
      originY: 10,
      width: 0,
      height: 800,
    });

    const stage = { originX: 0, originY: 0, width: 800, height: 600 };
    expect(petLocalPosition({ x: -40, y: 900 }, stage, { clampToStage: true, bodyWidth: 260, bodyHeight: 300 }))
      .toEqual({ localX: 0, localY: 300 });
    expect(clampPetLocalToStage({ localX: 900, localY: -10 }, stage, 260, 300))
      .toEqual({ localX: 540, localY: 0 });
  });
});

describe("occlusion presentation", () => {
  it("normalizes free-region strips into unit rectangles", () => {
    expect(occlusionVisibleRegions([
      { x0: 1.2, y0: 0.2, x1: 0.1, y1: 0.8 },
      { x0: 0.4, y0: 0.4, x1: 0.4, y1: 0.9 },
    ])).toEqual([{ x0: 0.1, y0: 0.2, x1: 1, y1: 0.8 }]);
  });

  it("clips only free regions and hides fully occluded pets", () => {
    expect(occlusionClipPath([])).toBe("inset(100%)");
    expect(occlusionClipPath([{ x0: 0, y0: 0, x1: 0.5, y1: 1 }])).toContain("polygon(");
    expect(occlusionClipPath([
      { x0: 0, y0: 0, x1: 0.4, y1: 1 },
      { x0: 0.6, y0: 0, x1: 1, y1: 1 },
    ])).toContain("path(evenodd fill-box");
    expect(occlusionPresentation({ coverage: 1, fullyHidden: true, strips: [] })).toEqual({
      opacity: 0,
      clipPath: "inset(100%)",
      coverage: 1,
      fullyHidden: true,
    });
    expect(occlusionPresentation({ coverage: 0.2, fullyHidden: false, strips: [] })).toEqual({
      opacity: 1,
      clipPath: "none",
      coverage: 0.2,
      fullyHidden: false,
    });
  });

  it("mutes ambient bubbles above the coverage threshold", () => {
    expect(occlusionMutesAmbient(0.85)).toBe(false);
    expect(occlusionMutesAmbient(0.85001)).toBe(true);
    expect(occlusionMutesAmbient(0.9)).toBe(true);
  });
});

describe("subject motion priority", () => {
  it("lets directive animation/action beat companion signal and lifecycle", () => {
    expect(resolvePetSubjectMotion({
      directiveAnimation: "pet.work",
      directiveAction: "observe",
      companionAction: "play",
      lifecycleState: "sleeping",
    })).toBe("pet.work");
    expect(resolvePetSubjectMotion({
      directiveAnimation: null,
      directiveAction: "observe",
      companionAction: "play",
      lifecycleState: "sleeping",
    })).toBe("observe");
    expect(resolvePetSubjectMotion({
      companionAction: "play",
      lifecycleState: "sleeping",
    })).toBe("play");
    expect(resolvePetSubjectMotion({
      lifecycleState: "walking",
    })).toBe("walking");
    expect(resolvePetSubjectMotion({})).toBe("idle");
  });

  it("normalizes motion into BuiltinPet3D render states", () => {
    expect(resolvePetRenderState("pet.walk")).toBe("walking");
    expect(resolvePetRenderState("work_busy")).toBe("work");
    expect(resolvePetRenderState("working")).toBe("work");
    expect(resolvePetRenderState("work_crash")).toBe("crash");
    expect(resolvePetRenderState("sleep")).toBe("sleeping");
    expect(resolvePetRenderState("observe")).toBe("observing");
    expect(resolvePetRenderState("drag")).toBe("dragged");
    expect(resolvePetRenderState("idle")).toBe("idle");
    expect(resolvePetRenderState(null)).toBe("idle");
  });

  it("keeps directiveSpeech as string | null for exactOptionalPropertyTypes", () => {
    expect(normalizeDirectiveSpeech("你好")).toBe("你好");
    expect(normalizeDirectiveSpeech(null)).toBeNull();
    expect(normalizeDirectiveSpeech(undefined)).toBeNull();
  });
});

describe("screen-space stage clamp", () => {
  it("clamps optimistic drag poses into multi-monitor work areas", () => {
    const stage = { originX: -1920, originY: 0, width: 1920, height: 1080 };
    expect(clampPetScreenToStage({ x: -3000, y: -40 }, stage, 260, 300)).toEqual({
      x: -1920,
      y: 0,
    });
    expect(clampPetScreenToStage({ x: 100, y: 2000 }, stage, 260, 300)).toEqual({
      x: -260,
      y: 780,
    });
  });

  it("passes through screen poses when stage size is unknown", () => {
    expect(clampPetScreenToStage({ x: 12, y: 34 }, { originX: 0, originY: 0, width: 0, height: 0 }))
      .toEqual({ x: 12, y: 34 });
  });
});

describe("petAmbientMoment", () => {
  const HEALTHY = { energy: 80, mood: 80, satiety: 80, cleanliness: 80, emotion: "neutral" as const };
  const NOON = { hourOfDay: 14 as const, nowMs: 0 };

  it("returns the full living moment (speech + attention + micro-act) for gaze wiring", () => {
    const moment = petAmbientMoment({ state: "working" as const, ...HEALTHY }, { sequence: 0, ...NOON });
    expect(typeof moment.speech).toBe("string");
    expect(moment.speech.length).toBeGreaterThan(0);
    expect(typeof moment.attention).toBe("string");
    expect(typeof moment.microAct).toBe("string");
  });

  it("matches petStatusMessage speech for identical inputs (single source of truth)", () => {
    const pet = { state: "idle" as const, ...HEALTHY };
    const options = { sequence: 3, ...NOON };
    expect(petAmbientMoment(pet, options).speech).toBe(petStatusMessage(pet, options));
  });
});

