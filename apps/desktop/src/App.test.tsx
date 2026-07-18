import { describe, expect, it } from "vitest";
import { itemPresentation, keepsakePresentation, navigation, navItemClassName, normalizedPetName, runtimeActivities, voiceGain } from "./App";
import { petInventoryQuantity, petItemPresentation } from "./components/petItems";

describe("navItemClassName", () => {
  it("adds the active state only to the selected destination", () => {
    expect(navItemClassName(true)).toBe("nav-item active");
    expect(navItemClassName(false)).toBe("nav-item");
  });
});

describe("navigation", () => {
  it("exposes the local Agent workspace as a first-class destination", () => {
    expect(navigation).toContain("Agent");
    expect(navigation).toContain("自动化");
  });
});

describe("voiceGain", () => {
  it("converts bounded decibels to a safe browser volume", () => {
    expect(voiceGain(0)).toBe(1);
    expect(voiceGain(-6)).toBeCloseTo(0.501, 3);
    expect(voiceGain(6)).toBe(1);
  });
});

describe("normalizedPetName", () => {
  it("trims valid names and rejects empty or oversized names", () => {
    expect(normalizedPetName("  灵栖  ")).toBe("灵栖");
    expect(normalizedPetName(" ")).toBeNull();
    expect(normalizedPetName("灵".repeat(65))).toBeNull();
  });
});

describe("keepsakePresentation", () => {
  it("maps stable domain identifiers to local presentation", () => {
    expect(keepsakePresentation("first_hello")).toEqual({ glyph: "✦", label: "第一次回应" });
    expect(keepsakePresentation("hundred_moments").label).toBe("百刻相伴");
  });
});

describe("itemPresentation", () => {
  it("keeps domain identity separate from localized presentation", () => {
    expect(itemPresentation("berry_bite").label).toBe("莓果小食");
    expect(itemPresentation("bubble_soap").effect).toContain("清洁 +45");
    expect(itemPresentation("star_ball")).toBe(petItemPresentation("star_ball"));
    expect(petInventoryQuantity([
      { itemId: "berry_bite", quantity: 3 },
      { itemId: "star_ball", quantity: 2 },
    ])).toBe(5);
  });
});

describe("runtimeActivities", () => {
  it("surfaces durable queue health without event payloads", () => {
    expect(runtimeActivities({ pending: 4, leased: 1, delivered: 8, deadLetter: 0 })[0]).toEqual({
      title: "持久事件队列健康",
      meta: "4 待投递 · 1 租约中",
      tone: "mint",
    });
    expect(runtimeActivities({ pending: 0, leased: 0, delivered: 8, deadLetter: 2 })[0]?.title).toBe("2 条事件需要处理");
  });
});
