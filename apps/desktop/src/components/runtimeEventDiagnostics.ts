import type { NimoraEvent } from "@nimora/schemas";

export type EventSourceCategory =
  | "core"
  | "skill"
  | "automation"
  | "agent"
  | "connector"
  | "gateway"
  | "system";

export const RUNTIME_EVENT_BUFFER_CAP = 200;

export function eventSourceCategory(source: NimoraEvent["source"]): EventSourceCategory {
  if (source === "core") return "core";
  const prefix = source.split(":", 1)[0];
  if (
    prefix === "skill" ||
    prefix === "automation" ||
    prefix === "agent" ||
    prefix === "connector" ||
    prefix === "gateway" ||
    prefix === "system"
  ) {
    return prefix;
  }
  return "system";
}

export function eventSourceLabel(category: EventSourceCategory): string {
  switch (category) {
    case "core":
      return "内核";
    case "skill":
      return "技能";
    case "automation":
      return "自动化";
    case "agent":
      return "Agent";
    case "connector":
      return "连接器";
    case "gateway":
      return "能力网关";
    case "system":
      return "系统";
  }
}

/**
 * Accumulate destructively-drained events into a bounded, most-recent-first buffer.
 * De-duplicates by event id (the runtime bus never re-emits a drained event, but a
 * defensive de-dupe keeps the panel stable if a caller double-drains).
 */
export function mergeDrainedEvents(
  buffer: NimoraEvent[],
  drained: NimoraEvent[],
  cap: number = RUNTIME_EVENT_BUFFER_CAP,
): NimoraEvent[] {
  if (drained.length === 0) return buffer;
  const seen = new Set(buffer.map((event) => event.id));
  const fresh = drained.filter((event) => !seen.has(event.id));
  if (fresh.length === 0) return buffer;
  const ordered = [...fresh].reverse();
  return [...ordered, ...buffer].slice(0, Math.max(0, cap));
}

export function describeDrainNotice(drainedCount: number, freshCount: number): string {
  if (drainedCount === 0) return "运行时事件队列为空，没有可诊断的新事件";
  if (freshCount === 0) return `排空了 ${drainedCount} 条事件，均已在诊断流中`;
  return `排空了 ${drainedCount} 条运行时事件，新增 ${freshCount} 条到诊断流`;
}

export function formatEventTimestamp(timestamp: string): string {
  const parsed = new Date(timestamp);
  if (Number.isNaN(parsed.getTime())) return timestamp;
  return parsed.toLocaleTimeString("zh-CN", { hour12: false });
}

export function shortTraceId(traceId: string): string {
  return traceId.length <= 8 ? traceId : `${traceId.slice(0, 8)}…`;
}

export interface EventSourceTally {
  category: EventSourceCategory;
  label: string;
  count: number;
}

export function tallyEventSources(buffer: NimoraEvent[]): EventSourceTally[] {
  const counts = new Map<EventSourceCategory, number>();
  for (const event of buffer) {
    const category = eventSourceCategory(event.source);
    counts.set(category, (counts.get(category) ?? 0) + 1);
  }
  return [...counts.entries()]
    .map(([category, count]) => ({ category, label: eventSourceLabel(category), count }))
    .sort((a, b) => b.count - a.count || a.category.localeCompare(b.category));
}
