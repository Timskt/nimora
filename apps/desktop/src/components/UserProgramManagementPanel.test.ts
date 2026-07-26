import { describe, expect, it } from "vitest";
import {
  describePermissionAudit,
  describeProgramStatus,
  formatMemoryBudget,
  summarizeProgramReceipt,
} from "./UserProgramManagementPanel";
import type {
  UserProgramCatalogEntry,
  UserProgramExecutionReceipt,
  UserProgramPermissionStatus,
} from "../platform/desktop";

function entry(overrides: Partial<UserProgramCatalogEntry>): UserProgramCatalogEntry {
  return {
    programId: "program:demo",
    version: "1.0.0",
    capabilities: ["read-pet-state"],
    commands: [],
    subscriptions: [],
    timeoutMs: 1_000,
    memoryBytes: 4 * 1024 * 1024,
    permissionGranted: false,
    ...overrides,
  };
}

describe("describeProgramStatus", () => {
  it("reports pending authorization until permissions are granted", () => {
    expect(describeProgramStatus(entry({}))).toBe("待授权");
  });

  it("reports authorized once permissions are granted", () => {
    expect(describeProgramStatus(entry({ permissionGranted: true }))).toBe("已授权");
  });
});

describe("formatMemoryBudget", () => {
  it("labels a missing budget", () => {
    expect(formatMemoryBudget(0)).toBe("无内存预算");
    expect(formatMemoryBudget(-1)).toBe("无内存预算");
  });

  it("renders MiB with one decimal below ten", () => {
    expect(formatMemoryBudget(4 * 1024 * 1024)).toBe("4.0 MiB");
  });

  it("drops decimals for ten MiB and above", () => {
    expect(formatMemoryBudget(16 * 1024 * 1024)).toBe("16 MiB");
  });

  it("falls back to KiB for sub-MiB budgets", () => {
    expect(formatMemoryBudget(512 * 1024)).toBe("512 KiB");
  });
});

describe("summarizeProgramReceipt", () => {
  const base: UserProgramExecutionReceipt = {
    executionId: "018f0000-0000-7000-8000-000000000099",
    responses: [],
    agentResults: [],
  };

  it("counts capability responses", () => {
    const summary = summarizeProgramReceipt("program:demo", {
      ...base,
      responses: [{} as never, {} as never],
    });
    expect(summary).toContain("program:demo");
    expect(summary).toContain("2 条能力响应");
  });
});

describe("describePermissionAudit", () => {
  const base = entry({
    programId: "studio.demo",
    version: "1.2.0",
    permissionGranted: true,
    capabilities: ["read-pet-state", "store-local-data"],
  });
  const status: UserProgramPermissionStatus = {
    programId: "studio.demo",
    version: "1.2.0",
    granted: true,
    capabilities: ["store-local-data", "read-pet-state"],
  };

  it("reports no drift when authoritative status matches (order-insensitive)", () => {
    const result = describePermissionAudit(base, status);
    expect(result.drift).toBe(false);
    expect(result.message).toContain("一致");
    expect(result.message).toContain("2 项能力");
  });

  it("flags granted-state drift", () => {
    const result = describePermissionAudit(base, { ...status, granted: false });
    expect(result.drift).toBe(true);
    expect(result.message).toContain("待授权");
  });

  it("flags version drift", () => {
    const result = describePermissionAudit(base, { ...status, version: "1.3.0" });
    expect(result.drift).toBe(true);
    expect(result.message).toContain("v1.3.0");
  });

  it("flags capability-set drift", () => {
    const result = describePermissionAudit(base, { ...status, capabilities: ["read-pet-state"] });
    expect(result.drift).toBe(true);
    expect(result.message).toContain("能力集合已变化");
  });

  it("stays non-drift but advisory when authoritative status is unavailable", () => {
    const result = describePermissionAudit(base, null);
    expect(result.drift).toBe(false);
    expect(result.message).toContain("未返回权威授权状态");
  });
});
