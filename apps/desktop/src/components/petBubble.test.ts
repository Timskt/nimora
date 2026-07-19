import { describe, expect, it } from "vitest";
import { canPresentPetBubble, PET_BUBBLE_DURATION_MS } from "./petBubble";

describe("pet bubble presentation", () => {
  it("allows a short local expression while the pet is idle", () => {
    expect(canPresentPetBubble({ menuOpen: false, pointerActive: false })).toBe(true);
    expect(PET_BUBBLE_DURATION_MS).toBeGreaterThanOrEqual(3000);
    expect(PET_BUBBLE_DURATION_MS).toBeLessThanOrEqual(5000);
  });

  it("stays out of the way during menus and pointer gestures", () => {
    expect(canPresentPetBubble({ menuOpen: true, pointerActive: false })).toBe(false);
    expect(canPresentPetBubble({ menuOpen: false, pointerActive: true })).toBe(false);
  });
});
