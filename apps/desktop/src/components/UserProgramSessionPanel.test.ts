import { describe, expect, it } from "vitest";
import {
  availableCapabilityActions,
  describeCapabilityResponse,
  describeSessionReceipt,
  sessionEligiblePrograms,
  sessionManifestFromEntry,
} from "./UserProgramSessionPanel";
import type {
  UserProgramCatalogEntry,
  UserProgramSessionReceipt,
} from "../platform/desktop";

function entry(overrides: Partial<UserProgramCatalogEntry>): UserProgramCatalogEntry {
  return {
    programId: "studio.demo.companion",
    version: "1.2.0",
    capabilities: ["read-pet-state"],
    commands: [],
    subscriptions: [],
    timeoutMs: 5_000,
    memoryBytes: 8 * 1024 * 1024,
    permissionGranted: true,
    ...overrides,
  };
}

describe("sessionEligiblePrograms", () => {
  it("keeps authorized programs that declare capabilities", () => {
    expect(sessionEligiblePrograms([entry({})])).toHaveLength(1);
  });

  it("drops programs pending authorization", () => {
    expect(sessionEligiblePrograms([entry({ permissionGranted: false })])).toHaveLength(0);
  });

  it("drops programs without any capability", () => {
    expect(sessionEligiblePrograms([entry({ capabilities: [] })])).toHaveLength(0);
  });
});

describe("sessionManifestFromEntry", () => {
  it("reconstructs a serial manifest that mirrors the catalog entry", () => {
    const manifest = sessionManifestFromEntry(entry({ commands: ["safe.pet.notify"], capabilities: ["invoke-safe-commands"] }));
    expect(manifest.id).toBe("studio.demo.companion");
    expect(manifest.eventConcurrency).toBe("serial");
    expect(manifest.commands).toEqual(["safe.pet.notify"]);
    expect(manifest.memoryBytes).toBe(8 * 1024 * 1024);
  });
});

describe("availableCapabilityActions", () => {
  it("exposes only actions backed by granted capabilities", () => {
    const actions = availableCapabilityActions(entry({ capabilities: ["read-pet-state"] }));
    expect(actions.map((action) => action.id)).toEqual(["read-pet-state"]);
  });

  it("expands local-data capability into read/write/delete", () => {
    const actions = availableCapabilityActions(entry({ capabilities: ["store-local-data"] }));
    expect(actions.map((action) => action.id)).toEqual([
      "read-local-data",
      "write-local-data",
      "delete-local-data",
    ]);
  });

  it("binds the invoke-command action to the first declared safe command", () => {
    const actions = availableCapabilityActions(entry({ capabilities: ["invoke-safe-commands"], commands: ["safe.pet.notify"] }));
    const invokeAction = actions.find((action) => action.id === "invoke-command");
    expect(invokeAction).toBeDefined();
    const request = invokeAction?.build('{"text":"hi"}');
    expect(request).toEqual({ type: "invokeCommand", command: "safe.pet.notify", arguments: { text: "hi" } });
  });

  it("builds a write-local-data request by splitting key=value", () => {
    const actions = availableCapabilityActions(entry({ capabilities: ["store-local-data"] }));
    const writeAction = actions.find((action) => action.id === "write-local-data");
    expect(writeAction?.build("counter=42")).toEqual({ type: "writeLocalData", key: "counter", value: "42" });
  });
});

describe("describeSessionReceipt", () => {
  it("summarizes the started session budget", () => {
    const receipt: UserProgramSessionReceipt = {
      executionId: "018f0000-0000-7000-8000-0000000000ab",
      programId: "studio.demo.companion",
      timeoutMs: 5_000,
      memoryBytes: 8 * 1024 * 1024,
    };
    const summary = describeSessionReceipt(receipt);
    expect(summary).toContain("studio.demo.companion");
    expect(summary).toContain("5000 ms");
    expect(summary).toContain("8 MiB");
  });
});

describe("describeCapabilityResponse", () => {
  it("describes a pet-state response", () => {
    expect(describeCapabilityResponse({ type: "petState", value: { mood: "curious" } })).toContain("宠物状态");
  });

  it("distinguishes an empty local-data read", () => {
    expect(describeCapabilityResponse({ type: "localData", value: null })).toBe("本地键为空");
  });

  it("reports a deleted local key", () => {
    expect(describeCapabilityResponse({ type: "localDataDeleted", deleted: true })).toBe("本地键已删除");
  });

  it("reports a missing local key on delete", () => {
    expect(describeCapabilityResponse({ type: "localDataDeleted", deleted: false })).toBe("本地键不存在");
  });

  it("describes an accepted command", () => {
    expect(describeCapabilityResponse({ type: "commandAccepted", value: { spec: "nimora.command/1", executionId: "018f0000-0000-7000-8000-0000000000ab", commandId: "safe.pet.notify", traceId: "018f0000-0000-7000-8000-0000000000ac", arguments: { text: "hi" }, risk: "safe", status: "succeeded", idempotencyKey: null } })).toContain("命令已受理");
  });
});
