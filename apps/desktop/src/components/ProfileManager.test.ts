import { describe, expect, it } from "vitest";
import { normalizedProfileName, proactiveFrequencyLabel } from "./ProfileManager";

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
