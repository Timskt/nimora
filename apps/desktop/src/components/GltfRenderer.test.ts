import { describe, expect, it } from "vitest";
import { resolveModelAnimation } from "./GltfRenderer";

const clips = {
  "pet.idle": { animation: "Idle", looped: true },
  "pet.click": { animation: "Wave", looped: false },
};

describe("resolveModelAnimation", () => {
  it("resolves exact and declared fallback actions", () => {
    const available = new Set(["Idle", "Wave"]);
    expect(resolveModelAnimation("pet.click", clips, {}, available)).toEqual(clips["pet.click"]);
    expect(resolveModelAnimation("pet.happy", clips, { "pet.happy": "pet.click" }, available)).toEqual(clips["pet.click"]);
  });

  it("falls back to idle and rejects unavailable model clips", () => {
    expect(resolveModelAnimation("pet.unknown", clips, {}, new Set(["Idle"]))).toEqual(clips["pet.idle"]);
    expect(resolveModelAnimation("pet.click", clips, {}, new Set(["Idle"]))).toBeNull();
  });

  it("terminates cyclic fallback graphs", () => {
    expect(resolveModelAnimation(
      "pet.a",
      clips,
      { "pet.a": "pet.b", "pet.b": "pet.a" },
      new Set(["Idle", "Wave"]),
    )).toEqual(clips["pet.idle"]);
  });
});
