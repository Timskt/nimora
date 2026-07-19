import { describe, expect, it } from "vitest";
import { isPetView, petBodyClasses } from "./petView";

describe("pet view shell", () => {
  it("selects only the explicit pet route", () => {
    expect(isPetView("?view=pet")).toBe(true);
    expect(isPetView("?view=control-center")).toBe(false);
    expect(isPetView("?preview=pet")).toBe(false);
  });

  it("keeps Tauri transparent and gives browsers an exact-size preview frame", () => {
    expect(petBodyClasses(true, true)).toEqual(["pet-window"]);
    expect(petBodyClasses(true, false)).toEqual(["pet-window", "pet-browser-preview"]);
    expect(petBodyClasses(false, false)).toEqual([]);
  });
});
