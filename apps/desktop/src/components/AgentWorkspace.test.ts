import { describe, expect, it } from "vitest";
import { agentToolAccessLabel, agentUsageTotal } from "./AgentWorkspace";

describe("AgentWorkspace", () => {
  it("labels module access by effect instead of provider risk wording", () => {
    expect(agentToolAccessLabel("read_only")).toBe("只读");
    expect(agentToolAccessLabel("reversible_write")).toBe("需确认");
  });

  it("summarizes provider usage for the completed task", () => {
    expect(agentUsageTotal({ usage: { inputTokens: 12, outputTokens: 7 } } as never)).toBe(19);
  });
});
