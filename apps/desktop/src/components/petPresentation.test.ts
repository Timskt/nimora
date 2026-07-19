import { describe, expect, it } from "vitest";
import { petStatusMessage } from "./petPresentation";

describe("petStatusMessage", () => {
  it("describes autonomous observation without pretending it is celebration", () => {
    expect(petStatusMessage({ state: "observing", energy: 100, mood: 100, satiety: 100, cleanliness: 100 })).toBe("正好奇地看看桌面");
  });

  it("prioritizes active behavior over vitals", () => {
    expect(petStatusMessage({ state: "sleeping", energy: 10, mood: 10, satiety: 10, cleanliness: 10 })).toBe("正在安静恢复体力");
    expect(petStatusMessage({ state: "walking", energy: 100, mood: 100, satiety: 100, cleanliness: 100 })).toBe("去桌面上走走看看");
  });

  it("expresses low vitals without alarming the user", () => {
    expect(petStatusMessage({ state: "idle", energy: 25, mood: 10, satiety: 10, cleanliness: 10 })).toBe("有点困了，想休息一下");
    expect(petStatusMessage({ state: "idle", energy: 80, mood: 80, satiety: 25, cleanliness: 10 })).toBe("肚子有点空，陪我吃点东西吧");
    expect(petStatusMessage({ state: "idle", energy: 80, mood: 80, satiety: 80, cleanliness: 25 })).toBe("想整理一下，保持清清爽爽");
    expect(petStatusMessage({ state: "idle", energy: 80, mood: 25, satiety: 80, cleanliness: 80 })).toBe("今天想和你待一会儿");
    expect(petStatusMessage({ state: "idle", energy: 80, mood: 80, satiety: 80, cleanliness: 80 })).toBe("本地陪伴中");
  });
});
