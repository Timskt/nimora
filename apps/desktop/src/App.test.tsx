import { describe, expect, it } from "vitest";
import { navItemClassName } from "./App";

describe("navItemClassName", () => {
  it("adds the active state only to the selected destination", () => {
    expect(navItemClassName(true)).toBe("nav-item active");
    expect(navItemClassName(false)).toBe("nav-item");
  });
});
