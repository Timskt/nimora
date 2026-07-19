import { describe, expect, it } from "vitest";
import {
  canPresentPetBubble,
  PET_BUBBLE_DURATION_MS,
  PET_BUBBLE_MAX_CHARACTERS,
  PET_STATUS_COOLDOWN_MS,
  nextPetBubblePresentation,
  normalizePetBubbleText,
  shouldAcceptPetBubble,
} from "./petBubble";

describe("pet bubble presentation", () => {
  it("allows a short local expression while the pet is idle", () => {
    expect(canPresentPetBubble({ menuOpen: false, pointerActive: false })).toBe(true);
    expect(PET_BUBBLE_DURATION_MS).toBeGreaterThanOrEqual(3000);
    expect(PET_BUBBLE_DURATION_MS).toBeLessThanOrEqual(5000);
  });

  it("stays out of the way during menus and pointer gestures", () => {
    expect(canPresentPetBubble({ menuOpen: true, pointerActive: false })).toBe(false);
    expect(canPresentPetBubble({ menuOpen: false, pointerActive: true })).toBe(false);
  });

  it("rate-limits spontaneous status changes without delaying feedback", () => {
    const history = { lastStatusAtMs: 1000, protectedUntilMs: Number.NEGATIVE_INFINITY };
    expect(shouldAcceptPetBubble(history, { text: "去走走", channel: "status" }, 1000 + PET_STATUS_COOLDOWN_MS - 1)).toBe(false);
    expect(shouldAcceptPetBubble(history, { text: "去走走", channel: "status" }, 1000 + PET_STATUS_COOLDOWN_MS)).toBe(true);
    expect(shouldAcceptPetBubble(history, { text: "谢谢你", channel: "feedback" }, 1001)).toBe(true);
    expect(shouldAcceptPetBubble(history, { text: "资源不可用", channel: "error" }, 1001)).toBe(true);
  });

  it("does not let ambient status overwrite active feedback", () => {
    const history = { lastStatusAtMs: Number.NEGATIVE_INFINITY, protectedUntilMs: 5000 };
    expect(shouldAcceptPetBubble(history, { text: "正在散步", channel: "status" }, 4999)).toBe(false);
    expect(shouldAcceptPetBubble(history, { text: "正在散步", channel: "status" }, 5000)).toBe(true);
    expect(shouldAcceptPetBubble(history, { text: "新的互动", channel: "feedback" }, 4999)).toBe(true);
  });

  it("rejects empty expressions", () => {
    expect(shouldAcceptPetBubble({ lastStatusAtMs: Number.NEGATIVE_INFINITY, protectedUntilMs: Number.NEGATIVE_INFINITY }, { text: "  ", channel: "feedback" }, 0)).toBe(false);
  });

  it("creates a new revision when identical feedback repeats", () => {
    const current = { message: "谢谢你", revision: 4, visible: false };
    expect(nextPetBubblePresentation(current, "谢谢你")).toEqual({
      message: "谢谢你",
      revision: 5,
      visible: true,
    });
  });

  it("bounds long Unicode text without splitting code points", () => {
    const text = "  以后就叫我  " + "星".repeat(50) + "🌟🌟  ";
    const normalized = normalizePetBubbleText(text);
    expect([...normalized]).toHaveLength(PET_BUBBLE_MAX_CHARACTERS);
    expect(normalized.endsWith("…")).toBe(true);
    expect(normalized).not.toContain("  ");
  });
});
