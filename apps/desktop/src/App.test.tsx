import { describe, expect, it } from "vitest";
import { navigation, navItemClassName, runtimeActivities } from "./App";

describe("navItemClassName", () => {
  it("adds the active state only to the selected destination", () => {
    expect(navItemClassName(true)).toBe("nav-item active");
    expect(navItemClassName(false)).toBe("nav-item");
  });
});

describe("navigation", () => {
  it("exposes the local Agent workspace as a first-class destination", () => {
    expect(navigation).toContain("Agent");
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
