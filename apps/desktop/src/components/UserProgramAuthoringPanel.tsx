import { useCallback, useMemo, useState } from "react";
import {
  desktopApi,
  type ProgramPolicyReport,
  type UserCodeCapability,
  type UserProgramExecutionReceipt,
  type UserProgramManifest,
} from "../platform/desktop";

const MAX_SUBSCRIPTIONS = 32;
const MAX_COMMANDS = 32;
const MAX_RUNTIME_MS = 30_000;
const MAX_MEMORY_BYTES = 64 * 1024 * 1024;
const MAX_EVENT_QUEUE_CAPACITY = 64;

export const USER_CODE_CAPABILITIES: readonly UserCodeCapability[] = [
  "read-pet-state",
  "read-profile-state",
  "subscribe-events",
  "invoke-safe-commands",
  "invoke-agent-tasks",
  "store-local-data",
];

export const capabilityLabels: Record<UserCodeCapability, string> = {
  "read-pet-state": "读取宠物状态",
  "read-profile-state": "读取档案状态",
  "subscribe-events": "订阅事件",
  "invoke-safe-commands": "调用安全命令",
  "invoke-agent-tasks": "调用 Agent 任务",
  "store-local-data": "本地存储",
};

const capabilityHints: Record<UserCodeCapability, string> = {
  "read-pet-state": "只读宠物情绪、注意力与位置快照。",
  "read-profile-state": "只读当前档案与陪伴成长数据。",
  "subscribe-events": "接收桌面事件流，必须搭配订阅列表。",
  "invoke-safe-commands": "调用 safe.* 前缀的安全命令，必须搭配命令列表。",
  "invoke-agent-tasks": "委派本地 AI Agent 任务（高风险，将进入审查）。",
  "store-local-data": "在隔离沙箱内读写本程序的本地键值。",
};

export interface ManifestDraft {
  id: string;
  version: string;
  capabilities: UserCodeCapability[];
  subscriptionsText: string;
  commandsText: string;
  eventConcurrency: UserProgramManifest["eventConcurrency"];
  eventQueueCapacity: number;
  timeoutMs: number;
  memoryMiB: number;
}

export function defaultManifestDraft(): ManifestDraft {
  return {
    id: "studio.example.hello",
    version: "1.0.0",
    capabilities: ["read-pet-state"],
    subscriptionsText: "",
    commandsText: "",
    eventConcurrency: "serial",
    eventQueueCapacity: 8,
    timeoutMs: 5_000,
    memoryMiB: 8,
  };
}

export const defaultProgramSource = `// 用户程序运行在隔离沙箱中，可访问已授权的能力。
// 通过 return 一个对象来声明本次执行的能力调用。
export default async function main(ctx) {
  const pet = await ctx.readPetState();
  return { mood: pet?.mood ?? "unknown" };
}
`;

function splitLines(value: string): string[] {
  return value
    .split(/[\n,]/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
}

function isNamespacedId(value: string): boolean {
  const segments = value.split(".");
  if (segments.length < 3) return false;
  return segments.every(
    (segment) => segment.length > 0 && /^[a-z0-9-]+$/.test(segment),
  );
}

function isSemver(value: string): boolean {
  const parts = value.split(".");
  return parts.length === 3 && parts.every((part) => /^[0-9]+$/.test(part));
}

export function validateManifestDraft(draft: ManifestDraft): string[] {
  const errors: string[] = [];
  if (!isNamespacedId(draft.id)) {
    errors.push("程序 ID 必须是至少三段的小写命名空间标识（如 studio.example.hello）。");
  }
  if (!isSemver(draft.version)) {
    errors.push("版本号必须是三段纯数字的语义化版本（如 1.0.0）。");
  }

  const subscriptions = splitLines(draft.subscriptionsText);
  const commands = splitLines(draft.commandsText);

  if (subscriptions.length > MAX_SUBSCRIPTIONS) {
    errors.push(`订阅事件不能超过 ${MAX_SUBSCRIPTIONS} 个。`);
  }
  if (commands.length > MAX_COMMANDS) {
    errors.push(`安全命令不能超过 ${MAX_COMMANDS} 个。`);
  }
  if (draft.eventQueueCapacity < 1 || draft.eventQueueCapacity > MAX_EVENT_QUEUE_CAPACITY) {
    errors.push(`事件队列容量必须在 1 到 ${MAX_EVENT_QUEUE_CAPACITY} 之间。`);
  }
  if (draft.timeoutMs < 1 || draft.timeoutMs > MAX_RUNTIME_MS) {
    errors.push(`超时必须在 1 到 ${MAX_RUNTIME_MS} 毫秒之间。`);
  }
  const memoryBytes = Math.round(draft.memoryMiB * 1024 * 1024);
  if (memoryBytes < 1 || memoryBytes > MAX_MEMORY_BYTES) {
    errors.push("内存预算必须在 0 到 64 MiB 之间。");
  }

  const canSubscribe = draft.capabilities.includes("subscribe-events");
  if (subscriptions.length > 0 && !canSubscribe) {
    errors.push("声明了订阅事件，必须勾选「订阅事件」能力。");
  }
  for (const eventType of subscriptions) {
    if (!isNamespacedId(eventType)) {
      errors.push(`订阅事件「${eventType}」不是有效的命名空间事件类型。`);
    }
  }

  const canInvoke = draft.capabilities.includes("invoke-safe-commands");
  if (commands.length > 0 && !canInvoke) {
    errors.push("声明了安全命令，必须勾选「调用安全命令」能力。");
  }
  for (const command of commands) {
    if (!command.startsWith("safe.") || !isNamespacedId(command)) {
      errors.push(`命令「${command}」必须以 safe. 开头且为命名空间标识。`);
    }
  }

  return errors;
}

export function manifestFromDraft(draft: ManifestDraft): UserProgramManifest {
  return {
    id: draft.id.trim(),
    version: draft.version.trim(),
    capabilities: [...draft.capabilities],
    subscriptions: splitLines(draft.subscriptionsText),
    eventConcurrency: draft.eventConcurrency,
    eventQueueCapacity: draft.eventQueueCapacity,
    commands: splitLines(draft.commandsText),
    timeoutMs: draft.timeoutMs,
    memoryBytes: Math.round(draft.memoryMiB * 1024 * 1024),
  };
}

export function describePolicyReport(report: ProgramPolicyReport): string {
  const count = report.grantedCapabilities.length;
  const caps = count
    ? report.grantedCapabilities.map((cap) => capabilityLabels[cap] ?? cap).join("、")
    : "无能力";
  return `${report.programId} 校验通过 · ${count} 项能力（${caps}）`;
}

export function describeExecutionReceipt(receipt: UserProgramExecutionReceipt): string {
  return `执行完成 · ${receipt.responses.length} 条能力响应 · ${receipt.agentResults.length} 个 Agent 结果`;
}

export function UserProgramAuthoringPanel({ disabled }: { disabled: boolean }) {
  const [draft, setDraft] = useState<ManifestDraft>(defaultManifestDraft);
  const [source, setSource] = useState(defaultProgramSource);
  const [busy, setBusy] = useState<"validate" | "run" | null>(null);
  const [report, setReport] = useState<ProgramPolicyReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const clientErrors = useMemo(() => validateManifestDraft(draft), [draft]);
  const canSubmit = !disabled && busy === null && clientErrors.length === 0;

  const toggleCapability = useCallback((capability: UserCodeCapability) => {
    setReport(null);
    setDraft((current) => {
      const has = current.capabilities.includes(capability);
      return {
        ...current,
        capabilities: has
          ? current.capabilities.filter((item) => item !== capability)
          : [...current.capabilities, capability],
      };
    });
  }, []);

  const patch = useCallback(<K extends keyof ManifestDraft>(key: K, value: ManifestDraft[K]) => {
    setReport(null);
    setDraft((current) => ({ ...current, [key]: value }));
  }, []);

  const handleValidate = useCallback(async () => {
    if (clientErrors.length > 0) {
      setError(clientErrors[0] ?? "清单不合法");
      return;
    }
    setBusy("validate");
    setError(null);
    setNotice(null);
    try {
      const result = await desktopApi.validateUserProgram(manifestFromDraft(draft));
      if (!result) {
        setError("校验未返回结果");
        return;
      }
      setReport(result);
      setNotice(describePolicyReport(result));
    } catch (reason) {
      setReport(null);
      setError(reason instanceof Error ? reason.message : "校验失败");
    } finally {
      setBusy(null);
    }
  }, [clientErrors, draft]);

  const handleRun = useCallback(async () => {
    if (clientErrors.length > 0) {
      setError(clientErrors[0] ?? "清单不合法");
      return;
    }
    setBusy("run");
    setError(null);
    setNotice(null);
    try {
      const receipt = await desktopApi.executeUserProgram(manifestFromDraft(draft), source);
      if (!receipt) {
        setError("执行未返回结果");
        return;
      }
      setNotice(describeExecutionReceipt(receipt));
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "执行失败");
    } finally {
      setBusy(null);
    }
  }, [clientErrors, draft, source]);

  return (
    <section className="skill-lifecycle-panel">
      <header className="skill-lifecycle-head">
        <div><small>USER PROGRAM</small><h3>编写用户程序</h3></div>
        <button
          className="skill-refresh"
          disabled={disabled || busy !== null}
          onClick={() => { setDraft(defaultManifestDraft()); setSource(defaultProgramSource); setReport(null); setError(null); setNotice(null); }}
          type="button"
        >重置</button>
      </header>

      {error ? <p className="skill-error" role="alert">{error}</p> : null}
      {notice ? <p className="skill-notice" role="status">{notice}</p> : null}

      <div className="user-program-authoring">
        <div className="user-program-manifest-grid">
          <label className="user-program-field">
            <span>程序 ID</span>
            <input
              disabled={disabled}
              onChange={(event) => patch("id", event.target.value)}
              placeholder="studio.example.hello"
              type="text"
              value={draft.id}
            />
          </label>
          <label className="user-program-field">
            <span>版本</span>
            <input
              disabled={disabled}
              onChange={(event) => patch("version", event.target.value)}
              placeholder="1.0.0"
              type="text"
              value={draft.version}
            />
          </label>
          <label className="user-program-field">
            <span>超时（毫秒，≤ 30000）</span>
            <input
              disabled={disabled}
              max={MAX_RUNTIME_MS}
              min={1}
              onChange={(event) => patch("timeoutMs", Number(event.target.value))}
              type="number"
              value={draft.timeoutMs}
            />
          </label>
          <label className="user-program-field">
            <span>内存预算（MiB，≤ 64）</span>
            <input
              disabled={disabled}
              max={64}
              min={0}
              onChange={(event) => patch("memoryMiB", Number(event.target.value))}
              step={1}
              type="number"
              value={draft.memoryMiB}
            />
          </label>
          <label className="user-program-field">
            <span>事件并发策略</span>
            <select
              disabled={disabled}
              onChange={(event) => patch("eventConcurrency", event.target.value as ManifestDraft["eventConcurrency"])}
              value={draft.eventConcurrency}
            >
              <option value="serial">串行</option>
              <option value="drop">丢弃新事件</option>
              <option value="cancel-previous">取消前一个</option>
            </select>
          </label>
          <label className="user-program-field">
            <span>事件队列容量（1-64）</span>
            <input
              disabled={disabled}
              max={MAX_EVENT_QUEUE_CAPACITY}
              min={1}
              onChange={(event) => patch("eventQueueCapacity", Number(event.target.value))}
              type="number"
              value={draft.eventQueueCapacity}
            />
          </label>
        </div>

        <fieldset className="user-program-capabilities">
          <legend>能力声明</legend>
          {USER_CODE_CAPABILITIES.map((capability) => (
            <label className="user-program-capability" key={capability}>
              <input
                checked={draft.capabilities.includes(capability)}
                disabled={disabled}
                onChange={() => toggleCapability(capability)}
                type="checkbox"
              />
              <span>
                <strong>{capabilityLabels[capability]}</strong>
                <small>{capabilityHints[capability]}</small>
              </span>
            </label>
          ))}
        </fieldset>

        <label className="user-program-field user-program-field-wide">
          <span>订阅事件（每行一个命名空间事件，如 pet.example.clicked）</span>
          <textarea
            disabled={disabled}
            onChange={(event) => patch("subscriptionsText", event.target.value)}
            placeholder="pet.example.clicked"
            rows={2}
            value={draft.subscriptionsText}
          />
        </label>
        <label className="user-program-field user-program-field-wide">
          <span>安全命令（每行一个 safe.* 命令）</span>
          <textarea
            disabled={disabled}
            onChange={(event) => patch("commandsText", event.target.value)}
            placeholder="safe.example.notify"
            rows={2}
            value={draft.commandsText}
          />
        </label>
        <label className="user-program-field user-program-field-wide">
          <span>程序源码</span>
          <textarea
            className="user-program-source"
            disabled={disabled}
            onChange={(event) => setSource(event.target.value)}
            rows={10}
            spellCheck={false}
            value={source}
          />
        </label>

        {clientErrors.length > 0 ? (
          <ul className="user-program-client-errors">
            {clientErrors.map((message) => (
              <li key={message}>{message}</li>
            ))}
          </ul>
        ) : null}

        {report ? (
          <div className="user-program-report">
            <strong>校验通过</strong>
            <span>超时 {report.timeoutMs} ms · 内存 {(report.memoryBytes / (1024 * 1024)).toFixed(0)} MiB</span>
            <div className="skill-capability-chips">
              {report.grantedCapabilities.length
                ? report.grantedCapabilities.map((capability) => (
                    <span className="skill-capability-chip" key={capability}>{capabilityLabels[capability] ?? capability}</span>
                  ))
                : <span className="skill-capability-chip skill-capability-empty">无需能力</span>}
            </div>
          </div>
        ) : null}

        <div className="skill-card-actions">
          <button
            className="skill-action"
            disabled={!canSubmit}
            onClick={() => void handleValidate()}
            type="button"
          >{busy === "validate" ? "校验中…" : "校验清单"}</button>
          <button
            className="skill-action skill-action-run"
            disabled={!canSubmit}
            onClick={() => void handleRun()}
            type="button"
          >{busy === "run" ? "运行中…" : "运行源码"}</button>
        </div>
        <p className="skill-card-hint">运行会直接在隔离沙箱执行你的源码；如需长期安装并授权，请用 AI Creator 生成程序包。</p>
      </div>
    </section>
  );
}
