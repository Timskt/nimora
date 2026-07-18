import { describe, expect, it } from "vitest";
import { exceedsPetDragThreshold, PET_DRAG_THRESHOLD_PX } from "./petGesture";

describe("pet gesture arbitration", () => {
  it("keeps small pointer jitter as a click", () => {
    expect(exceedsPetDragThreshold({ clientX: 100, clientY: 100 }, 103, 104)).toBe(false);
  });

  it("starts dragging at the inclusive movement threshold", () => {
    expect(exceedsPetDragThreshold({ clientX: 100, clientY: 100 }, 100 + PET_DRAG_THRESHOLD_PX, 100)).toBe(true);
    expect(exceedsPetDragThreshold({ clientX: 100, clientY: 100 }, 106, 108)).toBe(true);
  });
});
