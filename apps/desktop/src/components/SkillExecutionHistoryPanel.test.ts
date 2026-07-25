import { describe, expect, it } from "vitest";
import {
  formatSkillHistoryTime,
  isCancellable,
  skillStatusLabel,
  summarizeSkillHistoryRecord,
} from "./SkillExecutionHistoryPanel";
import type { SkillExecutionHistoryRecord } from "../platform/desktop";

function record(overrides: Partial<SkillExecutionHistoryRecord>): SkillExecutionHistoryRecord {
  return {
    spec: "nimora.skill-execution-history/1",
    executionId: "018f0000-0000-7000-8000-0000000000aa",
    skillId: "studio.example.greeter",
    status: "completed",
    commandCount: 2,
    agentTaskCount: 0,
    createdAtMs: 1_700_000_000_000,
    updatedAtMs: 1_700_000_000_500,
    error: null,
    ...overrides,
  };
}

describe("skillStatusLabel", () => {
  it("maps each known status to a Chinese label", () => {
    expect(skillStatusLabel("waitingForApproval")).toBe("等待批准");
    expect(skillStatusLabel("completed")).toBe("已完成");
    expect(skillStatusLabel("rejected")).toBe("已拒绝");
    expect(skillStatusLabel("cancelled")).toBe("已取消");
    expect(skillStatusLabel("failed")).toBe("已失败");
  });

  it("falls back to the raw status for unknown values", () => {
    expect(skillStatusLabel("mystery" as SkillExecutionHistoryRecord["status"])).toBe("mystery");
  });
});

describe("isCancellable", () => {
  it("only allows cancelling records awaiting approval", () => {
    expect(isCancellable(record({ status: "waitingForApproval" }))).toBe(true);
    expect(isCancellable(record({ status: "completed" }))).toBe(false);
    expect(isCancellable(record({ status: "failed" }))).toBe(false);
  });
});

describe("summarizeSkillHistoryRecord", () => {
  it("reports the command count on its own", () => {
    expect(summarizeSkillHistoryRecord(record({ commandCount: 3, agentTaskCount: 0 }))).toBe("3 命令");
  });

  it("adds agent task count when present", () => {
    const summary = summarizeSkillHistoryRecord(record({ commandCount: 1, agentTaskCount: 2 }));
    expect(summary).toContain("1 命令");
    expect(summary).toContain("2 Agent 任务");
  });

  it("appends the error message when the run failed", () => {
    const summary = summarizeSkillHistoryRecord(record({ status: "failed", error: "boom" }));
    expect(summary).toContain("错误：boom");
  });
});

describe("formatSkillHistoryTime", () => {
  it("labels missing or invalid timestamps", () => {
    expect(formatSkillHistoryTime(0)).toBe("时间未知");
    expect(formatSkillHistoryTime(-1)).toBe("时间未知");
    expect(formatSkillHistoryTime(Number.NaN)).toBe("时间未知");
  });

  it("formats a positive timestamp into a locale string", () => {
    expect(formatSkillHistoryTime(1_700_000_000_000)).toBe(new Date(1_700_000_000_000).toLocaleString());
  });
});
