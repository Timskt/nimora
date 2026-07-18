import { describe, expect, it } from "vitest";
import { petStatusMessage } from "./petPresentation";

describe("petStatusMessage", () => {
  it("prioritizes active behavior over vitals", () => {
    expect(petStatusMessage({ state: "sleeping", energy: 10, mood: 10 })).toBe("正在安静恢复体力");
    expect(petStatusMessage({ state: "walking", energy: 100, mood: 100 })).toBe("去桌面上走走看看");
  });

  it("expresses low vitals without alarming the user", () => {
    expect(petStatusMessage({ state: "idle", energy: 25, mood: 10 })).toBe("有点困了，想休息一下");
    expect(petStatusMessage({ state: "idle", energy: 80, mood: 25 })).toBe("今天想和你待一会儿");
    expect(petStatusMessage({ state: "idle", energy: 80, mood: 80 })).toBe("本地陪伴中");
  });
});
