import { describe, expect, it } from "vitest";
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

describe("petStatusMessage", () => {
  it("describes autonomous observation without pretending it is celebration", () => {
    expect(petStatusMessage({ state: "observing", energy: 100, mood: 100, satiety: 100, cleanliness: 100 })).toBe("正好奇地看看桌面");
  });

  it("prioritizes active behavior over vitals", () => {
    expect(petStatusMessage({ state: "sleeping", energy: 10, mood: 10, satiety: 10, cleanliness: 10 })).toBe("正在安静恢复体力");
    expect(petStatusMessage({ state: "walking", energy: 100, mood: 100, satiety: 100, cleanliness: 100 })).toBe("去桌面上走走看看");
    expect(petStatusMessage({ state: "playing", energy: 100, mood: 100, satiety: 100, cleanliness: 100 })).toBe("正在桌面上自得其乐");
  });

  it("expresses low vitals without alarming the user", () => {
    expect(petStatusMessage({ state: "idle", energy: 25, mood: 10, satiety: 10, cleanliness: 10 })).toBe("有点困了，想休息一下");
    expect(petStatusMessage({ state: "idle", energy: 80, mood: 80, satiety: 25, cleanliness: 10 })).toBe("肚子有点空，陪我吃点东西吧");
    expect(petStatusMessage({ state: "idle", energy: 80, mood: 80, satiety: 80, cleanliness: 25 })).toBe("想整理一下，保持清清爽爽");
    expect(petStatusMessage({ state: "idle", energy: 80, mood: 25, satiety: 80, cleanliness: 80 })).toBe("今天想和你待一会儿");
    expect(petStatusMessage({ state: "idle", energy: 80, mood: 80, satiety: 80, cleanliness: 80 })).toBe("在桌面上陪着你呢～");
    expect(petStatusMessage(
      { state: "idle", energy: 80, mood: 80, satiety: 80, cleanliness: 80 },
      { sequence: 2 },
    )).toBe("想不想摸摸我的护目镜？");
  });

  it("reacts to desktop lifeform sense without leaking titles", () => {
    expect(petStatusMessage(
      { state: "idle", energy: 80, mood: 80, satiety: 80, cleanliness: 80 },
      { desktop: { meetingActive: true, meetingHint: "zoom" } },
    )).toBe("会议中，我先安静靠边～");
    expect(petStatusMessage(
      { state: "idle", energy: 80, mood: 80, satiety: 80, cleanliness: 80 },
      { desktop: { onBattery: true, batteryPercent: 12, charging: false } },
    )).toBe("电量有点低，我轻一点活动");
    expect(petStatusMessage(
      { state: "walking", energy: 80, mood: 80, satiety: 80, cleanliness: 80 },
      { desktop: { displayCount: 2 } },
    )).toBe("去隔壁屏幕逛逛");
    expect(petStatusMessage(
      { state: "idle", energy: 80, mood: 80, satiety: 80, cleanliness: 80 },
      { desktop: { notificationUnread: true } },
    )).toBe("好像有事等你看一眼～");
  });

  it("prefers directive speech over ambient vitals status", () => {
    expect(petStatusMessage(
      { state: "working", energy: 80, mood: 80, satiety: 80, cleanliness: 80 },
      { directiveSpeech: "正在帮你看任务进度" },
    )).toBe("正在帮你看任务进度");
    expect(petStatusMessage(
      { state: "working", energy: 80, mood: 80, satiety: 80, cleanliness: 80 },
      { directiveSpeech: "   " },
    )).toBe("正在专心陪你工作");
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
