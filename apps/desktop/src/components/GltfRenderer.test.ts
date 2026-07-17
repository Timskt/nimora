import { describe, expect, it } from "vitest";
import { cameraDistanceForRadius, modelAssetUrl } from "./GltfRenderer";

describe("GltfRenderer helpers", () => {
  it("encodes each controlled model path segment", () => {
    expect(modelAssetUrl("nimora-asset://localhost/character.local.aurora/", "models/my model.glb"))
      .toBe("nimora-asset://localhost/character.local.aurora/models/my%20model.glb");
  });

  it("frames finite and degenerate model bounds", () => {
    expect(cameraDistanceForRadius(1, 60)).toBeCloseTo(2);
    expect(cameraDistanceForRadius(0, 60)).toBeGreaterThan(0);
  });
});
