import { describe, expect, it } from "vitest";
import { eventSchema, petSchema } from "./index";

describe("eventSchema", () => {
  it("accepts the versioned event wire format", () => {
    const result = eventSchema.safeParse({
      spec: "asterpet.event/1",
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
      spec: "asterpet.event/1",
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

