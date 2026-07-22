import { describe, expect, it } from "vitest";
import { DEFAULT_BLINK_CONFIG, nextBlink, type BlinkConfig } from "./petBlink";

const CONFIG: BlinkConfig = DEFAULT_BLINK_CONFIG;

describe("nextBlink", () => {
  it("blinks more often and more crisply when fully rested", () => {
    const rested = nextBlink(100, 0, CONFIG);
    expect(rested.delayMs).toBeCloseTo(CONFIG.minIntervalMs, 5);
    expect(rested.durationMs).toBeCloseTo(CONFIG.minDurationMs, 5);
  });

  it("blinks rarely and heavily when exhausted", () => {
    const tired = nextBlink(0, 0, CONFIG);
    expect(tired.delayMs).toBeCloseTo(CONFIG.maxIntervalMs, 5);
    expect(tired.durationMs).toBeCloseTo(CONFIG.maxDurationMs, 5);
  });

  it("slows the blink interval monotonically as energy falls", () => {
    const high = nextBlink(90, 0, CONFIG);
    const mid = nextBlink(50, 0, CONFIG);
    const low = nextBlink(10, 0, CONFIG);
    expect(mid.delayMs).toBeGreaterThan(high.delayMs);
    expect(low.delayMs).toBeGreaterThan(mid.delayMs);
  });

  it("lengthens the blink duration as energy falls (dry-eye tell)", () => {
    const high = nextBlink(90, 0, CONFIG);
    const low = nextBlink(10, 0, CONFIG);
    expect(low.durationMs).toBeGreaterThan(high.durationMs);
  });

  it("adds jitter scaled by the random roll so blinks are not metronomic", () => {
    const none = nextBlink(50, 0, CONFIG);
    const full = nextBlink(50, 1, CONFIG);
    expect(full.delayMs - none.delayMs).toBeCloseTo(CONFIG.jitterMs, 5);
  });

  it("clamps energy outside 0..100", () => {
    const over = nextBlink(500, 0, CONFIG);
    const under = nextBlink(-100, 0, CONFIG);
    expect(over.delayMs).toBeCloseTo(CONFIG.minIntervalMs, 5);
    expect(under.delayMs).toBeCloseTo(CONFIG.maxIntervalMs, 5);
  });

  it("treats non-finite energy as fully rested and a non-finite roll as zero jitter", () => {
    const schedule = nextBlink(Number.NaN, Number.NaN, CONFIG);
    expect(schedule.delayMs).toBeCloseTo(CONFIG.minIntervalMs, 5);
  });

  it("clamps a roll outside 0..1", () => {
    const high = nextBlink(50, 5, CONFIG);
    const low = nextBlink(50, -5, CONFIG);
    expect(high.delayMs).toBeCloseTo(nextBlink(50, 1, CONFIG).delayMs, 5);
    expect(low.delayMs).toBeCloseTo(nextBlink(50, 0, CONFIG).delayMs, 5);
  });
});
