import { describe, expect, it } from "vitest";
import {
  createFollower,
  followerAtRest,
  stepFollower,
  type FollowerConfig,
  type FollowerPoint,
} from "./petSecondaryMotion";

const CONFIG: FollowerConfig = { stiffness: 0.3, damping: 0.7, maxLeash: 30 };

function settle(anchor: FollowerPoint, frames: number, config = CONFIG) {
  let state = createFollower({ x: 0, y: 0 });
  for (let i = 0; i < frames; i += 1) {
    state = stepFollower(state, anchor, config);
  }
  return state;
}

describe("createFollower", () => {
  it("starts at rest at the origin with zero implied velocity", () => {
    const state = createFollower({ x: 5, y: 7 });
    expect(state.position).toEqual({ x: 5, y: 7 });
    expect(state.previous).toEqual({ x: 5, y: 7 });
    expect(followerAtRest(state, { x: 5, y: 7 }, 0.001)).toBe(true);
  });

  it("snaps a non-finite origin to a safe zero point", () => {
    const state = createFollower({ x: Number.NaN, y: 3 });
    expect(state.position).toEqual({ x: 0, y: 0 });
  });
});

describe("stepFollower", () => {
  it("lags behind the anchor on the first frame rather than snapping to it", () => {
    const state = stepFollower(createFollower({ x: 0, y: 0 }), { x: 100, y: 0 }, CONFIG);
    // With stiffness 0.3 the follower has only closed part of the gap.
    expect(state.position.x).toBeGreaterThan(0);
    expect(state.position.x).toBeLessThan(100);
  });

  it("eventually converges on a stationary anchor", () => {
    const state = settle({ x: 40, y: 20 }, 400);
    expect(state.position.x).toBeCloseTo(40, 1);
    expect(state.position.y).toBeCloseTo(20, 1);
    expect(followerAtRest(state, { x: 40, y: 20 }, 0.05)).toBe(true);
  });

  it("overshoots a moving anchor when damping keeps momentum", () => {
    // Drive the anchor out, then stop it dead; a springy follower should swing
    // past the stop point at least once before settling.
    let state = createFollower({ x: 0, y: 0 });
    const springy: FollowerConfig = { stiffness: 0.2, damping: 0.9, maxLeash: 500 };
    for (let i = 0; i < 20; i += 1) {
      state = stepFollower(state, { x: 100, y: 0 }, springy);
    }
    let overshot = false;
    for (let i = 0; i < 200; i += 1) {
      state = stepFollower(state, { x: 100, y: 0 }, springy);
      if (state.position.x > 100.5) overshot = true;
    }
    expect(overshot).toBe(true);
  });

  it("never sits farther than the leash from the anchor", () => {
    // A violent jump must not fling the appendage beyond the leash.
    let state = createFollower({ x: 0, y: 0 });
    for (let i = 0; i < 50; i += 1) {
      const anchor = { x: i % 2 === 0 ? 1000 : -1000, y: 0 };
      state = stepFollower(state, anchor, CONFIG);
      const distance = Math.hypot(state.position.x - anchor.x, state.position.y - anchor.y);
      expect(distance).toBeLessThanOrEqual(CONFIG.maxLeash + 1e-6);
    }
  });

  it("treats a non-finite anchor as a no-op", () => {
    const start = createFollower({ x: 3, y: 4 });
    const next = stepFollower(start, { x: Number.NaN, y: 0 }, CONFIG);
    expect(next).toBe(start);
  });

  it("recovers from a corrupted state by re-seeding at the anchor", () => {
    const corrupt = { position: { x: Number.NaN, y: 0 }, previous: { x: 0, y: 0 } };
    const next = stepFollower(corrupt, { x: 10, y: 10 }, CONFIG);
    expect(next.position).toEqual({ x: 10, y: 10 });
  });

  it("clamps out-of-range stiffness and damping instead of exploding", () => {
    const wild: FollowerConfig = { stiffness: 5, damping: 9, maxLeash: 50 };
    const state = settle({ x: 25, y: 0 }, 100, wild);
    expect(Number.isFinite(state.position.x)).toBe(true);
    expect(Number.isFinite(state.position.y)).toBe(true);
  });
});
