import { describe, expect, it } from "vitest";
import { computeGaze, NEUTRAL_GAZE, type GazeInput } from "./petGaze";

const BASE: GazeInput = {
  pointer: { x: 100, y: 100 },
  eyeCenter: { x: 100, y: 100 },
  maxOffset: { dx: 4, dy: 6 },
  saturationDistance: 200,
};

describe("computeGaze", () => {
  it("returns a neutral gaze when the pointer is exactly on the eye center", () => {
    expect(computeGaze(BASE)).toEqual(NEUTRAL_GAZE);
  });

  it("looks toward the pointer within the elliptical socket", () => {
    const gaze = computeGaze({ ...BASE, pointer: { x: 300, y: 300 } });
    // Down-right pointer -> positive dx and dy.
    expect(gaze.dx).toBeGreaterThan(0);
    expect(gaze.dy).toBeGreaterThan(0);
    // Never leaves the socket.
    expect(Math.abs(gaze.dx)).toBeLessThanOrEqual(4);
    expect(Math.abs(gaze.dy)).toBeLessThanOrEqual(6);
  });

  it("reverses direction for a pointer up and to the left", () => {
    const gaze = computeGaze({ ...BASE, pointer: { x: 0, y: 0 } });
    expect(gaze.dx).toBeLessThan(0);
    expect(gaze.dy).toBeLessThan(0);
  });

  it("saturates at the socket edge for a distant pointer", () => {
    // A pointer far beyond saturationDistance along +x reaches full dx.
    const gaze = computeGaze({ ...BASE, pointer: { x: 5000, y: 100 } });
    expect(gaze.dx).toBeCloseTo(4, 5);
    expect(gaze.dy).toBeCloseTo(0, 5);
  });

  it("deflects proportionally less for a nearby pointer", () => {
    // Halfway to saturation along +x -> roughly half deflection.
    const gaze = computeGaze({ ...BASE, pointer: { x: 200, y: 100 } });
    expect(gaze.dx).toBeCloseTo(2, 5);
  });

  it("never exceeds the socket even when intensity is at maximum on a diagonal", () => {
    const gaze = computeGaze({ ...BASE, pointer: { x: 9000, y: 9000 } });
    expect(Math.abs(gaze.dx)).toBeLessThanOrEqual(4);
    expect(Math.abs(gaze.dy)).toBeLessThanOrEqual(6);
  });

  it("falls back to a neutral gaze for non-finite inputs", () => {
    expect(computeGaze({ ...BASE, pointer: { x: Number.NaN, y: 100 } })).toEqual(NEUTRAL_GAZE);
    expect(computeGaze({ ...BASE, pointer: { x: Number.POSITIVE_INFINITY, y: 100 } })).toEqual(
      NEUTRAL_GAZE,
    );
    expect(computeGaze({ ...BASE, eyeCenter: { x: Number.NaN, y: 100 } })).toEqual(NEUTRAL_GAZE);
  });

  it("falls back to a neutral gaze for a non-positive saturation distance", () => {
    expect(computeGaze({ ...BASE, saturationDistance: 0 })).toEqual(NEUTRAL_GAZE);
    expect(computeGaze({ ...BASE, saturationDistance: -50 })).toEqual(NEUTRAL_GAZE);
  });
});
