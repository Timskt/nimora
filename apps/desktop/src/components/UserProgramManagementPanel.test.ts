import { describe, expect, it } from "vitest";
import {
  describeProgramStatus,
  formatMemoryBudget,
  summarizeProgramReceipt,
} from "./UserProgramManagementPanel";
import type {
  UserProgramCatalogEntry,
  UserProgramExecutionReceipt,
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
