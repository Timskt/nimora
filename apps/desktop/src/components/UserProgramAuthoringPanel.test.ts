import { describe, expect, it } from "vitest";
import {
  defaultManifestDraft,
  describeExecutionReceipt,
  describePolicyReport,
  manifestFromDraft,
  validateManifestDraft,
  type ManifestDraft,
} from "./UserProgramAuthoringPanel";
import type {
  ProgramPolicyReport,
  UserProgramExecutionReceipt,
} from "../platform/desktop";

function draft(overrides: Partial<ManifestDraft>): ManifestDraft {
  return { ...defaultManifestDraft(), ...overrides };
}

describe("validateManifestDraft", () => {
  it("accepts the default draft", () => {
    expect(validateManifestDraft(defaultManifestDraft())).toEqual([]);
  });

  it("rejects a non-namespaced id", () => {
    const errors = validateManifestDraft(draft({ id: "hello" }));
    expect(errors.some((message) => message.includes("命名空间"))).toBe(true);
  });

  it("rejects a non-semver version", () => {
    const errors = validateManifestDraft(draft({ version: "1.0" }));
    expect(errors.some((message) => message.includes("语义化版本"))).toBe(true);
  });

  it("requires subscribe-events capability when subscriptions are declared", () => {
    const errors = validateManifestDraft(
      draft({ capabilities: ["read-pet-state"], subscriptionsText: "pet.example.clicked" }),
    );
    expect(errors.some((message) => message.includes("订阅事件"))).toBe(true);
  });

  it("accepts subscriptions when the capability is present", () => {
    const errors = validateManifestDraft(
      draft({
        capabilities: ["read-pet-state", "subscribe-events"],
        subscriptionsText: "pet.example.clicked",
      }),
    );
    expect(errors).toEqual([]);
  });

  it("requires invoke-safe-commands capability when commands are declared", () => {
    const errors = validateManifestDraft(
      draft({ capabilities: ["read-pet-state"], commandsText: "safe.example.notify" }),
    );
    expect(errors.some((message) => message.includes("调用安全命令"))).toBe(true);
  });

  it("rejects commands without the safe. prefix", () => {
    const errors = validateManifestDraft(
      draft({
        capabilities: ["read-pet-state", "invoke-safe-commands"],
        commandsText: "system.example.shutdown",
      }),
    );
    expect(errors.some((message) => message.includes("safe."))).toBe(true);
  });

  it("rejects a timeout above the 30s limit", () => {
    const errors = validateManifestDraft(draft({ timeoutMs: 30_001 }));
    expect(errors.some((message) => message.includes("超时"))).toBe(true);
  });

  it("rejects a memory budget above 64 MiB", () => {
    const errors = validateManifestDraft(draft({ memoryMiB: 65 }));
    expect(errors.some((message) => message.includes("内存"))).toBe(true);
  });

  it("rejects an out-of-range event queue capacity", () => {
    expect(validateManifestDraft(draft({ eventQueueCapacity: 0 })).length).toBeGreaterThan(0);
    expect(validateManifestDraft(draft({ eventQueueCapacity: 65 })).length).toBeGreaterThan(0);
  });
});

describe("manifestFromDraft", () => {
  it("splits subscriptions and commands and converts memory to bytes", () => {
    const manifest = manifestFromDraft(
      draft({
        capabilities: ["subscribe-events", "invoke-safe-commands"],
        subscriptionsText: "pet.a.one\npet.a.two",
        commandsText: "safe.a.one, safe.a.two",
        memoryMiB: 8,
      }),
    );
    expect(manifest.subscriptions).toEqual(["pet.a.one", "pet.a.two"]);
    expect(manifest.commands).toEqual(["safe.a.one", "safe.a.two"]);
    expect(manifest.memoryBytes).toBe(8 * 1024 * 1024);
  });

  it("trims the id and version", () => {
    const manifest = manifestFromDraft(draft({ id: "  studio.example.hello  ", version: " 1.0.0 " }));
    expect(manifest.id).toBe("studio.example.hello");
    expect(manifest.version).toBe("1.0.0");
  });
});

describe("describePolicyReport", () => {
  it("summarizes granted capabilities", () => {
    const report: ProgramPolicyReport = {
      programId: "studio.example.hello",
      grantedCapabilities: ["read-pet-state", "subscribe-events"],
      timeoutMs: 5_000,
      memoryBytes: 8 * 1024 * 1024,
    };
    const summary = describePolicyReport(report);
    expect(summary).toContain("studio.example.hello");
    expect(summary).toContain("2 项能力");
    expect(summary).toContain("读取宠物状态");
  });

  it("labels an empty capability set", () => {
    const report: ProgramPolicyReport = {
      programId: "studio.example.hello",
      grantedCapabilities: [],
      timeoutMs: 5_000,
      memoryBytes: 1024,
    };
    expect(describePolicyReport(report)).toContain("无能力");
  });
});

describe("describeExecutionReceipt", () => {
  it("counts responses and agent results", () => {
    const receipt: UserProgramExecutionReceipt = {
      executionId: "018f0000-0000-7000-8000-0000000000aa",
      responses: [{} as never, {} as never],
      agentResults: [{} as never],
    };
    const summary = describeExecutionReceipt(receipt);
    expect(summary).toContain("2 条能力响应");
    expect(summary).toContain("1 个 Agent 结果");
  });
});
