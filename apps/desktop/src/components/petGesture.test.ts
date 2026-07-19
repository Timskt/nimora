import { describe, expect, it } from "vitest";
import {
  appendPetGesturePoint,
  createPetGestureTrail,
  exceedsPetDragThreshold,
  isPetStroke,
  petClickResolution,
  PET_DRAG_THRESHOLD_PX,
} from "./petGesture";

describe("pet gesture arbitration", () => {
  it("resolves the second click as a double interaction", () => {
    expect(petClickResolution(0)).toBe("single");
    expect(petClickResolution(1)).toBe("single");
    expect(petClickResolution(2)).toBe("double");
    expect(petClickResolution(3)).toBe("ignore");
    expect(petClickResolution(4)).toBe("ignore");
  });

  it("keeps small pointer jitter as a click", () => {
    expect(exceedsPetDragThreshold({ clientX: 100, clientY: 100 }, 103, 104)).toBe(false);
  });

  it("starts dragging at the inclusive movement threshold", () => {
    expect(exceedsPetDragThreshold({ clientX: 100, clientY: 100 }, 100 + PET_DRAG_THRESHOLD_PX, 100)).toBe(true);
    expect(exceedsPetDragThreshold({ clientX: 100, clientY: 100 }, 112, 116)).toBe(true);
  });

  it("recognizes a bounded deliberate back-and-forth stroke", () => {
    let trail = createPetGestureTrail({ clientX: 100, clientY: 100 }, 1_000);
    for (const clientX of [108, 100, 108, 100, 108, 102]) {
      trail = appendPetGesturePoint(trail, { clientX, clientY: 100 });
    }
    expect(isPetStroke(trail, 1_220)).toBe(true);
  });

  it("rejects fast, one-way, and outward paths as strokes", () => {
    let trail = createPetGestureTrail({ clientX: 100, clientY: 100 }, 1_000);
    for (const clientX of [108, 100, 108, 100, 108, 102]) {
      trail = appendPetGesturePoint(trail, { clientX, clientY: 100 });
    }
    expect(isPetStroke(trail, 1_100)).toBe(false);

    let oneWay = createPetGestureTrail({ clientX: 100, clientY: 100 }, 1_000);
    oneWay = appendPetGesturePoint(oneWay, { clientX: 111, clientY: 100 });
    expect(isPetStroke(oneWay, 1_300)).toBe(false);

    let outward = createPetGestureTrail({ clientX: 100, clientY: 100 }, 1_000);
    outward = appendPetGesturePoint(outward, { clientX: 112, clientY: 100 });
    outward = appendPetGesturePoint(outward, { clientX: 100, clientY: 100 });
    outward = appendPetGesturePoint(outward, { clientX: 112, clientY: 100 });
    expect(isPetStroke(outward, 1_300)).toBe(false);
  });
});
