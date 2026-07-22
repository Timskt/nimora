import { describe, expect, it } from "vitest";
import {
  awarenessFromReason,
  isBusyEnvironment,
  NEUTRAL_AWARENESS,
} from "./petAwareness";

describe("awarenessFromReason", () => {
  it("stays neutral and available on a plain desktop", () => {
    expect(awarenessFromReason("base_policy")).toEqual(NEUTRAL_AWARENESS);
    expect(awarenessFromReason("user_forced_visible")).toEqual(NEUTRAL_AWARENESS);
  });

  it("quiets down and focuses when the foreground is fullscreen", () => {
    const reaction = awarenessFromReason("fullscreen");
    expect(reaction.quiet).toBe(true);
    expect(reaction.mood).toBe("focused");
  });

  it("turns playful but quiet during a game", () => {
    const reaction = awarenessFromReason("game");
    expect(reaction.quiet).toBe(true);
    expect(reaction.mood).toBe("playful");
  });

  it("goes shy and quiet under do-not-disturb", () => {
    const reaction = awarenessFromReason("do_not_disturb");
    expect(reaction.quiet).toBe(true);
    expect(reaction.mood).toBe("shy");
  });

  it("stays calm and out of the way during screen share", () => {
    const reaction = awarenessFromReason("screen_share_privacy");
    expect(reaction.quiet).toBe(true);
    expect(reaction.mood).toBe("calm");
  });

  it("is quiet but neutral when suppressed or recovering", () => {
    expect(awarenessFromReason("user_forced_hidden").quiet).toBe(true);
    expect(awarenessFromReason("safe_mode_recovery").quiet).toBe(true);
  });

  it("falls back to neutral for an unknown reason rather than throwing", () => {
    expect(awarenessFromReason("some_future_reason")).toEqual(NEUTRAL_AWARENESS);
    expect(awarenessFromReason("")).toEqual(NEUTRAL_AWARENESS);
  });

  it("never leaks content — cues are generic and short", () => {
    for (const reason of [
      "fullscreen",
      "game",
      "do_not_disturb",
      "screen_share_privacy",
    ]) {
      const { cue } = awarenessFromReason(reason);
      // A cue must be a short, generic phrase, not a title or path.
      expect(cue.length).toBeLessThanOrEqual(20);
      expect(cue).not.toMatch(/[/\\]/); // no file paths
    }
  });
});

describe("isBusyEnvironment", () => {
  it("reports busy for immersive or private contexts", () => {
    expect(isBusyEnvironment("fullscreen")).toBe(true);
    expect(isBusyEnvironment("game")).toBe(true);
    expect(isBusyEnvironment("do_not_disturb")).toBe(true);
    expect(isBusyEnvironment("screen_share_privacy")).toBe(true);
  });

  it("reports not busy on a plain desktop", () => {
    expect(isBusyEnvironment("base_policy")).toBe(false);
    expect(isBusyEnvironment("user_forced_visible")).toBe(false);
  });
});
