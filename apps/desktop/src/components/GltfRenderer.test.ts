import { describe, expect, it } from "vitest";
import {
  dispatchModelAction,
  isThreeDimensionalBackend,
  resolveModelAnimation,
} from "./GltfRenderer";
import {
  applyVrmExpression,
  resolveVrmExpression,
  VRM_EXPRESSION_PRESETS,
} from "./vrmExpressions";

it("routes only verified GLTF and VRM backends through the 3D adapter", () => {
  expect(isThreeDimensionalBackend("gltf")).toBe(true);
  expect(isThreeDimensionalBackend("vrm")).toBe(true);
  expect(isThreeDimensionalBackend("live2d")).toBe(false);
  expect(isThreeDimensionalBackend("sprite-atlas")).toBe(false);
});

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

describe("VRM expression semantics", () => {
  it("maps public actions only to fixed VRM presets", () => {
    expect(resolveVrmExpression("pet.observe")).toEqual({ name: "surprised", weight: 0.22 });
    expect(resolveVrmExpression("pet.perch")).toEqual({ name: "relaxed", weight: 0.28 });
    expect(resolveVrmExpression("pet.climb")).toEqual({ name: "surprised", weight: 0.18 });
    expect(resolveVrmExpression("pet.peek")).toEqual({ name: "surprised", weight: 0.42 });
    expect(resolveVrmExpression("pet.stretch")).toEqual({ name: "happy", weight: 0.32 });
    expect(resolveVrmExpression("pet.click")).toEqual({ name: "happy", weight: 0.85 });
    expect(resolveVrmExpression("pet.celebrate")).toEqual({ name: "happy", weight: 1 });
    expect(resolveVrmExpression("pet.drag")).toEqual({ name: "surprised", weight: 0.55 });
    expect(resolveVrmExpression("pet.sleep")).toEqual({ name: "relaxed", weight: 0.7 });
    expect(resolveVrmExpression("pet.error")).toEqual({ name: "sad", weight: 0.65 });
    expect(resolveVrmExpression("pet.work")).toBeNull();
    expect(resolveVrmExpression("vendor.private-expression")).toBeNull();
    expect(VRM_EXPRESSION_PRESETS).not.toContain("vendor.private-expression");
  });

  it("allows verified package mappings to override public actions only", () => {
    const overrides = {
      "pet.click": { preset: "surprised" as const, weight: 0.4 },
    };
    expect(resolveVrmExpression("pet.click", overrides)).toEqual({ name: "surprised", weight: 0.4 });
    expect(resolveVrmExpression("pet.sleep", overrides)).toEqual({ name: "relaxed", weight: 0.7 });
  });

  it("resets stale values before setting an available preset", () => {
    const calls: string[] = [];
    const controller = {
      getExpression: (name: string) => name === "happy" ? {} : null,
      resetValues: () => calls.push("reset"),
      setValue: (name: string, weight: number) => calls.push(`${name}:${weight}`),
    };

    expect(applyVrmExpression(controller, "pet.click")).toBe(true);
    expect(calls).toEqual(["reset", "happy:0.85"]);
  });

  it("returns to neutral for unmapped actions and missing presets", () => {
    const calls: string[] = [];
    const controller = {
      getExpression: () => null,
      resetValues: () => calls.push("reset"),
      setValue: () => calls.push("set"),
    };

    expect(applyVrmExpression(controller, "pet.work")).toBe(true);
    expect(applyVrmExpression(controller, "pet.click")).toBe(false);
    expect(calls).toEqual(["reset", "reset"]);
  });

  it("fails closed when the controller is absent or damaged", () => {
    expect(applyVrmExpression(undefined, "pet.click")).toBe(false);
    expect(applyVrmExpression({
      getExpression: () => ({}),
      resetValues: () => { throw new Error("damaged manager"); },
      setValue: () => undefined,
    }, "pet.click")).toBe(false);
  });

  it("dispatches VRM expressions when no animation clip player exists", () => {
    const calls: string[] = [];
    dispatchModelAction("vrm", {
      getExpression: () => ({}),
      resetValues: () => calls.push("reset"),
      setValue: (name, weight) => calls.push(`${name}:${weight}`),
    }, null, "pet.sleep");

    expect(calls).toEqual(["reset", "relaxed:0.7"]);
  });

  it("dispatches a verified package expression override", () => {
    const calls: string[] = [];
    dispatchModelAction("vrm", {
      getExpression: () => ({}),
      resetValues: () => calls.push("reset"),
      setValue: (name, weight) => calls.push(`${name}:${weight}`),
    }, null, "pet.click", {
      "pet.click": { preset: "sad", weight: 0.25 },
    });

    expect(calls).toEqual(["reset", "sad:0.25"]);
  });

  it("keeps GLTF animation dispatch independent from VRM expressions", () => {
    const calls: string[] = [];
    dispatchModelAction("gltf", undefined, (action) => calls.push(action), "pet.walk");
    expect(calls).toEqual(["pet.walk"]);
  });

  it("keeps every declared weight within the normalized range", () => {
    for (const action of ["pet.observe", "pet.click", "pet.celebrate", "pet.drag", "pet.sleep", "pet.error"]) {
      const binding = resolveVrmExpression(action);
      expect(binding?.weight).toBeGreaterThanOrEqual(0);
      expect(binding?.weight).toBeLessThanOrEqual(1);
    }
  });
});

import { BoxGeometry, Mesh, MeshBasicMaterial, Object3D } from "three";
import {
  cameraDistanceForGroundedPet,
  cameraDistanceForRadius,
  CONTACT_SHADOW_STOPS,
  contactShadowRadius,
  frameGroundedModel,
} from "./petSceneHelpers";

describe("pet scene framing", () => {
  it("grounds the model so feet rest near y=0", () => {
    const root = new Object3D();
    const mesh = new Mesh(new BoxGeometry(2, 4, 1), new MeshBasicMaterial());
    mesh.position.y = 5;
    root.add(mesh);
    const framed = frameGroundedModel(root, 1);
    expect(framed.height).toBeCloseTo(4, 4);
    expect(framed.lookAtY).toBeCloseTo(1.68, 2);
    expect(framed.spanX).toBeCloseTo(2, 4);
    expect(root.position.y).toBeCloseTo(-3, 4);
  });

  it("frames camera distance from body height rather than a tiny model", () => {
    const byRadius = cameraDistanceForRadius(1, 32);
    const grounded = cameraDistanceForGroundedPet(2, 1.2, 0.8, 32);
    expect(grounded).toBeGreaterThan(0);
    expect(byRadius).toBeGreaterThan(0);
    expect(cameraDistanceForGroundedPet(4, 2, 1, 32)).toBeGreaterThan(
      cameraDistanceForGroundedPet(2, 1, 0.5, 32),
    );
  });
});

describe("contact shadow grounding", () => {
  it("keeps a strong soft oval under grounded pets", () => {
    expect(contactShadowRadius(2, 1)).toBeCloseTo(Math.max(2, 1 * 0.38) * 0.68, 5);
    expect(contactShadowRadius(1, 2)).toBeCloseTo(Math.max(1, 2 * 0.38) * 0.68, 5);
    expect(contactShadowRadius(2, 1)).toBeGreaterThan(Math.max(2, 1 * 0.35) * 0.55);
  });

  it("uses a denser center falloff so the pet does not float", () => {
    expect(CONTACT_SHADOW_STOPS[0]?.[0]).toBe(0);
    // Dense core (alpha ~0.9) + soft multi-stop falloff for grounded lifeform feet.
    expect(CONTACT_SHADOW_STOPS[0]?.[1]).toMatch(/0\.9[0-9]?/);
    expect(CONTACT_SHADOW_STOPS.length).toBeGreaterThanOrEqual(4);
    expect(CONTACT_SHADOW_STOPS.at(-1)?.[0]).toBe(1);
    expect(CONTACT_SHADOW_STOPS.at(-1)?.[1]).toMatch(/rgba\([^)]*,\s*0\)/);
  });
});
