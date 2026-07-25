import { useCallback, useEffect, useMemo, useState } from "react";
import {
  desktopApi,
  type UserProgramCatalogEntry,
  type UserProgramEventBatch,
  type UserProgramEventExecutionReceipt,
  type UserProgramEventSessionReceipt,
  type UserProgramEventSessionStatus,
} from "../platform/desktop";

export function subscribingPrograms(
  entries: readonly UserProgramCatalogEntry[],
): UserProgramCatalogEntry[] {
  return entries.filter(
    (entry) =>
      entry.subscriptions.length > 0 &&
      entry.capabilities.includes("subscribe-events"),
  );
}

export function describeSessionReceipt(receipt: UserProgramEventSessionReceipt): string {
  return `${receipt.programId} 事件会话已开启 · 订阅 ${receipt.eventTypes.length} 类事件 · 队列 ${receipt.queueCapacity}`;
}

export function describeSessionStatus(status: UserProgramEventSessionStatus): string {
  const mode = status.automatic ? "自动循环中" : "手动";
  const tail = status.lastError ? ` · 最近错误：${status.lastError}` : "";
  return `${mode} · 已执行 ${status.executed} · 丢弃 ${status.dropped}${tail}`;
}

export function summarizeEventExecution(receipt: UserProgramEventExecutionReceipt): string {
  if (!receipt.execution) {
    return receipt.dropped > 0
      ? `队列为空 · 本轮丢弃 ${receipt.dropped} 个事件`
      : "队列为空，暂无待处理事件";
  }
  return `执行完成 · ${receipt.execution.responses.length} 条能力响应 · ${receipt.execution.agentResults.length} 个 Agent 结果 · 丢弃 ${receipt.dropped}`;
}

export function describeEventBatch(batch: UserProgramEventBatch): string {
  if (batch.events.length === 0) {
    return batch.dropped > 0 ? `队列为空 · 丢弃 ${batch.dropped}` : "队列为空";
  }
  return `排空 ${batch.events.length} 个事件 · 丢弃 ${batch.dropped}`;
}

interface OpenSession {
  receipt: UserProgramEventSessionReceipt;
  status: UserProgramEventSessionStatus | null;
}

export function UserProgramEventSessionPanel({ disabled }: { disabled: boolean }) {
  const [programs, setPrograms] = useState<UserProgramCatalogEntry[]>([]);
  const [session, setSession] = useState<OpenSession | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);

  const eligible = useMemo(() => subscribingPrograms(programs), [programs]);

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

  const guarded = useCallback(async (action: () => Promise<string | null>) => {
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const message = await action();
      if (message) setNotice(message);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "操作失败");
    } finally {
      setBusy(false);
    }
  }, []);

  const openSession = useCallback(
    (programId: string) =>
      guarded(async () => {
        const receipt = await desktopApi.openUserProgramEventSession(programId);
        if (!receipt) return `${programId} 未返回会话回执`;
        setSession({ receipt, status: null });
        return describeSessionReceipt(receipt);
      }),
    [guarded],
  );

  const refreshStatus = useCallback(
    (subscriptionId: string) =>
      guarded(async () => {
        const status = await desktopApi.userProgramEventSessionStatus(subscriptionId);
        setSession((current) =>
          current && current.receipt.subscriptionId === subscriptionId
            ? { ...current, status }
            : current,
        );
        return status ? describeSessionStatus(status) : "会话状态不可用";
      }),
    [guarded],
  );

  const executeNext = useCallback(
    (subscriptionId: string) =>
      guarded(async () => {
        const receipt = await desktopApi.executeNextUserProgramEvent(subscriptionId);
        const status = await desktopApi.userProgramEventSessionStatus(subscriptionId);
        setSession((current) =>
          current && current.receipt.subscriptionId === subscriptionId
            ? { ...current, status }
            : current,
        );
        return receipt ? summarizeEventExecution(receipt) : "未返回执行回执";
      }),
    [guarded],
  );

  const drain = useCallback(
    (subscriptionId: string) =>
      guarded(async () => {
        const batch = await desktopApi.drainUserProgramEvents(subscriptionId);
        return describeEventBatch(batch);
      }),
    [guarded],
  );

  const startLoop = useCallback(
    (subscriptionId: string) =>
      guarded(async () => {
        await desktopApi.startUserProgramEventLoop(subscriptionId);
        const status = await desktopApi.userProgramEventSessionStatus(subscriptionId);
        setSession((current) =>
          current && current.receipt.subscriptionId === subscriptionId
            ? { ...current, status }
            : current,
        );
        return "自动事件循环已启动";
      }),
    [guarded],
  );

  const closeSession = useCallback(
    (subscriptionId: string) =>
      guarded(async () => {
        await desktopApi.closeUserProgramEventSession(subscriptionId);
        setSession(null);
        return "事件会话已关闭";
      }),
    [guarded],
  );

  if (loaded && eligible.length === 0 && !session && !error) {
    return (
      <section className="skill-lifecycle-panel">
        <header className="skill-lifecycle-head">
          <div><small>USER PROGRAM</small><h3>用户程序事件会话</h3></div>
          <button className="skill-refresh" disabled={disabled} onClick={() => void refresh()} type="button">刷新</button>
        </header>
        <div className="skill-empty-state">
          <span>✦</span>
          <p>还没有声明事件订阅的已安装程序。安装一个勾选「订阅事件」且带订阅列表的用户程序后，就能在这里开启事件会话，让它对桌面事件做出实时反应。</p>
        </div>
      </section>
    );
  }

  return (
    <section className="skill-lifecycle-panel">
      <header className="skill-lifecycle-head">
        <div><small>USER PROGRAM</small><h3>用户程序事件会话</h3></div>
        <button className="skill-refresh" disabled={disabled || busy} onClick={() => void refresh()} type="button">刷新</button>
      </header>

      {error ? <p className="skill-error" role="alert">{error}</p> : null}
      {notice ? <p className="skill-notice" role="status">{notice}</p> : null}

      {session ? (
        <div className="user-program-session-card">
          <div className="skill-card-head">
            <div>
              <strong>{session.receipt.programId}</strong>
              <span className="skill-card-meta">v{session.receipt.version} · 订阅 {session.receipt.eventTypes.join("、")} · 队列 {session.receipt.queueCapacity}</span>
            </div>
            <span className="skill-status-badge" data-state={session.status?.automatic ? "authorized" : "permission-required"}>
              {session.status ? (session.status.automatic ? "自动循环中" : "手动模式") : "已开启"}
            </span>
          </div>
          {session.status ? (
            <p className="skill-card-hint" style={{ color: "var(--muted)" }}>{describeSessionStatus(session.status)}</p>
          ) : null}
          <div className="skill-card-actions">
            <button className="skill-action skill-action-run" disabled={disabled || busy} onClick={() => void executeNext(session.receipt.subscriptionId)} type="button">执行下一个事件</button>
            <button className="skill-action" disabled={disabled || busy} onClick={() => void drain(session.receipt.subscriptionId)} type="button">排空队列</button>
            <button className="skill-action" disabled={disabled || busy || session.status?.automatic} onClick={() => void startLoop(session.receipt.subscriptionId)} type="button">启动自动循环</button>
            <button className="skill-action" disabled={disabled || busy} onClick={() => void refreshStatus(session.receipt.subscriptionId)} type="button">刷新状态</button>
            <button className="skill-action skill-action-danger" disabled={disabled || busy} onClick={() => void closeSession(session.receipt.subscriptionId)} type="button">关闭会话</button>
          </div>
        </div>
      ) : (
        <ul className="skill-catalog-list">
          {eligible.map((entry) => (
            <li className="skill-catalog-card" data-healthy={true} key={entry.programId}>
              <div className="skill-card-head">
                <div>
                  <strong>{entry.programId}</strong>
                  <span className="skill-card-meta">v{entry.version} · 订阅 {entry.subscriptions.join("、")}</span>
                </div>
                <span className="skill-status-badge" data-state={entry.permissionGranted ? "authorized" : "permission-required"}>{entry.permissionGranted ? "已授权" : "待授权"}</span>
              </div>
              <div className="skill-card-actions">
                <button
                  className="skill-action skill-action-run"
                  disabled={disabled || busy || !entry.permissionGranted}
                  onClick={() => void openSession(entry.programId)}
                  type="button"
                >开启事件会话</button>
              </div>
              {!entry.permissionGranted ? <p className="skill-card-hint">先在上方管理面板授权后才能开启事件会话。</p> : null}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
