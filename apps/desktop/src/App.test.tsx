import { describe, expect, it } from "vitest";
import { keepsakePresentation, navigation, navItemClassName, runtimeActivities, voiceGain } from "./App";

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

describe("keepsakePresentation", () => {
  it("maps stable domain identifiers to local presentation", () => {
    expect(keepsakePresentation("first_hello")).toEqual({ glyph: "✦", label: "第一次回应" });
    expect(keepsakePresentation("hundred_moments").label).toBe("百刻相伴");
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
