import { describe, expect, it } from "vitest";
import { eventSchema, petSchema, profileSnapshotSchema, safetySnapshotSchema } from "./index";

describe("eventSchema", () => {
  it("accepts the versioned event wire format", () => {
    const result = eventSchema.safeParse({
      spec: "nimora.event/1",
      id: "019bf2c6-4d40-7000-8000-000000000001",
      eventType: "pet.state.changed",
      source: "core",
      timestamp: "2026-07-17T01:00:00Z",
      traceId: "019bf2c6-4d40-7000-8000-000000000002",
      data: { state: "idle" },
    });
    expect(result.success).toBe(true);
  });

  it("rejects unqualified event types", () => {
    const result = eventSchema.safeParse({
      spec: "nimora.event/1",
      id: "019bf2c6-4d40-7000-8000-000000000001",
      eventType: "clicked",
      source: "core",
      timestamp: "2026-07-17T01:00:00Z",
      traceId: "019bf2c6-4d40-7000-8000-000000000002",
      data: null,
    });
    expect(result.success).toBe(false);
  });
});

describe("petSchema", () => {
  it("rejects vitals outside the domain range", () => {
    const result = petSchema.safeParse({
      id: "019bf2c6-4d40-7000-8000-000000000001",
      name: "Aster",
      state: "idle",
      emotion: "happy",
      position: { x: 0, y: 0 },
      energy: 101,
      mood: 70,
      affinity: 0,
    });
    expect(result.success).toBe(false);
  });
});

describe("profileSnapshotSchema", () => {
  it("accepts a versioned active profile collection", () => {
    const id = "019bf2c6-4d40-7000-8000-000000000010";
    expect(profileSnapshotSchema.safeParse({
      schemaVersion: 1,
      activeProfileId: id,
      profiles: [{
        id,
        name: "Focus",
        policy: {
          alwaysOnTop: true,
          clickThrough: false,
          soundEnabled: true,
          proactiveFrequency: 25,
        },
      }],
    }).success).toBe(true);
  });

  it("rejects policy values outside the domain range", () => {
    const id = "019bf2c6-4d40-7000-8000-000000000010";
    expect(profileSnapshotSchema.safeParse({
      schemaVersion: 1,
      activeProfileId: id,
      profiles: [{
        id,
        name: "Focus",
        policy: {
          alwaysOnTop: null,
          clickThrough: null,
          soundEnabled: null,
          proactiveFrequency: 101,
        },
      }],
    }).success).toBe(false);
  });
});

describe("safetySnapshotSchema", () => {
  it("keeps runtime mode and reason consistent", () => {
    expect(safetySnapshotSchema.safeParse({ mode: "safe", reason: "manual" }).success).toBe(true);
    expect(safetySnapshotSchema.safeParse({ mode: "normal", reason: null }).success).toBe(true);
    expect(safetySnapshotSchema.safeParse({ mode: "safe", reason: null }).success).toBe(false);
    expect(safetySnapshotSchema.safeParse({ mode: "normal", reason: "crash_loop" }).success).toBe(false);
  });
});
