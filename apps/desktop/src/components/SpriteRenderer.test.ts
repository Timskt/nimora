import { describe, expect, it } from "vitest";
import type { SpriteClips } from "@nimora/schemas";
import { assetImageUrl, nextFrameIndex, petStateAction, resolveSpriteAction } from "./SpriteRenderer";

const clips: SpriteClips["clips"] = {
  "pet.idle": { loop: true, frames: [{ file: "idle.webp", durationMs: 100 }] },
  "pet.walk": { loop: true, frames: [{ file: "walk.webp", durationMs: 80 }] },
};

describe("SpriteRenderer helpers", () => {
  it("maps every runtime state to its canonical sprite action", () => {
    expect(petStateAction("idle")).toBe("pet.idle");
    expect(petStateAction("observing")).toBe("pet.observe");
    expect(petStateAction("walking")).toBe("pet.walk");
    expect(petStateAction("perching")).toBe("pet.perch");
    expect(petStateAction("climbing")).toBe("pet.climb");
    expect(petStateAction("sleeping")).toBe("pet.sleep");
    expect(petStateAction("dragged")).toBe("pet.drag");
    expect(petStateAction("interacting")).toBe("pet.click");
    expect(petStateAction("working")).toBe("pet.work");
    expect(petStateAction("recovering")).toBe("pet.idle");
    expect(petStateAction("unknown")).toBe("pet.idle");
  });

  it("resolves direct actions and fallback chains", () => {
    expect(resolveSpriteAction("pet.walk", clips, {})).toBe("pet.walk");
    expect(resolveSpriteAction("pet.run", clips, {
      "pet.run": "pet.jog",
      "pet.jog": "pet.walk",
    })).toBe("pet.walk");
    expect(resolveSpriteAction("pet.observe", clips, {})).toBe("pet.idle");
  });

  it("stops fallback cycles and returns idle for unknown actions", () => {
    expect(resolveSpriteAction("pet.run", clips, {
      "pet.run": "pet.jog",
      "pet.jog": "pet.run",
    })).toBe("pet.idle");
    expect(resolveSpriteAction("pet.missing", clips, {})).toBe("pet.idle");
  });

  it("encodes each asset path segment without hiding separators", () => {
    expect(assetImageUrl(
      "nimora-asset://localhost/character.example.mochi/",
      "角色 图/#1?100%.webp",
    )).toBe(
      "nimora-asset://localhost/character.example.mochi/%E8%A7%92%E8%89%B2%20%E5%9B%BE/%231%3F100%25.webp",
    );
  });

  it("advances, loops, stops, and normalizes invalid frame indexes", () => {
    expect(nextFrameIndex(0, 3, true)).toBe(1);
    expect(nextFrameIndex(2, 3, true)).toBe(0);
    expect(nextFrameIndex(2, 3, false)).toBe(2);
    expect(nextFrameIndex(-1, 3, true)).toBe(0);
    expect(nextFrameIndex(3, 3, true)).toBe(0);
    expect(nextFrameIndex(0, 0, true)).toBe(0);
  });
});
