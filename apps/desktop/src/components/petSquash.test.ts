import { describe, expect, it } from "vitest";
import {
  computeSquash,
  NEUTRAL_SQUASH,
  squashTransform,
  type SquashInput,
} from "./petSquash";

const BASE: SquashInput = { intensity: 1, axis: "vertical", maxStrain: 0.25 };

describe("computeSquash", () => {
  it("returns a neutral scale at zero intensity", () => {
    expect(computeSquash({ ...BASE, intensity: 0 })).toEqual(NEUTRAL_SQUASH);
  });

  it("returns a neutral scale when no strain is allowed", () => {
    expect(computeSquash({ ...BASE, maxStrain: 0 })).toEqual(NEUTRAL_SQUASH);
  });

  it("stretches tall and thin on the vertical axis", () => {
    const scale = computeSquash({ ...BASE, axis: "vertical" });
    expect(scale.sy).toBeGreaterThan(1);
    expect(scale.sx).toBeLessThan(1);
  });

  it("squashes wide and short on the horizontal axis", () => {
    const scale = computeSquash({ ...BASE, axis: "horizontal" });
    expect(scale.sx).toBeGreaterThan(1);
    expect(scale.sy).toBeLessThan(1);
  });

  it("preserves apparent volume: sx * sy === 1", () => {
    for (const axis of ["vertical", "horizontal"] as const) {
      for (const intensity of [0.1, 0.5, 1]) {
        const scale = computeSquash({ intensity, axis, maxStrain: 0.4 });
        expect(scale.sx * scale.sy).toBeCloseTo(1, 10);
      }
    }
  });

  it("scales deflection with intensity", () => {
    const half = computeSquash({ ...BASE, intensity: 0.5 });
    const full = computeSquash({ ...BASE, intensity: 1 });
    expect(full.sy).toBeGreaterThan(half.sy);
  });

  it("clamps intensity above one to the maximum deflection", () => {
    const full = computeSquash({ ...BASE, intensity: 1 });
    const over = computeSquash({ ...BASE, intensity: 5 });
    expect(over).toEqual(full);
  });

  it("clamps strain to a ceiling that can never invert the body", () => {
    const extreme = computeSquash({ intensity: 1, axis: "vertical", maxStrain: 100 });
    // Even at absurd strain the secondary axis stays positive (no inversion).
    expect(extreme.sx).toBeGreaterThan(0);
    expect(extreme.sy).toBeGreaterThan(1);
  });

  it("collapses non-finite inputs to a neutral scale", () => {
    expect(computeSquash({ ...BASE, intensity: Number.NaN })).toEqual(NEUTRAL_SQUASH);
    expect(computeSquash({ ...BASE, maxStrain: Number.POSITIVE_INFINITY }).sy).toBeGreaterThan(1);
  });
});

describe("squashTransform", () => {
  it("formats a neutral scale as an identity transform", () => {
    expect(squashTransform(NEUTRAL_SQUASH)).toBe("scale(1, 1)");
  });

  it("rounds to four decimals for stable output", () => {
    const transform = squashTransform({ sx: 1.234567, sy: 0.810005 });
    expect(transform).toBe("scale(1.2346, 0.81)");
  });
});
