import { describe, expect, it } from "vitest";
import { BUILTIN_FOX_RENDERER } from "./builtinFox";

const PUBLIC_ACTIONS = [
  "pet.idle",
  "pet.observe",
  "pet.walk",
  "pet.play",
  "pet.perch",
  "pet.climb",
  "pet.peek",
  "pet.stretch",
  "pet.sleep",
  "pet.drag",
  "pet.click",
  "pet.work",
  "pet.celebrate",
] as const;

describe("BUILTIN_FOX_RENDERER", () => {
  it("loads the packaged animated fox", () => {
    expect(BUILTIN_FOX_RENDERER.assetBaseUrl).toBe("/models/");
    expect(BUILTIN_FOX_RENDERER.model).toBe("companion-fox.glb");
    expect(BUILTIN_FOX_RENDERER.backend).toBe("gltf");
  });

  it("maps every public pet action to an available fox animation", () => {
    const clips = BUILTIN_FOX_RENDERER.animationMap?.clips ?? {};
    expect(PUBLIC_ACTIONS.every((action) => action in clips)).toBe(true);
    expect(new Set(Object.values(clips).map(({ animation }) => animation))).toEqual(
      new Set(["Survey", "Walk", "Run"]),
    );
  });
});
