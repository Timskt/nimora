import { lazy, Suspense, useEffect, useMemo, useState } from "react";
import {
  desktopApi,
  type AgentCatalog,
  type AgentCompanionSignal,
  type AgentCompanionStatus,
  type AgentHistoryPage,
  type AgentProviderStatus,
  type AgentToolResult,
  type AuthorizationGrantSummary,
  type AuthorizationTier,
  type AwaySummary,
  type ConcreteReasoningEffort,
  type DesktopAutoModeControlCenter,
  type LocalAgentResult,
  type ModelReasoningPolicy,
} from "../platform/desktop";
import {
  agentCompanionBubble,
  agentCompanionDirective,
  autoModePhaseLabel,
  companionBubbleFromAutoMode,
  companionStatusFromAutoMode,
  createAgentCompanionSignal,
  type AgentCompanionBubble,
} from "./agentCompanion";

const ProviderSettings = lazy(async () => {
  const module = await import("./ProviderSettings");
  return { default: module.ProviderSettings };
});

type ControlEntry = DesktopAutoModeControlCenter["entries"][number];
type PendingControl =
  | { kind: "pause"; entry: ControlEntry }
  | { kind: "cancel"; entry: ControlEntry }
  | { kind: "resolve"; entry: ControlEntry; decision: "confirmed_not_executed" | "accept_external_effect_and_cancel" };

interface AgentWorkspaceProps {
  safeMode: boolean;
  recoveryMode: boolean;
  initialView?: "run" | "control";
  onNotice(message: string): void;
}

export function agentToolAccessLabel(effect: string): string {
  return effect === "read_only" ? "只读" : "需确认";
}

export function agentUsageTotal(result: LocalAgentResult): number {
  return result.usage ? result.usage.inputTokens + result.usage.outputTokens : 0;
}

export function agentRiskLabel(risk: AgentToolResult["effectiveRisk"]): string {
  return ({ safe: "安全", low: "低风险", medium: "中风险", high: "高风险", critical: "严重风险" })[risk];
}

export function defaultModelForProvider(providerId: string): string {
  return providerId === "provider:ollama-loopback" ? "qwen3:8b" : "model:echo-v1";
}

async function publishCompanionStatus(status: AgentCompanionStatus, taskId: string | null = null): Promise<void> {
  await desktopApi.publishAgentCompanionSignal(createAgentCompanionSignal(status, taskId)).catch(() => undefined);
  await desktopApi.applyPetDirective(agentCompanionDirective(status)).catch(() => undefined);
}

export function providerStatusLabel(status: AgentProviderStatus | null): string {
  if (!status) return "检测中";
  if (!status.serviceReachable) return "服务离线";
  if (status.models.length === 0) return "无模型";
  return status.providerId === "provider:deterministic-local" ? "可用" : "服务在线";
}

/** Safe / Recovery Mode lock copy — Chinese, next-step oriented. */
export function modeLockReason(safeMode: boolean, recoveryMode: boolean): string | null {
  if (safeMode) return "安全模式已启用：执行类操作已锁定。退出安全模式后再试。";
  if (recoveryMode) return "恢复模式仅允许查看与修复：退出恢复模式后再启动目标或运行任务。";
  return null;
}


/** Map host errors from unattended start / grant issue into Chinese next-step copy. */
export function unattendedStartFailureMessage(error: unknown): string {
  const raw = error instanceof Error ? error.message : String(error ?? "");
  const lower = raw.toLowerCase();
  if (
    lower.includes("authorization grant key unavailable")
    || lower.includes("secret store")
    || lower.includes("keychain")
    || lower.includes("nimora_allow_local_grant_key")
  ) {
    return "无法签发授权：系统密钥库不可用或无法保存授权密钥。请恢复 macOS 钥匙串 / Windows 凭据管理器访问后重试；未创建新的授权或任务。";
  }
  if (lower.includes("safe mode")) {
    return "安全模式已启用，无法签发无人值守授权。请退出安全模式后重试。";
  }
  if (lower.includes("recovery mode")) {
    return "恢复模式仅允许修复操作，无法签发无人值守授权。请退出恢复模式后重试。";
  }
  if (lower.includes("workspace") && lower.includes("invalid")) {
    return "工作区路径无效或不可用。请选择可写工作区后重试。";
  }
  if (raw.trim()) {
    // Prefer a short host message without leaking stack; keep Chinese wrapper.
    const clipped = raw.replace(/^Agent runtime failed:\s*/i, "").trim();
    if (clipped && clipped.length < 180 && !clipped.includes("\n")) {
      return `启动无人值守目标失败：${clipped}；未创建新的授权或任务。`;
    }
  }
  return "启动无人值守目标失败；未创建新的授权或任务";
}

/** Browser preview / host lock for control actions. */
export function controlActionLockReason(options: {
  safeMode?: boolean;
  recoveryMode?: boolean;
  native?: boolean;
  busy?: boolean;
}): string | null {
  const mode = modeLockReason(Boolean(options.safeMode), Boolean(options.recoveryMode));
  if (mode) return mode;
  if (options.native === false) return "浏览器预览不能提交控制操作。请在 Nimora 桌面端暂停、取消或对账。";
  if (options.busy) return "上一项控制操作仍在提交，请稍候…";
  return null;
}

/**
 * Why the Run Task primary button is locked.
 * Returns null when the owner can submit.
 */
export function runTaskLockReason(options: {
  busy?: boolean;
  prompt: string;
  model: string;
  providerStatus: AgentProviderStatus | null;
  networkRequiresConsent?: boolean;
  allowNetwork?: boolean;
  safeMode?: boolean;
  recoveryMode?: boolean;
}): string | null {
  const mode = modeLockReason(Boolean(options.safeMode), Boolean(options.recoveryMode));
  if (mode) return mode;
  if (options.busy) return "任务运行中，请等待当前轮次结束或取消。";
  if (!options.prompt.trim()) return "请先填写任务内容。";
  if (!options.model.trim()) return "请填写模型名称，或从列表中选择已探测到的模型。";
  const status = options.providerStatus;
  if (!status) return "正在检测 Provider，请稍候…";
  if (status.state !== "ready") {
    if (!status.serviceReachable) {
      return "Provider 服务离线。请到「模型连接」检查地址/密钥，或改用本地模型。";
    }
    if (status.models.length === 0) {
      return "该 Provider 暂无可用模型。请到「模型连接」配置默认模型，或确认服务已加载模型。";
    }
    return status.message?.trim()
      ? `${status.message.trim()}。请到「模型连接」修复后重试。`
      : "Provider 尚未就绪。请到「模型连接」完成 HTTPS 连接与模型配置。";
  }
  if (!status.models.some((item) => item.name === options.model.trim())) {
    return `模型「${options.model.trim()}」不在当前 Provider 列表中。请从下拉建议中选择，或先在「模型连接」添加该模型。`;
  }
  if (options.networkRequiresConsent && !options.allowNetwork) {
    return "网络 Provider 需要你勾选「允许本次请求联网」后才能发送任务内容。";
  }
  return null;
}

/**
 * Owner-facing readiness line under the run / creator form.
 * Always Chinese; always points at a next step when not ready.
 */
export function providerReadinessGuidance(
  status: AgentProviderStatus | null,
  options?: {
    model?: string;
    networkRequiresConsent?: boolean;
    allowNetwork?: boolean;
    hostMessage?: string | null;
  },
): string {
  if (!status) return "正在检测 Provider 与模型列表…";
  if (status.state === "ready") {
    const model = options?.model?.trim() ?? "";
    if (model && !status.models.some((item) => item.name === model)) {
      return `模型「${model}」未出现在探测列表。可改选已有模型，或到「模型连接」声明默认模型。`;
    }
    if (options?.networkRequiresConsent && !options.allowNetwork) {
      return "服务在线。勾选联网授权后即可发送（内容会出境到你配置的 HTTPS 服务）。";
    }
    return status.message?.trim() || "Provider 就绪，可以运行。";
  }
  if (!status.serviceReachable) {
    return "服务离线：确认桌面宿主可访问该地址；公网必须是 HTTPS，本机可用 loopback。下一步：打开「模型连接」。";
  }
  if (status.models.length === 0) {
    return "已连通但无模型：在「模型连接」填写默认模型，或确认 Ollama/兼容服务已拉取模型。";
  }
  const host = options?.hostMessage?.trim() || status.message?.trim();
  if (host) return `${host}。若持续失败，请到「模型连接」检查 Base URL、凭据与默认模型。`;
  return "Provider 未就绪。请到「模型连接」添加 HTTPS Provider 与模型后再试。";
}

/** Actionable Chinese notices for host IPC soft-failures (toast / banner). */
export function agentCatalogUnavailableMessage(): string {
  return "Agent 工具目录未能加载。请确认 Nimora 桌面端正在运行，然后点「重试加载」，或先到「模型连接」检查 Provider。";
}

export function controlCenterUnavailableMessage(): string {
  return "目标控制中心未能同步。请确认桌面宿主可用后点「重新同步」；浏览器预览仅展示演示数据。";
}

export function agentHistoryUnavailableMessage(): string {
  return "本机执行历史未能读取。可先运行一次对话任务生成记录，或稍后点「重试」。";
}

export function awaySummaryUnavailableMessage(): string {
  return "离开摘要未能生成。请确认已有无人值守目标，或点「重试」；无目标时摘要会保持为空。";
}

export function providerStatusUnavailableMessage(providerId: string): string {
  return `无法检测「${providerId}」。请到「模型连接」核对地址与凭据，或改用内置本地模型。`;
}

/**
 * Why unattended goal start is locked.
 * Returns null when the owner can open/submit start.
 */
export function unattendedStartLockReason(options: {
  safeMode?: boolean;
  recoveryMode?: boolean;
  startBusy?: boolean;
  title?: string;
  objective?: string;
  stepCount?: number;
  native?: boolean;
}): string | null {
  const mode = modeLockReason(Boolean(options.safeMode), Boolean(options.recoveryMode));
  if (mode) return mode;
  if (options.native === false) return "浏览器预览不能签发真实授权。请在 Nimora 桌面端启动无人值守目标。";
  if (options.startBusy) return "正在启动目标并签发授权，请稍候…";
  if (!(options.title ?? "").trim()) return "请填写目标标题。";
  if (!(options.objective ?? "").trim()) return "请填写目标描述与完成标准。";
  if ((options.stepCount ?? 0) <= 0) return "请至少添加一步计划（每行一步）。";
  return null;
}

/** Module tool button lock reason (safe/recovery). */
export function moduleToolLockReason(options: {
  toolBusy?: boolean;
  safeMode?: boolean;
  recoveryMode?: boolean;
}): string | null {
  const mode = modeLockReason(Boolean(options.safeMode), Boolean(options.recoveryMode));
  if (mode) return mode;
  if (options.toolBusy) return "上一项模块操作仍在处理…";
  return null;
}

type ReasoningChoice = "adaptive" | "cost_saver" | "quality_first" | `fixed:${ConcreteReasoningEffort}`;

export function reasoningPolicyForChoice(choice: ReasoningChoice): ModelReasoningPolicy {
  if (choice.startsWith("fixed:")) {
    return { strategy: "fixed", requested: choice.slice(6) as ConcreteReasoningEffort, allowAutomaticDowngrade: false };
  }
  return { strategy: choice as "adaptive" | "cost_saver" | "quality_first", requested: "auto", allowAutomaticDowngrade: true };
}

/** localStorage keys for Agent/Auto Mode reasoning selectors (owner preference). */
export const REASONING_CHOICE_STORAGE_KEY = "nimora.agent.reasoningChoice";
export const UNATTENDED_REASONING_CHOICE_STORAGE_KEY = "nimora.agent.unattendedReasoningChoice";

const REASONING_STRATEGY_CHOICES = new Set<string>(["adaptive", "cost_saver", "quality_first"]);
const FIXED_EFFORTS = new Set<string>(["minimal", "low", "medium", "high", "very_high", "maximum"]);

/** Parse a stored reasoning selector value; reject unknown tokens fail-closed. */
export function parseReasoningChoice(raw: string | null | undefined): ReasoningChoice | null {
  if (!raw || typeof raw !== "string") return null;
  const value = raw.trim();
  if (!value) return null;
  if (REASONING_STRATEGY_CHOICES.has(value)) return value as ReasoningChoice;
  if (value.startsWith("fixed:")) {
    const effort = value.slice(6);
    if (FIXED_EFFORTS.has(effort)) return value as ReasoningChoice;
  }
  return null;
}

/** Load persisted reasoning choice; default when missing/invalid. */
export function loadReasoningChoice(
  storageKey: string,
  fallback: ReasoningChoice = "adaptive",
  storage: Pick<Storage, "getItem"> | null = typeof localStorage !== "undefined" ? localStorage : null,
): ReasoningChoice {
  if (!storage) return fallback;
  try {
    return parseReasoningChoice(storage.getItem(storageKey)) ?? fallback;
  } catch {
    return fallback;
  }
}

/** Persist reasoning choice; ignore storage failures (private mode / SSR). */
export function saveReasoningChoice(
  storageKey: string,
  choice: ReasoningChoice,
  storage: Pick<Storage, "setItem"> | null = typeof localStorage !== "undefined" ? localStorage : null,
): void {
  if (!storage) return;
  try {
    storage.setItem(storageKey, choice);
  } catch {
    // best-effort preference only
  }
}

/** Keep fixed effort only when provider still advertises it. */
export function coerceReasoningChoice(
  choice: ReasoningChoice,
  supportedEfforts: readonly string[] | null | undefined,
): ReasoningChoice {
  if (!choice.startsWith("fixed:")) return choice;
  const effort = choice.slice(6);
  if (supportedEfforts && supportedEfforts.includes(effort)) return choice;
  return "adaptive";
}

const AUTHORIZATION_TIERS: AuthorizationTier[] = [
  "observe",
  "workspace",
  "trusted_workspace",
  "unattended",
  "full_device",
];

export function tierLabel(tier: AuthorizationTier): string {
  return ({
    observe: "观察",
    workspace: "工作区",
    trusted_workspace: "信任工作区",
    unattended: "无人值守",
    full_device: "完全设备访问",
  })[tier];
}

export function tierRequiresDangerAck(tier: AuthorizationTier): boolean {
  return tier === "unattended" || tier === "full_device";
}

/** Host maps trusted_workspace / unattended / full_device → NeverAskWithinGrant. */
export type TierApprovalPolicy = "always_ask" | "ask_risky" | "never_ask_within_grant";

export function tierApprovalPolicy(tier: AuthorizationTier): TierApprovalPolicy {
  if (tier === "observe") return "always_ask";
  if (tier === "workspace") return "ask_risky";
  return "never_ask_within_grant";
}

export function tierApprovalLabel(tier: AuthorizationTier): string {
  return ({
    always_ask: "始终确认",
    ask_risky: "风险操作确认",
    never_ask_within_grant: "Grant 内免确认",
  })[tierApprovalPolicy(tier)];
}

export function tierSandboxLabel(tier: AuthorizationTier): string {
  return ({
    observe: "只读观察",
    workspace: "工作区写入",
    trusted_workspace: "工作区写入（自动）",
    unattended: "选定根目录",
    full_device: "整机访问",
  })[tier];
}

export function tierRiskSummary(tier: AuthorizationTier): string {
  return ({
    observe: "仅观察与只读；敏感写操作会拦截。",
    workspace: "可写入工作区；高风险步骤仍会请求确认。",
    trusted_workspace: "工作区内自动放行（NeverAskWithinGrant）；硬禁区仍不可绕过。",
    unattended: "在选定根目录内自动执行最长 8 小时（NeverAskWithinGrant）；硬禁区仍生效，风险高。",
    full_device: "整机范围内自动执行最长 4 小时；可读写系统可达路径，风险极高。",
  })[tier];
}

export function tierUsesNeverAsk(tier: AuthorizationTier): boolean {
  return tierApprovalPolicy(tier) === "never_ask_within_grant";
}

export function grantStatusLabel(grant: Pick<AuthorizationGrantSummary, "status" | "tier"> | null | undefined): string {
  if (!grant) return "无授权";
  if (grant.status === "revoked") return "已撤销";
  if (grant.status === "expired") return "已过期";
  return tierLabel(grant.tier);
}

export function grantBadgeClass(grant: Pick<AuthorizationGrantSummary, "status" | "tier"> | null | undefined): string {
  if (!grant) return "grant-badge revoked";
  if (grant.status === "revoked") return "grant-badge revoked";
  if (grant.status === "expired") return "grant-badge expired";
  return ({
    observe: "grant-badge observe",
    workspace: "grant-badge workspace",
    trusted_workspace: "grant-badge trusted-workspace",
    unattended: "grant-badge unattended",
    full_device: "grant-badge full-device",
  })[grant.tier];
}

/** Short tier badge glyph text for dense lists (观察/工作区/…). */
export function grantTierBadgeText(tier: AuthorizationTier): string {
  return tierLabel(tier);
}

/** Sleep-safe NeverAskWithinGrant explainer for risk cards and dialogs. */
export function tierSleepSafeCopy(tier: AuthorizationTier): string {
  if (!tierUsesNeverAsk(tier)) {
    return "此档位仍会在敏感步骤弹窗确认，适合你坐在电脑前一起操作。";
  }
  if (tier === "full_device") {
    return "睡眠/离开安全：有效期内整机范围内免逐步弹窗（NeverAskWithinGrant）；支付、Secret 与关闭安全机制等硬禁区仍不可绕过。随时可撤销。";
  }
  if (tier === "unattended") {
    return "睡眠/离开安全：选定根目录内自动执行最长 8 小时，Grant 内免确认；硬禁区仍生效，醒来可在授权列表撤销。";
  }
  return "睡眠/离开安全：信任工作区内自动放行（NeverAskWithinGrant），不会反复弹窗打断；硬禁区与组织策略仍不可绕过。";
}

export type ControlBudgetSlice = {
  label: string;
  used: number;
  max: number;
  display: string;
  ratio: number;
};

/** Chinese Auto Mode effective/job status for control-center chips. */
export function controlEffectiveStatusLabel(status: string | null | undefined): string {
  if (!status) return "未知";
  return ({
    running: "执行中",
    paused: "已暂停",
    completed: "已完成",
    cancelled: "已取消",
    failed: "失败",
    indeterminate: "结果未知",
    starting: "启动中",
    pausing: "暂停中",
    cancelling: "取消中",
    submitted: "已提交",
    waiting_for_confirmation: "等待确认",
    interrupted: "已中断",
  } as Record<string, string>)[status] ?? status;
}

/** Host goal.status tokens → owner-facing Chinese. */
export function goalStatusLabel(status: string | null | undefined): string {
  if (!status) return "未知";
  return ({
    draft: "草稿",
    active: "进行中",
    running: "执行中",
    paused: "已暂停",
    completed: "已完成",
    failed: "失败",
    cancelled: "已取消",
    archived: "已归档",
    waiting: "等待中",
    blocked: "受阻",
  } as Record<string, string>)[status] ?? status;
}

/** Short Chinese hint under each tier option in the grant picker. */
export function tierPickerHint(tier: AuthorizationTier): string {
  return ({
    observe: "最安全 · 只读",
    workspace: "日常推荐 · 风险确认",
    trusted_workspace: "工作区自动 · 睡眠友好",
    unattended: "长任务 · 高风险",
    full_device: "整机 · 极高风险",
  })[tier];
}

/** Plan step status chips in Chinese. */
export function planStepStatusLabel(status: string | null | undefined): string {
  if (!status) return "待开始";
  return ({
    pending: "待开始",
    ready: "就绪",
    in_progress: "进行中",
    running: "进行中",
    blocked: "受阻",
    waiting: "等待中",
    completed: "已完成",
    done: "已完成",
    failed: "失败",
    cancelled: "已取消",
    skipped: "已跳过",
  } as Record<string, string>)[status] ?? status;
}

/** Host pause_reason tokens → owner-facing Chinese. */
export function pauseReasonLabel(reason: string | null | undefined): string {
  if (!reason) return "运行边界正常";
  return ({
    confirmation_required: "需要你确认高风险步骤",
    unsafe_effect: "检测到不安全副作用",
    budget_exhausted: "预算已用尽",
    max_cycles: "达到最大轮次",
    max_tool_calls: "达到工具调用上限",
    max_elapsed: "达到时长上限",
    token_budget: "达到 Token 预算",
    grant_revoked: "授权已撤销",
    user_paused: "你已手动暂停",
    projection_stale: "投影收敛中",
  } as Record<string, string>)[reason] ?? reason;
}

export function formatElapsedMs(ms: number | null | undefined): string {
  if (typeof ms !== "number" || !Number.isFinite(ms) || ms < 0) return "—";
  if (ms < 1000) return "不足 1 秒";
  const totalSec = Math.round(ms / 1000);
  if (totalSec < 60) return `${totalSec} 秒`;
  const minutes = Math.floor(totalSec / 60);
  const seconds = totalSec % 60;
  if (minutes < 60) return seconds > 0 ? `${minutes} 分 ${seconds} 秒` : `${minutes} 分`;
  const hours = Math.floor(minutes / 60);
  const remMin = minutes % 60;
  return remMin > 0 ? `${hours} 小时 ${remMin} 分` : `${hours} 小时`;
}

export function formatTokenCount(n: number | null | undefined): string {
  if (typeof n !== "number" || !Number.isFinite(n) || n < 0) return "—";
  if (n < 1000) return `${Math.round(n)}`;
  if (n < 10_000) return `${(n / 1000).toFixed(1).replace(/\.0$/, "")}k`;
  return `${Math.round(n / 1000)}k`;
}

/** Compact budget rows: cycles · tools · elapsed · tokens from session usage/policy. */
export function controlBudgetSlices(session: {
  usage: { cycles: number; toolCalls: number; elapsedMs: number; inputTokens: number; outputTokens: number };
  policy: {
    maxCycles: number;
    budget: {
      maxSteps: number;
      maxToolCalls: number;
      maxElapsedMs: number;
      maxInputTokens: number;
      maxOutputTokens: number;
    };
  };
}): ControlBudgetSlice[] {
  const usage = session.usage;
  const budget = session.policy.budget;
  const maxCycles = Math.max(1, session.policy.maxCycles || budget.maxSteps || 1);
  const usedTokens = Math.max(0, usage.inputTokens) + Math.max(0, usage.outputTokens);
  const maxTokens = Math.max(0, budget.maxInputTokens) + Math.max(0, budget.maxOutputTokens);
  const maxTools = Math.max(1, budget.maxToolCalls || 1);
  const maxElapsed = Math.max(1, budget.maxElapsedMs || 1);
  const ratio = (used: number, max: number) => Math.max(0, Math.min(1, max > 0 ? used / max : 0));
  return [
    {
      label: "轮次",
      used: usage.cycles,
      max: maxCycles,
      display: `${usage.cycles} / ${maxCycles}`,
      ratio: ratio(usage.cycles, maxCycles),
    },
    {
      label: "工具",
      used: usage.toolCalls,
      max: maxTools,
      display: `${usage.toolCalls} / ${maxTools}`,
      ratio: ratio(usage.toolCalls, maxTools),
    },
    {
      label: "用时",
      used: usage.elapsedMs,
      max: maxElapsed,
      display: `${formatElapsedMs(usage.elapsedMs)} / ${formatElapsedMs(maxElapsed)}`,
      ratio: ratio(usage.elapsedMs, maxElapsed),
    },
    {
      label: "Token",
      used: usedTokens,
      max: Math.max(1, maxTokens),
      display: maxTokens > 0
        ? `${formatTokenCount(usedTokens)} / ${formatTokenCount(maxTokens)}`
        : formatTokenCount(usedTokens),
      ratio: maxTokens > 0 ? ratio(usedTokens, maxTokens) : 0,
    },
  ];
}

/** Overall control budget fill from the tightest (highest) slice ratio. */
export function controlBudgetFillRatio(slices: readonly ControlBudgetSlice[]): number {
  if (slices.length === 0) return 0;
  return Math.max(0, Math.min(1, Math.max(...slices.map((slice) => slice.ratio))));
}

/** Concrete / strategy effort → Chinese chip. */
export function reasoningEffortLabel(effort: string | null | undefined): string | null {
  if (!effort) return null;
  return ({
    auto: "自动",
    adaptive: "自适应",
    cost_saver: "节省",
    quality_first: "极致",
    minimal: "最少",
    low: "轻量",
    medium: "均衡",
    high: "深入",
    very_high: "很深入",
    maximum: "极致",
  } as Record<string, string>)[effort] ?? effort;
}

/**
 * Best-effort read of thinking effort from checkpoint extras (host may attach
 * reasoningPolicy / requestedEffort without a fixed schema field).
 */
export function extractCheckpointReasoning(checkpoint: unknown): string | null {
  if (!checkpoint || typeof checkpoint !== "object") return null;
  const record = checkpoint as Record<string, unknown>;
  const direct = record.reasoningEffort ?? record.requestedEffort ?? record.effort;
  if (typeof direct === "string" && direct.trim()) return reasoningEffortLabel(direct.trim());
  const policy = record.reasoningPolicy;
  if (policy && typeof policy === "object") {
    const rec = policy as Record<string, unknown>;
    const strategy = typeof rec.strategy === "string" ? rec.strategy : null;
    const requested = typeof rec.requested === "string" ? rec.requested : null;
    if (requested && requested !== "auto") return reasoningEffortLabel(requested);
    if (strategy) return reasoningEffortLabel(strategy);
  }
  return null;
}

/** Pet-driven domain for control-center module tools (Skill/Worker/Connector/Subject). */
export function agentToolDomainLabel(toolId: string | null | undefined): string {
  if (!toolId) return "模块";
  const id = toolId.toLowerCase();
  if (id.startsWith("pet.") || id.includes("lifeform") || id.includes("companion")) return "伙伴";
  if (id.includes("skill")) return "技能树";
  if (id.includes("worker") || id.includes("infer") || id.includes("ollama")) return "体力";
  if (id.includes("connector") || id.includes("sensor") || id.includes("context")) return "感官";
  if (id.includes("auto") || id.includes("goal") || id.includes("grant")) return "目标";
  return "模块";
}

/**
 * Host ContextCompactionPolicy.max_messages default (auto_mode_runner).
 * Control-center does not yet stream live policy — display-only ceiling.
 */
export const DEFAULT_CONTEXT_MAX_MESSAGES = 128;

/** Session / checkpoint-derived context budget fields for owner-facing chips. */
export type ContextBudgetUsage = {
  inputTokens?: number | null;
  outputTokens?: number | null;
  maxInputTokens?: number | null;
  maxOutputTokens?: number | null;
  messageCount?: number | null;
  maxMessages?: number | null;
  sourceMessageCount?: number | null;
  retainedMessageCount?: number | null;
  droppedMessageCount?: number | null;
  cacheHits?: number | null;
  compactionState?: string | null;
};

/**
 * Best-effort workspace tracking view.
 * Prefer real `nimora.workspace-snapshot/1` extras when host attaches them;
 * otherwise fall back to workspaceRevision / grant.workspaceRoot already on the entry.
 */
export type WorkspaceTrackingSnapshot = {
  revision?: number | string | null;
  fingerprint?: string | null;
  fileCount?: number | null;
  files?: ReadonlyArray<unknown> | null;
  workspaceRoot?: string | null;
  workspaceRevision?: string | null;
  parentFingerprint?: string | null;
};

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}

function finiteNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

/** Shorten sha256:/git: fingerprints for dense chips (full host tokens stay in <code>). */
export function shortFingerprint(value: string | null | undefined, keep = 8): string | null {
  if (!value || typeof value !== "string") return null;
  const trimmed = value.trim();
  if (!trimmed) return null;
  const bare = trimmed.replace(/^(sha256:|git:)/i, "");
  if (bare.length <= keep) return bare || trimmed;
  return bare.slice(0, keep);
}

function compactionStateLabel(state: string | null | undefined): string | null {
  if (!state || typeof state !== "string") return null;
  const key = state.trim().toLowerCase();
  if (!key) return null;
  return ({
    idle: "未压缩",
    none: "未压缩",
    fresh: "未压缩",
    compacted: "已压缩",
    compacted_context: "已压缩",
    near_limit: "接近上限",
    compacting: "压缩中",
    skipped: "跳过压缩",
  } as Record<string, string>)[key] ?? state.trim();
}

/**
 * Chinese compact context line: tokens used / budget + compaction hint.
 * Missing fields degrade gracefully to a Chinese empty state (no English jargon walls).
 */
export function formatContextBudget(usage: ContextBudgetUsage | null | undefined): string {
  if (!usage) return "上下文用量暂不可用";
  const parts: string[] = [];
  const inTok = finiteNumber(usage.inputTokens);
  const outTok = finiteNumber(usage.outputTokens);
  const maxIn = finiteNumber(usage.maxInputTokens);
  const maxOut = finiteNumber(usage.maxOutputTokens);
  const hasToken = inTok != null || outTok != null;
  const used = Math.max(0, inTok ?? 0) + Math.max(0, outTok ?? 0);
  const max = Math.max(0, maxIn ?? 0) + Math.max(0, maxOut ?? 0);

  if (hasToken && max > 0) {
    parts.push(`Token ${formatTokenCount(used)} / ${formatTokenCount(max)}`);
    const ratio = used / max;
    if (ratio >= 0.9) parts.push("接近上限");
    else if (ratio >= 0.7) parts.push("偏满");
  } else if (hasToken) {
    parts.push(`Token ${formatTokenCount(used)}`);
  }

  const dropped = finiteNumber(usage.droppedMessageCount);
  const retained = finiteNumber(usage.retainedMessageCount);
  const source = finiteNumber(usage.sourceMessageCount);
  // After compaction, retained is more accurate than raw checkpoint messages[] length.
  const msgCount = retained ?? finiteNumber(usage.messageCount);
  const maxMsg = finiteNumber(usage.maxMessages) ?? DEFAULT_CONTEXT_MAX_MESSAGES;
  const inferredDropped =
    dropped != null && dropped > 0
      ? dropped
      : source != null && retained != null && source > retained
        ? source - retained
        : null;

  if (inferredDropped != null && inferredDropped > 0) {
    parts.push(`已压缩 ${inferredDropped} 条`);
    if (retained != null && source != null) {
      parts.push(`保留 ${retained}/${source}`);
    } else if (retained != null) {
      parts.push(`保留 ${retained} 条`);
    }
  } else {
    const state = compactionStateLabel(usage.compactionState);
    if (state && state !== "未压缩") {
      parts.push(state);
    } else if (msgCount != null) {
      if (msgCount >= maxMsg * 0.85) parts.push(`消息 ${msgCount} · 将触发压缩`);
      else parts.push(`消息 ${msgCount} / ${maxMsg}`);
    } else if (state) {
      parts.push(state);
    }
  }

  const cacheHits = finiteNumber(usage.cacheHits);
  if (cacheHits != null && cacheHits > 0) parts.push(`缓存命中 ${cacheHits}`);

  return parts.length > 0 ? parts.join(" · ") : "上下文用量暂不可用";
}

/**
 * Workspace tracking one-liner: file count · revision · short fingerprint.
 * Empty when host has not bound a root or snapshot-like revision yet.
 */
export function workspaceTrackingSummary(snapshot: WorkspaceTrackingSnapshot | null | undefined): string {
  if (!snapshot) return "工作区追踪暂无快照";
  const parts: string[] = [];
  const rev = snapshot.revision;
  if (typeof rev === "number" && Number.isFinite(rev)) parts.push(`修订 r${rev}`);
  else if (typeof rev === "string" && rev.trim()) parts.push(`修订 ${rev.trim()}`);

  let fileCount = finiteNumber(snapshot.fileCount);
  if (fileCount == null && Array.isArray(snapshot.files)) fileCount = snapshot.files.length;
  if (fileCount != null) parts.push(`${fileCount} 个文件`);

  const fpSource =
    (typeof snapshot.fingerprint === "string" && snapshot.fingerprint.trim()) ||
    (typeof snapshot.workspaceRevision === "string" && snapshot.workspaceRevision.trim()) ||
    "";
  const fp = shortFingerprint(fpSource || null);
  if (fp) parts.push(`指纹 ${fp}`);

  if (parts.length > 0) return parts.join(" · ");
  if (snapshot.workspaceRoot) return "已绑定工作区 · 尚无快照明细";
  return "工作区追踪暂无快照";
}

/** Thinking-effort chip label (same mapping as reasoningEffortLabel). */
export function reasoningEffortChip(effort: string | null | undefined): string | null {
  return reasoningEffortLabel(effort);
}

/**
 * Best-effort context budget from control-center fields.
 * No dedicated context_management IPC on the entry yet — derive from session
 * usage/policy, job.cacheHits, and optional checkpoint extras (messages[],
 * compactedContext, compactionState). Do not invent host invokes.
 */
export function deriveContextBudgetUsage(source: {
  usage?: { inputTokens?: number; outputTokens?: number } | null;
  budget?: { maxInputTokens?: number; maxOutputTokens?: number } | null;
  checkpoint?: unknown;
  cacheHits?: number | null;
}): ContextBudgetUsage {
  const usage: ContextBudgetUsage = {
    inputTokens: source.usage?.inputTokens ?? null,
    outputTokens: source.usage?.outputTokens ?? null,
    maxInputTokens: source.budget?.maxInputTokens ?? null,
    maxOutputTokens: source.budget?.maxOutputTokens ?? null,
    cacheHits: source.cacheHits ?? null,
    maxMessages: DEFAULT_CONTEXT_MAX_MESSAGES,
  };
  const cp = asRecord(source.checkpoint);
  if (!cp) return usage;

  if (Array.isArray(cp.messages)) usage.messageCount = cp.messages.length;

  const compacted =
    asRecord(cp.compactedContext) ??
    asRecord(cp.compacted) ??
    asRecord(cp.context);
  if (compacted) {
    const sourceCount = finiteNumber(compacted.sourceMessageCount);
    const retainedCount = finiteNumber(compacted.retainedMessageCount);
    const droppedCount = finiteNumber(compacted.droppedMessageCount);
    if (sourceCount != null) usage.sourceMessageCount = sourceCount;
    if (retainedCount != null) usage.retainedMessageCount = retainedCount;
    if (droppedCount != null) usage.droppedMessageCount = droppedCount;
    if (usage.messageCount == null && Array.isArray(compacted.messages)) {
      usage.messageCount = compacted.messages.length;
    }
  }

  if (typeof cp.compactionState === "string") usage.compactionState = cp.compactionState;
  else if (typeof cp.contextCompactionState === "string") usage.compactionState = cp.contextCompactionState;

  const policy = asRecord(cp.contextCompactionPolicy) ?? asRecord(cp.compactionPolicy);
  const maxMessages = finiteNumber(policy?.maxMessages);
  if (maxMessages != null) usage.maxMessages = maxMessages;

  return usage;
}

/**
 * Best-effort workspace tracking from grant/session/checkpoint.
 * Prefer `nimora.workspace-snapshot/1`-shaped extras; otherwise use revision strings.
 */
export function deriveWorkspaceTrackingSnapshot(source: {
  workspaceRoot?: string | null;
  workspaceRevision?: string | null;
  checkpoint?: unknown;
}): WorkspaceTrackingSnapshot {
  const snap: WorkspaceTrackingSnapshot = {
    workspaceRoot: source.workspaceRoot ?? null,
    workspaceRevision: source.workspaceRevision ?? null,
  };
  const cp = asRecord(source.checkpoint);
  if (cp && typeof cp.workspaceRevision === "string" && cp.workspaceRevision.trim()) {
    snap.workspaceRevision = cp.workspaceRevision;
  }

  const nested =
    asRecord(cp?.workspaceSnapshot) ??
    asRecord(cp?.workspace) ??
    asRecord(cp?.snapshot) ??
    (cp && cp.spec === "nimora.workspace-snapshot/1" ? cp : null);

  if (nested) {
    if (typeof nested.revision === "number" || typeof nested.revision === "string") {
      snap.revision = nested.revision as number | string;
    }
    if (typeof nested.fingerprint === "string") snap.fingerprint = nested.fingerprint;
    if (typeof nested.parentFingerprint === "string") snap.parentFingerprint = nested.parentFingerprint;
    if (Array.isArray(nested.files)) {
      snap.files = nested.files;
      snap.fileCount = nested.files.length;
    }
    const fileCount = finiteNumber(nested.fileCount);
    if (fileCount != null) snap.fileCount = fileCount;
  }

  if (!snap.fingerprint && typeof snap.workspaceRevision === "string" && snap.workspaceRevision.trim()) {
    snap.fingerprint = snap.workspaceRevision;
  }
  return snap;
}

/** Whether the tracking strip should render (root or any snapshot-like signal). */
export function hasWorkspaceTrackingSignal(snapshot: WorkspaceTrackingSnapshot | null | undefined): boolean {
  if (!snapshot) return false;
  if (snapshot.workspaceRoot && String(snapshot.workspaceRoot).trim()) return true;
  if (snapshot.workspaceRevision && String(snapshot.workspaceRevision).trim()) return true;
  if (snapshot.fingerprint && String(snapshot.fingerprint).trim()) return true;
  if (snapshot.revision != null && snapshot.revision !== "") return true;
  if (finiteNumber(snapshot.fileCount) != null) return true;
  if (Array.isArray(snapshot.files) && snapshot.files.length > 0) return true;
  return false;
}

export function buildPlanSteps(raw: string): string[] {
  return raw
    .split(/\r?\n/)
    .map((line) => line.replace(/^\s*(?:[-*]|\d+[.)])\s*/, "").trim())
    .filter(Boolean);
}


const DANGER_RISKS_BASE = [
  "Grant 内免确认（NeverAskWithinGrant）：有效期内不会逐步弹窗打断。",
  "仍受硬禁区约束：支付、密钥、关闭安全机制等不可绕过。",
  "Prompt Injection 或供应链内容可能诱导错误操作。",
  "随时可在授权列表撤销；撤销后新的派发会被阻止。",
] as const;

const DANGER_RISKS_UNATTENDED = [
  ...DANGER_RISKS_BASE,
  "可在选定根目录内读取、覆盖或删除文件（最长约 8 小时）。",
  "可在授权范围内运行命令与脚本。",
  "关闭「离线优先」时，任务相关数据可能离开设备。",
] as const;

const DANGER_RISKS_FULL_DEVICE = [
  ...DANGER_RISKS_BASE,
  "可读写本机可达路径（整机沙箱），风险极高。",
  "可运行命令、安装和执行第三方代码。",
  "工具可能触及凭据、浏览器会话或钥匙串可达面。",
  "外部 API、发布、部署和账户操作可能不可逆。",
  "关闭「离线优先」时网络出站不受逐步确认打断。",
] as const;

/** Chinese bullet risks for danger-ack dialogs (tier-aware). */
export function dangerRiskItems(tier: AuthorizationTier): readonly string[] {
  if (tier === "full_device") return DANGER_RISKS_FULL_DEVICE;
  if (tier === "unattended") return DANGER_RISKS_UNATTENDED;
  if (tierUsesNeverAsk(tier)) {
    return [
      "此档位使用 NeverAskWithinGrant：授权范围内自动放行，适合睡眠/离开场景。",
      "硬禁区与组织策略仍不可绕过。",
      "可随时在授权列表撤销。",
    ];
  }
  return DANGER_RISKS_BASE;
}

/** @deprecated use dangerRiskItems(tier) — kept for dense unattended copy paths */
const DANGER_RISKS = DANGER_RISKS_UNATTENDED;

/** Capability × scope risk matrix shown before confirming full_device grants. */
export const FULL_DEVICE_DANGER_MATRIX: ReadonlyArray<{ capability: string; scope: string; risk: string }> = [
  { capability: "文件系统", scope: "整机可达路径", risk: "严重" },
  { capability: "命令执行", scope: "本机进程与脚本", risk: "严重" },
  { capability: "网络出站", scope: "若关闭离线优先", risk: "高" },
  { capability: "凭据与会话", scope: "浏览器/钥匙串可达面", risk: "严重" },
  { capability: "外部副作用", scope: "发布 · 部署 · 账户", risk: "不可逆" },
  { capability: "审批策略", scope: "NeverAskWithinGrant", risk: "Grant 内免确认" },
];

/** Away Summary host IPC contract (see `desktopApi.getAwaySummary`). */
export type { AwaySummary } from "../platform/desktop";

export interface AwaySummaryPanelProps {
  summary: AwaySummary | null;
  busy?: boolean;
  /** Chinese error copy when host IPC fails; takes priority over empty/ready. */
  error?: string | null;
  /** Goal currently bound to the summary query (host `get_away_summary`). */
  goalId?: string | null;
  goalTitle?: string | null;
  onRefresh?: () => void;
}

export type AwaySummaryViewState = "loading" | "error" | "empty" | "ready";

export function formatAwayDuration(ms: number | null | undefined): string {
  if (typeof ms !== "number" || !Number.isFinite(ms) || ms < 0) return "—";
  const totalMinutes = Math.round(ms / 60_000);
  if (totalMinutes < 1) return "不足 1 分钟";
  if (totalMinutes < 60) return `${totalMinutes} 分钟`;
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  return minutes > 0 ? `${hours} 小时 ${minutes} 分` : `${hours} 小时`;
}

export function awaySummaryHasActivity(summary: AwaySummary | null | undefined): boolean {
  if (!summary) return false;
  return (
    summary.completedGoals > 0
    || summary.failedGoals > 0
    || summary.pendingConfirmations > 0
    || summary.grantsRevoked > 0
    || summary.companionMoments > 0
    || summary.highlights.length > 0
    || (typeof summary.durationMs === "number" && summary.durationMs > 0)
  );
}

/** Pure view-state for Away Summary empty / loading / error / ready surfaces. */
export function awaySummaryViewState(options: {
  summary: AwaySummary | null | undefined;
  busy?: boolean;
  error?: string | null;
}): AwaySummaryViewState {
  if (options.error) return "error";
  if (options.busy && !awaySummaryHasActivity(options.summary)) return "loading";
  if (!awaySummaryHasActivity(options.summary)) return "empty";
  return "ready";
}

/** One-line Chinese headline for the ready Away Summary surface. */
export function awaySummaryHeadline(summary: AwaySummary): string {
  if (summary.pendingConfirmations > 0) {
    return `有 ${summary.pendingConfirmations} 项待你确认`;
  }
  if (summary.failedGoals > 0) {
    return `有 ${summary.failedGoals} 个目标需要回看`;
  }
  if (summary.completedGoals > 0) {
    return `完成了 ${summary.completedGoals} 个目标`;
  }
  if (summary.grantsRevoked > 0) {
    return `期间撤销了 ${summary.grantsRevoked} 个授权`;
  }
  if (summary.companionMoments > 0) {
    return `伙伴留下了 ${summary.companionMoments} 个瞬间`;
  }
  if (typeof summary.durationMs === "number" && summary.durationMs > 0) {
    return `你离开了 ${formatAwayDuration(summary.durationMs)}`;
  }
  return "离开期间有新动态";
}

/** Sleep-safe / NeverAsk callout under ready metrics (Chinese). */
export function awaySummarySleepSafeNote(summary: AwaySummary): string {
  if (summary.pendingConfirmations > 0) {
    return "仍有步骤在等你确认：NeverAskWithinGrant 只覆盖已授权范围内的自动步骤，不会跳过硬禁区。";
  }
  if (summary.grantsRevoked > 0) {
    return "期间有授权被撤销，后续自动步骤已收敛停止；醒来后可重新签发 Grant。";
  }
  if (summary.failedGoals > 0) {
    return "失败目标不会自动重试高风险步骤；请先核对日志再决定是否继续。";
  }
  return "睡眠/离开安全：Grant 内免确认步骤不会反复弹窗打断；支付、Secret 与关闭安全机制等硬禁区仍不可绕过。";
}

/** Compact away window label when host timestamps exist. */
export function awaySummaryWindowLabel(summary: AwaySummary): string | null {
  if (summary.awayStartedAtMs == null && summary.awayEndedAtMs == null) return null;
  const start = summary.awayStartedAtMs != null
    ? new Date(summary.awayStartedAtMs).toLocaleString("zh-CN")
    : "—";
  const end = summary.awayEndedAtMs != null
    ? new Date(summary.awayEndedAtMs).toLocaleString("zh-CN")
    : "现在";
  return `${start} → ${end}`;
}

export function grantExpiryLabel(grant: Pick<AuthorizationGrantSummary, "status" | "expiresAtMs" | "revokedAtMs">, nowMs = Date.now()): string {
  if (grant.status === "revoked") {
    return grant.revokedAtMs ? `撤销于 ${new Date(grant.revokedAtMs).toLocaleString("zh-CN")}` : "已撤销";
  }
  if (grant.status === "expired") return "已过期";
  if (grant.expiresAtMs == null) return "无固定过期";
  if (grant.expiresAtMs <= nowMs) return "已过期";
  const remainMs = grant.expiresAtMs - nowMs;
  return `剩余 ${formatAwayDuration(remainMs)} · ${new Date(grant.expiresAtMs).toLocaleString("zh-CN")}`;
}

/** Active grants first, then newest issued — dense list ordering. */
export function sortAuthorizationGrants<T extends Pick<AuthorizationGrantSummary, "status" | "issuedAtMs">>(
  grants: readonly T[],
): T[] {
  const rank = (status: string) => (status === "active" ? 0 : status === "expired" ? 1 : 2);
  return [...grants].sort((a, b) => {
    const byStatus = rank(a.status) - rank(b.status);
    if (byStatus !== 0) return byStatus;
    return (b.issuedAtMs ?? 0) - (a.issuedAtMs ?? 0);
  });
}

/** Short Goal id for grant rows without inventing titles. */
export function shortGoalId(goalId: string | null | undefined, keep = 10): string {
  if (!goalId) return "—";
  const trimmed = goalId.trim();
  if (trimmed.length <= keep) return trimmed;
  return `${trimmed.slice(0, keep)}…`;
}

/** Count active grants for header / bulk revoke affordance. */
export function countActiveGrants(grants: readonly Pick<AuthorizationGrantSummary, "status">[]): number {
  return grants.filter((grant) => grant.status === "active").length;
}

export function AwaySummaryPanel({
  summary,
  busy = false,
  error = null,
  goalId = null,
  goalTitle = null,
  onRefresh,
}: AwaySummaryPanelProps) {
  const view = awaySummaryViewState({ summary, busy, error });
  const goalLine = goalTitle?.trim() || (goalId ? `Goal ${goalId.slice(0, 8)}` : null);
  return (
    <section className="away-summary-panel" aria-busy={busy || undefined} aria-label="离开摘要">
      <header className="away-summary-header">
        <div>
          <p className="card-label">AWAY SUMMARY</p>
          <h3>离开期间发生了什么</h3>
          <p>
            汇总无人值守目标、待确认与伙伴动态
            {goalLine ? ` · 当前目标「${goalLine}」` : " · 尚未绑定目标"}
            。
          </p>
        </div>
        {onRefresh ? (
          <button disabled={busy} onClick={onRefresh} type="button" aria-label="刷新离开摘要">
            {busy ? "刷新中…" : "刷新摘要"}
          </button>
        ) : null}
      </header>
      {view === "loading" ? (
        <div className="away-summary-loading" role="status" aria-live="polite">
          <strong>正在汇总离开期间的动态…</strong>
          <p>读取目标进度、待确认与伙伴瞬间，请稍候。</p>
        </div>
      ) : null}
      {view === "error" ? (
        <div className="away-summary-error" role="alert">
          <strong>离开摘要暂时不可用</strong>
          <p>{error || "宿主未能返回摘要，请稍后重试。"}</p>
          {onRefresh ? (
            <button className="secondary-button" disabled={busy} onClick={onRefresh} type="button" aria-label="重试刷新离开摘要">
              {busy ? "重试中…" : "重试"}
            </button>
          ) : null}
        </div>
      ) : null}
      {view === "empty" ? (
        <div className="away-summary-empty" role="status">
          <strong>暂无离开摘要</strong>
          <p>
            {goalLine
              ? `目标「${goalLine}」暂无离开期间动态。离开后完成、失败与待确认会出现在这里。`
              : "当你离开后，无人值守目标的完成、失败与待确认会汇总在这里。可先启动一个目标，或稍后再刷新。"}
          </p>
          {onRefresh ? (
            <button className="secondary-button" disabled={busy} onClick={onRefresh} type="button">
              {busy ? "刷新中…" : "立即刷新"}
            </button>
          ) : null}
        </div>
      ) : null}
      {view === "ready" && summary ? (
        <>
          <div className="away-summary-ready-lead" role="status">
            <strong>{awaySummaryHeadline(summary)}</strong>
            {awaySummaryWindowLabel(summary) ? (
              <p className="away-summary-window">离开窗口 · {awaySummaryWindowLabel(summary)}</p>
            ) : null}
          </div>
          <div className="away-summary-metrics" aria-label="离开期间指标">
            <div><small>离开时长</small><b>{formatAwayDuration(summary.durationMs)}</b></div>
            <div data-tone={summary.completedGoals > 0 ? "ok" : undefined}><small>完成目标</small><b>{summary.completedGoals}</b></div>
            <div data-tone={summary.failedGoals > 0 ? "danger" : undefined}><small>失败</small><b>{summary.failedGoals}</b></div>
            <div data-tone={summary.pendingConfirmations > 0 ? "warn" : undefined}><small>待确认</small><b>{summary.pendingConfirmations}</b></div>
            <div data-tone={summary.grantsRevoked > 0 ? "warn" : undefined}><small>撤销授权</small><b>{summary.grantsRevoked}</b></div>
            <div data-tone={summary.companionMoments > 0 ? "ok" : undefined}><small>伙伴瞬间</small><b>{summary.companionMoments}</b></div>
          </div>
          {summary.highlights.length > 0 ? (
            <ul className="away-summary-highlights" aria-label="摘要亮点">
              {summary.highlights.map((item) => (
                <li key={item} className="away-summary-chip">{item}</li>
              ))}
            </ul>
          ) : (
            <p className="away-summary-no-highlights">有指标变动，但还没有文字亮点。</p>
          )}
          <p className="away-summary-sleep-note" role="note">{awaySummarySleepSafeNote(summary)}</p>
          <footer className="away-summary-footer">
            {goalLine ? <span className="away-summary-goal" title={goalId ?? undefined}>关联目标 · {goalLine}</span> : null}
            <span>生成于 {new Date(summary.generatedAtMs).toLocaleString("zh-CN")}</span>
          </footer>
        </>
      ) : null}
    </section>
  );
}

export function CompanionStatusStrip({ bubble }: { bubble: AgentCompanionBubble | null }) {
  if (!bubble) {
    return (
      <div className="agent-companion-strip is-idle" role="status" aria-label="伙伴状态：空闲">
        <span className="companion-bubble tone-idle">
          <i aria-hidden="true" />
          <strong>空闲</strong>
          <em>伙伴准备好陪你开始</em>
          <span className="companion-narrative-chips" aria-label="伙伴叙事">
            <span className="companion-chip action">安静待命</span>
            <span className="companion-chip mood">平静</span>
          </span>
        </span>
      </div>
    );
  }
  return (
    <div className={`agent-companion-strip tone-${bubble.tone}`} role="status" aria-live="polite" aria-label={`伙伴状态：${bubble.label}`}>
      <span className={`companion-bubble tone-${bubble.tone}`}>
        <i aria-hidden="true" />
        <strong>{bubble.label}</strong>
        <em>{bubble.message}</em>
        <span className="companion-narrative-chips" aria-label="伙伴叙事">
          <span className="companion-chip action">{bubble.actionLabel}</span>
          <span className="companion-chip mood">{bubble.moodLabel}</span>
        </span>
      </span>
    </div>
  );
}


export function AgentWorkspace({ safeMode, recoveryMode, initialView = "run", onNotice }: AgentWorkspaceProps) {
  const [view, setView] = useState<"run" | "control" | "providers" | "history">(initialView);
  const [controlCenter, setControlCenter] = useState<DesktopAutoModeControlCenter | null>(null);
  const [pendingControl, setPendingControl] = useState<PendingControl | null>(null);
  const [controlReason, setControlReason] = useState("");
  const [controlBusy, setControlBusy] = useState(false);
  const [controlLoading, setControlLoading] = useState(false);
  const [controlError, setControlError] = useState<string | null>(null);
  const [catalog, setCatalog] = useState<AgentCatalog | null>(null);
  const [catalogError, setCatalogError] = useState<string | null>(null);
  const [historyError, setHistoryError] = useState<string | null>(null);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [prompt, setPrompt] = useState("总结一下当前可用的本地能力");
  const [providerId, setProviderId] = useState("provider:deterministic-local");
  const [model, setModel] = useState("model:echo-v1");
  const [result, setResult] = useState<LocalAgentResult | null>(null);
  const [providerStatus, setProviderStatus] = useState<AgentProviderStatus | null>(null);
  const [allowNetwork, setAllowNetwork] = useState(false);
  const [reasoningChoice, setReasoningChoice] = useState<ReasoningChoice>(() =>
    loadReasoningChoice(REASONING_CHOICE_STORAGE_KEY, "adaptive"),
  );
  const [busy, setBusy] = useState(false);
  const [toolBusy, setToolBusy] = useState(false);
  const [toolResult, setToolResult] = useState<AgentToolResult | null>(null);
  const [turnCancelled, setTurnCancelled] = useState(false);
  const [history, setHistory] = useState<AgentHistoryPage | null>(null);
  const [goalTitle, setGoalTitle] = useState("");
  const [goalObjective, setGoalObjective] = useState("");
  const [goalStepsText, setGoalStepsText] = useState("");
  const [workspaceRoot, setWorkspaceRoot] = useState("");
  const [authorizationTier, setAuthorizationTier] = useState<AuthorizationTier>("workspace");
  const [offlineMode, setOfflineMode] = useState(true);
  const [maxTurnsPerBatch, setMaxTurnsPerBatch] = useState(8);
  const [unattendedReasoningChoice, setUnattendedReasoningChoice] = useState<ReasoningChoice>(() =>
    loadReasoningChoice(UNATTENDED_REASONING_CHOICE_STORAGE_KEY, "adaptive"),
  );
  const [showStartPanel, setShowStartPanel] = useState(false);
  const [pendingDangerAck, setPendingDangerAck] = useState(false);
  const [dangerAcknowledged, setDangerAcknowledged] = useState(false);
  const [authorizationGrants, setAuthorizationGrants] = useState<AuthorizationGrantSummary[]>([]);
  const [startBusy, setStartBusy] = useState(false);
  const [companionSignal, setCompanionSignal] = useState<AgentCompanionSignal | null>(null);
  const [awaySummary, setAwaySummary] = useState<AwaySummary | null>(null);
  const [awayBusy, setAwayBusy] = useState(false);
  const [awayError, setAwayError] = useState<string | null>(null);
  const [awayGoalId, setAwayGoalId] = useState<string | null>(null);
  const [awayGoalTitle, setAwayGoalTitle] = useState<string | null>(null);
  const focusAutoEntry = useMemo(() => {
    const entries = controlCenter?.entries ?? [];
    return entries.find((entry) => (
      entry.session.status === "running"
      || entry.session.status === "paused"
      || entry.job.status === "starting"
      || entry.job.status === "running"
      || entry.job.status === "pausing"
      || entry.job.status === "cancelling"
      || entry.job.status === "failed"
      || entry.attempt?.status === "indeterminate"
    )) ?? entries[0] ?? null;
  }, [controlCenter]);

  const companionBubble = useMemo(() => {
    // Prefer live Auto Mode focus so Running/Paused/Failed chips stay accurate in Control Center.
    if (view === "control" && focusAutoEntry) {
      return companionBubbleFromAutoMode(
        focusAutoEntry.attempt?.status === "indeterminate" ? "indeterminate" : focusAutoEntry.job.status,
        {
          pauseReason: focusAutoEntry.job.pauseReason ?? focusAutoEntry.session.pauseReason,
          indeterminate: focusAutoEntry.attempt?.status === "indeterminate",
        },
      );
    }
    return companionSignal ? agentCompanionBubble(companionSignal.status) : null;
  }, [view, focusAutoEntry, companionSignal]);
  const activeProviderId = result?.task.providerId ?? providerId;
  const activeProvider = catalog?.providers.find((provider) => provider.id === activeProviderId);
  const reasoningCapabilities = activeProvider?.capabilities.reasoning;
  const startProvider = catalog?.providers.find((provider) => provider.id === providerId);
  const startReasoningCapabilities = startProvider?.capabilities.reasoning;
  const planSteps = buildPlanSteps(goalStepsText);
  const canStartUnattended = Boolean(
    goalTitle.trim()
    && goalObjective.trim()
    && planSteps.length > 0
    && !startBusy
    && !safeMode
    && !recoveryMode
    && desktopApi.native,
  );
  const runLockReason = runTaskLockReason({
    busy,
    prompt,
    model,
    providerStatus,
    networkRequiresConsent: activeProvider?.locality === "network",
    allowNetwork,
    safeMode,
    recoveryMode,
  });
  const controlLockReason = controlActionLockReason({
    safeMode,
    recoveryMode,
    native: desktopApi.native,
    busy: controlBusy,
  });
  const toolLockReason = moduleToolLockReason({ toolBusy, safeMode, recoveryMode });

  useEffect(() => {
    void refreshCatalog();
    void loadHistory();
  }, [onNotice]);

  useEffect(() => setView(initialView), [initialView]);

  useEffect(() => {
    if (view !== "control") return;
    void refreshControlCenter().catch(() => undefined);
    void refreshAwaySummary().catch(() => undefined);
  }, [view, onNotice]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void desktopApi.onAgentCompanionSignal((signal) => {
      if (!cancelled) setCompanionSignal(signal);
    }).then((dispose) => {
      unlisten = dispose;
    }).catch(() => undefined);
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (!pendingDangerAck && !pendingControl) return;
    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== "Escape" || startBusy || controlBusy) return;
      event.preventDefault();
      setPendingDangerAck(false);
      setDangerAcknowledged(false);
      setPendingControl(null);
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [pendingDangerAck, pendingControl, startBusy, controlBusy]);

  async function loadHistory() {
    setHistoryLoading(true);
    setHistoryError(null);
    try {
      setHistory(await desktopApi.agentHistory(5));
    } catch {
      setHistoryError(agentHistoryUnavailableMessage());
      onNotice(agentHistoryUnavailableMessage());
    } finally {
      setHistoryLoading(false);
    }
  }

  async function refreshHistory() {
    await loadHistory();
  }

  async function refreshCatalog() {
    setCatalogError(null);
    try {
      setCatalog(await desktopApi.agentCatalog());
    } catch {
      setCatalogError(agentCatalogUnavailableMessage());
      onNotice(agentCatalogUnavailableMessage());
    }
  }

  async function refreshControlCenter() {
    setControlLoading(true);
    setControlError(null);
    try {
      const center = await desktopApi.autoModeControlCenter();
      try {
        const grants = await desktopApi.listAuthorizationGrants();
        setAuthorizationGrants(grants);
        const byGoal = new Map(grants.map((grant) => [grant.goalId, grant]));
        setControlCenter({
          ...center,
          entries: center.entries.map((entry) => ({
            ...entry,
            grant: entry.grant ?? byGoal.get(entry.goal.id) ?? null,
          })),
        });
      } catch {
        setAuthorizationGrants([]);
        setControlCenter(center);
      }
      // Surface Goal/Auto Mode phase on the companion strip without re-applying pet
      // directives on every control-center poll (start/pause/cancel still publish).
      const focus = center.entries.find((entry) => (
        entry.session.status === "running"
        || entry.session.status === "paused"
        || entry.job.status === "starting"
        || entry.job.status === "running"
        || entry.job.status === "pausing"
        || entry.job.status === "cancelling"
        || entry.attempt?.status === "indeterminate"
      )) ?? center.entries[0] ?? null;
      if (focus) {
        const nextStatus = companionStatusFromAutoMode(
          focus.attempt?.status === "indeterminate" ? "indeterminate" : focus.job.status,
          {
            pauseReason: focus.job.pauseReason ?? focus.session.pauseReason,
            indeterminate: focus.attempt?.status === "indeterminate",
          },
        );
        setCompanionSignal(createAgentCompanionSignal(nextStatus, focus.job.jobId));
      }
    } catch {
      setControlError(controlCenterUnavailableMessage());
      onNotice(controlCenterUnavailableMessage());
      throw new Error("control-center-unavailable");
    } finally {
      setControlLoading(false);
    }
  }

  async function refreshAwaySummary(preferredGoalId?: string | null) {
    setAwayBusy(true);
    setAwayError(null);
    try {
      const preferredEntry = preferredGoalId
        ? controlCenter?.entries.find((entry) => entry.goal.id === preferredGoalId)
        : null;
      const activeEntry = preferredEntry
        ?? controlCenter?.entries.find((entry) => (
          entry.session.status === "running" || entry.session.status === "paused"
        ))
        ?? controlCenter?.entries[0]
        ?? null;
      const goalId = preferredGoalId ?? activeEntry?.goal.id ?? null;
      setAwayGoalId(goalId);
      setAwayGoalTitle(activeEntry?.goal.title ?? null);
      setAwaySummary(await desktopApi.getAwaySummary(goalId ?? undefined));
    } catch {
      setAwaySummary(null);
      setAwayError(awaySummaryUnavailableMessage());
      onNotice(awaySummaryUnavailableMessage());
    } finally {
      setAwayBusy(false);
    }
  }

  function prepareControl(action: PendingControl) {
    if (safeMode || recoveryMode || !desktopApi.native) return;
    setControlReason("");
    setPendingControl(action);
  }

  async function executeControl() {
    if (!pendingControl || controlBusy || safeMode || recoveryMode || !desktopApi.native) return;
    if (pendingControl.kind === "resolve" && !controlReason.trim()) return;
    setControlBusy(true);
    try {
      if (pendingControl.kind === "pause") {
        await desktopApi.pauseAutoModeJob(pendingControl.entry.job.jobId);
        await publishCompanionStatus("waiting_for_confirmation", pendingControl.entry.job.jobId).catch(() => undefined);
        onNotice("已请求安全暂停，当前原子步骤结束后生效");
      } else if (pendingControl.kind === "cancel") {
        await desktopApi.cancelAutoModeJob(pendingControl.entry.job.jobId);
        await publishCompanionStatus("cancelled", pendingControl.entry.job.jobId).catch(() => undefined);
        onNotice("已请求取消，正在收敛运行状态");
      } else {
        const attempt = pendingControl.entry.attempt;
        if (!attempt) throw new Error("attempt-missing");
        await desktopApi.resolveAutoModeAttempt({
          sessionId: pendingControl.entry.session.id,
          attemptId: attempt.id,
          checkpointSequence: attempt.checkpointSequence,
          requestFingerprint: attempt.requestFingerprint,
          decision: pendingControl.decision,
          reason: controlReason.trim(),
        });
        onNotice(pendingControl.decision === "confirmed_not_executed" ? "已确认外部操作未执行；任务保持暂停" : "已接受潜在外部副作用并取消任务");
      }
      setPendingControl(null);
      await refreshControlCenter();
    } catch {
      onNotice("控制请求已失效或状态发生变化，没有自动重试");
      await refreshControlCenter().catch(() => undefined);
    } finally {
      setControlBusy(false);
    }
  }

  async function revokeGrant(grantId: string) {
    if (controlBusy || safeMode || recoveryMode) return;
    setControlBusy(true);
    try {
      const result = await desktopApi.revokeAuthorizationGrant(grantId);
      onNotice(result.revoked ? "授权已撤销，新的派发将被阻止" : "授权状态未变化");
      await refreshControlCenter().catch(() => undefined);
    } catch {
      onNotice("撤销授权失败，请稍后重试");
    } finally {
      setControlBusy(false);
    }
  }

  async function revokeAllActiveGrants() {
    if (controlBusy || safeMode || recoveryMode) return;
    const active = authorizationGrants.filter((grant) => grant.status === "active");
    if (active.length === 0) return;
    setControlBusy(true);
    let revoked = 0;
    try {
      for (const grant of active) {
        const result = await desktopApi.revokeAuthorizationGrant(grant.grantId);
        if (result.revoked) revoked += 1;
      }
      onNotice(revoked > 0 ? `已撤销 ${revoked} 个有效授权，新的派发将被阻止` : "授权状态未变化");
      await refreshControlCenter().catch(() => undefined);
    } catch {
      onNotice("批量撤销未完成，请逐条重试");
      await refreshControlCenter().catch(() => undefined);
    } finally {
      setControlBusy(false);
    }
  }

  function openStartPanel() {
    setShowStartPanel(true);
    setPendingDangerAck(false);
    setDangerAcknowledged(false);
  }

  function requestStartUnattended() {
    if (!canStartUnattended) return;
    if (tierRequiresDangerAck(authorizationTier)) {
      setDangerAcknowledged(false);
      setPendingDangerAck(true);
      return;
    }
    void submitUnattendedStart();
  }

  async function submitUnattendedStart() {
    if (!canStartUnattended) return;
    if (tierRequiresDangerAck(authorizationTier) && !dangerAcknowledged) return;
    setStartBusy(true);
    try {
      const started = await desktopApi.startUnattendedAutoMode({
        title: goalTitle.trim(),
        objective: goalObjective.trim(),
        steps: planSteps,
        workspaceRoot: workspaceRoot.trim(),
        tier: authorizationTier,
        offline: offlineMode,
        maxTurnsPerBatch,
        reasoningPolicy: startReasoningCapabilities ? reasoningPolicyForChoice(unattendedReasoningChoice) : null,
      });
      setShowStartPanel(false);
      setPendingDangerAck(false);
      setDangerAcknowledged(false);
      setGoalTitle("");
      setGoalObjective("");
      setGoalStepsText("");
      setWorkspaceRoot("");
      setAuthorizationTier("workspace");
      setOfflineMode(true);
      setMaxTurnsPerBatch(8);
      // Keep unattendedReasoningChoice — owner preference is persisted across starts.
      await refreshControlCenter().catch(() => undefined);
      await refreshAwaySummary().catch(() => undefined);
      await publishCompanionStatus("running", started.jobId).catch(() => undefined);
      onNotice(`无人值守目标已启动 · Grant ${started.grantId.slice(0, 8)} · Job ${started.jobId.slice(0, 8)}`);
    } catch (error) {
      onNotice(unattendedStartFailureMessage(error));
    } finally {
      setStartBusy(false);
    }
  }

  useEffect(() => {
    let current = true;
    setProviderStatus(null);
    void desktopApi.agentProviderStatus(providerId).then((status) => {
      if (current) {
        setProviderStatus(status);
        const firstModel = status.models[0];
        if (firstModel && !status.models.some((item) => item.name === model)) {
          setModel(firstModel.name);
        }
      }
    }).catch(() => current && setProviderStatus({
      spec: "nimora.desktop-agent-provider-status/1",
      providerId,
      state: "unavailable",
      workerVerified: false,
      serviceReachable: false,
      locality: "local",
      credentialPresent: false,
      models: [],
      message: providerStatusUnavailableMessage(providerId),
    }));
    return () => { current = false; };
  }, [providerId]);

  async function run() {
    if (!prompt.trim() || busy) return;
    setBusy(true);
    setTurnCancelled(false);
    await publishCompanionStatus("thinking");
    const runningSignal = window.setTimeout(() => {
      void publishCompanionStatus("running");
    }, 450);
    try {
      const next = await desktopApi.runLocalAgent(
        prompt.trim(),
        providerId,
        model.trim(),
        allowNetwork,
        reasoningCapabilities ? reasoningPolicyForChoice(reasoningChoice) : null,
      );
      setResult(next);
      await publishCompanionStatus(next.status === "completed" ? "completed" : "waiting_for_confirmation", next.task.id);
      if (next.status === "completed") await refreshHistory();
      onNotice(next.companionGrowth?.status === "awarded"
        ? `任务已完成，与伙伴的陪伴点 +${next.companionGrowth.bondPointsAwarded}`
        : allowNetwork ? "网络 Agent 任务已完成" : "离线 Agent 任务已完成");
    } catch {
      await publishCompanionStatus("failed");
      onNotice("Agent 任务失败，未执行任何模块操作");
    } finally {
      window.clearTimeout(runningSignal);
      setBusy(false);
    }
  }

  async function resolveAgentTurnTool(invocationId: string, approved: boolean) {
    if (!result || busy || safeMode || recoveryMode) return;
    setBusy(true);
    await publishCompanionStatus("running", result.task.id);
    try {
      if (approved) {
        const next = await desktopApi.confirmAgentRunTool(invocationId);
        setResult(next);
        await publishCompanionStatus(next.status === "completed" ? "completed" : "waiting_for_confirmation", next.task.id);
        if (next.status === "completed") await refreshHistory();
        onNotice(next.companionGrowth?.status === "awarded"
          ? `任务已完成，与伙伴的陪伴点 +${next.companionGrowth.bondPointsAwarded}`
          : next.status === "completed" ? "模块结果已返回 Provider，任务已完成" : "批准已记录，整轮工具仍等待确认");
      } else {
        await desktopApi.rejectAgentTool(invocationId);
        setResult(null);
        setTurnCancelled(true);
        await publishCompanionStatus("cancelled", result.task.id);
        onNotice("已拒绝本轮模块操作，整组调用均未执行");
      }
    } catch {
      setResult(null);
      setTurnCancelled(true);
      await publishCompanionStatus("failed", result.task.id);
      onNotice("Agent 确认已失效，整轮调用未自动重试");
    } finally {
      setBusy(false);
    }
  }

  async function prepareTool(toolId: string) {
    if (toolBusy || safeMode || recoveryMode) return;
    const argumentsValue = toolId === "pet.animation.play" ? { action: "celebrate" } : toolId === "pet.care.perform" ? { action: "feed" } : toolId === "pet.position.move" ? { x: 120, y: 120 } : {};
    setToolBusy(true);
    try {
      const next = await desktopApi.prepareAgentTool(toolId, argumentsValue);
      setToolResult(next);
      onNotice(next.requiresConfirmation ? "模块操作正在等待你的确认" : "只读模块能力已安全完成");
    } catch {
      onNotice("模块能力请求被安全边界拒绝");
    } finally {
      setToolBusy(false);
    }
  }

  async function resolveTool(approved: boolean) {
    if (!toolResult?.requiresConfirmation || toolBusy) return;
    setToolBusy(true);
    try {
      if (approved) {
        const next = await desktopApi.confirmAgentTool(toolResult.invocation.invocationId);
        setToolResult(next);
        onNotice("已确认并通过 Capability Gateway 执行");
      } else {
        await desktopApi.rejectAgentTool(toolResult.invocation.invocationId);
        setToolResult(null);
        onNotice("已拒绝模块操作，未产生副作用");
      }
    } catch {
      setToolResult(null);
      onNotice("确认已失效或执行失败，未自动重试");
    } finally {
      setToolBusy(false);
    }
  }

  async function clearHistory() {
    if (!history?.records.length || busy) return;
    setBusy(true);
    try {
      await desktopApi.deleteAgentHistory();
      await refreshHistory();
      onNotice("Agent 历史已从本机删除");
    } catch {
      onNotice("Agent 历史删除失败，现有记录保持不变");
    } finally {
      setBusy(false);
    }
  }

  const tabs = <nav className="agent-view-tabs" aria-label="Agent 工作区视图">
    {(["run", "control", "providers", "history"] as const).map((item) => <button aria-current={view === item ? "page" : undefined} key={item} onClick={() => setView(item)} type="button">{{ run: "对话运行", control: "目标控制", providers: "模型连接", history: "执行历史" }[item]}</button>)}
  </nav>;

  if (view === "control") return <>
    <header className="agent-section-header">
      <div>
        <p className="card-label">GOAL CONTROL CENTER · SUBJECT</p>
        <h2>指挥活着的伙伴：目标、授权与每一次执行，都有据可查。</h2>
      </div>
      <div className="control-header-actions">
        <span>{controlLoading ? "同步中…" : `${controlCenter?.entries.length ?? 0} 个任务`}</span>
        <button
          className="secondary-button"
          disabled={controlLoading || controlBusy}
          onClick={() => void refreshControlCenter().catch(() => undefined)}
          title={controlLoading ? "正在同步控制中心…" : "从宿主重新拉取 Goal / Job / Grant 事实"}
          type="button"
        >
          {controlLoading ? "同步中…" : "重新同步"}
        </button>
        <button
          className="primary-button"
          disabled={safeMode || recoveryMode || startBusy || !desktopApi.native}
          onClick={openStartPanel}
          title={modeLockReason(safeMode, recoveryMode) ?? (!desktopApi.native ? "浏览器预览不能签发真实授权，请使用桌面端" : "启动无人值守目标并签发绑定 Goal 的执行授权")}
          type="button"
        >
          启动无人值守目标
        </button>
      </div>
    </header>
    {tabs}
    {(safeMode || recoveryMode || !desktopApi.native) && (
      <div className="control-readonly" role="status">
        {!desktopApi.native
          ? "浏览器预览仅展示演示数据：可浏览界面，但不能签发授权、暂停/取消任务或完成对账。请打开 Nimora 桌面端。"
          : modeLockReason(safeMode, recoveryMode)}
      </div>
    )}
    <CompanionStatusStrip bubble={companionBubble} />
    <AwaySummaryPanel
      summary={awaySummary}
      busy={awayBusy}
      error={awayError}
      goalId={awayGoalId}
      goalTitle={awayGoalTitle}
      onRefresh={() => void refreshAwaySummary()}
    />
    {showStartPanel && <section className="unattended-start-panel" aria-labelledby="unattended-start-heading">
      <div className="unattended-start-heading">
        <div>
          <p className="card-label">UNATTENDED EXECUTION GRANT</p>
          <h3 id="unattended-start-heading">启动无人值守目标</h3>
          <p>授权绑定本次 Goal、计划步骤与工作区范围；高风险档位需要二次确认。</p>
        </div>
        <button disabled={startBusy} onClick={() => { setShowStartPanel(false); setPendingDangerAck(false); }} type="button">收起</button>
      </div>
      <div className="unattended-start-grid">
        <label>
          <span>目标标题</span>
          <input aria-label="目标标题" disabled={startBusy} maxLength={120} onChange={(event) => setGoalTitle(event.target.value)} placeholder="例如：夜间编译与回归" value={goalTitle} />
        </label>
        <label>
          <span>目标描述</span>
          <textarea aria-label="目标描述" disabled={startBusy} maxLength={4000} onChange={(event) => setGoalObjective(event.target.value)} placeholder="说明完成标准、约束与验收方式" value={goalObjective} />
        </label>
        <label>
          <span>计划步骤（每行一步）</span>
          <textarea aria-label="计划步骤" disabled={startBusy} maxLength={8000} onChange={(event) => setGoalStepsText(event.target.value)} placeholder={"准备环境\n运行测试\n汇总结果"} value={goalStepsText} />
          <small>{planSteps.length} 个有效步骤</small>
        </label>
        <label>
          <span>工作区路径</span>
          <input aria-label="工作区路径" disabled={startBusy} maxLength={1024} onChange={(event) => setWorkspaceRoot(event.target.value)} placeholder="由宿主校验的本地路径，可留空" value={workspaceRoot} />
        </label>
        <div className="authorization-tier-picker" role="radiogroup" aria-label="授权档位">
          <div className="authorization-tier-picker-label">
            <span>授权档位</span>
            <small>沙箱：{tierSandboxLabel(authorizationTier)} · 审批：{tierApprovalLabel(authorizationTier)}</small>
          </div>
          <div className="authorization-tier-options">
            {AUTHORIZATION_TIERS.map((tier) => {
              const selected = authorizationTier === tier;
              const danger = tierRequiresDangerAck(tier);
              const neverAsk = tierUsesNeverAsk(tier);
              return (
                <label
                  key={tier}
                  className={[
                    "authorization-tier-option",
                    selected ? "is-selected" : "",
                    danger ? "is-danger" : "",
                    neverAsk ? "is-never-ask" : "",
                    tier === "full_device" ? "is-full-device" : "",
                  ].filter(Boolean).join(" ")}
                >
                  <input
                    aria-label={`授权档位：${tierLabel(tier)}`}
                    checked={selected}
                    disabled={startBusy}
                    name="authorization-tier"
                    onChange={() => {
                      setAuthorizationTier(tier);
                      setDangerAcknowledged(false);
                      setPendingDangerAck(false);
                    }}
                    type="radio"
                    value={tier}
                  />
                  <span className="authorization-tier-option-body">
                    <strong>{tierLabel(tier)}</strong>
                    <em>{tierPickerHint(tier)}</em>
                    <small>{tierApprovalLabel(tier)} · {tierSandboxLabel(tier)}</small>
                  </span>
                </label>
              );
            })}
          </div>
        </div>
        <div className={`tier-policy-card${tierUsesNeverAsk(authorizationTier) ? " never-ask" : ""}${authorizationTier === "full_device" ? " full-device" : ""}`} role="note">
          <strong>{tierLabel(authorizationTier)}</strong>
          <p>{tierRiskSummary(authorizationTier)}</p>
          {tierUsesNeverAsk(authorizationTier) && (
            <p className="never-ask-callout">
              审批策略为 <b>NeverAskWithinGrant（Grant 内免确认）</b>
              {authorizationTier === "full_device" ? "，在整机范围内可自动派发写操作。" : "，在授权范围内可自动派发，无需逐步弹窗。"}
              {" "}{tierSleepSafeCopy(authorizationTier)}
            </p>
          )}
        </div>
        {startReasoningCapabilities && <label>
          <span>推理策略</span>
          <select
            aria-label="推理策略"
            disabled={startBusy}
            onChange={(event) => {
              const next = event.target.value as ReasoningChoice;
              setUnattendedReasoningChoice(next);
              saveReasoningChoice(UNATTENDED_REASONING_CHOICE_STORAGE_KEY, next);
            }}
            value={coerceReasoningChoice(unattendedReasoningChoice, startReasoningCapabilities.supportedEfforts)}
          >
            <option value="adaptive">自适应 · 均衡</option>
            <option value="cost_saver">节省 · 更快</option>
            <option value="quality_first">极致 · 最深入</option>
            {startReasoningCapabilities.supportedEfforts.map((effort) => <option key={effort} value={`fixed:${effort}`}>{({ minimal: "最少", low: "轻量", medium: "均衡", high: "深入", very_high: "很深入", maximum: "极致" } as const)[effort]} · 固定</option>)}
          </select>
          <small>无人值守推理偏好会跨启动记住。</small>
        </label>}
        <label className="unattended-toggle">
          <input checked={offlineMode} disabled={startBusy} onChange={(event) => setOfflineMode(event.target.checked)} type="checkbox" />
          <span><strong>离线优先</strong><small>默认不外发任务内容；联网仍受 Grant 与策略约束。</small></span>
        </label>
        <label>
          <span>每批最大轮次</span>
          <input aria-label="每批最大轮次" disabled={startBusy} max={64} min={1} onChange={(event) => setMaxTurnsPerBatch(Math.max(1, Math.min(64, Number(event.target.value) || 1)))} type="number" value={maxTurnsPerBatch} />
        </label>
      </div>
      {authorizationTier === "trusted_workspace" && (
        <div className="unattended-risk-note never-ask" role="note">
          <strong>信任工作区启用 NeverAskWithinGrant</strong>
          <p>
            睡眠/离开安全：工作区内自动放行，不会反复弹窗打断。硬禁区与组织策略仍不可绕过；醒来可在授权列表撤销。
            适合已信任项目的长任务，不适合陌生仓库。
          </p>
        </div>
      )}
      {tierRequiresDangerAck(authorizationTier) && <div className={`unattended-risk-note${authorizationTier === "full_device" ? " full-device" : ""}`} role="note">
        <strong>{authorizationTier === "full_device" ? "完全设备访问" : "无人值守"} 属于高风险授权</strong>
        <p>
          {authorizationTier === "full_device"
            ? "整机沙箱 + NeverAskWithinGrant：在有效期内可自动读写本机可达路径，并可能联网（若关闭离线优先）。"
            : "选定根目录 + NeverAskWithinGrant：在有效期内可在授权根路径内自动执行，无需逐步确认。"}
          提交前会要求二次确认；硬禁区仍不可绕过。
        </p>
        <ul className="danger-risk-list compact" aria-label="启用前风险摘要">
          {dangerRiskItems(authorizationTier).slice(0, 4).map((risk) => <li key={risk}>{risk}</li>)}
        </ul>
      </div>}
      {authorizationTier === "full_device" && (
        <div className="full-device-danger-matrix" aria-label="完全设备访问风险矩阵" role="region">
          <header>
            <strong>完全设备访问 · 风险矩阵</strong>
            <span>确认前请逐项阅读</span>
          </header>
          <table>
            <thead>
              <tr><th scope="col">能力</th><th scope="col">范围</th><th scope="col">风险</th></tr>
            </thead>
            <tbody>
              {FULL_DEVICE_DANGER_MATRIX.map((row) => (
                <tr key={row.capability}>
                  <th scope="row">{row.capability}</th>
                  <td>{row.scope}</td>
                  <td><span className="matrix-risk-chip">{row.risk}</span></td>
                </tr>
              ))}
            </tbody>
          </table>
          <ul className="danger-risk-list" aria-label="风险清单">
            {dangerRiskItems(authorizationTier).map((risk) => <li key={risk}>{risk}</li>)}
          </ul>
        </div>
      )}
      <div className="unattended-start-actions">
        <button disabled={startBusy} onClick={() => { setShowStartPanel(false); setPendingDangerAck(false); }} type="button">取消</button>
        <button
          className="primary-button"
          disabled={!canStartUnattended}
          onClick={requestStartUnattended}
          title={
            unattendedStartLockReason({
              safeMode,
              recoveryMode,
              startBusy,
              title: goalTitle,
              objective: goalObjective,
              stepCount: planSteps.length,
              native: desktopApi.native,
            }) ?? "签发绑定 Goal 的执行授权并启动"
          }
          type="button"
        >
          {startBusy ? "启动中…" : tierRequiresDangerAck(authorizationTier) ? "核对风险并启动" : "启动并签发授权"}
        </button>
      </div>
    </section>}
    <section className="authorization-grants-panel" aria-label="授权 Grant 列表">
      <header>
        <div>
          <p className="card-label">AUTHORIZATION GRANTS</p>
          <h3>有效与历史授权</h3>
          <p className="authorization-grants-lead">档位徽章、NeverAskWithinGrant 策略与撤销入口一目了然。高风险档位可随时一键撤回。</p>
        </div>
        <div className="authorization-grants-header-meta">
          <span>{countActiveGrants(authorizationGrants)} 个有效 · {authorizationGrants.length} 条记录</span>
          {countActiveGrants(authorizationGrants) > 0 ? (
            <button
              aria-label="撤销全部有效授权"
              className="grant-revoke-all"
              disabled={Boolean(controlLockReason)}
              onClick={() => void revokeAllActiveGrants()}
              title={controlLockReason ?? "立即撤销全部有效授权，阻止后续自动派发"}
              type="button"
            >
              撤销全部有效
            </button>
          ) : null}
        </div>
      </header>
      {authorizationGrants.length > 0 ? (
        <ul>
          {sortAuthorizationGrants(authorizationGrants).map((grant) => (
            <li key={grant.grantId}>
              <div>
                <span className={grantBadgeClass(grant)} data-tier={grant.tier} title={tierRiskSummary(grant.tier)}>{grantStatusLabel(grant)}</span>
                <strong>{tierLabel(grant.tier)}</strong>
                <small>{tierApprovalLabel(grant.tier)} · {tierSandboxLabel(grant.tier)}</small>
                {tierUsesNeverAsk(grant.tier) && grant.status === "active" && (
                  <span className="grant-policy-chip" title={tierSleepSafeCopy(grant.tier)}>NeverAskWithinGrant · 睡眠安全</span>
                )}
                <span className="grant-goal-chip" title={grant.goalId}>Goal · {shortGoalId(grant.goalId)}</span>
                {grant.workspaceRoot && <code title={grant.workspaceRoot}>{grant.workspaceRoot}</code>}
                <em>Grant {grant.grantId.slice(0, 10)}</em>
                <span className="grant-expiry">{grantExpiryLabel(grant)}</span>
              </div>
              {grant.status === "active" && (
                <button
                  aria-label={`撤销 ${tierLabel(grant.tier)} 授权 ${grant.grantId.slice(0, 8)}`}
                  disabled={safeMode || recoveryMode || controlBusy}
                  onClick={() => void revokeGrant(grant.grantId)}
                  type="button"
                >
                  撤销
                </button>
              )}
            </li>
          ))}
        </ul>
      ) : (
        <div className="authorization-grants-empty" role="status">
          <strong>还没有授权 Grant</strong>
          <p>下一步：启动无人值守目标时会签发绑定 Goal 的执行授权。高风险档位会先展示 NeverAskWithinGrant 风险清单，确认后才生效；可随时在此撤销。</p>
          <button
            className="primary-button"
            disabled={safeMode || recoveryMode || startBusy || !desktopApi.native}
            onClick={openStartPanel}
            title={modeLockReason(safeMode, recoveryMode) ?? (!desktopApi.native ? "请在桌面端签发授权" : "启动目标并签发授权")}
            type="button"
          >
            启动目标并签发授权
          </button>
        </div>
      )}
    </section>
    <section className="control-center" aria-label="目标控制中心">
      {controlCenter?.entries.length ? controlCenter.entries.map((entry) => {
        const grant = entry.grant ?? null;
        const grantActive = grant?.status === "active";
        const statusKey = entry.attempt?.status === "indeterminate" ? "indeterminate" : entry.effectiveStatus;
        const budgetSlices = controlBudgetSlices(entry.session);
        const budgetFill = controlBudgetFillRatio(budgetSlices);
        const thinkingEffort = extractCheckpointReasoning(entry.checkpoint);
        const contextUsage = deriveContextBudgetUsage({
          usage: entry.session.usage,
          budget: entry.session.policy.budget,
          checkpoint: entry.checkpoint,
          cacheHits: entry.job.cacheHits,
        });
        const contextLine = formatContextBudget(contextUsage);
        const workspaceSnap = deriveWorkspaceTrackingSnapshot({
          workspaceRoot: grant?.workspaceRoot ?? null,
          workspaceRevision: entry.checkpoint?.workspaceRevision ?? entry.session.policy.workspaceRevision,
          checkpoint: entry.checkpoint,
        });
        const workspaceLine = workspaceTrackingSummary(workspaceSnap);
        const showWorkspaceTrack = hasWorkspaceTrackingSignal(workspaceSnap);
        const jobStatus = entry.attempt?.status === "indeterminate" ? "indeterminate" : entry.job.status;
        const pauseReason = entry.job.pauseReason ?? entry.session.pauseReason;
        const companionPhase = autoModePhaseLabel(jobStatus, {
          pauseReason,
          indeterminate: entry.attempt?.status === "indeterminate",
        });
        const companionPhaseBubble = companionBubbleFromAutoMode(jobStatus, {
          pauseReason,
          indeterminate: entry.attempt?.status === "indeterminate",
        });
        return (
        <article className={entry.job.status === "indeterminate" ? "control-entry risk" : "control-entry"} key={entry.job.jobId}>
          <header>
            <div>
              <div className="control-status-row">
                <span className="control-status-chip" data-status={statusKey}>{controlEffectiveStatusLabel(statusKey)}</span>
                <span className="control-goal-chip" data-status={entry.goal.status} title={`Goal ${entry.goal.id}`}>目标 · {goalStatusLabel(entry.goal.status)}</span>
                <span
                  className={`control-companion-chip tone-${companionPhaseBubble.tone}`}
                  data-status={companionPhaseBubble.status}
                  title={`${companionPhaseBubble.message} · ${companionPhaseBubble.actionLabel} · ${companionPhaseBubble.moodLabel}`}
                >
                  伙伴 · {companionPhase}
                </span>
                {thinkingEffort ? <span className="control-effort-chip" title="思考强度">思考 · {thinkingEffort}</span> : null}
              </div>
              <h3>{entry.goal.title}</h3>
              <p>{entry.goal.objective}</p>
            </div>
            <div className="control-entry-status">
              <strong>Plan r{entry.plan.revision}</strong>
              <span className={grantBadgeClass(grant)} title={grant ? `Grant ${grant.grantId}` : "无授权"}>{grantStatusLabel(grant)}</span>
            </div>
          </header>
          {entry.projectionStale && <p className="projection-warning" role="status">运行投影正在收敛；此处以持久化 Session 事实为准，不会自动重试外部操作。</p>}
          <div className="control-metrics" aria-label="运行指标">
            <div><small>进度</small><b>{entry.session.usage.cycles} / {entry.session.policy.maxCycles}</b></div>
            <div><small>检查点</small><b>#{entry.checkpoint?.sequence ?? 0}</b></div>
            <div><small>缓存命中</small><b>{entry.job.cacheHits}</b></div>
            <div><small>模型</small><b>{entry.checkpoint?.model ?? "待启动"}</b></div>
            {thinkingEffort ? <div><small>思考强度</small><b>{thinkingEffort}</b></div> : null}
          </div>
          <div className="control-budget-panel" aria-label="预算使用">
            <div className="control-budget" title={`预算占用 ${Math.round(budgetFill * 100)}%`}>
              <span style={{ width: `${Math.round(budgetFill * 100)}%` }} />
            </div>
            <ul className="control-budget-slices">
              {budgetSlices.map((slice) => (
                <li key={slice.label} data-tight={slice.ratio >= 0.85 ? "true" : undefined}>
                  <small>{slice.label}</small>
                  <b>{slice.display}</b>
                </li>
              ))}
            </ul>
          </div>
          <div className="control-context-strip" aria-label="上下文">
            <span className="control-strip-label">上下文</span>
            <p>{contextLine}</p>
          </div>
          {showWorkspaceTrack ? (
            <div className="control-workspace-strip" aria-label="工作区追踪">
              <span className="control-strip-label">工作区追踪</span>
              <p>{workspaceLine}</p>
              {workspaceSnap.workspaceRoot ? <code title={workspaceSnap.workspaceRoot}>{workspaceSnap.workspaceRoot}</code> : null}
              {!workspaceSnap.workspaceRoot && workspaceSnap.workspaceRevision ? <code title={workspaceSnap.workspaceRevision}>{workspaceSnap.workspaceRevision}</code> : null}
            </div>
          ) : null}
          <ol aria-label="计划步骤">{entry.plan.steps.map((step) => <li key={step.id} data-status={step.status}><i /><span>{step.description}</span><em>{planStepStatusLabel(step.status)}</em></li>)}</ol>
          <footer><code>{entry.session.policy.workspaceRevision}</code><span>{pauseReasonLabel(entry.job.pauseReason ?? entry.session.pauseReason)}</span></footer>
          {grant && <div className="control-grant-meta"><span>授权 · {tierLabel(grant.tier)}</span><span className="grant-policy-chip">{tierApprovalLabel(grant.tier)}</span>{tierUsesNeverAsk(grant.tier) && grantActive ? <span className="grant-policy-chip never-ask" title={tierSleepSafeCopy(grant.tier)}>NeverAsk · 睡眠安全</span> : null}{grant.workspaceRoot && <code>{grant.workspaceRoot}</code>}{grantActive && <button disabled={Boolean(controlLockReason)} onClick={() => void revokeGrant(grant.grantId)} title={controlLockReason ?? "立即撤销此授权，阻止后续自动派发"} type="button">撤销授权</button>}</div>}
          {entry.attempt?.status === "indeterminate" && <div className="control-risk" role="alert"><strong>外部执行结果未知，禁止自动重试</strong><p>Attempt {entry.attempt.id} · Checkpoint #{entry.attempt.checkpointSequence}</p><code>{entry.attempt.requestFingerprint}</code><p className="control-risk-next">下一步：根据你掌握的外部证据，二选一完成对账；理由会永久写入审计。</p><div><button disabled={Boolean(controlLockReason)} onClick={() => prepareControl({ kind: "resolve", entry, decision: "confirmed_not_executed" })} title={controlLockReason ?? "确认外部操作未执行，并安全暂停"} type="button">确认未执行并暂停</button><button disabled={Boolean(controlLockReason)} onClick={() => prepareControl({ kind: "resolve", entry, decision: "accept_external_effect_and_cancel" })} title={controlLockReason ?? "接受可能已产生的副作用并取消任务"} type="button">接受副作用并取消</button></div></div>}
          {!entry.attempt && ["starting", "running"].includes(entry.job.status) && <div className="control-actions"><button disabled={Boolean(controlLockReason)} onClick={() => prepareControl({ kind: "pause", entry })} title={controlLockReason ?? "在当前原子步骤结束后暂停，保留 Checkpoint"} type="button">暂停</button><button disabled={Boolean(controlLockReason)} onClick={() => prepareControl({ kind: "cancel", entry })} title={controlLockReason ?? "取消不可恢复为同一运行；已产生副作用不会自动回滚"} type="button">取消任务</button></div>}
          {entry.resolutions.length > 0 && <details className="resolution-history"><summary>不可变对账记录 · {entry.resolutions.length}</summary>{entry.resolutions.map((resolution) => <article key={resolution.id}><strong>{resolution.decision}</strong><p>{resolution.reason}</p><small>{resolution.actor} · {new Date(resolution.resolvedAtMs).toLocaleString("zh-CN")}</small></article>)}</details>}
        </article>
        );
      }) : controlLoading ? (
        <div className="control-empty control-loading" role="status" aria-live="polite">
          <strong>正在同步目标控制中心…</strong>
          <p>读取 Goal、Job、Session、Checkpoint 与授权事实，请稍候。</p>
        </div>
      ) : controlError ? (
        <div className="control-empty control-error" role="alert">
          <strong>控制中心暂时连不上</strong>
          <p>{controlError}</p>
          <button
            className="primary-button"
            disabled={controlLoading || controlBusy}
            onClick={() => void refreshControlCenter().catch(() => undefined)}
            type="button"
          >
            {controlLoading ? "重试中…" : "重新同步"}
          </button>
        </div>
      ) : (
        <div className="control-empty">
          <strong>伙伴还没有执行中的目标</strong>
          <p>
            下一步：点「启动无人值守目标」，填写标题、完成标准与计划步骤，选择授权档位后签发 Grant。
            控制中心只展示宿主持久化并验证过的事实；浏览器预览使用明确的演示数据。
          </p>
          <button
            className="primary-button"
            disabled={safeMode || recoveryMode || startBusy || !desktopApi.native}
            onClick={openStartPanel}
            title={modeLockReason(safeMode, recoveryMode) ?? (!desktopApi.native ? "请在桌面端启动" : "启动第一个无人值守目标")}
            type="button"
          >
            启动第一个无人值守目标
          </button>
          {(safeMode || recoveryMode || !desktopApi.native) && (
            <p className="control-empty-lock">{modeLockReason(safeMode, recoveryMode) ?? "浏览器预览不能签发真实授权。"}</p>
          )}
        </div>
      )}
    </section>
    {pendingControl && <div className="control-dialog-backdrop"><section aria-labelledby="control-dialog-title" aria-modal="true" className={pendingControl.kind === "pause" ? "control-dialog" : "control-dialog danger"} role="dialog"><p className="card-label">参数绑定确认</p><h3 id="control-dialog-title">{pendingControl.kind === "pause" ? "暂停这个任务？" : pendingControl.kind === "cancel" ? "取消这个任务？" : pendingControl.decision === "confirmed_not_executed" ? "确认外部操作未执行？" : "接受潜在副作用并取消？"}</h3><p>{pendingControl.kind === "pause" ? "任务会在当前原子步骤结束后暂停，不会丢弃 Checkpoint。" : pendingControl.kind === "cancel" ? "取消不可恢复为同一个运行；已产生的外部副作用不会自动回滚。" : "此决议将绑定当前 Attempt、Checkpoint 与请求指纹，并永久写入审计记录。"}</p><dl><div><dt>Goal</dt><dd>{pendingControl.entry.goal.title}</dd></div><div><dt>Session</dt><dd><code>{pendingControl.entry.session.id}</code></dd></div>{pendingControl.kind === "resolve" && <div><dt>Attempt</dt><dd><code>{pendingControl.entry.attempt?.id}</code></dd></div>}</dl>{pendingControl.kind === "resolve" && <label><span>对账理由（必填）</span><textarea autoFocus maxLength={2048} onChange={(event) => setControlReason(event.target.value)} placeholder="说明你核对了什么证据，以及为什么选择此决议" value={controlReason} /></label>}<div><button disabled={controlBusy} onClick={() => setPendingControl(null)} type="button">返回检查</button><button className="primary-button" disabled={controlBusy || (pendingControl.kind === "resolve" && !controlReason.trim())} onClick={() => void executeControl()} type="button">{controlBusy ? "提交中…" : "确认提交"}</button></div></section></div>}
    {pendingDangerAck && <div className="control-dialog-backdrop"><section aria-labelledby="danger-grant-title" aria-describedby="danger-grant-desc" aria-modal="true" className={`control-dialog danger${authorizationTier === "full_device" ? " full-device" : ""}`} role="dialog">
      <p className="card-label">HIGH RISK AUTHORIZATION</p>
      <h3 id="danger-grant-title">{authorizationTier === "full_device" ? "确认完全设备访问？" : "确认无人值守授权？"}</h3>
      <p id="danger-grant-desc">
        审批策略：<strong>{tierApprovalLabel(authorizationTier)}</strong>
        （NeverAskWithinGrant · 睡眠/离开安全）· 沙箱：{tierSandboxLabel(authorizationTier)}。
        Grant 有效期内不会反复弹窗打断；以下风险会绑定到本次 Goal 的执行范围，且仍无法绕过产品硬禁区与组织策略。随时可在授权列表撤销。
      </p>
      <p className="never-ask-dialog-callout">{tierRiskSummary(authorizationTier)}</p>
      <p className="never-ask-dialog-sleep" role="note">{tierSleepSafeCopy(authorizationTier)}</p>
      {authorizationTier === "full_device" && (
        <div className="full-device-danger-matrix in-dialog" aria-label="完全设备访问风险矩阵">
          <table>
            <thead>
              <tr><th scope="col">能力</th><th scope="col">范围</th><th scope="col">风险</th></tr>
            </thead>
            <tbody>
              {FULL_DEVICE_DANGER_MATRIX.map((row) => (
                <tr key={row.capability}>
                  <th scope="row">{row.capability}</th>
                  <td>{row.scope}</td>
                  <td><span className="matrix-risk-chip">{row.risk}</span></td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <ul className="danger-risk-list">{dangerRiskItems(authorizationTier).map((risk) => <li key={risk}>{risk}</li>)}</ul>
      <dl>
        <div><dt>档位</dt><dd>{tierLabel(authorizationTier)}</dd></div>
        <div><dt>Goal</dt><dd>{goalTitle.trim() || "未命名目标"}</dd></div>
        <div><dt>工作区</dt><dd>{workspaceRoot.trim() || "由宿主判定"}</dd></div>
      </dl>
      <label className="danger-ack">
        <input aria-required="true" checked={dangerAcknowledged} disabled={startBusy} onChange={(event) => setDangerAcknowledged(event.target.checked)} type="checkbox" />
        <span>我理解 NeverAskWithinGrant 与上述风险，并授权本次 Goal 绑定的执行范围</span>
      </label>
      <div>
        <button disabled={startBusy} onClick={() => { setPendingDangerAck(false); setDangerAcknowledged(false); }} type="button">返回修改</button>
        <button className="primary-button" disabled={startBusy || !dangerAcknowledged} onClick={() => void submitUnattendedStart()} type="button">{startBusy ? "启动中…" : "确认并启动"}</button>
      </div>
    </section></div>}
  </>;

  if (view === "history") return <><header className="agent-section-header"><div><p className="card-label">LOCAL EXECUTION HISTORY</p><h2>本机记录，清晰、私密、可删除。</h2></div></header>{tabs}<section className="history-page" aria-labelledby="history-page-heading"><div><h3 id="history-page-heading">最近完成</h3><button disabled={busy || !history?.records.length} onClick={() => void clearHistory()} type="button">清除全部</button></div>{history?.records.length ? history.records.map((record) => <article key={record.task.id}><strong>{record.prompt}</strong><p>{record.response || "任务已完成，无文本回答"}</p><small>{record.model} · {record.usage.inputTokens + record.usage.outputTokens} tokens · {new Date(record.completedAtMs).toLocaleString("zh-CN")}</small></article>) : <p>完成一次任务后，记录会安全保存在本机。</p>}</section></>;

  if (view === "providers") return <>{tabs}<Suspense fallback={<div className="provider-page-loading" role="status">正在载入安全 Provider 管理器…</div>}><ProviderSettings disabled={safeMode || recoveryMode} onCatalogChanged={() => void refreshCatalog()} onNotice={onNotice} /></Suspense></>;

  return <>{tabs}<CompanionStatusStrip bubble={companionBubble} /><section className="agent-workspace" aria-labelledby="agent-heading">
    <div className="agent-main">
      <header className="agent-hero">
        <div>
          <p className="card-label">LOCAL AGENT WORKSPACE</p>
          <h2 id="agent-heading">把想法交给 Nimora，也看清每一步。</h2>
          <p>本地 Provider 可离线运行；无论选择哪种模型，模块操作都只能经过工具目录、风险确认与 Capability Gateway。</p>
        </div>
        <span className="agent-online"><i /> 本地离线</span>
      </header>

      <div className="conversation-surface">
        <div className="agent-message system-message"><span>✦</span><div><strong>Nimora Agent</strong><p>我不会直接调用内部模块。任何动作都会先展示实际参数和风险。</p></div></div>
        {result?.status === "completed" && <div className="agent-message response-message"><span>AI</span><div><strong>任务已完成</strong><p>{result.content}</p><small>{agentUsageTotal(result)} tokens · ¥0 · {result.task.status}</small></div></div>}
        {result?.status === "waitingForConfirmation" && <div className="agent-turn-request">
          <div className="turn-request-heading"><span>!</span><div><strong>Agent 请求模块操作</strong><p>整组全部批准前不会执行任何写操作；拒绝任意一项会取消整组。</p></div></div>
          <div className="turn-tool-list">{result.pendingTools.map((tool) => <article key={tool.invocation.invocationId}>
            <div className="turn-tool-meta"><span>{agentRiskLabel(tool.effectiveRisk)}</span><code>{tool.invocation.toolId}</code></div>
            <pre>{JSON.stringify(tool.invocation.arguments, null, 2)}</pre>
            <small>{tool.expiresAtMs ? `确认有效至 ${new Date(tool.expiresAtMs).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" })}` : "等待确认"}</small>
            <div className="turn-tool-actions"><button disabled={busy || safeMode || recoveryMode} onClick={() => void resolveAgentTurnTool(tool.invocation.invocationId, false)} type="button">拒绝整组</button><button className="primary-button" disabled={busy || safeMode || recoveryMode} onClick={() => void resolveAgentTurnTool(tool.invocation.invocationId, true)} type="button">批准此项</button></div>
          </article>)}</div>
        </div>}
        {turnCancelled && <div className="agent-message cancelled-message"><span>×</span><div><strong>本轮操作已取消</strong><p>模块未收到这组写操作，Provider 也不会收到不完整的 Tool Result。</p></div></div>}
      </div>

      <form className="agent-composer" onSubmit={(event) => { event.preventDefault(); void run(); }}>
        <div className="agent-runtime-controls">
          <label><span>Provider</span><select aria-label="Agent Provider" disabled={busy} value={providerId} onChange={(event) => { const nextProviderId = event.target.value; setProviderId(nextProviderId); setModel(defaultModelForProvider(nextProviderId)); setAllowNetwork(false); }}>{catalog?.providers.map((provider) => <option key={provider.id} value={provider.id}>{provider.displayName}</option>)}</select></label>
          <label><span>模型</span><input aria-label="Agent 模型" disabled={busy} list="agent-provider-models" maxLength={128} value={model} onChange={(event) => setModel(event.target.value)} /><datalist id="agent-provider-models">{providerStatus?.models.map((item) => <option key={item.name} value={item.name} />)}</datalist></label>
          {reasoningCapabilities && <label><span>思考强度</span><select aria-label="Agent 思考强度" disabled={busy} value={coerceReasoningChoice(reasoningChoice, reasoningCapabilities.supportedEfforts)} onChange={(event) => { const next = event.target.value as ReasoningChoice; setReasoningChoice(next); saveReasoningChoice(REASONING_CHOICE_STORAGE_KEY, next); }}><option value="adaptive">自动 · 均衡</option><option value="cost_saver">节省 · 更快</option><option value="quality_first">极致 · 最深入</option>{reasoningCapabilities.supportedEfforts.map((effort) => <option key={effort} value={`fixed:${effort}`}>{({ minimal: "最少", low: "轻量", medium: "均衡", high: "深入", very_high: "很深入", maximum: "极致" } as const)[effort]} · 固定</option>)}</select><small>选择会记住；固定等级不自动降级，能力映射由宿主验证。</small></label>}
        </div>
        {activeProvider?.locality === "network" && <label className="agent-network-consent"><input checked={allowNetwork} disabled={busy} type="checkbox" onChange={(event) => setAllowNetwork(event.target.checked)} /><span><strong>允许本次请求联网</strong><small>任务内容将发送到你配置的第三方服务；模块能力仍受本机网关约束。</small></span></label>}
        <textarea value={prompt} maxLength={32768} onChange={(event) => setPrompt(event.target.value)} aria-label="Agent 任务内容" />
        <div><span>{providerStatus?.message ?? "正在检查 Provider"}</span><button className="primary-button" disabled={busy || !prompt.trim() || !model.trim() || providerStatus?.state !== "ready" || !providerStatus.models.some((item) => item.name === model) || (activeProvider?.locality === "network" && !allowNetwork)} type="submit">{busy ? "运行中…" : "运行任务"}</button></div>
      </form>
    </div>

    <aside className="agent-inspector" aria-label="Agent 运行检查器">
      <div className="inspector-title"><div><p className="card-label">运行检查器</p><h3>能力与边界</h3></div><span>{catalog?.tools.length ?? 0} tools</span></div>
      <div className="provider-tile"><span className="provider-glyph">⌁</span><div><strong>{activeProvider?.displayName ?? activeProviderId}</strong><p>{activeProvider?.locality === "network" ? "网络 Provider · 受数据策略约束" : "本地 Provider · 可离线运行"}{reasoningCapabilities ? ` · ${reasoningCapabilities.supportedEfforts.length} 档推理` : " · 标准推理"}</p></div><i>{providerStatusLabel(providerStatus)}</i></div>
      <div className="boundary-note"><strong>{safeMode ? "安全模式已启用" : recoveryMode ? "恢复模式已启用" : "确认策略正常"}</strong><p>写操作与外部副作用始终要求绑定实际参数的批准。</p></div>
      <div className="tool-catalog"><p className="card-label">伙伴能力 · 模块工具</p>{catalog?.tools.map((tool) => <article key={tool.id}><span>{tool.effect === "read_only" ? "R" : "W"}</span><div><strong>{tool.title}</strong><code>{tool.id}</code><em className="tool-domain-chip" data-domain={agentToolDomainLabel(tool.id)}>{agentToolDomainLabel(tool.id)}</em></div><button className={tool.effect === "read_only" ? "read-only" : "approval"} disabled={toolBusy || safeMode || recoveryMode} onClick={() => void prepareTool(tool.id)} type="button">{agentToolAccessLabel(tool.effect)}</button></article>)}</div>
      <section className="agent-history" aria-labelledby="agent-history-heading">
        <div><div><p className="card-label">本机历史</p><h4 id="agent-history-heading">最近完成</h4></div><button disabled={busy || !history?.records.length} onClick={() => void clearHistory()} type="button">全部清除</button></div>
        {history?.historyDegraded && <p className="history-warning">最近一次历史写入失败；任务结果不受影响。</p>}
        {history?.records.length ? history.records.map((record) => <article key={record.task.id}><strong>{record.prompt}</strong><p>{record.response || "任务已完成，无文本回答"}</p><small>{record.model} · {record.usage.inputTokens + record.usage.outputTokens} tokens</small></article>) : <p className="history-empty">完成一次任务后，记录会保存在本机。</p>}
      </section>
      {toolResult?.requiresConfirmation && <section className="tool-confirmation" aria-labelledby="tool-confirmation-heading">
        <p className="card-label">参数绑定确认</p>
        <h4 id="tool-confirmation-heading">允许这次模块操作？</h4>
        <code>{toolResult.invocation.toolId}</code>
        <pre>{JSON.stringify(toolResult.invocation.arguments, null, 2)}</pre>
        <p>风险：{toolResult.effectiveRisk} · 仅本次 Invocation 有效 · 5 分钟后失效</p>
        <div><button className="secondary-button" disabled={toolBusy} onClick={() => void resolveTool(false)} type="button">拒绝</button><button className="primary-button" disabled={toolBusy} onClick={() => void resolveTool(true)} type="button">确认执行</button></div>
      </section>}
      {toolResult && !toolResult.requiresConfirmation && <div className="tool-complete" role="status"><strong>Gateway 执行完成</strong><p>{toolResult.invocation.toolId}</p></div>}
      {result?.usage && <div className="usage-card"><p className="card-label">最近任务</p><dl><div><dt>输入</dt><dd>{result.usage.inputTokens}</dd></div><div><dt>输出</dt><dd>{result.usage.outputTokens}</dd></div><div><dt>费用</dt><dd>0</dd></div></dl></div>}
    </aside>
  </section></>;
}
