import { describe, expect, it } from "vitest";
import { suggestAnimationMap } from "./CreatorStudio";

describe("suggestAnimationMap", () => {
  it("maps recognized names with action-specific loop behavior", () => {
    expect(suggestAnimationMap(["Idle", "WalkCycle", "MorningStretch", "FriendlyWave"])).toEqual({
      "pet.idle": { animation: "Idle", looped: true },
      "pet.walk": { animation: "WalkCycle", looped: true },
      "pet.stretch": { animation: "MorningStretch", looped: false },
      "pet.click": { animation: "FriendlyWave", looped: false },
    });
  });

  it("does not invent bindings for unrelated names", () => {
    expect(suggestAnimationMap(["Take 001", "Animation"])).toEqual({});
  });
});
