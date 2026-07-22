import { describe, expect, it } from "vitest";
import {
  collisionResponse,
  DEFAULT_COLLISION_CONFIG,
  NEUTRAL_COLLISION,
  surfaceToWall,
  type CollisionConfig,
  type CollisionImpact,
} from "./petCollision";

const CONFIG: CollisionConfig = DEFAULT_COLLISION_CONFIG;
const HARD_LEFT: CollisionImpact = { wall: "left", speed: 1 };

describe("collisionResponse", () => {
  it("is neutral and done for a zero-speed impact", () => {
    expect(collisionResponse({ wall: "left", speed: 0 }, 0, CONFIG)).toEqual(NEUTRAL_COLLISION);
  });

  it("is neutral for a non-finite speed", () => {
    expect(collisionResponse({ wall: "left", speed: Number.NaN }, 0, CONFIG)).toEqual(
      NEUTRAL_COLLISION,
    );
  });

  it("squashes against the wall during the squash phase", () => {
    // Mid-squash on a horizontal (left) wall -> wide and short: sx > 1 > sy.
    const mid = collisionResponse(HARD_LEFT, CONFIG.squashDurationMs, CONFIG);
    expect(mid.squash.sx).toBeGreaterThan(1);
    expect(mid.squash.sy).toBeLessThan(1);
    expect(mid.done).toBe(false);
    // Recoil has barely begun during the squash phase.
    expect(mid.recoilX).toBe(0);
  });

  it("recoils away from the wall after the squash phase", () => {
    // Hitting the left wall should push the pet to the right (+x).
    const recoilTime = CONFIG.squashDurationMs + CONFIG.recoilDurationMs / 2;
    const response = collisionResponse(HARD_LEFT, recoilTime, CONFIG);
    expect(response.recoilX).toBeGreaterThan(0);
    expect(response.recoilY).toBe(0);
  });

  it("recoils in the correct direction for each wall", () => {
    const t = CONFIG.squashDurationMs + CONFIG.recoilDurationMs / 2;
    expect(collisionResponse({ wall: "left", speed: 1 }, t, CONFIG).recoilX).toBeGreaterThan(0);
    expect(collisionResponse({ wall: "right", speed: 1 }, t, CONFIG).recoilX).toBeLessThan(0);
    expect(collisionResponse({ wall: "top", speed: 1 }, t, CONFIG).recoilY).toBeGreaterThan(0);
    expect(collisionResponse({ wall: "bottom", speed: 1 }, t, CONFIG).recoilY).toBeLessThan(0);
  });

  it("squashes tall-and-thin for a floor/ceiling hit", () => {
    const mid = collisionResponse({ wall: "bottom", speed: 1 }, CONFIG.squashDurationMs, CONFIG);
    // Vertical axis grows on a floor/ceiling collision.
    expect(mid.squash.sy).toBeGreaterThan(1);
    expect(mid.squash.sx).toBeLessThan(1);
  });

  it("gets dizzy on a hard hit but not a gentle one", () => {
    expect(collisionResponse({ wall: "left", speed: 1 }, 0, CONFIG).dizzy).toBe(true);
    expect(collisionResponse({ wall: "left", speed: 0.2 }, 0, CONFIG).dizzy).toBe(false);
  });

  it("stops being dizzy after the dizzy duration elapses", () => {
    const before = collisionResponse(HARD_LEFT, CONFIG.dizzyDurationMs - 1, CONFIG);
    const after = collisionResponse(HARD_LEFT, CONFIG.dizzyDurationMs, CONFIG);
    expect(before.dizzy).toBe(true);
    expect(after.dizzy).toBe(false);
  });

  it("settles to rest once both phases and dizziness have elapsed", () => {
    const totalMs = CONFIG.squashDurationMs + CONFIG.recoilDurationMs;
    const settleTime = Math.max(totalMs, CONFIG.dizzyDurationMs);
    const response = collisionResponse(HARD_LEFT, settleTime, CONFIG);
    expect(response.squash).toEqual({ sx: 1, sy: 1 });
    expect(response.recoilX).toBe(0);
    expect(response.recoilY).toBe(0);
    expect(response.done).toBe(true);
  });

  it("can still be dizzy after motion has settled, without being done", () => {
    // Motion phases are shorter than the dizzy window with the default config.
    const totalMs = CONFIG.squashDurationMs + CONFIG.recoilDurationMs;
    expect(totalMs).toBeLessThan(CONFIG.dizzyDurationMs);
    const response = collisionResponse(HARD_LEFT, totalMs, CONFIG);
    expect(response.recoilX).toBe(0);
    expect(response.dizzy).toBe(true);
    expect(response.done).toBe(false);
  });

  it("scales the response with impact speed", () => {
    const t = CONFIG.squashDurationMs + CONFIG.recoilDurationMs / 2;
    const soft = collisionResponse({ wall: "left", speed: 0.3 }, t, CONFIG);
    const hard = collisionResponse({ wall: "left", speed: 1 }, t, CONFIG);
    expect(hard.recoilX).toBeGreaterThan(soft.recoilX);
  });

  it("preserves volume in the squash at every instant", () => {
    for (const time of [0, 55, 110, 200, 300]) {
      const { squash } = collisionResponse(HARD_LEFT, time, CONFIG);
      expect(squash.sx * squash.sy).toBeCloseTo(1, 6);
    }
  });
});

describe("surfaceToWall", () => {
  it("maps each edge surface to its wall", () => {
    expect(surfaceToWall("left")).toBe("left");
    expect(surfaceToWall("right")).toBe("right");
    expect(surfaceToWall("top")).toBe("top");
    expect(surfaceToWall("bottom")).toBe("bottom");
  });

  it("resolves corner surfaces to their vertical edge", () => {
    expect(surfaceToWall("top_left")).toBe("top");
    expect(surfaceToWall("top_right")).toBe("top");
    expect(surfaceToWall("bottom_left")).toBe("bottom");
    expect(surfaceToWall("bottom_right")).toBe("bottom");
  });

  it("returns null for free space and unknown values", () => {
    expect(surfaceToWall("free")).toBeNull();
    expect(surfaceToWall("nonsense")).toBeNull();
  });
});
