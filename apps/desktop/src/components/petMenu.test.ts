import { describe, expect, it } from "vitest";
import { nextMenuItemIndex } from "./petMenu";

describe("nextMenuItemIndex", () => {
  it("wraps directional navigation in both directions", () => {
    expect(nextMenuItemIndex(5, 6, "ArrowRight")).toBe(0);
    expect(nextMenuItemIndex(0, 6, "ArrowLeft")).toBe(5);
    expect(nextMenuItemIndex(2, 6, "ArrowDown")).toBe(3);
    expect(nextMenuItemIndex(2, 6, "ArrowUp")).toBe(1);
  });

  it("supports menu boundary keys and ignores unrelated input", () => {
    expect(nextMenuItemIndex(3, 6, "Home")).toBe(0);
    expect(nextMenuItemIndex(3, 6, "End")).toBe(5);
    expect(nextMenuItemIndex(3, 6, "Enter")).toBeNull();
    expect(nextMenuItemIndex(0, 0, "ArrowRight")).toBeNull();
  });
});
