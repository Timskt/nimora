import { describe, expect, it } from "vitest";
import type { NimoraEvent } from "@nimora/schemas";
import {
  RUNTIME_EVENT_BUFFER_CAP,
  describeDrainNotice,
  eventSourceCategory,
  eventSourceLabel,
  formatEventTimestamp,
  mergeDrainedEvents,
  shortTraceId,
  tallyEventSources,
} from "./runtimeEventDiagnostics";

function event(id: string, source: NimoraEvent["source"], eventType = "pet.mood.changed"): NimoraEvent {
  return {
    spec: "nimora.event/1",
    id,
    eventType,
    source,
    timestamp: "2026-07-26T08:00:00.000+08:00",
    traceId: "0123456789abcdef",
    data: { secret: "never-shown" },
  };
}

describe("eventSourceCategory", () => {
  it("maps core and namespaced sources to categories", () => {
    expect(eventSourceCategory("core")).toBe("core");
    expect(eventSourceCategory("skill:preview")).toBe("skill");
    expect(eventSourceCategory("automation:build")).toBe("automation");
    expect(eventSourceCategory("agent:reasoner")).toBe("agent");
    expect(eventSourceCategory("connector:fs")).toBe("connector");
    expect(eventSourceCategory("gateway:capability")).toBe("gateway");
    expect(eventSourceCategory("system:power")).toBe("system");
  });
});

describe("eventSourceLabel", () => {
  it("returns Chinese labels for every category", () => {
    expect(eventSourceLabel("core")).toBe("内核");
    expect(eventSourceLabel("gateway")).toBe("能力网关");
    expect(eventSourceLabel("agent")).toBe("Agent");
  });
});

describe("mergeDrainedEvents", () => {
  it("returns the same buffer when nothing was drained", () => {
    const buffer = [event("a", "core")];
    expect(mergeDrainedEvents(buffer, [])).toBe(buffer);
  });

  it("prepends fresh events most-recent-first", () => {
    const buffer = [event("a", "core")];
    const merged = mergeDrainedEvents(buffer, [event("b", "skill:x"), event("c", "agent:y")]);
    expect(merged.map((item) => item.id)).toEqual(["c", "b", "a"]);
  });

  it("de-dupes by id defensively", () => {
    const buffer = [event("a", "core")];
    const merged = mergeDrainedEvents(buffer, [event("a", "core"), event("b", "skill:x")]);
    expect(merged.map((item) => item.id)).toEqual(["b", "a"]);
  });

  it("caps the buffer at the most recent entries", () => {
    const buffer = [event("old", "core")];
    const drained = Array.from({ length: 5 }, (_, index) => event(`n${index}`, "core"));
    const merged = mergeDrainedEvents(buffer, drained, 3);
    expect(merged).toHaveLength(3);
    expect(merged[0]!.id).toBe("n4");
    expect(merged.some((item) => item.id === "old")).toBe(false);
  });

  it("uses a sane default cap", () => {
    expect(RUNTIME_EVENT_BUFFER_CAP).toBeGreaterThan(0);
  });
});

describe("describeDrainNotice", () => {
  it("reports an empty queue", () => {
    expect(describeDrainNotice(0, 0)).toContain("为空");
  });

  it("reports duplicates when nothing new was added", () => {
    expect(describeDrainNotice(3, 0)).toContain("均已在诊断流");
  });

  it("reports fresh additions", () => {
    expect(describeDrainNotice(3, 2)).toContain("新增 2 条");
  });
});

describe("formatEventTimestamp", () => {
  it("returns the raw value for an unparseable timestamp", () => {
    expect(formatEventTimestamp("not-a-date")).toBe("not-a-date");
  });
});

describe("shortTraceId", () => {
  it("truncates long trace ids", () => {
    expect(shortTraceId("0123456789abcdef")).toBe("01234567…");
  });

  it("keeps short trace ids intact", () => {
    expect(shortTraceId("012345")).toBe("012345");
  });
});

describe("tallyEventSources", () => {
  it("counts categories and orders by count then category", () => {
    const tallies = tallyEventSources([
      event("a", "core"),
      event("b", "core"),
      event("c", "agent:x"),
    ]);
    expect(tallies[0]).toEqual({ category: "core", label: "内核", count: 2 });
    expect(tallies[1]).toEqual({ category: "agent", label: "Agent", count: 1 });
  });
});
