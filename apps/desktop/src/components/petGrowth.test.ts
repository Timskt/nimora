import { describe, expect, it } from "vitest";
import { petGrowth } from "./petGrowth";

describe("petGrowth", () => {
  it.each([
    [0, 1, 0],
    [49, 1, 49],
    [50, 2, 0],
    [84, 2, 34],
  ])("maps %i bond points to level %i", (bondPoints, level, progress) => {
    expect(petGrowth(bondPoints, 0)).toMatchObject({ bondPoints, level, levelProgress: progress });
  });

  it("uses legacy affinity as the effective migration baseline", () => {
    expect(petGrowth(undefined, 34)).toMatchObject({ bondPoints: 34, level: 1, levelProgress: 34 });
    expect(petGrowth(0, 100)).toMatchObject({ bondPoints: 100, level: 3, levelProgress: 0 });
  });

  it("rejects invalid persisted values at the display boundary", () => {
    expect(petGrowth(Number.NaN, 20)).toMatchObject({ bondPoints: 20, level: 1 });
    expect(petGrowth(-1, 20)).toMatchObject({ bondPoints: 20, level: 1 });
  });
});
