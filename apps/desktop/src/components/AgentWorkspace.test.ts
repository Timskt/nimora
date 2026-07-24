import { describe, expect, it } from "vitest";
import {
  agentRiskLabel,
  agentToolAccessLabel,
  agentToolDomainLabel,
  agentUsageTotal,
  awaySummaryHasActivity,
  awaySummaryHeadline,
  awaySummarySleepSafeNote,
  awaySummaryViewState,
  awaySummaryWindowLabel,
  buildPlanSteps,
  coerceReasoningChoice,
  controlBudgetFillRatio,
  controlBudgetSlices,
  controlEffectiveStatusLabel,
  countActiveGrants,
  dangerRiskItems,
  defaultModelForProvider,
  deriveContextBudgetUsage,
  deriveWorkspaceTrackingSnapshot,
  extractCheckpointReasoning,
  formatAwayDuration,
  formatContextBudget,
  formatElapsedMs,
  formatTokenCount,
  hasWorkspaceTrackingSignal,
  FULL_DEVICE_DANGER_MATRIX,
  grantBadgeClass,
  grantExpiryLabel,
  grantStatusLabel,
  goalStatusLabel,
  grantTierBadgeText,
  loadReasoningChoice,
  parseReasoningChoice,
  pauseReasonLabel,
  planStepStatusLabel,
  providerStatusLabel,
  reasoningEffortChip,
  reasoningEffortLabel,
  reasoningPolicyForChoice,
  REASONING_CHOICE_STORAGE_KEY,
  saveReasoningChoice,
  shortFingerprint,
  shortGoalId,
  sortAuthorizationGrants,
  tierApprovalLabel,
  workspaceTrackingSummary,
  tierApprovalPolicy,
  tierLabel,
  tierPickerHint,
  tierRequiresDangerAck,
  tierRiskSummary,
  tierSandboxLabel,
  tierSleepSafeCopy,
  tierUsesNeverAsk,
  controlEmptyGuidance,
  unattendedStartFailureMessage,
  UNATTENDED_REASONING_CHOICE_STORAGE_KEY,
} from "./AgentWorkspace";

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
    expect(providerStatusLabel({
      spec: "nimora.desktop-agent-provider-status/1",
      providerId: "provider:ollama-loopback",
      state: "unavailable",
      workerVerified: true,
      serviceReachable: false,
      locality: "local",
      credentialPresent: true,
      models: [],
      message: "offline",
    })).toBe("服务离线");
    expect(providerStatusLabel({
      spec: "nimora.desktop-agent-provider-status/1",
      providerId: "provider:ollama-loopback",
      state: "unavailable",
      workerVerified: true,
      serviceReachable: true,
      locality: "local",
      credentialPresent: true,
      models: [],
      message: "empty",
    })).toBe("无模型");
  });

  it("normalizes plan steps for unattended goal auto mode", () => {
    expect(buildPlanSteps("- 准备\n2. 运行\n* 汇总\n\n")).toEqual(["准备", "运行", "汇总"]);
  });
});

describe("Authorization grant UX helpers", () => {
  it("maps tiers to Chinese labels", () => {
    expect(tierLabel("observe")).toBe("观察");
    expect(tierLabel("workspace")).toBe("工作区");
    expect(tierLabel("trusted_workspace")).toBe("信任工作区");
    expect(tierLabel("unattended")).toBe("无人值守");
    expect(tierLabel("full_device")).toBe("完全设备访问");
  });

  it("requires danger ack only for unattended and full_device", () => {
    expect(tierRequiresDangerAck("workspace")).toBe(false);
    expect(tierRequiresDangerAck("trusted_workspace")).toBe(false);
    expect(tierRequiresDangerAck("unattended")).toBe(true);
    expect(tierRequiresDangerAck("full_device")).toBe(true);
  });

  it("maps grant status and distinct badge classes for all five tiers", () => {
    expect(grantStatusLabel(null)).toBe("无授权");
    expect(grantStatusLabel({ status: "revoked", tier: "workspace" })).toBe("已撤销");
    expect(grantStatusLabel({ status: "expired", tier: "unattended" })).toBe("已过期");
    expect(grantStatusLabel({ status: "active", tier: "unattended" })).toBe("无人值守");
    expect(grantBadgeClass({ status: "active", tier: "observe" })).toBe("grant-badge observe");
    expect(grantBadgeClass({ status: "active", tier: "workspace" })).toBe("grant-badge workspace");
    expect(grantBadgeClass({ status: "active", tier: "trusted_workspace" })).toBe("grant-badge trusted-workspace");
    expect(grantBadgeClass({ status: "active", tier: "unattended" })).toBe("grant-badge unattended");
    expect(grantBadgeClass({ status: "active", tier: "full_device" })).toBe("grant-badge full-device");
    expect(grantBadgeClass({ status: "revoked", tier: "full_device" })).toBe("grant-badge revoked");
    expect(grantBadgeClass({ status: "expired", tier: "unattended" })).toBe("grant-badge expired");
    expect(grantTierBadgeText("trusted_workspace")).toBe("信任工作区");
  });

  it("formats grant expiry for dense grant list rows", () => {
    expect(grantExpiryLabel({ status: "revoked", expiresAtMs: null, revokedAtMs: null })).toBe("已撤销");
    expect(grantExpiryLabel({ status: "expired", expiresAtMs: 1, revokedAtMs: null })).toBe("已过期");
    expect(grantExpiryLabel({ status: "active", expiresAtMs: null, revokedAtMs: null })).toBe("无固定过期");
    const now = Date.UTC(2026, 0, 1, 12, 0, 0);
    expect(grantExpiryLabel({ status: "active", expiresAtMs: now - 1, revokedAtMs: null }, now)).toBe("已过期");
    expect(grantExpiryLabel({ status: "active", expiresAtMs: now + 30 * 60_000, revokedAtMs: null }, now)).toContain("剩余 30 分钟");
  });
});

describe("NeverAskWithinGrant risk labeling", () => {
  it("maps host approval policies by tier", () => {
    expect(tierApprovalPolicy("observe")).toBe("always_ask");
    expect(tierApprovalPolicy("workspace")).toBe("ask_risky");
    expect(tierApprovalPolicy("trusted_workspace")).toBe("never_ask_within_grant");
    expect(tierApprovalPolicy("unattended")).toBe("never_ask_within_grant");
    expect(tierApprovalPolicy("full_device")).toBe("never_ask_within_grant");
  });

  it("exposes Chinese approval and sandbox labels", () => {
    expect(tierApprovalLabel("observe")).toBe("始终确认");
    expect(tierApprovalLabel("workspace")).toBe("风险操作确认");
    expect(tierApprovalLabel("full_device")).toBe("Grant 内免确认");
    expect(tierSandboxLabel("unattended")).toBe("选定根目录");
    expect(tierSandboxLabel("full_device")).toBe("整机访问");
  });

  it("flags NeverAskWithinGrant tiers for risk callouts", () => {
    expect(tierUsesNeverAsk("workspace")).toBe(false);
    expect(tierUsesNeverAsk("trusted_workspace")).toBe(true);
    expect(tierUsesNeverAsk("unattended")).toBe(true);
    expect(tierUsesNeverAsk("full_device")).toBe(true);
  });

  it("describes full_device as whole-device automatic execution", () => {
    expect(tierRiskSummary("full_device")).toContain("整机");
    expect(tierRiskSummary("trusted_workspace")).toMatch(/NeverAskWithinGrant|免确认|自动/);
    expect(tierRiskSummary("unattended")).toMatch(/根目录|自动/);
  });

  it("exposes sleep-safe NeverAskWithinGrant copy in Chinese", () => {
    expect(tierSleepSafeCopy("workspace")).toMatch(/弹窗确认|坐在电脑前/);
    expect(tierSleepSafeCopy("trusted_workspace")).toMatch(/睡眠|离开|NeverAskWithinGrant|免确认/);
    expect(tierSleepSafeCopy("unattended")).toMatch(/睡眠|根目录|8 小时|免确认/);
    expect(tierSleepSafeCopy("full_device")).toMatch(/睡眠|整机|硬禁区|撤销/);
  });

  it("exposes a full_device danger matrix before confirm", () => {
    expect(FULL_DEVICE_DANGER_MATRIX.length).toBeGreaterThanOrEqual(5);
    expect(FULL_DEVICE_DANGER_MATRIX.some((row) => row.capability === "文件系统")).toBe(true);
    expect(FULL_DEVICE_DANGER_MATRIX.some((row) => row.scope.includes("NeverAskWithinGrant") || row.risk.includes("免确认"))).toBe(true);
  });
});

describe("Away Summary presentational helpers", () => {
  const emptySummary = {
    awayStartedAtMs: null,
    awayEndedAtMs: null,
    durationMs: 0,
    completedGoals: 0,
    failedGoals: 0,
    pendingConfirmations: 0,
    grantsRevoked: 0,
    companionMoments: 0,
    highlights: [] as string[],
    generatedAtMs: 1,
  };
  const readySummary = {
    awayStartedAtMs: 1,
    awayEndedAtMs: 2,
    durationMs: 60_000,
    completedGoals: 1,
    failedGoals: 0,
    pendingConfirmations: 0,
    grantsRevoked: 0,
    companionMoments: 0,
    highlights: ["跑完回归"],
    generatedAtMs: 3,
  };

  it("formats duration in Chinese units", () => {
    expect(formatAwayDuration(null)).toBe("—");
    expect(formatAwayDuration(-1)).toBe("—");
    expect(formatAwayDuration(20_000)).toBe("不足 1 分钟");
    expect(formatAwayDuration(12 * 60_000)).toBe("12 分钟");
    expect(formatAwayDuration(90 * 60_000)).toBe("1 小时 30 分");
    expect(formatAwayDuration(120 * 60_000)).toBe("2 小时");
  });

  it("treats empty summary as empty state", () => {
    expect(awaySummaryHasActivity(null)).toBe(false);
    expect(awaySummaryHasActivity(emptySummary)).toBe(false);
    expect(awaySummaryHasActivity(readySummary)).toBe(true);
  });

  it("maps loading, error, empty, and ready surfaces in Chinese UX order", () => {
    expect(awaySummaryViewState({ summary: null, busy: true })).toBe("loading");
    expect(awaySummaryViewState({ summary: emptySummary, busy: true })).toBe("loading");
    expect(awaySummaryViewState({ summary: readySummary, busy: true })).toBe("ready");
    expect(awaySummaryViewState({ summary: null, error: "暂时不可用" })).toBe("error");
    expect(awaySummaryViewState({ summary: readySummary, error: "暂时不可用" })).toBe("error");
    expect(awaySummaryViewState({ summary: emptySummary })).toBe("empty");
    expect(awaySummaryViewState({ summary: null })).toBe("empty");
    expect(awaySummaryViewState({ summary: readySummary })).toBe("ready");
  });

  it("builds Chinese ready-state headlines by priority", () => {
    expect(awaySummaryHeadline({ ...readySummary, pendingConfirmations: 2 })).toContain("待你确认");
    expect(awaySummaryHeadline({ ...readySummary, completedGoals: 0, failedGoals: 1 })).toContain("需要回看");
    expect(awaySummaryHeadline(readySummary)).toContain("完成了");
    expect(awaySummaryHeadline({
      ...emptySummary,
      durationMs: 0,
      grantsRevoked: 1,
      highlights: ["x"],
    })).toContain("撤销");
    expect(awaySummaryHeadline({
      ...emptySummary,
      durationMs: 0,
      companionMoments: 3,
      highlights: ["x"],
    })).toContain("瞬间");
  });

  it("surfaces sleep-safe notes and away windows for ready panels", () => {
    expect(awaySummarySleepSafeNote(readySummary)).toMatch(/睡眠|离开|硬禁区|NeverAskWithinGrant|免确认/);
    expect(awaySummarySleepSafeNote({ ...readySummary, pendingConfirmations: 1 })).toMatch(/待你确认|硬禁区/);
    expect(awaySummarySleepSafeNote({ ...readySummary, grantsRevoked: 1 })).toMatch(/撤销/);
    expect(awaySummarySleepSafeNote({ ...readySummary, failedGoals: 1 })).toMatch(/失败|重试/);
    expect(awaySummaryWindowLabel(emptySummary)).toBeNull();
    expect(awaySummaryWindowLabel(readySummary)).toContain("→");
  });
});

describe("Auto Mode control-center labels and budget", () => {
  it("maps effective status and plan steps to Chinese", () => {
    expect(controlEffectiveStatusLabel("running")).toBe("执行中");
    expect(controlEffectiveStatusLabel("paused")).toBe("已暂停");
    expect(controlEffectiveStatusLabel("indeterminate")).toBe("结果未知");
    expect(planStepStatusLabel("in_progress")).toBe("进行中");
    expect(planStepStatusLabel("completed")).toBe("已完成");
    expect(planStepStatusLabel("pending")).toBe("待开始");
  });

  it("maps pause reasons and keeps unknown tokens", () => {
    expect(pauseReasonLabel(null)).toBe("运行边界正常");
    expect(pauseReasonLabel("confirmation_required")).toBe("需要你确认高风险步骤");
    expect(pauseReasonLabel("budget_exhausted")).toBe("预算已用尽");
    expect(pauseReasonLabel("custom_reason")).toBe("custom_reason");
  });

  it("builds budget slices from session usage and policy", () => {
    const slices = controlBudgetSlices({
      usage: { cycles: 7, toolCalls: 4, elapsedMs: 408_000, inputTokens: 8400, outputTokens: 2100 },
      policy: {
        maxCycles: 32,
        budget: {
          maxSteps: 32,
          maxToolCalls: 16,
          maxElapsedMs: 3_600_000,
          maxInputTokens: 64_000,
          maxOutputTokens: 16_000,
        },
      },
    });
    expect(slices.map((s) => s.label)).toEqual(["轮次", "工具", "用时", "Token"]);
    expect(slices[0]?.display).toBe("7 / 32");
    expect(slices[1]?.display).toBe("4 / 16");
    expect(slices[2]?.display).toContain("分");
    expect(slices[3]?.display).toContain("/");
    expect(controlBudgetFillRatio(slices)).toBeGreaterThan(0);
    expect(controlBudgetFillRatio(slices)).toBeLessThanOrEqual(1);
  });

  it("formats elapsed and token helpers for dense chips", () => {
    expect(formatElapsedMs(500)).toBe("不足 1 秒");
    expect(formatElapsedMs(90_000)).toBe("1 分 30 秒");
    expect(formatTokenCount(840)).toBe("840");
    expect(formatTokenCount(8400)).toBe("8.4k");
  });

  it("labels reasoning effort and extracts checkpoint extras", () => {
    expect(reasoningEffortLabel("high")).toBe("深入");
    expect(reasoningEffortLabel("adaptive")).toBe("自适应");
    expect(extractCheckpointReasoning(null)).toBeNull();
    expect(extractCheckpointReasoning({ model: "x", reasoningEffort: "low" })).toBe("轻量");
    expect(extractCheckpointReasoning({
      model: "x",
      reasoningPolicy: { strategy: "cost_saver", requested: "auto" },
    })).toBe("节省");
    expect(extractCheckpointReasoning({
      model: "x",
      reasoningPolicy: { strategy: "fixed", requested: "very_high" },
    })).toBe("很深入");
  });

  it("maps module tools onto pet-driven domains", () => {
    expect(agentToolDomainLabel("pet.animation.play")).toBe("伙伴");
    expect(agentToolDomainLabel("skill.install")).toBe("技能树");
    expect(agentToolDomainLabel("worker.infer")).toBe("体力");
    expect(agentToolDomainLabel("connector.list")).toBe("感官");
    expect(agentToolDomainLabel("auto.goal.start")).toBe("目标");
    expect(agentToolDomainLabel("fs.read")).toBe("模块");
  });
});

describe("Goal / Auto Mode Chinese surface labels", () => {
  it("maps host goal.status tokens for control-center chips", () => {
    expect(goalStatusLabel("active")).toBe("进行中");
    expect(goalStatusLabel("completed")).toBe("已完成");
    expect(goalStatusLabel("failed")).toBe("失败");
    expect(goalStatusLabel("paused")).toBe("已暂停");
    expect(goalStatusLabel(null)).toBe("未知");
    expect(goalStatusLabel("custom_goal_state")).toBe("custom_goal_state");
  });

  it("exposes scannable five-tier picker hints", () => {
    expect(tierPickerHint("observe")).toMatch(/只读|安全/);
    expect(tierPickerHint("workspace")).toMatch(/推荐|日常/);
    expect(tierPickerHint("trusted_workspace")).toMatch(/自动|睡眠/);
    expect(tierPickerHint("unattended")).toMatch(/高风险|长任务/);
    expect(tierPickerHint("full_device")).toMatch(/整机|极高/);
  });

  it("keeps unattended risk summary Chinese and NeverAsk-aware", () => {
    expect(tierRiskSummary("unattended")).not.toMatch(/Hardblocks/i);
    expect(tierRiskSummary("unattended")).toMatch(/硬禁区|根目录|8 小时|NeverAskWithinGrant|免确认|风险/);
    expect(tierRiskSummary("full_device")).toMatch(/整机|风险/);
  });
});

describe("Context compaction + workspace tracking helpers", () => {
  it("formats context budget with tokens and compaction hints", () => {
    expect(formatContextBudget(null)).toBe("上下文用量暂不可用");
    expect(formatContextBudget({})).toBe("上下文用量暂不可用");
    expect(formatContextBudget({
      inputTokens: 8400,
      outputTokens: 2100,
      maxInputTokens: 64_000,
      maxOutputTokens: 16_000,
      cacheHits: 3,
    })).toMatch(/Token 11k \/ 80k/);
    expect(formatContextBudget({
      inputTokens: 8400,
      outputTokens: 2100,
      maxInputTokens: 64_000,
      maxOutputTokens: 16_000,
      cacheHits: 3,
    })).toContain("缓存命中 3");
    expect(formatContextBudget({
      inputTokens: 70_000,
      outputTokens: 5_000,
      maxInputTokens: 64_000,
      maxOutputTokens: 16_000,
    })).toContain("接近上限");
    expect(formatContextBudget({
      inputTokens: 100,
      outputTokens: 20,
      droppedMessageCount: 12,
    })).toContain("已压缩 12 条");
    expect(formatContextBudget({
      inputTokens: 100,
      outputTokens: 20,
      droppedMessageCount: 12,
      retainedMessageCount: 8,
      sourceMessageCount: 20,
    })).toMatch(/已压缩 12 条 · 保留 8\/20/);
    expect(formatContextBudget({
      messageCount: 40,
      maxMessages: 128,
    })).toMatch(/消息 40 \/ 128/);
    expect(formatContextBudget({
      messageCount: 120,
      maxMessages: 128,
    })).toContain("将触发压缩");
    expect(formatContextBudget({
      compactionState: "compacted",
    })).toContain("已压缩");
    // Prefer retained over raw messageCount after compaction.
    expect(formatContextBudget({
      messageCount: 2,
      retainedMessageCount: 8,
      sourceMessageCount: 20,
    })).toMatch(/已压缩 12 条 · 保留 8\/20/);
  });

  it("summarizes workspace tracking snapshot fields", () => {
    expect(workspaceTrackingSummary(null)).toBe("工作区追踪暂无快照");
    expect(workspaceTrackingSummary({})).toBe("工作区追踪暂无快照");
    expect(workspaceTrackingSummary({ workspaceRoot: "/preview/workspace" })).toMatch(/已绑定工作区/);
    expect(workspaceTrackingSummary({
      revision: 3,
      fileCount: 12,
      fingerprint: "sha256:abcdef0123456789",
    })).toBe("修订 r3 · 12 个文件 · 指纹 abcdef01");
    expect(workspaceTrackingSummary({
      files: [{}, {}, {}],
      workspaceRevision: "git:preview",
    })).toMatch(/3 个文件/);
    expect(shortFingerprint("sha256:abcdef0123456789")).toBe("abcdef01");
    expect(shortFingerprint("git:preview")).toBe("preview");
  });

  it("derives context and workspace views from control-center shaped fields", () => {
    const context = deriveContextBudgetUsage({
      usage: { inputTokens: 8400, outputTokens: 2100 },
      budget: { maxInputTokens: 64_000, maxOutputTokens: 16_000 },
      cacheHits: 3,
      checkpoint: {
        model: "qwen3:8b",
        messages: [{ role: "user" }, { role: "assistant" }],
        compactedContext: {
          sourceMessageCount: 20,
          retainedMessageCount: 8,
          droppedMessageCount: 13,
        },
      },
    });
    expect(context.messageCount).toBe(2);
    expect(context.droppedMessageCount).toBe(13);
    expect(formatContextBudget(context)).toMatch(/已压缩 13 条/);
    expect(formatContextBudget(context)).toMatch(/保留 8\/20/);
    expect(formatContextBudget(context)).toContain("缓存命中 3");

    const snap = deriveWorkspaceTrackingSnapshot({
      workspaceRoot: "/preview/workspace",
      workspaceRevision: "git:preview",
      checkpoint: {
        workspaceRevision: "sha256:deadbeefcafebabe",
        workspaceSnapshot: {
          spec: "nimora.workspace-snapshot/1",
          revision: 4,
          fingerprint: "sha256:1122334455667788",
          files: [{}, {}, {}, {}, {}],
        },
      },
    });
    expect(hasWorkspaceTrackingSignal(snap)).toBe(true);
    expect(snap.revision).toBe(4);
    expect(snap.fileCount).toBe(5);
    expect(workspaceTrackingSummary(snap)).toMatch(/修订 r4/);
    expect(workspaceTrackingSummary(snap)).toContain("5 个文件");
  });

  it("exposes reasoningEffortChip for status-row chips", () => {
    expect(reasoningEffortChip("high")).toBe("深入");
    expect(reasoningEffortChip("adaptive")).toBe("自适应");
    expect(reasoningEffortChip(null)).toBeNull();
  });
});

describe("Reasoning effort selector persistence", () => {
  it("parses strategy and fixed effort choices fail-closed", () => {
    expect(parseReasoningChoice("adaptive")).toBe("adaptive");
    expect(parseReasoningChoice("cost_saver")).toBe("cost_saver");
    expect(parseReasoningChoice("quality_first")).toBe("quality_first");
    expect(parseReasoningChoice("fixed:high")).toBe("fixed:high");
    expect(parseReasoningChoice("fixed:nope")).toBeNull();
    expect(parseReasoningChoice("mystery")).toBeNull();
    expect(parseReasoningChoice("")).toBeNull();
    expect(parseReasoningChoice(null)).toBeNull();
  });

  it("loads and saves preferences through a storage shim", () => {
    const mem = new Map<string, string>();
    const storage = {
      getItem: (key: string) => mem.get(key) ?? null,
      setItem: (key: string, value: string) => { mem.set(key, value); },
    };
    expect(loadReasoningChoice(REASONING_CHOICE_STORAGE_KEY, "adaptive", storage)).toBe("adaptive");
    saveReasoningChoice(REASONING_CHOICE_STORAGE_KEY, "fixed:medium", storage);
    expect(loadReasoningChoice(REASONING_CHOICE_STORAGE_KEY, "adaptive", storage)).toBe("fixed:medium");
    saveReasoningChoice(UNATTENDED_REASONING_CHOICE_STORAGE_KEY, "cost_saver", storage);
    expect(loadReasoningChoice(UNATTENDED_REASONING_CHOICE_STORAGE_KEY, "adaptive", storage)).toBe("cost_saver");
  });

  it("coerces fixed efforts when provider no longer supports them", () => {
    expect(coerceReasoningChoice("fixed:high", ["low", "medium", "high"])).toBe("fixed:high");
    expect(coerceReasoningChoice("fixed:very_high", ["low", "medium"])).toBe("adaptive");
    expect(coerceReasoningChoice("quality_first", ["low"])).toBe("quality_first");
    expect(reasoningPolicyForChoice("fixed:high")).toEqual({
      strategy: "fixed",
      requested: "high",
      allowAutomaticDowngrade: false,
    });
  });
});

describe("Tier danger risk copy + grant list helpers", () => {
  it("exposes Chinese NeverAsk-aware risk bullets by tier", () => {
    const unattended = dangerRiskItems("unattended");
    expect(unattended.some((item) => /NeverAskWithinGrant|免确认/.test(item))).toBe(true);
    expect(unattended.some((item) => /根目录|8 小时/.test(item))).toBe(true);
    const full = dangerRiskItems("full_device");
    expect(full.some((item) => /整机|可达路径/.test(item))).toBe(true);
    expect(full.length).toBeGreaterThan(unattended.length);
    const trusted = dangerRiskItems("trusted_workspace");
    expect(trusted.some((item) => /NeverAskWithinGrant|睡眠|离开/.test(item))).toBe(true);
  });

  it("sorts active grants first and counts them", () => {
    const grants = [
      { status: "revoked" as const, issuedAtMs: 30, grantId: "a", goalId: "g1", tier: "workspace" as const, workspaceRoot: null, expiresAtMs: null, revokedAtMs: 1, spec: "nimora.authorization-grant-summary/1" as const },
      { status: "active" as const, issuedAtMs: 10, grantId: "b", goalId: "goal-long-identifier", tier: "unattended" as const, workspaceRoot: null, expiresAtMs: null, revokedAtMs: null, spec: "nimora.authorization-grant-summary/1" as const },
      { status: "active" as const, issuedAtMs: 40, grantId: "c", goalId: "g3", tier: "full_device" as const, workspaceRoot: null, expiresAtMs: null, revokedAtMs: null, spec: "nimora.authorization-grant-summary/1" as const },
      { status: "expired" as const, issuedAtMs: 50, grantId: "d", goalId: "g4", tier: "trusted_workspace" as const, workspaceRoot: null, expiresAtMs: 1, revokedAtMs: null, spec: "nimora.authorization-grant-summary/1" as const },
    ];
    expect(countActiveGrants(grants)).toBe(2);
    expect(sortAuthorizationGrants(grants).map((g) => g.grantId)).toEqual(["c", "b", "d", "a"]);
    expect(shortGoalId("goal-long-identifier", 10)).toBe("goal-long-…");
    expect(shortGoalId("short")).toBe("short");
  });
});


describe("unattendedStartFailureMessage", () => {
  it("maps grant key / keychain failures to Chinese recovery guidance", () => {
    expect(
      unattendedStartFailureMessage(
        new Error(
          "Agent runtime failed: Authorization grant key unavailable (system secret store unavailable). Restore OS keychain access, or set NIMORA_ALLOW_LOCAL_GRANT_KEY=1 only for local dogfood (not production).",
        ),
      ),
    ).toContain("系统密钥库");
    expect(unattendedStartFailureMessage("secret store rejected grant key write")).toContain("钥匙串");
  });

  it("keeps a safe generic fallback when the host message is opaque", () => {
    expect(unattendedStartFailureMessage(null)).toBe("启动无人值守目标失败；未创建新的授权或任务");
  });
});

describe("controlEmptyGuidance", () => {
  it("offers dual next steps for native empty control center", () => {
    const g = controlEmptyGuidance({ native: true, hasProviderCatalog: true });
    expect(g.title).toContain("目标");
    expect(g.primaryLabel).toContain("无人值守");
    expect(g.secondaryLabel).toContain("模型");
    expect(g.lockHint).toBeNull();
  });

  it("guides browser preview without real grants", () => {
    const g = controlEmptyGuidance({ native: false });
    expect(g.lockHint).toContain("浏览器");
    expect(g.body).toContain("桌面端");
  });

  it("surfaces safe/recovery lock before start", () => {
    const g = controlEmptyGuidance({ safeMode: true, native: true });
    expect(g.lockHint).toBeTruthy();
    expect(g.title).toContain("不能启动");
  });
});

