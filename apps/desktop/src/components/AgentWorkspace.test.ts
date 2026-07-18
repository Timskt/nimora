import { describe, expect, it } from "vitest";
import { agentRiskLabel, agentToolAccessLabel, agentUsageTotal, defaultModelForProvider, providerStatusLabel } from "./AgentWorkspace";

describe("AgentWorkspace", () => {
  it("labels module access by effect instead of provider risk wording", () => {
    expect(agentToolAccessLabel("read_only")).toBe("只读");
    expect(agentToolAccessLabel("reversible_write")).toBe("需确认");
  });

  it("summarizes provider usage for the completed task", () => {
    expect(agentUsageTotal({ usage: { inputTokens: 12, outputTokens: 7 } } as never)).toBe(19);
    expect(agentUsageTotal({ usage: null } as never)).toBe(0);
  });

  it("uses user-facing risk labels for provider module requests", () => {
    expect(agentRiskLabel("safe")).toBe("安全");
    expect(agentRiskLabel("critical")).toBe("严重风险");
  });

  it("suggests a provider-appropriate model without hiding the editable field", () => {
    expect(defaultModelForProvider("provider:ollama-loopback")).toBe("qwen3:8b");
    expect(defaultModelForProvider("provider:deterministic-local")).toBe("model:echo-v1");
  });

  it("distinguishes worker verification from service and model readiness", () => {
    expect(providerStatusLabel(null)).toBe("检测中");
    expect(providerStatusLabel({ spec: "nimora.desktop-agent-provider-status/1", providerId: "provider:ollama-loopback", state: "unavailable", workerVerified: true, serviceReachable: false, locality: "local", credentialPresent: true, models: [], message: "offline" })).toBe("服务离线");
    expect(providerStatusLabel({ spec: "nimora.desktop-agent-provider-status/1", providerId: "provider:ollama-loopback", state: "unavailable", workerVerified: true, serviceReachable: true, locality: "local", credentialPresent: true, models: [], message: "empty" })).toBe("无模型");
  });
});
