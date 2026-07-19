import { describe, expect, it } from "vitest";
import { builtinPetBodyYaw, builtinPetPose, clampGaze } from "./BuiltinPet3D";

describe("BuiltinPet3D behavior", () => {
  it("keeps pointer gaze within a safe head rotation range", () => {
    expect(clampGaze(-4)).toBe(-1);
    expect(clampGaze(0.35)).toBe(0.35);
    expect(clampGaze(9)).toBe(1);
  });

  it("uses the third dimension for turning and full play spins", () => {
    expect(Math.abs(builtinPetBodyYaw("walking", 1, 0))).toBeGreaterThan(0.8);
    expect(Math.abs(builtinPetBodyYaw("observing", 2, 0))).toBeGreaterThan(0.5);
    expect(builtinPetBodyYaw("playing", Math.PI * 2 / 1.65, 0)).toBeCloseTo(Math.PI * 2);
  });

  it("gives active and resting states distinct motion", () => {
    expect(builtinPetPose("walking", "happy").bounce).toBeGreaterThan(builtinPetPose("idle", "neutral").bounce);
    expect(builtinPetPose("playing", "happy").bounce).toBeGreaterThan(builtinPetPose("walking", "neutral").bounce);
    expect(builtinPetPose("sleeping", "sleepy").eyeScale).toBeLessThan(0.1);
    expect(builtinPetPose("observing", "surprised").eyeScale).toBeGreaterThan(1);
  });
});
