import { describe, expect, it } from "vitest";
import { agentCompanionPresentation, createAgentCompanionSignal } from "./agentCompanion";
import { isAgentCompanionSignal } from "../platform/desktop";

describe("agent companion presentation", () => {
  it("maps active work to a persistent work pose", () => {
    expect(agentCompanionPresentation("running")).toEqual({ action: "work", message: "正在陪你完成任务", persistent: true });
  });

  it("celebrates completion without making it permanent", () => {
    expect(agentCompanionPresentation("completed")).toEqual({ action: "celebrate", message: "完成啦，辛苦了！", persistent: false });
  });

  it("creates a content-free versioned signal", () => {
    const signal = createAgentCompanionSignal("waiting_for_confirmation", "task-1");
    expect(signal).toMatchObject({ spec: "nimora.agent-companion-signal/1", status: "waiting_for_confirmation", taskId: "task-1" });
    expect(Object.keys(signal)).toEqual(["spec", "status", "taskId", "updatedAtMs"]);
    expect(isAgentCompanionSignal(signal)).toBe(true);
  });

  it("rejects unknown states and injected display content", () => {
    expect(isAgentCompanionSignal({ spec: "nimora.agent-companion-signal/1", status: "dance", taskId: null, updatedAtMs: 1 })).toBe(false);
    expect(isAgentCompanionSignal({ ...createAgentCompanionSignal("running"), message: "untrusted" })).toBe(false);
  });
});
