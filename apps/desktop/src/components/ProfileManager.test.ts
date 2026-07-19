import { describe, expect, it } from "vitest";
import { careNeedsModeLabel, cursorApproachLabel, minuteToTime, normalizedProfileName, proactiveFrequencyLabel, profileModeGuidance, quietHoursLabel, statusBubblesLabel, timeToMinute } from "./ProfileManager";

describe("normalizedProfileName", () => {
  it("trims valid names at the UI boundary", () => {
    expect(normalizedProfileName("  安静创作  ")).toBe("安静创作");
  });

  it("rejects empty and oversized names", () => {
    expect(normalizedProfileName("   ")).toBeNull();
    expect(normalizedProfileName("a".repeat(65))).toBeNull();
  });
});

describe("proactiveFrequencyLabel", () => {
  it("explains that zero disables autonomy", () => {
    expect(proactiveFrequencyLabel(0)).toBe("关闭自主互动");
    expect(proactiveFrequencyLabel(25)).toBe("主动频率 25%");
  });
});

describe("careNeedsModeLabel", () => {
  it("uses full care for migrated profiles and labels every policy", () => {
    expect(careNeedsModeLabel(undefined)).toBe("低压力完整照料");
    expect(careNeedsModeLabel("simple")).toBe("简化照料");
    expect(careNeedsModeLabel("off")).toBe("生命衰减关闭");
  });
});

describe("cursorApproachLabel", () => {
  it("keeps legacy profiles enabled and makes opt-out explicit", () => {
    expect(cursorApproachLabel(undefined)).toBe("偶尔靠近鼠标");
    expect(cursorApproachLabel(true)).toBe("偶尔靠近鼠标");
    expect(cursorApproachLabel(false)).toBe("不靠近鼠标");
  });
});

describe("statusBubblesLabel", () => {
  it("keeps legacy profiles enabled while making quiet profiles explicit", () => {
    expect(statusBubblesLabel(undefined)).toBe("自主气泡开启");
    expect(statusBubblesLabel(true)).toBe("自主气泡开启");
    expect(statusBubblesLabel(false)).toBe("自主气泡关闭");
  });
});

describe("profileModeGuidance", () => {
  it("distinguishes focus quieting from presentation hiding", () => {
    expect(profileModeGuidance("focus")).toContain("手动互动仍然可用");
    expect(profileModeGuidance("presentation")).toContain("桌宠会自动隐藏");
    expect(profileModeGuidance("companion")).toBeNull();
  });
});

describe("quiet-hour presentation", () => {
  it("converts HTML time values without timezone ambiguity", () => {
    expect(minuteToTime(1_320)).toBe("22:00");
    expect(timeToMinute("07:00")).toBe(420);
    expect(timeToMinute("24:00")).toBeNull();
  });

  it("summarizes enabled and migrated policies", () => {
    expect(quietHoursLabel(initialPolicyForTest())).toBe("无安静时段");
    expect(quietHoursLabel({ ...initialPolicyForTest(), quietHours: { enabled: true, startMinute: 1_320, endMinute: 420 } })).toBe("安静时段 22:00–07:00");
  });
});

function initialPolicyForTest() {
  return {
    mode: "companion" as const,
    alwaysOnTop: true,
    clickThrough: false,
    soundEnabled: true,
    proactiveFrequency: 25,
    cursorApproachEnabled: true,
    statusBubblesEnabled: true,
    careNeedsMode: "full" as const,
  };
}
