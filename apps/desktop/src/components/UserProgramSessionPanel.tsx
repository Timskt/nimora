import { useCallback, useEffect, useMemo, useState } from "react";
import {
  desktopApi,
  type UserProgramCapabilityRequest,
  type UserProgramCapabilityResponse,
  type UserProgramCatalogEntry,
  type UserProgramManifest,
  type UserProgramSessionReceipt,
} from "../platform/desktop";

const capabilityLabels: Record<string, string> = {
  "read-pet-state": "读取宠物状态",
  "read-profile-state": "读取档案状态",
  "subscribe-events": "订阅事件",
  "invoke-safe-commands": "调用安全命令",
  "invoke-agent-tasks": "调用 Agent 任务",
  "store-local-data": "本地存储",
};

export interface CapabilityAction {
  id: string;
  label: string;
  build: (input: string) => UserProgramCapabilityRequest;
  needsKey: boolean;
  needsValue: boolean;
  placeholder: string;
}

export function sessionManifestFromEntry(entry: UserProgramCatalogEntry): UserProgramManifest {
  return {
    id: entry.programId,
    version: entry.version,
    capabilities: [...entry.capabilities],
    subscriptions: [...entry.subscriptions],
    eventConcurrency: "serial",
    eventQueueCapacity: 8,
    commands: [...entry.commands],
    timeoutMs: entry.timeoutMs,
    memoryBytes: entry.memoryBytes,
  };
}

export function sessionEligiblePrograms(
  entries: readonly UserProgramCatalogEntry[],
): UserProgramCatalogEntry[] {
  return entries.filter(
    (entry) => entry.permissionGranted && entry.capabilities.length > 0,
  );
}

export function availableCapabilityActions(entry: UserProgramCatalogEntry): CapabilityAction[] {
  const actions: CapabilityAction[] = [];
  if (entry.capabilities.includes("read-pet-state")) {
    actions.push({ id: "read-pet-state", label: "读取宠物状态", build: () => ({ type: "readPetState" }), needsKey: false, needsValue: false, placeholder: "" });
  }
  if (entry.capabilities.includes("read-profile-state")) {
    actions.push({ id: "read-profile-state", label: "读取档案状态", build: () => ({ type: "readProfileState" }), needsKey: false, needsValue: false, placeholder: "" });
  }
  if (entry.capabilities.includes("store-local-data")) {
    actions.push({ id: "read-local-data", label: "读取本地键", build: (key) => ({ type: "readLocalData", key }), needsKey: true, needsValue: false, placeholder: "键名，如 counter" });
    actions.push({ id: "write-local-data", label: "写入本地键", build: (raw) => { const [key = "", ...rest] = raw.split("="); return { type: "writeLocalData", key: key.trim(), value: rest.join("=").trim() }; }, needsKey: true, needsValue: false, placeholder: "键=值，如 counter=1" });
    actions.push({ id: "delete-local-data", label: "删除本地键", build: (key) => ({ type: "deleteLocalData", key }), needsKey: true, needsValue: false, placeholder: "键名，如 counter" });
  }
  if (entry.capabilities.includes("invoke-safe-commands")) {
    const command = entry.commands.at(0) ?? "safe.pet.notify";
    actions.push({ id: "invoke-command", label: `调用命令（${command}）`, build: (raw) => ({ type: "invokeCommand", command, arguments: parseArguments(raw) }), needsKey: false, needsValue: true, placeholder: '参数 JSON，如 {"text":"hi"}' });
  }
  return actions;
}

function parseArguments(raw: string): unknown {
  const trimmed = raw.trim();
  if (trimmed.length === 0) return {};
  try {
    return JSON.parse(trimmed);
  } catch {
    return { value: trimmed };
  }
}

export function describeSessionReceipt(receipt: UserProgramSessionReceipt): string {
  const mib = (receipt.memoryBytes / (1024 * 1024)).toFixed(0);
  return `${receipt.programId} 会话已启动 · 超时 ${receipt.timeoutMs} ms · 内存 ${mib} MiB`;
}

export function describeCapabilityResponse(response: UserProgramCapabilityResponse): string {
  switch (response.type) {
    case "petState":
      return `宠物状态 · ${JSON.stringify(response.value)}`;
    case "profileState":
      return `档案状态 · ${JSON.stringify(response.value)}`;
    case "localData":
      return response.value === null ? "本地键为空" : `本地键值 · ${JSON.stringify(response.value)}`;
    case "localDataWritten":
      return "本地键已写入";
    case "localDataDeleted":
      return response.deleted ? "本地键已删除" : "本地键不存在";
    case "commandAccepted":
      return `命令已受理 · ${JSON.stringify(response.value)}`;
    default:
      return "已返回响应";
  }
}

interface LiveSession {
  receipt: UserProgramSessionReceipt;
  entry: UserProgramCatalogEntry;
}

interface TranscriptLine {
  id: string;
  actionLabel: string;
  detail: string;
  ok: boolean;
}

export function UserProgramSessionPanel({ disabled }: { disabled: boolean }) {
  const [programs, setPrograms] = useState<UserProgramCatalogEntry[]>([]);
  const [session, setSession] = useState<LiveSession | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [transcript, setTranscript] = useState<TranscriptLine[]>([]);
  const [actionInput, setActionInput] = useState("");
  const [loaded, setLoaded] = useState(false);

  const eligible = useMemo(() => sessionEligiblePrograms(programs), [programs]);
  const actions = useMemo(
    () => (session ? availableCapabilityActions(session.entry) : []),
    [session],
  );

  const refresh = useCallback(async () => {
    try {
      const catalog = await desktopApi.userProgramCatalog();
      setPrograms(catalog.programs);
      setError(null);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "用户程序目录暂时不可用");
    } finally {
      setLoaded(true);
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  useEffect(() => {
    if (selectedId === null && eligible.length > 0) {
      setSelectedId(eligible[0]?.programId ?? null);
    }
  }, [eligible, selectedId]);

  const startSession = useCallback(async () => {
    const entry = eligible.find((item) => item.programId === selectedId);
    if (!entry) return;
    setBusy(true);
    setError(null);
    try {
      const receipt = await desktopApi.startUserProgram(sessionManifestFromEntry(entry));
      if (!receipt) {
        setError("会话启动未返回结果");
        return;
      }
      setSession({ receipt, entry });
      setTranscript([]);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "会话启动失败");
    } finally {
      setBusy(false);
    }
  }, [eligible, selectedId]);

  const invoke = useCallback(async (action: CapabilityAction) => {
    if (!session) return;
    if ((action.needsKey || action.needsValue) && actionInput.trim().length === 0) {
      setError("请先填写该能力所需的参数");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const response = await desktopApi.invokeUserProgramCapability({
        executionId: session.receipt.executionId,
        traceId: crypto.randomUUID(),
        request: action.build(actionInput),
      });
      const line: TranscriptLine = response
        ? { id: crypto.randomUUID(), actionLabel: action.label, detail: describeCapabilityResponse(response), ok: true }
        : { id: crypto.randomUUID(), actionLabel: action.label, detail: "网关未返回响应", ok: false };
      setTranscript((current) => [line, ...current].slice(0, 20));
    } catch (reason) {
      setTranscript((current) => [
        { id: crypto.randomUUID(), actionLabel: action.label, detail: reason instanceof Error ? reason.message : "能力被拒绝", ok: false },
        ...current,
      ].slice(0, 20));
    } finally {
      setBusy(false);
    }
  }, [session, actionInput]);

  const stopSession = useCallback(async () => {
    if (!session) return;
    setBusy(true);
    setError(null);
    try {
      await desktopApi.stopUserProgram(session.receipt.executionId);
      setSession(null);
      setActionInput("");
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "会话停止失败");
    } finally {
      setBusy(false);
    }
  }, [session]);

  return (
    <section className="skill-lifecycle-panel">
      <header className="skill-lifecycle-head">
        <div><small>USER PROGRAM</small><h3>能力会话控制台</h3></div>
        <button className="skill-refresh" disabled={disabled || busy} onClick={() => void refresh()} type="button">刷新</button>
      </header>

      {error ? <p className="skill-error" role="alert">{error}</p> : null}

      {!session ? (
        loaded && eligible.length === 0 ? (
          <div className="skill-empty-state">
            <span>✦</span>
            <p>还没有已授权且声明能力的用户程序。先在上方安装并授权一个程序，就能在这里开启实时能力会话，逐条探测它被授予的能力。</p>
          </div>
        ) : (
          <div className="user-program-session-start">
            <label className="user-program-field">
              <span>选择已授权的程序</span>
              <select
                disabled={disabled || busy}
                onChange={(event) => setSelectedId(event.target.value)}
                value={selectedId ?? ""}
              >
                {eligible.map((entry) => (
                  <option key={entry.programId} value={entry.programId}>{entry.programId} · v{entry.version}</option>
                ))}
              </select>
            </label>
            <button className="skill-action skill-action-run" disabled={disabled || busy || selectedId === null} onClick={() => void startSession()} type="button">开启会话</button>
            <p className="skill-card-hint">会话在隔离沙箱中持有一个持久执行上下文，可反复调用程序被授予的能力，用完请停止以释放资源。</p>
          </div>
        )
      ) : (
        <div className="user-program-session-live">
          <div className="user-program-session-banner">
            <div>
              <strong>{session.entry.programId}</strong>
              <span className="skill-card-meta">{describeSessionReceipt(session.receipt)}</span>
            </div>
            <button className="skill-action skill-action-danger" disabled={disabled || busy} onClick={() => void stopSession()} type="button">停止会话</button>
          </div>

          <div className="skill-capability-chips">
            {session.entry.capabilities.map((capability) => (
              <span className="skill-capability-chip" key={capability}>{capabilityLabels[capability] ?? capability}</span>
            ))}
          </div>

          <label className="user-program-field user-program-field-wide">
            <span>能力参数（键 / 键=值 / JSON，视能力而定）</span>
            <input
              disabled={disabled || busy}
              onChange={(event) => setActionInput(event.target.value)}
              placeholder='如 counter=1 或 {"text":"hi"}'
              type="text"
              value={actionInput}
            />
          </label>

          <div className="skill-card-actions user-program-action-grid">
            {actions.map((action) => (
              <button className="skill-action" disabled={disabled || busy} key={action.id} onClick={() => void invoke(action)} type="button">{action.label}</button>
            ))}
          </div>

          {transcript.length > 0 ? (
            <ul className="user-program-transcript">
              {transcript.map((line) => (
                <li className={line.ok ? "transcript-ok" : "transcript-error"} key={line.id}>
                  <strong>{line.actionLabel}</strong>
                  <span>{line.detail}</span>
                </li>
              ))}
            </ul>
          ) : (
            <p className="skill-card-hint">尚未调用任何能力。点击上方按钮向这个会话发起一次能力请求。</p>
          )}
        </div>
      )}
    </section>
  );
}
