import { describe, expect, it } from "vitest";
import {
  isDistressExpression,
  NEUTRAL_VITALITY,
  vitalityStateFromCompanionStatus,
  workerVitalityReaction,
} from "./petWorkerVitality";

describe("workerVitalityReaction", () => {
  it("breaks a sweat and tires while a worker is busy", () => {
    const reaction = workerVitalityReaction("busy");
    expect(reaction.expression).toBe("sweat");
    expect(reaction.sustained).toBe(true);
    // Busy exerts the pet (drains energy); the cue is held while busy.
    expect(reaction.exertion).toBeGreaterThan(0);
  });

  it("bounces with delight on success (transient)", () => {
    const reaction = workerVitalityReaction("succeeded");
    expect(reaction.expression).toBe("bounce");
    expect(reaction.sustained).toBe(false);
    expect(reaction.exertion).toBe(0);
  });

  it("puffs dizzy smoke on failure (the crash tell)", () => {
    const reaction = workerVitalityReaction("failed");
    expect(reaction.expression).toBe("smoke");
    expect(reaction.sustained).toBe(false);
  });

  it("dozes off (snore) when a worker times out", () => {
    const reaction = workerVitalityReaction("timed_out");
    expect(reaction.expression).toBe("snore");
    expect(reaction.sustained).toBe(true);
  });

  it("is neutral when idle", () => {
    expect(workerVitalityReaction("idle")).toEqual(NEUTRAL_VITALITY);
  });

  it("falls back to neutral for an unknown state rather than throwing", () => {
    expect(workerVitalityReaction("some_future_state")).toEqual(NEUTRAL_VITALITY);
    expect(workerVitalityReaction("")).toEqual(NEUTRAL_VITALITY);
  });

  it("only drains energy while busy", () => {
    for (const state of ["idle", "succeeded", "failed", "timed_out"]) {
      expect(workerVitalityReaction(state).exertion).toBe(0);
    }
  });
});

describe("isDistressExpression", () => {
  it("treats smoke (a crash) as distress", () => {
    expect(isDistressExpression("smoke")).toBe(true);
  });

  it("does not treat calm or happy cues as distress", () => {
    expect(isDistressExpression("none")).toBe(false);
    expect(isDistressExpression("sweat")).toBe(false);
    expect(isDistressExpression("bounce")).toBe(false);
    expect(isDistressExpression("snore")).toBe(false);
  });
});

describe("vitalityStateFromCompanionStatus", () => {
  it("maps thinking and running to busy", () => {
    expect(vitalityStateFromCompanionStatus("thinking")).toBe("busy");
    expect(vitalityStateFromCompanionStatus("running")).toBe("busy");
  });

  it("maps completed to succeeded and failed to failed", () => {
    expect(vitalityStateFromCompanionStatus("completed")).toBe("succeeded");
    expect(vitalityStateFromCompanionStatus("failed")).toBe("failed");
  });

  it("maps quiet statuses and unknowns to idle", () => {
    expect(vitalityStateFromCompanionStatus("waiting_for_confirmation")).toBe("idle");
    expect(vitalityStateFromCompanionStatus("cancelled")).toBe("idle");
    expect(vitalityStateFromCompanionStatus("future_status")).toBe("idle");
  });
});
