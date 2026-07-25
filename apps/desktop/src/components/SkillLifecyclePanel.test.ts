import { describe, expect, it } from "vitest";
import { describeStatus, summarizeSkillReceipt } from "./SkillLifecyclePanel";
import type { SkillCatalogEntry, SkillExecutionReceipt } from "../platform/desktop";

function entry(overrides: Partial<SkillCatalogEntry>): SkillCatalogEntry {
  return {
    skillId: "skill:demo",
    version: "1.0.0",
    publisher: "publisher:test",
    capabilities: ["invoke-commands"],
    authorized: false,
    enabled: false,
    runtimeStatus: null,
    healthy: true,
    ...overrides,
  };
}

describe("describeStatus", () => {
  it("flags an unhealthy manifest before anything else", () => {
    expect(describeStatus(entry({ healthy: false, authorized: true, enabled: true, runtimeStatus: "activated" }))).toBe("清单已失效");
  });

  it("prefers the live runtime status when present", () => {
    expect(describeStatus(entry({ authorized: true, enabled: true, runtimeStatus: "activated" }))).toBe("运行中");
    expect(describeStatus(entry({ authorized: true, runtimeStatus: "crashed" }))).toBe("已崩溃");
  });

  it("walks the install → authorize → enable ladder without runtime status", () => {
    expect(describeStatus(entry({}))).toBe("待授权");
    expect(describeStatus(entry({ authorized: true }))).toBe("已授权 · 未启用");
    expect(describeStatus(entry({ authorized: true, enabled: true }))).toBe("已启用");
  });
});

describe("summarizeSkillReceipt", () => {
  const base: SkillExecutionReceipt = {
    executionId: "018f0000-0000-7000-8000-000000000099",
    skillId: "skill:demo",
    status: "completed",
    approval: null,
    commandResults: [],
    agentResults: [],
  };

  it("reports a waiting-for-approval execution", () => {
    expect(summarizeSkillReceipt({ ...base, status: "waitingForApproval" })).toContain("待批准");
  });

  it("reports a rejected execution", () => {
    expect(summarizeSkillReceipt({ ...base, status: "rejected" })).toContain("被拒绝");
  });

  it("counts command and agent results for a completed execution", () => {
    const summary = summarizeSkillReceipt({
      ...base,
      commandResults: [{} as never, {} as never],
      agentResults: [{} as never],
    });
    expect(summary).toContain("2 条命令");
    expect(summary).toContain("1 个 Agent 结果");
  });
});
